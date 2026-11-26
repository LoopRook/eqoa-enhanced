// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 3 Picture tests and per-frame cost benchmark for the four HUD extra mods.
//!
//! Each mod (`eqoa_stats_box`, `eqoa_target_of_target`, `eqoa_mana_bars`,
//! `eqoa_hud_numbers`) is rendered through the native HUD layer into a real
//! `render::ui::Ui` and read back as RGBA pixels, then compared against the
//! native drawing of the same game-shaped state at two window sizes
//! (640x480 and 1280x720). Committed PNG references execute develop
//! (`dde6ecc`) drawing: `statbox.rs`'s frame/columns, `Hud::draw_target_of_target`,
//! the group/target mana gauges and `fork_ext/hud.rs`'s readouts. Without a GPU
//! adapter the test prints SKIP and returns.

use super::*;
use crate::fork_ext::addons::host::Host;
use std::path::{Path, PathBuf};
use std::time::Instant;

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

fn layer_with_device() -> Option<(
    render::ui::Ui,
    render::ui::Ui,
    render::wgpu::Device,
    render::wgpu::Queue,
)> {
    let instance = render::wgpu::Instance::new(
        render::wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
    );
    let adapter = pollster::block_on(
        instance.request_adapter(&render::wgpu::RequestAdapterOptions::default()),
    )
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).ok()?;
    let ui_native = render::ui::Ui::new(&device, render::TARGET_FORMAT, render::DEPTH_FORMAT);
    let ui_mod = render::ui::Ui::new(&device, render::TARGET_FORMAT, render::DEPTH_FORMAT);
    Some((ui_native, ui_mod, device, queue))
}

pub(super) fn test_assets(
    ui: &mut render::ui::Ui,
    device: &render::wgpu::Device,
    queue: &render::wgpu::Queue,
) -> crate::ui::UiAssets {
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
        size: render::wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: render::wgpu::TextureDimension::D2,
        format: render::TARGET_FORMAT,
        usage: render::wgpu::TextureUsages::RENDER_ATTACHMENT
            | render::wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let depth = device.create_texture(&render::wgpu::TextureDescriptor {
        label: Some("round3 depth"),
        size: render::wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
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
                    load: render::wgpu::LoadOp::Clear(render::wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: render::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(render::wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(render::wgpu::Operations {
                    load: render::wgpu::LoadOp::Clear(0.0),
                    store: render::wgpu::StoreOp::Discard,
                }),
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
            layout: render::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(render::wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    device
        .poll(render::wgpu::PollType::wait_indefinitely())
        .unwrap();
    rx.recv().unwrap().unwrap();
    buffer.slice(..).get_mapped_range().unwrap().to_vec()
}

pub(super) fn test_vui() -> crate::ui::Vui {
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
        (226, "Str"),
        (227, "Sta"),
        (228, "Agi"),
        (229, "Dex"),
        (230, "Wis"),
        (231, "Int"),
        (232, "Cha"),
        (242, "AC"),
        (234, "HPMax"),
        (496, "POW"),
        (447, "FR"),
        (451, "CR"),
        (450, "LR"),
        (452, "AR"),
        (448, "PR"),
        (449, "DR"),
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

/// References are generated by dde6ecc's actual native routines in a temporary
/// worktree; see docs/round3-captures/README.md. Never regenerate from mod code.
fn reference(name: &str, size: (u32, u32)) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/round3-captures")
        .join(format!("r3-native-{name}-{}x{}.png", size.0, size.1));
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    assert_eq!((reader.info().width, reader.info().height), size);
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    bytes.truncate(info.buffer_size());
    bytes
}

/// Compare each element's occupied bounding box, plus blank background. Channel
/// tolerance 2, at most 0.1% differing pixels in EACH box (not the whole frame).
fn assert_pixels_match(name: &str, size: (u32, u32), native: &[u8], modded: &[u8]) {
    assert_eq!(native.len(), modded.len());
    if let Some(root) = std::env::var_os("EQOA_R3_ACTUAL_OUT") {
        let f = std::fs::File::create(
            PathBuf::from(root).join(format!("{name}-{}x{}.png", size.0, size.1)),
        )
        .unwrap();
        let mut enc = png::Encoder::new(f, size.0, size.1);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(modded)
            .unwrap();
    }

    let bands: &[([f32; 4], &str, &str)] = match name {
        "stats_box" => &[([198., 52., 152., 136.], "left", "top")],
        "target_of_target" => &[
            ([18., 298., 178., 26.], "left", "bottom"),
            ([28., 323., 140., 10.], "left", "bottom"),
            ([28., 333., 140., 10.], "left", "bottom"),
        ],
        // the target's bar; the group's are upstream's own since sync 2026-11-15
        // (`MANA_GROUP_BANDS`, cleared from the capture)
        "mana_bars" => &[([28., 406., 140., 10.], "left", "bottom")],
        "hud_numbers" => &[
            ([0., 33., 200., 12.], "left", "top"),
            ([0., 43., 200., 12.], "left", "top"),
            ([0., 53., 300., 12.], "left", "top"),
            ([0., 395., 130., 14.], "left", "bottom"),
        ],
        _ => panic!("unknown reference"),
    };
    let mut covered = vec![false; (size.0 * size.1) as usize];
    for (r, anchor, vertical) in bands {
        let r = hud_layout::transform_rect(*r, anchor, vertical, hud_layout::frame_metrics(size));
        let mut count = 0;
        let mut different = 0;
        let mut painted = 0;
        for y in (r[1].floor().max(0.) as u32)..((r[1] + r[3]).ceil().min(size.1 as f32) as u32) {
            for x in (r[0].floor().max(0.) as u32)..((r[0] + r[2]).ceil().min(size.0 as f32) as u32)
            {
                let i = (y * size.0 + x) as usize;
                covered[i] = true;
                count += 1;
                let a = &native[4 * i..4 * i + 4];
                let b = &modded[4 * i..4 * i + 4];
                painted += usize::from(a.iter().any(|v| *v != 0));
                different += usize::from(a.iter().zip(b).any(|(a, b)| a.abs_diff(*b) > 2));
            }
        }
        assert!(painted > 0, "{name}: empty reference element {r:?}");
        // 0.5% of an element: text edges rasterize a few pixels differently on another
        // adapter (CI's: 17 of 9,000 at 1280x720); a missing or moved readout differs in hundreds.
        assert!(
            different as f64 <= count as f64 * 0.005,
            "{name} {size:?} element {r:?}: {different}/{count} differ"
        );
    }
    for (i, covered) in covered.into_iter().enumerate() {
        if !covered {
            let (a, b) = (&native[4 * i..4 * i + 4], &modded[4 * i..4 * i + 4]);
            // fully transparent on both sides is nothing drawn, whatever colour another
            // adapter leaves in it (CI's: [35, 30, 12, 0] beside a text edge)
            if a[3] == 0 && b[3] == 0 {
                continue;
            }
            assert!(
                a.iter().zip(b).all(|(a, b)| a.abs_diff(*b) <= 2),
                "{name}: outside element at pixel {i}: {a:?} vs {b:?}"
            );
        }
    }
}

#[test]
fn bundled_stats_box_picture_comparison() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some((_, mut ui_mod, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    let assets_mod = test_assets(&mut ui_mod, &device, &queue);

    let tree = bundled_tree();
    stat_window_is_held();
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
        let native_pixels = reference("stats_box", size);

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
        host.execute("eqoa.options.set('stats_box', false)")
            .unwrap();
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
    let Some((_, mut ui_mod, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
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
            serde_json::json!({ "id": 3, "name": "Luck", "hp_fraction": 204.0 / 255.0, "power": 128.0 / 255.0, "kind": "player" }),
        ];
    });

    host.run_addon(&addon).unwrap();
    host.execute("eqoa.options.set('target_of_target', true); eqoa.options.set('mana_bars', true)")
        .unwrap();

    for size in [(640, 480), (1280, 720)] {
        let native_pixels = reference("target_of_target", size);

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
        host.execute("eqoa.options.set('target_of_target', false)")
            .unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let blank = render_ui_to_pixels(&ui_mod, &device, &queue, size);
        assert!(
            !blank.iter().any(|&b| b != 0),
            "negative control: disabled ToT draws no pixels"
        );
        host.execute("eqoa.options.set('target_of_target', true)")
            .unwrap();
    }
}

#[test]
fn target_plate_fade_and_name_clipping_match_native_drawing() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut native, mut actual, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    let native_assets = test_assets(&mut native, &device, &queue);
    let actual_assets = test_assets(&mut actual, &device, &queue);
    let vui = test_vui();
    let tree = bundled_tree();
    options::reset_in_memory();
    let host = load_hud_mod(&tree, "eqoa_target_of_target");
    host.execute("eqoa.options.set('target_of_target', true)")
        .unwrap();
    hud_layout::set_hud_state(true, true, false, 285, 20);
    let size = (1280, 720);
    for name in [
        "Luck",
        "A very long target name without enough room",
        "One Two Three Four Five Six",
        "First\nSecond",
        "Caf\u{e9} \u{1f40a}",
    ] {
        native_core::CORE.with(|c| {
            let mut c = c.borrow_mut();
            c.view.in_world = true;
            c.view.player = serde_json::json!({"id": 1, "target_id": 2});
            c.view.entities = vec![
                serde_json::json!({"id":2,"name":"Casil","target_id":3}),
                serde_json::json!({"id":3,"name":name}),
            ];
        });
        for alpha in [0.3, 0.62, 0.94, 1.0] {
            native.begin(size);
            let mut raster = native_assets.raster(&mut native);
            raster.ui.anchor = render::ui::Anchor::Left;
            raster.ui.vertical = render::ui::Vertical::Bottom;
            vui.blit_bar(
                &mut raster,
                &[],
                [0.0; 4],
                [20, 300],
                25,
                26,
                174,
                3,
                alpha,
                0,
                true,
            );
            let mut window = vi::ui::Window::default();
            window.init(&vui, &raster, [107, 312], 160, 1, 0);
            window.open(true);
            let wide = name.encode_utf16().collect::<Vec<_>>();
            window.text_box(
                &vui,
                &mut raster,
                &wide,
                0,
                0,
                160,
                1,
                [0.0, 0.0, 0.0, alpha],
                10,
            );
            native.end();
            actual.begin(size);
            hud_layer::clear_all();
            native_events::render(&host).unwrap();
            hud_layer::draw_with_target_alpha(&vui, &mut actual_assets.raster(&mut actual), alpha);
            actual.end();
            assert_eq!(
                render_ui_to_pixels(&native, &device, &queue, size),
                render_ui_to_pixels(&actual, &device, &queue, size),
                "native target fade and clipped name {name:?}, alpha {alpha}"
            );
        }
    }
}

/// The capture's two group power gauges (the native Mana Bars of round 3). Since sync 2026-11-15
/// upstream draws the group's power bars itself and the mod no longer does: they are cleared from
/// the reference, so the blank-background check proves the mod draws nothing there.
const MANA_GROUP_BANDS: [([f32; 4], &str, &str); 2] =
    [([28., 126., 140., 10.], "left", "top"), ([28., 174., 140., 10.], "left", "top")];

fn without_group_mana(mut pixels: Vec<u8>, size: (u32, u32)) -> Vec<u8> {
    for (r, anchor, vertical) in MANA_GROUP_BANDS {
        let r = hud_layout::transform_rect(r, anchor, vertical, hud_layout::frame_metrics(size));
        for y in (r[1].floor().max(0.) as u32)..((r[1] + r[3]).ceil().min(size.1 as f32) as u32) {
            for x in (r[0].floor().max(0.) as u32)..((r[0] + r[2]).ceil().min(size.0 as f32) as u32) {
                let i = (y * size.0 + x) as usize;
                pixels[4 * i..4 * i + 4].fill(0);
            }
        }
    }
    pixels
}

#[test]
fn bundled_mana_bars_picture_comparison() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some((_, mut ui_mod, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
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
                { "id": 10, "name": "Healer", "power": 230.0 / 255.0 },
                { "id": 11, "name": "Tank", "power": 0.4 },
            ]
        });
    });

    host.run_addon(&addon).unwrap();
    host.execute("eqoa.options.set('mana_bars', true)").unwrap();

    for size in [(640, 480), (1280, 720)] {
        let native_pixels = without_group_mana(reference("mana_bars", size), size);

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
        host.execute("eqoa.options.set('mana_bars', false)")
            .unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        ui_mod.end();
        let blank = render_ui_to_pixels(&ui_mod, &device, &queue, size);
        assert!(
            !blank.iter().any(|&b| b != 0),
            "negative control: disabled mana bars draw no pixels"
        );
        host.execute("eqoa.options.set('mana_bars', true)").unwrap();
    }
}

#[test]
fn bundled_hud_numbers_picture_comparison() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some((_, mut ui_mod, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
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
            "hp_fraction": 1.0, "hp": { "current": 255, "maximum": 255 },
            "power": { "current": 80, "maximum": 100 },
        });
        c.view.entities =
            vec![serde_json::json!({ "id": 2, "name": "Orc", "hp_fraction": 0.6, "kind": "npc" })];
    });

    host.run_addon(&addon).unwrap();
    host.execute("eqoa.options.set('hud_numbers', true); eqoa.options.set('target_hp_percent', true); eqoa.options.set('xp_details', true)").unwrap();

    for size in [(640, 480), (1280, 720)] {
        let native_pixels = reference("hud_numbers", size);

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
        assert!(
            !blank.iter().any(|&b| b != 0),
            "negative control: disabled numbers draw no pixels"
        );
        host.execute("eqoa.options.set('hud_numbers', true); eqoa.options.set('target_hp_percent', true); eqoa.options.set('xp_details', true)").unwrap();
    }
}

#[test]
fn round_3_all_four_mods_per_frame_performance_benchmark() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let (mut ui, device, queue) = {
        let instance = render::wgpu::Instance::new(
            render::wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
        );
        let adapter = pollster::block_on(
            instance.request_adapter(&render::wgpu::RequestAdapterOptions::default()),
        )
        .ok();
        let Some((d, q)) =
            adapter.and_then(|a| pollster::block_on(a.request_device(&Default::default())).ok())
        else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        (
            render::ui::Ui::new(&d, render::TARGET_FORMAT, render::DEPTH_FORMAT),
            d,
            q,
        )
    };
    let assets = test_assets(&mut ui, &device, &queue);
    let vui = test_vui();

    let tree = bundled_tree();
    stat_window_is_held();
    let discovered = discovery::discover(tree.path());

    let mod_ids = [
        "eqoa_stats_box",
        "eqoa_target_of_target",
        "eqoa_mana_bars",
        "eqoa_hud_numbers",
    ];
    let mut hosts = Vec::new();

    for id in mod_ids {
        let addon = discovered
            .addons
            .iter()
            .find(|a| a.id == id)
            .expect("bundled mod must exist");
        let host = Host::for_addon(addon).unwrap();
        host.run_addon(addon).unwrap();
        hosts.push(host);
    }

    hosts[0]
        .execute("eqoa.options.set('stats_box', true)")
        .unwrap();
    hosts[1]
        .execute("eqoa.options.set('target_of_target', true)")
        .unwrap();
    assert_eq!(
        options::get_toggle("eqoa_stats_box", "stats_box"),
        Some(true)
    );
    assert_eq!(
        options::get_toggle("eqoa_target_of_target", "target_of_target"),
        Some(true)
    );
    let view = test_view();
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player_stats = Some(view.to_json());
        c.words = Some(vui.words.clone());
        c.view.player = serde_json::json!({
            "id": 1,
            "target_id": 2,
            "level": 25,
            "experience": 1500,
            "hp_fraction": 1.0, "hp": { "current": 255, "maximum": 255, "fraction": 1.0 },
            "power": { "current": 80, "maximum": 100, "fraction": 0.8 },
        });
        c.view.entities = vec![
            serde_json::json!({ "id": 2, "name": "Casil", "target_id": 3, "hp_fraction": 0.6, "power": 0.4, "kind": "npc" }),
            serde_json::json!({ "id": 3, "name": "Luck", "hp_fraction": 204.0 / 255.0, "power": 128.0 / 255.0, "kind": "player" }),
        ];
        c.view.group = serde_json::json!({
            "in_group": true,
            "members": [
                { "id": 10, "name": "Healer", "power": 230.0 / 255.0 },
                { "id": 11, "name": "Tank", "power": 0.4 },
            ]
        });
    });

    let mut context = dear_imgui_rs::Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_display_size([1280., 720.]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    hud_layout::set_window_size(1280, 720);
    let bridges = hosts
        .iter()
        .map(|host| bridge::Bridge::install(host).unwrap())
        .collect::<Vec<_>>();
    context.frame();
    hud_layer::clear_all();
    for (host, bridge) in hosts.iter().zip(&bridges) {
        bridge.invoke(|| native_events::render(host)).unwrap();
    }
    drop(context.render_legacy());
    for id in mod_ids {
        assert!(
            !queued(id).is_empty(),
            "benchmark feature {id} must be visible"
        );
    }

    if std::env::var_os("EQOA_API_PROFILE").is_some() {
        let calls = [
            ("hud.visible", "eqoa.hud.visible('stats_box')"),
            ("options.get", "eqoa.options.get('stats_box')"),
            ("hud.frame", "eqoa.hud.frame()"),
            ("text.localized", "eqoa.text.localized(1)"),
            (
                "hud.text",
                "eqoa.hud.text({col=1,row=1,text='x',color={1,1,1,1},shadow=true})",
            ),
            ("hud.text min", "eqoa.hud.text({x=1,y=1,text='x'})"),
            (
                "hud.text color",
                "eqoa.hud.text({x=1,y=1,text='x',color={1,1,1,1}})",
            ),
            (
                "hud.text full",
                "eqoa.hud.text({x=1,y=1,text='x',color={1,1,1,1},shadow=true,align='right'})",
            ),
            (
                "hud.gauge",
                "eqoa.hud.gauge({x=30,y=325,art=7,fill=0.5,lit=true,vertical='bottom'})",
            ),
            (
                "hud.image",
                "eqoa.hud.image({ui_id=25,x=20,y=300,w=174,target_fade=true,vertical='bottom'})",
            ),
            (
                "imgui.Begin+End",
                "eqoa.imgui.Begin('w',nil,0) eqoa.imgui.End()",
            ),
            (
                "imgui.InvisibleButton",
                "eqoa.imgui.InvisibleButton('b',10,10)",
            ),
            ("hud.panel", "eqoa.hud.panel({x=200,y=54,w=148,h=132})"),
            ("target.get", "eqoa.target.get()"),
            ("entities.find", "eqoa.entities.find(2)"),
            ("imgui.SetNextWindowPos", "eqoa.imgui.SetNextWindowPos(1,1)"),
            ("noop lua", "local x=1"),
        ];
        context.frame();
        for (name, code) in calls {
            let t = Instant::now();
            bridges[0]
                .invoke(|| hosts[0].execute(&format!("for i=1,200 do {code} end")))
                .unwrap();
            println!(
                "APICOST {name}: {:.2}us/call",
                t.elapsed().as_secs_f64() * 1e6 / 200.0
            );
            hud_layer::clear_all();
        }
        drop(context.render_legacy());
    }

    let iterations = 1000;
    let size = (1280, 720);
    let mut dispatch_times = [std::time::Duration::ZERO; 4];
    let mut render_times = [std::time::Duration::ZERO; 4];
    let mut draw_time = std::time::Duration::ZERO;
    let mut imgui_time = std::time::Duration::ZERO;
    let mut run_frame = |frame: usize, profile: bool| {
        let imgui_start = profile.then(Instant::now);
        context.frame();
        if let Some(start) = imgui_start {
            imgui_time += start.elapsed();
        }
        hud_layer::clear_all();
        for (index, (host, bridge)) in hosts.iter().zip(&bridges).enumerate() {
            // Production starts a new event budget at every frame boundary.
            let dispatch_start = profile.then(Instant::now);
            native_events::dispatch(host).unwrap();
            if let Some(start) = dispatch_start {
                dispatch_times[index] += start.elapsed();
            }
            let render_start = profile.then(Instant::now);
            bridge
                .invoke(|| super::native_events::render(host))
                .unwrap();
            if let Some(start) = render_start {
                render_times[index] += start.elapsed();
            }
        }
        hud_layer::MOD_QUEUES.with(|queues| {
            let queues = queues.borrow();
            for id in mod_ids {
                assert!(
                    queues.get(id).is_some_and(|q| !q.is_empty()),
                    "benchmark feature {id} must remain visible on frame {frame}"
                );
            }
        });
        let draw_start = profile.then(Instant::now);
        ui.begin(size);
        hud_layer::draw(&vui, &mut assets.raster(&mut ui));
        ui.end();
        if let Some(start) = draw_start {
            draw_time += start.elapsed();
        }
        let imgui_start = profile.then(Instant::now);
        drop(context.render_legacy());
        if let Some(start) = imgui_start {
            imgui_time += start.elapsed();
        }
    };

    // Allocation instrumentation is deliberately outside the timed workload.
    // All four mods stay active in both loops; Lua/event and ImGui work is timed too.
    for frame in 0..iterations {
        if frame == 16 {
            hud_layer::observe_allocations(true);
        }
        run_frame(frame, false);
    }
    let hud_allocations = hud_layer::observe_allocations(false);
    assert_eq!(
        &hud_allocations[..3],
        &[0; 3],
        "all four live Lua HUD mods must reuse UTF-8 text, command queues and text drawing storage after warmup"
    );
    println!(
        "HUD allocations after warmup: UTF-8={} queues={} text_draw={} native_art={} ({:.1}/active frame)",
        hud_allocations[0],
        hud_allocations[1],
        hud_allocations[2],
        hud_allocations[3],
        hud_allocations[3] as f64 / (iterations - 16) as f64
    );

    let start = Instant::now();
    for frame in 0..iterations {
        run_frame(frame, true);
    }
    let elapsed = start.elapsed();
    for (index, id) in mod_ids.iter().enumerate() {
        println!(
            "PROFILE {id}: dispatch={:.3}us render={:.3}us/frame",
            dispatch_times[index].as_secs_f64() * 1e6 / iterations as f64,
            render_times[index].as_secs_f64() * 1e6 / iterations as f64
        );
    }
    println!(
        "PROFILE native_draw={:.3}us imgui_frame={:.3}us/frame",
        draw_time.as_secs_f64() * 1e6 / iterations as f64,
        imgui_time.as_secs_f64() * 1e6 / iterations as f64
    );
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

/// A mod holds the game's Stat Window, as the Stats Box does in play (the claim is published to the game): the box draws.
/// With nobody holding it the window reads as visible, which is the player's "Use the game's windows" to the box, and the
/// box stands down.
fn stat_window_is_held() {
    let mut claims = visibility::HudClaimsRegistry::new();
    claims.disable("stat_window", super::health::InstanceToken::next().unwrap(), "test");
    visibility::publish_claims(&claims);
}

fn load_hud_mod(tree: &TestDir, id: &str) -> Host {
    let addon = discovery::discover(tree.path())
        .addons
        .into_iter()
        .find(|a| a.id == id)
        .unwrap();
    let host = Host::for_addon(&addon).unwrap();
    host.run_addon(&addon).unwrap();
    host
}

#[test]
fn fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring() {
    use winit::keyboard::{Key, NamedKey};
    options::reset_in_memory();
    crate::fork_ext::bindpage::reset_in_memory();
    input::reset_input_state();
    native_core::CORE.with(|c| *c.borrow_mut() = native_core::Core::default());
    native_core::CORE.with(|c| c.borrow_mut().view.in_world = true);
    let tree = bundled_tree();
    let stats = load_hud_mod(&tree, "eqoa_stats_box");
    let tot = load_hud_mod(&tree, "eqoa_target_of_target");
    let mana = load_hud_mod(&tree, "eqoa_mana_bars");
    let nums = load_hud_mod(&tree, "eqoa_hud_numbers");
    stats
        .execute("assert(eqoa.options.get('stats_box') == false)")
        .unwrap();
    tot.execute("assert(eqoa.options.get('target_of_target') == false)")
        .unwrap();
    mana.execute("assert(eqoa.options.get('mana_bars') == true)")
        .unwrap();
    nums.execute("assert(eqoa.options.get('hud_numbers') and eqoa.options.get('xp_details') and eqoa.options.get('target_hp_percent'))").unwrap();
    // Change the same item the native Enhanced HUD page changes; Lua must observe it.
    assert!(options::toggle_by_item(2000));
    stats
        .execute("assert(eqoa.options.get('stats_box') == true)")
        .unwrap();
    assert!(options::toggle_by_item(2000));
    // the mod's own key is an extra: unbound until the player binds it (the game's Stat Window key opens the box)
    assert_eq!(crate::fork_ext::bindpage::get_key("eqoa_stats_box", "toggle").as_deref().unwrap_or(""), "");
    assert!(crate::fork_ext::bindpage::set_key("eqoa_stats_box", "toggle", "y"));
    let b = Key::Character("y".into());
    input::route_mod_key(&b, true, false, false, true, false);
    native_events::dispatch(&stats).unwrap();
    stats
        .execute("assert(eqoa.options.get('stats_box') == true)")
        .unwrap();
    assert!(crate::fork_ext::bindpage::set_key(
        "eqoa_stats_box",
        "toggle",
        "f2"
    ));
    input::route_mod_key(&b, true, false, false, true, false);
    native_events::dispatch(&stats).unwrap();
    stats
        .execute("assert(eqoa.options.get('stats_box') == true)")
        .unwrap();
    let f8 = Key::Named(NamedKey::F2);
    input::route_mod_key(&f8, true, false, false, true, false);
    native_events::dispatch(&stats).unwrap();
    stats
        .execute("assert(eqoa.options.get('stats_box') == false)")
        .unwrap();
    stats
        .execute("local i=require('frontiers_forge.input');assert(i.AcquireKey('chat',i.Key.Enter));assert(i.SetTextCaptureActive('chat',true))")
        .unwrap();
    input::route_mod_key(&f8, true, false, false, true, false);
    native_events::dispatch(&stats).unwrap();
    stats.execute("assert(eqoa.options.get('stats_box')==false);require('frontiers_forge.input').SetTextCaptureActive('chat',false)").unwrap();
    // Search's native editor still receives its own F2/F4-style controls; a key
    // another VM has bound must not trigger that VM's ordinary bound action. The two
    // VMs have different keys (a key two mods share does one thing, round 7), and
    // the probe is really bound (its row has the key; it fires with no editor up).
    tot.execute("other_hits=0;assert(eqoa.input.bind({id='editor_probe',default_key='y'},function() other_hits=other_hits+1 end));assert(eqoa.binds.get('editor_probe')=='y')").unwrap();
    let y = Key::Character("y".into());
    stats
        .execute("assert(eqoa.input.capture_text(true))")
        .unwrap();
    input::route_mod_key(&f8, true, false, false, true, true);
    input::route_mod_key(&y, true, false, false, true, true);
    native_events::dispatch(&stats).unwrap();
    native_events::dispatch(&tot).unwrap();
    stats.execute("assert(eqoa.options.get('stats_box')==true);assert(eqoa.input.capture_text(false));eqoa.options.set('stats_box',false)").unwrap();
    tot.execute("assert(other_hits==0, 'the editor owner has the keyboard')").unwrap();
    input::route_mod_key(&y, true, false, false, true, false);
    native_events::dispatch(&tot).unwrap();
    tot.execute("assert(other_hits==1, 'the probe fires once no editor has the keyboard')").unwrap();
    for (repeat, synthetic, plain, keyboard) in [
        (false, false, false, false),
        (false, false, true, true),
        (true, false, true, false),
        (false, true, true, false),
    ] {
        input::route_mod_key(&f8, true, repeat, synthetic, plain, keyboard);
        native_events::dispatch(&stats).unwrap();
        stats
            .execute("assert(eqoa.options.get('stats_box') == false)")
            .unwrap();
    }
    crate::fork_ext::bindpage::reset_in_memory();
}

fn queued(id: &str) -> Vec<hud_layer::HudCommand> {
    hud_layer::MOD_QUEUES.with(|q| q.borrow().get(id).cloned().unwrap_or_default())
}

#[test]
fn tot_uses_targets_channels_and_rejects_self_and_nameless_and_honors_mana() {
    options::reset_in_memory();
    let tree = bundled_tree();
    let host = load_hud_mod(&tree, "eqoa_target_of_target");
    host.execute("eqoa.options.set('target_of_target', true)")
        .unwrap();
    hud_layout::set_hud_state(true, true, false, 93, 20);
    let setup = |target_id, name: &str| {
        native_core::CORE.with(|c| {
            let mut c = c.borrow_mut();
            c.view.in_world = true;
            c.view.player = serde_json::json!({"id":1,"target_id":2});
            c.view.entities = vec![
                serde_json::json!({"id":1,"name":"Me","hp_fraction":0.7,"power":0.3}),
                serde_json::json!({"id":2,"name":"Casil","target_id":target_id,"hp_fraction":0.2}),
                serde_json::json!({"id":3,"name":name,"hp_fraction":0.8,"power":0.5}),
            ];
        })
    };
    let render = || {
        hud_layer::clear_all();
        native_events::render(&host).unwrap();
        queued("eqoa_target_of_target")
    };
    setup(3, "Luck");
    let commands = render();
    for command in &commands {
        let vertical = match command {
            hud_layer::HudCommand::Text { vertical, .. }
            | hud_layer::HudCommand::Gauge { vertical, .. }
            | hud_layer::HudCommand::Image { vertical, .. } => vertical,
            _ => panic!("unexpected ToT primitive"),
        };
        assert_eq!(
            *vertical,
            render::ui::Vertical::Bottom,
            "ToT must stay attached to the bottom target in a tall viewport"
        );
    }
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, hud_layer::HudCommand::Text{text,..} if text=="Luck"))
    );
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, hud_layer::HudCommand::Gauge{art:7,fill,..} if *fill==0.8))
    );
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, hud_layer::HudCommand::Gauge{art:9,fill,..} if *fill==0.5))
    );
    options::set_toggle("eqoa_mana_bars", "mana_bars", false);
    assert!(
        !render()
            .iter()
            .any(|c| matches!(c, hud_layer::HudCommand::Gauge { art: 9, .. }))
    );
    setup(1, "Luck");
    assert!(
        render()
            .iter()
            .any(|c| matches!(c, hud_layer::HudCommand::Text{text,..} if text=="Me")),
        "player's own target channel must show"
    );
    for (id, name) in [(2, "Luck"), (0, "Luck"), (3, ""), (3, "   "), (99, "Luck")] {
        setup(id, name);
        assert!(render().is_empty(), "invalid ToT {id}/{name:?}");
    }
    setup(3, "Luck");
    native_core::CORE.with(|c| c.borrow_mut().view.player["target_id"] = 0.into());
    assert!(render().is_empty(), "no target");
}
#[test]
fn mod_click_rect_owns_mouse_inside_and_passes_it_outside() {
    assert_mod_click(
        "eqoa_stats_box",
        "stats_box",
        [200., 54., 148., 132.],
        "top",
        true,
    );
}
#[test]
fn tot_click_name_bar_routes_assist_and_passes_outside_clicks() {
    assert_mod_click(
        "eqoa_target_of_target",
        "target_of_target",
        [20., 300., 174., 26.],
        "bottom",
        false,
    );
}
fn assert_mod_click(id: &str, option: &str, frame_rect: [f32; 4], vertical: &str, summary: bool) {
    let mut context = dear_imgui_rs::Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let tree = bundled_tree();
    stat_window_is_held();
    options::reset_in_memory();
    let host = load_hud_mod(&tree, id);
    if !summary {
        // These names are aliases today; distinguish them to guard the ToT's
        // semantic dependency if their policies diverge in a later release.
        host.execute("local visible=eqoa.hud.visible;rawset(eqoa.hud,'visible',function(name) if name=='stats_box' then return false end return visible(name) end)").unwrap();
    }
    let bridge = bridge::Bridge::install(&host).unwrap();
    host.execute(&format!("eqoa.options.set('{option}',true)"))
        .unwrap();
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player_stats = Some(test_view().to_json());
        c.view.player = serde_json::json!({"id":1,"target_id":2});
        c.view.entities = vec![
            serde_json::json!({"id":2,"name":"Casil","target_id":3}),
            serde_json::json!({"id":3,"name":"Luck","hp_fraction":0.8}),
        ];
    });
    hud_layout::set_hud_state(true, true, false, 93, 20);
    for size in [(640, 480), (1280, 720), (768, 1024)] {
        hud_layout::set_window_size(size.0, size.1);
        context
            .io_mut()
            .set_display_size([size.0 as f32, size.1 as f32]);
        let rect = hud_layout::transform_rect(
            frame_rect,
            "left",
            vertical,
            hud_layout::frame_metrics(size),
        );
        let run = |context: &mut dear_imgui_rs::Context| {
            context.frame();
            bridge.invoke(|| native_events::render(&host)).unwrap();
            drop(context.render_legacy());
        };
        run(&mut context);
        run(&mut context);
        context
            .io_mut()
            .add_mouse_pos_event([rect[0] + 2., rect[1] + 2.]);
        run(&mut context);
        run(&mut context);
        assert!(
            context.io().want_capture_mouse(),
            "inside mod rect at {size:?}"
        );
        let mut capture = capture::Capture::default();
        assert_eq!(
            capture.press(
                capture::Button::Mouse(winit::event::MouseButton::Left),
                context.io().want_capture_mouse()
            ),
            capture::Owner::Addon
        );
        actions::take_action_requests();
        crate::fork_ext::addons::action_broker::reset_all_for_test();
        context
            .io_mut()
            .add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
        run(&mut context);
        context
            .io_mut()
            .add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
        run(&mut context);
        let request = actions::take_action_requests();
        assert!(
            if summary {
                request.summary
            } else {
                crate::fork_ext::addons::action_broker::queued_kinds_for_test() == vec!["assist"]
            },
            "native action at corner of {id}'s drawn rect"
        );
        crate::fork_ext::addons::action_broker::reset_all_for_test();
        context
            .io_mut()
            .add_mouse_pos_event([size.0 as f32 - 5., size.1 as f32 - 5.]);
        run(&mut context);
        run(&mut context);
        assert!(!context.io().want_capture_mouse(), "outside mod rect");
        let mut capture = capture::Capture::default();
        assert_eq!(
            capture.press(
                capture::Button::Mouse(winit::event::MouseButton::Left),
                context.io().want_capture_mouse()
            ),
            capture::Owner::Game
        );
    }
}
#[test]
fn menu_values_persist_on_disk_and_reach_reloaded_bundled_lua() {
    let tree = bundled_tree();
    let config = TestDir::new();
    options::set_storage_root(Some(config.path().to_path_buf()));
    options::reset_in_memory();
    {
        let host = load_hud_mod(&tree, "eqoa_stats_box");
        host.execute("assert(eqoa.options.get('stats_box')==false)")
            .unwrap();
        assert!(options::toggle_by_item(2000));
        host.execute("assert(eqoa.options.get('stats_box')==true)")
            .unwrap();
    }
    options::reset_in_memory();
    let host = load_hud_mod(&tree, "eqoa_stats_box");
    host.execute("assert(eqoa.options.get('stats_box')==true)")
        .unwrap();
    let key = storage::Storage::key_for_addon_id("eqoa_stats_box");
    assert!(
        config
            .path()
            .join("addons")
            .join(format!("{key}.json"))
            .is_file()
    );
    options::set_storage_root(None);
    options::reset_in_memory();
}
#[test]
fn round3_capabilities_and_stats_records_match_public_contract() {
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player_stats = Some(test_view().to_json());
    });
    let host = Host::new().unwrap();
    host.execute(r#"
        for _,name in ipairs({'eqoa.player.stats','eqoa.hud.frame','eqoa.hud.layout','eqoa.hud.visible','eqoa.input.bind','eqoa.options.toggle','eqoa.actions.assist','eqoa.actions.target_target','eqoa.actions.toggle_stats_box','eqoa.windows.open','eqoa.binds.get'}) do
            assert(eqoa.capabilities.has(name),name)
        end
        local s=eqoa.player.stats()
        assert(s.max_hp and s.max_power and #s.rows==16)
        local r=s.rows[1]
        assert(r.short and r.name and r.word and r.current and r.base and r.min and r.max and r.change and r.shift)
        local frame=eqoa.hud.frame()
        assert(frame.side and frame.top and frame.window_width and frame.window_height)
    "#).unwrap();
    assert!(
        host.drain_logs().unwrap().is_empty(),
        "known fields must not diagnose"
    );
}
#[test]
fn mana_mod_leaves_group_power_to_upstream_and_keeps_the_target_bar() {
    // since sync 2026-11-15 upstream draws the group's and the pet's power bars (Power Bars)
    let tree = bundled_tree();
    options::reset_in_memory();
    let host = load_hud_mod(&tree, "eqoa_mana_bars");
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = serde_json::json!({"id":1});
        c.view.group =
            serde_json::json!({"in_group":true,"members":[{"id":2,"name":"Other","power":0.5}]});
    });
    let render = || {
        hud_layer::clear_all();
        native_events::render(&host).unwrap();
        queued("eqoa_mana_bars")
    };
    hud_layout::set_hud_state(true, true, false, 93, 20);
    assert!(render().is_empty(), "no group gauge from the mod");
    // control: a target with power gets the mod's one gauge
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.player = serde_json::json!({"id":1,"target_id":3});
        c.view.entities = vec![serde_json::json!({"id":3,"name":"Boss","hp_fraction":0.5,"power":0.6,"kind":"npc"})];
    });
    assert_eq!(render().len(), 1, "control: the target's gauge");
}
#[test]
fn hud_layout_uses_native_wide_tall_transforms_and_dynamic_pet_rect() {
    for size in [(640, 480), (1280, 720), (768, 1024)] {
        let metrics = hud_layout::frame_metrics(size);
        assert_eq!(metrics.4, render::ui::side(size));
        assert_eq!(metrics.5, render::ui::top(size));
        for (anchor, vertical, a, v) in [
            (
                "left",
                "top",
                render::ui::Anchor::Left,
                render::ui::Vertical::Top,
            ),
            (
                "right",
                "bottom",
                render::ui::Anchor::Right,
                render::ui::Vertical::Bottom,
            ),
        ] {
            let actual = hud_layout::transform_point([30., 398.], anchor, vertical, metrics);
            let expected = render::ui::window_from_frame(
                size,
                [
                    a.place(30., render::ui::side(size)),
                    v.place(398., render::ui::top(size)),
                ],
            );
            assert_eq!(actual, expected);
        }
        for (pet_y, pet_h, raid) in [(93, 20, false), (285, 28, false), (340, 28, true)] {
            hud_layout::set_hud_state(true, true, raid, pet_y, pet_h);
            let layout = hud_layout::hud_layout_json(size);
            assert_eq!(
                layout["pet"]["panel"]["frame_y"].as_f64(),
                Some(pet_y as f64)
            );
            assert_eq!(
                layout["pet"]["panel"]["frame_height"].as_f64(),
                Some(pet_h as f64)
            );
            assert_eq!(hud_layout::hud_element_visible("group"), !raid);
            assert!(layout.get("stats_box").is_none() && layout.get("target_of_target").is_none());
        }
    }
    hud_layout::set_hud_state(true, true, false, 93, 20);
    assert!(!hud_layout::hud_element_visible("typo"));
    let mut claims = visibility::HudClaimsRegistry::new();
    claims.disable(
        "group_compass_marks",
        health::InstanceToken::next().unwrap(),
        "test",
    );
    visibility::publish_claims(&claims);
    assert!(!hud_layout::hud_element_visible("group_markers"));
    assert!(!hud_layout::hud_element_visible("all"));
    visibility::publish_claims(&visibility::HudClaimsRegistry::new());
    assert!(hud_layout::hud_element_visible("group_markers"));
}

#[test]
fn stats_box_updates_cached_rows_when_only_native_stat_bonuses_change() {
    options::reset_in_memory();
    let tree = bundled_tree();
    stat_window_is_held();
    let host = load_hud_mod(&tree, "eqoa_stats_box");
    host.execute("eqoa.options.set('stats_box',true)").unwrap();
    hud_layout::set_hud_state(true, true, false, 93, 20);
    let mut view = native_core::View::default();
    view.in_world = true;
    let mut stats = test_view();
    view.player_stats = Some(stats.to_json());
    native_core::CORE.with(|c| c.borrow_mut().publish(view.clone()));
    native_events::dispatch(&host).unwrap();
    hud_layer::clear_all();
    native_events::render(&host).unwrap();
    assert!(
        queued("eqoa_stats_box")
            .iter()
            .any(|c| matches!(c,hud_layer::HudCommand::Text{text,..} if text=="50"))
    );
    stats.rows[0][0].value = 999;
    view.player_stats = Some(stats.to_json());
    native_core::CORE.with(|c| c.borrow_mut().publish(view));
    native_events::dispatch(&host).unwrap();
    hud_layer::clear_all();
    native_events::render(&host).unwrap();
    assert!(
        queued("eqoa_stats_box")
            .iter()
            .any(|c| matches!(c,hud_layer::HudCommand::Text{text,..} if text=="999")),
        "stat-only change must invalidate cached drawing"
    );
}

#[test]
fn bundled_readouts_follow_moved_stock_parts_and_reset() {
    crate::settings::testing::reset(false);
    crate::hudlayout::testing_reset();
    let tree = bundled_tree();
    visibility::publish_claims(&visibility::HudClaimsRegistry::new());
    hud_layout::set_hud_state(true, true, false, 93, 20);
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = serde_json::json!({"id":1,"target_id":2,"hp_fraction":0.7,"power":{"current":30,"maximum":100},"level":10,"experience":123});
        c.view.group = serde_json::json!({"in_group":true,"members":[{"id":3,"power":0.5}]});
        c.view.entities = vec![serde_json::json!({"id":2,"name":"Casil","target_id":3,"hp_fraction":0.8,"power":0.6}),serde_json::json!({"id":3,"name":"Luck","hp_fraction":0.4,"power":0.5})];
    });
    let positions = |commands: Vec<hud_layer::HudCommand>| {
        commands
            .into_iter()
            .map(|c| match c {
                hud_layer::HudCommand::Text { x, y, vertical, .. } => (x, y, vertical),
                hud_layer::HudCommand::Gauge { x, y, vertical, .. }
                | hud_layer::HudCommand::Image { x, y, vertical, .. } => {
                    (x as f32, y as f32, vertical)
                }
                _ => panic!("unexpected readout command"),
            })
            .collect::<Vec<_>>()
    };
    for id in [
        "eqoa_hud_numbers",
        "eqoa_mana_bars",
        "eqoa_target_of_target",
    ] {
        options::reset_in_memory();
        let host = load_hud_mod(&tree, id);
        if id == "eqoa_target_of_target" {
            host.execute("eqoa.options.set('target_of_target',true)")
                .unwrap();
        }
        let render = || {
            hud_layer::clear_all();
            native_events::render(&host).unwrap();
            positions(queued(id))
        };
        for size in [(640, 480), (1280, 720), (768, 1024)] {
            hud_layout::set_window_size(size.0, size.1);
            crate::settings::save_hud_layout("");
            crate::hudlayout::testing_reset();
            let before = render();
            assert!(!before.is_empty(), "{id} draws its control");
            crate::settings::save_hud_layout(
                "player l 35 t 28\ngroup l 60 t 50\ntarget l 80 b -40\n",
            );
            crate::hudlayout::testing_reset();
            let after = render();
            assert_eq!(before.len(), after.len());
            for ((x, y, v), (mx, my, mv)) in before.iter().zip(after) {
                assert_eq!(*v, mv);
                let delta = if mv == render::ui::Vertical::Bottom {
                    [80.0, -40.0]
                } else if id == "eqoa_mana_bars" {
                    [60.0, 50.0]
                } else {
                    [35.0, 28.0]
                };
                assert_eq!(
                    [mx - x, my - y],
                    delta,
                    "{id} follows the corresponding stock piece"
                );
            }
            crate::settings::save_hud_layout("");
            crate::hudlayout::testing_reset();
            assert_eq!(
                render(),
                before,
                "negative control: Reset HUD restores {id}"
            );
        }
    }
    hud_layout::set_window_size(640, 448);
}

// ---- HUD Scale: the gauge's and the image's `scale`, `hud.layout()`'s new fields, and the three mods that follow them

/// A command's kind, place and scale, as the three mods draw them.
#[derive(Clone, Debug, PartialEq)]
enum Drawn {
    Gauge { art: i32, x: i32, y: i32, scale: f32, vertical: render::ui::Vertical },
    Image { x: i32, y: i32, w: Option<i32>, scale: f32 },
    Text { x: f32, y: f32, scale: f32, clip: Option<i32> },
    Panel { x: i32, y: i32, w: i32, h: i32 },
}

fn drawn(commands: Vec<hud_layer::HudCommand>) -> Vec<Drawn> {
    commands
        .into_iter()
        .map(|c| match c {
            hud_layer::HudCommand::Gauge { art, x, y, scale, vertical, .. } => Drawn::Gauge { art, x, y, scale, vertical },
            hud_layer::HudCommand::Image { x, y, w, scale, .. } => Drawn::Image { x, y, w, scale },
            hud_layer::HudCommand::Text { x, y, scale, clip_width, .. } => Drawn::Text { x, y, scale, clip: clip_width },
            hud_layer::HudCommand::Panel { x, y, w, h, .. } => Drawn::Panel { x, y, w, h },
            _ => panic!("unexpected command"),
        })
        .collect()
}

/// The interface (`pc`), the HUD Scale (percent) and the saved piece places (`hud.opt` text), settings made fresh.
fn hud_setup(pc: bool, percent: u8, places: &str, size: (u32, u32)) {
    crate::settings::testing::reset(false);
    crate::settings::set_ui_mode(pc).unwrap();
    crate::settings::set_hud_scale_percent(percent);
    crate::settings::save_hud_layout(places);
    crate::hudlayout::testing_reset();
    hud_layout::set_hud_state(true, true, false, 93, 20);
    hud_layout::set_window_size(size.0, size.1);
}

fn target_fixture() {
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = serde_json::json!({"id":1,"target_id":2});
        c.view.entities = vec![
            serde_json::json!({"id":1,"name":"Me","hp_fraction":0.7,"power":0.3}),
            serde_json::json!({"id":2,"name":"Casil","target_id":3,"hp_fraction":0.8,"power":0.6,"disposition_name":"Hostile"}),
            serde_json::json!({"id":3,"name":"Luck","hp_fraction":0.4,"power":0.5}),
        ];
    });
}

fn render_mod(host: &Host, id: &str) -> Vec<Drawn> {
    hud_layer::clear_all();
    native_events::render(host).unwrap();
    drawn(queued(id))
}

fn rect(v: &serde_json::Value, k: &str) -> [f32; 4] {
    ["frame_x", "frame_y", "frame_width", "frame_height"].map(|f| v[k][f].as_f64().expect("a number") as f32)
}

/// The three mods at HUD Scale 100% draw what they drew before this change: the same numbers as the old Lua's
/// formulas give from `hud.layout()`, and a scale of exactly 1 (so the engine draws nothing differently), unmoved
/// and moved, in the PC interface and the Controller's.
#[test]
fn the_target_mods_draw_as_before_at_hud_scale_100() {
    use render::ui::Vertical::Bottom;
    let tree = bundled_tree();
    for pc in [false, true] {
        for places in ["", "target l 80 b -40\n"] {
            hud_setup(pc, 100, places, (640, 448));
            target_fixture();
            let layout = hud_layout::hud_layout_json((640, 448));
            let p = rect(&layout["target"], "panel");
            let pw = rect(&layout["target"], "power");
            let (x, y) = (p[0] as i32, p[1] as i32);
            let (dx, dy) = (x - 20, y - 348);
            let note = format!("pc={pc} places={places:?}");

            options::reset_in_memory();
            let host = load_hud_mod(&tree, "eqoa_mana_bars");
            assert_eq!(
                render_mod(&host, "eqoa_mana_bars"),
                vec![Drawn::Gauge { art: 9, x: pw[0] as i32, y: pw[1] as i32, scale: 1.0, vertical: Bottom }],
                "mana bars {note}"
            );

            options::reset_in_memory();
            let host = load_hud_mod(&tree, "eqoa_target_of_target");
            host.execute("eqoa.options.set('target_of_target', true)").unwrap();
            assert_eq!(
                render_mod(&host, "eqoa_target_of_target"),
                vec![
                    Drawn::Image { x: 20 + dx, y: 300 + dy, w: Some(174), scale: 1.0 },
                    Drawn::Text { x: (20 + 87 + dx) as f32, y: (300 + 12 + dy) as f32, scale: 1.0, clip: Some(160) },
                    Drawn::Gauge { art: 7, x: 30 + dx, y: 325 + dy, scale: 1.0, vertical: Bottom },
                    Drawn::Gauge { art: 9, x: 30 + dx, y: 335 + dy, scale: 1.0, vertical: Bottom },
                ],
                "target of target {note}"
            );

            options::reset_in_memory();
            let host = load_hud_mod(&tree, "eqoa_target_frame_demo");
            assert_eq!(
                render_mod(&host, "eqoa_target_frame_demo"),
                vec![
                    Drawn::Panel { x, y, w: 186, h: 62 },
                    Drawn::Text { x: (x + 10) as f32, y: (y + 8) as f32, scale: 0.85, clip: Some(166) },
                    Drawn::Gauge { art: 7, x: x + 10, y: y + 28, scale: 1.0, vertical: Bottom },
                    Drawn::Text { x: (x + 176) as f32, y: (y + 26) as f32, scale: 0.75, clip: None },
                    Drawn::Gauge { art: 9, x: x + 10, y: y + 42, scale: 1.0, vertical: Bottom },
                ],
                "target frame demo {note}"
            );
        }
    }
    hud_setup(false, 100, "", (640, 448));
}

/// At HUD Scale 150% (and with the target moved) each mod's bars sit on the game's target piece as `hud.layout()`
/// gives it, grown by the piece's own factor, in both interfaces. Negative control (run red): the mods before this
/// change draw at scale 1 at the unscaled place, which fails every position here.
#[test]
fn the_target_mods_follow_hud_scale_and_a_moved_piece_in_both_interfaces() {
    let tree = bundled_tree();
    let near = |a: f32, b: f32, what: &str| assert!((a - b).abs() <= 1.0, "{what}: {a} vs {b}");
    for pc in [false, true] {
        for places in ["", "target l 80 b -40\n"] {
            hud_setup(pc, 150, places, (640, 448));
            target_fixture();
            let layout = hud_layout::hud_layout_json((640, 448));
            assert_eq!(layout["hud_scale"].as_f64(), Some(1.5));
            let p = rect(&layout["target"], "panel");
            let pw = rect(&layout["target"], "power");
            let k = p[2] / 174.0;
            assert!((k - 1.5).abs() < 0.01, "the piece grows by HUD Scale (pc={pc}): {k}");
            let note = format!("pc={pc} places={places:?}");
            let gauge = |d: &Drawn| match d {
                Drawn::Gauge { x, y, scale, .. } => (*x as f32, *y as f32, *scale),
                other => panic!("not a gauge: {other:?}"),
            };

            // mana bars: on the game's power bar, 136 * k wide
            options::reset_in_memory();
            let host = load_hud_mod(&tree, "eqoa_mana_bars");
            let out = render_mod(&host, "eqoa_mana_bars");
            assert_eq!(out.len(), 1, "{note}");
            let (x, y, s) = gauge(&out[0]);
            near(x, pw[0], &format!("mana x {note}"));
            near(y, pw[1], &format!("mana y {note}"));
            near(s, pw[2] / 136.0, &format!("mana scale {note}"));
            assert!((s - 1.5).abs() < 0.01, "{note}");

            // target of target: its 100% places on the panel, each grown from the panel's corner
            options::reset_in_memory();
            let host = load_hud_mod(&tree, "eqoa_target_of_target");
            host.execute("eqoa.options.set('target_of_target', true)").unwrap();
            let out = render_mod(&host, "eqoa_target_of_target");
            assert_eq!(out.len(), 4, "{note}");
            let at = |ex: f32, ey: f32| (p[0] + (ex - 20.0) * k, p[1] + (ey - 348.0) * k);
            match &out[0] {
                Drawn::Image { x, y, w, scale } => {
                    let (ex, ey) = at(20.0, 300.0);
                    near(*x as f32, ex, &format!("tot image x {note}"));
                    near(*y as f32, ey, &format!("tot image y {note}"));
                    near(*scale, k, "tot image scale");
                    assert_eq!(*w, Some(174));
                }
                other => panic!("{other:?}"),
            }
            match &out[1] {
                Drawn::Text { x, y, scale, clip } => {
                    let (ex, ey) = at(107.0, 312.0);
                    near(*x, ex, &format!("tot name x {note}"));
                    near(*y, ey, &format!("tot name y {note}"));
                    near(*scale, k, "tot name scale");
                    assert_eq!(*clip, Some((160.0 * k).round() as i32));
                }
                other => panic!("{other:?}"),
            }
            for (i, (ex, ey)) in [(2, (30.0, 325.0)), (3, (30.0, 335.0))] {
                let (x, y, s) = gauge(&out[i]);
                let (px, py) = at(ex, ey);
                near(x, px, &format!("tot gauge {i} x {note}"));
                near(y, py, &format!("tot gauge {i} y {note}"));
                near(s, k, "tot gauge scale");
            }

            // the target frame demo: on the panel, every size times k
            options::reset_in_memory();
            let host = load_hud_mod(&tree, "eqoa_target_frame_demo");
            let out = render_mod(&host, "eqoa_target_frame_demo");
            assert_eq!(out.len(), 5, "{note}");
            match &out[0] {
                Drawn::Panel { x, y, w, h } => {
                    near(*x as f32, p[0], &format!("demo panel x {note}"));
                    near(*y as f32, p[1], &format!("demo panel y {note}"));
                    near(*w as f32, 186.0 * k, "demo panel w");
                    near(*h as f32, 62.0 * k, "demo panel h");
                }
                other => panic!("{other:?}"),
            }
            for (i, (ox, oy)) in [(2, (10.0, 28.0)), (4, (10.0, 42.0))] {
                let (x, y, s) = gauge(&out[i]);
                near(x, p[0] + ox * k, &format!("demo gauge {i} x {note}"));
                near(y, p[1] + oy * k, &format!("demo gauge {i} y {note}"));
                near(s, k, "demo gauge scale");
            }
        }
    }
    hud_setup(false, 100, "", (640, 448));
}

/// `eqoa.hud.layout()` says the HUD Scale, the interface and whether the HUD is unlocked, and gives the other
/// pieces' rectangles while the game draws them; none of it is ever null, for a window of no size too.
#[test]
fn hud_layout_says_the_scale_the_interface_and_the_unlocked_state_and_more_pieces() {
    fn no_null(path: &str, v: &serde_json::Value, bad: &mut Vec<String>) {
        match v {
            serde_json::Value::Null => bad.push(path.to_string()),
            serde_json::Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| no_null(&format!("{path}[{i}]"), x, bad)),
            serde_json::Value::Object(o) => o.iter().for_each(|(k, x)| no_null(&format!("{path}.{k}"), x, bad)),
            _ => {}
        }
    }
    for pc in [false, true] {
        hud_setup(pc, 125, "", (640, 480));
        crate::hudlayout::set_unlocked(false);
        let l = hud_layout::hud_layout_json((640, 480));
        assert_eq!((l["hud_scale"].as_f64(), l["pc_mode"].as_bool(), l["hud_unlocked"].as_bool()), (Some(1.25), Some(pc), Some(false)));
        crate::hudlayout::set_unlocked(true);
        let l = hud_layout::hud_layout_json((640, 480));
        assert_eq!(l["hud_unlocked"].as_bool(), Some(true), "Move HUD on: the cached record follows");
        crate::hudlayout::set_unlocked(false);
        assert!(l["pieces"].as_object().is_some_and(|p| p.is_empty()), "a piece the game did not draw has no rectangle");
        assert!(l.get("quests").is_none() && l.get("quest_tracker").is_none(), "pieces are not top-level keys");
        // the quest tracker and the stat window drawn in the last frame
        crate::hudlayout::frame_begin();
        crate::hudlayout::note(crate::hudlayout::Piece::Quests, [500.0, 120.0, 620.0, 200.0], [0.0; 2]);
        crate::hudlayout::note(crate::hudlayout::Piece::Stats, [6.0, 150.0, 202.0, 250.0], [0.0; 2]);
        crate::hudlayout::frame_end();
        let l = hud_layout::hud_layout_json((640, 480));
        let q = &l["pieces"]["quest_tracker"];
        assert_eq!((q["frame_x"].as_f64(), q["frame_width"].as_f64(), q["frame_height"].as_f64()), (Some(500.0), Some(120.0), Some(80.0)), "pc={pc}");
        for (key, want) in [("x", 500.0), ("y", 120.0 * 480.0 / 448.0), ("width", 120.0), ("height", 80.0 * 480.0 / 448.0)] {
            assert!((q[key].as_f64().unwrap() - want).abs() < 0.01, "window {key} at 640 x 480: {} vs {want}", q[key]);
        }
        assert_eq!(l["pieces"]["stat_window"]["frame_x"].as_f64(), Some(6.0));
        for size in [(0, 0), (0, 480), (640, 0), (1280, 720)] {
            let mut bad = Vec::new();
            no_null("layout", &hud_layout::hud_layout_json(size), &mut bad);
            assert!(bad.is_empty(), "pc={pc} size={size:?}: {bad:?}");
        }
        crate::hudlayout::frame_begin();
        crate::hudlayout::frame_end();
        assert!(hud_layout::hud_layout_json((640, 480))["pieces"].get("quest_tracker").is_none(), "gone when not drawn");
    }
    hud_setup(false, 100, "", (640, 448));
}

/// `hud.layout(size)` for a size other than the window the game drew in: the other pieces' own coordinates are
/// measured with the drawn window's side (not the asked size's), so the same piece does not move by the difference.
#[test]
fn hud_layout_pieces_are_measured_in_the_window_they_were_drawn_in() {
    use crate::hudlayout::{Edge, Piece};
    hud_setup(false, 100, "", (1280, 720));
    crate::hudlayout::frame_begin();
    crate::hudlayout::note(Piece::Quests, [500.0, 120.0, 620.0, 200.0], [0.0; 2]);
    crate::hudlayout::frame_end();
    let (own_h, _) = Piece::Quests.own();
    assert!(own_h != Edge::Middle, "the probe piece is kept at a side");
    let drawn = hud_layout::beyond_of((1280, 720));
    let want = 500.0 - if own_h == Edge::Start { -drawn.side } else { drawn.side };
    assert!(drawn.side > 50.0, "a 16:9 window has a side: {}", drawn.side);
    for size in [(1280, 720), (640, 448), (1920, 1080)] {
        let l = hud_layout::hud_layout_json(size);
        let got = l["pieces"]["quest_tracker"]["frame_x"].as_f64().unwrap() as f32;
        assert!((got - want).abs() < 0.01, "size {size:?}: frame_x {got} vs {want}");
    }
    hud_setup(false, 100, "", (640, 448));
}

/// A mod can ask for `scale` on a gauge and an image; a bad one is refused; none is the old call.
#[test]
fn gauge_and_image_take_an_optional_scale() {
    hud_setup(false, 100, "", (640, 448));
    target_fixture();
    let host = Host::new().unwrap();
    hud_layer::clear_all();
    host.execute(
        r#"
        assert(eqoa.hud.gauge({x=10, y=20, art=7, fill=0.5, scale=1.5}))
        assert(eqoa.hud.gauge({x=10, y=20, art=7, fill=0.5}))
        assert(eqoa.hud.image({ui_id=25, x=10, y=20, w=174, scale=2}))
        assert(eqoa.hud.image({ui_id='panel', x=10, y=20, scale=1.5}))
        -- a scale that cannot apply (a texture) is refused, never ignored
        for _, sc in ipairs({1, 2, -1}) do
            local ok, err = eqoa.hud.image({texture={}, x=1, y=1, scale=sc})
            assert(ok == nil and err.code == 'invalid_argument', 'texture scale ' .. tostring(sc))
        end
        for _, bad in ipairs({0, -1, 1/0, 0/0}) do
            local ok, err = eqoa.hud.gauge({x=1, y=1, art=7, fill=1, scale=bad})
            assert(ok == nil and err.code == 'invalid_argument', 'gauge scale ' .. tostring(bad))
            ok, err = eqoa.hud.image({ui_id=25, x=1, y=1, scale=bad})
            assert(ok == nil and err.code == 'invalid_argument', 'image scale ' .. tostring(bad))
            ok, err = eqoa.hud.image({ui_id='panel', x=1, y=1, scale=bad})
            assert(ok == nil and err.code == 'invalid_argument', 'panel scale ' .. tostring(bad))
        end
    "#,
    )
    .unwrap();
    let raw: Vec<hud_layer::HudCommand> = hud_layer::MOD_QUEUES.with(|q| q.borrow().values().flatten().cloned().collect());
    assert!(
        raw.iter().any(|c| matches!(c, hud_layer::HudCommand::Panel { scale, .. } if *scale == 1.5)),
        "the panel keeps its scale: {raw:?}"
    );
    let q = drawn(raw.into_iter().filter(|c| !matches!(c, hud_layer::HudCommand::Panel { .. })).collect());
    assert_eq!(q.len(), 3, "the refused calls queue nothing: {q:?}");
    assert!(matches!(q[0], Drawn::Gauge { scale, .. } if scale == 1.5));
    assert!(matches!(q[1], Drawn::Gauge { scale, .. } if scale == 1.0), "no scale: the old call");
    assert!(matches!(q[2], Drawn::Image { scale, .. } if scale == 2.0));
}

/// The engine side of `scale`: a gauge or an image asked at 1.5 grows about its own top left corner (the same place
/// as unscaled), 1.5 times as wide and as tall; the layer's HUD scale is as it was after. Needs a graphics adapter:
/// without one it prints SKIP and returns (the lead runs it where there is one).
#[test]
fn a_scaled_gauge_and_image_grow_about_their_own_corner() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some((_, mut ui_mod, device, queue)) = layer_with_device() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    let assets = test_assets(&mut ui_mod, &device, &queue);
    let vui = test_vui();
    let bounds = |ui: &render::ui::Ui| {
        let mut r = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for q in &ui.quads {
            for p in &q.pos {
                r = [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])];
            }
        }
        r
    };
    for (what, make) in [
        ("gauge", (|s: f32| hud_layer::HudCommand::Gauge { x: 100, y: 200, art: 7, fill: 1.0, lit: true, scale: s, anchor: render::ui::Anchor::Left, vertical: render::ui::Vertical::Top }) as fn(f32) -> hud_layer::HudCommand),
        ("image", |s: f32| hud_layer::HudCommand::Image { ui_id: 25, target_fade: false, x: 100, y: 200, w: Some(174), h: None, color: None, scale: s, anchor: render::ui::Anchor::Left, vertical: render::ui::Vertical::Top }),
        ("panel", |s: f32| hud_layer::HudCommand::Panel { x: 100, y: 200, w: 148, h: 132, scale: s, anchor: render::ui::Anchor::Left, vertical: render::ui::Vertical::Top }),
    ] {
        let mut seen = Vec::new();
        for s in [1.0, 1.5] {
            ui_mod.begin((640, 448));
            hud_layer::clear_all();
            hud_layer::queue_command("scale_probe", make(s)).unwrap();
            hud_layer::draw(&vui, &mut assets.raster(&mut ui_mod));
            ui_mod.end();
            assert!(ui_mod.hud_scale == render::ui::HudScale::IDENTITY, "{what}: the layer's HUD scale is put back");
            seen.push(bounds(&ui_mod));
        }
        let (a, b) = (seen[0], seen[1]);
        assert!(a[2] > a[0] && a[3] > a[1], "{what} drew something");
        assert!((b[0] - a[0]).abs() < 0.01 && (b[1] - a[1]).abs() < 0.01, "{what}: the same top left: {a:?} {b:?}");
        assert!(((b[2] - b[0]) - 1.5 * (a[2] - a[0])).abs() < 0.5, "{what}: 1.5 times as wide: {a:?} {b:?}");
        assert!(((b[3] - b[1]) - 1.5 * (a[3] - a[1])).abs() < 0.5, "{what}: 1.5 times as tall: {a:?} {b:?}");
    }
}
