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
    native_events, window_sessions as ws, window_takeover,
};
use dear_imgui_rs::Context;
use serde_json::json;
use std::path::PathBuf;

pub(super) fn root(id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(id).canonicalize().unwrap()
}

pub(super) fn manifest(id: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root(id).join("mod.json")).unwrap()).unwrap()
}

pub(super) fn addon(id: &str) -> DiscoveredAddon {
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

pub(super) fn context() -> Context {
    let mut context = Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_display_size([1024.0, 768.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.font_atlas().try_claim_legacy_renderer().unwrap().build();
    context
}

pub(super) fn fixture() {
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

pub(super) fn frames(host: &Host, bridge: &Bridge, context: &mut Context, n: usize) -> usize {
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

pub(super) fn press(host: &Host, key: &str, combo: &str) {
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
    let mut src = std::fs::read_to_string(root(id).join("main.lua")).unwrap();
    // a mod may keep its code in modules (round A5: the Character mod): what they call counts as the mod's
    if let Ok(dir) = std::fs::read_dir(root(id).join("modules")) {
        for entry in dir.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "lua")) {
            src.push('
');
            src.push_str(&std::fs::read_to_string(entry.path()).unwrap());
        }
    }
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
        // `confirm` only reads whether the client's own question is up (round A5: a mod leaves Escape to it)
        assert!(matches!(name.as_str(), "open_session" | "close_session" | "confirm"), "{id} uses eqoa.windows.{name}");
        assert!(declared.contains(&"eqoa.windows.session".to_string()), "{id} must declare eqoa.windows.session");
    }
}

pub(super) fn run(id: &str, key: &str, combo: &str, title_kind: &str) {
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
    }
}

pub(super) fn click(host: &Host, x: f64, y: f64, button: &str) {
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": button})));
    native_events::dispatch(host).unwrap();
}

fn ability(n: i64, slot: Option<i64>) -> serde_json::Value {
    json!({"ref": {"$ref": "ability", "key": n.to_string(), "generation": 0, "incarnation": 1}, "id": n,
        "name": format!("Skill {n:02}"), "description": "", "display_key": n, "spellbook_slot": slot, "usable": true,
        "level": n, "range": 5.0, "cast_time": 1.0, "power_cost": 3, "scope": 0, "cooldown": 0.0, "equipment_mask": 0, "lockout": 0.0})
}

/// Owner test 2026-11-23: the Spellbook's scroll bar takes a click (it only drew before) and Memorize is a button,
/// with the game's refusal (an ability already memorized) said and nothing sent. The window's geometry is the
/// mod's (HUD pixels: list at y 24, 13 rows of 13; slots from y 201; buttons at y 342).
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
    let (w, h) = (332.0, 370.0);
    let x0 = (1024.0 - w * k).min(ox + (640.0 - w) / 2.0 * k).max(0.0);
    let y0 = (768.0 - h * k).min(oy + (448.0 - h) / 2.0 * k).max(0.0);
    let at = |x: f64, y: f64| (x0 + x * k, y0 + y * k);
    let mut tap = |x: f64, y: f64, button: &str| {
        let (px, py) = at(x, y);
        click(&host, px, py, button);
        frames(&host, &bridge, &mut context, 2);
    };
    // the first row is Skill 01, memorized in slot 1: chosen, Memorize is refused (no button) and so is a slot click
    tap(60.0, 30.0, "left");
    tap(14.0, 350.0, "left");
    tap(8.0 + 2.0 * 32.0 + 5.0, 201.0 + 5.0, "left"); // slot 3
    assert!(action_broker::queued_kinds_for_test().is_empty(), "a memorized ability is not memorized again");
    // a click near the foot of the scroll bar (x 325..331, y 24..193) jumps the list down: Skill 08 is first now
    tap(328.0, 24.0 + 169.0 * 0.95, "left");
    tap(60.0, 30.0, "left");
    tap(14.0, 350.0, "left"); // the Memorize button
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["assign_ability"], "the scrolled-to ability is memorized");
    assert_eq!(host.drain_logs().unwrap().len(), 0);
}
