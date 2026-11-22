// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1: the two new bundled mod windows, Spellbook and Quest Journal. Each ships off, declares exactly what
//! it calls, opens on its key into a session, draws without a diagnostic, closes on the key again, on Escape, and
//! when the client ends its session (zoning, death, the player's switch), and draws nothing once closed.
use super::{
    action_broker,
    bridge::Bridge,
    discovery::{DiscoveredAddon, EntryMode},
    host::Host,
    native_core::{CORE, Core},
    native_events, window_sessions as ws, window_takeover,
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
        module_dirs: vec![root(id)],
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
