// LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE).
//! Two mods want one game window: the plain message (`window_conflict`). Real mods (a `Host` each, asking through
//! `eqoa.windows.takeover` as a mod does) hold and are refused; the box's words, "once", the Mods & Addons line, and
//! what turning a mod off does are read from the production functions the box and the manager call.
//! Each test names its control: what the same steps show without the change.
use super::{
    a1_mod_tests as h,
    host::Host,
    native_events,
    window_conflict as wc,
    window_takeover::{self, TakeoverKind},
};
use winit::keyboard::{Key, NamedKey};

const TAKEOVER: &[&str] = &["eqoa.windows.takeover", "eqoa.windows.session", "eqoa.events.on"];

fn fresh() {
    h::fixture();
    wc::reset_for_test();
}

fn mod_with(id: &str, replaces: &[&str]) -> Host {
    let host = Host::new().unwrap();
    let sess = super::native::session(&host.lua);
    let mut s = sess.borrow_mut();
    s.addon = id.to_string();
    s.native_manifest = true;
    s.capabilities = Some(TAKEOVER.iter().map(|c| (*c).to_string()).collect());
    s.replaces = replaces.iter().map(|r| (*r).to_string()).collect();
    drop(s);
    host
}

fn names(id: &str) -> String {
    match id {
        "eqoa_dialogs" => "EQOA Dialog Window".into(),
        "turnin" => "Quest Turn-in Helper".into(),
        "third" => "Third Mod".into(),
        other => other.into(),
    }
}

const ASK: &str = "local ok, e = eqoa.windows.takeover('text_dialog', {scope = 'all'}); return ok, e and e.code, e and e.message";

fn ask(host: &Host) -> (bool, Option<String>, Option<String>) {
    host.lua.load(ASK).eval::<(Option<bool>, Option<String>, Option<String>)>().map(|(ok, code, msg)| (ok == Some(true), code, msg)).unwrap()
}

/// The two mods: the first holds the Dialog window, the second is refused.
fn two_mods() -> (Host, Host) {
    let holder = mod_with("eqoa_dialogs", &["text_dialog"]);
    let rival = mod_with("turnin", &["text_dialog"]);
    assert!(ask(&holder).0);
    let (ok, code, msg) = ask(&rival);
    assert!(!ok && code.as_deref() == Some("busy"));
    // the result a mod gets is what it always was, byte for byte
    assert_eq!(msg.as_deref(), Some("text_dialog window is already taken over by 'eqoa_dialogs'"));
    (holder, rival)
}

#[test]
fn the_box_says_who_has_the_window_and_whose_is_not_shown() {
    fresh();
    assert!(!wc::is_open(), "control: nothing asked, no box");
    let (_holder, _rival) = two_mods();
    assert!(wc::is_open());
    assert_eq!(
        wc::box_lines(&names),
        ["Dialog window: EQOA Dialog Window is using it, so Quest Turn-in Helper's is not shown."]
    );
}

#[test]
fn a_mod_that_holds_a_window_or_asks_for_a_free_one_brings_no_box() {
    fresh();
    let holder = mod_with("eqoa_dialogs", &["text_dialog", "map"]);
    assert!(ask(&holder).0);
    assert!(ask(&holder).0, "asking again while holding is no conflict");
    assert!(holder.lua.load("return eqoa.windows.takeover('map', {scope = 'all'})").eval::<bool>().unwrap());
    let other = mod_with("turnin", &["response_dialog"]);
    assert!(other.lua.load("return eqoa.windows.takeover('response_dialog', {scope = 'all'})").eval::<bool>().unwrap());
    assert!(!wc::is_open() && wc::unmet_lines("turnin", &names).is_empty());
}

#[test]
fn it_comes_once_however_often_the_loser_asks_and_again_after_a_change() {
    fresh();
    let (holder, rival) = two_mods();
    for _ in 0..25 {
        assert!(!ask(&rival).0);
    }
    assert_eq!(wc::box_lines(&names).len(), 1, "25 more asks add no line (control: the first ask made one)");
    // OK or Escape closes the box; the same pair does not bring it back
    wc::dismiss();
    assert!(!wc::is_open());
    for _ in 0..5 {
        assert!(!ask(&rival).0);
    }
    assert!(!wc::is_open(), "the same pair asking again is not a new message");
    assert_eq!(wc::unmet_lines("turnin", &names).len(), 1, "but the line in Mods & Addons stays");
    // the holder lets go and the rival holds it, then the holder asks and is refused: a different pair, a new message
    assert!(holder.lua.load("return eqoa.windows.release('text_dialog')").eval::<bool>().unwrap());
    assert!(ask(&rival).0);
    assert!(!ask(&holder).0);
    assert!(wc::is_open(), "a different pair (holder and loser swapped) is told");
    wc::dismiss();
    // something changes (a mod turned on or off): the first pair is told again
    wc::changed();
    assert!(holder.lua.load("return eqoa.windows.release('text_dialog')").eval::<bool>().unwrap() == false);
    assert!(rival.lua.load("return eqoa.windows.release('text_dialog')").eval::<bool>().unwrap());
    assert!(ask(&holder).0);
    assert!(!ask(&rival).0);
    assert!(wc::is_open(), "after a change the same pair is told again");
}

#[test]
fn the_loser_has_a_yellow_line_and_nobody_else_does() {
    fresh();
    let (_holder, _rival) = two_mods();
    assert_eq!(wc::unmet_lines("turnin", &names), ["Did not get the Dialog window: EQOA Dialog Window is using it."]);
    assert!(wc::unmet_lines("eqoa_dialogs", &names).is_empty(), "the holder has none");
    assert!(wc::unmet_lines("third", &names).is_empty(), "an unrelated mod has none");
}

#[test]
fn every_window_a_mod_can_hold_has_a_name_in_the_message() {
    for kind in TakeoverKind::ALL_KINDS {
        let name = wc::window_name(kind);
        assert!(name.ends_with(" window") && name.len() > " window".len(), "{kind:?}: {name}");
        assert!(super::window_kinds::title(kind.name()).is_some(), "{kind:?} has a game title");
        assert!(!name.contains('_'), "{name}");
    }
    assert_eq!(wc::window_name(TakeoverKind::TextDialog), "Dialog window");
    assert_eq!(wc::window_name(TakeoverKind::Loot), "Loot window");
}

#[test]
fn turning_the_holder_off_clears_the_message_and_the_other_mod_takes_the_window() {
    fresh();
    let (holder, rival) = two_mods();
    // the manager asks before it turns the holder off who is waiting for the window
    assert_eq!(wc::losers_of("eqoa_dialogs").iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["turnin"]);
    assert!(wc::losers_of("turnin").is_empty(), "the loser keeps nobody from a window");
    // the production disable path of a mod (`manager::disable`)
    native_events::clear(&holder);
    assert!(!wc::is_open(), "the box is gone");
    assert!(wc::unmet_lines("turnin", &names).is_empty(), "and the yellow line");
    assert!(!window_takeover::is_taken(TakeoverKind::TextDialog), "the window is free");
    assert!(ask(&rival).0, "the other mod takes it as soon as it asks (the manager has it ask at once)");
    assert_eq!(window_takeover::holder_of(TakeoverKind::TextDialog).as_deref(), Some("turnin"));
    assert!(!wc::is_open() && wc::unmet_lines("turnin", &names).is_empty());
}

#[test]
fn turning_the_loser_off_clears_it_too() {
    fresh();
    let (holder, rival) = two_mods();
    native_events::clear(&rival);
    assert!(!wc::is_open() && wc::unmet_lines("turnin", &names).is_empty());
    assert_eq!(window_takeover::holder_of(TakeoverKind::TextDialog).as_deref(), Some("eqoa_dialogs"), "the holder keeps it");
    drop(holder);
}

#[test]
fn a_holder_that_lets_go_by_itself_takes_the_message_with_it() {
    fresh();
    let (holder, _rival) = two_mods();
    assert!(holder.lua.load("return eqoa.windows.release('text_dialog')").eval::<bool>().unwrap());
    assert!(!wc::is_open() && wc::unmet_lines("turnin", &names).is_empty(), "nobody holds it: nothing to say");
}

#[test]
fn several_conflicts_are_one_box_with_a_line_each() {
    fresh();
    let holder = mod_with("eqoa_dialogs", &["text_dialog", "map"]);
    let rival = mod_with("turnin", &["text_dialog", "map"]);
    for kind in ["text_dialog", "map"] {
        assert!(holder.lua.load(format!("return eqoa.windows.takeover('{kind}', {{scope = 'all'}})")).eval::<bool>().unwrap());
        assert!(rival.lua.load(format!("local ok = eqoa.windows.takeover('{kind}', {{scope = 'all'}}); return ok == true")).eval::<bool>().unwrap() == false);
    }
    let lines = wc::box_lines(&names);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("Dialog window:")) && lines.iter().any(|l| l.starts_with("Map window:")));
    assert_eq!(wc::unmet_lines("turnin", &names).len(), 2);
}

fn draw_frame(context: &mut dear_imgui_rs::Context) -> bool {
    wc::begin_frame();
    let ui = context.frame();
    let open_manager = wc::draw(ui, &names);
    drop(context.render_legacy());
    open_manager
}

#[test]
fn escape_closes_the_box_and_only_the_box() {
    use winit::keyboard::SmolStr;
    fresh();
    let mut context = h::context();
    // control: no box, Escape is not the box's
    assert!(!wc::key(&Key::Named(NamedKey::Escape)));
    let (_holder, _rival) = two_mods();
    // the box is up but not on the screen (not yet drawn; or addon drawing stopped): Escape is not taken
    assert!(!wc::key(&Key::Named(NamedKey::Escape)), "a box that is not drawn takes no Escape");
    draw_frame(&mut context);
    assert!(!wc::key(&Key::Named(NamedKey::Enter)), "Enter is not taken: the chat line still opens");
    assert!(!wc::key(&Key::Character(SmolStr::new("a"))));
    assert!(wc::is_open());
    // a frame in which it was not drawn (the world is not showing): Escape is not taken
    wc::begin_frame();
    assert!(!wc::key(&Key::Named(NamedKey::Escape)));
    draw_frame(&mut context);
    assert!(wc::key(&Key::Named(NamedKey::Escape)), "Escape is taken by the box");
    assert!(!wc::is_open());
    assert!(!wc::key(&Key::Named(NamedKey::Escape)), "the next Escape is not the box's");
    assert_eq!(wc::unmet_lines("turnin", &names).len(), 1, "closing the box does not clear the line");
}

#[test]
fn an_unrelated_mod_changing_does_not_bring_back_a_box_that_was_answered() {
    fresh();
    let (holder, rival) = two_mods();
    wc::dismiss();
    // an unrelated mod is turned on or off (the manager says so by id)
    wc::changed_for("third");
    // a zone change: every claim is let go and both mods claim again
    window_takeover::release_all();
    assert!(ask(&holder).0);
    assert!(!ask(&rival).0);
    assert!(!wc::is_open(), "control: the pair did not change, so it is not told again");
    // a mod of the pair changes: told again
    wc::changed_for("turnin");
    window_takeover::release_all();
    assert!(ask(&holder).0);
    assert!(!ask(&rival).0);
    assert!(wc::is_open(), "a change to one of the pair brings it back");
    // a new session or profile forgets everything
    wc::dismiss();
    wc::changed();
    window_takeover::release_all();
    assert!(ask(&holder).0);
    assert!(!ask(&rival).0);
    assert!(wc::is_open());
}

#[test]
fn a_mod_in_the_middle_of_something_is_not_reloaded() {
    fresh();
    let (_holder, rival) = two_mods();
    let vm = native_events::bus(&rival.lua).borrow().vm;
    assert!(!wc::mid_use(vm), "control: a mod with nothing up can be reloaded");
    super::input::set_text_capture_for_test(vm, true);
    assert!(wc::mid_use(vm), "typing into its text box");
    super::input::set_text_capture_for_test(vm, false);
    assert!(!wc::mid_use(vm));
    let _s = super::window_sessions::open_client(vm, "turnin", "map", "key", serde_json::json!({}));
    assert!(wc::mid_use(vm), "a window of its own up");
    super::window_sessions::end_vm(vm);
    assert!(!wc::mid_use(vm));
}

#[test]
fn a_long_name_and_many_conflicts_keep_the_box_short() {
    fresh();
    let long = |_: &str| "A".repeat(100);
    let (_holder, _rival) = two_mods();
    let line = wc::box_lines(&long).remove(0);
    assert!(!line.contains(&"A".repeat(33)), "{line}");
    assert!(line.contains('\u{2026}'));
    // seven windows at once: five lines and a count
    let a = mod_with("a", &[]);
    let b = mod_with("b", &[]);
    for kind in ["loot", "merchant", "bank", "map", "trade", "duel"] {
        assert!(a.lua.load(format!("return eqoa.windows.takeover('{kind}', {{scope = 'all'}})")).eval::<bool>().unwrap());
        assert!(!b.lua.load(format!("local ok = eqoa.windows.takeover('{kind}', {{scope = 'all'}}); return ok == true")).eval::<bool>().unwrap());
    }
    let lines = wc::box_lines(&names);
    assert_eq!(lines.len(), 6, "{lines:?}");
    assert!(lines[5].starts_with("and 2 more"), "{lines:?}");
}

#[test]
fn the_box_is_drawn_with_both_buttons_over_play_and_not_when_nothing_conflicts() {
    fresh();
    let mut context = h::context();
    let draw = |context: &mut dear_imgui_rs::Context| {
        let ui = context.frame();
        let open_manager = wc::draw(ui, &names);
        drop(context.render_legacy());
        open_manager
    };
    assert!(!draw(&mut context));
    let name = std::ffi::CString::new(wc::WINDOW).unwrap();
    let found = || unsafe { !dear_imgui_rs::sys::igFindWindowByName(name.as_ptr()).is_null() };
    assert!(!found(), "control: nothing conflicts, no window");
    let (_holder, _rival) = two_mods();
    assert!(!draw(&mut context));
    assert!(found(), "the box is drawn");
    assert!(wc::WINDOW.contains("###eqoa-client-") && super::confirm::reserved(wc::WINDOW), "a mod's window can never take its id");
}
