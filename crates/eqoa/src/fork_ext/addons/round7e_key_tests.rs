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
