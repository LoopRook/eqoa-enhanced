use super::*;
use crate::ui::Layout;
use render::ui::{Anchor,Vertical};
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

fn test_view() -> crate::statbox::View {
    let mut v = crate::statbox::View::default();
    for col in 0..2 {
        for row in 0..8 {
            v.rows[col][row] = crate::statbox::Row {
                word: crate::statbox::ROWS[col][row].0,
                value: 50 + col as i32 * 10 + row as i32 * 5,
                without: Some((50, 50)),
            };
        }
    }
    v
}


#[test]
fn capture_develop_native_round3() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e|e.into_inner());
    let (mut ui, _, device, queue) = layer_with_device().expect("GPU required to produce references");
    let assets = test_assets(&mut ui, &device, &queue);
    let vui = test_vui();
    for size in [(640,480),(1280,720)] {
        for name in ["stats_box","target_of_target","mana_bars","hud_numbers"] {
            ui.begin(size);
            {
                let mut r = assets.raster(&mut ui);
                // Entry state established by native Hud::draw before these routines.
                r.ui.anchor=Anchor::Left; r.ui.vertical=Vertical::Top;
                let mut hud = Hud::new();
                hud.glow0=1.0;
                match name {
                    "stats_box" => crate::statbox::draw(&vui,&mut r,&test_view()),
                    "target_of_target" => {
                        let state=HudState {target_name:Some("Casil".into()),target_of_target:Some("Luck".into()),tot_hp:Some(204),tot_power:Some(128),..Default::default()};
                        r.ui.vertical=Vertical::Bottom;
                        hud.draw_target_of_target(&vui,&mut r,TOT_AT,&state);
                        for g in target_gauges(&state) { draw_gauge(&vui,&mut r,g.bar,136,g.at,g.lit,g.v); }
                    },
                    "mana_bars" => {
                        let member=|id,power| MemberState {id,name:"member".into(),power:Some(power),is_target:false,hp:Some((200,false,false)),marks:0};
                        let state=HudState {group:vec![member(10,230),member(11,102)],target_name:Some("Boss".into()),target_power:Some(153),..Default::default()};
                        for g in member_gauges(&state) {draw_gauge(&vui,&mut r,g.bar,136,g.at,g.lit,g.v);}
                        r.ui.vertical=Vertical::Bottom;
                        for g in target_gauges(&state) {draw_gauge(&vui,&mut r,g.bar,136,g.at,g.lit,g.v);}
                    },
                    "hud_numbers" => {
                        let state=HudState {has_self:true,self_hp:255,power:80,max_power:100,level:25,exp:1500,target_name:Some("Orc".into()),target_hp:153,..Default::default()};
                        fork_hud::draw(&mut r,&state);
                    },
                    _=>unreachable!()
                }
            }
            ui.end();
            let pixels=render_ui_to_pixels(&ui,&device,&queue,size);
            let out=std::path::PathBuf::from(std::env::var_os("EQOA_NATIVE_CAPTURE_OUT").unwrap());
            std::fs::create_dir_all(&out).unwrap();
            let f=std::fs::File::create(out.join(format!("r3-native-{name}-{}x{}.png",size.0,size.1))).unwrap();
            let mut enc=png::Encoder::new(f,size.0,size.1);
            enc.set_color(png::ColorType::Rgba);enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&pixels).unwrap();
        }
    }
}

