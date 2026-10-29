//! Physical press ownership for native addon UI input routing.
//! Initial physical press ownership survives capture changes and overlay errors.
//! Focus loss clears tracking but is always forwarded to game bookkeeping.

use std::collections::HashMap;
use winit::{event::MouseButton, keyboard::PhysicalKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Button {
    Key(PhysicalKey),
    Mouse(MouseButton),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Owner {
    Game,
    Addon,
}

#[derive(Default)]
pub(super) struct Capture {
    held: HashMap<Button, Owner>,
}

impl Capture {
    pub fn press(&mut self, button: Button, wants_capture: bool) -> Owner {
        *self.held.entry(button).or_insert(if wants_capture {
            Owner::Addon
        } else {
            Owner::Game
        })
    }
    pub fn release(&mut self, button: Button) -> Owner {
        self.held.remove(&button).unwrap_or(Owner::Game)
    }
    pub fn motion(&self, wants_capture: bool) -> bool {
        wants_capture
            && !self
                .held
                .iter()
                .any(|(button, owner)| matches!(button, Button::Mouse(_)) && *owner == Owner::Game)
    }
    pub fn wheel(&self, wants_capture: bool) -> bool {
        self.motion(wants_capture)
    }
    pub fn text(&self, wants_capture: bool) -> bool {
        wants_capture
    }
    pub fn focus_lost(&mut self) {
        self.held.clear();
    }
    pub fn cursor_left(&mut self) {
        // Native viewer cancels game mouse drags on CursorLeft.
        self.held
            .retain(|button, owner| !matches!(button, Button::Mouse(_)) || *owner != Owner::Game);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::KeyCode;
    fn key(code: KeyCode) -> Button {
        Button::Key(PhysicalKey::Code(code))
    }

    #[test]
    fn forge_capture_cursor_exit_cancels_game_drag_but_retains_addon_releases() {
        let mut capture = Capture::default();
        let left = Button::Mouse(MouseButton::Left);
        let right = Button::Mouse(MouseButton::Right);
        capture.press(left, false);
        capture.press(right, true);
        capture.press(key(KeyCode::KeyA), true);
        capture.cursor_left();
        assert_eq!(
            capture.press(left, true),
            Owner::Addon,
            "the native game already cancelled its old drag"
        );
        assert_eq!(capture.release(right), Owner::Addon);
        assert_eq!(capture.release(key(KeyCode::KeyA)), Owner::Addon);
    }

    #[test]
    fn forge_capture_game_press_keeps_repeats_and_release_when_ui_starts_capturing() {
        let mut capture = Capture::default();
        let a = key(KeyCode::KeyA);
        assert_eq!(capture.press(a, false), Owner::Game);
        for wants in [true, false, true] {
            assert_eq!(capture.press(a, wants), Owner::Game);
        }
        assert_eq!(capture.release(a), Owner::Game);
        assert_eq!(
            capture.press(a, true),
            Owner::Addon,
            "a new physical press may change owner"
        );
    }

    #[test]
    fn forge_capture_addon_press_keeps_repeats_and_release_after_ui_closes_or_fails() {
        let mut capture = Capture::default();
        let a = key(KeyCode::KeyA);
        let b = key(KeyCode::KeyB);
        assert_eq!(capture.press(a, true), Owner::Addon);
        assert_eq!(capture.press(b, false), Owner::Game);
        assert_eq!(capture.press(a, false), Owner::Addon);
        assert_eq!(
            capture.release(a),
            Owner::Addon,
            "addon-owned release must not leak to game"
        );
        assert_eq!(
            capture.release(b),
            Owner::Game,
            "separate game press must still release"
        );
        assert_eq!(
            capture.release(a),
            Owner::Game,
            "unknown release is forwarded for game cleanup"
        );
    }

    #[test]
    fn forge_capture_mouse_drag_wheel_and_motion_preserve_initial_game_owner() {
        let mut capture = Capture::default();
        let left = Button::Mouse(MouseButton::Left);
        let right = Button::Mouse(MouseButton::Right);
        assert_eq!(capture.press(left, false), Owner::Game);
        assert_eq!(capture.press(right, true), Owner::Addon);
        assert!(!capture.motion(true));
        assert!(!capture.wheel(true));
        assert_eq!(capture.release(left), Owner::Game);
        assert!(capture.motion(true));
        assert!(capture.wheel(true));
        assert_eq!(capture.release(right), Owner::Addon);
        assert!(!capture.motion(false));
        assert!(!capture.wheel(false));
        assert!(capture.text(true));
        assert!(!capture.text(false));
    }

    #[test]
    fn forge_capture_focus_loss_clears_all_owners_and_accepts_fresh_presses() {
        let mut capture = Capture::default();
        let a = key(KeyCode::KeyA);
        capture.press(a, true);
        capture.press(Button::Mouse(MouseButton::Left), false);
        capture.focus_lost();
        assert!(capture.held.is_empty());
        assert!(
            capture.motion(true),
            "old game drag must not survive focus loss"
        );
        assert_eq!(capture.press(a, false), Owner::Game);
    }
}
