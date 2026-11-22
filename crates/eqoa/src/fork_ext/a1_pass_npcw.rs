// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1 `pass` for the corpse and merchant windows (`eqoa.windows.pass`, `addons::window_sessions`): a child of
//! `npcw` (one declaration line there), because the flags that make the game open its window on the next step are
//! private to that module. A hidden session's window was never opened: the hooks at 0x0018 and 0x00B7 put the mode
//! and the flag back. A pass puts them where the server's reply would have put them, and the game's own window opens
//! with the list the server sent.
use super::Play;

/// The corpse window opens, as 0x0018's handler leaves it when no mod holds the window.
pub(in crate::play) fn show_loot(play: &mut Play) {
    play.npcw.loot_open = true;
    play.mode = 1;
}

/// The merchant window opens, as 0x00B7's handler leaves it when no mod holds the window.
pub(in crate::play) fn show_merchant(play: &mut Play) {
    play.cues.push(crate::sound::Cue::Ui(15));
    play.npcw.shop.pending = false;
    play.npcw.merchant_open = true;
    play.mode = 10;
}
