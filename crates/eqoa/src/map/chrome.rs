//! What the map draws over its picture, in the game's look: the area names
//! (`DrawCenteredStringEx`'s shadowed words), the title (the world and the
//! player's area, as the start menu's location line has it), the buttons, the
//! legend and the build's progress, each on `VIWndGoto`'s panel (atlas 61
//! stretched at 0.75, the game's own map-like window, its GM teleport). Laid
//! out against the window's edges in the frame's units; a pointer finds the
//! buttons where they were drawn (`draw`'s answer) and the legend's pad
//! glyphs where the raster noted them.

use crate::pointer::key;
use vi::ui::{Raster, Vui, wide};

/// The buttons, right to left from the top right corner: their words and
/// the pad's keys they press (Suggest, L3's: a community marker, `community`).
pub const BUTTONS: [(&str, i32); 5] = [("Close", key::TRIANGLE), ("+", key::R1), ("-", key::L1), ("Center", key::CROSS), ("Suggest", key::L3)];

/// The legend for the pad: the inline glyphs (△ 65, ✕ 66, □ 67, ○ 68, L1
/// 185, R1 187, L3 214) and what each does.
const PAD_LEGEND: [(i32, &str); 10] = [(65, "Close"), (66, "Center"), (67, "Waypoint"), (68, "Markers"), (189, "Towns"), (214, "Suggest"), (185, ""), (187, "Zoom"), (-1, "Sticks, drag, pinch: pan and zoom"), (-1, "Tap: waypoint")];
const KEY_LEGEND: [&str; 8] = ["M Close", "Space Center", "+ - or wheel Zoom", "Arrows, WASD or drag Pan", "Click or Q Waypoint", "X Markers", "T Towns", "N Suggest"];
/// In a group, the legend's words for the waypoint keys, the group's beside
/// the player's own (`group_waypoint`): □ held, a finger held, G or a click
/// with Shift.
const PAD_LEGEND_GROUP: (i32, &str) = (67, "Waypoint, held: the group's");
const TAP_LEGEND_GROUP: &str = "Tap: waypoint, hold: the group's";
const KEY_LEGEND_GROUP: &str = "G, Q held or Shift+click Group waypoint";

/// Long rebound key names must wrap inside the legend, not enlarge its panel.
fn wrap_keyboard_label(r: &dyn Raster, label: &str, max_width: i32) -> Vec<String> {
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
fn keyboard_legend(b: &crate::binds::Binds, group: bool, quests: bool) -> Vec<String> {
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
    labels[1] = format!("{} Center", key("attack", "Space"));
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
    labels[7] = format!("{} Suggest", key("minimap", "N"));
    if quests {
        labels.insert(7, format!("{} Quests", key("look_left", "J")));
    }
    if group {
        let (g, q) = (key("map_group_waypoint", "G"), key("talk", "Q"));
        labels.insert(
            5,
            if g == "G" && q == "Q" {
                KEY_LEGEND_GROUP.into()
            } else {
                format!("{g}, {q} held or Shift+click Group waypoint")
            },
        );
    }
    labels
}

pub const NAME: [f32; 4] = super::theme::TEXT;
/// The community list's answer under the title: the game's gold words.
const NOTICE: [f32; 4] = [1.0, 0.82, 0.25, 1.0];
/// A button's words while what it shows is hidden.
const DIM: [f32; 4] = [0.55, 0.53, 0.48, 1.0];


/// What the chrome shows.
pub struct Chrome {
    /// the window's corners (top left, bottom right) as frame points
    pub corners: [[f32; 2]; 2],
    pub title: String,
    /// the cells' names and their middles, frame points
    pub names: Vec<([f32; 2], String)>,
    /// a keyboard is in use: its keys in the legend, else the pad's glyphs
    pub keyboard: bool,
    /// zones on the map, of how many, while it is being drawn
    pub progress: Option<(usize, usize)>,
    /// while a server offers quest marks (`quests`): whether they are shown,
    /// for their button and the legend's R3 (J on a keyboard)
    pub quests: Option<bool>,
    /// the world's place under the pointer (or a pad's cross once the map is
    /// panned), as words, under the title (F3 20261026-090726, a player's
    /// ask: "X, Y coordinates of cursor position displayed on map")
    pub pointer: Option<String>,
    /// what the community's list answered a suggestion, for a while
    /// (`community::Community::notice_now`), under the title in gold
    pub notice: Option<String>,
    /// in a group: the legend names the group's waypoint (`group_waypoint`)
    pub group: bool,
}

/// A panel the size of `w` x `h` at `at` (VIWndGoto's).
pub fn panel(vui: &Vui, r: &mut dyn Raster, at: [i32; 2], w: i32, h: i32) {
    let _=vui;super::theme::panel(r,[at[0],at[1],at[0]+w,at[1]+h]);
}


/// A button drawn: its frame rectangle (x0, y0, x1, y1) and its key.
pub type Button = ([i32; 4], i32);

/// The cells' names (under the marker, which goes over them).
pub fn names(r: &mut dyn Raster, vui: &Vui, c: &Chrome) {
    for (at, name) in &c.names {
        vui.draw_centered_string_ex(r, &wide(name), at[0] as i32, at[1] as i32, NAME, 0, true);
    }
}

/// Where `draw` will put the chrome's pieces (frame rects x0, y0, x1, y1:
/// every panel, word and glyph, 4 more each way), laid out against raster
/// `r`'s font: for what keeps clear of it (the towns' names, a card), drawn
/// before it.
pub fn footprint(r: &dyn Raster, vui: &Vui, c: &Chrome) -> Vec<[f32; 4]> {
    struct Measure<'a> {
        r: &'a dyn Raster,
        rects: Vec<[f32; 4]>,
    }
    impl Measure<'_> {
        fn note(&mut self, at: [i32; 2], size: [i32; 2]) {
            let e = 4.0;
            self.rects.push([at[0] as f32 - e, at[1] as f32 - e, (at[0] + size[0]) as f32 + e, (at[1] + size[1]) as f32 + e]);
        }
    }
    impl Raster for Measure<'_> {
        fn blit(&mut self, _: u32, _: u32, at: [i32; 2], _: [i32; 2], b: [i32; 2], _: i32, _: f32) {
            self.note(at, b);
        }
        fn stretch_blit(&mut self, _: u32, _: u32, at: [i32; 2], size: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {
            self.note(at, size);
        }
        fn fill_fan(&mut self, _: [f32; 4], corners: [[i32; 2]; 4]) {
            self.note(corners[0], [corners[2][0] - corners[0][0], corners[2][1] - corners[0][1]]);
        }
        fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) {}
        fn draw_string(&mut self, at: [i32; 2], _: [f32; 4], s: &[u16]) {
            let size = [self.r.string_width(s).max(0), self.r.point_size()];
            self.note(at, size);
        }
        fn draw_centered_string(&mut self, at: [i32; 2], _: [f32; 4], s: &[u16], mode: u32) {
            let (w, h) = (self.r.string_width(s).max(0), self.r.point_size());
            let x = if mode == 1 { at[0] } else { at[0] - w / 2 };
            let y = if mode == 2 { at[1] } else { at[1] - h / 2 };
            self.note([x, y], [w, h]);
        }
        fn string_width(&self, s: &[u16]) -> i32 {
            self.r.string_width(s)
        }
        fn point_size(&self) -> i32 {
            self.r.point_size()
        }
        fn viewport(&self) -> [i32; 4] {
            self.r.viewport()
        }
        fn set_viewport(&mut self, _: [i32; 4]) {}
    }
    let mut m = Measure { r, rects: Vec::new() };
    draw(&mut m, vui, c);
    m.rects
}

/// The band between the chrome's top and its foot (frame y: under the
/// lowest piece above the middle, over the highest below it) out of
/// `footprint`'s rects, within the window's corners.
pub fn clear_band(rects: &[[f32; 4]], corners: [[f32; 2]; 2]) -> [f32; 2] {
    let mid = (corners[0][1] + corners[1][1]) / 2.0;
    let top = rects.iter().filter(|r| r[3] < mid).map(|r| r[3]).fold(corners[0][1], f32::max);
    let foot = rects.iter().filter(|r| r[1] > mid).map(|r| r[1]).fold(corners[1][1], f32::min);
    [top, foot]
}

pub fn search_anchor(r:&dyn Raster,vui:&Vui,c:&Chrome)->i32 {clear_band(&footprint(r,vui,c),c.corners)[0].ceil()as i32+6}

/// Draws the chrome over everything; where its buttons went, with their
/// keys. In a window too narrow for a row, the title goes under the buttons
/// and the legend onto as many lines as it needs, from the foot up.
pub fn draw(r: &mut dyn Raster, vui: &Vui, c: &Chrome) -> Vec<Button> {
    let mut hits = Vec::new();
    let [tl, br] = c.corners.map(|p| [p[0].round() as i32, p[1].round() as i32]);
    let (m, line) = (8, r.point_size() + 6);
    let avail = br[0] - tl[0] - 2 * m;
    // the buttons, top right
    let mut x = br[0] - m;
    for (label, k) in BUTTONS {
        let s = wide(label);
        let bw = (r.string_width(&s).max(0) + 16).max(line + 4);
        let at = [x - bw, tl[1] + m];
        panel(vui, r, at, bw, line);
        vui.draw_centered_string_ex(r, &s, at[0] + bw / 2, at[1] + line / 2, NAME, 0, false);
        hits.push(([at[0], at[1], at[0] + bw, at[1] + line], k));
        x = at[0] - 6;
    }
    // the quest marks' button, while a server sends them: dim while hidden
    if let Some(shown) = c.quests {
        let s = wide("Quests");
        let bw = r.string_width(&s).max(0) + 16;
        let at = [x - bw, tl[1] + m];
        panel(vui, r, at, bw, line);
        vui.draw_centered_string_ex(r, &s, at[0] + bw / 2, at[1] + line / 2, if shown { NAME } else { DIM }, 0, true);
        hits.push(([at[0], at[1], at[0] + bw, at[1] + line], key::R3));
        x = at[0] - 6;
    }
    // the title, top left (under the buttons when they would meet), and the
    // build's progress under it
    let s = wide(&c.title);
    let tw = r.string_width(&s).max(0);
    let mut y = tl[1] + m;
    if tl[0] + m + tw + 12 > x {
        y += line + 4;
    }
    panel(vui, r, [tl[0] + m, y], tw + 12, line);
    vui.draw_string_ex(r, &s, tl[0] + m + 6, y + 3, NAME, false);
    let mut y = y;
    let lines = [c.progress.map(|(done, of)| format!("Drawing the map: {done} of {of} areas")), c.pointer.clone()].into_iter().flatten().map(|w| (w, NAME));
    for (words, color) in lines.chain(c.notice.clone().map(|n| (n, NOTICE))) {
        let s = wide(&words);
        let tw = r.string_width(&s).max(0);
        y += line + 4;
        panel(vui, r, [tl[0] + m, y], tw + 12, line);
        vui.draw_string_ex(r, &s, tl[0] + m + 6, y + 3, color, true);
    }
    // the legend: its pieces (a glyph or none, and words) in lines that fit
    let keyboard = if c.keyboard {
        crate::settings::with_binds(|b| keyboard_legend(b, c.group, c.quests.is_some()))
    } else {
        Vec::new()
    };
    let mut pieces: Vec<(i32, &str)> = if c.keyboard {
        keyboard.iter().map(|s| (-1, s.as_str())).collect() } else { PAD_LEGEND.to_vec() };
    if c.group && !c.keyboard {
            for p in &mut pieces {
                if *p == PAD_LEGEND[2] {
                    *p = PAD_LEGEND_GROUP;
                } else if p.1 == "Tap: waypoint" {
                    p.1 = TAP_LEGEND_GROUP;
                }
            }
        }
    if c.quests.is_some() && !c.keyboard {
        // after the towns' key (R3's glyph, 215, inline)
        let at = pieces.iter().position(|p| p.1.ends_with("Towns")).map_or(pieces.len(), |i| i + 1);
            pieces.insert(at, (215, "Quests"));
    }
    let wrapped: Vec<(i32, String)> = pieces
        .into_iter()
        .flat_map(|(glyph, label)| {
            if glyph < 0 {
                wrap_keyboard_label(r, label, (avail - 12).max(1))
                    .into_iter()
                    .map(|part| (-1, part))
                    .collect()
            } else {
                vec![(glyph, label.to_string())]
            }
        })
        .collect();
    let pieces: Vec<(i32, &str)> = wrapped
        .iter()
        .map(|(glyph, label)| (*glyph, label.as_str()))
        .collect();
    let width = |r: &dyn Raster, (g, s): (i32, &str)| if g >= 0 { vui.entry(g).uv1[0] + 2 } else { 0 } + r.string_width(&wide(s)).max(0) + 12;
    let mut lines: Vec<Vec<(i32, &str)>> = vec![Vec::new()];
    let mut used = 0;
    for p in pieces {
        let w = width(r, p);
        if used + w > avail && !lines.last().unwrap().is_empty() {
            lines.push(Vec::new());
            used = 0;
        }
        used += w;
        lines.last_mut().unwrap().push(p);
    }
    for (k, pieces) in lines.iter().enumerate() {
        let y = br[1] - m - line - (lines.len() - 1 - k) as i32 * (line + 4);
        let w: i32 = pieces.iter().map(|p| width(r, *p)).sum();
        panel(vui, r, [tl[0] + m, y], w + 6, line);
        let mut x = tl[0] + m + 6;
        for &(glyph, label) in pieces {
            if glyph >= 0 {
                let e = vui.entry(glyph);
                vui.blit(r, glyph, [x, y + (line - e.uv1[1]) / 2]);
                x += e.uv1[0] + 2;
            }
            let s = wide(label);
            vui.draw_string_ex(r, &s, x, y + 3, NAME, false);
            x += r.string_width(&s).max(0) + 12;
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claimed_shifted_alternate_is_not_advertised_for_zoom() {
        let mut b = crate::binds::Binds::default();
        b.bind_key(crate::binds::row_named("map").unwrap(), "+");
        assert!(matches!(b.alias("+"), crate::binds::Alias::To("m")));
        assert!(matches!(b.alias("="), crate::binds::Alias::Same));
        let hints = keyboard_legend(&b, false, false);
        assert!(hints.iter().any(|s| s == "+ Close"), "{hints:?}");
        assert!(
            hints.iter().any(|s| s == "= - or wheel Zoom"),
            "zoom must show its surviving primary key: {hints:?}"
        );
    }

    #[test]
    fn keyboard_hints_follow_the_controls_bindings() {
        let mut b = crate::binds::Binds::default();
        let defaults = keyboard_legend(&b, false, false);
        assert_eq!(defaults, KEY_LEGEND.map(str::to_string));
        for (action, key) in [
            ("map", "v"),
            ("talk", "u"),
            ("map_group_waypoint", "f10"),
            ("autorun", "y"),
            ("tell", "o"),
            ("minimap", "f2"),
            ("look_left", "f4"),
            ("map_in", "insert"),
            ("map_out", "delete"),
            ("walk_forward", "8"),
        ] {
            b.bind_key(crate::binds::row_named(action).unwrap(), key);
        }
        let hints = keyboard_legend(&b, true, true);
        for expected in [
            "V Close",
            "Click or U Waypoint",
            "F10, U held or Shift+click Group waypoint",
            "Y Markers",
            "O Towns",
            "F2 Suggest",
            "F4 Quests",
            "Insert Delete or wheel Zoom",
        ] {
            assert!(
                hints.iter().any(|s| s == expected),
                "missing {expected}: {hints:?}"
            );
        }
        assert!(
            hints.iter().any(|s| s.contains("8/S/A/D")),
            "panning uses the moved movement keys: {hints:?}"
        );
        assert!(
            !hints.iter().any(|s| s == "M Close" || s == "J Quests"),
            "defaults are no longer advertised"
        );
        b.bind_key(crate::binds::row_named("map").unwrap(), "");
        assert!(
            keyboard_legend(&b, false, false)
                .iter()
                .any(|s| s == "Unbound Close")
        );
    }

    #[test]
    fn rebound_map_key_dispatches_the_advertised_action_but_typing_stays_text() {
        use crate::binds::{Binds, KeyEdge, Sent};
        let mut b = Binds::default();
        b.bind_key(crate::binds::row_named("map").unwrap(), "v");
        let mut edge = KeyEdge::default();
        let Some(Sent::Key(key_name)) = edge.key("v", true, false, false, &b) else {
            panic!("the rebound map key must reach the keyboard dispatch");
        };
        assert_eq!(
            super::super::keyboard(&key_name),
            Some((key::TRIANGLE, super::super::Hold::Once))
        );
        assert!(
            edge.key("m", true, false, false, &b).is_none(),
            "old map key is released"
        );
        let mut typing = KeyEdge::default();
        assert!(matches!(typing.key("v", true, false, true, &b), Some(Sent::Key(k)) if k == "v"));
    }

    use crate::pointer::testing::{NullRaster, mid, vui};

    /// In a phone's tall window (frame units 640 wide) the legend breaks into
    /// lines that each fit, the foot's last, and the buttons stay inside the
    /// window; in a wide one it is one line. The control: the wide window's
    /// one line is wider than the narrow window.
    #[test]
    fn a_narrow_window_breaks_the_legend_into_lines() {
        #[derive(Default)]
        struct Panels(Vec<[i32; 4]>);
        impl crate::ui::Layout for Panels {}
        impl Raster for Panels {
            fn blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
            fn stretch_blit(&mut self, _: u32, _: u32, at: [i32; 2], size: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {
                self.0.push([at[0], at[1], at[0] + size[0], at[1] + size[1]]);
            }
            fn fill_fan(&mut self, color: [f32; 4], c: [[i32; 2]; 4]) { if color==super::super::theme::SURFACE {self.0.push([c[0][0],c[0][1],c[2][0],c[2][1]]);} }
            fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) {}
            fn draw_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16]) {}
            fn draw_centered_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16], _: u32) {}
            // ArialUI's letters run about 12 frame pixels with their gap
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
        // a stand-in atlas: every entry 61 (the panel) of surface 1, so panels draw
        let atlas: Vec<vi::ui::UiTexEntry> = (0..252).map(|i| vi::ui::UiTexEntry { x00: i, uv0: [0, 0], uv1: [16, 16], ..Default::default() }).collect();
        let v = vi::ui::Vui::new(Default::default(), Default::default(), atlas);
        let tall = Chrome { corners: [[0.0, -470.0], [640.0, 918.0]], ..chrome(false) };
        let mut r = Panels::default();
        let hits = draw(&mut r, &v, &tall);
        let foot: Vec<&[i32; 4]> = r.0.iter().filter(|p| p[1] > 600).collect();
        assert!(foot.len() >= 2, "{} lines: {foot:?}", foot.len());
        assert!(foot.iter().all(|p| p[2] <= 640 - 8 + 6), "each line inside the window: {foot:?}");
        assert!(hits.iter().all(|h| h.0[0] >= 0 && h.0[2] <= 640), "{hits:?}");
        let mut w = Panels::default();
        draw(&mut w, &v, &Chrome { corners: [[-400.0, 0.0], [1040.0, 448.0]], ..chrome(false) });
        let one: Vec<&[i32; 4]> = w.0.iter().filter(|p| p[1] > 380).collect();
        assert_eq!(one.len(), 1, "a wide window's one line");
        assert!(one[0][2] - one[0][0] > 640 - 16, "the control: wider than the narrow window");
        crate::settings::testing::reset(true);
        let mut b = crate::binds::Binds::default();
        for (id, name) in [
            ("menu_up", "backspace"),
            ("menu_down", "pagedown"),
            ("menu_prev", "pageup"),
            ("menu_next", "delete"),
            ("walk_forward", "insert"),
            ("walk_back", "space"),
            ("walk_left", "enter"),
            ("walk_right", "home"),
        ] {
            b.bind_key(crate::binds::row_named(id).unwrap(), name);
        }
        crate::settings::set_binds(b);
        let mut rebound = Panels::default();
        draw(
            &mut rebound,
            &v,
            &Chrome {
                keyboard: true,
                group: true,
                quests: Some(true),
                ..tall
            },
        );
        crate::settings::testing::reset(false);
        let foot: Vec<_> = rebound.0.iter().filter(|p| p[1] > 450).collect();
        assert!(
            foot.iter().all(|p| p[2] <= 640 - 8 + 6),
            "rebound keyboard labels fit: {foot:?}"
        );
    }

    #[test]
    fn search_toolbar_clears_coordinates_progress_and_notices() {
        let c=Chrome{pointer:Some("x 25274, z 14430".into()),notice:Some("Marker sent for approval".into()),..chrome(true)};
        let band=clear_band(&footprint(&NullRaster,&vui(),&c),c.corners);
        assert!(search_anchor(&NullRaster,&vui(),&c)>=band[0].ceil()as i32+6,"toolbar must follow all header lines");
        let empty=Chrome{progress:None,pointer:None,notice:None,..chrome(true)};
        assert!(search_anchor(&NullRaster,&vui(),&empty)<search_anchor(&NullRaster,&vui(),&c));
    }

    fn chrome(keyboard: bool) -> Chrome {
        Chrome { corners: [[-106.0, 0.0], [746.0, 448.0]], title: "Tunaria: Freeport (North)".into(), names: Vec::new(), keyboard, progress: Some((3, 175)), quests: None, pointer: None, notice: None, group: false }
    }

    /// The chrome's footprint covers its title (top left), its buttons (top
    /// right) and its legend (the foot), and the band clear of it runs from
    /// under the title's progress line to over the legend; the control: with
    /// no chrome the band is the whole window.
    #[test]
    fn the_chrome_says_where_it_goes() {
        let c = chrome(true);
        let rects = footprint(&NullRaster, &vui(), &c);
        let at = |x: f32, y: f32| {
            rects.iter().any(|r| (r[0]..r[2]).contains(&x) && (r[1]..r[3]).contains(&y))
        };
        // the title's words at (-90, 11), "Close" right, "M Close" at the foot
        assert!(at(-80.0, 16.0) && at(730.0, 16.0) && at(-80.0, 430.0), "title, buttons, legend: {rects:?}");
        assert!(!at(320.0, 224.0), "nothing in the middle");
        let band = clear_band(&rects, c.corners);
        assert!(band[0] > 40.0 && band[1] < 420.0 && band[0] < band[1], "{band:?}");
        let hits = draw(&mut NullRaster, &vui(), &c);
        for (r, _) in hits {
            assert!(at(r[0] as f32 + 1.0, r[1] as f32 + 1.0), "button {r:?} in it");
        }
        assert_eq!(clear_band(&[], c.corners), [0.0, 448.0], "the control");
    }

    /// Each button is where it was drawn, against the window's top right
    /// corner (here a 16:9 window's, past the frame's side), and a pointer
    /// there presses its key; a pointer elsewhere presses none. The control:
    /// the map with no buttons noted finds nothing at the Close button.
    #[test]
    fn a_pointer_presses_the_button_under_it() {
        let hits = draw(&mut NullRaster, &vui(), &chrome(true));
        let mut map = super::super::Map::default();
        let close = hits.iter().find(|h| h.1 == key::TRIANGLE).unwrap().0;
        assert!(close[2] <= 746 - 8 && close[2] > 640, "against the window's right side, past the frame's: {close:?}");
        assert!(map.point(mid(close), false).is_empty(), "the control: no buttons noted");
        map.buttons = hits.clone();
        assert_eq!(map.point(mid(close), false), vec![key::TRIANGLE]);
        for &(r, k) in &hits {
            assert_eq!(map.point(mid(r), false), vec![k]);
        }
        assert_eq!(hits.len(), BUTTONS.len());
        assert!(map.point([320.0, 224.0], false).is_empty(), "the map itself");
        // a finger's slop reaches a button just missed
        assert_eq!(map.point([close[2] as f32 + 3.0, mid(close)[1]], true), vec![key::TRIANGLE]);
        assert!(map.point([close[2] as f32 + 3.0, mid(close)[1]], false).is_empty());
    }

    /// What the community's list answered goes under the title, the
    /// pointer's place too, and the chrome's footprint has it; the legend
    /// names the Suggest key (N, a pad's L3). The control: no notice, no
    /// line there.
    #[test]
    fn the_notice_goes_under_the_title() {
        let with = Chrome { notice: Some("Sent for a GM's approval.".into()), pointer: Some("x 1, z 2".into()), ..chrome(true) };
        let rects = footprint(&NullRaster, &vui(), &with);
        let without = footprint(&NullRaster, &vui(), &Chrome { pointer: Some("x 1, z 2".into()), ..chrome(true) });
        let low = |rs: &[[f32; 4]]| {
            rs.iter().filter(|r| r[0] < 0.0 && r[1] < 224.0).map(|r| r[3]).fold(0.0, f32::max)
        };
        assert!(low(&rects) > low(&without) + 10.0, "a line more under the title: {} {}", low(&rects), low(&without));
        assert!(KEY_LEGEND.contains(&"N Suggest") && PAD_LEGEND.contains(&(214, "Suggest")));
        assert!(BUTTONS.contains(&("Suggest", key::L3)));
    }

    /// While a server offers quest marks the chrome has their button, R3's
    /// (hidden or shown), left of the others and pressed where drawn; the
    /// control: with none, no R3 anywhere.
    #[test]
    fn the_quest_marks_button_is_there_only_with_marks() {
        for shown in [true, false] {
            let hits = draw(&mut NullRaster, &vui(), &Chrome { quests: Some(shown), ..chrome(true) });
            let quests = hits.iter().find(|h| h.1 == key::R3).expect("the marks' button").0;
            assert!(hits.iter().filter(|h| h.1 != key::R3).all(|h| h.0[0] > quests[2]), "left of the others: {hits:?}");
            let mut map = super::super::Map { buttons: hits.clone(), ..Default::default() };
            assert_eq!(map.point(mid(quests), false), vec![key::R3]);
            map.buttons.clear();
        }
        assert!(draw(&mut NullRaster, &vui(), &chrome(true)).iter().all(|h| h.1 != key::R3), "no marks, no button");
    }

    /// In a group the legend names the group's waypoint beside the player's
    /// own: a keyboard's G, Q held or Shift and a click, a pad's □ held and a
    /// finger held on the map. The control: out of a group, none of it.
    #[test]
    fn in_a_group_the_legend_names_the_groups_waypoint() {
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
        let said = |keyboard: bool, group: bool| {
            let mut r = Words::default();
            draw(&mut r, &vui(), &Chrome { group, ..chrome(keyboard) });
            r.0
        };
        let keys = said(true, true);
        assert!(keys.iter().any(|s| s == KEY_LEGEND_GROUP) && keys.iter().any(|s| s == "Click or Q Waypoint"), "{keys:?}");
        let pad = said(false, true);
        assert!(pad.iter().any(|s| s == PAD_LEGEND_GROUP.1) && pad.iter().any(|s| s == TAP_LEGEND_GROUP), "{pad:?}");
        for keyboard in [true, false] {
            let alone = said(keyboard, false);
            assert!(alone.iter().any(|s| s.contains("aypoint")), "the control drew its legend: {alone:?}");
            assert!(!alone.iter().any(|s| s.contains("group's") || s.contains("Group waypoint")), "out of a group: {alone:?}");
        }
    }
}
