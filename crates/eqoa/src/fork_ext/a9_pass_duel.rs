// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A9 for the duel windows (`a9_play.rs`): a child of `duelplay` (one declaration line there), because
//! the window's focus flag (`activate`) and its key queue are private to that module.
use super::Play;

/// The duel window (the invitation or the wager) is put away before it draws: its focus is not given, its keys are
/// dropped, the window closed. The caller sets the mode.
pub(in crate::play) fn hide_window(play: &mut Play) {
    play.duel.activate = false;
    play.duel.keys.clear();
    play.duel.wnd.close();
}

/// The window of mode 43 or 44 opens again (a pass), with the window's focus given at the next frame.
pub(in crate::play) fn show_window(play: &mut Play, mode: i32) {
    play.mode = mode;
    play.duel.activate = true;
}
