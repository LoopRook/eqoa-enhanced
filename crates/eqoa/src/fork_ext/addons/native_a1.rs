// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1: the Lua side of the modern UI's contract: window sessions (`eqoa.windows.open_session`, `session`,
//! `close_session`, `pass`, `focus`, `release_focus`, `confirm`), the three item actions the Bags window needs that
//! `eqoa/1` did not have (`destroy`, `link_item`, `hide_look`) and the "are you sure?" that `destroy` and
//! `drop_quest` always ask (`confirm.rs`). Plain data and bindings only: `fork_ext/a1_play.rs` carries the actions
//! out and ends the sessions, so nothing here knows the app.
//!
//! What a result means: `destroy` and `drop_quest` complete when the question has been put to the player
//! (`detail.asked`); nothing is destroyed or dropped until the player's own Yes, which the mod learns from the
//! `confirm.closed` event (`answer` = `yes`, `no`, `expired`, or the reason the window ended it) and the usual
//! `inventory.changed` / `quests.changed`. `link_item` and `hide_look` complete when the client has sent them.
use super::{
    action_broker::{ActionKind, enqueue_lua_with},
    confirm,
    host::Host,
    native::{bind, describe, error, record, require_capability, session},
    native_items::{ResolvedItemSpec, resolve_item_handle},
    window_sessions,
};
use mlua::{Lua, Result, Value};
use serde_json::json;

/// The item actions of round A1, carried by the broker as `ActionKind::A1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum A1Action {
    /// Asks the player first. `count` none is the whole stack.
    Destroy { item: ResolvedItemSpec, count: Option<i64> },
    /// Bag Viewer multi-select: asks once about several whole stacks (`destroy_many`).
    DestroyMany { items: Vec<ResolvedItemSpec> },
    LinkItem { item: ResolvedItemSpec },
    HideLook { item: ResolvedItemSpec, hide: bool },
    /// Round A2: `dialog_answer(ref, option)` (`Some`, 1-based) or `close_window(ref)` (`None`) on a dialog or offer
    /// session. Carried out by `fork_ext/a2_play.rs` with the game's own keys.
    SessionAnswer { session: String, option: Option<usize> },
}

impl A1Action {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::Destroy { .. } => "destroy",
            Self::DestroyMany { .. } => "destroy_many",
            Self::LinkItem { .. } => "link_item",
            Self::HideLook { .. } => "hide_look",
            Self::SessionAnswer { option: Some(_), .. } => "dialog_answer",
            Self::SessionAnswer { option: None, .. } => "close_window",
        }
    }
}

const ROUND: u8 = 8;

fn vm_and_addon(lua: &Lua) -> (u64, String) {
    let vm = super::native_events::bus(lua).borrow().vm;
    let addon = session(lua).borrow().addon.clone();
    (vm, addon)
}

fn str_arg(v: &Value) -> Option<String> {
    v.as_string().and_then(|s| s.to_str().ok().map(|s| s.to_string()))
}

fn ref_arg(lua: &Lua, symbol: &str, v: &Value) -> std::result::Result<String, Result<mlua::MultiValue>> {
    match str_arg(v) {
        Some(s) => Ok(s),
        None => Err(error(lua, symbol, "invalid_argument", "ref must be the ref of an open window session")),
    }
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    // the record shapes
    for (path, fields) in [
        ("eqoa.windows.Session", "ref kind title origin opened_from revision focused data"),
        ("eqoa.windows.Confirm", "id kind text"),
    ] {
        describe(lua, path, "supported", Some(ROUND), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(ROUND), "");
        }
    }

    bind(
        lua,
        "windows",
        "open_session",
        lua.create_function(|lua, (kind, opts): (Value, Option<Value>)| {
            const SYMBOL: &str = "eqoa.windows.open_session";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, "eqoa.windows.session") {
                return error(lua, SYMBOL, code, &msg);
            }
            let Some(kind) = str_arg(&kind) else {
                return error(lua, SYMBOL, "invalid_argument", "kind must be 'inventory', 'spellbook' or 'quest_log'");
            };
            let opened_from = match opts {
                None | Some(Value::Nil) => "mod".to_string(),
                Some(Value::Table(t)) => match t.get::<Value>("opened_from")? {
                    Value::Nil => "mod".to_string(),
                    v => match str_arg(&v) {
                        Some(s) if s == "key" || s == "mod" => s,
                        _ => return error(lua, SYMBOL, "invalid_argument", "opened_from must be 'key' or 'mod'"),
                    },
                },
                Some(_) => return error(lua, SYMBOL, "invalid_argument", "options must be a table or nil"),
            };
            let (vm, addon) = vm_and_addon(lua);
            match window_sessions::open(vm, &addon, &kind, &opened_from) {
                Ok(s) => {
                    let focused = window_sessions::is_focused(&s.id);
                    lua.pack_multi((record(lua, "eqoa.windows.Session", &s.json(focused))?, Value::Nil))
                }
                Err((code, msg)) => error(lua, SYMBOL, code, &msg),
            }
        })?,
    )?;
    bind(
        lua,
        "windows",
        "session",
        lua.create_function(|lua, kind: Value| {
            const SYMBOL: &str = "eqoa.windows.session";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, "eqoa.windows.session") {
                return error(lua, SYMBOL, code, &msg);
            }
            let Some(kind) = str_arg(&kind) else {
                return error(lua, SYMBOL, "invalid_argument", "kind must name a game window");
            };
            if !super::window_kinds::is_window(&kind) {
                return error(lua, SYMBOL, "invalid_argument", "kind must name a game window");
            }
            let (vm, _) = vm_and_addon(lua);
            match window_sessions::get(vm, &kind) {
                Some(s) => {
                    let focused = window_sessions::is_focused(&s.id);
                    lua.pack_multi((record(lua, "eqoa.windows.Session", &s.json(focused))?, Value::Nil))
                }
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;
    bind(
        lua,
        "windows",
        "close_session",
        lua.create_function(|lua, r: Value| {
            const SYMBOL: &str = "eqoa.windows.close_session";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, "eqoa.windows.session") {
                return error(lua, SYMBOL, code, &msg);
            }
            let id = match ref_arg(lua, SYMBOL, &r) {
                Ok(id) => id,
                Err(e) => return e,
            };
            let (vm, _) = vm_and_addon(lua);
            match window_sessions::close(vm, &id, "closed") {
                Ok(closed) => lua.pack_multi((closed, Value::Nil)),
                Err((code, msg)) => error(lua, SYMBOL, code, &msg),
            }
        })?,
    )?;
    bind(
        lua,
        "windows",
        "pass",
        lua.create_function(|lua, r: Value| {
            const SYMBOL: &str = "eqoa.windows.pass";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, "eqoa.windows.session") {
                return error(lua, SYMBOL, code, &msg);
            }
            let id = match ref_arg(lua, SYMBOL, &r) {
                Ok(id) => id,
                Err(e) => return e,
            };
            let (vm, _) = vm_and_addon(lua);
            match window_sessions::pass(vm, &id) {
                Ok(()) => lua.pack_multi((true, Value::Nil)),
                Err((code, msg)) => error(lua, SYMBOL, code, &msg),
            }
        })?,
    )?;
    bind(
        lua,
        "windows",
        "focus",
        lua.create_function(|lua, (r, opts): (Value, Option<Value>)| {
            const SYMBOL: &str = "eqoa.windows.focus";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, "eqoa.windows.focus") {
                return error(lua, SYMBOL, code, &msg);
            }
            let id = match ref_arg(lua, SYMBOL, &r) {
                Ok(id) => id,
                Err(e) => return e,
            };
            let movement = match opts {
                None | Some(Value::Nil) => true,
                Some(Value::Table(t)) => match t.get::<Value>("movement")? {
                    Value::Nil => true,
                    Value::Boolean(b) => b,
                    _ => return error(lua, SYMBOL, "invalid_argument", "movement must be a boolean"),
                },
                Some(_) => return error(lua, SYMBOL, "invalid_argument", "options must be a table or nil"),
            };
            let (vm, _) = vm_and_addon(lua);
            match window_sessions::focus(vm, &id, movement) {
                Ok(()) => lua.pack_multi((true, Value::Nil)),
                Err((code, msg)) => error(lua, SYMBOL, code, &msg),
            }
        })?,
    )?;
    bind(
        lua,
        "windows",
        "release_focus",
        lua.create_function(|lua, r: Value| {
            const SYMBOL: &str = "eqoa.windows.release_focus";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, "eqoa.windows.focus") {
                return error(lua, SYMBOL, code, &msg);
            }
            let id = match ref_arg(lua, SYMBOL, &r) {
                Ok(id) => id,
                Err(e) => return e,
            };
            let (vm, _) = vm_and_addon(lua);
            lua.pack_multi((window_sessions::release_focus(vm, &id), Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "windows",
        "confirm",
        lua.create_function(|lua, ()| {
            const SYMBOL: &str = "eqoa.windows.confirm";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, "eqoa.windows.session") {
                return error(lua, SYMBOL, code, &msg);
            }
            let (vm, _) = vm_and_addon(lua);
            match confirm::current_for(vm) {
                Some(p) => lua.pack_multi((
                    record(lua, "eqoa.windows.Confirm", &json!({"id": p.id, "kind": p.act.kind(), "text": p.text}))?,
                    Value::Nil,
                )),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;

    for (name, kind) in [("destroy", 0u8), ("link_item", 1), ("hide_look", 2)] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, (item, arg): (Value, Option<Value>)| {
                enqueue_lua_with(lua, &format!("eqoa.actions.{name}"), || {
                    let spec = resolve_item_handle(&item)?;
                    if spec.list != "inventory" {
                        return Err((
                            "invalid_argument",
                            format!("{name} requires an inventory item handle, got '{}'", spec.list),
                        ));
                    }
                    let action = match kind {
                        0 => {
                            let count = match arg {
                                None | Some(Value::Nil) => None,
                                Some(Value::Integer(n)) if n >= 1 => Some(n),
                                Some(Value::Number(n)) if n >= 1.0 && n.fract() == 0.0 && n < 1e9 => Some(n as i64),
                                Some(_) => {
                                    return Err(("invalid_argument", "count must be a whole number of 1 or more".into()));
                                }
                            };
                            A1Action::Destroy { item: spec, count }
                        }
                        1 => A1Action::LinkItem { item: spec },
                        _ => {
                            let hide = match arg {
                                None | Some(Value::Nil) => true,
                                Some(Value::Boolean(b)) => b,
                                Some(_) => return Err(("invalid_argument", "hide must be a boolean".into())),
                            };
                            A1Action::HideLook { item: spec, hide }
                        }
                    };
                    Ok(ActionKind::A1(action))
                })
            })?,
        )?;
    }
    // Bag Viewer multi-select: one question for a list of whole stacks (never more than the pack holds at once)
    const MAX_DESTROY_MANY: usize = 64;
    describe(lua, "eqoa.actions.destroy_many", "supported", Some(ROUND), "");
    bind(
        lua,
        "actions",
        "destroy_many",
        lua.create_function(|lua, items: Value| {
            enqueue_lua_with(lua, "eqoa.actions.destroy_many", || {
                let Value::Table(list) = items else {
                    return Err(("invalid_argument", "items must be a list of inventory item handles".into()));
                };
                let n = list.raw_len();
                if n == 0 || n > MAX_DESTROY_MANY {
                    return Err(("invalid_argument", format!("items must hold 1 to {MAX_DESTROY_MANY} item handles")));
                }
                let mut specs: Vec<ResolvedItemSpec> = Vec::with_capacity(n);
                for i in 1..=n {
                    let handle: Value =
                        list.raw_get(i).map_err(|_| ("invalid_argument", "items must be a plain list".to_string()))?;
                    let spec = resolve_item_handle(&handle)?;
                    if spec.list != "inventory" {
                        return Err((
                            "invalid_argument",
                            format!("destroy_many requires inventory item handles, got '{}'", spec.list),
                        ));
                    }
                    if specs.iter().any(|s| s.inv_index == spec.inv_index) {
                        return Err(("invalid_argument", "the same item is listed twice".into()));
                    }
                    specs.push(spec);
                }
                Ok(ActionKind::A1(A1Action::DestroyMany { items: specs }))
            })
        })?,
    )?;
    Ok(())
}
