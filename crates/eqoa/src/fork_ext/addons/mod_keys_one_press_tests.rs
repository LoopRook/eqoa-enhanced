// LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE).
// 2026-11-27, the owner's rule: in a mod's window one press of a key does one thing. Esc (or the x) closes or leaves THAT
// window and nothing else happens; a key typed into a text box is that text box's and nobody else's. Night audit Job B,
// findings H1 (keys typed into one mod's text box reached every mod), H2 (one mod could hold Escape, so one Esc closed every
// window, and the window left after another closed let the next Esc through to the Start menu), L3 (Social: Esc while
// typing closed the whole window) and L1 (Chat: the game's chat-history key scrolled the hidden game chat, not the mod's).
// Included by click_through_tests.rs: its two real windows (Spellbook behind, Quest Journal in front) and its frame loop.
//
// Every key goes through `Runtime::scripted_key`, the same `route_key` -> `route_mod_key_based` path a window key takes,
// and the mods are the real bundled ones. Negative control for each test: the client's routing before this change
// (`input.key` told to every mod; `eqoa.input.claim("Escape")` refused `busy` to a second window) fails the assertion
// named in the test's comment.

use super::{
    action_broker as keys_broker,
    dialog_sessions::{self as keys_dialogs, Item as KeysItem},
    input as keys_input, native_a6 as keys_a6,
    native_core::{self as keys_core, CORE as KEYS_CORE},
    runtime::{Paths as KeysPaths, Runtime as KeysRuntime},
};
use winit::keyboard::{Key as KeysKey, NamedKey as KeysNamed};

fn keys_setup() -> Context {
    let context = setup();
    keys_input::reset_input_state();
    keys_input::set_modifiers((false, false, false));
    keys_core::fixture_demand(true);
    super::options::reset_in_memory();
    keys_dialogs::reset_for_test();
    context
}

fn keys_runtime() -> KeysRuntime {
    KeysRuntime::new(KeysPaths::select(PathBuf::new(), None, None, None))
}

/// One key down and up through the runtime, as the window gives it; `true` when the press was kept from the game.
fn keys_press(rt: &mut KeysRuntime, key: &KeysKey, mods: &[&Mod], context: &mut Context) -> bool {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(7000);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let kept = rt.scripted_key(id, key, None, winit::event::ElementState::Pressed, true);
    rt.scripted_key(id, key, None, winit::event::ElementState::Released, true);
    keys_core::flush_deferred();
    frames(mods, context, 2);
    kept
}

fn keys_escape(rt: &mut KeysRuntime, mods: &[&Mod], context: &mut Context) -> bool {
    keys_press(rt, &KeysKey::Named(KeysNamed::Escape), mods, context)
}

fn keys_type(rt: &mut KeysRuntime, text: &str, mods: &[&Mod], context: &mut Context) {
    for c in text.chars() {
        let key = if c == ' ' { KeysKey::Named(KeysNamed::Space) } else { KeysKey::Character(c.to_string().into()) };
        keys_press(rt, &key, mods, context);
    }
}

/// A bundled mod as the client finds it (its manifest's `replaces` and `provides` included: Chat and Dialogs hold windows).
fn keys_bundled(id: &str) -> Mod {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").canonicalize().unwrap();
    let addon = super::discovery::discover(&dir).addons.into_iter().find(|a| a.id == id).unwrap_or_else(|| panic!("{id} is bundled"));
    Mod::start(addon, None)
}

fn keys_str(m: &Mod, code: &str) -> String {
    m.host.lua.load(format!("return tostring({code})")).eval().unwrap()
}

/// A scratch mod (a third-party one) that writes down every `input.key` it hears, as "<key>+" (down) and "<key>-" (up),
/// and may claim keys.
fn keys_listener(name: &str) -> Mod {
    let dir = std::env::temp_dir().join(format!("eqoa-one-press-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("main.lua"),
        "heard = {}\neqoa.events.on('input.key', function(e) heard[#heard + 1] = e.data.key .. (e.data.down and '+' or '-') end)\n",
    )
    .unwrap();
    let dir = dir.canonicalize().unwrap();
    let addon = DiscoveredAddon {
        id: name.into(),
        name: name.into(),
        entry: dir.join("main.lua"),
        script_root: dir.clone(),
        package_root: dir.clone(),
        module_dirs: vec![dir.clone()],
        resource_dirs: vec![dir.clone()],
        api: Some("eqoa/1".into()),
        entry_mode: super::discovery::EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: ["eqoa.events.on", "eqoa.input.claim", "eqoa.input.capture_text"].iter().map(|c| c.to_string()).collect(),
        extensions: Default::default(),
        initially_enabled: false,
        mods_root: None,
    };
    Mod::start(addon, Some(dir))
}

fn keys_heard(m: &Mod) -> String {
    let s: String = m.host.lua.load("local s = table.concat(heard, ' '); heard = {}; return s").eval().unwrap();
    s
}

/// H2: with two windows up, one Escape closes the one in front and nothing else; the one behind then holds Escape without
/// asking again, so the next Escape closes it; only then is Escape the game's (the Start menu).
/// Negative control: before, both windows heard the first Escape and closed at once ("the window behind is still open").
#[test]
fn one_press_h2_one_escape_closes_only_the_window_in_front_and_the_next_holds_it() {
    let mut context = keys_setup();
    let (back, front) = two_windows(&mut context);
    let mods = [&back, &front];
    let mut rt = keys_runtime();
    assert!(keys_escape(&mut rt, &mods, &mut context), "the first Escape is kept from the game");
    assert!(ws::get(front.vm, "quest_log").is_none(), "the window in front (opened last) closed");
    assert!(ws::get(back.vm, "spellbook").is_some(), "the window behind is still open");
    assert!(keys_input::is_key_claimed(0x1B), "and holds Escape now, without asking again");
    assert!(keys_escape(&mut rt, &mods, &mut context), "the second Escape is kept from the game too");
    assert!(ws::get(back.vm, "spellbook").is_none(), "it closed the window behind");
    // control: no mod window is up, so Escape is the game's (its Start menu)
    assert!(!keys_input::is_key_claimed(0x1B));
    assert!(!keys_escape(&mut rt, &mods, &mut context), "control: with no mod window up Escape reaches the game");
    for m in mods {
        let logs: Vec<String> = m.host.drain_logs().unwrap().into_iter().filter(|l| !l.contains("[info]")).collect();
        assert!(logs.is_empty(), "no warning: {logs:?}");
    }
}

/// H2, the audit's own steps: Spellbook, then the Quest Journal over it; the Spellbook is closed with its own key. The
/// Journal is still up and holds Escape: the next Escape closes it and does NOT also reach the game (the Start menu).
/// Negative control: before, the Journal's claim was refused `busy` when it opened (the Spellbook held Escape), so once the
/// Spellbook let go nobody held Escape: "the Journal holds Escape" failed and the Escape reached the game.
#[test]
fn one_press_h2_the_window_left_after_another_closes_still_keeps_escape_from_the_start_menu() {
    let mut context = keys_setup();
    let (back, front) = two_windows(&mut context);
    let mods = [&back, &front];
    let mut rt = keys_runtime();
    // the Spellbook's own key closes it (Shift+K)
    h::press(&back.host, "k", "shift+k");
    frames(&mods, &mut context, 3);
    assert!(ws::get(back.vm, "spellbook").is_none(), "control: the Spellbook closed");
    assert!(ws::get(front.vm, "quest_log").is_some(), "the Journal is still up");
    assert!(keys_input::is_key_claimed(0x1B), "the Journal holds Escape");
    assert!(keys_escape(&mut rt, &mods, &mut context), "Escape is kept from the game: no Start menu");
    assert!(ws::get(front.vm, "quest_log").is_none(), "and closed the Journal");
    assert!(!keys_escape(&mut rt, &mods, &mut context), "control: the next Escape is the game's");
}

/// H2, "the one in front is the one last clicked": a click on the window behind brings it to the front, and Escape is then
/// its: it closes, the other stays.
/// Negative control: before, one Escape closed both.
#[test]
fn one_press_h2_a_click_on_the_window_behind_makes_escape_its() {
    let mut context = keys_setup();
    let (back, front) = two_windows(&mut context);
    let mods = [&back, &front];
    let (rb, rf) = (rect(back.vm), rect(front.vm));
    deliver(&mods);
    let only_back = find_point(|x, y| inside(&rb, x, y) && !inside(&rf, x, y));
    assert!(press(only_back.0, only_back.1, &mut context), "the click is the Spellbook's");
    frames(&mods, &mut context, 2);
    let mut rt = keys_runtime();
    assert!(keys_escape(&mut rt, &mods, &mut context));
    assert!(ws::get(back.vm, "spellbook").is_none(), "Escape closed the window clicked last");
    assert!(ws::get(front.vm, "quest_log").is_some(), "and not the other");
    assert!(keys_escape(&mut rt, &mods, &mut context));
    assert!(ws::get(front.vm, "quest_log").is_none(), "the next Escape is the other window's");
}

/// The line of Escape claims, by the API alone: a second window's claim of Escape is granted and holds the key; when it
/// lets go the first holds it again; a claim let go while it waits leaves the line; a mod that stops leaves it too. Every
/// other key keeps one owner (`busy`). The key goes to the holder alone.
/// Negative control: before, the second claim was refused `busy` (the first assertion).
#[test]
fn one_press_h2_escape_claims_wait_in_line_and_other_keys_keep_one_owner() {
    let mut context = keys_setup();
    let a = keys_listener("keys-line-a");
    let b = keys_listener("keys-line-b");
    let mods = [&a, &b];
    let mut rt = keys_runtime();
    a.host.execute("ea = assert(eqoa.input.claim('Escape', true))").unwrap();
    b.host.execute("eb = assert(eqoa.input.claim('Escape', true))").unwrap();
    keys_escape(&mut rt, &mods, &mut context);
    assert_eq!((keys_heard(&a), keys_heard(&b)), (String::new(), "escape+ escape-".to_string()), "the newest claim hears it alone");
    b.host.execute("assert(eqoa.input.release(eb) == true)").unwrap();
    assert!(keys_escape(&mut rt, &mods, &mut context), "the first claim holds it again (suppressed)");
    assert_eq!((keys_heard(&a), keys_heard(&b)), ("escape+ escape-".to_string(), String::new()));
    // a claim let go while it waits in line leaves the line
    b.host.execute("eb = assert(eqoa.input.claim('Escape', true))").unwrap();
    a.host.execute("assert(eqoa.input.release(ea) == true)").unwrap();
    keys_escape(&mut rt, &mods, &mut context);
    assert_eq!((keys_heard(&a), keys_heard(&b)), (String::new(), "escape+ escape-".to_string()));
    b.host.execute("assert(eqoa.input.release(eb) == true); assert(eqoa.input.release(eb) == false)").unwrap();
    assert!(!keys_input::is_key_claimed(0x1B), "nobody holds Escape");
    assert!(!keys_escape(&mut rt, &mods, &mut context), "control: then it is the game's");
    assert_eq!((keys_heard(&a), keys_heard(&b)), ("escape+ escape-".to_string(), "escape+ escape-".to_string()), "and every mod's");
    // every other key has one owner, as before
    a.host.execute("en = assert(eqoa.input.claim('Enter', true))").unwrap();
    b.host.execute("local v, e = eqoa.input.claim('Enter'); assert(v == nil and e.code == 'busy')").unwrap();
    // a mod that stops leaves the line
    a.host.execute("ea = assert(eqoa.input.claim('Escape', true))").unwrap();
    b.host.execute("eb = assert(eqoa.input.claim('Escape', true))").unwrap();
    // the client stops the mod (turned off, or its handler failed): `native_events::clear` lets go of all it held
    native_events::clear(&b.host);
    assert!(keys_escape(&mut rt, &[&a], &mut context), "still kept from the game");
    assert_eq!(keys_heard(&a), "escape+ escape-", "the claim before the stopped mod's holds Escape again");
}

/// H1, the audit's case: the Dialogs box is up (it holds Escape, Enter and the answer digits) and the player types in the
/// Chat mod's line. "brb 2" is the line's: the 2 answers nothing. Escape cancels the line and leaves the conversation open.
/// Control: with the line closed, the same 2 answers the dialog.
/// Negative control: before, the 2 also reached Dialogs and was sent as answer 2 ("the 2 answered nothing" failed), and the
/// Escape also closed the dialog's session.
#[test]
fn one_press_h1_keys_typed_in_the_chat_line_are_the_lines_alone_not_the_dialogs() {
    let mut context = keys_setup();
    keys_a6::set_chat_info(keys_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec!["say"])], reply_to: String::new() });
    let dialogs = keys_bundled("eqoa_dialogs");
    let chat = keys_bundled("eqoa_chat");
    let mods = [&dialogs, &chat];
    frames(&mods, &mut context, 3);
    let wide = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    keys_dialogs::hold(
        KeysItem::Box { id: 10, text: wide("Welcome, traveller. What do you seek?"), options: ["Rumors", "Work", "Nothing"].map(wide).to_vec(), talked_to: None },
        None,
    );
    assert!(keys_dialogs::start_next(true, None).is_some(), "the dialog session started");
    frames(&mods, &mut context, 3);
    assert!(keys_input::is_key_claimed(b'2'), "control: the Dialogs box is up and holds its answer keys");
    // the player opens the Chat mod's line (what `a6_play::chat_entry` does on Enter, T, R or /)
    ws::open_client(chat.vm, "eqoa_chat", "chat_entry", "key", serde_json::json!({"text": "", "channel": "say", "links": []}));
    keys_core::flush_deferred();
    frames(&mods, &mut context, 2);
    assert!(keys_input::is_text_capture_active(), "control: the line holds the keyboard");
    keys_broker::reset_all_for_test();
    let mut rt = keys_runtime();
    keys_type(&mut rt, "brb 2", &mods, &mut context);
    assert_eq!(keys_str(&chat, "chat_probe.state.text"), "brb 2", "the line got every key");
    assert!(keys_broker::queued_kinds_for_test().is_empty(), "the 2 answered nothing: {:?}", keys_broker::queued_kinds_for_test());
    // Escape cancels the line, and only the line
    keys_escape(&mut rt, &mods, &mut context);
    assert!(ws::get(chat.vm, "chat_entry").is_none(), "Escape cancelled the line");
    assert!(keys_broker::queued_kinds_for_test().is_empty(), "the conversation was not left: {:?}", keys_broker::queued_kinds_for_test());
    assert!(keys_input::is_key_claimed(b'2'), "the Dialogs box is still up");
    // control: with no text box up, the same key is the dialog's answer
    assert!(!keys_input::is_text_capture_active());
    keys_type(&mut rt, "2", &mods, &mut context);
    assert_eq!(keys_broker::queued_kinds_for_test(), vec!["dialog_answer"], "control: 2 answers the dialog");
    let logs: Vec<String> = dialogs.host.drain_logs().unwrap().into_iter().filter(|l| !l.contains("[info]")).collect();
    assert!(logs.is_empty(), "no warning from Dialogs (it held every key it asked for): {logs:?}");
}

/// H1 for any mod (a third-party one too): a mod listening to `input.key` hears nothing typed into another mod's text box,
/// and still hears the release of a key it saw go down before the box took the keyboard (W held to walk while a box
/// opens). The mod holding the box hears everything. Control: with no box, it hears every key.
/// Negative control: before, the listener heard "x+ x-" while the other mod's box held the keyboard.
#[test]
fn one_press_h1_another_mods_text_box_keys_are_not_heard_but_a_release_of_a_key_seen_down_is() {
    let mut context = keys_setup();
    let listener = keys_listener("keys-h1-listener");
    let typist = keys_listener("keys-h1-typist");
    let mods = [&listener, &typist];
    let mut rt = keys_runtime();
    let w = KeysKey::Character("w".into());
    // W goes down in plain play: both hear it
    rt.scripted_key(7900, &w, None, winit::event::ElementState::Pressed, true);
    keys_core::flush_deferred();
    deliver(&mods);
    assert_eq!((keys_heard(&listener), keys_heard(&typist)), ("w+".to_string(), "w+".to_string()));
    // the typist's text box takes the keyboard
    keys_input::set_text_capture_for_test(typist.vm, true);
    keys_type(&mut rt, "x", &mods, &mut context);
    assert_eq!(keys_heard(&listener), "", "a key typed into another mod's box is not heard");
    assert_eq!(keys_heard(&typist), "x+ x-", "the box's own mod hears it");
    // W comes up while the box holds the keyboard: everyone who saw it go down sees it come up
    rt.scripted_key(7900, &w, None, winit::event::ElementState::Released, true);
    keys_core::flush_deferred();
    deliver(&mods);
    assert_eq!((keys_heard(&listener), keys_heard(&typist)), ("w-".to_string(), "w-".to_string()), "a release follows its press");
    // control: no box, every mod hears every key
    keys_input::set_text_capture_for_test(typist.vm, false);
    keys_type(&mut rt, "x", &mods, &mut context);
    assert_eq!((keys_heard(&listener), keys_heard(&typist)), ("x+ x-".to_string(), "x+ x-".to_string()), "control");
}

/// L3: in Social, Escape while typing (a tell here) leaves the text box; the window, the tell and its text stay. The next
/// Escape closes the window.
/// Negative control: before, the first Escape closed the whole window and cleared the tell ("the window is still open").
#[test]
fn one_press_l3_social_escape_while_typing_leaves_the_box_not_the_window() {
    let mut context = keys_setup();
    let social = keys_bundled("eqoa_social");
    let mods = [&social];
    frames(&mods, &mut context, 2);
    KEYS_CORE.with(|c| c.borrow_mut().emit("mod.command", serde_json::json!({"from": "test", "command": "open", "args": {"page": "friends"}})));
    frames(&mods, &mut context, 3);
    assert_eq!(keys_str(&social, "social_keys_probe.state.open"), "true", "control: the window opened");
    assert!(keys_input::is_key_claimed(0x1B), "control: it holds Escape");
    // a tell is being typed: its box holds the keyboard (what the focused ImGui field makes the mod ask, each frame)
    social
        .host
        .execute("local s = social_keys_probe.state; s.tell_target = 'Wil'; s.tell_input = 'on my way'; s.capturing_text = true; eqoa.input.capture_text(true)")
        .unwrap();
    let mut rt = keys_runtime();
    let id = 7950;
    assert!(rt.scripted_key(id, &KeysKey::Named(KeysNamed::Escape), None, winit::event::ElementState::Pressed, true), "kept from the game");
    rt.scripted_key(id, &KeysKey::Named(KeysNamed::Escape), None, winit::event::ElementState::Released, true);
    keys_core::flush_deferred();
    deliver(&mods);
    assert_eq!(keys_str(&social, "social_keys_probe.state.open"), "true", "the window is still open");
    assert_eq!(keys_str(&social, "social_keys_probe.state.capturing_text"), "false", "typing stopped: the keyboard is let go");
    assert!(!keys_input::is_text_capture_active());
    assert_eq!(keys_str(&social, "social_keys_probe.state.tell_target"), "Wil", "the tell is still there");
    assert_eq!(keys_str(&social, "social_keys_probe.state.tell_input"), "on my way", "with its text");
    // the next Escape closes the window
    assert!(keys_escape(&mut rt, &mods, &mut context));
    assert_eq!(keys_str(&social, "social_keys_probe.state.open"), "false", "the second Escape closed it");
}

/// L1: with the Chat mod on (the game's chat hidden), the game's chat-history key with Up and Down scrolls the mod's
/// window: the client's `chat.history_scroll` (`active`, `steps`). Control: an event with `active = false` (the game's own
/// chat is up) scrolls nothing.
/// Negative control: before, the mod did not listen to the event and its window did not move.
#[test]
fn one_press_l1_the_chat_history_key_scrolls_the_chat_mods_window() {
    let mut context = keys_setup();
    keys_a6::set_chat_info(keys_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec!["say"])], reply_to: String::new() });
    let chat = keys_bundled("eqoa_chat");
    let mods = [&chat];
    frames(&mods, &mut context, 3);
    let scroll = |steps: i32, active: bool| {
        KEYS_CORE.with(|c| c.borrow_mut().emit("chat.history_scroll", serde_json::json!({"active": active, "steps": steps})));
        deliver(&mods);
        keys_str(&chat, "chat_probe.state.scroll")
    };
    assert_eq!(scroll(1, false), "0", "control: the game's own chat is up: the mod's window does not move");
    assert_eq!(scroll(1, true), "1", "Up with the history key held scrolls back a row");
    assert_eq!(scroll(1, true), "2");
    assert_eq!(scroll(-1, true), "1", "Down scrolls forward");
    assert_eq!(scroll(-1, true), "0");
    assert_eq!(scroll(-1, true), "0", "never past the newest line");
}
