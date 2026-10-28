//! HUD-number adapter; stock HUD layout remains in the parent.
use super::{HudState, UiRaster};
use render::ui::{Anchor, Vertical};

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
        let pct = ((s.self_hp as f32 / 255.0) * 100.0).round() as i32;
        out.push(Readout {
            text: format!("{pct}%"),
            center: [99, 35],
            scale: 0.6,
            color: [1.0; 4],
            vertical: Vertical::Top,
        });
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

pub(super) fn draw(ui: &mut UiRaster<'_>, s: &HudState) {
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
