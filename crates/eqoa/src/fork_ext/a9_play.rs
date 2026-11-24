// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A9 on `Play`: trading, duels and the blacksmith for a mod (`addons::native_a9`). A child of the Play
//! adapter, so it reads `Play`'s private state; the three small private corners it needs are reached through
//! children of `npcw`, `duelplay` and `support` (`a1_pass_npcw.rs`, `a9_pass_duel.rs`, `a9_pass_support.rs`).
//!
//! A mod that holds `trade_ask`, `trade`, `duel_invite`, `duel` or `blacksmith` (scope `all`) is given the window
//! the game would have drawn: after the packet that opened it is handled (`post_event`, or `tick` for a window the
//! game's own step opened) the window is put away and the session kept, the way A7 does for a friend request. The
//! server's side is unchanged; only the drawing is.
//!
//! What this file keeps to itself, because a mod's Lua must never be trusted with it:
//! - accepting a trade (or a duel's wager) asks the player first (`Act::TradeAccept`), and the Yes is carried out only
//!   if both offers are still exactly what the question showed; no other action accepts;
//! - one holder per kind, actions only on a window the caller holds;
//! - what the game does with no mod holding the kind is not touched: the code is reached only while a hold exists;
//! - a hidden request that nobody answers is refused after 45 seconds (a duel invitation after its own 20); a hidden
//!   window whose mod goes away, whose switch is turned on, or whose world ends is refused or handed back;
//! - a second request while a hidden window is held: a trade request is answered busy (as the game does with a window
//!   up) without disturbing the held trade; a duel invitation is kept waiting behind a hidden wager as the game
//!   queues it, and declined (the game would queue it behind the open invitation too) behind a hidden invitation.
//!
//! The actions are the game's own builders: `TradeState::send_*`, `duel::Part`'s methods, the blacksmith's 0x0062 and
//! 0x0063. They report `sent`, never `confirmed`, and nothing is ever sent twice.
use super::Play;
use crate::duel::{Do, INVITE_TIME, Invite, QUEUE};
use crate::fork_ext::addons::{
    action_broker::{self, DispatchOutcome},
    confirm::{self, Act},
    native_a9::{self as a9, A9Action, MAX_OFFER, REQUEST_TIMEOUT, View},
    native_core,
    player::{CLASSES, RACES},
    window_sessions,
    window_takeover::{self, TakeoverKind},
};
use crate::play::duelplay::fork_a9 as duel_pass;
use crate::play::npcw::fork_pass as trade_pass;
use crate::play::support::fork_a9 as smith_pass;
use net::dump::NetItem;
use serde_json::{Value as Json, json};
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

/// A duel invitation lasts `INVITE_TIME` ticks of the client's clock (983.04 a second).
fn invite_life() -> Duration {
    Duration::from_secs_f64(f64::from(INVITE_TIME) / 983.04)
}

/// Who holds a window and since when.
#[derive(Clone, Debug)]
struct Hold {
    vm: u64,
    addon: String,
    since: Instant,
    /// The other player's id (the partner of a trade, the challenger of an invitation, the smith).
    other: u32,
    /// The name the other side goes by.
    name: String,
}

#[derive(Clone, Debug)]
struct InviteHold {
    hold: Hold,
    rec: Invite,
}

#[derive(Clone, Debug)]
struct SmithHold {
    hold: Hold,
    /// The one repair out: the item's game slot, its condition then, the purse then, and when it was sent.
    pending: Option<(u32, i64, i32, Instant)>,
}

/// The packet being handled, as it looked before the game handled it.
#[derive(Default)]
struct Pre {
    op: Option<u16>,
    sub: u8,
    /// The first number after the sub-opcode (a duel's invitation id in sub 3).
    arg: u32,
    mode: i32,
    trade_state: Option<crate::trade::TradeState>,
    trade: bool,
    invite: bool,
    wager: bool,
}

#[derive(Default)]
struct Local {
    ask: Option<Hold>,
    trade: Option<Hold>,
    invite: Option<InviteHold>,
    wager: Option<Hold>,
    smith: Option<SmithHold>,
    /// Invitations that came while a wager was held, waiting as the game's queue would (with when they came).
    parked: Vec<(Invite, Instant)>,
    /// The partner's accept at the moment a held trade was passed back: put on the game's window after it opens.
    restore_they: bool,
    last_mode: i32,
    pre: Option<Pre>,
    /// Why the held trade or wager ended, as the packet that ended it said.
    reason: Option<&'static str>,
    /// Held things whose world ended (zoning, a teleport, a lost connection): refused at the next step.
    owed: Vec<Owed>,
}

#[derive(Clone, Debug)]
enum Owed {
    Trade(u32),
    Invite { duel_id: u32, id: u32, name: String },
    Wager(u32),
    Smith,
}

thread_local! {
    static LOCAL: RefCell<Local> = RefCell::new(Local::default());
}

fn local<R>(f: impl FnOnce(&mut Local) -> R) -> R {
    LOCAL.with(|l| f(&mut l.borrow_mut()))
}

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

fn sent() -> DispatchOutcome {
    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "confirmed": false})) }
}

fn holder(kind: TakeoverKind) -> Option<(u64, String)> {
    if window_takeover::use_game_windows() {
        return None;
    }
    window_takeover::holder_vm(kind).zip(window_takeover::holder_of(kind))
}

/// Something of round A9 is held or wanted: the per-step work has something to do.
fn engaged() -> bool {
    local(|l| l.ask.is_some() || l.trade.is_some() || l.invite.is_some() || l.wager.is_some() || l.smith.is_some() || !l.parked.is_empty() || !l.owed.is_empty())
        || TakeoverKind::A9.iter().any(|k| window_takeover::is_taken(*k))
        || native_core::capture_requested()
}

fn class_name(id: i32) -> Option<String> {
    usize::try_from(id).ok().and_then(|i| CLASSES.get(i)).map(|s| (*s).to_string())
}

fn race_name(id: i32) -> Option<String> {
    usize::try_from(id).ok().and_then(|i| RACES.get(i)).map(|s| (*s).to_string())
}

fn wide(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}

fn name_of(play: &Play, id: u32) -> String {
    play.entity_name(id).unwrap_or_else(|| "Unknown".to_string())
}

fn purse(play: &Play) -> i32 {
    play.dump.player.p34 as i32
}

fn hold_for(play: &Play, kind: TakeoverKind, other: u32, name: String) -> Option<Hold> {
    let _ = play;
    holder(kind).map(|(vm, addon)| Hold { vm, addon, since: Instant::now(), other, name })
}

// ---- reading the game's state ----

fn item_row(slot: usize, it: &NetItem) -> Json {
    json!({"slot": slot, "id": it.item.id, "name": it.item.name, "quantity": it.quantity.max(1)})
}

fn bag_row(slot: usize, it: &NetItem, room: bool) -> Json {
    let worn = it.equip_slot != -1;
    let no_trade = it.item.no_trade > 0;
    json!({
        "slot": slot,
        "id": it.item.id,
        "name": it.item.name,
        "quantity": it.quantity.max(1),
        "worn": worn,
        "no_trade": no_trade,
        "can_offer": !worn && !no_trade && room,
    })
}

/// The trade or wager window's state, with the clock of a request still waiting.
fn trade_state(play: &Play) -> Option<(Json, Option<(Instant, Duration)>)> {
    let l = LOCAL.with(|l| (l.borrow().ask.clone(), l.borrow().trade.clone(), l.borrow().wager.clone()));
    let (ask, trade, wager) = l;
    let st = &play.npcw.trade_state;
    let (i_acc, they_acc) = trade_pass::trade_marks(play);
    let asked_confirm = |vm: Option<u64>| vm.is_some_and(|vm| confirm::current_for(vm).is_some_and(|p| matches!(p.act, Act::TradeAccept { .. })));
    if wager.is_some() || play.mode == 44 {
        let part = &play.duel.part;
        let name = wager.as_ref().map(|h| h.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| name_of(play, part.a));
        let room = part.mine.len() < MAX_OFFER;
        let mine: Vec<Json> = part.mine.iter().enumerate().map(|(i, it)| item_row(i + 1, it)).collect();
        let theirs: Vec<Json> = part.theirs.iter().enumerate().map(|(i, it)| item_row(i + 1, it)).collect();
        let bags: Vec<Json> = part
            .inv
            .iter()
            .enumerate()
            .map(|(i, it)| bag_row(i + 1, it, room || part.mine.iter().any(|m| m.inv_index == it.inv_index)))
            .collect();
        return Some((
            json!({
                "kind": "duel",
                "phase": "open",
                "held": wager.is_some(),
                "partner": {"id": part.a, "name": name},
                "initiated": Json::Null,
                "mine": {"items": mine, "money": part.my_tunar, "accepted": part.my_acc},
                "theirs": {"items": theirs, "money": part.their_tunar, "accepted": part.their_acc},
                "bags": bags,
                "carried": (purse(play) - part.my_tunar as i32).max(0),
                "max_items": MAX_OFFER,
                "can_accept": !part.my_acc,
                "accept_asked": asked_confirm(wager.as_ref().map(|h| h.vm)),
            }),
            None,
        ));
    }
    if trade.is_some() || play.mode == 15 {
        let other = if st.with != u32::MAX { st.with } else { st.partner };
        let room = st.mine.len() < MAX_OFFER;
        let mine: Vec<Json> = st.mine.iter().enumerate().map(|(i, o)| item_row(i + 1, &o.item)).collect();
        let theirs: Vec<Json> = st.theirs.iter().enumerate().map(|(i, o)| item_row(i + 1, &o.item)).collect();
        let bags: Vec<Json> = st
            .inv
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| bag_row(i + 1, it, room || st.mine.iter().any(|m| m.item.inv_index == it.inv_index)))
            .collect();
        return Some((
            json!({
                "kind": "trade",
                "phase": "open",
                "held": trade.is_some(),
                "partner": {"id": other, "name": name_of(play, other)},
                "initiated": st.asked,
                "mine": {"items": mine, "money": st.my_money, "accepted": i_acc},
                "theirs": {"items": theirs, "money": st.their_money, "accepted": they_acc},
                "bags": bags,
                "carried": st.carried,
                "max_items": MAX_OFFER,
                "can_accept": !i_acc,
                "accept_asked": asked_confirm(trade.as_ref().map(|h| h.vm)),
            }),
            None,
        ));
    }
    let request = ask.is_some() || (play.mode == 16 && st.with != u32::MAX);
    if request {
        let timer = ask.as_ref().map(|h| (h.since, REQUEST_TIMEOUT));
        return Some((
            json!({
                "kind": "trade",
                "phase": "request",
                "held": ask.is_some(),
                "partner": {"id": st.with, "name": name_of(play, st.with)},
                "initiated": false,
            }),
            timer,
        ));
    }
    None
}

fn invite_json(rec: &Invite) -> Json {
    json!({
        "id": rec.id,
        "name": rec.name,
        "race_id": rec.race,
        "race_name": race_name(i32::from(rec.race)),
        "class_id": rec.class,
        "class_name": class_name(i32::from(rec.class)),
        "level": rec.level,
        "guild": wide(&rec.guild),
        "title": wide(&rec.title),
    })
}

fn duel_state(play: &Play) -> (Json, Option<(Instant, Duration)>) {
    let part = &play.duel.part;
    let (held, wager, parked) = LOCAL.with(|l| {
        let l = l.borrow();
        (l.invite.clone(), l.wager.is_some(), l.parked.len())
    });
    let native_invite = (play.mode == 43).then(|| play.duel.wnd.invite.clone()).flatten();
    let invite = held.as_ref().map(|h| h.rec.clone()).or(native_invite);
    let state = if wager || play.mode == 44 {
        "wager"
    } else if invite.is_some() {
        "invited"
    } else if part.running {
        "running"
    } else if part.set {
        "set"
    } else {
        "none"
    };
    let timer = held.as_ref().map(|h| (h.hold.since, invite_life()));
    (
        json!({
            "enabled": part.on(),
            "state": state,
            "invite": invite.as_ref().map(invite_json),
            "queued": part.queue.len() + parked,
            "opponent_id": (part.a != u32::MAX).then_some(part.a),
        }),
        timer,
    )
}

fn smith_state(play: &Play) -> Option<Json> {
    let held = LOCAL.with(|l| l.borrow().smith.clone());
    if held.is_none() && play.mode != 19 {
        return None;
    }
    let items: Vec<Json> = smith_items(play)
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let cost = crate::blacksmith::repair_cost(it);
            let condition = if it.item.max_hp == 0 { 1.0 } else { (it.cur_hp as f64 / it.item.max_hp as f64).clamp(0.0, 1.0) };
            json!({
                "slot": i + 1,
                "id": it.item.id,
                "name": it.item.name,
                "quantity": it.quantity.max(1),
                "condition": condition,
                "cost": cost,
                "can_repair": cost > 0,
                "worn": it.equip_slot != -1,
            })
        })
        .collect();
    Some(json!({
        "coin": purse(play),
        "repair_pending": held.as_ref().is_some_and(|h| h.pending.is_some()),
        "items": items,
    }))
}

/// What the smith lists: what the game's window repairs on X (`blacksmith::usable`), in the bags' order.
fn smith_items(play: &Play) -> Vec<&NetItem> {
    play.dump.inventory.items.iter().filter(|it| crate::blacksmith::usable(it)).collect()
}

/// Publishes the three reads.
fn refresh(play: &Play) {
    if !native_core::capture_requested() && !engaged() {
        return;
    }
    let trade = trade_state(play);
    a9::publish(View::Trade, trade.as_ref().map(|(j, _)| j.clone()), trade.and_then(|(_, t)| t));
    let (duel, timer) = duel_state(play);
    a9::publish(View::Duel, Some(duel), timer);
    a9::publish(View::Smith, smith_state(play), None);
}

// ---- taking a window the game just opened ----

fn sync_session(kind: TakeoverKind, hold: Option<&Hold>, reason: &str) {
    window_sessions::sync_held_request(kind, hold.map(|h| (h.vm, h.addon.clone())), reason);
}

fn take_ask(play: &mut Play) {
    let st = &play.npcw.trade_state;
    let (other, name) = (st.with, name_of(play, st.with));
    let Some(hold) = hold_for(play, TakeoverKind::TradeAsk, other, name.clone()) else { return };
    trade_pass::hide_trade_window(play, false);
    play.mode = 0;
    sync_session(TakeoverKind::TradeAsk, Some(&hold), "ended");
    local(|l| l.ask = Some(hold));
    a9::emit_event("trade.request", json!({"kind": "trade", "id": other, "name": name}));
}

fn take_trade(play: &mut Play) {
    let st = &play.npcw.trade_state;
    let other = if st.with != u32::MAX { st.with } else { st.partner };
    let Some(hold) = hold_for(play, TakeoverKind::Trade, other, name_of(play, other)) else { return };
    trade_pass::hide_trade_window(play, true);
    play.mode = 0;
    // the request that led here (the mod's own held `trade_ask`, answered yes) is over
    end_ask(play, "accepted");
    sync_session(TakeoverKind::Trade, Some(&hold), "ended");
    local(|l| {
        l.trade = Some(hold);
        l.reason = None;
    });
}

fn take_invite(play: &mut Play) {
    let Some(rec) = play.duel.wnd.invite.clone() else { return };
    let Some(hold) = hold_for(play, TakeoverKind::DuelInvite, rec.id, rec.name.clone()) else { return };
    duel_pass::hide_window(play);
    play.mode = 0;
    sync_session(TakeoverKind::DuelInvite, Some(&hold), "ended");
    local(|l| l.invite = Some(InviteHold { hold, rec }));
}

fn take_wager(play: &mut Play) {
    let name = LOCAL.with(|l| l.borrow().invite.as_ref().map(|h| h.hold.name.clone())).unwrap_or_default();
    let Some(mut hold) = hold_for(play, TakeoverKind::Duel, play.duel.part.a, name) else { return };
    duel_pass::hide_window(play);
    play.mode = 0;
    // the chat box was fixed for the window (`OpenWager`); a window that is not drawn does not hold it
    if let Some(c) = play.wnd_chat.as_mut() {
        c.mode = 0;
    }
    hold.other = play.duel.part.duel_id;
    sync_session(TakeoverKind::Duel, Some(&hold), "ended");
    local(|l| {
        l.wager = Some(hold);
        l.reason = None;
    });
}

fn take_smith(play: &mut Play) {
    let id = smith_pass::smith_id(play);
    let Some(hold) = hold_for(play, TakeoverKind::Blacksmith, id, String::new()) else { return };
    smith_pass::hide_smith(play);
    play.mode = 0;
    sync_session(TakeoverKind::Blacksmith, Some(&hold), "ended");
    local(|l| l.smith = Some(SmithHold { hold, pending: None }));
}

/// A window of mode 16, 15, 43, 44 or 19 that appeared since the last look is taken for the mod that holds its kind
/// (never one already open before: that one is the player's).
fn take_new(play: &mut Play) {
    let prev = local(|l| std::mem::replace(&mut l.last_mode, play.mode));
    if prev == play.mode {
        return;
    }
    match play.mode {
        16 if play.npcw.trade_state.with != u32::MAX && window_takeover::is_taken(TakeoverKind::TradeAsk) => take_ask(play),
        15 if window_takeover::is_taken(TakeoverKind::Trade) => take_trade(play),
        43 if play.duel.wnd.invite.is_some() && window_takeover::is_taken(TakeoverKind::DuelInvite) => take_invite(play),
        44 if window_takeover::is_taken(TakeoverKind::Duel) => take_wager(play),
        19 if window_takeover::is_taken(TakeoverKind::Blacksmith) => take_smith(play),
        _ => {}
    }
    local(|l| l.last_mode = play.mode);
}

// ---- ending what is held ----

/// The request or window ends: the session closes and the mod is told.
fn closed(kind: &str, reason: &str) {
    a9::emit_event("trade.closed", json!({"kind": kind, "reason": reason}));
}

fn end_ask(play: &Play, reason: &'static str) {
    let _ = play;
    if local(|l| l.ask.take()).is_some() {
        sync_session(TakeoverKind::TradeAsk, None, reason);
    }
}

fn end_trade(reason: &'static str) {
    if local(|l| l.trade.take()).is_some() {
        confirm::cancel_kind("trade_accept", reason);
        sync_session(TakeoverKind::Trade, None, reason);
        closed("trade", reason);
    }
}

fn end_invite(reason: &'static str) {
    if local(|l| l.invite.take()).is_some() {
        sync_session(TakeoverKind::DuelInvite, None, reason);
    }
}

fn end_wager(play: &mut Play, reason: &'static str) {
    if local(|l| l.wager.take()).is_some() {
        confirm::cancel_kind("duel_accept", reason);
        sync_session(TakeoverKind::Duel, None, reason);
        closed("duel", reason);
    }
    // the invitations that waited behind the wager are the game's queue again (their time ran meanwhile)
    let parked = local(|l| std::mem::take(&mut l.parked));
    for (mut rec, since) in parked {
        let used = (since.elapsed().as_secs_f64() * 983.04) as u32;
        if used < rec.timer && play.duel.part.queue.len() < QUEUE {
            rec.timer -= used;
            play.duel.part.queue.push(rec);
        }
    }
}

fn end_smith(reason: &'static str) {
    if local(|l| l.smith.take()).is_some() {
        sync_session(TakeoverKind::Blacksmith, None, reason);
    }
}

/// The game's own bytes for turning a held request or window away.
fn send_trade_cancel(play: &mut Play, partner: u32) {
    let mut w = net::stream::Writer::op(0x0056);
    w.u8(1).u32(partner);
    play.session.send(&w.data);
}

fn send_invite_decline(play: &mut Play, duel_id: u32, id: u32, name: &str) {
    // `Part::decline` reads the id held at the time; the duel id the server last gave is what it expects back
    let mut part = play.duel.part.clone();
    part.duel_id = duel_id;
    for d in part.decline(id, name) {
        apply_do(play, d);
    }
}

/// What the duel part asks of the game, but only the part that talks to the server or to the chat: the windows are
/// this file's to manage.
fn apply_do(play: &mut Play, d: Do) {
    match d {
        Do::Send(m) => play.session.send(&m),
        Do::Line(text, color) => play.add_chat(text, color),
        Do::ShowInvite(_) | Do::CloseInvite | Do::OpenWager | Do::CloseWager => {}
    }
}

fn refuse_trade(play: &mut Play, reason: &'static str) {
    if let Some(h) = local(|l| l.trade.clone()) {
        send_trade_cancel(play, h.other);
        // the game's own state: reset as `trade_done` leaves it (mode is already 0)
        play.npcw.trade_state.reset();
        end_trade(reason);
    }
}

fn refuse_ask(play: &mut Play, reason: &'static str) {
    if local(|l| l.ask.is_some()) {
        let partner = play.npcw.trade_state.partner;
        send_trade_cancel(play, partner);
        play.npcw.trade_state.reset();
        end_ask(play, reason);
    }
}

fn refuse_invite(play: &mut Play, reason: &'static str) {
    if let Some(h) = local(|l| l.invite.clone()) {
        let name = play.dump.player.name.clone();
        let duel_id = play.duel.part.duel_id;
        send_invite_decline(play, duel_id, h.rec.id, &name);
        end_invite(reason);
    }
}

fn refuse_wager(play: &mut Play, reason: &'static str) {
    if local(|l| l.wager.is_some()) {
        for d in play.duel.part.cancel_duel() {
            apply_do(play, d);
        }
        end_wager(play, reason);
    }
}

fn refuse_smith(play: &mut Play, reason: &'static str) {
    if local(|l| l.smith.is_some()) {
        smith_pass::close_smith(play);
        end_smith(reason);
    }
}

fn refuse_all(play: &mut Play, reason: &'static str) {
    refuse_ask(play, reason);
    refuse_trade(play, reason);
    refuse_invite(play, reason);
    refuse_wager(play, reason);
    refuse_smith(play, reason);
    local(|l| l.parked.clear());
}

// ---- events ----

/// Before the game handles `event`: what it looks like now, when a mod holds something here.
pub(crate) fn pre_event(play: &Play, event: &net::session::Event) {
    use net::session::Event;
    let reset = matches!(event, Event::WorldEntry { .. } | Event::Teleport { .. } | Event::Disconnected(_));
    let held = local(|l| (l.ask.is_some(), l.trade.is_some(), l.invite.is_some(), l.wager.is_some()));
    if !(held.0 || held.1 || held.2 || held.3) && !reset && !window_takeover::has_activity() {
        local(|l| l.pre = None);
        return;
    }
    if reset {
        // the server must still hear a refusal for what is held: noted now, sent by the next step
        let owed: Vec<Owed> = local(|l| {
            let mut v = Vec::new();
            if let Some(h) = &l.ask {
                v.push(Owed::Trade(h.other));
            }
            if let Some(h) = &l.trade {
                v.push(Owed::Trade(h.other));
            }
            if let Some(h) = &l.invite {
                v.push(Owed::Invite { duel_id: play.duel.part.duel_id, id: h.rec.id, name: play.dump.player.name.clone() });
            }
            if let Some(h) = &l.wager {
                v.push(Owed::Wager(h.other));
            }
            if l.smith.is_some() {
                v.push(Owed::Smith);
            }
            v
        });
        local(|l| l.owed.extend(owed));
    }
    let (op, sub, arg) = match event {
        Event::Opcode { op, data } => (
            Some(*op),
            data.first().copied().unwrap_or(0),
            data.get(1..5).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        ),
        _ => (None, 0, 0),
    };
    let pre = Pre {
        op,
        sub,
        arg,
        mode: play.mode,
        // a new request must not overwrite what a held trade or request is made of
        trade_state: (op == Some(0x0057) && (held.0 || held.1)).then(|| play.npcw.trade_state.clone()),
        trade: held.1,
        invite: held.2,
        wager: held.3,
    };
    local(|l| l.pre = Some(pre));
}

/// After the game handled the packet: what it opened is taken for the mod; what it ended is told.
pub(crate) fn post_event(play: &mut Play) {
    let pre = local(|l| l.pre.take());
    if let Some(pre) = pre {
        handle_second_requests(play, &pre);
        note_endings(play, &pre);
    }
    if engaged() {
        take_new(play);
        check_closed(play);
        watch_question(play);
        refresh(play);
    } else {
        local(|l| l.last_mode = play.mode);
    }
}

/// A request that arrives while a hidden window holds the game's mode (which the hidden window leaves at 0).
fn handle_second_requests(play: &mut Play, pre: &Pre) {
    // a second trade request: the game answers "busy" to one that comes with a window up, and says so in the chat;
    // the held request or trade is not touched
    if let Some(old) = &pre.trade_state
        && play.mode == 16
        && pre.mode == 0
    {
        let newcomer = (play.npcw.trade_state.partner, play.npcw.trade_state.with);
        play.npcw.trade_state = old.clone();
        trade_pass::hide_trade_window(play, false);
        play.mode = 0;
        let name = name_of(play, newcomer.1);
        let fmt = play.words.as_ref().map_or_else(|| "%s".to_string(), |w| w.get(626, 0).to_string());
        play.add_chat(fmt.replacen("%s", &name, 1).replacen("%S", &name, 1), [1.0; 4]);
        let mut w = net::stream::Writer::op(0x0056);
        w.u8(20).u32(newcomer.0);
        play.session.send(&w.data);
        return;
    }
    // an invitation (0x1502 sub 2) while a mod holds a hidden invitation or wager: it was shown over the hidden one
    if pre.op == Some(crate::duel::OP_IN) && pre.sub == 2 && play.mode == 43 && pre.mode == 0 && (pre.invite || pre.wager) {
        let Some(rec) = play.duel.wnd.invite.clone() else { return };
        duel_pass::hide_window(play);
        play.mode = 0;
        if pre.wager && local(|l| l.parked.len()) < QUEUE {
            // as the game queues one that arrives while the wager is up
            local(|l| l.parked.push((rec, Instant::now())));
        } else {
            // the game queues it behind the open invitation (up to ten, then declines); a hidden invitation holds one
            // at a time, so the newcomer is declined with the game's own message
            let name = play.dump.player.name.clone();
            let duel_id = play.duel.part.duel_id;
            send_invite_decline(play, duel_id, rec.id, &name);
        }
    }
}

/// What the packet ended: a trade over, a wager closed, an invitation gone, the smith closed.
fn note_endings(play: &mut Play, pre: &Pre) {
    match (pre.op, pre.sub) {
        (Some(0x0056), 1) => local(|l| l.reason = Some("cancelled")),
        (Some(0x0056), 16) => local(|l| l.reason = Some("completed")),
        (Some(0x0056), 17 | 21 | 22) if pre.trade => a9::emit_event("trade.accepted", json!({"kind": "trade", "side": "refused"})),
        (Some(0x0056), 6) if pre.trade => a9::emit_event("trade.accepted", json!({"kind": "trade", "side": "theirs"})),
        (Some(0x0063), _) => {
            if local(|l| l.smith.is_some()) {
                end_smith("closed");
            }
        }
        (Some(op), 14) if op == crate::duel::OP_IN => {
            if local(|l| l.wager.is_some()) {
                end_wager(play, "closed");
            }
        }
        (Some(op), 15) if op == crate::duel::OP_IN => {
            if local(|l| l.wager.is_some()) && play.duel.part.my_acc && play.duel.part.their_acc {
                end_wager(play, "completed");
            }
        }
        (Some(op), 3) if op == crate::duel::OP_IN => {
            // an invitation gone (the challenger withdrew, or it ran out on the server)
            // the part drops it from its queue only; the one held here is told by its id
            if local(|l| l.invite.as_ref().is_some_and(|h| h.rec.id == pre.arg)) {
                end_invite("gone");
            }
        }
        _ => {}
    }
}

/// A question about accepting is asked about exactly what was on offer: when either side changes, the game clears
/// the accepts, and so the question ends (answered No) instead of waiting to be refused.
fn watch_question(play: &Play) {
    let Some(vm) = local(|l| l.trade.as_ref().map(|h| h.vm).or_else(|| l.wager.as_ref().map(|h| h.vm))) else { return };
    let Some(p) = confirm::current_for(vm) else { return };
    let Act::TradeAccept { duel, digest } = p.act else { return };
    let w = if duel { Which::Wager } else { Which::Trade };
    if describe_offers(play, &w).4 != digest {
        confirm::cancel_kind(accept_kind(&w), "changed");
    }
}

/// A held trade whose state the game has reset (cancelled, completed, refused by the other side) is over.
fn check_closed(play: &mut Play) {
    if local(|l| l.trade.is_some()) && play.npcw.trade_state.with == u32::MAX {
        let reason = local(|l| l.reason.take()).unwrap_or("cancelled");
        end_trade(reason);
    }
    if local(|l| l.ask.is_some()) && play.npcw.trade_state.with == u32::MAX {
        end_ask(play, "cancelled");
    }
}

// ---- the step ----

/// Each step's work: the lifecycle of what is held, and the snapshots a mod reads.
pub(crate) fn tick(play: &mut Play) {
    if !engaged() {
        local(|l| l.last_mode = play.mode);
        return;
    }
    // held things whose world ended: refused with the game's own bytes
    let owed = local(|l| std::mem::take(&mut l.owed));
    for o in owed {
        match o {
            Owed::Trade(partner) => send_trade_cancel(play, partner),
            Owed::Invite { duel_id, id, name } => send_invite_decline(play, duel_id, id, &name),
            Owed::Wager(duel_id) => {
                let mut w = net::stream::Writer::op(crate::duel::OP_OUT);
                w.u8(10).u32(duel_id);
                play.session.send(&w.data);
            }
            Owed::Smith => play.session.send_opcode(0x0063),
        }
    }
    let away = !play.in_world_now() || play.world_change.is_some();
    let dead = play.is_dead();
    if away {
        refuse_all_quiet(play);
        local(|l| l.last_mode = play.mode);
        refresh_cleared();
        return;
    }
    take_new(play);
    lifecycle(play, dead);
    check_closed(play);
    watch_question(play);
    expire(play);
    refresh(play);
}

/// The world ended: what was held is gone from the client (the server's refusal was sent, `owed`).
fn refuse_all_quiet(play: &mut Play) {
    let any = local(|l| l.ask.is_some() || l.trade.is_some() || l.invite.is_some() || l.wager.is_some() || l.smith.is_some());
    if any {
        local(|l| {
            l.ask = None;
            l.trade = None;
            l.invite = None;
            l.wager = None;
            l.smith = None;
            l.parked.clear();
        });
        for k in TakeoverKind::A9 {
            sync_session(k, None, "left_world");
        }
        confirm::cancel_kind("trade_accept", "left_world");
        confirm::cancel_kind("duel_accept", "left_world");
    }
    let _ = play;
}

fn refresh_cleared() {
    a9::clear_all();
}

/// Holder gone, the player's switch on, death: what is held goes back to the game or is turned away.
fn lifecycle(play: &mut Play, dead: bool) {
    let gw = window_takeover::use_game_windows();
    let gone = |kind: TakeoverKind, vm: u64| gw || window_takeover::holder_vm(kind) != Some(vm);
    let why = |gw: bool| if gw { "game_windows" } else { "released" };
    if let Some(h) = local(|l| l.ask.clone()) {
        if dead {
            refuse_ask(play, "death");
        } else if gone(TakeoverKind::TradeAsk, h.vm) {
            if play.mode == 0 {
                end_ask(play, why(gw));
                trade_pass::show_trade_ask(play);
                shown(play);
            } else {
                refuse_ask(play, why(gw));
            }
        }
    }
    if let Some(h) = local(|l| l.trade.clone()) {
        if dead {
            refuse_trade(play, "death");
        } else if gone(TakeoverKind::Trade, h.vm) {
            if play.mode == 0 {
                hand_back_trade(play, why(gw));
                shown(play);
            } else {
                refuse_trade(play, why(gw));
            }
        }
    }
    if let Some(h) = local(|l| l.invite.clone()) {
        if dead {
            refuse_invite(play, "death");
        } else if gone(TakeoverKind::DuelInvite, h.hold.vm) {
            if play.mode == 0 {
                end_invite(why(gw));
                duel_show_invite(play, &h.rec, &h.hold);
                shown(play);
            } else {
                refuse_invite(play, why(gw));
            }
        }
    }
    if let Some(h) = local(|l| l.wager.clone()) {
        if dead {
            refuse_wager(play, "death");
        } else if gone(TakeoverKind::Duel, h.vm) {
            if play.mode == 0 {
                end_wager(play, why(gw));
                play.duel.wnd.open_wager();
                duel_pass::show_window(play, 44);
                if let Some(c) = play.wnd_chat.as_mut() {
                    c.mode = 1;
                }
                shown(play);
            } else {
                refuse_wager(play, why(gw));
            }
        }
    }
    if let Some(h) = local(|l| l.smith.clone()) {
        if dead {
            refuse_smith(play, "death");
        } else if gone(TakeoverKind::Blacksmith, h.hold.vm) {
            if play.mode == 0 {
                end_smith(why(gw));
                smith_pass::show_smith(play);
                shown(play);
            } else {
                refuse_smith(play, why(gw));
            }
        }
    }
}

fn duel_show_invite(play: &mut Play, rec: &Invite, hold: &Hold) {
    let mut rec = rec.clone();
    let used = (hold.since.elapsed().as_secs_f64() * 983.04) as u32;
    rec.timer = rec.timer.saturating_sub(used).max(1);
    let mut cues = Vec::new();
    let clock = play.session.timer();
    play.duel.wnd.show_invite(rec, clock, &mut cues);
    play.cues.extend(cues.into_iter().map(crate::sound::Cue::Ui));
    duel_pass::show_window(play, 43);
}

/// A held trade handed back to the game's window. The window's `start` clears both accept marks, so the player's
/// own accept is taken back with the server first (the window shows it not accepted), and the partner's is put back
/// after the window opened.
fn hand_back_trade(play: &mut Play, reason: &'static str) {
    let (mine, theirs) = trade_pass::trade_marks(play);
    if mine {
        trade_pass::trade_send(play, 7);
    }
    local(|l| l.restore_they = theirs);
    confirm::cancel_kind("trade_accept", reason);
    if local(|l| l.trade.take()).is_some() {
        sync_session(TakeoverKind::Trade, None, reason);
    }
    trade_pass::show_trade(play);
}

/// The game's own window was handed back: its mode is the player's now, not a new window to take.
fn shown(play: &Play) {
    local(|l| l.last_mode = play.mode);
}

/// A request nobody answered is refused; a window's marks are put back after a pass.
fn expire(play: &mut Play) {
    if local(|l| l.ask.as_ref().is_some_and(|h| h.since.elapsed() >= REQUEST_TIMEOUT)) {
        refuse_ask(play, "timeout");
    }
    if local(|l| l.invite.as_ref().is_some_and(|h| h.hold.since.elapsed() >= invite_life())) {
        refuse_invite(play, "timeout");
    }
    local(|l| l.parked.retain(|(rec, since)| (since.elapsed().as_secs_f64() * 983.04) < f64::from(rec.timer)));
    // after a pass the window has started: the partner's accept goes back on it
    if local(|l| l.restore_they) && play.mode == 15 && !trade_pass::trade_opening(play) {
        trade_pass::mark_their_accept(play);
        local(|l| l.restore_they = false);
    }
    // a repair the server never answered stops blocking the next one
    local(|l| {
        if let Some(s) = l.smith.as_mut()
            && s.pending.as_ref().is_some_and(|p| p.3.elapsed() >= Duration::from_secs(3))
        {
            s.pending = None;
        }
    });
    smith_settle(play);
}

/// The repair out is answered once the item's condition or the purse is not what it was.
fn smith_settle(play: &Play) {
    let Some((key, hp, coin, _)) = local(|l| l.smith.as_ref().and_then(|s| s.pending)) else { return };
    let item = play.dump.inventory.items.iter().find(|it| it.inv_index == key);
    if item.is_none_or(|it| it.cur_hp != hp) || purse(play) != coin {
        local(|l| {
            if let Some(s) = l.smith.as_mut() {
                s.pending = None;
            }
        });
    }
}

/// A pass: the game's own window for a held session, at the next step (`a1_play::tick`).
pub(crate) fn pass(play: &mut Play, vm: u64, kind: TakeoverKind) -> bool {
    match kind {
        TakeoverKind::TradeAsk => {
            if local(|l| l.ask.as_ref().is_none_or(|h| h.vm != vm)) {
                return false;
            }
            end_ask(play, "passed");
            trade_pass::show_trade_ask(play);
            shown(play);
            true
        }
        TakeoverKind::Trade => {
            if local(|l| l.trade.as_ref().is_none_or(|h| h.vm != vm)) {
                return false;
            }
            hand_back_trade(play, "passed");
            shown(play);
            true
        }
        TakeoverKind::DuelInvite => {
            let Some(h) = local(|l| l.invite.clone()).filter(|h| h.hold.vm == vm) else { return false };
            end_invite("passed");
            duel_show_invite(play, &h.rec, &h.hold);
            shown(play);
            true
        }
        TakeoverKind::Duel => {
            if local(|l| l.wager.as_ref().is_none_or(|h| h.vm != vm)) {
                return false;
            }
            end_wager(play, "passed");
            play.duel.wnd.open_wager();
            duel_pass::show_window(play, 44);
            if let Some(c) = play.wnd_chat.as_mut() {
                c.mode = 1;
            }
            shown(play);
            true
        }
        TakeoverKind::Blacksmith => {
            if local(|l| l.smith.as_ref().is_none_or(|h| h.hold.vm != vm)) {
                return false;
            }
            end_smith("passed");
            smith_pass::show_smith(play);
            shown(play);
            true
        }
        _ => false,
    }
}

/// Refuses whatever is held before `Play` drops on a logout or a lost connection, so the server never waits.
pub(crate) fn play_dropped(play: &mut Play, leaving: bool) {
    if !leaving {
        return;
    }
    let owed = local(|l| std::mem::take(&mut l.owed));
    for o in owed {
        match o {
            Owed::Trade(partner) => send_trade_cancel(play, partner),
            Owed::Invite { duel_id, id, name } => send_invite_decline(play, duel_id, id, &name),
            Owed::Wager(duel_id) => {
                let mut w = net::stream::Writer::op(crate::duel::OP_OUT);
                w.u8(10).u32(duel_id);
                play.session.send(&w.data);
            }
            Owed::Smith => play.session.send_opcode(0x0063),
        }
    }
    refuse_all(play, "left_world");
    local(|l| *l = Local::default());
    a9::clear_all();
}

// ---- the actions ----

fn caller() -> Option<u64> {
    action_broker::current_dispatch_vm()
}

fn holds(kind: TakeoverKind) -> bool {
    caller().is_some() && window_takeover::holder_vm(kind) == caller()
}

pub(crate) fn dispatch(play: &mut Play, action: &A9Action) -> DispatchOutcome {
    let out = dispatch_inner(play, action);
    check_closed(play);
    refresh(play);
    out
}

fn dispatch_inner(play: &mut Play, action: &A9Action) -> DispatchOutcome {
    if play.is_dead() {
        return refuse("not_ready", "cannot do this while dead");
    }
    if window_takeover::use_game_windows() {
        return refuse("unavailable", "the player is using the game's windows");
    }
    match action {
        A9Action::TradeAnswer { accept } => trade_answer(play, *accept),
        A9Action::DuelAnswer { accept } => duel_answer(play, *accept),
        A9Action::BlacksmithRepair { slot, id } => repair(play, *slot, *id),
        A9Action::BlacksmithClose => {
            let Some(h) = local(|l| l.smith.clone()).filter(|h| holds(TakeoverKind::Blacksmith) && Some(h.hold.vm) == caller()) else {
                return refuse("not_ready", "hold the 'blacksmith' window first (eqoa.windows.takeover) and wait for the smith");
            };
            let _ = h;
            smith_pass::close_smith(play);
            end_smith("closed");
            sent()
        }
        _ => exchange(play, action),
    }
}

fn trade_answer(play: &mut Play, accept: bool) -> DispatchOutcome {
    let Some(h) = local(|l| l.ask.clone()).filter(|h| Some(h.vm) == caller() && holds(TakeoverKind::TradeAsk)) else {
        return refuse("not_ready", "no trade request is held: hold 'trade_ask' (eqoa.windows.takeover) and wait for one");
    };
    if play.npcw.trade_state.with == u32::MAX {
        return refuse("not_ready", "the request is gone");
    }
    if accept {
        // `TradeOut::Agree`: the trade set up from the bags, then sub 2 to the partner
        trade_pass::trade_agree(play);
        end_ask(play, "accepted");
    } else {
        trade_pass::trade_cancel(play);
        end_ask(play, "declined");
    }
    let _ = h;
    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "answer": if accept { "yes" } else { "no" }})) }
}

fn duel_answer(play: &mut Play, accept: bool) -> DispatchOutcome {
    let Some(h) = local(|l| l.invite.clone()).filter(|h| Some(h.hold.vm) == caller() && holds(TakeoverKind::DuelInvite)) else {
        return refuse("not_ready", "no duel invitation is held: hold 'duel_invite' (eqoa.windows.takeover) and wait for one");
    };
    if h.hold.since.elapsed() >= invite_life() {
        return refuse("not_ready", "the invitation has run out");
    }
    let name = play.dump.player.name.clone();
    let list = if accept { play.duel.part.accept(h.rec.id, &name) } else { play.duel.part.decline(h.rec.id, &name) };
    for d in list {
        apply_do(play, d);
    }
    end_invite(if accept { "accepted" } else { "declined" });
    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "answer": if accept { "yes" } else { "no" }})) }
}

/// Which exchange the trade actions are for: a held wager, or a held trade.
enum Which {
    Trade,
    Wager,
}

fn which(play: &Play) -> Option<Which> {
    let _ = play;
    let me = caller()?;
    if local(|l| l.wager.as_ref().is_some_and(|h| h.vm == me)) && holds(TakeoverKind::Duel) {
        return Some(Which::Wager);
    }
    if local(|l| l.trade.as_ref().is_some_and(|h| h.vm == me)) && holds(TakeoverKind::Trade) {
        return Some(Which::Trade);
    }
    None
}

fn exchange(play: &mut Play, action: &A9Action) -> DispatchOutcome {
    let Some(w) = which(play) else {
        return refuse("not_ready", "no trade or duel wager is held: hold 'trade' or 'duel' (eqoa.windows.takeover) and wait for it to open");
    };
    let (mine_acc, _) = match w {
        Which::Trade => trade_pass::trade_marks(play),
        Which::Wager => (play.duel.part.my_acc, play.duel.part.their_acc),
    };
    match action {
        A9Action::TradeAdd { slot, id, count } => {
            if mine_acc {
                return refuse("not_ready", "you have accepted: take the accept back (trade_unaccept) before changing the offer");
            }
            let (it, key, quantity): (NetItem, u32, i64) = match w {
                Which::Trade => match play.npcw.trade_state.inv.items.get(*slot) {
                    Some(it) => (it.clone(), it.inv_index, it.quantity),
                    None => return refuse("stale_handle", "that row is not in the trade's bags now"),
                },
                Which::Wager => match play.duel.part.inv.get(*slot) {
                    Some(it) => (it.clone(), it.inv_index, it.quantity),
                    None => return refuse("stale_handle", "that row is not in the wager's bags now"),
                },
            };
            if it.item.id != *id {
                return refuse("stale_handle", "that row holds a different item now");
            }
            if it.equip_slot != -1 {
                return refuse("not_ready", "that item is worn: take it off first");
            }
            if it.item.no_trade > 0 {
                return refuse("not_ready", "that item is NO TRADE");
            }
            let stack = it.item.max_stack != 0 && quantity >= 2;
            if *count > quantity.max(1) || (!stack && *count != 1) {
                return refuse("out_of_range", format!("the row holds {} of it: offer that many or fewer (a single item is offered one at a time)", quantity.max(1)));
            }
            let (offered, has_row) = match w {
                Which::Trade => (play.npcw.trade_state.mine.len(), play.npcw.trade_state.mine.iter().any(|o| o.item.inv_index == key)),
                Which::Wager => (play.duel.part.mine.len(), play.duel.part.mine.iter().any(|o| o.inv_index == key)),
            };
            if offered >= MAX_OFFER && !has_row {
                return refuse("not_ready", format!("the offer holds {MAX_OFFER} items already"));
            }
            confirm::cancel_kind(accept_kind(&w), "changed");
            match w {
                Which::Trade => match play.npcw.trade_state.send_add(*slot, *count as i32) {
                    Some(m) => play.session.send(&m),
                    None => return refuse("stale_handle", "that row is not in the trade's bags now"),
                },
                Which::Wager => {
                    let d = play.duel.part.add_item(key, *count as u32);
                    apply_do(play, d);
                }
            }
            sent()
        }
        A9Action::TradeRemove { slot, id } => {
            if mine_acc {
                return refuse("not_ready", "you have accepted: take the accept back (trade_unaccept) before changing the offer");
            }
            confirm::cancel_kind(accept_kind(&w), "changed");
            match w {
                Which::Trade => {
                    let Some(o) = play.npcw.trade_state.mine.get(*slot) else {
                        return refuse("stale_handle", "that row is not in your offer now");
                    };
                    if o.item.item.id != *id {
                        return refuse("stale_handle", "that row holds a different item now");
                    }
                    match play.npcw.trade_state.send_remove(*slot) {
                        Some(m) => play.session.send(&m),
                        None => return refuse("stale_handle", "that row is not in your offer now"),
                    }
                }
                Which::Wager => {
                    let Some(o) = play.duel.part.mine.get(*slot) else {
                        return refuse("stale_handle", "that row is not in your offer now");
                    };
                    if o.item.id != *id {
                        return refuse("stale_handle", "that row holds a different item now");
                    }
                    let d = play.duel.part.remove_item(o.inv_index);
                    apply_do(play, d);
                }
            }
            sent()
        }
        A9Action::TradeMoney { amount } => {
            if mine_acc {
                return refuse("not_ready", "you have accepted: take the accept back (trade_unaccept) before changing the offer");
            }
            let coin = purse(play);
            if *amount > i64::from(coin) {
                return refuse("out_of_range", format!("you carry {coin}: offer that much or less"));
            }
            confirm::cancel_kind(accept_kind(&w), "changed");
            match w {
                Which::Trade => {
                    let m = play.npcw.trade_state.send_money(*amount as i32);
                    play.session.send(&m);
                }
                Which::Wager => {
                    if let Some(d) = play.duel.part.wager_tunar(*amount as u32, coin.max(0) as u32) {
                        apply_do(play, d);
                    }
                }
            }
            sent()
        }
        A9Action::TradeAccept => {
            if mine_acc {
                return refuse("busy", "you have accepted already");
            }
            let (duel, partner, gives, gets, digest) = describe_offers(play, &w);
            let text = confirm::trade_accept_text(duel, &partner, &gives, &gets);
            let Some(vm) = caller() else { return refuse("not_ready", "no mod is asking") };
            let addon = action_broker::dispatch_addon().unwrap_or_default();
            let kind = if duel { "duel" } else { "trade" };
            let session = window_sessions::get(vm, kind).map(|s| s.id);
            match confirm::open(vm, &addon, session, Act::TradeAccept { duel, digest }, text, 0) {
                Ok(id) => DispatchOutcome::Completed { detail: Some(json!({"asked": true, "confirm": id})) },
                Err((code, msg)) => refuse(code, msg),
            }
        }
        A9Action::TradeUnaccept => {
            if !mine_acc {
                return refuse("not_ready", "you have not accepted");
            }
            confirm::cancel_kind(accept_kind(&w), "unaccepted");
            match w {
                Which::Trade => {
                    trade_pass::unmark_my_accept(play);
                    trade_pass::trade_send(play, 7);
                }
                Which::Wager => {
                    let d = play.duel.part.unaccept();
                    apply_do(play, d);
                }
            }
            sent()
        }
        A9Action::TradeCancel => {
            confirm::cancel_kind(accept_kind(&w), "cancelled");
            match w {
                Which::Trade => {
                    trade_pass::trade_cancel(play);
                    local(|l| l.reason = Some("cancelled"));
                    check_closed(play);
                }
                Which::Wager => {
                    for d in play.duel.part.cancel_duel() {
                        apply_do(play, d);
                    }
                    end_wager(play, "cancelled");
                }
            }
            sent()
        }
        _ => refuse("unsupported", "not a trade action"),
    }
}

fn accept_kind(w: &Which) -> &'static str {
    match w {
        Which::Trade => "trade_accept",
        Which::Wager => "duel_accept",
    }
}

fn line(it: &NetItem) -> String {
    let n = it.quantity.max(1);
    if n > 1 { format!("{n} x {}", it.item.name) } else { it.item.name.clone() }
}

/// What each side gives, as lines for the question, and a digest of it for the Yes to be checked against.
fn describe_offers(play: &Play, w: &Which) -> (bool, String, Vec<String>, Vec<String>, u64) {
    let (duel, partner, mine, theirs, my_money, their_money): (bool, String, Vec<&NetItem>, Vec<&NetItem>, i64, i64) = match w {
        Which::Trade => {
            let st = &play.npcw.trade_state;
            let other = if st.with != u32::MAX { st.with } else { st.partner };
            (
                false,
                name_of(play, other),
                st.mine.iter().map(|o| &o.item).collect(),
                st.theirs.iter().map(|o| &o.item).collect(),
                i64::from(st.my_money),
                i64::from(st.their_money),
            )
        }
        Which::Wager => {
            let part = &play.duel.part;
            let name = local(|l| l.wager.as_ref().map(|h| h.name.clone())).filter(|n| !n.is_empty()).unwrap_or_else(|| name_of(play, part.a));
            (true, name, part.mine.iter().collect(), part.theirs.iter().collect(), i64::from(part.my_tunar), i64::from(part.their_tunar))
        }
    };
    let mut gives: Vec<String> = mine.iter().map(|it| line(it)).collect();
    let mut gets: Vec<String> = theirs.iter().map(|it| line(it)).collect();
    if my_money > 0 {
        gives.push(format!("{my_money} tunar"));
    }
    if their_money > 0 {
        gets.push(format!("{their_money} tunar"));
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    duel.hash(&mut h);
    for it in mine.iter().chain(theirs.iter()) {
        (it.inv_index, it.item.id, it.quantity).hash(&mut h);
    }
    (mine.len(), theirs.len(), my_money, their_money).hash(&mut h);
    (duel, partner, gives, gets, h.finish())
}

/// A Yes to the accept question: carried out only if both offers are still what the question showed.
pub(crate) fn carry_out(play: &mut Play, act: &Act) -> Result<(), String> {
    let Act::TradeAccept { duel, digest } = act else { return Err("not an A9 confirmation".into()) };
    let w = if *duel { Which::Wager } else { Which::Trade };
    let held = if *duel { local(|l| l.wager.is_some()) } else { local(|l| l.trade.is_some()) };
    if !held {
        return Err(if *duel { "the duel's wager is no longer open" } else { "the trade is no longer open" }.into());
    }
    if play.is_dead() {
        return Err("you are dead".into());
    }
    let (_, _, _, _, now) = describe_offers(play, &w);
    if now != *digest {
        return Err("what is on offer changed since the question was asked".into());
    }
    match w {
        Which::Trade => {
            if play.npcw.trade_state.with == u32::MAX {
                return Err("the trade is over".into());
            }
            trade_pass::mark_my_accept(play);
            trade_pass::trade_send(play, 6);
        }
        Which::Wager => {
            let d = play.duel.part.accept_wager();
            apply_do(play, d);
        }
    }
    a9::emit_event("trade.accepted", json!({"kind": if *duel { "duel" } else { "trade" }, "side": "mine"}));
    refresh(play);
    Ok(())
}

fn repair(play: &mut Play, slot: usize, id: i64) -> DispatchOutcome {
    let Some(h) = local(|l| l.smith.clone()).filter(|h| Some(h.hold.vm) == caller() && holds(TakeoverKind::Blacksmith)) else {
        return refuse("not_ready", "hold the 'blacksmith' window first (eqoa.windows.takeover) and wait for the smith");
    };
    if h.pending.is_some() {
        return refuse("busy", "the last repair has not been answered yet");
    }
    let Some(it) = smith_items(play).get(slot).map(|it| (*it).clone()) else {
        return refuse("stale_handle", "that row is not in the smith's list now");
    };
    if it.item.id != id {
        return refuse("stale_handle", "that row holds a different item now");
    }
    let cost = crate::blacksmith::repair_cost(&it);
    if cost <= 0 {
        return refuse("not_ready", "that item needs no repair");
    }
    let coin = purse(play);
    if cost > coin {
        return refuse("not_ready", format!("repairing it costs {cost} and you carry {coin}"));
    }
    // `Send0062`: SendOpIntInt(98, the item's slot id, the smith)
    let mut w = net::stream::Writer::op(0x0062);
    w.u32(it.inv_index).u32(smith_pass::smith_id(play));
    play.session.send(&w.data);
    local(|l| {
        if let Some(s) = l.smith.as_mut() {
            s.pending = Some((it.inv_index, it.cur_hp, coin, Instant::now()));
        }
    });
    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "confirmed": false, "cost": cost})) }
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    local(|l| *l = Local::default());
    a9::clear_all();
}

/// A held request has waited `by` longer (tests).
#[cfg(test)]
pub(crate) fn age_for_test(by: Duration) {
    local(|l| {
        for h in [l.ask.as_mut(), l.trade.as_mut(), l.wager.as_mut()].into_iter().flatten() {
            h.since -= by;
        }
        if let Some(i) = l.invite.as_mut() {
            i.hold.since -= by;
        }
    });
}
