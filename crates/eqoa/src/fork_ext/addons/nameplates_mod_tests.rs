// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The bundled Name Plates mod (round A12's example): ships off, loads without a diagnostic, puts its plates at
//! `eqoa.camera.project_entity`'s point, holds back the game's plates and numbers only while it is on, honours its
//! distance limit, and costs little a frame.
use super::{
    action_broker,
    bridge::Bridge,
    discovery::{DiscoveredAddon, EntryMode},
    health::{HealthBarClaimsRegistry, HealthSnapshot},
    host::Host,
    native_a12, native_a3,
    native_core::{CORE, Core},
    native_events,
    visibility::{self, HudClaimsRegistry, VisibilityLease, publish_claims},
};
use dear_imgui_rs::Context;
use glam::{Mat4, Vec3};
use serde_json::{Value as Json, json};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

const SIZE: (u32, u32) = (1024, 768);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods/eqoa_nameplates").canonicalize().unwrap()
}

fn manifest() -> Json {
    serde_json::from_str(&std::fs::read_to_string(root().join("mod.json")).unwrap()).unwrap()
}

fn addon() -> DiscoveredAddon {
    let m = manifest();
    let strings = |key: &str| -> Vec<String> { m[key].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect() };
    let mut extensions: super::discovery::Extensions = Default::default();
    extensions.replaces = strings("replaces");
    DiscoveredAddon {
        id: "eqoa_nameplates".into(),
        name: "EQOA Name Plates".into(),
        entry: root().join("main.lua"),
        script_root: root(),
        package_root: root(),
        module_dirs: vec![root()],
        resource_dirs: vec![root()],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: strings("capabilities"),
        extensions,
        initially_enabled: false,
        mods_root: None,
    }
}

fn context() -> Context {
    let mut context = Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_display_size([SIZE.0 as f32, SIZE.1 as f32]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.font_atlas().try_claim_legacy_renderer().unwrap().build();
    context
}

/// The camera at the origin looking down -z, 90 degrees across the surface.
fn view() -> Mat4 {
    let fov_y = 2.0 * (0.5f32 * SIZE.1 as f32 / (SIZE.0 as f32 / 2.0)).atan();
    glam::camera::rh::proj::directx::perspective(fov_y, SIZE.0 as f32 / SIZE.1 as f32, 0.3, 6000.0)
        * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y)
}

fn entity(id: u32, name: &str, kind: &str, at: [f32; 3]) -> Json {
    json!({
        "ref": {"$ref": "entity", "key": id.to_string(), "generation": 0, "incarnation": 1},
        "id": id, "name": name, "level": 20 + (id % 7), "kind": kind, "is_creature": true, "corpse": false,
        "position": {"x": at[0], "y": at[1], "z": at[2]}, "actor_position": {"x": at[0], "y": at[1], "z": at[2]},
        "hp_fraction": 0.5 + (id % 5) as f64 / 10.0, "npc_flags": if kind == "npc" { Json::from(0) } else { Json::Null },
    })
}

/// A world with the player at the origin and `list` around; each one's plate point is 2 above its feet.
fn world(list: Vec<Json>) {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    action_broker::reset_all_for_test();
    native_a3::reset_for_test();
    let mut points = Vec::new();
    for e in &list {
        let p = &e["actor_position"];
        points.push((e["id"].as_u64().unwrap() as u32, [p["x"].as_f64().unwrap() as f32, p["y"].as_f64().unwrap() as f32 + 2.0, p["z"].as_f64().unwrap() as f32]));
    }
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = json!({
            "id": 256, "name": "Me", "level": 20, "target_id": null, "position": {"x": 0.0, "y": 0.0, "z": 0.0},
            "hp": {"current": 90, "maximum": 100, "base": 100},
        });
        c.view.entities = list;
    });
    native_a12::set_plates(points);
    native_a12::set_view(view(), SIZE);
}

/// One drawn frame: the events, the mod's `render`, then the background list's vertices.
fn frame(host: &Host, bridge: &Bridge, context: &mut Context) -> Vec<[f32; 2]> {
    context.frame();
    bridge.invoke(|| {
        native_events::dispatch(host)?;
        native_events::render(host)
    })
    .unwrap();
    let draw = context.render_legacy();
    let mut out = Vec::new();
    for list in draw.draw_data().draw_lists() {
        out.extend(list.vtx_buffer().iter().map(|v| v.pos));
    }
    out
}

struct Hud {
    claims: Rc<RefCell<HudClaimsRegistry>>,
    lease: VisibilityLease,
}

fn attach(host: &Host) -> Hud {
    let claims = Rc::new(RefCell::new(HudClaimsRegistry::new()));
    let lease = VisibilityLease::with_hud_claims(Rc::new(RefCell::new(HealthBarClaimsRegistry::new())), claims.clone()).unwrap();
    visibility::install_managed_health_adapter(host, &lease).unwrap();
    lease.update(HealthSnapshot::new(true, 100, 100), true);
    Hud { claims, lease }
}

fn flags() -> (bool, bool, bool) {
    (crate::hud::fork_hud::plates_hidden(), crate::hud::fork_hud::damage_hidden(), crate::hud::fork_hud::rings_hidden())
}

fn bounds(v: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    let mut lo = [f32::MAX; 2];
    let mut hi = [f32::MIN; 2];
    for p in v {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

#[test]
fn nameplates_ships_off_declares_what_it_uses_and_says_what_it_replaces() {
    let m = manifest();
    assert_eq!(m["enabled"], false);
    assert_eq!(m["id"], "eqoa_nameplates");
    assert!(crate::fork_ext::bundled::PACKAGES.contains(&"eqoa_nameplates"));
    for file in ["mod.json", "main.lua", "README.md"] {
        assert!(crate::fork_ext::bundled::EMBEDDED_FILES.iter().any(|f| f.path == format!("eqoa_nameplates/{file}")), "{file}");
    }
    // LF line endings, as the manifest tool and the mutation tests need
    for file in ["mod.json", "main.lua", "README.md"] {
        assert!(!std::fs::read(root().join(file)).unwrap().contains(&b'\r'), "{file} has CRLF");
    }
    let lines = super::runtime::disclosure_lines(&addon());
    assert!(lines.iter().any(|l| l == "Replaces HUD: Name plates, Damage numbers"), "{lines:?}");
    // the rings are only held when the player ticks the option: not disclosed as always replaced
    assert!(!lines.iter().any(|l| l.contains("Target rings")), "{lines:?}");
}

#[test]
fn nameplates_loads_draws_with_no_diagnostic_and_a_mod_not_run_draws_nothing() {
    world(vec![entity(7001, "Gnoll Pup", "npc", [1.0, 0.0, -10.0])]);
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    // control: the entry has not run
    assert!(frame(&host, &bridge, &mut context).is_empty());
    host.run_addon(&addon()).unwrap();
    let v = frame(&host, &bridge, &mut context);
    let logs = host.drain_logs().unwrap();
    assert!(!v.is_empty(), "a plate is drawn: {logs:?}");
    assert!(logs.is_empty(), "no diagnostic: {logs:?}");
}

#[test]
fn nameplates_plate_sits_at_project_entitys_point_and_follows_the_view() {
    world(vec![entity(7001, "Gnoll Pup", "npc", [2.0, 0.0, -10.0])]);
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    let want: (f64, f64) = host
        .lua
        .load("local p = assert(eqoa.camera.project_entity(7001, 0.25)); return p.x, p.y")
        .eval()
        .unwrap();
    let (lo, hi) = bounds(&frame(&host, &bridge, &mut context));
    let centre = (lo[0] + hi[0]) / 2.0;
    assert!((centre as f64 - want.0).abs() < 8.0, "plate centred on x {} not {}", centre, want.0);
    // the health bar is the plate's lowest part: its bottom edge is the projected point
    assert!((hi[1] as f64 - want.1).abs() < 2.0, "plate bottom {} not {}", hi[1], want.1);
    // the camera turns: the plate follows in the very next frame
    native_a12::set_view(
        {
            let fov_y = 2.0 * (0.5f32 * SIZE.1 as f32 / (SIZE.0 as f32 / 2.0)).atan();
            glam::camera::rh::proj::directx::perspective(fov_y, SIZE.0 as f32 / SIZE.1 as f32, 0.3, 6000.0)
                * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, Vec3::new(0.1, 0.0, -1.0).normalize(), Vec3::Y)
        },
        SIZE,
    );
    let (lo2, hi2) = bounds(&frame(&host, &bridge, &mut context));
    assert!(((lo2[0] + hi2[0]) / 2.0) < centre - 50.0, "the plate moved left with the turned view");
}

#[test]
fn nameplates_hides_the_games_plates_and_numbers_only_while_on_and_rings_only_when_asked() {
    world(vec![entity(7001, "Gnoll Pup", "npc", [1.0, 0.0, -10.0])]);
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let hud = attach(&host);
    // control: not run, the game's own are drawn
    frame(&host, &bridge, &mut context);
    publish_claims(&hud.claims.borrow());
    assert_eq!(flags(), (false, false, false));
    host.run_addon(&addon()).unwrap();
    frame(&host, &bridge, &mut context);
    publish_claims(&hud.claims.borrow());
    assert_eq!(flags(), (true, true, false), "plates and numbers held, the target rings stay");
    // the option
    set_setting(&host, "hide_rings", "true");
    frame(&host, &bridge, &mut context);
    publish_claims(&hud.claims.borrow());
    assert_eq!(flags(), (true, true, true), "the ticked option holds the rings too");
    set_setting(&host, "hide_rings", "false");
    frame(&host, &bridge, &mut context);
    publish_claims(&hud.claims.borrow());
    assert_eq!(flags(), (true, true, false), "and gives them back when untick");
    // the mod goes off (the manager clears its lease): the game's are back at once
    hud.lease.clear();
    publish_claims(&hud.claims.borrow());
    assert_eq!(flags(), (false, false, false));
}

/// The mod keeps its options in the global table `nameplates_state` (what its settings window edits).
fn set_setting(host: &Host, key: &str, value: &str) {
    host.lua.load(format!("nameplates_state.{key} = {value}")).exec().unwrap();
}

#[test]
fn nameplates_distance_limit_culls_before_projecting() {
    // one inside the default 40 units, one outside; and the same far one when it is moved in (control)
    world(vec![entity(7001, "Near One", "npc", [1.0, 0.0, -10.0]), entity(7002, "Far One", "npc", [-1.0, 0.0, -80.0])]);
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    let (lo, hi) = bounds(&frame(&host, &bridge, &mut context));
    // only the near one: the plate is on the right half (x > 512); the far one would sit just left of centre
    assert!(lo[0] > 400.0 && hi[0] > 512.0, "{lo:?} {hi:?}");
    let far_only = bounds(&{
        world(vec![entity(7002, "Far One", "npc", [-1.0, 0.0, -80.0])]);
        frame(&host, &bridge, &mut context)
    });
    assert!(far_only.0[0] > far_only.1[0], "a creature beyond the limit draws nothing");
    world(vec![entity(7002, "Far One", "npc", [-1.0, 0.0, -30.0])]);
    assert!(!frame(&host, &bridge, &mut context).is_empty(), "control: the same creature inside the limit is drawn");
}

#[test]
fn nameplates_kinds_options_and_long_names() {
    let long = "A Very Long Creature Name That Would Run Past The Screen Edge Entirely";
    world(vec![entity(7001, long, "npc", [0.0, 0.0, -10.0])]);
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    let (lo, hi) = bounds(&frame(&host, &bridge, &mut context));
    assert!(hi[0] - lo[0] < 260.0, "a long name is cut, not drawn at full length: {}", hi[0] - lo[0]);
    // creatures off: the same entity is not drawn; on again: it is (control)
    set_setting(&host, "creatures", "false");
    assert!(frame(&host, &bridge, &mut context).is_empty());
    set_setting(&host, "creatures", "true");
    assert!(!frame(&host, &bridge, &mut context).is_empty());
    // players off draws no player; a player is drawn with it on
    world(vec![entity(7003, "Mira", "player", [0.0, 0.0, -10.0])]);
    set_setting(&host, "players", "false");
    assert!(frame(&host, &bridge, &mut context).is_empty());
    set_setting(&host, "players", "true");
    assert!(!frame(&host, &bridge, &mut context).is_empty());
}

#[test]
fn nameplates_draws_quest_marks_and_merc_tags_stacked_above_the_name() {
    let plain = entity(7001, "Gnoll Pup", "npc", [0.0, 0.0, -10.0]);
    let mut marked = plain.clone();
    marked["head_mark"] = json!("!");
    marked["merc_tag"] = json!("<Lonewulf's Mercenary>");
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    world(vec![plain.clone()]);
    let (_, hi_plain) = bounds(&frame(&host, &bridge, &mut context));
    let (lo_p, _) = bounds(&frame(&host, &bridge, &mut context));
    world(vec![marked]);
    let (lo_m, hi_m) = bounds(&frame(&host, &bridge, &mut context));
    assert!(lo_m[1] < lo_p[1] - 10.0, "the mark and the tag stack above the name");
    assert!((hi_m[1] - hi_plain[1]).abs() < 1.0, "and the bar stays at the point");
}

/// Text really grows with Size: the same plate at 100 and 200 percent, with the font service installed as the
/// client has it (the mod must not depend on it).
#[test]
fn nameplates_size_option_really_scales_the_text() {
    use super::fonts::{FontBank, current_context_epoch};
    let mut context = context();
    let addon = addon();
    let host = Host::for_addon(&addon).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let fonts = Rc::new(RefCell::new(FontBank::new(0, current_context_epoch())));
    FontBank::install_for_addon(&fonts, &host.lua, &addon).unwrap();
    host.run_addon(&addon).unwrap();
    world(vec![entity(7001, "Gnoll Pup", "npc", [0.0, 0.0, -10.0])]);
    let mut run = |size: u32| {
        set_setting(&host, "size", &size.to_string());
        fonts.borrow_mut().flush_pending(&mut context);
        let v = frame(&host, &bridge, &mut context);
        bounds(&v)
    };
    let (lo1, hi1) = run(100);
    let (lo2, hi2) = run(200);
    let ratio = (hi2[0] - lo2[0]) / (hi1[0] - lo1[0]);
    assert!(ratio > 1.6, "text and plate roughly double with Size 200: {ratio}");
    assert!((hi2[1] - lo2[1]) > (hi1[1] - lo1[1]) * 1.5, "and so does its height");
    assert!(host.drain_logs().unwrap().is_empty());
}

#[test]
fn nameplates_damage_numbers_rise_from_what_was_hit_and_are_red_when_you_are_hit() {
    world(vec![entity(7001, "Gnoll Pup", "npc", [1.0, 0.0, -10.0])]);
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    let base = frame(&host, &bridge, &mut context).len();
    let hit = |attacker: u32, defender: u32, amount: u32| {
        native_core_emit(json!({"attacker_id": attacker, "defender_id": defender, "amount": amount, "kind": "damage", "color": null,
            "outgoing": attacker == 256, "incoming": defender == 256, "from_pet": false, "to_pet": false}));
    };
    hit(256, 7001, 12);
    let with_number = frame(&host, &bridge, &mut context).len();
    assert!(with_number > base, "an outgoing hit adds a number ({with_number} vs {base}): {:?}", host.drain_logs().unwrap());
    // someone else's hit on someone else is not drawn (control)
    hit(7001, 7002, 99);
    assert_eq!(frame(&host, &bridge, &mut context).len(), with_number);
    // it floats up: a later frame has it higher on the screen than at the start
    let top_now = bounds(&frame(&host, &bridge, &mut context)).0[1];
    std::thread::sleep(std::time::Duration::from_millis(150));
    let top_later = bounds(&frame(&host, &bridge, &mut context)).0[1];
    assert!(top_later < top_now, "the number rises ({top_now} to {top_later})");
}

fn native_core_emit(data: Json) {
    super::native_core::emit("combat.hit", data);
}

#[test]
fn nameplates_cost_per_frame_with_fifty_entities() {
    let mut list = Vec::new();
    for i in 0..50u32 {
        let (x, z) = (((i % 10) as f32 - 4.5) * 2.0, -8.0 - (i / 10) as f32 * 4.0);
        list.push(entity(8000 + i, &format!("Creature {i}"), if i % 5 == 0 { "player" } else { "npc" }, [x, 0.0, z]));
    }
    world(list);
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    for _ in 0..10 {
        frame(&host, &bridge, &mut context);
    }
    let vertices = frame(&host, &bridge, &mut context).len();
    assert!(vertices > 50 * 20, "fifty plates are drawn ({vertices} vertices)");
    let frames = 100;
    let time = |host: &Host, context: &mut Context, bridge: &Bridge| {
        let start = std::time::Instant::now();
        for _ in 0..frames {
            context.frame();
            bridge.invoke(|| native_events::render(host)).unwrap();
            let _ = context.render_legacy();
        }
        start.elapsed() / frames
    };
    let with_mod = time(&host, &mut context, &bridge);
    // the same frame with the mod not run: the ImGui frame and the event pass alone
    let idle = Host::for_addon(&addon()).unwrap();
    let idle_bridge = Bridge::install(&idle).unwrap();
    let baseline = time(&idle, &mut context, &idle_bridge);
    // what the mod's frame is made of
    let part = |code: &str| {
        let f = host.compile(code).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..frames {
            host.run(&f).unwrap();
        }
        start.elapsed() / frames
    };
    let list = part("local l = eqoa.entities.list(); for i = 1, #l do local e = l[i]; local _ = e.id, e.name, e.kind, e.corpse, e.npc_flags, e.actor_position.x, e.actor_position.y, e.actor_position.z end");
    let project = part("for i = 0, 49 do assert(eqoa.camera.project_entity(8000 + i, 0.25)) end");
    println!(
        "NAMEPLATES 50 entities in range ({} build): frame with the mod {with_mod:?}, without {baseline:?}, the mod itself {:?}; of it, reading the list and 8 fields each {list:?}, 50 project_entity calls {project:?}",
        if cfg!(debug_assertions) { "debug" } else { "release" },
        with_mod.saturating_sub(baseline)
    );
    assert!(with_mod.saturating_sub(baseline) < std::time::Duration::from_millis(60), "{with_mod:?} vs {baseline:?}");
    assert!(host.drain_logs().unwrap().is_empty());
}

// ---- fix round ----------------------------------------------------------------------------------------------------

fn hit(attacker: u32, defender: u32, amount: u32) {
    super::native_core::emit(
        "combat.hit",
        json!({"attacker_id": attacker, "defender_id": defender, "amount": amount, "kind": "damage", "color": null,
            "outgoing": attacker == 256, "incoming": defender == 256, "from_pet": false, "to_pet": false}),
    );
}

/// The numbers alone: plates off, one creature in front to hit.
fn numbers_only() -> (Host, Bridge, Context) {
    world(vec![entity(7001, "Gnoll Pup", "npc", [0.0, 0.0, -10.0])]);
    let context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    set_setting(&host, "creatures", "false");
    (host, bridge, context)
}

fn vertex_colors(context: &mut Context, host: &Host, bridge: &Bridge) -> Vec<u32> {
    context.frame();
    bridge.invoke(|| { native_events::dispatch(host)?; native_events::render(host) }).unwrap();
    let draw = context.render_legacy();
    let mut out = Vec::new();
    for list in draw.draw_data().draw_lists() {
        out.extend(list.vtx_buffer().iter().map(|v| v.col));
    }
    out
}

fn rgb(c: u32) -> [u32; 3] {
    [c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff]
}

fn height_of(v: &[[f32; 2]]) -> f32 {
    let (lo, hi) = bounds(v);
    hi[1] - lo[1]
}

#[test]
fn nameplates_a_hit_on_you_is_red_yours_is_white_and_a_big_hit_is_bigger() {
    // one ImGui context at a time: each part has its own scope
    {
        let (host, bridge, mut context) = numbers_only();
        assert!(frame(&host, &bridge, &mut context).is_empty(), "control: no hit, nothing drawn");
        // yours: white glyphs
        hit(256, 7001, 12);
        let cols = vertex_colors(&mut context, &host, &bridge);
        assert!(cols.iter().any(|c| rgb(*c) == [255, 255, 255]), "a white number");
        assert!(!cols.iter().any(|c| { let [r, g, b] = rgb(*c); r > 200 && g < 120 && b < 120 }), "and nothing red");
    }
    {
        // theirs on you (incoming): red, and not white
        let (host, bridge, mut context) = numbers_only();
        // the camera behind you, as the game's third-person view has it (at the eye itself you are not in view)
        let fov_y = 2.0 * (0.5f32 * SIZE.1 as f32 / (SIZE.0 as f32 / 2.0)).atan();
        native_a12::set_view(
            glam::camera::rh::proj::directx::perspective(fov_y, SIZE.0 as f32 / SIZE.1 as f32, 0.3, 6000.0)
                * glam::camera::rh::view::look_to_mat4(Vec3::new(0.0, 2.0, 6.0), Vec3::NEG_Z, Vec3::Y),
            SIZE,
        );
        hit(7001, 256, 30);
        let cols = vertex_colors(&mut context, &host, &bridge);
        assert!(cols.iter().any(|c| { let [r, g, b] = rgb(*c); r > 240 && g > 50 && g < 90 && b > 50 && b < 90 }), "a red number");
        assert!(!cols.iter().any(|c| rgb(*c) == [255, 255, 255]), "no white one");
    }
    // a hit well above your usual is bigger: its glyphs stand taller than a usual hit's
    let usual_h = {
        let (host, bridge, mut context) = numbers_only();
        for _ in 0..6 {
            hit(256, 7001, 10);
        }
        height_of(&frame(&host, &bridge, &mut context))
    };
    let big_h = {
        let (host, bridge, mut context) = numbers_only();
        for _ in 0..5 {
            hit(256, 7001, 10);
        }
        hit(256, 7001, 100);
        height_of(&frame(&host, &bridge, &mut context))
    };
    assert!(big_h > usual_h, "the bigger hit's number stands taller ({big_h} vs {usual_h})");
}

#[test]
fn nameplates_a_number_near_the_eye_plane_or_behind_you_is_not_drawn_and_never_fails_the_mod() {
    {
        let (host, bridge, mut context) = numbers_only();
        hit(256, 7001, 12);
        // the creature's plate point is just in front of the eye and a long way to the side: finite pixels far away
        native_a12::set_plates(vec![(7001, [1.0e5, 0.0, -0.35])]);
        let v = frame(&host, &bridge, &mut context);
        assert!(v.iter().all(|p| p[0].abs() < 1.0e5 && p[1].abs() < 1.0e5), "nothing far off the screen reaches the draw call");
        assert!(host.drain_logs().unwrap().is_empty(), "no diagnostic: the mod is not failed");
    }
    // behind the camera: the number is not left frozen on the screen
    let (host, bridge, mut context) = numbers_only();
    hit(256, 7001, 12);
    assert!(!frame(&host, &bridge, &mut context).is_empty(), "control: drawn while in view");
    native_a12::set_plates(vec![]);
    let fov_y = 2.0 * (0.5f32 * SIZE.1 as f32 / (SIZE.0 as f32 / 2.0)).atan();
    let turned = glam::camera::rh::proj::directx::perspective(fov_y, SIZE.0 as f32 / SIZE.1 as f32, 0.3, 6000.0)
        * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, Vec3::Z, Vec3::Y);
    native_a12::set_view(turned, SIZE);
    assert!(frame(&host, &bridge, &mut context).is_empty(), "turned away from where it was hit, the number is not drawn");
    assert!(host.drain_logs().unwrap().is_empty());
}

#[test]
fn nameplates_the_hides_are_taken_again_after_the_client_drops_them_in_a_zone() {
    // the client drops every claim on a zone; the mod lets go of its own on the events and takes them again
    let run = |deliver_events: bool| -> (bool, bool, bool) {
        world(vec![entity(7001, "Gnoll Pup", "npc", [1.0, 0.0, -10.0])]);
        let mut context = context();
        let host = Host::for_addon(&addon()).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        let hud = attach(&host);
        host.run_addon(&addon()).unwrap();
        frame(&host, &bridge, &mut context);
        publish_claims(&hud.claims.borrow());
        assert_eq!(flags(), (true, true, false), "held in the world");
        // the zone: the world goes, the client drops the claims
        CORE.with(|c| c.borrow_mut().view.in_world = false);
        hud.lease.update(HealthSnapshot::new(true, 100, 100), true);
        publish_claims(&hud.claims.borrow());
        assert_eq!(flags(), (false, false, false), "dropped by the client: the game's own would show");
        if deliver_events {
            super::native_core::emit("world.leaving", json!({}));
            super::native_core::emit("state.reset", json!({"in_world": false, "initial": false}));
            native_events::dispatch(&host).unwrap();
        }
        // the new world
        CORE.with(|c| {
            let mut c = c.borrow_mut();
            c.view.in_world = true;
            c.generation += 1;
        });
        hud.lease.update(HealthSnapshot::new(true, 100, 100), true);
        if deliver_events {
            super::native_core::emit("world.entered", json!({}));
            super::native_core::emit("state.reset", json!({"in_world": true, "initial": false}));
        }
        frame(&host, &bridge, &mut context);
        publish_claims(&hud.claims.borrow());
        flags()
    };
    assert_eq!(run(true), (true, true, false), "taken again after the zone");
    // negative control: a mod that kept its old tokens (the events not delivered) leaves the game's plates showing
    assert_eq!(run(false), (false, false, false), "without the events the stale tokens are never replaced");
}

#[test]
fn nameplates_crowded_keeps_the_nearest_and_always_the_target() {
    let crowd = |n: u32| -> Vec<Json> {
        (0..n)
            .map(|i| entity(9000 + i, &format!("Crowd {i}"), "npc", [((i % 14) as f32 - 6.5) * 1.5, 0.0, -6.0 - (i / 14) as f32 * 2.0]))
            .collect()
    };
    // the target is the farthest of 70
    world(crowd(70));
    CORE.with(|c| c.borrow_mut().view.player["target_id"] = json!(9069));
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    let cols = vertex_colors(&mut context, &host, &bridge);
    assert!(cols.iter().any(|c| rgb(*c) == [255, 230, 89]), "the target's yellow frame is drawn although it is the farthest of 70");
    let v70 = frame(&host, &bridge, &mut context).len();
    world(crowd(60));
    CORE.with(|c| c.borrow_mut().view.player["target_id"] = json!(9059));
    let v60 = frame(&host, &bridge, &mut context).len();
    assert!(v70 <= v60 + 400, "70 in range draw about what 60 do ({v70} vs {v60}): the cap holds");
}

// ---- the plate layer is never in anyone's way ---------------------------------------------------------------------

/// Every draw list of a frame, back to front, as positions.
fn lists(host: &Host, bridge: &Bridge, context: &mut Context) -> Vec<Vec<[f32; 2]>> {
    context.frame();
    bridge.invoke(|| { native_events::dispatch(host)?; native_events::render(host) }).unwrap();
    let draw = context.render_legacy();
    draw.draw_data().draw_lists().map(|l| l.vtx_buffer().iter().map(|v| v.pos).collect()).collect()
}

const OTHER_WINDOW: &str = "eqoa.events.on('render', function() ImGui.SetNextWindowPos(60, 60); \
    ImGui.SetNextWindowSize(300, 200); ImGui.Begin('np_other_window'); ImGui.Text('a real window'); ImGui.End() end)";

#[test]
fn nameplates_plates_are_behind_every_window_whether_it_opened_before_or_after_the_layer() {
    for before in [true, false] {
        world(vec![entity(7001, "Gnoll Pup", "npc", [-3.0, 0.0, -8.0])]);
        let mut context = context();
        let host = Host::for_addon(&addon()).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        if before {
            host.execute(OTHER_WINDOW).unwrap();
        }
        host.run_addon(&addon()).unwrap();
        if !before {
            host.execute(OTHER_WINDOW).unwrap();
        }
        let l = lists(&host, &bridge, &mut context);
        // the background list is first (behind every window) and holds the plate; the other window's list is later
        // and the layer's own window draws nothing
        assert!(!l.is_empty() && !l[0].is_empty(), "the plates are in the background list (before = {before})");
        let others: Vec<usize> = l.iter().enumerate().skip(1).filter(|(_, v)| !v.is_empty()).map(|(i, _)| i).collect();
        assert_eq!(others.len(), 1, "only the other window draws among the windows: {:?}", l.iter().map(|v| v.len()).collect::<Vec<_>>());
        // a plate in the middle of the other window is under it (the list order is the draw order)
        assert!(others[0] > 0);
    }
}

#[test]
fn nameplates_layer_never_counts_as_a_window_in_front_and_holds_no_focus() {
    use super::pointer_order;
    world(vec![entity(7001, "Gnoll Pup", "npc", [0.0, 0.0, -10.0])]);
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    host.execute(OTHER_WINDOW).unwrap();
    for _ in 0..3 {
        lists(&host, &bridge, &mut context);
    }
    // the front-most real window at a point inside the other window is that window, not the layer; and over empty
    // screen there is none at all
    let over_other = pointer_order::front_real_window_z(100.0, 100.0);
    let over_nothing = pointer_order::front_real_window_z(900.0, 700.0);
    assert!(over_other.is_some(), "the other window is the front-most real window");
    assert!(over_nothing.is_none(), "the layer is not a window in front of anything");
    // no focus, no keyboard, no pointer taken
    unsafe {
        let g = dear_imgui_rs::sys::igGetCurrentContext();
        let nav = (*g).NavWindow;
        if !nav.is_null() {
            let name = std::ffi::CStr::from_ptr((*nav).Name).to_string_lossy().to_string();
            assert_ne!(name, "##nameplates_layer", "the layer never holds focus");
        }
        let io = &*dear_imgui_rs::sys::igGetIO_Nil();
        assert!(!io.WantCaptureKeyboard && !io.WantTextInput, "the layer holds no keyboard");
        assert!(!io.WantCaptureMouse, "and no mouse");
    }
    assert!(host.drain_logs().unwrap().is_empty());
}

/// Negative control for the test above: the same full-size window WITHOUT the layer's flags is a real window in
/// front of everything, and it does take the front place.
#[test]
fn nameplates_control_a_window_without_the_layers_flags_would_be_in_front() {
    use super::pointer_order;
    world(vec![]);
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.execute("eqoa.events.on('render', function() ImGui.SetNextWindowPos(0, 0); ImGui.SetNextWindowSize(1024, 768); \
        ImGui.Begin('np_unflagged'); ImGui.End() end)").unwrap();
    for _ in 0..3 {
        lists(&host, &bridge, &mut context);
    }
    assert!(pointer_order::front_real_window_z(900.0, 700.0).is_some(), "an ordinary full-screen window counts as in front");
}

/// 2026-11-24: with the game's full map up (M) the plates and numbers drawn on the background list are not drawn; the
/// map closed, they are back. Control: the same frame with the map closed draws the plate.
#[test]
fn hide_under_map_nameplates_draws_nothing_while_the_game_map_is_up() {
    world(vec![entity(7001, "Gnoll Pup", "npc", [1.0, 0.0, -10.0])]);
    let map = |open: bool| CORE.with(|c| c.borrow_mut().view.map = json!({"enabled": true, "open": open, "minimap": !open, "markers_on": true, "world_id": 1, "revision": 2}));
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    map(false);
    frame(&host, &bridge, &mut context);
    assert!(!frame(&host, &bridge, &mut context).is_empty(), "control: the plate is drawn with the map closed");
    map(true);
    frame(&host, &bridge, &mut context);
    assert!(frame(&host, &bridge, &mut context).is_empty(), "map up: no plate, no number");
    map(false);
    frame(&host, &bridge, &mut context);
    assert!(!frame(&host, &bridge, &mut context).is_empty(), "map closed: the plate is back");
    assert!(host.drain_logs().unwrap().is_empty());
}
