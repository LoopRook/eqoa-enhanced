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
    /// the window in logical pixels, and the render scale of the target it is drawn to
    win: (u32, u32),
    scale: u32,
    /// the colour of the terrain stand-in the native minimap is drawn over
    stand_in: [f32; 4],
    target: Option<(wgpu::Texture, wgpu::TextureView)>,
    /// between worlds: the frame carries no game, as a zoning player's do
    zoning: bool,
    /// the native HUD layer's pass (the mods' `eqoa.hud.*` commands drawn to the target), when asked for
    hud: Option<RmHud>,
}
/// What the viewer's HUD pass has that the harness did not: the HUD layer drawn through a `Ui` to the GPU.
struct RmHud {
    ui: render::ui::Ui,
    assets: crate::ui::UiAssets,
    /// the disc's atlas and strings, when the disc is there
    real: Option<std::rc::Rc<crate::map::extension::RealUi>>,
    stand_in: crate::ui::Vui,
}
const RM_SIZE: (u32, u32) = (1024, 768);
const RM_GREEN: [f32; 4] = [0.1, 0.6, 0.2, 1.0];
const RM_ORANGE: [f32; 4] = [0.9, 0.45, 0.1, 1.0];
/// the colour the native minimap's dots are drawn in, so a test can find them
const RM_DOT: [f32; 4] = [1.0, 0.2, 1.0, 1.0];
impl Rm {
    fn new(with_search: bool) -> Option<Rm> {
        Rm::with(|p| p == "eqoa_round_minimap" || (with_search && p == "eqoa_map_search"), 0)
    }
    fn with(want: impl Fn(&str) -> bool, extra_entities: usize) -> Option<Rm> {
        Rm::sized(want, extra_entities, RM_SIZE, 1)
    }
    fn sized(want: impl Fn(&str) -> bool, extra_entities: usize, win: (u32, u32), scale: u32) -> Option<Rm> {
        let gpu = Gpu::new()?;
        crate::settings::testing::reset(false);
        crate::hudlayout::reset();
        crate::hudlayout::set_unlocked(false);
        crate::hideui::show();
        super::super::input::reset_input_state();
        super::super::hud_layout::set_window_size(win.0, win.1);
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
        play.entities.slots[2] = Some(rm_creature(3003, "Test Rat", [me[0] + 40.0, me[1], me[2] + 30.0], 0, 7));
        for i in 0..extra_entities {
            let a = i as f32 * 2.399_963;
            let d = 20.0 + (i % 40) as f32 * 5.0;
            let at = [me[0] + a.cos() * d, me[1], me[2] + a.sin() * d];
            play.entities.slots.push(Some(rm_creature(5000 + i as u32, &format!("Busy Creature {i}"), at, 0, 1 + (i % 50) as u8)));
        }
        let manager = gpu.manager(&root.0, Some(config.0.clone()));
        let scene = crate::map::Map::round4_fixture(&gpu.device, &gpu.queue, win, true);
        Some(Rm { gpu, _root: root, _config: config, manager, play, walker, me, scene, draws: 0, win, scale, stand_in: RM_GREEN, target: None, zoning: false, hud: None })
    }
    /// Adds the viewer's HUD pass to every frame: the HUD layer drawn through a `Ui` to the GPU (the disc's
    /// atlas and strings when `$EQOA_ISO` is set; the surfaces and the font are stand-ins).
    fn with_hud(mut self) -> Rm {
        let mut ui = render::ui::Ui::new(&self.gpu.device, render::TARGET_FORMAT, render::DEPTH_FORMAT);
        let assets = super::super::round_3_picture_tests::test_assets(&mut ui, &self.gpu.device, &self.gpu.queue);
        self.hud = Some(RmHud { ui, assets, real: crate::map::extension::real_ui(), stand_in: super::super::round_3_picture_tests::test_vui() });
        self
    }
    /// How many dots the native minimap is drawing.
    fn state_len_dots(&self) -> usize {
        self.mini_view().dots.len()
    }
    /// What the native minimap shows of the entities: a dot per creature but corpses and the inactive.
    fn mini_view(&self) -> crate::map::mini::MiniView {
        let dots: Vec<([f32; 2], [f32; 4])> = self
            .play
            .entities
            .slots
            .iter()
            .flatten()
            .filter(|e| e.p18 & 2 == 0 && e.p19 != 0 && e.creature.is_some())
            .map(|e| ([e.actor_pos[0], e.actor_pos[2]], RM_DOT))
            .collect();
        let marks = crate::map::quests::mini_marks(&self.play.map.quests.marks);
        crate::map::mini::MiniView { dots, target: None, marks }
    }
    /// The target's pixel size: the window at the render scale.
    fn target_size(&self) -> (u32, u32) {
        (self.win.0 * self.scale, self.win.1 * self.scale)
    }
    /// One whole frame: the addons' callbacks, the native minimap draw routed as the viewer
    /// routes it, then the shared ImGui pass over it.
    fn frame(&mut self) -> wgpu::Texture {
        let phase = super::super::addon_perf::section("frame.snapshot");
        let mut snapshot = if self.zoning { Snapshot::default() } else { self.play.forge_snapshot(Some(&self.walker)) };
        snapshot.capture_pending = false;
        drop(phase);
        let phase = super::super::addon_perf::section("frame.native minimap");
        let at = self.walker.cam.walk.phys.position;
        let at = [at.x.0, at.y.0, at.z.0];
        if self.play.map.mini.up {
            let view = self.mini_view();
            self.scene.round4_mini_frame(&self.gpu.device, &self.gpu.queue, self.win, at, &view, self.stand_in);
        } else {
            self.scene.mini_off();
        }
        drop(phase);
        self.manager.flush_pending_fonts(&mut self.gpu.context);
        {
            let io = self.gpu.context.io_mut();
            io.set_display_size([self.win.0 as f32, self.win.1 as f32]);
            io.set_display_framebuffer_scale([self.scale as f32; 2]);
            io.set_delta_time(1. / 60.);
        }
        self.gpu.context.frame();
        super::super::native_core::presentation();
        let phase = super::super::addon_perf::section("frame.manager");
        self.manager.frame(snapshot);
        drop(phase);
        let phase = super::super::addon_perf::section("frame.gpu encode");
        let size = self.target_size();
        let win = self.win;
        let gpu = &mut self.gpu;
        // the render targets are made once, as the viewer keeps its own
        if self.target.as_ref().is_none_or(|(t, _)| (t.width(), t.height()) != size) {
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
            self.target = Some((target, depth));
        }
        let (target, depth) = self.target.as_ref().unwrap();
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
                    view: depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        let before = super::super::map_views::draw_calls();
        crate::fork_ext::map::draw_minimap(&self.scene, &gpu.device, &mut encoder, &view, depth, win, size, wgpu::TextureFormat::Rgba8Unorm);
        self.draws += super::super::map_views::draw_calls() - before;
        drop(phase);
        let phase = super::super::addon_perf::section("frame.hud pass");
        if let Some(h) = &mut self.hud {
            // the viewer's HUD pass: the layer's commands drawn through a `Ui`, to the GPU
            h.ui.begin(win);
            let vui = h.real.as_ref().map_or(&h.stand_in, |r| &r.vui);
            super::super::hud_layer::draw(vui, &mut h.assets.raster(&mut h.ui));
            h.ui.end();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud layer"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            h.ui.draw(&gpu.device, &mut pass, size);
        }
        drop(phase);
        let phase = super::super::addon_perf::section("frame.imgui render");
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
        drop(phase);
        let phase = super::super::addon_perf::section("frame.submit");
        gpu.queue.submit([encoder.finish()]);
        drop(phase);
        let phase = super::super::addon_perf::section("frame.after_render");
        self.manager.after_render();
        drop(phase);
        self.target.as_ref().unwrap().0.clone()
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
        super::super::pointer_api::position([x, y], true, self.win);
    }
    fn circle(&self) -> (f64, f64, f64) {
        (self.state("round_minimap.circle.cx"), self.state("round_minimap.circle.cy"), self.state("round_minimap.circle.r"))
    }
    /// The stock minimap's rectangle in window pixels (x0, y0, x1, y1).
    fn stock_rect(&self) -> [f32; 4] {
        let r = super::super::hud_layout::hud_layout_json(self.win);
        let m = &r["minimap"];
        let f = |k: &str| m[k].as_f64().unwrap() as f32;
        [f("x"), f("y"), f("x") + f("width"), f("y") + f("height")]
    }
    /// Window pixels of world point (x, z) on the round minimap, by the view's own projection.
    fn dot_at(&self, x: f32, z: f32) -> (f64, f64) {
        let lay = self.stock_rect();
        let (kx, ky) = ((lay[2] - lay[0]) as f64 / 112.0, (lay[3] - lay[1]) as f64 / 112.0);
        let (px, py, _): (f64, f64, bool) = self.lua().load(format!("return round_minimap.view.project({x},{z})")).eval().unwrap();
        (lay[0] as f64 + px * kx, lay[1] as f64 + py * ky)
    }
    /// The card's lines as the mod built them, if it shows one.
    fn card_lines(&self) -> Option<Vec<String>> {
        self.lua()
            .load("if not round_minimap.card then return nil end local t={} for _,l in ipairs(round_minimap.card.lines) do t[#t+1]=l[1] end return t")
            .eval()
            .unwrap()
    }
    /// Hover window pixel (x, y) for two frames and give the card's first line.
    fn hover(&mut self, x: f64, y: f64) -> Option<String> {
        self.at(x as f32, y as f32);
        self.frames(2);
        self.card_lines().map(|l| l[0].clone())
    }
    /// Where the player stands: the walker, and with it the minimap's centre.
    fn stand_at(&mut self, x: f32, z: f32) {
        let p = &mut self.walker.cam.walk.phys.position;
        p.x.0 = x;
        p.z.0 = z;
        self.me = [x, self.me[1], z];
    }
    /// Puts creature `e` in the world, in its slot if it has one, and tells the API of it as the
    /// session's packet handling does.
    fn put(&mut self, e: net::entity::Entity) {
        let slots = &mut self.play.entities.slots;
        let slot = match slots.iter().position(|s| s.as_ref().is_some_and(|o| o.id == e.id)) {
            Some(i) => {
                slots[i] = Some(e);
                i
            }
            None => {
                slots.push(Some(e));
                slots.len() - 1
            }
        };
        self.play.entity_slot_changed(slot);
    }
    /// A creature goes, as the session's packet handling tells the API of it.
    fn remove(&mut self, id: u32) {
        let slot = self.play.entities.slots.iter().position(|s| s.as_ref().is_some_and(|o| o.id == id)).unwrap();
        self.play.entities.slots[slot] = None;
        self.play.entity_slot_changed(slot);
    }
}
fn rm_creature(id: u32, name: &str, at: [f32; 3], p256: u8, level: u8) -> net::entity::Entity {
    net::entity::Entity {
        id,
        kind: net::entity::TYPE_CREATURE,
        p19: 255,
        pos: at,
        actor_pos: at,
        creature: Some(net::entity::Creature { name: name.into(), level, p256, ..Default::default() }),
        ..Default::default()
    }
}
fn rm_px(pixels: &[u8], x: f64, y: f64) -> [u8; 4] {
    rm_px_in(pixels, RM_SIZE.0, x, y)
}
fn rm_px_in(pixels: &[u8], width: u32, x: f64, y: f64) -> [u8; 4] {
    let i = ((y as usize) * width as usize + x as usize) * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}
/// A target's pixels at any size (rows padded to the copy alignment).
fn rm_pixels(gpu: &Gpu, target: &wgpu::Texture) -> Vec<u8> {
    let (w, h) = (target.width() as usize, target.height() as usize);
    let row = (w * 4).next_multiple_of(256);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row as u32), rows_per_image: Some(h as u32) } },
        target.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let padded = buffer.slice(..).get_mapped_range().unwrap().to_vec();
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        out.extend_from_slice(&padded[y * row..y * row + w * 4]);
    }
    out
}

#[test]
fn round_minimap_hides_stock_blocks_its_clicks_and_returns_when_off() {
    big_stack(round_minimap_hides_stock_blocks_its_clicks_and_returns_when_off_body)
}
fn round_minimap_hides_stock_blocks_its_clicks_and_returns_when_off_body() {
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

const RM_BG: [u8; 4] = [51, 51, 102, 255];
fn rm_is_picture(p: [u8; 4]) -> bool {
    p[1] > 120 && p[0] < 80 && p[2] < 90
}

#[test]
fn round_minimap_circle_follows_move_hud_and_clips_to_a_circle() {
    big_stack(round_minimap_circle_follows_move_hud_and_clips_to_a_circle_body)
}
fn round_minimap_circle_follows_move_hud_and_clips_to_a_circle_body() {
    use crate::hudlayout::{self, Beyond, Piece};
    let Some(mut rm) = Rm::new(false) else { return };
    rm.frames(4);
    let t = rm.frames(2);
    let pixels = r4_pixels(&rm.gpu, &t);
    rm_png(&pixels, "r4-minimap-outdoors.png");
    assert_eq!(rm.state::<Option<String>>("round_minimap.image_error"), None);
    let (cx, cy, r) = rm.circle();
    let stock = rm.stock_rect();
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
    let moved = rm.stock_rect();
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
    big_stack(round_minimap_dot_card_wheel_zoom_and_n_toggle_body)
}
fn round_minimap_dot_card_wheel_zoom_and_n_toggle_body() {
    let Some(mut rm) = Rm::new(false) else { return };
    rm.frames(4);
    // Test Rat stands 40 east, 30 south of the player
    let (cx, cy, r) = rm.circle();
    let inside: bool = rm.state(&format!("select(3, round_minimap.view.project({}, {}))", rm.me[0] + 40.0, rm.me[2] + 30.0));
    assert!(inside, "the creature projects inside the circle (the view's own answer)");
    let far: bool = rm.state(&format!("select(3, round_minimap.view.project({}, {}))", rm.me[0] + 40_000.0, rm.me[2]));
    assert!(!far, "control: a point far away projects outside it");
    let (dx, dy) = rm.dot_at(rm.me[0] + 40.0, rm.me[2] + 30.0);
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
    big_stack(round_minimap_rim_arrow_only_where_the_circle_crops_the_square_mark_body)
}
fn round_minimap_rim_arrow_only_where_the_circle_crops_the_square_mark_body() {
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
    let ring = r + 6.0 * (rm.stock_rect()[2] - rm.stock_rect()[0]) as f64 / 112.0;
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
    big_stack(round_minimap_carries_the_search_button_and_box_when_the_search_mod_is_on_body)
}
fn round_minimap_carries_the_search_button_and_box_when_the_search_mod_is_on_body() {
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

/// Hover picks from this frame's positions: a creature that moved or went is found where it is
/// now, the nearest dot wins (the lower id on a tie), and the card says what kind it is.
#[test]
fn round_minimap_hover_follows_this_frames_creatures_and_picks_the_nearest_dot() {
    big_stack(round_minimap_hover_follows_this_frames_creatures_and_picks_the_nearest_dot_body)
}
fn round_minimap_hover_follows_this_frames_creatures_and_picks_the_nearest_dot_body() {
    let Some(mut rm) = Rm::new(false) else { return };
    rm.frames(4);
    let me = rm.me;
    let (dx, dy) = rm.dot_at(me[0] + 40.0, me[2] + 30.0);
    // the native dot is drawn where the mod's hit test looks for it
    let t = rm.frames(1);
    let pixels = rm_pixels(&rm.gpu, &t);
    let near = |p: [u8; 4]| p[0] > 200 && p[1] < 120 && p[2] > 200;
    let found = (-3..=3).any(|ox| (-3..=3).any(|oy| near(rm_px(&pixels, dx + ox as f64, dy + oy as f64))));
    assert!(found, "the native dot is drawn under the hover position");
    assert_eq!(rm.hover(dx, dy).as_deref(), Some("Test Rat"));
    // the creature walks on: its old place has no card now, its new place has one, at once
    let (nx, nz) = (me[0] - 30.0, me[2] + 30.0);
    let (mx, my) = rm.dot_at(nx, nz);
    assert!((mx - dx).hypot(my - dy) > 10.0, "the move is wide enough to tell apart");
    rm.put(rm_creature(3003, "Test Rat", [nx, me[1], nz], 0, 7));
    rm.at(dx as f32, dy as f32);
    rm.frames(2);
    assert!(rm.card_lines().is_none(), "no card where the creature was");
    rm.at(mx as f32, my as f32);
    rm.frames(2);
    let lines = rm.card_lines().expect("a card where the creature is");
    assert_eq!(lines[0], "Test Rat");
    assert!(lines[2].starts_with("42 away"), "the distance is to where it stands now: {lines:?}");
    // it goes: no card (a vanished creature is not remembered)
    rm.remove(3003);
    rm.frames(2);
    assert!(rm.card_lines().is_none(), "no card for a creature that left");
    // two dots 3 pixels apart: the nearer to the pointer, whatever the order
    let (ax, ay) = rm.dot_at(me[0] - 20.0, me[2] + 20.0);
    let (bx, by) = rm.dot_at(me[0] - 20.0 + 1.0, me[2] + 20.0);
    let per_px = ((bx - ax).hypot(by - ay)).max(1e-6); // pixels per world unit
    let apart = 3.0 / per_px as f32;
    rm.put(rm_creature(4002, "Second", [me[0] - 20.0 + apart, me[1], me[2] + 20.0], 0, 3));
    rm.put(rm_creature(4001, "First", [me[0] - 20.0, me[1], me[2] + 20.0], 0, 3));
    let (ax, ay) = rm.dot_at(me[0] - 20.0, me[2] + 20.0);
    let (bx, by) = rm.dot_at(me[0] - 20.0 + apart, me[2] + 20.0);
    assert!((bx - ax).hypot(by - ay) > 2.0 && (bx - ax).hypot(by - ay) < 4.0, "3 pixels apart: {}", (bx - ax).hypot(by - ay));
    assert_eq!(rm.hover(ax, ay).as_deref(), Some("First"));
    assert_eq!(rm.hover(bx, by).as_deref(), Some("Second"), "the second dot is nearer to its own place");
    // the same place: the lower id
    rm.put(rm_creature(4900, "Aaa Higher Id", [me[0] + 20.0, me[1], me[2] - 20.0], 0, 9));
    rm.put(rm_creature(4100, "Zzz Lower Id", [me[0] + 20.0, me[1], me[2] - 20.0], 0, 9));
    let (tx, ty) = rm.dot_at(me[0] + 20.0, me[2] - 20.0);
    assert_eq!(rm.hover(tx, ty).as_deref(), Some("Zzz Lower Id"), "equal distance: the lower id");
    // the card per kind: players, pets, NPCs, an unknown level; corpses and the inactive show none
    let at = [me[0] - 30.0, me[1], me[2] - 25.0];
    let (kx, ky) = rm.dot_at(at[0], at[2]);
    for (p256, level, line) in [(2, 45, "Level 45 / Player"), (3, 45, "Level 45 / Pet"), (0, 45, "Level 45 / NPC"), (1, 45, "Level 45 / NPC"), (2, 0, "Level unknown / Player")] {
        rm.put(rm_creature(4500, "Someone", at, p256, level));
        rm.at(kx as f32, ky as f32);
        rm.frames(1);
        assert_eq!(rm.card_lines().expect("a card")[1], line);
    }
    let mut corpse = rm_creature(4500, "Someone", at, 0, 45);
    corpse.p18 = 2;
    rm.put(corpse);
    rm.frames(1);
    assert!(rm.card_lines().is_none(), "a corpse has no dot, so no card");
    let mut inactive = rm_creature(4500, "Someone", at, 0, 45);
    inactive.p19 = 0;
    rm.put(inactive);
    rm.frames(1);
    assert!(rm.card_lines().is_none(), "an inactive creature has no dot, so no card");
    rm.manager.shutdown();
}

/// Moved with Move HUD to a window edge, the card stays on the window (beside the circle on the
/// other side when the near side has no room), long names wrapped.
#[test]
fn round_minimap_card_stays_inside_the_window_wherever_the_minimap_is_moved() {
    big_stack(round_minimap_card_stays_inside_the_window_wherever_the_minimap_is_moved_body)
}
fn round_minimap_card_stays_inside_the_window_wherever_the_minimap_is_moved_body() {
    use crate::hudlayout::{self, Beyond, Piece};
    let Some(mut rm) = Rm::new(false) else { return };
    let long = "A Very Long Server Provided Creature Name That Would Never Fit Beside A Minimap Without Wrapping Onto Several Lines Of Text";
    rm.put(rm_creature(3003, long, [rm.me[0] + 40.0, rm.me[1], rm.me[2] + 30.0], 0, 7));
    let inside = |rm: &Rm, label: &str| {
        let r: [f64; 4] = rm.state("(function() local c = round_minimap.card_rect return {c.x, c.y, c.w, c.h} end)()");
        assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= rm.win.0 as f64 && r[1] + r[3] <= rm.win.1 as f64, "{label}: card {r:?} is on the {:?} window", rm.win);
        r
    };
    rm.frames(4);
    let (dx, dy) = rm.dot_at(rm.me[0] + 40.0, rm.me[2] + 30.0);
    assert!(rm.hover(dx, dy).is_some());
    let home = inside(&rm, "home");
    assert!(home[2] < 400.0, "the long name is wrapped: card {} wide", home[2]);
    // Move HUD: far left and down
    hudlayout::frame_begin();
    hudlayout::note(Piece::Minimap, [474.0, 50.0, 586.0, 162.0], [0.0, 0.0]);
    hudlayout::frame_end();
    hudlayout::set_unlocked(true);
    let b = Beyond::of(RM_SIZE);
    hudlayout::press([530.0, 106.0], b);
    hudlayout::drag_to([-400.0, 900.0], b);
    assert!(hudlayout::release());
    hudlayout::set_unlocked(false);
    rm.frames(4);
    let moved = rm.stock_rect();
    assert!(moved[0] < 60.0, "the minimap sits at the left edge: {moved:?}");
    let (dx, dy) = rm.dot_at(rm.me[0] + 40.0, rm.me[2] + 30.0);
    assert!(rm.hover(dx, dy).is_some(), "a card on the dot at the left edge");
    let edge = inside(&rm, "left edge");
    let (cx, _, r) = rm.circle();
    assert!(edge[0] >= cx + r, "no room on the left: the card goes to the right of the circle ({edge:?}, circle at {cx} r {r})");
    assert!(moved[3] > rm.win.1 as f32 - 130.0, "and it sits low: {moved:?}");
    hudlayout::reset();
    rm.manager.shutdown();
}

/// The marks the hover finds are the marks the minimap draws: Map Markers on, no towns, the
/// topmost of overlapping marks, quest steps on the rim and only where the circle shows them.
#[test]
fn round_minimap_marker_hover_follows_what_the_minimap_draws() {
    big_stack(round_minimap_marker_hover_follows_what_the_minimap_draws_body)
}
fn round_minimap_marker_hover_follows_what_the_minimap_draws_body() {
    use net::session::QuestMark;
    let Some(mut rm) = Rm::new(false) else { return };
    let world = rm.play.player.world as i32;
    let all = crate::map::extension::static_markers(world);
    let first = |kind: &str| all.iter().find(|m| m["kind"] == kind).unwrap_or_else(|| panic!("a {kind} in world {world}")).clone();
    let camp = first("camp");
    let (camp_x, camp_z) = (camp["x"].as_f64().unwrap() as f32, camp["z"].as_f64().unwrap() as f32);
    rm.stand_at(camp_x - 12.0, camp_z - 9.0);
    rm.frames(4);
    let (kx, ky) = rm.dot_at(camp_x, camp_z);
    let (cx, cy, r) = rm.circle();
    assert!((kx - cx).hypot(ky - cy) < r - 6.0, "the camp lies in the circle");
    let label = rm.hover(kx, ky).expect("a card on a camp");
    assert!(label.ends_with("(XP camp)"), "{label}");
    // Map Markers off: the minimap draws no camps, so there is nothing to hover
    crate::settings::set_map_markers(false).unwrap();
    rm.frames(3);
    assert_eq!(rm.hover(kx, ky), None, "no card for a camp the minimap does not draw");
    crate::settings::set_map_markers(true).unwrap();
    rm.frames(3);
    assert!(rm.hover(kx, ky).is_some(), "Map Markers back on: the camp's card is back");
    // a quest to take over the camp is drawn on top of it: its card wins
    rm.play.map.quests.set(&[QuestMark { kind: 1, world: world as u8, x: camp_x, z: camp_z, npc: "Quest Giver".into(), quests: Vec::new() }]);
    rm.frames(3);
    assert_eq!(rm.hover(kx, ky).as_deref(), Some("Quest Giver"), "the quest is the topmost mark");
    // a quest step beyond the picture is drawn on its rim: hover finds it there; a step past a corner
    // is cropped by the circle, an offer beyond the picture is not drawn at all
    let lay = rm.stock_rect();
    let (kx, ky) = ((lay[2] - lay[0]) as f64 / 112.0, (lay[3] - lay[1]) as f64 / 112.0);
    let (x0, y0) = (lay[0] as f64 + 6.0 * kx, lay[1] as f64 + 6.0 * ky);
    let (x1, y1) = (x0 + 100.0 * kx, y0 + 100.0 * ky);
    let mid_y = (y0 + y1) / 2.0;
    let half = 8.0 * ky;
    let step = |x: f32, z: f32| QuestMark { kind: 2, world: world as u8, x, z, npc: "Far Step".into(), quests: Vec::new() };
    rm.play.map.quests.set(&[step(rm.me[0] + 3000.0, rm.me[2])]);
    rm.frames(3);
    assert_eq!(rm.hover(x1 - half * 0.6, mid_y).as_deref(), Some("Far Step"), "the step's rim mark has its card");
    rm.play.map.quests.set(&[step(rm.me[0] + 3000.0, rm.me[2] + 3000.0)]);
    rm.frames(3);
    assert_eq!(rm.hover(x1 - half * 0.6, y1 - half), None, "the corner is outside the circle: not drawn, no card");
    rm.play.map.quests.set(&[QuestMark { kind: 1, world: world as u8, x: rm.me[0] + 3000.0, z: rm.me[2], npc: "Far Offer".into(), quests: Vec::new() }]);
    rm.frames(3);
    assert_eq!(rm.hover(x1 - half * 0.6, mid_y), None, "an offer beyond the picture has no rim mark");
    // a town's name is not drawn on the minimap: standing at one, hovering it gives nothing
    rm.play.map.quests.set(&[]);
    let near = |m: &serde_json::Value, x: f64, z: f64| (m["x"].as_f64().unwrap() - x).hypot(m["z"].as_f64().unwrap() - z) < 60.0;
    let town = all
        .iter()
        .filter(|m| m["kind"] == "town")
        .find(|t| {
            let (x, z) = (t["x"].as_f64().unwrap(), t["z"].as_f64().unwrap());
            !all.iter().any(|m| m["kind"] != "town" && near(m, x, z))
        })
        .expect("a town with no camp or coach beside it")
        .clone();
    let (town_x, town_z) = (town["x"].as_f64().unwrap() as f32, town["z"].as_f64().unwrap() as f32);
    rm.stand_at(town_x - 12.0, town_z - 9.0);
    rm.frames(4);
    let (tx, ty) = rm.dot_at(town_x, town_z);
    assert!((tx - cx).hypot(ty - cy) < r - 6.0, "the town lies in the circle");
    assert_eq!(rm.hover(tx, ty), None, "no card for a town");
    rm.manager.shutdown();
}

/// The picture, the ring, the hover and the pointer claim line up on the window at any aspect and
/// render scale: window coordinates all through. The picture is a synthetic stand-in (no disc is
/// on this machine); the orange ones stand in for an indoor picture, which fills the whole square
/// as terrain does.
#[test]
fn round_minimap_stays_in_window_coordinates_at_aspects_and_render_scales() {
    big_stack(round_minimap_stays_in_window_coordinates_at_aspects_and_render_scales_body)
}
fn round_minimap_stays_in_window_coordinates_at_aspects_and_render_scales_body() {
    let combos: [((u32, u32), u32, [f32; 4]); 6] = [
        ((1024, 768), 1, RM_GREEN),
        ((1024, 768), 2, RM_ORANGE),
        ((640, 480), 1, RM_ORANGE),
        ((640, 480), 4, RM_GREEN),
        ((768, 1024), 2, RM_GREEN),
        ((768, 1024), 1, RM_ORANGE),
    ];
    for (win, scale, color) in combos {
        let Some(mut rm) = Rm::sized(|p| p == "eqoa_round_minimap", 0, win, scale) else { return };
        rm.stand_in = color;
        let note = format!("window {win:?} at {scale}x");
        rm.frames(4);
        let t = rm.frames(2);
        assert_eq!((t.width(), t.height()), (win.0 * scale, win.1 * scale));
        let pixels = rm_pixels(&rm.gpu, &t);
        let px = |x: f64, y: f64| rm_px_in(&pixels, t.width(), x * scale as f64, y * scale as f64);
        let (cx, cy, r) = rm.circle();
        let k = r / 50.0;
        let picture = |p: [u8; 4]| {
            let c = color.map(|c| (c * 255.0) as i32);
            (0..3).all(|i| (p[i] as i32 - c[i]).abs() < 12)
        };
        assert_eq!(rm.state::<Option<String>>("round_minimap.image_error"), None, "{note}");
        for (sx, sy) in [(0.5, 0.5), (-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5)] {
            assert!(picture(px(cx + sx * r, cy + sy * r)), "{note}: picture at ({sx}, {sy}) r {:?}", px(cx + sx * r, cy + sy * r));
        }
        // the picture's square corners are cut by the circle, and the ring sits at its edge
        let s = std::f64::consts::FRAC_1_SQRT_2;
        for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            assert_eq!(px(cx + sx * r * 0.95, cy + sy * r * 0.95), RM_BG, "{note}: corner ({sx}, {sy}) cut off");
            let ring = px(cx + sx * s * (r + 1.5 * k), cy + sy * s * (r + 1.5 * k));
            assert!((0..3).all(|i| (ring[i] as i32 - 51).abs() < 14), "{note}: ring at the diagonal ({sx}, {sy}): {ring:?}");
        }
        for (sx, sy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let ring = px(cx + sx * (r + 1.5 * k), cy + sy * (r + 1.5 * k));
            assert!((0..3).all(|i| (ring[i] as i32 - 51).abs() < 14), "{note}: ring on the axis ({sx}, {sy}): {ring:?}");
            assert!(picture(px(cx + sx * (r - 2.0 * k), cy + sy * (r - 2.0 * k))), "{note}: picture up to the ring ({sx}, {sy})");
            assert_eq!(px(cx + sx * (r + 5.0 * k), cy + sy * (r + 5.0 * k)), RM_BG, "{note}: nothing outside the ring");
        }
        // the hover and the claim work in the same window pixels
        let (dx, dy) = rm.dot_at(rm.me[0] + 40.0, rm.me[2] + 30.0);
        let dot = |p: [u8; 4]| p[0] > 200 && p[1] < 120 && p[2] > 200;
        assert!((-3..=3).any(|ox| (-3..=3).any(|oy| dot(px(dx + ox as f64, dy + oy as f64)))), "{note}: the native dot is where the hit test looks");
        assert_eq!(rm.hover(dx, dy).as_deref(), Some("Test Rat"), "{note}");
        let card: [f64; 4] = rm.state("(function() local c = round_minimap.card_rect return {c.x, c.y, c.w, c.h} end)()");
        assert!(card[0] >= 0.0 && card[1] >= 0.0 && card[0] + card[2] <= win.0 as f64 && card[1] + card[3] <= win.1 as f64, "{note}: card {card:?} in the window");
        assert_eq!(rm.hover(cx + r * 0.98 * s + 2.0, cy + r * 0.98 * s + 2.0), None, "{note}: nothing past the circle");
        rm.manager.shutdown();
    }
}

/// A waypoint cropped by the circle gets a rim arrow in its own colour: a personal one magenta, the
/// group's orange as on the stock minimap, whichever corner.
#[test]
fn round_minimap_rim_arrows_keep_the_waypoint_colours() {
    big_stack(round_minimap_rim_arrows_keep_the_waypoint_colours_body)
}
fn round_minimap_rim_arrows_keep_the_waypoint_colours_body() {
    let Some(mut rm) = Rm::new(false) else { return };
    let world = rm.play.player.world as i32;
    rm.frames(4);
    rm.play.map.round4_waypoint(world, Some([rm.me[0] + 3000.0, rm.me[2] + 3000.0]));
    rm.play.map.round4_group_waypoint(world, [rm.me[0] - 3000.0, rm.me[2] + 3000.0]);
    let t = rm.frames(3);
    let pixels = rm_pixels(&rm.gpu, &t);
    let arrows: Vec<(String, f64, f64)> = rm
        .lua()
        .load("local t={} for _,a in ipairs(round_minimap.arrows) do t[#t+1]={a.kind,a.x,a.y} end return t")
        .eval::<Vec<mlua::Table>>()
        .unwrap()
        .into_iter()
        .map(|a| (a.get(1).unwrap(), a.get(2).unwrap(), a.get(3).unwrap()))
        .collect();
    assert_eq!(arrows.len(), 2, "{arrows:?}");
    for (kind, x, y) in arrows {
        let p = rm_px(&pixels, x, y).map(|c| c as i32);
        match kind.as_str() {
            "waypoint" => assert!(p[0] > 150 && p[2] > 130 && p[1] * 2 < p[2], "a waypoint's arrow is magenta: {p:?}"),
            "group_waypoint" => assert!(p[0] > 150 && p[1] * 10 > p[0] * 3 && p[1] * 10 < p[0] * 7 && p[2] * 5 < p[1], "the group's arrow is orange: {p:?}"),
            other => panic!("{other}"),
        }
    }
    // the pointer claim stays inside the circle: nowhere just outside it does the minimap take the pointer
    let (cx, cy, r) = rm.circle();
    for deg in (0..360).step_by(5) {
        let a = (deg as f64).to_radians();
        let at = |rr: f64| [(cx + a.cos() * rr) as f32, (cy + a.sin() * rr) as f32];
        assert!(!rm_claims(&rm, at(r + 1.5)), "a click just outside the circle at {deg} degrees goes through");
        assert!(rm_claims(&rm, at(r * 0.8)), "a click inside the circle at {deg} degrees is taken");
    }
    rm.manager.shutdown();
}

/// Whether the pointer at `at` is taken by a claim of the mods (the game does not get the click).
fn rm_claims(rm: &Rm, at: [f32; 2]) -> bool {
    use winit::dpi::PhysicalPosition;
    use winit::event::{DeviceId, WindowEvent};
    super::super::pointer_api::observe(&WindowEvent::CursorMoved { device_id: DeviceId::dummy(), position: PhysicalPosition::new(at[0] as f64, at[1] as f64) }, rm.win)
}

/// Per-frame cost in a busy world: the stock square minimap with no mods as the native baseline,
/// then the mods. A frame is the whole of it: the native minimap's layout and drawing with 200 dots,
/// the capture the round minimap asks for, the addons' callbacks and the ImGui pass, to the GPU's
/// completion. Debug builds only prove the workload ran (`R4C_SAMPLES` sets the frames timed per
/// configuration, 300 by default); the recorded numbers come from `--release`.
#[test]
fn round4c_frame_time_with_all_map_mods_in_a_busy_world() {
    big_stack(round4c_frame_time_with_all_map_mods_in_a_busy_world_body)
}
fn round4c_frame_time_with_all_map_mods_in_a_busy_world_body() {
    use std::time::Instant;
    // the disc's own HUD art when `$EQOA_ISO` is set (the frame, the font): the real minimap chrome
    let real_art = crate::map::extension::use_real_ui(true);
    eprintln!("R4C art: {}", if real_art { "the disc's UI.CSF and SUPPORT atlas" } else { "stand-in (no disc)" });
    const ROUND3: [&str; 4] = ["eqoa_stats_box", "eqoa_target_of_target", "eqoa_mana_bars", "eqoa_hud_numbers"];
    const ROUND4: [&str; 3] = ["eqoa_creature_journal", "eqoa_map_search", "eqoa_round_minimap"];
    let mut results = Vec::new();
    let configs: [(&str, Vec<&str>); 8] = [
        ("stock square minimap, no mods (native baseline)", vec![]),
        ("round 3 mods (4)", ROUND3.to_vec()),
        ("creature journal", vec!["eqoa_creature_journal"]),
        ("journal + map search", vec!["eqoa_creature_journal", "eqoa_map_search"]),
        ("round minimap, pointer away", vec!["eqoa_round_minimap"]),
        ("round minimap, pointer on circle", vec!["eqoa_round_minimap"]),
        ("round 4 mods (3)", ROUND4.to_vec()),
        ("round 3 (4) + round 4 (3)", ROUND3.iter().chain(ROUND4.iter()).copied().collect()),
    ];
    for (label, packages) in configs {
        let on = packages.contains(&"eqoa_round_minimap");
        let Some(mut rm) = Rm::with(|p| packages.contains(&p), 200) else { return };
        rm.frames(30);
        // the native minimap is drawing the busy world's dots in every configuration
        assert!(rm.mini_view().dots.len() >= 200, "200 dots for the native minimap");
        rm.draws = 0;
        rm.frame();
        assert_eq!(rm.draws, 1, "the native minimap is drawn once a frame ({label})");
        if on {
            assert!(rm.state::<bool>("round_minimap.shown"));
            assert!(rm.lua().load("return #eqoa.entities.list()").eval::<usize>().unwrap() >= 200, "200 entities in the world");
            // the pointer rests on the circle's centre, so the hover scan runs every frame
            let (cx, cy, _) = rm.circle();
            if !label.contains("away") {
                rm.at(cx as f32, cy as f32);
            }
        }
        assert!(rm.manager.entries.iter().all(|e| e.status == Status::Enabled), "{:?}", rm.manager.diagnostics());
        assert_eq!(rm.manager.entries.len(), packages.len(), "every wanted mod loaded");
        let n: usize = std::env::var("R4C_SAMPLES").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
        let mut whole = Vec::with_capacity(n);
        let (mut cpu, mut wait) = (0.0f64, 0.0f64);
        for _ in 0..n {
            let t = Instant::now();
            let tex = rm.frame();
            let submitted = t.elapsed().as_secs_f64() * 1000.0;
            rm.gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let total = t.elapsed().as_secs_f64() * 1000.0;
            (cpu, wait) = (cpu + submitted, wait + (total - submitted));
            whole.push(total);
            drop(tex);
        }
        whole.sort_by(f64::total_cmp);
        let mean = whole.iter().sum::<f64>() / n as f64;
        eprintln!(
            "R4C frame time ({}, {label}): frames={n} mean={mean:.3} ms (CPU to submit {:.3}, then GPU wait {:.3}) p50={:.3} p95={:.3} max={:.3}",
            if cfg!(debug_assertions) { "debug" } else { "release" },
            cpu / n as f64,
            wait / n as f64,
            whole[n / 2],
            whole[(n * 95 / 100).min(n - 1)],
            whole[n - 1]
        );
        assert_eq!(whole.len(), n, "the workload ran");
        results.push(mean);
        rm.manager.shutdown();
    }
    assert!(results.iter().all(|m| m.is_finite()));
    crate::map::extension::use_real_ui(false);
}

/// Every pixel in the picture's last two pixels inside the circle, round its whole edge but the
/// "N" at the top and the player's dart (which sit inside it), is the terrain's: no part of
/// upstream's square frame shows in the circle.
fn rm_frame_free(rm: &Rm, pixels: &[u8], width: u32, scale: u32, note: &str) -> Vec<String> {
    let (cx, cy, r) = rm.circle();
    let k = r / 50.0;
    let c = rm.stand_in.map(|c| (c * 255.0) as i32);
    let picture = |p: [u8; 4]| (0..3).all(|i| (p[i] as i32 - c[i]).abs() < 12);
    let mut bad = Vec::new();
    for deg in (0..360).step_by(3) {
        // the "N" sits at the top
        if (deg as i32 - 270).abs() < 14 {
            continue;
        }
        let a = (deg as f64).to_radians();
        for inset in [1.0, 2.0, 4.0] {
            let rr = r - inset * k;
            let (x, y) = ((cx + a.cos() * rr) * scale as f64, (cy + a.sin() * rr) * scale as f64);
            let p = rm_px_in(pixels, width, x, y);
            if !picture(p) {
                bad.push(format!("{note}: {deg} deg, {inset} px in: {p:?}"));
            }
        }
    }
    bad
}

/// With the disc's own frame art (`$EQOA_ISO`; a SKIP without it) the round minimap shows only the
/// map picture: none of upstream's square frame inside the circle. The control reads the stock
/// square minimap, whose frame the same check must find.
#[test]
fn round_minimap_shows_no_square_frame_inside_the_circle_with_the_discs_art() {
    big_stack(round_minimap_shows_no_square_frame_inside_the_circle_with_the_discs_art_body)
}
fn round_minimap_shows_no_square_frame_inside_the_circle_with_the_discs_art_body() {
    if !crate::map::extension::use_real_ui(true) {
        return;
    }
    let Some(mut rm) = Rm::new(false) else {
        crate::map::extension::use_real_ui(false);
        return;
    };
    rm.frames(4);
    let t = rm.frames(2);
    let pixels = rm_pixels(&rm.gpu, &t);
    rm_png(&pixels, "r4-minimap-real-art.png");
    let bad = rm_frame_free(&rm, &pixels, t.width(), 1, "outdoors");
    // control: the stock square's own edge (the frame) is found by the same sampling, one picture
    // pixel past where the circle ends, at the four tangent points
    let (cx, cy, _r) = rm.circle();
    let stock = rm.stock_rect().map(f64::from);
    let is_frame = |x: f64, y: f64| !rm_is_picture(rm_px(&pixels, x, y));
    let control = [(stock[0] + 2.0, cy), (stock[2] - 2.0, cy), (cx, stock[1] + 2.0), (cx, stock[3] - 2.0)];
    let found = control.iter().filter(|(x, y)| is_frame(*x, *y)).count();
    crate::map::extension::use_real_ui(false);
    rm.manager.shutdown();
    assert!(found >= 3, "control: the frame's own edge is not picture ({found}/4) ");
    assert!(bad.is_empty(), "frame pixels inside the circle: {bad:?}");
}

/// The mod's wheel forwarding loses no notch: each one over the circle reaches the native minimap
/// as one whole step (a 1.25 change of zoom), the wheel runs the whole of upstream's range
/// (2 to 25 world units a frame pixel outdoors) and stops at its ends, and several notches within
/// one frame all arrive. Indoors takes the same forwarded value through the same `mini_wheel`
/// with its own limits (`indoor::SCALE_LIMITS`), which a test cannot enter without upstream's
/// private indoor state: not covered here.
#[test]
fn round_minimap_wheel_reaches_both_ends_of_upstreams_zoom_in_whole_steps() {
    big_stack(round_minimap_wheel_reaches_both_ends_of_upstreams_zoom_in_whole_steps_body)
}
fn round_minimap_wheel_reaches_both_ends_of_upstreams_zoom_in_whole_steps_body() {
    use crate::map::mini::SCALE_LIMITS;
    let Some(mut rm) = Rm::new(false) else { return };
    rm.frames(4);
    let (cx, cy, _) = rm.circle();
    rm.at(cx as f32, cy as f32);
    let mut notch = |rm: &mut Rm, n: f32| -> f32 {
        rm_wheel(n);
        rm.frames(1);
        let got = super::super::pointer_api::take_wheel();
        assert_eq!(got, n, "one notch of {n} arrives whole");
        rm.scene.mini_wheel(got);
        rm.scene.mini.scale
    };
    let start = rm.scene.mini.scale;
    // in: every notch is one step until the limit takes it
    let mut now = start;
    let mut in_notches = 0;
    while now > SCALE_LIMITS.0 {
        let next = notch(&mut rm, 1.0);
        in_notches += 1;
        assert!((next - (now / 1.25).max(SCALE_LIMITS.0)).abs() < 1e-4, "notch {in_notches}: {now} -> {next}");
        now = next;
        assert!(in_notches < 20, "the zoom never reached its limit");
    }
    assert_eq!(now, SCALE_LIMITS.0, "zoomed all the way in");
    assert_eq!(in_notches, 5, "6 units a pixel to the limit of 2 is five notches (control: four are not enough)");
    assert!(start / 1.25f32.powi(4) > SCALE_LIMITS.0, "control: one notch short is still above the limit");
    // out: all the way, the limit holds
    let mut out_notches = 0;
    while now < SCALE_LIMITS.1 {
        let next = notch(&mut rm, -1.0);
        out_notches += 1;
        assert!((next - (now * 1.25).min(SCALE_LIMITS.1)).abs() < 1e-3, "out notch {out_notches}: {now} -> {next}");
        now = next;
        assert!(out_notches < 30, "the zoom never reached its limit");
    }
    assert_eq!((now, out_notches), (SCALE_LIMITS.1, 12));
    assert_eq!(notch(&mut rm, -1.0), SCALE_LIMITS.1, "one past the limit changes nothing");
    // several notches in one frame all arrive, as one request
    for _ in 0..5 {
        rm_wheel(1.0);
    }
    rm.frames(1);
    assert_eq!(super::super::pointer_api::take_wheel(), 5.0, "five notches in a frame");
    rm.manager.shutdown();
}

/// One bundled mod through the real Manager on Play-shaped data: world entry, the minimap hidden
/// and shown, Move HUD, the map open and closed, zoning, disable and enable, and a stretch of
/// plain frames after each. Returns what the Manager recorded as diagnostics.
fn rm_session(packages: &[&str]) -> Vec<String> {
    use crate::hudlayout::{self, Beyond, Piece};
    let want: Vec<String> = packages.iter().map(|p| p.to_string()).collect();
    let Some(mut rm) = Rm::with(|p| want.iter().any(|w| w == p), 0) else { return vec!["SKIP".into()] };
    rm.frames(6);
    // the minimap hidden (N), then shown
    rm.play.map.mini.up = false;
    rm.frames(4);
    rm.play.map.mini.up = true;
    rm.frames(4);
    // Move HUD: drag the minimap and put it back
    hudlayout::frame_begin();
    hudlayout::note(Piece::Minimap, [474.0, 50.0, 586.0, 162.0], [0.0, 0.0]);
    hudlayout::frame_end();
    hudlayout::set_unlocked(true);
    let b = Beyond::of(RM_SIZE);
    hudlayout::press([530.0, 106.0], b);
    hudlayout::drag_to([470.0, 156.0], b);
    hudlayout::release();
    hudlayout::set_unlocked(false);
    rm.frames(4);
    hudlayout::reset();
    rm.frames(2);
    // the pointer over the minimap and a wheel notch
    let r = rm.stock_rect();
    rm.at((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
    rm_wheel(1.0);
    rm.frames(3);
    // the map open, then closed
    rm.play.map.open = true;
    rm.frames(4);
    rm.play.map.open = false;
    rm.frames(3);
    // zoning: no game for a while, then the world again
    rm.zoning = true;
    rm.frames(3);
    rm.zoning = false;
    rm.frames(5);
    // disable and enable, each mod
    for id in packages.iter().filter(|p| **p != "resources") {
        rm.manager.queue(Command::Enable(id.to_string(), false));
        rm.frames(3);
        rm.manager.queue(Command::Enable(id.to_string(), true));
        rm.frames(4);
    }
    let found: Vec<String> = rm.manager.diagnostics().iter().map(|d| format!("{}: [{}] {}", d.id.as_deref().unwrap_or("-"), d.phase, d.message)).collect();
    rm.manager.shutdown();
    hudlayout::reset();
    found
}

/// Every bundled mod, alone, runs that session with the Manager recording nothing.
#[test]
fn every_bundled_mod_runs_a_play_session_with_zero_manager_diagnostics() {
    big_stack(every_bundled_mod_runs_a_play_session_with_zero_manager_diagnostics_body)
}
fn every_bundled_mod_runs_a_play_session_with_zero_manager_diagnostics_body() {
    let mut packages: Vec<&str> = crate::fork_ext::bundled::EMBEDDED_FILES.iter().map(|f| f.package).collect();
    packages.sort();
    packages.dedup();
    packages.retain(|p| *p != "resources");
    assert_eq!(packages.len(), 12, "the bundled set (nine eqoa_* mods and the three Forge UIs): {packages:?}");
    let mut failed = Vec::new();
    let mut ran = 0;
    for p in &packages {
        // the map search reads the journal mod's saved file
        let with: Vec<&str> = if *p == "eqoa_map_search" { vec![p, "eqoa_creature_journal", "resources"] } else { vec![p, "resources"] };
        let found = rm_session(&with);
        if found == ["SKIP"] {
            eprintln!("SKIP zero-diagnostics gate: no graphics device");
            return;
        }
        ran += 1;
        if !found.is_empty() {
            failed.push(format!("{p}: {found:?}"));
        }
    }
    assert_eq!(ran, packages.len());
    assert!(failed.is_empty(), "Manager diagnostics in a Play-shaped session:\n{}", failed.join("\n"));
}

/// The control: a bad call from a mod (the empty rectangle the round minimap used to claim) is
/// recorded by the same Manager, so the gate above would have failed on it.
#[test]
fn the_zero_diagnostics_gate_fails_on_a_deliberately_bad_call() {
    big_stack(the_zero_diagnostics_gate_fails_on_a_deliberately_bad_call_body)
}
fn the_zero_diagnostics_gate_fails_on_a_deliberately_bad_call_body() {
    let Some(mut rm) = Rm::new(false) else { return };
    rm.frames(4);
    assert!(rm.manager.diagnostics().is_empty(), "clean first: {:?}", rm.manager.diagnostics());
    rm.lua().load("local ok = eqoa.input.claim_pointer({ x = 0, y = 0, width = 0, height = 0 }) assert(ok == nil)").exec().unwrap();
    rm.frames(3);
    assert!(
        rm.manager.diagnostics().iter().any(|d| d.message.contains("claim_pointer")),
        "a bad claim is recorded: {:?}",
        rm.manager.diagnostics().iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    rm.manager.shutdown();
}

/// The search button and box are the native search's size, in HUD frame pixels scaled as the HUD
/// is: a 20 square button, a 252 by 300 panel (the native search's `panel`), at every window size
/// and render scale. The control: the sizes the mod drew before (a fixed 20 and 336 by 400) are
/// not these at a window where the HUD scale is not one.
#[test]
fn round_minimap_search_button_and_box_keep_the_native_sizes_at_every_scale() {
    big_stack(round_minimap_search_button_and_box_keep_the_native_sizes_at_every_scale_body)
}
fn round_minimap_search_button_and_box_keep_the_native_sizes_at_every_scale_body() {
    use winit::keyboard::{Key, NamedKey};
    for (win, scale) in [((640u32, 480u32), 1u32), ((1024, 768), 1), ((1024, 768), 2), ((1920, 1080), 1)] {
        let Some(mut rm) = Rm::sized(|p| p == "eqoa_round_minimap" || p == "eqoa_map_search", 0, win, scale) else { return };
        rm.frames(4);
        let search = lua(&rm.manager, "eqoa_map_search");
        let get = |expr: &str| -> f64 { search.load(format!("return {expr}")).eval().unwrap() };
        let k = rm.stock_rect()[3] as f64 - rm.stock_rect()[1] as f64;
        let k = k / 112.0;
        let note = format!("window {win:?} at {scale}x (k {k:.3})");
        assert!((get("require('search').button_rect.w") - 20.0 * k).abs() < 0.6, "{note}: button width");
        assert!((get("require('search').button_rect.h") - 20.0 * k).abs() < 0.6, "{note}: button height");
        super::super::input::route_mod_key(&Key::Named(NamedKey::F2), true, false, false, true, false);
        rm.frames(4);
        assert!(search.load("return require('search').open").eval::<bool>().unwrap(), "{note}: opened");
        let (w, h) = (get("require('search').panel_rect.w"), get("require('search').panel_rect.h"));
        assert!((w - 252.0 * k).abs() < 0.6, "{note}: panel width {w}");
        assert!((h - (300.0 * k).min(win.1 as f64 - 70.0 * k)).abs() < 0.6, "{note}: panel height {h}");
        if k != 1.0 && win.1 as f64 - 70.0 * k >= 300.0 * k {
            assert!((w - 336.0).abs() > 1.0 && (h - 400.0).abs() > 1.0, "{note}: control: not the old fixed size");
        }
        rm.manager.shutdown();
        super::super::input::reset_input_state();
    }
}

/// A creature name that is one word wider than any card (no space to wrap at) is cut with an
/// ellipsis: the card stays narrower than its limit and on the window. The control: the same name
/// with spaces wraps whole and needs no ellipsis.
#[test]
fn round_minimap_card_cuts_a_single_oversized_word_with_an_ellipsis() {
    big_stack(round_minimap_card_cuts_a_single_oversized_word_with_an_ellipsis_body)
}
fn round_minimap_card_cuts_a_single_oversized_word_with_an_ellipsis_body() {
    let Some(mut rm) = Rm::new(false) else { return };
    let word = "Supercalifragilisticexpialidocious".repeat(8);
    rm.put(rm_creature(3003, &word, [rm.me[0] + 40.0, rm.me[1], rm.me[2] + 30.0], 0, 7));
    rm.frames(4);
    let (dx, dy) = rm.dot_at(rm.me[0] + 40.0, rm.me[2] + 30.0);
    assert!(rm.hover(dx, dy).is_some(), "a card on the dot");
    let card: [f64; 4] = rm.state("(function() local c = round_minimap.card_rect return {c.x, c.y, c.w, c.h} end)()");
    let (_, _, r) = rm.circle();
    let k = r / 50.0;
    assert!(card[2] <= 260.0 * k + 16.0 + 1.0, "the card is no wider than its limit: {card:?}");
    assert!(card[0] >= 0.0 && card[0] + card[2] <= rm.win.0 as f64, "and it is on the window: {card:?}");
    let drawn: Vec<String> = rm.state("round_minimap.card_text");
    let first = drawn[0].clone();
    assert!(first.ends_with("..."), "the word is cut with an ellipsis: {first}");
    assert!(first.len() < word.len());
    // control: spaced words wrap and keep every word
    rm.put(rm_creature(3003, &"Supercali ".repeat(30), [rm.me[0] + 40.0, rm.me[1], rm.me[2] + 30.0], 0, 7));
    rm.frames(3);
    assert!(rm.hover(dx, dy).is_some());
    let spaced: Vec<String> = rm.state("round_minimap.card_text");
    assert!(spaced.iter().take(spaced.len() - 2).all(|l| !l.ends_with("...")), "wrapped words are not cut: {spaced:?}");
    rm.manager.shutdown();
}

/// Move HUD places the round minimap where the player put it at render scales 2x and 4x too: the
/// circle follows the moved stock rectangle, the picture is there and no longer at the old place.
#[test]
fn round_minimap_follows_move_hud_at_render_scales_two_and_four() {
    big_stack(round_minimap_follows_move_hud_at_render_scales_two_and_four_body)
}
fn round_minimap_follows_move_hud_at_render_scales_two_and_four_body() {
    use crate::hudlayout::{self, Beyond, Piece};
    for (win, scale) in [((1024u32, 768u32), 2u32), ((640, 480), 4), ((1024, 768), 4)] {
        let Some(mut rm) = Rm::sized(|p| p == "eqoa_round_minimap", 0, win, scale) else { return };
        let note = format!("window {win:?} at {scale}x");
        rm.frames(4);
        let (ox, oy, r) = rm.circle();
        let home = rm.stock_rect();
        hudlayout::frame_begin();
        hudlayout::note(Piece::Minimap, [474.0, 50.0, 586.0, 162.0], [0.0, 0.0]);
        hudlayout::frame_end();
        hudlayout::set_unlocked(true);
        let b = Beyond::of(win);
        hudlayout::press([530.0, 106.0], b);
        hudlayout::drag_to([420.0, 200.0], b);
        assert!(hudlayout::release(), "{note}: the drag took");
        hudlayout::set_unlocked(false);
        rm.frames(4);
        let t = rm.frames(2);
        let moved = rm.stock_rect();
        assert!((moved[0] - home[0]).abs() > 20.0 && (moved[1] - home[1]).abs() > 20.0, "{note}: Move HUD moved the stock rectangle {home:?} -> {moved:?}");
        let (nx, ny, nr) = rm.circle();
        assert!((nx - (moved[0] + moved[2]) as f64 / 2.0).abs() < 1.0 && (ny - (moved[1] + moved[3]) as f64 / 2.0).abs() < 1.0, "{note}: the circle follows the rectangle");
        assert!((nr - r).abs() < 0.5, "{note}: same size");
        let pixels = rm_pixels(&rm.gpu, &t);
        let px = |x: f64, y: f64| rm_px_in(&pixels, t.width(), x * scale as f64, y * scale as f64);
        assert!(rm_is_picture_of(px(nx + nr * 0.5, ny + nr * 0.5), rm.stand_in), "{note}: the picture at the new place");
        assert_eq!(px(ox + r * 0.5, oy + r * 0.5), RM_BG, "{note}: nothing at the old place");
        hudlayout::reset();
        rm.manager.shutdown();
    }
}
fn rm_is_picture_of(p: [u8; 4], color: [f32; 4]) -> bool {
    let c = color.map(|c| (c * 255.0) as i32);
    (0..3).all(|i| (p[i] as i32 - c[i]).abs() < 12)
}

/// `EQOA_PERF`: the addon timing line names each enabled addon with its Lua, its native calls and
/// the shared ImGui pass, and the costliest `eqoa.*` calls beneath. With the timers off (no
/// `EQOA_PERF`) nothing is timed or stored.
#[test]
fn addon_perf_line_splits_each_addons_lua_from_its_native_calls() {
    big_stack(addon_perf_line_splits_each_addons_lua_from_its_native_calls_body)
}
fn addon_perf_line_splits_each_addons_lua_from_its_native_calls_body() {
    super::super::addon_perf::force_for_test(false);
    let Some(mut rm) = Rm::with(|p| p == "eqoa_round_minimap" || p == "eqoa_creature_journal", 50) else { return };
    rm.frames(8);
    assert!(super::super::addon_perf::is_empty_for_test(), "control: with EQOA_PERF unset nothing is timed or stored");
    assert!(super::super::addon_perf::take_report().is_empty());
    rm.manager.shutdown();
    drop(rm);
    super::super::addon_perf::force_for_test(true);
    let Some(mut rm) = Rm::with(|p| p == "eqoa_round_minimap" || p == "eqoa_creature_journal", 50) else { return };
    rm.frames(8);
    super::super::addon_perf::force_for_test(true);
    for _ in 0..20 {
        rm.frame();
    }
    let lines = super::super::addon_perf::take_report();
    let all = lines.join("\n");
    assert!(lines[0].starts_with("addons: "), "{all}");
    for id in ["eqoa_round_minimap", "eqoa_creature_journal"] {
        assert!(lines[0].contains(&format!("{id} ")), "{id} is named: {all}");
    }
    assert!(lines.iter().any(|l| l.starts_with("addon-api: eqoa_round_minimap ")), "its native calls are named: {all}");
    assert!(lines[0].contains("lua ") && lines[0].contains("api ") && lines[0].contains("bridge ") && lines[0].contains("imgui "), "{all}");
    assert!(lines.iter().any(|l| l.contains("hud.layout ")), "the minimap's layout call shows: {all}");
    // the mean is per frame: the numbers are plausible milliseconds, not totals
    let imgui: f64 = lines[0].rsplit("imgui ").next().unwrap().trim().parse().unwrap();
    assert!((0.0..200.0).contains(&imgui), "{all}");
    rm.manager.shutdown();
    super::super::addon_perf::force_for_test(false);
}
