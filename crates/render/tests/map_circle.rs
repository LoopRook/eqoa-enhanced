//! Pixel checks for actual GPU masking, including unmasked popup controls.
use render::{
    map::{MapLayer, MapQuad},
    ui::{Quad, Ui, WHOLE, frame_from_window},
    wgpu,
};

#[test]
fn map_and_ui_circle_masks_preserve_regular_views_and_outside_controls() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("SKIP: no GPU adapter for circular map pixel checks");
        return;
    };
    eprintln!("circle pixel checks on {}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut map = MapLayer::new(&device, render::TARGET_FORMAT);
    let white = map.texture(&device, 1, 1, 1);
    map.write(&queue, white, [0, 0], 1, 1, &[255; 4]);
    // The 64 logical pixels cover a 128-pixel render target. The mask must
    // follow the projected geometry rather than unscaled fragment pixels.
    map.quads.push(MapQuad::rect(
        white,
        [0.0, 0.0, 64.0, 64.0],
        [0.0, 0.0, 1.0, 1.0],
        [1.0; 4],
    ));
    let mut ui = Ui::new(&device, render::TARGET_FORMAT, render::DEPTH_FORMAT);
    ui.begin((128, 128));
    let tex = ui.add_texture(&device, &queue, 1, 1, &[255; 4]);
    let make = |rect: [f32; 4], color: [f32; 4]| {
        let a = frame_from_window((128, 128), [rect[0], rect[1]]);
        let b = frame_from_window((128, 128), [rect[2], rect[3]]);
        Quad {
            tex,
            blend: 3,
            pos: [[a[0], a[1]], [b[0], a[1]], [b[0], b[1]], [a[0], b[1]]],
            uv: [[0.5; 2]; 4],
            color,
            clip: Some(WHOLE),
        }
    };
    ui.push_whole(make([0.0, 0.0, 128.0, 128.0], [1.0, 0.0, 0.0, 1.0]));
    ui.push_whole(make([108.0, 108.0, 126.0, 126.0], [0.0, 1.0, 0.0, 1.0]));
    for mode in 0..3 {
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("circle test"),
            size: wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: render::TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let depth = render::depth_view(&device, 128, 128);
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("circle test"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            match mode {
                0 => map.draw(&device, &mut pass, (64, 64)),
                1 => map.draw_circle(&device, &mut pass, (64, 64), [32.0, 32.0, 20.0]),
                _ => {
                    ui.draw_circle_range(&device, &mut pass, (128, 128), 0..1, [64.0, 64.0, 40.0]);
                    ui.draw_range(&device, &mut pass, (128, 128), 1..2);
                }
            }
        }
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("circle pixels"),
            size: 128 * 512,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(512),
                    rows_per_image: Some(128),
                },
            },
            wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let pixels = buf.slice(..).get_mapped_range().unwrap();
        let at = |x: usize, y: usize| &pixels[(y * 128 + x) * 4..(y * 128 + x) * 4 + 3];
        assert!(
            at(64, 64).iter().any(|c| *c > 200),
            "center is drawn, mode {mode}"
        );
        if mode == 0 {
            assert_eq!(at(8, 8), &[255, 255, 255], "world map remains rectangular");
        } else {
            assert_eq!(
                at(8, 8),
                &[0, 0, 0],
                "square corners are discarded, mode {mode}"
            );
            assert!(
                at(96, 64).iter().any(|c| *c > 200),
                "inside rim remains drawn, mode {mode}"
            );
            assert_eq!(at(108, 64), &[0, 0, 0], "outside rim is clear, mode {mode}");
        }
        if mode == 2 {
            assert_eq!(
                at(116, 116),
                &[0, 255, 0],
                "popup/control ranges remain visible outside the circle"
            );
        }
    }
}
