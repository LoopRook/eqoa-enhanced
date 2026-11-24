// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A12: world overlays for a mod (spec section 7, "World overlays"): `eqoa.camera.project`, which says where a
//! point of the world falls on the screen a mod draws on. The three hide switches (`name_plates`, `damage_numbers`,
//! `target_rings`) are `eqoa.hud.hide` elements (`visibility.rs`).
//!
//! The view is the one the renderer used for the frame being drawn: `fork_report::view` hands it over from the
//! viewer right after the frame's view is made, before the mods' Lua runs (`prepare_addons`), so a plate follows a
//! head with no frame of lag. Presentation, not game logic: plain `f64` maths as the name plates' own projection
//! (`play::labels`), no `ee::*`, because nothing here is an input to the game's state or its replies.
use super::{
    host::Host,
    native::{bind, describe, error, record},
    native_core::CORE,
};
use glam::Mat4;
use mlua::{Result, Value};
use serde_json::json;
use std::cell::Cell;

pub(crate) const ROUND: u8 = 8;

thread_local! {
    /// The frame's view-projection (in the game's world units) and the size in pixels of the surface the mods
    /// draw on, as of the frame being drawn.
    static VIEW: Cell<Option<(Mat4, (u32, u32))>> = const { Cell::new(None) };
    /// Where the game puts each creature's name plate this frame (`Puppet::name_at`), by entity id.
    static PLATES: std::cell::RefCell<std::collections::HashMap<u32, [f32; 3]>> = std::cell::RefCell::new(Default::default());
}

/// The viewer's hook: this frame's plate points (`Play::fork_plate_points`).
pub(crate) fn set_plates(points: Vec<(u32, [f32; 3])>) {
    PLATES.with(|p| {
        let mut p = p.borrow_mut();
        p.clear();
        p.extend(points);
    });
}

/// The point the game puts `id`'s plate at this frame, if it draws one.
pub(crate) fn plate_point(id: u32) -> Option<[f32; 3]> {
    PLATES.with(|p| p.borrow().get(&id).copied())
}

/// The viewer's hook: this frame's view-projection and the pixel size of the surface the mods draw on.
pub(crate) fn set_view(view_proj: Mat4, size: (u32, u32)) {
    VIEW.set(Some((view_proj, size)));
}

#[cfg(test)]
pub(crate) fn clear_view() {
    VIEW.set(None);
}

/// Where `world` falls on a surface of `size` pixels seen through `view_proj`: `(x, y, depth, on_screen, behind)`.
/// `depth` is the distance along the view axis (the clip w); a point behind the eye (or on its plane) has no
/// screen position. `on_screen` is inside the view's sides and between its near and far planes.
pub(crate) fn project(view_proj: Mat4, size: (u32, u32), world: [f64; 3]) -> (Option<[f64; 2]>, f64, bool, bool) {
    // in doubles, as `play::labels`: at Freeport's x 25,300 a float steps 2 mm
    let clip = view_proj.as_dmat4() * glam::DVec4::new(world[0], world[1], world[2], 1.0);
    // a point too large to compute, or so close to the eye's plane that the division overflows, cannot be put on
    // the screen: it is reported as behind (depth 0), so every field is a finite number, or nil only when behind
    if !clip.to_array().iter().all(|v| v.is_finite()) {
        return (None, 0.0, false, true);
    }
    let behind = clip.w <= 0.0;
    if behind {
        return (None, clip.w, false, true);
    }
    let (nx, ny, nz) = (clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
    if ![nx, ny, nz].iter().all(|v| v.is_finite()) {
        return (None, 0.0, false, true);
    }
    let x = (nx + 1.0) * 0.5 * size.0 as f64;
    let y = (1.0 - ny) * 0.5 * size.1 as f64;
    if !x.is_finite() || !y.is_finite() {
        return (None, 0.0, false, true);
    }
    let inside = nx.abs() <= 1.0 && ny.abs() <= 1.0 && (0.0..=1.0).contains(&nz);
    (Some([x, y]), clip.w, inside, false)
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(n) => Some(*n as f64),
        Value::Number(n) if n.is_finite() => Some(*n),
        _ => None,
    }
}

fn vec3(v: &Value) -> Option<[f64; 3]> {
    let t = v.as_table()?;
    Some([
        number(&t.get::<Value>("x").ok()?)?,
        number(&t.get::<Value>("y").ok()?)?,
        number(&t.get::<Value>("z").ok()?)?,
    ])
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    describe(lua, "eqoa.camera.Projection", "supported", Some(ROUND), "");
    for field in ["x", "y", "depth", "on_screen", "behind"] {
        describe(lua, &format!("eqoa.camera.Projection.{field}"), "supported", Some(ROUND), "");
    }
    bind(
        lua,
        "camera",
        "project",
        lua.create_function(|lua, args: mlua::MultiValue| {
            const SYMBOL: &str = "eqoa.camera.project";
            let mut args = args.into_iter();
            let (Some(point), None) = (args.next(), args.next()) else {
                return error(lua, SYMBOL, "invalid_argument", "give one position: {x=, y=, z=}");
            };
            let Some(world) = vec3(&point) else {
                return error(lua, SYMBOL, "invalid_argument", "position needs numbers x, y and z");
            };
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(lua, SYMBOL, "not_in_world", "enter a loaded world first");
            }
            let Some((view_proj, size)) = VIEW.get() else {
                return error(lua, SYMBOL, "unavailable", "waiting for the first frame of the world");
            };
            let (at, depth, on_screen, behind) = project(view_proj, size, world);
            let record_value = json!({
                "x": at.map(|a| a[0]),
                "y": at.map(|a| a[1]),
                "depth": depth,
                "on_screen": on_screen,
                "behind": behind,
            });
            lua.pack_multi((record(lua, "eqoa.camera.Projection", &record_value)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "camera",
        "project_entity",
        lua.create_function(|lua, (target, lift): (Value, Option<Value>)| {
            const SYMBOL: &str = "eqoa.camera.project_entity";
            let id = match &target {
                Value::Table(t) => t.get::<Value>("id").ok().as_ref().and_then(number),
                other => number(other),
            }
            .filter(|n| n.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(n));
            let lift = match lift {
                None | Some(Value::Nil) => Some(0.0),
                Some(v) => number(&v),
            };
            let (Some(id), Some(lift)) = (id, lift) else {
                return error(lua, SYMBOL, "invalid_argument", "give an entity or its id, and optionally a height in game units to lift the point");
            };
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(lua, SYMBOL, "not_in_world", "enter a loaded world first");
            }
            let Some((view_proj, size)) = VIEW.get() else {
                return error(lua, SYMBOL, "unavailable", "waiting for the first frame of the world");
            };
            let Some(at) = plate_point(id as u32) else {
                return error(lua, SYMBOL, "not_found", "the game draws no name plate for that entity now");
            };
            let world = [at[0] as f64, at[1] as f64 + lift, at[2] as f64];
            let (px, depth, on_screen, behind) = project(view_proj, size, world);
            let value = json!({"x": px.map(|a| a[0]), "y": px.map(|a| a[1]), "depth": depth, "on_screen": on_screen, "behind": behind});
            lua.pack_multi((record(lua, "eqoa.camera.Projection", &value)?, Value::Nil))
        })?,
    )?;
    Ok(())
}

/// What one creature's puppet says about its plate, for `plate_points`.
#[derive(Clone, Copy)]
pub(crate) struct PlateSource {
    pub id: u32,
    pub has_creature: bool,
    pub name_empty: bool,
    pub shown: bool,
    pub name_at: Option<[f32; 3]>,
}

/// The creatures `Play::plates` would give a plate this frame, with the point it puts the plate at (`Puppet::name_at`:
/// 0.6 above the name node, or over a prop's top): the same tests as `plates`, minus the loading screen and the
/// hide switch (a mod that hides the plates still needs the points). Round A12, for `eqoa.camera.project_entity`.
pub(crate) fn plate_points_of(sources: impl IntoIterator<Item = PlateSource>) -> Vec<(u32, [f32; 3])> {
    sources
        .into_iter()
        .filter_map(|s| {
            let at = s.name_at?;
            (s.has_creature && !s.name_empty && s.shown).then_some((s.id, at))
        })
        .collect()
}
