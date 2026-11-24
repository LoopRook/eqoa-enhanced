// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A12: world overlays for a mod. `eqoa.camera.project` (the maths, then through Lua) and the three hide
//! switches (`name_plates`, `damage_numbers`, `target_rings`) on `eqoa.hud.hide`.
use super::{
    health::{HealthBarClaimsRegistry, HealthSnapshot},
    host::Host,
    native_a12,
    native_core::{CORE, Core},
    visibility::{self, HudClaimsRegistry, VisibilityLease, publish_claims},
};
use glam::{Mat4, Vec3};
use std::{cell::RefCell, rc::Rc};

const SIZE: (u32, u32) = (800, 600);

/// A camera at `eye` looking along `dir`, 90 degrees across a surface of `SIZE`, the game's projection (depth 0..1,
/// near 0.3, far 6000).
fn view_proj(eye: Vec3, dir: Vec3) -> Mat4 {
    let fov_y = 2.0 * (0.5f32 * 600.0 / 400.0).atan(); // tan(half fov_x) = 1 across 800 px, so tan(half fov_y) = 0.75
    let proj = glam::camera::rh::proj::directx::perspective(fov_y, 800.0 / 600.0, 0.3, 6000.0);
    proj * glam::camera::rh::view::look_to_mat4(eye, dir, Vec3::Y)
}

#[test]
fn a12_a_known_world_point_lands_on_the_expected_pixel() {
    let vp = view_proj(Vec3::ZERO, Vec3::NEG_Z);
    // straight ahead: the middle of the surface
    let (at, depth, on, behind) = native_a12::project(vp, SIZE, [0.0, 0.0, -10.0]);
    let at = at.unwrap();
    assert!((at[0] - 400.0).abs() < 1e-3 && (at[1] - 300.0).abs() < 1e-3, "{at:?}");
    assert!((depth - 10.0).abs() < 1e-6 && on && !behind);
    // 5 right and 4 up at 10 ahead: 400 + 400 * 5/10 across, 300 - 400 * 4/10 up (the fov is 90 degrees across)
    let (at, _, on, _) = native_a12::project(vp, SIZE, [5.0, 4.0, -10.0]);
    let at = at.unwrap();
    assert!((at[0] - 600.0).abs() < 1e-3 && (at[1] - 140.0).abs() < 1e-3, "{at:?}");
    assert!(on);
    // just inside the right edge is on the screen; a hair past it is not
    assert!(native_a12::project(vp, SIZE, [9.9, 0.0, -10.0]).2);
    assert!(!native_a12::project(vp, SIZE, [10.1, 0.0, -10.0]).2);
    // behind the eye: no pixel, `behind`
    let (at, _, on, behind) = native_a12::project(vp, SIZE, [0.0, 0.0, 10.0]);
    assert!(at.is_none() && !on && behind);
    // in front but past the far plane, or inside the near one: off the screen, not behind
    let (at, _, on, behind) = native_a12::project(vp, SIZE, [0.0, 0.0, -7000.0]);
    assert!(at.is_some() && !on && !behind);
    assert!(!native_a12::project(vp, SIZE, [0.0, 0.0, -0.1]).2);
}

#[test]
fn a12_a_far_corner_of_the_world_keeps_its_pixel() {
    // Freeport: x 25,300. The point is 10 ahead of the eye, 5 to the right.
    let eye = Vec3::new(25_300.0, 80.0, -4_000.0);
    let vp = view_proj(eye, Vec3::NEG_Z);
    let at = native_a12::project(vp, SIZE, [25_305.0, 80.0, -4_010.0]).0.unwrap();
    assert!((at[0] - 600.0).abs() < 0.5 && (at[1] - 300.0).abs() < 0.5, "{at:?}");
}

fn in_world_host() -> Host {
    crate::settings::testing::reset(false);
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_a12::clear_view();
    let p = crate::play::Play::bundled_ui_fixture();
    let h = Host::new().unwrap();
    h.snapshot.set(p.forge_snapshot(None));
    h
}

#[test]
fn a12_camera_project_through_lua_with_its_refusals() {
    // not in a world: refused
    crate::settings::testing::reset(false);
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_a12::clear_view();
    let h = Host::new().unwrap();
    h.execute("local r,e = eqoa.camera.project({x=0,y=0,z=0}); assert(r==nil and e.code=='not_in_world', tostring(e and e.code))").unwrap();
    let h = in_world_host();
    // the world is up but no frame has been drawn yet
    h.execute("local r,e = eqoa.camera.project({x=0,y=0,z=0}); assert(r==nil and e.code=='unavailable', tostring(e and e.code))").unwrap();
    native_a12::set_view(view_proj(Vec3::ZERO, Vec3::NEG_Z), SIZE);
    h.execute(
        r#"
        for _, bad in ipairs({ 1, "x", {x=1,y=2}, {x="a",y=0,z=0}, {x=0/0,y=0,z=0} }) do
          local r,e = eqoa.camera.project(bad); assert(r==nil and e.code=='invalid_argument', tostring(e and e.code))
        end
        local r,e = eqoa.camera.project(); assert(r==nil and e.code=='invalid_argument')
        local r,e = eqoa.camera.project({x=0,y=0,z=0},{x=0,y=0,z=0}); assert(r==nil and e.code=='invalid_argument')
        local p = assert(eqoa.camera.project({x=0,y=0,z=-10}))
        assert(math.abs(p.x-400)<1e-3 and math.abs(p.y-300)<1e-3 and math.abs(p.depth-10)<1e-6)
        assert(p.on_screen==true and p.behind==false)
        local b = assert(eqoa.camera.project({x=0,y=0,z=10}))
        assert(b.x==nil and b.y==nil and b.on_screen==false and b.behind==true)
        "#,
    )
    .unwrap();
}

#[test]
fn a12_the_point_follows_the_view_of_the_frame_drawn_over_with_no_lag() {
    let h = in_world_host();
    // frame 1: the camera looks ahead; frame 2: it has turned to the right. The same world point, asked for in each
    // frame, is where THAT frame put it (a plate is never a frame behind its head).
    let point = "assert(eqoa.camera.project({x=0,y=0,z=-10}))";
    native_a12::set_view(view_proj(Vec3::ZERO, Vec3::NEG_Z), SIZE);
    let x1: f64 = h.lua.load(format!("return {point}.x")).eval().unwrap();
    native_a12::set_view(view_proj(Vec3::ZERO, Vec3::new(0.5, 0.0, -1.0).normalize()), SIZE);
    let x2: f64 = h.lua.load(format!("return {point}.x")).eval().unwrap();
    assert!((x1 - 400.0).abs() < 1e-3);
    assert!(x2 < x1 - 100.0, "the view turned right, the point moves left ({x1} to {x2})");
    // and the surface's size is the frame's: the same point on a larger surface scales with it
    native_a12::set_view(view_proj(Vec3::ZERO, Vec3::NEG_Z), (1600, 1200));
    let x3: f64 = h.lua.load(format!("return {point}.x")).eval().unwrap();
    assert!((x3 - 800.0).abs() < 1e-3);
}

#[test]
fn a12_a_hundred_projections_a_frame_are_cheap() {
    let h = in_world_host();
    native_a12::set_view(view_proj(Vec3::ZERO, Vec3::NEG_Z), SIZE);
    h.execute("pts={} for i=1,100 do pts[i]={x=i-50,y=i%7,z=-20-i} end").unwrap();
    let script = h.compile("for i=1,100 do local r=eqoa.camera.project(pts[i]); assert(r) end").unwrap();
    h.run(&script).unwrap(); // warm up
    let frames = 200;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        h.run(&script).unwrap();
    }
    let per_frame = start.elapsed() / frames;
    println!("A12 camera.project: 100 calls take {per_frame:?} a frame");
    assert!(per_frame < std::time::Duration::from_millis(25), "100 projections took {per_frame:?}");
}

// ---- the hide switches -----------------------------------------------------------------------------------------

struct Hud {
    host: Host,
    lease: VisibilityLease,
    claims: Rc<RefCell<HudClaimsRegistry>>,
}

fn hud_on(claims: Rc<RefCell<HudClaimsRegistry>>) -> Hud {
    let p = crate::play::Play::bundled_ui_fixture();
    let host = Host::new().unwrap();
    let snapshot = p.forge_snapshot(None);
    host.snapshot.set(snapshot);
    let lease =
        VisibilityLease::with_hud_claims(Rc::new(RefCell::new(HealthBarClaimsRegistry::new())), claims.clone()).unwrap();
    lease.update(snapshot.health, true);
    visibility::install_managed_health_adapter(&host, &lease).unwrap();
    publish_claims(&claims.borrow());
    Hud { host, lease, claims }
}

fn hud() -> Hud {
    hud_on(Rc::new(RefCell::new(HudClaimsRegistry::new())))
}

impl Hud {
    fn publish(&self) {
        publish_claims(&self.claims.borrow());
    }
}

fn hidden() -> (bool, bool, bool) {
    (
        crate::hud::fork_hud::plates_hidden(),
        crate::hud::fork_hud::damage_hidden(),
        crate::hud::fork_hud::rings_hidden(),
    )
}

#[test]
fn a12_each_hide_switch_holds_back_only_its_own_and_comes_back() {
    let h = hud();
    // negative control: no mod holds anything, the game draws all three
    assert_eq!(hidden(), (false, false, false));
    for (token, which) in [("name_plates", 0), ("damage_numbers", 1), ("target_rings", 2)] {
        h.host.execute(&format!("claim=assert(eqoa.hud.hide('{token}'))")).unwrap();
        h.publish();
        let mut expect = [false; 3];
        expect[which] = true;
        let now = hidden();
        assert_eq!([now.0, now.1, now.2], expect, "{token}");
        assert!(!super::hud_layout::hud_element_visible(token), "{token} reads as hidden through eqoa.hud.visible");
        h.host.execute(&format!("local ids=eqoa.hud.hidden_by('{token}'); assert(#ids==1)")).unwrap();
        // showing it gives it back to the game
        h.host.execute("assert(eqoa.hud.show(claim))").unwrap();
        h.publish();
        assert_eq!(hidden(), (false, false, false), "{token} shown again");
        assert!(super::hud_layout::hud_element_visible(token));
    }
}

#[test]
fn a12_the_game_keeps_its_plates_numbers_and_rings_under_the_other_hide_tokens() {
    let h = hud();
    h.host
        .execute("a=assert(eqoa.hud.hide('all')); b=assert(eqoa.hud.hide('all_pieces')); c=assert(eqoa.hud.hide('player'))")
        .unwrap();
    h.publish();
    assert_eq!(hidden(), (false, false, false), "all, all_pieces and player leave the world's overlays to the game");
    assert!(super::hud_layout::hud_element_visible("name_plates"));
}

#[test]
fn a12_a_disabled_or_failed_mod_gives_them_back() {
    let h = hud();
    h.host
        .execute("assert(eqoa.hud.hide('name_plates')); assert(eqoa.hud.hide('damage_numbers')); assert(eqoa.hud.hide('target_rings'))")
        .unwrap();
    h.publish();
    assert_eq!(hidden(), (true, true, true));
    // the mod is disabled, or its Lua failed: the manager clears the lease
    h.lease.clear();
    h.publish();
    assert_eq!(hidden(), (false, false, false), "released when the mod stops");
    // and the claim does not come back on its own when the lease is updated again
    h.lease.update(HealthSnapshot::new(true, 10, 10), true);
    h.publish();
    assert_eq!(hidden(), (false, false, false));
}

#[test]
fn a12_two_mods_holding_one_switch_keep_it_until_both_let_go() {
    let a = hud();
    let b = hud_on(a.claims.clone());
    a.host.execute("t=assert(eqoa.hud.hide('name_plates'))").unwrap();
    b.host.execute("t=assert(eqoa.hud.hide('name_plates'))").unwrap();
    a.publish();
    assert!(crate::hud::fork_hud::plates_hidden());
    a.lease.clear();
    a.publish();
    assert!(crate::hud::fork_hud::plates_hidden(), "the other mod still holds it");
    b.lease.clear();
    b.publish();
    assert!(!crate::hud::fork_hud::plates_hidden());
}

#[test]
fn a12_the_disclosure_names_the_three_world_pieces() {
    use super::discovery::DiscoveredAddon;
    for (token, label) in [
        ("hud.name_plates", "Name plates"),
        ("hud.damage_numbers", "Damage numbers"),
        ("hud.target_rings", "Target rings"),
    ] {
        assert!(visibility::is_hud_disclosure(token), "{token} is accepted in `replaces`");
        let mut addon = DiscoveredAddon {
            id: "a12".into(),
            name: "a12".into(),
            entry: "main.lua".into(),
            script_root: ".".into(),
            package_root: ".".into(),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: Some("eqoa/1".into()),
            entry_mode: Default::default(),
            capabilities_declared: true,
            requested_capabilities: vec![],
            extensions: Default::default(),
            initially_enabled: true,
            mods_root: None,
        };
        addon.extensions.replaces = vec![token.to_string()];
        let lines = super::runtime::disclosure_lines(&addon);
        assert!(lines.iter().any(|l| l == &format!("Replaces HUD: {label}")), "{token}: {lines:?}");
    }
    assert!(!visibility::is_hud_disclosure("hud.nothing_like_it"));
}

// ---- the game's own draws obey the switch -----------------------------------------------------------------------

/// The guard lines in `play.rs` sit in the game's own draws, before its result is used. (`Play::plates` and
/// `Play::draw_rings` need real puppets and a window's renderer to run, so the lines are checked in place and the
/// switch itself above.)
#[test]
fn a12_the_guard_lines_sit_in_the_games_own_draws() {
    let src = include_str!("../../play.rs").replace("\r\n", "\n");
    let section = |from: &str, to: &str| {
        let a = src.find(from).unwrap_or_else(|| panic!("{from}"));
        let b = a + src[a..].find(to).unwrap_or_else(|| panic!("{to}"));
        src[a..b].to_string()
    };
    let plates = section("pub fn plates(&self) -> Vec<Plate> {", "        out\n    }");
    let guard = plates.find("fork_hud::plates_hidden()").expect("plates guard");
    assert!(guard < plates.find("for (slot, e)").unwrap(), "the guard comes before any plate is made");
    let rings = section("pub fn draw_rings(", "r.set_overlay(device, queue, view_proj, &tris");
    let (hide_ui, mine) = (rings.find("crate::hideui::hidden()").unwrap(), rings.find("fork_hud::rings_hidden()").unwrap());
    assert!(hide_ui < mine, "after Hide UI's own line, before the rings are handed to the renderer");
    let damage = section("let under = self.damage.place(", "ui.quads.splice(0..0, under)");
    assert!(damage.contains("fork_hud::damage_hidden()"));
}
