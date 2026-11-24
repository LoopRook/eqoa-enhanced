// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A9: trading, duels and the blacksmith for a mod (spec section 7, "Trade, duels and the blacksmith"): the
//! reads `eqoa.trade.get`, `eqoa.duel.get` and `eqoa.blacksmith.get`, the ten `eqoa.actions` below, and the events
//! `trade.request`, `trade.changed`, `trade.accepted`, `trade.closed`, `duel.changed` and `blacksmith.changed`.
//! Plain data and Lua bindings only: `fork_ext/a9_play.rs` reads the game's own state into the snapshots kept here
//! and carries the actions out through the game's own builders (`trade.rs`, `duel.rs`, `blacksmith.rs`).
//!
//! The windows are held with `eqoa.windows.takeover(kind, {scope = "all"})`, kinds `trade_ask` (someone asks to
//! trade), `trade`, `duel_invite`, `duel` (the wager) and `blacksmith`. A server request names no speaker, so the
//! kinds take scope `all` only (as the A2 dialogs and the A7 prompts do). A mod acts only on a window it holds.
//!
//! What a result means: an action completes when the client has sent the request (`detail.sent`); the server's
//! answer is read afterwards in the state and the events. Nothing is ever sent twice.
//!
//! Safety, kept here and in `a9_play.rs`, never in a mod's Lua: accepting a trade (or a duel's wager) always asks
//! the player first through A1's client-drawn question, which names what each side gives and starts on No; a held
//! trade never accepts by itself; a held request that nobody answers is refused after 45 seconds; and a held
//! window that ends (zoning, logout, death, the mod going away) is refused or handed back to the game's own
//! window, never left hanging.
use super::{
    action_broker::{ActionKind, enqueue_lua_with},
    host::Host,
    native::{bind, describe, error, record},
    native_core::CORE,
};
use mlua::{Result, Value};
use serde_json::{Value as Json, json};
use std::cell::RefCell;
use std::time::{Duration, Instant};

pub(crate) const ROUND: u8 = 8;

/// How long a held trade request or duel invitation waits for the mod before the client refuses it (the game's
/// friend and guild requests wait 45 seconds; a duel invitation lasts about 20, `duel::INVITE_TIME`).
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

/// Most items one side of a trade or a wager holds (`TradeState::add_item`, `Part::item_added`).
pub(crate) const MAX_OFFER: usize = 4;

/// The names of the new actions, as capabilities (`eqoa.actions.<name>`).
pub(crate) const NAMES: [&str; 10] = [
    "trade_answer",
    "trade_add",
    "trade_remove",
    "trade_money",
    "trade_accept",
    "trade_unaccept",
    "trade_cancel",
    "duel_answer",
    "blacksmith_repair",
    "blacksmith_close",
];

/// The actions of round A9, carried by the broker as `ActionKind::A9`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum A9Action {
    /// Yes or No to the request a held `trade_ask` shows.
    TradeAnswer { accept: bool },
    /// `count` of a row of `eqoa.trade.get().bags` onto the player's offer. `slot` is the 0-based row, `id` its item.
    TradeAdd { slot: usize, id: i64, count: i64 },
    /// A row of the player's own offer back (`slot` the 0-based row of `mine.items`).
    TradeRemove { slot: usize, id: i64 },
    /// The coin offered (the whole amount, as the game's money box sends it).
    TradeMoney { amount: i64 },
    /// The player's accept: asks the player first.
    TradeAccept,
    TradeUnaccept,
    TradeCancel,
    /// Yes or No to the invitation a held `duel_invite` shows.
    DuelAnswer { accept: bool },
    /// Repair one row of `eqoa.blacksmith.get().items`.
    /// By the row (`slot`, `id`) or, when `key` is set, by the item's `inventory_key` (the items API's own key).
    BlacksmithRepair { slot: usize, id: i64, key: Option<u32> },
    BlacksmithClose,
}

impl A9Action {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::TradeAnswer { .. } => "trade_answer",
            Self::TradeAdd { .. } => "trade_add",
            Self::TradeRemove { .. } => "trade_remove",
            Self::TradeMoney { .. } => "trade_money",
            Self::TradeAccept => "trade_accept",
            Self::TradeUnaccept => "trade_unaccept",
            Self::TradeCancel => "trade_cancel",
            Self::DuelAnswer { .. } => "duel_answer",
            Self::BlacksmithRepair { .. } => "blacksmith_repair",
            Self::BlacksmithClose => "blacksmith_close",
        }
    }
}

/// The three reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum View {
    Trade,
    Duel,
    Smith,
}

impl View {
    const fn changed_event(self) -> &'static str {
        match self {
            Self::Trade => "trade.changed",
            Self::Duel => "duel.changed",
            Self::Smith => "blacksmith.changed",
        }
    }
    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Default)]
struct Slot {
    state: Option<Json>,
    revision: u64,
    /// A request being waited on: when it began and how long it lasts (`seconds_remaining` is read from it).
    timer: Option<(Instant, Duration)>,
}

#[derive(Default)]
struct Store {
    generation: u64,
    slots: [Slot; 3],
}

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(Store::default());
}

fn emit(name: &str, data: Json) {
    if super::native_events::interested(name) {
        CORE.with(|c| c.borrow_mut().emit(name, data));
    }
}

/// The world changed: nothing of the old trade, duel or smith is left to read.
fn sync_generation() {
    let generation = CORE.with(|c| c.borrow().generation);
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        if s.generation != generation {
            *s = Store { generation, ..Store::default() };
        }
    });
}

/// The client's reading of the game's state. A changed state gets the next revision and the `.changed` event; an
/// unchanged one is left as it is (a mod can tell by `revision` alone). `timer` is a request's own clock.
pub(crate) fn publish(view: View, mut state: Option<Json>, timer: Option<(Instant, Duration)>) {
    sync_generation();
    let (before, after, revision) = STORE.with(|s| {
        let mut s = s.borrow_mut();
        let slot = &mut s.slots[view.index()];
        slot.timer = if state.is_some() { timer } else { None };
        let same = match (&slot.state, &state) {
            (None, None) => true,
            (Some(old), Some(new)) => {
                let mut old = old.clone();
                let mut new = new.clone();
                old["revision"] = Json::Null;
                new["revision"] = Json::Null;
                old == new
            }
            _ => false,
        };
        if same {
            return (None, None, 0);
        }
        slot.revision += 1;
        if let Some(st) = state.as_mut() {
            st["revision"] = json!(slot.revision);
        }
        let before = slot.state.take();
        slot.state = state.clone();
        (Some(before), Some(state), slot.revision)
    });
    let (Some(before), Some(after)) = (before, after) else { return };
    let _ = revision;
    let mut changed: Vec<String> = Vec::new();
    match (&before, &after) {
        (Some(Json::Object(b)), Some(Json::Object(a))) => {
            for (k, v) in a {
                if k != "revision" && b.get(k) != Some(v) {
                    changed.push(k.clone());
                }
            }
            for k in b.keys() {
                if k != "revision" && !a.contains_key(k) {
                    changed.push(k.clone());
                }
            }
        }
        _ => changed.push("state".into()),
    }
    emit(view.changed_event(), json!({"before": before, "after": after, "changed": changed}));
}

/// One of the events a mod listens to that is not a plain `.changed`.
pub(crate) fn emit_event(name: &str, data: Json) {
    emit(name, data);
}

pub(crate) fn current(view: View) -> Option<Json> {
    sync_generation();
    STORE.with(|s| {
        let s = s.borrow();
        let slot = &s.slots[view.index()];
        let mut state = slot.state.clone()?;
        if let Some((began, total)) = slot.timer {
            let left = (total.as_secs_f64() - began.elapsed().as_secs_f64()).max(0.0);
            state["seconds_remaining"] = json!(left);
            state["duration"] = json!(total.as_secs_f64());
        }
        Some(state)
    })
}

/// Nothing is open any more.
pub(crate) fn clear_all() {
    STORE.with(|s| *s.borrow_mut() = Store::default());
}

fn int_arg(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(n) => Some(*n),
        Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e12 => Some(*n as i64),
        _ => None,
    }
}

/// A `{slot = n, id = n}` pick of a listed row: `slot` is 1-based in Lua.
fn pick_of(table: &mlua::Table) -> std::result::Result<(usize, i64), (&'static str, String)> {
    let slot = table.get::<Value>("slot").ok().and_then(|v| int_arg(&v));
    let id = table.get::<Value>("id").ok().and_then(|v| int_arg(&v));
    match (slot, id) {
        (Some(slot), Some(id)) if (1..=100_000).contains(&slot) => Ok((slot as usize - 1, id)),
        _ => Err(("invalid_argument", "pass a row of the read's list: {slot = row (1 or more), id = that row's id}".into())),
    }
}

fn in_world() -> bool {
    CORE.with(|c| c.borrow().view.in_world)
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    for (path, fields) in [
        (
            "eqoa.trade.State",
            "revision kind phase held partner initiated seconds_remaining duration mine theirs bags carried max_items can_accept accept_asked",
        ),
        ("eqoa.trade.Partner", "id name"),
        ("eqoa.trade.Side", "items money accepted"),
        ("eqoa.trade.Item", "slot id name quantity"),
        ("eqoa.trade.Bag", "slot id name quantity worn no_trade can_offer"),
        (
            "eqoa.duel.State",
            "revision enabled state invite queued opponent_id seconds_remaining duration",
        ),
        (
            "eqoa.duel.Invite",
            "id name race_id race_name class_id class_name level guild title",
        ),
        ("eqoa.blacksmith.State", "revision coin items repair_pending total held"),
        ("eqoa.blacksmith.Item", "slot id inventory_key name quantity condition cost can_repair worn"),
    ] {
        describe(lua, path, "supported", Some(ROUND), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(ROUND), "");
        }
    }

    for (module, view, path) in [
        ("trade", View::Trade, "eqoa.trade.State"),
        ("duel", View::Duel, "eqoa.duel.State"),
        ("blacksmith", View::Smith, "eqoa.blacksmith.State"),
    ] {
        bind(
            lua,
            module,
            "get",
            lua.create_function(move |lua, args: mlua::MultiValue| {
                let symbol = format!("eqoa.{module}.get");
                if !in_world() {
                    return error(lua, &symbol, "not_in_world", "enter a loaded world first");
                }
                if !args.is_empty() {
                    return error(lua, &symbol, "invalid_argument", "this function takes no arguments");
                }
                match current(view) {
                    None => lua.pack_multi((Value::Nil, Value::Nil)),
                    Some(state) => lua.pack_multi((record(lua, path, &state)?, Value::Nil)),
                }
            })?,
        )?;
    }

    // no argument
    for (name, action) in [
        ("trade_accept", A9Action::TradeAccept),
        ("trade_unaccept", A9Action::TradeUnaccept),
        ("trade_cancel", A9Action::TradeCancel),
        ("blacksmith_close", A9Action::BlacksmithClose),
    ] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, args: mlua::MultiValue| {
                let symbol = format!("eqoa.actions.{name}");
                enqueue_lua_with(lua, &symbol, || {
                    if !args.is_empty() {
                        return Err(("invalid_argument", "this action takes no arguments".into()));
                    }
                    Ok(ActionKind::A9(action.clone()))
                })
            })?,
        )?;
    }
    // true (yes) or false (no)
    for (name, trade) in [("trade_answer", true), ("duel_answer", false)] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, yes: Value| {
                let symbol = format!("eqoa.actions.{name}");
                enqueue_lua_with(lua, &symbol, || match yes {
                    Value::Boolean(accept) => Ok(ActionKind::A9(if trade {
                        A9Action::TradeAnswer { accept }
                    } else {
                        A9Action::DuelAnswer { accept }
                    })),
                    _ => Err(("invalid_argument", "pass true to accept or false to decline".into())),
                })
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "trade_add",
        lua.create_function(|lua, pick: Value| {
            enqueue_lua_with(lua, "eqoa.actions.trade_add", || {
                let Value::Table(t) = &pick else {
                    return Err((
                        "invalid_argument",
                        "pass a row of eqoa.trade.get().bags: {slot = .., id = .., count = how many (default 1)}".into(),
                    ));
                };
                let (slot, id) = pick_of(t)?;
                let count = match t.get::<Value>("count").map_err(|_| ("invalid_argument", "invalid count".to_string()))? {
                    Value::Nil => 1,
                    v => int_arg(&v).filter(|n| (1..=1_000_000).contains(n)).ok_or((
                        "invalid_argument",
                        "count must be a whole number of 1 or more".to_string(),
                    ))?,
                };
                Ok(ActionKind::A9(A9Action::TradeAdd { slot, id, count }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "blacksmith_repair",
        lua.create_function(|lua, pick: Value| {
            enqueue_lua_with(lua, "eqoa.actions.blacksmith_repair", || {
                let Value::Table(t) = &pick else {
                    return Err((
                        "invalid_argument",
                        "pass a listed row {slot = .., id = ..} or an item's {inventory_key = ..}".into(),
                    ));
                };
                if let Some(v) = t.get::<Value>("inventory_key").ok().filter(|v| !matches!(v, Value::Nil)) {
                    return match int_arg(&v).and_then(|n| u32::try_from(n).ok()) {
                        Some(key) => Ok(ActionKind::A9(A9Action::BlacksmithRepair { slot: 0, id: 0, key: Some(key) })),
                        None => Err(("invalid_argument", "inventory_key must be an item's inventory_key".into())),
                    };
                }
                let (slot, id) = pick_of(t)?;
                Ok(ActionKind::A9(A9Action::BlacksmithRepair { slot, id, key: None }))
            })
        })?,
    )?;
    for (name, trade) in [("trade_remove", true)] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, pick: Value| {
                let symbol = format!("eqoa.actions.{name}");
                enqueue_lua_with(lua, &symbol, || {
                    let Value::Table(t) = &pick else {
                        return Err(("invalid_argument", "pass a listed row: {slot = .., id = ..}".into()));
                    };
                    let (slot, id) = pick_of(t)?;
                    let _ = trade;
                    Ok(ActionKind::A9(A9Action::TradeRemove { slot, id }))
                })
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "trade_money",
        lua.create_function(|lua, amount: Value| {
            enqueue_lua_with(lua, "eqoa.actions.trade_money", || match int_arg(&amount) {
                Some(n) if (0..=i32::MAX as i64).contains(&n) => Ok(ActionKind::A9(A9Action::TradeMoney { amount: n })),
                _ => Err(("invalid_argument", "amount must be a whole number of coin, 0 or more".into())),
            })
        })?,
    )?;
    Ok(())
}

/// The honest Mods & Addons line for a mod that declares any of the steps that change a trade, a duel or what the
/// player owns at the blacksmith (`runtime::disclosure_lines`). Accepting is named in `asks_first_label`.
pub(crate) fn trading_label(caps: &[String]) -> Vec<String> {
    let has = |name: &str| super::native::capability_grants(caps, &format!("eqoa.actions.{name}"));
    let mut lines = Vec::new();
    let mut steps: Vec<&str> = Vec::new();
    for (name, words) in [
        ("trade_answer", "answer trade requests"),
        ("trade_add", "offer items in a trade"),
        ("trade_money", "offer coin in a trade"),
        ("trade_remove", "take back what it offered"),
        ("trade_unaccept", "take back an accept"),
        ("trade_cancel", "cancel a trade"),
        ("duel_answer", "answer duel invitations"),
    ] {
        if has(name) && !steps.contains(&words) {
            steps.push(words);
        }
    }
    if !steps.is_empty() {
        lines.push(format!("Trading: can {}", steps.join(", ")));
    }
    // `blacksmith_repair` is named under "can act on its own" (`runtime::autonomous_action_label`)
    if has("blacksmith_close") {
        lines.push("Blacksmith: can close the blacksmith".to_string());
    }
    lines
}
