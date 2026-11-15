//! Search toolbar placement from the chrome that was actually composed.
use std::cell::Cell;
thread_local! { static ANCHOR: Cell<Option<[f32; 4]>> = const { Cell::new(None) }; }
pub(crate) fn note(rects: &[[f32; 4]], corners: [[f32; 2]; 2], size: (u32, u32)) {
    let band = crate::map::chrome::clear_band(rects, corners);
    let sy = size.1 as f32 / (corners[1][1] - corners[0][1]).max(1.);
    let y = (band[0] - corners[0][1]) * sy + 6.;
    let bottom = (band[1] - corners[0][1]) * sy - 6.;
    ANCHOR.with(|a| a.set(Some([16., y, size.0 as f32 - 32., (bottom - y).max(0.)])));
}
pub(crate) fn anchor() -> Option<[f32; 4]> {
    ANCHOR.with(Cell::get)
}

pub(crate) fn overlay(
    ui: &mut render::ui::Ui,
    white: usize,
    world: i32,
    project: impl Fn([f32; 2]) -> [f32; 2],
    bounds: [f32; 4],
    chrome: (&[[f32; 4]], [[f32; 2]; 2], (u32, u32)),
) {
    note(chrome.0, chrome.1, chrome.2);
    crate::fork_ext::map::overlay(ui, white, world, project, bounds);
}

#[cfg(test)]
mod tests {
    use crate::map::chrome::{self, Chrome};
    use crate::pointer::testing::vui;
    use vi::ui::Raster;
    #[derive(Default)]
    struct Words(Vec<String>);

    impl crate::ui::Layout for Words {}

    impl Raster for Words {
        fn blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
        fn stretch_blit(
            &mut self,
            _: u32,
            _: u32,
            _: [i32; 2],
            _: [i32; 2],
            _: [i32; 2],
            _: [i32; 2],
            _: i32,
            _: f32,
        ) {
        }
        fn fill_fan(&mut self, _: [f32; 4], _: [[i32; 2]; 4]) {}
        fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) {}
        fn draw_string(&mut self, _: [i32; 2], _: [f32; 4], s: &[u16]) {
            self.0.push(String::from_utf16_lossy(s));
        }
        fn draw_centered_string(&mut self, _: [i32; 2], _: [f32; 4], s: &[u16], _: u32) {
            self.0.push(String::from_utf16_lossy(s));
        }
        fn string_width(&self, s: &[u16]) -> i32 {
            s.len() as i32 * 12
        }
        fn point_size(&self) -> i32 {
            16
        }
        fn viewport(&self) -> [i32; 4] {
            [0, 0, 640, 448]
        }
        fn set_viewport(&mut self, _: [i32; 4]) {}
    }

    fn chrome(travel: bool, keyboard: bool) -> Chrome {
        Chrome {
            corners: [[-106.0, 0.0], [746.0, 448.0]],
            title: "Tunaria: Freeport (North)".into(),
            names: Vec::new(),
            keyboard,
            progress: Some((3, 175)),
            quests: None,
            pointer: None,
            notice: None,
            group: false,
            travel,
            places: None,
        }
    }

    #[test]
    fn round4b_fix_search_toolbar_clears_coordinates_progress_and_notices() {
        for width in [480., 852., 1280.] {
            for extras in [false, true] {
                let mut c = chrome(false, true);
                c.corners = [[0., 0.], [width, 768.]];
                if extras {
                    c.pointer = Some("X 25300 Y 15200".into());
                    c.notice = Some("Marker sent for approval".into());
                }
                let rects = chrome::footprint(&Words::default(), &vui(), &c);
                crate::fork_ext::map_toolbar::note(&rects, c.corners, (width as u32, 768));
                let a = crate::fork_ext::map_toolbar::anchor().unwrap();
                for r in rects.iter().filter(|r| r[3] < 384.) {
                    assert!(a[1] >= r[3] + 6., "toolbar overlaps header {r:?}: {a:?}");
                }
                assert!(a[3] > 20.);
            }
        }
    }
}
