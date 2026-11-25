// LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE).
//! Integration review P2 (2026-11-25): one situation, one error code, across the rounds. Each test pins one row of
//! the spec's section 1 table ("Which code for which situation") in at least two rounds, so a round that answers the
//! same situation with another code fails here.
//!
//! The negative control of each test is the code before the fix: A8 to A11 answered "the window is not open" with
//! `not_ready`, a game window in the way with `busy`, a missing server feature with `not_ready` (auction) or
//! `unavailable` (group search), and the ninth window with `busy`; and they took an item id as a Lua number. Each
//! assertion below names the code the table now fixes, and fails on the old one. The 0.7.0 rows quoted (round 2's
//! `abilities.find`, round 6's `command`, round 2's subscriptions) were already so and are the fixed points.
use super::*;
use crate::fork_ext::addons::{
    action_broker::{self, DispatchOutcome},
    confirm,
    frame_actions::FrameAction,
    host::Host,
    native_a6::A6Action,
    native_a7::A7Action,
    native_a8::A8Action,
    native_a9::A9Action,
    native_a10::A10Action,
    native_a11::A11Action,
    native_core::{self, CORE, Core},
    window_sessions as ws, window_takeover,
};

const VM: u64 = 0xC0DE;

fn fresh() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    confirm::reset_for_test();
    crate::fork_ext::addons::dialog_sessions::reset_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    super::a9_play::reset_for_test();
    super::a11_play::reset_for_test();
    super::a8_play::dropped();
    super::a10_play::dropped();
    CORE.with(|c| c.borrow_mut().view.in_world = true);
}

fn ready() -> Play {
    fresh();
    let mut p = super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p.session.record = Some(Vec::new());
    p
}

/// The refusal's code, or "(not refused)".
fn code(out: DispatchOutcome) -> &'static str {
    match out {
        DispatchOutcome::Refused { code, .. } => code,
        _ => "(not refused)",
    }
}

/// Runs `f` as this test's mod would be dispatched (the caller the rounds check a window session against).
fn as_vm<R>(f: impl FnOnce() -> R) -> R {
    action_broker::with_dispatch_for_test(VM, "codes_test", f)
}

/// Row "the window or session the action needs is not open": `window_closed` (0.7.0's loot, buy, sell, bank_move).
#[test]
fn codes_window_not_open_is_window_closed_in_every_round() {
    let mut p = ready();
    assert_eq!(code(super::a8_play::dispatch(&mut p, &A8Action::List { skill: 1 })), "window_closed", "A8: no crafting window");
    assert_eq!(code(super::a9_play::dispatch(&mut p, &A9Action::TradeAnswer { accept: true })), "window_closed", "A9: no request held");
    assert_eq!(
        code(super::a9_play::dispatch(&mut p, &A9Action::BlacksmithRepair { slot: 0, id: 1, key: None })),
        "window_closed",
        "A9: no smith open"
    );
    assert_eq!(code(super::a10_play::dispatch(&mut p, &A10Action::MailRefresh)), "window_closed", "A10: no mail window");
    assert_eq!(code(super::a11_play::dispatch(&mut p, &A11Action::FishCast)), "window_closed", "A11: no fishing window");
    // the control: with the fishing window up the same call is carried out
    assert!(p.fishing_opcode(crate::fishing::OP_IN, &[1]));
    p.fish.part.state = crate::fishing::state::WAITING;
    assert!(matches!(super::a11_play::dispatch(&mut p, &A11Action::FishCast), DispatchOutcome::Completed { .. }));
}

/// Row "the server does not offer this feature": `unsupported` (0.7.0's `weapon_set` and `open_window("search")`).
#[test]
fn codes_server_without_the_feature_is_unsupported() {
    let mut p = ready();
    assert!(!p.session.jumps && !p.session.merc_windows, "the fixture's server offers neither");
    assert_eq!(code(super::api_capture::a3::dispatch(&mut p, &FrameAction::Jump)), "unsupported", "A3: no jumping");
    assert_eq!(code(super::a7_play::dispatch(&mut p, &A7Action::LfgRegister)), "unsupported", "A7: no group search");
    assert_eq!(code(super::a11_play::dispatch(&mut p, &A11Action::MercEquip { id: 9 })), "unsupported", "A11: no mercenary windows");
    // A10: the bank offers no auction (the fixture's bank flag is off and it is not the live server)
    ws::open(VM, "codes_test", "auction_search", "mod").unwrap();
    assert_eq!(code(as_vm(|| super::a10_play::dispatch(&mut p, &A10Action::AuctionMyLots))), "unsupported", "A10: no auction");
    // the control: where the server offers the mercenary windows the refusal is another one (nothing sent yet)
    p.session.merc_windows = true;
    assert_ne!(code(super::a11_play::dispatch(&mut p, &A11Action::MercEquip { id: 9 })), "unsupported");
}

/// Row "a game window or box is in the way": `not_ready` (section 1's `not_ready`, "a window is up", from 0.7.0).
#[test]
fn codes_a_game_window_in_the_way_is_not_ready() {
    let mut p = ready();
    ws::open(VM, "codes_test", "recipe_book", "mod").unwrap();
    ws::open(VM, "codes_test", "auction_search", "mod").unwrap();
    p.mode = 37; // the game's own experiment
    assert_eq!(code(as_vm(|| super::a8_play::dispatch(&mut p, &A8Action::List { skill: 1 }))), "not_ready", "A8");
    p.mode = 39; // the game's own auction search
    assert_eq!(code(as_vm(|| super::a10_play::dispatch(&mut p, &A10Action::AuctionMyLots))), "not_ready", "A10");
    // the control: with the game's window gone the A10 call gets past that check (to the auction this server lacks)
    p.mode = 0;
    assert_eq!(code(as_vm(|| super::a10_play::dispatch(&mut p, &A10Action::AuctionMyLots))), "unsupported");
}

/// Row "a bad argument": `invalid_argument`. Here the integration review's P1: an item or ability id is the `Id` string
/// the reads give, so a Lua number is a wrong type in every round, as it is for round 2's `abilities.find` (0.7.0).
#[test]
fn codes_bad_argument_a_number_for_an_id_is_invalid_argument() {
    fresh();
    let h = Host::with_test_capabilities(&["eqoa.actions.*", "eqoa.abilities.find"]).unwrap();
    for call in [
        "eqoa.abilities.find(1234)",
        "eqoa.actions.craft_tray_add({slot = 1, id = 900001})",
        "eqoa.actions.trade_add({slot = 1, id = 10})",
        "eqoa.actions.blacksmith_repair({slot = 1, id = 20})",
        "eqoa.actions.blacksmith_repair({inventory_key = 1, id = 20})",
        "eqoa.actions.mail_send({to = 'Bob', subject = 's', body = 'b', item = {slot = 1, id = 9, count = 1}})",
        "eqoa.actions.auction_sell({slot = 1, id = 9, price = 5, buyout = 9, hours = 12})",
        "eqoa.actions.merc_spell_set(1, 950)",
        "eqoa.actions.craft_tray_add({slot = 1, id = '9e3'})",
        "eqoa.actions.trade_add({slot = 1, id = '010'})",
    ] {
        h.execute(&format!(
            "local r, e = {call}; assert(r == nil and e ~= nil and e.code == 'invalid_argument', [=[{call}: ]=] .. tostring(e and e.code))"
        ))
        .unwrap();
    }
    assert!(action_broker::queued_kinds_for_test().is_empty(), "nothing was queued by a bad call");
    // the controls: the same calls with the ids as the reads give them are queued
    h.execute(
        r#"
        assert(eqoa.actions.craft_tray_add({slot = 1, id = '900001'})); assert(eqoa.actions.trade_add({slot = 1, id = '10'}))
        assert(eqoa.actions.blacksmith_repair({slot = 1, id = '20'})); assert(eqoa.actions.blacksmith_repair({inventory_key = 1, id = '20'}))
        assert(eqoa.actions.blacksmith_repair({inventory_key = 1}))
        assert(eqoa.actions.mail_send({to = 'Bob', subject = 's', body = 'b', item = {slot = 1, id = '9', count = 1}}))
        assert(eqoa.actions.auction_sell({slot = 1, id = '9', price = 5, buyout = 9, hours = 12}))
        assert(eqoa.actions.merc_spell_set(1, '950')); assert(eqoa.actions.merc_spell_set(2, '0'))
        "#,
    )
    .unwrap();
    assert_eq!(action_broker::queued_kinds_for_test().len(), 9);
}

/// Row "the client never lets a mod do this": `permission_denied`, in round 6 (0.7.0) and round A6 (a report route),
/// and for a capability the mod lacks (A6's rule 4).
#[test]
fn codes_policy_refusal_is_permission_denied() {
    let mut p = ready();
    let s = ws::open_client(VM, "codes_test", "start_menu", "key", serde_json::json!({"entries": []}));
    let menu = |command: &str| A6Action::StartMenuCommand { vm: VM, session: s.id.clone(), command: command.into() };
    assert_eq!(code(super::a6_play::dispatch(&mut p, &menu("0xb0b1000:0"))), "permission_denied", "A6: a report route in the Start menu");
    // the control: a command the menu does not have is the "not offered" row, not this one
    assert_eq!(code(super::a6_play::dispatch(&mut p, &menu("0xdead:0"))), "unsupported");
    let h = Host::with_test_capabilities(&["eqoa.actions.command"]).unwrap();
    h.execute("local r, e = eqoa.actions.command('/bug it broke'); assert(r == nil and e.code == 'permission_denied', tostring(e and e.code))")
        .unwrap();
    h.execute("local r, e = eqoa.actions.chat_line('say', 'hello'); assert(r == nil and e.code == 'permission_denied', tostring(e and e.code))")
        .unwrap();
    assert!(action_broker::queued_kinds_for_test().is_empty());
    // the control: an ordinary command is queued
    h.execute("assert(eqoa.actions.command('/who'))").unwrap();
    assert_eq!(action_broker::queued_kinds_for_test().len(), 1);
}

/// Row "a mod holds as many as it may": `quota`, for round A1's windows as for round 2's subscriptions (0.7.0).
#[test]
fn codes_too_many_windows_is_quota() {
    fresh();
    for kind in ["inventory", "spellbook", "quest_log", "trainer", "mastery", "buddy_list", "ignore_list", "guild"] {
        ws::open(VM, "codes_test", kind, "mod").unwrap();
    }
    assert_eq!(ws::open(VM, "codes_test", "who", "mod").unwrap_err().0, "quota", "the ninth window");
    // the control: an open kind again is the same session, not a ninth
    assert!(ws::open(VM, "codes_test", "inventory", "mod").is_ok());
    let h = Host::with_test_capabilities(&["eqoa.events.on"]).unwrap();
    h.execute(
        "for i = 1, 128 do assert(eqoa.events.on('action.result', function() end)) end
         local t, e = eqoa.events.on('action.result', function() end); assert(t == nil and e.code == 'quota', tostring(e and e.code))",
    )
    .unwrap();
}

/// Row "the handle, ref or row no longer names what it named": `stale_handle`, in round A1 and round A10.
#[test]
fn codes_a_gone_handle_is_stale_handle() {
    let mut p = ready();
    assert_eq!(ws::pass(VM, "ws-999").unwrap_err().0, "stale_handle", "A1: a session ref that is not open");
    assert_eq!(ws::focus(VM, "ws-999", true).unwrap_err().0, "stale_handle");
    ws::open(VM, "codes_test", "mail_inbox", "mod").unwrap();
    assert_eq!(code(as_vm(|| super::a10_play::dispatch(&mut p, &A10Action::MailRead { id: 99 }))), "stale_handle", "A10: no such mail");
}

/// Row "something else holds what is needed for now": `busy`, for A1's one question (every asks-first action) and A11.
#[test]
fn codes_held_for_now_is_busy() {
    let mut p = ready();
    let act = || confirm::Act::Destroy { inv_index: 1, name: "Rock".into(), count: 1 };
    assert!(confirm::open(1, "a", None, act(), "x".into(), 0).is_ok());
    assert_eq!(confirm::open(2, "b", None, act(), "y".into(), 0).unwrap_err().0, "busy", "A1: a second question");
    confirm::reset_for_test();
    assert!(p.fishing_opcode(crate::fishing::OP_IN, &[1]));
    p.fish.part.state = crate::fishing::state::WAITING;
    assert!(matches!(super::a11_play::dispatch(&mut p, &A11Action::FishCast), DispatchOutcome::Completed { .. }), "the control");
    p.fish.wnd.cast(0); // the window's own meter starts (its next frame), as in the A11 test
    assert_eq!(code(super::a11_play::dispatch(&mut p, &A11Action::FishCast)), "busy", "A11: a cast under way");
}
