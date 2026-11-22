// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The item containers as plain data (spec section 6): the pack and the bank from the dump, the
//! corpse while its window is up (mode 1), the merchant's wares while its window is up (mode 10).
//! A child of Play's adapter for its private fields. The copies are made only where an addon wants
//! them: when a game event touches an item list, and on a frame where a list's key moved (a window
//! opened or closed, tunar changed). Handles and revisions are the core's (`native_items`).
use super::Play;
use crate::fork_ext::addons::{
    native_core,
    native_items::{LISTS, attack_name, stat_name, wear_slot_name},
};
use net::dump::{ItemList, NetItem};
use serde_json::{Value as Json, json};
use std::cell::Cell;
use std::hash::{Hash, Hasher};

/// The loot window's rows (`loot.rs`: 40 items and the rows around them).
const LOOT_CAPACITY: usize = 40;

fn bits(mask: i64, count: usize) -> Vec<usize> {
    (0..count).filter(|b| (mask as u64 >> b) & 1 == 1).collect()
}

/// The item page's tab keys this stack appears under, by the same four predicates `itemmenu.rs` sorts with
/// (spec section 6, `Tab.key`, in the page's order).
fn tab_keys(it: &NetItem) -> Vec<&'static str> {
    let tabs: [(&str, fn(&NetItem) -> bool); 4] = [
        ("usable", crate::infotext::is_usable),
        ("equipment", crate::infotext::is_wearable),
        ("charged", crate::infotext::is_charged),
        ("misc", crate::infotext::is_misc),
    ];
    tabs.into_iter().filter(|(_, has)| has(it)).map(|(key, _)| key).collect()
}

/// One stack as the API's `Item` record (without its handle, which the core gives).
fn item_json(it: &NetItem, list: &str, index: usize, flags: u32, attack_words: &[(i32, i64)]) -> Json {
    let i = &it.item;
    let wear = if i.wear_slot < 0 { i.wear_slot & 0xFFFF_FFFF } else { i.wear_slot };
    let icon = crate::inventory::item_icon(it, flags);
    let stats = i
        .stats
        .iter()
        .map(|s| json!({"id":s.id,"name":stat_name(s.id),"value":s.value}))
        .collect::<Vec<_>>();
    json!({
        "list": list,
        "index": index + 1,
        "inventory_key": it.inv_index,
        "id": i.id.to_string(),
        "name": i.name,
        "description": i.desc,
        "icon": (icon as i32 != 0 && icon as i32 != -1).then_some(icon as u32),
        "quantity": it.quantity,
        "charges": it.charges,
        "equipped": it.equip_slot >= 0,
        "equipped_slot": (it.equip_slot >= 0).then_some(it.equip_slot + 1),
        "quick_slot": (it.quick_slot >= 0).then_some(it.quick_slot + 1),
        "wear_slot": wear,
        "slot_name": wear_slot_name(wear),
        "level_required": i.level_req,
        "range": i.range,
        "max_stack": i.max_stack,
        "value": i.value,
        "current_hp": it.cur_hp,
        "maximum_hp": i.max_hp,
        "durability": i.durability,
        "damage": i.damage,
        "attack_type": i.attack_type,
        "attack_name": attack_name(attack_words, i.attack_type),
        "class_mask": i.class_mask as u64 & 0xFFFF_FFFF,
        "race_mask": i.race_mask as u64 & 0xFFFF_FFFF,
        "allowed_classes": bits(i.class_mask, crate::fork_ext::addons::player::CLASSES.len()),
        "allowed_races": bits(i.race_mask, crate::fork_ext::addons::player::RACES.len()),
        "equippable": wear != 0 && wear != 0xFFFF_FFFF,
        "no_trade": i.no_trade != 0,
        "no_rent": i.rent == 0,
        "lore": i.lore != 0,
        "stats": stats,
        "tabs": tab_keys(it),
    })
}

/// The item page's tabs, by key, with the words of their labels (`itemmenu.rs`).
const TABS: [(&str, i32); 4] = [("usable", 437), ("equipment", 438), ("charged", 1473), ("misc", 439)];

/// `Container.view` of the pack while the item page is up: its tab, the stacks under it in the page's own
/// order (the same four predicates), and the row lit. `row_keys` and `selected_key` are wire keys the core
/// turns into handles.
fn pack_view(play: &Play) -> Json {
    if !play.items.visible {
        return Json::Null;
    }
    let (tab, menu, sel) = play.items.read_state();
    let tab = tab.clamp(0, 3);
    let has: fn(&NetItem) -> bool = match tab {
        0 => crate::infotext::is_usable,
        1 => crate::infotext::is_wearable,
        2 => crate::infotext::is_charged,
        _ => crate::infotext::is_misc,
    };
    let rows: Vec<u32> = play.dump.inventory.items.iter().filter(|it| has(it)).map(|it| it.inv_index).collect();
    let lit = if menu >= 0 && sel >= 0 { rows.get(sel as usize).copied() } else { None };
    json!({
        "tab": tab + 1,
        "tabs": TABS.iter().enumerate().map(|(i, (key, word))| json!({"index":i + 1,"key":key,"label":crate::fork_ext::addons::native_items::words(*word)})).collect::<Vec<_>>(),
        "row_list": "inventory",
        "row_keys": rows,
        "selected_key": lit,
        "selected_row": lit.map(|_| sel + 1),
    })
}

/// The merchant window's mode, its menu's options and the list's rows (`MerchantState`, `Container.view`):
/// buying lists the wares, selling the pack; the menu lists neither.
fn merchant_window(play: &Play, shop: &net::dump::ItemList) -> (Json, Json) {
    let (mode, option, sel) = play.npcw.merchant.read_state();
    let mode_name = match mode {
        1 => "sell",
        2 => "buy",
        _ => "menu",
    };
    let options = [("buy", 63), ("sell", 62), ("exit", 161)];
    let in_menu = mode == 0;
    let button_hint = crate::fork_ext::addons::native_items::words(161)
        .map(|label| format!("|t{label}"));
    let state = json!({
        "mode": mode_name,
        "selected_option": in_menu.then(|| options.get(option as usize).map(|o| o.0)).flatten(),
        "options": options.iter().enumerate().map(|(i, (key, word))| json!({"key":key,"label":crate::fork_ext::addons::native_items::words(*word),"selected":in_menu && i as i32 == option})).collect::<Vec<_>>(),
        "button_hint": button_hint,
    });
    let (list, rows): (&str, &[NetItem]) = match mode {
        1 => ("inventory", &play.dump.inventory.items),
        2 => ("merchant", &shop.items),
        _ => ("merchant", &[]),
    };
    let lit = (sel >= 0 && mode != 0).then(|| rows.get(sel as usize).map(|it| it.inv_index)).flatten();
    let view = json!({
        "tabs": [],
        "row_list": list,
        "row_keys": rows.iter().map(|it| it.inv_index).collect::<Vec<_>>(),
        "selected_key": lit,
        "selected_row": lit.map(|_| sel + 1),
    });
    (state, view)
}

struct Context<'a> {
    flags: u32,
    attack_words: &'a [(i32, i64)],
}

fn container(
    kind: &str,
    list: &ItemList,
    open: bool,
    capacity: Option<usize>,
    money: Option<i64>,
    merchant: Json,
    ctx: &Context,
) -> Json {
    let used = list.items.len();
    json!({
        "kind": kind,
        "open": open,
        "items": list.items.iter().enumerate().map(|(i, it)| item_json(it, kind, i, ctx.flags, ctx.attack_words)).collect::<Vec<_>>(),
        "used": used,
        "capacity": capacity,
        "remaining": capacity.map(|c| c.saturating_sub(used)),
        "full": capacity.map(|c| used >= c),
        "money": money,
        "merchant": merchant,
    })
}

fn greeting(wide: &[u16]) -> String {
    String::from_utf16_lossy(wide).trim_end_matches('\0').to_string()
}

/// The four containers, in `LISTS` order; `Null` for a corpse or merchant window that is not up.
pub(super) fn build(play: &Play) -> [Json; 4] {
    #[cfg(test)]
    BUILDS.with(|n| n.set(n.get() + 1));
    let ctx = Context {
        flags: play.session.expansion & net::session::EXPANSION_FLAGS,
        attack_words: play.start.as_ref().map_or(&[][..], |s| s.attack_words.as_slice()),
    };
    let mut inventory = container(
        "inventory",
        &play.dump.inventory,
        true,
        Some(crate::inventory::pack_cap()),
        Some(play.dump.player.p34),
        Json::Null,
        &ctx,
    );
    let view = pack_view(play);
    if !view.is_null() {
        inventory["view"] = view;
    }
    let bank = container(
        "bank",
        &play.dump.bank.items,
        crate::play::fork_hooks::bank_session_open(play),
        Some(crate::inventory::bank_cap()),
        Some(play.dump.player.p38),
        Json::Null,
        &ctx,
    );
    let loot = match (crate::play::fork_hooks::loot_session_open(play), play.npcw.corpse.as_ref()) {
        (true, Some(corpse)) => {
            let mut loot = container("loot", corpse, true, Some(LOOT_CAPACITY), None, Json::Null, &ctx);
            loot["source"] = SOURCE.with(|s| s.borrow().clone()).map_or(Json::Null, |(id, name)| json!({"entity_id":id,"name":name}));
            loot
        }
        _ => Json::Null,
    };
    let shop = &play.npcw.shop;
    let merchant = match (crate::play::fork_hooks::merchant_session_open(play), shop.id) {
        (true, Some(id)) => {
            let (mut state, view) = merchant_window(play, &shop.wares);
            let fields = json!({"entity_id":id,"greeting":greeting(&shop.greeting),"pending":shop.pending,"sell_rate":shop.sell_rate,"buy_rate":shop.buy_rate});
            state.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
            let mut merchant = container("merchant", &shop.wares, true, None, None, state, &ctx);
            merchant["view"] = view;
            merchant
        }
        _ => Json::Null,
    };
    debug_assert_eq!(LISTS, ["inventory", "bank", "loot", "merchant"]);
    [inventory, bank, loot, merchant]
}

/// The raw state of the item page and the merchant window for the Forge layer: the page's allocation (the
/// start menu was built, which builds the page with it), whether it is showing, its tab and lit row; the
/// merchant window's mode, menu option and list row, while a merchant is open.
fn windows_raw(play: &Play) -> Json {
    let (tab, _, lit) = play.items.read_state();
    let (mode, option, row) = play.npcw.merchant.read_state();
    let merchant = crate::play::fork_hooks::merchant_session_open(play) && play.npcw.shop.id.is_some();
    json!({
        "page": {
            "allocated": play.start.is_some(),
            "open": play.items.visible,
            "tab": tab.clamp(0, 3),
            "lit": lit,
            "rows": play.start.as_ref().map_or([40; 4], |s| s.item_counts).map(|c| c.max(crate::inventory::pack_cap() as i32)),
        },
        "merchant": merchant.then(|| json!({"mode": mode, "option": option, "lit": row})),
    })
}

/// What a frame compares to see that a window opened or closed with no event of its own (a key
/// closed it) or tunar moved: scalars only. A stack's contents change through the native events in
/// `cause`, never by polling them; list lengths are here to catch a list replaced behind an event.
fn key(play: &Play) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (
        crate::play::fork_hooks::loot_session_open(play),
        crate::play::fork_hooks::merchant_session_open(play),
        crate::play::fork_hooks::bank_session_open(play),
        play.npcw.shop.id,
        play.npcw.shop.pending,
    )
        .hash(&mut h);
    (play.dump.player.p34, play.dump.player.p38).hash(&mut h);
    // the item page's tab and lit row, the merchant window's mode, option and row: what a key moves
    (play.items.visible, play.items.read_state(), play.npcw.merchant.read_state(), play.start.is_some()).hash(&mut h);
    (
        play.dump.inventory.items.len(),
        play.dump.bank.items.items.len(),
        play.npcw.corpse.as_ref().map(|l| l.items.len()),
        play.npcw.shop.wares.items.len(),
    )
        .hash(&mut h);
    h.finish()
}

#[cfg(test)]
thread_local! {static BUILDS: Cell<usize> = const { Cell::new(0) };}
/// How many times the containers were copied since the last call (a test's measure of stack work).
#[cfg(test)]
pub(super) fn take_builds() -> usize {
    BUILDS.with(|n| n.replace(0))
}

thread_local! {static KEY: Cell<Option<u64>> = const { Cell::new(None) };}
// The corpse the open loot window belongs to: the one the client's loot request (0x0015, `loot_source`)
// named, when the corpse's list (0x0018) arrives within `REPLY_MS` of it and the entity is a named corpse.
// Whatever the player targets in between does not matter; with no recent request it stays unknown.
thread_local! {static SOURCE: std::cell::RefCell<Option<(u32, String)>> = const { std::cell::RefCell::new(None) };}

fn note_source(play: &Play) {
    let asked = crate::fork_ext::loot_source::reply();
    let corpse = asked.and_then(|id| play.entities.slots.iter().flatten().find(|e| e.id == id && e.p18 & 2 != 0));
    let named = corpse.and_then(|e| e.creature.as_ref()).map(|c| c.name.clone()).filter(|n| !n.is_empty());
    SOURCE.with(|s| *s.borrow_mut() = asked.zip(named));
}

#[derive(Clone, Debug)]
struct PendingLootItem {
    inv_index: u32,
    item_id: i64,
    qty_before: i64,
    count: i64,
    gained_qty: i64,
}

#[derive(Clone, Debug)]
struct PendingLootTake {
    corpse_id: Option<u32>,
    before_json: Json,
    is_all: bool,
    items: Vec<PendingLootItem>,
    closed: bool,
}

thread_local! {static PENDING_TAKE: std::cell::RefCell<Option<PendingLootTake>> = const { std::cell::RefCell::new(None) };}

pub(crate) fn current_loot_source_id(play: &Play) -> Option<u32> {
    if let Some((id, _)) = SOURCE.with(|s| s.borrow().clone()) {
        return Some(id);
    }
    // a corpse the mod opened by id (the player's target is not it): the server's relists are that corpse's too
    if let Some(id) = crate::fork_ext::addons::window_takeover::by_id_corpse() {
        return Some(id);
    }
    (play.target != 0
        && play
            .entities
            .slots
            .iter()
            .flatten()
            .any(|e| e.id == play.target && e.p18 & 2 != 0))
    .then_some(play.target)
}

pub(crate) fn clear_pending_loot() {
    PENDING_TAKE.with(|p| *p.borrow_mut() = None);
}

pub(crate) fn note_loot_send(play: &Play, key: Option<u32>, count: i64, is_all: bool) {
    if !native_core::capture_requested() {
        return;
    }
    let Some(corpse) = play.npcw.corpse.as_ref() else {
        return;
    };
    let items: Vec<PendingLootItem> = if is_all {
        corpse
            .items
            .iter()
            .filter(|it| it.quantity > 0)
            .map(|it| PendingLootItem {
                inv_index: it.inv_index,
                item_id: it.item.id,
                qty_before: it.quantity,
                count: it.quantity,
                gained_qty: 0,
            })
            .collect()
    } else if let Some(k) = key
        && let Some(it) = corpse.items.iter().find(|it| it.inv_index == k)
    {
        vec![PendingLootItem {
            inv_index: it.inv_index,
            item_id: it.item.id,
            qty_before: it.quantity,
            count: count.max(1).min(it.quantity),
            gained_qty: 0,
        }]
    } else {
        return;
    };
    if items.is_empty() {
        return;
    }
    let ctx = Context {
        flags: play.session.expansion & net::session::EXPANSION_FLAGS,
        attack_words: play.start.as_ref().map_or(&[][..], |s| s.attack_words.as_slice()),
    };
    let mut before_json = container("loot", corpse, true, Some(LOOT_CAPACITY), None, Json::Null, &ctx);
    before_json["source"] = SOURCE
        .with(|s| s.borrow().clone())
        .map_or(Json::Null, |(id, name)| json!({"entity_id":id,"name":name}));
    let corpse_id = current_loot_source_id(play);
    PENDING_TAKE.with(|p| {
        *p.borrow_mut() = Some(PendingLootTake {
            corpse_id,
            before_json,
            is_all,
            items,
            closed: false,
        });
    });
}

pub(crate) fn note_loot_inv_add(item_id: i64, qty: i64) {
    if qty <= 0 {
        return;
    }
    let late_emit = PENDING_TAKE.with(|p| {
        let mut borrow = p.borrow_mut();
        let Some(pending) = borrow.as_mut() else {
            return None;
        };
        let mut remaining = qty;
        for entry in &mut pending.items {
            if entry.item_id == item_id && entry.gained_qty < entry.count {
                let add = remaining.min(entry.count - entry.gained_qty);
                entry.gained_qty += add;
                remaining -= add;
                if remaining == 0 {
                    break;
                }
            }
        }
        if !pending.closed {
            return None;
        }
        let taken: Vec<crate::fork_ext::addons::native_items::SandstormTakenItem> = pending
            .items
            .iter()
            .filter(|e| e.gained_qty > 0)
            .map(|e| crate::fork_ext::addons::native_items::SandstormTakenItem {
                inv_index: e.inv_index,
                item_id: e.item_id,
                qty: e.gained_qty.min(e.qty_before),
            })
            .collect();
        if taken.is_empty() {
            None
        } else {
            let out = (taken, pending.is_all, pending.before_json.clone());
            *borrow = None;
            Some(out)
        }
    });
    if let Some((taken, is_all, before_json)) = late_emit {
        let bytes = native_core::json_len(&before_json) + 512;
        native_core::defer(bytes, move |c| {
            c.emit_sandstorm_loot_delta(&taken, is_all, Some(&before_json));
        });
    }
}

/// The key the containers were last read with: set by a full publish, and forgotten when the
/// addon side lets go or the world ends (`forget`).
pub(super) fn note(play: &Play) {
    KEY.with(|k| k.set(Some(key(play))));
}
pub(super) fn forget() {
    // the world ended: a request that was in flight and the corpse of the window are not the next world's
    crate::fork_ext::loot_source::clear();
    SOURCE.with(|s| *s.borrow_mut() = None);
    clear_pending_loot();
    crate::fork_ext::addons::native_items::set_windows_raw(Json::Null);
    KEY.with(|k| k.set(None));
}

/// Read the four lists now and apply them to the view when the core next flushes (`cause` names the
/// native event for subscribers).
pub(super) fn capture(play: &Play, cause: &'static str) {
    let mut pre_emit: Option<(
        Vec<crate::fork_ext::addons::native_items::SandstormTakenItem>,
        bool,
        Json,
    )> = None;
    match cause {
        "loot_open" => {
            note_source(play);
            let next_cid = current_loot_source_id(play);
            pre_emit = PENDING_TAKE.with(|p| {
                let pending = p.borrow_mut().take()?;
                if pending.corpse_id.is_none() || next_cid != pending.corpse_id {
                    return None;
                }
                let next_items = play.npcw.corpse.as_ref().map_or(&[][..], |c| c.items.as_slice());
                let mut before_qty: std::collections::BTreeMap<i64, i64> =
                    std::collections::BTreeMap::new();
                for entry in &pending.items {
                    *before_qty.entry(entry.item_id).or_default() += entry.qty_before;
                }
                // Count total in before_json in case multiple stacks shared item_id
                if let Some(arr) = pending.before_json["items"].as_array() {
                    before_qty.clear();
                    for it in arr {
                        if let Some(id_str) = it["id"].as_str()
                            && let Ok(id) = id_str.parse::<i64>()
                        {
                            *before_qty.entry(id).or_default() +=
                                it["quantity"].as_i64().unwrap_or(0);
                        }
                    }
                }
                let mut after_qty: std::collections::BTreeMap<i64, i64> =
                    std::collections::BTreeMap::new();
                for it in next_items {
                    *after_qty.entry(it.item.id).or_default() += it.quantity;
                }
                let mut taken = Vec::new();
                for entry in &pending.items {
                    if entry.gained_qty <= 0 {
                        continue;
                    }
                    let b = before_qty.get(&entry.item_id).copied().unwrap_or(0);
                    let a = after_qty.get(&entry.item_id).copied().unwrap_or(0);
                    let diff = (b - a).max(0).min(entry.gained_qty).min(entry.qty_before);
                    if diff > 0 {
                        *before_qty.get_mut(&entry.item_id).unwrap() -= diff;
                        taken.push(crate::fork_ext::addons::native_items::SandstormTakenItem {
                            inv_index: entry.inv_index,
                            item_id: entry.item_id,
                            qty: diff,
                        });
                    }
                }
                (!taken.is_empty()).then_some((taken, pending.is_all, pending.before_json))
            });
        }
        "loot_one" | "loot_all" | "loot_refused" => {
            if cause == "loot_refused" {
                crate::fork_ext::loot_source::refused();
            }
            clear_pending_loot();
        }
        "loot_close" => {
            SOURCE.with(|s| *s.borrow_mut() = None);
            crate::fork_ext::loot_source::closed();
            pre_emit = PENDING_TAKE.with(|p| {
                let mut borrow = p.borrow_mut();
                let pending = borrow.as_mut()?;
                let taken: Vec<crate::fork_ext::addons::native_items::SandstormTakenItem> = pending
                    .items
                    .iter()
                    .filter(|e| e.gained_qty > 0)
                    .map(|e| crate::fork_ext::addons::native_items::SandstormTakenItem {
                        inv_index: e.inv_index,
                        item_id: e.item_id,
                        qty: e.gained_qty.min(e.qty_before),
                    })
                    .collect();
                if taken.is_empty() {
                    pending.closed = true;
                    None
                } else {
                    let out = (taken, pending.is_all, pending.before_json.clone());
                    *borrow = None;
                    Some(out)
                }
            });
        }
        _ => {}
    }
    let lists = build(play);
    let extra_bytes = pre_emit
        .as_ref()
        .map_or(0, |(_, _, fb)| native_core::json_len(fb));
    let bytes = lists.iter().map(native_core::json_len).sum::<usize>() + extra_bytes + 512;
    note(play);
    let raw = windows_raw(play);
    native_core::defer(bytes, move |c| {
        crate::fork_ext::addons::native_items::set_windows_raw(raw);
        if let Some((taken, is_all, fb)) = pre_emit {
            c.emit_sandstorm_loot_delta(&taken, is_all, Some(&fb));
        }
        for (name, after) in LISTS.into_iter().zip(lists) {
            c.items_patch(name, after, cause);
        }
    });
}

/// Once a frame (while an addon wants state): a list that moved with no event of its own (a window
/// closed by a key, a sale's tunar) is read then.
pub(super) fn refresh(play: &Play) {
    if KEY.with(|k| k.get()) == Some(key(play)) {
        return;
    }
    capture(play, "frame");
}

/// The native events that change an item list, by the cause a subscriber sees.
pub(super) fn cause(event: &net::session::Event) -> Option<&'static str> {
    use net::session::Event::*;
    Some(match event {
        InvAdd(_) => "inv_add",
        InvTook { .. } => "inv_took",
        InvMove { .. } => "inv_move",
        InvUse { .. } => "inv_use",
        InvEquip { .. } => "inv_equip",
        InvUnequip(_) => "inv_unequip",
        QuickSlot { .. } => "quick_slot",
        GearSet { .. } => "gear_set",
        ItemHp { .. } => "item_hp",
        Opcode { op, .. } => match op {
            0x0018 => "loot_open",
            0x0016 => "loot_close",
            // why a corpse did not open: the answer to a loot request that has no list
            0x0085 => "loot_refused",
            0x0019 => "loot_one",
            0x001A => "loot_all",
            0x00B7 => "merchant_open",
            0x00D7 => "merchant_ready",
            0x0052 => "money",
            0x124D => "bank_open",
            0x1250 => "bank_took",
            0x1251 => "bank_added",
            0x1253 => "bank_money",
            0x1206 => "auction",
            // a catch (`fishing_caught`) adds to the pack with no inventory event of its own
            0x0F02 => "fishing",
            _ => return None,
        },
        _ => return None,
    })
}
