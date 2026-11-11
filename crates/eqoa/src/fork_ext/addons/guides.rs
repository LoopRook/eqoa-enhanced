//! Mod-owned guidance; no personal/group waypoint writes.
use super::{
    host::Host,
    native::{bind, error},
    native_core::{CORE, Core},
};
use mlua::{Result, Table, Value};
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap};
struct Guide {
    world: i32,
    at: Option<[f32; 2]>,
    entity: Option<u32>,
    dots: Vec<[f32; 2]>,
    label: Option<String>,
}
thread_local! {static GUIDES:RefCell<BTreeMap<u64,Guide>>=const{RefCell::new(BTreeMap::new())};}
#[cfg(test)]
pub(crate) fn guide_count()->usize { GUIDES.with(|g|g.borrow().len()) }
fn point(t: &Table) -> Result<[f32; 2]> {
    let p = [t.get::<f32>("x")?, t.get::<f32>("z")?];
    if !p.iter().all(|v| v.is_finite()) {
        return Err(mlua::Error::runtime("finite coordinates required"));
    }
    Ok(p)
}
fn entity_point(c: &Core, id: u32, world: i32) -> Option<[f32; 2]> {
    let record = c
        .view
        .entities
        .iter()
        .find(|e| e["id"].as_u64() == Some(id as u64))
        .or_else(|| {
            c.view.group["members"].as_array()?.iter().find(|e| {
                e["id"].as_u64() == Some(id as u64) && e["active"].as_bool() == Some(true)
            })
        })?;
    let w = record["world_id"]
        .as_i64()
        .map(|v| v as i32)
        .or_else(|| c.entity_worlds.get(&id).copied())
        .or(c.view.world_id)?;
    if w != world {
        return None;
    }
    Some([
        record["position"]["x"].as_f64()? as f32,
        record["position"]["z"].as_f64()? as f32,
    ])
}
pub(crate) fn release_vm(vm: u64) {
    GUIDES.with(|g| {
        g.borrow_mut().remove(&vm);
    });
}
pub(crate) fn update(c: &mut Core) {
    GUIDES.with(|g|{let mut g=g.borrow_mut();let lost=g.iter().filter_map(|(vm,v)|(!c.view.in_world||v.entity.is_some_and(|id|entity_point(c,id,v.world).is_none())).then_some(*vm)).collect::<Vec<_>>();for vm in lost{let v=g.remove(&vm).unwrap();c.emit("guide.lost",json!({"owner":vm,"world_id":v.world,"entity_id":v.entity,"label":v.label,"reason":if c.view.in_world {"entity_left"}else{"world_left"}}));}});
}
pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    bind(
        lua,
        "map",
        "set_guide",
        lua.create_function(|lua, t: Table| {
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(
                    lua,
                    "eqoa.map.set_guide",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Some(world) = super::native_state::uint(&t.get::<Value>("world_id")?, 255) else {
                return error(
                    lua,
                    "eqoa.map.set_guide",
                    "invalid_argument",
                    "valid world_id required",
                );
            };
            let entity = match t.get::<Value>("entity_id")? {
                Value::Nil => None,
                v => match super::native_state::uint(&v, u32::MAX as u64)
                    .filter(|n| *n > 0 && *n < u32::MAX as u64)
                {
                    Some(id) => Some(id as u32),
                    None => {
                        return error(
                            lua,
                            "eqoa.map.set_guide",
                            "invalid_argument",
                            "valid entity_id required",
                        );
                    }
                },
            };
            let at = if entity.is_none() {
                match point(&t) { Ok(p)=>Some(p), Err(e)=>return error(lua,"eqoa.map.set_guide","invalid_argument",&e.to_string()) }
            } else {
                None
            };
            if let Some(id) = entity {
                if !CORE.with(|c| entity_point(&c.borrow(), id, world as i32).is_some()) {
                    return error(
                        lua,
                        "eqoa.map.set_guide",
                        "not_found",
                        "entity not observed in that world",
                    );
                }
            }
            let label = t.get::<Option<String>>("label")?;
            if label.as_ref().is_some_and(|s| s.len() > 128) {
                return error(
                    lua,
                    "eqoa.map.set_guide",
                    "out_of_range",
                    "label exceeds 128 bytes",
                );
            }
            let mut dots = Vec::new();
            if let Some(ts) = t.get::<Option<Table>>("dots")? {
                if ts.raw_len() > 80000 {
                    return error(
                        lua,
                        "eqoa.map.set_guide",
                        "quota",
                        "at most 80000 guide dots",
                    );
                }
                for p in ts.sequence_values::<Table>() {
                    match point(&p?) {Ok(p)=>dots.push(p),Err(e)=>return error(lua,"eqoa.map.set_guide","invalid_argument",&e.to_string())}
                }
            }
            let vm = super::native_events::bus(lua).borrow().vm;
            GUIDES.with(|g| {
                g.borrow_mut().insert(
                    vm,
                    Guide {
                        world: world as i32,
                        at,
                        entity,
                        dots,
                        label,
                    },
                )
            });
            lua.pack_multi((true, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "map",
        "clear_guide",
        lua.create_function(|lua, ()| {
            release_vm(super::native_events::bus(lua).borrow().vm);
            lua.pack_multi((true, Value::Nil))
        })?,
    )
}
fn quad(ui: &mut render::ui::Ui, white: usize, p: [[f32; 2]; 4], clip: [f32; 4]) {
    ui.quads.push(render::ui::Quad {
        tex: white,
        blend: 3,
        pos: p,
        uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        color: [0.9, 0.22, 0.19, 0.95],
        clip: Some(clip.map(|v| v as i32)),
    });
}
fn clipped_line(a: [f32; 2], b: [f32; 2], r: [f32; 4]) -> Option<([f32; 2], [f32; 2])> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d[0], a[0] - r[0]),
        (d[0], r[2] - a[0]),
        (-d[1], a[1] - r[1]),
        (d[1], r[3] - a[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let k = q / p;
            if p < 0.0 {
                lo = lo.max(k);
            } else {
                hi = hi.min(k);
            }
        }
    }
    if lo > hi {
        return None;
    }
    Some((
        [a[0] + d[0] * lo, a[1] + d[1] * lo],
        [a[0] + d[0] * hi, a[1] + d[1] * hi],
    ))
}
pub(crate) fn draw(
    ui: &mut render::ui::Ui,
    white: usize,
    world: i32,
    project: impl Fn([f32; 2]) -> [f32; 2],
    bounds: [f32; 4],
) {
    GUIDES.with(|guides| {
        CORE.with(|c| {
            let c = c.borrow();
            let guides = guides.borrow();
            for g in guides.values().filter(|g| g.world == world) {
                for &p in &g.dots {
                    let p = project(p);
                    if p[0] >= bounds[0]
                        && p[0] <= bounds[2]
                        && p[1] >= bounds[1]
                        && p[1] <= bounds[3]
                    {
                        quad(
                            ui,
                            white,
                            [
                                [p[0] - 1.5, p[1] - 1.5],
                                [p[0] + 1.5, p[1] - 1.5],
                                [p[0] + 1.5, p[1] + 1.5],
                                [p[0] - 1.5, p[1] + 1.5],
                            ],
                            bounds,
                        );
                    }
                }
                if c.view.world_id != Some(world) {
                    continue;
                }
                let target = g
                    .entity
                    .and_then(|id| entity_point(&c, id, g.world))
                    .or(g.at);
                let player = c.view.player["position"]["x"]
                    .as_f64()
                    .zip(c.view.player["position"]["z"].as_f64())
                    .map(|(x, z)| [x as f32, z as f32]);
                if let Some((a, b)) = player
                    .zip(target)
                    .and_then(|(a, b)| clipped_line(project(a), project(b), bounds))
                {
                    let d = [b[0] - a[0], b[1] - a[1]];
                    let len = d[0].hypot(d[1]);
                    if len < 0.01 {
                        continue;
                    }
                    let unit = [d[0] / len, d[1] / len];
                    let side = [-unit[1], unit[0]];
                    let count = ((len / 8.0).ceil() as usize).min(2048);
                    for k in 0..count {
                        let from = k as f32 * 8.0;
                        let to = (from + 4.0).min(len);
                        let a = [a[0] + unit[0] * from, a[1] + unit[1] * from];
                        let b = [a[0] + unit[0] * (to - from), a[1] + unit[1] * (to - from)];
                        quad(
                            ui,
                            white,
                            [
                                [a[0] - side[0], a[1] - side[1]],
                                [b[0] - side[0], b[1] - side[1]],
                                [b[0] + side[0], b[1] + side[1]],
                                [a[0] + side[0], a[1] + side[1]],
                            ],
                            bounds,
                        );
                    }
                }
            }
        })
    });
}
#[cfg(test)]
pub(crate) fn target_for(vm: u64) -> Option<[f32; 2]> {
    GUIDES.with(|g| {
        CORE.with(|c| {
            let g = g.borrow();
            let v = g.get(&vm)?;
            v.entity
                .and_then(|id| entity_point(&c.borrow(), id, v.world))
                .or(v.at)
        })
    })
}
