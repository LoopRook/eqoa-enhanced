//! HUD-number adapter; stock HUD layout remains in the parent.
use super::{HudState, UiRaster};
use render::ui::{Anchor, Vertical};

pub(super) fn health_visible(s: &HudState) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        !s.forge_health_hidden
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = s;
        true
    }
}

#[derive(Debug)]
struct Readout {
    text: String,
    center: [i32; 2],
    scale: f32,
    color: [f32; 4],
    vertical: Vertical,
}
fn readouts(s: &HudState, need: u32, flags: [bool; 3]) -> Vec<Readout> {
    let [target_hp_percent, xp_details, hud_numbers] = flags;
    let mut out = Vec::with_capacity(4);
    if hud_numbers {
        if health_visible(s) {
            let pct = ((s.self_hp as f32 / 255.0) * 100.0).round() as i32;
            out.push(Readout {
                text: format!("{pct}%"),
                center: [99, 35],
                scale: 0.6,
                color: [1.0; 4],
                vertical: Vertical::Top,
            });
        }
        if s.max_power > 0 {
            out.push(Readout {
                text: format!("{}/{}", s.power, s.max_power),
                center: [99, 45],
                scale: 0.6,
                color: [0.4, 0.7, 1.0, 1.0],
                vertical: Vertical::Top,
            });
        }
    }
    if xp_details && need > 0 {
        let pct = (s.exp as f32 / need as f32 * 100.0).clamp(0.0, 100.0);
        out.push(Readout {
            text: format!("{}/{} ({:.1}%)", s.exp, need, pct),
            center: [99, 55],
            scale: 0.6,
            color: [1.0, 0.85, 0.35, 1.0],
            vertical: Vertical::Top,
        });
    }
    if target_hp_percent && s.target_name.is_some() {
        let pct = ((s.target_hp as f32 / 255.0) * 100.0).round() as i32;
        let color = if pct > 50 {
            [0.4, 1.0, 0.4, 1.0]
        } else if pct > 20 {
            [1.0, 0.9, 0.3, 1.0]
        } else {
            [1.0, 0.3, 0.3, 1.0]
        };
        out.push(Readout {
            text: format!("{pct}%"),
            center: [super::TARGET_AT[0] + 40, super::TARGET_AT[1] + 49],
            scale: 0.65,
            color,
            vertical: Vertical::Bottom,
        });
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn should_suppress_quad(
    q: &render::ui::Quad,
    vis: &crate::fork_ext::addons::visibility::PublishedVisibility,
) -> bool {
    let min_x = q.pos[0][0].min(q.pos[1][0]).min(q.pos[2][0]).min(q.pos[3][0]);
    let max_x = q.pos[0][0].max(q.pos[1][0]).max(q.pos[2][0]).max(q.pos[3][0]);
    let min_y = q.pos[0][1].min(q.pos[1][1]).min(q.pos[2][1]).min(q.pos[3][1]);
    let max_y = q.pos[0][1].max(q.pos[1][1]).max(q.pos[2][1]).max(q.pos[3][1]);

    // 1. Compass: center [39, 54], radius ~25
    if vis.compass && min_x >= 10.0 && max_x <= 70.0 && min_y >= 25.0 && max_y <= 85.0 {
        return true;
    }

    // 2. Power bar: y around [43..56], x around [65..215]
    if vis.power_bar && min_x >= 60.0 && max_x <= 215.0 && min_y >= 43.0 && max_y <= 56.0 {
        return true;
    }

    // 3. Exp bar: y around [55..80], x around [60..215]
    if vis.exp_bar && min_x >= 60.0 && max_x <= 215.0 && min_y >= 55.0 && max_y <= 80.0 {
        return true;
    }

    // 4. Entire Me plate if health + power + exp are all hidden
    if vis.health_bar
        && vis.power_bar
        && vis.exp_bar
        && min_x >= 5.0
        && max_x <= 215.0
        && min_y >= 10.0
        && max_y <= 85.0
    {
        return true;
    }

    // 5. Group display: GROUP_AT = [10, 110]
    if vis.group && min_x >= 5.0 && max_x <= 230.0 && min_y >= 100.0 && max_y <= 360.0 {
        return true;
    }

    // 6. Target display: TARGET_AT = [-146, 381] or bottom right
    if vis.target && min_y >= 370.0 && max_y <= 450.0 {
        return true;
    }

    // 7. Chat strip: [-3, 331]
    if vis.chat && min_y >= 325.0 && max_y <= 460.0 && max_x <= 220.0 {
        return true;
    }

    false
}

pub(super) fn draw(ui: &mut UiRaster<'_>, s: &HudState) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let vis = crate::fork_ext::addons::visibility::published_visibility();
        if vis.any_hidden() {
            ui.ui.quads.retain(|q| !should_suppress_quad(q, &vis));
        }
    }

    let Some(font) = ui.assets.font else { return };
    let c = crate::settings::client();
    let labels = readouts(
        s,
        super::exp_for_level(s.level + 1),
        [c.target_hp_percent, c.xp_details, c.hud_numbers],
    );
    let (anchor, vertical) = (ui.ui.anchor, ui.ui.vertical);
    ui.ui.anchor = Anchor::Left;
    for r in labels {
        ui.ui.vertical = r.vertical;
        let text = crate::ui::wide(&r.text);
        let width = ui.ui.fonts[font].string_width(&text) as f32 * r.scale;
        let x = (r.center[0] as f32 - width / 2.0).round();
        let y = r.center[1] as f32;
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            ui.ui
                .text_scaled(font, x + dx, y + dy, r.scale, [0.0, 0.0, 0.0, 0.95], &text);
        }
        ui.ui.text_scaled(font, x, y, r.scale, r.color, &text);
    }
    (ui.ui.anchor, ui.ui.vertical) = (anchor, vertical);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_health_visibility_suppresses_only_health_gauge_and_percent_label() {
        let mut state = HudState {
            self_hp: 128,
            power: 4,
            max_power: 10,
            exp: 20,
            target_name: Some("Target".into()),
            target_hp: 128,
            ..Default::default()
        };
        let shown = readouts(&state, 100, [true, true, true]);
        assert!(health_visible(&state));
        assert_eq!(shown.len(), 4);
        state.forge_health_hidden = true;
        assert!(
            !health_visible(&state),
            "native health gauge uses this same predicate"
        );
        let hidden = readouts(&state, 100, [true, true, true]);
        assert_eq!(hidden.len(), 3);
        assert_eq!(
            hidden
                .iter()
                .map(|r| (&r.text, r.center))
                .collect::<Vec<_>>(),
            shown[1..]
                .iter()
                .map(|r| (&r.text, r.center))
                .collect::<Vec<_>>(),
            "power, XP and target labels unchanged"
        );
        state.forge_health_hidden = false;
        assert_eq!(readouts(&state, 100, [true, true, true]).len(), 4);
    }
    #[test]
    fn number_readouts_keep_text_positions_and_independent_switches() {
        let s = HudState {
            self_hp: 128,
            power: 123,
            max_power: 456,
            exp: 50,
            target_name: Some("Rat".into()),
            target_hp: 128,
            ..Default::default()
        };
        let all = readouts(&s, 100, [true; 3]);
        assert_eq!(
            all.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["50%", "123/456", "50/100 (50.0%)", "50%"]
        );
        assert_eq!(all[0].center, [99, 35]);
        assert_eq!(
            all[3].center,
            [
                super::super::TARGET_AT[0] + 40,
                super::super::TARGET_AT[1] + 49
            ]
        );
        assert_eq!(all[3].color, [1.0, 0.9, 0.3, 1.0]);
        assert_eq!(all[3].scale, 0.65);
        assert!(
            readouts(&s, 100, [false; 3]).is_empty(),
            "disabled preferences draw no labels"
        );
        let no_values = HudState {
            max_power: 0,
            target_name: None,
            ..s.clone()
        };
        assert_eq!(
            readouts(&no_values, 0, [true; 3]).len(),
            1,
            "unknown power/xp/target has no invented value"
        );
        let target_only = readouts(&s, 100, [true, false, false]);
        assert_eq!(target_only.len(), 1);
        assert_eq!(target_only[0].text, "50%");
    }
}
