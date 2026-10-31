//! Unit and GPU integration tests for FrontiersForge ability & effect icons.

use super::{
    bridge::Bridge,
    host::Host,
    icons::{self, DecodedIcon, IconProvider, MAX_CACHED_ICONS},
    textures::Textures,
};
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
use render::wgpu;
use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

struct TestGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    context: Context,
    renderer: Rc<RefCell<WgpuRenderer>>,
}

impl TestGpu {
    fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).ok()?;
        let mut context = Context::create();
        context.set_ini_filename::<PathBuf>(None).unwrap();
        let renderer = Rc::new(RefCell::new(
            WgpuRenderer::new(
                WgpuInitInfo::new(
                    device.clone(),
                    queue.clone(),
                    wgpu::TextureFormat::Rgba8Unorm,
                ),
                &mut context,
            )
            .ok()?,
        ));
        Some(Self {
            device,
            queue,
            context,
            renderer,
        })
    }
}

/// Helper to render a quad using ImGui background draw list and read back the sampled pixel.
fn render_and_read_pixel(
    gpu: &mut TestGpu,
    bridge: &Bridge,
    host: &Host,
    lua_code: &str,
    sample_x: u32,
    sample_y: u32,
) -> [u8; 4] {
    gpu.context.io_mut().set_display_size([64.0, 64.0]);
    gpu.context
        .io_mut()
        .set_display_framebuffer_scale([1.0, 1.0]);
    gpu.context.frame();

    bridge
        .invoke(|| host.lua.load(lua_code).exec())
        .unwrap();

    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test render target"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());
    let mut encoder = gpu.device.create_command_encoder(&Default::default());

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("test pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        let frame = gpu.context.render(gpu.renderer.borrow_mut().renderer_consumer().unwrap());
        gpu.renderer
            .borrow_mut()
            .render(frame, &mut pass, FramebufferExtent::new(64, 64))
            .unwrap();
    }

    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback buffer"),
        size: 64 * 64 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(64 * 4),
                rows_per_image: Some(64),
            },
        },
        target.size(),
    );
    gpu.queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| tx.send(res).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();

    let data = slice.get_mapped_range().unwrap();
    let o = ((sample_y * 64 + sample_x) * 4) as usize;
    let pixel = [data[o], data[o + 1], data[o + 2], data[o + 3]];
    drop(data);
    readback.unmap();
    pixel
}

/// A synthetic icon provider for testing.
struct SyntheticProvider {
    surfaces: HashMap<u32, DecodedIcon>,
    ui_textures: HashMap<u32, DecodedIcon>,
    generation: u64,
}

impl SyntheticProvider {
    fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            ui_textures: HashMap::new(),
            generation: 0,
        }
    }

    fn add(&mut self, id: u32, w: u32, h: u32, rgba: Vec<u8>) {
        self.surfaces.insert(id, DecodedIcon { width: w, height: h, rgba });
    }

    fn add_ui_tex(&mut self, tex_id: u32, w: u32, h: u32, rgba: Vec<u8>) {
        self.ui_textures.insert(tex_id, DecodedIcon { width: w, height: h, rgba });
    }
}

impl IconProvider for SyntheticProvider {
    fn decode_icon(&self, id: u32) -> Option<DecodedIcon> {
        self.surfaces.get(&id).cloned()
    }

    fn decode_ui_texture(&self, tex_id: u32) -> Option<DecodedIcon> {
        self.ui_textures.get(&tex_id).cloned()
    }

    fn generation(&self) -> u64 {
        self.generation
    }
}

/// Helper to create a 32x32 RGBA test icon with:
/// - A flat green border (padding color) of 5px
/// - An inner 22x22 red box ([255, 0, 0, 255])
fn make_test_icon_surface() -> (u32, u32, Vec<u8>) {
    let (w, h) = (32u32, 32u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let o = ((y * w + x) * 4) as usize;
            if x >= 5 && x < 27 && y >= 5 && y < 27 {
                rgba[o] = 255;
                rgba[o + 1] = 0;
                rgba[o + 2] = 0;
                rgba[o + 3] = 255;
            } else {
                rgba[o] = 0;
                rgba[o + 1] = 255;
                rgba[o + 2] = 0;
                rgba[o + 3] = 255;
            }
        }
    }
    (w, h, rgba)
}

#[test]
fn forge_icon_synthetic_surface_pixel_match() {
    let mut gpu = match TestGpu::new() {
        Some(g) => g,
        None => {
            eprintln!("SKIP forge_icon: no GPU adapter");
            return;
        }
    };

    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    icons::attach_icon_cache(&host.lua, textures.clone()).unwrap();

    let mut provider = SyntheticProvider::new();
    let (w, h, rgba) = make_test_icon_surface();
    provider.add(1001, w, h, rgba);
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let res: mlua::Value = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture(1001, { trim_transparent = true, trim_color = true })
            assert(tex ~= nil, "tex must not be nil")
            assert(w == 22, "width must be 22")
            assert(h == 22, "height must be 22")
            _G.test_tex = tex
            return true
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(res, mlua::Value::Boolean(true));

    let pixel = render_and_read_pixel(
        &mut gpu,
        &bridge,
        &host,
        r#"
        local draw_list = ImGui.GetBackgroundDrawList()
        local white = ImGui.GetColorU32(1, 1, 1, 1)
        draw_list:AddImage(_G.test_tex, ImVec2.new(0, 0), ImVec2.new(22, 22), ImVec2.new(0, 0), ImVec2.new(1, 1), white)
        "#,
        10,
        10,
    );
    assert_eq!(pixel, [255, 0, 0, 255], "rendered pixel at (10, 10) must be RED");

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_trim_options_change_dimensions() {
    let mut provider = SyntheticProvider::new();
    let (w, h, rgba) = make_test_icon_surface();
    provider.add(2001, w, h, rgba);
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    icons::attach_icon_cache(&host.lua, textures).unwrap();

    let (w_untrimmed, h_untrimmed): (u32, u32) = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture(2001)
            return w, h
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!((w_untrimmed, h_untrimmed), (32, 32));

    let (w_trimmed, h_trimmed): (u32, u32) = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture(2001, { trim_color = true })
            return w, h
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!((w_trimmed, h_trimmed), (22, 22));

    let (w_alias, h_alias): (u32, u32) = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture(2001, { trim = true })
            return w, h
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!((w_alias, h_alias), (32, 32));

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_miss_returns_nil_tuple_with_no_allocation() {
    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    icons::attach_icon_cache(&host.lua, textures.clone()).unwrap();

    let is_nil: bool = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture(999999)
            return tex == nil and w == nil and h == nil
            "#,
        )
        .eval()
        .unwrap();
    assert!(is_nil, "miss must return nil tuple");

    let zero_is_nil: bool = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local t1 = Icon.GetTexture(0)
            local t2 = Icon.GetTexture(nil)
            return t1 == nil and t2 == nil
            "#,
        )
        .eval()
        .unwrap();
    assert!(zero_is_nil, "0 and nil must return nil");

    assert_eq!(textures.borrow().cached_count_for_test(), 0);
    assert_eq!(textures.borrow().allocated_bytes_for_test(), 0);
}

#[test]
fn forge_icon_cache_reuse() {
    let mut provider = SyntheticProvider::new();
    let (w, h, rgba) = make_test_icon_surface();
    provider.add(3001, w, h, rgba);
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    icons::attach_icon_cache(&host.lua, textures.clone()).unwrap();

    let same: bool = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local t1 = Icon.GetTexture(3001, { trim_color = true })
            local t2 = Icon.GetTexture(3001, { trim_color = true })
            return t1 ~= nil and t1 == t2
            "#,
        )
        .eval()
        .unwrap();
    assert!(same, "cache hit must return the same texture handle");
    assert_eq!(textures.borrow().cached_count_for_test(), 1);

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_held_handle_survives_eviction_pressure_and_f9_draws_valid_pixels() {
    let mut gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };

    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    textures.borrow_mut().set_budget(super::textures::ADDON_BUDGET);
    let cache = icons::attach_icon_cache(&host.lua, textures.clone()).unwrap();

    let mut provider = SyntheticProvider::new();
    // ID 1: 10x10 RED box [255, 0, 0, 255]
    provider.add(1, 10, 10, vec![255, 0, 0, 255].repeat(100));
    // IDs 2..=75: 10x10 GREEN boxes [0, 255, 0, 255]
    for id in 2..=75 {
        provider.add(id, 10, 10, vec![0, 255, 0, 255].repeat(100));
    }
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    // Lua acquires owner and loads icon 1 into _G.h1
    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            Icon.Acquire("wereoxxs_ui")
            local h1, w, h = Icon.GetTexture(1)
            assert(h1 ~= nil)
            _G.h1 = h1
            "#,
        )
        .exec()
        .unwrap();

    // Forced eviction pressure (> 64 distinct icons requested)
    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            for i = 2, 75 do
                Icon.GetTexture(i)
            end
            "#,
        )
        .exec()
        .unwrap();

    // Icon 1 is still referenced because _G.h1 holds a live token
    assert!(cache.borrow().cached_count() <= MAX_CACHED_ICONS);

    // Simulate F9 / 2002 art toggle
    icons::notify_art_changed();

    // New provider generation where icon 1 is BLUE [0, 0, 255, 255]
    let mut provider2 = SyntheticProvider::new();
    provider2.add(1, 10, 10, vec![0, 0, 255, 255].repeat(100));
    provider2.generation = 1;
    icons::set_custom_icon_provider(Some(Box::new(provider2)));

    // Lua requests icon 1 under the new generation
    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local h1_new = Icon.GetTexture(1)
            assert(h1_new ~= nil)
            _G.h1_new = h1_new
            "#,
        )
        .exec()
        .unwrap();

    // Drawing _G.h1 must not error ("expired texture handle"), and must draw original RED
    let p_old = render_and_read_pixel(
        &mut gpu,
        &bridge,
        &host,
        r#"
        local draw_list = ImGui.GetBackgroundDrawList()
        local white = ImGui.GetColorU32(1, 1, 1, 1)
        draw_list:AddImage(_G.h1, ImVec2.new(0, 0), ImVec2.new(10, 10), ImVec2.new(0, 0), ImVec2.new(1, 1), white)
        "#,
        5,
        5,
    );
    assert_eq!(p_old, [255, 0, 0, 255], "held handle across eviction and F9 must draw valid RED pixels");

    // Drawing _G.h1_new must draw the newly requested BLUE pixels
    let p_new = render_and_read_pixel(
        &mut gpu,
        &bridge,
        &host,
        r#"
        local draw_list = ImGui.GetBackgroundDrawList()
        local white = ImGui.GetColorU32(1, 1, 1, 1)
        draw_list:AddImage(_G.h1_new, ImVec2.new(0, 0), ImVec2.new(10, 10), ImVec2.new(0, 0), ImVec2.new(1, 1), white)
        "#,
        5,
        5,
    );
    assert_eq!(p_new, [0, 0, 255, 255], "fresh handle under new art view must draw BLUE pixels");

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_disable_and_reenable_preserves_held_handle() {
    let mut gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };

    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    textures.borrow_mut().set_budget(super::textures::ADDON_BUDGET);
    icons::attach_icon_cache(&host.lua, textures).unwrap();

    let mut provider = SyntheticProvider::new();
    provider.add(100, 10, 10, vec![255, 128, 0, 255].repeat(100));
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            Icon.Acquire("wereoxxs_ui")
            local tex = Icon.GetTexture(100)
            assert(tex ~= nil)
            _G.held_c = tex

            -- Disable addon (Cleanup -> Icon.Release)
            Icon.Release("wereoxxs_ui")

            -- Re-enable addon (Init -> Icon.Acquire)
            Icon.Acquire("wereoxxs_ui")
            "#,
        )
        .exec()
        .unwrap();

    // Drawing held_c must succeed without expired texture handle error
    let pixel = render_and_read_pixel(
        &mut gpu,
        &bridge,
        &host,
        r#"
        local draw_list = ImGui.GetBackgroundDrawList()
        local white = ImGui.GetColorU32(1, 1, 1, 1)
        draw_list:AddImage(_G.held_c, ImVec2.new(0, 0), ImVec2.new(10, 10), ImVec2.new(0, 0), ImVec2.new(1, 1), white)
        "#,
        5,
        5,
    );
    assert_eq!(pixel, [255, 128, 0, 255]);

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_shutdown_and_reload_releases_all_textures() {
    let mut provider = SyntheticProvider::new();
    let (w, h, rgba) = make_test_icon_surface();
    provider.add(4001, w, h, rgba);
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    textures.borrow_mut().set_budget(super::textures::ADDON_BUDGET);
    let cache = icons::attach_icon_cache(&host.lua, textures.clone()).unwrap();

    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            Icon.Acquire("wereoxxs_ui")
            _G.temp = Icon.GetTexture(4001, { trim_color = true })
            "#,
        )
        .exec()
        .unwrap();
    assert_eq!(cache.borrow().cached_count(), 1);
    assert_eq!(textures.borrow().cached_count_for_test(), 1);

    // Drop Lua variable and collect GC
    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            _G.temp = nil
            Icon.Release("wereoxxs_ui")
            "#,
        )
        .exec()
        .unwrap();
    host.lua.gc_collect().unwrap();
    cache.borrow_mut().drop_cache();

    // Cache count is 0, textures count is 0, allocated bytes is 0
    assert_eq!(cache.borrow().cached_count(), 0);
    assert_eq!(textures.borrow().cached_count_for_test(), 0);
    assert_eq!(textures.borrow().allocated_bytes_for_test(), 0);

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_budget_exhaustion_diagnostic() {
    let mut provider = SyntheticProvider::new();
    for id in 1..=70 {
        provider.add(id, 10, 10, vec![255, 255, 255, 255].repeat(100));
    }
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    textures.borrow_mut().set_budget(super::textures::ADDON_BUDGET);
    let cache = icons::attach_icon_cache(&host.lua, textures).unwrap();

    // Hold all 64 icons in Lua table
    let count: usize = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            Icon.Acquire("test_mod")
            _G.held_table = {}
            for id = 1, 64 do
                _G.held_table[id] = Icon.GetTexture(id)
            end
            -- Request 65th icon: all 64 entries are live and cannot be evicted!
            local t65, w65, h65 = Icon.GetTexture(65)
            assert(t65 == nil and w65 == nil and h65 == nil, "budget exhaustion must return nil tuple")
            return #_G.held_table
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(count, 64);
    assert_eq!(cache.borrow().cached_count(), MAX_CACHED_ICONS);

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_get_ui_texture() {
    let mut provider = SyntheticProvider::new();
    // Synthetic UI texture 10: 16x16 YELLOW box [255, 255, 0, 255]
    provider.add_ui_tex(10, 16, 16, vec![255, 255, 0, 255].repeat(256));
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    icons::attach_icon_cache(&host.lua, textures).unwrap();

    let (w, h): (u32, u32) = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetUITexture(10)
            assert(tex ~= nil, "GetUITexture must return texture")
            return w, h
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!((w, h), (16, 16));

    // Request missing UI texture
    let miss: bool = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetUITexture(9999)
            return tex == nil and w == nil and h == nil
            "#,
        )
        .eval()
        .unwrap();
    assert!(miss, "missing UI texture must return nil tuple");

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_real_disc_ui_and_item_icon_pixel_match() {
    let iso_path = std::env::var("EQOA_ISO").ok().or_else(|| {
        let p = "C:/Users/hocka/Documents/PS2 Games/EverQuest - Online Adventures - Frontiers (USA).iso";
        if std::path::Path::new(p).exists() {
            Some(p.to_string())
        } else {
            None
        }
    });

    let Some(path) = iso_path else {
        eprintln!("SKIP forge_icon: disc ISO not found");
        return;
    };

    let mut iso = match iso::Iso::open(&path) {
        Ok(iso) => iso,
        Err(e) => {
            eprintln!("SKIP forge_icon: failed to open disc {path}: {e}");
            return;
        }
    };

    // Note: SPELLICON.ESF is not present on the EQOA Frontiers disc.
    // The disc stores UI and ability plate textures in DATA2/UI.CSF and item icons in DATA/ITEMICON.ESF.
    let ui_bytes = match iso.read("UI.CSF") {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("SKIP forge_icon: UI.CSF not found on disc: {e}");
            return;
        }
    };

    let item_bytes = match iso.read("ITEMICON.ESF") {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("SKIP forge_icon: ITEMICON.ESF not found on disc: {e}");
            return;
        }
    };

    let ui_container = match esf::Container::open(ui_bytes) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP forge_icon: UI.CSF parse error: {e}");
            return;
        }
    };

    let item_container = match esf::Container::open(item_bytes) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP forge_icon: ITEMICON.ESF parse error: {e}");
            return;
        }
    };

    // Find first decodable ability/UI surface in UI.CSF
    let Some(ability_surface) = ui_container.surfaces.iter().find(|s| s.decode(&ui_container.data).is_some()) else {
        eprintln!("SKIP forge_icon: no decodable surface in UI.CSF");
        return;
    };
    let stock_ability = ability_surface.decode(&ui_container.data).unwrap();
    let ability_id = ability_surface.dict_id as u32;

    // Find first decodable item surface in ITEMICON.ESF
    let Some(item_surface) = item_container.surfaces.iter().find(|s| s.decode(&item_container.data).is_some()) else {
        eprintln!("SKIP forge_icon: no decodable surface in ITEMICON.ESF");
        return;
    };
    let stock_item = item_surface.decode(&item_container.data).unwrap();
    let item_id = item_surface.dict_id as u32;

    let mut lib = esf::Library::new();
    lib.add("UI.CSF", ui_container);
    lib.add("ITEMICON.ESF", item_container);
    icons::update_live_data(&lib, None);

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => {
            eprintln!("SKIP forge_icon: no GPU adapter");
            return;
        }
    };

    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    icons::attach_icon_cache(&host.lua, textures).unwrap();

    // Query ability icon via Lua
    let (ab_w, ab_h): (u32, u32) = host
        .lua
        .load(&format!(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture({})
            assert(tex ~= nil, "ability icon texture must not be nil")
            return w, h
            "#,
            ability_id
        ))
        .eval()
        .unwrap();
    assert_eq!(ab_w, stock_ability.width);
    assert_eq!(ab_h, stock_ability.height);

    // Byte-for-byte pixel comparison for ability icon against stock HUD decode path
    icons::with_active_provider(|p| {
        let decoded = p.decode_icon(ability_id).expect("provider must decode ability icon");
        assert_eq!(
            decoded.rgba,
            stock_ability.rgba,
            "ability icon pixels must match stock HUD decode path byte-for-byte"
        );
    });

    // Query item icon via Lua
    let (it_w, it_h): (u32, u32) = host
        .lua
        .load(&format!(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture({})
            assert(tex ~= nil, "item icon texture must not be nil")
            return w, h
            "#,
            item_id
        ))
        .eval()
        .unwrap();
    assert_eq!(it_w, stock_item.width);
    assert_eq!(it_h, stock_item.height);

    // Byte-for-byte pixel comparison for item icon against stock HUD decode path
    icons::with_active_provider(|p| {
        let decoded = p.decode_icon(item_id).expect("provider must decode item icon");
        assert_eq!(
            decoded.rgba,
            stock_item.rgba,
            "item icon pixels must match stock HUD decode path byte-for-byte"
        );
    });

    let snap_size = icons::snapshot_size_bytes();
    assert!(snap_size > 0, "snapshot size must be greater than zero");
    eprintln!(
        "SUCCESS forge_icon: real disc ability icon {} ({}x{}) and item icon {} ({}x{}) matched stock HUD decode path byte-for-byte (snapshot: {} bytes / {:.2} MiB)",
        ability_id, ab_w, ab_h, item_id, it_w, it_h, snap_size, snap_size as f64 / (1024.0 * 1024.0)
    );
}

#[test]
fn forge_icon_zone_streaming_does_not_trigger_resnapshot() {
    let mut lib = esf::Library::new();
    let mut ui_c = esf::Container::empty();
    ui_c.surfaces.push(esf::Surface {
        dict_id: 100,
        width: 32,
        height: 32,
        depth: 1,
        mips: 1,
        pal_off: 0,
        pal_entries: 0,
        row_bytes: 128,
        pix_off: 0,
        bpp: 32,
    });
    ui_c.data = vec![0u8; 1024];
    lib.add("UI.CSF", ui_c);

    // Initial update snapshots UI.CSF
    icons::update_live_data(&lib, None);
    let count1 = icons::snapshot_count();
    let bytes1 = icons::snapshot_size_bytes();
    assert_eq!(bytes1, 1024);

    // Simulate zone streaming: add multiple unrelated world/zone containers
    for i in 1..=10 {
        let mut zone_c = esf::Container::empty();
        zone_c.data = vec![0u8; 100_000]; // 100 KB each (1 MB total)
        lib.add(format!("TUNARIA.ESF#{i}+"), zone_c);
    }

    // Call update_live_data again as happens per frame
    icons::update_live_data(&lib, None);

    // Snapshot count must NOT increase, and snapshot size must remain only the icon containers
    assert_eq!(
        icons::snapshot_count(),
        count1,
        "adding unrelated files (zone streaming) must NOT trigger a re-snapshot or copy"
    );
    assert_eq!(
        icons::snapshot_size_bytes(),
        bytes1,
        "snapshot size must not clone streamed zone containers"
    );
}

#[test]
fn forge_icon_ui_texture_budget_does_not_starve_get_texture() {
    let mut provider = SyntheticProvider::new();
    // 64 distinct UI textures
    for id in 0..64 {
        provider.add_ui_tex(id, 16, 16, vec![255, 255, 0, 255].repeat(256));
    }
    // 1 ability icon
    let (w, h, rgba) = make_test_icon_surface();
    provider.add(9001, w, h, rgba);
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    textures.borrow_mut().set_budget(super::textures::ADDON_BUDGET);
    let cache = icons::attach_icon_cache(&host.lua, textures).unwrap();

    // Fill all 64 UI texture slots (host.execute resets the instruction budget to 200,000)
    host.execute(
        r#"
        local Icon = require("frontiers_forge.icon")
        Icon.Acquire("ui_test")
        _G.ui_held = {}
        for id = 0, 63 do
            _G.ui_held[id + 1] = Icon.GetUITexture(id)
            assert(_G.ui_held[id + 1] ~= nil)
        end
        "#,
    )
    .unwrap();
    assert_eq!(cache.borrow().ui_cached_count(), 64);

    // Calling GetTexture must NOT return nil (it has its own independent 64-texture budget)
    let (icon_tex_loaded, ab_w, ab_h): (bool, u32, u32) = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture(9001, { trim_color = true })
            return tex ~= nil, w, h
            "#,
        )
        .eval()
        .unwrap();
    assert!(icon_tex_loaded, "filling UI textures must not starve GetTexture");
    assert_eq!((ab_w, ab_h), (22, 22));

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_after_render_cleans_unreferenced_stale_textures() {
    let mut provider = SyntheticProvider::new();
    provider.add(1001, 10, 10, vec![255, 0, 0, 255].repeat(100));
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    textures.borrow_mut().set_budget(super::textures::ADDON_BUDGET);
    let cache = icons::attach_icon_cache(&host.lua, textures.clone()).unwrap();

    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            Icon.Acquire("stale_test")
            _G.stale_held = Icon.GetTexture(1001)
            assert(_G.stale_held ~= nil)
            "#,
        )
        .exec()
        .unwrap();

    // F9 / art change
    icons::notify_art_changed();

    // Provider gen 1
    let mut provider2 = SyntheticProvider::new();
    provider2.add(1001, 10, 10, vec![0, 0, 255, 255].repeat(100));
    provider2.generation = 1;
    icons::set_custom_icon_provider(Some(Box::new(provider2)));

    // Requesting under gen 1 moves old texture to stale_textures
    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            _G.fresh_held = Icon.GetTexture(1001)
            assert(_G.fresh_held ~= nil)
            "#,
        )
        .exec()
        .unwrap();
    assert_eq!(cache.borrow().stale_count(), 1);

    // Drop the held old handle in Lua and collect GC
    host.lua
        .load(
            r#"
            _G.stale_held = nil
            "#,
        )
        .exec()
        .unwrap();
    host.lua.gc_collect().unwrap();

    // Call cleanup_active_stale (as called per-frame by after_render) WITHOUT calling GetTexture
    icons::cleanup_active_stale();

    // Stale texture must be cleaned up immediately
    assert_eq!(
        cache.borrow().stale_count(),
        0,
        "stale textures without live Lua handles must be freed during after_render without GetTexture"
    );

    icons::set_custom_icon_provider(None);
}
