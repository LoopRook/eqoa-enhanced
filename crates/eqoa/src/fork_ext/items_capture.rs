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
        (true, Some(corpse)) => container("loot", corpse, true, Some(LOOT_CAPACITY), None, Json::Null, &ctx),
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

/// What a frame compares to see that a list moved without a game event naming it: the windows'
/// state, the money, and a hash of every stack's changing fields.
fn key(play: &Play) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (play.mode == 1, play.mode == 10, play.mode == 11, play.npcw.shop.id, play.npcw.shop.pending).hash(&mut h);
    (play.dump.player.p34, play.dump.player.p38).hash(&mut h);
    let corpse = play.npcw.corpse.as_ref();
    let lists = [
        Some(&play.dump.inventory),
        Some(&play.dump.bank.items),
        corpse,
        Some(&play.npcw.shop.wares),
    ];
    for list in lists {
        list.map(|l| l.items.len()).hash(&mut h);
        for it in list.into_iter().flat_map(|l| &l.items) {
            (it.inv_index, it.quantity, it.cur_hp, it.charges, it.equip_slot, it.quick_slot).hash(&mut h);
        }
    }
    h.finish()
}

thread_local! {static KEY: Cell<Option<u64>> = const { Cell::new(None) };}

/// The key the containers were last read with: set by a full publish, and forgotten when the
/// addon side lets go or the world ends (`forget`).
pub(super) fn note(play: &Play) {
    KEY.with(|k| k.set(Some(key(play))));
}
pub(super) fn forget() {
    KEY.with(|k| k.set(None));
}

/// Read the four lists now and apply them to the view when the core next flushes (`cause` names the
/// native event for subscribers).
pub(super) fn capture(play: &Play, cause: &'static str) {
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
            _ => return None,
        },
        _ => return None,
    })
}
