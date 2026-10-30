//! Native FrontiersForge player adapter module (`frontiers_forge.player`).
//!
//! Authoritative Contracts:
//! - `docs/forge-handoff/2026-10-29/player-next-contract.md`
//! - Upstream pinned reference: `scripts/modules/frontiers_forge/player.lua`
//!   and `wereoxxs_ui`.
//!
//! Scope & Invariant Guarantees:
//! - Provides full native implementation of `frontiers_forge.player` module table.
//! - Read-only access to engine player state via `host.snapshot`.
//! - Safe numeric conversions, zero/nil handling for uninitialized/disconnected states.
//! - Pure functions for speed and heading trigonometry matching authentic EE coordinates.

use super::host::Host;
use mlua::{Result as LuaResult, Value};
use std::f32::consts::PI;

/// Fixed-size, Copy snapshot of player state for the active frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerSnapshot {
    pub name: [u8; 24],
    pub name_len: u8,
    pub level: u32,
    pub exp: u32,
    pub exp_debt: u32,
    pub current_hp: u32,
    pub max_hp: u32,
    pub base_hp: u32,
    pub current_pwr: u32,
    pub max_pwr: u32,
    pub base_pwr: u32,
    pub total_str: u32,
    pub total_sta: u32,
    pub total_agi: u32,
    pub total_dex: u32,
    pub total_wis: u32,
    pub total_int: u32,
    pub total_cha: u32,
    pub base_str: u32,
    pub base_sta: u32,
    pub base_agi: u32,
    pub base_dex: u32,
    pub base_wis: u32,
    pub base_int: u32,
    pub base_cha: u32,
    pub ac: u32,
    pub poison_resist: u32,
    pub disease_resist: u32,
    pub fire_resist: u32,
    pub cold_resist: u32,
    pub lightning_resist: u32,
    pub arcane_resist: u32,
    pub cms: u32,
    pub cms_spent: u32,
    pub cm_pct: u32,
    pub position: [f32; 3], // x, y, z
    pub velocity: [f32; 3], // vx, vy, vz
    pub heading_rad: f32,
    pub entity_id: u32,
    pub target_entity_id: u32, // 0 if no target
    pub pet_entity_id: Option<u32>,
    pub is_auto_attacking: bool,
    pub is_dead: bool,
    pub class_id: u32,
    pub race_id: u32,
    pub location: PlayerLocationSnapshot,
    pub available: bool,
}

impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            name: [0u8; 24],
            name_len: 0,
            level: 0,
            exp: 0,
            exp_debt: 0,
            current_hp: 0,
            max_hp: 0,
            base_hp: 0,
            current_pwr: 0,
            max_pwr: 0,
            base_pwr: 0,
            total_str: 0,
            total_sta: 0,
            total_agi: 0,
            total_dex: 0,
            total_wis: 0,
            total_int: 0,
            total_cha: 0,
            base_str: 0,
            base_sta: 0,
            base_agi: 0,
            base_dex: 0,
            base_wis: 0,
            base_int: 0,
            base_cha: 0,
            ac: 0,
            poison_resist: 0,
            disease_resist: 0,
            fire_resist: 0,
            cold_resist: 0,
            lightning_resist: 0,
            arcane_resist: 0,
            cms: 0,
            cms_spent: 0,
            cm_pct: 0,
            position: [0.0; 3],
            velocity: [0.0; 3],
            heading_rad: 0.0,
            entity_id: 0,
            target_entity_id: 0,
            pet_entity_id: None,
            is_auto_attacking: false,
            is_dead: false,
            class_id: 0,
            race_id: 0,
            location: PlayerLocationSnapshot::default(),
            available: false,
        }
    }
}

/// Fixed-size, Copy snapshot of player location for the active frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerLocationSnapshot {
    pub place: [u8; 64],
    pub place_len: u8,
    pub compass: [u8; 16],
    pub compass_len: u8,
    pub name: [u8; 80],
    pub name_len: u8,
    pub cell_x: i32,
    pub cell_z: i32,
    pub valid: bool,
}

impl Default for PlayerLocationSnapshot {
    fn default() -> Self {
        Self {
            place: [0u8; 64],
            place_len: 0,
            compass: [0u8; 16],
            compass_len: 0,
            name: [0u8; 80],
            name_len: 0,
            cell_x: 0,
            cell_z: 0,
            valid: false,
        }
    }
}

pub const CLASSES: [&str; 15] = [
    "Warrior",
    "Ranger",
    "Paladin",
    "Shadowknight",
    "Monk",
    "Bard",
    "Rogue",
    "Druid",
    "Shaman",
    "Cleric",
    "Magician",
    "Necromancer",
    "Enchanter",
    "Wizard",
    "Alchemist",
];

pub const RACES: [&str; 10] = [
    "Human",
    "Elf",
    "Dark Elf",
    "Gnome",
    "Dwarf",
    "Troll",
    "Barbarian",
    "Halfling",
    "Erudite",
    "Ogre",
];

pub fn install_player_adapter(host: &Host) -> LuaResult<()> {
    let lua = &host.lua;
    let player = lua.create_table()?;
    let snapshot_cell = host.snapshot.clone();

    // 1. Identity & Level
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetName",
            lua.create_function(move |lua_ctx, ()| {
                let snap = s.get().player;
                if snap.available && snap.name_len > 0 {
                    let len = (snap.name_len as usize).min(snap.name.len());
                    let name_str = std::str::from_utf8(&snap.name[..len]).unwrap_or("");
                    Ok(Value::String(lua_ctx.create_string(name_str)?))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetLevel",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(if snap.available { snap.level } else { 0 })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetExp",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(if snap.available { snap.exp } else { 0 })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetExpDebt",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(if snap.available { snap.exp_debt } else { 0 })
            })?,
        )?;
    }

    // 2. Health & Power
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetCurrentHp",
            lua.create_function(move |_, ()| {
                let snap = s.get();
                if snap.health.is_available() {
                    Ok(snap.health.current_hp.max(0) as u32)
                } else if snap.player.available {
                    Ok(snap.player.current_hp)
                } else {
                    Ok(0)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetMaxHp",
            lua.create_function(move |_, ()| {
                let snap = s.get();
                if snap.health.is_available() {
                    Ok(snap.health.max_hp.max(0) as u32)
                } else if snap.player.available {
                    Ok(snap.player.max_hp)
                } else {
                    Ok(0)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBaseHp",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(if snap.available { snap.base_hp } else { 0 })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetCurrentPwr",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(if snap.available && snap.max_pwr > 0 {
                    snap.current_pwr
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetMaxPwr",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(if snap.available { snap.max_pwr } else { 0 })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBasePwr",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(if snap.available { snap.base_pwr } else { 0 })
            })?,
        )?;
    }

    // 3. Total & Base Attributes
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetTotalStr",
            lua.create_function(move |_, ()| Ok(s.get().player.total_str))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetTotalSta",
            lua.create_function(move |_, ()| Ok(s.get().player.total_sta))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetTotalAgi",
            lua.create_function(move |_, ()| Ok(s.get().player.total_agi))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetTotalDex",
            lua.create_function(move |_, ()| Ok(s.get().player.total_dex))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetTotalWis",
            lua.create_function(move |_, ()| Ok(s.get().player.total_wis))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetTotalInt",
            lua.create_function(move |_, ()| Ok(s.get().player.total_int))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetTotalCha",
            lua.create_function(move |_, ()| Ok(s.get().player.total_cha))?,
        )?;
    }

    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBaseStr",
            lua.create_function(move |_, ()| Ok(s.get().player.base_str))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBaseSta",
            lua.create_function(move |_, ()| Ok(s.get().player.base_sta))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBaseAgi",
            lua.create_function(move |_, ()| Ok(s.get().player.base_agi))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBaseDex",
            lua.create_function(move |_, ()| Ok(s.get().player.base_dex))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBaseWis",
            lua.create_function(move |_, ()| Ok(s.get().player.base_wis))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBaseInt",
            lua.create_function(move |_, ()| Ok(s.get().player.base_int))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBaseCha",
            lua.create_function(move |_, ()| Ok(s.get().player.base_cha))?,
        )?;
    }

    // 4. Defense & Resists
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetAc",
            lua.create_function(move |_, ()| Ok(s.get().player.ac))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetBaseResist",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                let bonus = snap.total_wis / 7;
                Ok(40 + bonus)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetPoisonResistBuff",
            lua.create_function(move |_, ()| Ok(s.get().player.poison_resist))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetDiseaseResistBuff",
            lua.create_function(move |_, ()| Ok(s.get().player.disease_resist))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetFireResistBuff",
            lua.create_function(move |_, ()| Ok(s.get().player.fire_resist))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetColdResistBuff",
            lua.create_function(move |_, ()| Ok(s.get().player.cold_resist))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetLightningResistBuff",
            lua.create_function(move |_, ()| Ok(s.get().player.lightning_resist))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetArcaneResistBuff",
            lua.create_function(move |_, ()| Ok(s.get().player.arcane_resist))?,
        )?;
    }

    // 5. Class Mastery (CM)
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetCMs",
            lua.create_function(move |_, ()| Ok(s.get().player.cms))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetCMsSpent",
            lua.create_function(move |_, ()| Ok(s.get().player.cms_spent))?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetCMPct",
            lua.create_function(move |_, ()| Ok(s.get().player.cm_pct))?,
        )?;
    }

    // 6. Coordinates & Physics
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetCoordinates",
            lua.create_function(move |lua, ()| {
                let snap = s.get().player;
                let t = lua.create_table()?;
                t.set("x", snap.position[0])?;
                t.set("y", snap.position[1])?;
                t.set("z", snap.position[2])?;
                Ok(t)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetVelocity",
            lua.create_function(move |lua, ()| {
                let snap = s.get().player;
                let t = lua.create_table()?;
                t.set("x", snap.velocity[0])?;
                t.set("y", snap.velocity[1])?;
                t.set("z", snap.velocity[2])?;
                Ok(t)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetSpeed",
            lua.create_function(move |_, ()| {
                let v = s.get().player.velocity;
                let spd = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                Ok(spd)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetHorizontalSpeed",
            lua.create_function(move |_, ()| {
                let v = s.get().player.velocity;
                let spd = (v[0] * v[0] + v[2] * v[2]).sqrt();
                Ok(spd)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetHeadingRadians",
            lua.create_function(move |_, ()| {
                let rad = s.get().player.heading_rad;
                let wrapped = rad.rem_euclid(2.0 * PI);
                Ok(wrapped)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetHeadingDegrees",
            lua.create_function(move |_, ()| {
                let rad = s.get().player.heading_rad;
                let deg = (rad * (180.0 / PI)).rem_euclid(360.0);
                Ok(deg)
            })?,
        )?;
    }

    // 7. Entities & State
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetEntityId",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(if snap.available { snap.entity_id } else { 0 })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetTargetEntityId",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(if snap.available {
                    snap.target_entity_id
                } else {
                    0
                })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "IsAutoAttacking",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(snap.available && snap.is_auto_attacking)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "IsDead",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(snap.available && snap.is_dead)
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetPetEntityId",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                if snap.available {
                    match snap.pet_entity_id {
                        Some(id) if id > 0 => Ok(Value::Integer(id as i64)),
                        _ => Ok(Value::Nil),
                    }
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "HasPet",
            lua.create_function(move |_, ()| {
                let snap = s.get().player;
                Ok(snap.available && snap.pet_entity_id.is_some_and(|id| id > 0))
            })?,
        )?;
    }

    // 8. Class & Race
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetClassId",
            lua.create_function(move |_, ()| {
                let cid = s.get().player.class_id;
                Ok(if cid <= 14 { cid } else { 0 })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetRaceId",
            lua.create_function(move |_, ()| {
                let rid = s.get().player.race_id;
                Ok(if rid <= 9 { rid } else { 0 })
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetClassName",
            lua.create_function(move |lua_ctx, ()| {
                let cid = s.get().player.class_id as usize;
                if cid < CLASSES.len() {
                    Ok(Value::String(lua_ctx.create_string(CLASSES[cid])?))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetRaceName",
            lua.create_function(move |lua_ctx, ()| {
                let rid = s.get().player.race_id as usize;
                if rid < RACES.len() {
                    Ok(Value::String(lua_ctx.create_string(RACES[rid])?))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }

    // 9. Tables: classes, races, max constants
    let classes_tbl = lua.create_table()?;
    for (idx, name) in CLASSES.iter().enumerate() {
        classes_tbl.set(idx as i64, *name)?;
    }
    player.set("classes", classes_tbl)?;

    let races_tbl = lua.create_table()?;
    for (idx, name) in RACES.iter().enumerate() {
        races_tbl.set(idx as i64, *name)?;
    }
    player.set("races", races_tbl)?;

    player.set("max_class_id", 14)?;
    player.set("max_race_id", 9)?;

    // 10. Location
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetLocation",
            lua.create_function(move |lua, ()| {
                let snap = s.get().player;
                if snap.available && snap.location.valid {
                    let loc = snap.location;
                    let t = lua.create_table()?;
                    let p_str =
                        std::str::from_utf8(&loc.place[..loc.place_len as usize]).unwrap_or("");
                    let c_str =
                        std::str::from_utf8(&loc.compass[..loc.compass_len as usize]).unwrap_or("");
                    let n_str =
                        std::str::from_utf8(&loc.name[..loc.name_len as usize]).unwrap_or("");
                    t.set("place", p_str)?;
                    t.set("compass", c_str)?;
                    t.set("name", n_str)?;
                    t.set("cell_x", loc.cell_x)?;
                    t.set("cell_z", loc.cell_z)?;
                    Ok(Value::Table(t))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        player.set(
            "GetLocationName",
            lua.create_function(move |lua_ctx, ()| {
                let snap = s.get().player;
                if snap.available && snap.location.valid {
                    let loc = snap.location;
                    let n_str =
                        std::str::from_utf8(&loc.name[..loc.name_len as usize]).unwrap_or("");
                    Ok(Value::String(lua_ctx.create_string(n_str)?))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }

    host.register_host_module("frontiers_forge.player", Value::Table(player))?;
    Ok(())
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
            std::env::temp_dir().join(format!("eqoa_player_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_addon(root: &Path) -> DiscoveredAddon {
        let entry = root.join("main.lua");
        fs::write(&entry, "-- test player addon").unwrap();
        DiscoveredAddon {
            id: "test_player".to_string(),
            name: "Test Player".to_string(),
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
    fn forge_player_empty_snapshot_returns_defaults_or_nil() {
        let dir = temp_test_dir("empty");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let script = host
            .compile(
                r#"
            local P = require("frontiers_forge.player")
            assert(P.GetName() == nil, "name must be nil")
            assert(P.GetLevel() == 0, "level must be 0")
            assert(P.GetExp() == 0, "exp must be 0")
            assert(P.GetExpDebt() == 0, "exp debt must be 0")
            assert(P.GetCurrentHp() == 0, "hp must be 0")
            assert(P.GetMaxHp() == 0, "max hp must be 0")
            assert(P.GetCurrentPwr() == 0, "pwr must be 0")
            assert(P.GetMaxPwr() == 0, "max pwr must be 0")
            assert(P.GetBasePwr() == 0, "base pwr must be 0")
            assert(P.GetEntityId() == 0, "entity id must be 0")
            assert(P.GetTargetEntityId() == 0, "target entity id must be 0")
            assert(P.GetPetEntityId() == nil, "pet entity id must be nil")
            assert(P.HasPet() == false, "has pet must be false")
            assert(P.IsAutoAttacking() == false, "auto attack must be false")
            assert(P.IsDead() == false, "is dead must be false")
            assert(P.GetLocation() == nil, "location must be nil")
            assert(P.GetLocationName() == nil, "location name must be nil")

            local coords = P.GetCoordinates()
            assert(coords.x == 0.0 and coords.y == 0.0 and coords.z == 0.0)
            local vel = P.GetVelocity()
            assert(vel.x == 0.0 and vel.y == 0.0 and vel.z == 0.0)
            assert(P.GetSpeed() == 0.0)
            assert(P.GetHorizontalSpeed() == 0.0)
            assert(P.GetHeadingRadians() == 0.0)
            assert(P.GetHeadingDegrees() == 0.0)
            return true
        "#,
            )
            .unwrap();

        host.run(&script).unwrap();
    }

    #[test]
    fn forge_player_populated_stats_and_power_reflection() {
        let dir = temp_test_dir("populated");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut p_snap = PlayerSnapshot::default();
        let name_bytes = b"Wererat";
        p_snap.name[..name_bytes.len()].copy_from_slice(name_bytes);
        p_snap.name_len = name_bytes.len() as u8;
        p_snap.level = 60;
        p_snap.exp = 4500000;
        p_snap.exp_debt = 12000;
        p_snap.current_hp = 3200;
        p_snap.max_hp = 3200;
        p_snap.base_hp = 2800;
        p_snap.current_pwr = 1850;
        p_snap.max_pwr = 2000;
        p_snap.base_pwr = 1600;
        p_snap.total_str = 150;
        p_snap.total_sta = 140;
        p_snap.total_agi = 130;
        p_snap.total_dex = 120;
        p_snap.total_wis = 77;
        p_snap.total_int = 80;
        p_snap.total_cha = 70;
        p_snap.base_str = 100;
        p_snap.base_sta = 100;
        p_snap.base_agi = 95;
        p_snap.base_dex = 90;
        p_snap.base_wis = 70;
        p_snap.base_int = 75;
        p_snap.base_cha = 65;
        p_snap.ac = 450;
        p_snap.poison_resist = 25;
        p_snap.disease_resist = 30;
        p_snap.fire_resist = 35;
        p_snap.cold_resist = 40;
        p_snap.lightning_resist = 45;
        p_snap.arcane_resist = 50;
        p_snap.cms = 15;
        p_snap.cms_spent = 10;
        p_snap.cm_pct = 75;
        p_snap.entity_id = 1001;
        p_snap.target_entity_id = 2002;
        p_snap.pet_entity_id = Some(3003);
        p_snap.is_auto_attacking = true;
        p_snap.is_dead = false;
        p_snap.class_id = 3; // Shadowknight
        p_snap.race_id = 2; // Dark Elf
        p_snap.available = true;

        host.snapshot.set(Snapshot {
            in_game: true,
            start_menu: false,
            health: crate::fork_ext::addons::health::HealthSnapshot {
                available: true,
                current_hp: 3200,
                max_hp: 3200,
            },
            player: p_snap,
            ..Default::default()
        });

        let script = host
            .compile(
                r#"
            local P = require("frontiers_forge.player")
            assert(P.GetName() == "Wererat")
            assert(P.GetLevel() == 60)
            assert(P.GetExp() == 4500000)
            assert(P.GetExpDebt() == 12000)
            assert(P.GetCurrentHp() == 3200)
            assert(P.GetMaxHp() == 3200)
            assert(P.GetBaseHp() == 2800)
            assert(P.GetCurrentPwr() == 1850)
            assert(P.GetMaxPwr() == 2000)
            assert(P.GetBasePwr() == 1600)
            assert(P.GetTotalStr() == 150)
            assert(P.GetTotalSta() == 140)
            assert(P.GetTotalAgi() == 130)
            assert(P.GetTotalDex() == 120)
            assert(P.GetTotalWis() == 77)
            assert(P.GetTotalInt() == 80)
            assert(P.GetTotalCha() == 70)
            assert(P.GetBaseStr() == 100)
            assert(P.GetBaseSta() == 100)
            assert(P.GetBaseAgi() == 95)
            assert(P.GetBaseDex() == 90)
            assert(P.GetBaseWis() == 70)
            assert(P.GetBaseInt() == 75)
            assert(P.GetBaseCha() == 65)
            assert(P.GetAc() == 450)
            -- 40 + math.floor(77 / 7) = 40 + 11 = 51
            assert(P.GetBaseResist() == 51)
            assert(P.GetPoisonResistBuff() == 25)
            assert(P.GetDiseaseResistBuff() == 30)
            assert(P.GetFireResistBuff() == 35)
            assert(P.GetColdResistBuff() == 40)
            assert(P.GetLightningResistBuff() == 45)
            assert(P.GetArcaneResistBuff() == 50)
            assert(P.GetCMs() == 15)
            assert(P.GetCMsSpent() == 10)
            assert(P.GetCMPct() == 75)
            assert(P.GetEntityId() == 1001)
            assert(P.GetTargetEntityId() == 2002)
            assert(P.GetPetEntityId() == 3003)
            assert(P.HasPet() == true)
            assert(P.IsAutoAttacking() == true)
            assert(P.IsDead() == false)
            assert(P.GetClassId() == 3)
            assert(P.GetClassName() == "Shadowknight")
            assert(P.GetRaceId() == 2)
            assert(P.GetRaceName() == "Dark Elf")
            return true
        "#,
            )
            .unwrap();

        host.run(&script).unwrap();
    }

    #[test]
    fn forge_player_coordinates_speed_and_heading_trigonometry() {
        let dir = temp_test_dir("physics");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut p_snap = PlayerSnapshot::default();
        p_snap.position = [100.5, 25.25, -200.75];
        p_snap.velocity = [3.0, 4.0, 4.0];
        p_snap.heading_rad = PI / 2.0; // West = 90 degrees
        p_snap.available = true;

        host.snapshot.set(Snapshot {
            in_game: true,
            start_menu: false,
            health: crate::fork_ext::addons::health::HealthSnapshot::default(),
            player: p_snap,
            ..Default::default()
        });

        let script = host
            .compile(
                r#"
            local P = require("frontiers_forge.player")
            local c = P.GetCoordinates()
            assert(math.abs(c.x - 100.5) < 0.001)
            assert(math.abs(c.y - 25.25) < 0.001)
            assert(math.abs(c.z - (-200.75)) < 0.001)

            local v = P.GetVelocity()
            assert(math.abs(v.x - 3.0) < 0.001)
            assert(math.abs(v.y - 4.0) < 0.001)
            assert(math.abs(v.z - 4.0) < 0.001)

            -- Horizontal speed: sqrt(3^2 + 4^2) = 5.0
            local hspd = P.GetHorizontalSpeed()
            assert(math.abs(hspd - 5.0) < 0.001, "horizontal speed must be 5.0")

            -- Total speed: sqrt(3^2 + 4^2 + 4^2) = sqrt(41) ~ 6.4031
            local spd = P.GetSpeed()
            assert(math.abs(spd - math.sqrt(41)) < 0.001, "total speed must be sqrt(41)")

            -- Heading: PI/2 rad -> 90 degrees
            local rad = P.GetHeadingRadians()
            assert(math.abs(rad - (math.pi / 2)) < 0.001)
            local deg = P.GetHeadingDegrees()
            assert(math.abs(deg - 90.0) < 0.001)
            return true
        "#,
            )
            .unwrap();

        host.run(&script).unwrap();
    }

    #[test]
    fn forge_player_location_strings_and_cells() {
        let dir = temp_test_dir("loc");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut p_snap = PlayerSnapshot::default();
        let mut loc = PlayerLocationSnapshot::default();
        let place = b"Neriak, Nektulos";
        loc.place[..place.len()].copy_from_slice(place);
        loc.place_len = place.len() as u8;

        let compass = b"C";
        loc.compass[..compass.len()].copy_from_slice(compass);
        loc.compass_len = compass.len() as u8;

        let name = b"Neriak, Nektulos (C)";
        loc.name[..name.len()].copy_from_slice(name);
        loc.name_len = name.len() as u8;

        loc.cell_x = 12;
        loc.cell_z = 34;
        loc.valid = true;

        p_snap.location = loc;
        p_snap.available = true;

        host.snapshot.set(Snapshot {
            in_game: true,
            start_menu: false,
            health: crate::fork_ext::addons::health::HealthSnapshot::default(),
            player: p_snap,
            ..Default::default()
        });

        let script = host
            .compile(
                r#"
            local P = require("frontiers_forge.player")
            local loc = P.GetLocation()
            assert(loc ~= nil)
            assert(loc.place == "Neriak, Nektulos")
            assert(loc.compass == "C")
            assert(loc.name == "Neriak, Nektulos (C)")
            assert(loc.cell_x == 12)
            assert(loc.cell_z == 34)

            assert(P.GetLocationName() == "Neriak, Nektulos (C)")
            return true
        "#,
            )
            .unwrap();

        host.run(&script).unwrap();
    }

    #[test]
    fn forge_player_dynamic_frame_snapshot_updates_without_re_requiring() {
        let dir = temp_test_dir("dynamic");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let script = host
            .compile(
                r#"
            local P = require("frontiers_forge.player")
            _G.read_stats = function()
                return P.GetLevel(), P.GetCurrentHp(), P.GetCurrentPwr()
            end
            return true
        "#,
            )
            .unwrap();
        host.run(&script).unwrap();

        let read_fn: mlua::Function = host.lua.globals().get("read_stats").unwrap();

        // Frame 0: empty
        let (lvl0, hp0, pwr0): (u32, u32, u32) = read_fn.call(()).unwrap();
        assert_eq!((lvl0, hp0, pwr0), (0, 0, 0));

        // Frame 1: updated snapshot
        let mut p1 = PlayerSnapshot::default();
        p1.level = 42;
        p1.current_hp = 1500;
        p1.max_hp = 1500;
        p1.current_pwr = 800;
        p1.max_pwr = 1000;
        p1.available = true;
        host.snapshot.set(Snapshot {
            in_game: true,
            start_menu: false,
            health: crate::fork_ext::addons::health::HealthSnapshot {
                available: true,
                current_hp: 1500,
                max_hp: 1500,
            },
            player: p1,
            ..Default::default()
        });

        let (lvl1, hp1, pwr1): (u32, u32, u32) = read_fn.call(()).unwrap();
        assert_eq!((lvl1, hp1, pwr1), (42, 1500, 800));
    }
}
