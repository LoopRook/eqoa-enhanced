// Real Manager + GPU evidence for the bundled eqoa_round_minimap mod (included in manager tests).
struct Rm {
    gpu: Gpu,
    _root: Scratch,
    _config: Scratch,
    manager: Manager,
    play: crate::play::Play,
    walker: crate::walker::Walker,
    me: [f32; 3],
    scene: crate::map::Map,
    draws: usize,
}
const RM_SIZE: (u32, u32) = (1024, 768);
impl Rm {
    fn new(with_search: bool) -> Option<Rm> {
        Rm::with(|p| p == "eqoa_round_minimap" || (with_search && p == "eqoa_map_search"), 0)
    }
    fn with(want: impl Fn(&str) -> bool, extra_entities: usize) -> Option<Rm> {
        let gpu = Gpu::new()?;
        crate::settings::testing::reset(false);
        crate::hudlayout::reset();
        crate::hudlayout::set_unlocked(false);
        crate::hideui::show();
        super::super::input::reset_input_state();
        super::super::hud_layout::set_window_size(RM_SIZE.0, RM_SIZE.1);
        super::super::hud_layout::set_hud_state(true, true, false, 93, 0);
        let root = Scratch::new();
        let config = Scratch::new();
        for file in crate::fork_ext::bundled::EMBEDDED_FILES.iter().filter(|f| want(f.package)) {
            let path = root.0.join(file.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, file.data).unwrap();
        }
        let mut play = crate::play::Play::bundled_ui_fixture();
        play.map.mini.up = true;
        let walker = crate::play::Play::bundled_ui_walker();
        let at = walker.cam.walk.phys.position;
        let me = [at.x.0, at.y.0, at.z.0];
        // a creature 40 units east and 30 south of the player at (100, 200)
        play.entities.slots[2] = Some(net::entity::Entity {
            id: 3003,
            kind: net::entity::TYPE_CREATURE,
            p19: 255,
            pos: [me[0] + 40.0, me[1], me[2] + 30.0],
            actor_pos: [me[0] + 40.0, me[1], me[2] + 30.0],
            creature: Some(net::entity::Creature { name: "Test Rat".into(), level: 7, ..Default::default() }),
            ..Default::default()
        });
        for i in 0..extra_entities {
            let a = i as f32 * 2.399_963;
            let d = 20.0 + (i % 40) as f32 * 5.0;
            let at = [me[0] + a.cos() * d, me[1], me[2] + a.sin() * d];
            play.entities.slots.push(Some(net::entity::Entity {
                id: 5000 + i as u32,
                kind: net::entity::TYPE_CREATURE,
                p19: 255,
                pos: at,
                actor_pos: at,
                creature: Some(net::entity::Creature { name: format!("Busy Creature {i}"), level: 1 + (i % 50) as u8, ..Default::default() }),
                ..Default::default()
            }));
        }
        let manager = gpu.manager(&root.0, Some(config.0.clone()));
        let scene = crate::map::Map::round4_fixture(&gpu.device, &gpu.queue, RM_SIZE, true);
        Some(Rm { gpu, _root: root, _config: config, manager, play, walker, me, scene, draws: 0 })
    }
    /// One whole frame: the addons' callbacks, the native minimap draw routed as the viewer
    /// routes it, then the shared ImGui pass over it.
    fn frame(&mut self) -> wgpu::Texture {
        let mut snapshot = self.play.forge_snapshot(Some(&self.walker));
        snapshot.capture_pending = false;
        if self.play.map.mini.up {
            self.scene.round4_mini_frame(&self.gpu.device, &self.gpu.queue, RM_SIZE);
        } else {
            self.scene.mini_off();
        }
        self.manager.flush_pending_fonts(&mut self.gpu.context);
        r4_begin(&mut self.gpu);
        super::super::native_core::presentation();
        self.manager.frame(snapshot);
        let size = RM_SIZE;
        let gpu = &mut self.gpu;
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("round minimap frame"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = render::depth_view(&gpu.device, size.0, size.1);
        let view = target.create_view(&Default::default());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.2, g: 0.2, b: 0.4, a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        let before = super::super::map_views::draw_calls();
        crate::fork_ext::map::draw_minimap(&self.scene, &gpu.device, &mut encoder, &view, &depth, size, size, wgpu::TextureFormat::Rgba8Unorm);
        self.draws += super::super::map_views::draw_calls() - before;
        {
            let mut renderer = gpu.renderer.borrow_mut();
            let frame = gpu.context.render(renderer.renderer_consumer().unwrap());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderer.render(frame, &mut pass, FramebufferExtent::new(size.0, size.1)).unwrap();
        }
        gpu.queue.submit([encoder.finish()]);
        self.manager.after_render();
        target
    }
    fn frames(&mut self, n: usize) -> wgpu::Texture {
        let mut t = self.frame();
        for _ in 1..n {
            t = self.frame();
        }
        t
    }
    fn lua(&self) -> Lua {
        lua(&self.manager, "eqoa_round_minimap")
    }
    fn state<T: mlua::FromLua>(&self, expr: &str) -> T {
        self.lua().load(format!("return {expr}")).eval().unwrap()
    }
    fn at(&self, x: f32, y: f32) {
        super::super::pointer_api::position([x, y], true, RM_SIZE);
    }
    fn circle(&self) -> (f64, f64, f64) {
        (self.state("round_minimap.circle.cx"), self.state("round_minimap.circle.cy"), self.state("round_minimap.circle.r"))
    }
}
fn rm_px(pixels: &[u8], x: f64, y: f64) -> [u8; 4] {
    let i = ((y as usize) * RM_SIZE.0 as usize + x as usize) * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

#[test]
fn round_minimap_hides_stock_blocks_its_clicks_and_returns_when_off() {
    let Some(mut rm) = Rm::new(false) else { return };
    // stock minimap: its rectangle takes clicks while no mod holds it
    let inside = {
        let r = super::super::hud_layout::hud_layout_json(RM_SIZE);
        [r["minimap"]["x"].as_f64().unwrap() as f32 + 56.0, r["minimap"]["y"].as_f64().unwrap() as f32 + 56.0]
    };
    rm.play.map.mini.frame_px = {
        let r = super::super::hud_layout::hud_layout_json(RM_SIZE);
        let (x, y, w, h) = (r["minimap"]["x"].as_f64().unwrap() as f32, r["minimap"]["y"].as_f64().unwrap() as f32, r["minimap"]["width"].as_f64().unwrap() as f32, r["minimap"]["height"].as_f64().unwrap() as f32);
        [x, y, x + w, y + h]
    };
    assert!(crate::fork_ext::minimap_contains(&rm.play.map, inside), "no mod: stock minimap takes clicks");
    rm.frames(4);
    assert!(rm.manager.entries.iter().all(|e| e.status == Status::Enabled), "{:?}", rm.manager.diagnostics());
    assert!(rm.manager.is_hud_element_hidden("minimap"), "the mod hides the stock minimap");
    assert!(!crate::fork_ext::minimap_contains(&rm.play.map, inside), "stock clicks blocked while the mod is on");
    assert!(rm.state::<bool>("round_minimap.shown"), "the round minimap is showing");
    // one native minimap draw per frame
    rm.draws = 0;
    rm.frames(5);
    assert_eq!(rm.draws, 5, "exactly one native minimap draw per frame");
    // mod off: the stock minimap is back
    rm.manager.queue(Command::Enable("eqoa_round_minimap".into(), false));
    rm.frames(3);
    assert!(!rm.manager.is_hud_element_hidden("minimap"));
    assert!(crate::fork_ext::minimap_contains(&rm.play.map, inside), "mod off: stock minimap takes clicks again");
    rm.draws = 0;
    rm.frames(3);
    assert_eq!(rm.draws, 3, "stock draw only, once a frame");
    rm.manager.shutdown();
}

fn rm_stock_rect(rm: &Rm) -> [f32; 4] {
    let r = super::super::hud_layout::hud_layout_json(RM_SIZE);
    let m = &r["minimap"];
    let f = |k: &str| m[k].as_f64().unwrap() as f32;
    [f("x"), f("y"), f("x") + f("width"), f("y") + f("height")].map(|v| { let _ = rm; v })
}

const RM_BG: [u8; 4] = [51, 51, 102, 255];
fn rm_is_picture(p: [u8; 4]) -> bool {
    p[1] > 120 && p[0] < 80 && p[2] < 90
}

#[test]
fn round_minimap_circle_follows_move_hud_and_clips_to_a_circle() {
    use crate::hudlayout::{self, Beyond, Piece};
    let Some(mut rm) = Rm::new(false) else { return };
    rm.frames(4);
    let t = rm.frames(2);
    let pixels = r4_pixels(&rm.gpu, &t);
    rm_png(&pixels, "r4-minimap-outdoors.png");
    assert_eq!(rm.state::<Option<String>>("round_minimap.image_error"), None);
    let (cx, cy, r) = rm.circle();
    let stock = rm_stock_rect(&rm);
    assert!((cx - (stock[0] + stock[2]) as f64 / 2.0).abs() < 1.0 && (cy - (stock[1] + stock[3]) as f64 / 2.0).abs() < 1.0, "circle sits in the stock rectangle");
    assert!(rm_is_picture(rm_px(&pixels, cx + r * 0.5, cy + r * 0.5)), "the picture shows inside the circle");
    assert!(rm_is_picture(rm_px(&pixels, cx - r * 0.5, cy - r * 0.5)));
    // the square picture's corner lies outside the circle: cut off (control: the same corner in the stock square)
    assert_eq!(rm_px(&pixels, cx + r * 0.95, cy + r * 0.95), RM_BG, "the picture's corner is cut off by the circle");
    {
        r4_begin(&mut rm.gpu);
        let t = r4_render(&mut rm.gpu, &rm.scene);
        let direct = r4_pixels(&rm.gpu, &t);
        assert!(rm_is_picture(rm_px(&direct, cx + r * 0.95, cy + r * 0.95)), "control: the stock square draws that corner");
    }
    // Move HUD: drag the minimap left and down
    hudlayout::frame_begin();
    hudlayout::note(Piece::Minimap, [474.0, 50.0, 586.0, 162.0], [0.0, 0.0]);
    hudlayout::frame_end();
    hudlayout::set_unlocked(true);
    let b = Beyond::of(RM_SIZE);
    hudlayout::press([530.0, 106.0], b);
    hudlayout::drag_to([470.0, 156.0], b);
    assert!(hudlayout::release());
    hudlayout::set_unlocked(false);
    let moved = rm_stock_rect(&rm);
    assert!((moved[0] - stock[0]).abs() > 20.0, "Move HUD moved the stock rectangle");
    rm.frames(4);
    let t = rm.frames(2);
    let pixels = r4_pixels(&rm.gpu, &t);
    rm_png(&pixels, "r4-minimap-moved.png");
    let (nx, ny, _) = rm.circle();
    assert!((nx - (moved[0] + moved[2]) as f64 / 2.0).abs() < 1.0 && (ny - (moved[1] + moved[3]) as f64 / 2.0).abs() < 1.0, "circle follows the moved rectangle");
    assert!(rm_is_picture(rm_px(&pixels, nx + r * 0.5, ny + r * 0.5)), "picture at the new place");
    assert_eq!(rm_px(&pixels, cx + r * 0.5, cy + r * 0.5), RM_BG, "nothing left at the old place");
    hudlayout::reset();
    rm.manager.shutdown();
}

fn rm_png(pixels: &[u8], name: &str) {
    if let Some(dir) = std::env::var_os("EQOA_R4_CAPTURE_DIR") {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).unwrap();
        let mut enc = png::Encoder::new(fs::File::create(dir.join(name)).unwrap(), RM_SIZE.0, RM_SIZE.1);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(pixels).unwrap();
    }
}

#[test]
fn round_minimap_dot_card_wheel_zoom_and_n_toggle() {
    let Some(mut rm) = Rm::new(false) else { return };
    rm.frames(4);
    // Test Rat stands 40 east, 30 south of the player
    let lay = rm_stock_rect(&rm);
    let (cx, cy, r) = rm.circle();
    let k = (lay[2] - lay[0]) as f64 / 112.0;
    let (px, py, inside): (f64, f64, bool) = rm
        .lua()
        .load(format!("return round_minimap.view.project({},{})", rm.me[0] + 40.0, rm.me[2] + 30.0))
        .eval()
        .unwrap();
    assert!(inside, "the creature projects inside the circle");
    let (dx, dy) = (lay[0] as f64 + px * k, lay[1] as f64 + py * k);
    assert!(((dx - cx).powi(2) + (dy - cy).powi(2)).sqrt() < r, "the creature is within the circle");
    // pointer away: no card (control)
    rm.at((cx - r * 0.6) as f32, (cy - r * 0.6) as f32);
    rm.frames(2);
    assert!(!rm.state::<bool>("round_minimap.card ~= nil"), "no card away from a dot");
    // pointer on the dot: its card
    rm.at(dx as f32, dy as f32);
    rm.frames(2);
    assert!(rm.state::<bool>("round_minimap.card ~= nil"), "a card on the dot");
    let lines: Vec<String> = rm.lua().load("local t={} for _,l in ipairs(round_minimap.card.lines) do t[#t+1]=l[1] end return t").eval().unwrap();
    assert_eq!(lines[0], "Test Rat");
    assert_eq!(lines[1], "Level 7 / NPC");
    assert!(lines[2].starts_with("50 away"), "distance 50: {lines:?}");
    let t = rm.frames(1);
    rm_png(&r4_pixels(&rm.gpu, &t), "r4-minimap-hover-card.png");
    // wheel zooms the native minimap (and a wheel outside the circle does not)
    let before = rm.scene.mini.scale;
    rm.at((cx + r * 1.5) as f32, cy as f32);
    rm_wheel(1.0);
    rm.frames(1);
    assert_eq!(super::super::pointer_api::take_wheel(), 0.0, "outside the circle: no zoom request");
    rm.at(cx as f32, cy as f32);
    rm_wheel(1.0);
    rm.frames(1);
    let wheel = super::super::pointer_api::take_wheel();
    assert!(wheel > 0.0, "inside the circle: a zoom request");
    rm.scene.mini_wheel(wheel);
    assert!(rm.scene.mini.scale < before, "the native minimap zoomed in: {} -> {}", before, rm.scene.mini.scale);
    // N: the stock setting off removes the round minimap; on brings it back
    assert!(rm.state::<bool>("round_minimap.shown"));
    rm.play.map.mini.up = false;
    rm.frames(3);
    assert!(!rm.state::<bool>("round_minimap.shown"), "N off: no round minimap");
    rm.play.map.mini.up = true;
    rm.frames(3);
    assert!(rm.state::<bool>("round_minimap.shown"), "N on: round minimap back");
    rm.manager.shutdown();
}

fn rm_wheel(steps: f32) {
    use winit::event::{DeviceId, MouseScrollDelta, TouchPhase, WindowEvent};
    super::super::pointer_api::observe(
        &WindowEvent::MouseWheel { device_id: DeviceId::dummy(), delta: MouseScrollDelta::LineDelta(0.0, steps), phase: TouchPhase::Moved },
        RM_SIZE,
    );
}

#[test]
fn round_minimap_rim_arrow_only_where_the_circle_crops_the_square_mark() {
    let Some(mut rm) = Rm::new(false) else { return };
    let world = rm.play.player.world as i32;
    rm.frames(4);
    let (cx, cy, r) = rm.circle();
    let arrows = |rm: &Rm| -> Vec<(f64, f64)> {
        rm.lua().load("local t={} for _,a in ipairs(round_minimap.arrows) do t[#t+1]={a.x,a.y} end return t").eval::<Vec<Vec<f64>>>().unwrap().into_iter().map(|a| (a[0], a[1])).collect()
    };
    assert!(arrows(&rm).is_empty(), "no waypoint: no arrow");
    // a waypoint far off to the south-east: the square picture puts its mark in a corner, which the circle crops
    rm.play.map.round4_waypoint(world, Some([rm.me[0] + 3000.0, rm.me[2] + 3000.0]));
    rm.frames(3);
    let a = arrows(&rm);
    assert_eq!(a.len(), 1, "one rim arrow for the diagonal waypoint: {a:?}");
    let (dx, dy) = (a[0].0 - cx, a[0].1 - cy);
    let ring = r + 6.0 * (rm_stock_rect(&rm)[2] - rm_stock_rect(&rm)[0]) as f64 / 112.0;
    assert!((dx.hypot(dy) - ring).abs() < 1.0, "arrow sits just outside the ring: {} vs {ring}", dx.hypot(dy));
    assert!(dx > 0.0 && dy > 0.0 && (dx / dy - 1.0).abs() < 0.3, "south-east of the centre: {dx} {dy}");
    // control: straight east the square's own rim mark stays inside the circle, so the mod adds none
    rm.play.map.round4_waypoint(world, Some([rm.me[0] + 3000.0, rm.me[2]]));
    rm.frames(3);
    assert!(arrows(&rm).is_empty(), "an axis-aligned waypoint keeps the native rim mark only: {:?}", arrows(&rm));
    // control: a waypoint inside the circle is no rim mark
    rm.play.map.round4_waypoint(world, Some([rm.me[0] + 20.0, rm.me[2] + 20.0]));
    rm.frames(3);
    assert!(arrows(&rm).is_empty());
    rm.manager.shutdown();
}

#[test]
fn round_minimap_carries_the_search_button_and_box_when_the_search_mod_is_on() {
    use winit::keyboard::{Key, NamedKey};
    let Some(mut rm) = Rm::new(true) else { return };
    rm.frames(4);
    assert!(rm.state::<bool>("round_minimap.shown"), "round minimap up beside the search mod");
    super::super::input::route_mod_key(&Key::Named(NamedKey::F2), true, false, false, true, false);
    let t = rm.frames(4);
    let search = lua(&rm.manager, "eqoa_map_search");
    assert!(search.load("return require('search').open").eval::<bool>().unwrap(), "F2 opens the search box over the round minimap (the stock one is hidden)");
    let pixels = r4_pixels(&rm.gpu, &t);
    assert!(pixels.chunks_exact(4).filter(|p| p[0] > 150 && p[1] < 80 && p[2] < 90).count() > 100, "the red search controls render");
    rm_png(&pixels, "r4-minimap-search.png");
    rm.manager.shutdown();
    super::super::input::reset_input_state();
}

/// Per-frame cost in a busy world: all mods off, then the three round 4 mods and round 3's four on.
/// Debug builds only prove the workload ran; the recorded numbers come from `--release`.
#[test]
fn round4c_frame_time_with_all_map_mods_in_a_busy_world() {
    use std::time::Instant;
    const ROUND3: [&str; 4] = ["eqoa_stats_box", "eqoa_target_of_target", "eqoa_mana_bars", "eqoa_hud_numbers"];
    const ROUND4: [&str; 3] = ["eqoa_creature_journal", "eqoa_map_search", "eqoa_round_minimap"];
    let mut results = Vec::new();
    for (label, on) in [("all mods off", false), ("round 3 (4) + round 4 (3) mods on", true)] {
        let Some(mut rm) = Rm::with(|p| on && (ROUND3.contains(&p) || ROUND4.contains(&p)), 200) else { return };
        rm.frames(30);
        if on {
            assert!(rm.manager.entries.iter().all(|e| e.status == Status::Enabled), "{:?}", rm.manager.diagnostics());
            assert_eq!(rm.manager.entries.len(), 7, "all seven mods loaded");
            assert!(rm.state::<bool>("round_minimap.shown"));
            assert!(rm.lua().load("return #eqoa.entities.list()").eval::<usize>().unwrap() >= 200, "200 entities in the world");
            // the pointer rests on the circle's centre, so the hover scan runs every frame
            let (cx, cy, _) = rm.circle();
            rm.at(cx as f32, cy as f32);
        }
        let n = 300;
        let mut whole = Vec::with_capacity(n);
        for _ in 0..n {
            let t = Instant::now();
            let tex = rm.frame();
            whole.push(t.elapsed().as_secs_f64() * 1000.0);
            drop(tex);
        }
        whole.sort_by(f64::total_cmp);
        let mean = whole.iter().sum::<f64>() / n as f64;
        eprintln!(
            "R4C frame time ({}, {label}): frames={n} mean={mean:.3} ms p50={:.3} p95={:.3} max={:.3}",
            if cfg!(debug_assertions) { "debug" } else { "release" },
            whole[n / 2],
            whole[n * 95 / 100],
            whole[n - 1]
        );
        assert_eq!(whole.len(), n, "the workload ran");
        results.push(mean);
        rm.manager.shutdown();
    }
    assert!(results[1].is_finite() && results[0].is_finite());
}
