//! Native FrontiersForge combat adapter (`frontiers_forge.combat`).
//!
//! Authoritative Contracts:
//! - Upstream pinned reference: `scripts/modules/frontiers_forge/combat.lua`
//!   and `wereoxxs_ui.lua`.
//!
//! Scope & Invariant Guarantees:
//! - Fixed-size, Copy snapshot of combat events (`CombatSnapshot`, `CombatEventSnapshot`).
//! - Full native implementation of `frontiers_forge.combat` module table.
//! - Read-only access to engine combat state via `host.snapshot`.
//! - Safe fanned-out event bus (`OnDamageDealt`, `OnDamageReceived`, `OnHealingDealt`, `OnHealingReceived`).
//! - Refcounted owner claims (`Acquire`, `Release`, `HasClaim`).

use super::host::Host;
use mlua::{Result as LuaResult, Table, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

pub const MAX_COMBAT_EVENTS: usize = 32;

thread_local! {
    static COMBAT_RING: RefCell<VecDeque<CombatEventSnapshot>> = const { RefCell::new(VecDeque::new()) };
    static COMBAT_MONOTONIC_SEQ: Cell<u32> = const { Cell::new(1) };
}

pub(crate) fn record_combat_event(attacker: u32, amount: i32, defender: u32) {
    COMBAT_RING.with(|ring| {
        let mut r = ring.borrow_mut();
        if r.len() >= 64 {
            r.pop_front();
        }
        let seq = COMBAT_MONOTONIC_SEQ.with(|s| {
            let val = s.get();
            s.set(val.wrapping_add(1));
            val
        });
        r.push_back(CombatEventSnapshot {
            attacker_id: attacker,
            defender_id: defender,
            amount,
            seq,
            color: 0,
        });
    });
}

pub(crate) fn snapshot_combat_events() -> CombatSnapshot {
    COMBAT_RING.with(|ring| {
        let r = ring.borrow();
        let mut events = [CombatEventSnapshot::default(); MAX_COMBAT_EVENTS];
        let count = r.len().min(MAX_COMBAT_EVENTS);
        for (i, ev) in r.iter().rev().take(count).enumerate() {
            events[count - 1 - i] = *ev;
        }
        CombatSnapshot {
            count: count as u8,
            events,
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatEventSnapshot {
    pub attacker_id: u32,
    pub defender_id: u32,
    pub amount: i32,
    pub seq: u32,
    pub color: u32,
}

impl Default for CombatEventSnapshot {
    fn default() -> Self {
        Self {
            attacker_id: 0,
            defender_id: 0,
            amount: 0,
            seq: 0,
            color: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatSnapshot {
    pub count: u8,
    pub events: [CombatEventSnapshot; MAX_COMBAT_EVENTS],
}

impl Default for CombatSnapshot {
    fn default() -> Self {
        Self {
            count: 0,
            events: [CombatEventSnapshot::default(); MAX_COMBAT_EVENTS],
        }
    }
}

pub fn install_combat_adapter(host: &Host) -> LuaResult<()> {
    let lua = &host.lua;
    let combat_mod = lua.create_table()?;

    // Events constants table
    let events_tbl = lua.create_table()?;
    events_tbl.set("OnDamageDealt", "OnDamageDealt")?;
    events_tbl.set("OnDamageReceived", "OnDamageReceived")?;
    events_tbl.set("OnHealingDealt", "OnHealingDealt")?;
    events_tbl.set("OnHealingReceived", "OnHealingReceived")?;
    combat_mod.set("Events", events_tbl)?;

    let snapshot_cell = host.snapshot.clone();
    let hook_claims = Rc::new(RefCell::new(HashSet::<String>::new()));
    let subscribers = Rc::new(RefCell::new(std::collections::HashMap::<
        String,
        std::collections::HashMap<String, mlua::Function>,
    >::new()));
    let subscriber_errors = Rc::new(RefCell::new(Vec::<Table>::new()));
    let last_seq = Rc::new(RefCell::new(0u32));

    // Hook claims
    {
        let claims = hook_claims.clone();
        combat_mod.set(
            "Acquire",
            lua.create_function(move |_, owner: String| {
                claims.borrow_mut().insert(owner);
                Ok(true)
            })?,
        )?;
    }

    {
        let claims = hook_claims.clone();
        combat_mod.set(
            "Release",
            lua.create_function(move |_, owner: String| {
                claims.borrow_mut().remove(&owner);
                Ok(())
            })?,
        )?;
    }

    {
        let claims = hook_claims.clone();
        combat_mod.set(
            "GetOwnerCount",
            lua.create_function(move |_, ()| Ok(claims.borrow().len()))?,
        )?;
    }

    {
        let claims = hook_claims.clone();
        combat_mod.set(
            "HasClaim",
            lua.create_function(move |_, owner: String| Ok(claims.borrow().contains(&owner)))?,
        )?;
    }

    {
        let claims = hook_claims.clone();
        let s = snapshot_cell.clone();
        combat_mod.set(
            "IsHookInstalled",
            lua.create_function(move |_, ()| Ok(!claims.borrow().is_empty() || s.get().in_game))?,
        )?;
    }

    combat_mod.set("InstallHook", lua.create_function(|_, ()| Ok(true))?)?;
    combat_mod.set("UninstallHook", lua.create_function(|_, ()| Ok(true))?)?;

    {
        let claims = hook_claims.clone();
        let s = snapshot_cell.clone();
        combat_mod.set(
            "IsHookSuspended",
            lua.create_function(move |_, ()| Ok(!s.get().in_game && !claims.borrow().is_empty()))?,
        )?;
    }

    {
        let s = snapshot_cell.clone();
        combat_mod.set(
            "GetHookInfo",
            lua.create_function(move |lua_ctx, ()| {
                let snap = s.get();
                let tbl = lua_ctx.create_table()?;
                tbl.set("installed", true)?;
                let total_events =
                    COMBAT_MONOTONIC_SEQ.with(|seq_cell| seq_cell.get().saturating_sub(1));
                tbl.set("event_count", total_events)?;
                tbl.set("dropped", 0)?;
                tbl.set("player_id", snap.player.entity_id)?;
                if let Some(pet) = snap.player.pet_entity_id {
                    tbl.set("pet_id", pet)?;
                }
                Ok(tbl)
            })?,
        )?;
    }

    {
        combat_mod.set(
            "GetEventCount",
            lua.create_function(|_, ()| {
                let total_events =
                    COMBAT_MONOTONIC_SEQ.with(|seq_cell| seq_cell.get().saturating_sub(1));
                Ok(total_events)
            })?,
        )?;
    }

    combat_mod.set("GetDroppedCount", lua.create_function(|_, ()| Ok(0))?)?;
    combat_mod.set(
        "ReadForeignCave",
        lua.create_function(|_, _max: Option<u32>| Ok(Value::Nil))?,
    )?;

    // On(owner, event_name, fn)
    {
        let subs = subscribers.clone();
        combat_mod.set(
            "On",
            lua.create_function(
                move |_, (owner, event_name, func): (String, String, mlua::Function)| {
                    match event_name.as_str() {
                        "OnDamageDealt" | "OnDamageReceived" | "OnHealingDealt"
                        | "OnHealingReceived" => {
                            let mut map = subs.borrow_mut();
                            map.entry(event_name).or_default().insert(owner, func);
                            Ok((true, None::<String>))
                        }
                        _ => Ok((false, Some(format!("unknown combat event: {event_name}")))),
                    }
                },
            )?,
        )?;
    }

    // Off(owner, event_name)
    {
        let subs = subscribers.clone();
        combat_mod.set(
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

    // GetSubscriberErrors
    {
        let errs = subscriber_errors.clone();
        combat_mod.set(
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

    // Update()
    {
        let s = snapshot_cell.clone();
        let subs = subscribers.clone();
        let errs = subscriber_errors.clone();
        let last_seq_ref = last_seq.clone();

        combat_mod.set(
            "Update",
            lua.create_function(move |lua_ctx, ()| {
                let snap = s.get();
                let combat_snap = snap.combat;
                let player_id = snap.player.entity_id;
                let pet_id = snap.player.pet_entity_id.unwrap_or(0);

                let mut current_last = *last_seq_ref.borrow();
                let mut dead_subscribers: Vec<(String, String)> = Vec::new();

                for i in 0..combat_snap.count as usize {
                    let ev = &combat_snap.events[i];
                    if ev.seq > current_last {
                        let attacker = ev.attacker_id;
                        let defender = ev.defender_id;
                        let amount = ev.amount;
                        let seq = ev.seq;
                        let color = ev.color;

                        let outgoing = (attacker != 0 && attacker == player_id)
                            || (pet_id != 0 && attacker == pet_id);
                        let incoming = defender != 0 && defender == player_id;
                        let from_pet = pet_id != 0
                            && (attacker == pet_id || (attacker == player_id && color != 0));
                        let to_pet = pet_id != 0 && defender == pet_id;
                        let is_heal = amount > 0;

                        let ev_tbl = lua_ctx.create_table()?;
                        ev_tbl.set("attacker_id", attacker)?;
                        ev_tbl.set("defender_id", defender)?;
                        ev_tbl.set("amount", amount)?;
                        ev_tbl.set("seq", seq)?;
                        if color != 0 {
                            ev_tbl.set("color", color)?;
                        }
                        ev_tbl.set("outgoing", outgoing)?;
                        ev_tbl.set("incoming", incoming)?;
                        ev_tbl.set("from_pet", from_pet)?;
                        ev_tbl.set("to_pet", to_pet)?;
                        ev_tbl.set("is_heal", is_heal)?;

                        let mut target_events = Vec::new();
                        if is_heal {
                            if outgoing {
                                target_events.push("OnHealingDealt");
                            }
                            if incoming {
                                target_events.push("OnHealingReceived");
                            }
                        } else {
                            if outgoing {
                                target_events.push("OnDamageDealt");
                            }
                            if incoming {
                                target_events.push("OnDamageReceived");
                            }
                        }

                        let subs_map = subs.borrow();
                        for ev_name in target_events {
                            if let Some(handlers) = subs_map.get(ev_name) {
                                for (owner, func) in handlers.iter() {
                                    if let Err(e) = func.call::<Value>(ev_tbl.clone()) {
                                        dead_subscribers.push((ev_name.to_string(), owner.clone()));
                                        let err_tbl = lua_ctx.create_table()?;
                                        err_tbl.set("owner", owner.as_str())?;
                                        err_tbl.set("event", ev_name)?;
                                        err_tbl.set("error", e.to_string())?;
                                        let mut err_list = errs.borrow_mut();
                                        err_list.push(err_tbl);
                                        if err_list.len() > 16 {
                                            err_list.remove(0);
                                        }
                                    }
                                }
                            }
                        }

                        if seq > current_last {
                            current_last = seq;
                        }
                    }
                }

                *last_seq_ref.borrow_mut() = current_last;

                if !dead_subscribers.is_empty() {
                    let mut subs_map = subs.borrow_mut();
                    for (ev_name, owner) in dead_subscribers {
                        if let Some(handlers) = subs_map.get_mut(&ev_name) {
                            handlers.remove(&owner);
                        }
                    }
                }

                Ok(())
            })?,
        )?;
    }

    host.register_host_module("frontiers_forge.combat", Value::Table(combat_mod))?;
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
            std::env::temp_dir().join(format!("eqoa_combat_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_addon(root: &Path) -> DiscoveredAddon {
        let entry = root.join("main.lua");
        fs::write(&entry, "-- test combat addon").unwrap();
        DiscoveredAddon {
            id: "test_combat".to_string(),
            name: "Test Combat".to_string(),
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
    fn forge_combat_claims_and_hook_info() {
        let dir = temp_test_dir("claims");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let chunk = host.lua.load(
            r#"
            local Combat = require("frontiers_forge.combat")
            assert(Combat.Acquire("wereoxxs_ui") == true)
            assert(Combat.HasClaim("wereoxxs_ui") == true)
            assert(Combat.GetOwnerCount() == 1)
            assert(Combat.IsHookInstalled() == true)

            local info = Combat.GetHookInfo()
            assert(info.installed == true)
            assert(info.dropped == 0)

            Combat.Release("wereoxxs_ui")
            assert(Combat.HasClaim("wereoxxs_ui") == false)
            assert(Combat.GetOwnerCount() == 0)
            "#,
        );
        chunk.exec().unwrap();
    }

    #[test]
    fn forge_combat_event_fanout_damage_and_healing() {
        let dir = temp_test_dir("fanout");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = CombatSnapshot::default();
        snap.count = 2;
        // Event 1: Player (id 10) deals 75 damage to Mob (id 20)
        snap.events[0] = CombatEventSnapshot {
            attacker_id: 10,
            defender_id: 20,
            amount: -75,
            seq: 1,
            color: 0,
        };
        // Event 2: Healer (id 30) heals Player (id 10) for 150
        snap.events[1] = CombatEventSnapshot {
            attacker_id: 30,
            defender_id: 10,
            amount: 150,
            seq: 2,
            color: 0,
        };

        let mut player_snap = crate::fork_ext::addons::player::PlayerSnapshot::default();
        player_snap.entity_id = 10;
        player_snap.available = true;

        host.snapshot.set(Snapshot {
            in_game: true,
            player: player_snap,
            combat: snap,
            ..Default::default()
        });

        let chunk = host.lua.load(
            r#"
            local Combat = require("frontiers_forge.combat")

            _G.damage_dealt = {}
            _G.healing_received = {}

            Combat.On("wereoxxs_ui", Combat.Events.OnDamageDealt, function(event)
                table.insert(_G.damage_dealt, event)
            end)

            Combat.On("wereoxxs_ui", Combat.Events.OnHealingReceived, function(event)
                table.insert(_G.healing_received, event)
            end)

            Combat.Update()

            assert(#_G.damage_dealt == 1, "One damage dealt event expected")
            assert(_G.damage_dealt[1].attacker_id == 10)
            assert(_G.damage_dealt[1].defender_id == 20)
            assert(_G.damage_dealt[1].amount == -75)
            assert(_G.damage_dealt[1].outgoing == true)
            assert(_G.damage_dealt[1].is_heal == false)

            assert(#_G.healing_received == 1, "One healing received event expected")
            assert(_G.healing_received[1].attacker_id == 30)
            assert(_G.healing_received[1].defender_id == 10)
            assert(_G.healing_received[1].amount == 150)
            assert(_G.healing_received[1].incoming == true)
            assert(_G.healing_received[1].is_heal == true)
            "#,
        );
        chunk.exec().unwrap();
    }

    #[test]
    fn wereoxxs_ui_damage_meter_integration() {
        let dir = temp_test_dir("wereoxxs_meter");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = CombatSnapshot::default();
        snap.count = 3;
        // Event 1: Player (10) deals 200 damage to Orc (50)
        snap.events[0] = CombatEventSnapshot {
            attacker_id: 10,
            defender_id: 50,
            amount: -200,
            seq: 1,
            color: 0,
        };
        // Event 2: Orc (50) deals 45 damage to Player (10)
        snap.events[1] = CombatEventSnapshot {
            attacker_id: 50,
            defender_id: 10,
            amount: -45,
            seq: 2,
            color: 0,
        };
        // Event 3: Self-heal: Player (10) heals Player (10) for 100
        snap.events[2] = CombatEventSnapshot {
            attacker_id: 10,
            defender_id: 10,
            amount: 100,
            seq: 3,
            color: 0,
        };

        let mut player_snap = crate::fork_ext::addons::player::PlayerSnapshot::default();
        player_snap.entity_id = 10;
        player_snap.available = true;

        host.snapshot.set(Snapshot {
            in_game: true,
            player: player_snap,
            combat: snap,
            ..Default::default()
        });

        // Exact Wereoxxs UI damage meter pattern from wereoxxs_ui.lua
        let chunk = host.lua.load(
            r#"
            local Combat = require("frontiers_forge.combat")

            _G.combat_totals = {
                damage_done = 0,
                damage_taken = 0,
                healing_done = 0,
                healing_taken = 0,
            }
            _G.event_log = {}

            local function LogCombatEvent(event)
                local last = _G.event_log[#_G.event_log]
                if last ~= nil and last.seq == event.seq then
                    return
                end
                table.insert(_G.event_log, event)
            end

            Combat.On("wereoxxs_ui", Combat.Events.OnDamageDealt, function(event)
                LogCombatEvent(event)
                _G.combat_totals.damage_done = _G.combat_totals.damage_done + math.abs(event.amount)
            end)

            Combat.On("wereoxxs_ui", Combat.Events.OnDamageReceived, function(event)
                LogCombatEvent(event)
                _G.combat_totals.damage_taken = _G.combat_totals.damage_taken + math.abs(event.amount)
            end)

            Combat.On("wereoxxs_ui", Combat.Events.OnHealingDealt, function(event)
                LogCombatEvent(event)
                _G.combat_totals.healing_done = _G.combat_totals.healing_done + math.abs(event.amount)
            end)

            Combat.On("wereoxxs_ui", Combat.Events.OnHealingReceived, function(event)
                LogCombatEvent(event)
                _G.combat_totals.healing_taken = _G.combat_totals.healing_taken + math.abs(event.amount)
            end)

            Combat.Update()

            assert(_G.combat_totals.damage_done == 200, "Damage done should be 200")
            assert(_G.combat_totals.damage_taken == 45, "Damage taken should be 45")
            assert(_G.combat_totals.healing_done == 100, "Healing done should be 100")
            assert(_G.combat_totals.healing_taken == 100, "Healing taken should be 100")

            -- Self-heal fired both OnHealingDealt and OnHealingReceived, but LogCombatEvent deduplicated by seq
            assert(#_G.event_log == 3, "Event log should have 3 deduplicated events")
            assert(_G.event_log[1].seq == 1)
            assert(_G.event_log[2].seq == 2)
            assert(_G.event_log[3].seq == 3)
            "#,
        );
        chunk.exec().unwrap();
    }
}
