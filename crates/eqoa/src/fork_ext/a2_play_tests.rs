// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A2 from Play-shaped state: NPC dialogs and the game's yes/no offers a mod holds. A packet is handled by the
//! game's own code (`Play::show_dialog`, `dialog_box`, `summon_prompt`, the invitation's handler) between the client's
//! own begin/divert/end hooks, so the tests see what a held dialog does in the real order.
//!
//! Each test says what its negative control is: the behaviour without the change would fail the assertion named.
use super::*;
use crate::dialog::{DialogBox, Key};
use crate::fork_ext::addons::{
    action_broker, confirm, dialog_sessions,
    host::Host,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws,
    window_takeover::{self, TakeoverKind, TakeoverScope},
};
use net::session::Event;
use net::stream::Writer;
use winit::keyboard::{Key as WKey, NamedKey};

const VM: u64 = 77;

fn ready(live: bool) -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::take_log_for_test();
    r6a::forget();
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    if live {
        p.session.record = Some(Vec::new());
    } else {
        p.session.conn = Some(usize::MAX);
    }
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    p
}

/// What the session sent since the last look (live connection only).
fn sent(p: &mut Play) -> Vec<Vec<u8>> {
    p.session.record.as_mut().map(std::mem::take).unwrap_or_default().into_iter().map(|(_, d)| d).collect()
}

fn answer_msg(id: u32, choice: u8) -> Vec<u8> {
    let mut w = Writer::op(0x0035);
    w.u32(id).u8(choice);
    w.data
}

fn hold(vm: u64, kinds: &[TakeoverKind]) {
    for k in kinds {
        window_takeover::takeover(vm, "mod", *k, TakeoverScope::All).unwrap();
    }
}

fn wide(s: &str) -> Vec<u16> {
    crate::ui::wide(s)
}

fn utf(t: &[u16]) -> String {
    let end = t.iter().position(|&c| c == 0).unwrap_or(t.len());
    String::from_utf16_lossy(&t[..end])
}

/// A packet, handled by the game's own code between the client's three hooks.
fn deliver(p: &mut Play, ev: Event) {
    let cap = super::super::begin_game_event(p, &ev);
    match &ev {
        Event::Dialog { text, .. } => p.show_dialog(text),
        Event::DialogBox { id, text, options } => p.dialog_box(*id, text, options),
        Event::SummonPrompt { name, kind } => p.summon_prompt(name, *kind),
        Event::GroupInvite { id, name, .. } => {
            // Msg0773, as `play.rs` handles it
            if p.group.in_group {
                p.session.send_msg(0x0624, *id, 2);
            } else {
                p.group.invites.push(crate::group::Invite { id: *id, name: name.clone() });
                if p.mode == 0 {
                    p.show_invite(name);
                }
            }
        }
        Event::GroupInviteGone(id) => {
            if let Some(k) = p.group.invites.iter().position(|i| i.id == *id) {
                p.group.invites.remove(k);
            }
        }
        other => panic!("not a dialog packet: {other:?}"),
    }
    super::super::divert_event(p);
    super::super::end_game_event(p, cap);
}

fn text(p: &mut Play, s: &str) {
    deliver(p, Event::Dialog { id: 0, text: wide(s) });
}

fn dialog(p: &mut Play, id: u32, body: &str, options: &[&str]) {
    deliver(p, Event::DialogBox { id, text: wide(body), options: options.iter().map(|o| wide(o)).collect() });
}

fn session_of(kind: &str) -> Option<ws::Session> {
    ws::sessions_for_test().into_iter().find(|s| s.kind == kind)
}

fn pump(p: &mut Play) {
    super::super::begin_iteration(p, true);
}

fn rig() -> (Play, Host) {
    rig_with(false)
}

fn rig_with(live: bool) -> (Play, Host) {
    let mut p = ready(live);
    p.forge_snapshot(None);
    r6a::refresh(&p, None);
    let host = Host::with_test_capabilities(&[
        "eqoa.actions.*",
        "eqoa.windows.session",
        "eqoa.windows.takeover",
        "eqoa.events.on",
    ])
    .unwrap();
    host.execute(
        r#"
        results, closed, ev = {}, {}, {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        eqoa.events.on("confirm.closed", function(e) closed[#closed + 1] = e.data end)
        eqoa.events.on("windows.session", function(e) ev[#ev + 1] = e.data.phase .. ":" .. e.data.kind .. ":" .. tostring(e.data.reason) end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "9" }, function() last_req, last_err = next_fn() end)
        "#,
    )
    .unwrap();
    (p, host)
}

fn vm_of(host: &Host) -> u64 {
    native_events::bus(&host.lua).borrow().vm
}

/// The mod asks (in a player gesture, as a key press), the game steps, the mod hears.
fn ask(p: &mut Play, host: &Host, code: &str) {
    r6a::refresh(p, None);
    host.execute(&format!("results = {{}}; closed = {{}}; last_err = nil; last_req = nil; next_fn = function() {code} end")).unwrap();
    CORE.with(|c| {
        c.borrow_mut().emit("input.key", serde_json::json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}))
    });
    native_events::dispatch(host).unwrap();
    pump(p);
    native_events::dispatch(host).unwrap();
}

fn key(k: NamedKey) {
    assert!(confirm::key(&WKey::Named(k)));
}

/// The game's own keys on the same dialog, with nobody holding it: what the player's press sends.
fn native_answer(id: u32, body: &str, options: &[&str], downs: usize) -> Vec<Vec<u8>> {
    let mut p = ready(true);
    dialog(&mut p, id, body, options);
    assert_eq!(p.mode, 7, "nobody holds it: the game's own window");
    for _ in 0..downs {
        p.dialog_key(Key::Down);
    }
    p.dialog_key(Key::Enter);
    assert_eq!(p.mode, 0);
    sent(&mut p)
}

const OPTIONS: [&str; 8] = ["One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight"];

#[test]
fn a2_a_held_text_dialog_is_a_session_and_not_the_games_window() {
    let mut p = ready(false);
    // the control: nobody holds it, and the game's own window opens (this is what the held case must not do)
    text(&mut p, "Hail, traveller");
    assert_eq!(p.mode, 9, "control: the game shows its window");
    let mut p = ready(false);
    hold(VM, &[TakeoverKind::TextDialog]);
    text(&mut p, "Hail, traveller");
    assert_eq!(p.mode, 0, "held: the game's window stays away");
    let s = session_of("text_dialog").expect("a session");
    assert_eq!((s.origin, s.opened_from.as_str()), ("server", "npc"));
    assert_eq!(s.data["text"], "Hail, traveller");
    assert_eq!(s.vm, VM);
    let log = window_takeover::take_log_for_test().join("\n");
    assert!(log.contains("text_dialog HIDDEN for the mod"), "{log}");
}

#[test]
fn a2_a_window_already_open_before_the_packet_keeps_its_text_for_the_mod_only_when_it_ends() {
    let mut p = ready(false);
    hold(VM, &[TakeoverKind::TextDialog]);
    // the start menu is up: the game queues the words behind it
    p.mode = 13;
    text(&mut p, "Behind the menu");
    assert!(p.dialogs.is_empty(), "taken out of the game's queue, so the game does not open it itself");
    assert!(session_of("text_dialog").is_none(), "not shown over a game window");
    p.mode = 0;
    pump(&mut p);
    let s = session_of("text_dialog").expect("shown once play is plain again, as the game's UpdateGame would");
    assert_eq!(s.data["text"], "Behind the menu");
    // and a dialog the game had already opened before the claim is the player's
    let mut p = ready(false);
    text(&mut p, "Already up");
    assert_eq!(p.mode, 9);
    hold(VM, &[TakeoverKind::TextDialog]);
    text(&mut p, "Next");
    assert_eq!(p.mode, 9, "the open window stays the player's");
    assert_eq!(utf(p.priv_chat.read_text()), "Already up");
}

#[test]
fn a2_pass_shows_the_same_dialog_in_the_games_window() {
    let (mut p, host) = rig();
    hold(vm_of(&host), &[TakeoverKind::ResponseDialog, TakeoverKind::TextDialog]);
    dialog(&mut p, 5, "Choose", &OPTIONS[..3]);
    assert_eq!(p.mode, 0);
    host.execute("s = eqoa.windows.session('response_dialog'); assert(s ~= nil and #s.data.options == 3 and s.data.text == 'Choose'); assert(eqoa.windows.pass(s.ref))").unwrap();
    pump(&mut p);
    assert_eq!(p.mode, 7, "the game's window opens for it (negative control: no pass leaves mode 0)");
    let (shown, opts, _) = p.response.read_state();
    assert_eq!(utf(shown), "Choose");
    assert_eq!(opts.iter().filter(|(_, k, _)| *k == 1).count(), 3);
    native_events::dispatch(&host).unwrap();
    host.execute("assert(ev[#ev] == 'closed:response_dialog:passed', ev[#ev]); assert(eqoa.windows.session('response_dialog') == nil)").unwrap();
    assert!(window_takeover::is_taken(TakeoverKind::ResponseDialog), "the claim stays for the next one");
}

#[test]
fn a2_answering_a_session_sends_what_the_games_own_key_sends() {
    for pick in [0usize, 2, 7] {
        let want = native_answer(9, "Which one?", &OPTIONS, pick);
        assert_eq!(want, vec![answer_msg(9, pick as u8)], "the game's own key sends the dialog id and the choice");
        let mut p2 = ready(true);
        hold(VM, &[TakeoverKind::ResponseDialog]);
        dialog(&mut p2, 9, "Which one?", &OPTIONS);
        assert_eq!(p2.mode, 0);
        let id = session_of("response_dialog").unwrap().id;
        let got = action_broker::with_dispatch_for_test(VM, "mod", || {
            super::super::a2_play::dispatch_answer(&mut p2, &id, super::super::a2_play::Choice::Option(pick + 1))
        });
        assert!(matches!(got, action_broker::DispatchOutcome::Completed { .. }), "{got:?}");
        assert_eq!(sent(&mut p2), want, "option {}: the same bytes as the game's own key", pick + 1);
        assert_eq!(p2.mode, 0, "and the game's window is not left up");
        assert!(session_of("response_dialog").is_none(), "the session ended answered");
    }
}

#[test]
fn a2_a_stale_or_foreign_ref_is_refused_and_an_answer_is_not_replayed() {
    let (mut p, host) = rig();
    hold(vm_of(&host), &[TakeoverKind::ResponseDialog]);
    dialog(&mut p, 5, "Choose", &OPTIONS[..3]);
    ask(&mut p, &host, "local s = eqoa.windows.session('response_dialog'); old = s.ref; return eqoa.actions.dialog_answer(s.ref, 2)");
    host.execute("assert(results[1] and results[1].ok == true, tostring(last_err and last_err.code))").unwrap();
    assert!(session_of("response_dialog").is_none());
    // the same ref again: the dialog is gone
    ask(&mut p, &host, "return eqoa.actions.dialog_answer(old, 2)");
    host.execute("assert(last_err and last_err.code == 'stale_handle', tostring(last_err and last_err.code))").unwrap();
    // a ref that is no session at all
    ask(&mut p, &host, "return eqoa.actions.dialog_answer('ws-9999', 1)");
    host.execute("assert(last_err and last_err.code == 'stale_handle')").unwrap();
    // an option past the end
    dialog(&mut p, 6, "Again", &OPTIONS[..3]);
    ask(&mut p, &host, "local s = eqoa.windows.session('response_dialog'); return eqoa.actions.dialog_answer(s.ref, 4)");
    host.execute("assert(last_err and last_err.code == 'out_of_range', tostring(last_err and last_err.code))").unwrap();
    assert!(session_of("response_dialog").is_some(), "a refused answer leaves the dialog up");
}

#[test]
fn a2_a_second_answerable_dialog_is_refused_to_the_server_like_the_game_does() {
    let mut p = ready(true);
    hold(VM, &[TakeoverKind::ResponseDialog]);
    dialog(&mut p, 1, "First", &OPTIONS[..2]);
    dialog(&mut p, 2, "Second", &OPTIONS[..2]);
    assert_eq!(dialog_sessions::waiting_for_test(), (0, true, 0), "one waits");
    assert!(sent(&mut p).is_empty(), "nothing refused yet");
    dialog(&mut p, 3, "Third", &OPTIONS[..2]);
    // negative control: without the one waiting place the third would wait too and nothing would be sent
    assert_eq!(sent(&mut p), vec![answer_msg(3, 255)], "the third is refused as the game's own 0x0035 255");
    assert_eq!(session_of("response_dialog").unwrap().data["text"], "First");
    // when the first is answered the second starts
    let id = session_of("response_dialog").unwrap().id;
    action_broker::with_dispatch_for_test(VM, "mod", || {
        super::super::a2_play::dispatch_answer(&mut p, &id, super::super::a2_play::Choice::Option(1))
    });
    pump(&mut p);
    assert_eq!(session_of("response_dialog").unwrap().data["text"], "Second");
}

#[test]
fn a2_plain_text_queues_eleven_and_drops_the_rest() {
    let mut p = ready(false);
    hold(VM, &[TakeoverKind::TextDialog]);
    for i in 0..14 {
        text(&mut p, &format!("Words {i}"));
    }
    // one showing, eleven waiting, two dropped (negative control: a larger queue would hold 13)
    assert_eq!(dialog_sessions::waiting_for_test().0, 11);
    assert_eq!(session_of("text_dialog").unwrap().data["text"], "Words 0");
}

#[test]
fn a2_a_mod_that_goes_away_gives_every_dialog_back_to_the_games_window() {
    for how in ["release", "vm_gone", "all_ended"] {
        let mut p = ready(true);
        hold(VM, &[TakeoverKind::ResponseDialog, TakeoverKind::TextDialog]);
        dialog(&mut p, 5, "Choose", &OPTIONS[..3]);
        text(&mut p, "Behind it");
        assert_eq!(p.mode, 0);
        match how {
            "release" => assert!(window_takeover::release(VM, TakeoverKind::ResponseDialog)),
            "vm_gone" => {
                window_takeover::release_vm(VM);
                ws::end_vm(VM);
            }
            _ => ws::end_all("game_windows"),
        }
        pump(&mut p);
        // negative control: without the give-back the dialog is dropped and mode stays 0
        assert_eq!(p.mode, 7, "{how}: the dialog with answers is the game's window now");
        let (shown, _, _) = p.response.read_state();
        assert_eq!(utf(shown), "Choose", "{how}");
        if how == "vm_gone" {
            assert_eq!(p.dialogs.len(), 1, "{how}: the text waits in the game's own queue");
        }
        assert!(session_of("response_dialog").is_none());
    }
}

#[test]
fn a2_the_dialog_a_mod_stops_showing_goes_back_to_the_game_after_the_idle_time() {
    let mut p = ready(false);
    hold(VM, &[TakeoverKind::TextDialog]);
    text(&mut p, "Idle words");
    pump(&mut p);
    assert!(session_of("text_dialog").is_some());
    dialog_sessions::age_for_test(dialog_sessions::IDLE);
    pump(&mut p);
    assert_eq!(p.mode, 9, "the game shows it");
    assert!(session_of("text_dialog").is_none());
    // control: a mod that asks for its session each frame keeps it
    let mut p = ready(false);
    hold(VM, &[TakeoverKind::TextDialog]);
    text(&mut p, "Shown words");
    let id = session_of("text_dialog").unwrap().id;
    dialog_sessions::age_for_test(dialog_sessions::IDLE);
    dialog_sessions::touch(&id);
    pump(&mut p);
    assert!(session_of("text_dialog").is_some() && p.mode == 0);
}

#[test]
fn a2_walking_away_lets_the_dialog_go_as_the_games_escape_would() {
    let mut p = ready(true);
    hold(VM, &[TakeoverKind::ResponseDialog]);
    p.self_at = Some(([0.0, 0.0, 0.0], 0.0));
    dialog(&mut p, 8, "Choose", &OPTIONS[..3]);
    pump(&mut p);
    assert!(session_of("response_dialog").is_some());
    p.self_at = Some(([10.0, 0.0, 0.0], 0.0));
    pump(&mut p);
    assert!(session_of("response_dialog").is_some(), "control: 10 units is still beside the NPC");
    p.self_at = Some(([30.0, 0.0, 0.0], 0.0));
    pump(&mut p);
    // negative control: without the walk-away check the dialog stays and nothing is sent
    assert!(session_of("response_dialog").is_none());
    assert_eq!(sent(&mut p), vec![answer_msg(8, 255)], "answered no choice");
}

#[test]
fn a2_the_world_changing_drops_the_old_worlds_dialogs_and_sends_nothing() {
    let mut p = ready(true);
    hold(VM, &[TakeoverKind::ResponseDialog, TakeoverKind::TextDialog]);
    dialog(&mut p, 8, "Choose", &OPTIONS[..3]);
    text(&mut p, "Waiting");
    window_takeover::release_all();
    pump(&mut p);
    assert!(session_of("response_dialog").is_none());
    assert_eq!((p.mode, p.dialogs.len()), (0, 0), "nothing of the old world reaches the game's window");
    assert!(sent(&mut p).is_empty(), "and nothing is sent for it");
}

#[test]
fn a2_scope_own_is_refused_for_every_dialog_kind_and_all_needs_replaces() {
    let (_p, host) = rig();
    for kind in ["text_dialog", "response_dialog", "group_invite", "revive_offer", "summon_offer"] {
        host.execute(&format!(
            "local ok, err = eqoa.windows.takeover('{kind}', {{scope='own'}}); assert(ok == nil and err and err.code == 'invalid_argument', '{kind} own: ' .. tostring(err and err.code))"
        ))
        .unwrap();
        host.execute(&format!(
            "local ok, err = eqoa.windows.takeover('{kind}', {{scope='all'}}); assert(ok == nil and err and err.code == 'not_declared', '{kind} all: ' .. tostring(err and err.code))"
        ))
        .unwrap();
    }
}

#[test]
fn a2_a_second_mod_asking_for_the_dialog_is_told_busy() {
    hold(VM, &[TakeoverKind::ResponseDialog]);
    let err = window_takeover::takeover(VM + 1, "other", TakeoverKind::ResponseDialog, TakeoverScope::All).unwrap_err();
    assert_eq!(err.0, "busy");
    assert!(err.1.contains("mod"), "names the holder: {}", err.1);
}

#[test]
fn a2_a_group_invitation_is_a_session_accepted_and_declined_through_the_games_box() {
    for accept in [true, false] {
        let mut p = ready(true);
        hold(VM, &[TakeoverKind::GroupInvite]);
        deliver(&mut p, Event::GroupInvite { id: 3001, x: 0, name: "Ally".into() });
        assert_eq!(p.mode, 0, "the game's box is not up");
        assert!(p.group.invites.is_empty(), "parked out of the game's list so it does not ask again");
        let s = session_of("group_invite").expect("a session");
        assert_eq!(s.data["name"], "Ally");
        pump(&mut p);
        assert_eq!(p.mode, 0, "control: a step later the game has still not asked");
        let got = action_broker::with_dispatch_for_test(VM, "mod", || {
            super::super::a2_play::dispatch_answer(&mut p, &s.id, super::super::a2_play::Choice::Option(if accept { 1 } else { 2 }))
        });
        assert!(matches!(got, action_broker::DispatchOutcome::Completed { .. }), "{got:?}");
        let out = sent(&mut p);
        let mut w = Writer::op(if accept { 0x0622 } else { 0x0624 });
        w.u32(3001);
        if !accept {
            w.u8(1);
        }
        assert!(out.contains(&w.data), "accept={accept}: the game's own answer went out: {out:?}");
        assert_eq!(p.group.in_group, accept);
    }
}

#[test]
fn a2_a_resurrection_is_accepted_only_through_the_players_own_yes() {
    let (mut p, host) = rig_with(true);
    p.death = Some(crate::death::DeathCam::new([0.; 3], [0., 0., 1.], [0.; 3]));
    let vm = vm_of(&host);
    hold(vm, &[TakeoverKind::ReviveOffer, TakeoverKind::SummonOffer]);
    deliver(&mut p, Event::SummonPrompt { name: "Healer".into(), kind: 1 });
    assert_eq!(p.mode, 0, "the game's box is not up");
    let s = session_of("revive_offer").expect("a resurrection offer is its own kind, not 'other'");
    assert_eq!(s.data["from"], "Healer");
    // Yes through the mod: asked, nothing sent
    ask(&mut p, &host, "local s = eqoa.windows.session('revive_offer'); return eqoa.actions.dialog_answer(s.ref, 1)");
    host.execute("assert(results[1] and results[1].ok == true and results[1].detail.asked == true, tostring(last_err and last_err.code))").unwrap();
    // negative control: a mod's Yes that sent at once would have put 0x00D4 here
    assert!(sent(&mut p).is_empty(), "a mod's Yes sends nothing");
    assert!(confirm::is_open(), "the client's own question is up, even for a dead character");
    // the player's own No: nothing sent either, the offer stays
    key(NamedKey::Enter);
    pump(&mut p);
    assert!(sent(&mut p).is_empty() && session_of("revive_offer").is_some());
    // asked again, the player's Yes sends the game's own answer
    ask(&mut p, &host, "local s = eqoa.windows.session('revive_offer'); return eqoa.actions.dialog_answer(s.ref, 1)");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    pump(&mut p);
    let mut yes = Writer::op(0x00D4);
    yes.u8(1);
    assert_eq!(sent(&mut p), vec![yes.data], "0x00D4 with 1, the game's own");
    assert!(session_of("revive_offer").is_none());
}

#[test]
fn a2_declining_an_offer_is_always_allowed_and_sent_at_once() {
    let (mut p, host) = rig_with(true);
    let vm = vm_of(&host);
    hold(vm, &[TakeoverKind::SummonOffer]);
    deliver(&mut p, Event::SummonPrompt { name: "Mage".into(), kind: 0 });
    let s = session_of("summon_offer").unwrap();
    let got = action_broker::with_dispatch_for_test(vm, "mod", || {
        super::super::a2_play::dispatch_answer(&mut p, &s.id, super::super::a2_play::Choice::Option(2))
    });
    assert!(matches!(got, action_broker::DispatchOutcome::Completed { .. }), "{got:?}");
    let mut no = Writer::op(0x00D4);
    no.u8(0);
    assert_eq!(sent(&mut p), vec![no.data]);
    assert_eq!(p.mode, 0);
}

#[test]
fn a2_an_offer_over_another_window_leaves_that_window_as_it_was() {
    let mut p = ready(false);
    hold(VM, &[TakeoverKind::SummonOffer]);
    p.mode = 10;
    deliver(&mut p, Event::SummonPrompt { name: "Mage".into(), kind: 0 });
    assert_eq!(p.mode, 10, "the merchant window is not replaced by the game's box (negative control: mode 14)");
    assert!(session_of("summon_offer").is_some());
    // a prompt that was already up before the packet stays the game's
    let mut p = ready(false);
    p.mode = 14;
    hold(VM, &[TakeoverKind::SummonOffer]);
    deliver(&mut p, Event::SummonPrompt { name: "Mage".into(), kind: 0 });
    assert_eq!(p.mode, 14);
    assert!(session_of("summon_offer").is_none());
}

#[test]
fn a2_close_window_on_a_dialog_session_is_the_games_no_choice() {
    let (mut p, host) = rig_with(true);
    hold(vm_of(&host), &[TakeoverKind::ResponseDialog, TakeoverKind::TextDialog]);
    dialog(&mut p, 12, "Choose", &OPTIONS[..3]);
    ask(&mut p, &host, "return eqoa.actions.close_window(eqoa.windows.session('response_dialog').ref)");
    host.execute("assert(results[1] and results[1].ok == true, tostring(last_err and last_err.code))").unwrap();
    assert_eq!(sent(&mut p), vec![answer_msg(12, 255)]);
    text(&mut p, "Done?");
    ask(&mut p, &host, "return eqoa.actions.dialog_answer(eqoa.windows.session('text_dialog').ref, 1)");
    host.execute("assert(results[1] and results[1].ok == true, tostring(last_err and last_err.code))").unwrap();
    assert!(sent(&mut p).is_empty(), "the game's Done sends nothing");
    assert!(session_of("text_dialog").is_none());
}

#[test]
fn a2_an_option_on_a_later_page_is_reached_with_the_games_more_entry() {
    let mut p = ready(true);
    hold(VM, &[TakeoverKind::ResponseDialog]);
    let many: Vec<Vec<u16>> = (1..=14).map(|i| wide(&format!("Choice {i}"))).collect();
    dialog_sessions::hold(
        dialog_sessions::Item::Box { id: 21, text: wide("Many"), options: many, talked_to: None },
        None,
    );
    assert!(dialog_sessions::start_next(true, None).is_some());
    let id = session_of("response_dialog").unwrap().id;
    assert_eq!(session_of("response_dialog").unwrap().data["options"].as_array().unwrap().len(), 14);
    let got = action_broker::with_dispatch_for_test(VM, "mod", || {
        super::super::a2_play::dispatch_answer(&mut p, &id, super::super::a2_play::Choice::Option(14))
    });
    assert!(matches!(got, action_broker::DispatchOutcome::Completed { .. }), "{got:?}");
    assert_eq!(sent(&mut p), vec![answer_msg(21, 13)], "choice 13 (the fourteenth) through More");
}

#[test]
fn a2_a_dialog_session_that_the_player_cannot_answer_now_is_busy_not_lost() {
    let mut p = ready(true);
    hold(VM, &[TakeoverKind::TextDialog]);
    text(&mut p, "Words");
    let id = session_of("text_dialog").unwrap().id;
    p.mode = 13;
    let got = action_broker::with_dispatch_for_test(VM, "mod", || {
        super::super::a2_play::dispatch_answer(&mut p, &id, super::super::a2_play::Choice::Option(1))
    });
    assert!(matches!(got, action_broker::DispatchOutcome::Refused { code: "busy", .. }), "{got:?}");
    assert!(session_of("text_dialog").is_some());
}

#[test]
fn a2_an_invitation_withdrawn_while_parked_ends_its_session() {
    let mut p = ready(false);
    hold(VM, &[TakeoverKind::GroupInvite]);
    deliver(&mut p, Event::GroupInvite { id: 3002, x: 0, name: "Ally".into() });
    assert!(session_of("group_invite").is_some());
    deliver(&mut p, Event::GroupInviteGone(3002));
    assert!(session_of("group_invite").is_none());
    assert_eq!((p.mode, p.group.invites.len()), (0, 0), "nothing is left to ask about");
}
