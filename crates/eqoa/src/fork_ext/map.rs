//! Circular minimap geometry retained alongside upstream rectangle placement.
pub(crate) fn circular_rim(mid: [f32; 2], q: [f32; 2], r: [f32; 4]) -> [f32; 2] {
    let d = [q[0] - mid[0], q[1] - mid[1]];
    let half = [(r[2] - r[0]) / 2.0, (r[3] - r[1]) / 2.0];
    let k = d[0].hypot(d[1]) / half[0].min(half[1]).max(1e-6);
    if k <= 1.0 {
        return q;
    }
    [mid[0] + d[0] / k, mid[1] + d[1] / k]
}

#[cfg(test)]
mod tests {
    use crate::map::chrome::{self, Chrome, LEGEND_TRAVEL};
    use crate::pointer::testing::vui;
    use vi::ui::Raster;

    /// A raster that records the words drawn, so the legend's labels can be
    /// counted (as the map chrome's own tests do).
    #[derive(Default)]
    struct Words(Vec<String>);

    impl crate::ui::Layout for Words {}

    impl Raster for Words {
        fn blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
        fn stretch_blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
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
        }
    }

    /// A GM's offered travel names its click once, not twice: the merged
    /// chrome briefly drew the block that inserts it twice (upstream has it
    /// once). The control: anyone else's legend names it not at all.
    #[test]
    fn a_gms_travel_click_is_named_once() {
        for keyboard in [true, false] {
            let mut r = Words::default();
            chrome::draw(&mut r, &vui(), &chrome(true, keyboard));
            let said = r.0.iter().filter(|s| s.as_str() == LEGEND_TRAVEL).count();
            assert_eq!(said, 1, "one travel line, keyboard {keyboard}: {:?}", r.0);
            let mut alone = Words::default();
            chrome::draw(&mut alone, &vui(), &chrome(false, keyboard));
            assert!(
                !alone.0.iter().any(|s| s.as_str() == LEGEND_TRAVEL),
                "no travel for anyone else: {:?}",
                alone.0
            );
        }
    }
}
