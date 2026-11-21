// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 7H: the last corrections before the `eqoa/1` freeze, driven from a `Play` through the action broker
//! as a mod's key press would (items 1, 2, 5, 6 and 7 of the D2 list).
use super::*;
use crate::fork_ext::addons::{
    action_broker,
    host::Host,
    native_core::{self, CORE, Core},
    native_events,
};

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    r6a::forget();
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

fn rig() -> (Play, Host) {
    let p = ready();
    let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
    host.execute(
        r#"
        results = {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "u" }, function()
            last_req, last_err = next_fn()
        end)
        "#,
    )
    .unwrap();
    (p, host)
}

/// The player's key press that runs `next_fn` (a gesture), then one client iteration that dispatches.
fn gesture(p: &mut Play, host: &Host, code: &str) {
    r6a::refresh(p, None);
    host.execute(&format!("results = {{}}; last_err = nil; last_req = nil; next_fn = function() {code} end"))
        .unwrap();
    CORE.with(|c| {
        c.borrow_mut().emit(
            "input.key",
            serde_json::json!({"key":"u","down":true,"repeat":false,"synthetic":false,"text":""}),
        )
    });
    native_events::dispatch(host).unwrap();
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn done(host: &Host) {
    let _ = host;
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

/// Another player's character in view, the kind the game lets you follow.
fn other_player(p: &mut Play, id: u32) {
    p.entities.slots[1] = Some(net::entity::Entity {
        id,
        world: p.player.world as i8,
        kind: net::entity::TYPE_CREATURE,
        pos: [0.0, 0.0, -5.0],
        actor_pos: [0.0, 0.0, -5.0],
        p18: 1,
        p19: 255,
        creature: Some(net::entity::Creature { name: "Ally".into(), p256: 2, ..Default::default() }),
        ..Default::default()
    });
}

/// Item 1 (did not reproduce): `Windows.active` is the window that has the keyboard, the last one listed.
#[test]
fn round7h_1_windows_active_is_the_focused_window() {
    let (mut p, host) = rig();
    p.mode = 22; // the quest journal
    r6a::refresh(&p, None);
    host.execute(
        r#"
        local g = eqoa.windows.get()
        local last = g.windows[#g.windows]
        assert(last ~= nil and last.focused == true, 'no focused window')
        assert(g.active ~= nil and g.active == last.ref, 'Windows.active is ' .. tostring(g.active))
    "#,
    )
    .unwrap();
    p.mode = 0;
    r6a::refresh(&p, None);
    host.execute("local g = eqoa.windows.get(); assert(#g.windows == 0 and g.active == nil)").unwrap();
    done(&host);
}

/// Item 2: `follow(nil)` only stops a follow. With nothing followed it completes and starts nothing, even
/// with a followable target selected.
#[test]
fn round7h_2_follow_nil_only_stops_following() {
    let (mut p, host) = rig();
    other_player(&mut p, 700);
    p.target = 700;
    p.following = false;
    gesture(&mut p, &host, "return eqoa.actions.follow(nil)");
    host.execute("assert(last_err == nil and results[1] and results[1].state == 'completed', tostring(results[1] and results[1].reason))")
        .unwrap();
    assert!(!p.following, "follow(nil) with nothing followed started following the target");

    // following someone: follow(nil) stops it
    gesture(&mut p, &host, "return eqoa.actions.follow(700)");
    assert!(p.following && p.follow_id == 700, "the control: follow(700) follows");
    gesture(&mut p, &host, "return eqoa.actions.follow(nil)");
    host.execute("assert(results[1] and results[1].state == 'completed')").unwrap();
    assert!(!p.following, "follow(nil) stops the follow");
    done(&host);
}

/// Item 5: the game has no hostile or friendly cycling (upstream `Play::cycle_target` takes only "group or
/// not"), so those filters are refused `unsupported` instead of cycling as if no filter were given.
#[test]
fn round7h_5_cycle_target_refuses_filters_the_game_does_not_have() {
    let (mut p, host) = rig();
    for filter in ["hostile", "friendly"] {
        gesture(&mut p, &host, &format!("return eqoa.actions.cycle_target('{filter}')"));
        host.execute(&format!(
            "assert(last_req == nil and last_err and last_err.code == 'unsupported', '{filter}: ' .. tostring(last_err and last_err.code))"
        ))
        .unwrap();
    }
    for filter in ["nil", "'group'", "'corpse'"] {
        gesture(&mut p, &host, &format!("return eqoa.actions.cycle_target({filter})"));
        host.execute(&format!("assert(last_req ~= nil and last_err == nil, '{filter} was refused')")).unwrap();
    }
    done(&host);
}

/// Item 6: `ok` is nil until the outcome is known: a request sent and awaiting the server is `dispatched`
/// with no `ok`, in its event and in `status`; the server's answer makes it `completed` with `ok = true`.
#[test]
fn round7h_6_a_dispatched_result_has_no_ok_until_the_outcome_is_known() {
    let (mut p, host) = rig();
    p.abilities.insert(
        601,
        net::dump::Ability { id: 601, display_index: 3, valid: 1, slot: -1, name: "Heal".into(), ..Default::default() },
    );
    gesture(&mut p, &host, "return eqoa.actions.assign_ability(1, 601)");
    host.execute(
        r#"
        assert(last_req ~= nil, tostring(last_err and last_err.code))
        local r = results[#results]
        assert(r.state == 'dispatched' and r.phase == 'dispatch', r.state)
        assert(r.ok == nil, 'a dispatched result says ok = ' .. tostring(r.ok))
        local s = eqoa.actions.status(last_req)
        assert(s.state == 'dispatched' and s.ok == nil, 'status says ok = ' .. tostring(s.ok))
    "#,
    )
    .unwrap();
    let ev = net::session::Event::AbilitySlot { token: 1, index: 3, slot: 0 };
    let cap = super::super::begin_game_event(&p, &ev);
    super::super::end_game_event(&p, cap);
    native_events::dispatch(&host).unwrap();
    host.execute("local r = results[#results]; assert(r.state == 'completed' and r.ok == true, r.state)").unwrap();
    done(&host);
}

/// Item 7: the game asks Yes/No before dropping a quest (`questlog.rs`, word 574) and before buying or
/// refunding a Class Mastery skill (`classmastery.rs`, words 1663/1664 and 1665/1666). Until a mod-drawn
/// confirmation can be tied to the player's own answer, these three are `not_yet` for native mods and send
/// nothing. The game never asks before Unmemorize All (`spellbook.rs`, the menu's 0x0809_0000), so
/// `unmemorize_all` stays supported.
#[test]
fn round7h_7_steps_the_game_confirms_are_not_yet_for_mods() {
    let (mut p, host) = rig();
    p.quests.titles = ["Bat Eyes", "The Militia"].iter().map(|t| crate::ui::wide(t)).collect();
    p.quests.stamp = 7;
    p.mode = 22;
    p.cm.m.x08 = 50;
    p.cm.skills = vec![crate::classmastery::Skill { id: 100, x00: 1, known: 1, ..Default::default() }];
    for (name, call) in [
        ("drop_quest", "eqoa.actions.drop_quest(eqoa.quests.list()[2].ref)"),
        ("mastery_buy", "eqoa.actions.mastery_buy(100)"),
        ("mastery_refund", "eqoa.actions.mastery_refund(100)"),
    ] {
        let sent = p.session.sends_lost;
        gesture(&mut p, &host, &format!("return {call}"));
        host.execute(&format!(
            r#"
            assert(eqoa.capabilities.get('eqoa.actions.{name}').status == 'not_yet', '{name} is ' .. eqoa.capabilities.get('eqoa.actions.{name}').status)
            assert(last_req == nil and last_err and last_err.code == 'not_yet', '{name}: ' .. tostring(last_err and last_err.code))
            assert(last_err.message:find('confirm', 1, true), last_err.message)
        "#
        ))
        .unwrap();
        assert_eq!(p.session.sends_lost, sent, "{name} sent something");
    }
    assert_eq!(p.quests.titles.len(), 2);
    host.execute("assert(eqoa.capabilities.get('eqoa.actions.unmemorize_all').status == 'supported')").unwrap();
    done(&host);
}
