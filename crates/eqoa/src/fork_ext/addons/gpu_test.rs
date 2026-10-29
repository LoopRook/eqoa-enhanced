//! Optional local-source/GPU evidence; never connects to a server.
use super::{
    bridge::Bridge,
    host::{Host, Snapshot},
    profile,
    textures::Textures,
};
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
use render::wgpu;
use std::{cell::RefCell, rc::Rc};

#[test]
fn forge_gpu_pinned_grid_and_texture_fixture() {
    let Some(source) = std::env::var_os("EQOA_FORGE_SPIKE") else {
        eprintln!("SKIP forge_gpu: set EQOA_FORGE_SPIKE to the pinned local screen_grid.lua");
        return;
    };
    let instance = wgpu::Instance::default();
    let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!("SKIP forge_gpu: no GPU adapter: {error}");
            return;
        }
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut context = Context::create();
    context
        .set_ini_filename::<std::path::PathBuf>(None)
        .unwrap();
    let renderer = Rc::new(RefCell::new(
        WgpuRenderer::new(
            WgpuInitInfo::new(
                device.clone(),
                queue.clone(),
                wgpu::TextureFormat::Rgba8Unorm,
            ),
            &mut context,
        )
        .unwrap(),
    ));
    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = Textures::new(&device, &queue, renderer.clone());
    Textures::install(&textures, &host.lua).unwrap();
    let script = host
        .compile(&std::fs::read_to_string(source).unwrap())
        .unwrap();
    let widgets = host.compile(include_str!("fixtures/widgets.lua")).unwrap();
    // Explicit fixture state, not a claim about a live game session.
    host.snapshot.set(Snapshot {
        in_game: true,
        start_menu: false,
    });
    let (width, height) = (640, 360);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Forge spike acceptance target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    for pass_index in 0..2 {
        context
            .io_mut()
            .set_display_size([width as f32, height as f32]);
        context.io_mut().set_display_framebuffer_scale([1.0, 1.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context.frame();
        bridge
            .invoke(|| {
                host.run(&script)?;
                host.run(&widgets)
            })
            .unwrap();
        let invalid = host
            .compile("UiForge.CreateTextureFromMemory('bad',1,1)")
            .unwrap();
        assert!(host.run(&invalid).is_err());
        if pass_index == 1 {
            host.run(
                &host
                    .compile("UiForge.ReleaseTexture(fixture_texture)")
                    .unwrap(),
            )
            .unwrap();
            let stale = host.compile("ImGui.Image(fixture_texture,32,32)").unwrap();
            assert!(
                bridge
                    .invoke(|| host.run(&stale))
                    .unwrap_err()
                    .to_string()
                    .contains("expired")
            );
            // The valid Image already emitted this frame must still render.
        }
        // The grid's callbacks roundtrip real saved values with numeric table keys.
        let saved = host.callback(2, mlua::Value::Nil).unwrap();
        assert_eq!(
            saved.as_table().unwrap().get::<i32>("h_spacing").unwrap(),
            60
        );
        host.callback(
            3,
            profile::decode(&host.lua, &profile::encode(saved).unwrap()).unwrap(),
        )
        .unwrap();
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut renderer = renderer.borrow_mut();
            let frame = context.render(renderer.renderer_consumer().unwrap());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Forge spike acceptance"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderer
                .render(frame, &mut pass, FramebufferExtent::new(width, height))
                .unwrap();
        }
        if pass_index == 0 {
            queue.submit([encoder.finish()]);
            continue;
        }
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (width * height * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        let (send, recv) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        recv.recv().unwrap().unwrap();
        let pixels = buffer.slice(..).get_mapped_range().unwrap();
        let red = (10..80).any(|y| {
            (318..323).any(|x| {
                let i = (y * width + x) as usize * 4;
                pixels[i] > 180 && pixels[i + 1] < 60 && pixels[i + 2] < 60
            })
        });
        assert!(red, "unchanged grid must draw its red physical center line");
        assert!(
            pixels
                .chunks_exact(4)
                .any(|p| p[0] < 40 && p[1] > 220 && p[2] < 40),
            "texture fixture must render green RGBA pixels"
        );
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/forge-spike-grid.png");
        let mut png = png::Encoder::new(std::fs::File::create(&out).unwrap(), width, height);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        eprintln!("Forge GPU fixture captured {}", out.display());
    }
    textures.borrow_mut().collect(true);
}
