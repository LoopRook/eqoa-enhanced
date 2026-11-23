// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A8: crafting for a mod (spec section 7, "Crafting"): the `eqoa.craft.get` read and the eight
//! `eqoa.actions.craft_*` actions, over the two windows a mod can open for itself, `recipe_book` and
//! `experiment` (`window_sessions::OWN_KINDS`). Plain data and bindings only: `fork_ext/a8_play.rs` reads the game's
//! crafting state into the snapshot kept here and carries the actions out through `Craft`'s own request builders.
//!
//! What a result means: an action completes when the client has sent the request (`detail.sent`), or, for the tray
//! (which is the client's own list until a combine), when the tray changed (`detail.tray`). The server's reply is
//! read afterwards: the mod watches `eqoa.craft.get()` (its `pending`, `notice` and `question`) and, for the items
//! made, `inventory.changed`. Nothing is ever sent twice: an action that finds a request still out is refused `busy`.
use super::{
    action_broker::{ActionKind, enqueue_lua_with},
    host::Host,
    native::{bind, describe, error, record_cached, require_capability},
};
use mlua::{Lua, Result, Value};
use serde_json::{Value as Json, json};
use std::cell::{Cell, RefCell};

pub(crate) const ROUND: u8 = 8;

/// The most the tray holds (`Craft::add_to_combine`): the game's own limit.
pub(crate) const TRAY_MAX: usize = 4;

/// The names of the new actions, as capabilities (`eqoa.actions.<name>`).
pub(crate) const NAMES: [&str; 8] = [
    "craft_list",
    "craft_inspect",
    "craft_tray_add",
    "craft_tray_remove",
    "craft_tray_clear",
    "craft_fill",
    "craft_combine",
    "craft_keep",
];

/// The actions of round A8, carried by the broker as `ActionKind::A8`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum A8Action {
    /// Ask for a skill's recipe list (`Craft::send6`).
    List { skill: u8 },
    /// Ask for one recipe's detail (`Craft::send7`).
    Inspect { recipe: i32 },
    /// One of a bag row into the tray. `slot` is the 0-based row of `eqoa.craft.get().items`, `id` that row's item.
    TrayAdd { slot: usize, id: i64 },
    TrayRemove { slot: usize, id: i64 },
    TrayClear,
    /// The tray becomes exactly these (a row more than once puts more of it in), or nothing at all.
    Fill { picks: Vec<(usize, i64)> },
    /// One combine of what the tray holds (`Craft::send_combine`).
    Combine,
    /// The server's "keep this recipe?" question: Yes scribes it (`Craft::send5`), No drops the question.
    Keep { yes: bool },
}

impl A8Action {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::List { .. } => "craft_list",
            Self::Inspect { .. } => "craft_inspect",
            Self::TrayAdd { .. } => "craft_tray_add",
            Self::TrayRemove { .. } => "craft_tray_remove",
            Self::TrayClear => "craft_tray_clear",
            Self::Fill { .. } => "craft_fill",
            Self::Combine => "craft_combine",
            Self::Keep { .. } => "craft_keep",
        }
    }
}

thread_local! {
    /// The crafting state the client last read for the open mod window (`fork_ext/a8_play.rs`), or none when no mod
    /// holds a crafting window.
    static STATE: RefCell<Option<Json>> = const { RefCell::new(None) };
    static REVISION: Cell<u64> = const { Cell::new(0) };
}

/// The client's reading of the game's crafting state. A changed state gets the next revision; an unchanged one
/// is left as it is (so a mod can tell by `revision` alone).
pub(crate) fn publish(mut state: Json) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(old) = s.as_ref() {
            let mut same = old.clone();
            same["revision"] = Json::Null;
            let mut now = state.clone();
            now["revision"] = Json::Null;
            if same == now {
                return;
            }
        }
        let next = REVISION.with(|r| {
            r.set(r.get() + 1);
            r.get()
        });
        state["revision"] = json!(next);
        *s = Some(state);
    });
}

/// No mod crafting window is open: nothing to read.
pub(crate) fn clear() {
    STATE.with(|s| *s.borrow_mut() = None);
}

pub(crate) fn current() -> Option<Json> {
    STATE.with(|s| s.borrow().clone())
}

fn int_arg(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(n) => Some(*n),
        Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e12 => Some(*n as i64),
        _ => None,
    }
}

/// A `{slot = n, id = n}` pick: `slot` is 1-based in Lua (the first row of `items` is 1).
fn pick_of(table: &mlua::Table) -> std::result::Result<(usize, i64), (&'static str, String)> {
    let slot = table.get::<Value>("slot").ok().and_then(|v| int_arg(&v));
    let id = table.get::<Value>("id").ok().and_then(|v| int_arg(&v));
    match (slot, id) {
        (Some(slot), Some(id)) if (1..=100_000).contains(&slot) => Ok((slot as usize - 1, id)),
        _ => Err(("invalid_argument", "a tray pick is {slot = a row of items (1 or more), id = that row's id}".into())),
    }
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    for (path, fields) in [
        (
            "eqoa.craft.State",
            "revision pending listed_skill skills recipes detail tray items question notice",
        ),
        ("eqoa.craft.Skill", "id name level rank"),
        ("eqoa.craft.Recipe", "id name level skill component component_name difficulty difficulty_name order"),
        ("eqoa.craft.Detail", "recipe name description rarity count level ingredients tools"),
        ("eqoa.craft.Ingredient", "name need have"),
        ("eqoa.craft.Tool", "name have"),
        ("eqoa.craft.TrayItem", "slot id name"),
        ("eqoa.craft.Item", "slot id name quantity in_tray"),
        ("eqoa.craft.Question", "text recipe"),
        ("eqoa.craft.Notice", "text seq"),
    ] {
        describe(lua, path, "supported", Some(ROUND), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(ROUND), "");
        }
    }

    bind(
        lua,
        "craft",
        "get",
        lua.create_function(|lua, args: mlua::MultiValue| {
            const SYMBOL: &str = "eqoa.craft.get";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, "eqoa.craft.get") {
                return error(lua, SYMBOL, code, &msg);
            }
            if !args.is_empty() {
                return error(lua, SYMBOL, "invalid_argument", "this function takes no arguments");
            }
            match current() {
                // not open yet (the client reads the state at its next step) or not a window a mod holds
                None => lua.pack_multi((Value::Nil, Value::Nil)),
                Some(state) => lua.pack_multi((record_cached(lua, "craft.get", "eqoa.craft.State", &state)?, Value::Nil)),
            }
        })?,
    )?;

    // no argument
    for (name, action) in [("craft_tray_clear", A8Action::TrayClear), ("craft_combine", A8Action::Combine)] {
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
                    Ok(ActionKind::A8(action.clone()))
                })
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "craft_list",
        lua.create_function(|lua, skill: Value| {
            enqueue_lua_with(lua, "eqoa.actions.craft_list", || match int_arg(&skill) {
                Some(n) if (0..=5).contains(&n) => Ok(ActionKind::A8(A8Action::List { skill: n as u8 })),
                _ => Err(("invalid_argument", "skill must be a whole number from 0 to 5 (the id of an eqoa.craft.get() skill)".into())),
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "craft_inspect",
        lua.create_function(|lua, recipe: Value| {
            enqueue_lua_with(lua, "eqoa.actions.craft_inspect", || match int_arg(&recipe) {
                Some(n) if i32::try_from(n).is_ok() => Ok(ActionKind::A8(A8Action::Inspect { recipe: n as i32 })),
                _ => Err(("invalid_argument", "recipe must be the id of an eqoa.craft.get() recipe".into())),
            })
        })?,
    )?;
    for (name, add) in [("craft_tray_add", true), ("craft_tray_remove", false)] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, pick: Value| {
                let symbol = format!("eqoa.actions.{name}");
                enqueue_lua_with(lua, &symbol, || {
                    let Value::Table(t) = &pick else {
                        return Err(("invalid_argument", "pass a row of eqoa.craft.get().items: {slot = .., id = ..}".into()));
                    };
                    let (slot, id) = pick_of(t)?;
                    Ok(ActionKind::A8(if add { A8Action::TrayAdd { slot, id } } else { A8Action::TrayRemove { slot, id } }))
                })
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "craft_fill",
        lua.create_function(|lua, picks: Value| {
            enqueue_lua_with(lua, "eqoa.actions.craft_fill", || {
                let Value::Table(t) = &picks else {
                    return Err(("invalid_argument", "pass a list of up to four rows: {{slot = .., id = ..}, ...}".into()));
                };
                let mut out = Vec::new();
                for v in t.sequence_values::<Value>() {
                    let Ok(Value::Table(p)) = v else {
                        return Err(("invalid_argument", "every pick is {slot = .., id = ..}".into()));
                    };
                    out.push(pick_of(&p)?);
                    if out.len() > TRAY_MAX {
                        return Err(("out_of_range", format!("the tray holds {TRAY_MAX} items at most")));
                    }
                }
                Ok(ActionKind::A8(A8Action::Fill { picks: out }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "craft_keep",
        lua.create_function(|lua, yes: Value| {
            enqueue_lua_with(lua, "eqoa.actions.craft_keep", || match yes {
                Value::Boolean(b) => Ok(ActionKind::A8(A8Action::Keep { yes: b })),
                _ => Err(("invalid_argument", "pass true to keep the recipe or false to leave it".into())),
            })
        })?,
    )?;
    Ok(())
}

/// The honest Mods & Addons line for a mod that declares any of the steps that change what the player has
/// (`runtime::disclosure_lines`). Listing a recipe, reading its detail and clearing the tray change nothing the
/// game's own window does not do by looking, so they are not named.
pub(crate) fn crafting_label(caps: &[String]) -> Option<String> {
    let mut steps: Vec<&str> = Vec::new();
    for (name, words) in [
        ("craft_tray_add", "fill the crafting tray"),
        ("craft_fill", "fill the crafting tray"),
        ("craft_combine", "combine what is in it"),
        ("craft_keep", "answer the keep-this-recipe question"),
    ] {
        if super::native::capability_grants(caps, &format!("eqoa.actions.{name}")) && !steps.contains(&words) {
            steps.push(words);
        }
    }
    (!steps.is_empty()).then(|| format!("Crafting: can {}", steps.join(", ")))
}
