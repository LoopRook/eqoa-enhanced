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
        "icon": u32::try_from(icon).ok().filter(|v| *v != 0),
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
    let inventory = container(
        "inventory",
        &play.dump.inventory,
        true,
        Some(crate::inventory::pack_cap()),
        Some(play.dump.player.p34),
        Json::Null,
        &ctx,
    );
    let bank = container(
        "bank",
        &play.dump.bank.items,
        play.mode == 11,
        Some(crate::inventory::bank_cap()),
        Some(play.dump.player.p38),
        Json::Null,
        &ctx,
    );
    let loot = match (play.mode == 1, play.npcw.corpse.as_ref()) {
        (true, Some(corpse)) => {
            let mut loot = container("loot", corpse, true, Some(LOOT_CAPACITY), None, Json::Null, &ctx);
            loot["source"] = SOURCE.with(|s| s.borrow().clone()).map_or(Json::Null, |(id, name)| json!({"entity_id":id,"name":name}));
            loot
        }
        _ => Json::Null,
    };
    let shop = &play.npcw.shop;
    let merchant = match (play.mode == 10, shop.id) {
        (true, Some(id)) => container(
            "merchant",
            &shop.wares,
            true,
            None,
            None,
            json!({"entity_id":id,"greeting":greeting(&shop.greeting),"pending":shop.pending,"sell_rate":shop.sell_rate,"buy_rate":shop.buy_rate}),
            &ctx,
        ),
        _ => Json::Null,
    };
    debug_assert_eq!(LISTS, ["inventory", "bank", "loot", "merchant"]);
    [inventory, bank, loot, merchant]
}

/// What a frame compares to see that a window opened or closed with no event of its own (a key
/// closed it) or tunar moved: scalars only. A stack's contents change through the native events in
/// `cause`, never by polling them; list lengths are here to catch a list replaced behind an event.
fn key(play: &Play) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (play.mode == 1, play.mode == 10, play.mode == 11, play.npcw.shop.id, play.npcw.shop.pending).hash(&mut h);
    (play.dump.player.p34, play.dump.player.p38).hash(&mut h);
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
// The corpse the open loot window belongs to. The loot request (0x0015) names the player's target when it is
// sent, and the reply (0x0018) does not say which corpse it is for, so the target at the reply is the corpse
// asked for only when it did not change in between. Fork code cannot see the request, so it is certain only
// when the target had been steady for `STEADY_MS` before the reply (a longer time than any reply takes); a
// target that changed more recently, or a target that is not a named corpse, leaves the source unknown.
thread_local! {static SOURCE: std::cell::RefCell<Option<(u32, String)>> = const { std::cell::RefCell::new(None) };}
// The target last seen and when it was first seen (the game's clock, milliseconds), kept while an addon
// wants state: once a frame and when a corpse arrives, so each is O(1).
thread_local! {static TARGET: Cell<Option<(u32, u32)>> = const { Cell::new(None) };}
const STEADY_MS: u32 = 1000;
#[cfg(test)]
thread_local! {static SKEW: Cell<u32> = const { Cell::new(0) };}
/// A test moves the game's clock on by `ms`.
#[cfg(test)]
pub(super) fn advance_clock(ms: u32) {
    SKEW.with(|s| s.set(s.get() + ms));
}

fn clock(play: &Play) -> u32 {
    #[cfg(test)]
    return play.session.now().wrapping_add(SKEW.with(|s| s.get()));
    #[cfg(not(test))]
    play.session.now()
}

/// Notes the target on the game's clock: the moment it was first seen is when it last changed.
fn observe_target(play: &Play) {
    let now = clock(play);
    TARGET.with(|t| {
        if t.get().map(|(id, _)| id) != Some(play.target) {
            t.set(Some((play.target, now)));
        }
    });
}

fn note_source(play: &Play) {
    observe_target(play);
    let steady = TARGET.with(|t| t.get()).is_some_and(|(_, since)| clock(play).wrapping_sub(since) >= STEADY_MS);
    let corpse = play.entities.slots.iter().flatten().find(|e| e.id == play.target && e.p18 & 2 != 0);
    let named = corpse.and_then(|e| e.creature.as_ref()).map(|c| c.name.clone()).filter(|n| !n.is_empty());
    SOURCE.with(|s| *s.borrow_mut() = named.filter(|_| steady).map(|n| (play.target, n)));
}

/// The key the containers were last read with: set by a full publish, and forgotten when the
/// addon side lets go or the world ends (`forget`).
pub(super) fn note(play: &Play) {
    observe_target(play);
    KEY.with(|k| k.set(Some(key(play))));
}
pub(super) fn forget() {
    TARGET.with(|t| t.set(None));
    KEY.with(|k| k.set(None));
}

/// Read the four lists now and apply them to the view when the core next flushes (`cause` names the
/// native event for subscribers).
pub(super) fn capture(play: &Play, cause: &'static str) {
    match cause {
        "loot_open" => note_source(play),
        "loot_close" => SOURCE.with(|s| *s.borrow_mut() = None),
        _ => {}
    }
    let lists = build(play);
    let bytes = lists.iter().map(native_core::json_len).sum::<usize>() + 512;
    note(play);
    native_core::defer(bytes, move |c| {
        for (name, after) in LISTS.into_iter().zip(lists) {
            c.items_patch(name, after, cause);
        }
    });
}

/// Once a frame (while an addon wants state): a list that moved with no event of its own (a window
/// closed by a key, a sale's tunar) is read then.
pub(super) fn refresh(play: &Play) {
    observe_target(play);
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
