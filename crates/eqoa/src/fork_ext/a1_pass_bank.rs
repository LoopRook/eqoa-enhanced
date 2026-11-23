// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1 `pass` for the bank window (`eqoa.windows.pass`, `addons::window_sessions`): a child of `bankplay` (one
//! declaration line there), because the flag that makes the game open the bank window on the next step is private to
//! that module. See `a1_pass_npcw.rs`.
use super::Play;

/// The bank window opens, as 0x124D's handler leaves it when no mod holds the window.
pub(in crate::play) fn show_bank(play: &mut Play) {
    play.cues.push(crate::sound::Cue::Ui(15));
    play.bank.open = true;
    play.bank.state = 0;
    play.mode = 11;
}
