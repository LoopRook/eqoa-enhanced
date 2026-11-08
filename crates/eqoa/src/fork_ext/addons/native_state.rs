use super::{
    host::Host,
    native::{bind, error, record},
    native_core::{CORE, View},
};
use mlua::{AnyUserData, Lua, MetaMethod, Result, Table, UserData, UserDataMethods, Value};
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
        Some(v) => lua.pack_multi((record(lua, path, &v)?, Value::Nil)),
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
            if module=="cooldowns" {let lockout=value["lockout"].as_f64().unwrap_or(0.0);return lua.pack_multi((record(lua,"eqoa.cooldowns.Cooldown",&json!({"active":lockout>0.0,"lockout":lockout,"started_at":null,"estimated_remaining":null}))?,Value::Nil));}
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
        lua.create_function(|lua, name: Value| {
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
            read(lua, "eqoa.entities.named", "eqoa.entities.Entity", |v| {
                json!(
                    v.entities
                        .iter()
                        .filter(|e| e["name"].as_str() == Some(&name))
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
                Some(crate::hud::exp_for_level(level as i32 + 1))
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
    Ok(())
}
