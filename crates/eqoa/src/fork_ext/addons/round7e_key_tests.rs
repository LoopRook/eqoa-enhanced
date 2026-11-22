use super::{Host, option_test_host};
use crate::bindpage::{Column, ControlsPage};
use crate::binds::{self, Input, KeyEdge, ROWS, Sent, row_named};
use crate::fork_ext::addons::{input, native_core, native_events};
use crate::fork_ext::bindpage;
use crate::pointer::{
    key,
    testing::{Said, vui},
};
use crate::settings::{self, testing};
use vi::ui::Window;

const KEYS: &str = "keys.opt";
const MOD_KEYS: &str = "mod-keys.opt";

fn reset() {
    testing::reset(false);
    input::reset_input_state();
    native_core::CORE.with(|core| *core.borrow_mut() = native_core::Core::default());
}

fn bind(addon: &str, default: &str) -> Host {
    let host = option_test_host(addon);
    host.execute(&format!(
        "hits = 0; assert(eqoa.input.bind({{id='act', label='{addon}', default_key='{default}'}}, \
         function() hits = hits + 1 end))"
    ))
    .unwrap();
    host
}

fn hits(host: &Host, expected: usize) {
    host.execute(&format!("assert(hits == {expected}, 'hits = ' .. hits)"))
        .unwrap();
}

fn press(name: &str, plain_play: bool, keyboard: bool, hosts: &[&Host]) {
    let key = binds::key_of(name);
    input::route_mod_key(&key, true, false, false, plain_play, keyboard);
    input::route_mod_key(&key, false, false, false, plain_play, keyboard);
    for host in hosts {
        native_events::dispatch(host).unwrap();
    }
}

fn capture(page: &mut ControlsPage, row: usize, name: &str) {
    page.row = row as i32;
    // the client has drawn the page before a key reaches it
    page_text(page);
    let mut cues = Vec::new();
    assert!(!page.key(key::CROSS, true, 0, &mut cues));
    assert_eq!(page.capture, Some(Column::Keyboard));
    page.capture_key(&vui(), name, &mut cues);
    assert_eq!(page.capture, None);
}

/// What the page says with `page` as it is: every text it draws (the help
/// line shows the last change, or the selected row's own).
fn page_text(page: &mut ControlsPage) -> Vec<String> {
    let vui = vui();
    let mut raster = Said::default();
    let mut window = Window::default();
    let rows = ControlsPage::window_rows(vui.font_height(&raster));
    window.init(&vui, &raster, [320, 224], 600, rows, 0);
    window.open(true);
    page.draw(&vui, &mut raster, &mut window, &mut Vec::new());
    raster.0
}

fn controls_text(row: usize) -> Vec<String> {
    let mut page = ControlsPage::default();
    page.row = row as i32;
    page_text(&mut page)
}

#[test]
fn round7e_mod_takes_game_key_on_controls() {
    reset();
    let host = bind("alpha", "b");
    let jump = row_named("jump").unwrap();
    let mut page = ControlsPage::default();
    capture(&mut page, ROWS.len(), "space");

    assert_eq!(settings::binds().key(jump), Some("b"));
    assert_eq!(bindpage::get_key("alpha", "act").as_deref(), Some("space"));
    assert_eq!(
        KeyEdge::default().key("space", true, false, false, &settings::binds()),
        None
    );
    press("space", true, false, &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_game_takes_mod_key_on_controls_and_keeps_the_swap() {
    reset();
    let host = bind("alpha", "b");
    let jump = row_named("jump").unwrap();
    capture(&mut ControlsPage::default(), jump, "b");

    assert_eq!(settings::binds().key(jump), Some("b"));
    assert_eq!(bindpage::get_key("alpha", "act").as_deref(), Some("space"));
    press("b", true, false, &[&host]);
    hits(&host, 0);
    press("space", true, false, &[&host]);
    hits(&host, 1);

    assert!(settings::save(&Default::default()));
    let game_keys = testing::kept(KEYS).unwrap();
    let mod_keys = testing::kept(MOD_KEYS).unwrap();
    reset();
    testing::put(KEYS, &game_keys);
    testing::put(MOD_KEYS, &mod_keys);
    let host = bind("alpha", "b");
    assert_eq!(settings::binds().key(jump), Some("b"));
    press("space", true, false, &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_mods_swap_keys_on_controls() {
    reset();
    let alpha = bind("alpha", "b");
    let beta = bind("beta", "9");
    capture(&mut ControlsPage::default(), ROWS.len() + 1, "b");
    assert_eq!(bindpage::get_key("alpha", "act").as_deref(), Some("9"));
    press("b", true, false, &[&alpha, &beta]);
    hits(&alpha, 0);
    hits(&beta, 1);
    press("9", true, false, &[&alpha, &beta]);
    hits(&alpha, 1);
    hits(&beta, 1);
}

#[test]
fn round7e_default_does_not_take_existing_game_or_mod_key() {
    reset();
    let _alpha = bind("alpha", "space");
    let _beta = bind("beta", "b");
    let _gamma = bind("gamma", "b");
    assert_eq!(bindpage::get_key("alpha", "act").as_deref(), Some(""));
    assert_eq!(bindpage::get_key("beta", "act").as_deref(), Some("b"));
    assert_eq!(bindpage::get_key("gamma", "act").as_deref(), Some(""));
    let text = controls_text(ROWS.len()).join(" ");
    assert!(
        text.contains("Jump"),
        "the blocked default names its owner: {text}"
    );
}

#[test]
fn round7e_legacy_conflicts_have_one_winner_without_rewriting_the_file() {
    reset();
    let saved = b"mod.alpha.act=space\nmod.beta.act=b\nmod.gamma.act=b\n";
    testing::put(MOD_KEYS, saved);
    let alpha = bind("alpha", "");
    let gamma = bind("gamma", "");
    let beta = bind("beta", "");
    let mut edge = KeyEdge::default();
    assert_eq!(
        edge.key("space", true, false, false, &settings::binds()),
        Some(Sent::Key("space".into()))
    );
    press("space", true, false, &[&alpha]);
    hits(&alpha, 0);
    press("b", true, false, &[&gamma, &beta]);
    hits(&beta, 1);
    hits(&gamma, 0);
    assert_eq!(testing::kept(MOD_KEYS).as_deref(), Some(saved.as_slice()));
}

#[test]
fn round7e_controls_marks_both_sides_of_a_saved_conflict() {
    reset();
    assert!(!controls_text(row_named("jump").unwrap()).contains(&"!".into()));
    testing::put(
        MOD_KEYS,
        b"mod.alpha.act=space\nmod.beta.act=b\nmod.gamma.act=b\n",
    );
    let _alpha = bind("alpha", "");
    let _beta = bind("beta", "");
    let _gamma = bind("gamma", "");
    let game = controls_text(row_named("jump").unwrap());
    assert_eq!(game.iter().filter(|text| *text == "!").count(), 1);
    let mods = controls_text(ROWS.len() + 2);
    assert_eq!(mods.iter().filter(|text| *text == "!").count(), 3);
    assert!(controls_text(ROWS.len()).join(" ").contains("Jump"));
}

#[test]
fn round7e_mod_takes_game_key_with_no_key_to_swap() {
    reset();
    testing::put(KEYS, b"key.walk_forward=\nkey.bar1_slot1=y\n");
    let host = bind("alpha", "");
    let mut page = ControlsPage::default();
    capture(&mut page, ROWS.len(), "y");
    assert_eq!(
        settings::binds().key(row_named("bar1_slot1").unwrap()),
        None
    );
    assert_eq!(
        settings::binds().key(row_named("walk_forward").unwrap()),
        Some("")
    );
    press("y", true, false, &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_disabled_binding_cannot_fire_and_still_has_one_winner_on_return() {
    reset();
    testing::put(MOD_KEYS, b"mod.alpha.act=b\nmod.beta.act=b\n");
    let beta = bind("beta", "");
    let alpha = bind("alpha", "");
    bindpage::set_addon_enabled("alpha", false);
    press("b", true, false, &[&alpha, &beta]);
    hits(&alpha, 0);
    hits(&beta, 1);
    bindpage::set_addon_enabled("alpha", true);
    press("b", true, false, &[&alpha, &beta]);
    hits(&alpha, 1);
    hits(&beta, 1);
}

#[test]
fn round7e_chat_and_menus_suppress_the_binding() {
    reset();
    let host = bind("alpha", "b");
    press("b", false, false, &[&host]);
    press("b", true, true, &[&host]);
    hits(&host, 0);
    press("b", true, false, &[&host]);
    hits(&host, 1);
}

fn said(page: &mut ControlsPage) -> String {
    page_text(page).join(" ")
}

#[test]
fn round7e_notes_say_swapped_and_freed_in_both_directions() {
    reset();
    let _alpha = bind("alpha", "b");
    let d = binds::display;
    // a mod takes a game key: the game row takes the mod's old key
    let mut page = ControlsPage::default();
    capture(&mut page, ROWS.len(), "space");
    assert!(
        said(&mut page).contains(&format!("alpha: {}. Jump: {}.", d("space"), d("b"))),
        "{}",
        said(&mut page)
    );
    // a game row takes the mod's key: the mod takes the row's old key
    let mut page = ControlsPage::default();
    capture(&mut page, row_named("jump").unwrap(), "space");
    assert!(
        said(&mut page).contains(&format!("Jump: {}. alpha: {}.", d("space"), d("b"))),
        "{}",
        said(&mut page)
    );
}

#[test]
fn round7e_notes_say_freed_when_there_is_no_key_to_swap() {
    reset();
    let _alpha = bind("alpha", "");
    let d = binds::display;
    let mut page = ControlsPage::default();
    capture(&mut page, ROWS.len(), "space");
    assert!(
        said(&mut page).contains(&format!("alpha: {}. Jump has no key now.", d("space"))),
        "{}",
        said(&mut page)
    );
    // a bar slot has no key: giving it the mod's key leaves the mod with none
    let slot = row_named("bar1_slot1").unwrap();
    let mut page = ControlsPage::default();
    capture(&mut page, slot, "space");
    assert!(
        said(&mut page).contains(&format!("Bar 1: slot 1: {}. alpha has no key now.", d("space"))),
        "{}",
        said(&mut page)
    );
    assert_eq!(bindpage::get_key("alpha", "act").as_deref(), Some(""));
}

#[test]
fn round7e_mod_taking_a_game_key_survives_a_restart() {
    reset();
    let alpha = bind("alpha", "b");
    capture(&mut ControlsPage::default(), ROWS.len(), "space");
    assert!(settings::save(&Default::default()));
    let (game_keys, mod_keys) = (testing::kept(KEYS).unwrap(), testing::kept(MOD_KEYS).unwrap());
    drop(alpha);
    reset();
    testing::put(KEYS, &game_keys);
    testing::put(MOD_KEYS, &mod_keys);
    let alpha = bind("alpha", "b");
    assert_eq!(bindpage::get_key("alpha", "act").as_deref(), Some("space"));
    assert_eq!(settings::binds().key(row_named("jump").unwrap()), Some("b"));
    press("space", true, false, &[&alpha]);
    hits(&alpha, 1);
    assert!(!controls_text(ROWS.len()).contains(&"!".into()), "no conflict is left to mark");
}

#[test]
fn round7e_a_macro_trigger_key_is_not_given_to_a_mod_and_wins_at_press() {
    reset();
    let host = bind("alpha", "");
    let mut game = settings::binds();
    game.set_trigger(0, None, vec![Input::Key("y".into())]).unwrap();
    settings::set_binds(game);
    capture_refused(ROWS.len(), "y");
    assert_eq!(bindpage::get_key("alpha", "act").as_deref(), Some(""));
    // an old saved key of the same kind does nothing at press time
    assert!(bindpage::set_key("alpha", "act", "y"));
    press("y", true, false, &[&host]);
    hits(&host, 0);
}

fn capture_refused(row: usize, name: &str) {
    let mut page = ControlsPage::default();
    page.row = row as i32;
    page_text(&mut page);
    let mut cues = Vec::new();
    page.key(key::CROSS, true, 0, &mut cues);
    page.capture_key(&vui(), name, &mut cues);
    assert_eq!(page.capture, None, "the refusal is said, not hidden behind the prompt");
    assert!(said(&mut page).contains("trigger of Macro 1"), "{}", said(&mut page));
}

#[test]
fn round7e_defaults_button_keeps_a_blocked_default_empty() {
    reset();
    let _alpha = bind("alpha", "space");
    assert!(bindpage::set_key("alpha", "act", "9"));
    let mut page = ControlsPage::default();
    page.row = crate::bindpage::defaults();
    let mut cues = Vec::new();
    page.key(key::CROSS, true, 0, &mut cues);
    assert_eq!(bindpage::get_key("alpha", "act").as_deref(), Some(""));
}

#[test]
fn round7e_a_saved_conflict_says_who_wins() {
    reset();
    testing::put(MOD_KEYS, b"mod.alpha.act=space\n");
    let _alpha = bind("alpha", "");
    let text = controls_text(ROWS.len()).join(" ");
    assert!(text.contains("Jump") && text.contains("wins"), "{text}");
}

#[test]
fn round7e_typing_in_chat_suppresses_the_binding() {
    reset();
    let host = bind("alpha", "b");
    input::testing_text_capture(true);
    press("b", true, false, &[&host]);
    input::testing_text_capture(false);
    hits(&host, 0);
    press("b", true, false, &[&host]);
    hits(&host, 1);
}

// ---- fix round 1 (review of aa4fe9ba) ---------------------------------------------------
//
// These go through `Runtime::scripted_key`, the same `route_key` -> `route_mod_key` path a
// window key takes in `Runtime::event`, with the modifiers the window last said. The App's
// own `key_name`/`key_input` cannot be built in a test (it owns the window and the
// renderer), so the game's side is `game_sends`: `App::key_name`'s rule (viewer.rs, "the
// modifiers held where that is bound") written out and fed to the real `KeyEdge`.

use super::super::runtime::{Paths, Runtime};
use winit::event::ElementState;
use winit::keyboard::{Key, NamedKey};

const NONE: (bool, bool, bool) = (false, false, false);
const CTRL: (bool, bool, bool) = (true, false, false);
const ALT: (bool, bool, bool) = (false, true, false);

fn runtime() -> Runtime {
    Runtime::new(Paths::select(std::path::PathBuf::new(), None, None, None))
}

fn press_key(rt: &mut Runtime, mods: (bool, bool, bool), key: &Key, hosts: &[&Host]) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(1000);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    input::set_modifiers(mods);
    rt.scripted_key(id, key, None, ElementState::Pressed, true);
    rt.scripted_key(id, key, None, ElementState::Released, true);
    for host in hosts {
        native_events::dispatch(host).unwrap();
    }
    input::set_modifiers(NONE);
}

/// What the game does with a key pressed with `mods` held (see `GameKeys`).
fn game_sends(mods: (bool, bool, bool), key: &Key) -> Option<Sent> {
    GameKeys::new().press(mods, key, None)
}

fn mod_key_of(addon: &str) -> String {
    bindpage::get_key(addon, "act").unwrap_or_default()
}

/// A capture on `row` of `name` the page refuses: the key stays as it was, the page
/// is not left waiting, and what it says names `why`.
fn refused(row: usize, name: &str, why: &str) {
    let mut page = ControlsPage::default();
    page.row = row as i32;
    page_text(&mut page);
    let mut cues = Vec::new();
    page.key(key::CROSS, true, 0, &mut cues);
    page.capture_key(&vui(), name, &mut cues);
    assert_eq!(page.capture, None, "{name}: a refusal is said, not hidden behind the prompt");
    let text = said(&mut page);
    assert!(text.contains(why), "{name}: {text}");
}

#[test]
fn round7e_fix_h1_ctrl_u_on_a_game_row_does_not_also_fire_a_bare_u_mod() {
    reset();
    let host = bind("alpha", "9");
    let mut rt = runtime();
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "ctrl+9");
    settings::set_binds(game);
    let u = Key::Character("9".into());
    assert_eq!(game_sends(CTRL, &u), Some(Sent::Slot(0, 0)), "the game acts on Ctrl+9");
    press_key(&mut rt, CTRL, &u, &[&host]);
    hits(&host, 0);
    // control: a plain U is the mod's
    press_key(&mut rt, NONE, &u, &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix_h1_a_mod_bound_to_a_combo_fires_on_that_combo_only() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    capture(&mut ControlsPage::default(), ROWS.len(), "ctrl+9");
    assert_eq!(mod_key_of("alpha"), "ctrl+9");
    let u = Key::Character("9".into());
    press_key(&mut rt, NONE, &u, &[&host]);
    press_key(&mut rt, ALT, &u, &[&host]);
    hits(&host, 0);
    press_key(&mut rt, CTRL, &u, &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix_h1_a_ctrl_combo_over_a_plain_game_key_is_the_mods_and_w_still_walks() {
    // Owner rule: Ctrl+W is a key of its own. A mod may have it; W alone is Walk forward's.
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    capture(&mut ControlsPage::default(), ROWS.len(), "ctrl+w");
    assert_eq!(mod_key_of("alpha"), "ctrl+w", "accepted: no refusal");
    assert!(!controls_text(ROWS.len()).contains(&"!".into()), "and no mark");
    let w = Key::Character("w".into());
    assert!(press_consumed(&mut rt, CTRL, &w, None, &[&host]), "the mod's press is the mod's alone");
    hits(&host, 1);
    // control: W alone is not taken from the game and does not fire the mod
    assert!(!press_consumed(&mut rt, NONE, &w, None, &[&host]));
    assert_eq!(game_sends(NONE, &w), Some(Sent::Key("w".into())));
    hits(&host, 1);
}

#[test]
fn round7e_fix_h1_alt_enter_is_the_windows_not_a_mods_enter() {
    reset();
    let chat = row_named("chat").unwrap();
    let mut game = settings::binds();
    game.bind_key(chat, "y");
    settings::set_binds(game);
    let host = bind("alpha", "enter");
    let mut rt = runtime();
    assert_eq!(mod_key_of("alpha"), "enter", "Enter is free once chat has moved");
    let enter = Key::Named(NamedKey::Enter);
    press_key(&mut rt, ALT, &enter, &[&host]);
    hits(&host, 0);
    press_key(&mut rt, NONE, &enter, &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix_h2_m7_client_keys_are_never_mod_keys() {
    reset();
    testing::put(MOD_KEYS, b"mod.beta.act=f3\n");
    let alpha = bind("alpha", "f11");
    let beta = bind("beta", "");
    let gamma = bind("gamma", "escape");
    for who in ["alpha", "beta", "gamma"] {
        assert_eq!(mod_key_of(who), "", "{who}: a key the client keeps is not a mod's");
    }
    let mut rt = runtime();
    for key in [
        Key::Named(NamedKey::F11),
        Key::Named(NamedKey::F3),
        Key::Named(NamedKey::Escape),
    ] {
        press_key(&mut rt, NONE, &key, &[&alpha, &beta, &gamma]);
    }
    for host in [&alpha, &beta, &gamma] {
        hits(host, 0);
    }
    assert_eq!(
        testing::kept(MOD_KEYS).as_deref(),
        Some(b"mod.beta.act=f3\n".as_slice()),
        "the saved file is not rewritten"
    );
    assert!(controls_text(ROWS.len()).join(" ").contains("client"), "the row says why");
}

#[test]
fn round7e_fix_h2_the_clients_own_combos_are_refused_for_a_mod() {
    reset();
    let _host = bind("alpha", "");
    // Escape is the page's own (it gives a capture up), the rest are refused
    for key in ["alt+enter", "ctrl+c", "ctrl+v", "ctrl+x", "ctrl+f11"] {
        refused(ROWS.len(), key, "client");
        assert_eq!(mod_key_of("alpha"), "", "{key}");
    }
}

#[test]
fn round7e_fix_m7_a_bare_mod_key_does_not_fire_on_the_clipboard_combo() {
    reset();
    let host = bind("alpha", "9");
    let mut rt = runtime();
    press_key(&mut rt, CTRL, &Key::Character("9".into()), &[&host]);
    hits(&host, 0);
    press_key(&mut rt, NONE, &Key::Character("9".into()), &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix_m6_the_winner_of_a_shared_default_does_not_depend_on_load_order() {
    for order in [["alpha", "beta"], ["beta", "alpha"]] {
        reset();
        for who in order {
            let _ = bind(who, "9");
        }
        assert_eq!(mod_key_of("alpha"), "9", "{order:?}");
        assert_eq!(mod_key_of("beta"), "", "{order:?}");
    }
}

#[test]
fn round7e_fix_m6_a_saved_key_beats_a_default_in_either_load_order() {
    for order in [["alpha", "beta"], ["beta", "alpha"]] {
        reset();
        testing::put(MOD_KEYS, b"mod.beta.act=9\n");
        let mut hosts = Vec::new();
        for who in order {
            hosts.push((who, bind(who, if who == "alpha" { "9" } else { "" })));
        }
        assert_eq!(mod_key_of("beta"), "9", "{order:?}");
        assert_eq!(mod_key_of("alpha"), "", "{order:?}");
        let (alpha, beta) = (
            &hosts.iter().find(|h| h.0 == "alpha").unwrap().1,
            &hosts.iter().find(|h| h.0 == "beta").unwrap().1,
        );
        press("9", true, false, &[alpha, beta]);
        hits(alpha, 0);
        hits(beta, 1);
    }
}

#[test]
fn round7e_fix_m5_a_mod_returning_from_disabled_is_checked_again() {
    reset();
    let _alpha = bind("alpha", "9");
    bindpage::set_addon_enabled("alpha", false);
    let _beta = bind("beta", "");
    capture(&mut ControlsPage::default(), ROWS.len(), "9");
    assert_eq!(mod_key_of("beta"), "9");
    bindpage::set_addon_enabled("alpha", true);
    assert_eq!(mod_key_of("alpha"), "", "u was taken while it was off");
    assert_eq!(mod_key_of("beta"), "9");
}

#[test]
fn round7e_fix_m5_a_mod_returning_is_checked_against_the_game_too() {
    reset();
    let _alpha = bind("alpha", "9");
    bindpage::set_addon_enabled("alpha", false);
    // while it is off, a game control takes the key
    let slot = row_named("bar1_slot1").unwrap();
    capture(&mut ControlsPage::default(), slot, "9");
    assert_eq!(settings::binds().key(slot), Some("9"));
    bindpage::set_addon_enabled("alpha", true);
    assert_eq!(mod_key_of("alpha"), "", "u is the game's now");
    assert!(controls_text(ROWS.len()).join(" ").contains("Bar 1: slot 1"), "and the row says whose");
}

#[test]
fn round7e_fix_m5_the_defaults_button_obeys_the_rule_for_every_mod_and_keeps_its_reason() {
    reset();
    let _alpha = bind("alpha", "space");
    let _gamma = bind("gamma", "space");
    bindpage::set_addon_enabled("gamma", false);
    assert!(bindpage::set_key("alpha", "act", "9"));
    let mut page = ControlsPage::default();
    page.row = crate::bindpage::defaults();
    let mut cues = Vec::new();
    page.key(key::CROSS, true, 0, &mut cues);
    assert_eq!(mod_key_of("alpha"), "", "Space is Jump's");
    bindpage::set_addon_enabled("gamma", true);
    assert_eq!(mod_key_of("gamma"), "", "a mod that was off gets no key in use either");
    // a restart: the saved file says nothing for it, so the row still says why
    let saved = testing::kept(MOD_KEYS).unwrap_or_default();
    assert!(
        !String::from_utf8_lossy(&saved).contains("mod.alpha.act="),
        "an empty entry would lose the reason: {}",
        String::from_utf8_lossy(&saved)
    );
    let keep = (testing::kept(KEYS), testing::kept(MOD_KEYS));
    reset();
    if let Some(k) = keep.0 {
        testing::put(KEYS, &k);
    }
    if let Some(k) = keep.1 {
        testing::put(MOD_KEYS, &k);
    }
    let _alpha = bind("alpha", "space");
    assert!(controls_text(ROWS.len()).join(" ").contains("Jump"));
}

#[test]
fn round7e_fix_l9_a_key_that_cannot_be_kept_is_said_not_hidden() {
    reset();
    testing::reset(true);
    let _host = bind("alpha", "");
    let mut page = ControlsPage::default();
    capture(&mut page, ROWS.len(), "9");
    assert!(said(&mut page).contains("Key not kept"), "{}", said(&mut page));
    assert_eq!(mod_key_of("alpha"), "", "the old key stays");
}

#[test]
fn round7e_fix_h1_the_windows_modifier_events_reach_the_key_naming() {
    use winit::event::{Modifiers, WindowEvent};
    use winit::keyboard::ModifiersState;
    reset();
    assert_eq!(input::modifiers(), NONE);
    input::observe_modifiers(&WindowEvent::ModifiersChanged(Modifiers::from(
        ModifiersState::CONTROL | ModifiersState::SHIFT,
    )));
    assert_eq!(input::modifiers(), (true, false, true));
    input::observe_modifiers(&WindowEvent::ModifiersChanged(Modifiers::from(
        ModifiersState::empty(),
    )));
    assert_eq!(input::modifiers(), NONE);
}

// ---- H3: a cancelled Options session puts the mod keys back with the game's -----------------
//
// Through the real Options window: open on the Controls page (F1's way), move a key, leave
// by Triangle twice (the page to the menu, the menu closed and put back) or by Save Changes.

use crate::options::{GameOptions, WndOptions};
use crate::pointer::testing::NullRaster;

struct Session {
    w: WndOptions,
    o: GameOptions,
    vui: vi::ui::Vui,
}

impl Session {
    fn open() -> Session {
        let vui = vui();
        let o = GameOptions::default();
        let mut w = WndOptions::default();
        w.open_controls(&vui, &NullRaster, &o);
        let mut s = Session { w, o, vui };
        s.draw();
        s
    }
    fn draw(&mut self) {
        self.w.draw(&self.vui, &mut NullRaster, &self.o, &mut Vec::new());
    }
    fn press(&mut self, k: i32) {
        let (mut cues, mut ups) = (Vec::new(), Vec::new());
        self.w.key(&self.vui, &mut NullRaster, &mut self.o, k, 0, &mut cues, &mut ups);
        self.w.release(k);
        self.draw();
    }
    /// Walks to entry `row` of the Controls page, asks for a key there and gives it.
    fn give(&mut self, row: usize, name: &str) {
        for _ in 0..row {
            self.press(key::DOWN);
        }
        self.press(key::CROSS);
        self.w.capture_key(&self.vui, name, &mut Vec::new());
        self.draw();
        for _ in 0..row {
            self.press(key::UP);
        }
    }
    /// Triangle twice: the page to the menu, the menu closed with everything put back.
    fn cancel(&mut self) {
        self.press(key::TRIANGLE);
        self.press(key::TRIANGLE);
    }
    /// Save Changes, Yes.
    fn save(&mut self) {
        for k in [key::TRIANGLE, key::DOWN, key::DOWN, key::CROSS, key::LEFT, key::CROSS] {
            self.press(k);
        }
        for _ in 0..3 {
            self.draw();
        }
    }
}

fn mod_file() -> String {
    String::from_utf8(testing::kept(MOD_KEYS).unwrap_or_default()).unwrap()
}

#[test]
fn round7e_h3_a_cancel_puts_back_what_a_game_row_took_from_a_mod() {
    reset();
    let _alpha = bind("alpha", "b");
    let jump = row_named("jump").unwrap();
    let mut s = Session::open();
    s.give(jump, "b");
    assert_eq!(settings::binds().key(jump), Some("b"));
    assert_eq!(mod_key_of("alpha"), "space", "swapped");
    s.cancel();
    assert_eq!(settings::binds().key(jump), Some("space"), "Jump is back");
    assert_eq!(mod_key_of("alpha"), "b", "and so is the mod's key");
    assert!(!mod_file().contains("mod.alpha.act="), "nothing of the swap is left in the file: {}", mod_file());
}

#[test]
fn round7e_h3_a_cancel_puts_back_what_a_mod_took_from_a_game_row() {
    reset();
    let _alpha = bind("alpha", "b");
    let jump = row_named("jump").unwrap();
    let mut s = Session::open();
    s.give(ROWS.len(), "space");
    assert_eq!(mod_key_of("alpha"), "space");
    assert_eq!(settings::binds().key(jump), Some("b"));
    s.cancel();
    assert_eq!(mod_key_of("alpha"), "b");
    assert_eq!(settings::binds().key(jump), Some("space"));
    assert!(!mod_file().contains("mod.alpha.act="), "{}", mod_file());
}

#[test]
fn round7e_h3_a_cancel_after_defaults_brings_the_saved_mod_keys_back() {
    reset();
    let _alpha = bind("alpha", "b");
    assert!(bindpage::set_key("alpha", "act", "9"));
    let mut s = Session::open();
    s.give(crate::bindpage::defaults() as usize, "unused");
    assert_eq!(mod_key_of("alpha"), "b", "Defaults put the default back");
    assert!(!mod_file().contains("mod.alpha.act="), "and forgot the saved one: {}", mod_file());
    s.cancel();
    assert_eq!(mod_key_of("alpha"), "9");
    assert!(mod_file().contains("mod.alpha.act=9"), "{}", mod_file());
}

#[test]
fn round7e_h3_save_changes_keeps_a_swap() {
    reset();
    let _alpha = bind("alpha", "b");
    let jump = row_named("jump").unwrap();
    let mut s = Session::open();
    s.give(ROWS.len(), "space");
    s.save();
    assert_eq!(mod_key_of("alpha"), "space");
    assert_eq!(settings::binds().key(jump), Some("b"));
    assert!(mod_file().contains("mod.alpha.act=space"), "{}", mod_file());
    let keys = String::from_utf8(testing::kept(KEYS).unwrap()).unwrap();
    assert!(keys.contains("key.jump=b"), "{keys}");
    // a later window cancels nothing of what was saved
    let mut s = Session::open();
    s.cancel();
    assert_eq!(mod_key_of("alpha"), "space");
    assert_eq!(settings::binds().key(jump), Some("b"));
}

// ---- fix round 2 ------------------------------------------------------------------------

/// The game's own naming of a key (`App::key_name`, viewer.rs), written out branch by branch,
/// and its key dispatch (`KeyEdge`). It takes what the App takes as arguments: the key,
/// its place on the keyboard (`ev.base`), the modifiers (`self.mods`), and the two flags
/// the App works out first (`capture`: a Controls page waits for a key, `typing`: an edit
/// box has the keyboard). It leaves out where those flags come from, the window events
/// themselves, and `KeyEdge`'s `typing` mode, which no test here uses.
struct GameKeys {
    key_names: std::collections::HashMap<String, String>,
    edge: KeyEdge,
}

impl GameKeys {
    fn new() -> GameKeys {
        GameKeys { key_names: Default::default(), edge: KeyEdge::default() }
    }

    fn name(
        &mut self,
        mods: (bool, bool, bool),
        logical: &Key,
        base: Option<&'static str>,
        down: bool,
        repeat: bool,
        capture: bool,
        typing: bool,
    ) -> Option<String> {
        // let plain = crate::binds::name_of(&ev.logical_key)?;
        let plain = binds::name_of(logical)?;
        // let own = ev.base.map(str::to_string).unwrap_or_else(|| plain.clone());
        let own = base.map(str::to_string).unwrap_or_else(|| plain.clone());
        // if ev.state == ElementState::Released { return Some(self.key_names.remove(&own).unwrap_or(plain)); }
        if !down {
            return Some(self.key_names.remove(&own).unwrap_or(plain));
        }
        // if ev.repeat && let Some(n) = self.key_names.get(&own) { return Some(n.clone()); }
        if repeat && let Some(n) = self.key_names.get(&own) {
            return Some(n.clone());
        }
        // let (c, a, sh) = self.mods;
        let (c, a, sh) = mods;
        // let name = if (c || a || sh) && !typing {
        //     let combo = crate::binds::combo_name(c, a, sh, &own);
        //     if capture || crate::settings::with_binds(|b| b.combo_bound(&combo)) { combo } else { plain }
        // } else { plain };
        let name = if (c || a || sh) && !typing {
            let combo = binds::combo_name(c, a, sh, &own);
            if capture || settings::with_binds(|b| b.combo_bound(&combo)) { combo } else { plain }
        } else {
            plain
        };
        // self.key_names.insert(own, name.clone()); Some(name)
        self.key_names.insert(own, name.clone());
        Some(name)
    }

    /// What the game does with a key going down (None: nothing) under `mods`.
    fn press(&mut self, mods: (bool, bool, bool), logical: &Key, base: Option<&'static str>) -> Option<Sent> {
        let name = self.name(mods, logical, base, true, false, false, false)?;
        self.edge.key(&name, true, false, false, &settings::binds())
    }
}

fn rt_key(rt: &mut Runtime, id: usize, down: bool, key: &Key, base: Option<&'static str>) {
    let state = if down { ElementState::Pressed } else { ElementState::Released };
    rt.scripted_key_based(id, key, base, None, state, true);
}

/// A press of `key` with `mods` held, through the same route a window key takes.
fn press_based(rt: &mut Runtime, mods: (bool, bool, bool), key: &Key, base: Option<&'static str>, hosts: &[&Host]) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(5000);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    input::set_modifiers(mods);
    rt_key(rt, id, true, key, base);
    rt_key(rt, id, false, key, base);
    for host in hosts {
        native_events::dispatch(host).unwrap();
    }
    input::set_modifiers(NONE);
}

const SHIFT: (bool, bool, bool) = (false, false, true);

/// A press and release of `key` with `mods` held, through the mod runtime; whether the PRESS was
/// consumed (a consumed press never reaches the game: `window_event` returns on it).
fn press_consumed(rt: &mut Runtime, mods: (bool, bool, bool), key: &Key, base: Option<&'static str>, hosts: &[&Host]) -> bool {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(8000);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    input::set_modifiers(mods);
    let state = ElementState::Pressed;
    let taken = rt.scripted_key_based(id, key, base, None, state, true);
    let released = rt.scripted_key_based(id, key, base, None, ElementState::Released, true);
    assert_eq!(taken, released, "a release follows its press's owner");
    for host in hosts {
        native_events::dispatch(host).unwrap();
    }
    input::set_modifiers(NONE);
    taken
}

#[test]
fn round7e_fix2_h1_shift_on_a_named_key_is_a_key_of_its_own_for_a_mod() {
    // Owner rule: a modifier plus a key the game uses plain is a legitimate separate key.
    for combo in ["shift+space", "shift+tab", "shift+f5", "shift+up", "shift+`", "shift+enter"] {
        reset();
        let _host = bind("alpha", "");
        capture(&mut ControlsPage::default(), ROWS.len(), combo);
        assert_eq!(mod_key_of("alpha"), combo, "{combo}: accepted");
        assert!(!controls_text(ROWS.len()).contains(&"!".into()), "{combo}: no mark");
    }
}

#[test]
fn round7e_fix2_h1_shift_space_belongs_to_the_mod_and_space_still_jumps() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    let space = Key::Named(NamedKey::Space);
    // before any mod holds it, Shift+Space is the game's plain Space (Jump)
    assert!(!press_consumed(&mut rt, SHIFT, &space, None, &[&host]), "nothing owns it: it reaches the game");
    assert_eq!(GameKeys::new().press(SHIFT, &space, None), Some(Sent::Key("space".into())));
    hits(&host, 0);
    // a mod holds Shift+Space: the mod fires, the game never sees the press
    assert!(bindpage::set_key("alpha", "act", "shift+space"));
    assert!(press_consumed(&mut rt, SHIFT, &space, None, &[&host]), "owned: consumed, so no jump");
    hits(&host, 1);
    // Space alone still reaches the game and does not fire the mod
    assert!(!press_consumed(&mut rt, NONE, &space, None, &[&host]));
    assert_eq!(GameKeys::new().press(NONE, &space, None), Some(Sent::Key("space".into())));
    hits(&host, 1);
}

#[test]
fn round7e_fix2_h1_a_game_row_on_a_shifted_symbol_does_not_stop_shift_1_being_a_mod_key() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "!");
    settings::set_binds(game);
    // the slot is on the bare key `!`; Shift+1 is the name `shift+1`: a different key
    capture(&mut ControlsPage::default(), ROWS.len(), "shift+1");
    assert_eq!(mod_key_of("alpha"), "shift+1");
    let bang = Key::Character("!".into());
    assert!(press_consumed(&mut rt, SHIFT, &bang, Some("1"), &[&host]));
    hits(&host, 1);
}

#[test]
fn round7e_fix2_h1_a_macro_on_the_same_name_still_refuses_the_mod() {
    reset();
    let _host = bind("alpha", "");
    let mut game = settings::binds();
    game.set_trigger(0, None, vec![Input::Key("!".into())]).unwrap();
    settings::set_binds(game);
    // a macro on the bare `!` does not hold Shift+1
    capture(&mut ControlsPage::default(), ROWS.len(), "shift+1");
    assert_eq!(mod_key_of("alpha"), "shift+1");
    // a macro on the SAME name (shift+1) does
    reset();
    let _host = bind("alpha", "");
    let mut game = settings::binds();
    game.set_trigger(0, None, vec![Input::Key("shift+1".into())]).unwrap();
    settings::set_binds(game);
    refused(ROWS.len(), "shift+1", "Macro 1");
    assert_eq!(mod_key_of("alpha"), "");
}

#[test]
fn round7e_fix2_m2_a_release_is_named_as_its_press_was() {
    reset();
    let host = bind("alpha", "");
    host.execute("log = {}; eqoa.events.on('input.key', function(e) log[#log + 1] = e.data.combo .. (e.data.down and '+' or '-') end)")
        .unwrap();
    let mut rt = runtime();
    let u = Key::Character("9".into());
    // Ctrl+9 down, Ctrl let go, U let go
    input::set_modifiers(CTRL);
    rt_key(&mut rt, 9001, true, &u, None);
    input::set_modifiers(NONE);
    rt_key(&mut rt, 9001, false, &u, None);
    // a plain U down, Ctrl taken, U let go: still a plain U
    rt_key(&mut rt, 9002, true, &u, None);
    input::set_modifiers(CTRL);
    rt_key(&mut rt, 9002, false, &u, None);
    input::set_modifiers(NONE);
    native_events::dispatch(&host).unwrap();
    host.execute("assert(table.concat(log, ',') == 'ctrl+9+,ctrl+9-,9+,9-', table.concat(log, ','))")
        .unwrap();
}

#[test]
fn round7e_fix2_m2_the_game_names_a_release_the_same_way() {
    // the mirror agrees: the game's own release is named as its press (viewer.rs key_names)
    reset();
    let mut game = GameKeys::new();
    let u = Key::Character("9".into());
    let mut b = settings::binds();
    b.bind_key(row_named("bar1_slot1").unwrap(), "ctrl+9");
    settings::set_binds(b);
    assert_eq!(game.name(CTRL, &u, None, true, false, false, false).as_deref(), Some("ctrl+9"));
    assert_eq!(game.name(NONE, &u, None, false, false, false, false).as_deref(), Some("ctrl+9"));
}

#[test]
fn round7e_fix2_l3_super_is_no_plain_key() {
    reset();
    let host = bind("alpha", "9");
    let mut rt = runtime();
    // Cmd+U is not a plain U for a mod
    input::set_super(true);
    press_based(&mut rt, NONE, &Key::Character("9".into()), None, &[&host]);
    input::set_super(false);
    hits(&host, 0);
    press_based(&mut rt, NONE, &Key::Character("9".into()), None, &[&host]);
    hits(&host, 1);
    // Cmd+C, V, X and Cmd+, are the client's as Ctrl+C, V, X are
    for key in ["super+c", "super+v", "super+x", "super+,"] {
        refused(ROWS.len(), key, "client");
    }
    let _b = bind("beta", "super+c");
    assert_eq!(mod_key_of("beta"), "", "a default on a client key is refused");
}

#[test]
fn round7e_fix2_l3_window_modifier_events_carry_super() {
    use winit::event::{Modifiers, WindowEvent};
    use winit::keyboard::ModifiersState;
    reset();
    input::observe_modifiers(&WindowEvent::ModifiersChanged(Modifiers::from(ModifiersState::SUPER)));
    let host = bind("alpha", "9");
    let mut rt = runtime();
    press_based(&mut rt, NONE, &Key::Character("9".into()), None, &[&host]);
    hits(&host, 0);
    input::observe_modifiers(&WindowEvent::ModifiersChanged(Modifiers::from(ModifiersState::empty())));
    press_based(&mut rt, NONE, &Key::Character("9".into()), None, &[&host]);
    hits(&host, 1);
}

// ---- fix round 3: the press is exact, a table only fills in where no press exists -------------

const CTRL_SHIFT: (bool, bool, bool) = (true, false, true);

#[test]
fn round7e_fix3_a_german_shift_7_is_the_mods_by_its_name_not_by_what_it_types() {
    // Rule: the name decides. A mod on shift+7 owns that press on every layout, so on a German
    // layout (where it types `/`, the command key) it fires and the command line does not open.
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    assert!(bindpage::set_key("alpha", "act", "shift+7"));
    let slash = Key::Character("/".into());
    assert!(press_consumed(&mut rt, SHIFT, &slash, Some("7"), &[&host]));
    hits(&host, 1);
    // the bare key `/` is the game's: the mod does not fire on it, and it is not taken
    assert!(!press_consumed(&mut rt, NONE, &slash, Some("/"), &[&host]));
    hits(&host, 1);
    // US: Shift+7 types & and is the same name
    let amp = Key::Character("&".into());
    assert!(press_consumed(&mut rt, SHIFT, &amp, Some("7"), &[&host]));
    hits(&host, 2);
}

#[test]
fn round7e_fix3_a_uk_shift_quote_is_the_mods_by_its_name_even_with_a_game_row_on_at() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "@");
    settings::set_binds(game);
    assert!(bindpage::set_key("alpha", "act", "shift+'"));
    let at = Key::Character("@".into());
    assert!(press_consumed(&mut rt, SHIFT, &at, Some("'"), &[&host]), "shift+' is the mod's, though it types @");
    hits(&host, 1);
    let quote = Key::Character("\"".into());
    assert!(press_consumed(&mut rt, SHIFT, &quote, Some("'"), &[&host]));
    hits(&host, 2);
}

#[test]
fn round7e_fix3_the_numpad_with_numlock_off_is_named_by_its_place() {
    // Numpad1 with NumLock off arrives as End but is `1` by its place: Ctrl+Numpad1 is `ctrl+1`.
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    let end = Key::Named(NamedKey::End);
    for (mods, combo) in [(CTRL, "ctrl+1"), (SHIFT, "shift+1")] {
        assert!(bindpage::set_key("alpha", "act", combo));
        assert!(press_consumed(&mut rt, mods, &end, Some("1"), &[&host]), "{combo}");
    }
    hits(&host, 2);
    // End alone (Orbit right) is the game's, untouched
    assert!(!press_consumed(&mut rt, NONE, &end, None, &[&host]));
    hits(&host, 2);
}

#[test]
fn round7e_fix3_a_capture_is_accepted_whatever_the_layout_types() {
    reset();
    let _host = bind("alpha", "");
    let mut rt = runtime();
    // a German Shift+7 pressed (it types /): the name is shift+7, a key of its own
    press_based(&mut rt, SHIFT, &Key::Character("/".into()), Some("7"), &[]);
    capture(&mut ControlsPage::default(), ROWS.len(), "shift+7");
    assert_eq!(mod_key_of("alpha"), "shift+7");
}

#[test]
fn round7e_fix3_a_release_is_named_as_its_press_even_when_unmodified() {
    reset();
    let host = bind("alpha", "");
    host.execute("log = {}; eqoa.events.on('input.key', function(e) log[#log + 1] = e.data.key .. '/' .. e.data.combo .. (e.data.down and '+' or '-') end)")
        .unwrap();
    let mut rt = runtime();
    // a plain 1 goes down, Shift is taken, 1 is let go (it types ! now): still 1
    rt_key(&mut rt, 9101, true, &Key::Character("1".into()), Some("1"));
    input::set_modifiers(SHIFT);
    rt_key(&mut rt, 9101, false, &Key::Character("!".into()), Some("1"));
    input::set_modifiers(NONE);
    native_events::dispatch(&host).unwrap();
    host.execute("assert(table.concat(log, ',') == '1/1+,1/1-', table.concat(log, ','))").unwrap();
}

#[test]
fn round7e_fix3_a_let_go_key_is_forgotten_wherever_it_went() {
    reset();
    let host = bind("alpha", "");
    host.execute("log = {}; eqoa.events.on('input.key', function(e) log[#log + 1] = e.data.combo .. (e.data.down and '+' or '-') end)")
        .unwrap();
    let mut rt = runtime();
    let u = Key::Character("9".into());
    input::set_modifiers(CTRL);
    rt.scripted_key_based(9201, &u, None, None, ElementState::Pressed, true);
    input::set_modifiers(NONE);
    // a menu has the keyboard when it is let go: nothing is sent, and it is forgotten
    rt.scripted_key_based(9201, &u, None, None, ElementState::Released, false);
    // later a plain U goes down in a menu and comes up in play: a plain U
    rt.scripted_key_based(9202, &u, None, None, ElementState::Pressed, false);
    rt.scripted_key_based(9202, &u, None, None, ElementState::Released, true);
    native_events::dispatch(&host).unwrap();
    host.execute("assert(table.concat(log, ',') == 'ctrl+9+,9-', table.concat(log, ','))").unwrap();
}

#[test]
fn round7e_fix3_modifier_order_in_a_saved_name_does_not_matter() {
    reset();
    testing::put(MOD_KEYS, b"mod.alpha.act=shift+ctrl+9\nmod.beta.act=ctrl+super+c\nmod.gamma.act=alt+super+enter\n");
    let alpha = bind("alpha", "");
    let _beta = bind("beta", "");
    let _gamma = bind("gamma", "");
    assert_eq!(mod_key_of("alpha"), "ctrl+shift+9", "read in the order the client names a press");
    assert_eq!(mod_key_of("beta"), "", "ctrl+super+c is the client's however it is written");
    assert_eq!(mod_key_of("gamma"), "", "alt+super+enter too");
    let mut rt = runtime();
    press_based(&mut rt, CTRL_SHIFT, &Key::Character("U".into()), Some("9"), &[&alpha]);
    hits(&alpha, 1);
}

#[test]
fn round7e_fix3_nothing_listening_means_no_key_work() {
    use super::super::perf_alloc::measure;
    reset();
    bindpage::reset_in_memory();
    let mut rt = runtime();
    let u = Key::Character("9".into());
    // no mod at all: the runtime keeps nothing of the key
    input::set_modifiers(CTRL);
    rt_key(&mut rt, 9301, true, &u, Some("9"));
    rt_key(&mut rt, 9301, false, &u, Some("9"));
    let (_, _, allocs, _) = measure(|| {
        for _ in 0..300 {
            rt_key(&mut rt, 9301, true, &u, Some("9"));
            rt_key(&mut rt, 9301, false, &u, Some("9"));
        }
    });
    input::set_modifiers(NONE);
    assert_eq!(allocs, 0, "no mod: no allocation on the key path");
    assert_eq!(input::testing_key_ring_len(), 0, "no mod: no key ring kept");
    // one mod that is on, with no key bound and no input.key listener
    let _quiet = option_test_host("quiet");
    input::set_modifiers(CTRL);
    input::route_mod_key_based(&u, Some("9"), true, false, false, true, false);
    input::route_mod_key_based(&u, Some("9"), false, false, false, true, false);
    let (_, _, allocs, _) = measure(|| {
        for _ in 0..300 {
            input::route_mod_key_based(&u, Some("9"), true, false, false, true, false);
            input::route_mod_key_based(&u, Some("9"), false, false, false, true, false);
        }
    });
    input::set_modifiers(NONE);
    assert_eq!(allocs, 0, "a mod with nothing bound or listening: no allocation");
    // control: a mod that has a key bound does the work
    let _loud = bind("loud", "");
    input::set_modifiers(CTRL);
    let (_, _, allocs, _) = measure(|| {
        input::route_mod_key_based(&u, Some("9"), true, false, false, true, false);
        input::route_mod_key_based(&u, Some("9"), false, false, false, true, false);
    });
    input::set_modifiers(NONE);
    assert!(allocs > 0, "the meter sees a listening mod's key work");
}

#[test]
fn round7e_fix3_a_mark_means_the_same_name_is_the_games() {
    reset();
    let _host = bind("alpha", "");
    // no game control holds shift+7: no mark, on any layout
    assert!(bindpage::set_key("alpha", "act", "shift+7"));
    let mut rt = runtime();
    press_based(&mut rt, SHIFT, &Key::Character("/".into()), Some("7"), &[]);
    assert!(!controls_text(ROWS.len()).contains(&"!".into()));
    // a game row on that very name: the mod is marked and says whose it is
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "shift+7");
    settings::set_binds(game);
    let text = controls_text(ROWS.len());
    assert!(text.contains(&"!".into()), "marked: {text:?}");
    assert!(text.join(" ").contains("Bar 1: slot 1"), "{text:?}");
}

// ---- owner test fix 1: the capture as the live client orders it -------------------------------
//
// The order in the client: the window's ModifiersChanged and the key reach the mod runtime
// (`report_viewer::addon_event` -> `Runtime::event` -> `route_key`, with the Options page up so
// `plain_play` is false), then `App::key_input` names the key with the capture waiting
// (`key_name(capture = true)`, `GameKeys` here) and hands the name to the Options window, whose
// Controls page asks the mod hook. The test enters at the Options window (`WndOptions`) and the
// mod runtime (`Runtime::scripted_key_based`); only `Runtime::event`'s own match on the window
// event and `App::key_input` are not run (they need a `Window` and the App).

fn live_capture(
    row: usize,
    mods: (bool, bool, bool),
    logical: &Key,
    base: Option<&'static str>,
) -> (String, String) {
    use winit::event::{Modifiers, WindowEvent};
    use winit::keyboard::ModifiersState;
    let mut s = Session::open();
    let mut rt = runtime();
    for _ in 0..row {
        s.press(key::DOWN);
    }
    s.press(key::CROSS);
    let mut state = ModifiersState::empty();
    state.set(ModifiersState::CONTROL, mods.0);
    state.set(ModifiersState::ALT, mods.1);
    state.set(ModifiersState::SHIFT, mods.2);
    input::observe_modifiers(&WindowEvent::ModifiersChanged(Modifiers::from(state)));
    // the mod runtime sees the key first; the Options page is up, so it is not plain play
    rt.scripted_key_based(7001, logical, base, None, ElementState::Pressed, false);
    // then the game's naming with the capture waiting, and the page
    let name = GameKeys::new().name(mods, logical, base, true, false, true, false).unwrap();
    s.w.capture_key(&s.vui, &name, &mut Vec::new());
    s.draw();
    rt.scripted_key_based(7001, logical, base, None, ElementState::Released, false);
    input::observe_modifiers(&WindowEvent::ModifiersChanged(Modifiers::from(ModifiersState::empty())));
    let waiting = if s.w.capturing().is_some() { "waiting" } else { "done" };
    (name, format!("{}/{}", mod_key_of("alpha"), waiting))
}

#[test]
fn round7e_owner1_shift_and_friends_over_a_game_key_are_accepted_in_the_live_order() {
    // Owner (2026-11-20): "you can have a key that is already bound, plus a modifier".
    let cases: [(&str, (bool, bool, bool), Key, Option<&'static str>); 7] = [
        ("shift+space", SHIFT, Key::Named(NamedKey::Space), None),
        ("shift+tab", SHIFT, Key::Named(NamedKey::Tab), None),
        ("shift+enter", SHIFT, Key::Named(NamedKey::Enter), None),
        ("shift+up", SHIFT, Key::Named(NamedKey::ArrowUp), None),
        ("shift+f5", SHIFT, Key::Named(NamedKey::F5), None),
        ("ctrl+space", CTRL, Key::Named(NamedKey::Space), None),
        ("alt+space", ALT, Key::Named(NamedKey::Space), None),
    ];
    let mut wrong = Vec::new();
    for (name, mods, logical, base) in cases {
        reset();
        let _alpha = bind("alpha", "");
        let _beta = bind("beta", "b");
        let (got, result) = live_capture(ROWS.len(), mods, &logical, base);
        if got != name || result != format!("{name}/done") {
            wrong.push(format!("{name}: named {got:?}, mod key/capture {result:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn round7e_owner1_a_shifted_character_is_what_it_types() {
    // Shift+1 types ! on a US keyboard: nothing of the game's is on !, so it is accepted
    reset();
    let _alpha = bind("alpha", "");
    let (name, result) = live_capture(ROWS.len(), SHIFT, &Key::Character("!".into()), Some("1"));
    assert_eq!((name.as_str(), result.as_str()), ("shift+1", "shift+1/done"));
}

/// Shift+Space given to the mod on `row` in `s`, in the live order (see `live_capture`):
/// what the mod's key is after, and whether the page is still waiting.
fn shift_space_in(s: &mut Session, rt: &mut Runtime, row: usize, who: &str) -> String {
    use winit::event::{Modifiers, WindowEvent};
    use winit::keyboard::ModifiersState;
    for _ in 0..row {
        s.press(key::DOWN);
    }
    s.press(key::CROSS);
    let space = Key::Named(NamedKey::Space);
    input::observe_modifiers(&WindowEvent::ModifiersChanged(Modifiers::from(ModifiersState::SHIFT)));
    rt.scripted_key_based(7101, &space, None, None, ElementState::Pressed, false);
    let name = GameKeys::new().name(SHIFT, &space, None, true, false, true, false).unwrap();
    s.w.capture_key(&s.vui, &name, &mut Vec::new());
    s.draw();
    rt.scripted_key_based(7101, &space, None, None, ElementState::Released, false);
    input::observe_modifiers(&WindowEvent::ModifiersChanged(Modifiers::from(ModifiersState::empty())));
    for _ in 0..row {
        s.press(key::UP);
    }
    format!("{}/{}", mod_key_of(who), if s.w.capturing().is_some() { "waiting" } else { "done" })
}

#[test]
fn round7e_owner1_shift_space_is_accepted_after_the_swaps_the_owner_made() {
    let jump = row_named("jump").unwrap();
    let mut wrong = Vec::new();
    for scenario in ["fresh", "swap then cancel", "jump takes then cancel", "defaults then cancel", "swap saved then swapped back"] {
        reset();
        let _alpha = bind("alpha", "b");
        let _beta = bind("beta", "");
        let mut rt = runtime();
        let mut s = Session::open();
        match scenario {
            "swap then cancel" => {
                s.give(ROWS.len(), "space");
                s.cancel();
                s = Session::open();
            }
            "jump takes then cancel" => {
                s.give(jump, "b");
                s.cancel();
                s = Session::open();
            }
            "defaults then cancel" => {
                s.give(crate::bindpage::defaults() as usize, "unused");
                s.cancel();
                s = Session::open();
            }
            "swap saved then swapped back" => {
                s.give(ROWS.len(), "space");
                s.save();
                s = Session::open();
                s.give(jump, "space");
                s.save();
                s = Session::open();
            }
            _ => {}
        }
        assert_eq!(settings::binds().key(jump), Some("space"), "{scenario}: Jump is on Space");
        let result = shift_space_in(&mut s, &mut rt, ROWS.len() + 1, "beta");
        if result != "shift+space/done" {
            wrong.push(format!("{scenario}: {result}"));
        }
        assert_eq!(settings::binds().key(jump), Some("space"), "{scenario}: Jump keeps Space");
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn round7e_owner1_a_saved_shift_space_fires_the_mod_and_not_jump_and_is_not_marked() {
    reset();
    testing::put(MOD_KEYS, b"mod.alpha.act=shift+space\n");
    let host = bind("alpha", "");
    let mut rt = runtime();
    assert_eq!(mod_key_of("alpha"), "shift+space");
    assert!(!controls_text(ROWS.len()).contains(&"!".into()), "no conflict: no mark");
    let space = Key::Named(NamedKey::Space);
    // the live behaviour: the mod fires and the game never sees the press (no jump)
    assert!(press_consumed(&mut rt, SHIFT, &space, None, &[&host]));
    hits(&host, 1);
    // Space alone still reaches the game and jumps
    assert!(!press_consumed(&mut rt, NONE, &space, None, &[&host]));
    assert_eq!(GameKeys::new().press(NONE, &space, None), Some(Sent::Key("space".into())));
    hits(&host, 1);
}

#[test]
fn round7e_owner1_a_game_control_on_shift_space_is_a_real_conflict_and_swaps_as_any_game_key() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "shift+space");
    settings::set_binds(game);
    // the same name is bound in the game: the game's control wins a saved one at press time
    assert!(bindpage::set_key("alpha", "act", "shift+space"));
    let space = Key::Named(NamedKey::Space);
    assert!(!press_consumed(&mut rt, SHIFT, &space, None, &[&host]), "the game acts: not consumed");
    hits(&host, 0);
    assert!(controls_text(ROWS.len()).contains(&"!".into()), "and the row is marked");
    // giving the mod the key on Controls frees the slot, as between two game rows
    assert!(bindpage::set_key("alpha", "act", ""));
    capture(&mut ControlsPage::default(), ROWS.len(), "shift+space");
    assert_eq!(mod_key_of("alpha"), "shift+space");
    assert_eq!(settings::binds().key(slot), None, "the slot has none now");
    assert!(press_consumed(&mut rt, SHIFT, &space, None, &[&host]));
    hits(&host, 1);
}

#[test]
fn round7e_owner1_a_bare_mod_key_is_never_consumed_only_a_modifier_combination_is() {
    reset();
    let host = bind("alpha", "9");
    let mut rt = runtime();
    let u = Key::Character("9".into());
    assert!(!press_consumed(&mut rt, NONE, &u, Some("9"), &[&host]), "a bare key reaches the game as always");
    hits(&host, 1);
    assert!(bindpage::set_key("alpha", "act", "ctrl+9"));
    assert!(press_consumed(&mut rt, CTRL, &u, Some("9"), &[&host]), "Ctrl+9 is the mod's alone");
    hits(&host, 2);
}

// ---- owner request 1: the Controls key that opens the Mods & Addons window --------------------

use super::super::{request_toggle, take_open_request, take_toggle_request};

const MGR: (&str, &str) = (bindpage::MANAGER_ADDON, bindpage::MANAGER_ID);

fn mgr_key() -> String {
    bindpage::get_key(MGR.0, MGR.1).unwrap_or_default()
}

/// A press and release with the plain-play flag as given; whether the manager asked to toggle.
fn press_for_manager(rt: &mut Runtime, key: &Key, plain_play: bool) -> bool {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(12000);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let _ = take_toggle_request(); // only what THIS press asks
    rt.scripted_key_based(id, key, None, None, ElementState::Pressed, plain_play);
    rt.scripted_key_based(id, key, None, None, ElementState::Released, plain_play);
    // report it, and leave the request for the frame (`apply_requests`) as the press made it
    let asked = take_toggle_request();
    if asked {
        request_toggle();
    }
    asked
}

/// A fresh settings state with no request left over from another test.
fn mgr_reset() {
    reset();
    bindpage::set_manager_blocked(false);
    let _ = take_toggle_request();
    let _ = take_open_request();
}

fn f(n: u8) -> Key {
    Key::Named(match n {
        9 => NamedKey::F9,
        10 => NamedKey::F10,
        _ => unreachable!(),
    })
}

#[test]
fn round7e_mgr1_a_fresh_install_has_f10_and_it_opens_and_closes_the_manager() {
    mgr_reset();
    bindpage::register_manager_row();
    assert_eq!(mgr_key(), "f10", "the default, one constant");
    assert_eq!(bindpage::MANAGER_DEFAULT_KEY, "f10");
    assert_eq!(bindpage::extra_rows(), 1, "the row is there with no mod enabled");
    let mut rt = runtime();
    assert!(!rt.manager_open());
    assert!(press_for_manager(&mut rt, &f(10), true), "F10 in the world asks for the manager");
    rt.apply_requests();
    assert!(rt.manager_open(), "opened");
    assert!(!take_open_request(), "the Options route's open request is not used");
    assert!(press_for_manager(&mut rt, &f(10), true));
    rt.apply_requests();
    assert!(!rt.manager_open(), "closed again");
    // control: F9 is not the key
    assert!(!press_for_manager(&mut rt, &f(9), true));
    let _ = take_toggle_request();
}

#[test]
fn round7e_mgr1_it_is_not_fired_while_a_menu_or_chat_has_the_keyboard() {
    mgr_reset();
    bindpage::register_manager_row();
    let mut rt = runtime();
    assert!(!press_for_manager(&mut rt, &f(10), false), "a menu has the keyboard (not plain play)");
    input::testing_text_capture(true);
    assert!(!press_for_manager(&mut rt, &f(10), true), "typing in chat or an editor");
    input::testing_text_capture(false);
    assert!(press_for_manager(&mut rt, &f(10), true), "control: plain play");
}

#[test]
fn round7e_mgr1_a_key_a_game_control_has_leaves_the_row_empty_and_names_it() {
    mgr_reset();
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "f10");
    settings::set_binds(game);
    bindpage::register_manager_row();
    assert_eq!(mgr_key(), "", "the default never takes a key in use");
    let text = controls_text(ROWS.len()).join(" ");
    assert!(text.contains("Bar 1: slot 1"), "{text}");
    // a macro's key the same
    mgr_reset();
    let mut game = settings::binds();
    game.set_trigger(0, None, vec![Input::Key("f10".into())]).unwrap();
    settings::set_binds(game);
    bindpage::register_manager_row();
    assert_eq!(mgr_key(), "");
    // a mod's default for it: the first by name has it (the row's id is `enhanced`)
    mgr_reset();
    let _alpha = bind("alpha", "f10");
    bindpage::register_manager_row();
    assert_eq!((mgr_key().as_str(), mod_key_of("alpha").as_str()), ("", "f10"), "alpha sorts first");
    mgr_reset();
    let _zeta = bind("zeta", "f10");
    bindpage::register_manager_row();
    assert_eq!((mgr_key().as_str(), mod_key_of("zeta").as_str()), ("f10", ""), "enhanced sorts first");
}

#[test]
fn round7e_mgr1_giving_it_a_key_a_game_control_has_swaps_as_for_a_mod_key() {
    mgr_reset();
    bindpage::register_manager_row();
    let art = row_named("art").unwrap();
    assert_eq!(settings::binds().key(art), Some("f9"));
    capture(&mut ControlsPage::default(), ROWS.len(), "f9");
    assert_eq!(mgr_key(), "f9");
    assert_eq!(settings::binds().key(art), Some("f10"), "Art took the key the row left");
    let mut rt = runtime();
    assert!(press_for_manager(&mut rt, &f(9), true), "F9 opens it now");
    assert!(!press_for_manager(&mut rt, &f(10), true), "F10 does not");
    // the client's own keys are refused, as for a mod
    // (F3, F11 and F12 are refused by the page itself before any row is asked)
    refused(ROWS.len(), "alt+enter", "client");
    refused(ROWS.len(), "ctrl+c", "client");
    assert_eq!(mgr_key(), "f9");
}

#[test]
fn round7e_mgr1_a_saved_key_a_game_control_has_is_marked_and_does_nothing() {
    mgr_reset();
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "f10");
    settings::set_binds(game);
    testing::put(MOD_KEYS, b"mod.enhanced.mods_addons=f10\n");
    bindpage::register_manager_row();
    assert_eq!(mgr_key(), "f10", "an old saved key is kept as it is");
    assert!(controls_text(ROWS.len()).contains(&"!".into()), "and marked");
    let mut rt = runtime();
    assert!(!press_for_manager(&mut rt, &f(10), true), "the game's control wins at press time");
}

#[test]
fn round7e_mgr1_no_key_set_means_no_key_work_and_a_key_survives_a_restart_and_a_cancel() {
    use super::super::perf_alloc::measure;
    mgr_reset();
    bindpage::register_manager_row();
    assert!(bindpage::set_key(MGR.0, MGR.1, ""));
    let mut rt = runtime();
    let u = Key::Character("9".into());
    // no key set: F10 and every other key cost no allocation
    rt.scripted_key_based(13001, &f(10), None, None, ElementState::Pressed, true);
    rt.scripted_key_based(13001, &f(10), None, None, ElementState::Released, true);
    let (_, _, allocs, _) = measure(|| {
        for _ in 0..300 {
            for (key, base) in [(&f(10), None), (&u, Some("9"))] {
                rt.scripted_key_based(13002, key, base, None, ElementState::Pressed, true);
                rt.scripted_key_based(13002, key, base, None, ElementState::Released, true);
            }
        }
    });
    assert_eq!(allocs, 0, "no key set: nothing on the key path allocates");
    assert!(!take_toggle_request());
    // with the default key set, the other keys still allocate nothing
    assert!(bindpage::set_key(MGR.0, MGR.1, "f10"));
    let (_, _, allocs, _) = measure(|| {
        for _ in 0..300 {
            rt.scripted_key_based(13003, &u, Some("9"), None, ElementState::Pressed, true);
            rt.scripted_key_based(13003, &u, Some("9"), None, ElementState::Released, true);
        }
    });
    assert_eq!(allocs, 0, "a key that is not the manager's costs nothing");
    // a key kept across a restart
    assert!(bindpage::set_key(MGR.0, MGR.1, "f9"));
    let kept = testing::kept(MOD_KEYS).unwrap();
    assert!(String::from_utf8_lossy(&kept).contains("mod.enhanced.mods_addons=f9"));
    mgr_reset();
    testing::put(MOD_KEYS, &kept);
    bindpage::register_manager_row();
    assert_eq!(mgr_key(), "f9");
    // a cancelled Options session puts it back
    let mut s = Session::open();
    s.give(ROWS.len(), "f10");
    assert_eq!(mgr_key(), "f10");
    s.cancel();
    assert_eq!(mgr_key(), "f9");
}

#[test]
fn round7e_mgr2_a_combo_on_the_manager_row_owns_its_press_with_no_mod_enabled() {
    mgr_reset();
    bindpage::register_manager_row();
    assert!(bindpage::set_key(MGR.0, MGR.1, "shift+space"));
    let mut rt = runtime(); // no mod, no host: nothing is running
    let space = Key::Named(NamedKey::Space);
    // press, and its release, are the row's alone: the game never sees either (taken == released)
    assert!(press_consumed(&mut rt, SHIFT, &space, None, &[]), "Shift+Space is the manager's: no jump");
    assert!(take_toggle_request(), "and it asked to open the manager");
    // Space alone is the game's and asks for nothing
    assert!(!press_consumed(&mut rt, NONE, &space, None, &[]));
    assert!(!take_toggle_request());
}

#[test]
fn round7e_mgr2_the_manager_key_does_nothing_where_the_window_would_not_be_usable() {
    mgr_reset();
    bindpage::register_manager_row();
    let mut rt = runtime();
    let space = Key::Named(NamedKey::Space);
    bindpage::set_manager_blocked(true);
    // the bare default key: nothing, and nothing consumed
    assert!(!press_for_manager(&mut rt, &f(10), true), "blocked: F10 does nothing");
    // a combination: not consumed either, so the game still gets it
    assert!(bindpage::set_key(MGR.0, MGR.1, "shift+space"));
    assert!(!press_consumed(&mut rt, SHIFT, &space, None, &[]), "blocked: not consumed");
    assert!(!take_toggle_request());
    // control: where it is usable the same press is the row's
    bindpage::set_manager_blocked(false);
    assert!(press_consumed(&mut rt, SHIFT, &space, None, &[]));
    assert!(take_toggle_request());
}
