//! Native FrontiersForge group adapter module (`frontiers_forge.group`).
//!
//! Authoritative Contracts:
//! - Upstream pinned reference: `scripts/modules/frontiers_forge/group.lua`
//!   and `wereoxxs_ui.lua`.
//!
//! Scope & Invariant Guarantees:
//! - Fixed-size, Copy snapshot of group state (`GroupSnapshot`, `GroupMemberSnapshot`).
//! - Full native implementation of `frontiers_forge.group` module table.
//! - Read-only access to engine group state via `host.snapshot`.
//! - Pure functions for group membership, leader queries, and member iterators.

use super::host::Host;
use mlua::{Result as LuaResult, Table, Value};

pub const MAX_GROUP_MEMBERS: usize = 8;

/// Fixed-size, Copy snapshot of a single group member.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupMemberSnapshot {
    pub entity_id: u32,
    pub name: [u8; 24],
    pub name_len: u8,
    pub active: bool,
    pub hp_known: bool,
    pub percent_hp_255: u8,
    pub position: [f32; 3],
}

impl Default for GroupMemberSnapshot {
    fn default() -> Self {
        Self {
            entity_id: 0,
            name: [0u8; 24],
            name_len: 0,
            active: false,
            hp_known: false,
            percent_hp_255: 0,
            position: [0.0; 3],
        }
    }
}

/// Fixed-size, Copy snapshot of the active group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupSnapshot {
    pub in_group: bool,
    pub is_leader: bool,
    pub member_count: u8,
    pub members: [GroupMemberSnapshot; MAX_GROUP_MEMBERS],
}

impl Default for GroupSnapshot {
    fn default() -> Self {
        Self {
            in_group: false,
            is_leader: false,
            member_count: 0,
            members: [GroupMemberSnapshot::default(); MAX_GROUP_MEMBERS],
        }
    }
}

pub fn install_group_adapter(host: &Host) -> LuaResult<()> {
    let lua = &host.lua;
    let group = lua.create_table()?;
    let snapshot_cell = host.snapshot.clone();

    // 1. Group.IsInGroup() -> bool
    {
        let s = snapshot_cell.clone();
        group.set(
            "IsInGroup",
            lua.create_function(move |_, ()| {
                let snap = s.get().group;
                Ok(snap.in_group)
            })?,
        )?;
    }

    // 2. Group.IsSelfLeader() -> bool
    {
        let s = snapshot_cell.clone();
        group.set(
            "IsSelfLeader",
            lua.create_function(move |_, ()| {
                let snap = s.get().group;
                Ok(snap.in_group && snap.is_leader)
            })?,
        )?;
    }

    // 3. Group.GetMemberCount() -> integer
    {
        let s = snapshot_cell.clone();
        group.set(
            "GetMemberCount",
            lua.create_function(move |_, ()| {
                let snap = s.get().group;
                Ok(if snap.in_group {
                    snap.member_count as u32
                } else {
                    0
                })
            })?,
        )?;
    }

    // Metatable for GroupMember objects
    let member_mt = lua.create_table()?;
    let member_methods = lua.create_table()?;

    {
        let s = snapshot_cell.clone();
        member_methods.set(
            "GetEntityId",
            lua.create_function(move |_, this: Table| {
                let idx: usize = this.get("_idx")?;
                let snap = s.get().group;
                if snap.in_group && idx < (snap.member_count as usize) {
                    Ok(snap.members[idx].entity_id)
                } else {
                    Ok(0)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        member_methods.set(
            "GetName",
            lua.create_function(move |lua_ctx, this: Table| {
                let idx: usize = this.get("_idx")?;
                let snap = s.get().group;
                if snap.in_group && idx < (snap.member_count as usize) {
                    let m = &snap.members[idx];
                    let len = (m.name_len as usize).min(m.name.len());
                    let name_str = std::str::from_utf8(&m.name[..len]).unwrap_or("");
                    Ok(Value::String(lua_ctx.create_string(name_str)?))
                } else {
                    Ok(Value::String(lua_ctx.create_string("")?))
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        member_methods.set(
            "GetHealthPercent255",
            lua.create_function(move |_, this: Table| {
                let idx: usize = this.get("_idx")?;
                let snap = s.get().group;
                if snap.in_group && idx < (snap.member_count as usize) && snap.members[idx].hp_known
                {
                    Ok(Value::Integer(snap.members[idx].percent_hp_255 as i64))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        member_methods.set(
            "GetHealthPercent",
            lua.create_function(move |_, this: Table| {
                let idx: usize = this.get("_idx")?;
                let snap = s.get().group;
                if snap.in_group && idx < (snap.member_count as usize) && snap.members[idx].hp_known
                {
                    let pct = (snap.members[idx].percent_hp_255 as f64 / 255.0) * 100.0;
                    Ok(Value::Number(pct))
                } else {
                    Ok(Value::Nil)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        member_methods.set(
            "IsActive",
            lua.create_function(move |_, this: Table| {
                let idx: usize = this.get("_idx")?;
                let snap = s.get().group;
                if snap.in_group && idx < (snap.member_count as usize) {
                    Ok(snap.members[idx].active)
                } else {
                    Ok(false)
                }
            })?,
        )?;
    }
    {
        let s = snapshot_cell.clone();
        member_methods.set(
            "GetCoordinates",
            lua.create_function(move |lua_ctx, this: Table| {
                let idx: usize = this.get("_idx")?;
                let snap = s.get().group;
                let t = lua_ctx.create_table()?;
                if snap.in_group && idx < (snap.member_count as usize) {
                    let pos = snap.members[idx].position;
                    t.set("x", pos[0] as f64)?;
                    t.set("y", pos[1] as f64)?;
                    t.set("z", pos[2] as f64)?;
                } else {
                    t.set("x", 0.0)?;
                    t.set("y", 0.0)?;
                    t.set("z", 0.0)?;
                }
                Ok(t)
            })?,
        )?;
    }

    member_mt.set("__index", member_methods)?;

    // 4. Group.GetMemberByIndex(index) -> member or nil
    {
        let s = snapshot_cell.clone();
        let mt = member_mt.clone();
        group.set(
            "GetMemberByIndex",
            lua.create_function(move |lua_ctx, index: i64| {
                let snap = s.get().group;
                if !snap.in_group || index < 0 || index >= snap.member_count as i64 {
                    return Ok(Value::Nil);
                }
                let tbl = lua_ctx.create_table()?;
                tbl.set("_idx", index as usize)?;
                let _ = tbl.set_metatable(Some(mt.clone()));
                Ok(Value::Table(tbl))
            })?,
        )?;
    }

    // 5. Group.Members() -> iterator function
    {
        let s = snapshot_cell;
        let mt = member_mt;
        group.set(
            "Members",
            lua.create_function(move |lua_ctx, ()| {
                let s_iter = s.clone();
                let mt_iter = mt.clone();
                let mut current_idx: i64 = -1;
                lua_ctx.create_function_mut(move |lua_ctx, ()| {
                    current_idx += 1;
                    let snap = s_iter.get().group;
                    if !snap.in_group || current_idx >= snap.member_count as i64 {
                        return Ok((Value::Nil, Value::Nil));
                    }
                    let tbl = lua_ctx.create_table()?;
                    tbl.set("_idx", current_idx as usize)?;
                    let _ = tbl.set_metatable(Some(mt_iter.clone()));
                    Ok((Value::Integer(current_idx), Value::Table(tbl)))
                })
            })?,
        )?;
    }

    host.register_host_module("frontiers_forge.group", Value::Table(group))?;
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
            std::env::temp_dir().join(format!("eqoa_group_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_addon(root: &Path) -> DiscoveredAddon {
        let entry = root.join("main.lua");
        fs::write(&entry, "-- test group addon").unwrap();
        DiscoveredAddon {
            id: "test_group".to_string(),
            name: "Test Group".to_string(),
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
    fn forge_group_adapter_not_in_group_defaults() {
        let dir = temp_test_dir("group_not_in_group");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();
        let chunk = host.lua.load(
            r#"
            local Group = require("frontiers_forge.group")
            assert(Group.IsInGroup() == false)
            assert(Group.IsSelfLeader() == false)
            assert(Group.GetMemberCount() == 0)
            assert(Group.GetMemberByIndex(0) == nil)
            assert(Group.GetMemberByIndex(-1) == nil)
            local count = 0
            for idx, member in Group.Members() do
                count = count + 1
            end
            assert(count == 0)
            return true
            "#,
        );
        let ok: bool = chunk.eval().unwrap();
        assert!(ok);
    }

    #[test]
    fn forge_group_adapter_in_group_roster_queries_and_methods() {
        let dir = temp_test_dir("group_in_group");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = Snapshot::default();
        snap.in_game = true;
        snap.group.in_group = true;
        snap.group.is_leader = true;
        snap.group.member_count = 3;

        // Member 0: Warrior (full HP, active)
        let mut m0 = GroupMemberSnapshot::default();
        m0.entity_id = 1001;
        let name0 = b"Warrior";
        m0.name[..name0.len()].copy_from_slice(name0);
        m0.name_len = name0.len() as u8;
        m0.active = true;
        m0.hp_known = true;
        m0.percent_hp_255 = 255;
        m0.position = [10.5, 20.0, 30.5];
        snap.group.members[0] = m0;

        // Member 1: Cleric (half HP, active)
        let mut m1 = GroupMemberSnapshot::default();
        m1.entity_id = 1002;
        let name1 = b"Cleric";
        m1.name[..name1.len()].copy_from_slice(name1);
        m1.name_len = name1.len() as u8;
        m1.active = true;
        m1.hp_known = true;
        m1.percent_hp_255 = 128;
        m1.position = [-5.0, 0.0, 15.0];
        snap.group.members[1] = m1;

        // Member 2: Wizard (unknown HP, inactive)
        let mut m2 = GroupMemberSnapshot::default();
        m2.entity_id = 1003;
        let name2 = b"Wizard";
        m2.name[..name2.len()].copy_from_slice(name2);
        m2.name_len = name2.len() as u8;
        m2.active = false;
        m2.hp_known = false;
        m2.percent_hp_255 = 0;
        m2.position = [100.0, 50.0, 200.0];
        snap.group.members[2] = m2;

        host.snapshot.set(snap);

        let chunk = host.lua.load(
            r#"
            local Group = require("frontiers_forge.group")
            assert(Group.IsInGroup() == true)
            assert(Group.IsSelfLeader() == true)
            assert(Group.GetMemberCount() == 3)

            -- Out of range indices return nil
            assert(Group.GetMemberByIndex(-1) == nil)
            assert(Group.GetMemberByIndex(3) == nil)
            assert(Group.GetMemberByIndex(99) == nil)

            -- Member 0
            local m0 = Group.GetMemberByIndex(0)
            assert(m0 ~= nil)
            assert(m0:GetEntityId() == 1001)
            assert(m0:GetName() == "Warrior")
            assert(m0:IsActive() == true)
            assert(m0:GetHealthPercent255() == 255)
            assert(math.abs(m0:GetHealthPercent() - 100.0) < 0.001)
            local coords0 = m0:GetCoordinates()
            assert(coords0.x == 10.5 and coords0.y == 20.0 and coords0.z == 30.5)

            -- Member 1
            local m1 = Group.GetMemberByIndex(1)
            assert(m1 ~= nil)
            assert(m1:GetEntityId() == 1002)
            assert(m1:GetName() == "Cleric")
            assert(m1:IsActive() == true)
            assert(m1:GetHealthPercent255() == 128)
            local pct1 = m1:GetHealthPercent()
            assert(math.abs(pct1 - (128 / 255 * 100)) < 0.001)
            local coords1 = m1:GetCoordinates()
            assert(coords1.x == -5.0 and coords1.y == 0.0 and coords1.z == 15.0)

            -- Member 2
            local m2 = Group.GetMemberByIndex(2)
            assert(m2 ~= nil)
            assert(m2:GetEntityId() == 1003)
            assert(m2:GetName() == "Wizard")
            assert(m2:IsActive() == false)
            assert(m2:GetHealthPercent255() == nil)
            assert(m2:GetHealthPercent() == nil)
            local coords2 = m2:GetCoordinates()
            assert(coords2.x == 100.0 and coords2.y == 50.0 and coords2.z == 200.0)

            -- Iterator Group.Members()
            local iterated = {}
            for idx, m in Group.Members() do
                table.insert(iterated, { idx = idx, id = m:GetEntityId(), name = m:GetName() })
            end
            assert(#iterated == 3)
            assert(iterated[1].idx == 0 and iterated[1].id == 1001 and iterated[1].name == "Warrior")
            assert(iterated[2].idx == 1 and iterated[2].id == 1002 and iterated[2].name == "Cleric")
            assert(iterated[3].idx == 2 and iterated[3].id == 1003 and iterated[3].name == "Wizard")

            return true
            "#,
        );
        let ok: bool = chunk.eval().unwrap();
        assert!(ok);
    }

    #[test]
    fn forge_group_adapter_stale_member_handle_safe() {
        let dir = temp_test_dir("group_stale_handle");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = Snapshot::default();
        snap.in_game = true;
        snap.group.in_group = true;
        snap.group.member_count = 2;

        let mut m0 = GroupMemberSnapshot::default();
        m0.entity_id = 500;
        let name0 = b"PlayerOne";
        m0.name[..name0.len()].copy_from_slice(name0);
        m0.name_len = name0.len() as u8;
        m0.active = true;
        snap.group.members[0] = m0;
        host.snapshot.set(snap);

        // Get handle
        let chunk1 = host.lua.load(
            r#"
            local Group = require("frontiers_forge.group")
            held_member = Group.GetMemberByIndex(0)
            assert(held_member:GetName() == "PlayerOne")
            "#,
        );
        chunk1.exec().unwrap();

        // Disband group: member_count = 0, in_group = false
        let mut snap2 = Snapshot::default();
        snap2.in_game = true;
        snap2.group.in_group = false;
        snap2.group.member_count = 0;
        host.snapshot.set(snap2);

        // Held member methods should return safe fallback values without panicking
        let chunk2 = host.lua.load(
            r#"
            assert(held_member:GetName() == "")
            assert(held_member:GetEntityId() == 0)
            assert(held_member:IsActive() == false)
            assert(held_member:GetHealthPercent255() == nil)
            assert(held_member:GetHealthPercent() == nil)
            local coords = held_member:GetCoordinates()
            assert(coords.x == 0.0 and coords.y == 0.0 and coords.z == 0.0)
            return true
            "#,
        );
        let ok: bool = chunk2.eval().unwrap();
        assert!(ok);
    }
}
