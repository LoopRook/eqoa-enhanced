// 2026-11-24: every bundled mod that draws a window leaves the screen to the game's own full map (M). Included by a6_tests.rs.

fn map_state(open: bool) {
    CORE.with(|c| {
        c.borrow_mut().view.map = json!({"enabled": true, "open": open, "minimap": !open, "markers_on": true, "world_id": 1, "revision": if open { 2 } else { 3 }})
    });
}

#[test]
fn hide_under_map_chat_draws_nothing_ends_a_typed_line_and_returns_as_it_was() {
    let (host, bridge, mut context) = chat_with_long_lines(10);
    map_state(false);
    frames(&host, &bridge, &mut context, 3);
    host.execute("chat_probe.state.tab = 'say'; chat_probe.state.scroll = 1").unwrap();
    // a line is being typed
    let vm = native_events::bus(&host.lua).borrow().vm;
    ws::open_client(vm, "eqoa_chat", "chat_entry", "key", json!({"channel": "say", "text": "half a thought"}));
    native_core::flush_deferred();
    let shown = frames(&host, &bridge, &mut context, 3);
    assert!(shown > 0, "control: the window is drawn with the map closed");
    let typing: bool = host.lua.load("return chat_probe.state.session ~= nil").eval().unwrap();
    assert!(typing, "control: the line is open");
    let hit_before: bool = host.lua.load("return chat_probe.hit('tab:all') ~= nil").eval().unwrap();
    assert!(hit_before);

    map_state(true);
    assert_eq!(frames(&host, &bridge, &mut context, 3), 0, "map up: the chat draws nothing");
    let (typing, hit, tab, scroll): (bool, bool, String, i64) = host
        .lua
        .load("return chat_probe.state.session ~= nil, chat_probe.hit('tab:all') ~= nil, chat_probe.state.tab, chat_probe.state.scroll")
        .eval()
        .unwrap();
    assert!(!typing, "the line being typed ended, as the game's would: the keyboard is the game's");
    assert!(!hit, "no tab or button can be clicked under the map");
    assert_eq!((tab.as_str(), scroll), ("say", 1), "its tab and scroll are kept");
    // a key and a press under the map reach nothing
    key(&host, "return", "");
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": 100.0, "y": 600.0, "inside_window": true, "button": "left"})));
    frames(&host, &bridge, &mut context, 2);

    map_state(false);
    assert!(frames(&host, &bridge, &mut context, 3) > 0, "map closed: drawn again");
    let (tab, scroll): (String, i64) = host.lua.load("return chat_probe.state.tab, chat_probe.state.scroll").eval().unwrap();
    assert_eq!((tab.as_str(), scroll), ("say", 1), "exactly as it was");
    assert!(host.drain_logs().unwrap().is_empty());
}

#[test]
fn hide_under_map_start_menu_draws_nothing_under_the_map_and_is_drawn_again_after() {
    fresh();
    let mut context = context();
    let host = Host::for_addon(&addon("eqoa_start_menu")).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon("eqoa_start_menu")).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    map_state(false);
    ws::open_client(vm, "eqoa_start_menu", "start_menu", "key", json!({"entries": [{"text": "Quit", "enabled": true, "command": "0xa010000:0"}]}));
    native_core::flush_deferred();
    let shown = frames(&host, &bridge, &mut context, 3);
    assert!(shown > 0, "control: the menu is drawn with the map closed");
    map_state(true);
    assert_eq!(frames(&host, &bridge, &mut context, 3), 0, "map up: nothing is drawn");
    // a key under the map does not run the entry
    action_broker::reset_all_for_test();
    key(&host, "return", "");
    assert!(action_broker::queued_kinds_for_test().is_empty(), "no key reaches the hidden menu");
    map_state(false);
    assert_eq!(frames(&host, &bridge, &mut context, 3), shown, "map closed: the same menu again");
}

/// A bundled mod that draws gets the map rule: every `render` handler of a mod that draws windows is guarded (or the mod is
/// one of the map's own overlays, or draws nothing).
#[test]
fn hide_under_map_every_drawing_bundled_mod_has_the_rule() {
    // the map's own overlays and mods that draw nothing stay as they are; the Forge UIs are vendored (see the report)
    let exempt = ["eqoa_map_search", "eqoa_round_minimap", "eqoa_creature_journal"];
    let vendored = ["arpg_ui", "avoids_modern_ui", "wereoxxs_ui"];
    let mut checked = 0;
    for entry in std::fs::read_dir(root("eqoa_chat").parent().unwrap()).unwrap() {
        let dir = entry.unwrap().path();
        let id = dir.file_name().unwrap().to_string_lossy().to_string();
        if !dir.join("mod.json").exists() || exempt.contains(&id.as_str()) || vendored.contains(&id.as_str()) {
            continue;
        }
        let mut src = String::new();
        for f in ["main.lua", "modules/character.lua"] {
            if let Ok(t) = std::fs::read_to_string(dir.join(f)) {
                src += &t;
            }
        }
        if !src.contains("\"render\"") {
            continue;
        }
        checked += 1;
        assert!(src.contains("local function map_up()") && src.contains("map_up()"), "{id} draws and has no map rule");
        assert!(src.contains("m.open == true"), "{id} reads the game's map through eqoa.map.get().open");
    }
    assert!(checked >= 14, "the check saw {checked} mods");
}

/// A box being typed in lets go of the keyboard the moment the game's map is up (so M and Esc reach the game), and the
/// window is drawn again, not typing, when the map closes. Control: while the map is closed the capture is held.
fn capture_released_under_map(id: &str, probe: &str, typing: &str) {
    fresh();
    let mut context = context();
    let host = Host::for_addon(&addon(id)).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon(id)).unwrap();
    map_state(false);
    CORE.with(|c| c.borrow_mut().emit("mod.command", json!({"from": "test", "command": "open", "args": {}})));
    frames(&host, &bridge, &mut context, 3);
    host.execute(&format!("{probe}.state.{typing} = true")).unwrap();
    let shown = frames(&host, &bridge, &mut context, 3);
    assert!(shown > 0, "{id}: control: the window is drawn");
    assert!(super::input::is_text_capture_active(), "{id}: control: the box holds the keyboard");
    map_state(true);
    frames(&host, &bridge, &mut context, 1);
    assert!(!super::input::is_text_capture_active(), "{id}: the keyboard is the game's the moment the map is up");
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0, "{id}: nothing drawn under the map");
    map_state(false);
    assert!(frames(&host, &bridge, &mut context, 3) > 0, "{id}: drawn again");
    let open: bool = host.lua.load(&format!("return {probe}.state.open")).eval().unwrap();
    assert!(open, "{id}: still open");
    assert!(host.drain_logs().unwrap().is_empty());
}

#[test]
fn hide_under_map_spellbook_lets_go_of_the_keyboard_at_once() {
    capture_released_under_map("eqoa_spellbook", "spellbook_probe", "focus");
}

#[test]
fn hide_under_map_crafting_lets_go_of_the_keyboard_at_once() {
    capture_released_under_map("eqoa_crafting", "crafting_probe", "typing");
}
