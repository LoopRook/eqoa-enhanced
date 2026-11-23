// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A3: the data a unit-frame mod needs (spec section 4 "Frames"). Plain data and Lua bindings only:
//! `fork_ext/a3_capture.rs` fills this from `Play`, so nothing here knows the app.
//!
//! What it holds, and what the client really knows:
//! - the buffs the server's buff window sends (time left, whose, removable, spell id), and the pet's effects;
//! - every cast the server announces (0x00B1: caster, target, duration, effect). The client is **not told**
//!   when a cast is interrupted or lands, so the end of a cast is the end of its time, or a replacing cast, or the
//!   caster leaving;
//! - the quest tracker's goals where the server sends them, the training points, the breath gauge, and the
//!   target's title, guild and consider colour.
//!
//! Time is counted in game steps of `frames.rs`'s 30 Hz clock (`CORE.tick`), never in frames drawn.
use super::{
    host::Host,
    native::{bind, describe, error, record},
    native_core::CORE,
    native_state::Ref,
};
use mlua::{Lua, Result, Value};
use serde_json::{Value as Json, json};
use std::cell::RefCell;

/// The game's step rate: what `Event.tick` and `Client.tick` count.
pub(crate) const TICKS_PER_SECOND: u64 = 30;

/// Whole game steps in `ms` milliseconds, rounded up (a line is not gone before its time is).
pub(crate) fn ticks_of_ms(ms: u64) -> u64 {
    (ms * TICKS_PER_SECOND).div_ceil(1000)
}

fn seconds_of(ticks: u64) -> f64 {
    ticks as f64 / TICKS_PER_SECOND as f64
}

/// One line of the buff window.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BuffLine {
    /// 0 the player's, 1 the pet's (`net::buffs::ON_PLAYER`, `ON_PET`)
    pub whose: u8,
    pub id: u32,
    pub icon: u32,
    pub removable: bool,
    pub name: String,
    /// the game step the line runs out; none where the server keeps no end for it
    pub ends_tick: Option<u64>,
    /// what the client's own ability list says the effect does (an ability of the player's with the effect's
    /// name); none for an effect of someone else's spell the player does not have. Not part of what changes a line.
    pub description: Option<String>,
}

impl BuffLine {
    fn same(&self, other: &Self) -> bool {
        (self.whose, self.id, self.icon, self.removable, &self.name)
            == (other.whose, other.id, other.icon, other.removable, &other.name)
    }
}

/// One of the pet's effects (the strip the server sends apart from the buff window).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PetLine {
    pub icon: u32,
    pub name: String,
    pub ends_tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Goal {
    pub text: String,
    pub have: u16,
    pub need: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrackedQuest {
    pub name: String,
    pub ability: bool,
    pub goals: Vec<Goal>,
}

/// A cast in progress, as announced.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cast {
    pub caster: u32,
    pub target: u32,
    pub targets: Vec<u32>,
    pub effect: u32,
    pub duration_ms: u32,
    pub start_tick: u64,
    pub ends_tick: u64,
    pub own: bool,
}

/// One member of the raid list: where it is and what part it plays. `group` 0 where the server sends none.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RaidRole {
    pub id: u32,
    pub name: String,
    pub group: u8,
    pub class: u8,
    pub merc: bool,
    /// `raidlist::LEADER` 1, `HELPER` 2, `ASSIST` 4
    pub marks: u8,
}

#[derive(Default)]
pub(crate) struct State {
    pub generation: u64,
    pub buffs: Option<Vec<BuffLine>>,
    pub buffs_at: u32,
    pub buff_revision: u64,
    pub pet: Option<Vec<PetLine>>,
    pub pet_at: u32,
    pub goals: Option<Vec<TrackedQuest>>,
    pub casts: Vec<Cast>,
    pub raid: Vec<RaidRole>,
    pub training: Option<(i64, i64, [i64; 7])>,
    pub breath: Option<(f64, Option<bool>)>,
    /// the target's description, or null with no target
    pub target: Json,
    /// what the last build of `goals`, `target` and `raid` was made from: a frame whose source hashes the same
    /// builds nothing (`sync_goals`, `set_target`, `set_raid`)
    pub goals_key: Option<Option<u64>>,
    pub target_key: Option<u64>,
    pub raid_key: Option<u64>,
    /// `buffs_at` / `pet_effects_at` as they stood while the server's window was not on: a set has come when
    /// they differ (the session keeps them across zoning, which clears the sets)
    pub buffs_baseline: Option<u32>,
    pub pet_baseline: Option<u32>,
}

thread_local! {
    pub(crate) static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Forgets everything held (a world left, capture switched off).
pub(crate) fn forget() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

fn tick_and_generation() -> (u64, u64) {
    CORE.with(|c| {
        let c = c.borrow();
        (c.tick, c.generation)
    })
}

/// Starts afresh when the world generation moved, so nothing of an earlier world is read or reported.
pub(crate) fn enter_generation() -> bool {
    let (_, generation) = tick_and_generation();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.generation != generation {
            *s = State { generation, ..State::default() };
            true
        } else {
            false
        }
    })
}

fn emit(name: &str, data: Json) {
    if super::native_events::interested(name) {
        CORE.with(|c| c.borrow_mut().emit(name, data));
    }
}

fn entity_ref(id: u32) -> Json {
    CORE.with(|c| {
        c.borrow()
            .view
            .entities
            .iter()
            .find(|e| e["id"] == id)
            .map_or(Json::Null, |e| e["ref"].clone())
    })
}

// ---- buffs ------------------------------------------------------------------------------------------------------

fn buff_json(line: &BuffLine, index: usize, now: u64, generation: u64, revision: u64) -> Json {
    json!({
        "index": index + 1,
        "whose": if line.whose == 1 { "pet" } else { "player" },
        "name": line.name,
        "icon": line.icon,
        "spell_id": (line.id != 0).then_some(line.id),
        "description": line.description,
        "time_left": line.ends_tick.map(|t| seconds_of(t.saturating_sub(now))),
        "removable": line.removable,
        "ref": {"$ref": "buff", "key": index.to_string(), "generation": generation, "incarnation": revision},
    })
}

fn buffs_json(s: &State, now: u64, generation: u64) -> Json {
    match &s.buffs {
        None => Json::Null,
        Some(lines) => Json::Array(
            lines
                .iter()
                .enumerate()
                .map(|(i, l)| buff_json(l, i, now, generation, s.buff_revision))
                .collect(),
        ),
    }
}

fn same_buffs(a: &Option<Vec<BuffLine>>, b: &Option<Vec<BuffLine>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.same(y)),
        _ => false,
    }
}

/// Brings the buff list up to date. `fresh` is the server's set to take when it is new (`at` differs from the one
/// last taken), or `None` when the buff window is not on; `now` is this game step.
/// Fires `buffs.changed` when a line comes, goes (also by its time running out) or changes.
pub(crate) fn sync_buffs(available: bool, at: u32, fresh: impl FnOnce() -> Vec<BuffLine>) {
    let (now, generation) = tick_and_generation();
    let change = STATE.with(|s| {
        let mut s = s.borrow_mut();
        // nothing is cloned or built while the list is as it was: `before` is made only when something changes
        let mut before_json: Option<Json> = None;
        if !available {
            if s.buffs.is_some() {
                before_json = Some(buffs_json(&s, now, generation));
                s.buffs = None;
            }
            s.buffs_at = 0;
        } else if s.buffs.is_none() || s.buffs_at != at {
            let new = Some(fresh());
            // a re-sent set with the same lines is no change: its end times read straight out of the lines
            if !same_buffs(&s.buffs, &new) {
                before_json = Some(buffs_json(&s, now, generation));
            }
            s.buffs = new;
            s.buffs_at = at;
        }
        // a line whose time ran out is gone, as the game's own window drops it
        if s.buffs.as_ref().is_some_and(|l| l.iter().any(|x| x.ends_tick.is_some_and(|t| t <= now))) {
            if before_json.is_none() {
                before_json = Some(buffs_json(&s, now, generation));
            }
            if let Some(lines) = s.buffs.as_mut() {
                lines.retain(|l| l.ends_tick.is_none_or(|t| t > now));
            }
        }
        let before_json = before_json?;
        s.buff_revision += 1;
        Some((before_json, buffs_json(&s, now, generation)))
    });
    if let Some((before, after)) = change {
        emit("buffs.changed", json!({"before": before, "after": after, "changed": ["records"]}));
    }
}

/// The pet's effects strip. No event: it is read with `eqoa.effects.pet`.
pub(crate) fn sync_pet(available: bool, at: u32, fresh: impl FnOnce() -> Vec<PetLine>) {
    let (now, _) = tick_and_generation();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !available {
            s.pet = None;
            s.pet_at = 0;
        } else if s.pet.is_none() || s.pet_at != at {
            s.pet = Some(fresh());
            s.pet_at = at;
        }
        if let Some(lines) = s.pet.as_mut() {
            lines.retain(|l| l.ends_tick > now);
        }
    });
}

// ---- quest goals ------------------------------------------------------------------------------------------------

fn goals_json(goals: &Option<Vec<TrackedQuest>>) -> Json {
    let journal: Vec<Json> = CORE.with(|c| c.borrow().view.quests.clone());
    match goals {
        None => Json::Null,
        Some(quests) => Json::Array(
            quests
                .iter()
                .map(|q| {
                    let journal_ref = journal
                        .iter()
                        .find(|j| j["title"] == q.name.as_str())
                        .map_or(Json::Null, |j| j["ref"].clone());
                    json!({
                        "title": q.name,
                        "ability": q.ability,
                        "quest": journal_ref,
                        "goals": q.goals.iter().map(|g| json!({
                            "text": g.text,
                            "have": g.have,
                            "need": g.need,
                            "done": g.need > 0 && g.have >= g.need,
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect(),
        ),
    }
}

/// Takes the tracker's set; fires `quests.goals_changed` when it differs from the one held.
/// `hash` stands for the source: a call whose source hashes as the last one's builds nothing.
pub(crate) fn sync_goals(available: bool, hash: u64, fresh: impl FnOnce() -> Vec<TrackedQuest>) {
    let change = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let key = available.then_some(hash);
        if s.goals_key == Some(key) {
            return None;
        }
        s.goals_key = Some(key);
        let new = available.then(fresh);
        if s.goals == new {
            return None;
        }
        let before = goals_json(&s.goals);
        s.goals = new;
        Some((before, goals_json(&s.goals)))
    });
    if let Some((before, after)) = change {
        emit("quests.goals_changed", json!({"before": before, "after": after, "changed": ["records"]}));
    }
}

// ---- the small reads --------------------------------------------------------------------------------------------

/// The training points, the cap the level sets, and each trainable stat's cap with the masteries' bonus.
pub(crate) fn set_training(points: Option<(i64, i64, [i64; 7])>) {
    STATE.with(|s| s.borrow_mut().training = points);
}

pub(crate) fn set_breath(value: Option<(f64, Option<bool>)>) {
    STATE.with(|s| s.borrow_mut().breath = value);
}

/// Builds the target's description only when `hash` of its source is new.
pub(crate) fn set_target(hash: u64, build: impl FnOnce() -> Json) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.target_key != Some(hash) {
            s.target_key = Some(hash);
            s.target = build();
        }
    });
}

/// Builds the raid's roles only when `hash` of its source is new.
pub(crate) fn set_raid(hash: u64, build: impl FnOnce() -> Vec<RaidRole>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.raid_key != Some(hash) {
            s.raid_key = Some(hash);
            s.raid = build();
        }
    });
}

/// Whether the server's buff window (`pet`: the pet's effects) has sent a set in this world. While the window is
/// not on, `at` is noted; a set has come when `at` differs from it (or, with nothing noted, is not 0).
pub(crate) fn set_arrived(pet: bool, window_on: bool, at: u32) -> bool {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let base = if pet { &mut s.pet_baseline } else { &mut s.buffs_baseline };
        if !window_on {
            *base = Some(at);
            return false;
        }
        match *base {
            Some(b) => at != b,
            None => at != 0,
        }
    })
}

fn raid_json(raid: &[RaidRole]) -> Json {
    use crate::raidlist::{ASSIST, HELPER, LEADER, Role, role};
    Json::Array(
        raid.iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "name": m.name,
                    "group": (m.group != 0).then_some(m.group),
                    "role": if m.merc {
                        "merc"
                    } else {
                        match role(m.class) {
                            Role::Tank => "tank",
                            Role::Healer => "healer",
                            Role::Damage | Role::Merc => "damage",
                        }
                    },
                    "class_id": m.class,
                    "leader": m.marks & LEADER != 0,
                    "helper": m.marks & HELPER != 0,
                    "main_assist": m.marks & ASSIST != 0,
                    "merc": m.merc,
                })
            })
            .collect(),
    )
}

// ---- casts ------------------------------------------------------------------------------------------------------

fn cast_json(c: &Cast, now: u64) -> Json {
    let duration = f64::from(c.duration_ms) / 1000.0;
    let remaining = seconds_of(c.ends_tick.saturating_sub(now));
    json!({
        "caster_id": c.caster,
        "caster": entity_ref(c.caster),
        "target_id": (!matches!(c.target, 0 | u32::MAX)).then_some(c.target),
        "targets": c.targets,
        "effect": c.effect,
        "duration": duration,
        "remaining": remaining,
        "fraction": if duration > 0.0 { (1.0 - remaining / duration).clamp(0.0, 1.0) } else { 1.0 },
        "own": c.own,
    })
}

thread_local! {
    /// The entity the player has targeted (0: none), whose casts are written to eqoa.log (`set_watched`).
    static WATCHED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The entity whose casts are logged: the player's target. Set each frame from the capture.
pub(crate) fn set_watched(id: u32) {
    WATCHED.with(|w| w.set(if id == u32::MAX { 0 } else { id }));
}

/// The log line for a cast event on the watched target (or the player's own): the source packet and its fields,
/// so a log can tell a real cast from anything else the server sent. `src` is the packet.
pub(crate) fn cast_log_line(name: &str, c: &Cast, reason: Option<&str>) -> String {
    let mut line = format!(
        "frames: {name} caster={:#x} target={:#x} targets={} effect={:#010x} time={}ms own={} src=0x00B1",
        c.caster,
        c.target,
        c.targets.len(),
        c.effect,
        c.duration_ms,
        c.own
    );
    if let Some(reason) = reason {
        line.push_str(&format!(" reason={reason}"));
    }
    line
}

fn cast_event(name: &str, c: &Cast, reason: Option<&str>) {
    if c.own || (c.caster != 0 && WATCHED.with(|w| w.get()) == c.caster) {
        eprintln!("{}", cast_log_line(name, c, reason));
    }
    let mut data = json!({
        "caster_id": c.caster,
        "caster": entity_ref(c.caster),
        "target_id": (!matches!(c.target, 0 | u32::MAX)).then_some(c.target),
        "targets": c.targets,
        "effect": c.effect,
        "duration": f64::from(c.duration_ms) / 1000.0,
        "own": c.own,
    });
    if let Some(reason) = reason {
        data["reason"] = json!(reason);
    }
    emit(name, data);
}

/// A cast the server announced (0x00B1). Fires `cast.started`; a cast with no time (`duration_ms` 0) is announced
/// and not tracked. A caster's earlier cast still running ends as `replaced`.
pub(crate) fn cast_started(
    caster: u32,
    target: u32,
    targets: Vec<u32>,
    effect: u32,
    duration_ms: u32,
    own: bool,
) {
    enter_generation();
    let (now, _) = tick_and_generation();
    let cast = Cast {
        caster,
        target,
        targets,
        effect,
        duration_ms,
        start_tick: now,
        ends_tick: now + ticks_of_ms(u64::from(duration_ms)),
        own,
    };
    let replaced = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let old = s.casts.iter().position(|c| c.caster == caster).map(|i| s.casts.remove(i));
        if duration_ms > 0 {
            s.casts.push(cast.clone());
        }
        old
    });
    if let Some(old) = replaced {
        cast_event("cast.ended", &old, Some("replaced"));
    }
    cast_event("cast.started", &cast, None);
}

/// Ends casts whose time ran out (`elapsed`), whose caster `gone` says left, or the player's own when `dead`.
pub(crate) fn expire_casts(gone: impl Fn(u32) -> bool, dead: bool) {
    let (now, _) = tick_and_generation();
    let ended: Vec<(Cast, &'static str)> = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut ended = Vec::new();
        s.casts.retain(|c| {
            let reason = if c.ends_tick <= now {
                Some("elapsed")
            } else if c.own && dead {
                Some("dead")
            } else if !c.own && gone(c.caster) {
                Some("gone")
            } else {
                None
            };
            match reason {
                Some(r) => {
                    ended.push((c.clone(), r));
                    false
                }
                None => true,
            }
        });
        ended
    });
    for (cast, reason) in ended {
        cast_event("cast.ended", &cast, Some(reason));
    }
}

// ---- reading a buff reference (for `remove_effect`) --------------------------------------------------------------

/// Resolves a `BuffRef` against the list now: the line it named, or why not.
pub(crate) fn resolve_buff(value: &Value) -> std::result::Result<(usize, BuffLine), (&'static str, String)> {
    let Some(ud) = value.as_userdata() else {
        return Err(("invalid_argument", "expected a buff reference from eqoa.effects.buffs()".into()));
    };
    let Ok(r) = ud.borrow::<Ref>() else {
        return Err(("invalid_argument", "expected a buff reference from eqoa.effects.buffs()".into()));
    };
    if r.kind != "buff" {
        return Err(("invalid_argument", format!("expected a buff reference, got a {} reference", r.kind)));
    }
    let (now, generation) = tick_and_generation();
    STATE.with(|s| {
        let s = s.borrow();
        let stale = || ("stale_handle", "the buff list changed since this reference was read".to_string());
        if r.generation != generation || r.incarnation != s.buff_revision {
            return Err(stale());
        }
        let index: usize = r.key.parse().map_err(|_| stale())?;
        let line = s.buffs.as_ref().and_then(|l| l.get(index)).ok_or_else(stale)?;
        if line.ends_tick.is_some_and(|t| t <= now) {
            return Err(stale());
        }
        Ok((index, line.clone()))
    })
}

/// The tick the lines are counted by, for tests and the capture.
pub(crate) fn now_tick() -> u64 {
    tick_and_generation().0
}

// ---- Lua ---------------------------------------------------------------------------------------------------------

fn in_world() -> bool {
    CORE.with(|c| c.borrow().view.in_world)
}

fn not_in_world(lua: &Lua, symbol: &str) -> Result<mlua::MultiValue> {
    error(lua, symbol, "not_in_world", "enter a loaded world first")
}

fn answer(lua: &Lua, symbol: &str, path: &str, value: Option<Json>, why: &str) -> Result<mlua::MultiValue> {
    if !in_world() {
        return not_in_world(lua, symbol);
    }
    match value {
        Some(v) => lua.pack_multi((record(lua, path, &v)?, Value::Nil)),
        None => error(lua, symbol, "unavailable", why),
    }
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;

    bind(
        lua,
        "effects",
        "buffs",
        lua.create_function(|lua, ()| {
            enter_generation();
            let (now, generation) = tick_and_generation();
            let value = STATE.with(|s| {
                let s = s.borrow();
                s.buffs.as_ref().map(|_| buffs_json(&s, now, generation))
            });
            answer(
                lua,
                "eqoa.effects.buffs",
                "eqoa.effects.Buff",
                value,
                "this server sends no buff window, or Buff Window is off in Options, or the set has not arrived",
            )
        })?,
    )?;
    bind(
        lua,
        "effects",
        "pet",
        lua.create_function(|lua, ()| {
            let (now, _) = tick_and_generation();
            let value = STATE.with(|s| {
                s.borrow().pet.as_ref().map(|lines| {
                    Json::Array(
                        lines
                            .iter()
                            .enumerate()
                            .map(|(i, l)| {
                                json!({
                                    "index": i + 1,
                                    "name": l.name,
                                    "icon": l.icon,
                                    "time_left": seconds_of(l.ends_tick.saturating_sub(now)),
                                })
                            })
                            .collect(),
                    )
                })
            });
            answer(
                lua,
                "eqoa.effects.pet",
                "eqoa.effects.PetEffect",
                value,
                "this server sends no pet effects, or Pet Effects is off in Options",
            )
        })?,
    )?;

    bind(
        lua,
        "casts",
        "list",
        lua.create_function(|lua, ()| {
            let (now, _) = tick_and_generation();
            let value = STATE.with(|s| Json::Array(s.borrow().casts.iter().map(|c| cast_json(c, now)).collect()));
            answer(lua, "eqoa.casts.list", "eqoa.casts.Cast", Some(value), "")
        })?,
    )?;
    bind(
        lua,
        "casts",
        "get",
        lua.create_function(|lua, who: Value| {
            if !in_world() {
                return not_in_world(lua, "eqoa.casts.get");
            }
            let id = match &who {
                Value::Integer(n) => u32::try_from(*n).ok(),
                Value::Number(n) if n.fract() == 0.0 && *n >= 0.0 => u32::try_from(*n as i64).ok(),
                Value::UserData(_) => {
                    let wanted = who.as_userdata().and_then(|u| u.borrow::<Ref>().ok().map(|r| r.clone()));
                    match wanted {
                        Some(r) if r.kind == "entity" => {
                            let found = CORE.with(|c| {
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
                            });
                            match found {
                                Some(id) => u32::try_from(id).ok(),
                                None => return error(lua, "eqoa.casts.get", "stale_handle", "entity handle is stale"),
                            }
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let Some(id) = id else {
                return error(
                    lua,
                    "eqoa.casts.get",
                    "invalid_argument",
                    "expected an EntityRef or an entity id",
                );
            };
            let (now, _) = tick_and_generation();
            let value = STATE.with(|s| s.borrow().casts.iter().find(|c| c.caster == id).map(|c| cast_json(c, now)));
            match value {
                Some(v) => lua.pack_multi((record(lua, "eqoa.casts.Cast", &v)?, Value::Nil)),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;

    bind(
        lua,
        "quests",
        "goals",
        lua.create_function(|lua, ()| {
            let value = STATE.with(|s| {
                let s = s.borrow();
                s.goals.as_ref().map(|_| goals_json(&s.goals))
            });
            answer(
                lua,
                "eqoa.quests.goals",
                "eqoa.quests.QuestGoals",
                value,
                "this server sends no quest goals, or Quest Tracker is off in Options",
            )
        })?,
    )?;
    bind(
        lua,
        "group",
        "raid_roles",
        lua.create_function(|lua, ()| {
            let value = STATE.with(|s| raid_json(&s.borrow().raid));
            answer(lua, "eqoa.group.raid_roles", "eqoa.group.RaidRole", Some(value), "")
        })?,
    )?;
    bind(
        lua,
        "player",
        "training",
        lua.create_function(|lua, ()| {
            let value = STATE
                .with(|s| s.borrow().training)
                .map(|(points, cap, caps)| {
                    json!({"points": points, "stat_cap": cap, "caps": {
                        "strength": caps[0], "stamina": caps[1], "agility": caps[2], "dexterity": caps[3],
                        "wisdom": caps[4], "intelligence": caps[5], "charisma": caps[6],
                    }})
                });
            answer(lua, "eqoa.player.training", "eqoa.player.Training", value, "the client has no training points yet")
        })?,
    )?;
    bind(
        lua,
        "player",
        "breath",
        lua.create_function(|lua, ()| {
            let value = STATE
                .with(|s| s.borrow().breath)
                .map(|(fraction, underwater)| json!({"fraction": fraction, "underwater": underwater}));
            answer(lua, "eqoa.player.breath", "eqoa.player.Breath", value, "the client has no breath value yet")
        })?,
    )?;
    bind(
        lua,
        "target",
        "info",
        lua.create_function(|lua, ()| {
            if !in_world() {
                return not_in_world(lua, "eqoa.target.info");
            }
            let value = STATE.with(|s| s.borrow().target.clone());
            if value.is_null() {
                return lua.pack_multi((Value::Nil, Value::Nil));
            }
            lua.pack_multi((record(lua, "eqoa.target.TargetInfo", &value)?, Value::Nil))
        })?,
    )?;

    for (path, fields) in [
        ("eqoa.effects.Buff", "index whose name icon spell_id description time_left removable ref"),
        ("eqoa.effects.PetEffect", "index name icon time_left"),
        (
            "eqoa.casts.Cast",
            "caster_id caster target_id targets effect duration remaining fraction own",
        ),
        ("eqoa.quests.QuestGoals", "title ability quest goals"),
        ("eqoa.quests.QuestGoal", "text have need done"),
        ("eqoa.player.Training", "points stat_cap caps"),
        ("eqoa.player.Breath", "fraction underwater"),
        ("eqoa.target.TargetInfo", "id title guild consider"),
        ("eqoa.target.Consider", "code name color"),
        (
            "eqoa.group.RaidRole",
            "id name group role class_id leader helper main_assist merc",
        ),
    ] {
        describe(lua, path, "supported", Some(8), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(8), "");
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    forget();
}
