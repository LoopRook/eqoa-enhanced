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
    // round A2: a dialog or offer is answered whatever the character's state (a resurrection is offered to the dead)
    if let A1Action::SessionAnswer { session, option } = action {
        use super::a2_play::{Choice, dispatch_answer};
        return dispatch_answer(play, session, option.map_or(Choice::Close, Choice::Option));
    }
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
        A1Action::DestroyMany { items } => {
            // one question for every whole stack named; each is checked to be in the pack now, and again at its turn
            let mut lines = Vec::with_capacity(items.len());
            let mut acts = Vec::with_capacity(items.len());
            for item in items {
                let idx = match item_index(play, item) {
                    Ok(i) => i,
                    Err(e) => return e,
                };
                let it = &play.dump.inventory.items[idx];
                // a worn stack is never part of a group destroy, whoever asks (the player scans a list, they do not audit it)
                if it.equip_slot != -1 {
                    return refuse("not_ready", "worn items cannot be group-destroyed");
                }
                let count = it.quantity.max(1);
                lines.push((it.item.name.clone(), count, it.equip_slot != -1, it.quick_slot >= 0, it.item.no_trade > 0));
                acts.push((it.inv_index, it.item.name.clone(), count));
            }
            ask(Act::DestroyMany { items: acts }, confirm::destroy_many_text(&lines), "inventory")
        }
        A1Action::LinkItem { item } => {
            let idx = match item_index(play, item) {
                Ok(i) => i,
                Err(e) => return e,
            };
            // round A6: a mod that holds the chat line is offered the link instead (its session's `offer`)
            if let Some(done) = super::a6_play::offer_link(play, idx) {
                return done;
            }
            // the game's own Link Item: the item's link goes into the chat line (opened if it is not). It goes
            // nowhere where the server offers no links, the line is full or already holds four
            if let Some(no) = super::a6_play::links_unavailable(play) {
                return no;
            }
            if !play.link_item(0, idx as i32) {
                return refuse("not_ready", "the item could not be linked: the chat line has no room");
            }
            DispatchOutcome::Completed { detail: Some(json!({"sent": true})) }
        }
        A1Action::SessionAnswer { .. } => refuse("unsupported", "not an item action"),
        A1Action::HideLook { item, hide } => {
            let idx = match item_index(play, item) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let equip_slot = play.dump.inventory.items[idx].equip_slot;
            if equip_slot == -1 {
                return refuse("not_ready", "only a worn item has a look to hide");
            }
            let slot = i32::try_from(equip_slot).unwrap_or(-1);
            // the game's own Hide Look / Show Look: only where the server offers it and the slot has a look
            if !(play.session.hide_look.offered && net::hidelook::shown(slot)) {
                return refuse("unsupported", "this server does not offer Hide Look for that item");
            }
            play.session.hide_look(slot, *hide);
            DispatchOutcome::Completed { detail: Some(json!({"sent": true, "hide": hide})) }
        }
    }
}

/// `drop_quest` after the broker's own checks: the question, never the drop.
pub(crate) fn ask_drop_quest(index: usize, title: &str, stamp: u32, revision: u64) -> DispatchOutcome {
    let act = Act::DropQuest { index, title: title.to_string(), stamp, revision };
    ask(act, confirm::drop_quest_text(title), "quest_log")
}

pub(super) fn ask(act: Act, text: String, session_kind: &str) -> DispatchOutcome {
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
    // a resurrection offered to the dead is accepted dead: only the world must be there
    let offer = matches!(p.act, Act::AcceptOffer { .. });
    if (play.is_dead() && !offer) || !play.in_world_now() {
        return Err("not in the world".into());
    }
    match &p.act {
        Act::AcceptOffer { session, .. } => super::a2_play::accept_offer(play, session),
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
        // a group destroy is carried out stack by stack (`step_runs`), never in one step
        Act::DestroyMany { .. } => Err("a group destroy runs stack by stack".into()),
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
        Act::GuildKick { .. } | Act::FriendAccept { .. } | Act::GuildAccept { .. } => {
            super::a7_play::carry_out(play, &p.act)
        }
        // round A9: a trade or a duel's wager accepted, after re-checking what the question showed
        Act::TradeAccept { .. } => super::a9_play::carry_out(play, &p.act),
        // round A10: mail and the auction
        Act::MailDelete { .. } | Act::MailSend { .. } | Act::AuctionBid { .. } | Act::AuctionSell { .. } => {
            super::a10_play::carry_out(play, &p.act)
        }
        // round A5: the character window's steps are carried out where they are judged (`a5_capture`)
        Act::TrainStats { .. } | Act::Mastery { .. } => super::api_capture::a5::carry_out(play, &p.act),
        // round A11: the mercenary's dismissal and the dye picker's colour
        Act::DismissMerc | Act::DyeAnswer { .. } => super::a11_play::carry_out(play, &p.act),
    }
}

/// The world is let go (`Play`'s drop calls this): after a logout or a lost connection (`leaving`: the viewer goes
/// back to the game's own screens with no further step) every mod window, a question and the controller end now,
/// not at a step that will never run, so nothing stays drawn or masked over the pregame. A `Play` dropped for any
/// other reason is not a leaving and changes nothing here.
pub(crate) fn play_dropped(leaving: bool) {
    if !leaving {
        return;
    }
    window_sessions::set_env(Env { plain: false, dead: false, away: true, passable: false });
    window_sessions::mods_stopped("left_world");
}

/// Each game step's work for the modern UI's contract.
pub(crate) fn tick(play: &mut Play) {
    // what play is doing is told every step, even when nothing is open: a window a mod opens a moment later must
    // be judged by now, not by the last time anything was
    let away = !play.in_world_now() || play.world_change.is_some();
    let dead = play.is_dead();
    // the same test mod keys use (`addon_input_available`): a mod window is over plain play, never over a game window.
    // The full-screen World Map is not plain play either: nothing is masked behind it (ordinary mod keys, which
    // `addon_input_available` serves, are unchanged).
    let plain = !away && play.addon_input_available() && !play.map.open;
    // the game's own window opens from plain play with the game's camera only (`pass` below)
    let passable = plain && play.mode == 0;
    window_sessions::set_env(Env { plain, dead, away, passable });
    // a Yes to a Class Mastery step that waited for the server's last answer (`a5_capture::hold`)
    for p in super::api_capture::a5::take_released(play) {
        finish_answer(play, &p);
    }
    if !window_sessions::is_active() && !confirm::has_work() && !window_takeover::has_activity() {
        return;
    }
    window_sessions::sync_held([
        window_takeover::hidden_holder(TakeoverKind::Loot),
        window_takeover::hidden_holder(TakeoverKind::Merchant),
        window_takeover::hidden_holder(TakeoverKind::Bank),
    ]);
    // a pass: the game's own window for a held session, as the server's reply would have opened it
    for (vm, kind) in window_sessions::take_passes() {
        if kind == TakeoverKind::Map {
            // round A13: the game's own map for the mod's map session; a map that cannot open now leaves the session
            let _ = super::a13_play::pass(play, vm);
            window_sessions::cancel_pass(vm, kind);
            continue;
        }
        if matches!(kind, TakeoverKind::FriendRequest | TakeoverKind::GuildInvite) {
            if play.mode != 0 || !super::a7_play::pass_request(play, vm, kind) {
                window_sessions::cancel_pass(vm, kind);
                continue;
            }
            window_sessions::finish_pass(vm, kind);
            continue;
        }
        if TakeoverKind::A9.contains(&kind) {
            if play.mode != 0 || !super::a9_play::pass(play, vm, kind) {
                window_sessions::cancel_pass(vm, kind);
                continue;
            }
            window_sessions::finish_pass(vm, kind);
            continue;
        }
        if play.mode != 0 || !window_takeover::hand_back(vm, kind) {
            window_sessions::cancel_pass(vm, kind);
            continue;
        }
        match kind {
            // the flags that open the game's window are private to `npcw` and `bankplay`: their children set them
            TakeoverKind::Loot => crate::play::npcw::fork_pass::show_loot(play),
            TakeoverKind::Merchant => crate::play::npcw::fork_pass::show_merchant(play),
            TakeoverKind::Bank => crate::play::bankplay::fork_pass::show_bank(play),
            // the raid strips are not held sessions: nothing of theirs is passed
            _ => {}
        }
        window_sessions::finish_pass(vm, kind);
    }
    confirm::expire();
    for p in confirm::take_answered() {
        // the game's window waits behind its box for one request at a time: a Yes that comes while the last answer
        // is awaited waits for it (or the 10 s give-up) instead of being refused
        if matches!(p.act, Act::Mastery { .. }) && play.cm.pending != 0 {
            super::api_capture::a5::hold(p);
            continue;
        }
        if matches!(p.act, Act::DestroyMany { .. }) {
            confirm::begin_run(p);
            continue;
        }
        finish_answer(play, &p);
    }
    step_runs(play);
}

/// How long a group destroy waits for the server's answer to one stack before it gives up on the rest (the game's own
/// item window gives up after 10 s too).
const RUN_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// A group destroy the player said Yes to, one stack a step: the game's own destroy (0x003C) waits for the server's
/// answer to the last, so this does too. Each stack is checked to be what the question named at its turn (the slot may
/// have moved, emptied or changed meanwhile); one that is not is skipped and told to the mod, never destroyed.
fn step_runs(play: &mut Play) {
    let mut runs = confirm::take_runs();
    runs.retain_mut(|run| advance_run(play, run));
    confirm::put_runs(runs);
}

/// Returns whether the run goes on.
fn advance_run(play: &mut Play, run: &mut confirm::Run) -> bool {
    if play.is_dead() || !play.in_world_now() {
        confirm::report_done(&run.p, "yes", run.detail(Some("not in the world")));
        return false;
    }
    loop {
        if play.waiting {
            let since = *run.waiting_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= RUN_WAIT {
                confirm::report_done(&run.p, "yes", run.detail(Some("the server did not answer")));
                return false;
            }
            return true;
        }
        run.waiting_since = None;
        let Some((inv_index, name, count)) = run.queue.pop_front() else {
            confirm::report_done(&run.p, "yes", run.detail(None));
            return false;
        };
        let found = play.dump.inventory.items.iter().position(|it| it.inv_index == inv_index);
        match found {
            Some(idx) => {
                let it = &play.dump.inventory.items[idx];
                // worn by its turn (the player put it on while the run went): skipped, never destroyed
                if it.item.name == name && count <= it.quantity.max(1) && it.equip_slot == -1 {
                    play.item_outs(vec![crate::itemmenu::ItemOut::UseItemOn(idx as i32, count as i32)]);
                    run.sent += 1;
                    // the game waits for the server now: the next stack goes on a later step
                    return true;
                }
                run.skipped.push(name);
            }
            None => run.skipped.push(name),
        }
    }
}

fn finish_answer(play: &mut Play, p: &Pending) {
    match carry_out(play, p) {
        Ok(()) => confirm::report_done(p, "yes", json!({"sent": true})),
        Err(reason) => confirm::report_done(p, "yes", json!({"sent": false, "reason": reason})),
    }
}
