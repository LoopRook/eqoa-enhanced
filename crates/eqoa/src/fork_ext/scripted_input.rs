// SPDX-License-Identifier: AGPL-3.0-or-later
//! Play test inputs through the native window's event routes.
use super::{ActiveEventLoop, App, Key, KeyIn, NamedKey, WindowEvent};
use std::{cell::RefCell, collections::VecDeque};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase},
    keyboard::ModifiersState,
};

thread_local! {
    static REPLAY: RefCell<Replay> = RefCell::new(Replay::parse(
        std::env::var("EQOA_MOUSE").ok().as_deref(),
        std::env::var("EQOA_MOD_KEYS").ok().as_deref(),
    ));
}

#[derive(Debug, PartialEq)]
enum Point {
    Pixels(f64, f64),
    Creature(String),
    Link(usize),
}

#[derive(Debug, PartialEq)]
enum Mouse {
    Click(Point, MouseButton),
    Button(MouseButton, ElementState),
    Move(f64, f64),
    Drag(f64, f64),
    Wheel(f32),
    Ctrl(bool),
    Shift(bool),
}

#[derive(Debug, PartialEq)]
enum Input {
    Mouse(Mouse),
    Key(usize, KeyIn),
}

struct Replay(VecDeque<(i64, Input)>);

fn steps(script: &str) -> impl Iterator<Item = (i64, &str)> {
    script.split(';').filter_map(|step| {
        let (action, time) = step.trim().rsplit_once('@')?;
        let time: i64 = time.trim().parse().ok()?;
        (0..=i64::MAX / 2)
            .contains(&time)
            .then_some((time, action.trim_start()))
    })
}

impl Replay {
    fn parse(mouse: Option<&str>, keys: Option<&str>) -> Self {
        let mut inputs = Vec::new();
        for (at, action) in steps(mouse.unwrap_or_default()) {
            if let Some(mouse) = Mouse::parse(action.trim()) {
                inputs.push((at, Input::Mouse(mouse)));
            }
        }
        for (at, action) in steps(keys.unwrap_or_default()) {
            if let Some(text) = action.strip_prefix("text ") {
                for (index, ch) in text.chars().enumerate() {
                    let logical = if ch == ' ' {
                        Key::Named(NamedKey::Space)
                    } else {
                        Key::Character(ch.to_string().into())
                    };
                    add_key(&mut inputs, at + index as i64 * 80, 40, logical, Some(ch));
                }
            } else {
                let name = action.trim();
                let key = crate::binds::key_of(if name == "esc" { "escape" } else { name });
                let text = match &key {
                    Key::Named(NamedKey::Space) => Some(' '),
                    Key::Character(s) if s.chars().count() == 1 => s.chars().next(),
                    Key::Character(_) => continue,
                    _ => None,
                };
                add_key(&mut inputs, at, 100, key, text);
            }
        }
        inputs.sort_by_key(|(at, _)| *at);
        Self(inputs.into())
    }

    fn next(&mut self, now: i64) -> Option<Input> {
        (self.0.front()?.0 <= now).then(|| self.0.pop_front().unwrap().1)
    }
}

fn add_key(inputs: &mut Vec<(i64, Input)>, at: i64, hold: i64, key: Key, text: Option<char>) {
    let id = inputs.len();
    for (when, state) in [
        (at, ElementState::Pressed),
        (at + hold, ElementState::Released),
    ] {
        inputs.push((
            when,
            Input::Key(
                id,
                KeyIn {
                    logical_key: key.clone(),
                    state,
                    repeat: false,
                    base: None,
                    text: if state.is_pressed() {
                        text.map(|ch| ch.to_string().into())
                    } else {
                        None
                    },
                },
            ),
        ));
    }
}

impl Mouse {
    fn parse(action: &str) -> Option<Self> {
        let (verb, args) = action.split_once(' ').unwrap_or((action, ""));
        let args = args.trim();
        let pair = || {
            let (x, y) = args.split_once(' ')?;
            let x: f64 = x.parse().ok()?;
            let y: f64 = y.trim().parse().ok()?;
            (x.is_finite() && y.is_finite()).then_some((x, y))
        };
        Some(match verb {
            "click" | "rclick" => {
                let point = if let Some(name) = args.strip_prefix('@') {
                    Point::Creature(name.to_owned())
                } else {
                    let (x, y) = pair()?;
                    Point::Pixels(x, y)
                };
                Self::Click(
                    point,
                    if verb == "click" {
                        MouseButton::Left
                    } else {
                        MouseButton::Right
                    },
                )
            }
            "linkclick" => Self::Click(
                Point::Link(if args.is_empty() {
                    0
                } else {
                    args.parse().ok()?
                }),
                MouseButton::Left,
            ),
            "press" | "release" => Self::Button(
                match args {
                    "l" => MouseButton::Left,
                    "r" => MouseButton::Right,
                    _ => return None,
                },
                if verb == "press" {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                },
            ),
            "move" => {
                let (x, y) = pair()?;
                Self::Move(x, y)
            }
            "drag" => {
                let (x, y) = pair()?;
                Self::Drag(x, y)
            }
            "wheel" => {
                let lines: f32 = args.parse().ok()?;
                if !lines.is_finite() {
                    return None;
                }
                Self::Wheel(lines)
            }
            "ctrl" => Self::Ctrl(match args {
                "down" => true,
                "up" => false,
                _ => return None,
            }),
            "shift" => Self::Shift(match args {
                "down" => true,
                "up" => false,
                _ => return None,
            }),
            _ => return None,
        })
    }

    fn dispatch(self, app: &mut App, el: &ActiveEventLoop) {
        let Some(gfx) = &app.gfx else { return };
        let size = (gfx.config.width, gfx.config.height);
        let window = gfx.window.id();
        let device_id = DeviceId::dummy();
        let motion = |(x, y)| WindowEvent::CursorMoved {
            device_id,
            position: PhysicalPosition::new(x, y),
        };
        let button_event = |button, state| WindowEvent::MouseInput {
            device_id,
            button,
            state,
        };
        let cursor = app.cursor;
        let mut send = |event| app.window_event(el, window, event);
        match self {
            Self::Click(point, button) => {
                let at = match &point {
                    Point::Pixels(x, y) => Some((*x, *y)),
                    Point::Creature(name) => app
                        .play
                        .as_ref()
                        .zip(app.last_view)
                        .and_then(|(p, vp)| p.screen_of(name, size, vp)),
                    Point::Link(k) => app.play.as_ref().and_then(|p| p.link_on_screen(size, *k)),
                };
                if let Some((x, y)) = at {
                    crate::say(&format!("script: mouse click {button:?} at {x:.0} {y:.0}"));
                    app.window_event(el, window, motion((x, y)));
                    app.window_event(el, window, button_event(button, ElementState::Pressed));
                    app.window_event(el, window, button_event(button, ElementState::Released));
                } else {
                    crate::say(&format!("script: mouse target {point:?} not on screen"));
                }
            }
            Self::Button(button, state) => send(button_event(button, state)),
            Self::Move(dx, dy) => {
                app.device_event(el, device_id, DeviceEvent::MouseMotion { delta: (dx, dy) });
                app.window_event(el, window, motion((cursor.0 + dx, cursor.1 + dy)));
            }
            Self::Drag(dx, dy) => {
                let center = (f64::from(size.0) / 2., f64::from(size.1) / 2.);
                send(motion(center));
                send(button_event(MouseButton::Left, ElementState::Pressed));
                app.device_event(el, device_id, DeviceEvent::MouseMotion { delta: (dx, dy) });
                app.window_event(el, window, motion((center.0 + dx, center.1 + dy)));
                app.window_event(
                    el,
                    window,
                    button_event(MouseButton::Left, ElementState::Released),
                );
            }
            Self::Wheel(lines) => send(WindowEvent::MouseWheel {
                device_id,
                delta: MouseScrollDelta::LineDelta(0., lines),
                phase: TouchPhase::Moved,
            }),
            Self::Ctrl(down) => {
                let mut modifiers = ModifiersState::empty();
                modifiers.set(ModifiersState::CONTROL, down);
                modifiers.set(ModifiersState::ALT, app.mods.1);
                modifiers.set(ModifiersState::SHIFT, app.mods.2);
                app.window_event(el, window, WindowEvent::ModifiersChanged(modifiers.into()));
            }
            Self::Shift(down) => {
                let mut modifiers = ModifiersState::empty();
                modifiers.set(ModifiersState::CONTROL, app.mods.0);
                modifiers.set(ModifiersState::ALT, app.mods.1);
                modifiers.set(ModifiersState::SHIFT, down);
                app.window_event(el, window, WindowEvent::ModifiersChanged(modifiers.into()));
            }
        }
    }
}

pub(super) fn step(app: &mut App, el: &ActiveEventLoop) {
    if app.gfx.is_none() || REPLAY.with(|replay| replay.borrow().0.is_empty()) {
        return;
    }
    let now = app.clock.elapsed().as_millis() as i64;
    while let Some(input) = REPLAY.with(|replay| replay.borrow_mut().next(now)) {
        match input {
            Input::Mouse(mouse) => mouse.dispatch(app, el),
            Input::Key(id, key) => {
                if key.state.is_pressed() {
                    crate::say("script: mod key pressed");
                }
                if super::report_key(
                    app,
                    el,
                    &key.logical_key,
                    key.text.as_deref(),
                    key.state,
                    key.repeat,
                ) {
                    continue;
                }
                // The window-key rule (`addon_event`): a native window that
                // owns the keyboard gets the key; the addons still see its
                // release so a key held across the change lets go.
                let native = super::native_owns_keys(app);
                let plain_play = crate::fork_ext::is_plain_play(app.play.as_mut());
                let consumed = (!native || !key.state.is_pressed())
                    && app.gfx.as_mut().is_some_and(|g| {
                        g.forge_runtime.scripted_key(
                            id,
                            &key.logical_key,
                            key.text.as_deref(),
                            key.state,
                            plain_play,
                        )
                    });
                if !consumed || native {
                    app.key_input(el, key, false);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_input_absent_or_invalid_options_queue_no_events() {
        assert!(Replay::parse(None, None).next(i64::MAX).is_none());
        assert!(
            Replay::parse(
                Some("click NaN 0@0;wheel inf@2;press x@3"),
                Some("garbage@3;enter@-1")
            )
            .next(i64::MAX)
            .is_none()
        );
    }

    #[test]
    fn scripted_input_shift_steps_parse_and_a_bad_one_is_refused() {
        let mut replay = Replay::parse(Some("shift down@10;shift up@20;shift sideways@30;ctrl down@40"), None);
        assert_eq!(replay.next(10), Some(Input::Mouse(Mouse::Shift(true))));
        assert_eq!(replay.next(20), Some(Input::Mouse(Mouse::Shift(false))));
        assert_eq!(replay.next(30), Some(Input::Mouse(Mouse::Ctrl(true))), "the bad step is dropped, not queued");
        assert_eq!(replay.next(i64::MAX), None);
    }

    #[test]
    fn scripted_input_interleaves_pointer_with_key_edges() {
        let mut replay = Replay::parse(
            Some("rclick @fire bat@75;click 12 27@170"),
            Some("b@100;text x y@400"),
        );
        assert_eq!(replay.next(74), None);
        assert_eq!(
            replay.next(75),
            Some(Input::Mouse(Mouse::Click(
                Point::Creature("fire bat".into()),
                MouseButton::Right
            )))
        );
        let Some(Input::Key(id, pressed)) = replay.next(100) else {
            panic!("missing key press")
        };
        assert_eq!(pressed.text.as_deref(), Some("b"));
        assert_eq!(
            replay.next(170),
            Some(Input::Mouse(Mouse::Click(
                Point::Pixels(12., 27.),
                MouseButton::Left
            )))
        );
        assert_eq!(replay.next(199), None);
        let Some(Input::Key(released_id, released)) = replay.next(200) else {
            panic!("missing release")
        };
        assert_eq!(
            (released_id, released.state, released.text),
            (id, ElementState::Released, None)
        );
        assert_eq!(replay.next(399), None);
        let Some(Input::Key(_, typed)) = replay.next(400) else {
            panic!("missing typed letter")
        };
        assert_eq!(typed.text.as_deref(), Some("x"));
    }
}
