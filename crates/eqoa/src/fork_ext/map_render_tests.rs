//! The world map keeps window coordinates when drawn into a Render Scale target.
//! Upstream's `tool::capture` draws window and target at one size, so this takes both.
use super::*;
use render::wgpu;

fn capture(map: &Map, device: &wgpu::Device, queue: &wgpu::Queue, window: (u32, u32), size: (u32, u32)) -> Vec<u8> {
    let (w, h) = size;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("map scale capture"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: render::TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = render::depth_view(device, w, h);
    let row = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    let tv = target.create_view(&Default::default());
    {
        let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &tv, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    map.draw(device, &mut enc, &tv, &depth, window, size);
    enc.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let data = buf.slice(..).get_mapped_range().unwrap();
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize {
        px.extend_from_slice(&data[y * row as usize..y * row as usize + w as usize * 4]);
    }
    px
}

#[test]
fn world_map_stays_in_window_coordinates_at_render_scales() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
        eprintln!("SKIP world map render-scale GPU test: no adapter");
        return;
    };
    let Ok((device, queue)) = pollster::block_on(adapter.request_device(&Default::default())) else {
        eprintln!("SKIP world map render-scale GPU test: no device");
        return;
    };
    let lib = esf::Library::new();
    for size in [(64, 64), (80, 48)] {
        let cx = Context { lib: &lib, device: &device, queue: &queue, format: render::TARGET_FORMAT, size, dt: 0.0, player: None, world: 0, origin: [0.0; 2], group: Vec::new(), in_group: false, target: None, sticks: [0.0; 4], keyboard: true, words: None, vui: None, quest_marks: None };
        let mut map = Map::default();
        map.size = size;
        map.open = true;
        map.layers(&cx);
        let layers = map.layers.as_mut().unwrap();
        layers.map.quads.push(render::map::MapQuad::rect(layers.white, [0.0, 0.0, size.0 as f32, size.1 as f32], [0.0, 0.0, 1.0, 1.0], [0.0, 0.0, 1.0, 1.0]));
        let (x0, y0, x1, y1) = (size.0 as f32 * 0.5, size.1 as f32 * 0.5, size.0 as f32 - 4.0, size.1 as f32 - 4.0);
        layers.map.quads.push(render::map::MapQuad::rect(layers.white, [x0, y0, x1, y1], [0.0, 0.0, 1.0, 1.0], [1.0, 0.0, 0.0, 1.0]));
        let (ux0, uy0, ux1, uy1) = (x0 + 4.0, y0 + 4.0, x1 - 4.0, y1 - 4.0);
        let ui_pts = [[ux0, uy0], [ux1, uy0], [ux1, uy1], [ux0, uy1]].map(|p| render::ui::frame_from_window(size, p));
        layers.ui.quads.push(render::ui::Quad { tex: layers.assets.white.unwrap(), blend: 3, pos: ui_pts, uv: [[0.5, 0.5]; 4], color: [0.0, 1.0, 0.0, 1.0], clip: None });
        for k in [1, 2, 4, 2, 1] {
            let target = (size.0 * k, size.1 * k);
            let pixels = capture(&map, &device, &queue, size, target);
            let pixel = |x: u32, y: u32| {
                let i = ((y * target.0 + x) * 4) as usize;
                [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
            };
            assert!(pixel(4 * k, 4 * k)[2] > 200, "backdrop top-left missing at size {size:?} scale {k}");
            assert!(pixel((size.0 - 2) * k, (size.1 - 2) * k)[2] > 200, "backdrop bottom-right missing at size {size:?} scale {k}");
            assert!(pixel((x0 as u32 + 2) * k, (y0 as u32 + 2) * k)[0] > 200, "terrain missing at size {size:?} scale {k}");
            assert!(pixel(((ux0 + ux1) * 0.5) as u32 * k, ((uy0 + uy1) * 0.5) as u32 * k)[1] > 200, "UI quad misaligned at size {size:?} scale {k}");
        }
    }
}
