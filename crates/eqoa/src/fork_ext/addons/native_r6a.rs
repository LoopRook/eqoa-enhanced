// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 6 Part A native Lua bindings (`eqoa.quests.*`, `eqoa.mastery.*`, and later camera/windows).
use super::{
    host::Host,
    native::{bind, describe, error, record},
    native_core::{CORE, View},
    native_state::Ref,
};
use mlua::{Lua, Result, Value};
use serde_json::{Value as Json, json};

pub(crate) fn resolve_in(value: &Value, list: &[Json], generation: u64, kind: &str) -> Option<Json> {
    let ud = value.as_userdata()?;
    let r = ud.borrow::<Ref>().ok()?;
    if r.kind != kind || r.generation != generation {
        return None;
    }
    list.iter()
        .find(|entry| {
            entry["ref"]["key"].as_str() == Some(r.key.as_str())
                && entry["ref"]["incarnation"].as_u64() == Some(r.incarnation)
        })
        .cloned()
}

fn world_read<F>(
    lua: &Lua,
    symbol: &str,
    path: &str,
    f: F,
) -> Result<mlua::MultiValue>
where
    F: FnOnce(&View) -> Json,
{
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

    // eqoa.quests.*
    bind(
        lua,
        "quests",
        "list",
        lua.create_function(|lua, ()| {
            world_read(lua, "eqoa.quests.list", "eqoa.quests.Quest", |v| {
                json!(v.quests)
            })
        })?,
    )?;
    bind(
        lua,
        "quests",
        "get",
        lua.create_function(|lua, r: Value| {
            let (world, entry) = CORE.with(|c| {
                let c = c.borrow();
                (
                    c.view.in_world,
                    resolve_in(&r, &c.view.quests, c.generation, "quest"),
                )
            });
            if !world {
                return error(
                    lua,
                    "eqoa.quests.get",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Some(entry) = entry else {
                return error(
                    lua,
                    "eqoa.quests.get",
                    "stale_handle",
                    "quest handle is stale or invalid",
                );
            };
            lua.pack_multi((record(lua, "eqoa.quests.Quest", &entry)?, Value::Nil))
        })?,
    )?;

    // eqoa.mastery.*
    bind(
        lua,
        "mastery",
        "get",
        lua.create_function(|lua, ()| {
            world_read(lua, "eqoa.mastery.get", "eqoa.mastery.Mastery", |v| {
                v.mastery.clone().unwrap_or(Json::Null)
            })
        })?,
    )?;
    bind(
        lua,
        "mastery",
        "skills",
        lua.create_function(|lua, ()| {
            let (world, skills) = CORE.with(|c| {
                let c = c.borrow();
                (c.view.in_world, c.view.mastery_skills.clone())
            });
            if !world {
                return error(
                    lua,
                    "eqoa.mastery.skills",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Some(skills) = skills else {
                return error(
                    lua,
                    "eqoa.mastery.skills",
                    "unavailable",
                    "waiting for class mastery skill list (sub-op 8)",
                );
            };
            lua.pack_multi((
                record(lua, "eqoa.mastery.MasterySkill", &json!(skills))?,
                Value::Nil,
            ))
        })?,
    )?;
    bind(
        lua,
        "mastery",
        "selected",
        lua.create_function(|lua, ()| {
            let (world, node) = CORE.with(|c| {
                let c = c.borrow();
                (c.view.in_world, c.view.mastery_node.clone())
            });
            if !world {
                return error(
                    lua,
                    "eqoa.mastery.selected",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            match node {
                Some(n) => lua.pack_multi((record(lua, "eqoa.mastery.MasteryNode", &n)?, Value::Nil)),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;

    for (path, fields) in [
        ("eqoa.quests.Quest", "ref index title"),
        (
            "eqoa.mastery.Mastery",
            "available spent experience_share progress_raw pending stats bonuses",
        ),
        (
            "eqoa.mastery.MasterySkill",
            "id parent_key rank_key name category cost known owned master_class",
        ),
        (
            "eqoa.mastery.MasteryNode",
            "id skill_id parent_key name title category cost text known owned level_required points_spent_required minimum_stats",
        ),
    ] {
        describe(lua, path, "supported", Some(4), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(4), "");
        }
    }

    Ok(())
}
