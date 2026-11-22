// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1 on `Play`: what the modern UI's contract needs from the running game (`addons::window_sessions`,
//! `addons::confirm`, `addons::native_a1`). A child of the Play adapter, so it reads `Play`'s private state; it
//! adds no line to any upstream file.
//!
//! Each step (`tick`, from `begin_iteration`) it tells the session registry what play is doing (plain, dead, out of
//! the world), keeps the corpse, merchant and bank sessions a mod holds in step with `window_takeover`, opens the
//! game's own window for a session a mod passed back, times the question out, and carries out a Yes.
//!
//! A Yes is carried out only from `confirm::take_answered`, which only the player's own key press or click feeds:
//! this file never answers a question and has no function a mod's call reaches that does.
use super::Play;
use crate::fork_ext::addons::{
    action_broker::DispatchOutcome,
    confirm::{self, Act, Pending},
    native_a1::A1Action,
    native_items::{ResolvedItemSpec, is_handle_fresh},
    window_sessions::{self, Env},
    window_takeover::{self, TakeoverKind},
};
use serde_json::json;

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

/// The index in the inventory of the item a handle names, if it is still that item.
fn item_index(play: &Play, item: &ResolvedItemSpec) -> Result<usize, DispatchOutcome> {
    if !is_handle_fresh(item) {
        return Err(refuse("stale_handle", "item handle is stale or item moved"));
    }
    play.dump
        .inventory
        .items
        .iter()
        .position(|it| it.inv_index == item.inv_index)
        .ok_or_else(|| refuse("stale_handle", "item is no longer in inventory"))
}

pub(crate) fn dispatch(play: &mut Play, action: &A1Action) -> DispatchOutcome {
    if play.is_dead() {
        return refuse("not_ready", "cannot do this while dead");
    }
    match action {
        A1Action::Destroy { item, count } => {
            let idx = match item_index(play, item) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let it = &play.dump.inventory.items[idx];
            let have = it.quantity.max(1);
            let count = count.unwrap_or(have);
            if count > have {
                return refuse("out_of_range", format!("the stack holds {have}, not {count}"));
            }
            let text = confirm::destroy_text(&it.item.name, count, it.equip_slot != -1, it.quick_slot >= 0, it.item.no_trade > 0);
            let act = Act::Destroy { inv_index: it.inv_index, name: it.item.name.clone(), count };
            ask(act, text, "inventory")
        }
        A1Action::LinkItem { item } => {
            let idx = match item_index(play, item) {
                Ok(i) => i,
                Err(e) => return e,
            };
            play.item_outs(vec![crate::itemmenu::ItemOut::Link(idx as i32)]);
            DispatchOutcome::Completed { detail: Some(json!({"sent": true})) }
        }
        A1Action::HideLook { item, hide } => {
            let idx = match item_index(play, item) {
                Ok(i) => i,
                Err(e) => return e,
            };
            if play.dump.inventory.items[idx].equip_slot == -1 {
                return refuse("not_ready", "only a worn item has a look to hide");
            }
            play.item_outs(vec![crate::itemmenu::ItemOut::HideLook(idx as i32, *hide)]);
            DispatchOutcome::Completed { detail: Some(json!({"sent": true, "hide": hide})) }
        }
    }
}

/// `drop_quest` after the broker's own checks: the question, never the drop.
pub(crate) fn ask_drop_quest(index: usize, title: &str, stamp: u32, revision: u64) -> DispatchOutcome {
    let act = Act::DropQuest { index, title: title.to_string(), stamp, revision };
    ask(act, confirm::drop_quest_text(title), "quest_log")
}

fn ask(act: Act, text: String, session_kind: &str) -> DispatchOutcome {
    let Some(vm) = crate::fork_ext::addons::action_broker::current_dispatch_vm() else {
        return refuse("not_ready", "no mod is asking");
    };
    let addon = crate::fork_ext::addons::action_broker::dispatch_addon().unwrap_or_default();
    let session = window_sessions::get(vm, session_kind).map(|s| s.id);
    match confirm::open(vm, &addon, session, act, text, 0) {
        Ok(id) => DispatchOutcome::Completed { detail: Some(json!({"asked": true, "confirm": id})) },
        Err((code, msg)) => refuse(code, msg),
    }
}

/// What a Yes does, as the game's own destroy and drop do, once the item or quest is checked to be what the
/// question named.
fn carry_out(play: &mut Play, p: &Pending) -> Result<(), String> {
    if play.is_dead() || !play.in_world_now() {
        return Err("not in the world".into());
    }
    match &p.act {
        Act::Destroy { inv_index, name, count } => {
            let Some(idx) = play.dump.inventory.items.iter().position(|it| it.inv_index == *inv_index) else {
                return Err("the item is no longer in the inventory".into());
            };
            let it = &play.dump.inventory.items[idx];
            if it.item.name != *name {
                return Err("the slot holds a different item now".into());
            }
            if play.waiting {
                return Err("the server has not answered the last item request".into());
            }
            if *count > it.quantity.max(1) {
                return Err("the stack is smaller than when it was asked about".into());
            }
            play.item_outs(vec![crate::itemmenu::ItemOut::UseItemOn(idx as i32, *count as i32)]);
            Ok(())
        }
        Act::DropQuest { index, title, stamp, revision } => {
            let current = crate::fork_ext::addons::native_core::CORE.with(|c| c.borrow().quest_revision);
            let now_title = play
                .quests
                .titles
                .get(index.wrapping_sub(1))
                .map(|t| String::from_utf16_lossy(&t[..t.iter().position(|&c| c == 0).unwrap_or(t.len())]));
            if current != *revision || play.quests.stamp != *stamp || now_title.as_deref() != Some(title.as_str()) {
                return Err("the quest journal changed since the question was asked".into());
            }
            let m = play.quests.send_remove(*index as i32 - 1);
            play.session.send(&m);
            Ok(())
        }
    }
}

/// Each game step's work for the modern UI's contract.
pub(crate) fn tick(play: &mut Play) {
    // what play is doing is told every step, even when nothing is open: a window a mod opens a moment later must
    // be judged by now, not by the last time anything was
    let away = !play.in_world_now() || play.world_change.is_some();
    let dead = play.is_dead();
    // the same test mod keys use (`addon_input_available`): a mod window is over plain play, never over a game window
    let plain = !away && play.addon_input_available();
    window_sessions::set_env(Env { plain, dead, away });
    if !window_sessions::is_active() && !confirm::is_open() && !window_takeover::has_activity() {
        return;
    }
    window_sessions::sync_held([
        window_takeover::hidden_holder(TakeoverKind::Loot),
        window_takeover::hidden_holder(TakeoverKind::Merchant),
        window_takeover::hidden_holder(TakeoverKind::Bank),
    ]);
    // a pass: the game's own window for a held session, as the server's reply would have opened it
    for (vm, kind) in window_sessions::take_passes() {
        if play.mode != 0 || !window_takeover::hand_back(vm, kind) {
            window_sessions::cancel_pass(vm, kind);
            continue;
        }
        match kind {
            // the flags that open the game's window are private to `npcw` and `bankplay`: their children set them
            TakeoverKind::Loot => crate::play::npcw::fork_pass::show_loot(play),
            TakeoverKind::Merchant => crate::play::npcw::fork_pass::show_merchant(play),
            TakeoverKind::Bank => crate::play::bankplay::fork_pass::show_bank(play),
        }
        window_sessions::finish_pass(vm, kind);
    }
    confirm::expire();
    for p in confirm::take_answered() {
        match carry_out(play, &p) {
            Ok(()) => confirm::report_done(&p, "yes", json!({"sent": true})),
            Err(reason) => confirm::report_done(&p, "yes", json!({"sent": false, "reason": reason})),
        }
    }
}
