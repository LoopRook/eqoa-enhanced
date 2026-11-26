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

// ---- the game's own plate point -----------------------------------------------------------------------------------

#[test]
fn a12_plate_points_are_the_ones_play_plates_would_draw() {
    use super::native_a12::{PlateSource, plate_points_of};
    let ok = PlateSource { id: 7, has_creature: true, name_empty: false, shown: true, name_at: Some([1.0, 2.0, 3.0]) };
    let with = |f: fn(&mut PlateSource)| {
        let mut s = PlateSource { id: ok.id, has_creature: ok.has_creature, name_empty: ok.name_empty, shown: ok.shown, name_at: ok.name_at };
        f(&mut s);
        s
    };
    assert_eq!(plate_points_of([ok]), vec![(7, [1.0, 2.0, 3.0])], "the point is Puppet::name_at, unchanged");
    // negative controls: each reason `Play::plates` skips a creature skips it here
    assert!(plate_points_of([with(|s| s.has_creature = false)]).is_empty());
    assert!(plate_points_of([with(|s| s.name_empty = true)]).is_empty());
    assert!(plate_points_of([with(|s| s.shown = false)]).is_empty());
    assert!(plate_points_of([with(|s| s.name_at = None)]).is_empty());
    // and the tests are the ones `Play::plates` makes (a change there must come here)
    let src = include_str!("../../play.rs").replace("\r\n", "\n");
    assert!(src.contains("let (Some(c), Some(at)) = (&e.creature, p.name_at) else { continue };"));
    assert!(src.contains("if c.name.is_empty() || !p.shown {"));
    assert!(src.contains("out.push(Plate { at, color: self.name_color(c)"));
}

#[test]
fn a12_project_entity_projects_the_point_the_game_puts_its_plate_at() {
    let h = in_world_host();
    native_a12::set_plates(vec![]);
    native_a12::set_view(view_proj(Vec3::ZERO, Vec3::NEG_Z), SIZE);
    // control: no plate point for it
    h.execute("local r,e = eqoa.camera.project_entity(7); assert(r==nil and e.code=='not_found', tostring(e and e.code))").unwrap();
    native_a12::set_plates(vec![(7, [0.0, 0.0, -10.0])]);
    h.execute(
        r#"
        local a = assert(eqoa.camera.project_entity(7))
        local b = assert(eqoa.camera.project({x=0,y=0,z=-10}))
        assert(a.x==b.x and a.y==b.y and a.depth==b.depth and a.on_screen and not a.behind)
        local c = assert(eqoa.camera.project_entity({id=7}, 1))
        local d = assert(eqoa.camera.project({x=0,y=1,z=-10}))
        assert(c.y==d.y and c.y < a.y)
        for _, bad in ipairs({ "x", -1, 1.5, {} }) do
          local r,e = eqoa.camera.project_entity(bad); assert(r==nil and e.code=='invalid_argument', tostring(e and e.code))
        end
        local r,e = eqoa.camera.project_entity(7, "up"); assert(r==nil and e.code=='invalid_argument')
        local r,e = eqoa.camera.project_entity(8); assert(r==nil and e.code=='not_found')
        "#,
    )
    .unwrap();
    // the next frame has no plate for it (out of range or gone): not found again
    native_a12::set_plates(vec![]);
    h.execute("local r,e = eqoa.camera.project_entity(7); assert(r==nil and e.code=='not_found')").unwrap();
}

// ---- fix round: finite results, quest marks and merc tags, guard negative control ----------------------------------

#[test]
fn a12_project_never_returns_a_value_that_is_not_a_number() {
    let vp = view_proj(Vec3::ZERO, Vec3::NEG_Z);
    let all_sane = |name: &str, (at, depth, on, behind): (Option<[f64; 2]>, f64, bool, bool)| {
        assert!(depth.is_finite(), "{name}: depth {depth}");
        match at {
            Some(a) => assert!(a[0].is_finite() && a[1].is_finite() && !behind, "{name}: {a:?}"),
            None => assert!(behind && !on, "{name}: nil position only when behind"),
        }
    };
    // 1e308 in each way, a point a denormal's width in front of the eye (the division overflows), and the control
    for (name, p) in [
        ("1e308", [1e308, 1e308, 1e308]),
        ("-1e308 mix", [1e308, -1e308, 1e308]),
        ("1e308 ahead", [0.0, 0.0, -1e308]),
        ("1e-315 ahead", [0.0, 0.0, -1e-315]),
        ("1e-315 side", [1e-315, 0.0, -1e-315]),
    ] {
        let r = native_a12::project(vp, SIZE, p);
        all_sane(name, r);
        assert!(!r.2, "{name}: never on the screen");
        if name != "1e308 ahead" {
            assert!(r.3, "{name}: cannot be computed, reported as behind");
        } else {
            assert!(!r.3, "{name}: in front, only past the far plane");
        }
    }
    let control = native_a12::project(vp, SIZE, [0.0, 0.0, -10.0]);
    all_sane("control", control);
    assert!(!control.3 && control.2, "the control is a point on the screen");
}

#[test]
fn a12_project_through_lua_gives_a_number_or_nil_only_when_behind() {
    let h = in_world_host();
    native_a12::set_view(view_proj(Vec3::ZERO, Vec3::NEG_Z), SIZE);
    h.execute(
        r#"
        local function check(p)
          local r, e = eqoa.camera.project(p)
          assert(r ~= nil, tostring(e and e.code))
          assert(type(r.depth) == 'number' and r.depth == r.depth and r.depth ~= math.huge and r.depth ~= -math.huge)
          if r.behind then assert(r.x == nil and r.y == nil and r.on_screen == false)
          else assert(type(r.x) == 'number' and type(r.y) == 'number') end
        end
        check({x=1e308,y=1e308,z=1e308}); check({x=1e308,y=-1e308,z=1e308}); check({x=0,y=0,z=-1e308})
        check({x=0,y=0,z=-1e-315}); check({x=0,y=0,z=-10}); check({x=0,y=0,z=10})
        -- not numbers at all: refused
        for _, bad in ipairs({ {x=1/0,y=0,z=0}, {x=-1/0,y=0,z=0}, {x=0,y=0/0,z=0} }) do
          local r, e = eqoa.camera.project(bad); assert(r == nil and e.code == 'invalid_argument')
        end
        -- an explicit nil second argument is a second argument
        local r, e = eqoa.camera.project({x=0,y=0,z=0}, nil); assert(r == nil and e.code == 'invalid_argument')
        "#,
    )
    .unwrap();
}

#[test]
fn a12_the_entity_record_carries_the_plates_quest_mark_and_merc_tag() {
    crate::settings::testing::reset(false);
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let mut p = crate::play::Play::bundled_ui_fixture();
    let creature = |id: u32, name: &str, p256: u8| net::entity::Entity {
        id,
        kind: net::entity::TYPE_CREATURE,
        p19: 255,
        creature: Some(net::entity::Creature { name: name.into(), level: 5, p256, ..Default::default() }),
        ..Default::default()
    };
    p.entities.slots[2] = Some(creature(9101, "Quest Giver", 0));
    p.entities.slots[3] = Some(creature(9102, "Ralthas", 3));
    p.entities.slots[4] = Some(creature(9103, "Plain Rat", 0));
    p.session.head_marks_offer = true;
    p.session.agree_head_marks(true);
    p.session.head_marks.insert(9101, net::headmarks::TAKE);
    p.session.head_marks.insert(9102, net::headmarks::STEP);
    p.session.mercs = Some(net::session::Mercs { own: false, names: vec!["Ralthas".into()], owners: vec!["Lonewulf".into()], ..Default::default() });
    // what `Play::plates` reads for each is what the record carries (the same two calls)
    let extras = |id: u32| p.fork_plate_extras(p.entities.slots.iter().flatten().find(|e| e.id == id).unwrap());
    assert_eq!(extras(9101), (Some("!"), None));
    assert_eq!(extras(9102), (Some("?"), Some("<Lonewulf's Mercenary>".to_string())));
    assert_eq!(extras(9103), (None, None), "control: nothing on a plain creature");
    let h = Host::new().unwrap();
    h.snapshot.set(p.forge_snapshot(None));
    h.execute(
        r#"
        local by = {}
        for _, e in ipairs(eqoa.entities.list()) do by[e.id] = e end
        assert(by[9101].head_mark == '!' and by[9101].merc_tag == nil)
        assert(by[9102].head_mark == '?' and by[9102].merc_tag == "<Lonewulf's Mercenary>")
        assert(by[9103].head_mark == nil and by[9103].merc_tag == nil)
        "#,
    )
    .unwrap();
    // Extended's Quest Markers off: no mark on the plate, none in the record
    let mut off = crate::settings::client();
    off.head_marks = false;
    crate::settings::set_client(off);
    assert_eq!(extras(9101).0, None, "negative control: the setting that hides marks on the plate hides them here");
    // and the plate reads them in the way this record does
    let src = include_str!("../../play.rs").replace("\r\n", "\n");
    assert!(src.contains("let mark = crate::settings::head_marks().then(|| self.session.head_mark(e.id)).flatten();"));
    assert!(src.contains("tag: self.merc_tag(c), mark"));
}

#[test]
fn a12_the_guard_placement_check_fails_without_the_guards() {
    // negative control for `a12_the_guard_lines_sit_in_the_games_own_draws`: the same test on play.rs with the
    // guard lines taken out must fail
    let src = include_str!("../../play.rs").replace("\r\n", "\n");
    assert!(guards_ok(&src));
    for guard in ["fork_hud::plates_hidden()", "fork_hud::rings_hidden()", "fork_hud::damage_hidden()"] {
        assert!(!guards_ok(&src.replace(guard, "true")), "without {guard} the check must fail");
    }
}

fn guards_ok(src: &str) -> bool {
    let section = |from: &str, to: &str| {
        let a = src.find(from)?;
        let b = a + src[a..].find(to)?;
        Some(src[a..b].to_string())
    };
    let ok = || -> Option<bool> {
        let plates = section("pub fn plates(&self) -> Vec<Plate> {", "        out\n    }")?;
        let rings = section("pub fn draw_rings(", "r.set_overlay(device, queue, view_proj, &tris")?;
        let damage = section("let under = self.damage.place(", "ui.quads.splice(0..0, under)")?;
        Some(
            plates.find("fork_hud::plates_hidden()")? < plates.find("for (slot, e)")?
                && rings.find("crate::hideui::hidden()")? < rings.find("fork_hud::rings_hidden()")?
                && damage.contains("fork_hud::damage_hidden()"),
        )
    };
    ok().unwrap_or(false)
}

// ---- the game's own Stat Window ---------------------------------------------------------------------------------

#[test]
fn stat_window_a_mod_holds_back_the_games_stat_window_and_gives_it_back() {
    let h = hud();
    // negative control: no mod holds it, the game draws it
    assert!(crate::hud::fork_hud::stat_window_visible());
    assert!(super::hud_layout::hud_element_visible("stat_window"));
    h.host.execute("claim=assert(eqoa.hud.hide('stat_window'))").unwrap();
    h.publish();
    assert!(!crate::hud::fork_hud::stat_window_visible());
    assert!(!super::hud_layout::hud_element_visible("stat_window"), "reads as hidden through eqoa.hud.visible");
    h.host.execute("assert(#eqoa.hud.hidden_by('stat_window')==1)").unwrap();
    // it holds back only the Stat Window: the other overlays stay the game's
    assert_eq!(hidden(), (false, false, false));
    assert!(super::hud_layout::hud_element_visible("name_plates"));
    assert!(super::hud_layout::hud_element_visible("target"));
    // the player's "Use the game's windows" gives the game's window back while the mod still holds its claim
    super::window_takeover::set_use_game_windows(true);
    assert!(crate::hud::fork_hud::stat_window_visible(), "the switch is on: the game's Stat Window");
    assert!(super::hud_layout::hud_element_visible("stat_window"));
    super::window_takeover::set_use_game_windows(false);
    assert!(!crate::hud::fork_hud::stat_window_visible(), "the switch is off again: held");
    h.host.execute("assert(eqoa.hud.show(claim))").unwrap();
    h.publish();
    assert!(crate::hud::fork_hud::stat_window_visible(), "shown again");
    super::window_takeover::forget_switch_for_test();
}

#[test]
fn stat_window_the_other_hide_tokens_leave_it_to_the_game() {
    let h = hud();
    h.host
        .execute("a=assert(eqoa.hud.hide('all')); b=assert(eqoa.hud.hide('all_pieces')); c=assert(eqoa.hud.hide('player'))")
        .unwrap();
    h.publish();
    assert!(crate::hud::fork_hud::stat_window_visible(), "all, all_pieces and player do not hide it");
}

#[test]
fn stat_window_a_disabled_or_failed_mod_gives_it_back_and_two_mods_share_the_claim() {
    let a = hud();
    let b = hud_on(a.claims.clone());
    a.host.execute("t=assert(eqoa.hud.hide('stat_window'))").unwrap();
    b.host.execute("t=assert(eqoa.hud.hide('stat_window'))").unwrap();
    a.publish();
    assert!(!crate::hud::fork_hud::stat_window_visible());
    a.lease.clear();
    a.publish();
    assert!(!crate::hud::fork_hud::stat_window_visible(), "the other mod still holds it");
    b.lease.clear();
    b.publish();
    assert!(crate::hud::fork_hud::stat_window_visible(), "released when the last mod stops");
    // and the claim does not come back on its own
    b.lease.update(HealthSnapshot::new(true, 10, 10), true);
    b.publish();
    assert!(crate::hud::fork_hud::stat_window_visible());
}

/// The game's own key for the Stat Window (and `/stats`) is told to the mod that holds the window, and to no other mod; with
/// no claim, or the player's "Use the game's windows" on, nobody is told (the key is the game's). The event's fields are as
/// declared (`piece`, `action`).
#[test]
fn stat_window_the_games_key_is_told_to_the_holder_only() {
    use crate::hud::fork_hud::{stat_window_set_held, stat_window_toggle_held};
    let a = hud();
    let b = hud_on(a.claims.clone());
    let listen = |h: &Hud, tag: &str| {
        h.host.execute(&format!("told = {{}}; eqoa.events.on('hud.piece_requested', function(e) told[#told + 1] = '{tag}:' .. e.data.piece .. ':' .. e.data.action end)")).unwrap();
    };
    listen(&a, "a");
    listen(&b, "b");
    let told = |h: &Hud| -> String { h.host.lua.load("return table.concat(told, ',')").eval().unwrap() };
    let deliver = || {
        super::native_core::flush_deferred();
        for h in [&a, &b] {
            super::native_events::dispatch(&h.host).unwrap();
        }
    };
    // no claim: the game's window and key; nobody is told
    assert!(!stat_window_toggle_held(), "no mod holds it: the game acts");
    deliver();
    assert_eq!((told(&a).as_str(), told(&b).as_str()), ("", ""));
    // `a` holds it: the game does nothing, `a` is told what was asked, `b` is not
    a.host.execute("claim=assert(eqoa.hud.hide('stat_window'))").unwrap();
    a.publish();
    assert!(stat_window_toggle_held(), "a mod holds it: the game does nothing");
    assert!(stat_window_set_held(true) && stat_window_set_held(false));
    deliver();
    assert_eq!(told(&a), "a:stat_window:toggle,a:stat_window:show,a:stat_window:hide");
    assert_eq!(told(&b), "", "a mod that does not hold the window is not told");
    // "Use the game's windows": the key is the game's again
    super::window_takeover::set_use_game_windows(true);
    assert!(!stat_window_toggle_held());
    deliver();
    assert_eq!(told(&a).matches(',').count(), 2, "nothing more was told");
    super::window_takeover::forget_switch_for_test();
    // the claim let go: the game's again
    a.host.execute("assert(eqoa.hud.show(claim))").unwrap();
    a.publish();
    assert!(!stat_window_toggle_held());
    deliver();
    assert_eq!(told(&a).matches(',').count(), 2);
}

#[test]
fn stat_window_the_disclosure_names_it() {
    assert!(visibility::is_hud_disclosure("hud.stat_window"), "accepted in `replaces`");
}

/// Two lines in the game's own code carry the switch: the draw (which also leaves no panel for a click to find)
/// and the one place F4 and `/stats` both end (`set_stat_window`). Their behaviour is tested in `play`'s
/// `stat_window_hide_tests`; this keeps the lines from being lost in a merge.
#[test]
fn stat_window_the_guard_lines_sit_in_the_games_own_code() {
    let play = include_str!("../../play.rs").replace("\r\n", "\n");
    let at = play.find("self.stat_panel = None;\n            if crate::settings::stat_window()").expect("the Stat Window draw");
    let line = play[at..].lines().nth(1).unwrap();
    assert!(line.contains("fork_hud::stat_window_visible()"), "{line}");
    let statplay = include_str!("../../statplay.rs").replace("\r\n", "\n");
    let a = statplay.find("fn set_stat_window(").expect("set_stat_window");
    let body = &statplay[a..a + statplay[a..].find("/// `/stats`").unwrap()];
    // the hold asks the mod (`hud.piece_requested`) before the setting is touched, for `/stats on|off` and a click on the X
    assert!(body.find("fork_hud::stat_window_set_held(on)").unwrap() < body.find("settings::set_stat_window(on)").unwrap());
    // and the key and `/stats` alone (`toggle_stat_window`) ask before the toggle is worked out from the setting
    let t = statplay.find("fn toggle_stat_window(").expect("toggle_stat_window");
    let toggle = &statplay[t..t + statplay[t..].find("
    }
").unwrap()];
    assert!(toggle.find("fork_hud::stat_window_toggle_held()").unwrap() < toggle.find("settings::stat_window()").unwrap(), "{toggle}");
}
