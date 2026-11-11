//! Owned texture images in the addon HUD pass. Rounded square geometry masks a circle.
use super::{
    native::error,
    textures::{Handle, Textures},
};
use dear_imgui_rs::sys;
use mlua::{AnyUserData, Lua, Result, Table, Value};
use std::{cell::RefCell, rc::Rc};
pub(crate) fn image(lua: &Lua, t: &Table) -> Result<mlua::MultiValue> {
    let info: Table = t.get("texture")?;
    let ud: AnyUserData = info.get("texture")?;
    let handle = ud.borrow::<Handle>()?;
    let bank = lua
        .app_data_ref::<Rc<RefCell<Textures>>>()
        .map(|b| b.clone());
    if bank.as_ref().is_none_or(|b| !Rc::ptr_eq(b, &handle.owner)) {
        return error(
            lua,
            "eqoa.hud.image",
            "stale_handle",
            "texture belongs to another VM",
        );
    }
    let (dw, dh) = super::textures::dimensions(&handle)?;
    let x: f32 = t.get("x")?;
    let y: f32 = t.get("y")?;
    let w = t
        .get::<Option<f32>>("w")?
        .or(t.get("width")?)
        .unwrap_or(info.get::<Option<f32>>("width")?.unwrap_or(dw as f32));
    let h = t
        .get::<Option<f32>>("h")?
        .or(t.get("height")?)
        .unwrap_or(info.get::<Option<f32>>("height")?.unwrap_or(dh as f32));
    let shape = t
        .get::<Option<String>>("shape")?
        .unwrap_or_else(|| "square".into());
    if ![x, y, w, h].iter().all(|n| n.is_finite())
        || w <= 0.0
        || h <= 0.0
        || w > 4096.0
        || h > 4096.0
        || !matches!(shape.as_str(), "circle" | "square")
        || shape == "circle" && (w - h).abs() > 0.01
    {
        return error(
            lua,
            "eqoa.hud.image",
            "invalid_argument",
            "finite positive image dimensions required; circle images must be square",
        );
    }
    let m = super::hud_layout::frame_metrics(super::hud_layout::get_window_size());
    let anchor = t
        .get::<Option<String>>("anchor")?
        .unwrap_or_else(|| "left".into());
    let vertical = t
        .get::<Option<String>>("vertical")?
        .unwrap_or_else(|| "top".into());
    let a = super::hud_layout::transform_point([x, y], &anchor, &vertical, m);
    let b = super::hud_layout::transform_point([x + w, y + h], &anchor, &vertical, m);
    let uv = info.get::<Option<Table>>("uv")?;
    let coords = if let Some(uv) = uv {
        [uv.get(1)?, uv.get(2)?, uv.get(3)?, uv.get(4)?]
    } else {
        [0.0f32, 0.0, 1.0, 1.0]
    };
    if !coords
        .iter()
        .all(|x| x.is_finite() && (0.0..=1.0).contains(x))
    {
        return error(
            lua,
            "eqoa.hud.image",
            "invalid_argument",
            "invalid texture UV rectangle",
        );
    }
    let tex = Textures::raw(&ud)?;
    unsafe {
        if sys::igGetCurrentContext().is_null() {
            return error(
                lua,
                "eqoa.hud.image",
                "unavailable",
                "HUD draw context unavailable",
            );
        }
        let draw = sys::igGetForegroundDrawList_ViewportPtr(sys::igGetMainViewport());
        let a = sys::ImVec2 { x: a[0], y: a[1] };
        let b = sys::ImVec2 { x: b[0], y: b[1] };
        let u = sys::ImVec2 {
            x: coords[0],
            y: coords[1],
        };
        let v = sys::ImVec2 {
            x: coords[2],
            y: coords[3],
        };
        if shape == "circle" {
            sys::ImDrawList_AddImageRounded(
                draw,
                tex,
                a,
                b,
                u,
                v,
                0xffffffff,
                (b.x - a.x) * 0.5,
                0,
            );
        } else {
            sys::ImDrawList_AddImage(draw, tex, a, b, u, v, 0xffffffff);
        }
    }
    lua.pack_multi((true, Value::Nil))
}
