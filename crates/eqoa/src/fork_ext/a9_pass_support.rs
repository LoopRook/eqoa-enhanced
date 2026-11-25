// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A9 for the blacksmith (`a9_play.rs`): a child of `support` (one declaration line there), because the smith's
//! id and the flag that makes the game open its window on the next step are private to that module.
use super::Play;

/// The smith's id from 0x0061 (`u32::MAX` once the window is closed).
pub(in crate::play) fn smith_id(play: &Play) -> u32 {
    play.support.smith_id
}

/// The blacksmith is put away before it opens: the opening flag (which would fix the chat box) is dropped. The
/// caller sets the mode.
pub(in crate::play) fn hide_smith(play: &mut Play) {
    play.support.opening = None;
    play.support.smith.visible = false;
}

/// The game's own blacksmith window opens (a pass), as 0x0061's handler leaves it.
pub(in crate::play) fn show_smith(play: &mut Play) {
    play.cues.push(crate::sound::Cue::Ui(15));
    play.mode = 19;
    play.support.opening = Some(19);
}

/// `SmithOut::Close` without the window: the smith forgotten and 0x0063 sent (the mode is already 0).
pub(in crate::play) fn close_smith(play: &mut Play) {
    play.support.smith_id = u32::MAX;
    play.session.send_opcode(0x0063);
}
