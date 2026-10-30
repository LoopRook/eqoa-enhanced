//! Native FrontiersForge entity and entity_list adapter module (`frontiers_forge.entity`, `frontiers_forge.entity_list`).
//!
//! Authoritative Contracts:
//! - Upstream pinned reference: `scripts/modules/frontiers_forge/entity.lua`,
//!   `scripts/modules/frontiers_forge/entity_list.lua`, and `wereoxxs_ui.lua`.
//!
//! Scope & Invariant Guarantees:
//! - Fixed-size, Copy snapshot of entity state (`EntitySnapshot`, `EntitiesSnapshot`).
//! - Full native implementation of `frontiers_forge.entity` and `frontiers_forge.entity_list`.
//! - Read-only access to engine entity state via `host.snapshot`.
//! - Metatable with both method invocation (`:GetId()`, `:GetName()`) and property access (`.id`, `.name`).
//! - Slot 0 synthesized for local player if not populated.

use super::host::Host;
use mlua::{Function, Result as LuaResult, Table, Value};
use std::cell::Cell;
use std::rc::Rc;

pub const MAX_ENTITIES: usize = 32;

/// Fixed-size, Copy snapshot of a single entity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntitySnapshot {
    pub id: u32,
    pub level: u8,
    pub health_percent_255: u8,
    pub position: [f32; 3],
    pub name: [u8; 24],
    pub name_len: u8,
    pub target_id: u32,
    pub class_byte: u8,
    pub status_flags: u8,
    pub disposition: Option<u8>,
    pub is_populated: bool,
}

impl Default for EntitySnapshot {
    fn default() -> Self {
        Self {
            id: 0,
            level: 0,
            health_percent_255: 0,
            position: [0.0; 3],
            name: [0u8; 24],
            name_len: 0,
            target_id: 0,
            class_byte: 0,
            status_flags: 0,
            disposition: None,
            is_populated: false,
        }
    }
}

/// Fixed-size, Copy snapshot of entity list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntitiesSnapshot {
    pub count: u8,
    pub entities: [EntitySnapshot; MAX_ENTITIES],
}

impl Default for EntitiesSnapshot {
    fn default() -> Self {
        Self {
            count: 24,
            entities: [EntitySnapshot::default(); MAX_ENTITIES],
        }
    }
}

fn get_disposition_name_str(disposition: Option<u8>) -> &'static str {
    match disposition {
        Some(0..=1) => "Hostile",
        Some(2..=3) => "Neutral",
        Some(4..=6) => "Friendly",
        _ => "Unknown",
    }
}

fn create_entity_object(
    lua: &mlua::Lua,
    entity_mt: &Table,
    snapshot_cell: &Rc<Cell<super::host::Snapshot>>,
    record: usize,
) -> LuaResult<Table> {
    let tbl = lua.create_table()?;
    tbl.set("record", record)?;
    let snap = snapshot_cell.get().entities;
    let id = if record < snap.entities.len() {
        snap.entities[record].id
    } else {
        0
    };
    tbl.set("_id", id)?;
    let _ = tbl.set_metatable(Some(entity_mt.clone()));
    Ok(tbl)
}

pub fn install_entity_adapter(host: &Host) -> LuaResult<()> {
    let lua = &host.lua;
    let entity = lua.create_table()?;
    let snapshot_cell = host.snapshot.clone();

    // 1. Constants
    let disposition_table = lua.create_table()?;
    disposition_table.set("HOSTILE_MIN", 0)?;
    disposition_table.set("HOSTILE_MAX", 1)?;
    disposition_table.set("NEUTRAL_MIN", 2)?;
    disposition_table.set("NEUTRAL_MAX", 3)?;
    disposition_table.set("FRIENDLY_MIN", 4)?;
    disposition_table.set("FRIENDLY_MAX", 6)?;
    entity.set("Disposition", disposition_table)?;

    let status_flag_table = lua.create_table()?;
    status_flag_table.set("NAMEPLATE_HIDDEN", 0x01)?;
    status_flag_table.set("POISONED", 0x04)?;
    status_flag_table.set("DISEASED", 0x08)?;
    entity.set("StatusFlag", status_flag_table)?;

    let status_icon_tex_id_table = lua.create_table()?;
    status_icon_tex_id_table.set("POISONED", 0x76)?;
    status_icon_tex_id_table.set("DISEASED", 0x77)?;
    entity.set("StatusIconTexId", status_icon_tex_id_table)?;

    // 2. Disposition helper
    entity.set(
        "GetDispositionName",
        lua.create_function(|_, disposition: Value| {
            let d_opt = match disposition {
                Value::Integer(i) if (0..=255).contains(&i) => Some(i as u8),
                Value::Number(n) if n >= 0.0 && n <= 255.0 => Some(n as u8),
                _ => None,
            };
            Ok(get_disposition_name_str(d_opt))
        })?,
    )?;

    // 3. Static field getters (record in)
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetId",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].id
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetHealthPercent",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].health_percent_255 as f64 / 255.0
                } else {
                    0.0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetPosition",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    let pos = snap.entities[record].position;
                    Ok((pos[0], pos[1], pos[2]))
                } else {
                    Ok((0.0, 0.0, 0.0))
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetName",
            lua.create_function(move |lua_ctx, record: usize| {
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    let e = &snap.entities[record];
                    let len = (e.name_len as usize).min(e.name.len());
                    let name_str = std::str::from_utf8(&e.name[..len]).unwrap_or("");
                    Ok(Value::String(lua_ctx.create_string(name_str)?))
                } else {
                    Ok(Value::String(lua_ctx.create_string("")?))
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetLevel",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].level as u32
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetDistanceTo",
            lua.create_function(move |_, (record, x, y, z): (usize, f64, f64, f64)| {
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    let pos = snap.entities[record].position;
                    let dx = pos[0] as f64 - x;
                    let dy = pos[1] as f64 - y;
                    let dz = pos[2] as f64 - z;
                    Ok((dx * dx + dy * dy + dz * dz).sqrt())
                } else {
                    Ok(0.0)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetClassByte",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].class_byte as u32
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "IsPlayerCharacter",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(record < snap.entities.len() && snap.entities[record].class_byte == 2)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "IsNpc",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(record < snap.entities.len() && snap.entities[record].class_byte < 2)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "IsPopulated",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    let e = &snap.entities[record];
                    e.is_populated || (e.id != 0 && e.level > 0)
                } else {
                    false
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "IsUnnamed",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(record >= snap.entities.len() || snap.entities[record].name_len == 0)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetStatusFlags",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].status_flags as u32
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "HasStatusFlag",
            lua.create_function(move |_, (record, mask): (usize, u32)| {
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    let flags = snap.entities[record].status_flags as u32;
                    Ok((flags & mask) == mask)
                } else {
                    Ok(false)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "IsPoisoned",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(
                    record < snap.entities.len()
                        && (snap.entities[record].status_flags & 0x04) != 0,
                )
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "IsDiseased",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(
                    record < snap.entities.len()
                        && (snap.entities[record].status_flags & 0x08) != 0,
                )
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetTargetId",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].target_id
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        entity.set(
            "GetDisposition",
            lua.create_function(move |_, record: usize| {
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    match snap.entities[record].disposition {
                        Some(d) => Ok(Value::Integer(d as i64)),
                        None => Ok(Value::Nil),
                    }
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }

    // 4. Methods table for entity instances
    let methods = lua.create_table()?;
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetId",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].id
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetHealthPercent",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].health_percent_255 as f64 / 255.0
                } else {
                    0.0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetPosition",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    let pos = snap.entities[record].position;
                    Ok((pos[0], pos[1], pos[2]))
                } else {
                    Ok((0.0, 0.0, 0.0))
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetName",
            lua.create_function(move |lua_ctx, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    let e = &snap.entities[record];
                    let len = (e.name_len as usize).min(e.name.len());
                    let name_str = std::str::from_utf8(&e.name[..len]).unwrap_or("");
                    Ok(Value::String(lua_ctx.create_string(name_str)?))
                } else {
                    Ok(Value::String(lua_ctx.create_string("")?))
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetLevel",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].level as u32
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetTargetId",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].target_id
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetClassByte",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].class_byte as u32
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "IsPlayerCharacter",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(record < snap.entities.len() && snap.entities[record].class_byte == 2)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "IsNpc",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(record < snap.entities.len() && snap.entities[record].class_byte < 2)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "IsPopulated",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    let e = &snap.entities[record];
                    e.is_populated || (e.id != 0 && e.level > 0)
                } else {
                    false
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "IsUnnamed",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(record >= snap.entities.len() || snap.entities[record].name_len == 0)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetDistanceTo",
            lua.create_function(move |_, (this, x, y, z): (Table, f64, f64, f64)| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    let pos = snap.entities[record].position;
                    let dx = pos[0] as f64 - x;
                    let dy = pos[1] as f64 - y;
                    let dz = pos[2] as f64 - z;
                    Ok((dx * dx + dy * dy + dz * dz).sqrt())
                } else {
                    Ok(0.0)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetDisposition",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    match snap.entities[record].disposition {
                        Some(d) => Ok(Value::Integer(d as i64)),
                        None => Ok(Value::Nil),
                    }
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "GetStatusFlags",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(if record < snap.entities.len() {
                    snap.entities[record].status_flags as u32
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "HasStatusFlag",
            lua.create_function(move |_, (this, mask): (Table, u32)| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                if record < snap.entities.len() {
                    let flags = snap.entities[record].status_flags as u32;
                    Ok((flags & mask) == mask)
                } else {
                    Ok(false)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "IsPoisoned",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(
                    record < snap.entities.len()
                        && (snap.entities[record].status_flags & 0x04) != 0,
                )
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "IsDiseased",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                Ok(
                    record < snap.entities.len()
                        && (snap.entities[record].status_flags & 0x08) != 0,
                )
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        methods.set(
            "IsValid",
            lua.create_function(move |_, this: Table| {
                let record: usize = this.get("record")?;
                let orig_id: u32 = this.get("_id")?;
                let snap = s.get().entities;
                Ok(record < snap.entities.len() && snap.entities[record].id == orig_id)
            })?,
        )?;
    }

    // 5. Metatable with unified method & property resolution
    let entity_mt = lua.create_table()?;
    {
        let s = snapshot_cell.clone();
        let methods_table = methods.clone();
        entity_mt.set(
            "__index",
            lua.create_function(move |lua_ctx, (this, key): (Table, String)| {
                // Check methods first
                let method_val: Value = methods_table.get(key.as_str())?;
                if method_val != Value::Nil {
                    return Ok(method_val);
                }

                // Check properties
                let record: usize = this.get("record")?;
                let snap = s.get().entities;
                let ent = if record < snap.entities.len() {
                    snap.entities[record]
                } else {
                    EntitySnapshot::default()
                };

                match key.as_str() {
                    "id" => Ok(Value::Integer(ent.id as i64)),
                    "percent_hp" => Ok(Value::Number(ent.health_percent_255 as f64 / 255.0)),
                    "name" => {
                        let len = (ent.name_len as usize).min(ent.name.len());
                        let name_str = std::str::from_utf8(&ent.name[..len]).unwrap_or("");
                        Ok(Value::String(lua_ctx.create_string(name_str)?))
                    }
                    "level" => Ok(Value::Integer(ent.level as i64)),
                    "target_id" => Ok(Value::Integer(ent.target_id as i64)),
                    "class_byte" => Ok(Value::Integer(ent.class_byte as i64)),
                    "is_player" => Ok(Value::Boolean(ent.class_byte == 2)),
                    "is_npc" => Ok(Value::Boolean(ent.class_byte < 2)),
                    "is_populated" => Ok(Value::Boolean(
                        ent.is_populated || (ent.id != 0 && ent.level > 0),
                    )),
                    "is_unnamed" => Ok(Value::Boolean(ent.name_len == 0)),
                    "disposition" => match ent.disposition {
                        Some(d) => Ok(Value::Integer(d as i64)),
                        None => Ok(Value::Nil),
                    },
                    "status_flags" => Ok(Value::Integer(ent.status_flags as i64)),
                    "is_poisoned" => Ok(Value::Boolean((ent.status_flags & 0x04) != 0)),
                    "is_diseased" => Ok(Value::Boolean((ent.status_flags & 0x08) != 0)),
                    "x" => Ok(Value::Number(ent.position[0] as f64)),
                    "y" => Ok(Value::Number(ent.position[1] as f64)),
                    "z" => Ok(Value::Number(ent.position[2] as f64)),
                    "disposition_name" => {
                        let disp_str = get_disposition_name_str(ent.disposition);
                        Ok(Value::String(lua_ctx.create_string(disp_str)?))
                    }
                    _ => Ok(Value::Nil),
                }
            })?,
        )?;
    }

    // 6. Entity.new(record)
    {
        let s = snapshot_cell;
        let mt = entity_mt.clone();
        entity.set(
            "new",
            lua.create_function(move |lua_ctx, record: usize| {
                create_entity_object(lua_ctx, &mt, &s, record).map(Value::Table)
            })?,
        )?;
    }

    host.register_host_module("frontiers_forge.entity", Value::Table(entity))?;
    Ok(())
}

pub fn install_entity_list_adapter(host: &Host) -> LuaResult<()> {
    let lua = &host.lua;
    let entity_list = lua.create_table()?;
    let snapshot_cell = host.snapshot.clone();

    // 1. EntityList.GetEntityByIndex(index)
    {
        entity_list.set(
            "GetEntityByIndex",
            lua.create_function(move |lua_ctx, index: i64| {
                if !(0..=23).contains(&index) {
                    return Err(mlua::Error::runtime(format!(
                        "Index out of bounds: Entity list index must be between 0 and 23"
                    )));
                }
                // Fetch Entity.new from frontiers_forge.entity module
                let entity_mod: Table = lua_ctx
                    .load("return require('frontiers_forge.entity')")
                    .eval()?;
                let new_fn: Function = entity_mod.get("new")?;
                new_fn.call::<Value>(index as usize)
            })?,
        )?;
    }

    // 2. EntityList.GetEntityById(target_id)
    {
        let s = snapshot_cell.clone();
        entity_list.set(
            "GetEntityById",
            lua.create_function(move |lua_ctx, target_id: u32| {
                let snap = s.get().entities;
                for i in 0..24 {
                    if i < snap.entities.len() && snap.entities[i].id == target_id {
                        let entity_mod: Table = lua_ctx
                            .load("return require('frontiers_forge.entity')")
                            .eval()?;
                        let new_fn: Function = entity_mod.get("new")?;
                        return new_fn.call::<Value>(i);
                    }
                }
                Ok(Value::Nil)
            })?,
        )?;
    }

    // 3. EntityList.GetAllEntitiesWithName(target_name)
    {
        let s = snapshot_cell.clone();
        entity_list.set(
            "GetAllEntitiesWithName",
            lua.create_function(move |lua_ctx, target_name: String| {
                let snap = s.get().entities;
                let matches = lua_ctx.create_table()?;
                let mut match_count = 0;
                let entity_mod: Table = lua_ctx
                    .load("return require('frontiers_forge.entity')")
                    .eval()?;
                let new_fn: Function = entity_mod.get("new")?;

                for i in 0..24 {
                    if i < snap.entities.len() {
                        let e = &snap.entities[i];
                        let len = (e.name_len as usize).min(e.name.len());
                        let name_str = std::str::from_utf8(&e.name[..len]).unwrap_or("");
                        if name_str == target_name {
                            match_count += 1;
                            let ent_obj = new_fn.call::<Value>(i)?;
                            matches.raw_set(match_count, ent_obj)?;
                        }
                    }
                }

                if match_count == 0 {
                    Ok(Value::Nil)
                } else {
                    Ok(Value::Table(matches))
                }
            })?,
        )?;
    }

    // 4. EntityList.GetFirstEntityWithName(target_name)
    {
        let s = snapshot_cell.clone();
        entity_list.set(
            "GetFirstEntityWithName",
            lua.create_function(move |lua_ctx, target_name: String| {
                let snap = s.get().entities;
                let entity_mod: Table = lua_ctx
                    .load("return require('frontiers_forge.entity')")
                    .eval()?;
                let new_fn: Function = entity_mod.get("new")?;

                for i in 0..24 {
                    if i < snap.entities.len() {
                        let e = &snap.entities[i];
                        let len = (e.name_len as usize).min(e.name.len());
                        let name_str = std::str::from_utf8(&e.name[..len]).unwrap_or("");
                        if name_str == target_name {
                            return new_fn.call::<Value>(i);
                        }
                    }
                }
                Ok(Value::Nil)
            })?,
        )?;
    }

    // 5. EntityList.GetAllEntities()
    {
        entity_list.set(
            "GetAllEntities",
            lua.create_function(move |lua_ctx, ()| {
                let all = lua_ctx.create_table()?;
                let entity_mod: Table = lua_ctx
                    .load("return require('frontiers_forge.entity')")
                    .eval()?;
                let new_fn: Function = entity_mod.get("new")?;
                for i in 0..24 {
                    let ent_obj = new_fn.call::<Value>(i)?;
                    all.raw_set(i + 1, ent_obj)?;
                }
                Ok(all)
            })?,
        )?;
    }

    host.register_host_module("frontiers_forge.entity_list", Value::Table(entity_list))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::discovery::DiscoveredAddon;
    use crate::fork_ext::addons::group::GroupMemberSnapshot;
    use crate::fork_ext::addons::host::Snapshot;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_test_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("eqoa_entity_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_addon(root: &Path) -> DiscoveredAddon {
        let entry = root.join("main.lua");
        fs::write(&entry, "-- test entity addon").unwrap();
        DiscoveredAddon {
            id: "test_entity".to_string(),
            name: "Test Entity".to_string(),
            entry,
            script_root: root.to_path_buf(),
            package_root: root.to_path_buf(),
            module_dirs: vec![root.to_path_buf()],
            resource_dirs: vec![],
            api: Some("eqoa/1".to_string()),
            requested_capabilities: vec![],
            initially_enabled: true,
        }
    }

    #[test]
    fn forge_entity_constants_and_helpers() {
        let dir = temp_test_dir("entity_consts");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let chunk = host.lua.load(
            r#"
            local Entity = require("frontiers_forge.entity")
            assert(Entity.Disposition.HOSTILE_MIN == 0)
            assert(Entity.Disposition.HOSTILE_MAX == 1)
            assert(Entity.Disposition.NEUTRAL_MIN == 2)
            assert(Entity.Disposition.NEUTRAL_MAX == 3)
            assert(Entity.Disposition.FRIENDLY_MIN == 4)
            assert(Entity.Disposition.FRIENDLY_MAX == 6)

            assert(Entity.StatusFlag.NAMEPLATE_HIDDEN == 1)
            assert(Entity.StatusFlag.POISONED == 4)
            assert(Entity.StatusFlag.DISEASED == 8)

            assert(Entity.StatusIconTexId.POISONED == 0x76)
            assert(Entity.StatusIconTexId.DISEASED == 0x77)

            assert(Entity.GetDispositionName(nil) == "Unknown")
            assert(Entity.GetDispositionName(0) == "Hostile")
            assert(Entity.GetDispositionName(1) == "Hostile")
            assert(Entity.GetDispositionName(2) == "Neutral")
            assert(Entity.GetDispositionName(3) == "Neutral")
            assert(Entity.GetDispositionName(4) == "Friendly")
            assert(Entity.GetDispositionName(5) == "Friendly")
            assert(Entity.GetDispositionName(6) == "Friendly")
            assert(Entity.GetDispositionName(7) == "Unknown")
            assert(Entity.GetDispositionName(-1) == "Unknown")

            return true
            "#,
        );
        let ok: bool = chunk.eval().unwrap();
        assert!(ok);
    }

    #[test]
    fn forge_entity_methods_and_properties() {
        let dir = temp_test_dir("entity_props");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = Snapshot::default();
        snap.in_game = true;

        // Slot 0: Local player (Hero, Level 50, Paladin PC, full HP)
        let mut e0 = EntitySnapshot::default();
        e0.id = 5001;
        let name0 = b"Hero";
        e0.name[..name0.len()].copy_from_slice(name0);
        e0.name_len = name0.len() as u8;
        e0.level = 50;
        e0.class_byte = 2; // player character
        e0.health_percent_255 = 255;
        e0.position = [10.0, 20.0, 30.0];
        e0.target_id = 5002;
        e0.is_populated = true;
        snap.entities.entities[0] = e0;

        // Slot 1: Hostile monster (Orc, Level 45, NPC, half HP, poisoned)
        let mut e1 = EntitySnapshot::default();
        e1.id = 5002;
        let name1 = b"Orc Warlord";
        e1.name[..name1.len()].copy_from_slice(name1);
        e1.name_len = name1.len() as u8;
        e1.level = 45;
        e1.class_byte = 0; // NPC
        e1.health_percent_255 = 128;
        e1.position = [13.0, 24.0, 30.0]; // distance = sqrt(3^2 + 4^2) = 5.0
        e1.target_id = 5001;
        e1.status_flags = 0x04; // POISONED
        e1.disposition = Some(0); // Hostile
        e1.is_populated = true;
        snap.entities.entities[1] = e1;

        host.snapshot.set(snap);

        let chunk = host.lua.load(
            r#"
            local Entity = require("frontiers_forge.entity")
            local EntityList = require("frontiers_forge.entity_list")

            local p = EntityList.GetEntityByIndex(0)
            assert(p ~= nil)
            -- Methods
            assert(p:GetId() == 5001)
            assert(p:GetName() == "Hero")
            assert(p:GetLevel() == 50)
            assert(p:GetClassByte() == 2)
            assert(p:IsPlayerCharacter() == true)
            assert(p:IsNpc() == false)
            assert(p:IsPopulated() == true)
            assert(p:IsUnnamed() == false)
            assert(math.abs(p:GetHealthPercent() - 1.0) < 0.001)
            local px, py, pz = p:GetPosition()
            assert(px == 10.0 and py == 20.0 and pz == 30.0)
            assert(p:GetTargetId() == 5002)
            assert(p:IsValid() == true)

            -- Properties
            assert(p.id == 5001)
            assert(p.name == "Hero")
            assert(p.level == 50)
            assert(p.class_byte == 2)
            assert(p.is_player == true)
            assert(p.is_npc == false)
            assert(p.is_populated == true)
            assert(p.is_unnamed == false)
            assert(p.x == 10.0 and p.y == 20.0 and p.z == 30.0)
            assert(p.target_id == 5002)

            -- Slot 1 (Target monster)
            local m = EntityList.GetEntityByIndex(1)
            assert(m ~= nil)
            assert(m:GetId() == 5002)
            assert(m:GetName() == "Orc Warlord")
            assert(m:GetLevel() == 45)
            assert(m:IsPlayerCharacter() == false)
            assert(m:IsNpc() == true)
            assert(m:IsPoisoned() == true)
            assert(m:IsDiseased() == false)
            assert(m:HasStatusFlag(4) == true)
            assert(m:GetDisposition() == 0)
            assert(m.disposition == 0)
            assert(m.disposition_name == "Hostile")
            assert(m.is_poisoned == true)
            assert(m.is_diseased == false)

            -- 3D distance check: (10,20,30) to (13,24,30) is 5.0
            local dist = p:GetDistanceTo(13.0, 24.0, 30.0)
            assert(math.abs(dist - 5.0) < 0.001)

            return true
            "#,
        );
        let ok: bool = chunk.eval().unwrap();
        assert!(ok);
    }

    #[test]
    fn forge_entity_list_queries_and_bounds() {
        let dir = temp_test_dir("entity_queries");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = Snapshot::default();
        snap.in_game = true;

        let mut e0 = EntitySnapshot::default();
        e0.id = 101;
        let n0 = b"PlayerOne";
        e0.name[..n0.len()].copy_from_slice(n0);
        e0.name_len = n0.len() as u8;
        e0.level = 10;
        e0.is_populated = true;
        snap.entities.entities[0] = e0;

        let mut e1 = EntitySnapshot::default();
        e1.id = 202;
        let n1 = b"Wolf";
        e1.name[..n1.len()].copy_from_slice(n1);
        e1.name_len = n1.len() as u8;
        e1.level = 5;
        e1.is_populated = true;
        snap.entities.entities[1] = e1;

        let mut e2 = EntitySnapshot::default();
        e2.id = 203;
        let n2 = b"Wolf";
        e2.name[..n2.len()].copy_from_slice(n2);
        e2.name_len = n2.len() as u8;
        e2.level = 6;
        e2.is_populated = true;
        snap.entities.entities[2] = e2;

        host.snapshot.set(snap);

        let chunk = host.lua.load(
            r#"
            local EntityList = require("frontiers_forge.entity_list")

            -- Out of bounds error check
            local ok_neg = pcall(function() EntityList.GetEntityByIndex(-1) end)
            assert(ok_neg == false)
            local ok_over = pcall(function() EntityList.GetEntityByIndex(24) end)
            assert(ok_over == false)

            -- Valid index
            local e0 = EntityList.GetEntityByIndex(0)
            assert(e0 ~= nil and e0:GetId() == 101)
            local e23 = EntityList.GetEntityByIndex(23)
            assert(e23 ~= nil)

            -- GetEntityById
            local by_id = EntityList.GetEntityById(202)
            assert(by_id ~= nil and by_id:GetName() == "Wolf" and by_id:GetLevel() == 5)
            assert(EntityList.GetEntityById(999999) == nil)

            -- GetFirstEntityWithName
            local first_wolf = EntityList.GetFirstEntityWithName("Wolf")
            assert(first_wolf ~= nil and first_wolf:GetId() == 202)
            assert(EntityList.GetFirstEntityWithName("Nonexistent") == nil)

            -- GetAllEntitiesWithName
            local wolves = EntityList.GetAllEntitiesWithName("Wolf")
            assert(wolves ~= nil and #wolves == 2)
            assert(wolves[1]:GetId() == 202)
            assert(wolves[2]:GetId() == 203)
            assert(EntityList.GetAllEntitiesWithName("Nonexistent") == nil)

            -- GetAllEntities
            local all = EntityList.GetAllEntities()
            assert(#all == 24)
            assert(all[1]:GetId() == 101)
            assert(all[2]:GetId() == 202)
            assert(all[3]:GetId() == 203)

            return true
            "#,
        );
        let ok: bool = chunk.eval().unwrap();
        assert!(ok);
    }

    #[test]
    fn forge_wereoxxs_group_and_target_integration() {
        let dir = temp_test_dir("wereoxxs_integ");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = Snapshot::default();
        snap.in_game = true;
        snap.group.in_group = true;
        snap.group.member_count = 3;

        // Player is entity 100 (slot 0)
        let mut p_ent = EntitySnapshot::default();
        p_ent.id = 100;
        let p_name = b"Hero";
        p_ent.name[..p_name.len()].copy_from_slice(p_name);
        p_ent.name_len = p_name.len() as u8;
        p_ent.level = 60;
        p_ent.class_byte = 2;
        p_ent.health_percent_255 = 255;
        p_ent.is_populated = true;
        snap.entities.entities[0] = p_ent;

        // Group member 0 is local player (entity 100)
        let mut gm0 = GroupMemberSnapshot::default();
        gm0.entity_id = 100;
        gm0.name[..p_name.len()].copy_from_slice(p_name);
        gm0.name_len = p_name.len() as u8;
        gm0.active = true;
        gm0.hp_known = true;
        gm0.percent_hp_255 = 255;
        snap.group.members[0] = gm0;

        // Group member 1 is entity 200 (slot 1), poisoned
        let mut gm1 = GroupMemberSnapshot::default();
        gm1.entity_id = 200;
        let gm1_name = b"Healer";
        gm1.name[..gm1_name.len()].copy_from_slice(gm1_name);
        gm1.name_len = gm1_name.len() as u8;
        gm1.active = true;
        gm1.hp_known = true;
        gm1.percent_hp_255 = 200;
        gm1.position = [15.0, 25.0, 35.0];
        snap.group.members[1] = gm1;

        let mut ent1 = EntitySnapshot::default();
        ent1.id = 200;
        ent1.name[..gm1_name.len()].copy_from_slice(gm1_name);
        ent1.name_len = gm1_name.len() as u8;
        ent1.level = 58;
        ent1.class_byte = 2;
        ent1.status_flags = 0x04; // POISONED
        ent1.is_populated = true;
        snap.entities.entities[1] = ent1;

        // Target is entity 300 (slot 2), hostile boss, diseased
        let mut ent2 = EntitySnapshot::default();
        ent2.id = 300;
        let boss_name = b"Dragon";
        ent2.name[..boss_name.len()].copy_from_slice(boss_name);
        ent2.name_len = boss_name.len() as u8;
        ent2.level = 65;
        ent2.class_byte = 0; // NPC
        ent2.status_flags = 0x08; // DISEASED
        ent2.disposition = Some(0); // Hostile
        ent2.health_percent_255 = 128;
        ent2.position = [50.0, 0.0, 100.0];
        ent2.is_populated = true;
        snap.entities.entities[2] = ent2;

        host.snapshot.set(snap);

        // Run Wereoxxs exact pattern
        let chunk = host.lua.load(
            r#"
            local EntityList = require("frontiers_forge.entity_list")
            local Group = require("frontiers_forge.group")
            local Entity = require("frontiers_forge.entity")

            local player_id = 100

            local function BuildStatusIcons(entity)
                if entity == nil then return {} end
                local icons = {}
                if entity:IsPoisoned() then
                    icons[#icons + 1] = { tex_id = Entity.StatusIconTexId.POISONED, name = "Poisoned" }
                end
                if entity:IsDiseased() then
                    icons[#icons + 1] = { tex_id = Entity.StatusIconTexId.DISEASED, name = "Diseased" }
                end
                return icons
            end

            -- BuildGroupRoster pattern from wereoxxs_ui.lua
            local roster = {}
            for _, member in Group.Members() do
                if member:IsActive() and member:GetEntityId() ~= player_id then
                    local coords = member:GetCoordinates()
                    roster[#roster + 1] = {
                        name = member:GetName(),
                        hp_percent = member:GetHealthPercent(),
                        status_icons = BuildStatusIcons(EntityList.GetEntityById(member:GetEntityId())),
                        x = coords and coords.x, y = coords and coords.y, z = coords and coords.z,
                        no_mod = true,
                    }
                end
            end

            assert(#roster == 1)
            assert(roster[1].name == "Healer")
            assert(roster[1].x == 15.0 and roster[1].y == 25.0 and roster[1].z == 35.0)
            assert(#roster[1].status_icons == 1)
            assert(roster[1].status_icons[1].name == "Poisoned")
            assert(roster[1].status_icons[1].tex_id == 0x76)

            -- Target frame pattern from wereoxxs_ui.lua
            local target_id = 300
            local target = EntityList.GetEntityById(target_id)
            assert(target ~= nil)
            local target_row = {
                name = target:GetName(),
                level = target:GetLevel(),
                hp_percent = (target:GetHealthPercent() or 0) * 100,
                status_icons = BuildStatusIcons(target),
                disposition = target:GetDisposition(),
                disposition_name = target.disposition_name,
            }
            assert(target_row.name == "Dragon")
            assert(target_row.level == 65)
            assert(math.abs(target_row.hp_percent - (128 / 255 * 100)) < 0.01)
            assert(#target_row.status_icons == 1)
            assert(target_row.status_icons[1].name == "Diseased")
            assert(target_row.status_icons[1].tex_id == 0x77)
            assert(target_row.disposition == 0)
            assert(target_row.disposition_name == "Hostile")

            return true
            "#,
        );
        let ok: bool = chunk.eval().unwrap();
        assert!(ok);
    }
}
