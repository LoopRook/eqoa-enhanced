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

use crate::map::chrome::{KEY_LEGEND, KEY_LEGEND_GROUP};
use vi::ui::{Raster, wide};

/// Long rebound key names must wrap inside the legend, not enlarge its panel.
pub(crate) fn wrap_keyboard_label(r: &dyn Raster, label: &str, max_width: i32) -> Vec<String> {
    let mut rest = label;
    let mut parts = Vec::new();
    while r.string_width(&wide(rest)) > max_width && !rest.is_empty() {
        let (mut end, mut space) = (0, None);
        for (at, ch) in rest.char_indices() {
            let next = at + ch.len_utf8();
            if r.string_width(&wide(&rest[..next])) > max_width {
                break;
            }
            end = next;
            if ch.is_whitespace() && at > 0 {
                space = Some(at);
            }
        }
        let end = space.unwrap_or_else(|| {
            if end > 0 {
                end
            } else {
                rest.chars().next().unwrap().len_utf8()
            }
        });
        parts.push(rest[..end].trim_end().to_string());
        rest = rest[end..].trim_start();
    }
    if !rest.is_empty() {
        parts.push(rest.to_string());
    }
    parts
}

/// Keyboard hints use the same rows as input dispatch; remapping must not leave
/// the map advertising a key that no longer performs its action.
pub(crate) fn keyboard_legend(b: &crate::binds::Binds, group: bool, quests: bool) -> Vec<String> {
    let key = |id: &str, default: &str| {
        let row = crate::binds::row_named(id).expect("map action has a Controls row");
        let preferred = crate::binds::ROWS[row]
            .keys
            .iter()
            .find(|name| crate::binds::display(name) == default)
            .copied()
            .unwrap_or(crate::binds::ROWS[row].keys[0]);
        if !b.moved(row) && matches!(b.alias(preferred), crate::binds::Alias::Same) {
            default.to_string()
        } else {
            b.key(row)
                .filter(|s| !s.is_empty())
                .map(crate::binds::display)
                .unwrap_or_else(|| "Unbound".into())
        }
    };
    let mut labels: Vec<String> = KEY_LEGEND.iter().map(|s| s.to_string()).collect();
    labels[0] = format!("{} Close", key("map", "M"));
    labels[1] = format!("{} Center", key("jump", "Space"));
    labels[2] = format!(
        "{} {} or wheel Zoom",
        key("map_in", "+"),
        key("map_out", "-")
    );
    let directions = |rows: &[(&str, &str)], default: &str| {
        if rows
            .iter()
            .any(|(id, _)| b.moved(crate::binds::row_named(id).unwrap()))
        {
            rows.iter()
                .map(|(id, label)| key(id, label))
                .collect::<Vec<_>>()
                .join("/")
        } else {
            default.to_string()
        }
    };
    let arrows = directions(
        &[
            ("menu_up", "↑"),
            ("menu_down", "↓"),
            ("menu_prev", "←"),
            ("menu_next", "→"),
        ],
        "Arrows",
    );
    let walking = directions(
        &[
            ("walk_forward", "W"),
            ("walk_back", "S"),
            ("walk_left", "A"),
            ("walk_right", "D"),
        ],
        "WASD",
    );
    labels[3] = format!("{arrows}, {walking} or drag Pan");
    labels[4] = format!("Click or {} Waypoint", key("talk", "Q"));
    labels[5] = format!("{} Markers", key("autorun", "X"));
    labels[6] = format!("{} Towns", key("tell", "T"));
    *labels.last_mut().expect("map legend ends with Suggest") =
        format!("{} Suggest", key("minimap", "N"));
    if quests {
        labels.insert(7, format!("{} Quests", key("look_left", "J")));
    }
    if group {
        let (g, q) = (key("map_group_waypoint", "O"), key("talk", "Q"));
        labels.insert(
            5,
            if g == "O" && q == "Q" {
                KEY_LEGEND_GROUP.into()
            } else {
                format!("{g}, {q} held or Shift+click Group waypoint")
            },
        );
    }
    labels
}

pub(crate) fn keyboard_pieces(group: bool, quests: bool, travel: bool) -> Vec<String> {
    let mut labels = crate::settings::with_binds(|b| keyboard_legend(b, group, quests));
    if travel {
        let at = labels
            .iter()
            .rposition(|p| p.contains("aypoint"))
            .map_or(labels.len(), |i| i + 1);
        labels.insert(at, crate::map::chrome::LEGEND_TRAVEL.into());
    }
    labels
}

/// The circle already uses the moved window rectangle. Its border is emitted
/// in that same space; the north label alone uses the HUD's frame shift.
pub(crate) fn round_minimap_border(
    ui: &mut render::ui::Ui,
    white: usize,
    size: (u32, u32),
    circle: [f32; 3],
    width: f32,
) {
    for i in 0..96 {
        let a = i as f32 * std::f32::consts::TAU / 96.0;
        let b = (i + 1) as f32 * std::f32::consts::TAU / 96.0;
        let at = |t: f32, r: f32| {
            render::ui::frame_from_window(size, [circle[0] + r * t.cos(), circle[1] + r * t.sin()])
        };
        let inner = (circle[2] - width).max(0.0);
        ui.quads.push(render::ui::Quad {
            tex: white,
            blend: 3,
            pos: [
                at(a, inner),
                at(a, circle[2]),
                at(b, circle[2]),
                at(b, inner),
            ],
            uv: [[0.5, 0.5]; 4],
            color: crate::map::theme::BORDER,
            clip: Some(render::ui::WHOLE),
        });
    }
}
