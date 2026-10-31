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

/// A synthetic icon provider for testing.
struct SyntheticProvider {
    surfaces: HashMap<u32, DecodedIcon>,
    generation: u64,
}

impl SyntheticProvider {
    fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            generation: 0,
        }
    }

    fn add(&mut self, id: u32, w: u32, h: u32, rgba: Vec<u8>) {
        self.surfaces.insert(id, DecodedIcon { width: w, height: h, rgba });
    }
}

impl IconProvider for SyntheticProvider {
    fn decode_icon(&self, id: u32) -> Option<DecodedIcon> {
        self.surfaces.get(&id).cloned()
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
                // Inner 22x22 icon content (RED)
                rgba[o] = 255;
                rgba[o + 1] = 0;
                rgba[o + 2] = 0;
                rgba[o + 3] = 255;
            } else {
                // Flat border padding (GREEN)
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

    // Request the icon with trim_transparent and trim_color
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

    // Render offscreen using draw_list:AddImage
    gpu.context.io_mut().set_display_size([64.0, 64.0]);
    gpu.context
        .io_mut()
        .set_display_framebuffer_scale([1.0, 1.0]);
    gpu.context.frame();

    bridge
        .invoke(|| {
            host.lua
                .load(
                    r#"
                    local draw_list = ImGui.GetBackgroundDrawList()
                    local white = ImGui.GetColorU32(1, 1, 1, 1)
                    draw_list:AddImage(_G.test_tex, ImVec2.new(0, 0), ImVec2.new(22, 22), ImVec2.new(0, 0), ImVec2.new(1, 1), white)
                    "#,
                )
                .exec()
        })
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
    // Pixel (10, 10) inside the 22x22 drawn quad should be RED [255, 0, 0, 255]
    let o = (10 * 64 + 10) * 4;
    assert_eq!(data[o], 255, "R channel must be 255");
    assert_eq!(data[o + 1], 0, "G channel must be 0");
    assert_eq!(data[o + 2], 0, "B channel must be 0");
    assert_eq!(data[o + 3], 255, "A channel must be 255");

    drop(data);
    readback.unmap();
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

    // 1. Untrimmed: full 32x32
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

    // 2. Trim color: crops away the green border to 22x22
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

    // 3. Trim alias: { trim = true } works identically to trim_transparent
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
    // Since make_test_icon_surface has no transparent pixels (only green border),
    // trim_transparent alone leaves it 32x32:
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

    // Query unmapped hash 999999
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

    // Query 0 and nil
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

    // Zero texture allocation in bank
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

    // Request twice
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

    // Only 1 texture was allocated
    assert_eq!(textures.borrow().cached_count_for_test(), 1);

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_release_on_shutdown_and_reload() {
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
            Icon.GetTexture(4001, { trim_color = true })
            "#,
        )
        .exec()
        .unwrap();
    assert_eq!(cache.borrow().cached_count(), 1);

    // Release owner
    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            Icon.Release("wereoxxs_ui")
            "#,
        )
        .exec()
        .unwrap();

    // Cache is cleared and textures are freed
    assert_eq!(cache.borrow().cached_count(), 0);
    assert_eq!(textures.borrow().allocated_bytes_for_test(), 0);

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_lru_eviction_under_bound() {
    let mut provider = SyntheticProvider::new();
    for id in 1..=70 {
        let (w, h, rgba) = make_test_icon_surface();
        provider.add(id, w, h, rgba);
    }
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    textures.borrow_mut().set_budget(super::textures::ADDON_BUDGET);
    let cache = icons::attach_icon_cache(&host.lua, textures.clone()).unwrap();

    // Request 70 distinct icons
    host.lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            for id = 1, 70 do
                Icon.GetTexture(id, { trim_color = true })
            end
            "#,
        )
        .exec()
        .unwrap();

    // Cache count must not exceed MAX_CACHED_ICONS (64)
    assert!(
        cache.borrow().cached_count() <= MAX_CACHED_ICONS,
        "cache must be bounded to MAX_CACHED_ICONS"
    );
    assert_eq!(cache.borrow().cached_count(), MAX_CACHED_ICONS);
    assert_eq!(textures.borrow().cached_count_for_test(), MAX_CACHED_ICONS);

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_art_view_change_drops_cache() {
    let mut provider = SyntheticProvider::new();
    // Frontiers version: Red box
    provider.add(5001, 10, 10, vec![255, 0, 0, 255].repeat(100));
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    let cache = icons::attach_icon_cache(&host.lua, textures.clone()).unwrap();

    let t1_id: u64 = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex = Icon.GetTexture(5001)
            assert(tex ~= nil)
            return 1
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(t1_id, 1);
    assert_eq!(cache.borrow().cached_count(), 1);

    // F9 / 2002 art toggle increments art generation
    icons::notify_art_changed();

    // Replace provider icon with 2002 version: Blue box
    let mut provider2 = SyntheticProvider::new();
    provider2.add(5001, 10, 10, vec![0, 0, 255, 255].repeat(100));
    provider2.generation = 1;
    icons::set_custom_icon_provider(Some(Box::new(provider2)));

    // Requesting 5001 again must drop old cache and load from new generation
    let t2_id: u64 = host
        .lua
        .load(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex = Icon.GetTexture(5001)
            assert(tex ~= nil)
            return 2
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(t2_id, 2);

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_itemicon_esf_support() {
    let mut provider = SyntheticProvider::new();
    // Simulate an item icon loaded from ITEMICON.ESF (e.g. DictID 0x1000)
    let item_icon_id = 0x1000u32;
    let (w, h, rgba) = make_test_icon_surface();
    provider.add(item_icon_id, w, h, rgba);
    icons::set_custom_icon_provider(Some(Box::new(provider)));

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => return,
    };
    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    icons::attach_icon_cache(&host.lua, textures).unwrap();

    let loaded: bool = host
        .lua
        .load(&format!(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture({}, {{ trim_transparent = true, trim_color = true }})
            return tex ~= nil and w == 22 and h == 22
            "#,
            item_icon_id
        ))
        .eval()
        .unwrap();
    assert!(loaded, "item icon must decode and trim properly");

    icons::set_custom_icon_provider(None);
}

#[test]
fn forge_icon_real_disc_ability_icon_matches_stock_hud() {
    // Check EQOA_ISO or default path
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

    let ui_csf_bytes = match iso.read("UI.CSF") {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("SKIP forge_icon: UI.CSF not found on disc: {e}");
            return;
        }
    };

    let container = match esf::Container::open(ui_csf_bytes) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP forge_icon: UI.CSF parse error: {e}");
            return;
        }
    };

    // Find first surface with a valid decoded base mip
    let Some(surface) = container.surfaces.iter().find(|s| s.decode(&container.data).is_some()) else {
        eprintln!("SKIP forge_icon: no decodable surface found in UI.CSF");
        return;
    };

    let stock_img = surface.decode(&container.data).unwrap();
    let dict_id = surface.dict_id as u32;

    let mut lib = esf::Library::new();
    lib.add("UI.CSF", container);
    icons::set_live_library(&lib as *const esf::Library);

    let gpu = match TestGpu::new() {
        Some(g) => g,
        None => {
            eprintln!("SKIP forge_icon: no GPU adapter");
            icons::set_live_library(std::ptr::null());
            return;
        }
    };

    let host = Host::new().unwrap();
    let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    icons::attach_icon_cache(&host.lua, textures).unwrap();

    let (w, h): (u32, u32) = host
        .lua
        .load(&format!(
            r#"
            local Icon = require("frontiers_forge.icon")
            local tex, w, h = Icon.GetTexture({})
            assert(tex ~= nil, "real disc icon texture must not be nil")
            return w, h
            "#,
            dict_id
        ))
        .eval()
        .unwrap();

    assert_eq!(w, stock_img.width, "icon width must match stock HUD decoded width");
    assert_eq!(h, stock_img.height, "icon height must match stock HUD decoded height");

    eprintln!("SUCCESS forge_icon: real disc ability icon {dict_id} ({w}x{h}) matched stock HUD");
    icons::set_live_library(std::ptr::null());
}
