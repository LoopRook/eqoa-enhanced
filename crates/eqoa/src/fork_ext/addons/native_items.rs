// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! `eqoa.inventory`, `eqoa.bank`, `eqoa.loot`, `eqoa.merchant` and `eqoa.items` (spec section 6):
//! observations and change events only. The containers are the client's own lists, copied by the
//! source capture (`fork_ext::api_capture::items`) when a game event or a window change touches
//! them; this file keeps them in the core view, gives each stack a handle that survives the list
//! compacting, and answers the Lua calls. No transaction is built here.
use super::{
    host::Host,
    native::{bind, describe, error, guard, record},
    native_core::{CORE, Core, View, changed_fields},
    native_events,
    native_state::{Ref, uint},
};
use mlua::{Lua, MultiValue, Result, Table, Value};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const LISTS: [&str; 4] = ["inventory", "bank", "loot", "merchant"];

const CONTAINER: &str = "eqoa.items.Container";
const ITEM: &str = "eqoa.items.Item";

fn slot<'a>(view: &'a mut View, name: &str) -> &'a mut Json {
    match name {
        "inventory" => &mut view.inventory,
        "bank" => &mut view.bank,
        "loot" => &mut view.loot,
        _ => &mut view.merchant,
    }
}
fn peek<'a>(view: &'a View, name: &str) -> &'a Json {
    match name {
        "inventory" => &view.inventory,
        "bank" => &view.bank,
        "loot" => &view.loot,
        _ => &view.merchant,
    }
}

/// A container change waiting to be told to the subscribers.
pub(crate) struct ItemChange {
    name: &'static str,
    before: Json,
    after: Json,
    cause: String,
}

fn without_revision(v: &Json) -> Json {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("revision");
    }
    v
}

/// The cause: the native event's name, except that a container that appears or goes is `open` /
/// `close` and a bank's window flag flipping is the same, whatever packet or frame saw it.
fn cause_of(before: &Json, after: &Json, native: &str) -> String {
    if before.is_null() && !after.is_null() {
        "open".into()
    } else if !before.is_null() && after.is_null() {
        "close".into()
    } else if before["open"] != after["open"] {
        if after["open"] == true { "open" } else { "close" }.into()
    } else {
        native.into()
    }
}

fn event_data(change: ItemChange) -> (String, Json) {
    let changed = changed_fields(&without_revision(&change.before), &without_revision(&change.after));
    (
        format!("{}.changed", change.name),
        json!({"before":change.before,"after":change.after,"changed":changed,"cause":change.cause}),
    )
}

impl Core {
    /// Give each stack of `container` its handle: one per wire key, kept while the stack stays in the
    /// list (compaction moves its position, not its handle), new for a stack that was not there, and
    /// all new for a new session of the list (a corpse or merchant opened again).
    fn assign_item_refs(&mut self, name: &'static str, container: &mut Json, new_session: bool) {
        let generation = self.generation;
        let mut versions = self.item_versions.remove(name).unwrap_or_default();
        if new_session {
            versions.clear();
        }
        if let Some(items) = container["items"].as_array_mut() {
            let keys: BTreeSet<u32> = items
                .iter()
                .filter_map(|i| i["inventory_key"].as_u64().map(|k| k as u32))
                .collect();
            versions.retain(|k, _| keys.contains(k));
            for item in items {
                let Some(key) = item["inventory_key"].as_u64().map(|k| k as u32) else {
                    continue;
                };
                let version = *versions.entry(key).or_insert_with(|| {
                    self.next_version_for_items()
                });
                item["ref"] = json!({"$ref":"item","key":format!("{name}:{key}"),"generation":generation,"incarnation":version});
            }
        }
        self.item_versions.insert(name, versions);
        self.resolve_view_refs(container);
    }

    /// A container's `view` arrives with wire keys (`row_keys`, `selected_key`) of the stacks of the list it
    /// names (`row_list`: its own, or the pack for a merchant that is buying from the player); they become
    /// the handles of those stacks, which the lists' own passes gave (the pack is passed before the merchant).
    fn resolve_view_refs(&self, container: &mut Json) {
        let Some(view) = container.get_mut("view").and_then(Json::as_object_mut) else { return };
        let list = view.remove("row_list").and_then(|l| l.as_str().map(String::from)).unwrap_or_default();
        let generation = self.generation;
        let handle = |key: u64| {
            let version = self.item_versions.get(list.as_str())?.get(&(key as u32))?;
            Some(json!({"$ref":"item","key":format!("{list}:{key}"),"generation":generation,"incarnation":version}))
        };
        let rows: Vec<Json> = view.remove("row_keys").and_then(|k| k.as_array().cloned()).unwrap_or_default().iter().filter_map(|k| handle(k.as_u64()?)).collect();
        let selected = view.remove("selected_key").and_then(|k| handle(k.as_u64()?));
        if selected.is_none() {
            view.remove("selected_row");
        }
        view.insert("rows".into(), Json::Array(rows));
        if let Some(selected) = selected {
            view.insert("selected".into(), selected);
        }
    }
    fn next_version_for_items(&mut self) -> u64 {
        self.next_incarnation()
    }

    /// A new session of a list: it was closed, or the client replaced it (a corpse's or a merchant's
    /// open packet while one is already up, with the same wire keys and the same merchant): the
    /// native open causes say so, and every handle of the earlier list goes stale.
    fn items_session_is_new(name: &str, before: &Json, after: &Json, native_cause: &str) -> bool {
        before.is_null()
            || (after["merchant"].is_object()
                && before["merchant"]["entity_id"] != after["merchant"]["entity_id"])
            || matches!((name, native_cause), ("loot", "loot_open") | ("merchant", "merchant_open"))
    }

    /// The containers of a full view, given their handles and revisions, and what differs from the
    /// held ones (only when a subscriber wants it, so no copy is made otherwise).
    pub(crate) fn items_prepare(&mut self, view: &mut View, entered: bool) -> Vec<ItemChange> {
        let mut changes = Vec::new();
        for name in LISTS {
            let before = peek(&self.view, name).clone();
            let after = slot(view, name);
            if after.is_object() {
                let new_session = Self::items_session_is_new(name, &before, after, "");
                self.assign_item_refs(name, after, new_session);
            }
            let rev = self.item_revisions.get(name).copied().unwrap_or(0);
            if after.is_object() {
                after["revision"] = json!(rev);
            }
            if before == *after {
                continue;
            }
            if after.is_object() {
                after["revision"] = json!(rev + 1);
            }
            self.item_revisions.insert(name, rev + 1);
            if native_events::interested(&format!("{name}.changed")) {
                let cause = if entered {
                    "reset".to_string()
                } else {
                    cause_of(&before, after, "refresh")
                };
                changes.push(ItemChange { name, before, after: after.clone(), cause });
            }
        }
        changes
    }
    pub(crate) fn items_emit(&mut self, changes: Vec<ItemChange>) {
        for change in changes {
            let (name, data) = event_data(change);
            self.emit(&name, data);
        }
    }
    /// A zone change or disconnect: each container that was held is gone.
    pub(crate) fn items_reset_events(&mut self) {
        for name in LISTS {
            let before = peek(&self.view, name).clone();
            if before.is_null() || !native_events::interested(&format!("{name}.changed")) {
                continue;
            }
            let (event, data) = event_data(ItemChange { name, before, after: Json::Null, cause: "reset".into() });
            self.emit(&event, data);
        }
    }
    /// A container as the client holds it now (`after`, from the source capture), applied to the
    /// view: its handles and revision, and the change event when something differs.
    pub(crate) fn items_patch(&mut self, name: &'static str, mut after: Json, native_cause: &str) {
        let before = peek(&self.view, name).clone();
        if after.is_object() {
            let new_session = Self::items_session_is_new(name, &before, &after, native_cause);
            self.assign_item_refs(name, &mut after, new_session);
        }
        let rev = self.item_revisions.get(name).copied().unwrap_or(0);
        if after.is_object() {
            after["revision"] = json!(rev);
        }
        if before == after {
            return;
        }
        if after.is_object() {
            after["revision"] = json!(rev + 1);
        }
        self.item_revisions.insert(name, rev + 1);
        let wanted = native_events::interested(&format!("{name}.changed"));
        let cause = wanted.then(|| cause_of(&before, &after, native_cause));
        *slot(&mut self.view, name) = after.clone();
        if let Some(cause) = cause {
            let (event, data) = event_data(ItemChange { name, before, after, cause });
            self.emit(&event, data);
        }
    }

    pub(crate) fn emit_sandstorm_loot_delta(
        &mut self,
        taken: &[SandstormTakenItem],
        is_all: bool,
        fallback_before: Option<&Json>,
    ) {
        if taken.is_empty() || !native_events::interested("loot.changed") {
            return;
        }
        let before = {
            let current = peek(&self.view, "loot");
            if current.is_object() {
                current.clone()
            } else if let Some(fb) = fallback_before.filter(|v| v.is_object()) {
                fb.clone()
            } else {
                return;
            }
        };
        let mut after = before.clone();
        let Some(items) = after["items"].as_array_mut() else {
            return;
        };
        let mut remaining_taken: Vec<(u32, i64, i64)> = taken
            .iter()
            .map(|t| (t.inv_index, t.item_id, t.qty))
            .collect();
        let mut next_items = Vec::with_capacity(items.len());
        let mut any_changed = false;
        for mut entry in items.drain(..) {
            let key = entry["inventory_key"].as_u64().unwrap_or(u64::MAX) as u32;
            let qty = entry["quantity"].as_i64().unwrap_or(0);
            let mut take_here = 0i64;
            if let Some(slot_taken) = remaining_taken
                .iter_mut()
                .find(|(k, _, q)| *k == key && *q > 0)
            {
                take_here = slot_taken.2.min(qty);
                slot_taken.2 -= take_here;
            }
            if take_here > 0 {
                any_changed = true;
                let left = qty - take_here;
                if left > 0 {
                    entry["quantity"] = json!(left);
                    next_items.push(entry);
                }
            } else {
                next_items.push(entry);
            }
        }
        if !any_changed {
            return;
        }
        *items = next_items;
        let cause = if is_all { "loot_all" } else { "loot_one" }.to_string();
        let (event, data) = event_data(ItemChange {
            name: "loot",
            before,
            after,
            cause,
        });
        self.emit(&event, data);
    }
}

// ---- Lua ----

/// The record last built for each container, by (generation, revision): a mod that reads the bag every
/// frame gets the same read-only table until the bag changes.
#[derive(Default)]
struct Built(BTreeMap<&'static str, (u64, u64, Table)>);

fn get_container(lua: &Lua, symbol: &'static str, name: &'static str, args: MultiValue) -> Result<MultiValue> {
    if !args.is_empty() {
        return error(lua, symbol, "invalid_argument", "this function takes no arguments");
    }
    let state = CORE.with(|c| {
        let c = c.borrow();
        if !c.view.in_world {
            return None;
        }
        let v = peek(&c.view, name);
        Some((c.generation, v["revision"].as_u64().unwrap_or(0), v.is_null()))
    });
    let Some((generation, revision, closed)) = state else {
        return error(lua, symbol, "not_in_world", "enter a loaded world first");
    };
    if closed {
        return lua.pack_multi((Value::Nil, Value::Nil));
    }
    if let Some(built) = lua.app_data_ref::<Built>()
        && let Some((g, r, table)) = built.0.get(name)
        && *g == generation
        && *r == revision
    {
        return lua.pack_multi((Value::Table(table.clone()), Value::Nil));
    }
    let value = CORE.with(|c| peek(&c.borrow().view, name).clone());
    let built = record(lua, CONTAINER, &value)?;
    if let Value::Table(t) = &built {
        let mut cache = lua.remove_app_data::<Built>().unwrap_or_default();
        cache.0.insert(name, (generation, revision, t.clone()));
        lua.set_app_data(cache);
    }
    lua.pack_multi((built, Value::Nil))
}

/// Where a handle's stack is now, without copying it: its list, place and the container's revision,
/// and the handle's key and incarnation (the cache's).
fn locate(v: &Value, c: &Core) -> Option<(&'static str, usize, u64, (String, u64))> {
    let r = v.as_userdata()?.borrow::<Ref>().ok()?;
    if r.kind != "item" || r.generation != c.generation {
        return None;
    }
    let (list, _) = r.key.split_once(':')?;
    let list = LISTS.into_iter().find(|l| *l == list)?;
    let container = peek(&c.view, list);
    let index = container["items"].as_array()?.iter().position(|i| {
        i["ref"]["key"].as_str() == Some(r.key.as_str())
            && i["ref"]["incarnation"].as_u64() == Some(r.incarnation)
    })?;
    Some((list, index, container["revision"].as_u64().unwrap_or(0), (r.key.clone(), r.incarnation)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LootItemIdentity {
    pub item_id: String,
    pub name: String,
    pub qty_before: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SandstormTakenItem {
    pub inv_index: u32,
    pub item_id: i64,
    pub qty: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedItemSpec {
    pub list: &'static str,
    pub index: usize,
    pub inv_index: u32,
    pub key: String,
    pub generation: u64,
    pub incarnation: u64,
}

pub(crate) fn item_identity_of(v: &Value) -> Option<LootItemIdentity> {
    CORE.with(|c| {
        let c = c.borrow();
        let (list, index, _, _) = locate(v, &c)?;
        let item = peek(&c.view, list)["items"].as_array()?.get(index)?;
        Some(LootItemIdentity {
            item_id: item["id"].as_str()?.to_string(),
            name: item["name"].as_str()?.to_string(),
            qty_before: item["quantity"].as_i64().unwrap_or(1),
        })
    })
}

pub(crate) fn re_resolve_loot_handle(
    ident: &LootItemIdentity,
    count: Option<i64>,
) -> Option<ResolvedItemSpec> {
    let min_qty = count.unwrap_or(1);
    CORE.with(|c| {
        let c = c.borrow();
        let items = peek(&c.view, "loot")["items"].as_array()?;
        let mut best: Option<(usize, &Json)> = None;
        for (idx, it) in items.iter().enumerate() {
            if it["id"].as_str() != Some(ident.item_id.as_str())
                || it["name"].as_str() != Some(ident.name.as_str())
            {
                continue;
            }
            let qty = it["quantity"].as_i64().unwrap_or(0);
            if qty < min_qty {
                continue;
            }
            if qty == ident.qty_before {
                best = Some((idx, it));
                break;
            }
            if best.is_none() {
                best = Some((idx, it));
            }
        }
        let (index, item) = best?;
        let inv_index = item["inventory_key"].as_u64()? as u32;
        let key = item["ref"]["key"].as_str()?.to_string();
        let incarnation = item["ref"]["incarnation"].as_u64()?;
        Some(ResolvedItemSpec {
            list: "loot",
            index,
            inv_index,
            key,
            generation: c.generation,
            incarnation,
        })
    })
}

pub(crate) fn resolve_item_handle(
    v: &Value,
) -> std::result::Result<ResolvedItemSpec, (&'static str, String)> {
    let Some(ud) = v.as_userdata() else {
        return Err(("invalid_argument", "item must be an item handle".into()));
    };
    let Ok(r) = ud.borrow::<Ref>() else {
        return Err(("invalid_argument", "item must be an item handle".into()));
    };
    if r.kind != "item" {
        return Err(("invalid_argument", format!("expected item handle, got {}", r.kind)));
    }
    let key = r.key.clone();
    let generation = r.generation;
    let inc = r.incarnation;
    CORE.with(|c| {
        let c = c.borrow();
        let Some((list, index, _rev, (k, incarnation))) = locate(v, &c) else {
            return Err(("stale_handle", "item handle is stale or item no longer exists".into()));
        };
        let Some((_, raw_idx)) = k.split_once(':') else {
            return Err(("invalid_argument", "malformed item handle key".into()));
        };
        let Ok(inv_index) = raw_idx.parse::<u32>() else {
            return Err(("invalid_argument", "malformed item handle key".into()));
        };
        let _ = (key, generation, inc);
        Ok(ResolvedItemSpec {
            list,
            index,
            inv_index,
            key: k,
            generation: c.generation,
            incarnation,
        })
    })
}

pub(crate) fn is_handle_fresh(spec: &ResolvedItemSpec) -> bool {
    CORE.with(|c| {
        let c = c.borrow();
        if spec.generation != c.generation {
            return false;
        }
        peek(&c.view, spec.list)["items"]
            .as_array()
            .is_some_and(|items| {
                items.iter().any(|i| {
                    i["ref"]["key"].as_str() == Some(spec.key.as_str())
                        && i["ref"]["incarnation"].as_u64() == Some(spec.incarnation)
                })
            })
    })
}

/// The record last built for each stack read, by (generation, revision): a mod (or the Forge item
/// objects, which read one property at a time) that asks about the same stack again and again gets the
/// same read-only table until its container changes.
#[derive(Default)]
struct BuiltItems(std::collections::HashMap<(String, u64), (u64, u64, Table)>);

fn get_item(lua: &Lua, r: &Value) -> Result<MultiValue> {
    let symbol = "eqoa.items.get";
    if !is_item_ref(r) {
        return error(lua, symbol, "invalid_argument", "expected an ItemRef");
    }
    let (in_world, generation, found) = CORE.with(|c| {
        let c = c.borrow();
        (c.view.in_world, c.generation, locate(r, &c))
    });
    if !in_world {
        return error(lua, symbol, "not_in_world", "enter a loaded world first");
    }
    let Some((list, index, revision, tag)) = found else {
        return error(lua, symbol, "stale_handle", "the item is no longer in its list");
    };
    if let Some(cache) = lua.app_data_ref::<BuiltItems>()
        && let Some((g, rev, table)) = cache.0.get(&tag)
        && *g == generation
        && *rev == revision
    {
        return lua.pack_multi((Value::Table(table.clone()), Value::Nil));
    }
    let item = CORE.with(|c| peek(&c.borrow().view, list)["items"][index].clone());
    let built = record(lua, ITEM, &item)?;
    if let Value::Table(t) = &built {
        let mut cache = lua.remove_app_data::<BuiltItems>().unwrap_or_default();
        if cache.0.len() >= 512 {
            cache.0.clear();
        }
        cache.0.insert(tag, (generation, revision, t.clone()));
        lua.set_app_data(cache);
    }
    lua.pack_multi((built, Value::Nil))
}

/// The stack a handle names right now, or none (another list's, another world's, taken, or from a
/// session that closed).
fn resolve(v: &Value, c: &Core) -> Option<(&'static str, Json)> {
    let r = v.as_userdata()?.borrow::<Ref>().ok()?;
    if r.kind != "item" || r.generation != c.generation {
        return None;
    }
    let (list, _) = r.key.split_once(':')?;
    let list = LISTS.into_iter().find(|l| *l == list)?;
    peek(&c.view, list)["items"]
        .as_array()?
        .iter()
        .find(|i| {
            i["ref"]["key"].as_str() == Some(r.key.as_str())
                && i["ref"]["incarnation"].as_u64() == Some(r.incarnation)
        })
        .map(|i| (list, i.clone()))
}

fn is_item_ref(v: &Value) -> bool {
    v.as_userdata()
        .and_then(|u| u.borrow::<Ref>().ok())
        .is_some_and(|r| r.kind == "item")
}

/// `Ok(item)` for a live handle; else the error to hand back.
fn item_of(lua: &Lua, symbol: &str, v: &Value) -> std::result::Result<(&'static str, Json), MultiValue> {
    if !is_item_ref(v) {
        return Err(error(lua, symbol, "invalid_argument", "expected an ItemRef")
            .unwrap_or_default());
    }
    let (in_world, found) = CORE.with(|c| {
        let c = c.borrow();
        (c.view.in_world, resolve(v, &c))
    });
    if !in_world {
        return Err(error(lua, symbol, "not_in_world", "enter a loaded world first")
            .unwrap_or_default());
    }
    found.ok_or_else(|| {
        error(lua, symbol, "stale_handle", "the item is no longer in its list")
            .unwrap_or_default()
    })
}

fn int_arg(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(i) => Some(*i),
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.007_199_254_740_991e15 => {
            Some(*n as i64)
        }
        _ => None,
    }
}

/// The weapon's, wear slot's and stat's names the game shows, from its own words.
fn word(id: i32) -> Option<String> {
    CORE.with(|c| {
        c.borrow().words.as_ref().and_then(|w| {
            let t = w.get(id, 0);
            (t != "(null)" && !t.is_empty()).then(|| t.to_string())
        })
    })
}
thread_local! {static WINDOWS: std::cell::RefCell<Json> = const { std::cell::RefCell::new(Json::Null) };}
/// The raw state of the item page and the merchant window, as `items_capture` last read it (for the Forge
/// layer, which reads the windows' fields).
pub(crate) fn set_windows_raw(raw: Json) {
    WINDOWS.with(|w| *w.borrow_mut() = raw);
}
fn plain(lua: &Lua, v: &Json) -> Result<Value> {
    Ok(match v {
        Json::Null => Value::Nil,
        Json::Bool(b) => Value::Boolean(*b),
        Json::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        Json::String(s) => Value::String(lua.create_string(s)?),
        Json::Array(a) => {
            let t = lua.create_table()?;
            for (i, x) in a.iter().enumerate() {
                t.set(i + 1, plain(lua, x)?)?;
            }
            Value::Table(t)
        }
        Json::Object(o) => {
            let t = lua.create_table()?;
            for (k, x) in o {
                t.set(k.as_str(), plain(lua, x)?)?;
            }
            Value::Table(t)
        }
    })
}
/// The windows' raw state as a Lua table, or nil outside a world.
pub(crate) fn windows_raw(lua: &Lua) -> Result<Value> {
    let in_world = CORE.with(|c| c.borrow().view.in_world);
    if !in_world {
        return Ok(Value::Nil);
    }
    let raw = WINDOWS.with(|w| w.borrow().clone());
    plain(lua, &raw)
}

/// The game's word `id` (a window's label), when the words are loaded.
pub(crate) fn words(id: i32) -> Option<String> {
    word(id)
}
pub(crate) fn stat_name(id: i64) -> Option<String> {
    crate::infotext::stat_word(id).and_then(word)
}
pub(crate) fn wear_slot_name(slot: i64) -> Option<String> {
    let slot = if slot == 0xFFFF_FFFF { -1 } else { slot };
    crate::infotext::wear_slot_word(slot).and_then(word)
}
pub(crate) fn attack_name(attack_words: &[(i32, i64)], id: i64) -> Option<String> {
    crate::infotext::attack_word(attack_words, id).and_then(word)
}

/// The pinned paperdoll ordering (`inventory.lua` `GetSlotLabelAndOrder`), by wear slot.
fn slot_order(wear_slot: i64) -> Option<u64> {
    Some(match wear_slot {
        12 | 14..=18 => 1,
        13 => 2,
        1 => 3,
        2 => 4,
        5 => 5,
        9 => 6,
        19 => 7,
        7 => 8,
        6 => 9,
        10 => 10,
        11 => 11,
        4 => 12,
        3 => 13,
        8 => 14,
        _ => return None,
    })
}

/// A minimal native stack out of an Item record, for the native price and use rules.
fn net_item(item: &Json) -> net::dump::NetItem {
    let n = |k: &str| item[k].as_i64().unwrap_or(0);
    net::dump::NetItem {
        quantity: n("quantity"),
        cur_hp: n("current_hp"),
        item: net::dump::Item {
            value: n("value"),
            max_hp: n("maximum_hp"),
            level_req: n("level_required"),
            class_mask: n("class_mask"),
            race_mask: n("race_mask"),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn constants(lua: &Lua, module: &str, name: &str, rows: &[(&str, i64)]) -> Result<()> {
    let table = lua.create_table()?;
    for (key, id) in rows {
        table.raw_set(*key, *id)?;
        describe(lua, &format!("eqoa.{module}.{name}.{key}"), "supported", Some(5), "");
    }
    let modules: Table = lua.named_registry_value("eqoa_modules")?;
    modules
        .get::<Table>(module)?
        .set(name, guard(lua, table, &format!("eqoa.{module}.{name}"), None)?)?;
    describe(lua, &format!("eqoa.{module}.{name}"), "supported", Some(5), "");
    Ok(())
}

const SLOTS: [(&str, i64); 28] = [
    ("none", 0xFFFF_FFFF), ("head", 1), ("robe", 2), ("ear", 3), ("neck", 4), ("torso", 5), ("forearm", 6),
    ("two_forearm", 7), ("ring", 8), ("waist", 9), ("legs", 10), ("feet", 11), ("primary", 12), ("shield", 13),
    ("secondary", 14), ("two_hand", 15), ("bow", 16), ("thrown", 17), ("held", 18), ("gloves", 19),
    ("fishing", 20), ("bait", 21), ("weapon_craft", 22), ("armor_craft", 23), ("tailoring", 24),
    ("jewel_craft", 25), ("carpentry", 26), ("alchemy", 27),
];
const STATS: [(&str, i64); 19] = [
    ("str", 0), ("sta", 1), ("agi", 2), ("dex", 3), ("wis", 4), ("int", 5), ("cha", 6), ("hp", 8), ("pow", 10),
    ("hot", 12), ("pot", 13), ("ac", 14), ("poison_resist", 22), ("disease_resist", 23), ("fire_resist", 24),
    ("cold_resist", 25), ("lightning_resist", 26), ("arcane_resist", 27), ("fish", 28),
];
const ATTACKS: [(&str, i64); 11] = [
    ("none", 0), ("one_hand_slash", 1), ("two_hand_slash", 2), ("one_hand_blunt", 3), ("two_hand_blunt", 4),
    ("one_hand_pierce", 5), ("two_hand_pierce", 6), ("bow", 7), ("one_hand_crossbow", 8),
    ("two_hand_crossbow", 9), ("thrown", 10),
];

fn install_fields(lua: &Lua) {
    for (path, fields) in [
        (CONTAINER, "kind revision open items capacity used remaining full money view merchant source"),
        (
            ITEM,
            "ref list index inventory_key id name description icon quantity charges equipped equipped_slot quick_slot wear_slot slot_name level_required range max_stack value current_hp maximum_hp durability damage attack_type attack_name class_mask race_mask allowed_classes allowed_races equippable no_trade no_rent lore stats tabs",
        ),
        ("eqoa.items.ItemStat", "id name value"),
        ("eqoa.items.LootSource", "entity_id name"),
        ("eqoa.items.CanUse", "allowed reasons"),
        ("eqoa.items.SlotInfo", "id name order"),
        ("eqoa.items.MerchantState", "entity_id greeting pending sell_rate buy_rate mode selected_option options"),
        ("eqoa.items.ContainerView", "tab tabs selected selected_row rows"),
        ("eqoa.items.Tab", "index key label"),
        ("eqoa.items.MerchantOption", "key label selected"),
        ("eqoa.gems.Gem", "type rarity stat"),
    ] {
        describe(lua, path, "supported", Some(5), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(5), "");
        }
    }

}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    install_fields(lua);
    for (module, name, symbol) in [
        ("inventory", "inventory", "eqoa.inventory.get"),
        ("bank", "bank", "eqoa.bank.get"),
        ("loot", "loot", "eqoa.loot.get"),
        ("merchant", "merchant", "eqoa.merchant.get"),
    ] {
        bind(
            lua,
            module,
            "get",
            lua.create_function(move |lua, args: MultiValue| get_container(lua, symbol, name, args))?,
        )?;
    }
    bind(
        lua,
        "items",
        "get",
        lua.create_function(|lua, r: Value| get_item(lua, &r))?,
    )?;
    bind(
        lua,
        "items",
        "stat",
        lua.create_function(|lua, (r, stat): (Value, Value)| {
            let (_, item) = match item_of(lua, "eqoa.items.stat", &r) {
                Ok(v) => v,
                Err(e) => return Ok(e),
            };
            let Some(stat) = int_arg(&stat) else {
                return error(lua, "eqoa.items.stat", "invalid_argument", "stat_id must be an int");
            };
            let value = item["stats"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["id"].as_i64() == Some(stat)))
                .and_then(|s| s["value"].as_i64())
                .unwrap_or(0);
            lua.pack_multi((value, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "items",
        "can_use",
        lua.create_function(|lua, (r, class, race, level): (Value, Value, Value, Value)| {
            let (_, item) = match item_of(lua, "eqoa.items.can_use", &r) {
                Ok(v) => v,
                Err(e) => return Ok(e),
            };
            let (own_class, own_race, own_level) = CORE.with(|c| {
                let p = &c.borrow().view.player;
                (p["class_id"].as_i64(), p["race_id"].as_i64(), p["level"].as_i64())
            });
            let pick = |v: &Value, own: Option<i64>, what: &str| -> std::result::Result<i64, String> {
                if v.is_nil() {
                    own.ok_or_else(|| format!("the player's {what} is not known yet"))
                } else {
                    int_arg(v).ok_or_else(|| format!("{what} must be an int"))
                }
            };
            let (class, race, level) = match (
                pick(&class, own_class, "class_id"),
                pick(&race, own_race, "race_id"),
                pick(&level, own_level, "level"),
            ) {
                (Ok(c), Ok(r), Ok(l)) => (c, r, l),
                (Err(m), _, _) | (_, Err(m), _) | (_, _, Err(m)) => {
                    return error(lua, "eqoa.items.can_use", "invalid_argument", &m);
                }
            };
            let it = net_item(&item);
            let (c, r) = (1i64 << class.clamp(0, 31), 1i64 << race.clamp(0, 31));
            let mut reasons = Vec::new();
            if it.item.class_mask & c == 0 {
                reasons.push("class");
            }
            if it.item.race_mask & r == 0 {
                reasons.push("race");
            }
            if level < it.item.level_req {
                reasons.push("level");
            }
            lua.pack_multi((
                record(
                    lua,
                    "eqoa.items.CanUse",
                    &json!({"allowed":crate::inventory::can_use(&it, class, race, level),"reasons":reasons}),
                )?,
                Value::Nil,
            ))
        })?,
    )?;
    bind(
        lua,
        "items",
        "slot_info",
        lua.create_function(|lua, slot: Value| {
            let Some(id) = int_arg(&slot) else {
                return error(lua, "eqoa.items.slot_info", "invalid_argument", "wear_slot must be an int");
            };
            lua.pack_multi((
                record(
                    lua,
                    "eqoa.items.SlotInfo",
                    &json!({"id":id,"name":wear_slot_name(id),"order":slot_order(id)}),
                )?,
                Value::Nil,
            ))
        })?,
    )?;
    bind(
        lua,
        "items",
        "attack_name",
        lua.create_function(|lua, id: Value| {
            let Some(id) = int_arg(&id) else {
                return error(lua, "eqoa.items.attack_name", "invalid_argument", "attack_type must be an int");
            };
            let words = CORE.with(|c| {
                let c = c.borrow();
                (c.words.is_some() && !c.attack_words.is_empty()).then(|| c.attack_words.clone())
            });
            let Some(words) = words else {
                return error(lua, "eqoa.items.attack_name", "unavailable", "the game's words have not loaded");
            };
            lua.pack_multi((attack_name(&words, id), Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "items",
        "stat_name",
        lua.create_function(|lua, id: Value| {
            let Some(id) = int_arg(&id) else {
                return error(lua, "eqoa.items.stat_name", "invalid_argument", "stat_id must be an int");
            };
            if CORE.with(|c| c.borrow().words.is_none()) {
                return error(lua, "eqoa.items.stat_name", "unavailable", "the game's words have not loaded");
            }
            lua.pack_multi((stat_name(id), Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "merchant",
        "price",
        lua.create_function(|lua, (r, direction, quantity): (Value, Value, Value)| {
            let symbol = "eqoa.merchant.price";
            let (list, item) = match item_of(lua, symbol, &r) {
                Ok(v) => v,
                Err(e) => return Ok(e),
            };
            let direction = match &direction {
                Value::String(s) => s.to_string_lossy(),
                _ => String::new(),
            };
            if direction != "buy" && direction != "sell" {
                return error(lua, symbol, "invalid_argument", "direction must be \"buy\" or \"sell\"");
            }
            let quantity = if quantity.is_nil() {
                1
            } else {
                match uint(&quantity, u32::MAX as u64).filter(|q| *q >= 1) {
                    Some(q) => q,
                    None => return error(lua, symbol, "invalid_argument", "quantity must be an int of 1 or more"),
                }
            };
            let shop = CORE.with(|c| c.borrow().view.merchant["merchant"].clone());
            if !shop.is_object() {
                return error(lua, symbol, "wrong_window", "no merchant is open");
            }
            let it = net_item(&item);
            let unit = match (direction.as_str(), list) {
                ("buy", "merchant") => crate::merchant::merchant_price(&it, shop["buy_rate"].as_i64().unwrap_or(0) as i32),
                ("sell", "inventory") => crate::merchant::sell_price(&it, shop["sell_rate"].as_i64().unwrap_or(0) as i32),
                _ => {
                    return error(
                        lua,
                        symbol,
                        "invalid_argument",
                        "buy prices a merchant's ware and sell prices an inventory item",
                    );
                }
            };
            lua.pack_multi((unit.max(0) as u64 * quantity, Value::Nil))
        })?,
    )?;
    constants(lua, "items", "slots", &SLOTS)?;
    constants(lua, "items", "stats", &STATS)?;
    constants(lua, "items", "attack_types", &ATTACKS)?;
    super::gems::install(lua)?;
    super::items_forge::install(host)?;
    Ok(())
}
