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
    keys_scratch(
        name,
        "heard = {}\neqoa.events.on('input.key', function(e) heard[#heard + 1] = e.data.key .. (e.data.down and '+' or '-') end)\n",
        &[],
    )
}

/// A scratch mod with source `source`, allowed the input calls and `more` besides.
fn keys_scratch(name: &str, source: &str, more: &[&str]) -> Mod {
    let dir = std::env::temp_dir().join(format!("eqoa-one-press-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.lua"), source).unwrap();
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
        requested_capabilities: ["eqoa.events.on", "eqoa.input.claim", "eqoa.input.capture_text"]
            .iter()
            .chain(more)
            .map(|c| c.to_string())
            .collect(),
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

/// A key given to ImGui (as the viewer gives a key to the mods' ImGui: `runtime::queue_scripted_key`, the same queue the
/// runtime fills when it has a window) and to the mods (`Runtime::scripted_key` -> `route_key`), down and up; then frames.
fn keys_both(rt: &mut KeysRuntime, key: &KeysKey, text: Option<&str>, mods: &[&Mod], context: &mut Context) {
    super::runtime::queue_scripted_key(context, key, text, true);
    rt.scripted_key(7800, key, None, winit::event::ElementState::Pressed, true);
    super::runtime::queue_scripted_key(context, key, None, false);
    rt.scripted_key(7800, key, None, winit::event::ElementState::Released, true);
    keys_core::flush_deferred();
    frames(mods, context, 2);
}

/// The rectangle ImGui drew window `name` in last frame: `[x, y, w, h]`.
fn keys_window_rect(name: &str) -> [f32; 4] {
    let cname = std::ffi::CString::new(name).unwrap();
    unsafe {
        let w = dear_imgui_rs::sys::igFindWindowByName(cname.as_ptr());
        assert!(!w.is_null(), "{name} was drawn");
        [(*w).Pos.x, (*w).Pos.y, (*w).Size.x, (*w).Size.y]
    }
}

/// L3 and review point 4, on the real route: Social opened with its own key (Shift+F), its "add a friend" box given the
/// keyboard by ImGui itself (Tab into the window's text field, as the player's click would), the Dialogs box up beside it.
/// Typing for more than a second keeps the keyboard (the hold is asked every frame): a "2" typed after that second is the
/// box's, not the dialog's answer. Escape then leaves the box (ImGui lets go of the field, the mod lets go of the
/// keyboard) and keeps the window; the next Escape closes it.
/// Negative controls: with Social asking for the keyboard only when typing starts, the hold lapses after a second and the
/// "2" answers the dialog; with the old Escape handler the first Escape closes the window.
#[test]
fn one_press_l3_social_typing_keeps_the_keyboard_and_escape_leaves_the_box_not_the_window() {
    let mut context = keys_setup();
    let dialogs = keys_bundled("eqoa_dialogs");
    let social = keys_bundled("eqoa_social");
    let mods = [&dialogs, &social];
    frames(&mods, &mut context, 3);
    let wide = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    keys_dialogs::hold(
        KeysItem::Box { id: 11, text: wide("Well met. What brings you?"), options: ["Rumors", "Work", "Nothing"].map(wide).to_vec(), talked_to: None },
        None,
    );
    assert!(keys_dialogs::start_next(true, None).is_some());
    frames(&mods, &mut context, 3);
    h::press(&social.host, "f", "shift+f");
    frames(&mods, &mut context, 3);
    assert_eq!(keys_str(&social, "social_keys_probe.state.open"), "true", "control: Shift+F opened Social");
    // the player clicks Social's window, then Tab puts ImGui's keyboard into its first text field (the friend's name)
    let r = keys_window_rect("Social##eqoa_social");
    context.io_mut().add_mouse_pos_event([r[0] + 40.0, r[1] + 8.0]);
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
    frames(&mods, &mut context, 1);
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
    frames(&mods, &mut context, 1);
    let mut rt = keys_runtime();
    keys_both(&mut rt, &KeysKey::Named(KeysNamed::Tab), None, &mods, &mut context);
    assert!(keys_input::is_text_capture_active(), "control: the friend's name box has the keyboard");
    keys_broker::reset_all_for_test();
    for c in ["W", "i", "l"] {
        keys_both(&mut rt, &KeysKey::Character(c.into()), Some(c), &mods, &mut context);
    }
    // more than a second of typing (frames drawn all the while, as the game draws them)
    for _ in 0..12 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        frames(&mods, &mut context, 1);
    }
    assert!(keys_input::is_text_capture_active(), "the box still holds the keyboard after a second");
    keys_both(&mut rt, &KeysKey::Character("2".into()), Some("2"), &mods, &mut context);
    assert!(keys_broker::queued_kinds_for_test().is_empty(), "the 2 is the box's, not the dialog's answer: {:?}", keys_broker::queued_kinds_for_test());
    assert_eq!(keys_str(&social, "social_keys_probe.state.friend_input"), "Wil2", "ImGui's box got the letters");
    // Escape: the box lets go, the window stays
    keys_both(&mut rt, &KeysKey::Named(KeysNamed::Escape), None, &mods, &mut context);
    assert_eq!(keys_str(&social, "social_keys_probe.state.open"), "true", "the window is still open");
    assert!(!keys_input::is_text_capture_active(), "typing stopped: the keyboard is let go");
    assert!(keys_broker::queued_kinds_for_test().is_empty(), "and the conversation was not left");
    keys_both(&mut rt, &KeysKey::Named(KeysNamed::Escape), None, &mods, &mut context);
    assert_eq!(keys_str(&social, "social_keys_probe.state.open"), "false", "the second Escape closed Social");
    assert!(keys_broker::queued_kinds_for_test().is_empty(), "and only Social: {:?}", keys_broker::queued_kinds_for_test());
}

/// L1, on the real route: the Chat mod on (it hides the game's chat, published as the runtime does), the game's own
/// chat-history key held (`Play::chat_history_key(99)`, C, mode 33) and Up/Down pressed through the game's own key code:
/// the mod's window scrolls a row each. Control: Up without the history key held scrolls nothing.
/// Negative control: before, the mod did not listen to `chat.history_scroll` and its window did not move.
#[test]
fn one_press_l1_the_chat_history_key_scrolls_the_chat_mods_window() {
    super::big_stack(one_press_l1_body)
}
fn one_press_l1_body() {
    let mut context = keys_setup();
    keys_a6::set_chat_info(keys_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec!["say"])], reply_to: String::new() });
    let claims = std::rc::Rc::new(std::cell::RefCell::new(super::visibility::HudClaimsRegistry::new()));
    let (chat, _lease) = keys_hud_mod("eqoa_chat", &claims);
    let mods = [&chat];
    frames(&mods, &mut context, 3);
    // the game made first (its fixture publishes the HUD as it starts), then the mod's hide published as the runtime does
    let mut play = crate::play::Play::bundled_ui_fixture();
    keys_core::fixture_demand(true);
    KEYS_CORE.with(|c| c.borrow_mut().view.in_world = true);
    play.mode = 0;
    // forty lines of chat, so there is something to scroll back through (the window keeps its scroll within its lines)
    for i in 0..40 {
        keys_core::observe_text(&format!("Dab: line {i}"), [1.0; 4]);
    }
    keys_core::flush_deferred();
    frames(&mods, &mut context, 1);
    super::visibility::publish_claims(&claims.borrow());
    assert!(!crate::hud::fork_hud::chat_window_visible(), "control: the mod hides the game's chat");
    let told = || KEYS_CORE.with(|c| c.borrow().stream_counts.get("chat.history_scroll").copied().unwrap_or(0));
    let last_told = || {
        KEYS_CORE.with(|c| {
            c.borrow().events.iter().rev().find(|e| e.name == "chat.history_scroll").map(|e| e.data.to_string()).unwrap_or_default()
        })
    };
    let scroll = |play: &mut crate::play::Play, key: i32, context: &mut Context| {
        play.chat_history_key(key, true);
        play.chat_history_key(key, false);
        keys_core::flush_deferred();
        frames(&mods, context, 1);
        keys_str(&chat, "chat_probe.state.scroll")
    };
    let base = told();
    assert_eq!(scroll(&mut play, 19, &mut context), "0", "control: Up without the history key held scrolls nothing");
    assert_eq!(told(), base, "control: and the game told the mods nothing");
    play.chat_history_key(99, true);
    assert_eq!(play.mode, 33, "control: C held is the game's chat history");
    let up = scroll(&mut play, 19, &mut context);
    // each link of the route said apart, so a failure names where it stopped
    assert_eq!(told(), base + 1, "the game's history key told the mods (`chat.history_scroll`)");
    assert!(last_told().contains("\"active\":true"), "with the game's chat hidden: {}", last_told());
    assert_eq!(up, "1", "Up with the history key held scrolls back a row");
    assert_eq!(scroll(&mut play, 19, &mut context), "2");
    assert_eq!(scroll(&mut play, 20, &mut context), "1", "Down scrolls forward");
    assert_eq!(scroll(&mut play, 20, &mut context), "0");
    assert_eq!(scroll(&mut play, 20, &mut context), "0", "never past the newest line");
    play.chat_history_key(99, false);
}

/// The Stats Box mod with the HUD claims its `eqoa.hud.hide("stat_window")` goes to (the runtime publishes them each frame).
fn keys_stats_box(claims: &std::rc::Rc<std::cell::RefCell<super::visibility::HudClaimsRegistry>>) -> (Mod, super::visibility::VisibilityLease) {
    keys_hud_mod("eqoa_stats_box", claims)
}

/// Bundled mod `id` with the HUD claims its `eqoa.hud.hide` goes to (the runtime publishes them each frame: `publish_claims`).
fn keys_hud_mod(
    id: &str,
    claims: &std::rc::Rc<std::cell::RefCell<super::visibility::HudClaimsRegistry>>,
) -> (Mod, super::visibility::VisibilityLease) {
    use std::{cell::RefCell, rc::Rc};
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").canonicalize().unwrap();
    let addon = super::discovery::discover(&dir).addons.into_iter().find(|a| a.id == id).unwrap_or_else(|| panic!("{id} is bundled"));
    let host = Host::for_addon(&addon).unwrap();
    let snapshot = crate::play::Play::bundled_ui_fixture().forge_snapshot(None);
    // the fixture asks for the manager's demand; the tests here deliver as the fixture's own demand
    keys_core::fixture_demand(true);
    host.snapshot.set(snapshot);
    let lease = super::visibility::VisibilityLease::with_hud_claims(
        Rc::new(RefCell::new(super::health::HealthBarClaimsRegistry::new())),
        claims.clone(),
    )
    .unwrap();
    lease.update(snapshot.health, true);
    super::visibility::install_managed_health_adapter(&host, &lease).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    (Mod { host, bridge, vm, dir: None }, lease)
}

/// The lead's request (owner's logs 2026-11-27: "stats box shown" then "stats box hidden" in pairs). On the real key route
/// (`Runtime::scripted_key` -> `route_key` -> `input.key` -> the mod's binding; the game's side as the viewer does it:
/// `KeyEdge`, then `Play::toggle_stat_window` for a key sent on as F4), PC interface, the game's Stat Window setting On, the
/// mod's own key given F1 on Controls (Controls is left with none): one press of F1 flips the box exactly once, one press of
/// F4 flips it exactly once, a held key's repeats do not flip it again, the game's Stat Window setting is never touched, and
/// a second mod (a binding with F1 as its default, a `hud.piece_requested` listener) is never fired or told.
/// "Exactly once" is read from the box's switch: every `toggle_stats_box` flips it and writes one "stats box shown/hidden"
/// line, so two calls on one press would leave it as it was (the assertion), three would not be told apart from one.
/// Control: the same F1 with the mod's key unbound does nothing to the box.
#[test]
fn one_press_stats_box_f1_and_f4_each_flip_the_box_once_on_the_real_key_route() {
    super::big_stack(one_press_stats_box_body)
}
fn one_press_stats_box_body() {
    use crate::binds::Sent;
    let mut context = keys_setup();
    crate::settings::testing::reset(false);
    super::options::reset_in_memory();
    crate::fork_ext::bindpage::reset_in_memory();
    crate::settings::set_ui_mode(true).unwrap();
    crate::settings::set_stat_window(true).unwrap();
    let claims = std::rc::Rc::new(std::cell::RefCell::new(super::visibility::HudClaimsRegistry::new()));
    let (stats, _lease) = keys_stats_box(&claims);
    frames(&[&stats], &mut context, 3);
    super::visibility::publish_claims(&claims.borrow());
    assert!(!crate::hud::fork_hud::stat_window_visible(), "control: the mod holds the game's Stat Window");
    let box_on = || super::options::get_toggle("eqoa_stats_box", "stats_box").unwrap_or(false);
    let mut play = crate::play::Play::bundled_ui_fixture();
    let mut rt = keys_runtime();
    let mut edge = crate::binds::KeyEdge::default();
    // one physical press of `name`, with `repeats` system repeats while it is held, as the viewer takes it: the mod runtime
    // first (`route_key`; a repeat goes through the same `route_mod_key_based` with `repeat` set), then the game's keys
    let mut press = |name: &str, repeats: usize, mods: &[&Mod], context: &mut Context| -> (bool, bool) {
        let (box_before, window_before) = (box_on(), crate::settings::stat_window());
        let key = crate::binds::key_of(name);
        let mut controls = false;
        for repeat in std::iter::once(false).chain(std::iter::repeat_n(true, repeats)) {
            if repeat {
                keys_input::route_mod_key(&key, true, true, false, true, false);
            } else {
                rt.scripted_key(7990, &key, None, winit::event::ElementState::Pressed, true);
            }
            edge.pc_mode = true;
            match crate::settings::with_binds(|b| edge.key(name, true, repeat, false, b)) {
                // the viewer's F4 (`statplay`), and F1 the Controls page, each on a press that is not a repeat
                Some(Sent::Key(to)) if to == "f4" && !repeat => play.toggle_stat_window(),
                Some(Sent::Key(to)) if to == "f1" && !repeat => controls = true,
                _ => {}
            }
            keys_core::flush_deferred();
            frames(mods, context, 1);
        }
        rt.scripted_key(7990, &key, None, winit::event::ElementState::Released, true);
        crate::settings::with_binds(|b| edge.key(name, false, false, false, b));
        keys_core::flush_deferred();
        frames(mods, context, 2);
        super::visibility::publish_claims(&claims.borrow());
        assert_eq!(crate::settings::stat_window(), window_before, "{name}: the game's own Stat Window setting is not touched");
        (box_on() != box_before, controls)
    };
    // control: the mod's key unbound, F1 is the Controls page's and does nothing to the box
    assert_eq!(press("f1", 0, &[&stats], &mut context), (false, true), "control: F1 is Controls' while the mod's key is unbound");
    // the player gives the mod's row F1 on Controls (the mod's row is the first after the game's)
    let (mut capture, mut note, mut cues) = (Some(crate::bindpage::Column::Keyboard), None, Vec::new());
    assert!(crate::fork_ext::bindpage::capture_key(&mut capture, &mut note, crate::binds::ROWS.len(), "f1", &mut cues), "{note:?}");
    assert_eq!(crate::fork_ext::bindpage::get_key("eqoa_stats_box", "toggle").as_deref(), Some("f1"));
    // a second mod: a binding that would like F1, and a listener for the game's Stat Window key
    let other = keys_scratch(
        "keys-stats-other",
        "fired, told = 0, 0\n\
         eqoa.input.bind({id = 'other', label = 'Other', default_key = 'f1'}, function() fired = fired + 1 end)\n\
         eqoa.events.on('hud.piece_requested', function() told = told + 1 end)\n",
        &["eqoa.input.bind"],
    );
    let mods = [&stats, &other];
    frames(&mods, &mut context, 2);
    for round in 0..2 {
        assert_eq!(press("f1", 0, &mods, &mut context), (true, false), "round {round}: one press of F1 flips the box once");
        assert_eq!(press("f4", 0, &mods, &mut context), (true, false), "round {round}: one press of F4 flips the box once");
        assert_eq!(press("f1", 3, &mods, &mut context), (true, false), "round {round}: F1 held (3 repeats) flips it once");
        assert_eq!(press("f4", 3, &mods, &mut context), (true, false), "round {round}: F4 held (3 repeats) flips it once");
    }
    let (fired, told): (i64, i64) = other.host.lua.load("return fired, told").eval().unwrap();
    assert_eq!((fired, told), (0, 0), "the second mod's binding never fired and it was never told of the Stat Window key");
    let logs: Vec<String> = stats.host.drain_logs().unwrap().into_iter().filter(|l| !l.contains("[info]")).collect();
    assert!(logs.is_empty(), "{logs:?}");
    crate::settings::testing::reset(false);
}

// ---- review of the branch (2026-11-27): points 1, 2, 3, 5 and 8 ------------------------------------------------------

/// A scratch window mod: claims Escape when it opens (`open()`), closes on Escape and releases it, and draws a plain ImGui
/// window (no `claim_pointer`) at `x, y`. `fail` makes its Escape handler throw ("key") or its drawing throw ("render").
fn keys_window_mod(name: &str, x: f32, y: f32, fail: &str) -> Mod {
    let source = format!(
        "local ui = eqoa.imgui\n\
         open, closed = false, 0\n\
         function open_window() open = true; esc = assert(eqoa.input.claim('Escape', true)) end\n\
         eqoa.events.on('render', function()\n\
           if not open then return end\n\
           if '{fail}' == 'render' then error('drawing failed') end\n\
           ui.SetNextWindowPos({x}, {y}, ImGuiCond.Always)\n\
           ui.SetNextWindowSize(180, 120, ImGuiCond.Always)\n\
           ui.Begin('Plain##{name}', true, 0)\n\
           ui.End()\n\
         end)\n\
         eqoa.events.on('input.key', function(e)\n\
           if not (open and e.data.down and e.data.key:lower() == 'escape') then return end\n\
           if '{fail}' == 'key' then error('key handling failed') end\n\
           open = false; closed = closed + 1; eqoa.input.release(esc); esc = nil\n\
         end)\n"
    );
    keys_scratch(name, &source, &["eqoa.imgui"])
}

/// A point on the whole ImGui display (1024 x 768) that `keep` accepts, on a coarse grid.
fn keys_find_point(keep: impl Fn(f32, f32) -> bool) -> (f32, f32) {
    for y in (0..768).step_by(4) {
        for x in (0..1024).step_by(4) {
            if keep(x as f32 + 0.5, y as f32 + 0.5) {
                return (x as f32 + 0.5, y as f32 + 0.5);
            }
        }
    }
    panic!("no such point on the screen");
}

fn keys_open(m: &Mod) {
    m.host.execute("open_window()").unwrap();
}

/// Review point 1: a window mod whose Escape handler fails (or whose drawing fails) cannot keep Escape. Its claim ends and the
/// window before it holds Escape: one Escape (the one the failed handler did not act on) is lost, and the next closes the
/// window behind. A claim of a mod that does not listen to `input.key` at all never holds Escape. The player is never left
/// with an Escape that nothing acts on and that does not reach the game.
/// Negative control: before, the failed mod stayed the holder: "the next Escape closes the Spellbook" failed, and every
/// later Escape went nowhere (kept from the game too).
#[test]
fn one_press_review_1_a_failed_window_mod_does_not_keep_escape() {
    let mut context = keys_setup();
    let (back, front) = two_windows(&mut context);
    let mut rt = keys_runtime();
    // the Journal is closed first: the Spellbook alone is up
    assert!(keys_escape(&mut rt, &[&back, &front], &mut context));
    assert!(ws::get(back.vm, "spellbook").is_some() && ws::get(front.vm, "quest_log").is_none());
    // a mod whose key handler fails opens its window over the Spellbook
    let broken = keys_window_mod("keys-broken-key", 30.0, 30.0, "key");
    keys_open(&broken);
    let mods = [&back, &broken];
    frames(&mods, &mut context, 2);
    assert!(keys_escape(&mut rt, &mods, &mut context), "the press its handler failed on is kept from the game");
    assert!(ws::get(back.vm, "spellbook").is_some(), "and did nothing else: the Spellbook is still up");
    assert!(keys_escape(&mut rt, &mods, &mut context), "the next Escape is the Spellbook's");
    assert!(ws::get(back.vm, "spellbook").is_none(), "the next Escape closes the Spellbook");
    // a mod whose drawing fails: its window is gone, so it holds no Escape from then on
    keys_runtime_reopen_spellbook(&back, &mut context);
    let drawless = keys_window_mod("keys-broken-render", 30.0, 30.0, "render");
    keys_open(&drawless);
    let mods = [&back, &drawless];
    frames(&mods, &mut context, 2);
    assert!(keys_escape(&mut rt, &mods, &mut context));
    assert!(ws::get(back.vm, "spellbook").is_none(), "one Escape closes the Spellbook: the window that failed to draw holds none");
    // a claim of a mod that never listens to `input.key` holds nothing
    keys_runtime_reopen_spellbook(&back, &mut context);
    let deaf = keys_scratch("keys-deaf", "esc = assert(eqoa.input.claim('Escape', true))\n", &[]);
    let mods = [&back, &deaf];
    frames(&mods, &mut context, 2);
    assert!(keys_escape(&mut rt, &mods, &mut context));
    assert!(ws::get(back.vm, "spellbook").is_none(), "the Spellbook's, not the claim of a mod that cannot hear Escape");
    assert!(!keys_escape(&mut rt, &mods, &mut context), "control: then Escape is the game's: the player is never locked out");
}

fn keys_runtime_reopen_spellbook(back: &Mod, context: &mut Context) {
    h::press(&back.host, "k", "shift+k");
    frames(&[back], context, 3);
    assert!(ws::get(back.vm, "spellbook").is_some(), "control: the Spellbook is up again");
}

/// Review points 5 and 8: one claim of Escape per mod. A mod that asks again while it holds one gets the same claim back
/// (equal tokens; one release ends it), its place in the line unchanged (a claim made every frame does not jump in front of
/// a window opened later); and a window that claims Escape without `suppress` in front of one that asked for it does not
/// let Escape through to the game.
/// Negative control: before, every call added a claim and moved it to the front ("the later window still holds Escape"
/// failed), and the front claim's `suppress = false` let Escape reach the game.
#[test]
fn one_press_review_5_one_escape_claim_per_mod_and_escape_kept_from_the_game_while_any_window_asks() {
    let mut context = keys_setup();
    let a = keys_listener("keys-once-a");
    let b = keys_listener("keys-once-b");
    let mods = [&a, &b];
    let mut rt = keys_runtime();
    a.host.execute("ea = assert(eqoa.input.claim('Escape', true))").unwrap();
    b.host.execute("eb = assert(eqoa.input.claim('Escape'))").unwrap();
    for _ in 0..50 {
        a.host.execute("local again = assert(eqoa.input.claim('Escape', true)); assert(again == ea, 'the same claim')").unwrap();
    }
    assert!(keys_escape(&mut rt, &mods, &mut context), "kept from the game: the window behind asked for that");
    assert_eq!((keys_heard(&a), keys_heard(&b)), (String::new(), "escape+ escape-".to_string()), "the later window still holds Escape");
    b.host.execute("assert(eqoa.input.release(eb) == true)").unwrap();
    keys_escape(&mut rt, &mods, &mut context);
    assert_eq!(keys_heard(&a), "escape+ escape-", "then the first");
    a.host.execute("assert(eqoa.input.release(ea) == true); assert(eqoa.input.release(ea) == false)").unwrap();
    assert!(!keys_input::is_key_claimed(0x1B), "one release ended the one claim: nothing is left in the line");
    assert!(!keys_escape(&mut rt, &mods, &mut context), "control: the game's");
}

/// Review point 2: a click on a mod's plain ImGui window (no `claim_pointer`: a third-party mod's, or Social's) gives that
/// window Escape, as a click on a claimed window does. The plain window is opened first, the Spellbook over it; the click
/// lands on the plain window where the Spellbook does not cover it; Escape then closes the plain window and not the
/// Spellbook.
/// Negative control: before, only a claimed window's click moved Escape: Escape closed the Spellbook.
#[test]
fn one_press_review_2_a_click_on_a_plain_imgui_window_gives_its_mod_escape() {
    let mut context = keys_setup();
    let plain = keys_window_mod("keys-plain", 20.0, 20.0, "");
    keys_open(&plain);
    let back = Mod::real("eqoa_spellbook");
    let mods = [&plain, &back];
    frames(&mods, &mut context, 2);
    h::press(&back.host, "k", "shift+k");
    frames(&mods, &mut context, 3);
    assert!(ws::get(back.vm, "spellbook").is_some(), "control: the Spellbook opened last");
    let (rp, rs) = (keys_window_rect("Plain##keys-plain"), rect(back.vm));
    let at = keys_find_point(|x, y| inside(&rp, x, y) && !inside(&rs, x, y));
    press(at.0, at.1, &mut context);
    frames(&mods, &mut context, 2);
    let mut rt = keys_runtime();
    assert!(keys_escape(&mut rt, &mods, &mut context));
    assert_eq!(keys_str(&plain, "closed"), "1", "Escape closed the plain window clicked last");
    assert!(ws::get(back.vm, "spellbook").is_some(), "and not the Spellbook");
}

/// Review point 2 with the bundled Social (a real ImGui window): Social, then the Spellbook; a click on Social; Escape closes
/// Social, the Spellbook stays. Control: without the click, Escape closes the Spellbook (opened last).
#[test]
fn one_press_review_2_a_click_on_social_gives_it_escape() {
    for click in [false, true] {
        let mut context = keys_setup();
        let social = keys_bundled("eqoa_social");
        let back = Mod::real("eqoa_spellbook");
        let mods = [&social, &back];
        frames(&mods, &mut context, 2);
        h::press(&social.host, "f", "shift+f");
        frames(&mods, &mut context, 3);
        h::press(&back.host, "k", "shift+k");
        frames(&mods, &mut context, 3);
        assert_eq!(keys_str(&social, "social_keys_probe.state.open"), "true");
        assert!(ws::get(back.vm, "spellbook").is_some());
        if click {
            let (rso, rs) = (keys_window_rect("Social##eqoa_social"), rect(back.vm));
            let at = keys_find_point(|x, y| inside(&rso, x, y) && !inside(&rs, x, y));
            press(at.0, at.1, &mut context);
            frames(&mods, &mut context, 2);
        }
        let mut rt = keys_runtime();
        assert!(keys_escape(&mut rt, &mods, &mut context));
        let (social_open, book_open) = (keys_str(&social, "social_keys_probe.state.open") == "true", ws::get(back.vm, "spellbook").is_some());
        if click {
            assert_eq!((social_open, book_open), (false, true), "the click on Social gave it Escape");
        } else {
            assert_eq!((social_open, book_open), (true, false), "control: without a click the Spellbook (opened last) has Escape");
        }
    }
}

/// Review point 3: the Chat mod's tab menu (a right click on a tab) holds Escape while it is up: with the Spellbook open
/// behind, one Escape closes the menu only; the next closes the Spellbook; the next is the game's.
/// Negative control: before, the menu held nothing: the first Escape closed the Spellbook and left the menu up.
#[test]
fn one_press_review_3_the_chat_tab_menu_holds_escape_while_it_is_up() {
    let mut context = keys_setup();
    keys_a6::set_chat_info(keys_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec!["say"])], reply_to: String::new() });
    let back = Mod::real("eqoa_spellbook");
    let chat = keys_bundled("eqoa_chat");
    let mods = [&back, &chat];
    frames(&mods, &mut context, 2);
    h::press(&back.host, "k", "shift+k");
    frames(&mods, &mut context, 3);
    assert!(ws::get(back.vm, "spellbook").is_some());
    let (x, y): (f32, f32) = chat.host.lua.load("local h = chat_probe.hit('tab:say'); return h.x + h.w / 2, h.y + h.h / 2").eval().unwrap();
    press_button(x, y, MouseButton::Right, &mut context);
    frames(&mods, &mut context, 2);
    assert_eq!(keys_str(&chat, "chat_probe.state.menu ~= nil"), "true", "control: the right click opened the tab menu");
    let mut rt = keys_runtime();
    assert!(keys_escape(&mut rt, &mods, &mut context));
    assert_eq!(keys_str(&chat, "chat_probe.state.menu ~= nil"), "false", "Escape closed the menu");
    assert!(ws::get(back.vm, "spellbook").is_some(), "and nothing else");
    assert!(keys_escape(&mut rt, &mods, &mut context));
    assert!(ws::get(back.vm, "spellbook").is_none(), "the next Escape is the Spellbook's");
    assert!(!keys_escape(&mut rt, &mods, &mut context), "control: then the game's");
}
