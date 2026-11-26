// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A11 on `Play`: the mercenary windows, the dye picker and fishing for a mod (`addons::native_a11`). A child of
//! the Play adapter, so it reads `Play`'s private state (`merc`, `dyepick`, `fish`); upstream's three files carry one
//! hook each (`mercplay.rs`, `dyeplay.rs`, `fishplay.rs`), all asking [`blocked`], and `dyeplay.rs` one visibility.
//!
//! **What this round does.** A mod reads what the windows hold (`eqoa.merc.get`, `eqoa.dye.get`, `eqoa.fishing.get`)
//! and does what their buttons do, through the game's own request builders and keys, with the game's own rules
//! enforced here in the client, not in a mod's Lua. A mod may also hold them (`eqoa.windows.takeover`, scope `all`):
//! the window the server opens is then the mod's session (`dialog_sessions`), and a held fishing window's cast meter
//! runs in [`hidden_fishing_frame`]. With no mod holding one, the windows are the game's, as before.
//!
//! **Windows the server opens over a hidden session.** A hidden bank, corpse, merchant, trade, duel or blacksmith leaves the
//! game in mode 0, so a window the server opens (a mercenary's inventory or spells, the dye picker, a fishing
//! session) saw "plain play" and opened over a session that, in the game, holds the player in a window of its own.
//! [`blocked`] says what the game does when any window is up, and the three hooks make them do exactly that: the
//! merc's windows wait and open when the hidden session ends, the dye
//! picker waits where the game keeps it waiting (it opens when the hidden session ends), and a fishing session is
//! cancelled to the server as the game cancels one that starts under another window.
use super::Play;
use crate::dyepicker::PickerUp;
use crate::fork_ext::addons::{
    action_broker::DispatchOutcome,
    confirm::Act,
    native_a11::{self, A11Action, Read},
    dialog_sessions::{self, Item},
    native_items::is_handle_fresh,
    window_sessions,
    window_takeover::{self, TakeoverKind},
};
use net::mercwin::{CARRIED, HEIRLOOM, OPEN, SLOTS, STOCK, WORN};
use serde_json::{Value as Json, json};
use std::cell::RefCell;

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

#[derive(Default)]
struct Local {
    /// The window a block was last logged for, so a window waiting does not log each frame.
    logged: Option<&'static str>,
    /// What the fishing session looked like when its last reel, keep or pull went out: `(state, catch count)`. Another
    /// of the three waits until the server has changed one of them (nothing is replayed).
    fish_sent: Option<(i32, usize)>,
    /// Mercenary windows (mode, passed-to-the-game) waiting to open: the server asked while a hidden session or another
    /// window held the game, or a mod gave one back. A passed one opens natively whoever holds the kind; the rest
    /// become a mod's session when a mod holds the kind. At most one of each mode.
    merc_waits: Vec<(i32, bool)>,
    /// A fishing window given back to the game, waiting for plain play.
    fish_wait: bool,
    pre_fish_event: bool,
    /// The dye picker a mod holds (its Open as the server sent it).
    dye: Option<net::dyepick::Open>,
    /// A held dye picker's token whose Cancel (`net::dyepick::cancelled`) is owed after zoning/teleport.
    dye_cancel_owed: Option<u32>,
    /// What the game looked like when the packet now being handled arrived.
    pre_mode: i32,
    pre_dye_event: bool,
    pre_dye_close: Option<u32>,
    was_dead: bool,
    /// Last `native_a11::world_gen()` seen: catches any `window_takeover::release_all()` that bypassed `begin_game_event`.
    world_gen: u64,
}

thread_local! {
    static LOCAL: RefCell<Local> = RefCell::new(Local::default());
}

fn clear_world_local(l: &mut Local, epoch: u64, owe_dye_cancel: bool) {
    l.logged = None;
    l.fish_sent = None;
    l.merc_waits.clear();
    l.fish_wait = false;
    l.pre_fish_event = false;
    if owe_dye_cancel {
        if let Some(o) = l.dye.take() {
            l.dye_cancel_owed = Some(o.token);
        }
    } else {
        l.dye = None;
        l.dye_cancel_owed = None;
    }
    l.pre_dye_event = false;
    l.pre_dye_close = None;
    l.world_gen = epoch;
}

/// Drops local state if `window_takeover::release_all` ran since the last call.
fn sync_world() {
    let epoch = native_a11::world_gen();
    LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        if l.world_gen != epoch {
            clear_world_local(&mut l, epoch, true);
        }
    });
}

/// Sends a Cancel owed for a held dye picker that was open when zoning/teleporting (matching native `run_dyepick`).
fn flush_owed_dye_cancel(play: &mut Play) {
    let token = LOCAL.with(|l| l.borrow_mut().dye_cancel_owed.take());
    if let Some(t) = token
        && play.disconnected.is_none()
        && !play.quit
        && play.session.self_id.is_some()
    {
        play.session.send(&net::dyepick::cancelled(t));
    }
}

/// Called from `begin_game_event` on every `WorldEntry`, `Teleport` and `Disconnected` (same-world included), right
/// after `api_capture` / `window_takeover::release_all` ended any held sessions with `"zoning"`.
pub(crate) fn world_changed(connected: bool) {
    let epoch = native_a11::world_gen();
    LOCAL.with(|l| clear_world_local(&mut l.borrow_mut(), epoch, connected));
    native_a11::clear_all();
}

/// A mod holds a session of this server window now.
fn up(kind: TakeoverKind) -> bool {
    dialog_sessions::windows().iter().any(|a| a.item.kind() == kind)
}

/// The one answer to "does a mod hold a window the game would count as up while its own mode reads 0": a hidden
/// corpse, merchant or bank session, a hidden trade, duel or blacksmith window (A9), a held NPC dialog, offer or
/// server window (A2, A11: `dialog_sessions`), and a Start menu a mod shows (A6; not the chat line or the map, which the game does not count). A window a mod opens
/// as a view of its own (Bags, Journal, the crafting and mail windows) hides nothing and does not count. Every round's
/// "open natively, or wait" decision asks this, and nothing else.
pub(crate) fn mod_window_up() -> bool {
    TakeoverKind::ALL.iter().any(|k| window_takeover::is_hidden_active(*k))
        || super::a9_play::hidden_active()
        || dialog_sessions::any_up()
        || window_sessions::client_window_up()
}

/// The server opens a mercenary window (`mode`) while a hidden session holds the game: it waits.
pub(crate) fn merc_waits(mode: i32) -> bool {
    sync_world();
    let wait = blocked("mercenary window");
    if wait {
        LOCAL.with(|l| {
            let mut l = l.borrow_mut();
            if !l.merc_waits.iter().any(|(m, _)| *m == mode) {
                l.merc_waits.push((mode, false));
            }
        });
    }
    wait
}

/// A fishing session starts while a hidden session holds the game: the game's cancel goes out (the caller), and the
/// player is told why, in one line.
pub(crate) fn fishing_blocked(play: &mut Play) -> bool {
    // a mod that holds fishing takes the session whatever hidden session runs: it is the mod's window, not a game window
    if window_takeover::is_taken(TakeoverKind::Fishing) {
        return false;
    }
    let wait = blocked("fishing session");
    if wait {
        play.add_chat("Fishing stopped: a mod window was busy, cast again".into(), [1.0, 0.25, 0.25, 1.0]);
    }
    wait
}

/// A hidden session of a mod (a corpse, merchant, bank, trade, duel or blacksmith) holds the game in a window of its own while the game's mode
/// reads 0. True means "act as the game does with a window up". `what` names the window for the log.
pub(crate) fn blocked(what: &'static str) -> bool {
    let held = mod_window_up();
    LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        if held {
            if l.logged != Some(what) {
                l.logged = Some(what);
                window_takeover::log(format!(
                    "takeover: the server's {what} arrived while a hidden session holds the game: treated as a window up (as the game treats it over any window)"
                ));
            }
        } else {
            l.logged = None;
        }
    });
    held
}

/// The world is let go: nothing of these windows is left to read.
pub(crate) fn dropped() {
    let epoch = native_a11::world_gen();
    LOCAL.with(|l| *l.borrow_mut() = Local { world_gen: epoch, ..Local::default() });
    native_a11::clear_all();
}

fn kind_name(kind: u8) -> &'static str {
    match kind {
        CARRIED => "carried",
        WORN => "worn",
        HEIRLOOM => "heirloom",
        STOCK => "stock",
        _ => "other",
    }
}

fn merc_state(play: &Play) -> Option<Json> {
    if play.merc.window.is_none() && play.merc.spells.is_none() {
        return None;
    }
    let inventory = play.merc.window.as_ref().map(|w| {
        json!({
            "open": play.mode == crate::play::mercplay::ITEMS_MODE || up(TakeoverKind::MercItems),
            "slot": w.slot,
            "name": w.name,
            "class": w.class,
            "race": w.race,
            "level": w.level,
            "hp": w.hp,
            "max_hp": w.max_hp,
            "power": w.power,
            "max_power": w.max_power,
            "ac": w.ac,
            "stats": w.stats,
            "most": w.most,
            "records": w.records.iter().map(|r| json!({
                "id": r.id,
                "kind": kind_name(r.kind),
                "item_id": r.item.item.id.to_string(),
                "name": r.item.item.name,
                "quantity": r.item.quantity.max(1),
                "equip_slot": r.item.equip_slot,
            })).collect::<Vec<_>>(),
        })
    });
    let spells = play.merc.spells.as_ref().map(|s| {
        json!({
            "open": play.mode == crate::play::mercplay::SPELLS_MODE || up(TakeoverKind::MercSpells),
            "slot": s.slot,
            "name": s.name,
            "class": s.class,
            "level": s.level,
            "set": s.set(),
            // spell ids are ability `Id` strings, as `Spell.id`; "0" is an empty slot (the game's own 0)
            "slots": s.slots.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
            "spells": s.spells.iter().map(|sp| json!({
                "id": sp.ability.id.to_string(),
                "name": sp.ability.name,
                "slot": s.slots.iter().position(|&id| id as i64 == sp.ability.id && id != 0).map_or(Json::Null, |k| json!(k + 1)),
            })).collect::<Vec<_>>(),
        })
    });
    Some(json!({
        "revision": 0,
        "available": play.session.merc_windows,
        "own_out": play.session.mercs.as_ref().is_some_and(|m| m.own),
        "waiting": play.merc.waiting,
        "inventory": inventory,
        "spells": spells,
    }))
}

fn dye_state(play: &Play) -> Option<Json> {
    sync_world();
    let held = LOCAL.with(|l| l.borrow().dye.clone());
    let o = &held.or_else(|| play.dyepick.open.clone().filter(|_| play.dyepick.picker.open && play.mode == crate::play::dyeplay::MODE))?;
    Some(json!({
        "revision": 0,
        "token": o.token,
        "target": match o.target {
            net::dyepick::Target::Character => "character",
            net::dyepick::Target::Pet => "pet",
            net::dyepick::Target::Aura => "aura",
        },
        "title": o.title,
        "slots": o.slots,
        "current": o.current,
        "alpha": o.alpha,
        "open": true,
        "no_dye": play.session.dye_picker_version >= 2 && o.target != net::dyepick::Target::Aura,
    }))
}

fn wide(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}

fn state_name(state: i32) -> &'static str {
    use crate::fishing::state;
    match state {
        state::OFF => "off",
        state::WAITING => "waiting",
        state::CAST => "cast",
        state::CAUGHT => "caught",
        state::KEEPING => "keeping",
        _ => "other",
    }
}

fn fishing_state(play: &Play) -> Option<Json> {
    let state = play.fish.part.state;
    if state == crate::fishing::state::OFF && play.mode != 36 && !up(TakeoverKind::Fishing) {
        return None;
    }
    let w = &play.fish.wnd;
    Some(json!({
        "revision": 0,
        "open": play.mode == 36 || up(TakeoverKind::Fishing),
        "state": state_name(state),
        "casting": w.casting,
        // the meter the game's own window shows, 0..120; a mod that draws its own meter reads this one and the player
        // releases it, as in the game's window (nothing here picks the power)
        "power": w.power,
        "ready": w.ready,
        "perfect_from": 56,
        "perfect_to": 63,
        "fair_from": 41,
        "fair_to": 78,
        "list": w.list_visible || (up(TakeoverKind::Fishing) && state == crate::fishing::state::CAUGHT),
        "catch": play.fish.part.catch.items.iter().enumerate().map(|(i, it)| json!({
            "slot": i + 1,
            "name": it.item.name,
            "quantity": it.quantity,
        })).collect::<Vec<_>>(),
        "message": wide(&w.text),
    }))
}

/// Each step: a mercenary window that waited opens once no hidden session holds the game; the fishing guard lets go
/// when the server has answered; and what the three windows hold, for `eqoa.merc.get`, `eqoa.dye.get` and
/// `eqoa.fishing.get`.
pub(crate) fn tick(play: &mut Play, game_step: bool) {
    sync_world();
    flush_owed_dye_cancel(play);
    if !play.in_world_now() || play.world_change.is_some() {
        native_a11::clear_all();
        return;
    }
    resume_merc(play);
    resume_fishing(play);
    // If a mod went away or released in the same step the character died, `dialog_sessions::death()` wiped `give_back`
    // before `give_window` ran; cancel and drop any orphaned dye token so it does not linger.
    if !dialog_sessions::tracks_window(TakeoverKind::DyePicker)
        && let Some(o) = LOCAL.with(|l| l.borrow_mut().dye.take())
    {
        play.session.send(&net::dyepick::cancelled(o.token));
    }
    if game_step {
        tick_windows(play);
    }
    // the one-request guard holds only until the server's answer has changed the state or the catch, or the session ended
    let now = (play.fish.part.state, play.fish.part.catch.items.len());
    LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        if l.fish_sent.is_some_and(|sent| sent != now || now.0 == crate::fishing::state::OFF) {
            l.fish_sent = None;
        }
    });
    // nothing is read while no mod listens
    if !game_step || !crate::fork_ext::addons::native_core::capture_requested() {
        return;
    }
    for (which, state) in [(Read::Merc, merc_state(play)), (Read::Dye, dye_state(play)), (Read::Fishing, fishing_state(play))] {
        match state {
            Some(s) => native_a11::publish(which, s),
            None => native_a11::clear(which),
        }
    }
}

fn resume_merc(play: &mut Play) {
    if play.is_dead() {
        // at death the game closes open windows (`msg_005e` -> `set_mode_0`): nothing waiting opens while dead or after
        LOCAL.with(|l| l.borrow_mut().merc_waits.clear());
        return;
    }
    let waits = LOCAL.with(|l| l.borrow().merc_waits.clone());
    if waits.is_empty() {
        return;
    }
    let (items, spells) = (crate::play::mercplay::ITEMS_MODE, crate::play::mercplay::SPELLS_MODE);
    let mut keep: Vec<(i32, bool)> = Vec::new();
    let mut opened = false;
    for (mode, bypass) in waits {
        // the window is still the server's to show while its state is there (every later state update clears the open
        // flag, the gone flag clears the state): the hire gone means nothing to open
        let present = if mode == items { play.merc.window.is_some() } else { play.merc.spells.is_some() };
        if !present {
            continue;
        }
        let kind = if mode == items { TakeoverKind::MercItems } else { TakeoverKind::MercSpells };
        if up(kind) {
            // a fresh mod session already holds this kind: a stale passed wait must not open natively over it
            continue;
        }
        if window_takeover::is_taken(kind) && !bypass {
            // a mod holds the window: it is the mod's session, not the game's window (a hidden session does not matter)
            hold_window(play, kind);
            continue;
        }
        if opened || mod_window_up() || play.mode != 0 {
            keep.push((mode, bypass));
            continue;
        }
        opened = true;
        play.merc.keys.clear();
        if mode == items {
            play.merc.items_open = true;
        } else {
            play.merc.book_open = true;
        }
        play.mode = mode;
        window_takeover::log("takeover: the mercenary window that waited opens now".into());
    }
    LOCAL.with(|l| l.borrow_mut().merc_waits = keep);
}

/// The game's fishing window opened again for a session a mod gave back: the window as the game shows it, with the
/// catch's list when something is on the line (`Recv::Loot` opens it, and focusing the window shuts it).
fn open_fishing_window(play: &mut Play) {
    play.mode = 36;
    let mut cues = Vec::new();
    play.fish.wnd.focus(&mut cues);
    let st = play.fish.part.state;
    if (st == crate::fishing::state::CAUGHT || st == crate::fishing::state::KEEPING) && !play.fish.part.catch.items.is_empty() {
        let now_ms = play.hud.as_ref().map_or(0, |h| h.0.time);
        play.fish.wnd.open_bait(play.wnd_chat.as_mut(), now_ms, &mut cues);
    }
    play.cues.extend(cues.into_iter().map(crate::sound::Cue::Ui));
}

fn resume_fishing(play: &mut Play) {
    if !LOCAL.with(|l| l.borrow().fish_wait) {
        return;
    }
    if play.is_dead() || play.fish.part.state == crate::fishing::state::OFF || up(TakeoverKind::Fishing) {
        LOCAL.with(|l| l.borrow_mut().fish_wait = false);
    } else if play.mode == 0 && !mod_window_up() {
        LOCAL.with(|l| l.borrow_mut().fish_wait = false);
        open_fishing_window(play);
    }
}

// ---- the windows a mod holds (takeover kinds `mercenary_items`, `mercenary_spells`, `dye_picker`, `fishing`) ----------

/// The session's `data`: the same record the window's read returns.
fn window_data(play: &Play, kind: TakeoverKind) -> Json {
    let mut v = match kind {
        TakeoverKind::MercItems | TakeoverKind::MercSpells => merc_state(play).unwrap_or(Json::Null),
        TakeoverKind::DyePicker => dye_state(play).unwrap_or(Json::Null),
        _ => fishing_state(play).unwrap_or(Json::Null),
    };
    if let Some(o) = v.as_object_mut() {
        o.remove("revision");
        // the meter changes every frame: a mod reads it with `eqoa.fishing.get()`, it is not a session change
        o.remove("power");
        if kind == TakeoverKind::MercItems {
            o.remove("spells");
        } else if kind == TakeoverKind::MercSpells {
            o.remove("inventory");
        }
    }
    v
}

fn hold_window(play: &mut Play, kind: TakeoverKind) {
    // a fresh session supersedes any earlier waiting/passed native open of the same kind
    match kind {
        TakeoverKind::MercItems => LOCAL.with(|l| l.borrow_mut().merc_waits.retain(|(m, _)| *m != crate::play::mercplay::ITEMS_MODE)),
        TakeoverKind::MercSpells => LOCAL.with(|l| l.borrow_mut().merc_waits.retain(|(m, _)| *m != crate::play::mercplay::SPELLS_MODE)),
        TakeoverKind::Fishing => LOCAL.with(|l| l.borrow_mut().fish_wait = false),
        _ => {}
    }
    let pos = play.self_at.map(|(p, _)| p);
    let data = window_data(play, kind);
    if matches!(kind, TakeoverKind::MercItems | TakeoverKind::MercSpells) {
        // the native window resets this as it opens: one lost answer must not block the mod for good
        play.merc.waiting = false;
    }
    dialog_sessions::hold(Item::Window { kind, data }, pos);
    window_takeover::log(format!("takeover: {} HIDDEN for the mod (a window the server opened; scope all)", kind.name()));
}

/// Before the game handles a packet: what it looks like, and whether the packet is the dye picker's.
pub(crate) fn pre_event(play: &Play, event: &net::session::Event) {
    sync_world();
    let mut dye_open = false;
    let mut dye_close = None;
    if let net::session::Event::Opcode { op, data } = event
        && *op == net::dyepick::OP_DYE_PICKER
    {
        match net::dyepick::read(data) {
            Some(net::dyepick::Message::Open(_)) => dye_open = true,
            Some(net::dyepick::Message::Close(t)) => dye_close = Some(t),
            None => {}
        }
    }
    let fish = matches!(event, net::session::Event::Opcode { op, .. } if *op == crate::fishing::OP_IN);
    LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        l.pre_mode = play.mode;
        l.pre_dye_event = dye_open;
        l.pre_dye_close = dye_close;
        l.pre_fish_event = fish;
    });
}

/// After the game handled the packet: what it opened is taken for the mod that holds its kind (the game's mode is put
/// back, as A2 does for a dialog), and a close from the server ends the mod's session.
pub(crate) fn post_event(play: &mut Play) {
    sync_world();
    flush_owed_dye_cancel(play);
    let (pre, dye_open, dye_close) = LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        // any answer of the fishing server lets the one-request guard go, even one that changes nothing (a full pack)
        if std::mem::take(&mut l.pre_fish_event) {
            l.fish_sent = None;
        }
        (l.pre_mode, std::mem::take(&mut l.pre_dye_event), l.pre_dye_close.take())
    });
    if play.is_dead() || !window_takeover::has_activity() {
        return;
    }
    let (items, spells) = (crate::play::mercplay::ITEMS_MODE, crate::play::mercplay::SPELLS_MODE);
    for (mode, kind) in [(items, TakeoverKind::MercItems), (spells, TakeoverKind::MercSpells)] {
        if !window_takeover::is_taken(kind) {
            continue;
        }
        let opened = play.mode == mode && pre != mode;
        // a window passed to the game (the bypass) is the game's whoever holds the kind
        let waiting = LOCAL.with(|l| l.borrow().merc_waits.iter().any(|(m, bypass)| *m == mode && !*bypass));
        if opened || waiting {
            if opened {
                play.mode = pre;
                play.merc.items_open = false;
                play.merc.book_open = false;
                play.merc.keys.clear();
            }
            LOCAL.with(|l| l.borrow_mut().merc_waits.retain(|(m, _)| *m != mode));
            hold_window(play, kind);
        }
    }
    if dye_open && window_takeover::is_taken(TakeoverKind::DyePicker) {
        if let Some(o) = play.dyepick.pending.take() {
            // the game cancels the picker it replaces
            if let Some(old) = LOCAL.with(|l| l.borrow_mut().dye.replace(o.clone())).filter(|old| old.token != o.token) {
                play.session.send(&net::dyepick::cancelled(old.token));
            }
            hold_window(play, TakeoverKind::DyePicker);
        }
    }
    if let Some(t) = dye_close
        && LOCAL.with(|l| l.borrow().dye.as_ref().is_some_and(|o| o.token == t))
    {
        LOCAL.with(|l| l.borrow_mut().dye = None);
        end_window(TakeoverKind::DyePicker, "server");
    }
    // a fishing session the game just opened (mode 36)
    if window_takeover::is_taken(TakeoverKind::Fishing) && play.mode == 36 && pre != 36 {
        play.mode = pre;
        play.fish.wnd.blur();
        hold_window(play, TakeoverKind::Fishing);
    }
}

fn end_window(kind: TakeoverKind, reason: &str) {
    if let Some(a) = dialog_sessions::windows().into_iter().find(|a| a.item.kind() == kind) {
        dialog_sessions::finish(&a.id, reason);
    }
}

/// The game's window for a session that ended with no answer (passed, released, disabled, idle): as the game would
/// have shown it.
pub(crate) fn give_window(play: &mut Play, kind: TakeoverKind) {
    sync_world();
    if play.is_dead() {
        if kind == TakeoverKind::DyePicker
            && let Some(o) = LOCAL.with(|l| l.borrow_mut().dye.take())
        {
            play.session.send(&net::dyepick::cancelled(o.token));
        }
        return;
    }
    match kind {
        TakeoverKind::MercItems | TakeoverKind::MercSpells => {
            // whatever the last state's flags say, the game's window opens (it waits for plain play, as the dye picker
            // does, and several given back together all wait)
            let mode = if kind == TakeoverKind::MercItems { crate::play::mercplay::ITEMS_MODE } else { crate::play::mercplay::SPELLS_MODE };
            LOCAL.with(|l| {
                let mut l = l.borrow_mut();
                l.merc_waits.retain(|(m, _)| *m != mode);
                l.merc_waits.push((mode, true));
            });
        }
        TakeoverKind::DyePicker => {
            if let Some(o) = LOCAL.with(|l| l.borrow_mut().dye.take()) {
                play.dyepick.pending = Some(o);
            }
        }
        _ => {
            if play.fish.part.state != crate::fishing::state::OFF {
                if play.mode == 0 && !mod_window_up() {
                    open_fishing_window(play);
                } else {
                    // another window is up: the session waits for plain play, nothing is cancelled
                    LOCAL.with(|l| l.borrow_mut().fish_wait = true);
                    play.add_chat("Fishing: the fishing window opens when the other window closes".into(), [1.0, 1.0, 1.0, 1.0]);
                }
            }
        }
    }
}

/// The mod closes its window with `close_window(ref)`: what the game's own close does, so the server's state ends as
/// it ends there.
pub(crate) fn close_window(play: &mut Play, kind: TakeoverKind) -> bool {
    match kind {
        TakeoverKind::MercItems => {
            let slot = play.merc.window.as_ref().map_or(0, |w| w.slot);
            play.session.send(&net::mercwin::ask(slot, net::mercwin::ASK_CLOSE, 0, 0));
        }
        TakeoverKind::MercSpells => {}
        TakeoverKind::DyePicker => {
            if let Some(o) = LOCAL.with(|l| l.borrow_mut().dye.take()) {
                play.session.send(&net::dyepick::cancelled(o.token));
            }
        }
        _ => {
            let st = play.fish.part.state;
            if st != crate::fishing::state::OFF {
                play.session.send(&crate::fishing::sub_msg(if st == crate::fishing::state::CAUGHT { 4 } else { 2 }));
            }
            // as the game's Escape: with a catch on the line it is let go and the window stays until the server stops the
            // session (close again, or the server's Stop, ends it)
            return st != crate::fishing::state::CAUGHT;
        }
    }
    true
}

/// Each step for the held windows: what ended on the server's side ends the session; a death ends them as the game's
/// own windows end; changed data is told to the mod.
fn tick_windows(play: &mut Play) {
    let wins = dialog_sessions::windows();
    if wins.is_empty() {
        LOCAL.with(|l| l.borrow_mut().was_dead = play.is_dead());
        return;
    }
    let dead = play.is_dead();
    if dead && !LOCAL.with(|l| std::mem::replace(&mut l.borrow_mut().was_dead, true)) {
        for a in &wins {
            close_window(play, a.item.kind());
            dialog_sessions::finish(&a.id, "death");
        }
        LOCAL.with(|l| l.borrow_mut().dye = None);
        return;
    }
    LOCAL.with(|l| l.borrow_mut().was_dead = dead);
    for a in wins {
        let kind = a.item.kind();
        let gone = match kind {
            TakeoverKind::MercItems => play.merc.window.is_none(),
            TakeoverKind::MercSpells => play.merc.spells.is_none(),
            TakeoverKind::DyePicker => LOCAL.with(|l| l.borrow().dye.is_none()),
            _ => play.fish.part.state == crate::fishing::state::OFF,
        };
        if gone {
            if kind == TakeoverKind::DyePicker {
                LOCAL.with(|l| l.borrow_mut().dye = None);
            }
            dialog_sessions::finish(&a.id, "server");
            continue;
        }
        if kind == TakeoverKind::Fishing {
            use crate::fishing::state;
            // something on the line, or a cast under way, is the mod at work: the idle limit does not run
            if play.fish.wnd.casting || matches!(play.fish.part.state, state::CAST | state::CAUGHT | state::KEEPING) {
                dialog_sessions::touch(&a.id);
            }
            // as a held dialog: more than 25 units from where it opened ends it with the game's own cancel
            if let (Some(from), Some((now, _))) = (a.pos, play.self_at) {
                let d = [now[0] - from[0], now[1] - from[1], now[2] - from[2]];
                if ee::lt(625.0, d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) {
                    play.session.send(&crate::fishing::sub_msg(2));
                    window_takeover::log("takeover: fishing ended (walked away more than 25 units)".into());
                    dialog_sessions::finish(&a.id, "walked_away");
                    continue;
                }
            }
        }
        let data = window_data(play, kind);
        if data != a.item.data() {
            dialog_sessions::refresh_window(&a.id, data);
        }
    }
}

/// A fishing window a mod holds has no drawing to run its meter: the window's own cast logic (the `draw` block, in
/// what it asks of the game) runs here each frame instead, on the same clock, so the power is only ever what the
/// game's meter shows when the throw happens. The left stick is the mod's, not the window's: it reads 0.
pub(crate) fn hidden_fishing_frame(play: &mut Play, walker: &crate::walker::Walker) {
    let c = &walker.cam.walk.committed;
    hidden_fishing_step(play, (c.smooth_location(), c.angles));
}

/// The cast logic of a held fishing window, at the walk's committed place and heading.
pub(crate) fn hidden_fishing_step(play: &mut Play, place: (vi::vect::Vect3<vi::fpu::Ee>, vi::vect::Vect3<vi::fpu::Ee>)) {
    if play.mode == 36 || !up(TakeoverKind::Fishing) || !play.fish.wnd.casting || play.fish.part.state == crate::fishing::state::OFF {
        return;
    }
    let now = play.session.timer();
    if play.fish.wnd.tick(now, vi::fpu::F::<vi::fpu::Ee>::new(0.0)) {
        // `Splash`: PlayAction(65), and FindWater from the walk's committed place and heading
        if play.session.self_id.is_some() {
            play.session.send_op_byte(0x0047, 65);
        }
        play.emote = Some(65);
        let actor = play.self_actor.map_or(-1, |a| a.actor as i32);
        play.fish.water = Some((actor, place.0, place.1));
    }
    if !play.fish.wnd.casting {
        let say = |play: &mut Play, id: i32| {
            let text = play.words.as_ref().map_or_else(|| format!("(word {id})"), |w| w.get(id, 0).to_string());
            let now = play.session.timer();
            play.fish.wnd.set_text(&crate::ui::wide(&text), now);
        };
        if play.fish.wnd.hit {
            let (w, m) = play.fish.part.cast(&play.dump.inventory, play.fish.wnd.quality);
            say(play, w);
            if let Some(m) = m {
                play.session.send(&m);
            }
        } else {
            say(play, 1421);
        }
    }
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    let epoch = native_a11::world_gen();
    LOCAL.with(|l| *l.borrow_mut() = Local { world_gen: epoch, ..Local::default() });
    native_a11::clear_all();
}

fn item_index(play: &Play, item: &crate::fork_ext::addons::native_items::ResolvedItemSpec) -> Result<usize, DispatchOutcome> {
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

pub(crate) fn dispatch(play: &mut Play, action: &A11Action) -> DispatchOutcome {
    sync_world();
    if play.is_dead() {
        return refuse("not_ready", "cannot do this while dead");
    }
    if !play.in_world_now() || play.world_change.is_some() {
        return refuse("not_ready", "not in the world");
    }
    match action {
        A11Action::MercGive { .. }
        | A11Action::MercEquip { .. }
        | A11Action::MercUnequip { .. }
        | A11Action::MercTake { .. }
        | A11Action::MercStock { .. }
        | A11Action::MercSpellSet { .. }
        | A11Action::MercSpellsClear
        | A11Action::MercDismiss => dispatch_merc(play, action),
        A11Action::DyePick { .. } | A11Action::DyeNoDye { .. } | A11Action::DyeCancel { .. } => dispatch_dye(play, action),
        _ => dispatch_fishing(play, action),
    }
}

fn sent() -> DispatchOutcome {
    DispatchOutcome::Completed { detail: Some(json!({"sent": true})) }
}

fn dispatch_merc(play: &mut Play, action: &A11Action) -> DispatchOutcome {
    use crate::mercwnd::{Ask, ask_bytes};
    match action {
        A11Action::MercDismiss => {
            // the game's Start menu entry sends at once; the player is asked here first (a hire is spent)
            if !play.session.mercs.as_ref().is_some_and(|m| m.own) {
                return refuse("not_ready", "the server says no mercenary of yours is out");
            }
            super::a1_play::ask(Act::DismissMerc, "Send your mercenary away? It leaves your service.".into(), "")
        }
        _ if !play.session.merc_windows => refuse("unsupported", "this server does not offer the mercenary windows"),
        A11Action::MercSpellSet { slot, spell } => {
            let Some(s) = play.merc.spells.as_ref() else {
                return refuse("not_ready", "the server has not sent the mercenary's spells (its menu: Spells)");
            };
            if usize::from(*slot) >= SLOTS {
                return refuse("out_of_range", "the merc's slots are 1 to 10");
            }
            if *spell != 0 && !s.spells.iter().any(|sp| sp.ability.id == i64::from(*spell)) {
                return refuse("stale_handle", "that spell is not in the mercenary's book");
            }
            let m = net::mercwin::spells_ask(s.slot, net::mercwin::SPELLS_SET, *slot, *spell);
            play.session.send(&m);
            sent()
        }
        A11Action::MercSpellsClear => {
            let Some(s) = play.merc.spells.as_ref() else {
                return refuse("not_ready", "the server has not sent the mercenary's spells (its menu: Spells)");
            };
            let m = net::mercwin::spells_ask(s.slot, net::mercwin::SPELLS_CLEAR, 0, 0);
            play.session.send(&m);
            sent()
        }
        _ => {
            let Some(w) = play.merc.window.clone() else {
                return refuse("not_ready", "the server has not sent the mercenary's inventory (its menu: Items, Inventory)");
            };
            // the game's window asks once and waits for the answer before the next ask
            if play.merc.waiting {
                return refuse("busy", "the server has not answered the last mercenary request");
            }
            let record = |id: u32, kinds: &[u8]| w.records.iter().any(|r| r.id == id && kinds.contains(&r.kind));
            let ask = match action {
                A11Action::MercGive { item } => {
                    if !play.merc_give_on() {
                        return refuse("not_ready", "your mercenary is not out");
                    }
                    let idx = match item_index(play, item) {
                        Ok(i) => i,
                        Err(e) => return e,
                    };
                    let it = &play.dump.inventory.items[idx];
                    if it.equip_slot != -1 {
                        return refuse("not_ready", "a worn item is not given (the game's Give to Merc is offered on items not worn)");
                    }
                    Ask::Give(it.inv_index)
                }
                A11Action::MercEquip { id } => {
                    if !record(*id, &[CARRIED]) {
                        return refuse("stale_handle", "the mercenary carries no such record to equip");
                    }
                    Ask::Equip(*id)
                }
                A11Action::MercUnequip { id } => {
                    if !record(*id, &[WORN]) {
                        return refuse("stale_handle", "the mercenary wears no such record");
                    }
                    Ask::Unequip(*id)
                }
                A11Action::MercTake { id } => {
                    if !record(*id, &[CARRIED, WORN]) {
                        return refuse("stale_handle", "the mercenary holds no such record to take back");
                    }
                    Ask::Take(*id)
                }
                A11Action::MercStock { id, count } => {
                    if !record(*id, &[STOCK]) {
                        return refuse("stale_handle", "no such record in the owner's stock");
                    }
                    Ask::Stock(*id, *count)
                }
                _ => return refuse("unsupported", "not a mercenary item request"),
            };
            play.session.send(&ask_bytes(ask, w.slot));
            play.merc.waiting = true;
            sent()
        }
    }
}

/// The picker the mod's token names, up now: the token is echoed exactly as the server sent it.
fn picker_open(play: &Play, token: u32) -> Result<net::dyepick::Open, DispatchOutcome> {
    sync_world();
    if let Some(o) = LOCAL.with(|l| l.borrow().dye.clone()) {
        if o.token != token {
            return Err(refuse("stale_handle", "that is not the token of the picker now up (eqoa.dye.get().token)"));
        }
        return Ok(o);
    }
    let Some(o) = play.dyepick.open.clone().filter(|_| play.dyepick.picker.open && play.mode == crate::play::dyeplay::MODE) else {
        return Err(refuse("window_closed", "no dye picker is up"));
    };
    if o.token != token {
        return Err(refuse("stale_handle", "that is not the token of the picker now up (eqoa.dye.get().token)"));
    }
    Ok(o)
}

/// The picker answered: through the game's own path when its window is up; for a picker a mod holds, the same
/// bytes (`dyepick_done`'s sends), and the session ends answered.
fn answer_dye(play: &mut Play, up: PickerUp, token: u32) {
    let held = LOCAL.with(|l| l.borrow_mut().dye.take());
    match held {
        None => play.dyepick_done(up),
        Some(o) => {
            let m = match up {
                PickerUp::Pick(c) => net::dyepick::picked(o.token, c),
                PickerUp::NoDye => net::dyepick::no_dye(o.token),
                PickerUp::Cancel => net::dyepick::cancelled(o.token),
            };
            let _ = token;
            play.session.send(&m);
            end_window(TakeoverKind::DyePicker, "answered");
        }
    }
}

fn dispatch_dye(play: &mut Play, action: &A11Action) -> DispatchOutcome {
    match action {
        A11Action::DyePick { token, rgba } => {
            let o = match picker_open(play, *token) {
                Ok(o) => o,
                Err(e) => return e,
            };
            // an alpha the player may not choose is FF, as the game's picker sends it
            let rgba = if o.alpha { *rgba } else { (*rgba & 0xFFFF_FF00) | 0xFF };
            let text = format!("Dye with #{:06X}? The dyer may charge for it.", rgba >> 8);
            super::a1_play::ask(Act::DyeAnswer { token: *token, rgba: Some(rgba) }, text, "dye_picker")
        }
        A11Action::DyeNoDye { token } => {
            let o = match picker_open(play, *token) {
                Ok(o) => o,
                Err(e) => return e,
            };
            if !(play.session.dye_picker_version >= 2 && o.target != net::dyepick::Target::Aura) {
                return refuse("unsupported", "this picker offers no 'No dye' (the game's has none either)");
            }
            super::a1_play::ask(Act::DyeAnswer { token: *token, rgba: None }, "Take the dye off? The dyer may charge to put it on again.".into(), "dye_picker")
        }
        A11Action::DyeCancel { token } => {
            if let Err(e) = picker_open(play, *token) {
                return e;
            }
            answer_dye(play, PickerUp::Cancel, *token);
            sent()
        }
        _ => refuse("unsupported", "not a dye request"),
    }
}

fn dispatch_fishing(play: &mut Play, action: &A11Action) -> DispatchOutcome {
    use crate::fishing::{state, sub_msg};
    let held = up(TakeoverKind::Fishing);
    if play.mode != 36 && !held {
        return refuse("window_closed", "no fishing window is up");
    }
    // a mod that acts is showing its window
    if let Some(a) = dialog_sessions::windows().into_iter().find(|a| a.item.kind() == TakeoverKind::Fishing) {
        dialog_sessions::touch(&a.id);
    }
    let st = play.fish.part.state;
    let catch_len = play.fish.part.catch.items.len();
    let guard_waits = LOCAL.with(|l| l.borrow().fish_sent == Some((st, catch_len)));
    let pressed = || DispatchOutcome::Completed { detail: Some(json!({"pressed": true})) };
    match action {
        // the game's own Cast key: the first press starts the meter, the second throws at the power it then shows. The
        // meter and its clock are the game's window's: a mod cannot choose the power.
        A11Action::FishCast => {
            if play.fish.wnd.casting {
                return refuse("busy", "a cast is under way (fishing_release throws it)");
            }
            if st != state::WAITING {
                return refuse("not_ready", "the line is not ready to cast");
            }
            if held {
                // the held window has no keys: the game's own start of the cast, on its clock
                let now = play.session.timer();
                play.fish.wnd.cast(now);
                play.cues.push(crate::sound::Cue::Ui(11));
            } else {
                play.fishing_key(134, true);
                play.fishing_key(134, false);
            }
            pressed()
        }
        A11Action::FishRelease => {
            if !play.fish.wnd.casting || play.fish.wnd.ready || st != state::WAITING {
                return refuse("not_ready", "no cast is under way");
            }
            if held {
                play.fish.wnd.release = true;
            } else {
                play.fishing_key(134, true);
                play.fishing_key(134, false);
            }
            pressed()
        }
        A11Action::FishReel | A11Action::FishPull | A11Action::FishKeep { .. } => {
            if st != state::CAUGHT || !(play.fish.wnd.list_visible || held) {
                return refuse("not_ready", "nothing is on the line to reel, keep or pull");
            }
            if guard_waits {
                return refuse("busy", "the server has not answered the last fishing request");
            }
            let m = match action {
                A11Action::FishReel => sub_msg(3),
                A11Action::FishPull => sub_msg(4),
                A11Action::FishKeep { slot } => {
                    let Some(it) = play.fish.part.catch.items.get(*slot) else {
                        return refuse("out_of_range", "no such row in the catch");
                    };
                    let x = it.quantity as i32;
                    match play.fish.part.keep(*slot, x) {
                        Some(m) => m,
                        None => return refuse("busy", "a catch is being kept"),
                    }
                }
                _ => return refuse("unsupported", "not a fishing request"),
            };
            play.session.send(&m);
            let after = (play.fish.part.state, catch_len);
            LOCAL.with(|l| l.borrow_mut().fish_sent = Some(after));
            sent()
        }
        // the game's Escape: cancel the session, or pull the line when something is on it
        A11Action::FishCancel => {
            if st == state::OFF {
                return refuse("not_ready", "no fishing session is going");
            }
            play.session.send(&sub_msg(if st == state::CAUGHT { 4 } else { 2 }));
            LOCAL.with(|l| l.borrow_mut().fish_sent = Some((st, catch_len)));
            sent()
        }
        _ => refuse("unsupported", "not a fishing request"),
    }
}

/// What a Yes does for the two steps the player is asked about (`a1_play::carry_out`): the game's own steps, once the
/// state is checked to be what the question named.
pub(crate) fn carry_out(play: &mut Play, act: &Act) -> Result<(), String> {
    match act {
        Act::DismissMerc => {
            if !play.session.mercs.as_ref().is_some_and(|m| m.own) {
                return Err("your mercenary is no longer out".into());
            }
            // the game's own entry: 0x7E5C
            play.merc_action(crate::play::mercplay::MSG_DISMISS_MERC);
            Ok(())
        }
        Act::DyeAnswer { token, rgba } => {
            if let Err(DispatchOutcome::Refused { reason, .. }) = picker_open(play, *token) {
                return Err(reason);
            }
            answer_dye(
                play,
                match rgba {
                    Some(c) => PickerUp::Pick(*c),
                    None => PickerUp::NoDye,
                },
                *token,
            );
            Ok(())
        }
        _ => Err("not an A11 step".into()),
    }
}

#[cfg(test)]
#[path = "a11_play_tests.rs"]
mod tests;
