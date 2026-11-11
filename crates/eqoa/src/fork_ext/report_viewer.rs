// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
// Upstream portions up to 1e23e80 remain MIT (NOTICE).
//! Native report adapter: input ownership and frame capture.
use super::*;
use crate::report;

/// Whether the local report takes this key: every key while its dialog is
/// open, and F3, except in the world of a server that offered bug reports
/// (`Session::bug_reports`), where F3 is Support's request form
/// (`Play::open_bug_report`, in `keyboard`, which `key_input` reaches).
fn claims_key(open: bool, key: &Key, play: Option<&crate::play::Play>) -> bool {
    open || (report::is_report_key(key) && !play.is_some_and(|p| p.session.bug_reports))
}

pub(super) fn event(app: &mut App, el: &ActiveEventLoop, event: &WindowEvent) -> bool {
    match event {
        WindowEvent::KeyboardInput { event, .. }
            if claims_key(
                report::is_open_local(),
                &event.logical_key,
                app.play.as_ref(),
            ) =>
        {
            if event.state == ElementState::Pressed {
                if report::is_open_local() {
                    if let Some(key) = report::key_of(event) {
                        report::key(key);
                    }
                } else if !event.repeat
                    && (app.host.as_ref().is_some_and(|h| h.ui_parts().is_some())
                        || app.play.as_ref().is_some_and(|p| p.ui_parts().is_some()))
                {
                    report::open();
                    app.let_go_keys(el);
                    app.grab_pointer(false);
                }
            }
            true
        }
        WindowEvent::MouseInput { state, .. } if report::is_open_local() => {
            if *state == ElementState::Pressed
                && let Some(g) = &app.gfx
            {
                report::click(render::ui::frame_from_window(
                    (g.config.width, g.config.height),
                    [app.cursor.0 as f32, app.cursor.1 as f32],
                ));
            }
            true
        }
        _ => false,
    }
}

pub(super) fn surface(
    config: &mut wgpu::SurfaceConfiguration,
    adapter: &wgpu::Adapter,
    surface: &wgpu::Surface<'_>,
) {
    if surface
        .get_capabilities(adapter)
        .usages
        .contains(wgpu::TextureUsages::COPY_SRC)
    {
        config.usage |= wgpu::TextureUsages::COPY_SRC;
    }
    let info = adapter.get_info();
    report::set_gpu(format!("{} ({:?})", info.name, info.backend));
}

pub(super) fn shot(g: &Gfx, frame: Option<&wgpu::SurfaceTexture>) {
    if !crate::report::wants_shot() {
        return;
    }
    let Some(t) = frame
        .map(|f| &f.texture)
        .or(g.offscreen.as_ref())
        .filter(|t| t.usage().contains(wgpu::TextureUsages::COPY_SRC))
    else {
        return;
    };
    let mut rgba = read_back(&g.device, &g.queue, t);
    if matches!(
        t.format(),
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        for px in rgba.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
    }
    crate::report::set_shot(t.width(), t.height(), &rgba);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play() -> crate::play::Play {
        struct Nowhere;
        impl net::session::Datagrams for Nowhere {
            fn send_to(&mut self, _: &[u8], _: [u8; 4], _: u16) {}
            fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
                None
            }
        }
        let s = net::session::Session::connect_with(
            Box::new(Nowhere),
            ([127, 0, 0, 1], 5000),
            ([127, 0, 0, 1], 10070),
            "test",
            [0; 32],
        )
        .unwrap();
        crate::play::Play::new(s, Default::default(), Box::default())
    }

    /// F3 is the local report's on a server that took no bug reports and
    /// before the world; on one that did it falls through to Support's
    /// request form. The open dialog keeps every key either way.
    #[test]
    fn f3_is_supports_form_where_the_server_takes_bug_reports() {
        let (f3, f2) = (Key::Named(NamedKey::F3), Key::Named(NamedKey::F2));
        let mut p = play();
        assert!(
            claims_key(false, &f3, None),
            "before the world: the local report"
        );
        assert!(
            claims_key(false, &f3, Some(&p)),
            "no offer: the local report"
        );
        assert!(!claims_key(false, &f2, Some(&p)), "another key passes");

        p.session.bug_reports = true;
        assert!(!claims_key(false, &f3, Some(&p)), "an offer: F3 passes on");
        assert!(p.open_bug_report(), "to Support's form");
        assert_eq!(p.mode, 31);
        assert!(claims_key(true, &f3, Some(&p)), "the open dialog keeps F3");
        assert!(claims_key(true, &f2, Some(&p)), "and every key");
    }
}

/// Run Lua before the native HUD consumes its drawing, but record the ImGui
/// overlay separately: submission stays after the scene's final scale resolve.
pub(super) fn prepare_addons(
    g: &mut Gfx,
    frame: Option<&wgpu::SurfaceTexture>,
    play: Option<&crate::play::Play>,
    walker: Option<&crate::walker::Walker>,
    server: Option<&str>,
) -> Option<wgpu::CommandBuffer> {
    let target = frame
        .map(|f| &f.texture)
        .or(g.offscreen.as_ref())?
        .create_view(&Default::default());
    let snapshot = play.map_or_else(Default::default, |p| p.forge_snapshot(walker));
    let mut encoder = g.device.create_command_encoder(&Default::default());
    g.forge_runtime.draw(
        &g.window,
        &g.device,
        &g.queue,
        g.config.format,
        &mut encoder,
        &target,
        snapshot,
        server,
    );
    Some(encoder.finish())
}
pub(super) fn submit_addons(
    queue: &wgpu::Queue,
    scene: wgpu::CommandEncoder,
    overlay: Option<wgpu::CommandBuffer>,
) -> wgpu::SubmissionIndex {
    queue.submit(std::iter::once(scene.finish()).chain(overlay))
}
#[cfg(test)]
mod addon_order_tests {
    use super::*;
    // The GPU test below checks submission behavior. This source-level contract
    // also guards the live call site: preparing after p.hud loses the current
    // frame's native mod primitives even if submit_addons itself is correct.
    #[test]
    fn live_redraw_prepares_mods_before_native_hud_and_submits_after_resolve() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/viewer.rs"),
        )
        .unwrap();
        let redraw = &source[source
            .rfind("self.last_view = Some(view.view_proj);")
            .unwrap()..];
        let prepare = redraw.find("fork_report::prepare_addons(").unwrap();
        let hud = redraw.find("p.hud(").unwrap();
        let resolve = redraw.find("g.renderer.scale.resolve(").unwrap();
        let submit = redraw.find("fork_report::submit_addons(").unwrap();
        assert!(
            prepare < hud && hud < resolve && resolve < submit,
            "Lua preparation must precede native HUD consumption; overlay submits after scene resolve"
        );
    }
    #[test]
    fn addon_overlay_recorded_first_is_submitted_after_the_scene() {
        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let record = |color| {
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            }
            encoder
        };
        let pixels = || {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                texture.size(),
            );
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            buffer.slice(..).get_mapped_range().unwrap()[..4].to_vec()
        };
        let overlay = record(wgpu::Color::BLUE).finish();
        let scene = record(wgpu::Color::RED);
        submit_addons(&queue, scene, Some(overlay));
        assert_eq!(
            pixels(),
            [0, 0, 255, 255],
            "overlay must remain above the later-recorded scene"
        );
        // Negative control: recording order submitted directly loses the overlay.
        let overlay = record(wgpu::Color::BLUE).finish();
        let scene = record(wgpu::Color::RED).finish();
        queue.submit([overlay, scene]);
        assert_eq!(pixels(), [255, 0, 0, 255]);
    }
}
