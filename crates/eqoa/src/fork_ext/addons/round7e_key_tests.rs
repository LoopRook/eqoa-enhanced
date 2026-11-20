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

/// What the game does with a key pressed with `mods` held: the name `App::key_name`
/// gives it (a combo where a row or a macro is on that, else the key's own), through
/// the real key dispatch.
fn game_sends(mods: (bool, bool, bool), key: &Key) -> Option<Sent> {
    let plain = binds::name_of(key).unwrap();
    let game = settings::binds();
    let (c, a, s) = mods;
    let name = if c || a || s {
        let combo = binds::combo_name(c, a, s, &plain);
        if game.combo_bound(&combo) { combo } else { plain }
    } else {
        plain
    };
    KeyEdge::default().key(&name, true, false, false, &game)
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
