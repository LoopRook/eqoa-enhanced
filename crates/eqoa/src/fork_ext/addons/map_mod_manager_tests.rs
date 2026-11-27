// Real Manager + GPU evidence for the two map mods (included in manager tests).
fn r4_begin(gpu: &mut Gpu) {
    r4_begin_at(gpu, (1024, 768))
}
fn r4_begin_at(gpu: &mut Gpu, size: (u32, u32)) {
    gpu.context.io_mut().set_display_size([size.0 as f32, size.1 as f32]);
    gpu.context.io_mut().set_display_framebuffer_scale([1., 1.]);
    gpu.context.io_mut().set_delta_time(1. / 60.);
    gpu.context.frame();
}
fn r4_render(gpu: &mut Gpu, map: &crate::map::Map) -> wgpu::Texture {
    r4_render_at(gpu, map, (1024, 768))
}
fn r4_render_at(gpu: &mut Gpu, map: &crate::map::Map, size: (u32, u32)) -> wgpu::Texture {
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("round4 native map + addons"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: target.size(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: render::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let depth = depth.create_view(&Default::default());
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.025,
                        g: 0.035,
                        b: 0.025,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    map.draw(&gpu.device, &mut encoder, &view, &depth, size, size);
    map.draw_mini(&gpu.device, &mut encoder, &view, &depth, size, size);
    {
        let mut renderer = gpu.renderer.borrow_mut();
        let frame = gpu.context.render(renderer.renderer_consumer().unwrap());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer
            .render(frame, &mut pass, FramebufferExtent::new(size.0, size.1))
            .unwrap();
    }
    gpu.queue.submit([encoder.finish()]);
    target
}
fn r4_pixels(gpu: &Gpu, target: &wgpu::Texture) -> Vec<u8> {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 1024 * 768 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4096),
                rows_per_image: Some(768),
            },
        },
        target.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    rx.recv().unwrap().unwrap();
    let bytes = buffer.slice(..).get_mapped_range().unwrap().to_vec();
    bytes
}
#[test]
fn round4b_manager_map_mods_large_journal_gpu_captures_and_frame_cost() {
    big_stack(round4b_manager_map_mods_large_journal_gpu_captures_and_frame_cost_body)
}
fn round4b_manager_map_mods_large_journal_gpu_captures_and_frame_cost_body() {
    use super::super::{hud_layout, input, native_core};
    use std::time::{Duration, Instant};
    use winit::keyboard::{Key, NamedKey};
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    crate::settings::testing::reset(false);
    crate::hudlayout::testing_reset();
    crate::hideui::show();
    input::reset_input_state();
    let root = Scratch::new();
    let config = Scratch::new();
    for file in crate::fork_ext::bundled::EMBEDDED_FILES
        .iter()
        .filter(|f| matches!(f.package, "eqoa_creature_journal" | "eqoa_map_search"))
    {
        let path = root.0.join(file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Normal runs use the embedded shipping bytes. Negative controls can
        // opt into the same package's edited Lua files without changing assets.
        if std::env::var_os("EQOA_R4_TEST_SOURCE").is_some() && file.path.ends_with(".lua") {
            fs::copy(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../bundled-mods")
                    .join(file.path),
                &path,
            )
            .unwrap();
        } else {
            fs::write(path, file.data).unwrap();
        }
    }
    let journal = super::super::discovery::discover(&root.0)
        .addons
        .into_iter()
        .find(|a| a.id == "eqoa_creature_journal")
        .unwrap();
    let data = Storage::new(Some(config.0.clone()))
        .data_dir(&journal)
        .unwrap()
        .unwrap();
    let path = data.join("files/data/journal.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let ids = (10000..33000)
        .map(|i| {
            (
                i.to_string(),
                serde_json::json!({"world":0,"level":19,"pos":[i as f64*40.,12.5,1800.25]}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let bytes = serde_json::to_vec(&serde_json::json!({"Performance Creature":ids})).unwrap();
    assert!(
        (1_400_000..1_500_000).contains(&bytes.len()),
        "workload size {}",
        bytes.len()
    );
    fs::write(&path, &bytes).unwrap();
    hud_layout::set_window_size(1024, 768);
    hud_layout::set_hud_state(true, true, false, 93, 0);
    let mut play = crate::play::Play::bundled_ui_fixture();
    play.map.mini.up = true; // the stock minimap setting is on; search follows it
    let mut snapshot = play.forge_snapshot(None);
    snapshot.capture_pending = false;
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    let scene = crate::map::Map::round4_fixture(&gpu.device, &gpu.queue, (1024, 768), true);
    // Real worker + sliced materialization, bounded by a wall-clock deadline.
    let load_started = Instant::now();
    let deadline = load_started + Duration::from_secs(60);
    let mut loaded = false;
    let mut load_state = (0usize, 0usize, 0usize);
    while Instant::now() < deadline {
        manager.flush_pending_fonts(&mut gpu.context);
        r4_begin(&mut gpu);
        native_core::presentation();
        manager.frame(snapshot);
        r4_render(&mut gpu, &scene);
        manager.after_render();
        assert!(
            manager.entries.iter().all(|e| e.status == Status::Enabled),
            "{:?}",
            manager.diagnostics()
        );
        let lua = lua(&manager, "eqoa_map_search");
        load_state = lua
            .load("local s=require('search');return #s.journal,#s.catalog,#s.jobs")
            .eval::<(usize, usize, usize)>()
            .unwrap();
        loaded = load_state.0 == 23000 && load_state.1 > 1000;
        if loaded {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        loaded,
        "large journal/catalogue workload did not run: {load_state:?}; {:?}",
        manager.diagnostics()
    );
    eprintln!(
        "R4 initial shared journal load: {:?}",
        load_started.elapsed()
    );
    {
        let l = lua(&manager, "eqoa_map_search");
        l.load("seen_keys=0;eqoa.events.on('input.key',function(e) seen_keys=seen_keys+1 end)")
            .exec()
            .unwrap();
        assert_eq!(
            l.load("return eqoa.hud.visible('minimap'),eqoa.map.get().open")
                .eval::<(bool, bool)>()
                .unwrap(),
            (true, false)
        );
        assert_eq!(
            l.load("return eqoa.fonts.load('package_resources/SpaceMono-Regular.ttf',13).status")
                .eval::<String>()
                .unwrap(),
            "ready"
        );
        assert!(
            manager.diagnostics().is_empty(),
            "{:?}",
            manager.diagnostics()
        );
    }
    input::route_mod_key(&Key::Named(NamedKey::F2), true, false, false, true, false);
    // Keep callbacks/data/UI through Manager; no direct calls to model functions.
    for _ in 0..90 {
        manager.flush_pending_fonts(&mut gpu.context);
        r4_begin(&mut gpu);
        native_core::presentation();
        manager.frame(snapshot);
        r4_render(&mut gpu, &scene);
        manager.after_render();
    }
    let lua = lua(&manager, "eqoa_map_search");
    assert_eq!(lua.globals().get::<i32>("seen_keys").unwrap(), 1);
    assert!(super::super::native::finish(&lua).is_empty());
    assert!(
        lua.load("return require('search').open")
            .eval::<bool>()
            .unwrap()
    );
    let mut costs = Vec::new();
    for _ in 0..120 {
        manager.flush_pending_fonts(&mut gpu.context);
        r4_begin(&mut gpu);
        native_core::presentation();
        let start = Instant::now();
        manager.frame(snapshot);
        costs.push(start.elapsed().as_secs_f64() * 1000.);
        r4_render(&mut gpu, &scene);
        manager.after_render();
    }
    costs.sort_by(f64::total_cmp);
    let mean = costs.iter().sum::<f64>() / costs.len() as f64;
    eprintln!(
        "R4 map mods {}: journal={} bytes, 23000 loaded spawns; frames={}; mean={mean:.3} ms p50={:.3} ms p95={:.3} ms max={:.3} ms",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        bytes.len(),
        costs.len(),
        costs[60],
        costs[114],
        costs[119]
    );
    assert!(
        manager.entries.iter().all(|e| e.status == Status::Enabled),
        "{:?}",
        manager.diagnostics()
    );
    let bad = manager
        .diagnostics()
        .iter()
        .filter(|d| matches!(d.phase, "events" | "entry" | "imgui" | "frame"))
        .collect::<Vec<_>>();
    assert!(bad.is_empty(), "{bad:?}");
    // A modal panel owns outside clicks and wheels before native map/world input.
    use winit::event::{
        DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
    };
    let selected = lua
        .load("return require('search').selected")
        .eval::<usize>()
        .unwrap();
    super::super::pointer_api::position([1000., 740.], true, (1024, 768));
    let wheel = WindowEvent::MouseWheel {
        device_id: DeviceId::dummy(),
        delta: MouseScrollDelta::LineDelta(0., 0.3),
        phase: TouchPhase::Moved,
    };
    assert!(
        super::super::pointer_api::observe(&wheel, (1024, 768)),
        "open search must consume an outside wheel"
    );
    for _ in 0..2 {
        r4_begin(&mut gpu);
        native_core::presentation();
        manager.frame(snapshot);
        r4_render(&mut gpu, &scene);
        manager.after_render();
    }
    assert_eq!(
        lua.load("return require('search').selected")
            .eval::<usize>()
            .unwrap(),
        selected,
        "outside wheel must not scroll the list"
    );
    let press = WindowEvent::MouseInput {
        device_id: DeviceId::dummy(),
        state: ElementState::Pressed,
        button: MouseButton::Left,
    };
    assert!(
        super::super::pointer_api::observe(&press, (1024, 768)),
        "closing click must be consumed before native world input"
    );
    for _ in 0..3 {
        r4_begin(&mut gpu);
        native_core::presentation();
        manager.frame(snapshot);
        r4_render(&mut gpu, &scene);
        manager.after_render();
    }
    assert!(
        !lua.load("return require('search').open")
            .eval::<bool>()
            .unwrap()
    );
    assert!(
        !super::super::pointer_api::observe(&wheel, (1024, 768)),
        "closed search must release the world"
    );
    input::route_mod_key(&Key::Named(NamedKey::F2), true, false, false, true, false);
    for _ in 0..3 {
        r4_begin(&mut gpu);
        native_core::presentation();
        manager.frame(snapshot);
        r4_render(&mut gpu, &scene);
        manager.after_render();
    }
    for mini in [true, false] {
        play.test_map_search_surface(!mini);
        play.map.mini.up = mini;
        snapshot = play.forge_snapshot(None);
        snapshot.capture_pending = false;
        let map = crate::map::Map::round4_fixture(&gpu.device, &gpu.queue, (1024, 768), mini);
        let mut target = None;
        for _ in 0..3 {
            manager.flush_pending_fonts(&mut gpu.context);
            r4_begin(&mut gpu);
            native_core::presentation();
            manager.frame(snapshot);
            target = Some(r4_render(&mut gpu, &map));
            manager.after_render();
        }
        if !mini {
            assert!(lua.load("local s=require('search');local a=eqoa.map.get().search_anchor;return a~=nil and s.panel_rect~=nil and s.panel_rect.y>=a.y+26").eval::<bool>().unwrap(),"rendered full-map search must clear the actual header");
        }
        let pixels = r4_pixels(&gpu, target.as_ref().unwrap());
        assert!(
            pixels
                .chunks_exact(4)
                .filter(|p| p[0] > 150 && p[1] < 80 && p[2] < 90)
                .count()
                > 100,
            "red search controls must actually render"
        );
        if let Some(out) = std::env::var_os("EQOA_R4_CAPTURE_DIR") {
            let out = PathBuf::from(out);
            fs::create_dir_all(&out).unwrap();
            let file = out.join(if mini {
                "r4-search-minimap.png"
            } else {
                "r4-search-map.png"
            });
            let mut encoder = png::Encoder::new(fs::File::create(file).unwrap(), 1024, 768);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
    }

    // A footer click delegates its physical activation through the real command
    // queue to the journal VM. Its worker publishes a real Desktop file.
    let desktop = config.0.join("Desktop");
    let j = manager
        .entries
        .iter()
        .find(|e| e.metadata.id == "eqoa_creature_journal")
        .unwrap()
        .instance
        .as_ref()
        .unwrap();
    j.host.test_player_files(None, desktop.clone());
    let (px, py, ph, k) = lua
        .load("local s=require('search');local r=s.panel_rect;return r.x,r.y,r.h,s.scale")
        .eval::<(f32, f32, f32, f32)>()
        .unwrap();
    // the Export button's middle: after Prev (44 wide and a 4 gap), 58 wide, 13 up from the foot
    native_core::emit(
        "pointer.pressed",
        serde_json::json!({"x":px+(8.+44.+4.+29.)*k,"y":py+ph-13.*k,"inside_window":true,"button":"left"}),
    );
    let exported = desktop.join("EQOA-Creature-Journal.json");
    for _ in 0..100 {
        manager.flush_pending_fonts(&mut gpu.context);
        r4_begin(&mut gpu);
        native_core::presentation();
        manager.frame(snapshot);
        r4_render(&mut gpu, &scene);
        manager.after_render();
        if exported.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let exported_value: serde_json::Value = serde_json::from_slice(
        &fs::read(&exported)
            .unwrap_or_else(|e| panic!("footer export: {e}; {:?}", manager.diagnostics())),
    )
    .unwrap();
    assert_eq!(
        exported_value["Performance Creature"]
            .as_object()
            .unwrap()
            .len(),
        23000
    );
    for _ in 0..3 {
        manager.flush_pending_fonts(&mut gpu.context);
        r4_begin(&mut gpu);
        native_core::presentation();
        manager.frame(snapshot);
        r4_render(&mut gpu, &scene);
        manager.after_render();
    }
    assert!(
        lua.load(
            "local s=require('search');return s.message and s.message:find('Saved to ',1,true)~=nil"
        )
        .eval::<bool>()
        .unwrap(),
        "footer must report export completion"
    );
    let forge = config.0.join("Forge.json");
    fs::write(
        &forge,
        r#"{"Footer Imported":{"99999":{"world":0,"level":17,"pos":[11,12,13]}}}"#,
    )
    .unwrap();
    let j = manager
        .entries
        .iter()
        .find(|e| e.metadata.id == "eqoa_creature_journal")
        .unwrap()
        .instance
        .as_ref()
        .unwrap();
    j.host.test_player_files(Some(forge), desktop.clone());
    native_core::emit(
        "pointer.pressed",
        serde_json::json!({"x":px+(8.+44.+4.+58.+4.+29.)*k,"y":py+ph-13.*k,"inside_window":true,"button":"left"}), // the Import button's middle
    );
    let import_started = Instant::now();
    let deadline = import_started + Duration::from_secs(60);
    // The receipt lasts six seconds; incremental loading of this deliberately
    // large journal may finish later. Observe both boundaries independently.
    let mut receipt_seen = false;
    let mut reloaded = false;
    let mut last_import_state = (0usize, None::<String>);
    while Instant::now() < deadline {
        manager.flush_pending_fonts(&mut gpu.context);
        r4_begin(&mut gpu);
        native_core::presentation();
        manager.frame(snapshot);
        r4_render(&mut gpu, &scene);
        manager.after_render();
        last_import_state = lua
            .load("local s=require('search');return #s.journal,s.message")
            .eval::<(usize, Option<String>)>()
            .unwrap();
        receipt_seen |= last_import_state
            .1
            .as_deref()
            .is_some_and(|m| m.contains("Imported 1 spawns"));
        reloaded |= last_import_state.0 == 23001;
        if receipt_seen && reloaded {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        receipt_seen && reloaded,
        "footer receipt={receipt_seen}, reload={reloaded}, last={last_import_state:?}: {:?}",
        manager.diagnostics(),
    );
    eprintln!(
        "R4 footer import and shared reload: {:?}",
        import_started.elapsed()
    );
    manager.shutdown();
    input::reset_input_state();
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        saved["Performance Creature"].as_object().unwrap().len(),
        23000,
        "disable preserves the large loaded journal"
    );
}

/// Map Search's controls on the full map are sized by the full map's own layout (window pixels per frame pixel), not by
/// HUD Scale or the minimap piece: the button, the Stop/Track buttons' height and the panel keep one size at any scale and
/// stay inside the band the header leaves. Control: under the minimap the button DOES follow the scale.
#[test]
fn map_search_on_the_full_map_keeps_its_size_at_any_hud_scale() {
    big_stack(map_search_full_map_hud_scale_body)
}
fn map_search_full_map_hud_scale_body() {
    use super::super::{hud_layout, input, native_core};
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("SKIP map_search_full_map_hud_scale: no GPU");
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    for file in crate::fork_ext::bundled::EMBEDDED_FILES.iter().filter(|f| f.package == "eqoa_map_search") {
        let path = root.0.join(file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        if std::env::var_os("EQOA_R4_TEST_SOURCE").is_some() && file.path.ends_with(".lua") {
            fs::copy(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(file.path), &path).unwrap();
        } else {
            fs::write(path, file.data).unwrap();
        }
    }
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    // (button w, button h, panel w, panel y, panel h, anchor y, anchor h) with the search open
    let measure = |gpu: &mut Gpu, manager: &mut Manager, pc: bool, pct: u8, mini_piece: u32, win: (u32, u32), minimap: bool| {
        crate::settings::testing::reset(false);
        crate::settings::set_ui_mode(pc).unwrap();
        crate::settings::set_hud_scale_percent(pct);
        crate::settings::save_hud_layout(&if mini_piece == 100 { String::new() } else { format!("size minimap {mini_piece}
") });
        crate::hudlayout::testing_reset();
        crate::hideui::show();
        input::reset_input_state();
        hud_layout::set_hud_state(true, true, false, 93, 0);
        hud_layout::set_window_size(win.0, win.1);
        let mut play = crate::play::Play::bundled_ui_fixture();
        play.test_map_search_surface(!minimap);
        play.map.mini.up = minimap;
        let mut snapshot = play.forge_snapshot(None);
        snapshot.capture_pending = false;
        let map = crate::map::Map::round4_fixture(&gpu.device, &gpu.queue, win, minimap);
        // the mod's VM exists after the manager's first frame; then the panel is opened and a few frames are drawn
        for i in 0..5 {
            manager.flush_pending_fonts(&mut gpu.context);
            r4_begin_at(gpu, win);
            native_core::presentation();
            manager.frame(snapshot);
            r4_render_at(gpu, &map, win);
            manager.after_render();
            if i == 0 {
                lua(manager, "eqoa_map_search").load("require('search').open=true").exec().unwrap();
            }
        }
        let lua = lua(manager, "eqoa_map_search");
        lua.load("local s=require('search');local a=eqoa.map.get().search_anchor or {y=0,height=0};local b,p=s.button_rect,s.panel_rect;return b.w,b.h,p.w,p.y,p.h,a.y,a.height")
            .eval::<(f32, f32, f32, f32, f32, f32, f32)>()
            .unwrap_or_else(|e| panic!("no search rectangles pc={pc} {pct}% piece {mini_piece} {win:?}: {e}"))
    };
    let mut failures = Vec::new();
    for win in [(1024, 768), (1280, 720)] {
        for pc in [false, true] {
            let base = measure(&mut gpu, &mut manager, pc, 100, 100, win, false);
            let mut cases = vec![(75u8, 100u32), (150, 100)];
            if pc {
                cases.push((100, 200));
                cases.push((150, 200));
            }
            for (pct, piece) in cases {
                let got = measure(&mut gpu, &mut manager, pc, pct, piece, win, false);
                let note = format!("pc={pc} win={win:?} scale {pct}% piece {piece}%");
                for (name, a, b) in [("button w", got.0, base.0), ("button h", got.1, base.1), ("panel w", got.2, base.2)] {
                    if (a - b).abs() >= 0.51 {
                        failures.push(format!("{note}: {name} {a} vs 100% {b}"));
                    }
                }
                if got.3 < got.5 - 0.51 || got.3 + got.4 > got.5 + got.6 + 0.51 || got.4 <= 0. {
                    failures.push(format!("{note}: panel y {} h {} outside the header band y {} h {}", got.3, got.4, got.5, got.6));
                }
            }
        }
    }
    // Control: with the minimap showing the button follows the scale (so "never scale" would fail here).
    for win in [(1024, 768), (1280, 720)] {
        let small = measure(&mut gpu, &mut manager, false, 75, 100, win, true);
        let big = measure(&mut gpu, &mut manager, false, 150, 100, win, true);
        assert!(big.0 > small.0 * 1.5, "minimap search button must follow HUD Scale: {} vs {}", small.0, big.0);
    }
    assert!(failures.is_empty(), "full-map search drifts with HUD Scale:
{}", failures.join("
"));
}
