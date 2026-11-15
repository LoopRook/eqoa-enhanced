// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 5: the items API from Play-shaped state. Items are encoded as the server sends them and read
//! back with the client's own readers; the lists are changed by the same native functions the session
//! pump calls; the corpse, merchant and bank open from their real opcodes.
use super::*;
use crate::fork_ext::addons::{
    host::Host,
    native_core::{self, CORE, Core},
    native_events,
};
use net::dump::{ItemList, NetItem};
use net::session::Event;
use net::stream::{Reader, Writer};

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p
}

#[derive(Clone)]
struct Spec {
    id: i64,
    name: &'static str,
    key: u32,
    quantity: i64,
    cur_hp: i64,
    equip_slot: i64,
    quick_slot: i64,
    wear: i64,
    class_mask: i64,
    race_mask: i64,
    rent: i64,
    level: i64,
    max_hp: i64,
    value: i64,
    max_stack: i64,
    stats: Vec<(i64, i64)>,
}
impl Spec {
    fn new(id: i64, name: &'static str, key: u32) -> Self {
        Spec {
            id,
            name,
            key,
            quantity: 1,
            cur_hp: 0,
            equip_slot: -1,
            quick_slot: -1,
            wear: -1,
            class_mask: -1,
            race_mask: -1,
            rent: 1,
            level: 1,
            max_hp: 0,
            value: 10,
            max_stack: 1,
            stats: vec![],
        }
    }
    /// `ReadNetItem`'s bytes: the slot's six words, the item's 23 words, name, description, stats.
    fn bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.signed(self.quantity).signed(self.cur_hp).signed(0).signed(self.equip_slot).signed(self.quick_slot).u32(self.key);
        for v in [
            self.id, self.value, 0, 4001, 0, self.wear, 0, 0, self.rent, 0, 0, 0, 0, self.level, self.max_stack,
            self.max_hp, 0, self.class_mask, self.race_mask, 0, 0, 0, 0,
        ] {
            w.signed(v);
        }
        w.wide(self.name).wide("a described thing").signed(self.stats.len() as i64);
        for (id, value) in &self.stats {
            w.signed(*id).signed(*value);
        }
        w.data
    }
    fn net(&self) -> NetItem {
        net::dump::read_net_item(&mut Reader::new(&self.bytes()))
    }
}
fn list(specs: &[Spec]) -> ItemList {
    ItemList { p04: 0, items: specs.iter().map(Spec::net).collect() }
}
fn list_bytes(specs: &[Spec]) -> Vec<u8> {
    let mut w = Writer::default();
    w.signed(specs.len() as i64).u32(0);
    for s in specs {
        w.bytes(&s.bytes());
    }
    w.data
}
fn shop_bytes(id: u32, sell: i64, buy: i64, wares: &[Spec], greeting: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.u32(id).signed(sell).signed(buy).bytes(&list_bytes(wares)).wide(greeting);
    w.data
}
fn signed_bytes(v: i64) -> Vec<u8> {
    let mut w = Writer::default();
    w.signed(v);
    w.data
}

fn sword() -> Spec {
    let mut s = Spec::new(900_001, "Rusty Sword", 11);
    s.wear = 12;
    s.equip_slot = 0;
    s.cur_hp = 30;
    s.max_hp = 40;
    s.value = 100;
    s.stats = vec![(0, 3), (14, -2), (77, 5)];
    s
}
fn potion() -> Spec {
    let mut s = Spec::new(900_002, "Healing Potion", 12);
    s.quantity = 5;
    s.max_stack = 20;
    s.quick_slot = 1;
    s
}
fn ring() -> Spec {
    let mut s = Spec::new(900_003, "Iron Ring", 13);
    s.wear = 8;
    s.class_mask = 0b11;
    s.race_mask = 0b101;
    s.rent = 0;
    s.level = 20;
    s
}
fn gem() -> Spec {
    let mut s = Spec::new(900_004, "Moonstone", 21);
    s.quantity = 3;
    s.max_stack = 100;
    s
}

/// The session pump's own arm for each event this round follows, then the capture hooks around it.
fn apply(p: &mut Play, event: Event) {
    let capture = begin_game_event(p, &event);
    match &event {
        Event::InvAdd(item) => {
            let limit = item.item.max_stack;
            crate::inventory::add_item(&mut p.dump.inventory, item, limit, crate::inventory::pack_cap());
        }
        Event::InvTook { slot, n, .. } => crate::inventory::took(&mut p.dump.inventory, *slot as usize, *n as i64),
        Event::ItemHp { id, hp } => {
            if let Some(k) = crate::inventory::find(&p.dump.inventory, *id) {
                crate::inventory::set_hp(&mut p.dump.inventory, k, *hp as i64);
            }
        }
        Event::InvEquip { i, slot } => {
            let _ = crate::inventory::equip(&mut p.dump.inventory, *i as usize, *slot as i64);
        }
        Event::Opcode { op, data } => {
            assert!(
                p.bank_opcode(*op, data) || p.npcw_opcode(*op, data),
                "opcode {op:#06x} is one of the item windows'"
            );
        }
        other => panic!("not an item event: {other:?}"),
    }
    end_game_event(p, capture);
}
fn opcode(op: u16, data: Vec<u8>) -> Event {
    Event::Opcode { op, data }
}

const LOG: &str = r#"
log = {}
for _, n in ipairs({'inventory.changed', 'bank.changed', 'loot.changed', 'merchant.changed'}) do
  eqoa.events.on(n, function(e)
    local d = e.data
    log[#log + 1] = { name = e.name, cause = d.cause, changed = table.concat(d.changed, ','), before = d.before, after = d.after }
  end)
end
"#;

/// A world with a pack of three stacks, a bank of one, tunar in both, and `h` listening.
fn world() -> (Play, Host) {
    let mut p = ready();
    let h = Host::new().unwrap();
    h.execute(LOG).unwrap();
    p.dump.inventory = list(&[sword(), potion(), ring()]);
    p.dump.bank.items = list(&[gem()]);
    p.dump.player.p34 = 1234;
    p.dump.player.p38 = 99_000;
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    // the world's entry told each container once (cause reset); the tests count what comes after
    h.execute("assert(#log >= 2 and log[1].cause == 'reset'); log = {}").unwrap();
    (p, h)
}

#[test]
fn round5_pack_and_bank_come_from_the_dump_with_real_item_fields() {
    let mut p = ready();
    let h = Host::new().unwrap();
    h.execute("for _, f in ipairs({eqoa.inventory.get, eqoa.bank.get, eqoa.loot.get, eqoa.merchant.get}) do local v, e = f(); assert(v == nil and e.code == 'not_in_world') end").unwrap();
    p.dump.inventory = list(&[sword(), potion(), ring()]);
    p.dump.bank.items = list(&[gem()]);
    p.dump.player.p34 = 1234;
    p.dump.player.p38 = 99_000;
    p.forge_snapshot(None);
    h.execute(
        r#"
        local inv = assert(eqoa.inventory.get())
        assert(inv.kind == 'inventory' and inv.open == true and inv.used == 3 and inv.capacity == 40)
        assert(inv.remaining == 37 and inv.full == false and inv.money == 1234 and #inv.items == 3)
        local s, q, r = inv.items[1], inv.items[2], inv.items[3]
        assert(s.name == 'Rusty Sword' and s.list == 'inventory' and s.index == 1 and s.inventory_key == 11)
        assert(s.id == '900001' and s.description == 'a described thing' and s.icon == 4001)
        -- worn in native slot 0 is position 1; a quick slot of -1 is nil, not 0
        assert(s.equipped == true and s.equipped_slot == 1 and s.quick_slot == nil)
        assert(q.equipped == false and q.equipped_slot == nil and q.quick_slot == 2 and q.quantity == 5 and q.max_stack == 20)
        assert(s.current_hp == 30 and s.maximum_hp == 40 and s.value == 100 and s.wear_slot == 12 and s.equippable == true)
        -- no wear slot is the enum 0xFFFFFFFF, not nil and not equippable
        assert(q.wear_slot == 4294967295 and q.equippable == false)
        -- a mask of every bit lists every known class and race
        assert(#q.allowed_classes == 15 and q.allowed_classes[1] == 0 and q.allowed_classes[15] == 14)
        assert(#q.allowed_races == 10 and q.class_mask == 4294967295)
        assert(#r.allowed_classes == 2 and r.allowed_classes[2] == 1 and #r.allowed_races == 2 and r.allowed_races[2] == 2)
        -- rent == 0 is no-rent; rent 1 is not
        assert(r.no_rent == true and s.no_rent == false and s.no_trade == false and s.lore == false)
        -- a penalty keeps its sign, an unknown stat id keeps its value
        assert(#s.stats == 3 and s.stats[1].id == 0 and s.stats[1].value == 3)
        assert(s.stats[2].id == 14 and s.stats[2].value == -2)
        assert(s.stats[3].id == 77 and s.stats[3].value == 5 and s.stats[3].name == nil)
        assert(eqoa.items.stat(s.ref, 14) == -2 and eqoa.items.stat(s.ref, 99) == 0)
        local bank = assert(eqoa.bank.get())
        assert(bank.kind == 'bank' and bank.open == false and bank.used == 1 and bank.money == 99000 and bank.items[1].name == 'Moonstone')
        assert(bank.items[1].list == 'bank' and bank.capacity == 40)
        local v, e = eqoa.loot.get(); assert(v == nil and e == nil)
        local v, e = eqoa.merchant.get(); assert(v == nil and e == nil)
        -- the same table back until the bag changes
        assert(eqoa.inventory.get() == inv)
        -- a fact the client has no way to give is absent, and asking says so
        assert(inv.view == nil)
        "#,
    )
    .unwrap();
}

#[test]
fn round5_full_pack_counts_remaining_from_the_native_capacity() {
    let mut p = ready();
    let h = Host::new().unwrap();
    let many: Vec<Spec> = (0..40).map(|i| Spec::new(1000 + i, "Filler", 100 + i as u32)).collect();
    p.dump.inventory = list(&many);
    p.forge_snapshot(None);
    h.execute("local inv = assert(eqoa.inventory.get()); assert(inv.used == 40 and inv.remaining == 0 and inv.full == true and inv.capacity == 40)").unwrap();
    // the control: a server that holds more says so through the native cap, and the API follows it
    crate::inventory::with_caps([60, 40], || {
        p.dump.inventory = list(&many);
        capture_all(&p);
    });
    h.execute("local inv = assert(eqoa.inventory.get()); assert(inv.capacity == 60 and inv.remaining == 20 and inv.full == false)").unwrap();
}

/// Read the lists again as a frame does when something moved without an event of its own.
fn capture_all(p: &Play) {
    items::capture(p, "test");
    native_core::flush_deferred();
}

#[test]
fn round5_taking_an_item_compacts_the_list_and_handles_follow_their_stacks() {
    let (mut p, h) = world();
    h.execute("held = {}; for i, it in ipairs(eqoa.inventory.get().items) do held[i] = it.ref end; assert(#log == 0)").unwrap();
    // the whole first stack goes (0x003C): the potion and the ring move up
    apply(&mut p, Event::InvTook { slot: 0, sound: 0, n: 1 });
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        assert(#log == 1 and log[1].name == 'inventory.changed' and log[1].cause == 'inv_took')
        assert(log[1].changed == 'items,remaining,used', log[1].changed)
        assert(#log[1].before.items == 3 and #log[1].after.items == 2)
        local inv = eqoa.inventory.get()
        assert(inv.used == 2 and inv.remaining == 38 and inv.items[1].name == 'Healing Potion' and inv.items[1].index == 1)
        -- a handle taken earlier names the same stack at its new place
        local potion = assert(eqoa.items.get(held[2]))
        assert(potion.name == 'Healing Potion' and potion.index == 1 and potion.ref == held[2])
        assert(eqoa.items.get(held[3]).name == 'Iron Ring')
        -- the removed one is stale, not the stack that took its place
        local gone, e = eqoa.items.get(held[1])
        assert(gone == nil and e.code == 'stale_handle')
        local v, e = eqoa.items.stat(held[1], 0); assert(v == nil and e.code == 'stale_handle')
        local v, e = eqoa.items.get(7); assert(v == nil and e.code == 'invalid_argument')
        "#,
    )
    .unwrap();
    // the same stack put back is a new stack: its handle is not the old one
    apply(&mut p, Event::InvAdd(Box::new(sword().net())));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        assert(#log == 2 and log[2].cause == 'inv_add' and log[2].changed == 'items,remaining,used')
        local inv = eqoa.inventory.get()
        assert(inv.items[3].name == 'Rusty Sword' and inv.items[3].ref ~= held[1])
        local still, e = eqoa.items.get(held[1]); assert(still == nil and e.code == 'stale_handle')
        "#,
    )
    .unwrap();
}

#[test]
fn round5_stack_quantity_item_hp_and_tunar_each_change_once_with_their_cause() {
    let (mut p, h) = world();
    // 3 more potions join the stack of 5: the same stack, quantity 8
    let mut more = potion();
    more.quantity = 3;
    apply(&mut p, Event::InvAdd(Box::new(more.net())));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        held_potion = eqoa.inventory.get().items[2].ref
        assert(#log == 1 and log[1].cause == 'inv_add' and eqoa.inventory.get().items[2].quantity == 8 and eqoa.inventory.get().used == 3)
        assert(log[1].before.items[2].ref == log[1].after.items[2].ref, 'a stack that grew is the same stack')
        "#,
    )
    .unwrap();
    // item hit points (0x0060) for the sword (wire key 11)
    apply(&mut p, Event::ItemHp { id: 11, hp: 12 });
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#log == 2 and log[2].cause == 'item_hp' and eqoa.inventory.get().items[1].current_hp == 12 and log[2].before.items[1].current_hp == 30)").unwrap();
    // tunar (0x0052)
    apply(&mut p, opcode(0x0052, signed_bytes(1500)));
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#log == 3 and log[3].cause == 'money' and log[3].changed == 'money' and eqoa.inventory.get().money == 1500)").unwrap();
    // an event that changes nothing in the bag says nothing (the sword's hit points are already 12)
    apply(&mut p, Event::ItemHp { id: 11, hp: 12 });
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#log == 3)").unwrap();
    // the stack's handle survived all of it
    h.execute("assert(eqoa.items.get(held_potion).quantity == 8)").unwrap();
}

#[test]
fn round5_bank_opens_by_0x124d_moves_items_and_money_and_is_readable_closed() {
    let (mut p, h) = world();
    apply(&mut p, opcode(0x124D, vec![]));
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#log == 1, #log); assert(log[1].name == 'bank.changed' and log[1].cause == 'open' and log[1].changed == 'open', log[1].name .. log[1].cause .. log[1].changed); assert(eqoa.bank.get().open == true)").unwrap();
    // the banker's 0x1251: a ring added; 0x1250: the first stack's 2 of 3 taken; 0x1253: the bank's tunar
    let mut w = Writer::default();
    w.bytes(&ring().bytes());
    apply(&mut p, opcode(0x1251, w.data));
    let mut w = Writer::default();
    w.u8(0).signed(2);
    apply(&mut p, opcode(0x1250, w.data));
    apply(&mut p, opcode(0x1253, signed_bytes(5000)));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        local causes = {}; for i, e in ipairs(log) do causes[i] = e.name .. ':' .. e.cause .. ':' .. e.changed end
        assert(#log == 4, table.concat(causes, ' | '))
        assert(log[2].name == 'bank.changed' and log[2].cause == 'bank_added' and log[2].changed == 'items,remaining,used', log[2].changed)
        assert(log[3].cause == 'bank_took' and log[3].changed == 'items')
        assert(log[4].cause == 'bank_money' and log[4].changed == 'money')
        local bank = eqoa.bank.get()
        assert(bank.used == 2 and bank.money == 5000 and bank.items[1].quantity == 1 and bank.items[2].name == 'Iron Ring' and bank.open == true)
        "#,
    )
    .unwrap();
    // the window goes (a key, `bank_close`'s own state change): the contents stay readable, `open` is false
    p.mode = 0;
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#log == 5 and log[5].cause == 'close' and log[5].changed == 'open'); local bank = eqoa.bank.get(); assert(bank.open == false and bank.used == 2)").unwrap();
}

#[test]
fn round5_corpse_window_opens_loses_rows_and_closes_to_nothing() {
    let (mut p, h) = world();
    let loot = [Spec::new(2001, "Rat Pelt", 31), {
        let mut s = Spec::new(2002, "Snake Fang", 32);
        s.quantity = 2;
        s
    }];
    apply(&mut p, opcode(0x0018, list_bytes(&loot)));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        assert(#log == 1 and log[1].name == 'loot.changed' and log[1].cause == 'open' and log[1].before == nil)
        local c = assert(eqoa.loot.get())
        assert(c.kind == 'loot' and c.open == true and c.used == 2 and #c.items == 2 and c.items[1].name == 'Rat Pelt')
        -- the window's "Leave All" row is not an item and takes no position
        assert(c.items[2].index == 2 and c.items[1].list == 'loot')
        rat = c.items[1].ref; snake = c.items[2].ref
        "#,
    )
    .unwrap();
    // 0x0019 for corpse slot 0: the pelt goes into the pack
    let mut w = Writer::default();
    w.u8(0).signed(0);
    apply(&mut p, opcode(0x0019, w.data));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        assert(#log == 3, #log)
        -- the pack changed and the corpse changed, one event each
        local by = {}; for _, e in ipairs(log) do by[e.name] = e end
        assert(by['loot.changed'] == log[2] or by['loot.changed'].cause == 'loot_one')
        assert(by['inventory.changed'].cause == 'loot_one' and by['inventory.changed'].changed == 'items,remaining,used')
        local c = eqoa.loot.get(); assert(c.used == 1 and c.items[1].name == 'Snake Fang')
        assert(eqoa.items.get(snake).name == 'Snake Fang' and select(2, eqoa.items.get(rat)).code == 'stale_handle')
        assert(eqoa.inventory.get().items[4].name == 'Rat Pelt')
        "#,
    )
    .unwrap();
    // the window closes (0x0016): get is nil, nil and the old handles are stale, never a cached corpse
    apply(&mut p, opcode(0x0016, vec![]));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        assert(log[#log].name == 'loot.changed' and log[#log].cause == 'close' and log[#log].after == nil)
        local v, e = eqoa.loot.get(); assert(v == nil and e == nil)
        local v, e = eqoa.items.get(snake); assert(v == nil and e.code == 'stale_handle')
        "#,
    )
    .unwrap();
}

#[test]
fn round5_empty_corpse_is_open_with_no_items() {
    let (mut p, h) = world();
    apply(&mut p, opcode(0x0018, list_bytes(&[])));
    native_events::dispatch(&h).unwrap();
    h.execute("local c = assert(eqoa.loot.get()); assert(c.open == true and c.used == 0 and #c.items == 0 and c.full == false and c.remaining == 40)").unwrap();
}

#[test]
fn round5_merchant_opens_by_0x00b7_prices_natively_and_close_discards_the_wares() {
    let (mut p, h) = world();
    let mut axe = Spec::new(3001, "Bronze Axe", 41);
    axe.value = 15;
    axe.wear = 12;
    let wares = [axe, Spec::new(3002, "Bread", 42)];
    apply(&mut p, opcode(0x00B7, shop_bytes(7001, 90, 110, &wares, "Welcome, traveller!")));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        assert(#log == 1 and log[1].name == 'merchant.changed' and log[1].cause == 'open')
        local m = assert(eqoa.merchant.get())
        assert(m.kind == 'merchant' and m.open == true and m.used == 2 and m.capacity == nil and m.money == nil)
        assert(m.merchant.entity_id == 7001 and m.merchant.greeting == 'Welcome, traveller!' and m.merchant.pending == false)
        assert(m.merchant.sell_rate == 90 and m.merchant.buy_rate == 110)
        -- only what the client knows: the window's mode and selection are not given
        assert(m.merchant.mode == nil and m.merchant.options == nil)
        axe = m.items[1].ref
        -- 15 x 110% = 16.5: the native rounding makes it 17, and a pair is 34 (not 33)
        assert(eqoa.merchant.price(axe, 'buy') == 17 and eqoa.merchant.price(axe, 'buy', 2) == 34)
        -- selling the pack's sword (100 tunar, 30/40 worn) at 90%: 90 less the wear's 12.5% (11), as the native rounding has it
        local sword = eqoa.inventory.get().items[1].ref
        assert(eqoa.merchant.price(sword, 'sell') == 79, eqoa.merchant.price(sword, 'sell'))
        local v, e = eqoa.merchant.price(sword, 'buy'); assert(v == nil and e.code == 'invalid_argument')
        local v, e = eqoa.merchant.price(axe, 'trade'); assert(v == nil and e.code == 'invalid_argument')
        local v, e = eqoa.merchant.price(axe, 'buy', 0); assert(v == nil and e.code == 'invalid_argument')
        "#,
    )
    .unwrap();
    // the merchant is closed (the same state `merchant_close` leaves): no wares, no session, handles stale
    assert_eq!(crate::merchant::sell_price(&sword().net(), 90), 79, "the native rounding the Lua value above is held to");
    p.npcw_respawn();
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        assert(log[#log].name == 'merchant.changed' and log[#log].cause == 'close' and log[#log].after == nil)
        local v, e = eqoa.merchant.get(); assert(v == nil and e == nil)
        local v, e = eqoa.items.get(axe); assert(v == nil and e.code == 'stale_handle')
        local v, e = eqoa.merchant.price(eqoa.inventory.get().items[1].ref, 'sell'); assert(v == nil and e.code == 'wrong_window')
        "#,
    )
    .unwrap();
    // the same merchant again is a new session: an old handle never comes back
    apply(&mut p, opcode(0x00B7, shop_bytes(7001, 90, 110, &wares, "Again")));
    native_events::dispatch(&h).unwrap();
    h.execute("local v, e = eqoa.items.get(axe); assert(v == nil and e.code == 'stale_handle'); assert(eqoa.merchant.get().items[1].ref ~= axe)").unwrap();
}

#[test]
fn round5_zoning_resets_every_container_and_stales_every_handle() {
    let (mut p, h) = world();
    apply(&mut p, opcode(0x0018, list_bytes(&[Spec::new(2001, "Rat Pelt", 31)])));
    native_events::dispatch(&h).unwrap();
    h.execute("held = eqoa.inventory.get().items[1].ref; held_bank = eqoa.bank.get().items[1].ref; held_loot = eqoa.loot.get().items[1].ref; log = {}").unwrap();
    // the session tells the client it moved to another world (Msg000D / teleport): the core resets
    CORE.with(|c| c.borrow_mut().reset("zone"));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        local seen = {}
        for _, e in ipairs(log) do assert(e.cause == 'reset' and e.after == nil); seen[e.name] = true end
        assert(seen['inventory.changed'] and seen['bank.changed'] and seen['loot.changed'] and not seen['merchant.changed'])
        for _, f in ipairs({eqoa.inventory.get, eqoa.bank.get, eqoa.loot.get, eqoa.merchant.get}) do
          local v, e = f(); assert(v == nil and e.code == 'not_in_world')
        end
        for _, r in ipairs({held, held_bank, held_loot}) do local v, e = eqoa.items.get(r); assert(v == nil and e.code == 'not_in_world') end
        log = {}
        "#,
    )
    .unwrap();
    // back in a world: the pack and bank are there again, the corpse is not, and the old handles stay stale
    p.mode = 0;
    p.npcw.corpse = None;
    p.dump.inventory = list(&[potion()]);
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        local seen = {}
        for _, e in ipairs(log) do assert(e.cause == 'reset' and e.before == nil); seen[e.name] = true end
        assert(seen['inventory.changed'] and seen['bank.changed'] and not seen['loot.changed'])
        assert(eqoa.inventory.get().used == 1)
        for _, r in ipairs({held, held_bank, held_loot}) do local v, e = eqoa.items.get(r); assert(v == nil and e.code == 'stale_handle') end
        local v, e = eqoa.loot.get(); assert(v == nil and e == nil)
        "#,
    )
    .unwrap();
}

#[test]
fn round5_an_empty_pack_and_bank_are_empty_lists_not_missing() {
    let mut p = ready();
    let h = Host::new().unwrap();
    p.forge_snapshot(None);
    h.execute(
        r#"
        local inv = assert(eqoa.inventory.get()); assert(inv.used == 0 and #inv.items == 0 and inv.remaining == 40 and inv.full == false)
        local bank = assert(eqoa.bank.get()); assert(bank.used == 0 and #bank.items == 0 and bank.open == false)
        assert(inv.money == 0 and bank.money == 0)
        "#,
    )
    .unwrap();
}

#[test]
fn round5_pure_item_helpers_use_the_native_rules() {
    let (_p, h) = world();
    h.execute(
        r#"
        local inv = eqoa.inventory.get()
        local sword, potion, ring = inv.items[1].ref, inv.items[2].ref, inv.items[3].ref
        -- the ring: classes 0-1, races 0 and 2, level 20; the player (class 0, race 0) is level 1 in the fixture
        local player = eqoa.player.get()
        local r = assert(eqoa.items.can_use(ring))
        local expect_level = player.level < 20
        assert(r.allowed == (not expect_level) and (#r.reasons == (expect_level and 1 or 0)))
        -- the reasons come in the order class, race, level
        local r = assert(eqoa.items.can_use(ring, 5, 1, 3))
        assert(r.allowed == false and #r.reasons == 3 and r.reasons[1] == 'class' and r.reasons[2] == 'race' and r.reasons[3] == 'level')
        local r = assert(eqoa.items.can_use(ring, 1, 2, 20)); assert(r.allowed == true and #r.reasons == 0)
        local v, e = eqoa.items.can_use(ring, 'x'); assert(v == nil and e.code == 'invalid_argument')
        -- constants are the pinned ids and cannot be changed
        assert(eqoa.items.slots.primary == 12 and eqoa.items.slots.none == 4294967295 and eqoa.items.slots.gloves == 19)
        assert(eqoa.items.stats.str == 0 and eqoa.items.stats.ac == 14 and eqoa.items.stats.fish == 28)
        assert(eqoa.items.attack_types.bow == 7 and eqoa.items.attack_types.thrown == 10)
        assert(not pcall(function() eqoa.items.slots.head = 5 end))
        -- slot info: the id always, the pinned paperdoll order, an unknown id keeps its id
        local s = assert(eqoa.items.slot_info(12)); assert(s.id == 12 and s.order == 1)
        assert(assert(eqoa.items.slot_info(1)).order == 3 and assert(eqoa.items.slot_info(8)).order == 14)
        local u = assert(eqoa.items.slot_info(99)); assert(u.id == 99 and u.name == nil and u.order == nil)
        local v, e = eqoa.items.slot_info('x'); assert(v == nil and e.code == 'invalid_argument')
        -- without the game's words loaded the names are unavailable, not made up
        local v, e = eqoa.items.stat_name(0); assert(v == nil and e.code == 'unavailable')
        local v, e = eqoa.items.attack_name(1); assert(v == nil and e.code == 'unavailable')
        "#,
    )
    .unwrap();
    // with the words: a known stat and wear slot have the game's text, an unknown id is nil, nil
    let table = vi::strtab::StringTable::parse(
        (1..=1300).map(|i| format!("word {i}")).collect::<Vec<_>>().join("\n").as_bytes(),
    )
    .unwrap();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.words = Some(table);
        c.attack_words = vec![(500, 1), (501, 2)];
    });
    h.execute(
        r#"
        assert(eqoa.items.stat_name(0) == 'word 226' and eqoa.items.stat_name(14) == 'word 242')
        local v, e = eqoa.items.stat_name(77); assert(v == nil and e == nil)
        assert(eqoa.items.attack_name(2) == 'word 501')
        local v, e = eqoa.items.attack_name(9); assert(v == nil and e == nil)
        assert(eqoa.items.slot_info(12).name == 'word 338')
        "#,
    )
    .unwrap();
}

#[test]
fn round5_gems_use_the_shipped_lookup() {
    let h = Host::new().unwrap();
    h.execute(
        r#"
        local g = assert(eqoa.gems.get('Achroite')); assert(g.type == 'Armorsmithing/Tailoring' and g.rarity == 'Common' and g.stat == 'AR')
        assert(eqoa.gems.is_gem('Moonstone') == true and eqoa.gems.is_gem('moonstone') == false and eqoa.gems.is_gem('Rock') == false)
        local v, e = eqoa.gems.get('Rock'); assert(v == nil and e == nil)
        assert(eqoa.gems.get('Takish Ivory').stat == 'DEX, AGI')
        assert(eqoa.gems.format('Moonstone', 3) == 'Moonstone x3 (Weaponsmithing, Common, INT)')
        assert(eqoa.gems.format('Rock', 2) == 'Rock x2')
        assert(eqoa.gems.format('Moonstone', 3, function(n, a) return a .. ' ' .. n end) == '3 Moonstone (Weaponsmithing, Common, INT)')
        local v, e = eqoa.gems.format('Moonstone', 1, function() error('boom') end); assert(v == nil and e.code == 'failed')
        local v, e = eqoa.gems.format('Moonstone', -1); assert(v == nil and e.code == 'invalid_argument')
        local v, e = eqoa.gems.get(5); assert(v == nil and e.code == 'invalid_argument')
        "#,
    )
    .unwrap();
}

#[test]
fn round5_capabilities_list_every_new_function_and_say_what_is_missing() {
    let h = Host::new().unwrap();
    h.execute(
        r#"
        for _, p in ipairs({'eqoa.inventory.get', 'eqoa.bank.get', 'eqoa.loot.get', 'eqoa.merchant.get', 'eqoa.merchant.price',
            'eqoa.items.get', 'eqoa.items.can_use', 'eqoa.items.stat', 'eqoa.items.slot_info', 'eqoa.items.attack_name',
            'eqoa.items.stat_name', 'eqoa.items.slots', 'eqoa.items.stats', 'eqoa.items.attack_types',
            'eqoa.gems.get', 'eqoa.gems.is_gem', 'eqoa.gems.format', 'eqoa.events.inventory.changed'}) do
          local c = eqoa.capabilities.get(p)
          if p:find('^eqoa.events') then assert(c.status == 'supported' or c.status == 'unknown', p) else assert(c.status == 'supported', p .. ' ' .. tostring(c.status)) end
        end
        local c = eqoa.capabilities.get('eqoa.items.Container.view'); assert(c.status == 'not_yet' and c.round == 6)
        local c = eqoa.capabilities.get('eqoa.items.MerchantState.mode'); assert(c.status == 'not_yet')
        assert(eqoa.capabilities.has('eqoa.inventory.get'))
        local listed = {}
        for _, c in ipairs(eqoa.capabilities.list('eqoa.items')) do listed[c.path] = c.status end
        assert(listed['eqoa.items.get'] == 'supported' and listed['eqoa.items.slots'] == 'supported')
        "#,
    )
    .unwrap();
}

#[test]
fn round5_no_addon_means_no_item_copies_and_no_events() {
    let mut p = ready();
    native_core::fixture_demand(false);
    native_core::manager_demand(false);
    updates::inactive();
    p.dump.inventory = list(&[sword()]);
    let before_seq = CORE.with(|c| c.borrow().seq);
    for e in [
        Event::InvAdd(Box::new(potion().net())),
        Event::ItemHp { id: 11, hp: 5 },
        opcode(0x0052, signed_bytes(9)),
        opcode(0x0018, list_bytes(&[Spec::new(2001, "Rat Pelt", 31)])),
    ] {
        let capture = begin_game_event(&p, &e);
        assert!(!capture.interested && capture.item_cause.is_none());
        match &e {
            Event::InvAdd(item) => {
                crate::inventory::add_item(&mut p.dump.inventory, item, 20, crate::inventory::pack_cap());
            }
            Event::Opcode { op, data } => {
                p.npcw_opcode(*op, data);
            }
            _ => {}
        }
        end_game_event(&p, capture);
    }
    assert_eq!(native_core::deferred_len(), 0, "nothing was copied for an addon");
    CORE.with(|c| {
        let c = c.borrow();
        assert_eq!(c.seq, before_seq, "no event was made");
        assert!(c.view.inventory.is_null() && c.view.loot.is_null());
    });
}

#[test]
fn round5_forge_item_modules_follow_the_pinned_shapes() {
    let (mut p, h) = world();
    let mods = h.registered_modules();
    for name in ["inventory", "item", "bank", "loot", "merchant", "gems"] {
        let table = mods.get(&format!("frontiers_forge.{name}")).unwrap_or_else(|| panic!("{name} registered")).clone();
        h.lua.globals().set(name, table).unwrap();
    }
    let mut axe = Spec::new(3001, "Bronze Axe", 41);
    axe.value = 15;
    apply(&mut p, opcode(0x0018, list_bytes(&[Spec::new(2001, "Rat Pelt", 31), Spec::new(2002, "Fang", 32)])));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        -- inventory: 1-based positions, the live item objects, the tab enum 0..3
        assert(inventory.max_slots == 40 and inventory.GetTunar() == 1234 and inventory.SlotsUsed() == 3)
        assert(inventory.InventoryUsed() == 3 and inventory.SlotsRemaining() == 37 and inventory.IsFull() == false)
        local sword = assert(inventory.GetItem(1))
        assert(sword.idx == 1 and sword.name == 'Rusty Sword' and sword:GetName() == 'Rusty Sword' and sword.amount == 1)
        assert(sword.slot == 12 and sword.slot_name == 'PRIMARY' and sword.equippable == true and sword.equipped == true)
        assert(sword:GetEquippedStatus() == 'Equipped' and inventory.GetItem(2):GetEquippedStatus() == 'Not Equipped')
        assert(sword.item_id == 900001 and sword.value == 100 and sword.max_hp == 40 and sword.current_hp == 30)
        assert(sword.inventory_index == 11 and sword.equipped_slot == 0 and inventory.GetItem(2).equipped_slot == nil)
        assert(inventory.GetItem(0) == nil and inventory.GetItem(41) == nil and inventory.GetItem(4) == nil)
        assert(#inventory.GetItems() == 3 and inventory.GetIconRef(1) == 4001 and inventory.GetIconRef(9) == nil)
        assert(inventory.Tab.magic == 0 and inventory.Tab.equipment == 1 and inventory.Tab.tradeskill == 2 and inventory.Tab.misc == 3)
        local label, order = inventory.GetSlotLabelAndOrder(12); assert(label == 'Weapon' and order == 1)
        local label, order = inventory.GetSlotLabelAndOrder(99); assert(label == 'Slot 99' and order == 9999)
        -- the item module's names and masks
        assert(item.SLOT.PRIMARY == 12 and item.SLOT_NONE == 0xFFFFFFFF and item.STAT.AC == 14 and item.ATTACK.BOW == 7)
        assert(item.GetSlotName(12) == 'PRIMARY' and item.GetAttackTypeName(7) == 'BOW' and item.GetSlotName(99) == nil)
        local stats = sword:GetStats()
        assert(#stats == 3 and stats[1].name == 'STR' and stats[1].value == 3 and stats[2].name == 'AC' and stats[2].value == -2)
        assert(stats[3].id == 77 and stats[3].name == nil and sword:GetStat(14) == -2 and sword:GetStat(99) == 0 and sword:GetStatCount() == 3)
        local ring = inventory.GetItem(3)
        assert(ring:IsNoRent() == true and sword:IsNoRent() == false and ring.level_req == 20)
        assert(ring:IsUsableByClass(1) == true and ring:IsUsableByClass(2) == false and ring:IsUsableByClass(15) == false and ring:IsUsableByClass(nil) == false)
        assert(ring:IsUsableByRace(2) == true and ring:IsUsableByRace(1) == false and #ring:GetAllowedClasses() == 2 and ring:GetAllowedRaces()[2] == 2)
        assert(#inventory.GetItem(2):GetAllowedClasses() == 15 and #inventory.GetItem(2):GetAllowedRaces() == 10)
        local info = sword:GetInspectInfo()
        assert(info.name == 'Rusty Sword' and info.slot_name == 'PRIMARY' and info.equipped_status == 'Equipped' and info.inventory_index == 11)
        assert(info.item_id == 900001 and info.icon_ref == 4001 and info.attack_type_name == 'NONE')
        assert(sword:IsValid())
        -- bank: a dense array of the Forge record
        assert(bank.SlotsUsed() == 1)
        local b = bank.GetItems(); assert(#b == 1 and b[1].idx == 1 and b[1].name == 'Moonstone' and b[1].amount == 3 and b[1].level == 1 and b[1].slot == 4294967295)
        -- loot: counted from 1, the iterator gives the index and the item
        assert(loot.max_slots == 40 and loot.IsLootWindowOpen() == true and loot.GetCount() == 2)
        local seen = {}; for i, it in loot.Items() do seen[#seen + 1] = i .. it.name end
        assert(seen[1] == '1Rat Pelt' and seen[2] == '2Fang' and #loot.GetItems() == 2)
        assert(loot.GetItem(3) == nil and loot.GetIconRef(1) == 4001)
        rat = loot.GetItem(1)
        "#,
    )
    .unwrap();
    // the corpse is gone: the Forge reads give the closed answers, and the old object is no longer valid
    apply(&mut p, opcode(0x0016, vec![]));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        assert(loot.IsLootWindowOpen() == false and loot.GetCount() == 0 and #loot.GetItems() == 0 and loot.GetItem(1) == nil)
        assert(rat:IsValid() == false and rat.name == '' and rat.amount == 0 and rat:GetEquippedStatus() == 'Empty')
        -- merchant closed
        assert(merchant.IsOpen() == false and merchant.GetItemCount() == 0 and merchant.GetEntityId() == nil and merchant.GetGreeting() == nil)
        "#,
    )
    .unwrap();
    apply(&mut p, opcode(0x00B7, shop_bytes(7001, 90, 110, &[axe, Spec::new(3002, "Bread", 42)], "Hello there")));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        assert(merchant.Mode.menu == 0 and merchant.Mode.sell == 1 and merchant.Mode.buy == 2)
        assert(merchant.Option.buy == 0 and merchant.Option.sell == 1 and merchant.Option.exit == 2)
        assert(merchant.IsOpen() and merchant.GetEntityId() == 7001 and merchant.GetGreeting() == 'Hello there')
        assert(merchant.GetItemCount() == 2 and merchant.GetItem(1).name == 'Bronze Axe' and merchant.GetItem(1).idx == 1)
        assert(merchant.GetItem(0) == nil and merchant.GetItem(3) == nil and #merchant.GetItems() == 2 and merchant.max_slots == 40)
        -- the window's mode and selection are not given by the client: no made-up answer
        assert(merchant.GetMode() == nil and merchant.GetSelectedOption() == nil and merchant.GetOptions() == nil and merchant.GetSelectedItem() == nil)
        assert(inventory.IsWindowOpen() == nil and inventory.GetOpenTab() == nil and inventory.GetTabs() == nil and inventory.GetSelectedSlot() == nil)
        local c = eqoa.capabilities.get('frontiers_forge.merchant.GetMode'); assert(c.status == 'not_yet')
        -- gems: Get, IsGem and a Format that calls the real Get
        assert(gems.IsGem('Moonstone') == true and gems.IsGem('Rock') == false and gems.IsGem(nil) == nil)
        assert(gems.Get('Moonstone').rarity == 'Common' and gems.Get('Rock') == nil)
        assert(gems.Format('Moonstone', 3) == 'Moonstone (3) - INT - Common')
        assert(gems.Format('Moonstone', 1) == 'Moonstone - INT - Common' and gems.Format('Rock', 2) == nil)
        assert(gems.Format('Moonstone', 3, function(n, a) return n .. '!' .. a end) == 'Moonstone!3 - INT - Common')
        "#,
    )
    .unwrap();
}

#[test]
fn round5_a_kept_corpse_and_a_shop_behind_another_window_are_closed() {
    let (mut p, h) = world();
    // the world-entry dump's corpse is kept by the client but its window is not up (`npcw_world_entry`)
    p.npcw.corpse = Some(list(&[Spec::new(2001, "Rat Pelt", 31)]));
    // a merchant's 0x00B7 that arrives while another window is open is stored, not shown
    p.npcw.shop = crate::merchant::read_shop(&shop_bytes(7001, 90, 110, &[Spec::new(3002, "Bread", 42)], "Hi"));
    assert_eq!(p.mode, 0);
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        local v, e = eqoa.loot.get(); assert(v == nil and e == nil, 'a cached closed corpse is not given')
        local v, e = eqoa.merchant.get(); assert(v == nil and e == nil, 'wares of a window that is not up are not given')
        assert(#log == 0)
        "#,
    )
    .unwrap();
}

#[test]
fn round5_a_frame_sees_a_window_closed_and_tunar_changed_with_no_event_of_their_own() {
    let (mut p, h) = world();
    // from here the live path: one full publish was made, the frames after it are incremental
    native_core::fixture_demand(false);
    apply(&mut p, opcode(0x124D, vec![]));
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#log == 1 and log[1].cause == 'open'); log = {}").unwrap();
    // the bank window is closed by its own key (no packet), and a sale's tunar lands without a packet here
    p.mode = 0;
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#log == 1, #log); assert(log[1].name == 'bank.changed' and log[1].cause == 'close' and log[1].changed == 'open'); assert(eqoa.bank.get().open == false); log = {}").unwrap();
    p.dump.player.p34 = 4321;
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#log == 1 and log[1].name == 'inventory.changed' and log[1].cause == 'frame' and log[1].changed == 'money'); assert(eqoa.inventory.get().money == 4321)").unwrap();
    // a frame in which nothing moved says nothing
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#log == 1)").unwrap();
    native_core::fixture_demand(true);
}

#[test]
fn round5_a_corpse_opened_again_with_the_same_wire_keys_is_a_new_session() {
    let (mut p, h) = world();
    apply(&mut p, opcode(0x0018, list_bytes(&[Spec::new(2001, "Rat Pelt", 31)])));
    native_events::dispatch(&h).unwrap();
    h.execute("old = eqoa.loot.get().items[1].ref; assert(eqoa.items.get(old).name == 'Rat Pelt')").unwrap();
    // a second 0x0018 while the window is up: the client replaces the corpse; the key 31 is reused
    assert_eq!(p.mode, 1);
    apply(&mut p, opcode(0x0018, list_bytes(&[Spec::new(2002, "Snake Fang", 31)])));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        local v, e = eqoa.items.get(old); assert(v == nil and e.code == 'stale_handle', 'the old corpse item must be stale')
        local new = eqoa.loot.get().items[1]
        assert(new.name == 'Snake Fang' and new.ref ~= old and eqoa.items.get(new.ref).name == 'Snake Fang')
        "#,
    )
    .unwrap();
}

#[test]
fn round5_a_merchant_sent_again_by_the_same_merchant_is_a_new_session() {
    let (mut p, h) = world();
    let wares = [Spec::new(3001, "Bronze Axe", 41)];
    apply(&mut p, opcode(0x00B7, shop_bytes(7001, 90, 110, &wares, "Hi")));
    native_events::dispatch(&h).unwrap();
    h.execute("old = eqoa.merchant.get().items[1].ref").unwrap();
    assert_eq!(p.mode, 10);
    let again = [Spec::new(3009, "Steel Axe", 41)];
    apply(&mut p, opcode(0x00B7, shop_bytes(7001, 90, 110, &again, "Hi again")));
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        local v, e = eqoa.items.get(old); assert(v == nil and e.code == 'stale_handle')
        local new = eqoa.merchant.get().items[1]
        assert(new.name == 'Steel Axe' and new.ref ~= old and eqoa.items.get(new.ref).name == 'Steel Axe')
        "#,
    )
    .unwrap();
}

#[test]
fn round5_a_quiet_frame_does_no_stack_work_and_events_fire_once() {
    let (mut p, h) = world();
    native_core::fixture_demand(false);
    p.forge_snapshot(None);
    items::take_builds();
    for _ in 0..5 {
        p.forge_snapshot(None);
    }
    native_events::dispatch(&h).unwrap();
    assert_eq!(items::take_builds(), 0, "frames with nothing changed copy no stacks");
    h.execute("assert(#log == 0)").unwrap();
    // hit points arrive by their event: one copy, one event; the frames after copy nothing
    apply(&mut p, Event::ItemHp { id: 11, hp: 7 });
    assert_eq!(items::take_builds(), 1);
    p.forge_snapshot(None);
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    assert_eq!(items::take_builds(), 0);
    h.execute("assert(#log == 1 and log[1].cause == 'item_hp' and eqoa.inventory.get().items[1].current_hp == 7)").unwrap();
    // a window closed by a key: one close event, no more
    apply(&mut p, opcode(0x124D, vec![]));
    p.mode = 0;
    p.forge_snapshot(None);
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    h.execute("local n = 0; for _, e in ipairs(log) do if e.cause == 'close' then n = n + 1 end end; assert(n == 1, n)").unwrap();
    // a catch (0x0F02) adds to the pack with no inventory event: its own cause
    assert_eq!(items::cause(&opcode(0x0F02, vec![])), Some("fishing"));
    native_core::fixture_demand(true);
}

#[test]
fn round5_forge_is_valid_needs_a_slot_id_as_the_reference_does() {
    let mut p = ready();
    let h = Host::new().unwrap();
    let mut odd = Spec::new(900_009, "Odd Thing", 51);
    odd.wear = 0;
    p.dump.inventory = list(&[odd, sword()]);
    p.forge_snapshot(None);
    let mods = h.registered_modules();
    h.lua.globals().set("inventory", mods.get("frontiers_forge.inventory").unwrap().clone()).unwrap();
    h.execute(
        r#"
        local odd, sword = inventory.GetItem(1), inventory.GetItem(2)
        assert(odd.name == 'Odd Thing' and odd.slot == 0)
        assert(odd:IsValid() == false, 'slot id 0 is an empty record')
        assert(sword:IsValid() == true)
        "#,
    )
    .unwrap();
}
