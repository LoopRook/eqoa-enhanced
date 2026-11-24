// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A11 on `Play`: the mercenary windows, the dye picker and fishing for a mod (`addons::native_a11`). A child of
//! the Play adapter, so it reads `Play`'s private state (`merc`, `dyepick`, `fish`); upstream's three files carry one
//! hook each (`mercplay.rs`, `dyeplay.rs`, `fishplay.rs`), all asking [`blocked`], and `dyeplay.rs` one visibility.
//!
//! **What this round does and does not do.** The three windows stay the game's: the server opens them, the game draws
//! them and the player sees them. A mod reads what they hold (`eqoa.merc.get`, `eqoa.dye.get`, `eqoa.fishing.get`)
//! and does what their buttons do, through the game's own request builders and keys, with the game's own rules
//! enforced here in the client, not in a mod's Lua. A mod cannot hide them or take them over in this version (the
//! kinds stay `unsupported` for `eqoa.windows.takeover`): the fishing window's meter runs inside its drawing, and the
//! merc and dye windows follow the same rule for the sake of one story.
//!
//! **Windows the server opens over a hidden session.** A hidden bank, corpse or merchant session of a mod leaves the
//! game in mode 0, so a window the server opens (a mercenary's inventory or spells, the dye picker, a fishing
//! session) saw "plain play" and opened over a session that, in the game, holds the player in a window of its own.
//! [`blocked`] says what the game does when any window is up, and the three hooks make them do exactly that: the
//! merc's windows are not opened (their state is kept; the server's Items > Inventory menu asks again), the dye
//! picker waits where the game keeps it waiting (it opens when the hidden session ends), and a fishing session is
//! cancelled to the server as the game cancels one that starts under another window.
use super::Play;
use crate::dyepicker::PickerUp;
use crate::fork_ext::addons::{
    action_broker::DispatchOutcome,
    confirm::Act,
    native_a11::{self, A11Action, Read},
    native_items::is_handle_fresh,
    window_takeover::{self, TakeoverKind},
};
use net::mercwin::{CARRIED, HEIRLOOM, SLOTS, STOCK, WORN};
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
}

thread_local! {
    static LOCAL: RefCell<Local> = RefCell::new(Local::default());
}

/// A hidden session of a mod (a corpse, merchant or bank) holds the game in a window of its own while the game's mode
/// reads 0. Asked by the hooks in `mercplay.rs`, `dyeplay.rs` and `fishplay.rs`; true means "act as the game does with
/// a window up". `what` names the window for the log.
pub(crate) fn blocked(what: &'static str) -> bool {
    let held = TakeoverKind::ALL.iter().any(|k| window_takeover::is_hidden_active(*k));
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
    LOCAL.with(|l| *l.borrow_mut() = Local::default());
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
            "open": play.mode == crate::play::mercplay::ITEMS_MODE,
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
                "item_id": r.item.item.id,
                "name": r.item.item.name,
                "quantity": r.item.quantity.max(1),
                "equip_slot": r.item.equip_slot,
            })).collect::<Vec<_>>(),
        })
    });
    let spells = play.merc.spells.as_ref().map(|s| {
        json!({
            "open": play.mode == crate::play::mercplay::SPELLS_MODE,
            "slot": s.slot,
            "name": s.name,
            "class": s.class,
            "level": s.level,
            "set": s.set(),
            "slots": s.slots,
            "spells": s.spells.iter().map(|sp| json!({
                "id": sp.ability.id,
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
    let o = play.dyepick.open.as_ref().filter(|_| play.dyepick.picker.open && play.mode == crate::play::dyeplay::MODE)?;
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
    if state == crate::fishing::state::OFF && play.mode != 36 {
        return None;
    }
    let w = &play.fish.wnd;
    Some(json!({
        "revision": 0,
        "open": play.mode == 36,
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
        "list": w.list_visible,
        "catch": play.fish.part.catch.items.iter().enumerate().map(|(i, it)| json!({
            "slot": i + 1,
            "name": it.item.name,
            "quantity": it.quantity,
        })).collect::<Vec<_>>(),
        "message": wide(&w.text),
    }))
}

/// Each step: what the three windows hold, for `eqoa.merc.get`, `eqoa.dye.get` and `eqoa.fishing.get`.
pub(crate) fn tick(play: &Play, game_step: bool) {
    if !game_step {
        return;
    }
    for (which, state) in [(Read::Merc, merc_state(play)), (Read::Dye, dye_state(play)), (Read::Fishing, fishing_state(play))] {
        match state {
            Some(s) => native_a11::publish(which, s),
            None => native_a11::clear(which),
        }
    }
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    LOCAL.with(|l| *l.borrow_mut() = Local::default());
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
    if !play.session.merc_windows {
        return refuse("unsupported", "this server does not offer the mercenary windows");
    }
    match action {
        A11Action::MercDismiss => {
            // the game's Start menu entry sends at once; the player is asked here first (a hire is spent)
            if !play.session.mercs.as_ref().is_some_and(|m| m.own) {
                return refuse("not_ready", "the server says no mercenary of yours is out");
            }
            super::a1_play::ask(Act::DismissMerc, "Send your mercenary away? It leaves your service.".into(), "")
        }
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
    let Some(o) = play.dyepick.open.clone().filter(|_| play.dyepick.picker.open && play.mode == crate::play::dyeplay::MODE) else {
        return Err(refuse("not_ready", "no dye picker is up"));
    };
    if o.token != token {
        return Err(refuse("stale_handle", "that is not the token of the picker now up (eqoa.dye.get().token)"));
    }
    Ok(o)
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
            super::a1_play::ask(Act::DyeAnswer { token: *token, rgba: Some(rgba) }, text, "")
        }
        A11Action::DyeNoDye { token } => {
            let o = match picker_open(play, *token) {
                Ok(o) => o,
                Err(e) => return e,
            };
            if !(play.session.dye_picker_version >= 2 && o.target != net::dyepick::Target::Aura) {
                return refuse("unsupported", "this picker offers no 'No dye' (the game's has none either)");
            }
            super::a1_play::ask(Act::DyeAnswer { token: *token, rgba: None }, "Take the dye off? The dyer may charge to put it on again.".into(), "")
        }
        A11Action::DyeCancel { token } => {
            if let Err(e) = picker_open(play, *token) {
                return e;
            }
            play.dyepick_done(PickerUp::Cancel);
            sent()
        }
        _ => refuse("unsupported", "not a dye request"),
    }
}

fn dispatch_fishing(play: &mut Play, action: &A11Action) -> DispatchOutcome {
    use crate::fishing::{state, sub_msg};
    if play.mode != 36 {
        return refuse("not_ready", "no fishing window is up");
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
            play.fishing_key(134, true);
            play.fishing_key(134, false);
            pressed()
        }
        A11Action::FishRelease => {
            if !play.fish.wnd.casting || play.fish.wnd.ready || st != state::WAITING {
                return refuse("not_ready", "no cast is under way");
            }
            play.fishing_key(134, true);
            play.fishing_key(134, false);
            pressed()
        }
        A11Action::FishReel | A11Action::FishPull | A11Action::FishKeep { .. } => {
            if st != state::CAUGHT || !play.fish.wnd.list_visible {
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
            if guard_waits {
                return refuse("busy", "the server has not answered the last fishing request");
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
            play.dyepick_done(match rgba {
                Some(c) => PickerUp::Pick(*c),
                None => PickerUp::NoDye,
            });
            Ok(())
        }
        _ => Err("not an A11 step".into()),
    }
}

#[cfg(test)]
#[path = "a11_play_tests.rs"]
mod tests;
