// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A3: the bundled Frames demo mod ships off, declares only what it uses, loads and draws without a
//! diagnostic on protocol-shaped state, and does nothing at all until it is enabled.
use super::{
    action_broker,
    bridge::Bridge,
    discovery::{DiscoveredAddon, EntryMode},
    host::Host,
    native_a3::{self, BuffLine, PetLine},
    native_core::{CORE, Core},
    native_events,
};
use dear_imgui_rs::Context;
use serde_json::json;
use std::path::PathBuf;

fn root() -> PathBuf {
    // as discovery makes it: the real path, so the entry is inside the script root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bundled-mods/eqoa_frames")
        .canonicalize()
        .unwrap()
}

fn manifest() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root().join("mod.json")).unwrap()).unwrap()
}

fn addon() -> DiscoveredAddon {
    let capabilities: Vec<String> = manifest()["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    DiscoveredAddon {
        id: "eqoa_frames".into(),
        name: "EQOA Frames (demo)".into(),
        entry: root().join("main.lua"),
        script_root: root(),
        package_root: root(),
        module_dirs: vec![root()],
        resource_dirs: vec![root()],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: capabilities,
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

/// The world as the capture would leave it: the player has a pet and a target, and the A3 state holds effects,
/// a pet effect and a cast.
fn fixture() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    action_broker::reset_all_for_test();
    native_a3::reset_for_test();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = json!({
            "id": 256, "name": "Me", "level": 20, "has_pet": true, "pet_id": 12289, "target_id": 8193,
            "hp": {"current": 90, "maximum": 100, "base": 100},
        });
        c.view.entities = vec![json!({
            "ref": {"$ref": "entity", "key": "8193", "generation": 0, "incarnation": 1},
            "id": 8193, "name": "Mira", "level": 22, "kind": "player", "is_creature": true,
            "position": {"x": 1.0, "y": 0.0, "z": 1.0}, "actor_position": {"x": 1.0, "y": 0.0, "z": 1.0},
        })];
    });
    native_a3::sync_buffs(true, 5, || {
        vec![
            BuffLine { whose: 0, id: 9304, icon: 11, removable: true, name: "Dark Ward".into(), ends_tick: Some(1800) },
            BuffLine { whose: 1, id: 77, icon: 12, removable: false, name: "Plague".into(), ends_tick: None },
        ]
    });
    native_a3::sync_pet(true, 5, || vec![PetLine { icon: 21, name: "Rage".into(), ends_tick: 900 }]);
    native_a3::set_target(1, || json!({"id": 8193, "title": "Warlord", "guild": "Iron Tide", "consider": {
        "code": 3, "name": "blue", "color": {"r": 0.0, "g": 0.0, "b": 1.0, "a": 1.0}}}));
    native_a3::cast_started(256, 8193, vec![], 0xAA, 2500, true);
    native_a3::cast_started(8193, 256, vec![], 0xBB, 4000, false);
}

fn frames(host: &Host, bridge: &Bridge, context: &mut Context, n: usize) -> usize {
    let mut vertices = 0;
    for _ in 0..n {
        context.frame();
        // the events of the frame, then the `render` event a once-mode mod draws in
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

#[test]
fn a3_frames_mod_ships_off_and_declares_exactly_the_actions_it_calls() {
    let manifest = manifest();
    assert_eq!(manifest["enabled"], false, "bundled demo mods start off");
    assert_eq!(manifest["id"], "eqoa_frames");
    let src = std::fs::read_to_string(root().join("main.lua")).unwrap();
    let mut used = std::collections::BTreeSet::new();
    let mut rest = src.as_str();
    while let Some(i) = rest.find("eqoa.actions.") {
        rest = &rest[i + "eqoa.actions.".len()..];
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        used.insert(name);
    }
    let declared: std::collections::BTreeSet<String> = manifest["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c.as_str()?.strip_prefix("eqoa.actions.").map(str::to_string))
        .collect();
    assert!(!declared.contains("*"), "no blanket eqoa.actions.*");
    assert_eq!(declared, used, "declared actions must equal the actions the mod calls");
    // Mods & Addons tells the player what it can do on its own, by the names it declares
    let caps: Vec<String> = manifest["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
    assert_eq!(
        super::runtime::autonomous_action_label(&caps).as_deref(),
        Some("Actions: can act on its own (remove_effect, group_invite, pet_attack, pet_back_off)")
    );
    // control: the same mod without its action capabilities shows no such line
    let reads: Vec<String> = caps.iter().filter(|c| !c.starts_with("eqoa.actions.")).cloned().collect();
    assert_eq!(super::runtime::autonomous_action_label(&reads), None);
    assert!(crate::fork_ext::bundled::PACKAGES.contains(&"eqoa_frames"));
    for file in ["mod.json", "main.lua", "README.md"] {
        assert!(
            crate::fork_ext::bundled::EMBEDDED_FILES.iter().any(|f| f.path == format!("eqoa_frames/{file}")),
            "{file} ships with the client"
        );
    }
}

#[test]
fn a3_frames_mod_loads_draws_and_raises_no_diagnostic_and_a_mod_that_is_not_run_draws_nothing() {
    fixture();
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    // control: the mod is off (its entry has not run): the frame has no window and draws nothing
    assert_eq!(frames(&host, &bridge, &mut context, 3), 0, "nothing is drawn while the mod is not enabled");

    host.run_addon(&addon()).unwrap();
    let vertices = frames(&host, &bridge, &mut context, 4);
    let logs = host.drain_logs().unwrap();
    let probe: String = host
        .lua
        .load("local p, e = eqoa.player.get(); local t, e2 = eqoa.target.get(); return tostring(p) .. ' ' .. tostring(e and e.code) .. ' / ' .. tostring(t) .. ' ' .. tostring(e2 and e2.code)")
        .eval()
        .unwrap();
    assert!(vertices > 0, "the Effects, Casting and Target windows are drawn: logs {logs:?}, state {probe}");
    assert!(logs.is_empty(), "no diagnostic, no log: {logs:?}");
}

#[test]
fn a3_frames_mod_draws_a_target_whose_consider_code_the_client_does_not_know() {
    fixture();
    // the server's second byte is outside the six colours: `consider` has a code and no name or colour
    native_a3::set_target(0xA3, || json!({"id": 8193, "title": null, "guild": null, "consider": {"code": 9}}));
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    assert!(frames(&host, &bridge, &mut context, 3) > 0);
    let logs = host.drain_logs().unwrap();
    assert!(logs.is_empty(), "the Target window did not fail: {logs:?}");
    // control: the colour is there for a known code and the same mod draws it
    native_a3::set_target(0xA4, || json!({"id": 8193, "consider": {"code": 3, "name": "blue",
        "color": {"r": 0.0, "g": 0.0, "b": 1.0, "a": 1.0}}}));
    assert!(frames(&host, &bridge, &mut context, 2) > 0);
    assert!(host.drain_logs().unwrap().is_empty());
}

#[test]
fn a3_frames_mod_keeps_drawing_when_the_server_sends_no_buff_window_and_when_nothing_is_targeted() {
    fixture();
    // no buff window, no pet strip, no target, no pet
    native_a3::sync_buffs(false, 0, Vec::new);
    native_a3::sync_pet(false, 0, Vec::new);
    native_a3::set_target(2, || serde_json::Value::Null);
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.player["target_id"] = serde_json::Value::Null;
        c.view.player["has_pet"] = json!(false);
    });
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    assert!(frames(&host, &bridge, &mut context, 3) > 0);
    let logs = host.drain_logs().unwrap();
    assert!(logs.is_empty(), "{logs:?}");
    // and outside the world it draws nothing and says nothing
    CORE.with(|c| c.borrow_mut().view.in_world = false);
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0);
    assert!(host.drain_logs().unwrap().is_empty());
}
