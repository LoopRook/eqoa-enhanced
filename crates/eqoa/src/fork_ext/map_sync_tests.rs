//! Sync regression: indoor render-target pictures use the circular map mask.
use super::*;

#[test]
fn an_indoor_picture_is_clipped_by_the_round_minimap_at_every_render_scale() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("SKIP indoor round-mask GPU test: no adapter");
        return;
    };
    let Ok((device, queue)) = pollster::block_on(adapter.request_device(&Default::default()))
    else {
        eprintln!("SKIP indoor round-mask GPU test: no device");
        return;
    };
    let lib = esf::Library::new();
    let size = (64, 64);
    let cx = Context {
        lib: &lib,
        device: &device,
        queue: &queue,
        format: render::TARGET_FORMAT,
        size,
        dt: 0.0,
        player: None,
        world: 0,
        origin: [0.0; 2],
        group: Vec::new(),
        in_group: false,
        target: None,
        sticks: [0.0; 4],
        keyboard: true,
        words: None,
        vui: None,
        quest_marks: None,
    };
    let mut map = Map::default();
    map.size = size;
    map.layers(&cx);
    let layers = map.layers.as_mut().unwrap();
    let texture = layers.map.target(&device, 8, 8, render::TARGET_FORMAT);
    let view = layers.map.target_view(texture).unwrap();
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("indoor fixture"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    queue.submit([enc.finish()]);
    layers.map.quads.push(MapQuad::rect(
        texture,
        [20.0, 20.0, 40.0, 40.0],
        [0.0, 0.0, 1.0, 1.0],
        [1.0; 4],
    ));
    // A second picture wholly inside the circle is the positive control.
    map.mini.circle = [30.0, 30.0, 10.0];
    map.mini.up = true;
    for scale in [1, 2, 4] {
        let target = (size.0 * scale, size.1 * scale);
        let read = |map: &Map| crate::map::tool::capture(map, &device, &queue, target).unwrap();
        let pixel =
            |bytes: &[u8], x: u32, y: u32| bytes[((y * scale * target.0 + x * scale) * 4) as usize];
        let round = read(&map);
        assert!(
            pixel(&round, 30, 30) > 200,
            "indoor floor missing at {scale}x"
        );
        assert!(
            pixel(&round, 21, 21) < 10,
            "indoor corner leaked at {scale}x"
        );
        // Disabling the circle must reveal that same floor pixel: this proves
        // the clipped corner had picture content, rather than an empty tile.
        map.mini.circle[2] = 0.0;
        let rectangle = read(&map);
        assert!(
            pixel(&rectangle, 21, 21) > 200,
            "unmasked control missing at {scale}x"
        );
        map.mini.circle[2] = 10.0;
    }
}
