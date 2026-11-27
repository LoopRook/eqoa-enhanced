// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A2: the bundled Dialog Window mod. It ships off, declares what it calls, holds its kinds with scope all, and
//! reacts to a dialog session the client starts (the registry the packets feed: `dialog_sessions`). The quest
//! hand-in helper is part of it: an answer is marked by the quest's own words, and Speed through hand-ins steps
//! through a talk with someone the journal sends the player to.
use super::{
    action_broker,
    bridge::Bridge,
    dialog_sessions::{self, Item},
    discovery::DiscoveredAddon,
    host::Host,
    native_core::{CORE, Core},
    native_events, options, window_sessions as ws, window_takeover,
};
use dear_imgui_rs::Context;
use serde_json::json;
use std::path::PathBuf;

fn root(id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(id).canonicalize().unwrap()
}

fn manifest(id: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root(id).join("mod.json")).unwrap()).unwrap()
}

/// The mod as discovery reads it (so its `replaces` and `provides` are the manifest's).
fn addon(id: &str) -> DiscoveredAddon {
    let found = super::discovery::discover(root(id).parent().unwrap());
    found.addons.into_iter().find(|a| a.id == id).unwrap_or_else(|| panic!("{id} is bundled"))
}

fn context() -> Context {
    let mut context = Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_display_size([1024.0, 768.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.font_atlas().try_claim_legacy_renderer().unwrap().build();
    context
}

fn fixture() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    options::reset_in_memory();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = json!({"id": 256, "name": "Me", "level": 20});
        c.view.quests = vec![
            json!({"ref": {"$ref": "quest", "key": "1", "generation": 0, "incarnation": 1}, "index": 1, "title": "Bat Eyes"}),
            json!({"ref": {"$ref": "quest", "key": "2", "generation": 0, "incarnation": 1}, "index": 2, "title": "The Militia"}),
        ];
    });
}

fn frames(host: &Host, bridge: &Bridge, context: &mut Context, n: usize) -> usize {
    let mut vertices = 0;
    for _ in 0..n {
        context.frame();
        bridge
            .invoke(|| {
                native_events::dispatch(host)?;
                native_events::render(host)
            })
            .unwrap();
        vertices = context.render_legacy().draw_data().total_vtx_count();
    }
    vertices
}

fn press(host: &Host, key: &str) {
    CORE.with(|c| {
        c.borrow_mut().emit("input.key", json!({"key": key, "combo": key, "down": true, "repeat": false, "synthetic": false, "text": ""}))
    });
    native_events::dispatch(host).unwrap();
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

struct Rig {
    host: Host,
    bridge: Bridge,
    context: Context,
    vm: u64,
}

fn rig(id: &str) -> Rig {
    fixture();
    let m = manifest(id);
    assert_eq!(m["enabled"], false, "{id} ships off");
    assert!(crate::fork_ext::bundled::PACKAGES.contains(&id), "{id} ships in the bundle");
    for file in ["mod.json", "main.lua", "README.md"] {
        assert!(crate::fork_ext::bundled::EMBEDDED_FILES.iter().any(|f| f.path == format!("{id}/{file}")), "{file} ships");
    }
    let context = context();
    let host = Host::for_addon(&addon(id)).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon(id)).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    Rig { host, bridge, context, vm }
}

impl Rig {
    fn frames(&mut self, n: usize) -> usize {
        frames(&self.host, &self.bridge, &mut self.context, n)
    }
    /// A dialog with answers arrives and starts as a session for this mod, as the client does it.
    fn dialog(&mut self, id: u32, text: &str, options: &[&str]) {
        dialog_sessions::hold(
            Item::Box { id, text: wide(text), options: options.iter().map(|o| wide(o)).collect(), talked_to: None },
            None,
        );
        assert!(dialog_sessions::start_next(true, None).is_some(), "the session started");
    }
}

/// The mod holds every kind it declares (scope all) after its first frames.
fn assert_holds(vm: u64, kinds: &[window_takeover::TakeoverKind]) {
    for k in kinds {
        assert_eq!(window_takeover::holder_vm(*k), Some(vm), "{} is held", k.name());
    }
}

#[test]
fn a2_manifests_declare_what_the_mods_call() {
    for id in ["eqoa_dialogs"] {
        let m = manifest(id);
        let declared: Vec<String> = m["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
        let src = std::fs::read_to_string(root(id).join("main.lua")).unwrap();
        for cap in ["eqoa.windows.takeover", "eqoa.windows.session", "eqoa.actions.dialog_answer"] {
            assert!(declared.iter().any(|d| d == cap), "{id} declares {cap}");
        }
        for (call, cap) in [("eqoa.options.toggle", "eqoa.options.toggle"), ("eqoa.options.get", "eqoa.options.get"), ("eqoa.log.write", "eqoa.log.write")] {
            if src.contains(call) {
                assert!(declared.iter().any(|d| d == cap), "{id} calls {call} and must declare {cap}");
            }
        }
        assert!(m["replaces"].as_array().unwrap().iter().any(|r| r == "response_dialog"), "{id} replaces the dialog");
    }
    // the Dialog Window also takes the offers
    let m = manifest("eqoa_dialogs");
    for kind in ["group_invite", "revive_offer", "summon_offer"] {
        assert!(m["replaces"].as_array().unwrap().iter().any(|r| r == kind), "{kind}");
    }
}

#[test]
fn a2_mods_and_addons_labels_the_dialog_mods() {
    let found = super::discovery::discover(root("eqoa_dialogs").parent().unwrap());
    let lines = |id: &str| {
        let a = found.addons.iter().find(|a| a.id == id).unwrap_or_else(|| panic!("{id} is bundled"));
        super::runtime::disclosure_lines(a)
    };
    let dialogs = lines("eqoa_dialogs");
    assert!(dialogs.contains(&"Can answer NPC dialogs".to_string()), "{dialogs:?}");
    assert!(
        dialogs.iter().any(|l| l == "Replaces game windows: Dialog, Group Invitation, Resurrection Offer, Summons"),
        "{dialogs:?}"
    );
    assert!(dialogs.contains(&"Provides: NPC dialog window".to_string()), "{dialogs:?}");
    assert!(crate::fork_ext::bundled::PACKAGES.iter().all(|p| *p != "eqoa_turnin"), "the separate helper is gone");
    // the control: a mod that answers nothing says nothing of the kind
    assert!(!lines("eqoa_stats_box").contains(&"Can answer NPC dialogs".to_string()));
}

/// Vertices drawn for one dialog with these answers, with the highlight option on or off (the same words either
/// way, so a difference is only the mark and the Hand in button).
fn drawn_with(options: &[&str], highlight: bool) -> usize {
    let mut r = rig("eqoa_dialogs");
    options::set_toggle("eqoa_dialogs", "highlight", highlight);
    r.frames(2);
    r.dialog(1, "Well?", options);
    r.frames(3)
}

#[test]
fn a2_dialogs_mark_an_answer_that_names_a_quest_by_its_words_and_pass_nothing() {
    let mut r = rig("eqoa_dialogs");
    r.frames(3);
    assert_eq!(options::get_toggle("eqoa_dialogs", "highlight"), Some(true), "highlight is on by default");
    assert_eq!(options::get_toggle("eqoa_dialogs", "speed_hand_ins"), Some(false), "speeding through hand-ins is off by default");
    r.dialog(1, "Well?", &["Tell me about the weather", "I have brought the bat eyes you wanted", "Goodbye"]);
    assert!(r.frames(3) > 0, "the dialog is drawn: {:?}", r.host.drain_logs());
    assert!(dialog_sessions::take_passes().is_empty(), "a hand-in dialog is not passed back");
    assert!(action_broker::queued_kinds_for_test().is_empty(), "and nothing is answered without the player's click");
    drop(r);
    let quest = ["Tell me about the weather", "I have brought the bat eyes you wanted", "Goodbye"];
    let on = drawn_with(&quest, true);
    // negative control: with the option off the same dialog draws no mark and no Hand in button
    assert!(on > drawn_with(&quest, false), "the hand-in is marked only when the option is on");
}

#[test]
fn a2_dialogs_match_on_the_options_words_never_on_their_position() {
    // the same words in another place are still a match
    let moved = ["Goodbye", "Not now", "About The Militia: I am done", "Tell me more"];
    assert!(drawn_with(&moved, true) > drawn_with(&moved, false), "a quest named in the third answer is marked");
    // answers that only look like hand-ins by position, or name no journal quest, are not marked
    for unknown in [
        ["Option 1", "Option 2", "Option 3", "Option 4"],
        ["Where is the inn?", "Goodbye", "Not now", "Tell me more"],
    ] {
        assert_eq!(drawn_with(&unknown, true), drawn_with(&unknown, false), "{unknown:?} is not marked");
    }
}

// ---- Speed through hand-ins (2026-11-27) -------------------------------------------------------------------------
// With the switch on, a talk to someone the journal sends the player to (a quest "?" step mark on their name) is stepped
// through by the mod: Done is pressed, the single real answer is picked, and it stops at a real choice. These replace the
// tests of the retired "Auto-answer hand-ins" rule (`a2_dialogs_answer_a_hand_in_by_themselves_only_when_told_to_and_only_with_one_real_answer`).

const NPC: &str = "Commander Nothard";

/// The client's quest marks: a "?" step on `name` (the guide's tag on the name, as the client writes it) in world 0.
fn mark_step(name: &str) {
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.map = json!({"enabled": true, "open": false, "minimap": false, "markers_on": true, "world_id": 0, "revision": 1});
        c.view.map_markers = vec![json!({"world_id": 0, "kind": "quest", "step": true, "name": format!("{name} (guide)"), "x": 100.0, "z": 200.0, "level_min": 1, "level_max": 9, "label": null})];
    });
}

fn speed_rig(on: bool) -> Rig {
    let mut r = rig("eqoa_dialogs");
    options::set_toggle("eqoa_dialogs", "speed_hand_ins", on);
    r.frames(2);
    r
}

impl Rig {
    /// A plain message from `who` (None: the speaker is not known) arrives and starts as a session.
    fn say_text(&mut self, who: Option<&str>, text: &str) {
        let talked_to = who.map(|w| (7u32, w.to_string()));
        dialog_sessions::hold(Item::Text { text: wide(text), talked_to }, None);
        assert!(dialog_sessions::start_next(true, None).is_some(), "the text session started");
    }
    /// A dialog with answers from `who` arrives and starts as a session.
    fn say_box(&mut self, id: u32, who: Option<&str>, text: &str, options: &[&str]) {
        let talked_to = who.map(|w| (7u32, w.to_string()));
        dialog_sessions::hold(Item::Box { id, text: wide(text), options: options.iter().map(|o| wide(o)).collect(), talked_to }, None);
        assert!(dialog_sessions::start_next(true, None).is_some(), "the box session started");
    }
    /// The server took the answer: the dialog's session ends "answered", as it does when the client sent it.
    fn answered(&mut self) {
        let s = ws::of_kind("text_dialog").or_else(|| ws::of_kind("response_dialog")).expect("a dialog is up");
        dialog_sessions::finish(&s.id, "answered");
    }
}

fn answers_sent() -> usize {
    action_broker::queued_kinds_for_test().iter().filter(|k| **k == "dialog_answer").count()
}

#[test]
fn speed_hand_ins_is_off_by_default_under_a_new_id_so_the_old_saved_rule_cannot_turn_it_on() {
    let mut r = rig("eqoa_dialogs");
    r.frames(3);
    assert_eq!(options::get_toggle("eqoa_dialogs", "speed_hand_ins"), Some(false), "off until turned on");
    assert_eq!(options::get_toggle("eqoa_dialogs", "auto_answer"), None, "the old rule's switch is gone, nothing reads a saved value of it");
}

/// (a) text, text, one answer, then two answers, to someone with a "?" for the player: three answers, and it stops at the
/// fourth, which has the choice marked; the earlier words stay (the record). Negative control: the old rule answered none
/// of this (no quest named in the answers), and (b) and (c) below show the same talk untouched without the mark or switch.
#[test]
fn speed_hand_ins_steps_through_a_talk_and_stops_at_the_first_real_choice() {
    mark_step(NPC);
    let mut r = speed_rig(true);
    r.say_text(Some(NPC), "Greetings, friend.");
    r.frames(2);
    assert_eq!(answers_sent(), 1, "Done is pressed: {:?}", r.host.drain_logs());
    r.answered();
    r.say_text(Some(NPC), "I have a task for you.");
    r.frames(2);
    assert_eq!(answers_sent(), 2, "Done again");
    r.answered();
    r.say_box(1, Some(NPC), "Will you take the bracer to the Militia?", &["Yes, I will take it", "Goodbye"]);
    r.frames(2);
    assert_eq!(answers_sent(), 3, "the single real answer is picked (a Goodbye beside it is not a choice)");
    r.answered();
    r.say_box(2, Some(NPC), "Which road will you walk?", &["The north road", "The south road"]);
    let marked = r.frames(3);
    assert_eq!(answers_sent(), 3, "two answers is a real choice: it waits for the player");
    assert!(action_broker::queued_kinds_for_test().iter().all(|k| *k == "dialog_answer"), "only answers were sent");
    let logs = r.host.drain_logs().unwrap();
    assert!(logs.iter().any(|l| l.contains("stopped: choice") && l.contains("3 steps in the record")), "stopped at the choice with the talk recorded: {logs:?}");
    // the stop is shown (a banner and a frame round the answers) and the earlier words are drawn: more than the same dialog bare
    drop(r);
    let mut bare = speed_rig(false);
    bare.say_box(2, Some(NPC), "Which road will you walk?", &["The north road", "The south road"]);
    assert!(marked > bare.frames(3), "the stopped choice is marked and the record is drawn");
}

/// (b) control: the same talk to someone with no "?" for the player is not touched.
#[test]
fn speed_hand_ins_leaves_a_talk_with_someone_without_a_question_mark_alone() {
    mark_step("Somebody Else");
    let mut r = speed_rig(true);
    r.say_text(Some(NPC), "Greetings, friend.");
    r.frames(2);
    r.answered();
    r.say_box(1, Some(NPC), "Take it?", &["Yes, I will take it"]);
    r.frames(3);
    assert_eq!(answers_sent(), 0, "no mark on this name: nothing answered");
    drop(r);
    // a mark that is a quest on offer (a "!", step = false) is not a "?" either
    mark_step(NPC);
    CORE.with(|c| c.borrow_mut().view.map_markers[0]["step"] = json!(false));
    let mut r = speed_rig(true);
    r.say_text(Some(NPC), "Greetings, friend.");
    r.frames(3);
    assert_eq!(answers_sent(), 0, "a quest on offer is not a hand-in");
    drop(r);
    // a mark in another world does not count
    mark_step(NPC);
    CORE.with(|c| c.borrow_mut().view.map_markers[0]["world_id"] = json!(5));
    let mut r = speed_rig(true);
    r.say_text(Some(NPC), "Greetings, friend.");
    r.frames(3);
    assert_eq!(answers_sent(), 0, "a mark elsewhere is not this person");
}

/// (c) control: with the switch off, nothing is answered, even to a marked person.
#[test]
fn speed_hand_ins_does_nothing_with_the_switch_off() {
    mark_step(NPC);
    let mut r = speed_rig(false);
    r.say_text(Some(NPC), "Greetings, friend.");
    r.frames(3);
    r.answered();
    r.say_box(1, Some(NPC), "Take it?", &["Yes, I will take it"]);
    r.frames(3);
    assert_eq!(answers_sent(), 0, "off: nothing runs");
}

/// (d) the run is capped at 12 steps.
#[test]
fn speed_hand_ins_stops_after_twelve_steps() {
    mark_step(NPC);
    let mut r = speed_rig(true);
    for i in 1..=13 {
        r.say_text(Some(NPC), &format!("Part {i} of a very long tale."));
        r.frames(2);
        if i <= 12 {
            r.answered();
        }
    }
    assert_eq!(answers_sent(), 12, "twelve steps, then it stops and waits for the player");
    let logs = r.host.drain_logs().unwrap();
    assert!(logs.iter().any(|l| l.contains("stopped: cap")), "{logs:?}");
}

/// (e) Escape stops it at once: on the live dialog (one press: the conversation is left, nothing else) and between steps
/// (the press is held for the mod, and the server's late next step is then not answered).
#[test]
fn speed_hand_ins_stops_on_escape_between_steps_and_on_a_dialog() {
    mark_step(NPC);
    let mut r = speed_rig(true);
    r.say_text(Some(NPC), "Greetings, friend.");
    r.frames(2);
    assert_eq!(answers_sent(), 1);
    r.answered();
    r.frames(2);
    assert!(super::input::is_key_suppressed(0x1B), "while it waits for the next step Escape is held for the mod");
    press(&r.host, "escape");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["dialog_answer"], "Escape between steps only stops the run: nothing else is sent");
    r.say_text(Some(NPC), "I have a task for you.");
    r.frames(3);
    assert_eq!(answers_sent(), 1, "the late step is shown, not answered");
    // Escape on the live dialog leaves, once
    action_broker::reset_all_for_test();
    press(&r.host, "escape");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["close_window"], "one press, one thing");
    drop(r);
    // at a stopped choice, Escape also just leaves
    let mut r = speed_rig(true);
    r.say_box(3, Some(NPC), "Which road?", &["North", "South"]);
    r.frames(3);
    press(&r.host, "escape");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["close_window"]);
    assert!(dialog_sessions::take_passes().is_empty(), "the game's window does not open");
}

/// (f) a speaker that is not known stops it (and a first dialog with no known speaker never starts it).
#[test]
fn speed_hand_ins_stops_when_it_is_not_known_who_is_speaking() {
    mark_step(NPC);
    let mut r = speed_rig(true);
    r.say_text(None, "Who said this?");
    r.frames(3);
    assert_eq!(answers_sent(), 0, "no speaker: not started");
    drop(r);
    let mut r = speed_rig(true);
    r.say_text(Some(NPC), "Greetings, friend.");
    r.frames(2);
    assert_eq!(answers_sent(), 1);
    r.answered();
    r.say_text(None, "And then?");
    r.frames(3);
    assert_eq!(answers_sent(), 1, "the speaker became unknown: it stops");
    let logs = r.host.drain_logs().unwrap();
    assert!(logs.iter().any(|l| l.contains("stopped: unknown")), "{logs:?}");
}

/// (g) only a real answer is picked: a lone Goodbye / Close / Back is never chosen for the player.
#[test]
fn speed_hand_ins_never_picks_a_goodbye_or_close() {
    mark_step(NPC);
    for lone in ["Goodbye", "Close", "Back", "Farewell."] {
        let mut r = speed_rig(true);
        r.say_box(1, Some(NPC), "Anything else?", &[lone]);
        r.frames(3);
        assert_eq!(answers_sent(), 0, "{lone} is not picked");
    }
}

/// (h) one answer per step: the same session seen for many frames is answered once, and the same words coming again
/// straight after are not answered a second time.
#[test]
fn speed_hand_ins_answers_a_step_once_and_not_a_repeat_of_it() {
    mark_step(NPC);
    let mut r = speed_rig(true);
    r.say_box(1, Some(NPC), "Take the bracer?", &["Yes, I will take it"]);
    r.frames(10);
    assert_eq!(answers_sent(), 1, "many frames, one answer");
    r.answered();
    r.say_box(1, Some(NPC), "Take the bracer?", &["Yes, I will take it"]);
    r.frames(5);
    assert_eq!(answers_sent(), 1, "a slow server repeating the step is not answered again");
    let logs = r.host.drain_logs().unwrap();
    assert!(logs.iter().any(|l| l.contains("stopped: repeat")), "{logs:?}");
}

/// The player's own key answer also stops it: after it the rest of the talk is his.
#[test]
fn speed_hand_ins_steps_aside_when_the_player_answers_himself() {
    mark_step(NPC);
    let mut r = speed_rig(true);
    r.say_box(4, Some(NPC), "Which road?", &["North", "South"]);
    r.frames(3);
    press(&r.host, "2");
    assert_eq!(answers_sent(), 1, "his own answer goes");
    r.answered();
    r.say_text(Some(NPC), "A fine choice.");
    r.frames(3);
    assert_eq!(answers_sent(), 1, "after his own answer the rest is his: it does not start again");
}

#[test]
fn a2_dialog_window_draws_a_dialog_answers_with_a_key_and_gives_it_back_on_escape() {
    let mut r = rig("eqoa_dialogs");
    assert_eq!(r.frames(3), 0, "no dialog, nothing drawn");
    assert_holds(
        r.vm,
        &[
            window_takeover::TakeoverKind::TextDialog,
            window_takeover::TakeoverKind::ResponseDialog,
            window_takeover::TakeoverKind::GroupInvite,
            window_takeover::TakeoverKind::ReviveOffer,
            window_takeover::TakeoverKind::SummonOffer,
        ],
    );
    r.dialog(10, "Welcome, traveller. What do you seek?", &["Rumors", "Work", "Nothing"]);
    let vertices = r.frames(3);
    assert!(vertices > 0, "the dialog window is drawn: {:?}", r.host.drain_logs());
    let logs: Vec<String> = r.host.drain_logs().unwrap().into_iter().filter(|l| !l.contains("[info]")).collect();
    assert!(logs.is_empty(), "no warning or error: {logs:?}");
    press(&r.host, "2");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["dialog_answer"], "key 2 answers the second option");
    // Escape leaves the conversation (the game's own "no choice"): the game's own window does NOT open (owner's test
    // 2026-11-27: Escape in a conversation opened the game's dialog). Negative control: the mod before the fix passed
    // the dialog back (`take_passes` of 1, no close queued).
    action_broker::reset_all_for_test();
    dialog_sessions::take_passes();
    press(&r.host, "escape");
    assert!(dialog_sessions::take_passes().is_empty(), "Escape does not hand the dialog to the game's window");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["close_window"], "Escape leaves: one close of the dialog's session");
}

/// An offer has to be answered: Escape is held (the game never sees it) and does nothing: no answer, no close, and the offer is
/// not handed to the game's window either.
#[test]
fn a2_dialog_window_an_offers_escape_does_nothing_and_does_not_pass() {
    const ESCAPE: u8 = 0x1B;
    let mut r = rig("eqoa_dialogs");
    r.frames(2);
    dialog_sessions::hold(Item::Offer { summons: true, name: "Mage".into(), text: wide("Mage summons you.") }, None);
    r.frames(3);
    assert!(super::input::is_key_suppressed(ESCAPE), "the offer holds Escape, so the game's Start menu does not open");
    action_broker::reset_all_for_test();
    dialog_sessions::take_passes();
    press(&r.host, "escape");
    assert!(action_broker::queued_kinds_for_test().is_empty(), "Escape neither declines nor answers an offer");
    assert!(dialog_sessions::take_passes().is_empty(), "and does not hand it to the game's window");
}

#[test]
fn a2_dialog_window_pages_a_long_list_and_shows_an_offer() {
    let mut r = rig("eqoa_dialogs");
    r.frames(2);
    let many: Vec<String> = (1..=10).map(|i| format!("Choice {i}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    r.dialog(11, "Many", &refs);
    assert!(r.frames(3) > 0);
    // 7 is More on a page of six: the seventh option needs the next page, so key 7 pages and does not answer
    press(&r.host, "7");
    assert!(action_broker::queued_kinds_for_test().is_empty(), "More only turns the page");
    r.frames(2);
    press(&r.host, "1");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["dialog_answer"], "key 1 on the second page is the seventh option");
    // an offer is shown in its own box and answered with the same call
    drop(r);
    let mut r = rig("eqoa_dialogs");
    r.frames(2);
    dialog_sessions::hold(Item::Offer { summons: true, name: "Mage".into(), text: wide("Mage summons you.") }, None);
    assert!(r.frames(3) > 0, "the offer box is drawn");
}

/// Owner test 2026-11-23: Enter in a dialog advanced it and also opened the game's chat line, after which chat would not
/// take typing. The mod asked for the Enter key under a name the host does not know ("enter"), so the claim was refused
/// and the key went on to the game. Enter is the window's while a dialog is up, on both kinds of dialog, and the game has
/// it again the moment the dialog is gone (nothing stays claimed).
#[test]
fn a2_dialog_window_keeps_enter_from_the_game_while_up_and_gives_it_back_after() {
    const ENTER: u8 = 0x0D;
    let suppressed = || super::input::is_key_suppressed(ENTER);
    let mut r = rig("eqoa_dialogs");
    r.frames(3);
    assert!(!suppressed(), "no dialog: Enter is the game's (control)");
    // a plain message: Enter is Done, and the game does not see the key
    dialog_sessions::hold(Item::Text { text: wide("Hail, traveller."), talked_to: None }, None);
    assert!(dialog_sessions::start_next(true, None).is_some());
    r.frames(3);
    assert!(suppressed(), "a text dialog: the game does not get Enter; {:?}", r.host.drain_logs());
    press(&r.host, "enter");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["dialog_answer"], "Enter presses Done");
    // Escape leaves (the game's window does not open); the claims end with the window
    action_broker::reset_all_for_test();
    dialog_sessions::take_passes();
    press(&r.host, "escape");
    assert!(dialog_sessions::take_passes().is_empty());
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["close_window"]);
    ws::reset_for_test();
    r.frames(3);
    assert!(!suppressed(), "the dialog is gone: Enter opens the chat line again");
    drop(r);
    // a dialog with answers: Enter answers nothing and still never reaches the game
    let mut r = rig("eqoa_dialogs");
    r.frames(2);
    r.dialog(21, "What do you seek?", &["Rumors", "Work"]);
    r.frames(3);
    assert!(suppressed(), "a response dialog: the game does not get Enter");
    press(&r.host, "enter");
    assert!(action_broker::queued_kinds_for_test().is_empty(), "Enter answers nothing there");
    press(&r.host, "1");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["dialog_answer"], "the number key still answers");
    // turning the mod off leaves nothing claimed
    native_events::clear(&r.host);
    assert!(!suppressed(), "disabled: Enter is the game's");
    assert!(r.host.drain_logs().unwrap().iter().all(|l| !l.contains("could not hold")));
}

/// Owner 2026-11-25: drawing a plain text dialog read `data.options`, a field only a response dialog has, and the
/// Addons window showed `eqoa.windows.SessionData.options [unknown_symbol]`. Each kind the mod holds draws silently.
#[test]
fn a2_dialogs_draw_every_kind_they_hold_without_an_unknown_field_diagnostic() {
    let noise = |r: &mut Rig| -> Vec<String> {
        r.host.drain_logs().unwrap().into_iter().filter(|l| l.contains("unknown_symbol") || l.contains("SessionData")).collect()
    };
    let mut r = rig("eqoa_dialogs");
    r.frames(3);
    dialog_sessions::hold(Item::Text { text: wide("Hail, traveller."), talked_to: None }, None);
    assert!(dialog_sessions::start_next(true, None).is_some());
    assert!(r.frames(3) > 0, "the text dialog is drawn");
    let seen = noise(&mut r);
    assert!(seen.is_empty(), "a text dialog drew with diagnostics: {seen:?}");
    drop(r);
    let mut r = rig("eqoa_dialogs");
    r.frames(3);
    r.dialog(1, "Well?", &["One", "Two"]);
    assert!(r.frames(3) > 0, "the response dialog is drawn");
    let seen = noise(&mut r);
    assert!(seen.is_empty(), "a response dialog drew with diagnostics: {seen:?}");
}

/// Owner's test 2026-11-27: the Dialogs box lay over the round minimap's ring in the top right. The box (a dialog, and an
/// offer) stays clear of the minimap's rectangle (`eqoa.hud.layout().minimap`, window pixels) at 100% and at other HUD
/// scales, in the PC interface and in the Controller's. Control: with the minimap off the box keeps its old centred place, and
/// that place does overlap the minimap's rectangle for the 360-wide dialog at 100% (so the check can fail), and the fix moves nothing then.
#[test]
fn a2_dialog_window_stays_clear_of_the_minimap_in_both_interfaces_at_any_hud_scale() {
    // one more frame, and the vertices it drew: the box's bounds (the layer draws nothing else)
    fn bounds(r: &mut Rig) -> Option<[f32; 4]> {
        r.context.frame();
        r.bridge
            .invoke(|| {
                native_events::dispatch(&r.host)?;
                native_events::render(&r.host)
            })
            .unwrap();
        let draw = r.context.render_legacy();
        let mut b: Option<[f32; 4]> = None;
        for list in draw.draw_data().draw_lists() {
            for v in list.vtx_buffer() {
                let c = b.get_or_insert([v.pos[0], v.pos[1], v.pos[0], v.pos[1]]);
                *c = [c[0].min(v.pos[0]), c[1].min(v.pos[1]), c[2].max(v.pos[0]), c[3].max(v.pos[1])];
            }
        }
        b
    }
    let overlaps = |a: [f32; 4], m: [f32; 4]| a[0] < m[0] + m[2] && a[2] > m[0] && a[1] < m[1] + m[3] && a[3] > m[1];
    for pc in [true, false] {
        for scale in [100u8, 150, 70] {
            for offer in [false, true] {
                crate::settings::testing::reset(false);
                crate::settings::set_ui_mode(pc).unwrap();
                crate::settings::set_hud_scale_percent(scale);
                super::hud_layout::set_window_size(1024, 768);
                super::hud_layout::set_hud_state(true, true, false, 93, 0);
                let mut r = rig("eqoa_dialogs");
                r.frames(2);
                let minimap = |r: &Rig| -> [f32; 4] {
                    let v: Vec<f32> = r.host.lua.load("local m = eqoa.hud.layout().minimap return {m.x, m.y, m.width, m.height}").eval::<Vec<f32>>().unwrap();
                    [v[0], v[1], v[2], v[3]]
                };
                let show = |r: &mut Rig| {
                    if offer {
                        dialog_sessions::hold(Item::Offer { summons: true, name: "Mage".into(), text: wide("Mage summons you, and says a long thing about it besides.") }, None);
                    } else {
                        r.dialog(30, "Your account has no verified e-mail address. Please verify it on the website.", &["Done"]);
                    }
                    r.frames(3);
                };
                // control: minimap off: the box is at its old place, which lies over the minimap's rectangle
                CORE.with(|c| c.borrow_mut().view.map = json!({"enabled": true, "open": false, "minimap": false, "markers_on": true, "world_id": 1, "revision": 1}));
                show(&mut r);
                let m = minimap(&r);
                assert!(m[2] > 0.0, "pc={pc} scale={scale}: the layout has a minimap rectangle {m:?}");
                let old = bounds(&mut r).expect("the box is drawn");
                if !offer && scale == 100 {
                    assert!(overlaps(old, m), "pc={pc} scale={scale}: control: the dialog's old place {old:?} is over the minimap {m:?}");
                }
                drop(r);
                // the minimap on: the box is clear of it
                let mut r = rig("eqoa_dialogs");
                r.frames(2);
                CORE.with(|c| c.borrow_mut().view.map = json!({"enabled": true, "open": false, "minimap": true, "markers_on": true, "world_id": 1, "revision": 2}));
                show(&mut r);
                let m = minimap(&r);
                let now = bounds(&mut r).expect("the box is drawn");
                assert!(!overlaps(now, m), "pc={pc} scale={scale} offer={offer}: the box {now:?} is clear of the minimap {m:?}");
                assert!(now[0] >= 0.0 && now[2] <= 1024.0 && now[1] >= 0.0 && now[3] <= 768.0, "and on the screen: {now:?}");
            }
        }
    }
    crate::settings::testing::reset(false);
}
