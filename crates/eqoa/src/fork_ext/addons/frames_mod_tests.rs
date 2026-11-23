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
            BuffLine { whose: 0, id: 9304, icon: 11, removable: true, name: "Dark Ward".into(), ends_tick: Some(1800), description: Some("Target, Increase armor by 30".into()) },
            BuffLine { whose: 1, id: 77, icon: 12, removable: false, name: "Plague".into(), ends_tick: None, description: None },
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

/// Whether a tooltip is up this frame: ImGui draws one in a window of its own whose name starts `##Tooltip`.
fn tooltip_up() -> bool {
    use dear_imgui_rs::sys;
    // SAFETY: the current context's window list is read, nothing is kept
    unsafe {
        let g = sys::igGetCurrentContext();
        (0..(*g).Windows.Size).any(|k| {
            let w = *(*g).Windows.Data.add(k as usize);
            (*w).Active && std::ffi::CStr::from_ptr((*w).Name).to_string_lossy().starts_with("##Tooltip")
        })
    }
}

/// An ImGui window's rectangle (x, y, width, height), by name.
fn window_rect(name: &std::ffi::CStr) -> [f32; 4] {
    use dear_imgui_rs::sys;
    // SAFETY: as `tooltip_up`
    unsafe {
        let w = sys::igFindWindowByName(name.as_ptr());
        assert!(!w.is_null(), "window {name:?} is drawn");
        [(*w).Pos.x, (*w).Pos.y, (*w).Size.x, (*w).Size.y]
    }
}

/// A player's left click at `at`: hover for two frames, press, release, as the manager's tests do.
fn click_at(host: &Host, bridge: &Bridge, context: &mut Context, at: [f32; 2]) {
    context.io_mut().add_mouse_pos_event(at);
    frames(host, bridge, context, 2);
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
    frames(host, bridge, context, 1);
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
    frames(host, bridge, context, 1);
}

/// Clicks down the Target window a few pixels at a time; says whether any click queued that action.
fn target_window_queues(host: &Host, bridge: &Bridge, context: &mut Context, action: &str) -> bool {
    let [x, y, _w, h] = window_rect(c"Target");
    let mut dy = 24.0;
    while dy < h - 2.0 {
        click_at(host, bridge, context, [x + 20.0, y + dy]);
        if action_broker::queued_kinds_for_test().contains(&action) {
            return true;
        }
        dy += 6.0;
    }
    false
}

/// Owner report 2026-11-23: "Invite to group" showed for every target, including monsters and yourself. It is now
/// drawn only for another player who is not already in your group.
///
/// Runs the mod against the fixture with `target_id`/`kind` and, with `member`, that id in the group, then says
/// whether any click down the Target window queued the invite.
fn invite_offered(target_id: u64, kind: &str, member: bool) -> bool {
    fixture();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.player["target_id"] = json!(target_id);
        c.view.entities[0]["id"] = json!(target_id);
        c.view.entities[0]["kind"] = json!(kind);
        c.view.group = json!({
            "in_group": member, "self_leader": null,
            "members": if member { json!([{"id": 8193, "name": "Mira", "active": true}]) } else { json!([]) },
            "raid": [], "invites": [],
        });
    });
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    assert!(frames(&host, &bridge, &mut context, 2) > 0);
    let offered = target_window_queues(&host, &bridge, &mut context, "group_invite");
    assert!(host.drain_logs().unwrap().is_empty());
    offered
}

#[test]
fn a3_frames_mod_offers_invite_only_for_another_player_outside_the_group() {
    // control: another player outside the group gets the button, and clicking it queues the invite
    assert!(invite_offered(8193, "player", false), "a player outside the group can be invited");
    // and these do not: a monster, yourself, and someone already in your group
    assert!(!invite_offered(8193, "npc", false), "a monster is not offered the Invite button");
    assert!(!invite_offered(256, "player", false), "yourself is not offered the Invite button");
    assert!(!invite_offered(8193, "player", true), "a group member is not offered the Invite button");
}

/// Hovers `at` for a few frames and says whether a tooltip is up.
fn tip_at(host: &Host, bridge: &Bridge, context: &mut Context, at: [f32; 2]) -> bool {
    context.io_mut().add_mouse_pos_event(at);
    frames(host, bridge, context, 3);
    tooltip_up()
}

#[test]
fn a3_frames_mod_shows_an_effects_tip_only_while_the_pointer_is_on_an_effect() {
    fixture();
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    let buffs: String = host
        .lua
        .load("local b, e = eqoa.effects.buffs(); return b and (#b .. ' removable=' .. tostring(b[1].removable)) or ('none ' .. tostring(e and e.code))")
        .eval()
        .unwrap();
    assert!(buffs.starts_with("2 removable=true"), "the fixture gives the mod two effects: {buffs}");
    // the pointer is nowhere: no tip, though the first effect can be taken off (the owner's report: the tip was always
    // over the cursor while the window was up, and over another mod's window)
    frames(&host, &bridge, &mut context, 4);
    assert!(!tooltip_up(), "no tip with the pointer away");
    let [x, y, w, h] = window_rect(c"Effects");
    // the pointer over a different window of this mod (Casting, Target): still no tip
    for name in [c"Casting", c"Target"] {
        let [ox, oy, ow, oh] = window_rect(name);
        assert!(ox + ow > x + w || oy > y + h || ox > x + w, "{name:?} is a window of its own");
        assert!(!tip_at(&host, &bridge, &mut context, [ox + ow / 2.0, oy + oh / 2.0]), "no tip over {name:?}");
    }
    // the pointer over the Effects window's lines: a tip over the effect rows
    let mut found = None;
    let mut dy = 22.0;
    while dy < h && found.is_none() {
        if tip_at(&host, &bridge, &mut context, [x + 25.0, y + dy]) {
            found = Some(dy);
        }
        dy += 6.0;
    }
    let diag = unsafe {
        use dear_imgui_rs::sys;
        let g = sys::igGetCurrentContext();
        let hw = (*g).HoveredWindow;
        let name = if hw.is_null() { "none".to_string() } else { std::ffi::CStr::from_ptr((*hw).Name).to_string_lossy().into_owned() };
        format!("mouse {:?} hovered window {name} hoveredId {} wantCaptureMouse {}", (*g).IO.MousePos, (*g).HoveredId, (*g).IO.WantCaptureMouse)
    };
    assert!(found.is_some(), "a tip shows while the pointer is on an effect (window {x},{y} {w}x{h}); {diag}");
    // and it goes when the pointer leaves
    assert!(!tip_at(&host, &bridge, &mut context, [900.0, 700.0]), "the tip goes with the pointer");
    assert!(host.drain_logs().unwrap().is_empty());
}
