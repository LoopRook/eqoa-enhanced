// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1: the modern UI's contract without a running game: window sessions, pass, pad focus, the question that
//! destroy and drop-quest always ask, the "Provides" label, the keys. The `Play`-driven parts (the Yes that really
//! sends, the game's own window opening again) are in `fork_ext/a1_play_tests.rs`.
//!
//! Each test names its negative control: what it would see if the change were not there.
use super::{
    confirm, discovery::DiscoveredAddon, host::Host, native_core, native_events,
    native_tests::{A1_EVENTS, install_field_check}, window_sessions as ws,
    window_takeover::{self, TakeoverKind},
};
use serde_json::json;

const SESSION_CAPS: &[&str] = &["eqoa.windows.session", "eqoa.windows.focus", "eqoa.events.on", "eqoa.windows.takeover"];

fn fresh() {
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    native_core::fixture_demand(true);
}

fn host(caps: &[&str]) -> Host {
    Host::with_test_capabilities(caps).unwrap()
}

fn pump(h: &Host) {
    native_events::dispatch(h).unwrap();
}

fn text(h: &Host, expr: &str) -> String {
    h.lua.load(format!("return tostring({expr})")).eval().unwrap()
}

/// The switch lives in the fork's settings file: a test that turns it on must leave it off.
struct SwitchOff;
impl Drop for SwitchOff {
    fn drop(&mut self) {
        window_takeover::set_use_game_windows(false);
    }
}

#[test]
fn a1_a_mod_opens_its_own_window_and_the_session_is_the_same_until_it_closes() {
    fresh();
    let h = host(SESSION_CAPS);
    h.execute(
        r#"
        s, e = eqoa.windows.open_session('spellbook', {opened_from = 'key'})
        assert(e == nil and s.kind == 'spellbook' and s.origin == 'mod' and s.opened_from == 'key', tostring(e and e.message))
        assert(type(s.ref) == 'string' and s.title == 'Abilities' and s.focused == false)
        again = eqoa.windows.open_session('spellbook')
        assert(again.ref == s.ref, 'one session per window')
        assert(eqoa.windows.session('spellbook').ref == s.ref)
        other = eqoa.windows.open_session('inventory')
        assert(other.ref ~= s.ref)
        assert(eqoa.windows.session('quest_log') == nil)
        local closed = eqoa.windows.close_session(s.ref)
        assert(closed == true)
        assert(eqoa.windows.session('spellbook') == nil, 'closed means gone')
        assert(eqoa.windows.close_session(s.ref) == false, 'closing twice finds nothing')
    "#,
    )
    .unwrap();
}

#[test]
fn a1_open_session_refuses_what_it_should_and_says_why() {
    fresh();
    let h = host(SESSION_CAPS);
    h.execute(
        r#"
        local function code(kind) local s, e = eqoa.windows.open_session(kind); assert(s == nil); return e.code end
        assert(code('merchant') == 'invalid_argument', 'a server window is held with takeover, not opened')
        assert(code('no_such_window') == 'invalid_argument')
        assert(code('trainer') == 'unsupported', 'a real window this version cannot show for a mod')
        local s, e = eqoa.windows.open_session('spellbook', {opened_from = 'elsewhere'})
        assert(s == nil and e.code == 'invalid_argument')
    "#,
    )
    .unwrap();
    // the negative control: a mod without the capability gets nothing, and says which
    let bare = host(&["eqoa.events.on"]);
    bare.execute("local s, e = eqoa.windows.open_session('spellbook'); assert(s == nil and e.code == 'not_declared' or e.code == 'permission_denied', e.code)").unwrap();
}

#[test]
fn a1_pass_hands_back_only_what_a_mod_holds_hidden() {
    fresh();
    let h = host(SESSION_CAPS);
    h.execute("s = eqoa.windows.open_session('inventory'); local ok, e = eqoa.windows.pass(s.ref); assert(ok == nil and e.code == 'unsupported', e and e.code)").unwrap();
    // a corpse the mod holds hidden is a session too, and can be passed
    let vm = native_events::bus(&h.lua).borrow().vm;
    assert!(window_takeover::takeover(vm, "x", TakeoverKind::Loot, window_takeover::TakeoverScope::Own).is_ok());
    assert!(window_takeover::activate_hidden_for_test(vm, TakeoverKind::Loot, 7));
    ws::sync_held([window_takeover::hidden_holder(TakeoverKind::Loot), None, None]);
    h.execute(
        r#"
        held = eqoa.windows.session('loot')
        assert(held ~= nil and held.origin == 'server' and held.opened_from == 'npc', 'the hidden corpse is a session')
        passed, e = eqoa.windows.pass(held.ref)
        assert(passed == true, e and e.message)
    "#,
    )
    .unwrap();
    assert_eq!(ws::take_passes(), vec![(vm, TakeoverKind::Loot)], "the pass waits for the client to open the game's window");
    // the client does it: the hidden session is handed back, the claim stays, the session ends as passed
    assert!(window_takeover::hand_back(vm, TakeoverKind::Loot));
    ws::finish_pass(vm, TakeoverKind::Loot);
    assert!(!window_takeover::is_hidden_active(TakeoverKind::Loot));
    assert!(window_takeover::is_taken(TakeoverKind::Loot), "the mod keeps its claim for the next opening");
    h.execute("assert(eqoa.windows.session('loot') == nil)").unwrap();
    // control: a pass from a mod that does not hold it
    let other = host(SESSION_CAPS);
    other.execute("local ok, e = eqoa.windows.pass('ws-1'); assert(ok == nil and e.code == 'stale_handle')").unwrap();
}

#[test]
fn a1_a_closed_session_is_told_to_its_own_mod_only_and_with_the_reason() {
    fresh();
    let a = host(SESSION_CAPS);
    let b = host(SESSION_CAPS);
    for h in [&a, &b] {
        h.execute(
            "log = {}; eqoa.events.on('windows.session', function(e) log[#log + 1] = e.data.phase .. ':' .. e.data.kind .. ':' .. tostring(e.data.reason) end)",
        )
        .unwrap();
    }
    a.execute("s = eqoa.windows.open_session('quest_log')").unwrap();
    pump(&a);
    pump(&b);
    assert_eq!(text(&a, "table.concat(log, ',')"), "opened:quest_log:nil");
    assert_eq!(text(&b, "#log"), "0", "the other mod hears nothing of a window that is not its own");
    // the character leaving the world ends it, and says so
    ws::set_env(ws::Env { away: true, ..ws::Env::default() });
    pump(&a);
    pump(&b);
    assert_eq!(text(&a, "table.concat(log, ',')"), "opened:quest_log:nil,closed:quest_log:left_world");
    assert_eq!(text(&b, "#log"), "0");
    a.execute("assert(eqoa.windows.session('quest_log') == nil)").unwrap();
}

#[test]
fn a1_zoning_death_leaving_the_world_and_the_players_switch_each_end_open_windows() {
    fresh();
    let _off = SwitchOff;
    let h = host(SESSION_CAPS);
    h.execute("log = {}; eqoa.events.on('windows.session', function(e) if e.data.phase == 'closed' then log[#log + 1] = e.data.reason end end)").unwrap();
    for (what, end) in [
        ("death", Box::new(|| ws::set_env(ws::Env { dead: true, ..ws::Env::default() })) as Box<dyn Fn()>),
        ("left_world", Box::new(|| ws::set_env(ws::Env { away: true, ..ws::Env::default() }))),
        ("game_windows", Box::new(|| window_takeover::set_use_game_windows(true))),
    ] {
        ws::set_env(ws::Env::default());
        window_takeover::set_use_game_windows(false);
        h.execute("s = assert(eqoa.windows.open_session('spellbook'))").unwrap();
        end();
        pump(&h);
        h.execute("assert(eqoa.windows.session('spellbook') == nil)").unwrap();
        assert_eq!(text(&h, "log[#log]"), what, "the reason the window ended");
    }
    // a world change (the core's reset) ends what belongs to the server's windows, not a view the mod opened itself:
    // that one closes when the character is out of the world (the `left_world` row above)
    ws::set_env(ws::Env::default());
    window_takeover::set_use_game_windows(false);
    h.execute("s = assert(eqoa.windows.open_session('spellbook'))").unwrap();
    window_takeover::release_all();
    h.execute("assert(eqoa.windows.session('spellbook') ~= nil)").unwrap();
    // while the switch is on, or dead, or away, no mod window opens (the control: it opened a moment ago)
    h.execute("eqoa.windows.close_session(s.ref)").unwrap();
    window_takeover::set_use_game_windows(true);
    h.execute("local s, e = eqoa.windows.open_session('spellbook'); assert(s == nil and e.code == 'unavailable', e and e.code)").unwrap();
    window_takeover::set_use_game_windows(false);
    ws::set_env(ws::Env { dead: true, ..ws::Env::default() });
    h.execute("local s, e = eqoa.windows.open_session('spellbook'); assert(s == nil and e.code == 'not_ready', e and e.code)").unwrap();
    ws::set_env(ws::Env::default());
    h.execute("assert(eqoa.windows.open_session('spellbook'))").unwrap();
}

#[test]
fn a1_disabling_or_reloading_a_mod_takes_its_windows_focus_and_question_with_it() {
    fresh();
    let h = host(SESSION_CAPS);
    h.execute("s = assert(eqoa.windows.open_session('inventory')); assert(eqoa.windows.focus(s.ref))").unwrap();
    let vm = native_events::bus(&h.lua).borrow().vm;
    confirm::open(vm, "x", ws::get(vm, "inventory").map(|s| s.id), confirm::Act::Destroy { inv_index: 1, name: "Rock".into(), count: 1 }, "Destroy Rock?".into(), 0).unwrap();
    assert!(confirm::is_open());
    native_events::clear(&h);
    assert!(ws::get(vm, "inventory").is_none());
    assert!(!confirm::is_open(), "the mod is gone: its question goes with it");
    let mut report = crate::input::Report { buttons: crate::input::bit::CROSS, ..Default::default() };
    ws::apply_pad(&mut report);
    assert_eq!(report.buttons, crate::input::bit::CROSS, "and the controller is the game's again");
}

#[test]
fn a1_focus_takes_the_menu_buttons_and_gives_them_back() {
    use crate::input::{Report, bit};
    fresh();
    let h = host(SESSION_CAPS);
    h.execute(
        r#"
        keys = {}
        eqoa.events.on('windows.key', function(e) keys[#keys + 1] = e.data.key .. (e.data.down and '+' or '-') end)
        s = assert(eqoa.windows.open_session('inventory'))
    "#,
    )
    .unwrap();
    let press = |buttons: u16, sticks: [f32; 4]| {
        let mut r = Report { buttons, sticks, ..Default::default() };
        ws::apply_pad(&mut r);
        r
    };
    // the control: no focus, the game gets every button
    let r = press(bit::CROSS | bit::UP | bit::SQUARE, [0.5; 4]);
    assert_eq!(r.buttons, bit::CROSS | bit::UP | bit::SQUARE);
    h.execute("assert(eqoa.windows.focus(s.ref, {movement = false}))").unwrap();
    // focus: confirm and d-pad are the window's, square is still the game's, and movement is held
    let r = press(bit::CROSS | bit::UP | bit::SQUARE, [0.5; 4]);
    assert_eq!(r.buttons, bit::SQUARE, "the window's buttons never reach the game");
    assert_eq!(r.sticks, [0.0; 4], "movement = false holds the sticks");
    press(bit::SQUARE, [0.0; 4]);
    pump(&h);
    assert_eq!(text(&h, "table.concat(keys, ',')"), "up+,confirm+,up-,confirm-");
    h.execute("keys = {}").unwrap();
    // focus with movement left on does not hold the walk
    h.execute("assert(eqoa.windows.focus(s.ref))").unwrap();
    assert_eq!(press(bit::DOWN, [0.5; 4]).sticks, [0.5; 4]);
    press(0, [0.0; 4]);
    pump(&h);
    // a game window or box up suspends it: the buttons are the game's, and a button held when it ends is no press
    pump(&h);
    ws::set_env(ws::Env { plain: false, ..ws::Env::default() });
    assert_eq!(press(bit::CROSS, [0.0; 4]).buttons, bit::CROSS);
    ws::set_env(ws::Env::default());
    pump(&h);
    h.execute("keys = {}").unwrap();
    assert_eq!(press(bit::CROSS, [0.0; 4]).buttons, 0);
    pump(&h);
    assert_eq!(text(&h, "#keys"), "0", "a button already down when the focus came back is no press");
    // the system took the window's focus: released
    crate::fork_ext::addons::input::focus(false);
    assert_eq!(press(bit::CROSS, [0.0; 4]).buttons, bit::CROSS);
}

#[test]
fn a1_one_window_holds_the_controller_and_closing_it_gives_it_back() {
    fresh();
    let h = host(SESSION_CAPS);
    let g = host(SESSION_CAPS);
    h.execute("s = assert(eqoa.windows.open_session('inventory')); assert(eqoa.windows.focus(s.ref))").unwrap();
    g.execute("t = assert(eqoa.windows.open_session('spellbook')); local ok, e = eqoa.windows.focus(t.ref); assert(ok == nil and e.code == 'busy', e and e.code)").unwrap();
    h.execute("assert(eqoa.windows.close_session(s.ref))").unwrap();
    g.execute("assert(eqoa.windows.focus(t.ref))").unwrap();
    // a mod cannot focus a session it does not have
    h.execute("local ok, e = eqoa.windows.focus('ws-99'); assert(ok == nil and e.code == 'stale_handle')").unwrap();
    // and gives it back itself
    g.execute("assert(eqoa.windows.release_focus(t.ref) == true); assert(eqoa.windows.release_focus(t.ref) == false)").unwrap();
    h.execute("assert(eqoa.windows.open_session('quest_log'))").unwrap();
}

#[test]
fn a1_the_question_starts_on_no_and_only_keys_the_player_presses_answer_it() {
    use winit::keyboard::{Key, NamedKey};
    fresh();
    let named = |k: NamedKey| Key::Named(k);
    let open = || confirm::open(1, "mod", None, confirm::Act::DropQuest { index: 1, title: "Bat Eyes".into(), stamp: 1, revision: 1 }, "Drop?".into(), 0).unwrap();
    open();
    assert!(!confirm::current_for_test().unwrap().yes, "it starts on No");
    // Enter on the starting choice is No (an accidental Enter is harmless)
    assert!(confirm::key(&named(NamedKey::Enter)));
    assert!(confirm::take_answered().is_empty(), "No acts on nothing");
    assert!(!confirm::is_open());
    // Right chooses Yes; Enter then answers it
    open();
    assert!(confirm::key(&named(NamedKey::ArrowRight)));
    assert!(confirm::is_open() && confirm::current_for_test().unwrap().yes);
    assert!(confirm::key(&named(NamedKey::Tab)));
    assert!(!confirm::current_for_test().unwrap().yes, "Tab flips it");
    assert!(confirm::key(&named(NamedKey::ArrowRight)));
    assert!(confirm::key(&named(NamedKey::Enter)));
    assert_eq!(confirm::take_answered().len(), 1, "only the player's Yes is carried out");
    // Escape is No even with Yes chosen
    open();
    confirm::key(&named(NamedKey::ArrowRight));
    assert!(confirm::key(&named(NamedKey::Escape)));
    assert!(confirm::take_answered().is_empty());
    // keys that are not the question's are the game's: the walk is not held
    open();
    assert!(!confirm::key(&Key::Character("w".into())));
    assert!(!confirm::key(&named(NamedKey::Space)));
    // a click on Yes is the other way a player answers
    confirm::click(true);
    assert_eq!(confirm::take_answered().len(), 1);
}

#[test]
fn a1_a_second_question_waits_and_time_running_out_is_no() {
    fresh();
    let act = || confirm::Act::Destroy { inv_index: 1, name: "Rock".into(), count: 1 };
    assert!(confirm::open(1, "a", None, act(), "x".into(), 0).is_ok());
    let busy = confirm::open(2, "b", None, act(), "y".into(), 0).unwrap_err();
    assert_eq!(busy.0, "busy");
    assert!(!confirm::expire(), "not yet");
    confirm::age_for_test(confirm::TIMEOUT);
    assert!(confirm::expire(), "30 seconds is No");
    assert!(confirm::take_answered().is_empty());
    assert!(confirm::open(2, "b", None, act(), "y".into(), 0).is_ok(), "and the next one may ask");
}

#[test]
fn a1_the_question_is_asked_only_to_a_living_player_in_the_world_and_ends_with_the_mod_window() {
    fresh();
    let act = || confirm::Act::Destroy { inv_index: 1, name: "Rock".into(), count: 1 };
    ws::set_env(ws::Env { dead: true, ..ws::Env::default() });
    assert_eq!(confirm::open(1, "a", None, act(), "x".into(), 0).unwrap_err().0, "not_ready");
    ws::set_env(ws::Env { away: true, ..ws::Env::default() });
    assert_eq!(confirm::open(1, "a", None, act(), "x".into(), 0).unwrap_err().0, "not_ready");
    // over a game window it is modal, not refused (the control: it was refused only for the two states above)
    ws::set_env(ws::Env { plain: false, ..ws::Env::default() });
    assert!(confirm::open(1, "a", None, act(), "x".into(), 0).is_ok());
    confirm::cancel_for_vm(1);
    ws::set_env(ws::Env::default());
    let h = host(SESSION_CAPS);
    h.execute("s = assert(eqoa.windows.open_session('inventory'))").unwrap();
    let vm = native_events::bus(&h.lua).borrow().vm;
    let sid = ws::get(vm, "inventory").unwrap().id;
    h.execute("log = {}; eqoa.events.on('confirm.closed', function(e) log[#log + 1] = e.data.kind .. ':' .. e.data.answer end)").unwrap();
    confirm::open(vm, "a", Some(sid), act(), "x".into(), 0).unwrap();
    h.execute("assert(eqoa.windows.close_session(s.ref))").unwrap();
    assert!(!confirm::is_open(), "the window closed: the question is No");
    pump(&h);
    assert_eq!(text(&h, "log[1]"), "destroy:closed");
}

fn discovered(id: &str) -> DiscoveredAddon {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(id);
    let found = super::discovery::discover(root.parent().unwrap());
    found.addons.into_iter().find(|a| a.id == id).unwrap_or_else(|| panic!("{id} is bundled: {:?}", found.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>()))
}

#[test]
fn a1_mods_and_addons_says_what_a_mod_provides_before_it_is_enabled() {
    let lines = |id: &str| super::runtime::disclosure_lines(&discovered(id));
    let spell = lines("eqoa_spellbook");
    assert!(spell.contains(&"Provides: Spellbook window (Shift+K)".to_string()), "{spell:?}");
    let journal = lines("eqoa_quest_journal");
    assert!(journal.contains(&"Provides: Quest Journal window (Shift+J)".to_string()), "{journal:?}");
    assert!(journal.contains(&"Asks you first: drop a quest".to_string()), "{journal:?}");
    // the label never says drop quest acts on its own
    assert!(!journal.iter().any(|l| l.starts_with("Actions: can act on its own") && l.contains("drop_quest")), "{journal:?}");
    // the control: a mod that provides nothing says nothing
    assert!(!lines("eqoa_stats_box").iter().any(|l| l.starts_with("Provides:")));
    // the label comes from the manifest and is shown as written: a bad entry is refused
    let temp = std::env::temp_dir().join(format!("a1-provides-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(temp.join("p")).unwrap();
    std::fs::write(temp.join("p/main.lua"), "").unwrap();
    for (provides, ok) in [(json!(["Bags (B)"]), true), (json!(["line\nbreak"]), false), (json!([""]), false), (json!(["x".repeat(49)]), false)] {
        std::fs::write(
            temp.join("p/mod.json"),
            json!({"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"p","name":"p","entry":"main.lua","capabilities":[],"provides":provides}).to_string(),
        )
        .unwrap();
        let found = super::discovery::discover(&temp);
        assert_eq!(found.addons.len() == 1, ok, "{provides}: {:?}", found.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>());
    }
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn a1_the_three_windows_take_their_keys_and_the_stats_box_gives_up_b() {
    use crate::fork_ext::bindpage;
    bindpage::reset_in_memory();
    for (addon, default) in [("eqoa_stats_box", "y"), ("eqoa_bag_viewer", "b"), ("eqoa_spellbook", "shift+k"), ("eqoa_quest_journal", "shift+j")] {
        let manifest = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(addon).join(if addon == "eqoa_bag_viewer" { "main.lua" } else { "main.lua" })).unwrap();
        assert!(manifest.contains(&format!("default_key = \"{default}\"")), "{addon} declares {default}");
        bindpage::register(addon, if addon == "eqoa_stats_box" { "toggle" } else { "open" }, default, addon);
    }
    assert_eq!(bindpage::get_key("eqoa_stats_box", "toggle").as_deref(), Some("y"));
    assert_eq!(bindpage::get_key("eqoa_bag_viewer", "open").as_deref(), Some("b"));
    assert_eq!(bindpage::get_key("eqoa_spellbook", "open").as_deref(), Some("shift+k"));
    assert_eq!(bindpage::get_key("eqoa_quest_journal", "open").as_deref(), Some("shift+j"));
    // a game key always wins: plain K is the camera's, so a mod default on it gets an empty row
    bindpage::register("eqoa_x", "k", "k", "Plain K");
    assert_eq!(bindpage::get_key("eqoa_x", "k").as_deref(), Some(""), "the game's camera key is not a mod's");
    // two mods wanting the same free key: one has it, the other's row is empty (never both)
    bindpage::register("eqoa_y", "dup", "shift+j", "Second journal");
    let holders = ["eqoa_quest_journal", "eqoa_y"].map(|a| bindpage::get_key(a, if a == "eqoa_y" { "dup" } else { "open" }).unwrap());
    assert_eq!(holders.iter().filter(|k| !k.is_empty()).count(), 1, "{holders:?}");
    // control: with the old default both would want B and one of them would have an empty row (the Stats Box would
    // lose it here, by name order: that is what moving it to Y avoids)
    bindpage::reset_in_memory();
    bindpage::register("eqoa_stats_box", "toggle", "b", "old default");
    bindpage::register("eqoa_bag_viewer", "open", "b", "bags");
    let both = [bindpage::get_key("eqoa_bag_viewer", "open").unwrap(), bindpage::get_key("eqoa_stats_box", "toggle").unwrap()];
    assert_eq!(both.iter().filter(|k| k.is_empty()).count(), 1, "{both:?}");
    bindpage::reset_in_memory();
}

#[test]
fn a1_every_new_event_reaches_lua_with_its_declared_fields_and_the_records_match() {
    use crate::input::{Report, bit};
    fresh();
    let h = host(SESSION_CAPS);
    install_field_check(&h).unwrap();
    h.lua.globals().set("a1_events", h.lua.create_sequence_from(A1_EVENTS.iter().copied()).unwrap()).unwrap();
    h.execute(
        r#"
        seen = {}
        for _, name in ipairs(a1_events) do
            assert(eqoa.capabilities.has('eqoa.events.' .. name), name)
            assert(eqoa.events.on(name, function(e) seen[name] = e; round7c_note(name, e) end))
        end
        s = assert(eqoa.windows.open_session('inventory', {opened_from = 'key'}))
        fields('windows.Session', s)
        assert(eqoa.windows.focus(s.ref))
        "#,
    )
    .unwrap();
    pump(&h);
    // a button the focused window holds
    let mut report = Report { buttons: bit::CROSS, ..Default::default() };
    ws::apply_pad(&mut report);
    pump(&h);
    // a question: put, read, answered Yes by the player, and carried out
    let vm = native_events::bus(&h.lua).borrow().vm;
    confirm::open(vm, "x", ws::get(vm, "inventory").map(|s| s.id), confirm::Act::Destroy { inv_index: 1, name: "Rock".into(), count: 1 }, "Destroy Rock?".into(), 0).unwrap();
    h.execute("fields('windows.Confirm', eqoa.windows.confirm())").unwrap();
    confirm::key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight));
    confirm::key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter));
    let yes = confirm::take_answered();
    assert_eq!(yes.len(), 1);
    confirm::report_done(&yes[0], "yes", json!({"sent": true}));
    // the focus is taken away, and the window closes
    crate::fork_ext::addons::input::focus(false);
    h.execute("assert(eqoa.windows.close_session(s.ref))").unwrap();
    pump(&h);
    h.execute(
        r#"
        local missing = {}
        for _, name in ipairs(a1_events) do if not seen[name] then missing[#missing + 1] = name end end
        assert(#missing == 0, 'not emitted: ' .. table.concat(missing, ', '))
        event_fields(seen, a1_events)
        "#,
    )
    .unwrap();
}

/// The Lua state has an 8 MiB ceiling (`host.rs`). The new bindings are a few dozen small functions: they must leave
/// the base host far from it, because the forge host test that tries a 32 MiB string and then loads a module is
/// the first to notice a host that has grown.
#[test]
fn a1_the_new_bindings_leave_the_lua_ceiling_far_away() {
    let h = Host::new().unwrap();
    let used = h.lua.used_memory();
    eprintln!("A1 base host Lua memory: {used} bytes of {}", 8 * 1024 * 1024);
    assert!(used < 6 * 1024 * 1024, "the base host already uses {used} bytes");
}

#[test]
fn a1_fix1_a_stopped_runtime_gives_the_controller_back_and_ends_sessions_and_the_question() {
    use crate::input::{Report, bit};
    fresh();
    let h = host(SESSION_CAPS);
    h.execute("s = assert(eqoa.windows.open_session('inventory')); assert(eqoa.windows.focus(s.ref))").unwrap();
    let vm = native_events::bus(&h.lua).borrow().vm;
    confirm::open(vm, "x", ws::get(vm, "inventory").map(|s| s.id), confirm::Act::Destroy { inv_index: 1, name: "Rock".into(), count: 1 }, "Destroy Rock?".into(), 0).unwrap();
    let mut r = Report { buttons: bit::CROSS, ..Default::default() };
    ws::apply_pad(&mut r);
    assert_eq!(r.buttons, 0, "control: the mod holds the controller");
    // `Runtime` calls this where it sets its error (the frame failed, no mod window is drawn now)
    ws::mods_stopped("ended");
    assert!(!confirm::is_open() && ws::sessions_for_test().is_empty() && !ws::is_active());
    let mut r = Report { buttons: bit::CROSS | bit::UP, ..Default::default() };
    ws::apply_pad(&mut r);
    assert_eq!(r.buttons, bit::CROSS | bit::UP, "the game's buttons are the game's again");
}
