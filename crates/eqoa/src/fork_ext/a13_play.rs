//! Round A13 on `Play`: the world map as a window a mod holds (`TakeoverKind::Map`, scope `all`, a client-side
//! view session). A child of the Play adapter, so it reads `Play`'s private state; it adds no line to any upstream file.
//!
//! The map key (or the compass, the minimap, `eqoa.map.open()`) calls `Map::show`. Each step (`tick`, from
//! `begin_iteration`, before any draw) a map that was just opened while a mod holds the kind is closed again at
//! once, as if the key had not been pressed, and the holder is given a `map` session (`opened_from = "key"`). The
//! key again closes the mod's map, as it closes the game's. `eqoa.windows.pass(ref)` gives the game's own map back:
//! it opens for that one opening and the mod's session ends as `passed`. A map that was already open when the mod
//! claimed the kind stays the game's until it closes. With nobody holding the kind nothing here does anything.
use super::Play;
use crate::fork_ext::addons::{
    window_sessions,
    window_takeover::{self, TakeoverKind},
};
use serde_json::json;
use std::cell::Cell;

thread_local! {
    /// The game's map was open at the last step.
    static WAS_OPEN: Cell<bool> = const { Cell::new(false) };
    /// The game's own map is wanted for the next open (a `pass`).
    static PASS_ONCE: Cell<bool> = const { Cell::new(false) };
}

fn holder() -> Option<(u64, String)> {
    Some((window_takeover::holder_vm(TakeoverKind::Map)?, window_takeover::holder_of(TakeoverKind::Map)?))
}

/// The mod's map ends when nobody holds the kind any more (released, disabled, the player's switch): the game's map is
/// the player's again.
fn drop_if_not_held() {
    if let Some(s) = window_sessions::of_kind("map")
        && window_takeover::holder_vm(TakeoverKind::Map) != Some(s.vm)
    {
        window_sessions::end_kind("map", "released");
    }
}

fn may_divert() -> bool {
    let env = window_sessions::env();
    !window_takeover::use_game_windows() && !env.away && !env.dead
}

/// What decides each step whether an opening is the mod's: pure, so it can be tested without a `Play`.
pub(crate) fn diverts(held: bool, fresh: bool, may: bool, pass_once: bool) -> bool {
    held && fresh && may && !pass_once
}

/// Each step's work for the map (`begin_iteration`).
pub(crate) fn tick(play: &mut Play) {
    let open_now = play.map.open;
    let fresh = open_now && !WAS_OPEN.with(|c| c.replace(open_now));
    if !open_now {
        WAS_OPEN.with(|c| c.set(false));
        PASS_ONCE.with(|p| p.set(false));
    }
    if !window_takeover::has_activity() && !window_sessions::is_active() {
        return;
    }
    drop_if_not_held();
    let Some((vm, addon)) = holder() else {
        PASS_ONCE.with(|p| p.set(false));
        return;
    };
    if !fresh {
        return;
    }
    let pass_once = PASS_ONCE.with(|p| p.replace(false));
    if !diverts(true, fresh, may_divert(), pass_once) {
        return;
    }
    // undo the open: the game's map never drew
    play.map.close();
    WAS_OPEN.with(|c| c.set(false));
    match window_sessions::get(vm, "map") {
        // the key again: the game's map closes on it, so does the mod's
        Some(s) => {
            let _ = window_sessions::close(vm, &s.id, "key");
        }
        None => {
            window_sessions::open_client(vm, &addon, "map", "key", json!({}));
            refresh_plain(play);
        }
    }
}

/// `a1_play::tick` judged this step with the game's map up (play was not plain), and the map was undone just above: judged
/// again, so a controller's focus and a `pass` the mod asks for as its session opens are not refused (as the Start menu's).
fn refresh_plain(play: &mut Play) {
    let mut env = window_sessions::env();
    env.plain = !env.away && play.addon_input_available() && !play.map.open;
    env.passable = env.plain && play.mode == 0;
    window_sessions::set_env(env);
}

/// A `pass` the mod asked for (`a1_play::tick`): the game's own map opens for this one opening and the mod's session
/// ends. False where the map cannot open now (a window of the game is up).
pub(crate) fn pass(play: &mut Play, vm: u64) -> bool {
    if play.mode != 0 || !crate::settings::map() {
        return false;
    }
    let Some(s) = window_sessions::get(vm, "map") else { return false };
    PASS_ONCE.with(|p| p.set(true));
    play.map.show();
    window_sessions::end_dialog_session(&s.id, "passed");
    true
}

#[cfg(test)]
#[path = "a13_play_tests.rs"]
mod tests;
