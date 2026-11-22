// Copyright LoopRook. SPDX-License-Identifier: AGPL-3.0-or-later
//! Raid list, ready-check, loot-roll strip session data, and master-loot state/actions (round A4).
//!
//! Kept in its own file with strip data shaped like the window-replacement design's session table
//! (`{kind, origin, revision, data}`) so ready checks and loot rolls can be wired onto A1's
//! `windows.session` contract cleanly after A1 merges.

use super::{
    action_broker::{ActionKind, enqueue_lua_with},
    host::Host,
    native::{bind, describe, error, record},
    native_core::CORE,
    native_items::resolve_item_handle,
    native_state::Ref,
};
use mlua::{Lua, Result, Value};
use serde_json::{Value as Json, json};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RaidMemberSnap {
    pub id: u32,
    pub name: String,
    pub hp_fraction: f32,
    pub power_fraction: f32,
    pub group: u8,
    pub merc: bool,
    pub leader: bool,
    pub helper: bool,
    pub assist: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReadyCheckSnap {
    pub id: u32,
    pub asker: String,
    pub seconds_remaining: f32,
    pub duration: f32,
    pub deadline_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LootRollEntry {
    pub id: u32,
    pub item_name: String,
    pub item_id: String,
    pub icon: Option<u32>,
    pub may_need: bool,
    pub seconds_remaining: f32,
    pub duration: f32,
    pub deadline_ms: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RaidSnapshot {
    pub in_raid: bool,
    pub leader_id: Option<u32>,
    pub helper_id: Option<u32>,
    pub assist_id: Option<u32>,
    pub loot_rule: &'static str,
    pub in_combat: bool,
    pub leads: bool,
    pub helps: bool,
    pub members: Vec<RaidMemberSnap>,
    pub ready: Option<ReadyCheckSnap>,
    pub rolls: Vec<LootRollEntry>,
}

#[derive(Default)]
struct RaidStripStore {
    snapshot: RaidSnapshot,
    ready_key: Option<(u32, String, i64)>,
    ready_revision: u64,
    rolls_key: Vec<(u32, i64, bool)>,
    rolls_revision: u64,
    master_loot: Option<net::raid::MasterLoot>,
}

thread_local! {
    static STORE: RefCell<RaidStripStore> = RefCell::new(RaidStripStore::default());
}

pub(crate) fn update(snapshot: RaidSnapshot) {
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        let next_ready = snapshot
            .ready
            .as_ref()
            .map(|r| (r.id, r.asker.clone(), r.deadline_ms));
        if next_ready != s.ready_key {
            s.ready_key = next_ready;
            s.ready_revision = s.ready_revision.saturating_add(1);
        }
        let next_rolls: Vec<(u32, i64, bool)> = snapshot
            .rolls
            .iter()
            .map(|r| (r.id, r.deadline_ms, r.may_need))
            .collect();
        if next_rolls != s.rolls_key {
            s.rolls_key = next_rolls;
            s.rolls_revision = s.rolls_revision.saturating_add(1);
        }
        s.snapshot = snapshot;
    });
}

pub(crate) fn set_master_loot_detail(detail: Option<net::raid::MasterLoot>) {
    STORE.with(|s| s.borrow_mut().master_loot = detail);
}

pub(crate) fn master_loot_detail() -> Option<net::raid::MasterLoot> {
    STORE.with(|s| s.borrow().master_loot.clone())
}

pub(crate) fn reset() {
    STORE.with(|s| *s.borrow_mut() = RaidStripStore::default());
}

/// Session-shaped table for an open ready check (matches `window-replacement-design.md`).
pub(crate) fn ready_session_json() -> Option<Json> {
    STORE.with(|s| {
        let s = s.borrow();
        let r = s.snapshot.ready.as_ref()?;
        Some(json!({
            "kind": "raid_ready",
            "origin": "server",
            "revision": s.ready_revision,
            "data": {
                "id": r.id,
                "asker": r.asker,
                "seconds_remaining": r.seconds_remaining,
                "duration": r.duration,
                "deadline_ms": r.deadline_ms,
                "choices": ["ready", "not_ready"],
            }
        }))
    })
}

/// Session-shaped table for open loot rolls (matches `window-replacement-design.md`).
pub(crate) fn roll_session_json() -> Option<Json> {
    STORE.with(|s| {
        let s = s.borrow();
        if s.snapshot.rolls.is_empty() {
            return None;
        }
        let rolls: Vec<Json> = s
            .snapshot
            .rolls
            .iter()
            .map(|r| {
                let choices: &[&str] = if r.may_need {
                    &["need", "greed", "pass"]
                } else {
                    &["greed", "pass"]
                };
                json!({
                    "id": r.id,
                    "item_name": r.item_name,
                    "item_id": r.item_id,
                    "icon": r.icon,
                    "may_need": r.may_need,
                    "choices": choices,
                    "seconds_remaining": r.seconds_remaining,
                    "duration": r.duration,
                    "deadline_ms": r.deadline_ms,
                })
            })
            .collect();
        Some(json!({
            "kind": "raid_roll",
            "origin": "server",
            "revision": s.rolls_revision,
            "data": {
                "rolls": rolls,
            }
        }))
    })
}

fn raid_state_json() -> Json {
    let ready_check = ready_session_json().unwrap_or(Json::Null);
    let loot_rolls = roll_session_json().unwrap_or(Json::Null);
    STORE.with(|s| {
        let s = s.borrow();
        let snap = &s.snapshot;
        let members: Vec<Json> = snap
            .members
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "name": m.name,
                    "hp_fraction": m.hp_fraction,
                    "power_fraction": m.power_fraction,
                    "group": m.group,
                    "merc": m.merc,
                    "leader": m.leader,
                    "helper": m.helper,
                    "assist": m.assist,
                })
            })
            .collect();
        json!({
            "in_raid": snap.in_raid,
            "leader_id": snap.leader_id,
            "helper_id": snap.helper_id,
            "assist_id": snap.assist_id,
            "loot_rule": if snap.loot_rule.is_empty() { "free_for_all" } else { snap.loot_rule },
            "in_combat": snap.in_combat,
            "leads": snap.leads,
            "helps": snap.helps,
            "members": members,
            "ready_check": ready_check,
            "loot_rolls": loot_rolls,
        })
    })
}

fn parse_u32_id(val: &Value, what: &str) -> std::result::Result<u32, (&'static str, String)> {
    match val {
        Value::Integer(n) if *n > 0 && *n <= u32::MAX as i64 => Ok(*n as u32),
        Value::Number(n)
            if n.is_finite() && n.fract() == 0.0 && *n >= 1.0 && *n <= u32::MAX as f64 =>
        {
            Ok(*n as u32)
        }
        _ => Err(("invalid_argument", format!("{what} must be a positive integer id"))),
    }
}

fn parse_member_id(val: &Value) -> std::result::Result<u32, (&'static str, String)> {
    if let Some(ud) = val.as_userdata() {
        let Ok(r) = ud.borrow::<Ref>() else {
            return Err(("invalid_argument", "member must be a player id or EntityRef".into()));
        };
        if r.kind != "entity" {
            return Err((
                "invalid_argument",
                format!("member must be a player id or EntityRef, got {}", r.kind),
            ));
        }
        let resolved = CORE.with(|c| {
            let c = c.borrow();
            if r.generation != c.generation {
                return None;
            }
            c.view
                .entities
                .iter()
                .find(|e| {
                    e["ref"]["key"].as_str() == Some(r.key.as_str())
                        && e["ref"]["incarnation"].as_u64() == Some(r.incarnation)
                })
                .and_then(|e| e["id"].as_u64())
                .and_then(|id| u32::try_from(id).ok())
        });
        return resolved.ok_or(("stale_handle", "entity handle is stale or invalid".to_string()));
    }
    parse_u32_id(val, "member")
}

fn install_fields(lua: &Lua) {
    for (path, fields) in [
        (
            "eqoa.raid.RaidState",
            "in_raid leader_id helper_id assist_id loot_rule in_combat leads helps members ready_check loot_rolls",
        ),
        (
            "eqoa.raid.RaidMember",
            "id name hp_fraction power_fraction group merc leader helper assist",
        ),
        (
            "eqoa.raid.ReadyCheckSession",
            "kind origin revision data",
        ),
        (
            "eqoa.raid.ReadyCheckData",
            "id asker seconds_remaining duration deadline_ms choices",
        ),
        (
            "eqoa.raid.LootRollSession",
            "kind origin revision data",
        ),
        ("eqoa.raid.LootRollData", "rolls"),
        (
            "eqoa.raid.LootRollItem",
            "id item_name item_id icon may_need choices seconds_remaining duration deadline_ms",
        ),
    ] {
        describe(lua, path, "supported", Some(7), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(7), "");
        }
    }
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    install_fields(lua);

    bind(
        lua,
        "raid",
        "get",
        lua.create_function(|lua, args: mlua::MultiValue| {
            if !args.is_empty() {
                return error(lua, "eqoa.raid.get", "invalid_argument", "this function takes no arguments");
            }
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(lua, "eqoa.raid.get", "not_in_world", "enter a loaded world first");
            }
            let state = raid_state_json();
            lua.pack_multi((record(lua, "eqoa.raid.RaidState", &state)?, Value::Nil))
        })?,
    )?;

    bind(
        lua,
        "raid",
        "ready_check",
        lua.create_function(|lua, args: mlua::MultiValue| {
            if !args.is_empty() {
                return error(lua, "eqoa.raid.ready_check", "invalid_argument", "this function takes no arguments");
            }
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(lua, "eqoa.raid.ready_check", "not_in_world", "enter a loaded world first");
            }
            match ready_session_json() {
                Some(v) => lua.pack_multi((record(lua, "eqoa.raid.ReadyCheckSession", &v)?, Value::Nil)),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;

    bind(
        lua,
        "raid",
        "loot_rolls",
        lua.create_function(|lua, args: mlua::MultiValue| {
            if !args.is_empty() {
                return error(lua, "eqoa.raid.loot_rolls", "invalid_argument", "this function takes no arguments");
            }
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(lua, "eqoa.raid.loot_rolls", "not_in_world", "enter a loaded world first");
            }
            match roll_session_json() {
                Some(v) => lua.pack_multi((record(lua, "eqoa.raid.LootRollSession", &v)?, Value::Nil)),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;

    bind(
        lua,
        "actions",
        "ready_check_answer",
        lua.create_function(|lua, (id, ready): (Value, Value)| {
            enqueue_lua_with(lua, "eqoa.actions.ready_check_answer", || {
                let id = parse_u32_id(&id, "id")?;
                let Some(ready) = ready.as_boolean() else {
                    return Err(("invalid_argument", "ready must be a boolean".into()));
                };
                Ok(ActionKind::ReadyCheckAnswer { id, ready })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "loot_roll",
        lua.create_function(|lua, (id, choice): (Value, Value)| {
            enqueue_lua_with(lua, "eqoa.actions.loot_roll", || {
                let id = parse_u32_id(&id, "id")?;
                let Some(choice_str) = choice
                    .as_string()
                    .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
                else {
                    return Err((
                        "invalid_argument",
                        "choice must be 'need', 'greed', or 'pass'".into(),
                    ));
                };
                let choice_code = match choice_str.as_str() {
                    "need" => 1u8,
                    "greed" => 2u8,
                    "pass" => 0u8,
                    _ => {
                        return Err((
                            "invalid_argument",
                            format!("choice must be 'need', 'greed', or 'pass', got '{choice_str}'"),
                        ));
                    }
                };
                Ok(ActionKind::LootRoll {
                    id,
                    choice: choice_code,
                })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "loot_assign",
        lua.create_function(|lua, (item, member): (Value, Value)| {
            enqueue_lua_with(lua, "eqoa.actions.loot_assign", || {
                let spec = resolve_item_handle(&item)?;
                if spec.list != "loot" {
                    return Err((
                        "invalid_argument",
                        format!("loot_assign requires a loot item handle, got '{}'", spec.list),
                    ));
                }
                let member_id = parse_member_id(&member)?;
                Ok(ActionKind::LootAssign {
                    item: spec,
                    member_id,
                })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "loot_abandon",
        lua.create_function(|lua, item: Option<Value>| {
            enqueue_lua_with(lua, "eqoa.actions.loot_abandon", || {
                let spec = match item {
                    None | Some(Value::Nil) => None,
                    Some(v) => {
                        let s = resolve_item_handle(&v)?;
                        if s.list != "loot" {
                            return Err((
                                "invalid_argument",
                                format!("loot_abandon item must be a loot item handle, got '{}'", s.list),
                            ));
                        }
                        Some(s)
                    }
                };
                Ok(ActionKind::LootAbandon { item: spec })
            })
        })?,
    )?;

    Ok(())
}
