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

/// Three stacks for the group destroy: a worn sword, the iron ore and some rat pelts.
fn rig3() -> (Play, Host) {
    let (mut p, host) = rig();
    p.dump.inventory.items.push(item(900_003, "Rat Pelt", 13, 3, -1, -1));
    p.forge_snapshot(None);
    r6a::refresh(&p, None);
    (p, host)
}

const BOTH: &str = "local inv = eqoa.inventory.get(); return eqoa.actions.destroy_many({inv.items[2].ref, inv.items[3].ref})";

/// The server's answer to a destroy: the stack leaves the pack and the game stops waiting.
fn server_answers(p: &mut Play, name: &str) {
    p.dump.inventory.items.retain(|it| it.item.name != name);
    p.waiting = false;
}

#[test]
fn a1_destroy_many_asks_once_with_the_list_and_only_the_players_yes_sends_each_stack_in_turn() {
    let (mut p, host) = rig3();
    let sent = p.session.sends_lost;
    ask(&mut p, &host, BOTH);
    host.execute("assert(results[1].ok == true and results[1].detail.asked == true, tostring(last_err and last_err.message))").unwrap();
    // negative control: a client that acted on the call (or asked once per stack) fails here
    assert_eq!(p.session.sends_lost, sent, "asking destroys nothing");
    let q = confirm::current_for_test().expect("one question is up");
    assert!(!q.yes, "starts on No");
    assert!(q.text.starts_with("Destroy these 2 items?"), "{}", q.text);
    assert!(q.text.contains("5 x \"Iron Ore\"") && q.text.contains("3 x \"Rat Pelt\""), "names every stack: {}", q.text);
    assert!(!q.text.contains("Rusty Sword"), "only the stacks named: {}", q.text);
    host.execute("local c = eqoa.windows.confirm(); assert(c.kind == 'destroy_many' and c.text:find('Rat Pelt'))").unwrap();
    // No: nothing is sent, the mod hears no, and it hears it once
    key(NamedKey::Escape);
    step(&mut p, &host);
    assert_eq!((p.session.sends_lost, p.waiting), (sent, false), "no Yes, nothing sent");
    host.execute("assert(#closed == 1 and closed[1].kind == 'destroy_many' and closed[1].answer == 'no')").unwrap();
    assert!(!confirm::has_work());
    // Yes: the first stack goes out and the game waits for the server before the second
    ask(&mut p, &host, BOTH);
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    assert_eq!(p.session.sends_lost, sent, "not before the next step");
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent + 1, "only the first stack goes out");
    assert!(p.waiting);
    step(&mut p, &host);
    step(&mut p, &host);
    // negative control: a run that sent everything at once would be at sent + 2 here
    assert_eq!(p.session.sends_lost, sent + 1, "the second waits for the server's answer to the first");
    host.execute("assert(#closed == 0, 'the mod hears nothing until the run is over')").unwrap();
    server_answers(&mut p, "Iron Ore");
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent + 2, "the second stack goes out in its turn");
    server_answers(&mut p, "Rat Pelt");
    step(&mut p, &host);
    host.execute(
        "assert(closed[1].answer == 'yes' and closed[1].kind == 'destroy_many' and closed[1].detail.sent == true \
         and closed[1].detail.sent_count == 2 and closed[1].detail.total == 2 and #closed[1].detail.skipped == 0)",
    )
    .unwrap();
    assert!(!confirm::has_work(), "the run is over");
}

#[test]
fn a1_destroy_many_skips_a_stack_that_changed_and_stops_with_the_world() {
    let (mut p, host) = rig3();
    let sent = p.session.sends_lost;
    // the second stack is something else by its turn: skipped and counted, never destroyed
    ask(&mut p, &host, BOTH);
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent + 1);
    server_answers(&mut p, "Iron Ore");
    let rat = p.dump.inventory.items.iter().position(|it| it.item.name == "Rat Pelt").unwrap();
    p.dump.inventory.items[rat] = item(900_009, "Something Else", 13, 3, -1, -1);
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent + 1, "never destroys what the player was not asked about");
    host.execute("assert(closed[1].detail.sent_count == 1 and closed[1].detail.total == 2 and closed[1].detail.skipped[1] == 'Rat Pelt')").unwrap();
    // zoning in the middle ends the run: what is left is not destroyed, and the mod is told how far it got
    let (mut p, host) = rig3();
    let sent = p.session.sends_lost;
    ask(&mut p, &host, BOTH);
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent + 1);
    confirm::cancel_all("zoning");
    server_answers(&mut p, "Iron Ore");
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent + 1, "the rest is not carried into the next world");
    host.execute("assert(closed[1].answer == 'zoning' and closed[1].detail.sent_count == 1 and closed[1].detail.total == 2)").unwrap();
    assert!(!confirm::has_work());
}

#[test]
fn a1_destroy_many_never_takes_a_worn_stack_asked_for_or_put_on_during_the_run() {
    let (mut p, host) = rig3();
    let sent = p.session.sends_lost;
    // asked for: refused for every mod, no question is put to the player (the rig's first item is the worn sword)
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy_many({inv.items[2].ref, inv.items[1].ref})");
    host.execute("assert(results[1].ok == false and results[1].code == 'not_ready' and results[1].reason:find('worn'), tostring(results[1] and results[1].reason))").unwrap();
    assert!(!confirm::is_open(), "a list with a worn stack is not asked at all");
    assert_eq!(p.session.sends_lost, sent);
    // put on while the run goes: skipped at its turn and counted, never destroyed
    ask(&mut p, &host, BOTH);
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent + 1);
    server_answers(&mut p, "Iron Ore");
    let rat = p.dump.inventory.items.iter().position(|it| it.item.name == "Rat Pelt").unwrap();
    p.dump.inventory.items[rat].equip_slot = 3;
    step(&mut p, &host);
    assert_eq!(p.session.sends_lost, sent + 1, "the stack that was put on is not destroyed");
    host.execute("assert(closed[1].detail.sent_count == 1 and closed[1].detail.total == 2 and closed[1].detail.skipped[1] == 'Rat Pelt')").unwrap();
}

#[test]
fn a1_destroy_many_refuses_what_cannot_be_asked() {
    let (mut p, host) = rig3();
    ask(&mut p, &host, "return eqoa.actions.destroy_many({})");
    host.execute("assert(last_err ~= nil and last_err.code == 'invalid_argument')").unwrap();
    ask(&mut p, &host, "return eqoa.actions.destroy_many('x')");
    host.execute("assert(last_err ~= nil and last_err.code == 'invalid_argument')").unwrap();
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy_many({inv.items[2].ref, inv.items[2].ref})");
    host.execute("assert(last_err ~= nil and last_err.code == 'invalid_argument', 'the same item twice')").unwrap();
    // a gone item never gets as far as the question
    r6a::refresh(&p, None);
    host.execute("old = eqoa.inventory.get().items[3].ref").unwrap();
    p.dump.inventory.items.retain(|it| it.item.name != "Rat Pelt");
    p.forge_snapshot(None);
    r6a::refresh(&p, None);
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy_many({inv.items[2].ref, old})");
    host.execute("assert((last_err and last_err.code) == 'stale_handle' or (results[1] and results[1].code == 'stale_handle'))").unwrap();
    assert!(!confirm::is_open());
    // one question at a time, as destroy's
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy_many({inv.items[2].ref})");
    assert!(confirm::is_open());
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy_many({inv.items[2].ref})");
    host.execute("assert(results[1].ok == false and results[1].code == 'busy', tostring(last_err and last_err.code))").unwrap();
    key(NamedKey::Escape);
}

/// Bag Viewer multi-select: a click says which of Ctrl, Shift and Alt were held. Negative control: before the change the
/// fields are not there (`ctrl` is nil) and every assertion on them fails.
#[test]
fn pointer_pressed_and_wheel_carry_the_modifiers_held() {
    use crate::fork_ext::addons::{input, pointer_api};
    use winit::event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
    let (_p, host) = rig();
    host.execute(
        "pev, wev = {}, {}; eqoa.events.on('pointer.pressed', function(e) pev[#pev + 1] = e.data end);          eqoa.events.on('pointer.wheel', function(e) wev[#wev + 1] = e.data end)",
    )
    .unwrap();
    let press = || WindowEvent::MouseInput { device_id: DeviceId::dummy(), state: ElementState::Pressed, button: MouseButton::Left };
    input::set_modifiers((true, false, false));
    pointer_api::observe(&press(), (1024, 768));
    input::set_modifiers((false, false, true));
    pointer_api::observe(&press(), (1024, 768));
    input::set_modifiers((false, true, false));
    pointer_api::observe(&WindowEvent::MouseWheel { device_id: DeviceId::dummy(), delta: MouseScrollDelta::LineDelta(0.0, 1.0), phase: winit::event::TouchPhase::Moved }, (1024, 768));
    input::set_modifiers((false, false, false));
    pointer_api::observe(&press(), (1024, 768));
    native_events::dispatch(&host).unwrap();
    host.execute(
        "assert(#pev == 3 and #wev == 1, #pev .. ',' .. #wev);          assert(pev[1].ctrl == true and pev[1].shift == false and pev[1].alt == false, 'ctrl');          assert(pev[2].ctrl == false and pev[2].shift == true and pev[2].alt == false, 'shift');          assert(pev[3].ctrl == false and pev[3].shift == false and pev[3].alt == false, 'plain');          assert(pev[1].button == 'left');          assert(wev[1].alt == true and wev[1].ctrl == false and wev[1].shift == false, 'wheel')",
    )
    .unwrap();
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
    // where the server offers no Hide Look, nothing is sent (the control for the line below)
    let sent = p.session.sends_lost;
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.hide_look(inv.items[1].ref, true)");
    host.execute("assert(results[1].ok == false and results[1].code == 'unsupported', tostring(results[1] and results[1].code))").unwrap();
    assert_eq!(p.session.sends_lost, sent);
    // offered: hide look on a worn item sends the game's own message
    p.session.hide_look.offered = true;
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.hide_look(inv.items[1].ref, true)");
    host.execute("assert(results[1].ok == true, tostring(last_err and last_err.message) .. tostring(results[1] and results[1].reason))").unwrap();
    assert!(p.session.sends_lost > sent, "hide look went out");
    // not worn: refused, nothing sent (the control: the worn one above was sent)
    let sent = p.session.sends_lost;
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.hide_look(inv.items[2].ref, true)");
    host.execute("assert(results[1].ok == false and results[1].code == 'not_ready')").unwrap();
    assert_eq!(p.session.sends_lost, sent);
    // link item: refused where the server offers no item links (the control), done where it does
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.link_item(inv.items[2].ref)");
    host.execute("assert(results[1].ok == false and results[1].code == 'not_ready', tostring(results[1] and results[1].code))").unwrap();
    assert_ne!(p.mode, 2, "the chat line was not opened");
    p.session.item_links_offer = true;
    ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.link_item(inv.items[2].ref)");
    host.execute("assert(results[1].ok == true, tostring(results[1] and results[1].reason))").unwrap();
    assert_eq!(p.mode, 2, "the item's link is in the chat line, which the game opened");
}

#[test]
fn a1_the_question_ends_as_no_on_death_zoning_and_leaving_the_world() {
    for (what, end) in [
        ("death", Box::new(|p: &mut Play| p.death = Some(crate::death::DeathCam::new([0.; 3], [0., 0., 1.], [0.; 3]))) as Box<dyn Fn(&mut Play)>),
        ("left_world", Box::new(|p: &mut Play| p.quit = true)),
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
    step(&mut p, &host);
    host.execute("held = assert(eqoa.windows.session('loot')); assert(held.origin == 'server')").unwrap();
    assert_eq!(p.mode, 0, "hidden: the game's window is not up");
    host.execute("assert(eqoa.windows.pass(held.ref))").unwrap();
    step(&mut p, &host);
    assert_eq!(p.mode, 1, "the game's own corpse window opens");
    assert!(p.npcw.corpse.is_some(), "with the list the server sent");
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
}

// ---- fix round 1 (review findings 3, 4 and 5) ----

#[test]
fn a1_fix1_leaving_the_world_ends_every_mod_window_the_question_and_the_controller() {
    use crate::input::{Report, bit};
    for how in ["quit", "disconnect"] {
        let (mut p, host) = rig();
        host.execute("s = assert(eqoa.windows.open_session('inventory')); assert(eqoa.windows.focus(s.ref))").unwrap();
        ask(&mut p, &host, "local inv = eqoa.inventory.get(); return eqoa.actions.destroy(inv.items[2].ref)");
        // control: all three are up while play runs
        assert!(confirm::is_open() && ws::is_active());
        let mut r = Report { buttons: bit::CROSS, ..Default::default() };
        ws::apply_pad(&mut r);
        assert_eq!(r.buttons, 0, "{how}: the controller is the mod's while play runs");
        match how {
            "quit" => p.quit = true,
            _ => p.disconnected = Some(3),
        }
        // control: a Play dropped without leaving (no quit, no lost connection) ends nothing
        drop(super::super::tests::play());
        assert!(confirm::is_open() && ws::is_active() && !ws::env().away, "{how}: a Play dropped for another reason changes nothing");
        // the viewer goes back to the pregame with no further step: the world is simply let go
        drop(p);
        assert!(!confirm::is_open(), "{how}: the question is over (as No), nothing is left to expire over the pregame");
        assert!(ws::sessions_for_test().is_empty(), "{how}: no mod window session stays");
        assert!(!ws::is_active(), "{how}: nothing is held or focused");
        let mut r = Report { buttons: bit::CROSS, ..Default::default() };
        ws::apply_pad(&mut r);
        assert_eq!(r.buttons, bit::CROSS, "{how}: the controller is the game's again");
        assert!(!ws::env().plain && ws::env().away, "{how}: no window opens until a world is entered");
        // and the mod hears that its window ended
        native_events::dispatch(&host).unwrap();
        host.execute("assert(closed[1].answer ~= 'yes' and closed[1].answer ~= nil)").unwrap();
    }
}

#[test]
fn a1_fix1_the_full_map_is_not_plain_play_for_the_controller_or_a_new_window() {
    use crate::input::{Report, bit};
    let (mut p, host) = rig();
    host.execute("s = assert(eqoa.windows.open_session('inventory')); assert(eqoa.windows.focus(s.ref))").unwrap();
    step(&mut p, &host);
    assert!(ws::env().plain, "control: plain play before the map opens");
    let mut r = Report { buttons: bit::CROSS, ..Default::default() };
    ws::apply_pad(&mut r);
    assert_eq!(r.buttons, 0, "control: the focused mod window holds Cross");
    ws::apply_pad(&mut Report::default());
    p.map.show();
    step(&mut p, &host);
    assert!(!ws::env().plain, "the full map up: not plain");
    let mut r = Report { buttons: bit::CROSS | bit::UP, ..Default::default() };
    ws::apply_pad(&mut r);
    assert_eq!(r.buttons, bit::CROSS | bit::UP, "nothing is masked behind the map");
    host.execute("local ok, e = eqoa.windows.focus(s.ref); assert(ok == nil and e.code == 'not_ready', e and e.code)").unwrap();
    // an ordinary mod key's rule is not what this changes (round7e_mgr2 keeps that)
    assert!(p.addon_input_available());
}

#[test]
fn a1_fix1_pass_is_refused_where_the_game_cannot_open_its_window() {
    let (mut p, host) = rig();
    let vm = native_events::bus(&host.lua).borrow().vm;
    window_takeover::takeover(vm, "mod", TakeoverKind::Loot, TakeoverScope::Own).unwrap();
    window_takeover::activate_hidden_for_test(vm, TakeoverKind::Loot, 5);
    p.npcw.corpse = Some(net::dump::ItemList { p04: 0, items: vec![item(1, "Bone", 5, 1, -1, -1)] });
    step(&mut p, &host);
    host.execute("held = assert(eqoa.windows.session('loot'))").unwrap();
    // the manual camera (mode 38) is plain play for mod keys, but the game's windows do not open from it
    p.mode = 38;
    step(&mut p, &host);
    host.execute("local ok, e = eqoa.windows.pass(held.ref); assert(ok == nil and e.code == 'busy', 'pass said ' .. tostring(ok))").unwrap();
    assert!(ws::take_passes().is_empty(), "nothing is queued to be silently cancelled");
    // control: from the game's own camera the same call is accepted and opens the window
    p.mode = 0;
    step(&mut p, &host);
    host.execute("assert(eqoa.windows.pass(held.ref) == true)").unwrap();
    step(&mut p, &host);
    assert_eq!(p.mode, 1);
}
