//! Round A13 from Play-shaped state: the map key opens the mod's map when a mod holds the kind, the game's own map when
//! none does, the key again closes the mod's, `pass` gives the game's map back for one opening, and a release or a
//! disable ends the mod's map.
//!
//! Each test says what its negative control is: the behaviour without the change would fail the assertion named.
use super::*;
use crate::fork_ext::addons::{
    action_broker,
    host::Host,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws,
    window_takeover::{self, TakeoverKind, TakeoverScope},
};

fn ready() -> Play {
    crate::settings::testing::reset(false);
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    PASS_ONCE.with(|p| p.set(false));
    WAS_OPEN.with(|p| p.set(false));
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p.session.conn = Some(usize::MAX);
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    p
}

fn host() -> Host {
    Host::with_test_capabilities(&["eqoa.windows.session", "eqoa.windows.takeover", "eqoa.events.on"]).unwrap()
}

fn hold(host: &Host) -> u64 {
    let vm = native_events::bus(&host.lua).borrow().vm;
    window_takeover::takeover(vm, "a13_mod", TakeoverKind::Map, TakeoverScope::All).unwrap();
    vm
}

fn step(p: &mut Play, host: &Host) {
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

#[test]
fn a13_with_no_mod_the_map_key_opens_the_games_map() {
    let mut p = ready();
    let h = host();
    p.map.show();
    step(&mut p, &h);
    assert!(p.map.open, "nobody holds the map: the game's own map stays up (the behaviour this round must not change)");
    assert!(ws::get(0, "map").is_none() && ws::of_kind("map").is_none());
    // the control for the tests below: the same key with a holder is diverted
    let mut q = ready();
    let h = host();
    hold(&h);
    q.map.show();
    step(&mut q, &h);
    assert!(!q.map.open, "a held map: the game's never stays up");
}

#[test]
fn a13_the_map_key_opens_the_mods_map_and_the_key_again_closes_it() {
    let mut p = ready();
    let h = host();
    let vm = hold(&h);
    p.map.show();
    assert!(p.map.open, "the key opens the game's map first: the diversion is the next step, before any draw");
    step(&mut p, &h);
    assert!(!p.map.open, "undone: the game's map never draws");
    let s = ws::get(vm, "map").expect("the mod has a session");
    assert_eq!((s.origin, s.opened_from.as_str(), s.kind.as_str()), ("player", "key", "map"));
    h.execute("s = eqoa.windows.session('map'); assert(s.kind == 'map' and s.title == 'Map' and s.opened_from == 'key')").unwrap();
    // the key again: the game's map closes on it, so does the mod's
    p.map.show();
    step(&mut p, &h);
    assert!(!p.map.open);
    assert!(ws::get(vm, "map").is_none(), "the second key press is the toggle: the mod's map closed");
    // and once more opens it again
    p.map.show();
    step(&mut p, &h);
    assert!(ws::get(vm, "map").is_some());
}

#[test]
fn a13_pass_gives_the_games_map_for_one_opening() {
    let mut p = ready();
    let h = host();
    let vm = hold(&h);
    p.map.show();
    step(&mut p, &h);
    h.execute("s = eqoa.windows.session('map'); ok, err = eqoa.windows.pass(s.ref); assert(ok == true, err and err.message)").unwrap();
    step(&mut p, &h);
    assert!(p.map.open, "the game's own map is up");
    assert!(ws::get(vm, "map").is_none(), "the mod's session ended as passed");
    step(&mut p, &h);
    assert!(p.map.open, "the game's map is not diverted again on the next step");
    // the game's map closes; the next key press is the mod's again (control: the pass was for one opening only)
    p.map.close();
    step(&mut p, &h);
    p.map.show();
    step(&mut p, &h);
    assert!(!p.map.open && ws::get(vm, "map").is_some());
}

#[test]
fn a13_pass_is_refused_while_a_game_window_is_up_and_a_map_open_before_the_claim_stays_the_games() {
    let mut p = ready();
    let h = host();
    // a map already open when the claim was made stays the game's until it closes
    p.map.show();
    step(&mut p, &h);
    assert!(p.map.open);
    let vm = hold(&h);
    step(&mut p, &h);
    assert!(p.map.open, "claimed while open: the game's map stays");
    assert!(ws::get(vm, "map").is_none());
    p.map.close();
    step(&mut p, &h);
    p.map.show();
    step(&mut p, &h);
    assert!(ws::get(vm, "map").is_some());
    // another window of the game is up: the pass is not_ready (a game window is in the way), nothing is queued
    p.mode = 22;
    step(&mut p, &h);
    h.execute("s = eqoa.windows.session('map'); ok, err = eqoa.windows.pass(s.ref); assert(ok == nil and err.code == 'not_ready', tostring(err and err.code))").unwrap();
    assert!(ws::take_passes().is_empty());
}

#[test]
fn a13_a_release_the_players_switch_or_a_disable_ends_the_mods_map() {
    let mut p = ready();
    let h = host();
    let vm = hold(&h);
    p.map.show();
    step(&mut p, &h);
    assert!(ws::get(vm, "map").is_some());
    h.execute("assert(eqoa.windows.release('map') == true)").unwrap();
    step(&mut p, &h);
    assert!(ws::get(vm, "map").is_none(), "released: the mod's map ends");
    p.map.show();
    step(&mut p, &h);
    assert!(p.map.open, "and the game's map is the player's again");
    // the player's switch ("Use the game's windows") releases an all-scope claim
    p.map.close();
    step(&mut p, &h);
    hold(&h);
    window_takeover::set_use_game_windows(true);
    p.map.show();
    step(&mut p, &h);
    assert!(p.map.open, "the switch is on: the game's map");
    window_takeover::forget_switch_for_test();
}

#[test]
fn a13_diverts_only_when_held_fresh_allowed_and_not_passed() {
    assert!(diverts(true, true, true, false));
    assert!(!diverts(false, true, true, false), "no holder");
    assert!(!diverts(true, false, true, false), "not a new opening");
    assert!(!diverts(true, true, false, false), "the switch, death or away");
    assert!(!diverts(true, true, true, true), "a pass");
}

#[test]
fn a13_a_mods_map_open_reaches_the_holder_in_the_same_step_and_never_toggles_it() {
    let mut p = ready();
    let h = host();
    // control: nobody holds the kind, so the game's map is the one to open
    assert!(!open_for_mod(&mut p), "no holder: the caller opens the game's map");
    let vm = hold(&h);
    // the holder's map opens at once: the game's map is never up, not even before the next step
    assert!(open_for_mod(&mut p));
    assert!(!p.map.open, "the game's map never opened");
    let s = ws::get(vm, "map").expect("the holder has its session now");
    assert_eq!((s.origin, s.opened_from.as_str()), ("player", "mod"), "told as the mod's own request, not as the player's key");
    // asked again (a mod that wants it open): refreshed, not closed
    assert!(open_for_mod(&mut p));
    assert!(ws::get(vm, "map").is_some(), "the second request does not close the holder's map");
    step(&mut p, &h);
    assert!(!p.map.open && ws::get(vm, "map").is_some());
    // a map the game already has up stays the game's (as for the key)
    ws::reset_for_test();
    p.map.show();
    assert!(!open_for_mod(&mut p));
}

#[test]
fn a13_disabling_the_holder_ends_its_map_through_the_real_disable_path() {
    let mut p = ready();
    let h = host();
    let vm = hold(&h);
    p.map.show();
    step(&mut p, &h);
    assert!(ws::get(vm, "map").is_some());
    native_events::clear(&h);
    step(&mut p, &h);
    assert!(ws::get(vm, "map").is_none() && !window_takeover::is_taken(TakeoverKind::Map), "the disable released the claim and the session");
    p.map.show();
    step(&mut p, &h);
    assert!(p.map.open, "and the game's map is the player's again");
}
