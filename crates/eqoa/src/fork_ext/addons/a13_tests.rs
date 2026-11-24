//! Round A13 without a running game: the `map` takeover kind and its rules, the session a mod reads, `pass`, and the
//! terrain layer's arguments and quotas. (The map key itself and the game's map coming back are in
//! `fork_ext/a13_play_tests.rs`; the picture and its pixels are in `fork_ext/map_terrain_tests.rs`.)
//!
//! Each test names its negative control: what it would see if the change were not there.
use super::{
    action_broker,
    host::Host,
    map_terrain,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws,
    window_takeover::{self, TakeoverKind},
};
use serde_json::json;

fn fresh() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    CORE.with(|c| c.borrow_mut().view.in_world = true);
}

fn host_with(id: &str, caps: &[&str], replaces: &[&str]) -> Host {
    let host = Host::new().unwrap();
    {
        let sess = super::native::session(&host.lua);
        let mut s = sess.borrow_mut();
        s.addon = id.to_string();
        s.native_manifest = true;
        s.capabilities = Some(caps.iter().map(|c| (*c).to_string()).collect());
        s.replaces = replaces.iter().map(|r| (*r).to_string()).collect();
    }
    host
}

const TAKEOVER: &[&str] = &["eqoa.windows.takeover", "eqoa.windows.session", "eqoa.events.on"];

#[test]
fn a13_the_map_is_held_whole_and_declared() {
    fresh();
    let h = host_with("holder", TAKEOVER, &["map"]);
    h.execute(
        r#"
        local ok, e = eqoa.windows.takeover('map', {scope = 'own'})
        assert(ok == nil and e.code == 'invalid_argument', 'own: ' .. tostring(e and e.code))
        local ok2, e2 = eqoa.windows.takeover('map')
        assert(ok2 == nil and e2.code == 'invalid_argument', 'the default scope is own')
        assert(eqoa.windows.takeover('map', {scope = 'all'}) == true)
        assert(eqoa.windows.taken('map') == true)
        local all = eqoa.windows.taken()
        assert(all.map == true and all.holders.map ~= nil)
        assert(eqoa.windows.release('map') == true)
        assert(eqoa.windows.taken('map') == false)
    "#,
    )
    .unwrap();
    // `all` needs the manifest to say it replaces the window
    let bare = host_with("bare", TAKEOVER, &[]);
    bare.execute(r#"local ok, e = eqoa.windows.takeover('map', {scope = 'all'}); assert(ok == nil and e.code == 'not_declared', e and e.code)"#).unwrap();
    // one holder: a second mod is told who has it
    let first = host_with("first", TAKEOVER, &["map"]);
    first.execute("assert(eqoa.windows.takeover('map', {scope = 'all'}) == true)").unwrap();
    let rival = host_with("rival", TAKEOVER, &["map"]);
    rival.execute(r#"local ok, e = eqoa.windows.takeover('map', {scope = 'all'}); assert(ok == nil and e.code == 'busy', e and e.code)"#).unwrap();
    // the player's switch refuses and releases it
    window_takeover::set_use_game_windows(true);
    assert!(!window_takeover::is_taken(TakeoverKind::Map), "the switch releases the claim");
    let late = host_with("late", TAKEOVER, &["map"]);
    late.execute(r#"local ok, e = eqoa.windows.takeover('map', {scope = 'all'}); assert(ok == nil, 'refused while the switch is on')"#).unwrap();
    window_takeover::forget_switch_for_test();
}

#[test]
fn a13_a_mod_cannot_open_the_map_itself_and_the_session_is_the_players_key() {
    fresh();
    let h = host_with("opener", TAKEOVER, &["map"]);
    h.execute(
        r#"
        local s, e = eqoa.windows.open_session('map')
        assert(s == nil and e.code == 'invalid_argument', tostring(e and e.code))
    "#,
    )
    .unwrap();
    let vm = native_events::bus(&h.lua).borrow().vm;
    window_takeover::takeover(vm, "opener", TakeoverKind::Map, window_takeover::TakeoverScope::All).unwrap();
    let s = ws::open_client(vm, "opener", "map", "key", json!({}));
    h.execute("s = eqoa.windows.session('map'); assert(s.kind == 'map' and s.title == 'Map' and s.origin == 'player' and s.opened_from == 'key')").unwrap();
    // the mod's own close ends it (nothing of the game's to close: the game's map never opened)
    h.execute("assert(eqoa.windows.close_session(s.ref) == true); assert(eqoa.windows.session('map') == nil)").unwrap();
    assert!(ws::find(vm, &s.id).is_none());
}

#[test]
fn a13_pass_queues_the_games_map_and_is_busy_while_a_window_is_up() {
    fresh();
    let h = host_with("passer", TAKEOVER, &["map"]);
    let vm = native_events::bus(&h.lua).borrow().vm;
    window_takeover::takeover(vm, "passer", TakeoverKind::Map, window_takeover::TakeoverScope::All).unwrap();
    ws::open_client(vm, "passer", "map", "key", json!({}));
    h.execute("s = eqoa.windows.session('map'); ok, err = eqoa.windows.pass(s.ref); assert(ok == true, err and err.message)").unwrap();
    assert_eq!(ws::take_passes(), vec![(vm, TakeoverKind::Map)]);
    ws::cancel_pass(vm, TakeoverKind::Map);
    // a game window is up: busy, nothing queued (negative control for the queue above)
    ws::set_env(ws::Env { plain: false, ..Default::default() });
    h.execute("ok, err = eqoa.windows.pass(s.ref); assert(ok == nil and err.code == 'busy', tostring(err and err.code))").unwrap();
    assert!(ws::take_passes().is_empty());
    // a window the mod opened itself still has nothing to pass (the older rule is unchanged)
    ws::set_env(ws::Env::default());
    let own = ws::open(vm, "passer", "inventory", "key").unwrap();
    h.execute(&format!("ok, err = eqoa.windows.pass('{}'); assert(ok == nil and err.code == 'unsupported', tostring(err and err.code))", own.id)).unwrap();
}

#[test]
fn a13_the_terrain_checks_its_arguments_and_keeps_its_quota() {
    fresh();
    map_terrain::release_vm(0);
    let h = host_with("terra", &[], &[]);
    h.execute(
        r#"
        local t, e = eqoa.map.terrain({size = 8})
        assert(t == nil and e.code == 'out_of_range', tostring(e and e.code))
        t, e = eqoa.map.terrain({size = 5000})
        assert(t == nil and e.code == 'out_of_range', tostring(e and e.code))
        t, e = eqoa.map.terrain({size = 'big'})
        assert(t == nil and e.code == 'invalid_argument', tostring(e and e.code))
        t, e = eqoa.map.terrain({layers = 'all'})
        assert(t == nil and e.code == 'invalid_argument', tostring(e and e.code))
        local a = assert(eqoa.map.terrain())
        local b = assert(eqoa.map.terrain({size = 512}))
        local c, ec = eqoa.map.terrain()
        assert(c == nil and ec.code == 'quota', tostring(ec and ec.code))
        -- no game map drawn yet: the transform is not known and the picture is nil
        assert(a.texture == nil and a.bounds == nil and a.world_id == nil)
        local px, err = a.project(1, 1)
        assert(px == nil and err.code == 'not_ready', tostring(err and err.code))
        px, err = a.project(0 / 0, 1)
        assert(px == nil and err.code == 'invalid_argument')
        assert(a.close() == true)
        local d = assert(eqoa.map.terrain(), 'a closed picture frees its place')
        local x, ex = a.unproject(1, 1)
        assert(x == nil and ex.code == 'stale_handle', tostring(ex and ex.code))
    "#,
    )
    .unwrap();
    assert_eq!(map_terrain::held_count(), 2, "b and d: a was closed");
    // disabling the mod (its VM going) releases every picture it held
    let vm = native_events::bus(&h.lua).borrow().vm;
    map_terrain::release_vm(vm);
    assert_eq!(map_terrain::held_count(), 0);
    // control: out of the world it is not offered at all
    CORE.with(|c| c.borrow_mut().view.in_world = false);
    h.execute("local t, e = eqoa.map.terrain(); assert(t == nil and e.code == 'not_in_world', tostring(e and e.code))").unwrap();
}

#[test]
fn a13_the_surface_names_are_supported() {
    fresh();
    let h = host_with("surface", &[], &[]);
    h.execute(
        r#"
        assert(eqoa.capabilities.has('eqoa.map.terrain') == true)
        assert(eqoa.capabilities.has('eqoa.map.set_waypoint') == true)
    "#,
    )
    .unwrap();
}
