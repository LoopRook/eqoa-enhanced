// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Mod HUD Layer: Native HUD pass drawing for Lua mods.
//!
//! Allows Lua addons to issue native HUD draw calls (`eqoa.hud.text`,
//! `eqoa.hud.gauge`, `eqoa.hud.image`, `eqoa.hud.panel`) that render directly in the
//! game's own HUD pass through the authentic `UiRaster` and `esf::Font` (ArialUI).
//!
//! Draw calls are queued per-mod each frame into reusable buffers (zero steady-state
//! allocations) and executed during `fork_hud::draw` when the HUD is visible.

use crate::ui::Layout;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

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
        anchor: render::ui::Anchor,
        vertical: render::ui::Vertical,
        clip_width: Option<i32>,
        stat_cell: Option<(usize, usize, bool)>,
    },
    Gauge {
        x: i32,
        y: i32,
        art: i32,
        fill: f32,
        lit: bool,
        anchor: render::ui::Anchor,
        vertical: render::ui::Vertical,
    },
    Image {
        ui_id: i32,
        x: i32,
        y: i32,
        w: Option<i32>,
        h: Option<i32>,
        color: Option<[f32; 4]>,
        anchor: render::ui::Anchor,
        vertical: render::ui::Vertical,
    },
    Panel {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        anchor: render::ui::Anchor,
        vertical: render::ui::Vertical,
    },
}

thread_local! {
    pub(crate) static MOD_QUEUES: RefCell<BTreeMap<String, Vec<HudCommand>>> = const { RefCell::new(BTreeMap::new()) };
    static WIDE_BUF: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
    static DIRTY: Cell<bool> = const { Cell::new(false) };
}

pub fn take_dirty() -> bool {
    DIRTY.with(|d| d.replace(false))
}

/// Queue a command for the given addon. Rejects if the per-mod cap is exceeded.
pub fn queue_command(addon_id: &str, cmd: HudCommand) -> Result<(), &'static str> {
    MOD_QUEUES.with(|queues| {
        let mut map = queues.borrow_mut();
        if let Some(queue) = map.get_mut(addon_id) {
            if queue.len() >= MAX_COMMANDS_PER_MOD_PER_FRAME {
                return Err("per_frame_command_limit_exceeded");
            }
            queue.push(cmd);
            DIRTY.with(|d| d.set(true));
            return Ok(());
        }
        let mut queue = Vec::with_capacity(32);
        queue.push(cmd);
        map.insert(addon_id.to_string(), queue);
        DIRTY.with(|d| d.set(true));
        Ok(())
    })
}

/// Clear all queued commands across all mods (buffers remain allocated for reuse).
pub fn clear_all() {
    MOD_QUEUES.with(|queues| {
        let mut had_any = false;
        for queue in queues.borrow_mut().values_mut() {
            if !queue.is_empty() {
                had_any = true;
                queue.clear();
            }
        }
        if had_any {
            DIRTY.with(|d| d.set(true));
        }
    });
}

/// Clear all queued commands for a specific addon.
pub fn clear_addon(addon_id: &str) {
    MOD_QUEUES.with(|queues| {
        if let Some(queue) = queues.borrow_mut().get_mut(addon_id) {
            if !queue.is_empty() {
                queue.clear();
                DIRTY.with(|d| d.set(true));
            }
        }
    });
}

struct StatLayout {
    scale: f32,
    lefts: [f32; 2],
    rights: [f32; 2],
    top: f32,
    step: f32,
}

fn compute_stat_layout(
    vui: &crate::ui::Vui,
    ui: &crate::ui::UiRaster<'_>,
    font: usize,
) -> StatLayout {
    const FRAME: [i32; 4] = [200, 54, 348, 186];
    const BORDER: [f32; 2] = [7.0, 9.0];
    const PAD: [f32; 2] = [3.0, 2.0];
    const GAP: f32 = 6.0;
    const LEADING: f32 = 0.08;
    const SCALE: f32 = 0.6;
    const ROOM: &[u16] = &[
        b' ' as u16,
        b'8' as u16,
        b'8' as u16,
        b'8' as u16,
        b'8' as u16,
    ];
    let f = &ui.ui.fonts[font];
    let font_h = (f.height as f32).max(1.0);
    let room = f.string_width(ROOM) as f32;
    let mut wide = [0.0f32; 2];
    WIDE_BUF.with(|wb| {
        let mut wbuf = wb.borrow_mut();
        for (col, rows) in crate::fork_ext::player_stats::ROWS.iter().enumerate() {
            let mut max_w = 0.0f32;
            for &(word, _) in rows {
                let s = vui.words.get(word, vui.language);
                wbuf.clear();
                wbuf.extend(s.encode_utf16());
                let w = f.string_width(&wbuf) as f32;
                if w > max_w {
                    max_w = w;
                }
            }
            wide[col] = max_w + room;
        }
    });
    let (x0, y0, x1, y1) = (
        FRAME[0] as f32,
        FRAME[1] as f32,
        FRAME[2] as f32,
        FRAME[3] as f32,
    );
    let (left, right) = (x0 + BORDER[0] + PAD[0], x1 - BORDER[0] - PAD[0]);
    let (top, bottom) = (y0 + BORDER[1] + PAD[1], y1 - BORDER[1] - PAD[1]);
    let by_height = (bottom - top) / (8.0 * font_h * (1.0 + LEADING));
    let by_width = (right - left - GAP) / (wide[0] + wide[1]).max(1.0);
    let scale = SCALE.min(by_height).min(by_width);
    let step = (bottom - top) / 8.0;
    StatLayout {
        scale,
        lefts: [left.round(), (right - wide[1] * scale).round()],
        rights: [left + wide[0] * scale, right],
        top: top + (step - font_h * scale) / 2.0,
        step,
    }
}

/// Draw all queued mod HUD commands through the native `UiRaster` and `Vui`.
pub fn draw(vui: &crate::ui::Vui, ui: &mut crate::ui::UiRaster<'_>) {
    if crate::hideui::hidden() {
        clear_all();
        return;
    }
    let (orig_anchor, orig_vert) = (ui.ui.anchor, ui.ui.vertical);
    let mut stat_layout: Option<StatLayout> = None;

    MOD_QUEUES.with(|queues| {
        let mut map = queues.borrow_mut();
        for queue in map.values_mut() {
            for cmd in queue.drain(..) {
                match cmd {
                    HudCommand::Text {
                        mut x,
                        mut y,
                        text,
                        color,
                        mut scale,
                        align,
                        outline,
                        shadow,
                        anchor,
                        vertical,
                        clip_width,
                        stat_cell,
                    } => {
                        ui.ui.anchor = anchor;
                        ui.ui.vertical = vertical;
                        let Some(font) = ui.assets.font else { continue };
                        if let Some((col, row, is_val)) = stat_cell {
                            let l = stat_layout
                                .get_or_insert_with(|| compute_stat_layout(vui, ui, font));
                            if col < 2 && row < 8 {
                                x = if is_val { l.rights[col] } else { l.lefts[col] };
                                y = l.top + row as f32 * l.step;
                                scale = l.scale;
                            }
                        }
                        WIDE_BUF.with(|wb| {
                            let mut wtext = wb.borrow_mut();
                            wtext.clear();
                            wtext.extend(text.encode_utf16());
                            if let Some(clip_w) = clip_width {
                                let mut w = vi::ui::Window::default();
                                w.init(
                                    vui,
                                    ui,
                                    [x.round() as i32, y.round() as i32],
                                    clip_w,
                                    1,
                                    0,
                                );
                                w.open(true);
                                w.text_box(vui, ui, &wtext, 0, 0, clip_w, 1, color, 10);
                                return;
                            }
                            let width = ui.ui.fonts[font].string_width(&wtext) as f32 * scale;
                            let base_x = match align {
                                TextAlign::Left => x,
                                TextAlign::Center => (x - width / 2.0).round(),
                                TextAlign::Right => (x - width).round(),
                            };
                            let ry = y.round();
                            if outline {
                                for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                                    ui.ui.text_scaled(
                                        font,
                                        base_x + dx,
                                        ry + dy,
                                        scale,
                                        [0.0, 0.0, 0.0, 0.95],
                                        &wtext,
                                    );
                                }
                            } else if shadow {
                                ui.ui.text_scaled(
                                    font,
                                    base_x + 1.0,
                                    ry + 1.0,
                                    scale,
                                    [0.0, 0.0, 0.0, 0.85],
                                    &wtext,
                                );
                            }
                            ui.ui.text_scaled(font, base_x, ry, scale, color, &wtext);
                        });
                    }
                    HudCommand::Gauge {
                        x,
                        y,
                        art,
                        fill,
                        lit,
                        anchor,
                        vertical,
                    } => {
                        ui.ui.anchor = anchor;
                        ui.ui.vertical = vertical;
                        crate::hud::draw_gauge(
                            vui,
                            ui,
                            art,
                            136,
                            [x, y],
                            lit,
                            fill.clamp(0.0, 1.0),
                        );
                    }
                    HudCommand::Image {
                        ui_id,
                        x,
                        y,
                        w,
                        h,
                        color,
                        anchor,
                        vertical,
                    } => {
                        ui.ui.anchor = anchor;
                        ui.ui.vertical = vertical;
                        let alpha = color.map(|c| c[3]).unwrap_or(1.0);
                        if ui_id == 25 {
                            let width = w.unwrap_or(174);
                            vui.blit_bar(
                                ui,
                                &[],
                                [0.0; 4],
                                [x, y],
                                25,
                                26,
                                width,
                                3,
                                alpha,
                                0,
                                true,
                            );
                        } else {
                            let size = [w.unwrap_or(-1), h.unwrap_or(-1)];
                            vui.blit_ex_size(ui, ui_id, [x, y], size, 3, alpha, 0);
                        }
                    }
                    HudCommand::Panel {
                        x,
                        y,
                        w,
                        h,
                        anchor,
                        vertical,
                    } => {
                        ui.ui.anchor = anchor;
                        ui.ui.vertical = vertical;
                        let f = [x, y, x + w, y + h];
                        let inside = [f[0] + 2, f[1] + 2, f[2] - 2, f[3] - 2];
                        let corners = |r: [i32; 4]| {
                            [
                                [r[0] as f32, r[1] as f32],
                                [r[2] as f32, r[1] as f32],
                                [r[2] as f32, r[3] as f32],
                                [r[0] as f32, r[3] as f32],
                            ]
                        };
                        ui.solid(3, [0.05, 0.06, 0.08, 0.85], corners(inside));
                        vui.stretch_blit_ex(
                            ui,
                            61,
                            [inside[0], inside[1]],
                            [inside[2] - inside[0], inside[3] - inside[1]],
                            3,
                            0.6,
                        );
                        vui.blit_frame(
                            ui,
                            [f[0], f[1]],
                            [f[2] - f[0], f[3] - f[1]],
                            &vi::ui::frame_sets()[0],
                            3,
                            1.0,
                            true,
                        );
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
            assert!(
                queue_command(
                    addon,
                    HudCommand::Gauge {
                        x: i as i32,
                        y: 0,
                        art: 9,
                        fill: 1.0,
                        lit: true,
                        anchor: render::ui::Anchor::Left,
                        vertical: render::ui::Vertical::Top,
                    }
                )
                .is_ok()
            );
        }
        // Exceeding cap returns error
        assert!(
            queue_command(
                addon,
                HudCommand::Gauge {
                    x: 999,
                    y: 0,
                    art: 9,
                    fill: 1.0,
                    lit: true,
                    anchor: render::ui::Anchor::Left,
                    vertical: render::ui::Vertical::Top,
                }
            )
            .is_err()
        );

        // Clear preserves buffers
        clear_all();
        assert!(
            queue_command(
                addon,
                HudCommand::Gauge {
                    x: 0,
                    y: 0,
                    art: 9,
                    fill: 1.0,
                    lit: true,
                    anchor: render::ui::Anchor::Left,
                    vertical: render::ui::Vertical::Top,
                }
            )
            .is_ok()
        );
        clear_all();
    }
}
