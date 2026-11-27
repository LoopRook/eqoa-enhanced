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
    input::focus(true);
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
        assert!(x >= 0.0, "no control labelled {label:?}: {} hits, menu {}, note {}, last press {}, view {}, logs {:?}", self.lua_str("#world_map.hits"), self.lua_str("world_map.menu"), self.lua_str("world_map.note"), self.lua_str("world_map.last_press"), self.lua_str("world_map.view"), self.host.drain_logs());
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

/// Owner 2026-11-26: the World Map's box finds creatures (the live ones Map Search lists, with the same filter) and group
/// members as well as places; a pick centres the map on it; the box says "Find a place or creature...".
#[test]
fn owner1126_world_map_search_finds_live_creatures_and_places_still_work() {
    fresh();
    let entity = |id: u64, name: &str, hp: f64, corpse: bool, at: [f64; 2]| {
        json!({
            "ref": {"$ref": "entity", "key": id.to_string(), "generation": 0, "incarnation": 1},
            "id": id, "name": name, "level": 7, "kind": "npc", "is_creature": true, "corpse": corpse, "hp_fraction": hp,
            "search_flags": 0, "class_code": 0, "world_id": 0,
            "position": {"x": at[0], "y": 0.0, "z": at[1]}, "actor_position": {"x": at[0], "y": 0.0, "z": at[1]},
        })
    };
    CORE.with(|c| {
        c.borrow_mut().view.entities = vec![
            entity(8001, "Zzyzx Gnoll", 1.0, false, [25500.0, 15400.0]),
            entity(8002, "Zzyzx Gnoll", 1.0, false, [25350.0, 15250.0]),
            entity(8003, "Zzyzx Corpse", 0.0, true, [25310.0, 15210.0]),
        ];
    });
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    let at = rig.hit("search");
    rig.press(at, "left");
    for ch in "zzyzx".chars() {
        rig.key(&ch.to_string(), &ch.to_string(), true);
    }
    rig.frames(2);
    // the two live ones are listed (the corpse is not), the nearer first, marked as creatures
    assert_eq!(rig.lua_f64("#world_map.results"), 2.0, "{}", rig.lua_str("#world_map.results"));
    assert_eq!(rig.lua_str("world_map.results[1].kind:sub(1, 8) .. world_map.results[1].x"), "creature25350");
    let before = (rig.lua_f64("world_map.view.x"), rig.lua_f64("world_map.view.z"));
    rig.key("Enter", "", true);
    let after = (rig.lua_f64("world_map.view.x"), rig.lua_f64("world_map.view.z"));
    assert_eq!(after, (25350.0, 15250.0), "Enter centred the map on the nearer creature (from {before:?})");
    assert!(rig.lua_f64("world_map.found.x") == 25350.0, "and marked it");
    // control: one letter lists no creatures; a creature name no one has finds nothing
    let at = rig.hit("search");
    rig.press(at, "left");
    rig.key("z", "z", true);
    let creatures = "(function() local n = 0 for _, r in ipairs(world_map.results) do if r.kind:find('^creature') then n = n + 1 end end return n end)()";
    assert_eq!(rig.lua_f64(creatures), 0.0, "control: one letter lists no creature");
    // control: with no creature in view the same query finds nothing
    CORE.with(|c| c.borrow_mut().view.entities = vec![]);
    rig.key("z", "z", true);
    rig.frames(2);
    assert_eq!(rig.lua_f64(creatures), 0.0, "control: no creature in view, none listed");
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

#[test]
fn a13_world_map_takes_the_map_key_back_after_a_zoning_and_after_the_switch() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    assert!(window_takeover::is_taken(TakeoverKind::Map));
    // the client lets go of every claim at a zoning, a teleport and a respawn
    window_takeover::release_all();
    assert!(!window_takeover::is_taken(TakeoverKind::Map), "released");
    CORE.with(|c| c.borrow_mut().emit("world.entered", json!({"world_id": 0, "initial": false})));
    native_events::dispatch(&rig.host).unwrap();
    assert!(window_takeover::is_taken(TakeoverKind::Map), "world.entered asks again");
    window_takeover::release_all();
    CORE.with(|c| c.borrow_mut().emit("state.reset", json!({})));
    native_events::dispatch(&rig.host).unwrap();
    assert!(window_takeover::is_taken(TakeoverKind::Map), "state.reset asks again");
    // the player's switch releases it; once it is off again the slow retry takes the key back
    window_takeover::set_use_game_windows(true);
    assert!(!window_takeover::is_taken(TakeoverKind::Map));
    window_takeover::set_use_game_windows(false);
    // the retry waits three seconds between tries: not yet, then its turn
    rig.host.execute("world_map.claim_at = 1e9").unwrap();
    rig.frames(2);
    assert!(!window_takeover::is_taken(TakeoverKind::Map), "control: not before its turn");
    rig.host.execute("world_map.claim_at = 0").unwrap();
    rig.frames(1);
    assert!(window_takeover::is_taken(TakeoverKind::Map), "the mod took the key back by itself");
}

fn route(rig: &mut Rig, key: &str, down: bool) {
    input::route_mod_key(&winit::keyboard::Key::Character(key.into()), down, false, false, true, false);
    native_events::dispatch(&rig.host).unwrap();
}

#[test]
fn a13_world_map_typing_ends_with_a_click_back_in_the_game_and_gives_the_keyboard_back_at_once() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    assert!(!input::is_text_capture_active(), "control: not typing");
    // search: typing holds the keyboard, W goes into the box
    let at = rig.hit("search");
    rig.press(at, "left");
    rig.frames(2);
    assert!(input::is_text_capture_active(), "typing in the search box holds the keyboard");
    route(&mut rig, "w", true);
    route(&mut rig, "w", false);
    assert_eq!(rig.lua_str("world_map.search"), "w", "W went into the box");
    // a click back in the game ends it, and W is the game's (and the map's pan key) again
    rig.press((2.0, 2.0), "left");
    rig.frames(1);
    assert!(!input::is_text_capture_active(), "a click outside released the keyboard at once");
    assert_eq!(rig.lua_str("world_map.focus"), "nil");
    route(&mut rig, "w", true);
    assert_eq!(rig.lua_str("world_map.search"), "w", "W no longer goes into the box");
    assert_eq!(rig.lua_str("world_map.held.w"), "true", "it pans the map");
    route(&mut rig, "w", false);
    // Enter on a result, Esc, a saved note and closing the map all release the keyboard at once
    let at = rig.hit("search");
    rig.press(at, "left");
    rig.frames(1);
    assert!(input::is_text_capture_active());
    rig.key("Escape", "", true);
    rig.frames(1);
    assert!(!input::is_text_capture_active(), "Esc released it");
    let mid = rig.middle();
    rig.press(mid, "right");
    let at = rig.hit("Drop a pin here...");
    rig.press(at, "left");
    assert!(input::is_text_capture_active(), "the note box holds the keyboard");
    rig.key("Enter", "", true);
    rig.frames(1);
    assert!(!input::is_text_capture_active(), "saving the note released it");
    // (a right-click on the pin just saved would remove it: the next one is made elsewhere)
    let mid = (mid.0 + 120.0, mid.1 + 60.0);
    rig.press(mid, "right");
    let at = rig.hit("Drop a pin here...");
    rig.press(at, "left");
    assert!(input::is_text_capture_active());
    // a click in the game while the note is open cancels it
    rig.press((2.0, 2.0), "left");
    rig.frames(1);
    assert!(!input::is_text_capture_active() && rig.lua_str("world_map.note") == "nil", "a click in the game closed the note box");
    // closing the map while typing
    let at = rig.hit("search");
    rig.press(at, "left");
    rig.frames(1);
    assert!(input::is_text_capture_active());
    let id = ws::get(rig.vm, "map").unwrap().id;
    ws::close(rig.vm, &id, "key").unwrap();
    native_core::flush_deferred();
    rig.frames(2);
    assert!(!input::is_text_capture_active(), "closing the map released the keyboard");
}

#[test]
fn a13_world_map_holds_the_shifted_pan_keys_and_ignores_keys_another_mod_is_typing() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    for key in [b'W', b'A', b'S', b'D'] {
        assert!(input::is_key_claimed(key), "Shift or Caps Lock W/A/S/D is the map's too: {key:#x}");
    }
    // another mod types in its own box: its Escape and letters are not this map's
    let other = Host::with_test_capabilities(&["eqoa.input.capture_text"]).unwrap();
    other.execute("eqoa.input.capture_text(true)").unwrap();
    rig.key("w", "w", true);
    rig.key("Escape", "", true);
    assert!(ws::get(rig.vm, "map").is_some(), "Esc closed the other mod's box, not the map");
    assert_eq!(rig.lua_str("world_map.held.w"), "nil", "W was typed there, it did not pan");
    other.execute("eqoa.input.capture_text(false)").unwrap();
    rig.key("Escape", "", true);
    assert!(ws::get(rig.vm, "map").is_none(), "control: with nobody typing, Esc closes the map");
}

#[test]
fn a13_world_map_waypoint_removal_runs_the_whole_round_trip_and_shows_a_refusal() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    // the player's own waypoint (no owner), on the player
    CORE.with(|c| {
        c.borrow_mut().view.map["waypoint"] = json!({"world_id": 0, "x": 25300.0, "z": 15200.0, "label": null, "owner": null});
    });
    rig.open();
    rig.frames(2);
    let mid = rig.middle();
    // 1. set again at the same point (to take it over)
    rig.press(mid, "right");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["set_waypoint"], "logs {:?}", rig.host.drain_logs());
    // 2. the answer arrives: the mod clears it (the control: with the answer never matched, nothing more is asked)
    assert_eq!(action_broker::dispatch_all_for_test(), vec!["set_waypoint"]);
    native_core::flush_deferred();
    rig.frames(2);
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["clear_waypoint"], "the answer was recognised and the pin is cleared");
    action_broker::reset_all_for_test();
    let before = rig.lua_f64("#world_map.expect");
    // a refusal is shown in the footer and nothing is cleared
    rig.press(mid, "right");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["set_waypoint"]);
    action_broker::refuse_all_for_test();
    native_core::flush_deferred();
    rig.frames(2);
    assert!(rig.lua_str("world_map.message").starts_with("Waypoint:"), "the refusal is on screen: {}", rig.lua_str("world_map.message"));
    assert!(action_broker::queued_kinds_for_test().is_empty(), "and nothing is cleared");
    assert_eq!(rig.lua_f64("#world_map.expect"), before, "the answered request is forgotten");
}

#[test]
fn a13_world_map_removal_always_sets_the_pin_again_for_its_own_world_before_clearing() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    // a pin this mod owns in world 0, while it also placed one in world 1 earlier (clear_waypoint would clear the latest)
    CORE.with(|c| {
        c.borrow_mut().view.map["waypoint"] = json!({"world_id": 0, "x": 25300.0, "z": 15200.0, "label": "World Map", "owner": "eqoa_world_map"});
    });
    rig.open();
    rig.frames(2);
    let mid = rig.middle();
    rig.press(mid, "right");
    let queued = action_broker::queued_actions_for_test();
    assert_eq!(queued.len(), 1);
    match &queued[0] {
        action_broker::ActionKind::SetWaypoint { world, keep, .. } => assert!(*world == 0 && *keep, "set again for world 0, kept"),
        other => panic!("the mod went straight to {:?}: clear_waypoint would clear the latest pin of any world", other.name()),
    }
}

#[test]
fn a13_world_map_is_in_the_window_order_so_the_click_goes_to_the_window_in_front() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    rig.frames(2);
    let mid = rig.middle();
    // a claim made inside the ImGui window has a place in ImGui's order; made outside one it has none, and every other
    // window would always be taken to be in front of the map
    assert!(pointer_api::front_claim_z(mid.0 as f32, mid.1 as f32).is_some(), "the map's claim has a place in the window order");
}

#[test]
fn a13_world_map_hides_camps_with_map_markers_off_even_for_the_hover_and_keeps_the_drawing_cheap() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    rig.frames(2);
    // centre the view on a camp
    rig.host.execute("local m; for _, k in ipairs(world_map.markers) do if k.kind == 'camp' then m = k; break end end assert(m); world_map.following = false; world_map.view.x, world_map.view.z, world_map.view.scale = m.x, m.z, 4").unwrap();
    rig.frames(2);
    let mid = rig.middle();
    pointer_api::position([mid.0 as f32, mid.1 as f32], true, (1024, 768));
    rig.frames(2);
    assert!(rig.lua_str("world_map.card ~= nil") == "true", "Map Markers on: the camp answers the pointer");
    CORE.with(|c| c.borrow_mut().view.map["markers_on"] = json!(false));
    rig.frames(2);
    assert_eq!(rig.lua_str("world_map.card"), "nil", "Map Markers off: no card over empty ground");
    // the whole world in view: two draw calls a camp at most (it was four triangles and their points)
    CORE.with(|c| c.borrow_mut().view.map["markers_on"] = json!(true));
    rig.host.execute("world_map.view.scale = 1e5").unwrap();
    rig.frames(3);
    let calls = rig.lua_f64("world_map.marker_calls");
    let total = rig.lua_f64("#world_map.markers");
    assert!(calls <= 2.0 * total + 1.0, "{calls} marker draw calls for {total} marks");
}

#[test]
fn a13_world_map_lets_go_of_held_keys_when_another_text_field_takes_the_keyboard() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    route(&mut rig, "w", true);
    assert_eq!(rig.lua_str("world_map.held.w"), "true");
    // a text field somewhere else takes the keyboard while W is down: the key-up never arrives
    let other = Host::with_test_capabilities(&["eqoa.input.capture_text"]).unwrap();
    other.execute("eqoa.input.capture_text(true)").unwrap();
    rig.frames(2);
    assert_eq!(rig.lua_str("world_map.held.w"), "nil", "W is not held any more: the map does not pan by itself");
    other.execute("eqoa.input.capture_text(false)").unwrap();
}

// ---- owner 2026-11-25: sizes follow the client's UI scale and the player's own settings

fn boxes_overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.0 + b.2 - 0.01 && b.0 < a.0 + a.2 - 0.01 && a.1 < b.1 + b.3 - 0.01 && b.1 < a.1 + a.3 - 0.01
}

fn bar_box(rig: &Rig, name: &str) -> (f64, f64, f64, f64) {
    let f = |k: &str| rig.lua_f64(&format!("world_map.bar[\"{name}\"].{k}"));
    (f("x"), f("y"), f("w"), f("h"))
}

#[test]
fn a13_world_map_text_follows_the_ui_scale_and_the_text_size_setting() {
    // Negative control: with the old fixed 12 pixel text the size would not change with the setting or the display.
    let mut sizes = Vec::new();
    let mut k = 1.0;
    for opt in [1, 2, 3, 4] {
        fresh();
        let mut rig = rig([1024.0, 768.0], None);
        rig.host.execute(&format!("world_map.opts.text = {opt}")).unwrap();
        rig.open();
        rig.frames(2);
        k = rig.lua_f64("world_map.ui_k");
        let mult = [0.85, 1.0, 1.3, 1.7][opt - 1];
        let px = rig.lua_f64("world_map.text_px");
        let want = (11.0 * k * mult).round();
        assert_eq!(px, want, "text px at option {opt}: {px}, wanted {want} (client scale {k})");
        assert!((rig.lua_f64("world_map.F") - k * mult).abs() < 1e-9);
        sizes.push((px, rig.lua_f64("world_map.line_h"), rig.lua_f64("world_map.bar.Center.h")));
    }
    assert!(sizes.windows(2).all(|w| w[1].0 > w[0].0 && w[1].1 > w[0].1 && w[1].2 > w[0].2), "text, line and button grow with the setting: {sizes:?}");
    // at the normal setting the text is what the other mods draw: 11 px of the 640 x 448 frame, scaled
    assert_eq!(sizes[1].0, (11.0 * k).round());
}

#[test]
fn a13_world_map_top_bar_and_footer_do_not_overlap_at_the_smallest_window_and_largest_text() {
    // Negative control: the old fixed 150 + 56 wide buttons and clipped footer overlap or run out at a larger text.
    for display in [[1024.0, 768.0], [640.0, 480.0], [1280.0, 720.0]] {
        for opt in [1, 4] {
            fresh();
            let mut rig = rig(display, None);
            rig.host.execute(&format!("world_map.opts.text = {opt}; world_map.win.w = 1; world_map.win.h = 1")).unwrap();
            rig.open();
            rig.frames(2);
            let r = (rig.lua_f64("world_map.rect.x"), rig.lua_f64("world_map.rect.y"), rig.lua_f64("world_map.rect.w"), rig.lua_f64("world_map.rect.h"));
            let names = ["title", "search", "Center", "Use the game's map", "X", "footer"];
            let boxes: Vec<_> = names.iter().map(|n| bar_box(&rig, n)).collect();
            for (n, b) in names.iter().zip(&boxes) {
                assert!(b.0 >= r.0 - 0.01 && b.0 + b.2 <= r.0 + r.2 + 0.01 && b.1 >= r.1 - 0.01 && b.1 + b.3 <= r.1 + r.3 + 0.01, "{n} {b:?} is outside the window {r:?} ({display:?}, text {opt})");
            }
            for i in 0..names.len() {
                for j in i + 1..names.len() {
                    assert!(!boxes_overlap(boxes[i], boxes[j]), "{} {:?} overlaps {} {:?} ({display:?}, text {opt})", names[i], boxes[i], names[j], boxes[j]);
                }
            }
            assert!(rig.lua_f64("world_map.overflow") <= 0.5, "text past the window ({display:?}, text {opt})");
            let foot = rig.lua_str("world_map.foot_text");
            assert!(foot.len() > 8, "the footer says something: {foot:?}");
        }
    }
}

#[test]
fn a13_world_map_card_stays_in_the_window_and_markers_grow_with_the_marker_size() {
    // Negative control: a card and hit area fixed in pixels would not change with the setting.
    let mut reach = Vec::new();
    for opt in [1, 4] {
        fresh();
        let mut rig = rig([1024.0, 768.0], None);
        rig.host.execute(&format!("world_map.opts.text = 4; world_map.opts.marker = {opt}; world_map.win.w = 1; world_map.win.h = 1; world_map.pins['0'] = {{ {{ x = 25300, z = 15200, note = 'A very long note that goes on and on and on and on and on' }} }}")).unwrap();
        rig.open();
        let mid = rig.middle();
        pointer_api::position([mid.0 as f32, mid.1 as f32], true, (1024, 768));
        rig.frames(2);
        assert_eq!(rig.lua_str("world_map.card ~= nil"), "true", "the pin answers the pointer");
        let (cx, cy, cw, ch) = (rig.lua_f64("world_map.card_box.x"), rig.lua_f64("world_map.card_box.y"), rig.lua_f64("world_map.card_box.w"), rig.lua_f64("world_map.card_box.h"));
        let (ax, ay, aw, ah) = (rig.lua_f64("world_map.area.x"), rig.lua_f64("world_map.area.y"), rig.lua_f64("world_map.area.w"), rig.lua_f64("world_map.area.h"));
        assert!(cx >= ax && cy >= ay && cx + cw <= ax + aw && cy + ch <= ay + ah, "the card ({cx},{cy},{cw},{ch}) is inside the map area ({ax},{ay},{aw},{ah})");
        assert!(rig.lua_f64("world_map.overflow") <= 0.5);
        // how far from the pin's point a pointer still finds it
        let mut far = 0.0;
        for dx in 0..80 {
            pointer_api::position([mid.0 as f32 + dx as f32, mid.1 as f32], true, (1024, 768));
            rig.frames(1);
            if rig.lua_str("world_map.card ~= nil") == "true" {
                far = dx as f64;
            }
        }
        reach.push(far);
    }
    assert!(reach[1] > reach[0] + 3.0, "a bigger marker is easier to hit: {reach:?}");
}

#[test]
fn a13_world_map_size_settings_are_saved_whole_and_come_back() {
    // Negative control: a save that carried only the two sizes would lose the pins and the window.
    fresh();
    let rig1 = rig([1024.0, 768.0], None);
    rig1.host.execute("world_map.pins['0'] = { { x = 25300, z = 15200, note = 'Keep me' } }; world_map.win.x = 40; world_map.win.y = 30; world_map.win.w = 700; world_map.win.h = 500; world_map.opts.text = 3; world_map.opts.marker = 4; world_map.persist()").unwrap();
    let saved = rig1.saved.borrow().last().cloned().expect("saved");
    let text = saved.to_string();
    assert!(text.contains("[\"text_size\",3]") && text.contains("[\"marker_size\",4]"), "the sizes are saved: {saved}");
    assert!(text.contains("Keep me") && text.contains("[\"w\",700]"), "the whole snapshot: {saved}");
    drop(rig1);
    fresh();
    let mut again = rig([1024.0, 768.0], Some(saved));
    again.open();
    assert_eq!((again.lua_f64("world_map.opts.text"), again.lua_f64("world_map.opts.marker")), (3.0, 4.0));
    assert_eq!(again.lua_str("world_map.pins['0'][1].note"), "Keep me");
    assert_eq!(again.lua_f64("world_map.win.w"), 700.0);
    // a bad value falls back to Normal
    drop(again);
    fresh();
    let bad = rig([1024.0, 768.0], Some(json!({"table": [["version", 1], ["text_size", 99], ["marker_size", "x"]]})));
    assert_eq!((bad.lua_f64("world_map.opts.text"), bad.lua_f64("world_map.opts.marker")), (2.0, 2.0));
}

/// Owner 2026-11-25: "can't zoom in much". The client's ground picture is at 1 world unit a pixel at the finest, so the
/// mod draws that picture larger: down to 0.25 units a pixel (4 times), the point under the pointer staying under it.
/// Negative control: the old limit was 1, so the scale stopped there and these assertions fail.
#[test]
fn owner1125b_world_map_zooms_in_to_a_quarter_unit_a_pixel_about_the_pointer() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    let mid = rig.middle();
    rig.wheel(mid, 40, true);
    let s = rig.lua_f64("world_map.view.scale");
    assert!((s - 0.25).abs() < 1e-9, "the closest zoom is a quarter unit a pixel, not {s}");
    assert!((rig.lua_f64("world_map.mag") - 4.0).abs() < 1e-9, "the picture is drawn 4 times larger");
    // a turn in at the closest does not go past it
    rig.wheel(mid, 3, true);
    assert!((rig.lua_f64("world_map.view.scale") - 0.25).abs() < 1e-9);
    // the point under an off-centre pointer stays under it while zooming out from there
    let at = (mid.0 + 120.0, mid.1 - 70.0);
    let under = |rig: &Rig| {
        (
            rig.lua_f64(&format!("world_map.view.x + ({} - world_map.area.x - world_map.area.w / 2) * world_map.view.scale", at.0)),
            rig.lua_f64(&format!("world_map.view.z + ({} - world_map.area.y - world_map.area.h / 2) * world_map.view.scale", at.1)),
        )
    };
    let before = under(&rig);
    rig.wheel(at, -2, true);
    let after = under(&rig);
    assert!(rig.lua_f64("world_map.view.scale") > 0.25 + 1e-6, "it zoomed out");
    assert!((before.0 - after.0).abs() < 1e-6 && (before.1 - after.1).abs() < 1e-6, "{before:?} {after:?}");
}

/// Owner 2026-11-25: the round minimap (picture, dots, arrow) still drew over the World Map window's corner while its frame
/// was gone. The World Map holds a hide claim on the minimap while its window is up and the round minimap steps aside for it.
/// Negative control: the old mods leave `suppressed` false with the window up.
#[test]
fn owner1125b_round_minimap_goes_while_the_world_map_window_is_up_and_returns_when_it_closes() {
    fresh();
    let mut map = rig([1024.0, 768.0], None);
    let round_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods/eqoa_round_minimap").canonicalize().unwrap();
    let rm: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(round_root.join("mod.json")).unwrap()).unwrap();
    let round = DiscoveredAddon {
        id: "eqoa_round_minimap".into(),
        name: "eqoa_round_minimap".into(),
        entry: round_root.join("main.lua"),
        script_root: round_root.clone(),
        package_root: round_root.clone(),
        module_dirs: vec![round_root.clone()],
        resource_dirs: vec![round_root.clone()],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: rm["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect(),
        extensions: Extensions::default(),
        initially_enabled: true,
        mods_root: None,
    };
    let host = Host::for_addon(&round).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    // both mods see one set of hide claims, as the manager gives them (`hidden_by` reads it)
    let health = Rc::new(RefCell::new(super::health::HealthBarClaimsRegistry::new()));
    let claims = Rc::new(RefCell::new(super::visibility::HudClaimsRegistry::new()));
    let mut fixture = crate::play::Play::bundled_ui_fixture();
    let snapshot = fixture.forge_snapshot(None);
    let mut leases = Vec::new();
    for h in [&map.host, &host] {
        h.snapshot.set(snapshot);
        let lease = super::visibility::VisibilityLease::with_hud_claims(health.clone(), claims.clone()).unwrap();
        lease.update(snapshot.health, true);
        super::visibility::install_managed_health_adapter(h, &lease).unwrap();
        leases.push(lease);
    }
    host.run_addon(&round).unwrap();
    let mut frame = |map: &mut Rig, n: usize| {
        for _ in 0..n {
            CORE.with(|c| c.borrow_mut().frame += 1);
            map.context.frame();
            bridge.invoke(|| { native_events::dispatch(&host)?; native_events::render(&host) }).unwrap();
            let _ = map.context.render_legacy();
            for l in &leases { l.update(snapshot.health, true); }
        }
    };
    let suppressed = |host: &Host| -> String { host.lua.load("return tostring(round_minimap.suppressed)").eval().unwrap() };
    frame(&mut map, 3);
    assert_eq!(suppressed(&host), "false", "control: no map window, the minimap is not set aside; {:?}", host.drain_logs());
    map.open();
    frame(&mut map, 3);
    assert_eq!(suppressed(&host), "true", "the World Map window is up: the whole minimap steps aside");
    // closed: it comes back
    let id = ws::get(map.vm, "map").unwrap().id;
    ws::end_dialog_session(&id, "closed");
    native_core::flush_deferred();
    map.frames(3);
    frame(&mut map, 3);
    assert_eq!(suppressed(&host), "false", "the window closed: the minimap returns");
}

#[test]
fn a13_world_map_asks_for_its_picture_at_scale_one_or_more_even_when_zoomed_closer() {
    // 2026-11-26 (H2): the API refuses a picture closer than 1 unit a pixel; the mod magnifies the 1-unit picture.
    // Negative control: asking for the saved 0.25 itself gives "The map picture is not available" (terrain nil).
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    // the zoom a player left it at (the saved view scale, which the window restores on opening)
    rig.host.execute("world_map.last_scale = 0.25").unwrap();
    rig.open();
    rig.frames(2);
    assert_eq!(rig.lua_f64("world_map.view.scale"), 0.25, "reopened at the saved zoom");
    assert_eq!(rig.lua_str("world_map.terrain_error"), "nil", "the picture request was accepted at 0.25");
    assert_eq!(rig.lua_str("world_map.terrain ~= nil"), "true", "a picture is held");
    assert_eq!(rig.lua_f64("world_map.mag"), 4.0, "and drawn 4 times larger");
    // resized while zoomed closer than 1: the picture is made again at the new size, still accepted
    let before = rig.lua_str("world_map.terrain_size");
    rig.host.execute("world_map.win.w = 500; world_map.win.h = 400").unwrap();
    rig.frames(3);
    assert_ne!(rig.lua_str("world_map.terrain_size"), before, "the picture was made again at the new size");
    assert_eq!(rig.lua_str("world_map.terrain_error"), "nil", "the resized picture request was accepted at 0.25");
    assert_eq!(rig.lua_str("world_map.terrain ~= nil"), "true");
}

/// The live dots wear the game's minimap colours (`Play::mini_view`): one colour for each kind the API can tell, and
/// each is not another kind's (the negative control: a plain NPC is tan, so every other kind differs from it).
#[test]
fn world_map_dots_use_the_minimaps_colours_for_each_kind() {
    fresh();
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    let rgb = |rig: &Rig, e: &str| rig.lua_str(&format!("(function() local c = world_map.dot_rgb({e}) return string.format('%.0f,%.0f,%.0f', c[1] * 255, c[2] * 255, c[3] * 255) end)()"));
    let cases = [
        ("an NPC with no role: tan", "{ kind = 'npc', name = 'Barmaid Terese', npc_flags = 128 }", "212,192,111"),
        ("a creature (no NPC bit): tan", "{ kind = 'npc', name = 'orc pawn', npc_flags = 0 }", "212,192,111"),
        ("a pet: blue", "{ kind = 'pet', name = 'Fang' }", "100,175,220"),
        ("a mercenary: reddish blue", "{ kind = 'pet', name = 'Merc', merc_tag = '<Ann Mercenary>' }", "155,165,210"),
        ("a player: green", "{ kind = 'player', name = 'Ann' }", "90,255,80"),
        ("a coachman: light blue", "{ kind = 'npc', name = 'Coachman Ronks', npc_flags = 129 }", "130,190,255"),
        ("a banker by the flag: cyan", "{ kind = 'npc', name = 'The Treasurer', npc_flags = 130 }", "0,240,240"),
        ("a spell merchant by name: purple", "{ kind = 'npc', name = 'Spell Archivist', npc_flags = 129 }", "176,96,255"),
        ("a smith by the flag: orange", "{ kind = 'npc', name = 'Yiggins', npc_flags = 136 }", "255,140,0"),
        ("a trainer by name: pink", "{ kind = 'npc', name = 'Guildmaster Ferris', npc_flags = 128 }", "255,110,199"),
        ("a quest giver with a mark: white", "{ kind = 'npc', name = 'Dilana', npc_flags = 128, head_mark = '!' }", "255,255,255"),
        ("a merchant: yellow", "{ kind = 'npc', name = 'Merchant Perry', npc_flags = 129 }", "255,236,40"),
        ("a coachman who sells is still a coachman", "{ kind = 'npc', name = 'Coachman Ronks', npc_flags = 129 }", "130,190,255"),
    ];
    for (what, entity, want) in cases {
        assert_eq!(rgb(&rig, entity), want, "{what}");
    }
}

/// The dots a frame draws are coloured from the entity records, not a single colour.
#[test]
fn world_map_dots_are_drawn_in_their_colours() {
    fresh();
    let entity = |id: u64, name: &str, kind: &str, flags: u64| {
        json!({
            "ref": {"$ref": "entity", "key": id.to_string(), "generation": 0, "incarnation": 1},
            "id": id, "name": name, "level": 7, "kind": kind, "is_creature": true, "corpse": false, "hp_fraction": 1.0,
            "search_flags": 0, "class_code": 0, "world_id": 0, "npc_flags": flags,
            "position": {"x": 25300.0, "y": 0.0, "z": 15200.0}, "actor_position": {"x": 25300.0, "y": 0.0, "z": 15200.0},
        })
    };
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.entities = vec![entity(9001, "Coachman Ronks", "npc", 129), entity(9002, "orc pawn", "npc", 0), entity(9003, "Ann", "player", 0)];
        c.view.dots = vec![(9001, 25300.0, 15200.0), (9002, 25310.0, 15200.0), (9003, 25320.0, 15200.0)];
    });
    let mut rig = rig([1024.0, 768.0], None);
    rig.open();
    rig.frames(3);
    let col = |id: u32| rig.lua_f64(&format!("world_map.dot_cache[{id}] and world_map.dot_cache[{id}].col or 0"));
    let (coach, orc, ann) = (col(9001), col(9002), col(9003));
    assert!(coach != 0.0 && orc != 0.0 && ann != 0.0, "each dot was coloured: {coach} {orc} {ann}");
    assert!(coach != orc && ann != orc && coach != ann, "a coachman, an orc and a player are three colours: {coach} {orc} {ann}");
}
