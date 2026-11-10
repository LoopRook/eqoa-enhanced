// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! HUD frame, layout rects and visibility for `eqoa.hud.frame()`, `eqoa.hud.layout()`, `eqoa.hud.visible()`.
use serde_json::{Value as Json, json};

pub const SCREEN_W: f32 = 640.0;
pub const SCREEN_H: f32 = 448.0;

static WINDOW_SIZE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new((640u64 << 32) | 448u64);

pub fn set_window_size(w: u32, h: u32) {
    let val = ((w as u64) << 32) | (h as u64);
    WINDOW_SIZE.store(val, std::sync::atomic::Ordering::Release);
}

pub fn get_window_size() -> (u32, u32) {
    let val = WINDOW_SIZE.load(std::sync::atomic::Ordering::Acquire);
    ((val >> 32) as u32, val as u32)
}

pub fn frame_metrics(window_size: (u32, u32)) -> (f32, f32, f32, f32, f32, f32) {
    let (tw, th) = (window_size.0 as f32, window_size.1 as f32);
    let fw = (th * 4.0 / 3.0).min(tw);
    let fh = if fw < th * 4.0 / 3.0 { fw * 3.0 / 4.0 } else { th };
    let ox = (tw - fw) / 2.0;
    let oy = (th - fh) / 2.0;
    let sx = fw / SCREEN_W;
    let sy = fh / SCREEN_H;
    let side = if sx > 0.0 { (ox / sx).floor() } else { 0.0 };
    let top = if sy > 0.0 { (oy / sy).floor() } else { 0.0 };
    (ox, oy, sx, sy, side, top)
}

pub fn transform_point(
    p: [f32; 2],
    anchor: &str,
    vertical: &str,
    metrics: (f32, f32, f32, f32, f32, f32),
) -> [f32; 2] {
    let (ox, oy, sx, sy, side, top) = metrics;
    let x_placed = match anchor {
        "left" => p[0] - side,
        "right" => p[0] + side,
        _ => p[0],
    };
    let y_placed = match vertical {
        "top" => p[1] - top,
        "bottom" => p[1] + top,
        _ => p[1],
    };
    [ox + x_placed * sx, oy + y_placed * sy]
}

pub fn transform_rect(
    r: [f32; 4],
    anchor: &str,
    vertical: &str,
    metrics: (f32, f32, f32, f32, f32, f32),
) -> [f32; 4] {
    let p0 = transform_point([r[0], r[1]], anchor, vertical, metrics);
    let p1 = transform_point([r[0] + r[2], r[1] + r[3]], anchor, vertical, metrics);
    [p0[0], p0[1], p1[0] - p0[0], p1[1] - p0[1]]
}

pub fn hud_frame_json(window_size: (u32, u32)) -> Json {
    let (ox, oy, sx, sy, side, top) = frame_metrics(window_size);
    json!({
        "width": 640,
        "height": 448,
        "offset_x": ox,
        "offset_y": oy,
        "scale_x": sx,
        "scale_y": sy,
        "side": side,
        "top": top,
        "window_width": window_size.0,
        "window_height": window_size.1,
    })
}

pub fn hud_layout_json(window_size: (u32, u32)) -> Json {
    let m = frame_metrics(window_size);
    let to_rect = |x: f32, y: f32, w: f32, h: f32, anc: &str, vert: &str| {
        let out = transform_rect([x, y, w, h], anc, vert, m);
        json!({
            "frame_x": x,
            "frame_y": y,
            "frame_width": w,
            "frame_height": h,
            "x": out[0],
            "y": out[1],
            "width": out[2],
            "height": out[3],
        })
    };

    let mut members = Vec::with_capacity(4);
    for k in 0..4 {
        let base_y = 93.0 + 48.0 * k as f32;
        members.push(json!({
            "index": k + 1,
            "panel": to_rect(20.0, base_y, 174.0, 46.0, "left", "top"),
            "health": to_rect(30.0, base_y + 25.0, 136.0, 8.0, "left", "top"),
            "power": to_rect(30.0, base_y + 35.0, 136.0, 8.0, "left", "top"),
        }));
    }

    json!({
        "player": {
            "panel": to_rect(20.0, 15.0, 174.0, 50.0, "left", "top"),
            "health": to_rect(30.0, 25.0, 136.0, 8.0, "left", "top"),
            "power": to_rect(30.0, 35.0, 136.0, 8.0, "left", "top"),
            "exp": to_rect(30.0, 45.0, 136.0, 8.0, "left", "top"),
        },
        "group_members": members,
        "target": {
            "panel": to_rect(20.0, 348.0, 174.0, 75.0, "left", "bottom"),
            "health": to_rect(30.0, 398.0, 136.0, 8.0, "left", "bottom"),
            "power": to_rect(30.0, 408.0, 136.0, 8.0, "left", "bottom"),
        },
        "target_of_target": {
            "panel": to_rect(20.0, 300.0, 174.0, 46.0, "left", "bottom"),
            "health": to_rect(30.0, 325.0, 136.0, 8.0, "left", "bottom"),
            "power": to_rect(30.0, 335.0, 136.0, 8.0, "left", "bottom"),
        },
        "pet": {
            "panel": to_rect(20.0, 285.0, 174.0, 20.0, "left", "top"),
        },
        "stats_box": {
            "panel": to_rect(200.0, 54.0, 148.0, 132.0, "left", "top"),
        },
    })
}

pub fn hud_element_visible(element: &str) -> bool {
    if crate::hideui::hidden() {
        return false;
    }
    let v = super::visibility::effective_visibility();
    match element {
        "health" | "health_bar" => !v.health_bar,
        "power" | "power_bar" => !v.power_bar,
        "experience_main" | "exp" | "main_exp_bar" => !v.main_exp_bar,
        "experience_secondary" | "secondary_exp_bar" => !v.secondary_exp_bar,
        "target" => !v.target,
        "group_panel" | "group" => !v.group_panel,
        "pet" => !v.pet,
        "compass" => !v.compass,
        "active_effects" | "effects" => !v.active_effects,
        "chat" => !v.chat,
        "hotbar" | "ability_bar" => !v.ability_bar,
        "stats_box" => true,
        "target_of_target" => !v.target,
        "mana_bars" => true,
        _ => true,
    }
}
