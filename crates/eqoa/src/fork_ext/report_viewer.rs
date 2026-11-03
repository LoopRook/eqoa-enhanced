//! Native report adapter: input ownership and frame capture.
use super::*;
use crate::report;

pub(super) fn event(app: &mut App, el: &ActiveEventLoop, event: &WindowEvent) -> bool {
    match event {
        WindowEvent::KeyboardInput { event, .. }
            if report::is_open() || report::is_report_key(&event.logical_key) =>
        {
            if event.state == ElementState::Pressed {
                if report::is_open() {
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
        WindowEvent::MouseInput { state, .. } if report::is_open() => {
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
