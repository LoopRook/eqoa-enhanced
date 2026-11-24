// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A11 from Play-shaped state: a window the server opens over a hidden session (the backlog item of
//! 2026-11-18), the mercenary windows' requests, the dye picker's token and its asking first, and fishing's keys and
//! requests. Where the game has a native press for the same act, the bytes are compared with that press.
//!
//! Each test says what its negative control is: the behaviour without the change would fail the assertion named.
use super::*;
use crate::fishing::state;
use crate::fork_ext::addons::{
    action_broker, confirm,
    host::Host,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws,
    window_takeover::{self, TakeoverKind, TakeoverScope},
};
use crate::pointer::testing::{NullRaster, vui};
use net::mercwin::testing::{item, spells_body, window_body};
use net::mercwin::{CARRIED, OPEN, SLOTS, STOCK, WORN};
use winit::keyboard::{Key, NamedKey};

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    reset_for_test();
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p.session.record = Some(Vec::new());
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    p
}

/// What the client sent since the last call, whole.
fn sent(p: &mut Play) -> Vec<Vec<u8>> {
    p.session.record.as_mut().map(std::mem::take).unwrap_or_default().into_iter().map(|(_, d)| d).collect()
}

/// What the client sent since the last call, of one opcode's low and high byte.
fn sent_op(p: &mut Play, lo: u8, hi: u8) -> Vec<Vec<u8>> {
    sent(p).into_iter().filter(|d| d.len() >= 2 && d[0] == lo && d[1] == hi).collect()
}

/// A hidden bank session of some mod: the game's mode reads 0 while the server's bank is open for it.
fn hide_a_session(p: &mut Play) {
    let _ = p;
    assert!(window_takeover::takeover(7, "mod", TakeoverKind::Bank, TakeoverScope::Own).is_ok());
    assert!(window_takeover::activate_hidden_for_test(7, TakeoverKind::Bank, 3));
    assert!(window_takeover::is_hidden_active(TakeoverKind::Bank));
}

fn merc_items() -> Vec<net::mercwin::MercRecord> {
    vec![item(7, WORN, "Mithril Breastplate", 5, 5), item(9, CARRIED, "Buckler", 13, -1), item(11, STOCK, "Healing Potion", 20, -1)]
}

fn open_merc(p: &mut Play) {
    p.session.merc_windows = true;
    p.session.mercs = net::session::Mercs::read(&[1, 1, 1, 0, 3, b'K', b'e', b'l']);
    assert!(p.merc_win_opcode(net::mercwin::OP_MERC_WINDOW_STATE, &window_body(OPEN, 1, "Kel", &merc_items())));
}

fn dye_open(token: u32, alpha: bool) -> Vec<u8> {
    let mut v = vec![net::dyepick::OPEN];
    v.extend(token.to_le_bytes());
    v.push(0); // a character's
    v.push(alpha as u8);
    v.extend(0x1122_33FFu32.to_le_bytes());
    v.push(4);
    v.extend(b"Dye!");
    v.push(1);
    v.push(0); // slot 0
    v.push(0); // no palette
    v
}

/// The picker up, as the server's open and the next HUD frame make it.
fn open_dye(p: &mut Play, token: u32, alpha: bool) {
    p.dyepick_opcode(&dye_open(token, alpha));
    let (v, mut r) = (vui(), NullRaster);
    p.run_dyepick(&v, &mut r);
}

fn rig() -> (Play, Host) {
    let p = ready();
    let host = Host::with_test_capabilities(&[
        "eqoa.actions.*",
        "eqoa.merc.get",
        "eqoa.dye.get",
        "eqoa.fishing.get",
        "eqoa.windows.session",
        "eqoa.events.on",
    ])
    .unwrap();
    host.execute(
        r#"
        results, closed = {}, {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        eqoa.events.on("confirm.closed", function(e) closed[#closed + 1] = e.data end)
        evs = {}
        for _, n in ipairs({"merc.changed", "dye.changed", "fishing.changed"}) do
            eqoa.events.on(n, function(e) evs[#evs + 1] = n .. ":" .. tostring(e.data.open) .. ":" .. tostring(e.data.revision ~= nil) end)
        end
        eqoa.input.bind({ id = "go", label = "Go", default_key = "9" }, function() last_req, last_err = next_fn() end)
        "#,
    )
    .unwrap();
    (p, host)
}

fn step(p: &mut Play, host: &Host) {
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn press_9(host: &Host) {
    CORE.with(|c| {
        c.borrow_mut().emit("input.key", serde_json::json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}))
    });
    native_events::dispatch(host).unwrap();
}

/// The mod asks in a player gesture (a key press), the game steps, the mod hears. The result's `ok`, `code`.
fn ask(p: &mut Play, host: &Host, code: &str) -> (bool, String) {
    host.execute(&format!("results = {{}}; closed = {{}}; last_err = nil; last_req = nil; next_fn = function() {code} end")).unwrap();
    press_9(host);
    step(p, host);
    host.lua
        .load("local r = results[#results]; if r == nil then return false, last_err and last_err.code or 'none' end return r.ok == true, r.code or ''")
        .eval()
        .unwrap()
}

fn key(k: NamedKey) {
    assert!(confirm::key(&Key::Named(k)));
}

// ---- windows the server opens over a hidden session ---------------------------------------------------------------

/// A mercenary's window opened by the server while a hidden session holds the game is not opened over it: the game
/// does not open one over any window. The state is kept, and with the hidden session gone the same open works.
/// Negative control: without the hook, the first assertion (mode stays 0) fails, the window opens over the session.
#[test]
fn a11_a_mercenary_window_does_not_open_over_a_hidden_session() {
    let mut p = ready();
    hide_a_session(&mut p);
    open_merc(&mut p);
    assert_eq!(p.mode, 0, "the game's mode stays plain: nothing opened over the hidden session");
    assert!(p.merc.window.is_some(), "the state is kept: a later Items > Inventory shows it");
    let log = window_takeover::take_log_for_test().join("\n");
    assert!(log.contains("mercenary window") && log.contains("hidden session"), "{log}");
    // the session ends: the open that waited is made, not lost (a tick, with no new packet)
    window_takeover::set_hidden_inactive(TakeoverKind::Bank);
    tick(&mut p, true);
    assert_eq!(p.mode, crate::play::mercplay::ITEMS_MODE, "the waiting inventory window opens when the hidden session ends");
    let (v, mut r) = (vui(), NullRaster);
    p.run_merc_windows(&v, &mut r);
    // the control: a window the server opens with no hidden session opens at once, as before
    p.mode = 0;
    p.merc_win_opcode(net::mercwin::OP_MERC_WINDOW_STATE, &window_body(OPEN, 1, "Kel", &merc_items()));
    assert_eq!(p.mode, crate::play::mercplay::ITEMS_MODE);
    // the spells window waits the same way, and a hire gone while it waits opens nothing
    p.mode = 0;
    hide_a_session(&mut p);
    p.merc_win_opcode(net::mercwin::OP_MERC_SPELLS_STATE, &spells_body(OPEN, &[(950, 5, -1)], [0; SLOTS]));
    assert_eq!(p.mode, 0, "the spell window does not open over it either");
    window_takeover::set_hidden_inactive(TakeoverKind::Bank);
    tick(&mut p, true);
    assert_eq!(p.mode, crate::play::mercplay::SPELLS_MODE);
    p.mode = 0;
    hide_a_session(&mut p);
    p.merc_win_opcode(net::mercwin::OP_MERC_WINDOW_STATE, &window_body(OPEN, 1, "Kel", &merc_items()));
    p.merc_win_opcode(net::mercwin::OP_MERC_WINDOW_STATE, &window_body(net::mercwin::GONE, 1, "Kel", &[]));
    window_takeover::set_hidden_inactive(TakeoverKind::Bank);
    tick(&mut p, true);
    assert_eq!(p.mode, 0, "the hire is gone: nothing opens");
}

/// The dye picker waits where the game keeps it waiting (a window up) and opens when the hidden session ends.
/// Negative control: without the hook the picker opens at once, mode 63 over the session.
#[test]
fn a11_the_dye_picker_waits_for_a_hidden_session_to_end() {
    let mut p = ready();
    hide_a_session(&mut p);
    p.dyepick_opcode(&dye_open(0xAABB_CCDD, true));
    let (v, mut r) = (vui(), NullRaster);
    for _ in 0..3 {
        p.run_dyepick(&v, &mut r);
    }
    assert_eq!(p.mode, 0, "waiting, not over the hidden session");
    assert!(!p.dyepick.picker.open);
    window_takeover::set_hidden_inactive(TakeoverKind::Bank);
    p.run_dyepick(&v, &mut r);
    assert_eq!(p.mode, crate::play::dyeplay::MODE, "the session ended: the picker opens, as when a game window closes");
    assert_eq!(p.dyepick.open.as_ref().map(|o| o.token), Some(0xAABB_CCDD));
}

/// A fishing session that starts over a hidden session is cancelled to the server, as the game cancels one that
/// starts under another window. Negative control: without the hook mode 36 opens and nothing is cancelled. The
/// control after it: with no hidden session the session opens and nothing is cancelled.
#[test]
fn a11_a_fishing_session_over_a_hidden_session_is_cancelled_as_under_any_window() {
    let mut p = ready();
    hide_a_session(&mut p);
    sent(&mut p);
    assert!(p.fishing_opcode(crate::fishing::OP_IN, &[1]));
    assert_eq!(p.mode, 0);
    assert_eq!(sent_op(&mut p, 0x01, 0x0F), vec![crate::fishing::sub_msg(2)], "the game's own cancel, 0x0F01 2");
    assert!(p.chat.iter().any(|(t, _)| t.starts_with("Fishing stopped: a mod window was busy")), "the player is told why");
    window_takeover::set_hidden_inactive(TakeoverKind::Bank);
    assert!(p.fishing_opcode(crate::fishing::OP_IN, &[1]));
    assert_eq!(p.mode, 36, "no hidden session: the window opens as before");
    assert!(sent_op(&mut p, 0x01, 0x0F).is_empty());
}

// ---- the mercenary's requests ---------------------------------------------------------------------------------------

/// Equip, Unequip, Take Back and a stock take send the bytes the window's own menu sends (the same literal the game's
/// window test pins), one at a time, only for a record the merc holds in that way.
/// Negative control: dropping the record check makes the stale ids reach the server.
#[test]
fn a11_merc_requests_are_the_windows_own_bytes_one_at_a_time() {
    let mut p = ready();
    open_merc(&mut p);
    sent(&mut p);
    assert!(matches!(dispatch(&mut p, &A11Action::MercEquip { id: 9 }), DispatchOutcome::Completed { .. }));
    assert_eq!(sent_op(&mut p, 0x73, 0x7E), vec![vec![0x73, 0x7E, 1, 3, 9, 0, 0, 0, 0xFF]], "the window's Equip, from its own test");
    assert!(p.merc.waiting);
    assert!(matches!(dispatch(&mut p, &A11Action::MercTake { id: 9 }), DispatchOutcome::Refused { code: "busy", .. }), "one ask at a time");
    assert!(sent(&mut p).is_empty());
    p.merc_win_opcode(net::mercwin::OP_MERC_WINDOW_RESULT, &[0x73, 3, 1]);
    // each in its kind
    for (action, code) in [
        (A11Action::MercEquip { id: 7 }, "stale_handle"),
        (A11Action::MercUnequip { id: 9 }, "stale_handle"),
        (A11Action::MercTake { id: 99 }, "stale_handle"),
        (A11Action::MercStock { id: 9, count: 1 }, "stale_handle"),
    ] {
        assert!(matches!(dispatch(&mut p, &action), DispatchOutcome::Refused { code: c, .. } if c == code), "{action:?}");
    }
    assert!(sent(&mut p).is_empty(), "nothing went for a record that is not there");
    assert!(matches!(dispatch(&mut p, &A11Action::MercUnequip { id: 7 }), DispatchOutcome::Completed { .. }));
    assert_eq!(sent_op(&mut p, 0x73, 0x7E), vec![crate::mercwnd::ask_bytes(crate::mercwnd::Ask::Unequip(7), 1)]);
    p.merc_win_opcode(net::mercwin::OP_MERC_WINDOW_RESULT, &[0x73, 4, 1]);
    assert!(matches!(dispatch(&mut p, &A11Action::MercStock { id: 11, count: 1 }), DispatchOutcome::Completed { .. }));
    assert_eq!(sent_op(&mut p, 0x73, 0x7E), vec![crate::mercwnd::ask_bytes(crate::mercwnd::Ask::Stock(11, 1), 1)]);
}

/// Memorizing onto one of the ten slots sends the spell window's own bytes (the literal its own test pins); a spell
/// that is not in the book, and a state the server has not sent, are refused.
#[test]
fn a11_merc_spell_slots_are_the_windows_own_bytes() {
    let mut p = ready();
    p.session.merc_windows = true;
    assert!(matches!(dispatch(&mut p, &A11Action::MercSpellsClear), DispatchOutcome::Refused { code: "not_ready", .. }), "no state yet");
    p.merc_win_opcode(net::mercwin::OP_MERC_SPELLS_STATE, &spells_body(0, &[(950, 5, -1), (951, 8, -1)], [0; SLOTS]));
    sent(&mut p);
    assert!(matches!(dispatch(&mut p, &A11Action::MercSpellSet { slot: 0, spell: 950 }), DispatchOutcome::Completed { .. }));
    assert_eq!(sent_op(&mut p, 0x76, 0x7E), vec![vec![0x76, 0x7E, 1, 2, 0, 0xB6, 3, 0, 0]], "Ice Bolt (950) onto slot 0, as the window's own test");
    assert!(matches!(dispatch(&mut p, &A11Action::MercSpellSet { slot: 1, spell: 12345 }), DispatchOutcome::Refused { code: "stale_handle", .. }));
    assert!(matches!(dispatch(&mut p, &A11Action::MercSpellSet { slot: 10, spell: 950 }), DispatchOutcome::Refused { code: "out_of_range", .. }));
    assert!(sent(&mut p).is_empty());
    assert!(matches!(dispatch(&mut p, &A11Action::MercSpellsClear), DispatchOutcome::Completed { .. }));
    assert_eq!(sent_op(&mut p, 0x76, 0x7E), vec![net::mercwin::spells_ask(1, net::mercwin::SPELLS_CLEAR, 0, 0)]);
}

/// Dismissing a mercenary is asked first: the call sends nothing, only the player's Yes sends 0x7E5C (the game's own
/// entry's bytes), and a No sends nothing. Negative control: an unasked dismissal would send at the call.
#[test]
fn a11_dismissing_a_mercenary_asks_the_player_first() {
    let (mut p, host) = rig();
    open_merc(&mut p);
    sent(&mut p);
    let (ok, _) = ask(&mut p, &host, "return eqoa.actions.merc_dismiss()");
    assert!(ok, "asked");
    assert!(sent(&mut p).is_empty(), "nothing is sent by the call");
    let q = confirm::current_for_test().expect("the question is up");
    assert!(!q.yes && q.act.kind() == "merc_dismiss", "starts on No: {}", q.text);
    key(NamedKey::Escape);
    step(&mut p, &host);
    assert!(sent(&mut p).is_empty(), "No sends nothing");
    host.execute("assert(closed[1].kind == 'merc_dismiss' and closed[1].answer == 'no')").unwrap();
    ask(&mut p, &host, "return eqoa.actions.merc_dismiss()");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(sent_op(&mut p, 0x5C, 0x7E), vec![vec![0x5C, 0x7E, 1]], "the Start menu entry's own bytes");
    host.execute("assert(closed[1].answer == 'yes')").unwrap();
    // with no merc of the player's out, the question is not even asked
    p.session.mercs = None;
    let (ok, code) = ask(&mut p, &host, "return eqoa.actions.merc_dismiss()");
    assert!(!ok && code == "not_ready", "{code}");
    assert!(!confirm::is_open());
}

// ---- the dye picker --------------------------------------------------------------------------------------------------

/// A colour is applied only with the picker's own token, echoed exactly, after the player's Yes: the bytes are the
/// picker's OK. A token that is not the picker's is refused before any question. A No sends nothing; Cancel goes at
/// once with the picker's cancel bytes. Negative control: an unchecked token would reach the server wrong.
#[test]
fn a11_the_dye_picker_echoes_its_token_and_asks_before_a_colour() {
    let (mut p, host) = rig();
    open_dye(&mut p, 0xAABB_CCDD, false);
    assert_eq!(p.mode, crate::play::dyeplay::MODE);
    sent(&mut p);
    let (ok, code) = ask(&mut p, &host, "return eqoa.actions.dye_pick(0x11111111, 0x336699FF)");
    assert!(!ok && code == "stale_handle", "a token that is not the picker's: {code}");
    assert!(!confirm::is_open());
    let (ok, _) = ask(&mut p, &host, "return eqoa.actions.dye_pick(0xAABBCCDD, 0x33669900)");
    assert!(ok);
    assert!(sent(&mut p).is_empty(), "asking sends nothing");
    let q = confirm::current_for_test().unwrap();
    assert!(q.text.contains("#336699") && !q.yes, "{}", q.text);
    key(NamedKey::Escape);
    step(&mut p, &host);
    assert!(sent(&mut p).is_empty() && p.dyepick.picker.open, "No: the picker stays as it was");
    ask(&mut p, &host, "return eqoa.actions.dye_pick(0xAABBCCDD, 0x33669900)");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    // alpha is not the player's to choose here: FF, as the picker sends it
    assert_eq!(sent_op(&mut p, 0x96, 0x7E), vec![net::dyepick::picked(0xAABB_CCDD, 0x3366_99FF)]);
    assert!(!p.dyepick.picker.open && p.mode == 0, "the picker is down, as after its OK");
    // a second pick after the picker is down has nothing to answer
    let (ok, code) = ask(&mut p, &host, "return eqoa.actions.dye_pick(0xAABBCCDD, 0x336699FF)");
    assert!(!ok && code == "not_ready", "{code}");
}

#[test]
fn a11_the_dye_picker_cancel_and_the_reads() {
    let (mut p, host) = rig();
    open_dye(&mut p, 0x0000_0102, true);
    step(&mut p, &host);
    host.execute(
        "local d = eqoa.dye.get(); assert(d.token == 0x102 and d.target == 'character' and d.title == 'Dye!' and d.open == true and d.alpha == true, d and d.title)",
    )
    .unwrap();
    sent(&mut p);
    let (ok, _) = ask(&mut p, &host, "return eqoa.actions.dye_cancel(0x102)");
    assert!(ok);
    assert_eq!(sent_op(&mut p, 0x96, 0x7E), vec![net::dyepick::cancelled(0x102)], "the picker's own Cancel");
    assert_eq!(p.mode, 0);
    step(&mut p, &host);
    host.execute("assert(eqoa.dye.get() == nil)").unwrap();
    // the open and the close are one stream: told when the picker came up and when it went
    host.execute("assert(evs[1] == 'dye.changed:true:true' and evs[#evs] == 'dye.changed:false:true', table.concat(evs, ' '))").unwrap();
}

// ---- fishing ---------------------------------------------------------------------------------------------------------

fn fishing(p: &mut Play) {
    assert!(p.fishing_opcode(crate::fishing::OP_IN, &[1]));
    assert_eq!(p.mode, 36);
    sent(p);
}

/// Reel, Pull and Keep send the catch window's own bytes; a second one waits for the server; Cancel is the Escape key's
/// (0x0F01 2, or 4 with a catch on the line). Negative control: with no guard a repeat would go out twice.
#[test]
fn a11_fishing_requests_are_the_windows_own_bytes_and_not_repeated() {
    let mut p = ready();
    fishing(&mut p);
    // nothing on the line: no reel, keep or pull
    assert!(matches!(dispatch(&mut p, &A11Action::FishReel), DispatchOutcome::Refused { code: "not_ready", .. }));
    p.fish.part.state = state::CAUGHT;
    p.fish.wnd.list_visible = true;
    let mut one = net::dump::NetItem { quantity: 3, equip_slot: -1, quick_slot: -1, inv_index: 10, ..Default::default() };
    one.item.name = "Perch".into();
    p.fish.part.catch.items.push(one);
    assert!(matches!(dispatch(&mut p, &A11Action::FishReel), DispatchOutcome::Completed { .. }));
    assert_eq!(sent(&mut p), vec![crate::fishing::sub_msg(3)], "the list's top row");
    assert!(matches!(dispatch(&mut p, &A11Action::FishReel), DispatchOutcome::Refused { code: "busy", .. }));
    assert!(matches!(dispatch(&mut p, &A11Action::FishPull), DispatchOutcome::Refused { code: "busy", .. }));
    assert!(sent(&mut p).is_empty(), "not replayed");
    // the server answers (its catch changes): the next may go
    p.fish.part.catch.items.clear();
    assert!(matches!(dispatch(&mut p, &A11Action::FishPull), DispatchOutcome::Completed { .. }));
    assert_eq!(sent(&mut p), vec![crate::fishing::sub_msg(4)]);
    // keep: a row of the catch, with its count, as the list's pick sends
    let mut again = net::dump::NetItem { quantity: 3, equip_slot: -1, quick_slot: -1, inv_index: 10, ..Default::default() };
    again.item.name = "Perch".into();
    p.fish.part.catch.items.push(again);
    p.fish.part.state = state::CAUGHT;
    assert!(matches!(dispatch(&mut p, &A11Action::FishKeep { slot: 5 }), DispatchOutcome::Refused { code: "out_of_range", .. }));
    assert!(matches!(dispatch(&mut p, &A11Action::FishKeep { slot: 0 }), DispatchOutcome::Completed { .. }));
    assert_eq!(sent(&mut p), vec![crate::fishing::keep_msg(10, 3)]);
    assert_eq!(p.fish.part.state, state::KEEPING);
    // the server answers the keep: the catch is gone and the line waits again; the guard lets go with no help from here
    p.fish.part.catch.items.clear();
    p.fish.part.state = state::WAITING;
    tick(&mut p, false);
    // Escape at a waiting line is 2
    assert!(matches!(dispatch(&mut p, &A11Action::FishCancel), DispatchOutcome::Completed { .. }));
    assert_eq!(sent(&mut p), vec![crate::fishing::sub_msg(2)]);
    assert!(matches!(dispatch(&mut p, &A11Action::FishCancel), DispatchOutcome::Refused { code: "busy", .. }), "not twice before an answer");
    // the server stops the session; a new one starts at the very same (waiting, no catch): cancel works again.
    // Negative control: the old guard never let go and refused here for the whole second session.
    p.fish.part.state = state::OFF;
    tick(&mut p, false);
    p.fish.part.state = state::WAITING;
    tick(&mut p, false);
    assert!(matches!(dispatch(&mut p, &A11Action::FishCancel), DispatchOutcome::Completed { .. }), "second session");
    assert_eq!(sent(&mut p), vec![crate::fishing::sub_msg(2)]);
    p.fish.part.state = state::OFF;
    tick(&mut p, false);
    p.fish.part.state = state::WAITING;
    tick(&mut p, false);
    // two catches of the same size, each reeled: the second is not refused
    for round in 0..2 {
        let mut fish = net::dump::NetItem { quantity: 1, equip_slot: -1, quick_slot: -1, inv_index: 20 + round, ..Default::default() };
        fish.item.name = "Perch".into();
        p.fish.part.catch.items.push(fish);
        p.fish.part.state = state::CAUGHT;
        p.fish.wnd.list_visible = true;
        tick(&mut p, false);
        assert!(matches!(dispatch(&mut p, &A11Action::FishReel), DispatchOutcome::Completed { .. }), "catch {round}");
        assert_eq!(sent(&mut p), vec![crate::fishing::sub_msg(3)]);
        p.fish.part.catch.items.clear();
        p.fish.part.state = state::WAITING;
        tick(&mut p, false);
    }
    // Escape with a catch on the line is 4
    let mut last = net::dump::NetItem { quantity: 1, equip_slot: -1, quick_slot: -1, inv_index: 30, ..Default::default() };
    last.item.name = "Perch".into();
    p.fish.part.catch.items.push(last);
    p.fish.part.state = state::CAUGHT;
    tick(&mut p, false);
    assert!(matches!(dispatch(&mut p, &A11Action::FishCancel), DispatchOutcome::Completed { .. }));
    assert_eq!(sent(&mut p), vec![crate::fishing::sub_msg(4)]);
}

/// The cast is the game's window's own: the call presses its key, the meter and its clock are the window's, and a mod
/// has no way to give a power. Release needs a cast under way; a cast under way is not started again; none of it
/// without the window up.
#[test]
fn a11_fishing_cast_is_the_windows_own_key_never_a_chosen_power() {
    let mut p = ready();
    assert!(matches!(dispatch(&mut p, &A11Action::FishCast), DispatchOutcome::Refused { code: "not_ready", .. }), "no window");
    fishing(&mut p);
    p.fish.part.state = state::WAITING;
    assert!(matches!(dispatch(&mut p, &A11Action::FishRelease), DispatchOutcome::Refused { code: "not_ready", .. }), "nothing cast yet");
    assert!(matches!(dispatch(&mut p, &A11Action::FishCast), DispatchOutcome::Completed { detail: Some(d) } if d["pressed"] == true));
    assert!(sent(&mut p).is_empty(), "the throw is the window's: nothing goes to the server at the press");
    p.fish.wnd.cast(0);
    assert!(matches!(dispatch(&mut p, &A11Action::FishCast), DispatchOutcome::Refused { code: "busy", .. }), "a cast is under way");
    // the release is the game's own second press of the same key (the window throws when its meter comes round)
    assert!(matches!(dispatch(&mut p, &A11Action::FishRelease), DispatchOutcome::Completed { detail: Some(d) } if d["pressed"] == true));
    // the action API has no argument for a power: a number given is no power, it is an argument error
    let (_p2, host) = rig();
    host.execute("local ok, e = eqoa.actions.fishing_cast(60); assert(ok == nil and e and e.code == 'invalid_argument', e and e.code)").unwrap();
}

/// The three reads, and nil when there is nothing to read; a merc's records and slots as the window holds them.
#[test]
fn a11_the_reads_say_what_the_windows_hold() {
    let (mut p, host) = rig();
    step(&mut p, &host);
    host.execute("assert(eqoa.merc.get() == nil and eqoa.dye.get() == nil and eqoa.fishing.get() == nil)").unwrap();
    open_merc(&mut p);
    p.mode = 0;
    p.merc_win_opcode(net::mercwin::OP_MERC_SPELLS_STATE, &spells_body(0, &[(950, 5, 1)], [0; SLOTS]));
    fishing(&mut p);
    p.fish.part.state = state::WAITING;
    step(&mut p, &host);
    host.execute(
        r#"
        local m = eqoa.merc.get()
        assert(m.available == true and m.own_out == true and m.inventory.open == false and m.inventory.name == 'Kel')
        assert(#m.inventory.records == 3 and m.inventory.records[2].kind == 'carried' and m.inventory.records[2].id == 9)
        assert(m.inventory.records[3].kind == 'stock' and m.spells.spells[1].id == 950)
        local f = eqoa.fishing.get()
        assert(f.open == true and f.state == 'waiting' and f.casting == false and f.perfect_from == 56 and f.perfect_to == 63)
        "#,
    )
    .unwrap();
    // the revision only moves when a field does
    host.execute("local seen = table.concat(evs, ' '); assert(seen:find('merc.changed:false:true') and seen:find('fishing.changed:true:true'), seen)").unwrap();
    host.execute("rev = eqoa.merc.get().revision").unwrap();
    step(&mut p, &host);
    host.execute("assert(eqoa.merc.get().revision == rev)").unwrap();
    p.merc_win_opcode(net::mercwin::OP_MERC_WINDOW_STATE, &window_body(net::mercwin::GONE, 1, "Kel", &[]));
    step(&mut p, &host);
    host.execute("assert(eqoa.merc.get().revision ~= rev and eqoa.merc.get().inventory == nil)").unwrap();
}

/// A mod that does not declare a capability cannot read or act; with no mod the game's windows are what they were.
#[test]
fn a11_capabilities_are_checked_and_the_games_windows_are_unchanged() {
    let p = ready();
    let host = Host::with_test_capabilities(&["eqoa.merc.get"]).unwrap();
    host.execute("assert(eqoa.merc.get() == nil)").unwrap();
    host.execute("local v, e = eqoa.dye.get(); assert(v == nil and e ~= nil and e.code == 'permission_denied', e and e.code)").unwrap();
    host.execute("local r, e = eqoa.actions.merc_dismiss(); assert(r == nil and e ~= nil and e.code == 'permission_denied', e and e.code)").unwrap();
    // no hidden session, no mod: the three hooks say "open" (the game as before)
    assert!(!blocked("mercenary window"));
    drop(p);
}
