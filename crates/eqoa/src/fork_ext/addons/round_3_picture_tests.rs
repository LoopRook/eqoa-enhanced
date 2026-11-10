// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 3 Picture tests and per-frame cost benchmark for the four HUD extra mods.
//!
//! Each mod (`eqoa_stats_box`, `eqoa_target_of_target`, `eqoa_mana_bars`,
//! `eqoa_hud_numbers`) is rendered through the native HUD layer into a real
//! `render::ui::Ui` and read back as RGBA pixels, then compared against the
//! native drawing of the same game-shaped state at two window sizes
//! (640x480 and 1280x720). The native reference reproduces the develop
//! (`dde6ecc`) drawing: `statbox.rs`'s frame/columns, `Hud::draw_target_of_target`,
//! the group/target mana gauges and `fork_ext/hud.rs`'s readouts. Without a GPU
//! adapter the test prints SKIP and returns.

use super::*;
use render::ui::{Anchor, Vertical};
use crate::ui::Layout;
use vi::ui::wide;
use std::path::{Path, PathBuf};
use std::time::Instant;
use crate::fork_ext::addons::host::Host;

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        static CNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = CNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("eqoa_test_tree_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn bundled_tree() -> TestDir {
    let root = TestDir::new();
    for file in crate::fork_ext::bundled::EMBEDDED_FILES {
        let path = root.path().join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.data).unwrap();
    }
    root
}

fn layer_with_device() -> Option<(render::ui::Ui, render::ui::Ui, render::wgpu::Device, render::wgpu::Queue)> {
    let instance = render::wgpu::Instance::new(render::wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&render::wgpu::RequestAdapterOptions::default())).ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).ok()?;
    let ui_native = render::ui::Ui::new(&device, render::TARGET_FORMAT, render::DEPTH_FORMAT);
    let ui_mod = render::ui::Ui::new(&device, render::TARGET_FORMAT, render::DEPTH_FORMAT);
    Some((ui_native, ui_mod, device, queue))
}

fn test_assets(ui: &mut render::ui::Ui, device: &render::wgpu::Device, queue: &render::wgpu::Queue) -> crate::ui::UiAssets {
    let mut assets = crate::ui::UiAssets::default();
    assets.add_white(ui, device, queue);
    let tex = ui.add_texture(device, queue, 1, 1, &[255; 4]);
    assets.surfaces.insert(vi::ui::icon_id("(null)"), tex);
    assets.surfaces.insert(vi::ui::icon_id(""), tex);
    assets.surfaces.insert(vi::ui::icon_id("sheet1"), tex);
    assets.surfaces.insert(vi::ui::icon_id("Patch_Art"), tex);
    for id in [7, 8, 9, 25, 26, 61, 136] {
        assets.surfaces.insert(id, tex);
    }
    let glyphs = (32..=126)
        .map(|c| esf::font::Glyph {
            code: c as u16,
            width: 12,
            rgba: vec![255; 12 * 21 * 4],
        })
        .collect();
    let font = esf::font::Font {
        id: 1,
        height: 21,
        glyphs,
    };
    assets.font = Some(ui.add_font(device, queue, &font));
    assets
}

/// Renders the layer's quads into an RGBA buffer the size of the target.
fn render_ui_to_pixels(
    ui: &render::ui::Ui,
    device: &render::wgpu::Device,
    queue: &render::wgpu::Queue,
    size: (u32, u32),
) -> Vec<u8> {
    let (width, height) = size;
    let target = device.create_texture(&render::wgpu::TextureDescriptor {
        label: Some("round3 pixel target"),
        size: render::wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: render::wgpu::TextureDimension::D2,
        format: render::TARGET_FORMAT,
        usage: render::wgpu::TextureUsages::RENDER_ATTACHMENT | render::wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let depth = device.create_texture(&render::wgpu::TextureDescriptor {
        label: Some("round3 depth"),
        size: render::wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: render::wgpu::TextureDimension::D2,
        format: render::DEPTH_FORMAT,
        usage: render::wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&render::wgpu::RenderPassDescriptor {
            label: Some("round3 ui"),
            color_attachments: &[Some(render::wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: render::wgpu::Operations {
                    load: render::wgpu::LoadOp::Clear(render::wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
                    store: render::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(render::wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(render::wgpu::Operations { load: render::wgpu::LoadOp::Clear(0.0), store: render::wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        ui.draw(device, &mut pass, size);
    }
    let buffer = device.create_buffer(&render::wgpu::BufferDescriptor {
        label: Some("round3 readback"),
        size: (width * height * 4) as u64,
        usage: render::wgpu::BufferUsages::COPY_DST | render::wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        render::wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: render::wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(render::wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    device.poll(render::wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    buffer.slice(..).get_mapped_range().unwrap().to_vec()
}

/// Counts pixels whose any channel differs by more than `tol` (0..255).
fn pixels_differ(a: &[u8], b: &[u8], tol: u8) -> (usize, usize) {
    assert_eq!(a.len(), b.len(), "pixel buffers must be the same size");
    let total = a.len() / 4;
    let tol = tol as i32;
    let mut diff = 0;
    for (x, y) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        if (x[0] as i32 - y[0] as i32).abs() > tol
            || (x[1] as i32 - y[1] as i32).abs() > tol
            || (x[2] as i32 - y[2] as i32).abs() > tol
            || (x[3] as i32 - y[3] as i32).abs() > tol
        {
            diff += 1;
        }
    }
    (diff, total)
}

fn save_png(name: &str, size: (u32, u32), pixels: &[u8]) {
    // Only capture the review reference when explicitly asked; a normal test
    // run must never write into the source tree (CI's release check requires
    // a clean worktree).
    if std::env::var_os("EQOA_R3_CAPTURE").is_none() {
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/round3-captures");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("r3-native-{name}-{}x{}.png", size.0, size.1));
    if let Ok(f) = std::fs::File::create(&path) {
        let mut encoder = png::Encoder::new(f, size.0, size.1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        if let Ok(mut w) = encoder.write_header() {
            let _ = w.write_image_data(pixels);
        }
    }
}

const TOT_AT: [i32; 2] = [20, 300];
const TOT_WIDTH: i32 = 174;
const GROUP_AT: [i32; 2] = [20, 93];
const TARGET_AT: [i32; 2] = [20, 348];

fn test_vui() -> crate::ui::Vui {
    let mut atlas = vec![vi::ui::UiTexEntry::default(); 256];
    atlas[7].uv1 = [136, 6];
    atlas[7].x00 = 7;
    atlas[8].uv1 = [136, 6];
    atlas[8].x00 = 8;
    atlas[9].uv1 = [136, 6];
    atlas[9].x00 = 9;
    atlas[25].uv1 = [16, 26];
    atlas[26].uv1 = [142, 26];
    atlas[61].uv1 = [128, 128];
    atlas[136].uv1 = [136, 6];
    atlas[136].x00 = 136;
    let mut words = vi::strtab::StringTable::parse(b"dummy\n").unwrap();
    for (id, s) in [
        (226, "Str"), (227, "Sta"), (228, "Agi"), (229, "Dex"),
        (230, "Wis"), (231, "Int"), (232, "Cha"), (242, "AC"),
        (234, "HPMax"), (496, "POW"), (447, "FR"), (451, "CR"),
        (450, "LR"), (452, "AR"), (448, "PR"), (449, "DR"),
    ] {
        words.set(id, 0, s);
    }
    crate::ui::Vui::new(words, Default::default(), atlas)
}

fn test_view() -> crate::fork_ext::player_stats::View {
    let mut v = crate::fork_ext::player_stats::View::default();
    for col in 0..2 {
        for row in 0..8 {
            v.rows[col][row] = crate::fork_ext::player_stats::Row {
                word: crate::fork_ext::player_stats::ROWS[col][row].0,
                value: 50 + col as i32 * 10 + row as i32 * 5,
                without: Some((50, 50)),
            };
        }
    }
    v
}

const STATBOX_FRAME: [i32; 4] = [200, 54, 348, 186];
const STATBOX_BORDER: [f32; 2] = [7.0, 9.0];
const STATBOX_PAD: [f32; 2] = [3.0, 2.0];
const STATBOX_GAP: f32 = 6.0;
const STATBOX_LEADING: f32 = 0.08;
const STATBOX_SCALE: f32 = 0.6;
const STATBOX_ROOM: &str = "8888";
const STATBOX_LABEL: [f32; 4] = [0.95, 0.82, 0.5, 1.0];
const STATBOX_SHADOW: [f32; 4] = [0.0, 0.0, 0.0, 0.85];
const STATBOX_BACK: [f32; 4] = [0.05, 0.06, 0.08, 0.85];

struct StatboxRows {
    lefts: [f32; 2],
    rights: [f32; 2],
    top: f32,
    step: f32,
    scale: f32,
}

fn statbox_layout(font_h: f32, wide: [f32; 2]) -> StatboxRows {
    let (x0, y0, x1, y1) = (STATBOX_FRAME[0] as f32, STATBOX_FRAME[1] as f32, STATBOX_FRAME[2] as f32, STATBOX_FRAME[3] as f32);
    let (left, right) = (x0 + STATBOX_BORDER[0] + STATBOX_PAD[0], x1 - STATBOX_BORDER[0] - STATBOX_PAD[0]);
    let (top, bottom) = (y0 + STATBOX_BORDER[1] + STATBOX_PAD[1], y1 - STATBOX_BORDER[1] - STATBOX_PAD[1]);
    let font_h = font_h.max(1.0);
    let by_height = (bottom - top) / (8.0 * font_h * (1.0 + STATBOX_LEADING));
    let by_width = (right - left - STATBOX_GAP) / (wide[0] + wide[1]).max(1.0);
    let scale = STATBOX_SCALE.min(by_height).min(by_width);
    let step = (bottom - top) / 8.0;
    StatboxRows { lefts: [left, right - wide[1] * scale], rights: [left + wide[0] * scale, right], top: top + (step - font_h * scale) / 2.0, step, scale }
}

fn statbox_widths(labels: &[[Vec<u16>; 8]; 2], width: impl Fn(&[u16]) -> f32) -> [f32; 2] {
    let room = width(&wide(&format!(" {STATBOX_ROOM}")));
    std::array::from_fn(|c| labels[c].iter().map(|l| width(l)).fold(0.0, f32::max) + room)
}

fn native_statbox_draw(vui: &crate::ui::Vui, ui: &mut crate::ui::UiRaster, v: &crate::fork_ext::player_stats::View) {
    ui.ui.anchor = Anchor::Left;
    ui.ui.vertical = Vertical::Top;
    let f = STATBOX_FRAME;
    let corners = |r: [i32; 4]| [[r[0] as f32, r[1] as f32], [r[2] as f32, r[1] as f32], [r[2] as f32, r[3] as f32], [r[0] as f32, r[3] as f32]];
    let inside = [f[0] + 2, f[1] + 2, f[2] - 2, f[3] - 2];
    ui.solid(3, STATBOX_BACK, corners(inside));
    vui.stretch_blit_ex(ui, 61, [inside[0], inside[1]], [inside[2] - inside[0], inside[3] - inside[1]], 3, 0.6);
    vui.blit_frame(ui, [f[0], f[1]], [f[2] - f[0], f[3] - f[1]], &vi::ui::frame_sets()[0], 3, 1.0, true);
    let Some(font) = ui.assets.font else { return };
    let labels: [[Vec<u16>; 8]; 2] = std::array::from_fn(|c| std::array::from_fn(|k| wide(vui.words.get(v.rows[c][k].word, vui.language))));
    let l = {
        let f = &ui.ui.fonts[font];
        statbox_layout(f.height as f32, statbox_widths(&labels, |s| f.string_width(s) as f32))
    };
    for (col, rows) in v.rows.iter().enumerate() {
        for (k, row) in rows.iter().enumerate() {
            let y = (l.top + k as f32 * l.step).round();
            let value = wide(&row.value.to_string());
            let w = ui.ui.fonts[font].string_width(&value) as f32 * l.scale;
            let x = (l.rights[col] - w).round();
            for (s, at, c) in [(&labels[col][k], l.lefts[col].round(), STATBOX_LABEL), (&value, x, row.color())] {
                ui.ui.text_scaled(font, at + 1.0, y + 1.0, l.scale, STATBOX_SHADOW, s);
                ui.ui.text_scaled(font, at, y, l.scale, c, s);
            }
        }
    }
}

fn native_hud_numbers_draw(vui: &crate::ui::Vui, ui: &mut crate::ui::UiRaster, s: &crate::hud::HudState) {
    let _ = vui;
    let Some(font) = ui.assets.font else { return };
    let (anchor, vertical) = (ui.ui.anchor, ui.ui.vertical);
    ui.ui.anchor = Anchor::Left;

    let mut labels = Vec::new();
    if s.has_self {
        let pct = (s.self_hp as f32 / 255.0 * 100.0).round() as i32;
        labels.push((format!("{pct}%"), [99, 35], 0.6, [1.0; 4], Vertical::Top));
        labels.push((format!("{}/{}", s.power, s.max_power), [99, 45], 0.6, [0.4, 0.7, 1.0, 1.0], Vertical::Top));
        let need = crate::hud::exp_for_level(s.level + 1);
        let xp_pct = if need > 0 { (s.exp as f64 / need as f64 * 100.0 * 10.0).round() / 10.0 } else { 0.0 };
        labels.push((format!("{}/{} ({xp_pct:.1}%)", s.exp, need), [99, 55], 0.6, [1.0, 0.85, 0.35, 1.0], Vertical::Top));
    }
    if s.target_name.is_some() {
        let pct = (s.target_hp as f32 / 255.0 * 100.0).round() as i32;
        let color = if pct > 50 {
            [0.4, 1.0, 0.4, 1.0]
        } else if pct > 20 {
            [1.0, 0.9, 0.3, 1.0]
        } else {
            [1.0, 0.3, 0.3, 1.0]
        };
        labels.push((format!("{pct}%"), [TARGET_AT[0] + 40, TARGET_AT[1] + 49], 0.65, color, Vertical::Bottom));
    }

    for (text_str, center, scale, color, vert) in labels {
        ui.ui.vertical = vert;
        let text = wide(&text_str);
        let width = ui.ui.fonts[font].string_width(&text) as f32 * scale;
        let x = (center[0] as f32 - width / 2.0).round();
        let y = center[1] as f32;
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            ui.ui.text_scaled(font, x + dx, y + dy, scale, [0.0, 0.0, 0.0, 0.95], &text);
        }
        ui.ui.text_scaled(font, x, y, scale, color, &text);
    }

    ui.ui.anchor = anchor;
    ui.ui.vertical = vertical;
}

/// Asserts the two same-sized RGBA buffers match within a small tolerance.
fn assert_pixels_match(name: &str, size: (u32, u32), native: &[u8], modded: &[u8]) {
    let (diff, total) = pixels_differ(native, modded, 8);
    assert!(
        (diff as f64) < (total as f64) * 0.02,
        "{name}: mod and native pixels differ at {size:?} ({diff}/{total} pixels over tolerance)"
    );
    assert!(
        native.iter().any(|&b| b != 0),
        "{name}: native reference must draw non-blank pixels at {size:?}"
    );
}

#[test]
fn bundled_stats_box_picture_comparison() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut ui_native, mut ui_mod, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    let assets_native = test_assets(&mut ui_native, &device, &queue);
    let assets_mod = test_assets(&mut ui_mod, &device, &queue);

    let tree = bundled_tree();
    let addon = discovery::discover(tree.path())
        .addons
        .into_iter()
        .find(|a| a.id == "eqoa_stats_box")
        .expect("eqoa_stats_box must be discovered");

    let host = Host::for_addon(&addon).unwrap();
    let vui = test_vui();
    let view = test_view();

    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player_stats = Some(view.to_json());
    });

    host.run_addon(&addon).unwrap();
    host.execute("eqoa.options.set('stats_box', true)").unwrap();

    for size in [(640, 480), (1280, 720)] {
        ui_native.begin(size);
        native_statbox_draw(&vui, &mut assets_native.raster(&mut ui_native), &view);
        ui_native.end();
        let native_pixels = render_ui_to_pixels(&ui_native, &device, &queue, size);
        save_png("stats_box", size, &native_pixels);

        ui_mod.begin(size);
        hud_layer::clear_all();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let mod_pixels = render_ui_to_pixels(&ui_mod, &device, &queue, size);

        assert_pixels_match("stats_box", size, &native_pixels, &mod_pixels);

        // Negative control: a disabled option draws nothing (blank).
        ui_mod.begin(size);
        hud_layer::clear_all();
        host.execute("eqoa.options.set('stats_box', false)").unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let blank = render_ui_to_pixels(&ui_mod, &device, &queue, size);
        assert!(
            !blank.iter().any(|&b| b != 0),
            "negative control: disabled stats box draws no pixels"
        );
        host.execute("eqoa.options.set('stats_box', true)").unwrap();
    }
}

#[test]
fn bundled_target_of_target_picture_comparison() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut ui_native, mut ui_mod, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    let assets_native = test_assets(&mut ui_native, &device, &queue);
    let assets_mod = test_assets(&mut ui_mod, &device, &queue);

    let tree = bundled_tree();
    let addon = discovery::discover(tree.path())
        .addons
        .into_iter()
        .find(|a| a.id == "eqoa_target_of_target")
        .expect("eqoa_target_of_target must be discovered");

    let host = Host::for_addon(&addon).unwrap();
    let vui = test_vui();

    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = serde_json::json!({ "id": 1, "target_id": 2 });
        c.view.entities = vec![
            serde_json::json!({ "id": 2, "name": "Casil", "target_id": 3, "kind": "npc" }),
            serde_json::json!({ "id": 3, "name": "Luck", "hp_fraction": 0.8, "power": 0.5, "kind": "player" }),
        ];
    });

    host.run_addon(&addon).unwrap();
    host.execute("eqoa.options.set('target_of_target', true); eqoa.options.set('mana_bars', true)").unwrap();

    for size in [(640, 480), (1280, 720)] {
        // Native drawing: the ToT bar, its name and gauges (develop's draw_target_of_target).
        ui_native.begin(size);
        {
            let mut r = assets_native.raster(&mut ui_native);
            r.ui.vertical = Vertical::Bottom;
            let at = TOT_AT;
            vui.blit_bar(&mut r, &[], [0.0; 4], at, 25, 26, TOT_WIDTH, 3, 1.0, 0, true);
            let mut w = vi::ui::Window::default();
            w.init(&vui, &mut r, [at[0] + TOT_WIDTH / 2, at[1] + 12], TOT_WIDTH - 14, 1, 0);
            w.open(true);
            w.text_box(&vui, &mut r, &vi::ui::wide("Luck"), 0, 0, TOT_WIDTH - 14, 1, [0.0, 0.0, 0.0, 1.0], 10);
            crate::hud::draw_gauge(&vui, &mut r, 7, 136, [at[0] + 10, at[1] + 25], true, 0.8);
            crate::hud::draw_gauge(&vui, &mut r, 9, 136, [at[0] + 10, at[1] + 35], true, 0.5);
        }
        ui_native.end();
        let native_pixels = render_ui_to_pixels(&ui_native, &device, &queue, size);
        save_png("target_of_target", size, &native_pixels);

        ui_mod.begin(size);
        hud_layer::clear_all();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let mod_pixels = render_ui_to_pixels(&ui_mod, &device, &queue, size);

        assert_pixels_match("target_of_target", size, &native_pixels, &mod_pixels);

        // Negative control: disabled option draws nothing.
        ui_mod.begin(size);
        hud_layer::clear_all();
        host.execute("eqoa.options.set('target_of_target', false)").unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let blank = render_ui_to_pixels(&ui_mod, &device, &queue, size);
        assert!(!blank.iter().any(|&b| b != 0), "negative control: disabled ToT draws no pixels");
        host.execute("eqoa.options.set('target_of_target', true)").unwrap();
    }
}

#[test]
fn bundled_mana_bars_picture_comparison() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut ui_native, mut ui_mod, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    let assets_native = test_assets(&mut ui_native, &device, &queue);
    let assets_mod = test_assets(&mut ui_mod, &device, &queue);

    let tree = bundled_tree();
    let addon = discovery::discover(tree.path())
        .addons
        .into_iter()
        .find(|a| a.id == "eqoa_mana_bars")
        .expect("eqoa_mana_bars must be discovered");

    let host = Host::for_addon(&addon).unwrap();
    let vui = test_vui();

    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = serde_json::json!({ "id": 1, "target_id": 2 });
        c.view.entities = vec![
            serde_json::json!({ "id": 2, "name": "Boss", "hp_fraction": 0.5, "power": 0.6, "kind": "npc" }),
        ];
        c.view.group = serde_json::json!({
            "in_group": true,
            "members": [
                { "id": 10, "name": "Healer", "power": 0.9 },
                { "id": 11, "name": "Tank", "power": 0.4 },
            ]
        });
    });

    host.run_addon(&addon).unwrap();
    host.execute("eqoa.options.set('mana_bars', true)").unwrap();

    for size in [(640, 480), (1280, 720)] {
        // Native drawing: power gauges for 2 group members and 1 target.
        ui_native.begin(size);
        {
            let mut r = assets_native.raster(&mut ui_native);
            crate::hud::draw_gauge(&vui, &mut r, 9, 136, [GROUP_AT[0] + 10, GROUP_AT[1] + 35], false, 0.9);
            crate::hud::draw_gauge(&vui, &mut r, 9, 136, [GROUP_AT[0] + 10, GROUP_AT[1] + 48 + 35], false, 0.4);
            r.ui.vertical = Vertical::Bottom;
            crate::hud::draw_gauge(&vui, &mut r, 9, 136, [TARGET_AT[0] + 10, TARGET_AT[1] + 60], true, 0.6);
        }
        ui_native.end();
        let native_pixels = render_ui_to_pixels(&ui_native, &device, &queue, size);
        save_png("mana_bars", size, &native_pixels);

        ui_mod.begin(size);
        hud_layer::clear_all();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let mod_pixels = render_ui_to_pixels(&ui_mod, &device, &queue, size);

        assert_pixels_match("mana_bars", size, &native_pixels, &mod_pixels);

        // Negative control: disabled option draws nothing.
        ui_mod.begin(size);
        hud_layer::clear_all();
        host.execute("eqoa.options.set('mana_bars', false)").unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let blank = render_ui_to_pixels(&ui_mod, &device, &queue, size);
        assert!(!blank.iter().any(|&b| b != 0), "negative control: disabled mana bars draw no pixels");
        host.execute("eqoa.options.set('mana_bars', true)").unwrap();
    }
}

#[test]
fn bundled_hud_numbers_picture_comparison() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut ui_native, mut ui_mod, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    let assets_native = test_assets(&mut ui_native, &device, &queue);
    let assets_mod = test_assets(&mut ui_mod, &device, &queue);

    let tree = bundled_tree();
    let addon = discovery::discover(tree.path())
        .addons
        .into_iter()
        .find(|a| a.id == "eqoa_hud_numbers")
        .expect("eqoa_hud_numbers must be discovered");

    let host = Host::for_addon(&addon).unwrap();
    let vui = test_vui();

    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = serde_json::json!({
            "id": 1,
            "target_id": 2,
            "level": 25,
            "experience": 1500,
            "hp": { "current": 255, "maximum": 255 },
            "power": { "current": 80, "maximum": 100 },
        });
        c.view.entities = vec![
            serde_json::json!({ "id": 2, "name": "Orc", "hp_fraction": 0.6, "kind": "npc" }),
        ];
    });

    host.run_addon(&addon).unwrap();
    host.execute("eqoa.options.set('hud_numbers', true); eqoa.options.set('target_hp_percent', true); eqoa.options.set('xp_details', true)").unwrap();

    for size in [(640, 480), (1280, 720)] {
        ui_native.begin(size);
        {
            let s = crate::hud::HudState {
                has_self: true,
                self_hp: 255,
                power: 80,
                max_power: 100,
                level: 25,
                exp: 1500,
                target_name: Some("Orc".into()),
                target_hp: 153,
                ..Default::default()
            };
            native_hud_numbers_draw(&vui, &mut assets_native.raster(&mut ui_native), &s);
        }
        ui_native.end();
        let native_pixels = render_ui_to_pixels(&ui_native, &device, &queue, size);
        save_png("hud_numbers", size, &native_pixels);

        ui_mod.begin(size);
        hud_layer::clear_all();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let mod_pixels = render_ui_to_pixels(&ui_mod, &device, &queue, size);

        assert_pixels_match("hud_numbers", size, &native_pixels, &mod_pixels);

        // Negative control: disabled options draw nothing.
        ui_mod.begin(size);
        hud_layer::clear_all();
        host.execute("eqoa.options.set('hud_numbers', false); eqoa.options.set('target_hp_percent', false); eqoa.options.set('xp_details', false)").unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let blank = render_ui_to_pixels(&ui_mod, &device, &queue, size);
        assert!(!blank.iter().any(|&b| b != 0), "negative control: disabled numbers draw no pixels");
        host.execute("eqoa.options.set('hud_numbers', true); eqoa.options.set('target_hp_percent', true); eqoa.options.set('xp_details', true)").unwrap();
    }
}

#[test]
fn round_3_all_four_mods_per_frame_performance_benchmark() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let (mut ui, device, queue) = {
        let instance = render::wgpu::Instance::new(render::wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&render::wgpu::RequestAdapterOptions::default())).ok();
        let Some((d, q)) = adapter.and_then(|a| pollster::block_on(a.request_device(&Default::default())).ok()) else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        (render::ui::Ui::new(&d, render::TARGET_FORMAT, render::DEPTH_FORMAT), d, q)
    };
    let assets = test_assets(&mut ui, &device, &queue);
    let vui = test_vui();

    let tree = bundled_tree();
    let discovered = discovery::discover(tree.path());

    let mod_ids = ["eqoa_stats_box", "eqoa_target_of_target", "eqoa_mana_bars", "eqoa_hud_numbers"];
    let mut hosts = Vec::new();

    for id in mod_ids {
        let addon = discovered.addons.iter().find(|a| a.id == id).expect("bundled mod must exist");
        let host = Host::for_addon(addon).unwrap();
        host.run_addon(addon).unwrap();
        hosts.push(host);
    }

    let view = test_view();
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player_stats = Some(view.to_json());
        c.view.player = serde_json::json!({
            "id": 1,
            "target_id": 2,
            "level": 25,
            "experience": 1500,
            "hp": { "current": 255, "maximum": 255, "fraction": 1.0 },
            "power": { "current": 80, "maximum": 100, "fraction": 0.8 },
        });
        c.view.entities = vec![
            serde_json::json!({ "id": 2, "name": "Casil", "target_id": 3, "hp_fraction": 0.6, "power": 0.4, "kind": "npc" }),
            serde_json::json!({ "id": 3, "name": "Luck", "hp_fraction": 0.8, "power": 0.5, "kind": "player" }),
        ];
        c.view.group = serde_json::json!({
            "in_group": true,
            "members": [
                { "id": 10, "name": "Healer", "power": 0.9 },
                { "id": 11, "name": "Tank", "power": 0.4 },
            ]
        });
    });

    let iterations = 1000;
    let size = (1280, 720);
    let start = Instant::now();

    // Time the full per-frame cost of the four mods: Lua render callbacks
    // (which enqueue commands) plus the native HUD-layer draw (which drains
    // and draws them).
    for _ in 0..iterations {
        hud_layer::clear_all();
        for h in &hosts {
            super::native_events::render(h).unwrap();
        }
        ui.begin(size);
        hud_layer::draw(&vui, &mut assets.raster(&mut ui));
        ui.end();
    }

    let elapsed = start.elapsed();
    let per_frame_micros = elapsed.as_micros() as f64 / iterations as f64;
    let per_frame_millis = per_frame_micros / 1000.0;

    println!(
        "Round 3 per-frame cost for all 4 bundled mods: {:.3} ms/frame ({:.1} µs/frame) across {} iterations",
        per_frame_millis, per_frame_micros, iterations
    );

    if cfg!(not(debug_assertions)) {
        assert!(
            per_frame_millis < 0.5,
            "per-frame cost of 4 mods ({:.3} ms) must be well under 0.5 ms/frame in release mode",
            per_frame_millis
        );
    } else {
        assert!(
            per_frame_millis < 5.0,
            "per-frame cost of 4 mods ({:.3} ms) in debug mode should be reasonable (< 5ms)",
            per_frame_millis
        );
    }
}
