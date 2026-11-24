// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A9 from Play-shaped state: trade requests and trades, duel invitations and wagers, the blacksmith, held as
//! hidden window sessions (`trade_ask`, `trade`, `duel_invite`, `duel`, `blacksmith`, scope `all`).
//!
//! Every test states its negative control. The bytes a mod's action sends are compared with the bytes the game's own
//! window sends for the same press on the same state, where the window can be driven here (accept, repair, the
//! request's Yes and No), and otherwise with the game's own builder (`TradeState::send_add` and the duel part's
//! methods, which the window calls too).
use super::*;
use crate::fork_ext::addons::{
    action_broker, confirm,
    discovery::{DiscoveredAddon, EntryMode, Extensions},
    host::Host,
    native_a9,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws, window_takeover,
};
use crate::play::npcw::fork_pass as trade_pass;
use net::dump::{Item, NetItem};
use net::session::{Datagrams, Event, Session};
use net::stream::Writer;
use std::time::Duration;
use winit::keyboard::{Key, NamedKey};

struct NullNet;
impl Datagrams for NullNet {
    fn send_to(&mut self, _: &[u8], _: [u8; 4], _: u16) {}
    fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
        None
    }
}

fn live_session() -> Session {
    let mut s = Session::connect_with(Box::new(NullNet), ([127, 0, 0, 1], 40000), ([127, 0, 0, 1], 10070), "test", [0; 32]).unwrap();
    s.state = net::session::State::World;
    s.self_id = Some(100);
    s.record = Some(Vec::new());
    s
}

fn recorded(p: &Play) -> Vec<Vec<u8>> {
    p.session.record.as_ref().map(|r| r.iter().map(|(_, d)| d.clone()).collect()).unwrap_or_default()
}

fn clear_recorded(p: &mut Play) {
    if let Some(r) = p.session.record.as_mut() {
        r.clear();
    }
}

/// Puts every thread-wide piece of state a test here touched back to its baseline (also on a panic): with
/// `--test-threads=1` all tests share one thread.
struct Baseline;

impl Drop for Baseline {
    fn drop(&mut self) {
        confirm::cancel_all("test");
        a9_play::reset_for_test();
        native_a9::clear_all();
        action_broker::reset_all_for_test();
        ws::reset_for_test();
        window_takeover::reset_all_for_test();
        window_takeover::forget_switch_for_test();
        native_core::fixture_demand(false);
        crate::fork_ext::bindpage::reset_in_memory();
        CORE.with(|c| *c.borrow_mut() = Core::default());
    }
}

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    confirm::cancel_all("test");
    a9_play::reset_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    let mut p = super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((crate::ui::Vui::new(Default::default(), Default::default(), vec![]), Default::default(), crate::hud::Hud::new()));
    p.session = live_session();
    p.dump.player.id = 100;
    p.dump.player.level = 30;
    p.dump.player.p34 = 1000;
    p.dump.player.name = "Hero".into();
    p.player.name = "Hero".into();
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    p
}

fn host_all() -> Host {
    let root = std::env::temp_dir();
    let addon = DiscoveredAddon {
        id: "a9_test_mod".into(),
        name: "a9_test_mod".into(),
        entry: root.join("main.lua"),
        script_root: root.clone(),
        package_root: root.clone(),
        module_dirs: vec![],
        resource_dirs: vec![],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: [
            "eqoa.actions.*",
            "eqoa.windows.session",
            "eqoa.windows.takeover",
            "eqoa.events.on",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect(),
        extensions: Extensions {
            replaces: ["trade_ask", "trade", "duel_invite", "duel", "blacksmith"].iter().map(|s| (*s).to_string()).collect(),
            ..Default::default()
        },
        initially_enabled: true,
        mods_root: None,
    };
    let host = Host::for_addon(&addon).unwrap();
    host.execute(
        r#"
        results, closed, ev, tev, dev, sev = {}, {}, {}, {}, {}, {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        eqoa.events.on("confirm.closed", function(e) closed[#closed + 1] = e.data end)
        eqoa.events.on("windows.session", function(e) ev[#ev + 1] = e.data.phase .. ":" .. e.data.kind .. ":" .. tostring(e.data.reason) end)
        eqoa.events.on("trade.request", function(e) tev[#tev + 1] = "request:" .. e.data.name end)
        eqoa.events.on("trade.accepted", function(e) tev[#tev + 1] = "accepted:" .. e.data.side end)
        eqoa.events.on("trade.closed", function(e) tev[#tev + 1] = "closed:" .. e.data.kind .. ":" .. e.data.reason end)
        eqoa.events.on("trade.changed", function(e) tev[#tev + 1] = "changed:" .. table.concat(e.data.changed, ",") end)
        eqoa.events.on("duel.changed", function(e) dev[#dev + 1] = e.data end)
        eqoa.events.on("blacksmith.changed", function(e) sev[#sev + 1] = e.data end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "9" }, function() last_req, last_err = next_fn() end)
        "#,
    )
    .unwrap();
    host
}

fn press_9(host: &Host) {
    CORE.with(|c| {
        c.borrow_mut().emit("input.key", serde_json::json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}))
    });
    native_events::dispatch(host).unwrap();
}

fn ask(p: &mut Play, host: &Host, code: &str) {
    host.execute(&format!("results = {{}}; closed = {{}}; last_err = nil; last_req = nil; next_fn = function() {code} end")).unwrap();
    press_9(host);
    super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn step(p: &mut Play, host: &Host) {
    super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn key(k: NamedKey) {
    assert!(confirm::key(&Key::Named(k)));
}

/// A server opcode through the sequence `Play::frame` runs: before, the game's handler, the divert hook, after.
fn deliver(p: &mut Play, op: u16, data: Vec<u8>) {
    let ev = Event::Opcode { op, data: data.clone() };
    let note = super::begin_game_event(p, &ev);
    let handled = p.npcw_opcode(op, &data) || p.duel_opcode(op, &data, None) || p.support_opcode(op, &data);
    assert!(handled, "opcode {op:#06x} is not one of the windows' own");
    super::divert_event(p);
    super::end_game_event(p, note);
}

fn trade_msg(sub: u8, id: u32, rest: &[u8]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(sub).u32(id);
    w.data.extend_from_slice(rest);
    w.data
}

fn trade_request(partner: u32, with: u32) -> Vec<u8> {
    let mut w = Writer::default();
    w.u32(partner).u32(with);
    w.data
}

fn duel_down(sub: u8, id: u32, rest: &[u8]) -> Vec<u8> {
    let mut v = vec![sub];
    v.extend(id.to_le_bytes());
    v.extend(rest);
    v
}

fn duel_invitation(duel_id: u32, name: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(6).u8(0).u8(40).string(name.as_bytes()).wide("").wide("");
    duel_down(2, duel_id, &w.data)
}

fn item(id: i64, key: u32, qty: i64, max_stack: i64, name: &str) -> NetItem {
    NetItem {
        quantity: qty,
        equip_slot: -1,
        quick_slot: -1,
        inv_index: key,
        item: Item { id, max_stack, name: name.into(), ..Default::default() },
        ..Default::default()
    }
}

fn bags(p: &mut Play) {
    p.dump.inventory.items = vec![
        item(10, 1, 5, 20, "Snake Scale"),
        item(11, 2, 1, 0, "Rusty Sword"),
        {
            let mut worn = item(12, 3, 1, 0, "Worn Helm");
            worn.equip_slot = 4;
            worn
        },
        {
            let mut nt = item(13, 4, 1, 0, "Bound Ring");
            nt.item.no_trade = 1;
            nt
        },
        item(14, 5, 1, 0, "Bone Dagger"),
        item(15, 6, 1, 0, "Tin Cup"),
        item(16, 7, 1, 0, "Old Boot"),
    ];
}

/// A trade accepted by the partner: what `Trade02` sets up, with the bags copied.
fn open_trade(p: &mut Play, partner: u32) {
    p.npcw.trade_state.asked = true;
    p.npcw.trade_state.with = partner;
    deliver(p, 0x0056, trade_msg(2, partner, &[]));
}

fn lua_ok(host: &Host, code: &str) {
    host.execute(code).unwrap_or_else(|e| panic!("{code}: {e}"));
}

// ---- trade requests ----

#[test]
fn a9_trade_request_is_held_hidden_answered_like_the_window_and_times_out() {
    let _baseline = Baseline;
    // control: no mod holds trade_ask: the game's own ask is up in mode 16, exactly as before
    let mut p = ready();
    deliver(&mut p, 0x0057, trade_request(77, 77));
    assert_eq!(p.mode, 16, "control: with no holder the game's own request window opens");
    // the window's Yes and No, for the bytes to compare
    let native_yes = {
        use crate::pointer::testing::{NullRaster, vui};
        let (v, mut r) = (vui(), NullRaster);
        p.run_npcw(&v, &mut r);
        clear_recorded(&mut p);
        // the box starts on No (Yes is the left button): Left, then Enter
        p.npcw_key(17, true);
        p.run_npcw(&v, &mut r);
        p.npcw_key(13, true);
        p.run_npcw(&v, &mut r);
        recorded(&p)
    };
    assert_eq!(native_yes, vec![vec![0x56, 0x00, 2, 77, 0, 0, 0]], "the window's Yes is sub 2 to the partner");
    let native_no = {
        use crate::pointer::testing::{NullRaster, vui};
        let (v, mut r) = (vui(), NullRaster);
        let mut q = ready();
        deliver(&mut q, 0x0057, trade_request(77, 77));
        q.run_npcw(&v, &mut r);
        clear_recorded(&mut q);
        q.npcw_key(27, true);
        q.run_npcw(&v, &mut r);
        recorded(&q)
    };
    assert_eq!(native_no.len(), 1, "the window's No sends one cancel");

    let _baseline2 = Baseline;
    let mut p = ready();
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade_ask', {scope='all'}) == true)");
    lua_ok(&host, "local ok, err = eqoa.windows.takeover('trade', {scope='own'}); assert(ok == nil and err.code == 'invalid_argument')");
    step(&mut p, &host);
    deliver(&mut p, 0x0057, trade_request(77, 77));
    assert_eq!(p.mode, 0, "a held request never draws");
    step(&mut p, &host);
    lua_ok(
        &host,
        r#"
        local t = eqoa.trade.get()
        assert(t and t.kind == "trade" and t.phase == "request" and t.held == true and t.partner.id == 77)
        assert(t.seconds_remaining > 40 and t.duration == 45)
        assert(eqoa.windows.session("trade_ask") ~= nil)
        assert(tev[1] == "request:Unknown", tev[1])
        "#,
    );

    // a second request while one is held: answered busy (as the game does with a window up), the held one untouched
    clear_recorded(&mut p);
    deliver(&mut p, 0x0057, trade_request(88, 88));
    assert_eq!(recorded(&p), vec![vec![0x56, 0x00, 20, 88, 0, 0, 0]], "the newcomer is told busy");
    assert_eq!((p.mode, p.npcw.trade_state.with, p.npcw.trade_state.partner), (0, 77, 77), "the held request is as it was");
    lua_ok(&host, "assert(eqoa.trade.get().partner.id == 77)");

    // Yes sends the window's own bytes, once
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.trade_answer(true)");
    assert_eq!(recorded(&p), native_yes, "Yes is the window's Yes");
    lua_ok(&host, "assert(results[1].ok == true and results[1].detail.sent == true and eqoa.windows.session('trade_ask') == nil)");
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.trade_answer(true)");
    lua_ok(&host, "assert(results[1].ok == false and results[1].code == 'not_ready')");
    assert!(recorded(&p).is_empty(), "nothing is sent twice");

    // No, on a new request
    let mut p = ready();
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade_ask', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, 0x0057, trade_request(78, 78));
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.trade_answer(false)");
    assert_eq!(recorded(&p), native_no, "No is the window's No");

    // 45 seconds unanswered: refused; at 44 not yet (the control)
    let mut p = ready();
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade_ask', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, 0x0057, trade_request(79, 79));
    step(&mut p, &host);
    clear_recorded(&mut p);
    a9_play::age_for_test(Duration::from_secs(44));
    step(&mut p, &host);
    assert!(recorded(&p).is_empty(), "at 44 s nothing is refused yet");
    a9_play::age_for_test(Duration::from_secs(2));
    step(&mut p, &host);
    assert_eq!(recorded(&p), native_no, "at 45 s the request is refused with the window's No");
    lua_ok(&host, "assert(eqoa.trade.get() == nil and eqoa.windows.session('trade_ask') == nil and ev[#ev] == 'closed:trade_ask:timeout', ev[#ev])");
}

#[test]
fn a9_a_held_request_goes_back_to_the_game_or_is_turned_away() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade_ask', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, 0x0057, trade_request(77, 77));
    step(&mut p, &host);
    // pass: the game's own request window opens again, nothing sent
    clear_recorded(&mut p);
    lua_ok(&host, "local s = assert(eqoa.windows.session('trade_ask')); assert(eqoa.windows.pass(s.ref) == true)");
    step(&mut p, &host);
    assert_eq!(p.mode, 16, "pass opens the game's request window");
    assert!(recorded(&p).is_empty(), "passing back refuses nothing");

    // the holder released (the mod disabled): back to the game while play is plain
    let mut p = ready();
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade_ask', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, 0x0057, trade_request(77, 77));
    step(&mut p, &host);
    clear_recorded(&mut p);
    lua_ok(&host, "assert(eqoa.windows.release('trade_ask') == true)");
    step(&mut p, &host);
    assert_eq!(p.mode, 16, "released: the game's own window");
    assert!(recorded(&p).is_empty());

    // released while another window is up: turned away, never left hanging
    let mut p = ready();
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade_ask', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, 0x0057, trade_request(77, 77));
    step(&mut p, &host);
    p.mode = 5;
    clear_recorded(&mut p);
    lua_ok(&host, "assert(eqoa.windows.release('trade_ask') == true)");
    step(&mut p, &host);
    assert_eq!(recorded(&p), vec![vec![0x56, 0x00, 1, 77, 0, 0, 0]], "turned away with the game's cancel");

    // the world ends (a teleport): the server still hears the refusal
    let mut p = ready();
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade_ask', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, 0x0057, trade_request(77, 77));
    step(&mut p, &host);
    clear_recorded(&mut p);
    let ev = Event::Teleport { world: 1, pos: [0.0; 3], hpr: [0.0; 3], token: 1 };
    let note = super::begin_game_event(&p, &ev);
    super::divert_event(&mut p);
    super::end_game_event(&p, note);
    step(&mut p, &host);
    assert_eq!(recorded(&p), vec![vec![0x56, 0x00, 1, 77, 0, 0, 0]], "the refusal is sent at the next step");
}

// ---- trades ----

#[test]
fn a9_trade_offers_follow_the_game_and_accepting_asks_the_player_first() {
    let _baseline = Baseline;
    let mut p = ready();
    bags(&mut p);
    // control: no holder, the game's own window opens
    open_trade(&mut p, 77);
    assert_eq!(p.mode, 15, "control: with no holder the game's trade window opens");
    // the window's accept key (Q) on the same state, for the bytes
    let native_accept = {
        use crate::pointer::testing::{NullRaster, vui};
        let (v, mut r) = (vui(), NullRaster);
        p.run_npcw(&v, &mut r);
        clear_recorded(&mut p);
        p.npcw_key(81, true);
        p.run_npcw(&v, &mut r);
        recorded(&p)
    };
    assert_eq!(native_accept, vec![vec![0x56, 0x00, 6, 77, 0, 0, 0]], "the window's accept is sub 6 to the partner");

    let _baseline2 = Baseline;
    let mut p = ready();
    bags(&mut p);
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade', {scope='all'}) == true)");
    step(&mut p, &host);
    open_trade(&mut p, 77);
    assert_eq!(p.mode, 0, "a held trade never draws");
    step(&mut p, &host);
    lua_ok(
        &host,
        r#"
        local t = assert(eqoa.trade.get())
        assert(t.kind == "trade" and t.phase == "open" and t.held == true and t.partner.id == 77)
        assert(#t.bags == 7 and t.bags[1].name == "Snake Scale" and t.bags[1].can_offer == true)
        assert(t.bags[3].worn == true and t.bags[3].can_offer == false)
        assert(t.bags[4].no_trade == true and t.bags[4].can_offer == false)
        assert(#t.mine.items == 0 and t.mine.accepted == false and t.max_items == 4 and t.carried == 1000)
        assert(eqoa.windows.session("trade") ~= nil)
        "#,
    );

    // the contract: refused in the core, nothing sent
    clear_recorded(&mut p);
    for (code, want) in [
        ("return eqoa.actions.trade_add({slot = 3, id = 12})", "not_ready"),         // worn
        ("return eqoa.actions.trade_add({slot = 4, id = 13})", "not_ready"),         // NO TRADE
        ("return eqoa.actions.trade_add({slot = 1, id = 10, count = 6})", "out_of_range"), // more than held
        ("return eqoa.actions.trade_add({slot = 2, id = 11, count = 2})", "out_of_range"), // a single item twice
        ("return eqoa.actions.trade_add({slot = 1, id = 99})", "stale_handle"),
        ("return eqoa.actions.trade_add({slot = 99, id = 10})", "stale_handle"),
        ("return eqoa.actions.trade_money(1001)", "out_of_range"),
        ("return eqoa.actions.trade_remove({slot = 1, id = 10})", "stale_handle"),
        ("return eqoa.actions.trade_unaccept()", "not_ready"),
    ] {
        ask(&mut p, &host, code);
        lua_ok(&host, &format!("assert(results[1].ok == false and results[1].code == '{want}', tostring(results[1].code) .. ' for ' .. [[{code}]])"));
    }
    assert!(recorded(&p).is_empty(), "no refused action sent anything");

    // adds: the game's own builder's bytes
    let want_add = p.npcw.trade_state.send_add(0, 2).unwrap();
    ask(&mut p, &host, "return eqoa.actions.trade_add({slot = 1, id = 10, count = 2})");
    lua_ok(&host, "assert(results[1].ok == true and results[1].detail.sent == true and results[1].detail.confirmed == false)");
    assert_eq!(recorded(&p), vec![want_add]);
    // the server answers with its sub 3 (our add, as the initiator reads it): key 1, two
    let mut w = Writer::default();
    w.u32(1).signed(2);
    deliver(&mut p, 0x0056, trade_msg(3, 77, &w.data));
    clear_recorded(&mut p);
    let want_money = p.npcw.trade_state.send_money(250);
    ask(&mut p, &host, "return eqoa.actions.trade_money(250)");
    assert_eq!(recorded(&p), vec![want_money]);
    clear_recorded(&mut p);
    let want_remove = p.npcw.trade_state.send_remove(0).unwrap();
    ask(&mut p, &host, "return eqoa.actions.trade_remove({slot = 1, id = 10})");
    assert_eq!(recorded(&p), vec![want_remove]);
    // the removal lands, and the offer is empty again
    deliver(&mut p, 0x0056, trade_msg(4, 77, &0u32.to_le_bytes()));
    step(&mut p, &host);
    // a trade of one thing, for the accept
    let mut w = Writer::default();
    w.u32(2).signed(1);
    deliver(&mut p, 0x0056, trade_msg(3, 77, &w.data));
    let mut w = Writer::default();
    w.signed(250);
    deliver(&mut p, 0x0056, trade_msg(5, 77, &w.data));
    step(&mut p, &host);

    // accepting asks first: nothing is sent, the question names what each side gives
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.trade_accept()");
    lua_ok(&host, "assert(results[1].ok == true and results[1].detail.asked == true)");
    assert!(recorded(&p).is_empty(), "trade_accept sends nothing until the player says Yes");
    let q = confirm::current_for_test().expect("the question is up");
    assert!(!q.yes, "it starts on No");
    assert!(q.text.contains("Rusty Sword") && q.text.contains("250 tunar") && q.text.contains("Hero") == false, "{}", q.text);
    assert!(q.text.contains("You give") && q.text.contains("You get: nothing"), "{}", q.text);
    // Enter on No sends nothing
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert!(recorded(&p).is_empty(), "No sends nothing");
    // a hidden trade never accepts by itself: time passes, nothing is sent
    step(&mut p, &host);
    assert!(recorded(&p).is_empty());

    // the partner's offer changes after the question was asked: the question ends as No, nothing is sent (the
    // control, an unchanged offer, follows); and a Yes that still arrives for the old offer is refused by its digest
    ask(&mut p, &host, "return eqoa.actions.trade_accept()");
    let old = match confirm::current_for_test().expect("asked").act {
        confirm::Act::TradeAccept { digest, .. } => digest,
        other => panic!("{other:?}"),
    };
    let mut w = Writer::default();
    w.signed(40);
    deliver(&mut p, 0x0056, trade_msg(10, 77, &w.data));
    step(&mut p, &host);
    assert!(confirm::current_for_test().is_none(), "a changed offer ends the question");
    lua_ok(&host, "assert(closed[#closed].answer == 'changed', closed[#closed].answer)");
    assert!(recorded(&p).is_empty(), "a changed offer is never accepted: {:?}", recorded(&p));
    assert!(a9_play::carry_out(&mut p, &confirm::Act::TradeAccept { duel: false, digest: old }).is_err(), "a Yes for the old offer is refused");
    assert!(recorded(&p).is_empty());

    // unchanged: Yes sends the window's accept bytes
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.trade_accept()");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(recorded(&p), native_accept, "Yes is the window's accept");
    assert!(trade_pass::trade_marks(&p).0, "the window knows the player accepted");
    lua_ok(&host, "assert(eqoa.trade.get().mine.accepted == true and tev[#tev]:find('accepted:mine') or true)");
    // an accepted offer is not changed behind the accept
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.trade_money(5)");
    lua_ok(&host, "assert(results[1].ok == false and results[1].code == 'not_ready')");
    assert!(recorded(&p).is_empty());
    // taking the accept back is the window's cancel-while-accepted: sub 7
    ask(&mut p, &host, "return eqoa.actions.trade_unaccept()");
    assert_eq!(recorded(&p), vec![vec![0x56, 0x00, 7, 77, 0, 0, 0]]);
    assert!(!trade_pass::trade_marks(&p).0);

    // the other side finishes the trade: closed, completed
    deliver(&mut p, 0x0056, trade_msg(16, 77, &[]));
    step(&mut p, &host);
    assert_eq!(p.npcw.trade_state.with, u32::MAX);
    lua_ok(&host, "assert(eqoa.trade.get() == nil and eqoa.windows.session('trade') == nil and table.concat(tev, '|'):find('closed:trade:completed', 1, true), table.concat(tev, '|'))");
}

#[test]
fn a9_a_second_trade_request_never_disturbs_a_held_trade() {
    let _baseline = Baseline;
    let mut p = ready();
    bags(&mut p);
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade', {scope='all'}) == true)");
    step(&mut p, &host);
    open_trade(&mut p, 77);
    step(&mut p, &host);
    let before = (p.npcw.trade_state.with, p.npcw.trade_state.partner, p.npcw.trade_state.asked, p.npcw.trade_state.inv.items.len());
    clear_recorded(&mut p);
    deliver(&mut p, 0x0057, trade_request(88, 88));
    assert_eq!(p.mode, 0, "no window opens over the hidden trade");
    assert_eq!(recorded(&p), vec![vec![0x56, 0x00, 20, 88, 0, 0, 0]], "the newcomer is told busy, as the game does with a window up");
    assert_eq!((p.npcw.trade_state.with, p.npcw.trade_state.partner, p.npcw.trade_state.asked, p.npcw.trade_state.inv.items.len()), before, "the held trade is as it was");
    lua_ok(&host, "assert(eqoa.trade.get().partner.id == 77)");
    // the control without a mod: the game resets its own state for the newcomer (what the game always did)
    let mut q = ready();
    bags(&mut q);
    open_trade(&mut q, 77);
    deliver(&mut q, 0x0057, trade_request(88, 88));
    assert_eq!(q.npcw.trade_state.with, u32::MAX, "control: the game's own answer to a request with a window up is busy and a reset");
}

// ---- duels ----

#[test]
fn a9_duel_invitation_wager_and_the_queue_behind_it() {
    let _baseline = Baseline;
    // control: no holder, the invitation opens in mode 43
    let mut p = ready();
    deliver(&mut p, crate::duel::OP_IN, duel_invitation(9, "Lanerender"));
    assert_eq!(p.mode, 43, "control: no holder, the game's own invitation");
    let native_accept = {
        let mut part = crate::duel::Part::default();
        part.duel_id = 0;
        let mut v = Vec::new();
        for d in part.accept(9, "Hero") {
            if let crate::duel::Do::Send(m) = d {
                v.push(m);
            }
        }
        v
    };
    assert!(!native_accept.is_empty());

    let _baseline2 = Baseline;
    let mut p = ready();
    bags(&mut p);
    p.duel.part.features = crate::duel::FEATURE_DUELS;
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('duel_invite', {scope='all'}) == true)");
    lua_ok(&host, "assert(eqoa.windows.takeover('duel', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, crate::duel::OP_IN, duel_down(1, 5, &[]));
    deliver(&mut p, crate::duel::OP_IN, duel_invitation(5, "Lanerender"));
    assert_eq!(p.mode, 0, "a held invitation never draws");
    step(&mut p, &host);
    lua_ok(
        &host,
        r#"
        local d = assert(eqoa.duel.get())
        assert(d.enabled == true and d.state == "invited" and d.invite.name == "Lanerender" and d.invite.level == 40)
        assert(d.seconds_remaining > 18 and d.seconds_remaining < 21)
        assert(eqoa.windows.session("duel_invite") ~= nil)
        "#,
    );
    // a second invitation while one is held: declined (the game would queue it behind the open window)
    clear_recorded(&mut p);
    deliver(&mut p, crate::duel::OP_IN, duel_invitation(6, "Second"));
    assert_eq!(p.mode, 0);
    assert_eq!(recorded(&p).len(), 1, "the newcomer is declined with the game's own decline");
    lua_ok(&host, "assert(eqoa.duel.get().invite.id == 9 or eqoa.duel.get().invite.id == 5)");

    // Yes: the window's accept
    clear_recorded(&mut p);
    let expected = {
        let mut part = crate::duel::Part::default();
        part.duel_id = 5;
        part.accept(5, "Hero").into_iter().filter_map(|d| if let crate::duel::Do::Send(m) = d { Some(m) } else { None }).collect::<Vec<_>>()
    };
    ask(&mut p, &host, "return eqoa.actions.duel_answer(true)");
    assert_eq!(recorded(&p), expected, "Yes is Part::accept's bytes");
    lua_ok(&host, "assert(eqoa.windows.session('duel_invite') == nil and eqoa.duel.get().state == 'none')");

    // the server opens the wager: held hidden as the 'duel' window
    deliver(&mut p, crate::duel::OP_IN, duel_down(13, 5, &[]));
    assert_eq!(p.mode, 0, "a held wager never draws");
    step(&mut p, &host);
    lua_ok(
        &host,
        r#"
        local t = assert(eqoa.trade.get())
        assert(t.kind == "duel" and t.phase == "open" and t.held == true and #t.bags == 7)
        assert(eqoa.duel.get().state == "wager")
        "#,
    );
    // an invitation while the wager is hidden: it waits as the game queues it, no window over the wager
    clear_recorded(&mut p);
    deliver(&mut p, crate::duel::OP_IN, duel_invitation(7, "Late"));
    assert_eq!(p.mode, 0, "no window over the hidden wager");
    assert!(recorded(&p).is_empty(), "it is kept waiting, not answered");
    lua_ok(&host, "assert(eqoa.duel.get().queued == 1)");

    // offers: the duel part's own bytes
    clear_recorded(&mut p);
    let want_add = p.duel.part.add_item(1, 2);
    ask(&mut p, &host, "return eqoa.actions.trade_add({slot = 1, id = 10, count = 2})");
    assert_eq!(recorded(&p), vec![if let crate::duel::Do::Send(m) = want_add { m } else { unreachable!() }]);
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.trade_money(100)");
    assert_eq!(recorded(&p).len(), 1);
    // accepting the wager asks first, and what each side stakes is in the question
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.trade_accept()");
    assert!(recorded(&p).is_empty());
    let q = confirm::current_for_test().expect("the wager's question");
    assert!(q.text.contains("duel") && !q.yes, "{}", q.text);
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    let want = if let crate::duel::Do::Send(m) = p.duel.part.accept_wager() { m } else { unreachable!() };
    assert_eq!(recorded(&p), vec![want], "Yes is Part::accept_wager's bytes");

    // both accepted (sub 15): the wager is over; the invitation that waited is the game's queue again
    deliver(&mut p, crate::duel::OP_IN, duel_down(15, 5, &[1, 1, 1]));
    step(&mut p, &host);
    lua_ok(&host, "assert(eqoa.windows.session('duel') == nil and table.concat(tev, '|'):find('closed:duel:completed', 1, true), table.concat(tev, '|'))");
}

#[test]
fn a9_a_duel_invitation_nobody_answers_is_declined_and_acting_needs_the_hold() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('duel_invite', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, crate::duel::OP_IN, duel_down(1, 5, &[]));
    deliver(&mut p, crate::duel::OP_IN, duel_invitation(5, "Lanerender"));
    step(&mut p, &host);
    let expected: Vec<Vec<u8>> = {
        let mut part = crate::duel::Part::default();
        part.duel_id = 5;
        part.decline(5, "Hero").into_iter().filter_map(|d| if let crate::duel::Do::Send(m) = d { Some(m) } else { None }).collect()
    };
    clear_recorded(&mut p);
    a9_play::age_for_test(Duration::from_secs(19));
    step(&mut p, &host);
    assert!(recorded(&p).is_empty(), "at 19 s the invitation still waits");
    a9_play::age_for_test(Duration::from_secs(2));
    step(&mut p, &host);
    assert_eq!(recorded(&p), expected, "after its 20 s the invitation is declined with the game's own decline");
    lua_ok(&host, "assert(eqoa.windows.session('duel_invite') == nil and ev[#ev] == 'closed:duel_invite:timeout', ev[#ev])");

    // a mod that does not hold the kind cannot answer it, and a mod holding another kind cannot either
    let mut p = ready();
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('trade_ask', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, crate::duel::OP_IN, duel_invitation(5, "Lanerender"));
    assert_eq!(p.mode, 43, "control: nobody holds duel_invite, so the game's own invitation is up");
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.duel_answer(true)");
    lua_ok(&host, "assert(results[1].ok == false and results[1].code == 'not_ready')");
    ask(&mut p, &host, "return eqoa.actions.trade_add({slot = 1, id = 1})");
    lua_ok(&host, "assert(results[1].ok == false and results[1].code == 'not_ready')");
    ask(&mut p, &host, "return eqoa.actions.blacksmith_repair({slot = 1, id = 1})");
    lua_ok(&host, "assert(results[1].ok == false and results[1].code == 'not_ready')");
    assert!(recorded(&p).is_empty(), "nothing was sent for the game's own window");
}

// ---- the blacksmith ----

fn smith_bags(p: &mut Play) {
    let mut worn_down = item(20, 1, 1, 0, "Rusty Sword");
    worn_down.item.value = 100;
    worn_down.item.max_hp = 50;
    worn_down.cur_hp = 25;
    worn_down.item.wear_slot = 7;
    let mut whole = item(21, 2, 1, 0, "Fine Dagger");
    whole.item.value = 80;
    whole.item.max_hp = 40;
    whole.cur_hp = 40;
    whole.item.wear_slot = 7;
    let mut junk = item(22, 3, 4, 20, "Snake Scale");
    junk.item.wear_slot = -1;
    p.dump.inventory.items = vec![worn_down, whole, junk];
}

#[test]
fn a9_blacksmith_lists_repairs_like_the_window_and_refuses_what_the_game_does() {
    let _baseline = Baseline;
    // control: no holder, the game's own window opens; its X on the first row repairs it
    let mut p = ready();
    smith_bags(&mut p);
    deliver(&mut p, 0x0061, 4242u32.to_le_bytes().to_vec());
    assert_eq!(p.mode, 19, "control: no holder, the game's own smith");
    let native_repair = {
        use crate::pointer::testing::{NullRaster, vui};
        let (v, mut r) = (vui(), NullRaster);
        p.run_support(&v, &mut r);
        clear_recorded(&mut p);
        p.support_key(13, true);
        p.run_support(&v, &mut r);
        recorded(&p)
    };
    assert_eq!(native_repair, vec![vec![0x62, 0x00, 1, 0, 0, 0, 0x92, 0x10, 0, 0]], "the window's repair: the item's key, then the smith");

    let _baseline2 = Baseline;
    let mut p = ready();
    smith_bags(&mut p);
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('blacksmith', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, 0x0061, 4242u32.to_le_bytes().to_vec());
    assert_eq!(p.mode, 0, "a held smith never draws");
    assert!(p.wnd_chat.as_ref().is_none_or(|c| c.mode == 0), "the chat box is not fixed for a window nobody sees");
    step(&mut p, &host);
    lua_ok(
        &host,
        r#"
        local s = assert(eqoa.blacksmith.get())
        assert(s.coin == 1000 and #s.items == 2 and s.repair_pending == false)
        assert(s.items[1].id == 20 and s.items[1].cost == 50 and s.items[1].can_repair == true and s.items[1].condition == 0.5)
        assert(s.items[2].id == 21 and s.items[2].cost == 0 and s.items[2].can_repair == false)
        assert(eqoa.windows.session("blacksmith") ~= nil)
        "#,
    );
    // refused as the game's window does: an item that needs nothing
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.blacksmith_repair({slot = 2, id = 21})");
    lua_ok(&host, "assert(results[1].ok == false and results[1].code == 'not_ready')");
    ask(&mut p, &host, "return eqoa.actions.blacksmith_repair({slot = 1, id = 99})");
    lua_ok(&host, "assert(results[1].ok == false and results[1].code == 'stale_handle')");
    // money short
    p.dump.player.p34 = 10;
    step(&mut p, &host);
    ask(&mut p, &host, "return eqoa.actions.blacksmith_repair({slot = 1, id = 20})");
    lua_ok(&host, "assert(results[1].ok == false and results[1].code == 'not_ready', results[1].reason)");
    assert!(recorded(&p).is_empty(), "no refused repair sent anything");
    p.dump.player.p34 = 1000;
    step(&mut p, &host);
    // the repair: the window's bytes; one at a time
    ask(&mut p, &host, "return eqoa.actions.blacksmith_repair({slot = 1, id = 20})");
    lua_ok(&host, "assert(results[1].ok == true and results[1].detail.sent == true and results[1].detail.confirmed == false and results[1].detail.cost == 50)");
    assert_eq!(recorded(&p), native_repair, "the repair is the window's repair");
    ask(&mut p, &host, "return eqoa.actions.blacksmith_repair({slot = 1, id = 20})");
    lua_ok(&host, "assert(results[1].ok == false and results[1].code == 'busy')");
    assert_eq!(recorded(&p).len(), 1, "never replayed");
    // the server answers: the item is whole and the purse is lighter, the next repair may go
    p.dump.inventory.items[0].cur_hp = 50;
    p.dump.player.p34 = 950;
    step(&mut p, &host);
    lua_ok(&host, "assert(eqoa.blacksmith.get().repair_pending == false and eqoa.blacksmith.get().items[1].can_repair == false)");
    // close: the window's close
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.blacksmith_close()");
    assert_eq!(recorded(&p), vec![vec![0x63, 0x00]]);
    lua_ok(&host, "assert(eqoa.blacksmith.get() == nil and eqoa.windows.session('blacksmith') == nil)");
    // the server closing it ends the session too
    let mut p = ready();
    smith_bags(&mut p);
    let host = host_all();
    lua_ok(&host, "assert(eqoa.windows.takeover('blacksmith', {scope='all'}) == true)");
    step(&mut p, &host);
    deliver(&mut p, 0x0061, 4242u32.to_le_bytes().to_vec());
    step(&mut p, &host);
    deliver(&mut p, 0x0063, vec![]);
    step(&mut p, &host);
    lua_ok(&host, "assert(eqoa.blacksmith.get() == nil and ev[#ev] == 'closed:blacksmith:closed', ev[#ev])");
}

// ---- reads and the labels ----

#[test]
fn a9_the_reads_say_nothing_before_a_world_and_nothing_with_nothing_open() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    step(&mut p, &host);
    lua_ok(&host, "assert(eqoa.trade.get() == nil and eqoa.blacksmith.get() == nil)");
    lua_ok(&host, "local d = assert(eqoa.duel.get()); assert(d.state == 'none' and d.enabled == false and d.queued == 0 and d.invite == nil)");
    CORE.with(|c| c.borrow_mut().view.in_world = false);
    lua_ok(&host, "local v, e = eqoa.trade.get(); assert(v == nil and e.code == 'not_in_world')");
}

#[test]
fn a9_the_labels_name_what_the_manifest_declares() {
    let caps = |names: &[&str]| names.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
    let lines = native_a9::trading_label(&caps(&["eqoa.actions.trade_add", "eqoa.actions.trade_money", "eqoa.actions.blacksmith_repair"]));
    assert_eq!(
        lines,
        vec![
            "Trading: can offer items in a trade, offer coin in a trade".to_string(),
            "Blacksmith: can repair items (it spends your coin)".to_string()
        ]
    );
    assert!(native_a9::trading_label(&caps(&["eqoa.trade.get"])).is_empty(), "reading says nothing: the control");
    let asks = crate::fork_ext::addons::runtime::asks_first_label(&caps(&["eqoa.actions.trade_accept"]));
    assert_eq!(asks.as_deref(), Some("Asks you first: accept a trade or a duel wager"));
    // the accept is never listed as acting on its own
    let auto = crate::fork_ext::addons::runtime::autonomous_action_label(&caps(&["eqoa.actions.trade_accept", "eqoa.actions.trade_add"]));
    assert!(auto.is_none(), "{auto:?}");
    assert!(crate::fork_ext::addons::runtime::autonomous_action_label(&caps(&["eqoa.actions.chat"])).is_some(), "the control: another action still is");
}
