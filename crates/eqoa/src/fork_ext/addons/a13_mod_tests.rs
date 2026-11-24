//! Round A13's bundled mod, EQOA World Map, loaded and driven the way a player does: the map session, the keys it holds,
//! `pass`, the pins and their settings, the search and the text fitting the window.
//!
//! Each test names its negative control: what it would see if the change were not there.
use super::{
    action_broker,
    bridge::Bridge,
    discovery::{DiscoveredAddon, EntryMode, Extensions},
    host::Host,
    input, native_core::{self, CORE, Core},
    native_events, native_settings, pointer_api,
    window_sessions as ws,
    window_takeover::{self, TakeoverKind},
};
use dear_imgui_rs::Context;
use serde_json::json;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

const ID: &str = "eqoa_world_map";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(ID).canonicalize().unwrap()
}

fn manifest() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root().join("mod.json")).unwrap()).unwrap()
}

fn addon() -> DiscoveredAddon {
    let m = manifest();
    let list = |name: &str| -> Vec<String> {
        m[name].as_array().map(|a| a.iter().map(|c| c.as_str().unwrap().to_string()).collect()).unwrap_or_default()
    };
    DiscoveredAddon {
        id: ID.into(),
        name: ID.into(),
        entry: root().join("main.lua"),
        script_root: root(),
        package_root: root(),
        module_dirs: vec![root()],
        resource_dirs: vec![root()],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: list("capabilities"),
        extensions: Extensions { replaces: list("replaces"), provides: list("provides"), ..Default::default() },
        initially_enabled: false,
        mods_root: None,
    }
}

fn context(size: [f32; 2]) -> Context {
    let mut context = Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_display_size(size);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.font_atlas().try_claim_legacy_renderer().unwrap().build();
    context
}

fn fresh() {
    crate::settings::testing::reset(false);
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    input::reset_input_state();
    pointer_api::position([0.0, 0.0], true, (1024, 768));
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = json!({"id": 256, "name": "Me", "level": 20, "position": {"x": 25300.0, "y": 0.0, "z": 15200.0}});
        c.view.map = json!({"enabled": true, "open": false, "minimap": true, "markers_on": true, "world_id": 0, "world_name": "Tunaria",
            "zone": null, "bounds": null, "waypoint": null, "revision": 1, "search_anchor": null});
    });
}

struct Rig {
    host: Host,
    bridge: Bridge,
    context: Context,
    vm: u64,
    saved: Rc<RefCell<Vec<serde_json::Value>>>,
}

fn rig(size: [f32; 2], saved: Option<serde_json::Value>) -> Rig {
    let context = context(size);
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let log: Rc<RefCell<Vec<serde_json::Value>>> = Rc::default();
    let sink = log.clone();
    native_settings::configure(&host, saved.clone(), Rc::new(move |v| {
        sink.borrow_mut().push(v.clone());
        Ok(())
    }));
    host.run_addon(&addon()).unwrap();
    if let Some(value) = saved {
        let value = super::profile::decode(&host.lua, &value).unwrap();
        host.callback(3, value).unwrap();
    }
    let vm = native_events::bus(&host.lua).borrow().vm;
    Rig { host, bridge, context, vm, saved: log }
}

impl Rig {
    fn frames(&mut self, n: usize) -> usize {
        let mut vertices = 0;
        for _ in 0..n {
            CORE.with(|c| c.borrow_mut().frame += 1);
            self.context.frame();
            self.bridge
                .invoke(|| {
                    native_events::dispatch(&self.host)?;
                    native_events::render(&self.host)
                })
                .unwrap();
            vertices = self.context.render_legacy().draw_data().total_vtx_count();
        }
        vertices
    }
    fn open(&mut self) {
        ws::open_client(self.vm, ID, "map", "key", json!({}));
        native_core::flush_deferred();
        self.frames(3);
    }
    fn key(&mut self, name: &str, text: &str, down: bool) {
        CORE.with(|c| {
            c.borrow_mut().emit("input.key", json!({"key": name, "combo": name, "down": down, "repeat": false, "synthetic": false, "text": text}))
        });
        native_events::dispatch(&self.host).unwrap();
    }
    fn lua_f64(&self, code: &str) -> f64 {
        self.host.lua.load(format!("return {code}")).eval().unwrap()
    }
    fn lua_str(&self, code: &str) -> String {
        self.host.lua.load(format!("return tostring({code})")).eval().unwrap()
    }
    /// The centre of the hit labelled `label` this frame.
    fn hit(&self, label: &str) -> (f64, f64) {
        let x = self.lua_f64(&format!("(function() for _, h in ipairs(world_map.hits) do if h.label == \"{label}\" then return h.x + h.w / 2 end end return -1 end)()"));
        let y = self.lua_f64(&format!("(function() for _, h in ipairs(world_map.hits) do if h.label == \"{label}\" then return h.y + h.h / 2 end end return -1 end)()"));
        assert!(x >= 0.0, "no control labelled {label:?}: {:?}", self.lua_str("#world_map.hits"));
        (x, y)
    }
    fn press(&mut self, at: (f64, f64), button: &str) {
        pointer_api::position([at.0 as f32, at.1 as f32], true, (1024, 768));
        CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": at.0, "y": at.1, "inside_window": true, "button": button})));
        native_events::dispatch(&self.host).unwrap();
        self.frames(2);
    }
    fn wheel(&mut self, at: (f64, f64), steps: i32, inside: bool) {
        CORE.with(|c| c.borrow_mut().emit("pointer.wheel", json!({"x": at.0, "y": at.1, "inside_window": inside, "steps": steps})));
        native_events::dispatch(&self.host).unwrap();
        self.frames(2);
    }
    /// The middle of the map area.
    fn middle(&self) -> (f64, f64) {
        (self.lua_f64("world_map.area.x + world_map.area.w / 2"), self.lua_f64("world_map.area.y + world_map.area.h / 2"))
    }
}

#[test]
fn a13_world_map_ships_off_holds_the_map_and_gives_it_back_when_off() {
    fresh();
    let m = manifest();
    assert_eq!(m["enabled"], false, "it ships off");
    assert_eq!(m["replaces"], json!(["map"]));
    assert!(m["provides"].as_array().is_some_and(|p| p.len() == 1));
    assert!(crate::fork_ext::bundled::PACKAGES.contains(&ID));
    for file in ["mod.json", "main.lua", "README.md", "resources/SpaceMono-Regular.ttf", "resources/OFL.txt"] {
        assert!(crate::fork_ext::bundled::EMBEDDED_FILES.iter().any(|f| f.path == format!("{ID}/{file}")), "{ID}/{file} ships");
    }
    for file in ["mod.json", "main.lua", "README.md"] {
        assert!(!std::fs::read(root().join(file)).unwrap().contains(&b'\r'), "{file} is LF");
    }
    let mut rig = rig([1024.0, 768.0], None);
    assert!(window_takeover::is_taken(TakeoverKind::Map), "it holds the map key: {:?}", rig.host.drain_logs());
    assert_eq!(rig.frames(2), 0, "shut, it draws nothing");
    // off: the game's map key again (negative control: while on, the kind is taken)
    native_events::clear(&rig.host);
    assert!(!window_takeover::is_taken(TakeoverKind::Map), "turned off: the game's map is the map key's again");
}

#[test]
fn a13_world_map_opens_with_the_map_key_holds_its_keys_only_while_open_and_pass_gives_the_games_map() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    // control: shut, the walking keys and Escape and Enter are the game's
    for key in [b'w', b'a', b's', b'd', 0x1B, 0x0D] {
        assert!(!input::is_key_claimed(key), "closed: key {key:#x} reaches the game");
    }
    rig.open();
    let vertices = rig.frames(2);
    // the headless host has no font loader: that one diagnostic is expected, any other is not
    let logs: Vec<String> = rig.host.drain_logs().unwrap().into_iter().filter(|l| !l.contains("eqoa.fonts.load")).collect();
    assert!(vertices > 0 && logs.is_empty(), "it draws its map without a diagnostic: {logs:?}");
    for key in [b'w', b'a', b's', b'd', 0x1B, 0x0D] {
        assert!(input::is_key_claimed(key), "open: key {key:#x} is the map's");
    }
    // the button: the game's own map for this opening
    let at = rig.hit("Use the game's map");
    rig.press(at, "left");
    assert_eq!(ws::take_passes(), vec![(rig.vm, TakeoverKind::Map)], "the game's map was asked for");
    // the client opens it and ends the session
    let id = ws::get(rig.vm, "map").unwrap().id;
    ws::end_dialog_session(&id, "passed");
    native_core::flush_deferred();
    rig.frames(3);
    assert_eq!(rig.frames(1), 0, "it drew nothing once it was told");
    for key in [b'w', b'a', b's', b'd', 0x1B, 0x0D] {
        assert!(!input::is_key_claimed(key), "closed again: key {key:#x} reaches the game");
    }
    // Escape closes the map
    rig.open();
    assert!(ws::get(rig.vm, "map").is_some());
    rig.key("Escape", "", true);
    assert!(ws::get(rig.vm, "map").is_none(), "Escape closed it");
    assert!(!input::is_key_claimed(0x1B));
}

#[test]
fn a13_world_map_pans_on_wasd_zooms_on_the_wheel_and_only_while_open() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    // control: W with the map shut does nothing here (the game walks)
    rig.key("w", "", true);
    rig.frames(3);
    rig.key("w", "", false);
    assert_eq!(rig.lua_str("world_map.view"), "nil", "shut: no view, nothing moved");
    rig.open();
    let (x0, z0) = (rig.lua_f64("world_map.view.x"), rig.lua_f64("world_map.view.z"));
    assert_eq!((x0, z0), (25300.0, 15200.0), "it opens on the player");
    rig.key("w", "", true);
    rig.frames(20);
    rig.key("w", "", false);
    let z1 = rig.lua_f64("world_map.view.z");
    assert!(z1 < z0 - 5.0 && rig.lua_f64("world_map.view.x") == x0, "W pans up (north is -z): {z0} -> {z1}");
    rig.frames(10);
    assert_eq!(rig.lua_f64("world_map.view.z"), z1, "and stops when the key is let go");
    // the wheel zooms about the pointer; the control: a wheel outside the window does nothing
    let mid = rig.middle();
    let s0 = rig.lua_f64("world_map.view.scale");
    rig.wheel((2.0, 2.0), 3, true);
    assert_eq!(rig.lua_f64("world_map.view.scale"), s0, "a turn off the window is not the map's");
    rig.wheel(mid, 2, true);
    let s1 = rig.lua_f64("world_map.view.scale");
    assert!((s1 - s0 / 1.25f64.powi(2)).abs() < 1e-3, "two steps in: {s0} -> {s1}");
    rig.wheel(mid, -2, true);
    assert!((rig.lua_f64("world_map.view.scale") - s0).abs() < 1e-3, "and two out");
}

#[test]
fn a13_world_map_pins_are_saved_per_world_removed_on_right_click_and_come_back() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    // right-click: the menu; "Drop a pin here..." opens the note box; Enter saves (the control: Esc drops it)
    let mid = rig.middle();
    rig.press(mid, "right");
    let at = rig.hit("Drop a pin here...");
    rig.press(at, "left");
    assert_eq!(rig.lua_str("world_map.note ~= nil"), "true", "the note box is up");
    for ch in ["O", "r", "c"] {
        rig.key(&ch.to_lowercase(), ch, true);
    }
    rig.key("Escape", "", true);
    assert_eq!(rig.lua_f64("#(world_map.pins['0'] or {})"), 0.0, "control: Esc cancels, nothing saved");
    assert!(ws::get(rig.vm, "map").is_some(), "and Esc in the note box does not close the map");
    rig.press(mid, "right");
    let at = rig.hit("Drop a pin here...");
    rig.press(at, "left");
    for ch in ["O", "r", "c", " ", "c", "a", "m", "p"] {
        rig.key(&ch.to_lowercase(), ch, true);
    }
    rig.frames(2);
    rig.key("Enter", "", true);
    assert_eq!(rig.lua_f64("#world_map.pins['0']"), 1.0, "one pin in world 0");
    assert_eq!(rig.lua_str("world_map.pins['0'][1].note"), "Orc camp");
    assert!((rig.lua_f64("world_map.pins['0'][1].x") - 25300.0).abs() < 200.0, "at the click");
    let saved = rig.saved.borrow().last().cloned().expect("saved through settings.save (the background writer)");
    assert!(saved.to_string().contains("Orc camp"), "the pin is in what was saved: {saved}");
    // another world has its own pins: nothing here (negative control: a pin of world 0 is not in world 1)
    CORE.with(|c| c.borrow_mut().view.map["world_id"] = json!(1));
    rig.frames(2);
    assert_eq!(rig.lua_f64("#(world_map.pins['1'] or {})"), 0.0);
    // a fresh start takes the pins back
    drop(rig);
    fresh();
    let mut again = rig_with(saved.clone());
    again.open();
    assert_eq!(again.lua_str("world_map.pins['0'][1].note"), "Orc camp", "the pin came back");
    // right-click on the pin removes it
    let pin = again.lua_f64("(function() local p = world_map.pins['0'][1]; local v = world_map.view; local a = world_map.area; return a.x + a.w / 2 + (p.x - v.x) / v.scale end)()");
    let pin_y = again.lua_f64("(function() local p = world_map.pins['0'][1]; local v = world_map.view; local a = world_map.area; return a.y + a.h / 2 + (p.z - v.z) / v.scale end)()");
    again.press((pin, pin_y), "right");
    assert_eq!(again.lua_f64("#world_map.pins['0']"), 0.0, "removed");
    assert!(!again.saved.borrow().last().unwrap().to_string().contains("Orc camp"), "and saved so");
}

fn rig_with(saved: serde_json::Value) -> Rig {
    rig([1024.0, 768.0], Some(saved))
}

#[test]
fn a13_world_map_sets_the_games_waypoint_and_searches_places_without_map_search() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    let mid = rig.middle();
    rig.press(mid, "right");
    let at = rig.hit("Set waypoint here");
    rig.press(at, "left");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["set_waypoint"], "logs {:?}", rig.host.drain_logs());
    // a place by name, from the map's own marker data (Map Search is not loaded here)
    let name = crate::map::extension::static_markers(0)
        .iter()
        .find(|m| matches!(m["kind"].as_str(), Some("camp" | "coach" | "town")) && m["label"].as_str().or(m["name"].as_str()).is_some_and(|n| n.len() > 3))
        .map(|m| m["label"].as_str().or(m["name"].as_str()).unwrap().to_string())
        .expect("the game's own data names a place");
    let at = rig.hit("search");
    rig.press(at, "left");
    let query: String = name.chars().take(4).collect();
    for ch in query.chars() {
        rig.key(&ch.to_string().to_lowercase(), &ch.to_string(), true);
    }
    rig.frames(2);
    assert!(rig.lua_f64("#world_map.results") >= 1.0, "'{query}' finds {name}");
    let before = (rig.lua_f64("world_map.view.x"), rig.lua_f64("world_map.view.z"));
    rig.key("Enter", "", true);
    let after = (rig.lua_f64("world_map.view.x"), rig.lua_f64("world_map.view.z"));
    assert!(after != before, "Enter went to the first match: {before:?} -> {after:?}");
    // control: a word that is no place finds nothing
    let at = rig.hit("search");
    rig.press(at, "left");
    for ch in "zzqx".chars() {
        rig.key(&ch.to_string(), &ch.to_string(), true);
    }
    assert_eq!(rig.lua_f64("#world_map.results"), 0.0);
}

#[test]
fn a13_world_map_text_fits_its_window_at_the_default_and_the_smallest_size() {
    fresh();
    for (win, display) in [(json!({"w": 760, "h": 540}), [1024.0, 768.0]), (json!({"w": 360, "h": 260}), [1024.0, 768.0]), (json!({"w": 360, "h": 260}), [640.0, 480.0])] {
        fresh();
        let mut rig = rig(display, None);
        rig.host.execute(&format!("world_map.win.w = {}; world_map.win.h = {}; world_map.pins['0'] = {{ {{ x = 25300, z = 15200, note = 'A very long note that goes on and on and on' }} }}", win["w"], win["h"])).unwrap();
        rig.open();
        rig.frames(2);
        // search results and a hover card on the pin are drawn too
        let at = rig.hit("search");
        rig.press(at, "left");
        for ch in "a".chars() {
            rig.key(&ch.to_string(), &ch.to_string(), true);
        }
        rig.frames(2);
        let over = rig.lua_f64("world_map.overflow");
        assert!(over <= 0.5, "text reaches {over} pixels past the window ({win}, display {display:?})");
        assert!(rig.lua_f64("world_map.rect.w") >= 360.0);
    }
}

#[test]
fn a13_world_map_moves_resizes_and_keeps_its_place() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    let (x0, y0, w0, h0) = (rig.lua_f64("world_map.rect.x"), rig.lua_f64("world_map.rect.y"), rig.lua_f64("world_map.rect.w"), rig.lua_f64("world_map.rect.h"));
    let drag = |rig: &mut Rig, from: (f64, f64), to: (f64, f64)| {
        pointer_api::position([from.0 as f32, from.1 as f32], true, (1024, 768));
        rig.context.io_mut().add_mouse_pos_event([from.0 as f32, from.1 as f32]);
        CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": from.0, "y": from.1, "inside_window": true, "button": "left"})));
        rig.context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
        rig.frames(2);
        pointer_api::position([to.0 as f32, to.1 as f32], true, (1024, 768));
        rig.context.io_mut().add_mouse_pos_event([to.0 as f32, to.1 as f32]);
        rig.frames(2);
        rig.context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
        rig.frames(3);
    };
    // the title strip moves it (the control: the map area pans, the window stays)
    drag(&mut rig, (x0 + 100.0, y0 + 150.0), (x0 + 180.0, y0 + 190.0));
    assert_eq!((rig.lua_f64("world_map.rect.x"), rig.lua_f64("world_map.rect.y")), (x0, y0), "dragging the map pans it, not the window");
    drag(&mut rig, (x0 + 100.0, y0 + 8.0), (x0 + 60.0, y0 - 2.0));
    let (x1, y1) = (rig.lua_f64("world_map.rect.x"), rig.lua_f64("world_map.rect.y"));
    assert!((x1 - (x0 - 40.0)).abs() <= 2.0 && (y1 - (y0 - 10.0)).abs() <= 2.0, "moved: {x0},{y0} -> {x1},{y1}");
    // the corner sizes it
    drag(&mut rig, (x1 + w0 - 4.0, y1 + h0 - 4.0), (x1 + w0 + 36.0, y1 + h0 + 16.0));
    let (w1, h1) = (rig.lua_f64("world_map.rect.w"), rig.lua_f64("world_map.rect.h"));
    assert!((w1 - (w0 + 40.0)).abs() <= 2.0 && (h1 - (h0 + 20.0)).abs() <= 2.0, "sized: {w0}x{h0} -> {w1}x{h1}");
    // the place is saved
    let saved = rig.saved.borrow().last().cloned().unwrap_or_else(|| panic!("saved after the drag: {}", rig.lua_str("world_map.save_error")));
    assert!(saved.to_string().contains("\"w\""), "the size is saved: {saved}");
    // a fresh start takes the place back (the control: the default size differs)
    drop(rig);
    fresh();
    let mut again = rig_with(saved);
    again.open();
    assert!((again.lua_f64("world_map.rect.w") - w1).abs() <= 2.0 && (again.lua_f64("world_map.rect.x") - x1).abs() <= 2.0, "came back");
    assert!((w1 - w0).abs() > 20.0);
}
