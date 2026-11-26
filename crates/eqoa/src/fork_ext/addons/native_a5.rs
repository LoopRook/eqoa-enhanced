// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A5: the character window's contract (spec sections 4 "Training", 7 "Class Mastery" and 8): spending training
//! points, the faction standings and the combat record. Plain data and Lua bindings only: `fork_ext/a5_capture.rs`
//! fills the reads from `Play` and carries the actions out, so nothing here knows the app.
//!
//! What a result means: `train_stats` (like `mastery_buy` and `mastery_refund`, which round A5 makes supported) never
//! acts at once. It completes when the client has put its own "are you sure?" to the player (`detail.asked`, see
//! `confirm.rs`); only the player's own Yes sends the request, and the mod hears that as `confirm.closed` with
//! `detail.sent` (never `confirmed`: the server's answer is the training points and stats changing). The two asks
//! (`request_factions`, `request_combat_record`) complete when the client has sent them; the server's answer is what
//! `eqoa.factions.list()` and `eqoa.combat.record()` then read.
use super::{
    action_broker::{ActionKind, enqueue_lua, enqueue_lua_with},
    host::Host,
    native::{bind, describe, error, record},
    native_core::CORE,
};
use mlua::{Lua, Result, Value};
use serde_json::{Value as Json, json};
use std::cell::RefCell;

/// The seven trainable stats, in the game's order (`trainer.rs` `STATS`: ids 0 to 6) and as `SevenStats` names them.
pub(crate) const STAT_NAMES: [&str; 7] = ["strength", "stamina", "agility", "dexterity", "wisdom", "intelligence", "charisma"];

/// Most points one `train_stats` call may name for one stat (a sanity bound; the points the character has decide).
const MAX_POINTS: i64 = 100_000;

/// What the broker carries for the actions of round A5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum A5Action {
    /// Asks the player first: spend these points, by stat in `STAT_NAMES` order.
    TrainStats { points: [i32; 7] },
    /// The game's faction page `page` (0 is the first; the game's own numbering on the wire).
    RequestFactions { page: i32 },
    RequestCombatRecord,
}

impl A5Action {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::TrainStats { .. } => "train_stats",
            Self::RequestFactions { .. } => "request_factions",
            Self::RequestCombatRecord => "request_combat_record",
        }
    }
}

#[derive(Default)]
pub(crate) struct State {
    generation: u64,
    /// The faction page the server last sent for the last request, as `eqoa.factions.FactionPage`.
    factions: Option<Json>,
    factions_key: Option<u64>,
    /// The record the server last sent, as `eqoa.combat.CombatRecord`.
    record: Option<Json>,
    record_key: Option<u64>,
    /// The server offers a combat record (`Session::combat_record`).
    record_offered: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Forgets everything held (a world left, capture switched off).
pub(crate) fn forget() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

/// Starts afresh when the world generation moved, so nothing of an earlier world is read.
pub(crate) fn enter_generation() {
    let generation = CORE.with(|c| c.borrow().generation);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.generation != generation {
            *s = State { generation, ..State::default() };
        }
    });
}

/// Takes the faction page held by the game. `key` stands for the source: a call whose source is the last one's
/// builds nothing.
pub(crate) fn set_factions(key: u64, build: impl FnOnce() -> Option<Json>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.factions_key != Some(key) {
            s.factions_key = Some(key);
            let page = build();
            // `factions.changed`: the page the read returns is not the one it returned
            if page != s.factions {
                s.factions = page;
                if super::change_events::wanted(super::change_events::Sys::Factions) {
                    let revision = super::change_events::next_revision(super::change_events::Sys::Factions);
                    super::change_events::note(super::change_events::Sys::Factions, revision, s.factions.is_some());
                }
            }
        }
    });
}

/// Takes the combat record held by the game and whether the server offers one.
pub(crate) fn set_record(key: u64, offered: bool, build: impl FnOnce() -> Option<Json>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.record_offered = offered;
        if s.record_key != Some(key) {
            s.record_key = Some(key);
            s.record = build();
        }
    });
}

fn in_world() -> bool {
    CORE.with(|c| c.borrow().view.in_world)
}

fn answer(lua: &Lua, symbol: &str, path: &str, value: Option<Json>, why: &str) -> Result<mlua::MultiValue> {
    if !in_world() {
        return error(lua, symbol, "not_in_world", "enter a loaded world first");
    }
    match value {
        Some(v) => lua.pack_multi((record(lua, path, &v)?, Value::Nil)),
        None => error(lua, symbol, "unavailable", why),
    }
}

/// The stat counts of a `train_stats` call: a table of stat name to a whole number of points.
fn parse_points(v: &Value) -> std::result::Result<[i32; 7], (&'static str, String)> {
    let Value::Table(t) = v else {
        return Err(("invalid_argument", "points must be a table such as {strength = 1, stamina = 2}".into()));
    };
    let mut points = [0i32; 7];
    for pair in t.pairs::<Value, Value>() {
        let (k, n) = pair.map_err(|_| ("invalid_argument", "points must be a table of stat names".to_string()))?;
        let name = k.as_string().and_then(|s| s.to_str().ok().map(|s| s.to_string()));
        let Some(index) = name.as_deref().and_then(|n| STAT_NAMES.iter().position(|s| *s == n)) else {
            return Err(("invalid_argument", format!("a stat is one of: {}", STAT_NAMES.join(", "))));
        };
        let n = match n {
            Value::Integer(n) => n,
            Value::Number(n) if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e9 => n as i64,
            _ => return Err(("invalid_argument", format!("{} must be a whole number of points", STAT_NAMES[index]))),
        };
        if !(0..=MAX_POINTS).contains(&n) {
            return Err(("out_of_range", format!("{} must be 0 to {MAX_POINTS} points", STAT_NAMES[index])));
        }
        points[index] = n as i32;
    }
    if points.iter().all(|p| *p == 0) {
        return Err(("invalid_argument", "name at least one point to spend".into()));
    }
    Ok(points)
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    for (path, fields) in [
        ("eqoa.factions.FactionPage", "page pages entries"),
        ("eqoa.factions.Faction", "name standing"),
        ("eqoa.combat.CombatRecord", "name rows"),
        ("eqoa.combat.CombatRow", "heading label value"),
    ] {
        describe(lua, path, "supported", Some(8), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(8), "");
        }
    }
    bind(
        lua,
        "actions",
        "train_stats",
        lua.create_function(|lua, points: Value| {
            enqueue_lua_with(lua, "eqoa.actions.train_stats", || {
                Ok(ActionKind::A5(A5Action::TrainStats { points: parse_points(&points)? }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "request_factions",
        lua.create_function(|lua, page: Option<Value>| {
            enqueue_lua_with(lua, "eqoa.actions.request_factions", || {
                let page = match page {
                    None | Some(Value::Nil) => 1,
                    Some(Value::Integer(n)) => n,
                    Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e9 => n as i64,
                    Some(_) => return Err(("invalid_argument", "page must be a whole number from 1".into())),
                };
                if !(1..=10_000).contains(&page) {
                    return Err(("out_of_range", "page must be 1 or more".into()));
                }
                Ok(ActionKind::A5(A5Action::RequestFactions { page: page as i32 - 1 }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "request_combat_record",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.actions.request_combat_record", ActionKind::A5(A5Action::RequestCombatRecord))
        })?,
    )?;
    bind(
        lua,
        "factions",
        "list",
        lua.create_function(|lua, ()| {
            let value = STATE.with(|s| s.borrow().factions.clone());
            answer(
                lua,
                "eqoa.factions.list",
                "eqoa.factions.FactionPage",
                value,
                "no faction page has arrived: ask for one with eqoa.actions.request_factions",
            )
        })?,
    )?;
    bind(
        lua,
        "combat",
        "record",
        lua.create_function(|lua, ()| {
            let (value, offered) = STATE.with(|s| {
                let s = s.borrow();
                (s.record.clone(), s.record_offered)
            });
            let why = if offered {
                "no combat record has arrived: ask for one with eqoa.actions.request_combat_record"
            } else {
                "this server keeps no combat record"
            };
            answer(lua, "eqoa.combat.record", "eqoa.combat.CombatRecord", value, why)
        })?,
    )?;
    Ok(())
}

/// The page a faction reply names, as the mod reads it: from 1, with the page count (0 where the server has not
/// said).
pub(crate) fn page_json(page: i32, pages: i32, entries: Vec<(String, i32)>) -> Json {
    json!({
        "page": page.max(0) + 1,
        "pages": pages.max(0),
        "entries": entries.into_iter().map(|(name, standing)| json!({"name": name, "standing": standing})).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    forget();
}
