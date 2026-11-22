// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1 from Play-shaped state: the question that `drop_quest` and `destroy` always ask, and what the player's
//! Yes then really sends; the item actions Bags needs (`link_item`, `hide_look`); the game's own window opening
//! again when a mod passes a corpse, merchant or bank back; the per-step work (`a1_play::tick`).
//!
//! Each test says what its negative control is: the behaviour without the change would fail the assertion named.
use super::*;
use crate::fork_ext::addons::{
    action_broker, confirm,
    host::Host,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws,
    window_takeover::{self, TakeoverKind, TakeoverScope},
};
use net::stream::{Reader, Writer};
use winit::keyboard::{Key, NamedKey};

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
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

/// An item as the server sends it, read back with the client's own reader.
fn item(id: i64, name: &str, key: u32, quantity: i64, equip_slot: i64, quick_slot: i64) -> net::dump::NetItem {
    let mut w = Writer::default();
    w.signed(quantity).signed(0).signed(0).signed(equip_slot).signed(quick_slot).u32(key);
    for v in [id, 10, 0, 4001, 0, -1, 0, 0, 1, 0, 0, 0, 0, 1, 20, 0, 0, -1, -1, 0, 0, 0, 0] {
        w.signed(v);
    }
    w.wide(name).wide("a thing").signed(0);
    net::dump::read_net_item(&mut Reader::new(&w.data))
}

fn rig() -> (Play, Host) {
    let mut p = ready();
    p.dump.inventory = net::dump::ItemList {
        p04: 0,
        items: vec![item(900_001, "Rusty Sword", 11, 1, 0, -1), item(900_002, "Iron Ore", 12, 5, -1, -1)],
    };
    p.quests.titles = ["Bat Eyes", "The Militia"].iter().map(|t| crate::ui::wide(t)).collect();
    p.quests.stamp = 7;
    p.forge_snapshot(None);
    r6a::refresh(&p, None);
    let host = Host::with_test_capabilities(&[
        "eqoa.actions.*",
        "eqoa.windows.session",
        "eqoa.windows.focus",
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

fn press_9(host: &Host) {
    CORE.with(|c| {
        c.borrow_mut().emit("input.key", serde_json::json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}))
    });
    native_events::dispatch(host).unwrap();
}

/// The mod asks (in a player gesture, as a key press), the game steps, the mod hears.
fn ask(p: &mut Play, host: &Host, code: &str) {
    r6a::refresh(p, None);
    host.execute(&format!("results = {{}}; closed = {{}}; last_err = nil; last_req = nil; next_fn = function() {code} end")).unwrap();
    press_9(host);
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn step(p: &mut Play, host: &Host) {
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn key(k: NamedKey) {
    assert!(confirm::key(&Key::Named(k)));
}

#[test]
fn a1_drop_quest_asks_first_and_only_the_players_yes_sends_it() {
    let (mut p, host) = rig();
    let sent = p.session.sends_lost;
    ask(&mut p, &host, "return eqoa.actions.drop_quest(eqoa.quests.list()[2].ref)");
    host.execute("assert(results[1] ~= nil and results[1].ok == true and results[1].detail.asked == true, tostring(last_err and last_err.code))").unwrap();
    // negative control: the old dispatch sent 0x007D here and this fails
    assert_eq!(p.session.sends_lost, sent, "nothing is dropped by the call");
    let q = confirm::current_for_test().expect("the question is up");
    assert!(!q.yes && q.text.contains("The Militia"), "starts on No and names the quest: {}", q.text);
    // the mod can see that its question is up, and read it, but has no way to answer it
    host.execute("local c = eqoa.windows.confirm(); assert(c.kind == 'drop_quest' and c.text:find('The Militia'))").unwrap();
    host.execute(
        r#"for _, name in ipairs({'answer', 'confirm_yes', 'accept', 'answer_confirm', 'press', 'click'}) do
            assert(eqoa.windows[name] == nil and eqoa.actions[name] == nil, name .. ' must not exist')
        end"#,
    )
    .unwrap();
    // Enter on the starting choice is No
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent);
    host.execute("assert(closed[1].kind == 'drop_quest' and closed[1].answer == 'no')").unwrap();
    // asked again: Right, then Enter is Yes, and the step after it sends the game's own removal
    ask(&mut p, &host, "return eqoa.actions.drop_quest(eqoa.quests.list()[2].ref)");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    assert_eq!(p.session.sends_lost, sent, "not before the next step");
    step(&mut p, &host);
    assert!(p.session.sends_lost > sent, "0x007D went out through the session");
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == true)").unwrap();
}

#[test]
fn a1_drop_quest_is_dropped_only_if_the_quest_is_still_the_one_asked_about() {
    let (mut p, host) = rig();
    let sent = p.session.sends_lost;
    ask(&mut p, &host, "return eqoa.actions.drop_quest(eqoa.quests.list()[2].ref)");
    // the journal changes while the question is up: a Yes must not drop whatever is now in that place
    p.quests.titles[1] = crate::ui::wide("Something Else");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent, "the quest is not the one the player was asked about");
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == false)").unwrap();
    // a stale handle never even gets to ask
    r6a::refresh(&p, None);
    host.execute("old = eqoa.quests.list()[1].ref").unwrap();
    p.quests.titles = vec![crate::ui::wide("The Militia")];
    ask(&mut p, &host, "return eqoa.actions.drop_quest(old)");
    host.execute("assert((last_err and last_err.code) or results[1].code == 'stale_handle')").unwrap();
    assert!(!confirm::is_open());
}

#[test]
fn a1_destroy_asks_first_names_the_item_and_only_the_players_yes_sends_it() {
    let (mut p, host) = rig();
    let sent = p.session.sends_lost;
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref)");
    host.execute("assert(results[1].ok == true and results[1].detail.asked == true, tostring(last_err and last_err.message))").unwrap();
    assert_eq!(p.session.sends_lost, sent, "asking destroys nothing");
    assert!(!p.waiting);
    let q = confirm::current_for_test().unwrap();
    assert!(q.text.contains("5 x \"Iron Ore\"") && !q.yes, "names the stack and starts on No: {}", q.text);
    // No: nothing is sent, the mod hears no
    key(NamedKey::Escape);
    step(&mut p, &host);
    assert_eq!((p.session.sends_lost, p.waiting), (sent, false));
    host.execute("assert(closed[1].kind == 'destroy' and closed[1].answer == 'no')").unwrap();
    // Yes, for two of five: 0x003C goes out and the game waits for the server like its own destroy does
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref, 2)");
    assert!(confirm::current_for_test().unwrap().text.contains("2 x \"Iron Ore\""));
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert!(p.session.sends_lost > sent, "the destroy message went out");
    assert!(p.waiting, "and the game waits for the server's answer");
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == true)").unwrap();
}

#[test]
fn a1_destroy_refuses_what_cannot_be_asked_and_a_yes_for_a_changed_slot() {
    let (mut p, host) = rig();
    let sent = p.session.sends_lost;
    // more than the stack holds
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref, 6)");
    host.execute("assert(results[1].ok == false and results[1].code == 'out_of_range', tostring(last_err and last_err.code))").unwrap();
    // a count that is not a whole number
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref, 0)");
    host.execute("assert(last_err ~= nil and last_err.code == 'invalid_argument')").unwrap();
    // the slot holds a different item by the time of the Yes
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref)");
    p.dump.inventory.items[1] = item(900_009, "Something Else", 12, 5, -1, -1);
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent, "never destroys what the player was not asked about");
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == false)").unwrap();
    // dead: nothing is asked
    p.dump.inventory.items[1] = item(900_002, "Iron Ore", 12, 5, -1, -1);
    p.forge_snapshot(None);
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref)");
    assert!(confirm::is_open());
    key(NamedKey::Escape);
}

#[test]
fn a1_a_worn_item_says_so_in_the_question() {
    let (mut p, host) = rig();
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[1].ref)");
    let q = confirm::current_for_test().unwrap();
    assert!(q.text.contains("Rusty Sword") && q.text.contains("It is worn."), "{}", q.text);
    key(NamedKey::Escape);
}

#[test]
fn a1_link_item_and_hide_look_use_the_games_own_item_menu_steps() {
    let (mut p, host) = rig();
    // hide look on a worn item: the game's own message
    let sent = p.session.sends_lost;
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.hide_look(inv.items[1].ref, true)");
    host.execute("assert(results[1].ok == true, tostring(last_err and last_err.message) .. tostring(results[1] and results[1].reason))").unwrap();
    assert!(p.session.sends_lost > sent, "hide look went out");
    // not worn: refused, nothing sent (the control: the worn one above was sent)
    let sent = p.session.sends_lost;
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.hide_look(inv.items[2].ref, true)");
    host.execute("assert(results[1].ok == false and results[1].code == 'not_ready')").unwrap();
    assert_eq!(p.session.sends_lost, sent);
    // link item puts the item's link in the chat line, as the item menu's Link Item does
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.link_item(inv.items[2].ref)");
    host.execute("assert(results[1].ok == true, tostring(results[1] and results[1].reason))").unwrap();
}

#[test]
fn a1_the_question_ends_as_no_on_death_zoning_and_leaving_the_world() {
    for (what, end) in [
        ("death", Box::new(|p: &mut Play| p.death = Some(crate::death::DeathCam::new([0.; 3], [0., 0., 1.], [0.; 3]))) as Box<dyn Fn(&mut Play)>),
        ("left_world", Box::new(|p: &mut Play| { CORE.with(|c| c.borrow_mut().view.in_world = false); p.session.conn = None; })),
    ] {
        let (mut p, host) = rig();
        ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref)");
        assert!(confirm::is_open(), "{what}: it was up");
        end(&mut p);
        step(&mut p, &host);
        assert!(!confirm::is_open(), "{what}: ended");
        host.execute("assert(closed[1].answer ~= 'yes')").unwrap();
    }
    let (mut p, host) = rig();
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref)");
    window_takeover::release_all();
    step(&mut p, &host);
    assert!(!confirm::is_open());
    host.execute("assert(closed[1].answer == 'zoning')").unwrap();
    // and a Yes the player gave a moment before the world changed is not carried into the next one
    let (mut p, host) = rig();
    let sent = p.session.sends_lost;
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref)");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    window_takeover::release_all();
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent);
}

#[test]
fn a1_a_session_tick_tells_the_registry_what_play_is_doing() {
    let (mut p, host) = rig();
    host.execute("s = assert(eqoa.windows.open_session('spellbook'))").unwrap();
    step(&mut p, &host);
    host.execute("assert(eqoa.windows.session('spellbook') ~= nil, 'plain play keeps the window')").unwrap();
    // a game window comes up over play: the window stays (it is not a death or a zone), the controller is the game's
    p.mode = 13;
    step(&mut p, &host);
    assert!(!ws::env().plain);
    host.execute("assert(eqoa.windows.session('spellbook') ~= nil)").unwrap();
    p.mode = 0;
    // death ends it, told with the reason
    p.death = Some(crate::death::DeathCam::new([0.; 3], [0., 0., 1.], [0.; 3]));
    step(&mut p, &host);
    host.execute("assert(eqoa.windows.session('spellbook') == nil and ev[#ev] == 'closed:spellbook:death', ev[#ev])").unwrap();
}

#[test]
fn a1_a_held_corpse_is_a_session_and_pass_opens_the_games_own_window() {
    let (mut p, host) = rig();
    let vm = native_events::bus(&host.lua).borrow().vm;
    assert!(window_takeover::takeover(vm, "mod", TakeoverKind::Loot, TakeoverScope::Own).is_ok());
    assert!(window_takeover::activate_hidden_for_test(vm, TakeoverKind::Loot, 7));
    p.npcw.corpse = Some(net::dump::ItemList { p04: 0, items: vec![item(1, "Bone", 5, 1, -1, -1)] });
    p.mode = 0;
    p.npcw.loot_open = false;
    step(&mut p, &host);
    host.execute("held = assert(eqoa.windows.session('loot')); assert(held.origin == 'server')").unwrap();
    assert_eq!(p.mode, 0, "hidden: the game's window is not up");
    host.execute("assert(eqoa.windows.pass(held.ref))").unwrap();
    step(&mut p, &host);
    assert_eq!(p.mode, 1, "the game's own corpse window opens");
    assert!(p.npcw.loot_open && p.npcw.corpse.is_some(), "with the list the server sent");
    assert!(!window_takeover::is_hidden_active(TakeoverKind::Loot));
    assert!(window_takeover::is_taken(TakeoverKind::Loot), "the claim stays for the next opening");
    host.execute("assert(eqoa.windows.session('loot') == nil and ev[#ev] == 'closed:loot:passed', ev[#ev])").unwrap();
    // control: with no pass the corpse stays hidden (the first half of this test)
    let (mut p2, host2) = rig();
    let vm2 = native_events::bus(&host2.lua).borrow().vm;
    window_takeover::takeover(vm2, "mod", TakeoverKind::Merchant, TakeoverScope::Own).unwrap();
    window_takeover::activate_hidden_for_test(vm2, TakeoverKind::Merchant, 9);
    step(&mut p2, &host2);
    step(&mut p2, &host2);
    assert_eq!(p2.mode, 0);
}

#[test]
fn a1_pass_waits_for_plain_play_and_is_refused_over_a_game_window() {
    let (mut p, host) = rig();
    let vm = native_events::bus(&host.lua).borrow().vm;
    window_takeover::takeover(vm, "mod", TakeoverKind::Bank, TakeoverScope::Own).unwrap();
    window_takeover::activate_hidden_for_test(vm, TakeoverKind::Bank, 3);
    step(&mut p, &host);
    host.execute("held = assert(eqoa.windows.session('bank'))").unwrap();
    p.mode = 13;
    step(&mut p, &host);
    host.execute("local ok, e = eqoa.windows.pass(held.ref); assert(ok == nil and e.code == 'busy', e and e.code)").unwrap();
    p.mode = 0;
    step(&mut p, &host);
    host.execute("assert(eqoa.windows.pass(held.ref))").unwrap();
    step(&mut p, &host);
    assert_eq!(p.mode, 11);
    assert!(p.bank.open);
}
