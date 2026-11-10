// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Mod HUD Layer: Native HUD pass drawing for Lua mods.
//!
//! Allows Lua addons to issue native HUD draw calls (`eqoa.hud.text`,
//! `eqoa.hud.gauge`, `eqoa.hud.image`, `eqoa.hud.panel`) that render directly in the
//! game's own HUD pass through the authentic `UiRaster` and `esf::Font` (ArialUI).
//!
//! Draw calls are queued per-mod each frame into reusable buffers (zero steady-state
//! allocations) and executed during `fork_hud::draw` when the HUD is visible.

use std::cell::RefCell;
use std::collections::HashMap;
use crate::ui::Layout;

pub const MAX_COMMANDS_PER_MOD_PER_FRAME: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub enum HudCommand {
    Text {
        x: f32,
        y: f32,
        text: String,
        color: [f32; 4],
        scale: f32,
        align: TextAlign,
        outline: bool,
        shadow: bool,
    },
    Gauge {
        x: i32,
        y: i32,
        art: i32,
        fill: f32,
        lit: bool,
    },
    Image {
        ui_id: i32,
        x: i32,
        y: i32,
        w: Option<i32>,
        h: Option<i32>,
        color: Option<[f32; 4]>,
    },
    Panel {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
}

thread_local! {
    pub(crate) static MOD_QUEUES: RefCell<HashMap<String, Vec<HudCommand>>> = RefCell::new(HashMap::new());
}

/// Queue a command for the given addon. Rejects if the per-mod cap is exceeded.
pub fn queue_command(addon_id: &str, cmd: HudCommand) -> Result<(), &'static str> {
    MOD_QUEUES.with(|queues| {
        let mut map = queues.borrow_mut();
        let queue = map.entry(addon_id.to_string()).or_insert_with(|| Vec::with_capacity(32));
        if queue.len() >= MAX_COMMANDS_PER_MOD_PER_FRAME {
            return Err("per_frame_command_limit_exceeded");
        }
        queue.push(cmd);
        Ok(())
    })
}

/// Clear all queued commands across all mods (buffers remain allocated for reuse).
pub fn clear_all() {
    MOD_QUEUES.with(|queues| {
        for queue in queues.borrow_mut().values_mut() {
            queue.clear();
        }
    });
}

/// Clear all queued commands for a specific addon.
pub fn clear_addon(addon_id: &str) {
    MOD_QUEUES.with(|queues| {
        if let Some(queue) = queues.borrow_mut().get_mut(addon_id) {
            queue.clear();
        }
    });
}

/// Draw all queued mod HUD commands through the native `UiRaster` and `Vui`.
pub fn draw(vui: &crate::ui::Vui, ui: &mut crate::ui::UiRaster<'_>) {
    let (orig_anchor, orig_vert) = (ui.ui.anchor, ui.ui.vertical);
    ui.ui.anchor = render::ui::Anchor::Left;
    ui.ui.vertical = render::ui::Vertical::Top;

    MOD_QUEUES.with(|queues| {
        let mut map = queues.borrow_mut();
        // Deterministic order: sort addon ids
        let mut addon_keys: Vec<String> = map.keys().cloned().collect();
        addon_keys.sort();

        for key in addon_keys {
            if let Some(queue) = map.get_mut(&key) {
                for cmd in queue.drain(..) {
                    match cmd {
                        HudCommand::Text { x, y, text, color, scale, align, outline, shadow } => {
                            let Some(font) = ui.assets.font else { continue };
                            let wtext = crate::ui::wide(&text);
                            let width = ui.ui.fonts[font].string_width(&wtext) as f32 * scale;
                            let base_x = match align {
                                TextAlign::Left => x,
                                TextAlign::Center => (x - width / 2.0).round(),
                                TextAlign::Right => (x - width).round(),
                            };
                            let ry = y.round();
                            if outline {
                                for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                                    ui.ui.text_scaled(font, base_x + dx, ry + dy, scale, [0.0, 0.0, 0.0, 0.95], &wtext);
                                }
                            } else if shadow {
                                ui.ui.text_scaled(font, base_x + 1.0, ry + 1.0, scale, [0.0, 0.0, 0.0, 0.85], &wtext);
                            }
                            ui.ui.text_scaled(font, base_x, ry, scale, color, &wtext);
                        }
                        HudCommand::Gauge { x, y, art, fill, lit } => {
                            crate::hud::draw_gauge(vui, ui, art, 136, [x, y], lit, fill.clamp(0.0, 1.0));
                        }
                        HudCommand::Image { ui_id, x, y, w, h, color } => {
                            let alpha = color.map(|c| c[3]).unwrap_or(1.0);
                            if ui_id == 25 {
                                let width = w.unwrap_or(174);
                                vui.blit_bar(ui, &[], [0.0; 4], [x, y], 25, 26, width, 3, alpha, 0, true);
                            } else {
                                let size = [w.unwrap_or(-1), h.unwrap_or(-1)];
                                vui.blit_ex_size(ui, ui_id, [x, y], size, 3, alpha, 0);
                            }
                        }
                        HudCommand::Panel { x, y, w, h } => {
                            let f = [x, y, x + w, y + h];
                            let inside = [f[0] + 2, f[1] + 2, f[2] - 2, f[3] - 2];
                            let corners = |r: [i32; 4]| [
                                [r[0] as f32, r[1] as f32],
                                [r[2] as f32, r[1] as f32],
                                [r[2] as f32, r[3] as f32],
                                [r[0] as f32, r[3] as f32],
                            ];
                            ui.solid(3, [0.08, 0.07, 0.06, 0.9], corners(inside));
                            vui.stretch_blit_ex(ui, 61, [inside[0], inside[1]], [inside[2] - inside[0], inside[3] - inside[1]], 3, 0.6);
                            vui.blit_frame(ui, [f[0], f[1]], [f[2] - f[0], f[3] - f[1]], &vi::ui::frame_sets()[0], 3, 1.0, true);
                        }
                    }
                }
            }
        }
    });

    (ui.ui.anchor, ui.ui.vertical) = (orig_anchor, orig_vert);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_layer_queue_bounds_and_clears() {
        let addon = "test_mod";
        clear_all();
        for i in 0..MAX_COMMANDS_PER_MOD_PER_FRAME {
            assert!(queue_command(addon, HudCommand::Gauge { x: i as i32, y: 0, art: 9, fill: 1.0, lit: true }).is_ok());
        }
        // Exceeding cap returns error
        assert!(queue_command(addon, HudCommand::Gauge { x: 999, y: 0, art: 9, fill: 1.0, lit: true }).is_err());

        // Clear preserves buffers
        clear_all();
        assert!(queue_command(addon, HudCommand::Gauge { x: 0, y: 0, art: 9, fill: 1.0, lit: true }).is_ok());
        clear_all();
    }
}
