// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A5 from Play-shaped state: the training points and their spending, Class Mastery from a mod's window
//! session, the faction page and the combat record, through the Lua API a mod sees. The steps that cannot be taken
//! back (spending points, buying or switching off a skill) always ask the player first.
//!
//! Each test says what its negative control is: the behaviour without the change would fail the assertion named.
use super::*;
use crate::fork_ext::addons::{
    action_broker, confirm,
    host::Host,
    native_a5,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws,
    window_takeover,
};
use net::stream::Writer;
use winit::keyboard::{Key, NamedKey};

fn game() -> Play {
    crate::settings::testing::reset(false);
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    native_a5::reset_for_test();
    a5::reset_for_test();
    r6a::forget();
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    // what the client sends is kept in `record`, message by message
    p.session.record = Some(Vec::new());
    p
}

fn rig_with(caps: &[&str]) -> (Play, Host) {
    let mut p = game();
    p.dump.player.p40 = 4;
    p.dump.player.p44 = 100;
    p.stats.base = [50; 29];
    let host = Host::with_test_capabilities(caps).unwrap();
    host.execute(
        r#"
        results, closed = {}, {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        eqoa.events.on("confirm.closed", function(e) closed[#closed + 1] = e.data end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "9" }, function() last_req, last_err = next_fn() end)
        "#,
    )
    .unwrap();
    (p, host)
}

fn rig() -> (Play, Host) {
    rig_with(&["eqoa.actions.*", "eqoa.windows.session", "eqoa.events.on"])
}

fn frame(p: &Play, h: &Host) {
    h.snapshot.set(p.forge_snapshot(None));
    native_events::dispatch(h).unwrap();
}

fn press_9(host: &Host) {
    CORE.with(|c| {
        c.borrow_mut().emit("input.key", serde_json::json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}))
    });
    native_events::dispatch(host).unwrap();
}

/// The mod asks (in a player gesture, as a key press), the game steps, the mod hears.
fn ask(p: &mut Play, host: &Host, code: &str) {
    frame(p, host);
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

/// The player's own Yes to the question that is up, and the game step that carries it out.
fn yes(p: &mut Play, host: &Host) {
    assert!(confirm::is_open(), "a question is up");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(p, host);
}

fn sent(p: &Play) -> Vec<Vec<u8>> {
    p.session.record.clone().unwrap_or_default().into_iter().map(|(_, d)| d).collect()
}

fn sent_op(p: &Play, op: u16) -> Vec<Vec<u8>> {
    sent(p).into_iter().filter(|d| d.len() >= 2 && d[..2] == op.to_le_bytes()).collect()
}

/// The Class Mastery requests sent (0x1401): their sub-ops.
fn mastery_requests(p: &Play) -> Vec<u8> {
    sent_op(p, 0x1401).into_iter().map(|d| d[2]).collect()
}

fn result_code(host: &Host) -> String {
    host.lua.load("return (last_err and last_err.code) or (results[1] and results[1].code) or 'none'").eval::<String>().unwrap()
}

/// The reply that carries the whole skill list (sub-op 8): (key, owned, known, id, rank below, category, cost, name).
fn list_reply(skills: &[(i32, bool, u8, i32, i32, u8, i32, &str)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(8).u32(77).u32(skills.len() as u32);
    for &(x00, on, known, id, parent, category, cost, name) in skills {
        w.signed(i64::from(x00)).u8(u8::from(on)).u8(known).signed(i64::from(id)).signed(i64::from(parent)).u8(category).signed(i64::from(cost)).wide(name);
    }
    w.data
}

/// The reply that opens one skill (sub-op 9).
fn node_reply(id: i32, owned: u8, known: u8, parent: i32, category: u8, name: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(9).u32(77);
    w.signed(0).u8(owned).u8(known).signed(i64::from(id)).signed(i64::from(parent)).u8(category).u8(1).signed(0).signed(0);
    for _ in 0..7 {
        w.signed(0);
    }
    w.wide(name).signed(0).wide(name).wide("it does a thing");
    w.signed(0).signed(0).signed(0);
    w.data
}

/// The game's own handling of the replies queued for Class Mastery.
fn receive(p: &mut Play, data: Vec<u8>) {
    let vui = crate::pointer::testing::vui();
    let mut r = crate::pointer::testing::NullRaster;
    p.cm_msgs.push(data);
    p.run_class_mastery(&vui, &mut r);
}

const SKILLS: [(i32, bool, u8, i32, i32, u8, i32, &str); 2] = [(1, false, 1, 100, 0, 1, 3, "Ironhide"), (2, true, 1, 101, 0, 1, 2, "Quickstep")];

fn open_mastery_session(p: &mut Play, host: &Host) {
    host.execute("session = assert(eqoa.windows.open_session('mastery'))").unwrap();
    step(p, host);
    receive(p, list_reply(&SKILLS));
    frame(p, host);
}

// ---- training points and spending ---------------------------------------------------------------------------------

/// The points read are the client's own `p40`, and each stat's cap is the level's cap plus what the masteries raise
/// it by (the trainer window's rule). Control: a different `p40` and bonus read differently, so the read is not fixed.
#[test]
fn a5_training_reads_the_points_and_each_stats_cap() {
    let (mut p, host) = rig();
    p.cm.m.bonuses[1] = 7;
    frame(&p, &host);
    host.execute(
        r#"
        local t = assert(eqoa.player.training())
        assert(t.points == 4 and t.stat_cap == 100, 'points ' .. tostring(t.points))
        assert(t.caps.strength == 100 and t.caps.stamina == 107 and t.caps.charisma == 100, 'caps ' .. tostring(t.caps.stamina))
        "#,
    )
    .unwrap();
    p.dump.player.p40 = 0;
    p.cm.m.bonuses[1] = 0;
    frame(&p, &host);
    host.execute("local t = eqoa.player.training(); assert(t.points == 0 and t.caps.stamina == 100)").unwrap();
}

/// Spending asks first: the call only puts the client's own question to the player; the game's own trainer request
/// (0x001D, the same bytes `trainer::train_stats` builds for the same points) goes out once, at the player's Yes.
/// Controls: No sends nothing; the question starts on No; a second step after the Yes sends nothing more.
#[test]
fn a5_train_stats_asks_first_and_only_the_players_yes_sends_the_trainer_request() {
    let (mut p, host) = rig();
    ask(&mut p, &host, "return eqoa.actions.train_stats({strength = 1, stamina = 2})");
    host.execute("assert(results[1] ~= nil and results[1].ok == true and results[1].detail.asked == true, tostring(last_err and last_err.code))").unwrap();
    // negative control: a call that spent at once would have put 0x001D here
    assert!(sent_op(&p, 0x001D).is_empty(), "asking spends nothing");
    let q = confirm::current_for_test().expect("the question is up");
    assert!(!q.yes, "starts on No");
    assert!(q.text.contains("3 training points") && q.text.contains("strength +1") && q.text.contains("stamina +2"), "names the spending: {}", q.text);
    host.execute("local c = eqoa.windows.confirm(); assert(c.kind == 'train_stats')").unwrap();
    // Enter on the starting choice is No
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert!(sent_op(&p, 0x001D).is_empty(), "No sends nothing");
    host.execute("assert(closed[1].kind == 'train_stats' and closed[1].answer == 'no')").unwrap();
    // asked again: Yes
    ask(&mut p, &host, "return eqoa.actions.train_stats({strength = 1, stamina = 2})");
    assert!(sent_op(&p, 0x001D).is_empty(), "not before the Yes");
    yes(&mut p, &host);
    let want = crate::trainer::train_stats(&[1, 2, 0, 0, 0, 0, 0], 4).expect("the game's own request");
    assert_eq!(sent_op(&p, 0x001D), vec![want], "the game's request, once");
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == true and closed[1].detail.confirmed == nil)").unwrap();
    step(&mut p, &host);
    assert_eq!(sent_op(&p, 0x001D).len(), 1, "never replayed");
}

/// What cannot be asked is refused at the call, as the game's own window refuses it: more than the points held, a
/// stat past its cap (with the masteries' bonus), a stat that is not one, no points at all. Nothing is asked or sent.
/// Control: the same stat is accepted once the mastery bonus lifts the cap.
#[test]
fn a5_train_stats_is_refused_over_the_points_past_a_cap_and_for_bad_arguments() {
    let (mut p, host) = rig();
    for (code, expect) in [
        ("return eqoa.actions.train_stats({strength = 3, stamina = 2})", "out_of_range"),
        ("return eqoa.actions.train_stats({strength = 0})", "invalid_argument"),
        ("return eqoa.actions.train_stats({luck = 1})", "invalid_argument"),
        ("return eqoa.actions.train_stats({strength = 1.5})", "invalid_argument"),
        ("return eqoa.actions.train_stats({strength = -1})", "out_of_range"),
        ("return eqoa.actions.train_stats(3)", "invalid_argument"),
    ] {
        ask(&mut p, &host, code);
        assert_eq!(result_code(&host), expect, "{code}");
        assert!(!confirm::is_open(), "{code} asked");
    }
    p.stats.base[2] = 100;
    ask(&mut p, &host, "return eqoa.actions.train_stats({agility = 1})");
    assert_eq!(result_code(&host), "out_of_range", "the stat is at its cap");
    p.cm.m.bonuses[2] = 5;
    ask(&mut p, &host, "return eqoa.actions.train_stats({agility = 1})");
    assert_eq!(result_code(&host), "none", "a mastery bonus lifts the cap");
    assert!(confirm::is_open());
    assert!(sent_op(&p, 0x001D).is_empty());
}

/// A Yes is carried out only if the points are still what the question named: spent elsewhere meanwhile, nothing is
/// sent (the trainer's request itself refuses more than the points held). Control: unchanged points are sent.
#[test]
fn a5_train_stats_yes_for_points_that_went_meanwhile_sends_nothing() {
    let (mut p, host) = rig();
    ask(&mut p, &host, "return eqoa.actions.train_stats({strength = 3})");
    p.dump.player.p40 = 2;
    yes(&mut p, &host);
    assert!(sent_op(&p, 0x001D).is_empty());
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == false)").unwrap();
    ask(&mut p, &host, "return eqoa.actions.train_stats({strength = 2})");
    yes(&mut p, &host);
    assert_eq!(sent_op(&p, 0x001D).len(), 1);
}

/// The call needs its capability; without it nothing is asked.
#[test]
fn a5_the_acting_calls_need_their_capabilities() {
    let (mut p, host) = rig_with(&["eqoa.actions.chat", "eqoa.events.on"]);
    for (code, name) in [
        ("return eqoa.actions.train_stats({strength = 1})", "train_stats"),
        ("return eqoa.actions.mastery_buy(100)", "mastery_buy"),
        ("return eqoa.actions.mastery_refund(100)", "mastery_refund"),
        ("return eqoa.actions.request_factions(1)", "request_factions"),
        ("return eqoa.actions.request_combat_record()", "request_combat_record"),
    ] {
        ask(&mut p, &host, code);
        assert_eq!(result_code(&host), "permission_denied", "{name}");
    }
    assert!(!confirm::is_open());
    assert!(sent(&p).is_empty());
    // control: with the grant the same call is heard
    let (mut p, host) = rig_with(&["eqoa.actions.train_stats", "eqoa.events.on"]);
    ask(&mut p, &host, "return eqoa.actions.train_stats({strength = 1})");
    assert_eq!(result_code(&host), "none");
    assert!(confirm::is_open());
}

/// What Mods & Addons says of a mod that can do these: the steps that ask first say so, in the existing words.
#[test]
fn a5_mods_and_addons_label_says_what_the_mod_can_do() {
    use crate::fork_ext::addons::runtime::{asks_first_label, autonomous_action_label};
    let caps = |names: &[&str]| names.iter().map(|n| format!("eqoa.actions.{n}")).collect::<Vec<_>>();
    assert_eq!(
        asks_first_label(&caps(&["train_stats", "mastery_buy", "mastery_refund"])).as_deref(),
        Some("Asks you first: spend training points, buy a Class Mastery skill, refund a Class Mastery skill")
    );
    // they never act on their own, so they are not in the line for what a mod does without asking
    assert_eq!(autonomous_action_label(&caps(&["train_stats", "mastery_buy", "mastery_refund"])), None);
    assert_eq!(
        autonomous_action_label(&caps(&["request_factions", "request_combat_record"])).as_deref(),
        Some("Actions: can act on its own (ask for faction standings, ask for the combat record)")
    );
    // control: a mod that declares none of them says none
    assert_eq!(asks_first_label(&caps(&["chat"])), None);
    assert_eq!(asks_first_label(&["eqoa.actions.*".to_string()]).as_deref().map(|s| s.contains("spend training points")), Some(true));
}

// ---- Class Mastery from a mod's window ----------------------------------------------------------------------------

/// A mod's window session gets the list the game's window gets: fetched when it opens (sub-op 4), cleared when it
/// closes. Control: with no session nothing is fetched at any step.
#[test]
fn a5_a_mastery_session_fetches_the_list_and_clears_it_on_close() {
    let (mut p, host) = rig();
    step(&mut p, &host);
    assert!(mastery_requests(&p).is_empty(), "no session, no fetch");
    host.execute("session = assert(eqoa.windows.open_session('mastery'))").unwrap();
    step(&mut p, &host);
    assert_eq!(mastery_requests(&p), vec![4], "the list is asked for once");
    step(&mut p, &host);
    assert_eq!(mastery_requests(&p), vec![4], "and not again");
    receive(&mut p, list_reply(&SKILLS));
    frame(&p, &host);
    host.execute("local s = assert(eqoa.mastery.skills()); assert(#s == 2 and s[1].name == 'Ironhide')").unwrap();
    host.execute("assert(eqoa.windows.close_session(session.ref))").unwrap();
    step(&mut p, &host);
    // what a mod reads goes with the list too: unavailable, not an empty list (negative control: before the fix to
    // `update_mastery_flags` this answered `{}`; CI run 45130914293 failed exactly this assertion)
    assert!(p.cm.skills.is_empty() && p.cm.node.is_none(), "the list goes with the window, as the game's close clears it");
    frame(&p, &host);
    host.execute("local s, err = eqoa.mastery.skills(); assert(s == nil and err.code == 'unavailable', tostring(s))").unwrap();
    // control: the session being open again fetches it again
    host.execute("session = assert(eqoa.windows.open_session('mastery'))").unwrap();
    step(&mut p, &host);
    assert_eq!(mastery_requests(&p), vec![4, 4], "a new session asks for the list again");
}

/// The game's own window is not disturbed: its list stays when a mod's session opens and closes beside it, and the
/// game's window that is not open is not opened.
#[test]
fn a5_a_mastery_session_leaves_the_games_own_window_alone() {
    let (mut p, host) = rig();
    p.mode = 40;
    p.cm.skills = list_skills();
    host.execute("session = assert(eqoa.windows.open_session('mastery'))").unwrap();
    step(&mut p, &host);
    assert!(mastery_requests(&p).is_empty(), "the game's window has its list: nothing fetched");
    host.execute("assert(eqoa.windows.close_session(session.ref))").unwrap();
    step(&mut p, &host);
    assert_eq!((p.cm.skills.len(), p.mode), (2, 40), "the game's list and window are as they were");
}

fn list_skills() -> Vec<crate::classmastery::Skill> {
    let mut part = crate::classmastery::Part::default();
    part.recv(&list_reply(&SKILLS));
    part.skills
}

/// Buying from a mod's window with the game's window closed: browse (the inspected skill survives without mode 40),
/// then buy asks; the player's Yes sends the game's own request (sub-op 2) once.
/// Controls: without a session the skill is dropped as the game drops it with no window, and a call has no window to
/// work on; No sends nothing.
#[test]
fn a5_mastery_buy_from_a_mod_window_asks_first_and_the_yes_sends_it_once() {
    let (mut p, host) = rig();
    // control: no session, no game window: the inspected skill is dropped, calls are refused
    receive(&mut p, list_reply(&SKILLS));
    receive(&mut p, node_reply(100, 0, 1, 0, 1, "Ironhide"));
    assert!(p.cm.node.is_none(), "with no window up the inspected skill is dropped, as before");
    ask(&mut p, &host, "return eqoa.actions.mastery_browse(100)");
    assert_eq!(result_code(&host), "not_ready");
    p.cm.skills.clear();
    // the session
    open_mastery_session(&mut p, &host);
    assert_eq!(p.mode, 0, "the game's own window is not open");
    ask(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    assert_eq!(result_code(&host), "not_ready", "inspect first");
    ask(&mut p, &host, "return eqoa.actions.mastery_browse(100)");
    assert_eq!(result_code(&host), "none");
    assert_eq!(mastery_requests(&p).last(), Some(&5), "the game's own inspect request");
    p.cm.pending = 0;
    receive(&mut p, node_reply(100, 0, 1, 0, 1, "Ironhide"));
    assert!(p.cm.node.as_ref().is_some_and(|n| n.x08 == 100), "the inspected skill survives for the mod's window");
    frame(&p, &host);
    host.execute("local n = assert(eqoa.mastery.selected()); assert(n.skill_id == 100 or n.id == 100 or n.name == 'Ironhide')").unwrap();
    // buying asks
    let before = mastery_requests(&p).len();
    ask(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    host.execute("assert(results[1].ok == true and results[1].detail.asked == true, tostring(last_err and last_err.code))").unwrap();
    assert_eq!(mastery_requests(&p).len(), before, "negative control: the old call sent sub-op 2 here");
    let q = confirm::current_for_test().expect("the question is up");
    assert!(q.text.contains("Ironhide") && q.text.contains("3 points") && !q.yes, "{}", q.text);
    host.execute("local c = eqoa.windows.confirm(); assert(c.kind == 'mastery_buy')").unwrap();
    key(NamedKey::Escape);
    step(&mut p, &host);
    assert_eq!(mastery_requests(&p).len(), before, "No sends nothing");
    host.execute("assert(closed[1].kind == 'mastery_buy' and closed[1].answer == 'no')").unwrap();
    // Yes
    ask(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    yes(&mut p, &host);
    assert_eq!(mastery_requests(&p).get(before..), Some(&[2u8][..]), "the purchase, once");
    assert_eq!(p.cm.pending, 2);
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == true)").unwrap();
    // one request at a time: a second purchase is refused until the server answers or the game gives up
    ask(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    assert_eq!(result_code(&host), "busy");
    let mut since = None;
    crate::classmastery::expire_wait(&mut p.cm.pending, &mut since, 1_000);
    crate::classmastery::expire_wait(&mut p.cm.pending, &mut since, 1_000 + crate::classmastery::WAIT_MS);
    assert_eq!(p.cm.pending, 0, "the 10 second give-up frees the lock");
    ask(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    assert_eq!(result_code(&host), "none");
}

/// Switching off an owned skill asks too; the skill must be owned. A Yes after the list changed under the question
/// (the skill is no longer owned) sends nothing.
#[test]
fn a5_mastery_refund_asks_first_and_checks_again_at_the_yes() {
    let (mut p, host) = rig();
    open_mastery_session(&mut p, &host);
    ask(&mut p, &host, "return eqoa.actions.mastery_browse(101)");
    p.cm.pending = 0;
    receive(&mut p, node_reply(101, 1, 1, 0, 1, "Quickstep"));
    let before = mastery_requests(&p).len();
    ask(&mut p, &host, "return eqoa.actions.mastery_refund(101)");
    host.execute("assert(results[1].ok == true and results[1].detail.asked == true, tostring(last_err and last_err.code))").unwrap();
    assert_eq!(mastery_requests(&p).len(), before, "asking sends nothing");
    host.execute("assert(eqoa.windows.confirm().kind == 'mastery_refund')").unwrap();
    // the skill is switched off by someone else while the question is up
    p.cm.skills[1].on = false;
    yes(&mut p, &host);
    assert_eq!(mastery_requests(&p).len(), before, "never sends for a skill that is not what was asked about");
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == false)").unwrap();
    // control: owned, the Yes sends sub-op 3
    p.cm.skills[1].on = true;
    ask(&mut p, &host, "return eqoa.actions.mastery_refund(101)");
    yes(&mut p, &host);
    assert_eq!(mastery_requests(&p).get(before..), Some(&[3u8][..]));
    // a skill that is not owned is refused at the call
    p.cm.pending = 0;
    ask(&mut p, &host, "return eqoa.actions.mastery_refund(100)");
    assert_eq!(result_code(&host), "not_ready");
}

/// The Master Class rule is the game's: a second Master Class is refused at the call (with the game's warning), not
/// asked about. Control: with no other Master Class owned the same skill is asked about.
#[test]
fn a5_mastery_buy_keeps_the_master_class_rule() {
    let (mut p, host) = rig();
    let master = crate::classmastery::MASTER_CLASS;
    let list = [(1, true, 1, 100, 0, master, 3, "Master Smith"), (2, false, 1, 101, 0, master, 3, "Master Mage")];
    host.execute("session = assert(eqoa.windows.open_session('mastery'))").unwrap();
    step(&mut p, &host);
    receive(&mut p, list_reply(&list));
    ask(&mut p, &host, "return eqoa.actions.mastery_browse(101)");
    p.cm.pending = 0;
    receive(&mut p, node_reply(101, 0, 1, 0, master, "Master Mage"));
    ask(&mut p, &host, "return eqoa.actions.mastery_buy(101)");
    assert_eq!(result_code(&host), "not_ready", "another Master Class is owned");
    assert!(!confirm::is_open());
    // control
    p.cm.skills[0].on = false;
    ask(&mut p, &host, "return eqoa.actions.mastery_buy(101)");
    assert_eq!(result_code(&host), "none");
    assert!(confirm::current_for_test().unwrap().text.contains("Master Class"));
}

/// With the game's own Class Mastery window open (no mod session) the same calls work and ask the same: a mod never
/// skips the question where the game would have asked. Control: nothing is sent before the Yes.
#[test]
fn a5_mastery_buy_in_the_games_own_window_asks_too() {
    let (mut p, host) = rig();
    p.mode = 40;
    p.cm.skills = list_skills();
    p.cm.node = Some(crate::classmastery::Node { x08: 100, x05: 1, ..Default::default() });
    p.cm.skills[0].name = "Ironhide".encode_utf16().collect();
    frame(&p, &host);
    ask(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    host.execute("assert(results[1].detail.asked == true)").unwrap();
    assert!(mastery_requests(&p).is_empty());
    yes(&mut p, &host);
    assert_eq!(mastery_requests(&p), vec![2]);
}

/// A session that ends (here by the mod's window closing) takes its question with it: a Yes cannot come for a
/// window that is gone.
#[test]
fn a5_closing_the_window_ends_its_question() {
    let (mut p, host) = rig();
    open_mastery_session(&mut p, &host);
    ask(&mut p, &host, "return eqoa.actions.mastery_browse(100)");
    p.cm.pending = 0;
    receive(&mut p, node_reply(100, 0, 1, 0, 1, "Ironhide"));
    ask(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    assert!(confirm::is_open());
    host.execute("assert(eqoa.windows.close_session(session.ref))").unwrap();
    assert!(!confirm::is_open(), "the question ended with the window");
    step(&mut p, &host);
    assert!(!mastery_requests(&p).contains(&2), "nothing was bought");
    host.execute("assert(closed[1].kind == 'mastery_buy' and closed[1].answer == 'closed', tostring(closed[1] and closed[1].answer))").unwrap();
}

/// The game's own window for the same data is unchanged with the mod off: nothing is fetched, no list is touched, and
/// a skill the game's window did not hold is dropped as before. The one hook is false without a mod's session.
#[test]
fn a5_with_no_mod_window_the_games_class_mastery_is_unchanged() {
    let (mut p, host) = rig();
    p.cm.skills = list_skills();
    for _ in 0..3 {
        step(&mut p, &host);
    }
    assert_eq!(p.cm.skills.len(), 2, "a list the game holds is left alone");
    assert!(mastery_requests(&p).is_empty(), "no Class Mastery request is sent by the game's steps");
    assert!(!a5::node_kept(&p) && !a5::mastery_open(&p));
    p.mode = 40;
    assert!(a5::mastery_open(&p) && !a5::node_kept(&p), "the game's own window decides for itself");
    // control: a session turns the node seam on, only when the game's window is not up
    host.execute("session = assert(eqoa.windows.open_session('mastery'))").unwrap();
    assert!(!a5::node_kept(&p), "mode 40: the game's rule");
    p.mode = 0;
    assert!(a5::node_kept(&p));
}

// ---- factions and the combat record -------------------------------------------------------------------------------

fn factions_reply(seq: i64, page: i64, pages: i64, list: &[(&str, i64)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(119).signed(seq).signed(page).signed(pages).signed(list.len() as i64);
    for (name, standing) in list {
        w.wide(name).signed(*standing);
    }
    w.data
}

/// A faction page is asked for as the game asks (0x0AC4, byte 102, the page from 0) and read when it arrives. Controls:
/// nothing to read before a reply; a reply for an older request is dropped as the game drops it; a page past the page
/// count is refused.
#[test]
fn a5_factions_are_asked_by_page_and_read_when_the_reply_comes() {
    let (mut p, host) = rig();
    frame(&p, &host);
    host.execute("local f, err = eqoa.factions.list(); assert(f == nil and err.code == 'unavailable')").unwrap();
    let want = p.factions.clone().request(0);
    ask(&mut p, &host, "return eqoa.actions.request_factions(1)");
    assert_eq!(result_code(&host), "none");
    assert_eq!(sent_op(&p, 0x0AC4), vec![want], "the game's own request for its first page");
    // a reply for an older request is dropped
    p.factions.recv(&factions_reply(99, 0, 2, &[("Old", 1)]));
    frame(&p, &host);
    host.execute("local f, err = eqoa.factions.list(); assert(f == nil and err.code == 'unavailable')").unwrap();
    p.factions.recv(&factions_reply(1, 0, 3, &[("Qeynos Guard", 1500), ("Dark Elves", -2500)]));
    frame(&p, &host);
    host.execute(
        r#"
        local f = assert(eqoa.factions.list())
        assert(f.page == 1 and f.pages == 3 and #f.entries == 2)
        assert(f.entries[1].name == 'Qeynos Guard' and f.entries[1].standing == 1500)
        assert(f.entries[2].standing == -2500)
        "#,
    )
    .unwrap();
    ask(&mut p, &host, "return eqoa.actions.request_factions(4)");
    assert_eq!(result_code(&host), "out_of_range", "the server said it has 3 pages");
    ask(&mut p, &host, "return eqoa.actions.request_factions(0)");
    assert_eq!(result_code(&host), "out_of_range");
    ask(&mut p, &host, "return eqoa.actions.request_factions(2)");
    assert_eq!(result_code(&host), "none");
    assert_eq!(sent_op(&p, 0x0AC4).len(), 2);
    frame(&p, &host);
    host.execute("local f, err = eqoa.factions.list(); assert(f == nil, 'the new request clears the page, as the game does')").unwrap();
}

/// The combat record is asked of a server that offers it and read when the server's push arrives. Controls: on a
/// server that does not offer it the ask is refused and nothing is sent, and the read says why.
#[test]
fn a5_the_combat_record_is_asked_and_read() {
    let (mut p, host) = rig();
    frame(&p, &host);
    host.execute("local r, err = eqoa.combat.record(); assert(r == nil and err.code == 'unavailable' and err.message:find('keeps no combat record'))").unwrap();
    ask(&mut p, &host, "return eqoa.actions.request_combat_record()");
    assert_eq!(result_code(&host), "unsupported");
    assert!(sent_op(&p, 0x7E21).is_empty());
    p.session.combat_record = true;
    ask(&mut p, &host, "return eqoa.actions.request_combat_record()");
    assert_eq!(result_code(&host), "none");
    assert_eq!(sent_op(&p, 0x7E21).len(), 1, "the game's own ask");
    assert_eq!(p.mode, 0, "the game's own window is not opened");
    frame(&p, &host);
    host.execute("local r, err = eqoa.combat.record(); assert(r == nil and err.message:find('no combat record has arrived'))").unwrap();
    p.record_data(&crate::record::tests::body("Dabran", &[(0, "Duels", ""), (1, "Won", "5")]));
    frame(&p, &host);
    host.execute(
        r#"
        local r = assert(eqoa.combat.record())
        assert(r.name == 'Dabran' and #r.rows == 2)
        assert(r.rows[1].heading == true and r.rows[1].label == 'Duels')
        assert(r.rows[2].heading == false and r.rows[2].label == 'Won' and r.rows[2].value == '5')
        "#,
    )
    .unwrap();
    // a count that comes again is read again
    p.record_data(&crate::record::tests::body("Dabran", &[(0, "Duels", ""), (1, "Won", "6")]));
    frame(&p, &host);
    host.execute("assert(eqoa.combat.record().rows[2].value == '6')").unwrap();
}

/// The reads and the asks are the game's own state: nothing is read outside the world.
#[test]
fn a5_the_reads_say_not_in_the_world_outside_it() {
    let (p, host) = rig();
    CORE.with(|c| c.borrow_mut().view.in_world = false);
    let _ = &p;
    host.execute(
        r#"
        local f, e1 = eqoa.factions.list(); assert(f == nil and e1.code == 'not_in_world')
        local r, e2 = eqoa.combat.record(); assert(r == nil and e2.code == 'not_in_world')
        "#,
    )
    .unwrap();
}
