use super::{
    host::Host,
    native::{bind, bind_fn, error, record},
    native_core::{CORE, View},
};
use mlua::{AnyUserData, Lua, MetaMethod, Result, UserData, UserDataMethods, Value};
use serde_json::{Value as Json, json};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Ref {
    pub kind: String,
    pub key: String,
    pub generation: u64,
    pub incarnation: u64,
}
impl UserData for Ref {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Eq, |_, this, other: AnyUserData| {
            Ok(other.borrow::<Ref>().is_ok_and(|o| *this == *o))
        });
    }
}
pub(crate) fn make_ref(lua: &Lua, v: &Json) -> Result<Value> {
    Ok(Value::UserData(lua.create_userdata(Ref {
        kind: v["$ref"].as_str().unwrap_or("").into(),
        key: v["key"].as_str().unwrap_or("").into(),
        generation: v["generation"].as_u64().unwrap_or(0),
        incarnation: v["incarnation"].as_u64().unwrap_or(0),
    })?))
}
fn resolve(value: &Value, view: &View, generation: u64, kind: &str) -> Option<Json> {
    let ud = value.as_userdata()?;
    let r = ud.borrow::<Ref>().ok()?;
    if r.kind != kind || r.generation != generation {
        return None;
    }
    let list = if kind == "entity" {
        &view.entities
    } else {
        &view.abilities
    };
    list.iter()
        .find(|a| {
            a["ref"]["key"].as_str() == Some(r.key.as_str())
                && a["ref"]["incarnation"].as_u64() == Some(r.incarnation)
        })
        .cloned()
}
fn read(
    lua: &Lua,
    symbol: &str,
    path: &str,
    f: impl FnOnce(&View) -> Json,
) -> Result<mlua::MultiValue> {
    let value = CORE.with(|c| {
        let c = c.borrow();
        c.view.in_world.then(|| f(&c.view))
    });
    match value {
        Some(v) => lua.pack_multi((super::native::record_cached(lua, symbol, path, &v)?, Value::Nil)),
        None => error(lua, symbol, "not_in_world", "enter a loaded world first"),
    }
}
pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    bind(lua,"client","get",lua.create_function(|lua,()| CORE.with(|c| {let c=c.borrow();record(lua,"eqoa.client.Client",&json!({"in_world":c.view.in_world,"phase":c.view.phase,"generation":c.generation,"tick":c.tick,"frame":c.frame,"time":super::native_core::time_unborrowed(&c),"world_id":c.view.world_id}))}))?)?;
    for (module, name, path) in [
        ("player", "get", "eqoa.player.Player"),
        ("entities", "list", "eqoa.entities.Entity"),
        ("target", "get", "eqoa.entities.Entity"),
        ("group", "get", "eqoa.group.Group"),
        ("abilities", "list", "eqoa.abilities.Ability"),
        ("hotbar", "list", "eqoa.hotbar.Hotbar"),
    ] {
        let symbol = format!("eqoa.{module}.{name}");
        bind(
            lua,
            module,
            name,
            lua.create_function(move |lua, ()| {
                read(lua, &symbol, path, |v| match module {
                    "player" => v.player.clone(),
                    "entities" => json!(v.entities),
                    "group" => v.group.clone(),
                    "abilities" => json!(v.abilities),
                    "hotbar" => json!(v.hotbar),
                    _ => v
                        .entities
                        .iter()
                        .find(|e| e["id"] == v.player["target_id"])
                        .cloned()
                        .unwrap_or(Json::Null),
                })
            })?,
        )?;
    }
    for (module, kind) in [
        ("entities", "entity"),
        ("abilities", "ability"),
        ("cooldowns", "ability"),
    ] {
        let symbol = format!("eqoa.{module}.get");
        bind(lua,module,"get",lua.create_function(move |lua,r:Value| {
            let (world,value)=CORE.with(|c| {let c=c.borrow();(c.view.in_world,resolve(&r,&c.view,c.generation,kind))});
            if !world {return error(lua,&symbol,"not_in_world","enter a loaded world first");}
            let Some(value)=value else {return error(lua,&symbol,"stale_handle","record no longer exists in this world");};
            if module=="cooldowns" {let lockout=value["lockout"].as_f64().unwrap_or(0.0);return lua.pack_multi((record(lua,"eqoa.cooldowns.Cooldown",&json!({"active":lockout>0.0,"lockout":lockout,"started_at":CORE.with(|c|c.borrow().cooldown_edges.get(value["id"].as_str().unwrap_or_default()).map(|e|e.0)),"estimated_remaining":CORE.with(|c|{let c=c.borrow();c.cooldown_edges.get(value["id"].as_str().unwrap_or_default()).map(|e|(e.2-(super::native_core::time_unborrowed(&c)-e.1)).max(0.0))})}))?,Value::Nil));}
            lua.pack_multi((record(lua,if kind=="entity" {"eqoa.entities.Entity"}else{"eqoa.abilities.Ability"},&value)?,Value::Nil))
        })?)?;
    }
    bind(
        lua,
        "entities",
        "find",
        lua.create_function(|lua, id: Value| {
            let Some(id) = uint(&id, u32::MAX as u64) else {
                return error(
                    lua,
                    "eqoa.entities.find",
                    "invalid_argument",
                    "id must be a u32 integer",
                );
            };
            read(lua, "eqoa.entities.find", "eqoa.entities.Entity", |v| {
                v.entities
                    .iter()
                    .find(|e| e["id"].as_u64() == Some(id))
                    .cloned()
                    .unwrap_or(Json::Null)
            })
        })?,
    )?;
    bind(
        lua,
        "entities",
        "named",
        lua.create_function(|lua, (name, exact): (Value, Value)| {
            let Some(name) = name
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.entities.named",
                    "invalid_argument",
                    "name must be a string",
                );
            };
            let exact = match exact {
                Value::Nil => true,
                Value::Boolean(exact) => exact,
                _ => {
                    return error(
                        lua,
                        "eqoa.entities.named",
                        "invalid_argument",
                        "exact must be a boolean",
                    );
                }
            };
            let name = name.to_lowercase();
            read(lua, "eqoa.entities.named", "eqoa.entities.Entity", |v| {
                json!(
                    v.entities
                        .iter()
                        .filter(|e| e["name"].as_str().is_some_and(|candidate| {
                            let candidate = candidate.to_lowercase();
                            if exact {
                                candidate == name
                            } else {
                                candidate.contains(&name)
                            }
                        }))
                        .collect::<Vec<_>>()
                )
            })
        })?,
    )?;
    bind(
        lua,
        "abilities",
        "find",
        lua.create_function(|lua, id: Value| {
            let id = id
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()));
            let Some(id) = id.filter(|s| s.parse::<i64>().is_ok_and(|id| id.to_string() == *s))
            else {
                return error(
                    lua,
                    "eqoa.abilities.find",
                    "invalid_argument",
                    "id must be a canonical decimal i64 string",
                );
            };
            read(lua, "eqoa.abilities.find", "eqoa.abilities.Ability", |v| {
                v.abilities
                    .iter()
                    .find(|a| a["id"].as_str() == Some(&id))
                    .cloned()
                    .unwrap_or(Json::Null)
            })
        })?,
    )?;
    bind(
        lua,
        "effects",
        "list",
        lua.create_function(|lua, ()| {
            let (world, effects) = CORE.with(|c| {
                let c = c.borrow();
                (c.view.in_world, c.view.effects.clone())
            });
            if !world {
                return error(
                    lua,
                    "eqoa.effects.list",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            match effects {
                Some(e) => {
                    lua.pack_multi((record(lua, "eqoa.effects.Effect", &json!(e))?, Value::Nil))
                }
                None => error(
                    lua,
                    "eqoa.effects.list",
                    "unavailable",
                    "waiting for effect state channel 0x43",
                ),
            }
        })?,
    )?;
    bind(
        lua,
        "chat",
        "get_channel",
        lua.create_function(|lua, ()| {
            read(lua, "eqoa.chat.get_channel", "", |v| json!(v.channel))
        })?,
    )?;
    bind(
        lua,
        "chat",
        "channel_name",
        lua.create_function(|_, channel: String| {
            Ok(match channel.as_str() {
                "say" => "Say",
                "group" => "Party",
                "guild" => "Guild",
                "shout" => "Shout",
                "tell" => "Tell",
                "system" => "System",
                _ => "Unknown Message Type",
            })
        })?,
    )?;
    bind(
        lua,
        "chat",
        "history",
        lua.create_function(|lua, (limit, before): (Option<Value>, Option<Value>)| {
            let Some(limit) = limit.as_ref().map_or(Some(100), |v| uint(v, 1000)) else {
                return error(
                    lua,
                    "eqoa.chat.history",
                    "invalid_argument",
                    "limit must be an integer in 0..1000",
                );
            };
            let before = match before {
                Some(v) => match uint(&v, 9_007_199_254_740_991) {
                    Some(n) => n,
                    None => {
                        return error(
                            lua,
                            "eqoa.chat.history",
                            "invalid_argument",
                            "before_seq must be a nonnegative integer",
                        );
                    }
                },
                None => u64::MAX,
            };
            let (world, messages) = CORE.with(|c| {
                let c = c.borrow();
                let mut records = c
                    .chat
                    .iter()
                    .rev()
                    .filter(|r| r["seq"].as_u64().is_some_and(|n| n < before))
                    .take(limit as usize)
                    .cloned()
                    .collect::<Vec<_>>();
                records.reverse();
                (c.view.in_world, records)
            });
            if !world {
                return error(
                    lua,
                    "eqoa.chat.history",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            lua.pack_multi((
                record(lua, "eqoa.chat.ChatMessage", &json!(messages))?,
                Value::Nil,
            ))
        })?,
    )?;
    bind(
        lua,
        "player",
        "experience_for_level",
        lua.create_function(|lua, level: Value| {
            let Some(level) = uint(&level, u32::MAX as u64) else {
                return error(
                    lua,
                    "eqoa.player.experience_for_level",
                    "invalid_argument",
                    "level must be an unsigned integer",
                );
            };
            lua.pack_multi(if (1..=59).contains(&level) {
                Some(crate::hud::exp_for_level(level as i32))
            } else {
                None
            })
        })?,
    )?;
    install_math(lua)?;
    bind(
        lua,
        "text",
        "localized",
        lua.create_function(|lua, id: Value| {
            let Some(id) = number(&id)
                .filter(|n| n.fract() == 0.0 && *n >= i32::MIN as f64 && *n <= i32::MAX as f64)
            else {
                return error(
                    lua,
                    "eqoa.text.localized",
                    "invalid_argument",
                    "word id must be an int",
                );
            };
            let value = CORE.with(|c| {
                let c = c.borrow();
                c.words.as_ref().map(|w| {
                    let value = w.get(id as i32, 0);
                    (value != "(null)").then(|| value.to_string())
                })
            });
            match value {
                Some(value) => lua.pack_multi((value, Value::Nil)),
                None => error(
                    lua,
                    "eqoa.text.localized",
                    "unavailable",
                    "localized words have not loaded",
                ),
            }
        })?,
    )?;
    bind(
        lua,
        "input",
        "get",
        lua.create_function(|lua, ()| super::input::native_get(lua))?,
    )?;
    super::input::native::install(host)?;
    Ok(())
}
pub(crate) fn uint(v: &Value, max: u64) -> Option<u64> {
    let n = match v {
        Value::Integer(n) => *n as f64,
        Value::Number(n) => *n,
        _ => return None,
    };
    (n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= max as f64).then_some(n as u64)
}
pub(crate) fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(n) => Some(*n as f64),
        Value::Number(n) if n.is_finite() => Some(*n),
        _ => None,
    }
}
fn vec3(v: &Value) -> Option<[f32; 3]> {
    let t = v.as_table()?;
    Some([
        number(&t.get::<Value>("x").ok()?)? as f32,
        number(&t.get::<Value>("y").ok()?)? as f32,
        number(&t.get::<Value>("z").ok()?)? as f32,
    ])
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [
        ee::sub(b[0], a[0]),
        ee::sub(b[1], a[1]),
        ee::sub(b[2], a[2]),
    ];
    ee::sqrt(ee::add(
        ee::add(ee::mul(d[0], d[0]), ee::mul(d[1], d[1])),
        ee::mul(d[2], d[2]),
    ))
}
// Release the session borrow before the caller may emit a diagnostic.
fn queue_hud_command(
    lua: &Lua,
    cmd: super::hud_layer::HudCommand,
) -> std::result::Result<(), &'static str> {
    let session = super::native::session(lua);
    let session = session.borrow();
    let addon = if session.addon.is_empty() {
        "default"
    } else {
        session.addon.as_str()
    };
    super::hud_layer::queue_command(addon, cmd)
}

fn install_math(lua: &Lua) -> Result<()> {
    bind(
        lua,
        "entities",
        "distance",
        lua.create_function(|lua, (a, b): (Value, Value)| match (vec3(&a), vec3(&b)) {
            (Some(a), Some(b)) => lua.pack_multi((distance(a, b), Value::Nil)),
            _ => error(
                lua,
                "eqoa.entities.distance",
                "invalid_argument",
                "expected finite Vec3 values",
            ),
        })?,
    )?;
    bind(
        lua,
        "math",
        "facing",
        lua.create_function(|lua, (p, h, d, t): (Value, Value, Value, Value)| {
            let Some(((p, d), (h, t))) = vec3(&p).zip(vec3(&d)).zip(number(&h).zip(number(&t)))
            else {
                return error(
                    lua,
                    "eqoa.math.facing",
                    "invalid_argument",
                    "expected positions and finite angles",
                );
            };
            if !(0.0..=std::f64::consts::PI).contains(&t) {
                return error(
                    lua,
                    "eqoa.math.facing",
                    "out_of_range",
                    "tolerance must be in 0..pi",
                );
            }
            let dx = ee::sub(d[0], p[0]);
            let dz = ee::sub(d[2], p[2]);
            let bearing = ee::libm::atan2f(-dx, -dz) as f64;
            let difference = (bearing - h + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            lua.pack_multi((
                (dx == 0.0 && dz == 0.0) || difference.abs() <= t,
                Value::Nil,
            ))
        })?,
    )?;
    for name in ["degrees", "radians", "normalize"] {
        bind(
            lua,
            "math",
            name,
            lua.create_function(move |lua, v: Value| {
                let Some(n) = number(&v) else {
                    return error(
                        lua,
                        &format!("eqoa.math.{name}"),
                        "invalid_argument",
                        "expected finite number",
                    );
                };
                let n = match name {
                    "degrees" => n.to_degrees(),
                    "radians" => n.to_radians(),
                    _ => n.rem_euclid(std::f64::consts::TAU),
                };
                lua.pack_multi((n, Value::Nil))
            })?,
        )?;
    }
    bind(
        lua,
        "math",
        "is_bit_zero",
        lua.create_function(|lua, (v, m): (Value, Value)| {
            match (uint(&v, u32::MAX as u64), uint(&m, u32::MAX as u64)) {
                (Some(v), Some(m)) => lua.pack_multi((v & m == 0, Value::Nil)),
                _ => error(
                    lua,
                    "eqoa.math.is_bit_zero",
                    "invalid_argument",
                    "expected u32 value and mask",
                ),
            }
        })?,
    )?;
    bind(
        lua,
        "math",
        "signed_angle",
        lua.create_function(
            |lua, (a, b): (Value, Value)| match (number(&a), number(&b)) {
                (Some(a), Some(b)) => lua.pack_multi((
                    ((b - a) + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                        - std::f64::consts::PI,
                    Value::Nil,
                )),
                _ => error(
                    lua,
                    "eqoa.math.signed_angle",
                    "invalid_argument",
                    "expected finite angles",
                ),
            },
        )?,
    )?;
    bind(
        lua,
        "math",
        "bearing",
        lua.create_function(|lua, (a, b): (Value, Value)| match (vec3(&a), vec3(&b)) {
            (Some(a), Some(b)) => {
                let dx = ee::sub(b[0], a[0]);
                let dz = ee::sub(b[2], a[2]);
                lua.pack_multi((
                    if dx == 0.0 && dz == 0.0 {
                        0.0
                    } else {
                        ee::libm::atan2f(-dx, -dz).rem_euclid(std::f32::consts::TAU)
                    },
                    Value::Nil,
                ))
            }
            _ => error(
                lua,
                "eqoa.math.bearing",
                "invalid_argument",
                "expected finite Vec3 values",
            ),
        })?,
    )?;
    bind(lua,"abilities","in_range",lua.create_function(|lua,(r,a,b):(Value,Value,Value)| {
        let Some((a,b))=vec3(&a).zip(vec3(&b)) else {return error(lua,"eqoa.abilities.in_range","invalid_argument","expected finite Vec3 values");};
        let ability=CORE.with(|c| {let c=c.borrow();resolve(&r,&c.view,c.generation,"ability")});
        let Some(ability)=ability else {return error(lua,"eqoa.abilities.in_range","stale_handle","ability is no longer present");};
        let d=distance(a,b);let range=ability["range"].as_f64().unwrap_or(0.0) as f32;
        lua.pack_multi((record(lua,"eqoa.abilities.RangeResult",&json!({"in_range":range==0.0 || ability["scope"]==0 || ee::le(d,range),"distance":d}))?,Value::Nil))
    })?)?;
    bind(
        lua,
        "player",
        "stats",
        lua.create_function(|lua, ()| {
            read(lua, "eqoa.player.stats", "eqoa.player.Stats", |v| {
                v.player_stats.clone().unwrap_or(Json::Null)
            })
        })?,
    )?;
    for action_name in ["assist", "target_target"] {
        let sym = format!("eqoa.actions.{action_name}");
        bind(
            lua,
            "actions",
            action_name,
            lua.create_function(move |lua, ()| {
                let in_world = CORE.with(|c| c.borrow().view.in_world);
                if !in_world {
                    return error(lua, &sym, "not_in_world", "enter a loaded world first");
                }
                if !super::actions::queue_assist() {
                    return error(lua, &sym, "busy", "assist already queued this tick");
                }
                lua.pack_multi((true, Value::Nil))
            })?,
        )?;
    }
    bind(
        lua,
        "windows",
        "open",
        lua.create_function(|lua, name: Value| {
            let Some(name) = name
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.windows.open",
                    "invalid_argument",
                    "window name must be a string",
                );
            };
            if name != "summary" {
                return error(
                    lua,
                    "eqoa.windows.open",
                    "invalid_argument",
                    "unsupported window name",
                );
            }
            let in_world = CORE.with(|c| c.borrow().view.in_world);
            if !in_world {
                return error(
                    lua,
                    "eqoa.windows.open",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            if !super::actions::queue_open_summary() {
                return error(
                    lua,
                    "eqoa.windows.open",
                    "busy",
                    "summary open already queued this tick",
                );
            }
            lua.pack_multi((true, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "hud",
        "frame",
        lua.create_function(|lua, arg: Option<Value>| {
            let (w, h) = match arg {
                Some(Value::Table(t)) => {
                    let w = t.get::<Option<u32>>("width")?.unwrap_or(640);
                    let h = t.get::<Option<u32>>("height")?.unwrap_or(448);
                    (w, h)
                }
                _ => super::hud_layout::get_window_size(),
            };
            let j = super::hud_layout::hud_frame_json((w, h));
            lua.pack_multi((super::native::record_cached(lua, "eqoa.hud.frame", "eqoa.hud.Frame", &j)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "hud",
        "layout",
        lua.create_function(|lua, arg: Option<Value>| {
            let (w, h) = match arg {
                Some(Value::Table(t)) => {
                    let w = t.get::<Option<u32>>("width")?.unwrap_or(640);
                    let h = t.get::<Option<u32>>("height")?.unwrap_or(448);
                    (w, h)
                }
                _ => super::hud_layout::get_window_size(),
            };
            let j = super::hud_layout::hud_layout_json((w, h));
            lua.pack_multi((super::native::record_cached(lua, "eqoa.hud.layout", "eqoa.hud.Layout", &j)?, Value::Nil))
        })?,
    )?;
    bind_fn(
        lua,
        "hud",
        "visible",
        |lua, element: Option<String>| -> Result<mlua::MultiValue> {
            let in_world = CORE.with(|c| c.borrow().view.in_world);
            if !in_world {
                return lua.pack_multi((false, Value::Nil));
            }
            let vis = match element {
                Some(name) => super::hud_layout::hud_element_visible(&name),
                None => super::hud_layout::hud_element_visible("hud"),
            };
            lua.pack_multi((vis, Value::Nil))
        },
    )?;
    let parse_anchor_vert =
        |t: &Spec| -> Result<(render::ui::Anchor, render::ui::Vertical)> {
            let anchor_s: Option<mlua::String> = t.fget("anchor")?;
            let anchor = match anchor_s.as_ref().map(|s| s.as_bytes()).as_deref() {
                Some(b"right") => render::ui::Anchor::Right,
                Some(b"center") => render::ui::Anchor::Middle,
                _ => render::ui::Anchor::Left,
            };
            let vert_s: Option<mlua::String> = t.fget("vertical")?;
            let vertical = match vert_s.as_ref().map(|s| s.as_bytes()).as_deref() {
                Some(b"bottom") => render::ui::Vertical::Bottom,
                Some(b"center") => render::ui::Vertical::Middle,
                _ => render::ui::Vertical::Top,
            };
            Ok((anchor, vertical))
        };
    bind_fn(
        lua,
        "hud",
        "text",
        move |lua, spec: Value| -> Result<mlua::MultiValue> {
            let in_world = CORE.with(|c| c.borrow().view.in_world);
            if !in_world {
                return error(
                    lua,
                    "eqoa.hud.text",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Value::Table(t) = spec else {
                return error(
                    lua,
                    "eqoa.hud.text",
                    "invalid_argument",
                    "spec must be a table",
                );
            };
            let t = Spec::new(lua, &t)?;
            let col: Option<usize> = t.fget("col")?;
            let row: Option<usize> = t.fget("row")?;
            let has_cell = col
                .zip(row)
                .is_some_and(|(c, r)| (1..=2).contains(&c) && (1..=8).contains(&r));
            let x: f32 = match t.fget::<Option<f32>>("x")? {
                Some(v) => v,
                None if has_cell => 0.0,
                None => return error(lua, "eqoa.hud.text", "invalid_argument", "x required"),
            };
            let y: f32 = match t.fget::<Option<f32>>("y")? {
                Some(v) => v,
                None if has_cell => 0.0,
                None => return error(lua, "eqoa.hud.text", "invalid_argument", "y required"),
            };
            let text: mlua::String = match t.fget::<mlua::String>("text") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.text", "invalid_argument", "text required");
                }
            };
            let color = match t.fget::<Option<mlua::Table>>("color")? {
                Some(c) if c.raw_len() >= 4 => {
                    [c.raw_get(1)?, c.raw_get(2)?, c.raw_get(3)?, c.raw_get(4)?]
                }
                Some(c) if c.raw_len() == 3 => [c.raw_get(1)?, c.raw_get(2)?, c.raw_get(3)?, 1.0],
                _ => [1.0, 1.0, 1.0, 1.0],
            };
            let scale: f32 = t.fget::<Option<f32>>("scale")?.unwrap_or(1.0);
            let align_str: Option<mlua::String> = t.fget("align")?;
            let align = match align_str.as_ref().map(|s| s.as_bytes()).as_deref() {
                Some(b"center") => super::hud_layer::TextAlign::Center,
                Some(b"right") => super::hud_layer::TextAlign::Right,
                _ => super::hud_layer::TextAlign::Left,
            };
            let outline: bool = t.fget::<Option<bool>>("outline")?.unwrap_or(false);
            let shadow: bool = t.fget::<Option<bool>>("shadow")?.unwrap_or(false);
            let (anchor, vertical) = parse_anchor_vert(&t)?;
            let clip_width: Option<i32> = t.fget("clip_width")?;
            let stat_cell = col
                .zip(row)
                .filter(|&(c, r)| (1..=2).contains(&c) && (1..=8).contains(&r))
                .map(|(c, r)| (c - 1, r - 1, align == super::hud_layer::TextAlign::Right));

            let cmd = super::hud_layer::HudCommand::Text {
                x,
                y,
                text: super::hud_layer::text_buffer(&text.to_str()?),
                target_fade: t.fget::<Option<bool>>("target_fade")?.unwrap_or(false),
                color,
                scale,
                align,
                outline,
                shadow,
                anchor,
                vertical,
                clip_width,
                stat_cell,
            };
            if let Err(e) = queue_hud_command(lua, cmd) {
                return error(lua, "eqoa.hud.text", "limit_exceeded", e);
            }
            lua.pack_multi((true, Value::Nil))
        },
    )?;
    bind_fn(
        lua,
        "hud",
        "gauge",
        move |lua, spec: Value| -> Result<mlua::MultiValue> {
            let in_world = CORE.with(|c| c.borrow().view.in_world);
            if !in_world {
                return error(
                    lua,
                    "eqoa.hud.gauge",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Value::Table(t) = spec else {
                return error(
                    lua,
                    "eqoa.hud.gauge",
                    "invalid_argument",
                    "spec must be a table",
                );
            };
            let t = Spec::new(lua, &t)?;
            let x: i32 = match t.fget::<i32>("x") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.gauge", "invalid_argument", "x required");
                }
            };
            let y: i32 = match t.fget::<i32>("y") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.gauge", "invalid_argument", "y required");
                }
            };
            let art: i32 = match t.fget::<i32>("art") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.gauge", "invalid_argument", "art required");
                }
            };
            let fill: f32 = match t.fget::<f32>("fill") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.gauge", "invalid_argument", "fill required");
                }
            };
            let lit: bool = t.fget::<Option<bool>>("lit")?.unwrap_or(true);
            let (anchor, vertical) = parse_anchor_vert(&t)?;

            let cmd = super::hud_layer::HudCommand::Gauge {
                x,
                y,
                art,
                fill,
                lit,
                anchor,
                vertical,
            };
            if let Err(e) = queue_hud_command(lua, cmd) {
                return error(lua, "eqoa.hud.gauge", "limit_exceeded", e);
            }
            lua.pack_multi((true, Value::Nil))
        },
    )?;
    bind_fn(
        lua,
        "hud",
        "image",
        move |lua, spec: Value| -> Result<mlua::MultiValue> {
            let in_world = CORE.with(|c| c.borrow().view.in_world);
            if !in_world {
                return error(
                    lua,
                    "eqoa.hud.image",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Value::Table(t) = spec else {
                return error(
                    lua,
                    "eqoa.hud.image",
                    "invalid_argument",
                    "spec must be a table",
                );
            };
            let t = Spec::new(lua, &t)?;
            let x: i32 = match t.fget::<i32>("x") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.image", "invalid_argument", "x required");
                }
            };
            let y: i32 = match t.fget::<i32>("y") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.image", "invalid_argument", "y required");
                }
            };
            let w: Option<i32> = t.fget("w")?;
            let h: Option<i32> = t.fget("h")?;
            let color = match t.fget::<Option<mlua::Table>>("color")? {
                Some(c) if c.raw_len() >= 4 => {
                    Some([c.raw_get(1)?, c.raw_get(2)?, c.raw_get(3)?, c.raw_get(4)?])
                }
                Some(c) if c.raw_len() == 3 => {
                    Some([c.raw_get(1)?, c.raw_get(2)?, c.raw_get(3)?, 1.0])
                }
                _ => None,
            };
            let (anchor, vertical) = parse_anchor_vert(&t)?;

            let cmd = match t.fget::<Value>("ui_id")? {
                Value::String(s) if s.to_str()? == "panel" => super::hud_layer::HudCommand::Panel {
                    x,
                    y,
                    w: w.unwrap_or(148),
                    h: h.unwrap_or(132),
                    anchor,
                    vertical,
                },
                Value::Integer(id) => super::hud_layer::HudCommand::Image {
                    ui_id: id as i32,
                    target_fade: t.fget::<Option<bool>>("target_fade")?.unwrap_or(false),
                    x,
                    y,
                    w,
                    h,
                    color,
                    anchor,
                    vertical,
                },
                Value::Number(n) => super::hud_layer::HudCommand::Image {
                    ui_id: n as i32,
                    target_fade: t.fget::<Option<bool>>("target_fade")?.unwrap_or(false),
                    x,
                    y,
                    w,
                    h,
                    color,
                    anchor,
                    vertical,
                },
                _ => return error(lua, "eqoa.hud.image", "invalid_argument", "ui_id required"),
            };

            if let Err(e) = queue_hud_command(lua, cmd) {
                return error(lua, "eqoa.hud.image", "limit_exceeded", e);
            }
            lua.pack_multi((true, Value::Nil))
        },
    )?;
    bind_fn(
        lua,
        "hud",
        "panel",
        move |lua, spec: Value| -> Result<mlua::MultiValue> {
            let in_world = CORE.with(|c| c.borrow().view.in_world);
            if !in_world {
                return error(
                    lua,
                    "eqoa.hud.panel",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Value::Table(t) = spec else {
                return error(
                    lua,
                    "eqoa.hud.panel",
                    "invalid_argument",
                    "spec must be a table",
                );
            };
            let t = Spec::new(lua, &t)?;
            let x: i32 = match t.fget::<i32>("x") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.panel", "invalid_argument", "x required");
                }
            };
            let y: i32 = match t.fget::<i32>("y") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.panel", "invalid_argument", "y required");
                }
            };
            let w: i32 = match t.fget::<i32>("w") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.panel", "invalid_argument", "w required");
                }
            };
            let h: i32 = match t.fget::<i32>("h") {
                Ok(v) => v,
                Err(_) => {
                    return error(lua, "eqoa.hud.panel", "invalid_argument", "h required");
                }
            };
            let (anchor, vertical) = parse_anchor_vert(&t)?;

            let cmd = super::hud_layer::HudCommand::Panel {
                x,
                y,
                w,
                h,
                anchor,
                vertical,
            };
            if let Err(e) = queue_hud_command(lua, cmd) {
                return error(lua, "eqoa.hud.panel", "limit_exceeded", e);
            }
            lua.pack_multi((true, Value::Nil))
        },
    )?;
    bind(
        lua,
        "actions",
        "toggle_stats_box",
        lua.create_function(|lua, ()| {
            let addon = super::native::session(lua).borrow().addon.clone();
            if addon != "eqoa_stats_box" {
                return error(lua, "eqoa.actions.toggle_stats_box", "permission_denied",
                    "this compatibility action belongs to eqoa_stats_box; use eqoa.options.set for your own toggles");
            }
            let cur = super::options::get_toggle(&addon, "stats_box").unwrap_or(false);
            let next = !cur;
            if let Err(message) = super::options::set_toggle_persisted(&addon, "stats_box", next) {
                return error(lua, "eqoa.actions.toggle_stats_box", "io_error", &message);
            }
            crate::say(stats_box_message(next));
            lua.pack_multi((next, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "options",
        "toggle",
        lua.create_function(|lua, spec: Value| {
            let Value::Table(t) = spec else {
                return error(
                    lua,
                    "eqoa.options.toggle",
                    "invalid_argument",
                    "spec must be a table",
                );
            };
            let t = Spec::new(lua, &t)?;
            let id: String = match t.fget("id") {
                Ok(s) => s,
                Err(_) => {
                    return error(
                        lua,
                        "eqoa.options.toggle",
                        "invalid_argument",
                        "id is required",
                    );
                }
            };
            let label: String = t.fget("label").unwrap_or_else(|_| id.clone());
            let default: bool = t.fget("default").unwrap_or(false);
            let addon = super::native::session(lua).borrow().addon.clone();
            super::options::attach_diagnostics(&addon, super::native::diagnostic_sink(lua));
            let (val, item_id) = super::options::register_toggle(&addon, &id, &label, default);
            let j = json!({
                "id": id,
                "label": label,
                "default": default,
                "value": val,
                "item_id": item_id,
            });
            lua.pack_multi((record(lua, "eqoa.options.Toggle", &j)?, Value::Nil))
        })?,
    )?;
    bind_fn(
        lua,
        "options",
        "get",
        |lua, (id, owner): (String, Option<String>)| -> Result<mlua::MultiValue> {
            let addon = super::native::session(lua).borrow().addon.clone();
            let val = if let Some(owner) = owner {
                super::options::get_toggle(&owner, &id)
                    .map(|value| value && super::options::is_addon_enabled(&owner))
            } else {
                super::options::get_toggle(&addon, &id)
            };
            lua.pack_multi((val, Value::Nil))
        },
    )?;
    bind(
        lua,
        "options",
        "set",
        lua.create_function(|lua, (id, val): (String, bool)| {
            let addon = super::native::session(lua).borrow().addon.clone();
            super::options::attach_diagnostics(&addon, super::native::diagnostic_sink(lua));
            match super::options::set_toggle_persisted(&addon, &id, val) {
                Ok(()) => lua.pack_multi((true, Value::Nil)),
                Err(message) => error(lua, "eqoa.options.set", "io_error", &message),
            }
        })?,
    )?;
    bind(
        lua,
        "binds",
        "register",
        lua.create_function(
            |lua, (id, default_key, description): (String, String, String)| {
                let addon = super::native::session(lua).borrow().addon.clone();
                let ok =
                    crate::fork_ext::bindpage::register(&addon, &id, &default_key, &description);
                lua.pack_multi((ok, Value::Nil))
            },
        )?,
    )?;
    bind(
        lua,
        "binds",
        "get",
        lua.create_function(|lua, id: String| {
            let addon = super::native::session(lua).borrow().addon.clone();
            let key = crate::fork_ext::bindpage::get_key(&addon, &id);
            lua.pack_multi((key, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "input",
        "bind",
        lua.create_function(|lua, (spec, callback): (Value, Option<Value>)| {
            let Value::Table(t) = spec else {
                return error(
                    lua,
                    "eqoa.input.bind",
                    "invalid_argument",
                    "spec must be a table",
                );
            };
            let id: String = match t.get("id") {
                Ok(s) => s,
                Err(_) => {
                    return error(lua, "eqoa.input.bind", "invalid_argument", "id is required");
                }
            };
            let default_key: String = t.get("default_key").unwrap_or_else(|_| "b".into());
            let label: String = t.get("label").unwrap_or_else(|_| id.clone());
            let cb_fn = match callback.or_else(|| t.get::<Option<Value>>("callback").ok().flatten())
            {
                Some(Value::Function(f)) => Some(f),
                Some(Value::Nil) | None => None,
                Some(_) => {
                    return error(
                        lua,
                        "eqoa.input.bind",
                        "invalid_argument",
                        "callback must be a function",
                    );
                }
            };
            let addon = super::native::session(lua).borrow().addon.clone();
            let ok = crate::fork_ext::bindpage::register(&addon, &id, &default_key, &label);
            if let Some(cb) = cb_fn {
                let id_for_cb = id.clone();
                let addon_for_cb = addon.clone();
                let wrapper = lua.create_function(move |_, ev: Value| {
                    if let Value::Table(et) = &ev {
                        let data: mlua::Table = et.get("data")?;
                        let down = data.get::<Option<bool>>("down")?.unwrap_or(false);
                        let rep = data.get::<Option<bool>>("repeat")?.unwrap_or(false)
                            || data.get::<Option<bool>>("repeat_")?.unwrap_or(false);
                        let key = data.get::<Option<String>>("key")?.unwrap_or_default();
                        if down && !rep {
                            let bound =
                                crate::fork_ext::bindpage::get_key(&addon_for_cb, &id_for_cb);
                            if bound.is_some_and(|bound| {
                                !bound.is_empty() && key.eq_ignore_ascii_case(&bound)
                            }) {
                                cb.call::<()>(ev)?;
                            }
                        }
                    }
                    Ok(())
                })?;
                let eqoa_tbl: mlua::Table = lua.globals().get("eqoa")?;
                let events_tbl: mlua::Table = eqoa_tbl.get("events")?;
                let on_fn: mlua::Function = events_tbl.get("on")?;
                let _ = on_fn.call::<mlua::MultiValue>(("input.key", wrapper))?;
            }
            lua.pack_multi((ok, Value::Nil))
        })?,
    )?;
    Ok(())
}

/// The native Stats Box toggle's local diagnostic wording.
pub(crate) fn stats_box_message(shown: bool) -> &'static str {
    if shown {
        "stats box shown"
    } else {
        "stats box hidden"
    }
}

/// Spec-table fields the HUD and option calls read.
const SPEC_KEYS: [&str; 23] = [
    "col", "row", "x", "y", "text", "color", "scale", "align", "outline", "shadow", "anchor",
    "vertical", "clip_width", "target_fade", "art", "fill", "lit", "w", "h", "ui_id", "id", "label",
    "default",
];

/// One pass over a spec table instead of a protected lookup per field: drawing
/// calls read ~13 optional fields, and absent-field lookups dominated their cost.
/// A table with a metatable keeps full `get` semantics (defaults via __index).
struct Spec {
    lua: Lua,
    slots: [Value; SPEC_KEYS.len()],
}

impl Spec {
    fn new(lua: &Lua, t: &mlua::Table) -> Result<Self> {
        let mut slots: [Value; SPEC_KEYS.len()] = std::array::from_fn(|_| Value::Nil);
        if t.metatable().is_some() {
            for (slot, key) in slots.iter_mut().zip(SPEC_KEYS) {
                *slot = t.get(key)?;
            }
        } else {
            t.for_each(|key: Value, value: Value| {
                if let Value::String(key) = key {
                    let bytes = key.as_bytes();
                    if let Some(i) = SPEC_KEYS.iter().position(|k| k.as_bytes() == &*bytes) {
                        slots[i] = value;
                    }
                }
                Ok(())
            })?;
        }
        Ok(Self {
            lua: lua.clone(),
            slots,
        })
    }

    fn fget<V: mlua::FromLua>(&self, key: &str) -> Result<V> {
        let i = SPEC_KEYS
            .iter()
            .position(|k| *k == key)
            .expect("spec key is listed in SPEC_KEYS");
        V::from_lua(self.slots[i].clone(), &self.lua)
    }
}
