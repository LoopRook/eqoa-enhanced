// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1: the two new bundled mod windows, Spellbook and Quest Journal. Each ships off, declares exactly what
//! it calls, opens on its key into a session, draws without a diagnostic, closes on the key again, on Escape, and
//! when the client ends its session (zoning, death, the player's switch), and draws nothing once closed.
use super::{
    action_broker,
    bridge::Bridge,
    confirm,
    discovery::{DiscoveredAddon, EntryMode},
    host::Host,
    native_core::{CORE, Core},
    native_events, pointer_api, window_sessions as ws, window_takeover,
};
use dear_imgui_rs::Context;
use serde_json::json;
use std::path::PathBuf;

fn root(id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(id).canonicalize().unwrap()
}

fn manifest(id: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root(id).join("mod.json")).unwrap()).unwrap()
}

fn addon(id: &str) -> DiscoveredAddon {
    let m = manifest(id);
    DiscoveredAddon {
        id: id.into(),
        name: id.into(),
        entry: root(id).join("main.lua"),
        script_root: root(id),
        package_root: root(id),
        module_dirs: std::iter::once(root(id)).chain(root(id).join("modules").canonicalize().ok()).collect(),
        resource_dirs: vec![root(id)],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: m["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect(),
        extensions: Default::default(),
        initially_enabled: false,
        mods_root: None,
    }
}

fn context() -> Context {
    let mut context = Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_display_size([1024.0, 768.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.font_atlas().try_claim_legacy_renderer().unwrap().build();
    context
}

fn fixture() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = json!({"id": 256, "name": "Me", "level": 20});
        c.view.quests = vec![
            json!({"ref": {"$ref": "quest", "key": "1", "generation": 0, "incarnation": 1}, "index": 1, "title": "Bat Eyes"}),
            json!({"ref": {"$ref": "quest", "key": "2", "generation": 0, "incarnation": 1}, "index": 2, "title": "The Militia"}),
        ];
    });
}

fn frames(host: &Host, bridge: &Bridge, context: &mut Context, n: usize) -> usize {
    let mut vertices = 0;
    for _ in 0..n {
        context.frame();
        bridge
            .invoke(|| {
                native_events::dispatch(host)?;
                native_events::render(host)
            })
            .unwrap();
        vertices = context.render_legacy().draw_data().total_vtx_count();
    }
    vertices
}

fn press(host: &Host, key: &str, combo: &str) {
    CORE.with(|c| {
        c.borrow_mut().emit(
            "input.key",
            json!({"key": key, "combo": combo, "down": true, "repeat": false, "synthetic": false, "text": ""}),
        )
    });
    native_events::dispatch(host).unwrap();
}

/// Every `eqoa.<module>.<name>` the script calls that needs a declaration is declared.
fn declared_covers_used(id: &str) {
    let src = std::fs::read_to_string(root(id).join("main.lua")).unwrap();
    let declared: Vec<String> =
        manifest(id)["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
    let used = |prefix: &str| -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = src.as_str();
        while let Some(i) = rest.find(prefix) {
            rest = &rest[i + prefix.len()..];
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !found.contains(&name) {
                found.push(name);
            }
        }
        found
    };
    for name in used("eqoa.actions.") {
        assert!(declared.contains(&format!("eqoa.actions.{name}")), "{id} calls eqoa.actions.{name} and must declare it");
    }
    let declared_actions: Vec<_> = declared.iter().filter(|c| c.starts_with("eqoa.actions.")).collect();
    for cap in declared_actions {
        let name = cap.trim_start_matches("eqoa.actions.");
        assert!(used("eqoa.actions.").iter().any(|u| u == name), "{id} declares {cap} and never calls it");
    }
    for name in used("eqoa.windows.") {
        assert!(matches!(name.as_str(), "open_session" | "close_session"), "{id} uses eqoa.windows.{name}");
        assert!(declared.contains(&"eqoa.windows.session".to_string()), "{id} must declare eqoa.windows.session");
    }
}

fn run(id: &str, key: &str, combo: &str, title_kind: &str) {
    fixture();
    let manifest = manifest(id);
    assert_eq!(manifest["enabled"], false, "ships off");
    assert!(manifest["provides"].as_array().is_some_and(|p| p.len() == 1), "{id} says what it provides");
    declared_covers_used(id);
    assert!(crate::fork_ext::bundled::PACKAGES.contains(&id));
    for file in ["mod.json", "main.lua", "README.md"] {
        assert!(crate::fork_ext::bundled::EMBEDDED_FILES.iter().any(|f| f.path == format!("{id}/{file}")), "{file} ships");
    }
    let mut context = context();
    let host = Host::for_addon(&addon(id)).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon(id)).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    assert_eq!(
        crate::fork_ext::bindpage::get_key(id, "open").as_deref(),
        Some(combo),
        "its default key is on its Controls row (a game key would have emptied it)"
    );
    // control: shut, it draws nothing and holds no session
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0);
    assert!(ws::get(vm, title_kind).is_none());
    // the key opens it: a session, a drawn window
    press(&host, key, combo);
    let session = ws::get(vm, title_kind).unwrap_or_else(|| panic!("{id} opened a {title_kind} session; logs {:?}", host.drain_logs()));
    assert_eq!((session.origin, session.opened_from.as_str()), ("mod", "key"));
    let vertices = frames(&host, &bridge, &mut context, 3);
    let logs = host.drain_logs().unwrap();
    assert!(vertices > 0, "{id} draws its window: {logs:?}");
    assert!(logs.is_empty(), "{id}: no diagnostic: {logs:?}");
    // the key again closes it and ends the session
    press(&host, key, combo);
    assert!(ws::get(vm, title_kind).is_none(), "{id} closes on its key");
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0, "and draws nothing");
    // Escape closes it too
    press(&host, key, combo);
    assert!(ws::get(vm, title_kind).is_some());
    press(&host, "escape", "escape");
    assert!(ws::get(vm, title_kind).is_none(), "{id} closes on Escape");
    // the client ends the session (zoning, death, the player's switch): the mod hears and closes
    for end in [
        Box::new(|| ws::set_env(ws::Env { away: true, ..ws::Env::default() })) as Box<dyn Fn()>,
        Box::new(|| ws::set_env(ws::Env { dead: true, ..ws::Env::default() })),
    ] {
        ws::set_env(ws::Env::default());
        press(&host, key, combo);
        assert!(ws::get(vm, title_kind).is_some());
        end();
        frames(&host, &bridge, &mut context, 2);
        assert!(ws::get(vm, title_kind).is_none());
        assert_eq!(frames(&host, &bridge, &mut context, 2), 0, "{id} stopped drawing when its session ended");
        assert!(host.drain_logs().unwrap().is_empty());
    }
    ws::set_env(ws::Env::default());
    // while the player uses the game's windows the key does nothing (the control: it opened a moment ago)
    window_takeover::set_use_game_windows(true);
    press(&host, key, combo);
    assert!(ws::get(vm, title_kind).is_none(), "{id} does not open while the game's windows are in use");
    window_takeover::set_use_game_windows(false);
    // disabling closes everything
    press(&host, key, combo);
    assert!(ws::get(vm, title_kind).is_some());
    native_events::clear(&host);
    assert!(ws::get(vm, title_kind).is_none());
}

#[test]
fn a1_spellbook_opens_on_shift_k_draws_and_closes_every_way_it_should() {
    run("eqoa_spellbook", "k", "shift+k", "spellbook");
}

#[test]
fn a1_quest_journal_opens_on_shift_j_draws_and_closes_every_way_it_should() {
    run("eqoa_quest_journal", "j", "shift+j", "quest_log");
}

// ---- fix round 1 (review findings 1 and 2) ----

/// A one-file mod in a scratch folder, with the capabilities it declares.
fn scratch_addon(name: &str, caps: &[&str], source: &str) -> (DiscoveredAddon, PathBuf) {
    let dir = std::env::temp_dir().join(format!("eqoa-a1-fix1-{}-{name}", std::process::id()));
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
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: caps.iter().map(|c| c.to_string()).collect(),
        extensions: Default::default(),
        initially_enabled: false,
        mods_root: None,
    };
    (addon, dir)
}

/// One frame the way the runtime draws it: the mods' windows, then the client's question over them.
fn frame_with_question(host: &Host, bridge: &Bridge, context: &mut Context) {
    let ui = context.frame();
    bridge
        .invoke(|| {
            super::runtime::with_question_on_top(ui, || {
                native_events::dispatch(host)?;
                native_events::render(host)
            })
        })
        .unwrap();
    drop(context.render_legacy());
}

fn window_rect(name: &str) -> Option<([f32; 2], [f32; 2])> {
    let name = std::ffi::CString::new(name).unwrap();
    let w = unsafe { dear_imgui_rs::sys::igFindWindowByName(name.as_ptr()) };
    (!w.is_null()).then(|| unsafe { ([(*w).Pos.x, (*w).Pos.y], [(*w).Size.x, (*w).Size.y]) })
}

fn hovered_window_name() -> String {
    unsafe {
        let g = dear_imgui_rs::sys::igGetCurrentContext();
        let w = (*g).HoveredWindow;
        if w.is_null() { String::new() } else { std::ffi::CStr::from_ptr((*w).Name).to_string_lossy().into_owned() }
    }
}

#[test]
fn a1_fix1_the_question_is_over_every_mod_window_and_a_mod_cannot_take_its_name() {
    fixture();
    // a mod window as big as the screen, with a fake button over where the question's Yes will be; and a mod that
    // tries to open a window with the question's id, to add to it
    let (a, dir) = scratch_addon(
        "cover",
        &["eqoa.events.on", "eqoa.imgui"],
        r#"
        local ui = eqoa.imgui
        eqoa.events.on("render", function()
            ui.SetNextWindowPos(0, 0, ImGuiCond.Always)
            ui.SetNextWindowSize(1024, 768, ImGuiCond.Always)
            ui.Begin("cover", true, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoSavedSettings)
            ui.Button("Fake harmless button")
            ui.End()
            local ok = pcall(function() ui.Begin("Are you sure?###eqoa-client-confirm"); ui.End() end)
            took_the_name = ok
        end)
        "#,
    );
    let mut context = context();
    let host = Host::for_addon(&a).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&a).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    let ask = |act| confirm::open(vm, "cover", None, act, "Destroy Rock?".into(), 0).unwrap();
    // control: with no question up, the mod's window is what the pointer finds anywhere on the screen
    frame_with_question(&host, &bridge, &mut context);
    context.io_mut().add_mouse_pos_event([512.0, 384.0]);
    context.frame();
    assert_eq!(hovered_window_name(), "cover");
    drop(context.render_legacy());
    // the question comes up: it is drawn after the mod's window ...
    ask(confirm::Act::Destroy { inv_index: 1, name: "Rock".into(), count: 1 });
    for _ in 0..3 {
        frame_with_question(&host, &bridge, &mut context);
    }
    let (pos, size) = window_rect(confirm::WINDOW).expect("the question is drawn");
    let centre = [pos[0] + size[0] * 0.5, pos[1] + size[1] * 0.5];
    // ... so a pointer anywhere on its face finds the question, not the mod's window over the whole screen
    context.io_mut().add_mouse_pos_event(centre);
    context.frame();
    let found = hovered_window_name();
    drop(context.render_legacy());
    assert!(found.starts_with("Are you sure?"), "the pointer on the question finds {found:?}, not the question");
    // and it stays in front every frame, even after the mod's window has been drawn and clicked many times
    for _ in 0..5 {
        frame_with_question(&host, &bridge, &mut context);
        context.io_mut().add_mouse_pos_event(centre);
        context.frame();
        let found = hovered_window_name();
        drop(context.render_legacy());
        assert!(found.starts_with("Are you sure?"), "still in front: {found:?}");
    }
    // the mod could not open a window with the question's id
    let took: String = host.lua.load("return tostring(took_the_name)").eval().unwrap();
    assert_eq!(took, "false", "Begin with the client's window id is refused");
    assert!(confirm::reserved("Are you sure?###eqoa-client-confirm") && !confirm::reserved("Are you sure?"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a1_fix1_a_mod_that_errors_in_render_or_windows_key_loses_the_controller_and_its_windows_at_once() {
    use crate::input::{Report, bit};
    for event in ["render", "windows.key"] {
        fixture();
        let (a, dir) = scratch_addon(
            "breaks",
            &["eqoa.events.on", "eqoa.windows.session", "eqoa.windows.focus", "eqoa.imgui"],
            &format!(
                r#"
                s = assert(eqoa.windows.open_session("inventory"))
                assert(eqoa.windows.focus(s.ref))
                eqoa.events.on("{event}", function() error("boom") end)
                "#
            ),
        );
        let mut context = context();
        let host = Host::for_addon(&a).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        host.run_addon(&a).unwrap();
        let vm = native_events::bus(&host.lua).borrow().vm;
        // control: before the error the mod holds the controller, the game gets no Cross
        assert!(ws::get(vm, "inventory").is_some());
        let mut r = Report { buttons: bit::CROSS, ..Default::default() };
        ws::apply_pad(&mut r);
        assert_eq!(r.buttons, 0, "{event}: the mod holds the controller until it fails");
        // that press was told to the mod as windows.key (its handler fails); the render handler fails on this frame
        context.frame();
        bridge
            .invoke(|| {
                native_events::dispatch(&host)?;
                native_events::render(&host)
            })
            .unwrap();
        drop(context.render_legacy());
        assert!(ws::get(vm, "inventory").is_none(), "{event}: its window session ended with the error");
        let mut r = Report { buttons: bit::CROSS | bit::UP | bit::TRIANGLE, ..Default::default() };
        ws::apply_pad(&mut r);
        assert_eq!(r.buttons, bit::CROSS | bit::UP | bit::TRIANGLE, "{event}: the game's buttons are the game's again");
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Names of the draw lists with something in them, back to front, of the frame just rendered.
fn drawn_back_to_front(context: &mut Context) -> Vec<String> {
    let _ = context;
    unsafe {
        let d = dear_imgui_rs::sys::igGetDrawData();
        let lists = &(*d).CmdLists;
        (0..lists.Size as usize)
            .filter_map(|i| {
                let l = *lists.Data.add(i);
                ((*l).VtxBuffer.Size > 0).then(|| std::ffi::CStr::from_ptr((*l)._OwnerName).to_string_lossy().into_owned())
            })
            .collect()
    }
}

#[test]
fn a1_fix1_no_mod_tooltip_popup_or_window_is_drawn_above_the_question() {
    fixture();
    let (a, dir) = scratch_addon(
        "layers",
        &["eqoa.events.on", "eqoa.imgui"],
        r#"
        local ui = eqoa.imgui
        eqoa.events.on("render", function()
            ui.SetNextWindowPos(480, 300, ImGuiCond.Always)
            ui.SetNextWindowSize(300, 200, ImGuiCond.Always)
            ui.Begin("over", true, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoBringToFrontOnFocus)
            ui.Text("over the question's place")
            ui.SetTooltip("Destroy nothing: this is safe")
            ui.OpenPopup("pp")
            if ui.BeginPopup("pp") then ui.Text("a popup over it"); ui.EndPopup() end
            ui.End()
        end)
        "#,
    );
    let mut context = context();
    let host = Host::for_addon(&a).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&a).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    // control: with no question up, the mod's tooltip is drawn above everything (the layer this closes)
    for _ in 0..4 {
        frame_with_question(&host, &bridge, &mut context);
    }
    let layers = drawn_back_to_front(&mut context);
    assert!(
        layers.last().is_some_and(|n| n.starts_with("##Tooltip")),
        "control: the tooltip is the topmost layer without a question: {layers:?}"
    );
    confirm::open(vm, "layers", None, confirm::Act::Destroy { inv_index: 1, name: "Rock".into(), count: 1 }, "Destroy Rock?".into(), 0).unwrap();
    for _ in 0..4 {
        frame_with_question(&host, &bridge, &mut context);
    }
    let layers = drawn_back_to_front(&mut context);
    assert!(
        layers.last().is_some_and(|n| n.starts_with("Are you sure?")),
        "the question is the topmost layer; mod tooltips, popups and windows are below it: {layers:?}"
    );
    assert!(!layers.iter().any(|n| n.starts_with("##Tooltip")), "no mod tooltip is drawn while the question is up: {layers:?}");
    // the question over, tooltips are the mod's again
    confirm::cancel_all("test");
    for _ in 0..4 {
        frame_with_question(&host, &bridge, &mut context);
    }
    assert!(drawn_back_to_front(&mut context).iter().any(|n| n.starts_with("##Tooltip")));
    let _ = std::fs::remove_dir_all(dir);
}

/// Owner test 2026-11-23: with "Use the game's windows" on, the key must not raise a mod error (the mods' "did not
/// open" note used to call `eqoa.log.write` without a level). Each of the three mods writes a leveled note instead.
#[test]
fn a1_owner_fix_a_key_that_cannot_open_a_window_logs_a_note_not_a_mod_error() {
    for (id, key, combo) in [
        ("eqoa_spellbook", "k", "shift+k"),
        ("eqoa_quest_journal", "j", "shift+j"),
        ("eqoa_bag_viewer", "b", "shift+b"),
    ] {
        fixture();
        let host = Host::for_addon(&addon(id)).unwrap();
        let _bridge = Bridge::install(&host).unwrap();
        host.run_addon(&addon(id)).unwrap();
        host.drain_logs().unwrap();
        window_takeover::set_use_game_windows(true);
        press(&host, key, combo);
        let logs = format!("{:?}", host.drain_logs().unwrap());
        window_takeover::set_use_game_windows(false);
        assert!(!logs.contains("invalid_argument"), "{id}: no mod error when the window cannot open: {logs}");
        assert!(logs.contains("did not open"), "{id}: says why it did not open: {logs}");
        // owner test 2026-11-23: the note is "info" (the game's log keeps it, and the Mods & Addons diagnostics, which
        // is where a player sees a mod's warning, do not): the player's own setting is not a fault
        assert!(logs.contains("eqoa/1 [info] "), "{id}: the note is info level: {logs}");
        assert!(!logs.contains("[warn]") && !logs.contains("[error]"), "{id}: no warn or error note: {logs}");
    }
}

fn click(host: &Host, x: f64, y: f64, button: &str) {
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": button})));
    native_events::dispatch(host).unwrap();
}

fn ability(n: i64, slot: Option<i64>) -> serde_json::Value {
    json!({"ref": {"$ref": "ability", "key": n.to_string(), "generation": 0, "incarnation": 1}, "id": n,
        "name": format!("Skill {n:02}"), "description": "", "display_key": n, "spellbook_slot": slot,
        // as the client fills them: `usable` is "on a slot", `learned` is "the character has it"
        "usable": slot.is_some(), "learned": true,
        "level": n, "range": 5.0, "cast_time": 1.0, "power_cost": 3, "scope": 0, "cooldown": 0.0, "equipment_mask": 0, "lockout": 0.0})
}

/// Owner test 2026-11-23: the Spellbook's scroll bar takes a click (it only drew before) and Memorize is a button,
/// with the game's refusal (an ability already memorized) said and nothing sent. The window's geometry is the
/// mod's (HUD pixels: search row at y 22, list at y 40, 12 rows of 13 with a header row for each ten levels; slots
/// from y 228; buttons at y 395; the window is 419 high).
#[test]
fn a1_owner_fix_spellbook_scrollbar_click_and_memorize_button() {
    let id = "eqoa_spellbook";
    fixture();
    // 20 abilities (more than the 13 rows), the first memorized in slot 1
    CORE.with(|c| c.borrow_mut().view.abilities = (1..=20).map(|n| ability(n, (n == 1).then_some(1))).collect());
    let mut context = context();
    let host = Host::for_addon(&addon(id)).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon(id)).unwrap();
    press(&host, "k", "shift+k");
    frames(&host, &bridge, &mut context, 3);
    let (k, ox, oy): (f64, f64, f64) = host
        .lua
        .load("local f = eqoa.hud.frame(); return math.min(f.scale_x, f.scale_y), f.offset_x, f.offset_y")
        .eval()
        .unwrap();
    let (w, h) = (332.0, 419.0);
    let x0 = (1024.0 - w * k).min(ox + (640.0 - w) / 2.0 * k).max(0.0);
    let y0 = (768.0 - h * k).min(oy + (448.0 - h) / 2.0 * k).max(0.0);
    let at = |x: f64, y: f64| (x0 + x * k, y0 + y * k);
    let mut tap = |x: f64, y: f64, button: &str| {
        let (px, py) = at(x, y);
        click(&host, px, py, button);
        frames(&host, &bridge, &mut context, 2);
    };
    // the first row is the header "Levels 1-10", the second Skill 01, memorized in slot 1: chosen, Memorize is refused
    // (no button) and so is a slot click
    tap(60.0, 40.0 + 13.0 + 6.0, "left");
    tap(14.0, 403.0, "left");
    tap(8.0 + 2.0 * 32.0 + 5.0, 228.0 + 5.0, "left"); // slot 3
    assert!(action_broker::queued_kinds_for_test().is_empty(), "a memorized ability is not memorized again");
    // a click near the foot of the scroll bar (x 325..331, y 40..196) jumps the list down: 22 rows, 10 hidden, so
    // Skill 10 is the first row now
    tap(328.0, 40.0 + 156.0 * 0.95, "left");
    tap(60.0, 40.0 + 6.0, "left");
    tap(14.0, 403.0, "left"); // the Memorize button
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["assign_ability"], "the scrolled-to ability is memorized");
    assert_eq!(host.drain_logs().unwrap().len(), 0);
}

/// Owner test 2026-11-23 (Return Home, level 1, free slot 7: "cannot be memorized yet"). The ability record's
/// `usable` is the game's "on a slot" flag, false for every ability not yet memorized, so it cannot be the test.
/// The game's own rule (`SpellBook::popup_message`): memorized refuses, not learned refuses, else it goes on the
/// slot: here the first free one.
#[test]
fn a1_owner_fix_spellbook_memorizes_a_learned_unmemorized_ability_into_the_first_free_slot() {
    let id = "eqoa_spellbook";
    fixture();
    // slots 1, 2 and 4 are taken; Skill 03 is learned and not memorized; Skill 05 is not learned at all
    CORE.with(|c| {
        c.borrow_mut().view.abilities = vec![
            ability(1, Some(1)),
            ability(2, Some(2)),
            ability(3, None),
            ability(4, Some(4)),
            {
                let mut a = ability(5, None);
                a["learned"] = json!(false);
                a
            },
        ]
    });
    let mut context = context();
    let host = Host::for_addon(&addon(id)).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon(id)).unwrap();
    press(&host, "k", "shift+k");
    frames(&host, &bridge, &mut context, 3);
    let (k, ox, oy): (f64, f64, f64) = host
        .lua
        .load("local f = eqoa.hud.frame(); return math.min(f.scale_x, f.scale_y), f.offset_x, f.offset_y")
        .eval()
        .unwrap();
    let (w, h) = (332.0, 419.0);
    let x0 = (1024.0 - w * k).min(ox + (640.0 - w) / 2.0 * k).max(0.0);
    let y0 = (768.0 - h * k).min(oy + (448.0 - h) / 2.0 * k).max(0.0);
    let mut tap = |x: f64, y: f64| {
        click(&host, x0 + x * k, y0 + y * k, "left");
        frames(&host, &bridge, &mut context, 2);
    };
    // the ability not learned is not listed: its would-be row (the header, then four listed, so the sixth, y 40 + 5 *
    // 13) selects nothing, so the Memorize button (shown off) sends nothing
    tap(60.0, 40.0 + 5.0 * 13.0 + 5.0);
    tap(14.0, 403.0);
    assert!(action_broker::queued_kinds_for_test().is_empty(), "nothing chosen, nothing memorized");
    // Skill 03 (the fourth row, after the header): chosen, then the Memorize button: it goes onto slot 3, the first
    // free one
    tap(60.0, 40.0 + 3.0 * 13.0 + 5.0);
    tap(14.0, 403.0);
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["assign_ability"], "the learned ability is memorized");
    // (the broker counts slots from 0: 2 is the third)
    assert_eq!(action_broker::queued_assign_slots_for_test(), vec![2], "into the first free slot, the third");
    assert_eq!(host.drain_logs().unwrap().len(), 0);
}

/// Every vertex the Spellbook's layer drew in the frame just rendered, as (x, y, colour).
fn spellbook_vertices() -> Vec<(f32, f32, u32)> {
    unsafe {
        let d = dear_imgui_rs::sys::igGetDrawData();
        let lists = &(*d).CmdLists;
        let mut out = Vec::new();
        for i in 0..lists.Size as usize {
            let l = *lists.Data.add(i);
            if !std::ffi::CStr::from_ptr((*l)._OwnerName).to_string_lossy().starts_with("eqoa_spellbook_layer") {
                continue;
            }
            for j in 0..(*l).VtxBuffer.Size as usize {
                let v = *(*l).VtxBuffer.Data.add(j);
                out.push((v.pos.x, v.pos.y, v.col));
            }
        }
        out
    }
}

/// Owner test 2026-11-23: the Spellbook's header, red status line and "Shift+K or Esc" hint ran off or behind the
/// window's X. Drawn headless at the default size with a long name, a long description and a long refusal: nothing is
/// drawn outside the window, the mod says it cut no text, and the hint ends before the X.
#[test]
fn a1_owner_fix_spellbook_text_stays_inside_its_window() {
    let id = "eqoa_spellbook";
    fixture();
    CORE.with(|c| {
        let mut one = ability(1, Some(1));
        one["name"] = json!("Anthem of Conquest and Valor");
        one["description"] = json!(
            "Sings a rousing anthem that raises the strength and the courage of every ally who can hear the bard, \
             lasting a long while, and does so again each time the bard finishes the verse before it fades away \
             completely, which is a great deal more than the three lines that the book can show for one ability"
        );
        c.borrow_mut().view.abilities = vec![one, ability(2, None)];
    });
    let mut context = context();
    let host = Host::for_addon(&addon(id)).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon(id)).unwrap();
    press(&host, "k", "shift+k");
    frames(&host, &bridge, &mut context, 3);
    let (k, ox, oy): (f64, f64, f64) = host
        .lua
        .load("local f = eqoa.hud.frame(); return math.min(f.scale_x, f.scale_y), f.offset_x, f.offset_y")
        .eval()
        .unwrap();
    let (w, h) = (332.0, 419.0);
    // The window is the rectangle its surface fill (0xF5111111) draws; everything but that fill and the 1 px border
    // (0x404040, anti-aliased, its fringe reaching past the edge) is text or a control and must lie inside it.
    let inside = |label: &str, vertices: &[(f32, f32, u32)]| -> (f64, f64) {
        let fill: Vec<_> = vertices.iter().filter(|v| v.2 == 0xF511_1111).collect();
        assert!(fill.len() >= 4, "{label}: the window's surface was drawn");
        let lo = |f: fn(&&(f32, f32, u32)) -> f32| fill.iter().map(f).fold(f32::MAX, f32::min) as f64;
        let hi = |f: fn(&&(f32, f32, u32)) -> f32| fill.iter().map(f).fold(f32::MIN, f32::max) as f64;
        let (left, top, right, bottom) = (lo(|v| v.0), lo(|v| v.1), hi(|v| v.0), hi(|v| v.1));
        assert!((right - left - w * k).abs() < 2.0 && (bottom - top - h * k).abs() < 2.0, "{label}: the window is {}x{}, expected {}x{}", right - left, bottom - top, w * k, h * k);
        let eps = 1.0;
        assert!(vertices.len() > 100, "{label}: the window drew ({} vertices)", vertices.len());
        for &(x, y, col) in vertices.iter().filter(|v| v.2 != 0xF511_1111 && v.2 & 0x00FF_FFFF != 0x0040_4040) {
            assert!(
                f64::from(x) >= left - eps && f64::from(x) <= right + eps && f64::from(y) >= top - eps && f64::from(y) <= bottom + eps,
                "{label}: a vertex at ({x}, {y}) colour {col:#010x} is outside the window ({left}, {top})-({right}, {bottom})"
            );
        }
        (left, top)
    };
    let (x0, y0) = inside("nothing chosen", &spellbook_vertices());
    // the hint ("Shift+K or Esc", the muted grey, 0xFF999999) is in the title row and ends before the X (at W - 18)
    let hint_right = spellbook_vertices()
        .iter()
        .filter(|v| v.2 == 0xFF99_9999 && f64::from(v.1) < y0 + 18.0 * k)
        .map(|v| f64::from(v.0))
        .fold(f64::MIN, f64::max);
    assert!(hint_right > x0 + 200.0 * k, "the hint was found in the title row ({hint_right})");
    assert!(hint_right <= x0 + (w - 18.0) * k, "the hint ends {hint_right} but the X begins at {}", x0 + (w - 18.0) * k);
    // choose the long-named memorized ability, then click a free slot: it is already memorized, a long refusal
    let mut tap = |x: f64, y: f64| {
        click(&host, x0 + x * k, y0 + y * k, "left");
        frames(&host, &bridge, &mut context, 2);
    };
    tap(60.0, 40.0 + 13.0 + 6.0);
    tap(8.0 + 2.0 * 32.0 + 5.0, 228.0 + 5.0);
    inside("long name, long description and a long refusal", &spellbook_vertices());
    // and the texts came out whole: no text was cut to fit (the mod says so in the game's log when it must)
    let logs = format!("{:?}", host.drain_logs().unwrap());
    assert!(!logs.contains("cut a text"), "nothing was cut: {logs}");
    // the red status line is drawn (the warning colour) and sits above the buttons (y 395), below the description
    let red = spellbook_vertices().iter().filter(|v| v.2 == 0xFF4D_4DFF).map(|v| f64::from(v.1)).fold(f64::MAX, f64::min);
    assert!(red >= y0 + 367.0 * k - 1.0, "the status starts under the description: {red}");
}

// ---- Spellbook 2 (2026-11-23): search, level groups, scope buttons, icons, drag and drop -------------------------
// Geometry is the mod's, in HUD pixels: search box x 8..112, y 22..36; scope buttons 38 wide, 41 apart from x 118
// (Self, Target, Group, Pet, All); list from y 40, 12 rows of 13; slots 30 wide, 32 apart, from (8, 228); the
// Memorize button at (8.., 395..411); the window 332 x 419.

fn named(n: i64, name: &str, level: i64, scope: i64, slot: Option<i64>) -> serde_json::Value {
    let mut a = ability(n, slot);
    a["name"] = json!(name);
    a["level"] = json!(level);
    a["scope"] = json!(scope);
    a
}

/// The centre of the nth list row (1-based).
fn row_xy(n: usize) -> (f64, f64) {
    (60.0, 40.0 + (n - 1) as f64 * 13.0 + 6.0)
}

/// The centre of slot n (1-based).
fn slot_xy(n: usize) -> (f64, f64) {
    (8.0 + ((n - 1) % 10) as f64 * 32.0 + 15.0, 228.0 + ((n - 1) / 10) as f64 * 32.0 + 15.0)
}

/// The nth scope button (1 Self, 2 Target, 3 Group, 4 Pet, 5 All).
fn filter_xy(n: usize) -> (f64, f64) {
    (118.0 + (n - 1) as f64 * 41.0 + 19.0, 29.0)
}

const SEARCH_XY: (f64, f64) = (30.0, 29.0);
const MEMORIZE_XY: (f64, f64) = (14.0, 403.0);

struct Book {
    host: Host,
    bridge: Bridge,
    context: Context,
    k: f64,
    x0: f64,
    y0: f64,
}

impl Book {
    fn open(abilities: Vec<serde_json::Value>) -> Book {
        Book::open_after(abilities, "")
    }

    /// Opens the window after running `lua` in the mod's VM (to watch what it calls from its first frame).
    fn open_after(abilities: Vec<serde_json::Value>, lua: &str) -> Book {
        fixture();
        CORE.with(|c| c.borrow_mut().view.abilities = abilities);
        let mut context = context();
        let host = Host::for_addon(&addon("eqoa_spellbook")).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        host.run_addon(&addon("eqoa_spellbook")).unwrap();
        host.lua.load(lua).exec().unwrap();
        press(&host, "k", "shift+k");
        frames(&host, &bridge, &mut context, 3);
        let (k, ox, oy): (f64, f64, f64) = host
            .lua
            .load("local f = eqoa.hud.frame(); return math.min(f.scale_x, f.scale_y), f.offset_x, f.offset_y")
            .eval()
            .unwrap();
        let (w, h) = (332.0, 419.0);
        let x0 = (1024.0 - w * k).min(ox + (640.0 - w) / 2.0 * k).max(0.0);
        let y0 = (768.0 - h * k).min(oy + (448.0 - h) / 2.0 * k).max(0.0);
        Book { host, bridge, context, k, x0, y0 }
    }

    fn px(&self, (x, y): (f64, f64)) -> [f32; 2] {
        [(self.x0 + x * self.k) as f32, (self.y0 + y * self.k) as f32]
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            // the game's frame counter moves on (a mod may claim the pointer 32 times per frame, no more)
            CORE.with(|c| c.borrow_mut().frame += 1);
            frames(&self.host, &self.bridge, &mut self.context, 1);
        }
    }

    fn tap_button(&mut self, at: (f64, f64), button: &str) {
        let p = self.px(at);
        click(&self.host, f64::from(p[0]), f64::from(p[1]), button);
        self.frames(2);
    }

    fn tap(&mut self, at: (f64, f64)) {
        self.tap_button(at, "left");
    }

    /// Typed characters while the search box has the keyboard (`text` is set only for a mod holding capture_text).
    fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            let lower = ch.to_lowercase().to_string();
            CORE.with(|c| {
                c.borrow_mut().emit(
                    "input.key",
                    json!({"key": lower, "combo": lower, "down": true, "repeat": false, "synthetic": false,
                        "text": ch.to_string()}),
                )
            });
        }
        native_events::dispatch(&self.host).unwrap();
        self.frames(1);
    }

    fn backspace(&mut self) {
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                json!({"key": "backspace", "combo": "backspace", "down": true, "repeat": false, "synthetic": false, "text": ""}),
            )
        });
        native_events::dispatch(&self.host).unwrap();
        self.frames(1);
    }

    /// The memorize/unmemorize requests the window has queued so far (sent to the game now, answered "completed").
    fn sent(&mut self) -> Vec<String> {
        let steps = action_broker::queued_ability_steps_for_test();
        action_broker::dispatch_all_for_test();
        native_events::dispatch(&self.host).unwrap();
        self.frames(1);
        steps
    }

    fn memorize(&mut self) {
        self.tap(MEMORIZE_XY);
    }

    /// A real drag: press at `from`, travel to `to` in steps with the button down, release there.
    fn drag(&mut self, from: (f64, f64), to: (f64, f64)) {
        let (a, b) = (self.px(from), self.px(to));
        pointer_api::position(a, true, (1024, 768));
        self.context.io_mut().add_mouse_pos_event(a);
        self.frames(1);
        click(&self.host, f64::from(a[0]), f64::from(a[1]), "left"); // the client's pointer.pressed
        self.context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
        self.frames(2);
        for i in 1..=4 {
            let t = i as f32 / 4.0;
            let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            pointer_api::position(p, true, (1024, 768));
            self.context.io_mut().add_mouse_pos_event(p);
            self.frames(1);
        }
        self.context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
        self.frames(2);
    }

    /// Everything a drag set going, one step at a time: each is answered "completed" before the next is sent.
    fn plan(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for _ in 0..8 {
            let queued = action_broker::queued_ability_steps_for_test();
            if queued.is_empty() {
                break;
            }
            assert_eq!(queued.len(), 1, "one step at a time, the next only after the game answered: {queued:?}");
            out.push(queued[0].clone());
            action_broker::dispatch_all_for_test();
            native_events::dispatch(&self.host).unwrap();
            self.frames(1);
        }
        out
    }
}

fn no_logs(b: &Book) {
    let logs = b.host.drain_logs().unwrap();
    assert!(logs.is_empty(), "no diagnostic: {logs:?}");
}

/// 1. A search box: typed letters filter the list by name, case-insensitively, as they come; Backspace widens it again;
/// Escape lets go of the keyboard first and only then closes the window. Negative control: the old window has no box,
/// so the second row (after the level header) is always the first ability by level.
#[test]
fn spellbook2_search_box_filters_the_list_by_name_as_you_type() {
    let mut b = Book::open(vec![
        named(1, "Anthem of Valor", 1, 0, None),
        named(2, "Bandage", 2, 1, None),
        named(3, "Cure Poison", 3, 1, None),
        named(4, "Bash", 4, 1, None),
    ]);
    // no search: header, Anthem, Bandage, Cure, Bash
    b.tap(row_xy(2));
    b.memorize();
    assert_eq!(b.sent(), vec!["assign slot 1 id 1"], "unfiltered, the first ability is Anthem");
    // type in capitals: "BA" matches Bandage and Bash only, in the list's order
    b.tap(SEARCH_XY);
    b.type_text("BA");
    b.tap(row_xy(2));
    b.memorize();
    assert_eq!(b.sent(), vec!["assign slot 1 id 2"], "the first match is Bandage");
    b.tap(row_xy(3));
    b.memorize();
    assert_eq!(b.sent(), vec!["assign slot 1 id 4"], "the second match is Bash");
    // Cure Poison and Anthem are not listed: the fourth row is empty, and a click on it chooses nothing
    b.tap(row_xy(2));
    b.tap(row_xy(4));
    b.memorize();
    assert_eq!(b.sent(), vec!["assign slot 1 id 2"], "the row where Cure Poison was is empty: Bandage stays chosen");
    // Backspace twice empties the box and the whole list is back
    b.tap(SEARCH_XY);
    b.backspace();
    b.backspace();
    b.tap(row_xy(2));
    b.memorize();
    assert_eq!(b.sent(), vec!["assign slot 1 id 1"], "an empty box lists everything again");
    // a name typed in the middle matches too ("poi" in Cure Poison)
    b.tap(SEARCH_XY);
    b.type_text("poi");
    b.tap(row_xy(2));
    b.memorize();
    assert_eq!(b.sent(), vec!["assign slot 1 id 3"], "a match inside the name");
    // Escape while typing lets go of the box and leaves the window open; the next Escape closes it
    b.tap(SEARCH_XY);
    let vm = native_events::bus(&b.host.lua).borrow().vm;
    press(&b.host, "escape", "escape");
    assert!(ws::get(vm, "spellbook").is_some(), "the first Escape only leaves the search box");
    press(&b.host, "escape", "escape");
    assert!(ws::get(vm, "spellbook").is_none(), "the second Escape closes the window");
    no_logs(&b);
}

/// 2. The list is grouped by level range, 1-10, 11-20 and so on, each under its own header row, which chooses nothing.
/// Negative control: with no groups the third row would be the second ability.
#[test]
fn spellbook2_list_is_grouped_by_level_range_with_header_rows() {
    let mut b = Book::open(vec![
        named(1, "Alpha", 5, 0, None),
        named(2, "Bravo", 12, 0, None),
        named(3, "Charlie", 25, 0, None),
        named(4, "Delta", 10, 0, None),
        named(5, "Echo", 11, 0, None),
    ]);
    // by level: Alpha 5, Delta 10 | Echo 11, Bravo 12 | Charlie 25, so the rows are
    // 1 header, 2 Alpha, 3 Delta, 4 header, 5 Echo, 6 Bravo, 7 header, 8 Charlie
    b.tap(row_xy(1));
    b.memorize();
    assert!(b.sent().is_empty(), "a header chooses nothing");
    for (row, id) in [(2, 1), (3, 4), (5, 5), (6, 2), (8, 3)] {
        b.tap(row_xy(row));
        b.memorize();
        assert_eq!(b.sent(), vec![format!("assign slot 1 id {id}")], "row {row}");
    }
    b.tap(row_xy(2));
    b.tap(row_xy(4));
    b.memorize();
    assert_eq!(b.sent(), vec!["assign slot 1 id 1"], "the second header is a header: the choice stays Alpha");
    no_logs(&b);
}

/// 3. Self, Target, Group, Pet and All buttons filter by `Ability.scope` (self 0, target 1, group 2, pet 3; All also
/// shows corpse 4 and unknown 5). Negative control: with no filter the second row is always the first ability.
#[test]
fn spellbook2_scope_buttons_filter_by_who_the_ability_affects() {
    let mut b = Book::open(vec![
        named(1, "Self One", 1, 0, None),
        named(2, "Target One", 2, 1, None),
        named(3, "Group One", 3, 2, None),
        named(4, "Pet One", 4, 3, None),
        named(5, "Corpse One", 5, 4, None),
        named(6, "Group Two", 6, 2, None),
    ]);
    // (button, first row's ability, the second row's ability if there is one)
    for (button, first, second) in [(4, 4, None), (3, 3, Some(6)), (1, 1, None), (2, 2, None)] {
        b.tap(filter_xy(button));
        b.tap(row_xy(2));
        b.memorize();
        assert_eq!(b.sent(), vec![format!("assign slot 1 id {first}")], "button {button}: first row");
        b.tap(row_xy(3));
        b.memorize();
        // an empty row chooses nothing: the choice stays
        assert_eq!(b.sent(), vec![format!("assign slot 1 id {}", second.unwrap_or(first))], "button {button}: second row");
    }
    // All: every ability, the corpse one included (header, 1, 2, 3, 4, 5, 6)
    b.tap(filter_xy(5));
    b.tap(row_xy(6));
    b.memorize();
    assert_eq!(b.sent(), vec!["assign slot 1 id 5"], "All lists the corpse-scope ability too");
    // a filter and a search together: Group + "two"
    b.tap(filter_xy(3));
    b.tap(SEARCH_XY);
    b.type_text("two");
    b.tap(row_xy(2));
    b.memorize();
    assert_eq!(b.sent(), vec!["assign slot 1 id 6"], "Group and 'two': Group Two");
    no_logs(&b);
}

/// 4. Icons: the list and the slots ask the game for the ability icon pictures (background and foreground) when the
/// window opens, and the window still draws when the game has not loaded the art (it has not, headless: the fallback
/// is a plain box). The art itself is the game's: a real picture is for the owner's test. Negative control: the old
/// window never calls `eqoa.icons.resource`, so no call is recorded.
#[test]
fn spellbook2_asks_for_the_ability_icons_and_draws_without_them() {
    // The headless host has no game art: `eqoa.icons.resource` answers with a refusal, and the host's diagnostics count
    // each call (one entry per symbol with a `count`), which is how a call is seen here.
    let mut b = Book::open(vec![
        {
            let mut a = named(1, "One", 1, 0, Some(1));
            a["icon_background"] = json!(111);
            a["icon_foreground"] = json!(222);
            a
        },
        {
            let mut a = named(2, "Two", 2, 0, None);
            a["icon_background"] = json!(333);
            a["icon_foreground"] = json!(444);
            a
        },
        named(3, "Three", 3, 0, None), // no icon fields
    ]);
    b.frames(9);
    let calls = |b: &Book| -> u64 {
        b.host
            .drain_logs()
            .unwrap()
            .iter()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter(|v| v["symbol"] == "eqoa.icons.resource")
            .map(|v| v["count"].as_u64().unwrap())
            .sum()
    };
    // The window asked the game for pictures when it opened. The host's diagnostics keep one entry per symbol and say
    // it once (and again each minute), so how many calls were made, and the pause before it asks again, are not
    // visible here: the throttle (`icons.due` in the mod) is for the owner's test and review.
    assert!(calls(&b) >= 1, "it asked for the ability icons");
    b.frames(2);
    assert_all_inside("icons not loaded", &b);
}

/// 5. Drag from the list onto a slot memorizes it there; onto a slot that is taken it replaces what was there; a
/// memorized ability dragged from the list moves; a press that does not travel is only a click, and a drop that is not
/// on a slot does nothing. Negative control: the old window ignores the release, so nothing is ever sent.
#[test]
fn spellbook2_dragging_from_the_list_onto_a_slot_memorizes_it_there() {
    let mut b = Book::open(vec![named(1, "One", 1, 0, Some(1)), named(2, "Two", 2, 0, None), named(3, "Three", 3, 0, None)]);
    // Two (row 3) onto the empty slot 5
    b.drag(row_xy(3), slot_xy(5));
    assert_eq!(b.plan(), vec!["assign slot 5 id 2"]);
    // Three (row 4) onto slot 1, which holds One: One is cleared first, then Three goes on
    b.drag(row_xy(4), slot_xy(1));
    assert_eq!(b.plan(), vec!["unmemorize slot 1", "assign slot 1 id 3"]);
    // One is memorized in slot 1 (the list row marks it): dragged to slot 4 it moves
    b.drag(row_xy(2), slot_xy(4));
    assert_eq!(b.plan(), vec!["unmemorize slot 1", "assign slot 4 id 1"]);
    // dropped back on its own slot: nothing; dropped on the list or off every slot: nothing
    b.drag(row_xy(2), slot_xy(1));
    b.drag(row_xy(3), row_xy(8));
    b.drag(row_xy(3), (0.0, 0.0));
    assert!(b.plan().is_empty(), "no drop on a slot, nothing sent");
    // a press that does not travel is a click: it chooses, sends nothing
    b.tap(row_xy(3));
    assert!(b.plan().is_empty());
    // the chosen one (Two) goes by the button, as before
    b.memorize();
    assert_eq!(b.plan(), vec!["assign slot 2 id 2"]);
    no_logs(&b);
}

/// 6. Drag a slot onto another slot: onto an empty one it moves (clear, then memorize), onto a taken one the two swap
/// (clear both, memorize each onto the other's slot), one request at a time and each only after the game confirmed the
/// one before. A refusal ends the move. Right-click clears a slot. Negative control: the old window has no drag.
#[test]
fn spellbook2_dragging_a_slot_moves_or_swaps_and_a_right_click_clears() {
    let mut b = Book::open(vec![
        named(1, "One", 1, 0, Some(1)),
        named(2, "Two", 2, 0, Some(2)),
        named(3, "Three", 3, 0, None),
    ]);
    // move: slot 1 onto the empty slot 6
    b.drag(slot_xy(1), slot_xy(6));
    assert_eq!(b.plan(), vec!["unmemorize slot 1", "assign slot 6 id 1"]);
    // swap: slot 1 onto slot 2
    b.drag(slot_xy(1), slot_xy(2));
    assert_eq!(
        b.plan(),
        vec!["unmemorize slot 1", "unmemorize slot 2", "assign slot 2 id 1", "assign slot 1 id 2"],
        "the game's own actions, in order"
    );
    // onto itself, or off every slot: nothing
    b.drag(slot_xy(1), slot_xy(1));
    b.drag(slot_xy(2), row_xy(5));
    assert!(b.plan().is_empty());
    // while a move is not finished a second drag waits (nothing new is queued)
    b.drag(slot_xy(1), slot_xy(6));
    let first = action_broker::queued_ability_steps_for_test();
    assert_eq!(first, vec!["unmemorize slot 1"], "only the first step is out");
    b.drag(slot_xy(2), slot_xy(7));
    assert_eq!(action_broker::queued_ability_steps_for_test(), first, "the second drag waited");
    // the first step is refused: the move ends there, nothing more is sent, and the next drag is accepted
    action_broker::refuse_all_for_test();
    native_events::dispatch(&b.host).unwrap();
    b.frames(2);
    assert!(action_broker::queued_ability_steps_for_test().is_empty(), "a refusal ends the move");
    b.drag(row_xy(4), slot_xy(9));
    assert_eq!(b.plan(), vec!["assign slot 9 id 3"], "and the window is ready for the next move");
    // right-click clears a slot that has an ability, and does nothing to an empty one
    b.tap_button(slot_xy(2), "right");
    assert_eq!(b.sent(), vec!["unmemorize slot 2"]);
    b.tap_button(slot_xy(12), "right");
    assert!(b.sent().is_empty(), "an empty slot has nothing to clear");
    // a click on a memorized slot only chooses it (the Cast and Unmemorize buttons then apply)
    b.tap(slot_xy(1));
    assert!(b.plan().is_empty());
    no_logs(&b);
}

/// Every vertex the layer drew must lie inside the window's own fill (as the owner-fix test does, for the new parts:
/// the search box with a full line of the widest letter, the buttons, the headers, the icons' frames).
fn assert_all_inside(label: &str, b: &Book) {
    let vertices = spellbook_vertices();
    let (w, h) = (332.0, 419.0);
    let fill: Vec<_> = vertices.iter().filter(|v| v.2 == 0xF511_1111).collect();
    assert!(fill.len() >= 4, "{label}: the window's surface was drawn");
    let lo = |f: fn(&&(f32, f32, u32)) -> f32| fill.iter().map(f).fold(f32::MAX, f32::min) as f64;
    let hi = |f: fn(&&(f32, f32, u32)) -> f32| fill.iter().map(f).fold(f32::MIN, f32::max) as f64;
    let (left, top, right, bottom) = (lo(|v| v.0), lo(|v| v.1), hi(|v| v.0), hi(|v| v.1));
    assert!((right - left - w * b.k).abs() < 2.0 && (bottom - top - h * b.k).abs() < 2.0, "{label}: window size");
    assert!(vertices.len() > 150, "{label}: drew ({} vertices)", vertices.len());
    for &(x, y, col) in vertices.iter().filter(|v| v.2 != 0xF511_1111 && v.2 & 0x00FF_FFFF != 0x0040_4040) {
        assert!(
            f64::from(x) >= left - 1.0 && f64::from(x) <= right + 1.0 && f64::from(y) >= top - 1.0 && f64::from(y) <= bottom + 1.0,
            "{label}: a vertex at ({x}, {y}) colour {col:#010x} is outside the window ({left}, {top})-({right}, {bottom})"
        );
    }
}

/// 7. At the default size nothing is cut off or drawn outside the window with the new parts in use: the search box full
/// of the widest letters, each scope button, a long list with headers and a scroll bar, memorized slots, and a drag
/// ghost's border (inside). The mod writes a debug note when it must cut a text: there is none.
#[test]
fn spellbook2_new_parts_fit_the_window_at_its_default_size() {
    let mut abilities: Vec<_> = (1..=30).map(|n| named(n, &format!("Wide Skill Number {n}"), n * 2 - 1, n % 4, (n <= 4).then_some(n))).collect();
    abilities[0]["name"] = json!("Anthem of Conquest and Valor Everlasting");
    let mut b = Book::open(abilities);
    assert_all_inside("opened", &b);
    for n in 1..=5 {
        b.tap(filter_xy(n));
        assert_all_inside(&format!("scope button {n}"), &b);
    }
    b.tap(SEARCH_XY);
    b.type_text("WWWWWWWWWWWWWWWWWWWWWWWW");
    assert_all_inside("the search box full of the widest letter, nothing matching", &b);
    for _ in 0..24 {
        b.backspace();
    }
    b.type_text("skill");
    assert_all_inside("a search with matches, focused", &b);
    b.tap(slot_xy(1));
    b.drag(row_xy(6), slot_xy(7));
    b.tap(slot_xy(2));
    assert_all_inside("after a drag and a choice", &b);
    let logs = format!("{:?}", b.host.drain_logs().unwrap());
    assert!(!logs.contains("cut a text"), "nothing was cut: {logs}");
}
