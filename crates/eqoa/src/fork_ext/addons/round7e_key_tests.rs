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
    let beta = bind("beta", "u");
    capture(&mut ControlsPage::default(), ROWS.len() + 1, "b");
    assert_eq!(bindpage::get_key("alpha", "act").as_deref(), Some("u"));
    press("b", true, false, &[&alpha, &beta]);
    hits(&alpha, 0);
    hits(&beta, 1);
    press("u", true, false, &[&alpha, &beta]);
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
    assert!(bindpage::set_key("alpha", "act", "u"));
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
    let host = bind("alpha", "u");
    let mut rt = runtime();
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "ctrl+u");
    settings::set_binds(game);
    let u = Key::Character("u".into());
    assert_eq!(game_sends(CTRL, &u), Some(Sent::Slot(0, 0)), "the game acts on Ctrl+U");
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
    capture(&mut ControlsPage::default(), ROWS.len(), "ctrl+u");
    assert_eq!(mod_key_of("alpha"), "ctrl+u");
    let u = Key::Character("u".into());
    press_key(&mut rt, NONE, &u, &[&host]);
    press_key(&mut rt, ALT, &u, &[&host]);
    hits(&host, 0);
    press_key(&mut rt, CTRL, &u, &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix_h1_a_ctrl_combo_over_a_plain_game_key_is_refused_and_never_fires() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    // Ctrl+W is pressed by the game as W (Walk forward): the mod may not have it
    refused(ROWS.len(), "ctrl+w", "Walk forward");
    assert_eq!(mod_key_of("alpha"), "");
    // an old saved key of the kind: the game acts, the mod does not
    assert!(bindpage::set_key("alpha", "act", "ctrl+w"));
    let w = Key::Character("w".into());
    assert_eq!(game_sends(CTRL, &w), Some(Sent::Key("w".into())));
    press_key(&mut rt, CTRL, &w, &[&host]);
    hits(&host, 0);
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
    let host = bind("alpha", "u");
    let mut rt = runtime();
    press_key(&mut rt, CTRL, &Key::Character("u".into()), &[&host]);
    hits(&host, 0);
    press_key(&mut rt, NONE, &Key::Character("u".into()), &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix_m6_the_winner_of_a_shared_default_does_not_depend_on_load_order() {
    for order in [["alpha", "beta"], ["beta", "alpha"]] {
        reset();
        for who in order {
            let _ = bind(who, "u");
        }
        assert_eq!(mod_key_of("alpha"), "u", "{order:?}");
        assert_eq!(mod_key_of("beta"), "", "{order:?}");
    }
}

#[test]
fn round7e_fix_m6_a_saved_key_beats_a_default_in_either_load_order() {
    for order in [["alpha", "beta"], ["beta", "alpha"]] {
        reset();
        testing::put(MOD_KEYS, b"mod.beta.act=u\n");
        let mut hosts = Vec::new();
        for who in order {
            hosts.push((who, bind(who, if who == "alpha" { "u" } else { "" })));
        }
        assert_eq!(mod_key_of("beta"), "u", "{order:?}");
        assert_eq!(mod_key_of("alpha"), "", "{order:?}");
        let (alpha, beta) = (
            &hosts.iter().find(|h| h.0 == "alpha").unwrap().1,
            &hosts.iter().find(|h| h.0 == "beta").unwrap().1,
        );
        press("u", true, false, &[alpha, beta]);
        hits(alpha, 0);
        hits(beta, 1);
    }
}

#[test]
fn round7e_fix_m5_a_mod_returning_from_disabled_is_checked_again() {
    reset();
    let _alpha = bind("alpha", "u");
    bindpage::set_addon_enabled("alpha", false);
    let _beta = bind("beta", "");
    capture(&mut ControlsPage::default(), ROWS.len(), "u");
    assert_eq!(mod_key_of("beta"), "u");
    bindpage::set_addon_enabled("alpha", true);
    assert_eq!(mod_key_of("alpha"), "", "u was taken while it was off");
    assert_eq!(mod_key_of("beta"), "u");
}

#[test]
fn round7e_fix_m5_a_mod_returning_is_checked_against_the_game_too() {
    reset();
    let _alpha = bind("alpha", "u");
    bindpage::set_addon_enabled("alpha", false);
    // while it is off, a game control takes the key
    let slot = row_named("bar1_slot1").unwrap();
    capture(&mut ControlsPage::default(), slot, "u");
    assert_eq!(settings::binds().key(slot), Some("u"));
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
    assert!(bindpage::set_key("alpha", "act", "u"));
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
    capture(&mut page, ROWS.len(), "u");
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
    assert!(bindpage::set_key("alpha", "act", "u"));
    let mut s = Session::open();
    s.give(crate::bindpage::defaults() as usize, "unused");
    assert_eq!(mod_key_of("alpha"), "b", "Defaults put the default back");
    assert!(!mod_file().contains("mod.alpha.act="), "and forgot the saved one: {}", mod_file());
    s.cancel();
    assert_eq!(mod_key_of("alpha"), "u");
    assert!(mod_file().contains("mod.alpha.act=u"), "{}", mod_file());
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

#[test]
fn round7e_fix2_h1_shift_on_a_named_key_is_the_games_plain_key() {
    for (combo, who) in [
        ("shift+space", "Jump"),
        ("shift+tab", "Target"),
        ("shift+f5", "Group"),
        ("shift+up", "Quick menu"),
        ("shift+`", "Target"),
    ] {
        reset();
        let _host = bind("alpha", "");
        refused(ROWS.len(), combo, who);
        assert_eq!(mod_key_of("alpha"), "", "{combo}");
    }
}

#[test]
fn round7e_fix2_h1_shift_space_does_not_fire_a_mod_with_jump() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    assert!(bindpage::set_key("alpha", "act", "shift+space"));
    let space = Key::Named(NamedKey::Space);
    assert_eq!(
        GameKeys::new().press(SHIFT, &space, None),
        Some(Sent::Key("space".into())),
        "the game takes Shift+Space as Space: Jump"
    );
    press_based(&mut rt, SHIFT, &space, None, &[&host]);
    hits(&host, 0);
    // control: a mod that no game control shares fires on its Shift combo
    assert!(bindpage::set_key("alpha", "act", "shift+u"));
    press_based(&mut rt, SHIFT, &Key::Character("U".into()), Some("u"), &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix2_h1_a_game_row_on_a_shifted_symbol_is_the_games_press() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "!");
    settings::set_binds(game);
    // Controls: the mod may not have Shift+1 (the game takes it as !)
    refused(ROWS.len(), "shift+1", "Bar 1: slot 1");
    assert!(bindpage::set_key("alpha", "act", "shift+1"));
    let bang = Key::Character("!".into());
    assert_eq!(GameKeys::new().press(SHIFT, &bang, Some("1")), Some(Sent::Slot(0, 0)));
    press_based(&mut rt, SHIFT, &bang, Some("1"), &[&host]);
    hits(&host, 0);
}

#[test]
fn round7e_fix2_h1_a_macro_on_a_shifted_symbol_is_the_games_press_too() {
    reset();
    let _host = bind("alpha", "");
    let mut game = settings::binds();
    game.set_trigger(0, None, vec![Input::Key("!".into())]).unwrap();
    settings::set_binds(game);
    refused(ROWS.len(), "shift+1", "Macro 1");
}

#[test]
fn round7e_fix2_m2_a_release_is_named_as_its_press_was() {
    reset();
    let host = bind("alpha", "");
    host.execute("log = {}; eqoa.events.on('input.key', function(e) log[#log + 1] = e.data.combo .. (e.data.down and '+' or '-') end)")
        .unwrap();
    let mut rt = runtime();
    let u = Key::Character("u".into());
    // Ctrl+U down, Ctrl let go, U let go
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
    host.execute("assert(table.concat(log, ',') == 'ctrl+u+,ctrl+u-,u+,u-', table.concat(log, ','))")
        .unwrap();
}

#[test]
fn round7e_fix2_m2_the_game_names_a_release_the_same_way() {
    // the mirror agrees: the game's own release is named as its press (viewer.rs key_names)
    reset();
    let mut game = GameKeys::new();
    let u = Key::Character("u".into());
    let mut b = settings::binds();
    b.bind_key(row_named("bar1_slot1").unwrap(), "ctrl+u");
    settings::set_binds(b);
    assert_eq!(game.name(CTRL, &u, None, true, false, false, false).as_deref(), Some("ctrl+u"));
    assert_eq!(game.name(NONE, &u, None, false, false, false, false).as_deref(), Some("ctrl+u"));
}

#[test]
fn round7e_fix2_l3_super_is_no_plain_key() {
    reset();
    let host = bind("alpha", "u");
    let mut rt = runtime();
    // Cmd+U is not a plain U for a mod
    input::set_super(true);
    press_based(&mut rt, NONE, &Key::Character("u".into()), None, &[&host]);
    input::set_super(false);
    hits(&host, 0);
    press_based(&mut rt, NONE, &Key::Character("u".into()), None, &[&host]);
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
    let host = bind("alpha", "u");
    let mut rt = runtime();
    press_based(&mut rt, NONE, &Key::Character("u".into()), None, &[&host]);
    hits(&host, 0);
    input::observe_modifiers(&WindowEvent::ModifiersChanged(Modifiers::from(ModifiersState::empty())));
    press_based(&mut rt, NONE, &Key::Character("u".into()), None, &[&host]);
    hits(&host, 1);
}

// ---- fix round 3: the press is exact, a table only fills in where no press exists -------------

const CTRL_SHIFT: (bool, bool, bool) = (true, false, true);

/// Whether the game acts on this press (a row or a macro on the name `App::key_name` gives it).
fn game_acts(mods: (bool, bool, bool), logical: &Key, base: Option<&'static str>) -> bool {
    let name = GameKeys::new().name(mods, logical, base, true, false, false, false).unwrap();
    let game = settings::binds();
    game.row_on(&name).is_some() || game.key_in_macros(&name)
}

#[test]
fn round7e_fix3_a_german_shift_7_types_slash_and_the_game_acts() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    assert!(bindpage::set_key("alpha", "act", "shift+7"));
    let slash = Key::Character("/".into());
    assert!(game_acts(SHIFT, &slash, Some("7")), "Shift+7 types /, the command key");
    press_based(&mut rt, SHIFT, &slash, Some("7"), &[&host]);
    hits(&host, 0);
    // control: where Shift+7 types & (US), nothing of the game's is on it and the mod fires
    let amp = Key::Character("&".into());
    assert!(!game_acts(SHIFT, &amp, Some("7")));
    press_based(&mut rt, SHIFT, &amp, Some("7"), &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix3_a_uk_shift_quote_types_at_and_the_game_acts() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    let slot = row_named("bar1_slot1").unwrap();
    let mut game = settings::binds();
    game.bind_key(slot, "@");
    settings::set_binds(game);
    assert!(bindpage::set_key("alpha", "act", "shift+'"));
    let at = Key::Character("@".into());
    assert!(game_acts(SHIFT, &at, Some("'")));
    press_based(&mut rt, SHIFT, &at, Some("'"), &[&host]);
    hits(&host, 0);
    let quote = Key::Character("\"".into());
    assert!(!game_acts(SHIFT, &quote, Some("'")));
    press_based(&mut rt, SHIFT, &quote, Some("'"), &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix3_the_numpad_with_numlock_off_arrives_as_end() {
    reset();
    let host = bind("alpha", "");
    let mut rt = runtime();
    // Target: self is on 1 by default; move it, so a wrong guess of "1" finds nothing
    let mut game = settings::binds();
    game.bind_key(row_named("target_self").unwrap(), "y");
    settings::set_binds(game);
    let end = Key::Named(NamedKey::End);
    for (mods, combo) in [(CTRL, "ctrl+1"), (SHIFT, "shift+1")] {
        assert!(bindpage::set_key("alpha", "act", combo));
        assert!(game_acts(mods, &end, Some("1")), "{combo}: the game takes it as End (Orbit right)");
        press_based(&mut rt, mods, &end, Some("1"), &[&host]);
        hits(&host, 0);
    }
    // control: the digit row's 1 with Ctrl is no game control now, and the mod fires
    assert!(bindpage::set_key("alpha", "act", "ctrl+1"));
    let one = Key::Character("1".into());
    assert!(!game_acts(CTRL, &one, Some("1")));
    press_based(&mut rt, CTRL, &one, Some("1"), &[&host]);
    hits(&host, 1);
}

#[test]
fn round7e_fix3_a_capture_uses_what_the_real_press_typed() {
    reset();
    let _host = bind("alpha", "");
    let mut rt = runtime();
    // a German Shift+7 pressed: the client learns it types /
    press_based(&mut rt, SHIFT, &Key::Character("/".into()), Some("7"), &[]);
    refused(ROWS.len(), "shift+7", "/");
    assert_eq!(mod_key_of("alpha"), "");
    // control: with no press seen, the US table says & (nothing's) and the key is accepted
    reset();
    let _host = bind("alpha", "");
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
    let u = Key::Character("u".into());
    input::set_modifiers(CTRL);
    rt.scripted_key_based(9201, &u, None, None, ElementState::Pressed, true);
    input::set_modifiers(NONE);
    // a menu has the keyboard when it is let go: nothing is sent, and it is forgotten
    rt.scripted_key_based(9201, &u, None, None, ElementState::Released, false);
    // later a plain U goes down in a menu and comes up in play: a plain U
    rt.scripted_key_based(9202, &u, None, None, ElementState::Pressed, false);
    rt.scripted_key_based(9202, &u, None, None, ElementState::Released, true);
    native_events::dispatch(&host).unwrap();
    host.execute("assert(table.concat(log, ',') == 'ctrl+u+,u-', table.concat(log, ','))").unwrap();
}

#[test]
fn round7e_fix3_modifier_order_in_a_saved_name_does_not_matter() {
    reset();
    testing::put(MOD_KEYS, b"mod.alpha.act=shift+ctrl+u\nmod.beta.act=ctrl+super+c\nmod.gamma.act=alt+super+enter\n");
    let alpha = bind("alpha", "");
    let _beta = bind("beta", "");
    let _gamma = bind("gamma", "");
    assert_eq!(mod_key_of("alpha"), "ctrl+shift+u", "read in the order the client names a press");
    assert_eq!(mod_key_of("beta"), "", "ctrl+super+c is the client's however it is written");
    assert_eq!(mod_key_of("gamma"), "", "alt+super+enter too");
    let mut rt = runtime();
    press_based(&mut rt, CTRL_SHIFT, &Key::Character("U".into()), Some("u"), &[&alpha]);
    hits(&alpha, 1);
}

#[test]
fn round7e_fix3_nothing_listening_means_no_key_work() {
    use super::super::perf_alloc::measure;
    reset();
    bindpage::reset_in_memory();
    let mut rt = runtime();
    let u = Key::Character("u".into());
    // no mod at all: the runtime keeps nothing of the key
    input::set_modifiers(CTRL);
    rt_key(&mut rt, 9301, true, &u, Some("u"));
    rt_key(&mut rt, 9301, false, &u, Some("u"));
    let (_, _, allocs, _) = measure(|| {
        for _ in 0..300 {
            rt_key(&mut rt, 9301, true, &u, Some("u"));
            rt_key(&mut rt, 9301, false, &u, Some("u"));
        }
    });
    input::set_modifiers(NONE);
    assert_eq!(allocs, 0, "no mod: no allocation on the key path");
    assert_eq!(input::testing_key_ring_len(), 0, "no mod: no key ring kept");
    // one mod that is on, with no key bound and no input.key listener
    let _quiet = option_test_host("quiet");
    input::set_modifiers(CTRL);
    input::route_mod_key_based(&u, Some("u"), true, false, false, true, false);
    input::route_mod_key_based(&u, Some("u"), false, false, false, true, false);
    let (_, _, allocs, _) = measure(|| {
        for _ in 0..300 {
            input::route_mod_key_based(&u, Some("u"), true, false, false, true, false);
            input::route_mod_key_based(&u, Some("u"), false, false, false, true, false);
        }
    });
    input::set_modifiers(NONE);
    assert_eq!(allocs, 0, "a mod with nothing bound or listening: no allocation");
    // control: a mod that has a key bound does the work
    let _loud = bind("loud", "");
    input::set_modifiers(CTRL);
    let (_, _, allocs, _) = measure(|| {
        input::route_mod_key_based(&u, Some("u"), true, false, false, true, false);
        input::route_mod_key_based(&u, Some("u"), false, false, false, true, false);
    });
    input::set_modifiers(NONE);
    assert!(allocs > 0, "the meter sees a listening mod's key work");
}

#[test]
fn round7e_fix3_the_marks_use_what_presses_showed_and_nothing_is_written() {
    reset();
    let _host = bind("alpha", "");
    let mut rt = runtime();
    assert!(bindpage::set_key("alpha", "act", "shift+7"));
    let before = testing::kept(MOD_KEYS);
    // no press seen: the US table says Shift+7 types &, which nothing has: no mark
    assert!(!controls_text(ROWS.len()).contains(&"!".into()));
    // a German Shift+7 is pressed: it types /, the command key, and the row is marked
    press_based(&mut rt, SHIFT, &Key::Character("/".into()), Some("7"), &[]);
    assert!(controls_text(ROWS.len()).contains(&"!".into()), "marked once a press showed it");
    assert_eq!(testing::kept(MOD_KEYS), before, "what was learned is not written anywhere");
}
