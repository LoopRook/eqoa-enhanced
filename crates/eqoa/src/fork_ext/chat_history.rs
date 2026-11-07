// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see LICENSE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Chat-history scrolling, published to addons as `eqoa.ChatHistory()`.
//!
//! Stock `VIWndGame` mode 33 (R2 held, or the keyboard's C) gives the chat
//! window the up and down keys; `VIWndChat` steps a line at once and repeats
//! after 0.45 s, then every 0.05 s while held. The stock window still consumes
//! those events when an addon draws its own chat (the fork's HUD wrapper keeps
//! its state advancing under a null raster), so this module taps the same key
//! and repeat events and keeps a running count of scroll lines. Addons read
//! the count through the snapshot; the first call after a frame returns the
//! lines scrolled since the addon's previous call.

use std::cell::Cell;

/// What an addon frame sees: whether chat history is being scrolled by an
/// addon-owned chat window, and the running scroll-line count (positive is
/// back/up, negative is forward/down).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChatHistorySnapshot {
    pub active: bool,
    pub total: i32,
}

/// Most lines one API call reports. A host that missed frames gets a bounded
/// jump instead of the whole backlog.
pub(crate) const MAX_STEPS_PER_CALL: i32 = 8;

thread_local! {
    /// The scroll key the stock window is repeating (19, 20, 138 or 139).
    static HELD: Cell<i32> = const { Cell::new(0) };
    /// Seconds until the next repeat, as `WndChat::draw` keeps it.
    static REPEAT: Cell<f32> = const { Cell::new(0.0) };
    /// Signed lines scrolled since the addon runtime started. Monotonic, so a
    /// snapshot can be built more than once a frame without losing a step.
    static TOTAL: Cell<i32> = const { Cell::new(0) };
}

fn step(back: bool) {
    TOTAL.with(|t| t.set(t.get().saturating_add(if back { 1 } else { -1 })));
}

/// Taps the stock chat's scroll key press or release (`WndChat::key_down` and
/// `key_up`). Called by `Play::chat_scroll_key`, which only runs in mode 33.
pub(crate) fn key(key: i32, down: bool) {
    if !down {
        // WndChat clears its held key on any release.
        HELD.with(|h| h.set(0));
        return;
    }
    // WndChat clears the held key before looking at the new one.
    HELD.with(|h| h.set(0));
    let back = match key {
        19 | 138 => true,
        20 | 139 => false,
        _ => return,
    };
    HELD.with(|h| h.set(key));
    REPEAT.with(|r| r.set(0.45));
    step(back);
}

/// Taps the stock chat's repeat clock (`WndChat::draw`): the first repeat
/// after 0.45 s, then every 0.05 s while the key is held. Called once a frame
/// by the fork's chat draw wrapper with the frame's `dt`.
pub(crate) fn tick(dt: f32) {
    let held = HELD.with(|h| h.get());
    if !matches!(held, 19 | 20 | 138 | 139) {
        return;
    }
    let left = REPEAT.with(|r| r.get()) - dt;
    if left > 0.0 {
        REPEAT.with(|r| r.set(left));
        return;
    }
    REPEAT.with(|r| r.set(0.05));
    step(matches!(held, 19 | 138));
}

/// The running count, for a host to seed its baseline when it loads.
pub(crate) fn total() -> i32 {
    TOTAL.with(|t| t.get())
}

/// Snapshot for this frame. `active` is mode 33 with an addon-owned chat.
/// Leaving that mode (dialog, zoning, death, or the claim) may never send a
/// key-up, so the held repeat is cleared here.
pub(crate) fn snapshot(active: bool) -> ChatHistorySnapshot {
    if !active {
        HELD.with(|h| h.set(0));
        REPEAT.with(|r| r.set(0.0));
    }
    ChatHistorySnapshot {
        active,
        total: total(),
    }
}

/// For tests: forget the held key and the count.
#[cfg(test)]
pub(crate) fn reset() {
    HELD.with(|h| h.set(0));
    REPEAT.with(|r| r.set(0.0));
    TOTAL.with(|t| t.set(0));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key steps at once, repeats while held, and stops on release; down is
    /// the opposite sign. The negative control is a non-scroll key.
    #[test]
    fn chat_history_steps_once_then_repeats_while_held() {
        reset();
        key(19, true);
        assert_eq!(total(), 1, "up steps back at once");
        tick(0.40);
        assert_eq!(total(), 1, "no repeat before 0.45 s");
        tick(0.05);
        assert_eq!(total(), 2, "first repeat after 0.45 s");
        tick(0.05);
        assert_eq!(total(), 3, "then every 0.05 s");
        key(19, false);
        tick(1.0);
        assert_eq!(total(), 3, "release stops the repeat");

        key(20, true);
        assert_eq!(total(), 2, "down steps forward");
        key(20, false);

        key(146, true);
        tick(1.0);
        assert_eq!(total(), 2, "a non-scroll key neither steps nor repeats");
        reset();
    }

    /// Leaving mode 33 without a key-up (a dialog, zoning, death) clears the
    /// held repeat. The control holds the same key while active and repeats.
    #[test]
    fn chat_history_stops_repeating_when_the_mode_leaves() {
        reset();
        key(19, true);
        assert_eq!(total(), 1, "up steps at once");
        snapshot(false);
        tick(1.0);
        assert_eq!(total(), 1, "no repeat after the mode leaves 33");

        key(19, true);
        assert_eq!(total(), 2, "control: a held key steps at once while active");
        tick(1.0);
        assert_eq!(total(), 3, "control: and keeps repeating while active");
        key(19, false);
        reset();
    }
}
