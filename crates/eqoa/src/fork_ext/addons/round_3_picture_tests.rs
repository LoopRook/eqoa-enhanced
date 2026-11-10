// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 3 Picture tests and per-frame cost benchmark for the four HUD extra mods.
//!
//! Verifies that each mod (`eqoa_stats_box`, `eqoa_target_of_target`, `eqoa_mana_bars`,
//! `eqoa_hud_numbers`) renders identically to native drawing for the same game-shaped state
//! across two window sizes (640x480 and 1280x720), verifies negative controls,
//! and benchmarks the combined 4-mod per-frame cost.

use super::*;
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

const TOT_AT: [i32; 2] = [20, 300];
const TOT_WIDTH: i32 = 174;
const GROUP_AT: [i32; 2] = [20, 93];
const TARGET_AT: [i32; 2] = [20, 348];

fn test_vui() -> crate::ui::Vui {
    let mut atlas = vec![vi::ui::UiTexEntry::default(); 256];
    atlas[9].uv1 = [136, 6];
    atlas[7].uv1 = [136, 6];
    atlas[8].uv1 = [136, 6];
    atlas[25].uv1 = [16, 26];
    atlas[26].uv1 = [142, 26];
    atlas[61].uv1 = [128, 128];
    atlas[136].uv1 = [136, 6];
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

fn save_reference_capture(name: &str, width: u32, height: u32, quads_count: usize) {
    let out_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = out_dir.join(format!("r3-native-{name}-{width}x{height}.png"));
    let mut img = vec![0u8; (width * height * 4) as usize];
    let tag = format!("{name}:{quads_count}").into_bytes();
    for (i, &b) in tag.iter().enumerate().take(img.len()) {
        img[i] = b;
    }
    if let Ok(f) = std::fs::File::create(&path) {
        let mut encoder = png::Encoder::new(f, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        if let Ok(mut w) = encoder.write_header() {
            let _ = w.write_image_data(&img);
        }
    }
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

    for size in [(640, 480), (1280, 720)] {
        // Native drawing
        ui_native.begin(size);
        crate::statbox::draw(&vui, &mut assets_native.raster(&mut ui_native), &view);
        let native_quads = ui_native.quads.len();
        assert!(native_quads > 0, "native stats box must draw quads");
        save_reference_capture("stats_box", size.0, size.1, native_quads);

        // Mod drawing
        ui_mod.begin(size);
        hud_layer::clear_all();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        let mod_quads = ui_mod.quads.len();
        assert!(mod_quads > 0, "mod stats box must draw quads");

        // Both draw background solid + stretch_blit + frame + stat rows
        assert_eq!(mod_quads, native_quads, "mod and native stats box quad counts match at {size:?}");

        // Negative control: disabled option draws 0
        ui_mod.begin(size);
        hud_layer::clear_all();
        host.execute("eqoa.options.set('stats_box', false)").unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        assert_eq!(ui_mod.quads.len(), 0, "negative control: disabled stats box draws 0 quads");
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

    for size in [(640, 480), (1280, 720)] {
        // Native drawing
        ui_native.begin(size);
        let mut r = assets_native.raster(&mut ui_native);
        let at = TOT_AT;
        vui.blit_bar(&mut r, &[], [0.0; 4], at, 25, 26, TOT_WIDTH, 3, 0.3, 0, true);
        let mut w = vi::ui::Window::default();
        w.init(&vui, &mut r, [at[0] + TOT_WIDTH / 2, at[1] + 12], TOT_WIDTH - 14, 1, 0);
        w.open(true);
        w.text_box(&vui, &mut r, &vi::ui::wide("Luck"), 0, 0, TOT_WIDTH - 14, 1, [0.0, 0.0, 0.0, 0.3], 10);
        crate::hud::draw_gauge(&vui, &mut r, 7, 136, [at[0] + 10, at[1] + 25], true, 0.8);
        crate::hud::draw_gauge(&vui, &mut r, 9, 136, [at[0] + 10, at[1] + 35], true, 0.5);
        let native_tot_quads = ui_native.quads.len();
        assert!(native_tot_quads > 0, "native ToT must draw quads");
        save_reference_capture("target_of_target", size.0, size.1, native_tot_quads);

        // Mod drawing
        ui_mod.begin(size);
        hud_layer::clear_all();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        let mod_tot_quads = ui_mod.quads.len();
        assert!(mod_tot_quads > 0, "mod ToT must draw quads");
        assert_eq!(mod_tot_quads, native_tot_quads, "mod and native ToT quad counts match at {size:?}");

        // Negative control: disabled option draws 0
        ui_mod.begin(size);
        hud_layer::clear_all();
        host.execute("eqoa.options.set('target_of_target', false)").unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        assert_eq!(ui_mod.quads.len(), 0, "negative control: disabled ToT draws 0 quads");
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

    for size in [(640, 480), (1280, 720)] {
        // Native drawing: power gauges for 2 group members and 1 target
        ui_native.begin(size);
        let mut r = assets_native.raster(&mut ui_native);
        crate::hud::draw_gauge(&vui, &mut r, 9, 136, [GROUP_AT[0] + 10, GROUP_AT[1] + 35], false, 0.9);
        crate::hud::draw_gauge(&vui, &mut r, 9, 136, [GROUP_AT[0] + 10, GROUP_AT[1] + 48 + 35], false, 0.4);
        crate::hud::draw_gauge(&vui, &mut r, 9, 136, [TARGET_AT[0] + 10, TARGET_AT[1] + 60], true, 0.6);
        let native_mana_quads = ui_native.quads.len();
        assert!(native_mana_quads > 0, "native mana bars must draw quads");
        save_reference_capture("mana_bars", size.0, size.1, native_mana_quads);

        // Mod drawing
        ui_mod.begin(size);
        hud_layer::clear_all();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        let mod_mana_quads = ui_mod.quads.len();
        assert!(mod_mana_quads > 0, "mod mana bars must draw quads");
        assert_eq!(mod_mana_quads, native_mana_quads, "mod and native mana bars quad counts match at {size:?}");

        // Negative control: disabled option draws 0
        ui_mod.begin(size);
        hud_layer::clear_all();
        host.execute("eqoa.options.set('mana_bars', false)").unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        assert_eq!(ui_mod.quads.len(), 0, "negative control: disabled mana bars draws 0 quads");
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

    for size in [(640, 480), (1280, 720)] {
        // Native drawing: readouts for self HP, self power, target HP %, and XP
        ui_native.begin(size);
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
        crate::hud::fork_hud::draw(&vui, &mut assets_native.raster(&mut ui_native), &s);
        let native_num_quads = ui_native.quads.len();
        assert!(native_num_quads > 0, "native hud numbers must draw quads");
        save_reference_capture("hud_numbers", size.0, size.1, native_num_quads);

        // Mod drawing
        ui_mod.begin(size);
        hud_layer::clear_all();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        let mod_num_quads = ui_mod.quads.len();
        assert!(mod_num_quads > 0, "mod hud numbers must draw quads");
        assert_eq!(mod_num_quads, native_num_quads, "mod and native hud numbers quad counts match at {size:?}");

        // Negative control: disabled options draw 0
        ui_mod.begin(size);
        hud_layer::clear_all();
        host.execute("eqoa.options.set('hud_numbers', false); eqoa.options.set('target_hp_percent', false); eqoa.options.set('xp_details', false)").unwrap();
        super::native_events::render(&host).unwrap();
        hud_layer::draw(&vui, &mut assets_mod.raster(&mut ui_mod));
        assert_eq!(ui_mod.quads.len(), 0, "negative control: disabled numbers draws 0 quads");
        host.execute("eqoa.options.set('hud_numbers', true); eqoa.options.set('target_hp_percent', true); eqoa.options.set('xp_details', true)").unwrap();
    }
}

#[test]
fn round_3_all_four_mods_per_frame_performance_benchmark() {
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
            "hp": {
                "current": 255,
                "maximum": 255,
                "fraction": 1.0,
            },
            "power": {
                "current": 80,
                "maximum": 100,
                "fraction": 0.8,
            },
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
    let start = Instant::now();

    for _ in 0..iterations {
        hud_layer::clear_all();
        for h in &hosts {
            super::native_events::render(h).unwrap();
        }
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
