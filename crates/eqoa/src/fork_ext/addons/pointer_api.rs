//! Window pointer observations and frame-lived addon rectangle claims.
use super::{
    host::Host,
    native::{bind, error},
    native_core::{self, CORE},
};
use mlua::{Result, Table, Value};
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap};
#[derive(Default)]
struct Pointer {
    at: [f32; 2],
    inside: bool,
    claims: BTreeMap<u64, (u64, Vec<[f32; 4]>)>,
    wheels: BTreeMap<u64, f32>,
}
thread_local! {static POINTER:RefCell<Pointer>=RefCell::new(Pointer::default());}
#[cfg(test)]
pub(crate) fn claim_count() -> usize {
    POINTER.with(|p| p.borrow().claims.len())
}
pub(crate) fn position(at: [f32; 2], inside: bool, _size: (u32, u32)) {
    POINTER.with(|p| {
        let mut p = p.borrow_mut();
        p.at = at;
        p.inside = inside;
    });
}
pub(crate) fn release_vm(vm: u64) {
    POINTER.with(|p| {
        let mut p = p.borrow_mut();
        p.claims.remove(&vm);
        p.wheels.remove(&vm);
    });
}
pub(crate) fn take_wheel() -> f32 {
    POINTER.with(|p| {
        let mut p = p.borrow_mut();
        let sum = p.wheels.values().sum::<f32>().clamp(-20.0, 20.0);
        p.wheels.clear();
        sum
    })
}
fn claimed() -> bool {
    let frame = CORE.with(|c| c.borrow().frame);
    POINTER.with(|p| {
        let p = p.borrow();
        p.inside
            && p.claims.values().any(|(f, rs)| {
                frame.saturating_sub(*f) <= 1
                    && rs.iter().any(|r| {
                        p.at[0] >= r[0]
                            && p.at[0] <= r[0] + r[2]
                            && p.at[1] >= r[1]
                            && p.at[1] <= r[1] + r[3]
                    })
            })
    })
}
pub(crate) fn point_in_vm_claim(vm: u64, x: f64, y: f64) -> bool {
    let frame = CORE.with(|c| c.borrow().frame);
    POINTER.with(|p| {
        let p = p.borrow();
        p.inside
            && p.claims.get(&vm).is_some_and(|(f, rs)| {
                frame.saturating_sub(*f) <= 1
                    && rs.iter().any(|r| {
                        x >= r[0] as f64
                            && x <= (r[0] + r[2]) as f64
                            && y >= r[1] as f64
                            && y <= (r[1] + r[3]) as f64
                    })
            })
    })
}
fn emit(name: &str, button: Option<&str>, steps: Option<f32>) {
    if !native_core::capture_requested() {
        return;
    }
    // the modifiers held at this press (a click that selects with Ctrl or Shift), as the window last said
    let (ctrl, alt, shift) = super::input::modifiers();
    let data = POINTER.with(|p| {
        let p = p.borrow();
        json!({"x":p.at[0],"y":p.at[1],"inside_window":p.inside,"button":button,"steps":steps,"ctrl":ctrl,"shift":shift,"alt":alt})
    });
    native_core::emit(name, data);
}
pub(crate) fn observe(event: &winit::event::WindowEvent, size: (u32, u32)) -> bool {
    use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent as E};
    match event {
        E::CursorMoved { position: p, .. } => position([p.x as f32, p.y as f32], true, size),
        E::CursorLeft { .. } | E::Focused(false) => {
            POINTER.with(|p| {
                let mut p = p.borrow_mut();
                p.inside = false;
                p.claims.clear();
                p.wheels.clear();
            });
        }
        E::MouseInput {
            state: ElementState::Pressed,
            button,
            ..
        } => emit(
            "pointer.pressed",
            Some(match button {
                MouseButton::Left => "left",
                MouseButton::Right => "right",
                MouseButton::Middle => "middle",
                _ => "other",
            }),
            None,
        ),
        E::MouseWheel { delta, .. } => {
            let n = match delta {
                MouseScrollDelta::LineDelta(_, y) => *y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            };
            emit("pointer.wheel", None, Some(n));
        }
        _ => {}
    }
    !crate::hudlayout::unlocked() && claimed()
}
pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    bind(
        lua,
        "input",
        "pointer",
        lua.create_function(|lua, ()| {
            let data = POINTER.with(|p| {
                let p = p.borrow();
                json!({"x":p.at[0],"y":p.at[1],"inside_window":p.inside})
            });
            lua.pack_multi((
                super::native::record_cached(
                    lua,
                    "eqoa.input.pointer",
                    "eqoa.input.Pointer",
                    &data,
                )?,
                Value::Nil,
            ))
        })?,
    )?;
    bind(
        lua,
        "input",
        "claim_pointer",
        lua.create_function(|lua, t: Table| {
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(
                    lua,
                    "eqoa.input.claim_pointer",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let r = [
                t.get::<f32>("x")?,
                t.get("y")?,
                t.get::<Option<f32>>("width")?
                    .or(t.get("w")?)
                    .unwrap_or(0.0),
                t.get::<Option<f32>>("height")?
                    .or(t.get("h")?)
                    .unwrap_or(0.0),
            ];
            if !r.iter().all(|v| v.is_finite() && v.abs() <= 32768.0) || r[2] <= 0.0 || r[3] <= 0.0
            {
                return error(
                    lua,
                    "eqoa.input.claim_pointer",
                    "invalid_argument",
                    "finite positive rectangle required",
                );
            }
            let vm = super::native_events::bus(lua).borrow().vm;
            let frame = CORE.with(|c| c.borrow().frame);
            let ok = POINTER.with(|p| {
                let mut p = p.borrow_mut();
                let (f, rs) = p.claims.entry(vm).or_insert_with(|| (frame, Vec::new()));
                if *f != frame {
                    *f = frame;
                    rs.clear();
                }
                if rs.len() >= 32 {
                    return false;
                }
                rs.push(r);
                true
            });
            if !ok {
                return error(
                    lua,
                    "eqoa.input.claim_pointer",
                    "quota",
                    "at most 32 rectangles per render frame",
                );
            }
            lua.pack_multi((true, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "input",
        "minimap_wheel",
        lua.create_function(|lua, n: f32| {
            if !n.is_finite() || n.abs() > 20.0 {
                return error(
                    lua,
                    "eqoa.input.minimap_wheel",
                    "invalid_argument",
                    "wheel steps must be finite and at most 20",
                );
            }
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(
                    lua,
                    "eqoa.input.minimap_wheel",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let vm = super::native_events::bus(lua).borrow().vm;
            POINTER.with(|p| {
                let mut p = p.borrow_mut();
                let v = p.wheels.entry(vm).or_default();
                *v = (*v + n).clamp(-20.0, 20.0);
            });
            lua.pack_multi((true, Value::Nil))
        })?,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round4_map_text_capture_owns_keys_and_releases_but_unclaimed_keys_pass() {
        let mut p = crate::play::Play::bundled_ui_fixture();
        p.map.open = true;
        let h = Host::new().unwrap();
        h.snapshot.set(p.forge_snapshot(None));
        h.execute("assert(eqoa.input.capture_text(true))").unwrap();
        let mut capture = super::super::capture::Capture::default();
        use super::super::capture::{Button, Owner};
        for (code, key) in [
            (b'P', winit::keyboard::KeyCode::KeyP),
            (b'M', winit::keyboard::KeyCode::KeyM),
            (b'O', winit::keyboard::KeyCode::KeyO),
            (b'G', winit::keyboard::KeyCode::KeyG),
            (13, winit::keyboard::KeyCode::Enter),
        ] {
            let button = Button::Key(winit::keyboard::PhysicalKey::Code(key));
            assert_eq!(
                capture.press(
                    button,
                    crate::fork_ext::addons::input::wants_key_capture(
                        false,
                        Some(code),
                        false,
                        crate::fork_ext::is_plain_play(Some(&mut p))
                    )
                ),
                Owner::Addon
            );
            h.execute("assert(eqoa.input.capture_text(false))").unwrap();
            assert_eq!(
                capture.release(button),
                Owner::Addon,
                "release follows text capture owner"
            );
            assert_eq!(
                capture.press(
                    button,
                    crate::fork_ext::addons::input::wants_key_capture(
                        false,
                        Some(code),
                        false,
                        true
                    )
                ),
                Owner::Game
            );
            capture.release(button);
            h.execute("assert(eqoa.input.capture_text(true))").unwrap();
        }
    }
    #[test]
    fn round4_wide_pointer_layout_and_observed_press_share_coordinates() {
        use winit::event::{
            DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
        };
        let mut p = crate::play::Play::bundled_ui_fixture();
        let h = Host::new().unwrap();
        super::super::hud_layout::set_window_size(1920, 1080);
        h.snapshot.set(p.forge_snapshot(None));
        h.execute("r=eqoa.hud.layout().minimap;assert(eqoa.input.claim_pointer(r))")
            .unwrap();
        let rect = super::super::hud_layout::hud_layout_json((1920, 1080));
        let r = &rect["minimap"];
        let at = [
            r["x"].as_f64().unwrap() + 10.,
            r["y"].as_f64().unwrap() + 10.,
        ];
        let device_id = DeviceId::dummy();
        observe(
            &WindowEvent::CursorMoved {
                device_id,
                position: winit::dpi::PhysicalPosition::new(at[0], at[1]),
            },
            (1920, 1080),
        );
        h.execute("local p=eqoa.input.pointer();assert(math.abs(p.x-r.x-10)<0.001 and math.abs(p.y-r.y-10)<0.001)").unwrap();
        let mut capture = super::super::capture::Capture::default();
        let press = WindowEvent::MouseInput {
            device_id,
            state: ElementState::Pressed,
            button: MouseButton::Left,
        };
        assert!(observe(&press, (1920, 1080)));
        assert_eq!(
            capture.press(
                super::super::capture::Button::Mouse(MouseButton::Left),
                observe(&press, (1920, 1080))
            ),
            super::super::capture::Owner::Addon
        );
        assert!(observe(
            &WindowEvent::MouseWheel {
                device_id,
                delta: MouseScrollDelta::LineDelta(0., 1.),
                phase: TouchPhase::Moved
            },
            (1920, 1080)
        ));
        crate::hudlayout::set_unlocked(true);
        assert!(!observe(&press, (1920, 1080)), "Move HUD owns its drag");
        crate::hudlayout::set_unlocked(false);
        observe(
            &WindowEvent::CursorMoved {
                device_id,
                position: winit::dpi::PhysicalPosition::new(0., 0.),
            },
            (1920, 1080),
        );
        assert!(
            !observe(&press, (1920, 1080)),
            "unclaimed position passes to game"
        );
        assert_eq!(
            capture.release(super::super::capture::Button::Mouse(MouseButton::Left)),
            super::super::capture::Owner::Addon
        );
        observe(&WindowEvent::Focused(false), (1920, 1080));
        assert!(!observe(&press, (1920, 1080)));
        capture.focus_lost();
        assert_eq!(
            capture.release(super::super::capture::Button::Mouse(MouseButton::Left)),
            super::super::capture::Owner::Game
        );
        super::super::hud_layout::set_window_size(640, 448);
    }
    #[test]
    fn round4_pointer_claim_routes_window_coordinates_and_expires() {
        CORE.with(|c| {
            *c.borrow_mut() = super::super::native_core::Core::default();
            c.borrow_mut().view.in_world = true;
        });
        let h = Host::new().unwrap();
        position([50.0, 60.0], true, (640, 448));
        h.execute("local p=eqoa.input.pointer();assert(math.abs(p.x-50)<0.001 and math.abs(p.y-60)<0.001 and p.inside_window);assert(eqoa.input.claim_pointer{x=40,y=50,width=30,height=30})").unwrap();
        assert!(claimed());
        position([10.0, 10.0], true, (640, 448));
        assert!(!claimed());
        position([50.0, 60.0], true, (640, 448));
        CORE.with(|c| c.borrow_mut().frame += 2);
        assert!(!claimed());
        h.execute("assert(eqoa.input.minimap_wheel(1))").unwrap();
        assert_eq!(take_wheel(), 1.0);
        assert_eq!(take_wheel(), 0.0);
        release_vm(super::super::native_events::bus(&h.lua).borrow().vm);
    }
}
