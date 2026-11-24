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

// ---- Round A9: the trade window held by a mod (`a9_play.rs`). The trade window's flags and its private
// helpers (`trade_cancel`, `trade_send`, `chat_mode`, `coin`) are `npcw`'s, so they are reached from here. ----

/// The trade window's two accept marks, as the game's own window shows them.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn trade_marks(play: &Play) -> (bool, bool) {
    (play.npcw.trade.i_accepted, play.npcw.trade.they_accepted)
}

/// The window a trade request or an open trade would show is put away before it draws (`mode` is the caller's):
/// its opening flags dropped, the window hidden, and the chat box let go of as `trade_done` does.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn hide_trade_window(play: &mut Play, chat_fixed: bool) {
    play.npcw.trade_ask = false;
    play.npcw.trade_start = false;
    play.npcw.trade.hide();
    if chat_fixed {
        play.chat_mode(0);
    }
}

/// The request's own window opens (a pass of a held `trade_ask`): as `trade_offered` leaves it.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn show_trade_ask(play: &mut Play) {
    play.mode = 16;
    play.npcw.trade_ask = true;
}

/// The open trade's window opens (a pass of a held `trade`): as `Trade02` leaves it, the offers kept.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn show_trade(play: &mut Play) {
    play.mode = 15;
    play.chat_mode(1);
    play.npcw.trade_start = true;
}

/// The player's own accept, as the window's accept key marks it.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn mark_my_accept(play: &mut Play) {
    play.npcw.trade.i_accepted = true;
}

/// The partner's accept, put back after a window's `start` cleared it.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn mark_their_accept(play: &mut Play) {
    play.npcw.trade.they_accepted = true;
}

/// Whether the window has been opened for the next frame (`trade_start`/`trade_ask` still to be taken).
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn trade_opening(play: &Play) -> bool {
    play.npcw.trade_start || play.npcw.trade_ask
}

/// `TradeCancel`: sub 1 to the partner, then `Trade16`.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn trade_cancel(play: &mut Play) {
    play.trade_cancel();
}

/// `TradeSend2`/`6`/`7`: a sub-opcode to the partner.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn trade_send(play: &mut Play, sub: u8) {
    play.trade_send(sub);
}

/// `TradeOut::Agree` without the window: `TradeSetup`, then `TradeSend2`.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn trade_agree(play: &mut Play) {
    let coin = play.coin();
    play.npcw.trade_state.setup(&play.dump.inventory, coin);
    play.trade_send(2);
}

/// The answerer's own trade window is running (mode 16 after the game's Yes: the window started, no question up).
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn window_started(play: &Play) -> bool {
    play.npcw.trade.visible && play.npcw.trade.focus >= 0
}

/// The player's accept taken back, as the window's cancel key does before it sends sub 7.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::play) fn unmark_my_accept(play: &mut Play) {
    play.npcw.trade.i_accepted = false;
}
