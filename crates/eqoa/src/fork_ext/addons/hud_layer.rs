// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Mod HUD Layer: Native HUD pass drawing for Lua mods.
//!
//! Allows Lua addons to issue native HUD draw calls (`eqoa.hud.text`,
//! `eqoa.hud.gauge`, `eqoa.hud.image`, `eqoa.hud.panel`) that render directly in the
//! game's own HUD pass through the authentic `UiRaster` and `esf::Font` (ArialUI).
//!
//! Draw calls use reusable command and text buffers (zero text-storage allocations
//! after warmup) and execute in `fork_hud::draw` when the HUD is visible. Native
//! art composition retains upstream resource-name hash formatting allocations.

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
        target_fade: bool,
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
        target_fade: bool,
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
    static TEXT_POOL: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static WIDE_BUF: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
    static DIRTY: Cell<bool> = const { Cell::new(false) };
}

pub fn take_dirty() -> bool {
    DIRTY.with(|d| d.replace(false))
}

#[cfg(test)]
thread_local! {
    static OBSERVE_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    // UTF-8 construction, command queues, text drawing, native art drawing.
    static PHASE_ALLOCATIONS: Cell<[u64; 4]> = const { Cell::new([0; 4]) };
}

#[cfg(test)]
pub(super) fn observe_allocations(enabled: bool) -> [u64; 4] {
    OBSERVE_ALLOCATIONS.with(|v| v.set(enabled));
    PHASE_ALLOCATIONS.with(|v| v.replace([0; 4]))
}

/// Measure HUD storage and each drawing command separately from Lua table
/// parsing, VM/event delivery and ImGui. Native art uses upstream's hash formatter.
fn hud_call<R>(phase: usize, f: impl FnOnce() -> R) -> R {
    #[cfg(test)]
    if OBSERVE_ALLOCATIONS.with(|v| v.get()) {
        let (result, _, allocations, _) = super::perf_alloc::measure(f);
        PHASE_ALLOCATIONS.with(|v| {
            let mut phases = v.get();
            phases[phase] += allocations;
            v.set(phases);
        });
        return result;
    }
    let _ = phase;
    f()
}

/// Borrow a retained UTF-8 buffer; returned commands recycle it after drawing or clearing.
pub fn text_buffer(text: &str) -> String {
    hud_call(0, || {
        TEXT_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            let available = pool.iter().position(|s| s.capacity() >= text.len());
            let mut buffer = match available {
                Some(index) => pool.swap_remove(index),
                None => pool.pop().unwrap_or_default(),
            };
            buffer.clear();
            buffer.push_str(text);
            buffer
        })
    })
}

fn recycle(cmd: HudCommand) {
    if let HudCommand::Text { text, .. } = cmd {
        recycle_text(text);
    }
}

fn recycle_text(text: String) {
    TEXT_POOL.with(|pool| pool.borrow_mut().push(text));
}

fn plate_color(mut color: [f32; 4], target_fade: bool, target_alpha: f32) -> [f32; 4] {
    if target_fade {
        color[3] *= target_alpha;
    }
    color
}

/// Queue a command for the given addon. Rejects if the per-mod cap is exceeded.
pub fn queue_command(addon_id: &str, cmd: HudCommand) -> Result<(), &'static str> {
    hud_call(1, || {
        MOD_QUEUES.with(|queues| {
            let mut map = queues.borrow_mut();
            if let Some(queue) = map.get_mut(addon_id) {
                if queue.len() >= MAX_COMMANDS_PER_MOD_PER_FRAME {
                    recycle(cmd);
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
    })
}

/// Clear all queued commands across all mods (buffers remain allocated for reuse).
pub fn clear_all() {
    MOD_QUEUES.with(|queues| {
        let mut had_any = false;
        for queue in queues.borrow_mut().values_mut() {
            if !queue.is_empty() {
                had_any = true;
                for cmd in queue.drain(..) {
                    recycle(cmd);
                }
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
                for cmd in queue.drain(..) {
                    recycle(cmd);
                }
                DIRTY.with(|d| d.set(true));
            }
        }
    });
}

use crate::fork_ext::player_stats::Rows as StatLayout;

fn compute_stat_layout(
    vui: &crate::ui::Vui,
    ui: &crate::ui::UiRaster<'_>,
    font: usize,
) -> StatLayout {
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
    crate::fork_ext::player_stats::layout(font_h, wide)
}

/// The native text_scaled geometry, streamed directly after validating the
/// complete string. Missing glyphs still reject the entire command atomically.
fn text_scaled(
    ui: &mut render::ui::Ui,
    font: usize,
    x: f32,
    y: f32,
    scale: f32,
    color: [f32; 4],
    text: &[u16],
) {
    if text.iter().any(|&c| ui.fonts[font].find(c).is_none()) {
        return;
    }
    let (tex, h) = (ui.fonts[font].tex, ui.fonts[font].height as f32 * scale);
    let mut x0 = x;
    for &c in text {
        let &(width, uv) = ui.fonts[font].find(c).expect("validated glyph");
        let x1 = x0 + width as f32 * scale;
        ui.push(render::ui::Quad {
            tex,
            blend: 3,
            pos: [[x0, y], [x1, y], [x1, y + h], [x0, y + h]],
            uv: [
                [uv[0], uv[1]],
                [uv[2], uv[1]],
                [uv[2], uv[3]],
                [uv[0], uv[3]],
            ],
            color,
            clip: None,
        });
        x0 = x1 + scale;
    }
}

/// `Window::text_box(..., rows=1, flags=10)` without its temporary Vecs for
/// plain names. Inline icon/control escapes retain the native interpreter.
fn clipped_name(
    vui: &crate::ui::Vui,
    ui: &mut crate::ui::UiRaster<'_>,
    window: &vi::ui::Window,
    text: &[u16],
    width: i32,
    color: [f32; 4],
) {
    if text.contains(&(b'|' as u16)) {
        window.text_box(vui, ui, text, 0, 0, width, 1, color, 10);
        return;
    }
    let (mut last_width, mut i, mut space, mut fit) = (0, 0, 0, 0);
    let (mut end, mut truncated) = (0, false);
    while i < text.len() && text[i] != 0 {
        if text[i] == b'\n' as u16 {
            end = i;
            truncated = text.get(i + 1).is_some_and(|&c| c != 0);
            break;
        }
        let previous_width = last_width;
        last_width = vui.string_width(ui, &text[..i.min(511)]);
        if last_width >= width - 8 {
            end = if space != 0 { space + 1 } else { fit };
            truncated = text.get(end).is_some_and(|&c| c != 0);
            break;
        }
        if previous_width != last_width {
            fit = i;
        }
        if text[i] == b' ' as u16 {
            space = i;
        }
        i += 1;
        end = i;
    }
    let mut buffer = [0u16; 514];
    let mut len = end.min(511);
    buffer[..len].copy_from_slice(&text[..len]);
    if truncated {
        let dots = [b'.' as u16; 3];
        let dots_width = vui.string_width(ui, &dots);
        if dots_width < width {
            while len > 0 && vui.string_width(ui, &buffer[..len]) + dots_width >= width {
                len -= 1;
            }
            buffer[len..len + 3].copy_from_slice(&dots);
            len += 3;
        }
    }
    if len > 0 {
        let x = window.top_left()[0] + (width >> 1) - (vui.string_width(ui, &buffer[..len]) >> 1);
        let y = window.row_center(vui, ui, 0)[1];
        if let Some(font) = ui.assets.font {
            let height = ui.ui.fonts[font].height as i32;
            let at = [x, y - (height >> 1)];
            text_scaled(
                ui.ui,
                font,
                at[0] as f32,
                at[1] as f32,
                1.0,
                color,
                &buffer[..len],
            );
            if color[3] > 0.0 {
                let text_width = vui.string_width(ui, &buffer[..len]).max(0);
                let a = ui.ui.placed([at[0] as f32, at[1] as f32]);
                let b = ui
                    .ui
                    .placed([(at[0] + text_width) as f32, (at[1] + height) as f32]);
                ui.glyphs.string([a[0], a[1], b[0], b[1]]);
            }
        }
    }
}

/// Draw all queued mod HUD commands through the native `UiRaster` and `Vui`.
pub fn draw(vui: &crate::ui::Vui, ui: &mut crate::ui::UiRaster<'_>) {
    draw_with_target_alpha(vui, ui, 1.0);
}

pub fn draw_with_target_alpha(
    vui: &crate::ui::Vui,
    ui: &mut crate::ui::UiRaster<'_>,
    target_alpha: f32,
) {
    draw_commands(vui, ui, target_alpha);
}

fn draw_commands(vui: &crate::ui::Vui, ui: &mut crate::ui::UiRaster<'_>, target_alpha: f32) {
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
                let phase = if matches!(&cmd, HudCommand::Text { .. }) {
                    2
                } else {
                    3
                };
                hud_call(phase, || match cmd {
                    HudCommand::Text {
                        mut x,
                        mut y,
                        text,
                        target_fade,
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
                        let color = plate_color(color, target_fade, target_alpha);
                        let Some(font) = ui.assets.font else {
                            recycle_text(text);
                            return;
                        };
                        if let Some((col, row, is_val)) = stat_cell {
                            let l = stat_layout
                                .get_or_insert_with(|| compute_stat_layout(vui, ui, font));
                            if col < 2 && row < 8 {
                                x = if is_val {
                                    l.rights[col]
                                } else {
                                    l.lefts[col].round()
                                };
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
                                w.init(vui, ui, [x.round() as i32, y.round() as i32], clip_w, 1, 0);
                                w.open(true);
                                clipped_name(vui, ui, &w, &wtext, clip_w, color);
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
                                    text_scaled(
                                        ui.ui,
                                        font,
                                        base_x + dx,
                                        ry + dy,
                                        scale,
                                        [0.0, 0.0, 0.0, 0.95],
                                        &wtext,
                                    );
                                }
                            } else if shadow {
                                text_scaled(
                                    ui.ui,
                                    font,
                                    base_x + 1.0,
                                    ry + 1.0,
                                    scale,
                                    [0.0, 0.0, 0.0, 0.85],
                                    &wtext,
                                );
                            }
                            text_scaled(ui.ui, font, base_x, ry, scale, color, &wtext);
                        });
                        recycle_text(text);
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
                        target_fade,
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
                        let alpha =
                            plate_color(color.unwrap_or([1.0; 4]), target_fade, target_alpha)[3];
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
                });
            }
        }
    });

    (ui.ui.anchor, ui.ui.vertical) = (orig_anchor, orig_vert);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_plate_alpha_tracks_native_glow_and_leaves_other_text_opaque() {
        for glow in [0.3, 0.62, 0.94, 1.0] {
            assert_eq!(plate_color([0.0, 0.0, 0.0, 1.0], true, glow)[3], glow);
            assert_eq!(plate_color([1.0; 4], true, glow)[3], glow);
            assert_eq!(plate_color([1.0; 4], false, glow)[3], 1.0);
        }
        assert_eq!(plate_color([1.0, 1.0, 1.0, 0.5], true, 0.3)[3], 0.15);
    }

    #[test]
    fn text_and_utf16_storage_survive_clear_and_command_limit_rejection() {
        fn command(value: &str) -> HudCommand {
            HudCommand::Text {
                x: 0.0,
                y: 0.0,
                text: text_buffer(value),
                target_fade: false,
                color: [1.0; 4],
                scale: 1.0,
                align: TextAlign::Left,
                outline: false,
                shadow: false,
                anchor: render::ui::Anchor::Left,
                vertical: render::ui::Vertical::Top,
                clip_width: None,
                stat_cell: None,
            }
        }
        clear_all();
        // Warm both queue and buffer pools, including the rejected command's buffer.
        for _ in 0..=MAX_COMMANDS_PER_MOD_PER_FRAME {
            let _ = queue_command("text_reuse", command("Non-ASCII target \u{1f40a}"));
        }
        WIDE_BUF.with(|b| {
            b.borrow_mut()
                .extend("Non-ASCII target \u{1f40a}".encode_utf16())
        });
        clear_addon("text_reuse");
        let (_, _, allocations, _) = super::super::perf_alloc::measure(|| {
            for frame in 0..8 {
                for _ in 0..MAX_COMMANDS_PER_MOD_PER_FRAME {
                    queue_command("text_reuse", command("Non-ASCII target \u{1f40a}")).unwrap();
                }
                assert!(queue_command("text_reuse", command("Short")).is_err());
                WIDE_BUF.with(|b| {
                    let mut wide = b.borrow_mut();
                    wide.clear();
                    wide.extend("Non-ASCII target \u{1f40a}".encode_utf16());
                    assert_eq!(wide.last(), Some(&0xdc0a));
                });
                if frame % 2 == 0 {
                    clear_all();
                } else {
                    clear_addon("text_reuse");
                }
            }
        });
        assert_eq!(
            allocations, 0,
            "clear and cap rejection must return retained text storage"
        );
    }

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
