//! Native FrontiersForge ability adapters (`frontiers_forge.ability`, `ability_list`, `ability_bar`, `icon`).
//!
//! Authoritative Contracts:
//! - Upstream pinned reference: `scripts/modules/frontiers_forge/ability.lua`,
//!   `scripts/modules/frontiers_forge/ability_list.lua`,
//!   `scripts/modules/frontiers_forge/ability_bar.lua`,
//!   `scripts/modules/frontiers_forge/icon.lua`, and `wereoxxs_ui.lua`.
//!
//! Scope & Invariant Guarantees:
//! - Fixed-size, Copy snapshot of ability state (`AbilitiesSnapshot`, `AbilitySnapshot`, `AbilityBarSnapshot`).
//! - Full native implementation of `frontiers_forge.ability`, `frontiers_forge.ability_list`,
//!   `frontiers_forge.ability_bar`, and `frontiers_forge.icon`.
//! - Read-only access to engine ability/menu state via `host.snapshot`.
//! - Safe cooldown event bus (`OnLockoutStart`, `OnLockoutEnd`) driven by `AbilityList.Update()`.
//! - Shared icon cache claims tracking with refcounting.

use super::host::Host;
use mlua::{Lua, Result as LuaResult, Table, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub const MAX_ABILITIES: usize = 256;
pub const MAX_BARS: usize = 3;
pub const MAX_BAR_SLOTS: usize = 5;
pub const MAX_ITEM_RECORDS: usize = 40;
pub const MAX_ITEM_SLOTS: usize = MAX_ITEM_RECORDS;

pub const ABILITY_SOURCE_TAG: u32 = 0x8060000;
pub const ITEM_SOURCE_TAG: u32 = 0x2030000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemSnapshot {
    pub name: [u8; 32],
    pub name_len: u8,
    pub description: [u8; 64],
    pub desc_len: u8,
    pub icon_ref: u32,
    pub slot_id: u32,
    pub populated: bool,
}

impl Default for ItemSnapshot {
    fn default() -> Self {
        Self {
            name: [0u8; 32],
            name_len: 0,
            description: [0u8; 64],
            desc_len: 0,
            icon_ref: 0,
            slot_id: 0,
            populated: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AbilitySnapshot {
    pub id: u32,
    pub display_index: u32,
    pub spellbook_slot: u32,
    pub level: u32,
    pub range: f32,
    pub cast_time: u32,
    pub power_cost: u32,
    pub icon_background_ref: u32,
    pub icon_foreground_ref: u32,
    pub scope: u32,
    pub cooldown_seconds: u32,
    pub cooldown_lockout_ms: u32,
    pub equip_requirements: u32,
    pub name: [u8; 32],
    pub name_len: u8,
    pub description: [u8; 64],
    pub desc_len: u8,
    pub valid: bool,
}

impl Default for AbilitySnapshot {
    fn default() -> Self {
        Self {
            id: 0,
            display_index: 0,
            spellbook_slot: 0,
            level: 0,
            range: 0.0,
            cast_time: 0,
            power_cost: 0,
            icon_background_ref: 0,
            icon_foreground_ref: 0,
            scope: 0,
            cooldown_seconds: 0,
            cooldown_lockout_ms: 0,
            equip_requirements: 0,
            name: [0u8; 32],
            name_len: 0,
            description: [0u8; 64],
            desc_len: 0,
            valid: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AbilityBarSlotSnapshot {
    pub source_type: u32,
    pub source_index: u32,
    pub ability_icon_ref: i32,
}

impl Default for AbilityBarSlotSnapshot {
    fn default() -> Self {
        Self {
            source_type: 0,
            source_index: 0,
            ability_icon_ref: -1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AbilityBarSnapshot {
    pub slot_count: u8,
    pub selected_slot: u8,
    pub slots: [AbilityBarSlotSnapshot; MAX_BAR_SLOTS],
}

impl Default for AbilityBarSnapshot {
    fn default() -> Self {
        Self {
            slot_count: 5,
            selected_slot: 0,
            slots: [AbilityBarSlotSnapshot::default(); MAX_BAR_SLOTS],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AbilitiesSnapshot {
    pub ability_count: u16,
    pub selected_bar: u8,
    pub abilities: [AbilitySnapshot; MAX_ABILITIES],
    pub bars: [AbilityBarSnapshot; MAX_BARS],
    pub items: [ItemSnapshot; MAX_ITEM_SLOTS],
}

impl Default for AbilitiesSnapshot {
    fn default() -> Self {
        Self {
            ability_count: 0,
            selected_bar: 0,
            abilities: [AbilitySnapshot::default(); MAX_ABILITIES],
            bars: [AbilityBarSnapshot::default(); MAX_BARS],
            items: [ItemSnapshot::default(); MAX_ITEM_SLOTS],
        }
    }
}

pub(crate) fn create_ability_object(lua: &Lua, snap: &AbilitySnapshot) -> LuaResult<Table> {
    let t = lua.create_table()?;
    let valid = snap.valid;
    t.set("IsValid", lua.create_function(move |_, ()| Ok(valid))?)?;

    let id = snap.id;
    t.set("GetId", lua.create_function(move |_, ()| Ok(id))?)?;

    let display_index = snap.display_index;
    t.set(
        "GetDisplayIndex",
        lua.create_function(move |_, ()| Ok(display_index))?,
    )?;

    let spellbook_slot = snap.spellbook_slot;
    t.set(
        "GetSpellbookSlot",
        lua.create_function(move |_, ()| Ok(spellbook_slot))?,
    )?;

    let level = snap.level;
    t.set("GetLevel", lua.create_function(move |_, ()| Ok(level))?)?;

    let range = snap.range;
    t.set("GetRange", lua.create_function(move |_, ()| Ok(range))?)?;

    let cast_time = snap.cast_time;
    t.set(
        "GetCastTime",
        lua.create_function(move |_, ()| Ok(cast_time))?,
    )?;

    let power_cost = snap.power_cost;
    t.set(
        "GetPwrCost",
        lua.create_function(move |_, ()| Ok(power_cost))?,
    )?;

    let icon_bg = snap.icon_background_ref;
    t.set(
        "GetIconBackgroundRef",
        lua.create_function(move |_, ()| Ok(icon_bg))?,
    )?;

    let icon_fg = snap.icon_foreground_ref;
    t.set(
        "GetIconForegroundRef",
        lua.create_function(move |_, ()| Ok(icon_fg))?,
    )?;

    let scope = snap.scope;
    t.set("GetScope", lua.create_function(move |_, ()| Ok(scope))?)?;

    let cooldown = snap.cooldown_seconds;
    t.set(
        "GetCooldown",
        lua.create_function(move |_, ()| Ok(cooldown))?,
    )?;

    let equip_req = snap.equip_requirements;
    t.set(
        "GetEquipRequirements",
        lua.create_function(move |_, ()| Ok(equip_req))?,
    )?;

    let lockout = snap.cooldown_lockout_ms;
    t.set(
        "IsOnCooldown",
        lua.create_function(move |_, ()| Ok(lockout > 0))?,
    )?;

    t.set(
        "GetCooldownLockoutMs",
        lua.create_function(move |_, ()| Ok(lockout))?,
    )?;

    t.set(
        "IsInRange",
        lua.create_function(move |_, args: mlua::MultiValue| {
            let (point_a, point_b) = if args.len() >= 3 {
                (
                    args.get(1).and_then(|v| v.as_table()).cloned(),
                    args.get(2).and_then(|v| v.as_table()).cloned(),
                )
            } else if args.len() >= 2 {
                (
                    args.get(0).and_then(|v| v.as_table()).cloned(),
                    args.get(1).and_then(|v| v.as_table()).cloned(),
                )
            } else {
                (None, None)
            };

            let (ax, ay, az) = if let Some(ref pa) = point_a {
                (
                    pa.get::<f32>("x").unwrap_or(0.0),
                    pa.get::<f32>("y").unwrap_or(0.0),
                    pa.get::<f32>("z").unwrap_or(0.0),
                )
            } else {
                (0.0, 0.0, 0.0)
            };

            let (bx, by, bz) = if let Some(ref pb) = point_b {
                (
                    pb.get::<f32>("x").unwrap_or(0.0),
                    pb.get::<f32>("y").unwrap_or(0.0),
                    pb.get::<f32>("z").unwrap_or(0.0),
                )
            } else {
                (0.0, 0.0, 0.0)
            };

            let dx = ax - bx;
            let dy = ay - by;
            let dz = az - bz;
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            if scope == 0 || range == 0.0 {
                Ok((true, distance))
            } else {
                Ok((distance <= range, distance))
            }
        })?,
    )?;

    let name = snap.name;
    let name_len = snap.name_len as usize;
    t.set(
        "GetName",
        lua.create_function(move |lua_ctx, ()| {
            let s = std::str::from_utf8(&name[..name_len.min(name.len())]).unwrap_or("");
            Ok(Value::String(lua_ctx.create_string(s)?))
        })?,
    )?;

    let desc = snap.description;
    let desc_len = snap.desc_len as usize;
    t.set(
        "GetDescription",
        lua.create_function(move |lua_ctx, ()| {
            let s = std::str::from_utf8(&desc[..desc_len.min(desc.len())]).unwrap_or("");
            Ok(Value::String(lua_ctx.create_string(s)?))
        })?,
    )?;

    Ok(t)
}

pub(crate) fn create_item_object(lua: &Lua, snap: &ItemSnapshot) -> LuaResult<Table> {
    let t = lua.create_table()?;
    let name = snap.name;
    let name_len = snap.name_len as usize;
    t.set(
        "GetName",
        lua.create_function(move |lua_ctx, ()| {
            let s = std::str::from_utf8(&name[..name_len.min(name.len())]).unwrap_or("");
            Ok(Value::String(lua_ctx.create_string(s)?))
        })?,
    )?;
    let desc = snap.description;
    let desc_len = snap.desc_len as usize;
    t.set(
        "GetDescription",
        lua.create_function(move |lua_ctx, ()| {
            let s = std::str::from_utf8(&desc[..desc_len.min(desc.len())]).unwrap_or("");
            Ok(Value::String(lua_ctx.create_string(s)?))
        })?,
    )?;
    let icon_ref = snap.icon_ref;
    t.set(
        "GetIconRef",
        lua.create_function(move |_, ()| Ok(icon_ref))?,
    )?;
    let slot_id = snap.slot_id;
    t.set(
        "GetSlotId",
        lua.create_function(move |_, ()| Ok(slot_id))?,
    )?;
    Ok(t)
}

pub fn install_ability_adapter(host: &Host) -> LuaResult<()> {
    let lua = &host.lua;
    let ability_mod = lua.create_table()?;

    let scope_tbl = lua.create_table()?;
    scope_tbl.set("SELF", 0)?;
    scope_tbl.set("TARGET", 1)?;
    scope_tbl.set("GROUP", 2)?;
    scope_tbl.set("PET", 3)?;
    scope_tbl.set("CORPSE", 4)?;
    scope_tbl.set("UNKNOWN", 5)?;
    ability_mod.set("Scope", scope_tbl)?;
    ability_mod.set("size", 0x1D8)?;

    host.register_host_module("frontiers_forge.ability", Value::Table(ability_mod))?;
    Ok(())
}

pub fn install_ability_list_adapter(host: &Host) -> LuaResult<()> {
    let lua = &host.lua;
    let list_mod = lua.create_table()?;

    let snapshot_cell = host.snapshot.clone();
    let hook_claims = Rc::new(RefCell::new(HashSet::<String>::new()));
    let subscribers = Rc::new(RefCell::new(HashMap::<
        String,
        HashMap<String, mlua::Function>,
    >::new()));
    let subscriber_errors = Rc::new(RefCell::new(Vec::<Table>::new()));
    let lockout_cache = Rc::new(RefCell::new(HashMap::<u32, u32>::new()));

    // Events constants table
    let events_tbl = lua.create_table()?;
    events_tbl.set("OnLockoutStart", "OnLockoutStart")?;
    events_tbl.set("OnLockoutEnd", "OnLockoutEnd")?;
    list_mod.set("Events", events_tbl)?;

    // GetCount
    {
        let s = snapshot_cell.clone();
        list_mod.set(
            "GetCount",
            lua.create_function(move |_, ()| {
                let snap = s.get().abilities;
                Ok(snap.ability_count as u32)
            })?,
        )?;
    }

    // GetAbilityByIndex (1-based index)
    {
        let s = snapshot_cell.clone();
        list_mod.set(
            "GetAbilityByIndex",
            lua.create_function(move |lua_ctx, index: usize| {
                let snap = s.get().abilities;
                if index >= 1 && index <= snap.ability_count as usize {
                    let a = &snap.abilities[index - 1];
                    if a.valid {
                        return Ok(Value::Table(create_ability_object(lua_ctx, a)?));
                    }
                }
                Ok(Value::Nil)
            })?,
        )?;
    }

    // GetAbilityById
    {
        let s = snapshot_cell.clone();
        list_mod.set(
            "GetAbilityById",
            lua.create_function(move |lua_ctx, id: u32| {
                let snap = s.get().abilities;
                for i in 0..snap.ability_count as usize {
                    let a = &snap.abilities[i];
                    if a.valid && a.id == id {
                        return Ok(Value::Table(create_ability_object(lua_ctx, a)?));
                    }
                }
                Ok(Value::Nil)
            })?,
        )?;
    }

    // GetAbilityByDisplayIndex
    {
        let s = snapshot_cell.clone();
        list_mod.set(
            "GetAbilityByDisplayIndex",
            lua.create_function(move |lua_ctx, display_index: u32| {
                let snap = s.get().abilities;
                for i in 0..snap.ability_count as usize {
                    let a = &snap.abilities[i];
                    if a.valid && a.display_index == display_index {
                        return Ok(Value::Table(create_ability_object(lua_ctx, a)?));
                    }
                }
                Ok(Value::Nil)
            })?,
        )?;
    }

    // Abilities iterator
    {
        let s = snapshot_cell.clone();
        list_mod.set(
            "Abilities",
            lua.create_function(move |lua_ctx, ()| {
                let s_iter = s.clone();
                let cursor = Rc::new(RefCell::new(1usize));
                lua_ctx.create_function(move |lua_ctx, ()| {
                    let mut cur = cursor.borrow_mut();
                    let snap = s_iter.get().abilities;
                    while *cur <= snap.ability_count as usize {
                        let idx = *cur;
                        *cur += 1;
                        let a = &snap.abilities[idx - 1];
                        if a.valid {
                            return Ok((
                                Value::Integer(idx as i64),
                                Value::Table(create_ability_object(lua_ctx, a)?),
                            ));
                        }
                    }
                    Ok((Value::Nil, Value::Nil))
                })
            })?,
        )?;
    }

    // Hook lifecycle and ownership
    {
        let claims = hook_claims.clone();
        let s = snapshot_cell.clone();
        list_mod.set(
            "IsHookInstalled",
            lua.create_function(move |_, ()| Ok(!claims.borrow().is_empty() || s.get().in_game))?,
        )?;
    }
    list_mod.set("InstallHook", lua.create_function(|_, ()| Ok(true))?)?;
    list_mod.set("UninstallHook", lua.create_function(|_, ()| Ok(true))?)?;
    list_mod.set(
        "GetHookInfo",
        lua.create_function(|lua_ctx, ()| {
            let t = lua_ctx.create_table()?;
            t.set("installed", true)?;
            t.set("dropped", 0)?;
            t.set("update_count", 0)?;
            Ok(t)
        })?,
    )?;
    list_mod.set("GetDroppedCount", lua.create_function(|_, ()| Ok(0))?)?;

    // Acquire
    {
        let claims = hook_claims.clone();
        list_mod.set(
            "Acquire",
            lua.create_function(move |_, owner: String| {
                claims.borrow_mut().insert(owner);
                Ok(true)
            })?,
        )?;
    }

    // Release
    {
        let claims = hook_claims.clone();
        list_mod.set(
            "Release",
            lua.create_function(move |_, owner: String| {
                claims.borrow_mut().remove(&owner);
                Ok(())
            })?,
        )?;
    }

    // GetOwnerCount
    {
        let claims = hook_claims.clone();
        list_mod.set(
            "GetOwnerCount",
            lua.create_function(move |_, ()| Ok(claims.borrow().len()))?,
        )?;
    }

    // HasClaim
    {
        let claims = hook_claims.clone();
        list_mod.set(
            "HasClaim",
            lua.create_function(move |_, owner: String| Ok(claims.borrow().contains(&owner)))?,
        )?;
    }

    // IsHookSuspended
    {
        let claims = hook_claims.clone();
        let s = snapshot_cell.clone();
        list_mod.set(
            "IsHookSuspended",
            lua.create_function(move |_, ()| Ok(!s.get().in_game && !claims.borrow().is_empty()))?,
        )?;
    }

    // Event bus: On, Off, GetSubscriberErrors, Update
    {
        let subs = subscribers.clone();
        list_mod.set(
            "On",
            lua.create_function(
                move |_, (owner, event_name, func): (String, String, mlua::Function)| {
                    if event_name != "OnLockoutStart" && event_name != "OnLockoutEnd" {
                        return Ok((false, Some(format!("unknown ability event: {event_name}"))));
                    }
                    let mut map = subs.borrow_mut();
                    map.entry(event_name).or_default().insert(owner, func);
                    Ok((true, None::<String>))
                },
            )?,
        )?;
    }

    {
        let subs = subscribers.clone();
        list_mod.set(
            "Off",
            lua.create_function(move |_, (owner, event_name): (String, Option<String>)| {
                let mut map = subs.borrow_mut();
                if let Some(ev) = event_name {
                    if let Some(handlers) = map.get_mut(&ev) {
                        handlers.remove(&owner);
                    }
                } else {
                    for handlers in map.values_mut() {
                        handlers.remove(&owner);
                    }
                }
                Ok(())
            })?,
        )?;
    }

    {
        let errs = subscriber_errors.clone();
        list_mod.set(
            "GetSubscriberErrors",
            lua.create_function(move |lua_ctx, ()| {
                let list = errs.borrow();
                let tbl = lua_ctx.create_table()?;
                for (i, err) in list.iter().enumerate() {
                    tbl.set(i + 1, err.clone())?;
                }
                Ok(tbl)
            })?,
        )?;
    }

    {
        let s = snapshot_cell.clone();
        let cache = lockout_cache.clone();
        let subs = subscribers.clone();
        let errs = subscriber_errors.clone();
        list_mod.set(
            "Update",
            lua.create_function(move |lua_ctx, ()| {
                let snap = s.get().abilities;
                let mut cache_map = cache.borrow_mut();

                for i in 0..snap.ability_count as usize {
                    let a = &snap.abilities[i];
                    if !a.valid {
                        continue;
                    }
                    let d = a.display_index;
                    let current = a.cooldown_lockout_ms;
                    let prev = cache_map.get(&d).copied();

                    match prev {
                        None => {
                            cache_map.insert(d, current);
                        }
                        Some(0) if current > 0 => {
                            cache_map.insert(d, current);
                            // Fire OnLockoutStart
                            let subs_map = subs.borrow();
                            if let Some(handlers) = subs_map.get("OnLockoutStart") {
                                let mut dead_owners = Vec::new();
                                for (owner, fn_val) in handlers.iter() {
                                    let payload = lua_ctx.create_table()?;
                                    payload.set("display_index", d)?;
                                    payload.set("lockout_ms", current)?;
                                    payload.set("seq", 0)?;
                                    if let Err(e) = fn_val.call::<Value>(payload) {
                                        dead_owners.push(owner.clone());
                                        let err_tbl = lua_ctx.create_table()?;
                                        err_tbl.set("owner", owner.as_str())?;
                                        err_tbl.set("event", "OnLockoutStart")?;
                                        err_tbl.set("error", e.to_string())?;
                                        errs.borrow_mut().push(err_tbl);
                                    }
                                }
                                drop(subs_map);
                                if !dead_owners.is_empty() {
                                    let mut subs_mut = subs.borrow_mut();
                                    if let Some(handlers_mut) = subs_mut.get_mut("OnLockoutStart") {
                                        for dead in dead_owners {
                                            handlers_mut.remove(&dead);
                                        }
                                    }
                                }
                            }
                        }
                        Some(p) if p > 0 && current == 0 => {
                            cache_map.insert(d, 0);
                            // Fire OnLockoutEnd
                            let subs_map = subs.borrow();
                            if let Some(handlers) = subs_map.get("OnLockoutEnd") {
                                let mut dead_owners = Vec::new();
                                for (owner, fn_val) in handlers.iter() {
                                    let payload = lua_ctx.create_table()?;
                                    payload.set("display_index", d)?;
                                    payload.set("lockout_ms", 0)?;
                                    payload.set("seq", 0)?;
                                    if let Err(e) = fn_val.call::<Value>(payload) {
                                        dead_owners.push(owner.clone());
                                        let err_tbl = lua_ctx.create_table()?;
                                        err_tbl.set("owner", owner.as_str())?;
                                        err_tbl.set("event", "OnLockoutEnd")?;
                                        err_tbl.set("error", e.to_string())?;
                                        errs.borrow_mut().push(err_tbl);
                                    }
                                }
                                drop(subs_map);
                                if !dead_owners.is_empty() {
                                    let mut subs_mut = subs.borrow_mut();
                                    if let Some(handlers_mut) = subs_mut.get_mut("OnLockoutEnd") {
                                        for dead in dead_owners {
                                            handlers_mut.remove(&dead);
                                        }
                                    }
                                }
                            }
                        }
                        Some(_) => {
                            if prev != Some(current) {
                                cache_map.insert(d, current);
                            }
                        }
                    }
                }
                Ok(())
            })?,
        )?;
    }

    host.register_host_module("frontiers_forge.ability_list", Value::Table(list_mod))?;
    Ok(())
}

pub fn install_ability_bar_adapter(host: &Host) -> LuaResult<()> {
    let lua = &host.lua;
    let bar_mod = lua.create_table()?;

    bar_mod.set("num_bars", MAX_BARS)?;
    bar_mod.set("slot_size", 0x20)?;

    let snapshot_cell = host.snapshot.clone();

    // GetSelectedBarIndex
    {
        let s = snapshot_cell.clone();
        bar_mod.set(
            "GetSelectedBarIndex",
            lua.create_function(move |_, ()| {
                let snap = s.get();
                if snap.in_game {
                    Ok(Value::Integer(snap.abilities.selected_bar as i64))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }

    // GetSelectedSlotIndex(bar_index)
    {
        let s = snapshot_cell.clone();
        bar_mod.set(
            "GetSelectedSlotIndex",
            lua.create_function(move |_, bar_index: usize| {
                let snap = s.get();
                if !snap.in_game {
                    return Ok(Value::Nil);
                }
                if bar_index >= MAX_BARS {
                    return Err(mlua::Error::runtime(format!(
                        "Invalid bar index: {bar_index}"
                    )));
                }
                Ok(Value::Integer(
                    snap.abilities.bars[bar_index].selected_slot as i64,
                ))
            })?,
        )?;
    }

    // GetSlotCount(bar_index)
    {
        let s = snapshot_cell.clone();
        bar_mod.set(
            "GetSlotCount",
            lua.create_function(move |_, bar_index: usize| {
                let snap = s.get();
                if !snap.in_game {
                    return Ok(0);
                }
                if bar_index >= MAX_BARS {
                    return Err(mlua::Error::runtime(format!(
                        "Invalid bar index: {bar_index}"
                    )));
                }
                Ok(snap.abilities.bars[bar_index].slot_count as i64)
            })?,
        )?;
    }

    // GetSlotUITexId(slot_index)
    bar_mod.set(
        "GetSlotUITexId",
        lua.create_function(|_, slot_index: usize| {
            // FrontiersForge compact HUD slot glyphs: slot 0..1 potion (5), slot 2..3 gem (3)
            const STATIC_GLYPHS: [i32; 5] = [5, 5, 3, 2, 0];
            if slot_index >= 5 {
                return Err(mlua::Error::runtime(format!(
                    "Invalid slot index: {slot_index}"
                )));
            }
            Ok(STATIC_GLYPHS[slot_index])
        })?,
    )?;

    // Helper: create AbilityBarSlot object
    let make_slot_object = |lua: &Lua,
                            slot: AbilityBarSlotSnapshot,
                            snap_abilities: &AbilitiesSnapshot|
     -> LuaResult<Table> {
        let t = lua.create_table()?;
        let is_empty = slot.source_type == 0;
        t.set("IsEmpty", lua.create_function(move |_, ()| Ok(is_empty))?)?;

        let source_type = slot.source_type;
        let source_index = slot.source_index;
        t.set(
            "GetAbilityIndex",
            lua.create_function(move |_, ()| {
                if source_type == ABILITY_SOURCE_TAG {
                    Ok(Value::Integer(source_index as i64))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;

        let ability_snap = if source_type == ABILITY_SOURCE_TAG
            && source_index >= 1
            && (source_index as usize) <= snap_abilities.ability_count as usize
        {
            let a = snap_abilities.abilities[source_index as usize - 1];
            Some(a)
        } else {
            None
        };
        t.set(
            "GetAbility",
            lua.create_function(move |lua_ctx, ()| {
                if let Some(a) = ability_snap {
                    Ok(Value::Table(create_ability_object(lua_ctx, &a)?))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;

        t.set(
            "GetItemIndex",
            lua.create_function(move |_, ()| {
                if source_type == ITEM_SOURCE_TAG {
                    Ok(Value::Integer(source_index as i64))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;

        let item_snap = if source_type == ITEM_SOURCE_TAG && (source_index as usize) < snap_abilities.items.len() {
            let it = snap_abilities.items[source_index as usize];
            if it.populated && it.slot_id != 0 {
                Some(it)
            } else {
                None
            }
        } else {
            None
        };
        t.set(
            "GetItem",
            lua.create_function(move |lua_ctx, ()| {
                if let Some(ref it) = item_snap {
                    Ok(Value::Table(create_item_object(lua_ctx, it)?))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;

        let icon_ref = slot.ability_icon_ref;
        t.set(
            "GetIconRef",
            lua.create_function(move |_, ()| Ok(icon_ref))?,
        )?;

        Ok(t)
    };

    // GetAbilitySlot(bar_index, slot_index)
    {
        let s = snapshot_cell.clone();
        bar_mod.set(
            "GetAbilitySlot",
            lua.create_function(move |lua_ctx, (bar_index, slot_index): (usize, usize)| {
                let snap = s.get();
                if !snap.in_game {
                    return Ok(Value::Nil);
                }
                if bar_index >= MAX_BARS {
                    return Err(mlua::Error::runtime(format!(
                        "Invalid bar index: {bar_index}"
                    )));
                }
                let bar = &snap.abilities.bars[bar_index];
                if slot_index >= bar.slot_count as usize || slot_index >= MAX_BAR_SLOTS {
                    return Err(mlua::Error::runtime(format!(
                        "Invalid slot index: {slot_index}"
                    )));
                }
                let slot = bar.slots[slot_index];
                Ok(Value::Table(make_slot_object(
                    lua_ctx,
                    slot,
                    &snap.abilities,
                )?))
            })?,
        )?;
    }

    // GetAbility(bar_index, slot_index)
    {
        let s = snapshot_cell.clone();
        bar_mod.set(
            "GetAbility",
            lua.create_function(move |lua_ctx, (bar_index, slot_index): (usize, usize)| {
                let snap = s.get();
                if !snap.in_game || bar_index >= MAX_BARS {
                    return Ok(Value::Nil);
                }
                let bar = &snap.abilities.bars[bar_index];
                if slot_index >= bar.slot_count as usize || slot_index >= MAX_BAR_SLOTS {
                    return Ok(Value::Nil);
                }
                let slot = bar.slots[slot_index];
                if slot.source_type == ABILITY_SOURCE_TAG
                    && slot.source_index >= 1
                    && (slot.source_index as usize) <= snap.abilities.ability_count as usize
                {
                    let a = snap.abilities.abilities[slot.source_index as usize - 1];
                    return Ok(Value::Table(create_ability_object(lua_ctx, &a)?));
                }
                Ok(Value::Nil)
            })?,
        )?;
    }

    // GetItem(bar_index, slot_index)
    {
        let s = snapshot_cell.clone();
        bar_mod.set(
            "GetItem",
            lua.create_function(move |lua_ctx, (bar_index, slot_index): (usize, usize)| {
                let snap = s.get();
                if !snap.in_game || bar_index >= MAX_BARS {
                    return Ok(Value::Nil);
                }
                let bar = &snap.abilities.bars[bar_index];
                if slot_index >= bar.slot_count as usize || slot_index >= MAX_BAR_SLOTS {
                    return Ok(Value::Nil);
                }
                let slot = bar.slots[slot_index];
                if slot.source_type == ITEM_SOURCE_TAG && (slot.source_index as usize) < snap.abilities.items.len() {
                    let it = &snap.abilities.items[slot.source_index as usize];
                    if it.populated && it.slot_id != 0 {
                        return Ok(Value::Table(create_item_object(lua_ctx, it)?));
                    }
                }
                Ok(Value::Nil)
            })?,
        )?;
    }

    host.register_host_module("frontiers_forge.ability_bar", Value::Table(bar_mod))?;
    Ok(())
}

pub fn install_icon_adapter(host: &Host) -> LuaResult<()> {
    super::icons::install_base_icon_adapter(&host.lua, host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::discovery::DiscoveredAddon;
    use crate::fork_ext::addons::host::Snapshot;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_test_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("eqoa_ability_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_addon(root: &Path) -> DiscoveredAddon {
        let entry = root.join("main.lua");
        fs::write(&entry, "-- test ability addon").unwrap();
        DiscoveredAddon {
            id: "test_ability".to_string(),
            name: "Test Ability".to_string(),
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
    fn forge_ability_modules_registration() {
        let dir = temp_test_dir("registration");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();
        let chunk = host.lua.load(
            r#"
            local Ability = require("frontiers_forge.ability")
            local AbilityList = require("frontiers_forge.ability_list")
            local AbilityBar = require("frontiers_forge.ability_bar")
            local Icon = require("frontiers_forge.icon")

            assert(type(Ability.Scope) == "table", "Scope must be table")
            assert(Ability.Scope.SELF == 0, "Scope.SELF must be 0")
            assert(Ability.Scope.TARGET == 1, "Scope.TARGET must be 1")
            assert(Ability.size == 0x1D8, "Ability.size must be 0x1D8")

            assert(type(AbilityList.GetCount) == "function", "GetCount must be func")
            assert(type(AbilityList.Abilities) == "function", "Abilities must be func")
            assert(type(AbilityList.On) == "function", "On must be func")
            assert(type(AbilityList.Update) == "function", "Update must be func")

            assert(AbilityBar.num_bars == 3, "num_bars must be 3")
            assert(type(AbilityBar.GetSelectedBarIndex) == "function", "GetSelectedBarIndex must be func")
            assert(type(AbilityBar.GetAbility) == "function", "GetAbility must be func")

            assert(type(Icon.Acquire) == "function", "Acquire must be func")
            assert(type(Icon.GetTexture) == "function", "GetTexture must be func")
            "#,
        );
        chunk.exec().unwrap();
    }

    #[test]
    fn forge_ability_bar_queries_and_slots() {
        let dir = temp_test_dir("bar_queries");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut name_bytes = [0u8; 32];
        name_bytes[..4].copy_from_slice(b"Heal");

        let mut desc_bytes = [0u8; 64];
        desc_bytes[..12].copy_from_slice(b"Restores HP.");

        let mut snap = AbilitiesSnapshot::default();
        snap.ability_count = 1;
        snap.selected_bar = 1;
        snap.abilities[0] = AbilitySnapshot {
            id: 101,
            display_index: 0,
            spellbook_slot: 0,
            level: 5,
            range: 50.0,
            cast_time: 2,
            power_cost: 15,
            icon_background_ref: 1001,
            icon_foreground_ref: 2001,
            scope: 1, // TARGET
            cooldown_seconds: 6,
            cooldown_lockout_ms: 0,
            equip_requirements: 0,
            name: name_bytes,
            name_len: 4,
            description: desc_bytes,
            desc_len: 12,
            valid: true,
        };

        // Bar 1 slot 0 points to ability index 1
        snap.bars[1].slot_count = 5;
        snap.bars[1].selected_slot = 0;
        snap.bars[1].slots[0] = AbilityBarSlotSnapshot {
            source_type: ABILITY_SOURCE_TAG,
            source_index: 1,
            ability_icon_ref: 2001,
        };

        // Bar 2 slot 1 points to item index 3
        snap.bars[2].slot_count = 4;
        snap.bars[2].selected_slot = 1;
        snap.bars[2].slots[1] = AbilityBarSlotSnapshot {
            source_type: ITEM_SOURCE_TAG,
            source_index: 3,
            ability_icon_ref: 500,
        };
        let mut item_name = [0u8; 32];
        item_name[..4].copy_from_slice(b"Item");
        snap.items[3] = ItemSnapshot {
            name: item_name,
            name_len: 4,
            description: [0u8; 64],
            desc_len: 0,
            icon_ref: 500,
            slot_id: 1,
            populated: true,
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            abilities: snap,
            ..Default::default()
        });

        let chunk = host.lua.load(
            r#"
            local AbilityBar = require("frontiers_forge.ability_bar")
            assert(AbilityBar.GetSelectedBarIndex() == 1, "Selected bar must be 1")
            assert(AbilityBar.GetSelectedSlotIndex(1) == 0, "Selected slot on bar 1 must be 0")
            assert(AbilityBar.GetSlotCount(1) == 5, "Slot count on bar 1 must be 5")
            assert(AbilityBar.GetSlotCount(2) == 4, "Slot count on bar 2 must be 4")
            assert(AbilityBar.GetSlotUITexId(0) == 5, "Glyph for slot 0 must be 5")
            assert(AbilityBar.GetSlotUITexId(3) == 2, "Glyph for slot 3 must be 2")

            -- Ability slot on bar 1 slot 0
            local slot0 = AbilityBar.GetAbilitySlot(1, 0)
            assert(slot0 ~= nil, "slot0 must not be nil")
            assert(slot0:IsEmpty() == false, "slot0 must not be empty")
            assert(slot0:GetAbilityIndex() == 1, "ability index must be 1")
            assert(slot0:GetIconRef() == 2001, "icon ref must be 2001")

            local ab = AbilityBar.GetAbility(1, 0)
            assert(ab ~= nil, "ab must not be nil")
            assert(ab:GetName() == "Heal", "name must be Heal")
            assert(ab:GetCastTime() == 2, "cast time must be 2")
            assert(ab:GetPwrCost() == 15, "power cost must be 15")
            assert(ab:IsOnCooldown() == false, "should not be on cooldown")

            -- Item slot on bar 2 slot 1
            local item_slot = AbilityBar.GetAbilitySlot(2, 1)
            assert(item_slot:IsEmpty() == false, "item slot must not be empty")
            assert(item_slot:GetItemIndex() == 3, "item index must be 3")
            local item = AbilityBar.GetItem(2, 1)
            assert(item ~= nil, "item must not be nil")
            assert(item:GetName() == "Item", "item name must be Item")
            assert(item:GetIconRef() == 500, "item icon must be 500")
            "#,
        );
        chunk.exec().unwrap();
    }

    #[test]
    fn forge_ability_bar_item_slots_and_stock_glyphs() {
        let dir = temp_test_dir("item_slots_test");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = AbilitiesSnapshot::default();
        snap.bars[2].slot_count = 4;
        snap.bars[2].slots[0] = AbilityBarSlotSnapshot {
            source_type: ITEM_SOURCE_TAG,
            source_index: 0,
            ability_icon_ref: 0x12345678,
        };
        snap.bars[2].slots[1] = AbilityBarSlotSnapshot {
            source_type: 0,
            source_index: 0,
            ability_icon_ref: -1,
        };

        let mut item_name = [0u8; 32];
        item_name[..17].copy_from_slice(b"Healing Potion 01");
        let mut item_desc = [0u8; 64];
        item_desc[..12].copy_from_slice(b"Heals 100 HP");

        snap.items[0] = ItemSnapshot {
            name: item_name,
            name_len: 17,
            description: item_desc,
            desc_len: 12,
            icon_ref: 0x12345678,
            slot_id: 1,
            populated: true,
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            abilities: snap,
            ..Default::default()
        });

        let chunk = host.lua.load(
            r#"
            local AbilityBar = require("frontiers_forge.ability_bar")
            local item = AbilityBar.GetItem(2, 0)
            assert(item ~= nil, "item on bar 2 slot 0 must not be nil")
            assert(item:GetName() == "Healing Potion 01", "name must match")
            assert(item:GetDescription() == "Heals 100 HP", "desc must match")
            assert(item:GetIconRef() == 0x12345678, "icon_ref must match")
            assert(item:GetSlotId() == 1, "slot_id must match")

            local slot0 = AbilityBar.GetAbilitySlot(2, 0)
            assert(slot0:IsEmpty() == false, "slot 0 must not be empty")
            assert(slot0:GetItemIndex() == 0, "item index must be 0")
            assert(slot0:GetIconRef() == 0x12345678, "slot icon ref must match")

            -- Negative control: unassigned slot returns nil for GetItem
            local empty_item = AbilityBar.GetItem(2, 1)
            assert(empty_item == nil, "unassigned quickslot must return nil")

            -- Verify stock compact glyph IDs: slots 0..1 are 5 (potion), slots 2..3 are 3/2
            assert(AbilityBar.GetSlotUITexId(0) == 5, "glyph 0 must be 5")
            assert(AbilityBar.GetSlotUITexId(1) == 5, "glyph 1 must be 5")
            assert(AbilityBar.GetSlotUITexId(2) == 3, "glyph 2 must be 3")
            assert(AbilityBar.GetSlotUITexId(3) == 2, "glyph 3 must be 2")
            "#,
        );
        chunk.exec().unwrap();
    }

    #[test]
    fn forge_abilities_spellbook_capacity_above_64() {
        let dir = temp_test_dir("large_spellbook_test");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = AbilitiesSnapshot::default();
        snap.ability_count = 100;

        let mut name = [0u8; 32];
        name[..15].copy_from_slice(b"High Rank Spell");
        snap.abilities[95] = AbilitySnapshot {
            id: 9999,
            display_index: 95,
            spellbook_slot: 95,
            level: 50,
            range: 100.0,
            cast_time: 3,
            power_cost: 50,
            icon_background_ref: 0x11112222,
            icon_foreground_ref: 0x33334444,
            scope: 1,
            cooldown_seconds: 10,
            cooldown_lockout_ms: 0,
            equip_requirements: 0,
            name,
            name_len: 15,
            description: [0u8; 64],
            desc_len: 0,
            valid: true,
        };

        snap.bars[0].slot_count = 5;
        snap.bars[0].slots[0] = AbilityBarSlotSnapshot {
            source_type: ABILITY_SOURCE_TAG,
            source_index: 96,
            ability_icon_ref: 0x33334444,
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            abilities: snap,
            ..Default::default()
        });

        let chunk = host.lua.load(
            r#"
            local AbilityBar = require("frontiers_forge.ability_bar")
            local ab = AbilityBar.GetAbility(0, 0)
            assert(ab ~= nil, "ability past index 64 must not be dropped")
            assert(ab:GetName() == "High Rank Spell", "name must match")
            assert(ab:GetIconForegroundRef() == 0x33334444, "fg ref must match")
            assert(ab:GetIconBackgroundRef() == 0x11112222, "bg ref must match")
            "#,
        );
        chunk.exec().unwrap();
    }

    #[test]
    fn forge_ability_list_iteration_and_queries() {
        let dir = temp_test_dir("list_queries");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut name1 = [0u8; 32];
        name1[..5].copy_from_slice(b"Smite");
        let mut name2 = [0u8; 32];
        name2[..4].copy_from_slice(b"Ward");

        let mut snap = AbilitiesSnapshot::default();
        snap.ability_count = 2;
        snap.abilities[0] = AbilitySnapshot {
            id: 201,
            display_index: 0,
            name: name1,
            name_len: 5,
            range: 40.0,
            scope: 1, // TARGET
            valid: true,
            ..Default::default()
        };
        snap.abilities[1] = AbilitySnapshot {
            id: 202,
            display_index: 1,
            name: name2,
            name_len: 4,
            range: 0.0,
            scope: 0, // SELF
            valid: true,
            ..Default::default()
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            abilities: snap,
            ..Default::default()
        });

        let chunk = host.lua.load(
            r#"
            local AbilityList = require("frontiers_forge.ability_list")
            assert(AbilityList.GetCount() == 2, "count must be 2")

            local ab1 = AbilityList.GetAbilityById(201)
            assert(ab1 ~= nil and ab1:GetName() == "Smite", "ab1 must be Smite")

            local ab2 = AbilityList.GetAbilityByDisplayIndex(1)
            assert(ab2 ~= nil and ab2:GetName() == "Ward", "ab2 must be Ward")

            -- Test IsInRange
            local p_from = { x = 0, y = 0, z = 0 }
            local p_close = { x = 10, y = 0, z = 0 }
            local p_far = { x = 100, y = 0, z = 0 }

            local in_range, dist = ab1:IsInRange(p_from, p_close)
            assert(in_range == true, "Smite should be in range")
            assert(math.abs(dist - 10) < 0.001, "dist should be 10")

            local in_range2, dist2 = ab1:IsInRange(p_from, p_far)
            assert(in_range2 == false, "Smite should be out of range")
            assert(math.abs(dist2 - 100) < 0.001, "dist should be 100")

            local in_range_self, _ = ab2:IsInRange(p_from, p_far)
            assert(in_range_self == true, "Self scope should always be in range")

            -- Test Abilities() iterator
            local count = 0
            for idx, ability in AbilityList.Abilities() do
                count = count + 1
                assert(idx == count, "idx matches count")
                assert(ability:IsValid(), "ability must be valid")
            end
            assert(count == 2, "iterated 2 abilities")
            "#,
        );
        chunk.exec().unwrap();
    }

    #[test]
    fn forge_ability_lockout_events_and_claims() {
        let dir = temp_test_dir("lockout_events");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = AbilitiesSnapshot::default();
        snap.ability_count = 1;
        snap.abilities[0] = AbilitySnapshot {
            id: 301,
            display_index: 4,
            valid: true,
            cooldown_lockout_ms: 0,
            ..Default::default()
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            abilities: snap,
            ..Default::default()
        });

        let chunk = host.lua.load(
            r#"
            local AbilityList = require("frontiers_forge.ability_list")
            assert(AbilityList.Acquire("test_owner") == true, "Acquire must succeed")
            assert(AbilityList.HasClaim("test_owner") == true, "HasClaim must be true")
            assert(AbilityList.GetOwnerCount() == 1, "Owner count must be 1")

            _G.start_events = {}
            _G.end_events = {}

            AbilityList.On("test_owner", AbilityList.Events.OnLockoutStart, function(event)
                table.insert(_G.start_events, event)
            end)

            AbilityList.On("test_owner", AbilityList.Events.OnLockoutEnd, function(event)
                table.insert(_G.end_events, event)
            end)

            -- Initial update: seeds cache
            AbilityList.Update()
            assert(#_G.start_events == 0, "No events on initial seed")
            "#,
        );
        chunk.exec().unwrap();

        // Mutate snapshot: ability goes on cooldown (lockout = 3500 ms)
        let mut snap2 = snap;
        snap2.abilities[0].cooldown_lockout_ms = 3500;
        host.snapshot.set(Snapshot {
            in_game: true,
            abilities: snap2,
            ..Default::default()
        });

        let chunk2 = host.lua.load(
            r#"
            local AbilityList = require("frontiers_forge.ability_list")
            AbilityList.Update()
            assert(#_G.start_events == 1, "One start event should fire")
            assert(_G.start_events[1].display_index == 4, "display_index must be 4")
            assert(_G.start_events[1].lockout_ms == 3500, "lockout_ms must be 3500")
            assert(#_G.end_events == 0, "No end events yet")
            "#,
        );
        chunk2.exec().unwrap();

        // Mutate snapshot: ability comes off cooldown (lockout = 0)
        let mut snap3 = snap2;
        snap3.abilities[0].cooldown_lockout_ms = 0;
        host.snapshot.set(Snapshot {
            in_game: true,
            abilities: snap3,
            ..Default::default()
        });

        let chunk3 = host.lua.load(
            r#"
            local AbilityList = require("frontiers_forge.ability_list")
            AbilityList.Update()
            assert(#_G.start_events == 1, "Still one start event")
            assert(#_G.end_events == 1, "One end event should fire")
            assert(_G.end_events[1].display_index == 4, "display_index must be 4")
            assert(_G.end_events[1].lockout_ms == 0, "lockout_ms must be 0")

            AbilityList.Release("test_owner")
            assert(AbilityList.HasClaim("test_owner") == false, "HasClaim must be false after Release")
            assert(AbilityList.GetOwnerCount() == 0, "Owner count must be 0")
            "#,
        );
        chunk3.exec().unwrap();
    }

    #[test]
    fn forge_icon_adapter_lifecycle() {
        let dir = temp_test_dir("icon_adapter");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let chunk = host.lua.load(
            r#"
            local Icon = require("frontiers_forge.icon")
            assert(Icon.Acquire("wereoxxs_ui") == true, "Icon.Acquire must return true")
            assert(Icon.HasClaim("wereoxxs_ui") == true, "Icon.HasClaim must return true")
            assert(Icon.GetOwnerCount() == 1, "Owner count must be 1")

            local tex, w, h = Icon.GetTexture(12345, { trim_transparent = true })
            assert(tex == nil and w == nil and h == nil, "GetTexture returns nil tuple when not decoded")

            local ui_tex, uw, uh = Icon.GetUITexture(5)
            assert(ui_tex == nil, "GetUITexture returns nil when not decoded")

            assert(Icon.GetSurfaceInfo(12345) == nil, "GetSurfaceInfo returns nil")

            Icon.Release("wereoxxs_ui")
            assert(Icon.HasClaim("wereoxxs_ui") == false, "HasClaim must be false after release")
            assert(Icon.GetOwnerCount() == 0, "Owner count must be 0")
            "#,
        );
        chunk.exec().unwrap();
    }

    #[test]
    fn wereoxxs_ui_hotbar_cast_pattern() {
        let dir = temp_test_dir("wereoxxs_pattern");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut name = [0u8; 32];
        name[..4].copy_from_slice(b"Bolt");
        let mut snap = AbilitiesSnapshot::default();
        snap.ability_count = 1;
        snap.abilities[0] = AbilitySnapshot {
            id: 401,
            display_index: 2,
            cast_time: 3,
            power_cost: 20,
            range: 60.0,
            icon_background_ref: 111,
            icon_foreground_ref: 222,
            valid: true,
            name,
            name_len: 4,
            cooldown_lockout_ms: 0,
            ..Default::default()
        };
        snap.selected_bar = 0;
        snap.bars[0].slot_count = 5;
        snap.bars[0].slots[0] = AbilityBarSlotSnapshot {
            source_type: ABILITY_SOURCE_TAG,
            source_index: 1,
            ability_icon_ref: 222,
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            abilities: snap,
            ..Default::default()
        });

        // Test the exact Wereoxxs UI cast / hotbar slot extraction pattern
        let chunk = host.lua.load(
            r#"
            local AbilityBar = require("frontiers_forge.ability_bar")
            local AbilityList = require("frontiers_forge.ability_list")
            local Icon = require("frontiers_forge.icon")

            Icon.Acquire("wereoxxs_ui")
            AbilityList.Acquire("wereoxxs_ui")

            _G.casting_started = false
            AbilityList.On("wereoxxs_ui", AbilityList.Events.OnLockoutStart, function(event)
                local ability = AbilityList.GetAbilityByDisplayIndex(event.display_index)
                local cast_time = ability ~= nil and (ability:GetCastTime() or 0) or 0
                if ability ~= nil and cast_time > 0 then
                    _G.casting_started = true
                end
            end)

            AbilityList.Update()

            -- Build slot
            local ab = AbilityBar.GetAbility(0, 0)
            assert(ab ~= nil, "Ability must resolve on bar 0 slot 0")
            local name = ab:GetName()
            assert(name == "Bolt", "Name must be Bolt")

            local fg, fg_w, fg_h = Icon.GetTexture(ab:GetIconForegroundRef(), { trim_transparent = true, trim_color = true })
            local bg, bg_w, bg_h = Icon.GetTexture(ab:GetIconBackgroundRef(), { trim_transparent = true, trim_color = true })
            -- In Wereoxxs UI: if fg ~= nil then draw_list:AddImage(fg, ...) end
            -- nil check is safe

            local tooltip = string.format("%s\nPower %d", name, ab:GetPwrCost() or 0)
            assert(tooltip == "Bolt\nPower 20", "Tooltip must format correctly")
            "#,
        );
        chunk.exec().unwrap();

        // Now trigger lockout start
        let mut snap2 = snap;
        snap2.abilities[0].cooldown_lockout_ms = 4000;
        host.snapshot.set(Snapshot {
            in_game: true,
            abilities: snap2,
            ..Default::default()
        });

        let chunk2 = host.lua.load(
            r#"
            local AbilityList = require("frontiers_forge.ability_list")
            AbilityList.Update()
            assert(_G.casting_started == true, "Casting start detected via OnLockoutStart")
            "#,
        );
        chunk2.exec().unwrap();
    }
}
