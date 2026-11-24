// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A11: the mercenary windows, the dye picker and fishing for a mod (spec section 7, "Mercenaries, the dye
//! picker and fishing"): the reads `eqoa.merc.get`, `eqoa.dye.get` and `eqoa.fishing.get`, and the actions whose
//! names begin `merc_`, `dye_` and `fishing_`. Plain data and bindings only: `fork_ext/a11_play.rs` reads the game's
//! own state into the snapshots kept here and carries the actions out through the game's own request builders.
//!
//! What a result means: an action completes when the client has sent the request (`detail.sent`), or, for the two
//! that spend something the player has (`merc_dismiss`, `dye_pick`), when the client's own "Are you sure?" has been
//! put to the player (`detail.asked`, `detail.confirm`); only the player's Yes sends. Never `confirmed`: the server's
//! answer arrives later as the next read (`merc.get().waiting`, the merc's changed records).
use super::{
    action_broker::{ActionKind, enqueue_lua_with},
    host::Host,
    native::{bind, describe, error, record_cached, require_capability},
    native_items::{ResolvedItemSpec, resolve_item_handle},
};
use mlua::{Result, Value};
use serde_json::{Value as Json, json};
use std::cell::{Cell, RefCell};

pub(crate) const ROUND: u8 = 8;

/// The names of the new actions, as capabilities (`eqoa.actions.<name>`).
pub(crate) const NAMES: [&str; 17] = [
    "merc_give",
    "merc_equip",
    "merc_unequip",
    "merc_take",
    "merc_stock",
    "merc_spell_set",
    "merc_spells_clear",
    "merc_dismiss",
    "dye_pick",
    "dye_no_dye",
    "dye_cancel",
    "fishing_cast",
    "fishing_release",
    "fishing_reel",
    "fishing_keep",
    "fishing_pull",
    "fishing_cancel",
];

/// The actions of round A11, carried by the broker as `ActionKind::A11`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum A11Action {
    /// The game's Give to Merc: a record of the player's own pack.
    MercGive { item: ResolvedItemSpec },
    /// Equip, Unequip and Take Back: a record of the merc's (`eqoa.merc.get().inventory.records[i].id`).
    MercEquip { id: u32 },
    MercUnequip { id: u32 },
    MercTake { id: u32 },
    /// From the owner's potion stock into the pack: the item's id and how many (0 a stack).
    MercStock { id: u32, count: u32 },
    /// Memorize onto one of the merc's ten slots (`slot` 0-based here, 1-based in Lua) and clear them all.
    MercSpellSet { slot: u8, spell: u32 },
    MercSpellsClear,
    /// Send the merc away. Asks the player first.
    MercDismiss,
    /// The dye picker's three answers, each echoing the picker's `token`. `dye_pick` asks the player first.
    DyePick { token: u32, rgba: u32 },
    DyeNoDye { token: u32 },
    DyeCancel { token: u32 },
    /// Fishing: the game's own window keys and request builders.
    FishCast,
    FishRelease,
    FishReel,
    FishKeep { slot: usize },
    FishPull,
    FishCancel,
}

impl A11Action {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::MercGive { .. } => "merc_give",
            Self::MercEquip { .. } => "merc_equip",
            Self::MercUnequip { .. } => "merc_unequip",
            Self::MercTake { .. } => "merc_take",
            Self::MercStock { .. } => "merc_stock",
            Self::MercSpellSet { .. } => "merc_spell_set",
            Self::MercSpellsClear => "merc_spells_clear",
            Self::MercDismiss => "merc_dismiss",
            Self::DyePick { .. } => "dye_pick",
            Self::DyeNoDye { .. } => "dye_no_dye",
            Self::DyeCancel { .. } => "dye_cancel",
            Self::FishCast => "fishing_cast",
            Self::FishRelease => "fishing_release",
            Self::FishReel => "fishing_reel",
            Self::FishKeep { .. } => "fishing_keep",
            Self::FishPull => "fishing_pull",
            Self::FishCancel => "fishing_cancel",
        }
    }
}

/// The three reads, in the order of `STATE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Read {
    Merc = 0,
    Dye = 1,
    Fishing = 2,
}

thread_local! {
    /// What the client last read of each window (`fork_ext/a11_play.rs`), or none while there is nothing to read.
    static STATE: RefCell<[Option<Json>; 3]> = const { RefCell::new([None, None, None]) };
    static REVISION: Cell<u64> = const { Cell::new(0) };
}

/// The client's reading of a window's state. A changed state gets the next revision; an unchanged one is left as it
/// is (so a mod can tell by `revision` alone).
pub(crate) fn publish(which: Read, mut state: Json) {
    let open = match which {
        Read::Merc => state["inventory"]["open"] == true || state["spells"]["open"] == true,
        Read::Dye => true,
        Read::Fishing => state["open"] == true,
    };
    let mut emitted = None;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(old) = s[which as usize].as_ref() {
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
        emitted = Some(next);
        s[which as usize] = Some(state);
    });
    if let Some(revision) = emitted {
        emit(which, json!({"revision": revision, "open": open}));
    }
}

/// The event of a window whose state changed (`merc.changed`, `dye.changed`, `fishing.changed`): `open` is whether the
/// game's own window is up, so a mod hears a window open, change and close as one stream.
fn emit(which: Read, data: Json) {
    let name = match which {
        Read::Merc => "merc.changed",
        Read::Dye => "dye.changed",
        Read::Fishing => "fishing.changed",
    };
    if super::native_events::interested(name) {
        super::native_core::CORE.with(|c| c.borrow_mut().emit(name, data));
    }
}

/// Nothing of that window is left to read.
pub(crate) fn clear(which: Read) {
    let was = STATE.with(|s| s.borrow_mut()[which as usize].take());
    if let Some(old) = was {
        emit(which, json!({"revision": old["revision"], "open": false}));
    }
}

pub(crate) fn clear_all() {
    for which in [Read::Merc, Read::Dye, Read::Fishing] {
        clear(which);
    }
}

pub(crate) fn current(which: Read) -> Option<Json> {
    STATE.with(|s| s.borrow()[which as usize].clone())
}

fn uint_arg(v: &Value, max: i64) -> Option<i64> {
    let n = match v {
        Value::Integer(n) => *n,
        Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e12 => *n as i64,
        _ => return None,
    };
    (0..=max).contains(&n).then_some(n)
}

pub(crate) fn nested_type(path: &str, key: &str) -> Option<&'static str> {
    match (path, key) {
        ("eqoa.merc.State", "inventory") => Some("eqoa.merc.Inventory"),
        ("eqoa.merc.State", "spells") => Some("eqoa.merc.Spells"),
        ("eqoa.merc.Inventory", "records") => Some("eqoa.merc.Record"),
        ("eqoa.merc.Spells", "spells") => Some("eqoa.merc.Spell"),
        ("eqoa.fishing.State", "catch") => Some("eqoa.fishing.Catch"),
        _ => None,
    }
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    for (path, fields) in [
        ("eqoa.merc.State", "revision available own_out waiting inventory spells"),
        (
            "eqoa.merc.Inventory",
            "open slot name class race level hp max_hp power max_power ac stats most records",
        ),
        ("eqoa.merc.Record", "id kind item_id name quantity equip_slot"),
        ("eqoa.merc.Spells", "open slot name class level set slots spells"),
        ("eqoa.merc.Spell", "id name slot"),
        ("eqoa.dye.State", "revision token target title slots current alpha open no_dye"),
        (
            "eqoa.fishing.State",
            "revision open state casting power ready perfect_from perfect_to fair_from fair_to list catch message",
        ),
        ("eqoa.fishing.Catch", "slot name quantity"),
    ] {
        describe(lua, path, "supported", Some(ROUND), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(ROUND), "");
        }
    }

    for (module, read, slot, path) in [
        ("merc", Read::Merc, "merc.get", "eqoa.merc.State"),
        ("dye", Read::Dye, "dye.get", "eqoa.dye.State"),
        ("fishing", Read::Fishing, "fishing.get", "eqoa.fishing.State"),
    ] {
        bind(
            lua,
            module,
            "get",
            lua.create_function(move |lua, args: mlua::MultiValue| {
                let symbol = format!("eqoa.{module}.get");
                if let Err((code, msg)) = require_capability(lua, &symbol, &symbol) {
                    return error(lua, &symbol, code, &msg);
                }
                if !args.is_empty() {
                    return error(lua, &symbol, "invalid_argument", "this function takes no arguments");
                }
                match current(read) {
                    None => lua.pack_multi((Value::Nil, Value::Nil)),
                    Some(state) => lua.pack_multi((record_cached(lua, slot, path, &state)?, Value::Nil)),
                }
            })?,
        )?;
    }

    // no argument
    for (name, action) in [
        ("merc_spells_clear", A11Action::MercSpellsClear),
        ("merc_dismiss", A11Action::MercDismiss),
        ("fishing_cast", A11Action::FishCast),
        ("fishing_release", A11Action::FishRelease),
        ("fishing_reel", A11Action::FishReel),
        ("fishing_pull", A11Action::FishPull),
        ("fishing_cancel", A11Action::FishCancel),
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
                    Ok(ActionKind::A11(action.clone()))
                })
            })?,
        )?;
    }
    // a record of the merc's, by the `id` of `eqoa.merc.get().inventory.records`
    for (name, kind) in [("merc_equip", 0u8), ("merc_unequip", 1), ("merc_take", 2)] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, id: Value| {
                let symbol = format!("eqoa.actions.{name}");
                enqueue_lua_with(lua, &symbol, || {
                    let Some(id) = uint_arg(&id, u32::MAX as i64) else {
                        return Err(("invalid_argument", "pass the id of a record of eqoa.merc.get().inventory.records".into()));
                    };
                    let id = id as u32;
                    Ok(ActionKind::A11(match kind {
                        0 => A11Action::MercEquip { id },
                        1 => A11Action::MercUnequip { id },
                        _ => A11Action::MercTake { id },
                    }))
                })
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "merc_give",
        lua.create_function(|lua, item: Value| {
            enqueue_lua_with(lua, "eqoa.actions.merc_give", || {
                let spec = resolve_item_handle(&item)?;
                if spec.list != "inventory" {
                    return Err(("invalid_argument", format!("merc_give requires an inventory item handle, got '{}'", spec.list)));
                }
                Ok(ActionKind::A11(A11Action::MercGive { item: spec }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "merc_stock",
        lua.create_function(|lua, (id, count): (Value, Option<Value>)| {
            enqueue_lua_with(lua, "eqoa.actions.merc_stock", || {
                let Some(id) = uint_arg(&id, u32::MAX as i64) else {
                    return Err(("invalid_argument", "pass the id of a stock record of eqoa.merc.get().inventory.records".into()));
                };
                let count = match count {
                    None | Some(Value::Nil) => 0,
                    Some(v) => match uint_arg(&v, 127) {
                        Some(n) => n,
                        None => return Err(("invalid_argument", "count must be a whole number from 0 (a stack) to 127".into())),
                    },
                };
                Ok(ActionKind::A11(A11Action::MercStock { id: id as u32, count: count as u32 }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "merc_spell_set",
        lua.create_function(|lua, (slot, spell): (Value, Value)| {
            enqueue_lua_with(lua, "eqoa.actions.merc_spell_set", || {
                let (Some(slot), Some(spell)) = (uint_arg(&slot, 10), uint_arg(&spell, u32::MAX as i64)) else {
                    return Err(("invalid_argument", "pass a slot from 1 to 10 and a spell id of eqoa.merc.get().spells.spells (0 empties the slot)".into()));
                };
                if slot == 0 {
                    return Err(("out_of_range", "the merc's slots are 1 to 10".into()));
                }
                Ok(ActionKind::A11(A11Action::MercSpellSet { slot: slot as u8 - 1, spell: spell as u32 }))
            })
        })?,
    )?;
    // the picker's token, echoed: `eqoa.dye.get().token`
    bind(
        lua,
        "actions",
        "dye_pick",
        lua.create_function(|lua, (token, rgba): (Value, Value)| {
            enqueue_lua_with(lua, "eqoa.actions.dye_pick", || {
                let (Some(token), Some(rgba)) = (uint_arg(&token, u32::MAX as i64), uint_arg(&rgba, u32::MAX as i64)) else {
                    return Err(("invalid_argument", "pass the picker's token (eqoa.dye.get().token) and a colour as 0xRRGGBBAA".into()));
                };
                Ok(ActionKind::A11(A11Action::DyePick { token: token as u32, rgba: rgba as u32 }))
            })
        })?,
    )?;
    for (name, no_dye) in [("dye_no_dye", true), ("dye_cancel", false)] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, token: Value| {
                let symbol = format!("eqoa.actions.{name}");
                enqueue_lua_with(lua, &symbol, || {
                    let Some(token) = uint_arg(&token, u32::MAX as i64) else {
                        return Err(("invalid_argument", "pass the picker's token (eqoa.dye.get().token)".into()));
                    };
                    let token = token as u32;
                    Ok(ActionKind::A11(if no_dye { A11Action::DyeNoDye { token } } else { A11Action::DyeCancel { token } }))
                })
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "fishing_keep",
        lua.create_function(|lua, slot: Value| {
            enqueue_lua_with(lua, "eqoa.actions.fishing_keep", || match uint_arg(&slot, 1000) {
                Some(n) if n >= 1 => Ok(ActionKind::A11(A11Action::FishKeep { slot: n as usize - 1 })),
                _ => Err(("invalid_argument", "pass the slot of a row of eqoa.fishing.get().catch (1 or more)".into())),
            })
        })?,
    )?;
    Ok(())
}

/// The honest Mods & Addons line for a mod that declares any of these steps (`runtime::disclosure_lines`). Reads and
/// the picker's Cancel change nothing the player has; dismissing a merc and applying a colour are shown as asking
/// the player first (the client's own question), the rest are named as what they do.
pub(crate) fn a11_label(caps: &[String]) -> Option<String> {
    let mut steps: Vec<&str> = Vec::new();
    let any = |names: &[&str]| names.iter().any(|n| super::native::capability_grants(caps, &format!("eqoa.actions.{n}")));
    if any(&["merc_give", "merc_equip", "merc_unequip", "merc_take", "merc_stock", "merc_spell_set", "merc_spells_clear"]) {
        steps.push("move items to and from your mercenary and set its spells");
    }
    if any(&["merc_dismiss"]) {
        steps.push("dismiss it (asks you first)");
    }
    if any(&["dye_pick", "dye_no_dye"]) {
        steps.push("answer the dye picker (a colour asks you first)");
    }
    if any(&["fishing_cast", "fishing_release", "fishing_reel", "fishing_keep", "fishing_pull", "fishing_cancel"]) {
        steps.push("cast, reel, keep or let go of a catch while fishing");
    }
    (!steps.is_empty()).then(|| format!("Mercenary, dye and fishing windows: can {}", steps.join(", ")))
}
