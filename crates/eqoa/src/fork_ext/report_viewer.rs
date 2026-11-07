// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
// Upstream portions up to 1e23e80 remain MIT (NOTICE).
//! Native report adapter: input ownership and frame capture.
use super::*;
use crate::report;

/// Whether the local report takes this key: every key while its dialog is
/// open, and F3, except in the world of a server that offered bug reports
/// (`Session::bug_reports`), where F3 is Support's request form
/// (`Play::open_bug_report`, in `key_input`).
fn claims_key(open: bool, key: &Key, play: Option<&crate::play::Play>) -> bool {
    open || (report::is_report_key(key) && !play.is_some_and(|p| p.session.bug_reports))
}

pub(super) fn event(app: &mut App, el: &ActiveEventLoop, event: &WindowEvent) -> bool {
    match event {
        WindowEvent::KeyboardInput { event, .. }
            if claims_key(report::is_open_local(), &event.logical_key, app.play.as_ref()) =>
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
        assert!(claims_key(false, &f3, None), "before the world: the local report");
        assert!(claims_key(false, &f3, Some(&p)), "no offer: the local report");
        assert!(!claims_key(false, &f2, Some(&p)), "another key passes");

        p.session.bug_reports = true;
        assert!(!claims_key(false, &f3, Some(&p)), "an offer: F3 passes on");
        assert!(p.open_bug_report(), "to Support's form");
        assert_eq!(p.mode, 31);
        assert!(claims_key(true, &f3, Some(&p)), "the open dialog keeps F3");
        assert!(claims_key(true, &f2, Some(&p)), "and every key");
    }
}
