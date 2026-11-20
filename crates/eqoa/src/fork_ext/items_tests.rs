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
    icon: i64,
    p4c: i64,
    p54: i64,
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
            icon: 4001,
            p4c: 0,
            p54: 0,
        }
    }
    /// `ReadNetItem`'s bytes: the slot's six words, the item's 23 words, name, description, stats.
    fn bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.signed(self.quantity).signed(self.cur_hp).signed(0).signed(self.equip_slot).signed(self.quick_slot).u32(self.key);
        for v in [
            self.id, self.value, 0, self.icon, 0, self.wear, 0, 0, self.rent, 0, 0, 0, 0, self.level, self.max_stack,
            self.max_hp, 0, self.class_mask, self.race_mask, self.p4c, 0, 0, self.p54,
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
    let capture = super::super::begin_game_event(p, &event);
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
    super::super::end_game_event(p, capture);
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
fn round7c_item_records_and_window_events_use_projected_native_state() {
    crate::fork_ext::addons::big_stack(|| {
        let (mut p, host) = world();
        let mut words = vec!["Fixture word"; 1474];
        for (id, label) in [
            (62, "Sell"),
            (63, "Buy"),
            (161, "Exit"),
            (437, "Usable"),
            (438, "Equipment"),
            (439, "Misc"),
            (1473, "Charged"),
        ] {
            words[id] = label;
        }
        CORE.with(|c| {
            c.borrow_mut().words = vi::strtab::StringTable::parse(words.join("\n").as_bytes());
        });
        p.bags_item_page(1, 0);
        frame(&p, &host);
        crate::fork_ext::addons::native_tests::install_field_check(&host).unwrap();
        host.execute(
            r#"
            local inv = eqoa.inventory.get()
            -- merchant: only at a merchant; source: only a corpse's loot
            fields('items.Container', inv, 'merchant source')
            -- selected/selected_row: only while a row is selected
            fields('items.ContainerView', inv.view, 'selected selected_row')
            fields('items.Tab', inv.view.tabs[1])
            local sword = inv.items[1]
            -- quick_slot/charges: only an item that has one; damage/attack_*: only a weapon
            fields('items.Item', sword, 'quick_slot charges damage attack_type attack_name')
            -- name: the stat's name comes from the disc's word table
            fields('items.ItemStat', sword.stats[1], 'name')
            fields('items.CanUse', eqoa.items.can_use(sword.ref, 0, 0, 50))
            fields('gems.Gem', eqoa.gems.get('Moonstone'))
            -- name: the slot's name comes from the disc's word table
            fields('items.SlotInfo', eqoa.items.slot_info(sword.wear_slot), 'name')
            seen = {}
            for _, n in ipairs({'loot.changed', 'merchant.changed'}) do
                eqoa.events.on(n, function(e) seen[n] = e end)
            end
        "#,
        )
        .unwrap();

        p.bags_close_item_page();
        let corpse_id = 1700;
        p.entities.slots.push(Some(net::entity::Entity {
            id: corpse_id,
            p18: 3,
            creature: Some(net::entity::Creature {
                name: "a beetle corpse".into(),
                ..Default::default()
            }),
            ..Default::default()
        }));
        p.target = corpse_id;
        crate::fork_ext::loot_source::requested(corpse_id);
        apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
        native_events::dispatch(&host).unwrap();
        host.execute(
            r#"
            local loot = eqoa.loot.get()
            -- money: only the pack and bank; view: only while its window shows; merchant: only at a merchant
            fields('items.Container', loot, 'money view merchant')
            fields('items.LootSource', loot.source)
            assert(log[#log].name == 'loot.changed', 'no loot event from the native open')
        "#,
        )
        .unwrap();
        apply(&mut p, opcode(0x0016, vec![]));
        apply(
            &mut p,
            opcode(0x00B7, shop_bytes(17, 90, 110, &[sword()], "Welcome")),
        );
        p.bags_merchant_open();
        frame(&p, &host);
        host.execute(
            r#"
            local shop = eqoa.merchant.get()
            fields('items.MerchantState', shop.merchant)
            fields('items.MerchantOption', shop.merchant.options[1])
            assert(log[#log].name == 'merchant.changed', 'no merchant event from the native open')
            event_fields(seen, {'loot.changed', 'merchant.changed'})
        "#,
        )
        .unwrap();
    });
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
        -- the window opens on its menu with Buy lit and no row lit; the button hint is still to come
        assert(m.merchant.mode == 'menu' and m.merchant.selected_option == 'buy' and #m.merchant.options == 3)
        assert(m.merchant.options[1].key == 'buy' and m.merchant.options[1].selected == true and m.merchant.options[3].key == 'exit')
        assert(m.view ~= nil and #m.view.tabs == 0 and #m.view.rows == 0 and m.view.selected == nil and m.view.selected_row == nil)
        assert(m.merchant.button_hint == nil)
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
        -- window state and the populated button hint are supported
        local c = eqoa.capabilities.get('eqoa.items.Container.view'); assert(c.status == 'supported')
        local c = eqoa.capabilities.get('eqoa.items.MerchantState.mode'); assert(c.status == 'supported')
        local c = eqoa.capabilities.get('eqoa.items.MerchantState.button_hint'); assert(c.status == 'supported')
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
        -- the window just opened on its menu, Buy lit: the mode and options are the client's, no row is lit
        assert(merchant.GetMode() == merchant.Mode.menu and merchant.GetSelectedOption() == merchant.Option.buy)
        assert(#merchant.GetOptions() == 3 and merchant.GetOptions()[1].selected == true and merchant.GetOptions()[2].selected == false)
        assert(merchant.GetSelectedItem() == nil and merchant.GetSelectedItemIndex() == nil)
        -- the item page is not up
        assert(inventory.IsWindowOpen() == false and inventory.GetOpenTab() == nil and #inventory.GetTabs() == 0 and inventory.GetSelectedSlot() == nil and inventory.GetSelectedItem() == nil and inventory.GetTabList() == nil)
        local c = eqoa.capabilities.get('frontiers_forge.merchant.GetMode'); assert(c.status == 'supported')
        local c = eqoa.capabilities.get('frontiers_forge.merchant.GetButtonHint'); assert(c.status == 'supported')
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

// ---- Which corpse a loot window belongs to (`Container.source`): the one the loot request named.
fn pelt() -> Vec<u8> {
    list_bytes(&[Spec::new(2001, "Rat Pelt", 31)])
}
const SOURCE: &str = "local s = eqoa.loot.get().source if s then return s.entity_id .. ':' .. s.name end return 'nil'";

#[test]
fn round5b_a_corpse_clicked_and_looted_at_once_is_the_source_of_its_loot_window() {
    let (mut p, h) = world();
    // the player clicks the corpse: it is targeted and the loot request goes out in the same action
    p.bags_target_corpse(5001, "a decaying skeleton");
    p.bags_press_loot();
    apply(&mut p, opcode(0x0018, pelt()));
    native_events::dispatch(&h).unwrap();
    assert_eq!(h.lua.load(SOURCE).eval::<String>().unwrap(), "5001:a decaying skeleton");
    // the window going away takes the source with it
    apply(&mut p, opcode(0x0016, vec![]));
    native_events::dispatch(&h).unwrap();
    h.execute("local v, e = eqoa.loot.get() assert(v == nil and e == nil)").unwrap();
}

#[test]
fn round5b_a_target_switched_before_the_reply_is_still_the_requested_corpse() {
    let (mut p, h) = world();
    p.bags_target_corpse(5001, "a decaying skeleton");
    p.bags_press_loot();
    // the player targets another corpse before the reply arrives
    p.bags_target_corpse(5002, "a rat's corpse");
    apply(&mut p, opcode(0x0018, pelt()));
    native_events::dispatch(&h).unwrap();
    assert_eq!(h.lua.load(SOURCE).eval::<String>().unwrap(), "5001:a decaying skeleton");
}

#[test]
fn round5b_requests_that_overlap_name_no_corpse_until_a_quiet_point() {
    let (mut p, h) = world();
    let said = |h: &Host| h.lua.load(SOURCE).eval::<String>().unwrap();
    let reply = |p: &mut Play, h: &Host| {
        apply(p, opcode(0x0018, pelt()));
        native_events::dispatch(h).unwrap();
    };
    // A, B, a reply, C, a reply: the first reply could be for either, and C's is told from none while the
    // state is tainted: both nil
    p.bags_target_corpse(5001, "a decaying skeleton");
    p.bags_press_loot();
    p.bags_target_corpse(5002, "a rat's corpse");
    p.bags_press_loot();
    reply(&mut p, &h);
    assert_eq!(said(&h), "nil", "A and B were both waiting");
    p.bags_target_corpse(5003, "a bat's corpse");
    p.bags_press_loot();
    reply(&mut p, &h);
    assert_eq!(said(&h), "nil", "B's reply may still be on its way: C's is not told apart");
    reply(&mut p, &h);
    assert_eq!(said(&h), "nil", "the last reply, tainted");
    // a quiet point: the window closes with nothing waiting; one request, one reply: named again
    apply(&mut p, opcode(0x0016, vec![]));
    p.bags_target_corpse(5004, "a gnoll's corpse");
    p.bags_press_loot();
    reply(&mut p, &h);
    assert_eq!(said(&h), "5004:a gnoll's corpse", "close, then a single request");
    // A with no reply for a long time, B, then a late reply: nothing says whose it is
    apply(&mut p, opcode(0x0016, vec![]));
    p.bags_target_corpse(5005, "a wolf's corpse");
    p.bags_press_loot();
    p.bags_target_corpse(5006, "a boar's corpse");
    p.bags_press_loot();
    reply(&mut p, &h);
    assert_eq!(said(&h), "nil", "a late reply to A, with B waiting");
    // the close with a request still waiting is not a quiet point (the reply of B is still to come)
    apply(&mut p, opcode(0x0016, vec![]));
    p.bags_target_corpse(5007, "a bear's corpse");
    p.bags_press_loot();
    reply(&mut p, &h);
    assert_eq!(said(&h), "nil", "B's reply could be the one that arrives");
    // sequential requests, each answered before the next: named every time
    reply(&mut p, &h);
    apply(&mut p, opcode(0x0016, vec![]));
    p.bags_target_corpse(5008, "an owl's corpse");
    p.bags_press_loot();
    reply(&mut p, &h);
    assert_eq!(said(&h), "5008:an owl's corpse");
    apply(&mut p, opcode(0x0016, vec![]));
    p.bags_target_corpse(5009, "a fox's corpse");
    p.bags_press_loot();
    reply(&mut p, &h);
    assert_eq!(said(&h), "5009:a fox's corpse");
}

#[test]
fn round5b_a_refused_request_is_answered_and_does_not_spoil_the_next() {
    let (mut p, h) = world();
    p.bags_target_corpse(5001, "a decaying skeleton");
    p.bags_press_loot();
    // the server refuses (0x0085, "too far"): that request has its answer
    apply(&mut p, opcode(0x0085, vec![1]));
    p.bags_target_corpse(5002, "a rat's corpse");
    p.bags_press_loot();
    apply(&mut p, opcode(0x0018, pelt()));
    native_events::dispatch(&h).unwrap();
    assert_eq!(h.lua.load(SOURCE).eval::<String>().unwrap(), "5002:a rat's corpse");
}

#[test]
fn round5b_a_request_does_not_survive_the_world_it_was_made_in() {
    let (mut p, h) = world();
    p.bags_target_corpse(5001, "a decaying skeleton");
    p.bags_press_loot();
    // the player zones before the reply; the new world has a corpse with the same entity id
    p.bags_zone();
    p.bags_set(&serde_json::json!({"pack": [], "bank": []}));
    p.forge_snapshot(None);
    native_events::dispatch(&h).unwrap();
    p.bags_target_corpse(5001, "a different corpse");
    apply(&mut p, opcode(0x0018, pelt()));
    native_events::dispatch(&h).unwrap();
    assert_eq!(h.lua.load(SOURCE).eval::<String>().unwrap(), "nil", "the old world's request is gone");
}

#[test]
fn round5b_a_reply_with_no_recent_request_for_a_corpse_has_no_source() {
    let (mut p, h) = world();
    // no request was made at all
    p.bags_target_corpse(5001, "a decaying skeleton");
    apply(&mut p, opcode(0x0018, pelt()));
    native_events::dispatch(&h).unwrap();
    assert_eq!(h.lua.load(SOURCE).eval::<String>().unwrap(), "nil");
    // attacking a living creature is not a loot request
    apply(&mut p, opcode(0x0016, vec![]));
    p.bags_target(2002);
    p.bags_press_loot();
    apply(&mut p, opcode(0x0018, pelt()));
    native_events::dispatch(&h).unwrap();
    assert_eq!(h.lua.load(SOURCE).eval::<String>().unwrap(), "nil");
}


/// A raster that draws nothing: the item page and the merchant window are driven by their keys.
struct Quiet;
impl crate::ui::Layout for Quiet {}
impl vi::ui::Raster for Quiet {
    fn blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
    fn stretch_blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
    fn fill_fan(&mut self, _: [f32; 4], _: [[i32; 2]; 4]) {}
    fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) {}
    fn draw_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16]) {}
    fn draw_centered_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16], _: u32) {}
    fn string_width(&self, s: &[u16]) -> i32 {
        s.len() as i32 * 8
    }
    fn point_size(&self) -> i32 {
        16
    }
    fn viewport(&self) -> [i32; 4] {
        [0, 0, 640, 448]
    }
    fn set_viewport(&mut self, _: [i32; 4]) {}
}
impl Play {
    /// The start menu's item page, opened on the usable tab, then `tabs` tab changes to the right and
    /// `downs` presses of Down in the tab's list: the game's own `ItemMenu` driven by its keys.
    pub(crate) fn bags_item_page(&mut self, tabs: usize, downs: usize) {
        use crate::pointer::key;
        let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
        let mut r = Quiet;
        let inv = self.dump.inventory.clone();
        let built = crate::itemmenu::build(&inv, [40; 4], &|_| 0, &|_| true);
        self.items.set_built(built);
        self.items.open(&vui, &r);
        let info = crate::infotext::Info { words: &vui.words, inv: &inv, hp: [0; 4], base: &self.stats.base, class: 0, level: 50, attack_words: &[] };
        let (mut cues, mut out) = (Vec::new(), Vec::new());
        self.items.draw(&vui, &mut r, &info, &mut cues);
        for k in std::iter::repeat_n(key::RIGHT, tabs).chain(std::iter::repeat_n(key::DOWN, downs)) {
            for down in [true, false] {
                self.items.key(&vui, &mut r, k, down, &inv, &info, false, &mut cues, &mut out);
            }
            // the page is drawn each frame: the tab's list takes its rows then
            self.items.draw(&vui, &mut r, &info, &mut cues);
        }
    }
    /// The merchant window opens on its menu after 0x00B7 (what the draw loop does with `merchant_open`).
    pub(crate) fn bags_merchant_open(&mut self) {
        let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
        let greeting = self.npcw.shop.greeting.clone();
        self.npcw.merchant.open(&vui, &mut Quiet, &greeting);
        self.npcw.merchant.visible = true;
        self.bags_merchant_keys(&[]);
    }
    /// Keys pressed in the merchant window (each down and up), the window drawn after each.
    pub(crate) fn bags_merchant_keys(&mut self, keys: &[i32]) {
        let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
        let mut r = Quiet;
        let inv = self.dump.inventory.clone();
        let info = crate::infotext::Info { words: &vui.words, inv: &inv, hp: [0; 4], base: &self.stats.base, class: 0, level: 50, attack_words: &[] };
        let coin = self.dump.player.p34 as i32;
        let (can_use, icon) = (|_: &NetItem| true, |_: &NetItem| 0);
        let npcw = &mut self.npcw;
        let view = crate::merchant::View { info: &info, shop: &npcw.shop, coin, can_use: &can_use, inv_icon: &icon };
        let (mut cues, mut out) = (Vec::new(), Vec::new());
        npcw.merchant.draw(&vui, &mut r, &view, &mut cues, &mut out);
        for k in keys {
            for down in [true, false] {
                npcw.merchant.key(&vui, &mut r, &view, *k, down, &mut cues, &mut out);
            }
            npcw.merchant.draw(&vui, &mut r, &view, &mut cues, &mut out);
        }
    }
    /// The start menu has been built (the item page with it, allocated but not showing).
    pub(crate) fn bags_start_built(&mut self) {
        let mut bytes = vec![0u8; 0x0200_0000];
        bytes[..4].copy_from_slice(b"SNR1");
        let img = crate::client_image::ClientImage::from_bytes(bytes).expect("a blank client image");
        let mut start = crate::startmenu::StartData::from_image(&img);
        start.item_counts = [40; 4];
        self.start = Some(start);
    }
    /// The start menu closes: the item page goes away.
    pub(crate) fn bags_close_item_page(&mut self) {
        self.items.visible = false;
    }
}

// ---- The item page and the merchant window's state, from the owner-approved read-only accessors.
/// What a frame does: sees a window's change with no event of its own, then tells the subscribers.
fn frame(p: &Play, h: &Host) {
    items::refresh(p);
    native_core::flush_deferred();
    native_events::dispatch(h).unwrap();
}

fn tabbed_pack() -> Vec<Spec> {
    let mut potion = Spec::new(900_002, "Healing Potion", 12);
    potion.p4c = 2;
    potion.quantity = 5;
    potion.max_stack = 20;
    let mut mana = Spec::new(900_020, "Mana Potion", 14);
    mana.p4c = 7;
    let mut wand = Spec::new(900_021, "Fire Wand", 15);
    wand.p4c = 3;
    wand.p54 = 5;
    vec![sword(), potion, ring(), mana, wand, Spec::new(900_022, "Rat Pelt", 16)]
}

#[test]
fn round5b_the_item_pages_tab_and_lit_row_are_the_pack_containers_view_and_each_change_is_one_event() {
    let (mut p, h) = world();
    p.dump.inventory = list(&tabbed_pack());
    capture_all(&p);
    native_events::dispatch(&h).unwrap();
    h.execute("log = {}; assert(eqoa.inventory.get().view == nil, 'no item page, no view')").unwrap();
    // the page opens on the usable tab with its first row lit: the potions
    p.bags_item_page(0, 0);
    frame(&p, &h);
    h.execute(
        r#"
        local c = eqoa.inventory.get(); local v = c.view
        assert(v ~= nil and v.tab == 1 and #v.tabs == 4 and v.tabs[1].key == 'usable' and v.tabs[3].key == 'charged' and v.tabs[3].index == 3 and v.tabs[4].key == 'misc')
        assert(#v.rows == 2 and v.rows[1] == c.items[2].ref and v.rows[2] == c.items[4].ref, 'the usable stacks, in the pack order')
        assert(v.selected_row == 1 and v.selected == c.items[2].ref)
        assert(#log == 1 and log[1].name == 'inventory.changed' and log[1].cause == 'frame' and log[1].changed:find('view', 1, true), log[1] and log[1].changed)
        "#,
    )
    .unwrap();
    // to the equipment tab and its second row (Right, Down): one change, one event
    p.bags_item_page(1, 1);
    frame(&p, &h);
    h.execute(
        r#"
        local c = eqoa.inventory.get(); local v = c.view
        assert(v.tab == 2 and #v.rows == 2 and v.rows[1] == c.items[1].ref and v.rows[2] == c.items[3].ref)
        assert(v.selected_row == 2 and v.selected == c.items[3].ref)
        assert(#log == 2 and log[2].cause == 'frame', #log)
        "#,
    )
    .unwrap();
    // nothing moved: a frame says nothing
    frame(&p, &h);
    frame(&p, &h);
    h.execute("assert(#log == 2)").unwrap();
    // the charged tab (the wand) and the misc tab (the pelt)
    p.bags_item_page(2, 0);
    frame(&p, &h);
    h.execute("local c = eqoa.inventory.get(); local v = c.view; assert(v.tab == 3 and #v.rows == 1 and v.rows[1] == c.items[5].ref and v.selected_row == 1 and #log == 3)").unwrap();
    p.bags_item_page(3, 0);
    frame(&p, &h);
    h.execute("local c = eqoa.inventory.get(); local v = c.view; assert(v.tab == 4 and #v.rows == 1 and v.rows[1] == c.items[6].ref and #log == 4)").unwrap();
    // the page closes: no view, one event
    p.bags_close_item_page();
    frame(&p, &h);
    h.execute("assert(eqoa.inventory.get().view == nil and #log == 5 and log[5].cause == 'frame')").unwrap();
    // Forge's calls over the same state, as the reference answers them (inventory.lua:178-285)
    let mods = h.registered_modules();
    h.lua.globals().set("inventory", mods.get("frontiers_forge.inventory").unwrap().clone()).unwrap();
    // the start menu was never built: the page's window is not allocated, so nothing is read from it
    h.execute("assert(inventory.IsWindowOpen() == false and inventory.GetOpenTab() == nil and inventory.GetTabList() == nil and inventory.GetTabList(1) == nil and #inventory.GetTabs() == 0 and inventory.GetSelectedSlot() == nil)").unwrap();
    p.bags_start_built();
    p.bags_item_page(1, 1);
    frame(&p, &h);
    h.execute(
        r#"
        assert(inventory.IsWindowOpen() == true and inventory.GetOpenTab() == inventory.Tab.equipment)
        local tabs = inventory.GetTabs()
        assert(#tabs == 4 and tabs[1].tab == 0 and tabs[4].tab == 3)
        local lit = 0
        for _, t in ipairs(tabs) do if t.selected then lit = lit + 1; assert(t.tab == inventory.Tab.equipment) end end
        assert(lit == 1, 'exactly the open tab is selected')
        -- the list child is a window object: its menu descriptor, its lit row
        local list = inventory.GetTabList(); assert(list ~= nil and list:IsVisible() == true and list:GetClassName() == 'VIWndMenu')
        local menu = list:GetMenu()
        assert(#menu.rows == 40 and menu.rows[1].parameter == 0 and menu.rows[1].name_handle ~= nil and menu.rows[1].quantity == 1)
        assert(menu.rows[2].parameter == 2, 'the ring is pack record 2, counted from zero')
        assert(menu.rows[3].name_handle == nil and menu.rows[3].quantity == nil, 'a row with no item behind it is a filler')
        assert(list:GetSelectedRow() == 2)
        assert(inventory.GetTabList(2):GetSelectedRow() == 1 and inventory.GetTabList(2):IsVisible() == false and #inventory.GetTabList(2):GetMenu().rows == 40)
        assert(inventory.GetTabList(2):GetMenu().rows[1].parameter == 4, 'the wand')
        assert(inventory.GetTabList(9) == nil)
        assert(inventory.GetSelectedSlot() == 3, 'the pack position of the lit stack, not its row in the tab')
        assert(inventory.GetSelectedItem().name == 'Iron Ring')
        "#,
    )
    .unwrap();
    // every method of the reference's WindowObject (window.lua:207-345, 412-430, 507) is on the list: the five
    // a native list can back answer, the rest return nothing and say so; none is a missing-method error
    let _ = h.drain_logs();
    h.execute(
        r#"
        local list = inventory.GetTabList()
        local backed = { 'IsVisible', 'GetClassName', 'GetMenu', 'GetSelectedRow', 'GetScrollTop' }
        local memory = { 'Read', 'ReadText', 'ReadTextPointer', 'Child', 'Parent', 'GetDrawAddress', 'GetDrawSlotAddress', 'GetChildWindows', 'IsActive' }
        for _, name in ipairs(backed) do
            local ok, v = pcall(function() return list[name](list) end)
            assert(ok and v ~= nil, name)
        end
        for _, name in ipairs(memory) do
            local ok, v = pcall(function() return list[name](list, 0x24) end)
            assert(ok and v == nil, name .. ' returns nothing and does not fail')
        end
        assert(list.base == nil)
        "#,
    )
    .unwrap();
    let logs = h.drain_logs().unwrap();
    for name in ["Read", "ReadText", "ReadTextPointer", "Child", "Parent", "GetDrawAddress", "GetDrawSlotAddress", "GetChildWindows", "IsActive"] {
        assert!(logs.iter().any(|s| s.contains(&format!("WindowObject:{name}"))), "{name} is diagnosed: {logs:?}");
    }
    assert!(!logs.iter().any(|s| s.contains("WindowObject:GetMenu") || s.contains("WindowObject:IsVisible")), "the backed ones are not: {logs:?}");
    // an empty tab (nothing charged in the pack): its first row is lit, and it is a filler: no slot
    p.dump.inventory = list(&[sword()]);
    capture_all(&p);
    p.bags_item_page(2, 0);
    frame(&p, &h);
    h.execute(
        r#"
        local c = eqoa.inventory.get()
        assert(inventory.GetOpenTab() == inventory.Tab.tradeskill)
        assert(inventory.GetTabList():GetSelectedRow() == 1, 'row ' .. tostring(inventory.GetTabList():GetSelectedRow()))
        assert(inventory.GetTabList():GetMenu().rows[1].name_handle == nil)
        assert(inventory.GetSelectedSlot() == nil and inventory.GetSelectedItem() == nil, 'slot')
        assert(c.view.selected == nil and c.view.selected_row == nil and #c.view.rows == 0, 'view')
        "#,
    )
    .unwrap();
    // the page put away, the window still allocated: the tab is still read, the page is not showing
    p.bags_close_item_page();
    frame(&p, &h);
    h.execute(
        r#"
        assert(inventory.IsWindowOpen() == false and #inventory.GetTabs() == 0, 'no tabs while the page is not showing')
        assert(inventory.GetOpenTab() == inventory.Tab.tradeskill, 'the window is still allocated and holds the tab it was left on')
        local list = inventory.GetTabList(); assert(list ~= nil and list:IsVisible() == false)
        assert(eqoa.inventory.get().view == nil)
        "#,
    )
    .unwrap();
}

#[test]
fn round5b_the_merchant_windows_mode_option_and_row_are_the_containers_state_and_each_change_is_one_event() {
    use crate::pointer::key;
    let (mut p, h) = world();
    let wares = [Spec::new(3001, "Bronze Axe", 41), Spec::new(3002, "Bread", 42)];
    apply(&mut p, opcode(0x00B7, shop_bytes(7001, 90, 110, &wares, "Hello there")));
    native_events::dispatch(&h).unwrap();
    p.bags_merchant_open();
    frame(&p, &h);
    h.execute(
        r#"
        log = {}
        local m = eqoa.merchant.get()
        assert(m.merchant.mode == 'menu' and m.merchant.selected_option == 'buy' and m.view.selected == nil and #m.view.rows == 0)
        assert(#m.merchant.options == 3 and m.merchant.options[1].selected == true and m.merchant.options[2].key == 'sell' and m.merchant.options[3].selected == false)
        "#,
    )
    .unwrap();
    // Down: the Sell option lit, one event
    p.bags_merchant_keys(&[key::DOWN]);
    frame(&p, &h);
    h.execute(
        r#"
        local m = eqoa.merchant.get()
        assert(m.merchant.selected_option == 'sell' and m.merchant.options[2].selected == true and m.merchant.options[1].selected == false)
        assert(#log == 1 and log[1].name == 'merchant.changed' and log[1].cause == 'frame' and log[1].changed:find('merchant', 1, true), log[1] and log[1].changed)
        "#,
    )
    .unwrap();
    // Cross: selling. The rows are the pack's stacks, the first lit; Down lights the second
    p.bags_merchant_keys(&[key::CROSS]);
    frame(&p, &h);
    h.execute(
        r#"
        local m = eqoa.merchant.get(); local inv = eqoa.inventory.get()
        assert(m.merchant.mode == 'sell' and m.merchant.selected_option == nil)
        assert(#m.view.rows == #inv.items and m.view.rows[1] == inv.items[1].ref and m.view.selected == inv.items[1].ref and m.view.selected_row == 1, 'selling lists the pack')
        assert(m.items[1].name == 'Bronze Axe', 'the wares stay the container items')
        assert(#log == 2)
        "#,
    )
    .unwrap();
    p.bags_merchant_keys(&[key::DOWN]);
    frame(&p, &h);
    h.execute("local m = eqoa.merchant.get(); local inv = eqoa.inventory.get(); assert(m.view.selected_row == 2 and m.view.selected == inv.items[2].ref and #log == 3)").unwrap();
    // Forge's calls, as the reference answers them (merchant.lua:97-237)
    let mods = h.registered_modules();
    h.lua.globals().set("merchant", mods.get("frontiers_forge.merchant").unwrap().clone()).unwrap();
    h.execute(
        r#"
        assert(merchant.GetMode() == merchant.Mode.sell)
        -- the menu's option is retained behind the list (Buy, which Cross in the menu reset it to), and listed
        assert(merchant.GetSelectedOption() == merchant.Option.buy and #merchant.GetOptions() == 3 and merchant.GetOptions()[1].selected == true)
        assert(merchant.GetSelectedItemIndex() == 2, 'the lit row + 1 in sell mode')
        assert(merchant.GetSelectedItem() == nil, 'sell mode lights a pack slot, not a ware')
        local v, e = merchant.GetButtonHint(); assert(v == nil)
        "#,
    )
    .unwrap();
    // Triangle: back to the menu; Cross on Buy: buying, the wares' rows
    p.bags_merchant_keys(&[key::TRIANGLE, key::CROSS]);
    frame(&p, &h);
    h.execute(
        r#"
        local m = eqoa.merchant.get()
        assert(m.merchant.mode == 'buy' and #m.view.rows == 2 and m.view.rows[1] == m.items[1].ref and m.view.selected == m.items[1].ref and m.view.selected_row == 1)
        assert(merchant.GetMode() == merchant.Mode.buy and merchant.GetSelectedItem().name == 'Bronze Axe' and merchant.GetSelectedItemIndex() == 1)
        "#,
    )
    .unwrap();
    // a merchant with no wares: buying lists nothing, its first row is lit, and the index is that row + 1
    apply(&mut p, opcode(0x0016, vec![]));
    p.npcw_respawn();
    apply(&mut p, opcode(0x00B7, shop_bytes(7002, 90, 110, &[], "Nothing today")));
    native_events::dispatch(&h).unwrap();
    p.bags_merchant_open();
    p.bags_merchant_keys(&[key::CROSS]);
    frame(&p, &h);
    h.execute(
        r#"
        local m = eqoa.merchant.get()
        assert(merchant.GetMode() == merchant.Mode.buy and #m.items == 0)
        assert(merchant.GetSelectedItemIndex() == 1, 'index ' .. tostring(merchant.GetSelectedItemIndex()))
        assert(merchant.GetSelectedItem() == nil, 'no ware behind the row')
        assert(m.view.selected == nil and m.view.selected_row == nil and #m.view.rows == 0, 'view')
        "#,
    )
    .unwrap();
    // the window closes with the merchant: nothing
    p.npcw_respawn();
    frame(&p, &h);
    h.execute(
        r#"
        local v, e = eqoa.merchant.get(); assert(v == nil and e == nil, 'closed')
        assert(merchant.GetMode() == nil, 'mode')
        assert(merchant.GetSelectedItem() == nil, 'item')
        assert(merchant.GetSelectedOption() == nil, 'option')
        assert(merchant.GetSelectedItemIndex() == nil, 'index')
        assert(type(merchant.GetOptions()) == 'table' and #merchant.GetOptions() == 0, 'no options, an empty table')
        "#,
    )
    .unwrap();
}

#[test]
fn round5b_the_merchants_selected_index_stops_at_forty_rows_as_the_reference_does() {
    use crate::pointer::key;
    let (mut p, h) = world();
    // 45 wares: buying lists them; the reference reports a row under 40 (merchant.lua:208-221)
    let wares: Vec<Spec> = (0..45).map(|i| Spec::new(4000 + i, "Ware", 60 + i as u32)).collect();
    apply(&mut p, opcode(0x00B7, shop_bytes(7003, 90, 110, &wares, "A big shop")));
    native_events::dispatch(&h).unwrap();
    p.bags_merchant_open();
    p.bags_merchant_keys(&[key::CROSS]);
    let mods = h.registered_modules();
    h.lua.globals().set("merchant", mods.get("frontiers_forge.merchant").unwrap().clone()).unwrap();
    frame(&p, &h);
    h.execute("assert(merchant.GetMode() == merchant.Mode.buy and merchant.GetSelectedItemIndex() == 1)").unwrap();
    p.bags_merchant_keys(&[key::DOWN; 38]);
    frame(&p, &h);
    h.execute("assert(merchant.GetSelectedItemIndex() == 39 and merchant.GetSelectedItem() ~= nil, tostring(merchant.GetSelectedItemIndex()))").unwrap();
    p.bags_merchant_keys(&[key::DOWN]);
    frame(&p, &h);
    h.execute("assert(merchant.GetSelectedItemIndex() == 40 and merchant.GetSelectedItem() ~= nil, 'row 40, the last index the reference gives: ' .. tostring(merchant.GetSelectedItemIndex()))").unwrap();
    // the client's list holds 40 rows and wraps: one more Down is the first row again
    p.bags_merchant_keys(&[key::DOWN]);
    frame(&p, &h);
    h.execute("assert(merchant.GetSelectedItemIndex() == 1, tostring(merchant.GetSelectedItemIndex()))").unwrap();
    // the layer's own limit, for a row the client could not light: raw row 40 is past the reference's range
    for (row, want) in [(39, "40"), (40, "nil"), (63, "nil")] {
        crate::fork_ext::addons::native_items::set_windows_raw(serde_json::json!({
            "page": {"allocated": false, "open": false, "tab": 0, "lit": 0, "rows": [40, 40, 40, 40]},
            "merchant": {"mode": 2, "option": 0, "lit": row},
        }));
        h.execute(&format!("assert(tostring(merchant.GetSelectedItemIndex()) == '{want}', 'raw row {row}: ' .. tostring(merchant.GetSelectedItemIndex()))")).unwrap();
    }
}

// ---- Round 5 Part B: the bag viewer's fixtures. The manager's tests (`addons::manager`) cannot name this
// module, so they drive the same native paths through these methods and plain JSON.
impl Spec {
    fn from_json(v: &serde_json::Value) -> Spec {
        let int = |k: &str, d: i64| v.get(k).and_then(|x| x.as_i64()).unwrap_or(d);
        let name: &'static str = Box::leak(v["name"].as_str().unwrap().to_string().into_boxed_str());
        let mut s = Spec::new(int("id", 0), name, int("key", 0) as u32);
        s.quantity = int("qty", 1);
        s.cur_hp = int("cur_hp", 0);
        s.equip_slot = int("equip", -1);
        s.quick_slot = int("quick", -1);
        s.wear = int("wear", -1);
        s.class_mask = int("class_mask", -1);
        s.race_mask = int("race_mask", -1);
        s.rent = int("rent", 1);
        s.level = int("level", 1);
        s.max_hp = int("max_hp", 0);
        s.value = int("value", 10);
        s.max_stack = int("max_stack", 1);
        s.icon = int("icon", 4001);
        s.p4c = int("p4c", 0);
        s.p54 = int("p54", 0);
        if let Some(stats) = v.get("stats").and_then(|x| x.as_array()) {
            s.stats = stats.iter().map(|p| (p[0].as_i64().unwrap(), p[1].as_i64().unwrap())).collect();
        }
        s
    }
}
fn specs(v: &serde_json::Value) -> Vec<Spec> {
    v.as_array().map(|a| a.iter().map(Spec::from_json).collect()).unwrap_or_default()
}
impl Play {
    /// The zone-in dump's pack, bank and both tunar (`{pack, bank, tunar, bank_tunar}`, items as `Spec::from_json`).
    pub(crate) fn bags_set(&mut self, world: &serde_json::Value) {
        self.dump.inventory = list(&specs(&world["pack"]));
        self.dump.bank.items = list(&specs(&world["bank"]));
        self.dump.player.p34 = world["tunar"].as_i64().unwrap_or(0);
        self.dump.player.p38 = world["bank_tunar"].as_i64().unwrap_or(0);
    }
    pub(crate) fn bags_player(&mut self, class: i64, race: i64, level: i64) {
        self.dump.player.class = class;
        self.dump.player.race = race;
        self.dump.player.level = level;
    }
    /// A corpse in the world and targeted, as when the player presses attack on it (the loot request names it).
    pub(crate) fn bags_target_corpse(&mut self, id: u32, name: &str) {
        let creature = net::entity::Creature { name: name.into(), ..Default::default() };
        self.entities.slots.push(Some(net::entity::Entity { id, p18: 2, creature: Some(creature), ..Default::default() }));
        self.target = id;
    }
    /// The player presses the attack button on the target: a loot request for a corpse (0x0015).
    pub(crate) fn bags_press_loot(&mut self) {
        self.attack_press();
    }
    /// The player targets another entity (the fixture's living Moss Snake is 2002).
    pub(crate) fn bags_target(&mut self, id: u32) {
        self.target = id;
    }
    /// 0x0018: a corpse's list arrives and its window opens.
    pub(crate) fn bags_corpse(&mut self, items: &serde_json::Value) {
        apply(self, opcode(0x0018, list_bytes(&specs(items))));
    }
    pub(crate) fn bags_replay(&mut self, row: &crate::fork_ext::session_replay::Row<'_>) {
        self.session.state = net::session::State::World;
        for event in row.deliver(&mut self.session) {
            apply(self, event);
        }
    }
    pub(crate) fn bags_record_take(&mut self, key: i32, count: i32) -> Vec<(u8, Vec<u8>)> {
        self.session.record = Some(Vec::new());
        self.bags_native_loot_one(key, count);
        self.session.record.take().unwrap()
    }
    /// 0x0019: the server's answer to taking the corpse's item in `slot`.
    pub(crate) fn bags_loot_one(&mut self, slot: u8) {
        let mut w = Writer::default();
        w.u8(slot).signed(0);
        apply(self, opcode(0x0019, w.data));
    }
    /// 0x001A: every corpse item into the pack, as far as it goes.
    pub(crate) fn bags_loot_all(&mut self) {
        apply(self, opcode(0x001A, vec![]));
    }
    /// 0x0016: the loot window closes (Leave All, or the window's own close).
    pub(crate) fn bags_loot_close(&mut self) {
        apply(self, opcode(0x0016, vec![]));
    }
    /// 0x003C: `n` of the pack's stack at `slot` gone.
    pub(crate) fn bags_took(&mut self, slot: i32, n: i32) {
        apply(self, Event::InvTook { slot: slot as _, sound: 0, n: n as _ });
    }
    /// An item added to the pack (0x003B).
    pub(crate) fn bags_added(&mut self, item: &serde_json::Value) {
        apply(self, Event::InvAdd(Box::new(Spec::from_json(item).net())));
    }
    /// 0x0052: the carried tunar.
    pub(crate) fn bags_tunar(&mut self, v: i64) {
        apply(self, opcode(0x0052, signed_bytes(v)));
    }
    /// The game's words, as far as the viewer reads them: a wear slot's and two stats' names (the rest "word N").
    pub(crate) fn bags_words(&mut self) {
        let named = |i: usize| match i {
            226 => "Strength".to_string(),
            242 => "Armor".to_string(),
            338 => "Primary".to_string(),
            i => format!("word {i}"),
        };
        let table = vi::strtab::StringTable::parse((1..=1600).map(named).collect::<Vec<_>>().join("
").as_bytes()).unwrap();
        CORE.with(|c| c.borrow_mut().words = Some(table));
    }
    /// The names the game's own item page lists under one of its tabs (`itemmenu.rs`'s four predicates).
    pub(crate) fn bags_native_tab(&self, tab: &str) -> Vec<String> {
        let has: fn(&NetItem) -> bool = match tab {
            "usable" => crate::infotext::is_usable,
            "equipment" => crate::infotext::is_wearable,
            "charged" => crate::infotext::is_charged,
            _ => crate::infotext::is_misc,
        };
        self.dump.inventory.items.iter().filter(|it| has(it)).map(|it| it.item.name.clone()).collect()
    }
    /// The server moves the player to another world; the next `bags_set` is the entry dump of the new one.
    pub(crate) fn bags_zone(&mut self) {
        CORE.with(|c| c.borrow_mut().reset("zone"));
        // what the session's Teleport / WorldEntry arm does with the capture
        items::forget();
        self.mode = 0;
        self.npcw.corpse = None;
    }
    pub(crate) fn bags_connect(&mut self) {
        self.session.conn = Some(usize::MAX);
    }
    pub(crate) fn bags_sends_lost(&self) -> u32 {
        self.session.sends_lost
    }
    pub(crate) fn bags_broker_tick(&mut self) {
        super::super::begin_iteration(self, true);
        let (vui, mut r) = (
            crate::pointer::testing::vui(),
            crate::pointer::testing::NullRaster,
        );
        self.run_npcw(&vui, &mut r);
        self.run_bank(&vui, &mut r);
    }
    pub(crate) fn bags_open_merchant(&mut self, id: u32) {
        self.npcw.shop = crate::merchant::Shop {
            id: Some(id),
            sell_rate: 100,
            buy_rate: 100,
            wares: Default::default(),
            greeting: Vec::new(),
            pending: false,
        };
        self.npcw.merchant.visible = true;
        self.mode = 10;
    }
    pub(crate) fn bags_close_merchant(&mut self) {
        self.npcw.merchant.visible = false;
        self.npcw.shop.id = None;
        self.mode = 0;
    }
    pub(crate) fn bags_shop_pending(&self) -> bool {
        self.npcw.shop.pending
    }
    pub(crate) fn bags_sell_confirm(&mut self, slot: usize, n: i64) {
        let ev = Event::InvTook { slot: slot as _, sound: 3, n: n as _ };
        let cap = super::super::begin_game_event(self, &ev);
        crate::inventory::took(&mut self.dump.inventory, slot, n);
        self.npcw_item_took(3);
        super::super::end_game_event(self, cap);
    }
    pub(crate) fn bags_open_bank(&mut self) {
        self.mode = 11;
        self.bank.state = 0;
    }
    pub(crate) fn bags_bank_state(&self) -> i32 {
        self.bank.state
    }
    pub(crate) fn bags_bank_deposit_confirm(&mut self, key: u32, id: u32, name: &str, qty: i64) {
        let spec = Spec::from_json(&serde_json::json!({"name": name, "id": id, "key": key, "qty": qty, "max_stack": 100}));
        let ev = opcode(0x1251, spec.bytes());
        let cap = super::super::begin_game_event(self, &ev);
        if let Event::Opcode { op, data } = &ev {
            let _ = self.bank_opcode(*op, data);
        }
        super::super::end_game_event(self, cap);
        self.bank.state = 0;
    }
    pub(crate) fn bags_bank_withdraw_confirm(&mut self, key: u32, qty: i64) {
        let slot = self
            .dump
            .bank
            .items
            .items
            .iter()
            .position(|it| it.inv_index == key)
            .unwrap_or(0) as u8;
        let mut w = Writer::default();
        w.u8(slot).signed(qty);
        let ev = opcode(0x1250, w.data);
        let cap = super::super::begin_game_event(self, &ev);
        if let Event::Opcode { op, data } = &ev {
            let _ = self.bank_opcode(*op, data);
        }
        super::super::end_game_event(self, cap);
        self.bank.state = 0;
    }
    pub(crate) fn bags_open_corpse_mode(&mut self) {
        self.mode = 1;
    }
    pub(crate) fn bags_player_at(&mut self, pos: [f32; 3]) {
        self.self_at = Some((pos, None));
    }
    /// The character in the world is called `name` (a login as another character).
    pub(crate) fn bags_set_name(&mut self, name: &str) {
        self.player.name = name.into();
    }
    /// Where the test put the player (the origin until it did).
    pub(crate) fn bags_player_pos(&self) -> [f32; 3] {
        self.self_at.map_or([0.0; 3], |(p, _)| p)
    }
    pub(crate) fn bags_spawn_npc(
        &mut self,
        id: u32,
        name: &str,
        npc_flags: u16,
        p18: u8,
        pos: [f32; 3],
    ) {
        self.player.world = 0;
        let creature = net::entity::Creature {
            name: name.into(),
            npc_flags,
            ..Default::default()
        };
        self.entities.slots.retain(|s| s.as_ref().is_none_or(|e| e.id != id));
        let slot = self
            .entities
            .slots
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                self.entities.slots.push(None);
                self.entities.slots.len() - 1
            });
        self.entities.slots[slot] = Some(net::entity::Entity {
            id,
            world: 0,
            kind: net::entity::TYPE_CREATURE,
            p18,
            pos,
            actor_pos: pos,
            creature: Some(creature),
            ..Default::default()
        });
        super::updates::capture(
            self,
            &super::updates::Kind::Entity(slot),
            None,
        );
    }
    pub(crate) fn bags_entity_die(&mut self, id: u32) {
        if let Some(slot) = self
            .entities
            .slots
            .iter()
            .position(|e| e.as_ref().is_some_and(|e| e.id == id))
        {
            if let Some(ent) = self.entities.slots[slot].as_mut() {
                ent.p18 = 3;
            }
            super::updates::capture(
                self,
                &super::updates::Kind::Entity(slot),
                None,
            );
        }
    }
    /// The player's own id, so combat hits can be told outgoing and a hostile "targeting you".
    pub(crate) fn bags_self_id(&mut self, id: u64) {
        self.session.self_id = Some(id);
    }
    /// A combat indication `attacker` -> `defender` (negative = damage), as the game's 0x0020 path delivers it.
    pub(crate) fn bags_combat_hit(&mut self, attacker: u32, defender: u32, amount: i32) {
        crate::fork_ext::addons::native_core::observe_hit(attacker, defender, amount, None);
    }
    /// Creature `id` is of `class` (p256: 2 = player, 0 = NPC) and targets `target`.
    pub(crate) fn bags_creature(&mut self, id: u32, class: u8, target: u32) {
        if let Some(slot) = self
            .entities
            .slots
            .iter()
            .position(|e| e.as_ref().is_some_and(|e| e.id == id))
        {
            if let Some(c) = self.entities.slots[slot].as_mut().and_then(|e| e.creature.as_mut()) {
                c.p256 = class;
                c.target = target;
            }
            super::updates::capture(self, &super::updates::Kind::Entity(slot), None);
        }
    }
    pub(crate) fn bags_corpse_held(&self) -> bool {
        self.npcw.corpse.is_some()
    }
    pub(crate) fn bags_auto_attack(&mut self, on: bool) {
        self.auto_attack = on;
        super::updates::capture(self, &super::updates::Kind::Player, None);
    }
    pub(crate) fn bags_target_sync(&mut self, id: u32) {
        self.target = id;
        super::updates::capture(
            self,
            &super::updates::Kind::Player,
            None,
        );
    }
    pub(crate) fn bags_merchant_packet(&mut self, id: u32) {
        apply(self, opcode(0x00B7, shop_bytes(id, 90, 110, &[sword()], "Shop")));
    }
    pub(crate) fn bags_bank_packet(&mut self) {
        apply(self, opcode(0x124D, vec![]));
    }
    pub(crate) fn bags_loot_confirm_all(&mut self) {
        let ev = opcode(0x001A, vec![]);
        let cap = super::super::begin_game_event(self, &ev);
        let _ = self.npcw_opcode(0x001A, &[]);
        super::super::end_game_event(self, cap);
    }
    pub(crate) fn bags_loot_confirm_one(&mut self, slot: u8, added: Option<&serde_json::Value>) {
        let mut w = Writer::default();
        w.u8(slot).u8(2);
        let data = w.data.clone();
        let ev = opcode(0x0019, w.data);
        let cap = super::super::begin_game_event(self, &ev);
        let _ = self.npcw_opcode(0x0019, &data);
        super::super::end_game_event(self, cap);
        if let Some(item) = added {
            apply(self, Event::InvAdd(Box::new(Spec::from_json(item).net())));
        }
    }
    pub(crate) fn bags_loot_refused(&mut self, code: u8) {
        apply(self, opcode(0x0085, vec![code]));
    }
    pub(crate) fn bags_chat(&mut self, text: &str) {
        let ev = Event::Chat(text.into(), [1.0; 4]);
        let cap = super::super::begin_game_event(self, &ev);
        self.server_line(text.into(), [1.0; 4]);
        super::super::end_game_event(self, cap);
    }
    pub(crate) fn bags_native_loot_one(&mut self, key: i32, count: i32) {
        let mut loot_out = vec![crate::loot::LootOut::One(key, count)];
        let mut merch_out = Vec::new();
        super::super::npcw_requests(self, &mut loot_out, &mut merch_out);
        let mut w = net::stream::Writer::op(0x0019);
        w.u32(key as u32).u32(count as u32);
        self.session.send(&w.data);
    }
    pub(crate) fn bags_fill_inventory_to(&mut self, target_len: usize) {
        let mut next_key = 9000u32;
        while self.dump.inventory.items.len() < target_len {
            next_key += 1;
            let spec = Spec::from_json(&serde_json::json!({
                "name": format!("Filler {next_key}"),
                "id": 910000 + next_key,
                "key": next_key,
                "wear": 1,
                "max_stack": 1,
            }));
            self.dump.inventory.items.push(spec.net());
        }
        items::forget();
        self.forge_snapshot(None);
    }
}

#[test]
fn round6b_items_use_equip_swap_quick_slot_and_stale_handles() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();

    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.dump.player.class = 0;
    p.dump.player.race = 0;
    p.dump.player.level = 10; // ring requires level 20!

    let mut bag_sword = sword();
    bag_sword.equip_slot = -1;
    let mut bag_pot = potion();
    bag_pot.quick_slot = -1;
    bag_pot.p4c = 2;
    let mut restricted_ring = ring();
    restricted_ring.equip_slot = -1;
    let extra_gem = gem();

    p.dump.inventory = list(&[bag_sword, bag_pot, restricted_ring, extra_gem]);
    p.forge_snapshot(None);

    let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
    host.execute(
        r#"
        results = {}
        eqoa.events.on("action.result", function(e)
            results[#results + 1] = e.data
        end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "k" }, function()
            last_req, last_err = next_fn()
        end)
        local inv = assert(eqoa.inventory.get())
        h_sword = inv.items[1].ref
        h_pot = inv.items[2].ref
        h_ring = inv.items[3].ref
        h_gem = inv.items[4].ref
    "#,
    )
    .unwrap();

    let run_lua = |p: &mut Play, code: &str| {
        host.execute(code).unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        native_events::dispatch(&host).unwrap();
        super::super::begin_iteration(p, true);
        let (vui, mut r) = (
            crate::pointer::testing::vui(),
            crate::pointer::testing::NullRaster,
        );
        p.run_npcw(&vui, &mut r);
        p.run_bank(&vui, &mut r);
        native_events::dispatch(&host).unwrap();
    };

    // equip & use_item on usable sword -> ok
    let before = p.session.sends_lost;
    run_lua(&mut p, "next_fn = function() return eqoa.actions.equip(h_sword) end");
    assert!(p.session.sends_lost > before);
    host.execute("assert(results[#results].ok == true and results[#results].kind == 'equip')").unwrap();

    // equip restricted ring (level 20 > 10) -> refused with native reason
    run_lua(&mut p, "next_fn = function() return eqoa.actions.equip(h_ring) end");
    host.execute("assert(results[#results].ok == false and string.find(results[#results].reason, 'level 20', 1, true))").unwrap();

    // equip non-wearable potion -> invalid_argument at dispatch
    run_lua(&mut p, "next_fn = function() return eqoa.actions.equip(h_pot) end");
    host.execute("assert(results[#results].ok == false and results[#results].code == 'invalid_argument')").unwrap();

    // quick_slot potion on slot 1 ok, on slot 3 invalid
    let before = p.session.sends_lost;
    run_lua(&mut p, "next_fn = function() return eqoa.actions.quick_slot(h_pot, 1) end");
    assert!(p.session.sends_lost > before);
    host.execute("assert(results[#results].ok == true)").unwrap();
    run_lua(&mut p, "next_fn = function() return eqoa.actions.quick_slot(h_pot, 3) end");
    host.execute("assert(results[#results].ok == false and results[#results].code == 'invalid_argument')").unwrap();

    // swap_items on two unequipped stacks ok
    let before = p.session.sends_lost;
    run_lua(&mut p, "next_fn = function() return eqoa.actions.swap_items(h_pot, h_gem) end");
    assert!(p.session.sends_lost > before);
    host.execute("assert(results[#results].ok == true)").unwrap();

    // Inventory busy (p.waiting = true) -> busy
    p.waiting = true;
    super::super::begin_iteration(&mut p, false);
    run_lua(&mut p, "next_fn = function() return eqoa.actions.use_item(h_pot) end");
    host.execute("assert(last_req == nil and last_err.code == 'busy')").unwrap();
    p.waiting = false;
    super::super::begin_iteration(&mut p, false);

    // Stale handle after inventory loses the gem
    p.dump.inventory.items.pop();
    p.forge_snapshot(None);
    run_lua(&mut p, "next_fn = function() return eqoa.actions.use_item(h_gem) end");
    host.execute("assert(last_req == nil and last_err.code == 'stale_handle')").unwrap();

    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round6b_merchant_buy_sell_bank_move_bank_money_and_loot() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();

    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.target = 777;
    p.dump.player.p34 = 500; // 500 tunar carried
    p.dump.player.p38 = 1_000; // 1000 tunar banked

    let mut worn_sword = sword();
    worn_sword.equip_slot = 0;
    let mut bag_pot = potion();
    bag_pot.equip_slot = -1;
    let mut bank_gem = gem();
    bank_gem.equip_slot = -1;

    p.dump.inventory = list(&[worn_sword, bag_pot.clone()]);
    p.dump.bank.items = list(&[bank_gem]);

    // Open merchant (mode 3) with wares
    let mut ware_pot = potion();
    ware_pot.key = 99;
    ware_pot.value = 50;
    p.npcw.shop = crate::merchant::Shop {
        id: Some(888),
        sell_rate: 100,
        buy_rate: 100,
        wares: list(&[ware_pot]),
        greeting: Vec::new(),
        pending: false,
    };
    p.npcw.merchant.visible = true;
    p.mode = 10;
    p.forge_snapshot(None);

    let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
    host.execute(
        r#"
        results = {}
        eqoa.events.on("action.result", function(e)
            results[#results + 1] = e.data
        end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "k" }, function()
            last_req, last_err = next_fn()
        end)
        local inv = assert(eqoa.inventory.get())
        h_worn = inv.items[1].ref
        h_pot = inv.items[2].ref
        local merch = assert(eqoa.merchant.get())
        h_ware = merch.items[1].ref
        local bnk = assert(eqoa.bank.get())
        h_bank_gem = bnk.items[1].ref
    "#,
    )
    .unwrap();

    let run_lua = |p: &mut Play, code: &str| {
        host.execute(code).unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        native_events::dispatch(&host).unwrap();
        super::super::begin_iteration(p, true);
        let (vui, mut r) = (
            crate::pointer::testing::vui(),
            crate::pointer::testing::NullRaster,
        );
        p.run_npcw(&vui, &mut r);
        p.run_bank(&vui, &mut r);
        native_events::dispatch(&host).unwrap();
    };

    let apply_broker = |p: &mut Play, event: Event| {
        let capture = super::super::begin_game_event(p, &event);
        match &event {
            Event::InvAdd(item) => {
                let limit = item.item.max_stack;
                crate::inventory::add_item(&mut p.dump.inventory, item, limit, crate::inventory::pack_cap());
                p.npcw_item_added();
            }
            Event::InvTook { slot, sound, n } => {
                crate::inventory::took(&mut p.dump.inventory, *slot as usize, *n as i64);
                p.npcw_item_took(*sound);
            }
            Event::Opcode { op, data } => {
                assert!(
                    p.bank_opcode(*op, data) || p.npcw_opcode(*op, data),
                    "opcode {op:#06x} is one of the item windows'"
                );
            }
            Event::MasterLoot(_) => {}
            other => panic!("unexpected event in apply_broker: {other:?}"),
        }
        super::super::end_game_event(p, capture);
    };

    // Equipped item cannot be sold
    run_lua(&mut p, "next_fn = function() return eqoa.actions.sell(h_worn, 1) end");
    host.execute("assert(results[#results].ok == false and results[#results].code == 'not_ready')").unwrap();

    // Sell 1 potion -> dispatched & merchant busy on 2nd call -> server reply InvTook resolves completed
    run_lua(&mut p, "next_fn = function() return eqoa.actions.sell(h_pot, 1) end");
    assert!(p.npcw.shop.pending);
    host.execute("assert(results[#results].phase == 'dispatch' and results[#results].state == 'dispatched')").unwrap();
    run_lua(&mut p, "next_fn = function() return eqoa.actions.buy(h_ware, 1) end");
    host.execute("assert(last_req == nil and last_err.code == 'busy')").unwrap();

    apply_broker(&mut p, Event::InvTook { slot: 1, sound: 3, n: 1 });
    native_events::dispatch(&host).unwrap();
    assert!(!p.npcw.shop.pending);
    host.execute("assert(results[#results].phase == 'server' and results[#results].state == 'completed' and results[#results].ok == true)").unwrap();

    // Buy 2 pots -> server reply InvAdd resolves completed
    run_lua(&mut p, "next_fn = function() return eqoa.actions.buy(h_ware, 2) end");
    assert!(p.npcw.shop.pending);
    apply_broker(&mut p, Event::InvAdd(Box::new(potion().net())));
    native_events::dispatch(&host).unwrap();
    assert!(!p.npcw.shop.pending);
    host.execute("assert(results[#results].phase == 'server' and results[#results].state == 'completed')").unwrap();

    // Insufficient tunar check
    run_lua(&mut p, "next_fn = function() return eqoa.actions.buy(h_ware, 20) end");
    host.execute("assert(results[#results].ok == false and results[#results].reason == 'not enough tunar')").unwrap();

    // Bank mode 11: move & bank_money
    p.npcw.merchant.visible = false;
    p.npcw.shop.id = None;
    p.mode = 11;
    p.bank.state = 0;
    p.forge_snapshot(None);
    host.execute("local inv = eqoa.inventory.get(); h_pot = inv.items[2].ref").unwrap();

    run_lua(&mut p, "next_fn = function() return eqoa.actions.bank_move(h_pot, 1) end");
    assert_eq!(p.bank.state, 2);
    // Busy lock on 2nd bank call
    run_lua(&mut p, "next_fn = function() return eqoa.actions.bank_money('deposit', 50) end");
    host.execute("assert(last_req == nil and last_err.code == 'busy')").unwrap();
    let mut pot_one = potion();
    pot_one.quantity = 1;
    apply_broker(&mut p, opcode(0x1251, pot_one.bytes()));
    native_events::dispatch(&host).unwrap();
    assert_eq!(p.bank.state, 0);
    host.execute("assert(results[#results].phase == 'server' and results[#results].state == 'completed')").unwrap();

    // Withdraw money
    run_lua(&mut p, "next_fn = function() return eqoa.actions.bank_money('withdraw', 250) end");
    assert_eq!(p.bank.state, 1);
    let mut mbytes = Writer::default();
    mbytes.signed(750);
    apply_broker(&mut p, opcode(0x1253, mbytes.data));
    native_events::dispatch(&host).unwrap();
    assert_eq!(p.bank.state, 0);
    host.execute("assert(results[#results].phase == 'server' and results[#results].state == 'completed')").unwrap();

    // Loot mode 1 + MasterLoot guard + 0x00D7 cancel
    let mut loot_gem = gem();
    loot_gem.key = 55;
    loot_gem.quantity = 1;
    p.npcw.corpse = Some(list(&[loot_gem]));
    p.mode = 1;
    p.forge_snapshot(None);
    host.execute("local l = assert(eqoa.loot.get()); h_loot = l.items[1].ref").unwrap();

    // Master loot blocks loot
    let ml = net::raid::MasterLoot {
        seconds: 60,
        members: vec![],
    };
    apply_broker(&mut p, Event::MasterLoot(Some(ml)));
    run_lua(&mut p, "next_fn = function() return eqoa.actions.loot(h_loot, 1) end");
    host.execute("assert(results[#results].ok == false and results[#results].code == 'permission_denied')").unwrap();

    // Clear master loot -> loot_all -> 0x00D7 refuses cleanly
    apply_broker(&mut p, Event::MasterLoot(None));
    run_lua(&mut p, "next_fn = function() return eqoa.actions.loot_all() end");
    apply_broker(&mut p, opcode(0x00D7, vec![]));
    native_events::dispatch(&host).unwrap();
    host.execute("assert(results[#results].phase == 'server' and results[#results].state == 'refused')").unwrap();

    // loot(h_loot, 1) -> 0x0019 server reply -> completed
    run_lua(&mut p, "next_fn = function() return eqoa.actions.loot(h_loot, 1) end");
    let mut loot_bytes = Writer::default();
    loot_bytes.u8(0).signed(0);
    apply_broker(&mut p, opcode(0x0019, loot_bytes.data));
    native_events::dispatch(&host).unwrap();
    host.execute("assert(results[#results].phase == 'server' and results[#results].state == 'completed' and results[#results].ok == true)").unwrap();

    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round6b_transactions_route_through_native_consumers_and_camera_hook() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();

    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.dump.player.p34 = 500;
    p.dump.player.p38 = 500;
    let mut bag_pot = potion();
    bag_pot.equip_slot = -1;
    let mut bank_gem = gem();
    bank_gem.equip_slot = -1;
    p.dump.inventory = list(&[bag_pot]);
    p.dump.bank.items = list(&[bank_gem]);
    p.forge_snapshot(None);

    let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
    host.execute(
        r#"
        results = {}
        eqoa.events.on("action.result", function(e)
            results[#results + 1] = e.data
        end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "k" }, function()
            last_req, last_err = next_fn()
        end)
        local inv = assert(eqoa.inventory.get())
        h_pot = inv.items[1].ref
        local bnk = assert(eqoa.bank.get())
        h_gem = bnk.items[1].ref
    "#,
    )
    .unwrap();

    let (vui, mut r) = (
        crate::pointer::testing::vui(),
        crate::pointer::testing::NullRaster,
    );
    let key_tick = |p: &mut Play, code: &str| {
        host.execute(code).unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        native_events::dispatch(&host).unwrap();
        super::super::begin_iteration(p, true);
        native_events::dispatch(&host).unwrap();
    };

    // Window closed: merchant/bank/corpse actions refuse with window_closed and native loops do not run/open anything.
    p.mode = 0;
    let before = p.session.sends_lost;
    key_tick(&mut p, "next_fn = function() return eqoa.actions.sell(h_pot, 1) end");
    host.execute("assert(results[#results].ok == false and results[#results].code == 'window_closed')").unwrap();
    key_tick(&mut p, "next_fn = function() return eqoa.actions.bank_money('deposit', 10) end");
    host.execute("assert(results[#results].ok == false and results[#results].code == 'window_closed')").unwrap();
    p.run_npcw(&vui, &mut r);
    p.run_bank(&vui, &mut r);
    assert_eq!(p.session.sends_lost, before);
    assert_eq!(p.mode, 0);
    assert!(!p.npcw.shop.pending);
    assert_eq!(p.bank.state, 0);

    // Merchant open: begin_iteration stages without packet or shop.pending; run_npcw native loop sends 0x004B & sets pending.
    p.npcw.shop = crate::merchant::Shop {
        id: Some(77),
        sell_rate: 100,
        buy_rate: 100,
        wares: Default::default(),
        greeting: Vec::new(),
        pending: false,
    };
    p.npcw.merchant.visible = true;
    p.mode = 10;
    let before = p.session.sends_lost;
    key_tick(&mut p, "next_fn = function() return eqoa.actions.sell(h_pot, 1) end");
    assert_eq!(p.session.sends_lost, before, "begin_iteration alone does not send 0x004B");
    assert!(!p.npcw.shop.pending, "begin_iteration alone does not write shop.pending");
    p.run_npcw(&vui, &mut r);
    assert!(p.session.sends_lost > before, "npcw.rs loop sent the sell packet");
    assert!(p.npcw.shop.pending, "npcw.rs loop set shop.pending");

    // Bank open: begin_iteration stages without packet or bank.state; run_bank native loop sends 0x1254 & sets state=2.
    p.mode = 11;
    p.bank.state = 0;
    let before_bank = p.session.sends_lost;
    key_tick(&mut p, "next_fn = function() return eqoa.actions.bank_move(h_gem, 1) end");
    assert_eq!(p.session.sends_lost, before_bank, "begin_iteration alone does not send 0x1254");
    assert_eq!(p.bank.state, 0, "begin_iteration alone does not write bank.state");
    p.run_bank(&vui, &mut r);
    assert!(p.session.sends_lost > before_bank, "bankplay.rs loop sent bank move");
    assert_eq!(p.bank.state, 2, "bankplay.rs loop set bank.state");

    // Camera action through apply_camera(play, walker) with native clamps.
    p.mode = 0;
    let rec = |kind: u16, children: u32, body: Vec<u8>| {
        let mut out = Vec::new();
        out.extend(kind.to_le_bytes());
        out.extend(1i16.to_le_bytes());
        out.extend((body.len() as i32).to_le_bytes());
        out.extend(children.to_le_bytes());
        out.extend(body);
        out
    };
    let root = rec(0x8000, 1, rec(0x8200, 1, rec(0x8230, 0, vec![0; 8])));
    let mut lib = esf::Library::new();
    let file = lib.add("synthetic-cam-world", esf::Container::parse_records(root));
    let mut world = vi::world::World::load(&lib, file).unwrap();
    world.leaves.push(0);
    world.zones.push(Default::default());
    let mut walker = crate::walker::Walker::arrive(
        world,
        crate::walker::Spawn {
            pos: [0.; 3],
            heading: 0.,
            placing: crate::walker::Placing::AsGiven,
        },
    );
    key_tick(
        &mut p,
        "next_fn = function() return eqoa.actions.camera({ mode = 3, heading = 1.25, pitch = 9.0, distance = 999.0 }) end",
    );
    super::super::apply_camera(&mut p, &mut walker);
    native_events::dispatch(&host).unwrap();
    host.execute("assert(results[#results].ok == true and results[#results].kind == 'camera')").unwrap();
    assert_eq!(walker.cam.camera_mode, 3);
    assert!((walker.cam.cur.orbit.y.0 - walker.cam.walk.max_pitch.0).abs() < 1e-4);
    assert!((walker.cam.cur.dist.0 - walker.cam.max_orbit_distance.0).abs() < 1e-4);

    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round6c_no_false_confirmations_unrelated_invadd_or_corpse_close() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();

    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.dump.player.p34 = 500;
    let mut bag1 = potion();
    bag1.key = 101;
    bag1.equip_slot = -1;
    let mut bag2 = gem();
    bag2.key = 102;
    bag2.equip_slot = -1;
    p.dump.inventory = list(&[bag1, bag2]);

    let mut ware = potion();
    ware.key = 99;
    ware.value = 10;
    p.npcw.shop = crate::merchant::Shop {
        id: Some(88),
        sell_rate: 100,
        buy_rate: 100,
        wares: list(&[ware]),
        greeting: Vec::new(),
        pending: false,
    };
    p.npcw.merchant.visible = true;
    p.mode = 10;
    p.forge_snapshot(None);

    let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
    host.execute(
        r#"
        ares, bprog, bres = {}, {}, {}
        eqoa.events.on("action.result", function(e) ares[#ares + 1] = e.data end)
        eqoa.events.on("batch.progress", function(e) bprog[#bprog + 1] = e.data end)
        eqoa.events.on("batch.result", function(e) bres[#bres + 1] = e.data end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "k" }, function()
            last_batch, last_err = eqoa.actions.batch(spec_fn())
        end)
        local inv = assert(eqoa.inventory.get())
        h_s1, h_s2 = inv.items[1].ref, inv.items[2].ref
        h_ware = assert(eqoa.merchant.get()).items[1].ref
    "#,
    )
    .unwrap();

    let (vui, mut r) = (
        crate::pointer::testing::vui(),
        crate::pointer::testing::NullRaster,
    );
    let mut tick = |p: &mut Play| {
        super::super::begin_iteration(p, true);
        p.run_npcw(&vui, &mut r);
        p.run_bank(&vui, &mut r);
        native_events::dispatch(&host).unwrap();
    };
    let apply_broker = |p: &mut Play, event: Event| {
        let capture = super::super::begin_game_event(p, &event);
        match &event {
            Event::InvAdd(item) => {
                let limit = item.item.max_stack;
                crate::inventory::add_item(&mut p.dump.inventory, item, limit, crate::inventory::pack_cap());
                p.npcw_item_added();
            }
            Event::InvTook { slot, sound, n } => {
                crate::inventory::took(&mut p.dump.inventory, *slot as usize, *n as i64);
                p.npcw_item_took(*sound);
            }
            Event::Opcode { op, data } => {
                assert!(
                    p.bank_opcode(*op, data) || p.npcw_opcode(*op, data),
                    "opcode {op:#06x}"
                );
            }
            _ => panic!("unexpected event"),
        }
        super::super::end_game_event(p, capture);
    };
    let press_k = |spec_code: &str| {
        host.execute(spec_code).unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        native_events::dispatch(&host).unwrap();
    };

    // 1. Sell batch: unrelated InvAdd must NOT confirm or advance.
    press_k("spec_fn = function() return { steps = { { kind = 'sell', item = h_s1, count = 1 }, { kind = 'sell', item = h_s2, count = 1 } }, require_window = 'merchant' } end");
    tick(&mut p);
    assert!(p.npcw.shop.pending);
    tick(&mut p); // sets saw_pending = true
    let unrelated = Spec::from_json(&serde_json::json!({"name":"Other","id":77777,"key":77,"qty":1}));
    apply_broker(&mut p, Event::InvAdd(Box::new(unrelated.net())));
    native_events::dispatch(&host).unwrap();
    host.execute(
        "assert(ares[#ares].phase == 'dispatch' and ares[#ares].kind == 'sell'); assert(#bres == 0)",
    )
    .unwrap();
    // On next tick, shop.pending cleared without confirmation -> batch stops as unknown ("merchant did not confirm")!
    tick(&mut p);
    host.execute(
        "assert(#bres == 1 and bres[1].state == 'unknown' and bres[1].ok == nil and bres[1].completed == 0 and bres[1].reason == 'merchant did not confirm')",
    )
    .unwrap();

    // 2. Buy batch: unrelated InvAdd (still pending) neither confirms nor advances; real InvAdd(potion) confirms!
    p.npcw.shop.pending = false;
    press_k("spec_fn = function() return { steps = { { kind = 'buy', item = h_ware, count = 1 }, { kind = 'buy', item = h_ware, count = 1 } }, require_window = 'merchant' } end");
    tick(&mut p);
    assert!(p.npcw.shop.pending);
    tick(&mut p);
    // Unrelated InvAdd (wrong item id) while merchant pending stays true -> does not confirm or advance.
    let cap_u = super::super::begin_game_event(&p, &Event::InvAdd(Box::new(unrelated.net())));
    crate::inventory::add_item(&mut p.dump.inventory, &unrelated.net(), 1, 40);
    super::super::end_game_event(&p, cap_u);
    native_events::dispatch(&host).unwrap();
    assert!(p.npcw.shop.pending);
    host.execute("assert(ares[#ares].phase == 'dispatch' and #bres == 1)").unwrap();
    // Real ware InvAdd clears shop.pending & confirms step 1!
    apply_broker(&mut p, Event::InvAdd(Box::new(potion().net())));
    native_events::dispatch(&host).unwrap();
    host.execute("assert(ares[#ares].phase == 'server' and ares[#ares].state == 'completed' and ares[#ares].ok == true)").unwrap();
    // Cleanly close merchant before step 2 -> resets nicely for corpse test.
    p.npcw.merchant.visible = false;
    p.npcw.shop.id = None;
    p.mode = 0;
    tick(&mut p);

    // 3. Corpse loot batch: wrong 0x0019 slot/key does NOT confirm step 1; 0x0016 LootClose -> unknown & batch stops!
    let mut c1 = gem();
    c1.key = 31;
    let mut c2 = sword();
    c2.key = 32;
    p.npcw.corpse = Some(list(&[c1, c2]));
    p.mode = 1;
    p.forge_snapshot(None);
    host.execute(
        "local l = assert(eqoa.loot.get()); h_c1, h_c2 = l.items[1].ref, l.items[2].ref",
    )
    .unwrap();
    press_k("spec_fn = function() return { steps = { { kind = 'loot', item = h_c1, count = 1 }, { kind = 'loot', item = h_c2, count = 1 } }, require_window = 'loot' } end");
    let sends_before_loot = p.session.sends_lost;
    tick(&mut p); // dispatches step 1 (key 31)
    assert!(p.session.sends_lost > sends_before_loot);
    // Inject 0x0019 for slot 1 (key 32) instead of key 31 -> must NOT confirm step 1!
    apply_broker(&mut p, opcode(0x0019, vec![1, 2]));
    native_events::dispatch(&host).unwrap();
    host.execute("assert(ares[#ares].phase == 'dispatch' and #bres == 2)").unwrap();
    // Corpse closes (0x0016) while step 1 is pending -> step 1 resolves unknown and batch stops!
    let sends_before_close = p.session.sends_lost;
    apply_broker(&mut p, opcode(0x0016, vec![]));
    p.npcw.corpse = None;
    p.mode = 0;
    native_events::dispatch(&host).unwrap();
    tick(&mut p);
    assert_eq!(p.session.sends_lost, sends_before_close, "step 2 must never send after close");
    host.execute(
        "assert(ares[#ares].state == 'unknown' and ares[#ares].ok == nil); assert(bres[#bres].state == 'unknown' and bres[#bres].ok == nil and bres[#bres].completed == 0)",
    )
    .unwrap();

    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round6c_contention_retains_staged_deposit_and_never_confirms_on_player_reply() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();

    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.dump.player.p34 = 500;
    p.dump.player.p38 = 500;
    let mut bag_pot = potion();
    bag_pot.equip_slot = -1;
    p.dump.inventory = list(&[bag_pot]);
    p.dump.bank.items = list(&[]);
    p.mode = 11;
    p.bank.state = 0;
    p.forge_snapshot(None);

    let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
    host.execute(
        r#"
        ares, bres = {}, {}
        eqoa.events.on("action.result", function(e) ares[#ares + 1] = e.data end)
        eqoa.events.on("batch.result", function(e) bres[#bres + 1] = e.data end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "k" }, function()
            last_req, last_err = next_fn()
        end)
        local inv = assert(eqoa.inventory.get())
        h_pot = inv.items[1].ref
    "#,
    )
    .unwrap();

    let press_k = |code: &str| {
        host.execute(code).unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        native_events::dispatch(&host).unwrap();
    };
    let apply_broker = |p: &mut Play, event: Event| {
        let capture = super::super::begin_game_event(p, &event);
        if let Event::Opcode { op, data } = &event {
            assert!(p.bank_opcode(*op, data), "opcode {op:#06x}");
        }
        super::super::end_game_event(p, capture);
    };

    // 1. Broker stages bank_move(deposit 1 potion), while native already queued a player BankOut::Move on same frame.
    // The staged broker request must NOT be dropped and must NOT confirm when player's 0x1251 arrives.
    press_k("next_fn = function() return eqoa.actions.batch({ steps = { { kind = 'bank_move', item = h_pot, count = 1 }, { kind = 'bank_money', direction = 'deposit', amount = 10 } }, require_window = 'bank' }) end");
    host.execute("assert(last_req ~= nil, last_err and last_err.reason or 'batch nil')").unwrap();
    super::super::begin_iteration(&mut p, true);
    let mut player_out = vec![crate::bank::BankOut::Move(0, 0, 1)];
    super::super::bank_requests(&p, &mut player_out);
    assert_eq!(player_out.len(), 1, "contended native output must block broker push without overwriting player output");
    native_events::dispatch(&host).unwrap();
    host.execute("assert(#ares == 0 and #bres == 0)").unwrap();

    let mut pot_one = potion();
    pot_one.quantity = 1;
    apply_broker(&mut p, opcode(0x1251, pot_one.bytes()));
    native_events::dispatch(&host).unwrap();
    host.execute("assert(#ares == 0 and #bres == 0, 'player reply must not confirm or resolve unsent staged broker deposit')").unwrap();

    // 2. Next idle tick: retained staged request retries and now sends via bank_requests!
    let sends_before_retry = p.session.sends_lost;
    p.bags_broker_tick();
    native_events::dispatch(&host).unwrap();
    assert!(p.session.sends_lost > sends_before_retry, "retained staged deposit must send once contention clears");
    assert_eq!(p.bank.state, 2);
    host.execute("assert(#ares == 1 and ares[1].state == 'dispatched' and #bres == 0)").unwrap();

    // 3. Mismatched deposit quantity (0x1251 quantity=2 for count=1) resolves unknown and stops batch!
    let mut pot_two = potion();
    pot_two.quantity = 2;
    apply_broker(&mut p, opcode(0x1251, pot_two.bytes()));
    native_events::dispatch(&host).unwrap();
    host.execute(
        "assert(#ares == 2 and ares[2].state == 'unknown' and ares[2].ok == nil); assert(#bres == 1 and bres[1].state == 'unknown' and bres[1].completed == 0)",
    )
    .unwrap();

    // 4. Already-sent deposit + same-frame player native BankOut contention -> reply resolves `unknown`, never `completed`.
    p.bank.state = 0;
    p.dump.bank.items = list(&[]);
    p.forge_snapshot(None);
    host.execute("h_pot = assert(eqoa.inventory.get()).items[1].ref").unwrap();
    press_k("next_fn = function() return eqoa.actions.bank_move(h_pot, 1) end");
    p.bags_broker_tick();
    native_events::dispatch(&host).unwrap();
    assert_eq!(p.bank.state, 2);
    super::super::begin_iteration(&mut p, true);
    let mut concurrent_out = vec![crate::bank::BankOut::Move(0, 0, 1)];
    super::super::bank_requests(&p, &mut concurrent_out);
    apply_broker(&mut p, opcode(0x1251, pot_one.bytes()));
    native_events::dispatch(&host).unwrap();
    host.execute("assert(ares[#ares].state == 'unknown' and ares[#ares].ok == nil)").unwrap();

    // 5. Continuous contention past timeout refuses staged request cleanly with `busy`.
    p.bank.state = 0;
    press_k("next_fn = function() return eqoa.actions.bank_move(h_pot, 1) end");
    for _ in 0..=action_broker::RATE_LIMIT_WAIT_TICKS {
        super::super::begin_iteration(&mut p, true);
        let mut busy_out = vec![crate::bank::BankOut::Move(0, 0, 1)];
        super::super::bank_requests(&p, &mut busy_out);
    }
    native_events::dispatch(&host).unwrap();
    host.execute("assert(ares[#ares].ok == false and ares[#ares].code == 'busy' and ares[#ares].state == 'refused')").unwrap();

    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round6c_loot_partial_decrease_and_noop_loot_all_yield_unknown_and_stop_batch() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();

    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    // 39 unique unstackable rings + 1 partial stack of Moonstone (room for LootOne, then full for LootAll).
    let mut filler: Vec<NetItem> = (0..crate::inventory::pack_cap().saturating_sub(1))
        .map(|i| {
            let mut r = ring();
            r.id = 910_000 + i as i64;
            r.key = 200 + i as u32;
            r.equip_slot = -1;
            r.net()
        })
        .collect();
    let mut inv_gem = gem();
    inv_gem.key = 299;
    inv_gem.quantity = 1;
    filler.push(inv_gem.net());
    p.dump.inventory = ItemList {
        items: filler,
        ..Default::default()
    };

    let mut stack_gem = gem();
    stack_gem.key = 31;
    stack_gem.quantity = 4;
    let mut c2 = sword();
    c2.key = 32;
    c2.equip_slot = -1;
    p.npcw.corpse = Some(list(&[stack_gem, c2]));
    p.mode = 1;
    p.forge_snapshot(None);

    let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
    host.execute(
        r#"
        ares, bres = {}, {}
        eqoa.events.on("action.result", function(e) ares[#ares + 1] = e.data end)
        eqoa.events.on("batch.result", function(e) bres[#bres + 1] = e.data end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "k" }, function()
            last_batch, last_err = eqoa.actions.batch(spec_fn())
        end)
        local l = assert(eqoa.loot.get())
        h_gem, h_sword = l.items[1].ref, l.items[2].ref
    "#,
    )
    .unwrap();

    let press_k = |spec_code: &str| {
        host.execute(spec_code).unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        native_events::dispatch(&host).unwrap();
    };

    // 1. LootOne requested count = 3 from stack of 4, server only reduces stack by 1 (partial) -> unknown & batch stops!
    press_k("spec_fn = function() return { steps = { { kind = 'loot', item = h_gem, count = 3 }, { kind = 'loot', item = h_sword, count = 1 } }, require_window = 'loot' } end");
    p.bags_broker_tick();
    native_events::dispatch(&host).unwrap();
    let sends_after_step1 = p.session.sends_lost;
    let mut w = Writer::default();
    w.u8(0).signed(0);
    let ev_19 = opcode(0x0019, w.data);
    let cap_19 = super::super::begin_game_event(&p, &ev_19);
    // Simulate partial decrease of stack 31 from 4 to 3 instead of expected 1 (count=3).
    p.npcw.corpse.as_mut().unwrap().items[0].quantity = 3;
    super::super::end_game_event(&p, cap_19);
    native_events::dispatch(&host).unwrap();
    p.bags_broker_tick();
    assert_eq!(p.session.sends_lost, sends_after_step1, "step 2 must not send after partial loot unknown");
    host.execute(
        "assert(ares[#ares].state == 'unknown' and ares[#ares].ok == nil); assert(#bres == 1 and bres[1].state == 'unknown' and bres[1].ok == nil and bres[1].completed == 0)",
    )
    .unwrap();

    // 2. LootAll receives 0x001A without any corpse state change (full pack & full gem stack) -> unknown & batch stops!
    p.dump.inventory.items.last_mut().unwrap().quantity = 100;
    press_k("spec_fn = function() return { steps = { { kind = 'loot_all' }, { kind = 'loot', item = h_sword, count = 1 } }, require_window = 'loot' } end");
    p.bags_broker_tick();
    native_events::dispatch(&host).unwrap();
    let sends_after_loot_all = p.session.sends_lost;
    let ev_1a = opcode(0x001A, vec![]);
    let cap_1a = super::super::begin_game_event(&p, &ev_1a);
    assert!(p.npcw_opcode(0x001A, &[]));
    super::super::end_game_event(&p, cap_1a);
    native_events::dispatch(&host).unwrap();
    p.bags_broker_tick();
    assert_eq!(p.session.sends_lost, sends_after_loot_all, "step 2 must not send after noop loot_all unknown");
    host.execute(
        "assert(ares[#ares].kind == 'loot_all' and ares[#ares].state == 'unknown' and ares[#ares].ok == nil); assert(#bres == 2 and bres[2].state == 'unknown' and bres[2].ok == nil and bres[2].completed == 0)",
    )
    .unwrap();

    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

// Round 6 fix 3: the player's own native output while a broker request is in flight makes the
// result `unknown` whatever the frame order (`begin_iteration` runs between the draw and the reply).
fn r6f3_host() -> Host {
    let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
    host.execute(
        r#"
        ares, bres = {}, {}
        eqoa.events.on("action.result", function(e) ares[#ares + 1] = e.data end)
        eqoa.events.on("batch.result", function(e) bres[#bres + 1] = e.data end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "k" }, function()
            last_req, last_err = next_fn()
        end)
    "#,
    )
    .unwrap();
    host
}

fn r6f3_press(host: &Host, code: &str) {
    use crate::fork_ext::addons::native_events;
    host.execute(code).unwrap();
    CORE.with(|c| {
        c.borrow_mut().emit(
            "input.key",
            serde_json::json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
        )
    });
    native_events::dispatch(host).unwrap();
}

#[test]
fn round6f3_player_loot_while_broker_loot_in_flight_is_unknown_and_stops_batch() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.dump.inventory = list(&[]);
    let mut stack = gem();
    stack.key = 31;
    stack.quantity = 4;
    let mut c2 = sword();
    c2.key = 32;
    c2.equip_slot = -1;
    p.npcw.corpse = Some(list(&[stack, c2]));
    p.mode = 1;
    p.forge_snapshot(None);
    let host = r6f3_host();
    host.execute("local l = assert(eqoa.loot.get()); h_gem, h_sword = l.items[1].ref, l.items[2].ref").unwrap();
    r6f3_press(&host, "next_fn = function() return eqoa.actions.batch({ steps = { { kind = 'loot', item = h_gem, count = 1 }, { kind = 'loot', item = h_sword, count = 1 } }, require_window = 'loot' }) end");
    // frame 1: step, then the draw sends the broker's loot
    p.bags_broker_tick();
    native_events::dispatch(&host).unwrap();
    let sent = p.session.sends_lost;
    // frame 2: the player's own loot reaches the native output vector in the draw
    super::super::begin_iteration(&mut p, true);
    let mut player = vec![crate::loot::LootOut::One(31, 1)];
    super::super::npcw_requests(&p, &mut player, &mut Vec::new());
    // frame 3: the next game step runs first (the old marker was cleared here), then the reply
    super::super::begin_iteration(&mut p, true);
    let mut w = Writer::default();
    w.u8(0).signed(0);
    let ev = opcode(0x0019, w.data);
    let cap = super::super::begin_game_event(&p, &ev);
    p.npcw.corpse.as_mut().unwrap().items[0].quantity = 3;
    super::super::end_game_event(&p, cap);
    native_events::dispatch(&host).unwrap();
    p.bags_broker_tick();
    assert_eq!(p.session.sends_lost, sent, "the batch must stop, no second loot");
    host.execute(
        "assert(ares[#ares].kind == 'loot' and ares[#ares].state == 'unknown' and ares[#ares].ok == nil, ares[#ares].state); assert(#bres == 1 and bres[1].state == 'unknown' and bres[1].completed == 0)",
    )
    .unwrap();
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round6f3_player_bank_deposit_while_broker_deposit_in_flight_is_unknown() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.dump.player.p34 = 500;
    p.dump.player.p38 = 500;
    let mut bag_pot = potion();
    bag_pot.equip_slot = -1;
    p.dump.inventory = list(&[bag_pot]);
    p.dump.bank.items = list(&[]);
    p.mode = 11;
    p.bank.state = 0;
    p.forge_snapshot(None);
    let host = r6f3_host();
    host.execute("h_pot = assert(eqoa.inventory.get()).items[1].ref").unwrap();
    r6f3_press(&host, "next_fn = function() return eqoa.actions.bank_move(h_pot, 1) end");
    p.bags_broker_tick();
    native_events::dispatch(&host).unwrap();
    assert_eq!(p.bank.state, 2, "broker deposit sent");
    // the player's own deposit appears in the native output during a later draw
    super::super::begin_iteration(&mut p, true);
    let mut player = vec![crate::bank::BankOut::Move(0, 0, 1)];
    super::super::bank_requests(&p, &mut player);
    // next step, then the reply that matches the broker's item and count
    super::super::begin_iteration(&mut p, true);
    let mut one = potion();
    one.quantity = 1;
    let ev = opcode(0x1251, one.bytes());
    let cap = super::super::begin_game_event(&p, &ev);
    assert!(p.bank_opcode(0x1251, &one.bytes()));
    super::super::end_game_event(&p, cap);
    native_events::dispatch(&host).unwrap();
    host.execute("assert(ares[#ares].kind == 'bank_move' and ares[#ares].state == 'unknown' and ares[#ares].ok == nil, ares[#ares].state)").unwrap();
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round6f3_player_sale_while_broker_sale_in_flight_is_unknown_and_stops_batch() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    let mut a = potion();
    a.key = 101;
    a.equip_slot = -1;
    let mut b = gem();
    b.key = 102;
    b.equip_slot = -1;
    p.dump.inventory = list(&[a, b]);
    p.bags_open_merchant(88);
    p.forge_snapshot(None);
    let host = r6f3_host();
    host.execute("local inv = assert(eqoa.inventory.get()); h_a, h_b = inv.items[1].ref, inv.items[2].ref").unwrap();
    r6f3_press(&host, "next_fn = function() return eqoa.actions.batch({ steps = { { kind = 'sell', item = h_a, count = 1 }, { kind = 'sell', item = h_b, count = 1 } }, require_window = 'merchant' }) end");
    p.bags_broker_tick();
    native_events::dispatch(&host).unwrap();
    assert!(p.npcw.shop.pending, "broker sale sent");
    let sent = p.session.sends_lost;
    super::super::begin_iteration(&mut p, true);
    let mut player = vec![crate::merchant::MerchantOut::Sell(0, 1)];
    super::super::npcw_requests(&p, &mut Vec::new(), &mut player);
    super::super::begin_iteration(&mut p, true);
    p.bags_sell_confirm(0, 1);
    native_events::dispatch(&host).unwrap();
    p.bags_broker_tick();
    assert_eq!(p.session.sends_lost, sent, "the batch must stop, no second sale");
    host.execute("assert(ares[#ares].kind == 'sell' and ares[#ares].state == 'unknown' and ares[#ares].ok == nil, ares[#ares].state); assert(#bres == 1 and bres[1].state == 'unknown' and bres[1].completed == 0)").unwrap();
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round6f3_staged_sale_refused_when_player_sale_shrinks_the_stack() {
    use crate::fork_ext::addons::{action_broker, native_events};
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    let mut a = potion();
    a.key = 101;
    a.equip_slot = -1;
    a.quantity = 3;
    p.dump.inventory = list(&[a]);
    p.bags_open_merchant(88);
    p.forge_snapshot(None);
    let host = r6f3_host();
    host.execute("h_a = assert(eqoa.inventory.get()).items[1].ref").unwrap();
    r6f3_press(&host, "next_fn = function() return eqoa.actions.sell(h_a, 2) end");
    // the step stages the sale; the player's own sale occupies the merchant output that frame
    super::super::begin_iteration(&mut p, true);
    let sent = p.session.sends_lost;
    let mut player = vec![crate::merchant::MerchantOut::Sell(0, 1)];
    super::super::npcw_requests(&p, &mut Vec::new(), &mut player);
    assert_eq!(player.len(), 1, "broker did not append beside the player's sale");
    // that sale shrinks the stack below the staged count
    p.dump.inventory.items[0].quantity = 1;
    p.bags_broker_tick();
    native_events::dispatch(&host).unwrap();
    assert_eq!(p.session.sends_lost, sent, "a sale the native loop would drop must never be sent");
    assert!(!p.npcw.shop.pending);
    host.execute("assert(ares[#ares].kind == 'sell' and ares[#ares].state == 'refused' and ares[#ares].code == 'stale_handle', ares[#ares].state)").unwrap();
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

struct CountingRaster(usize);
impl crate::ui::Layout for CountingRaster {}
impl vi::ui::Raster for CountingRaster {
    fn blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) { self.0 += 1; }
    fn stretch_blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) { self.0 += 1; }
    fn fill_fan(&mut self, _: [f32; 4], _: [[i32; 2]; 4]) { self.0 += 1; }
    fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) { self.0 += 1; }
    fn draw_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16]) { self.0 += 1; }
    fn draw_centered_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16], _: u32) { self.0 += 1; }
    fn string_width(&self, _: &[u16]) -> i32 { 0 }
    fn point_size(&self) -> i32 { 12 }
    fn viewport(&self) -> [i32; 4] { [0, 0, 639, 447] }
    fn set_viewport(&mut self, _: [i32; 4]) {}
}

#[test]
fn round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases() {
    use crate::fork_ext::addons::{
        action_broker,
        discovery::{DiscoveredAddon, EntryMode},
        native_events, window_takeover,
    };
    let mk_addon = |id: &str, caps: &[&str]| {
        let root = std::env::temp_dir();
        Host::for_addon(&DiscoveredAddon {
            id: id.into(),
            name: id.into(),
            entry: root.join("main.lua"),
            script_root: root.clone(),
            package_root: root.clone(),
            module_dirs: vec![root.clone()],
            resource_dirs: vec![root],
            api: Some("eqoa/1".into()),
            entry_mode: EntryMode::Once,
            capabilities_declared: true,
            requested_capabilities: caps.iter().map(|s| (*s).to_string()).collect(),
            extensions: Default::default(),
            initially_enabled: true,
            mods_root: None,
        })
        .unwrap()
    };

    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.dump.player.p34 = 500;
    p.dump.player.p38 = 500;
    let mut bag_pot = potion();
    bag_pot.equip_slot = -1;
    p.dump.inventory = list(&[bag_pot]);
    p.dump.bank.items = list(&[]);

    // Negative control: mod without eqoa.windows.takeover is refused.
    let no_cap = mk_addon("no_cap_mod", &["eqoa.actions.*"]);
    no_cap
        .execute(
            r#"
        local ok, err = eqoa.windows.takeover("loot")
        assert(ok == nil and err.code == "permission_denied")
    "#,
        )
        .unwrap();

    let host_a = mk_addon("holder_mod", &["eqoa.windows.takeover", "eqoa.actions.*"]);
    let host_b = mk_addon("rival_mod", &["eqoa.windows.takeover"]);
    host_a
        .execute(
            r#"
        assert(eqoa.windows.taken("loot") == false)
        assert(eqoa.windows.takeover("loot"))
        assert(eqoa.windows.takeover("merchant"))
        assert(eqoa.windows.takeover("bank"))
        assert(eqoa.windows.taken("loot") == true)
        local all = eqoa.windows.taken()
        assert(all.loot and all.merchant and all.bank and all.holders.loot == "holder_mod")
    "#,
        )
        .unwrap();

    // Two mods contesting: first wins, second gets busy naming holder_mod + diagnostic.
    host_b
        .execute(
            r#"
        local ok, err = eqoa.windows.takeover("loot")
        assert(ok == nil and err.code == "busy" and err.message:find("holder_mod", 1, true))
    "#,
        )
        .unwrap();
    let logs = host_b.drain_logs().unwrap();
    assert!(
        logs.iter().any(|l| l.contains("busy") && l.contains("holder_mod")),
        "conflict must diagnose naming holder: {logs:?}"
    );

    // Spawn entities in scene: corpse 9001 ([3,0,0]), merchant 9002 ([6,0,0]), banker 9003 ([8,0,0]).
    p.self_at = Some(([0.0, 0.0, 0.0], None));
    p.entities.slots = vec![None; 8];
    p.entities.slots[0] = Some(net::entity::Entity {
        id: 9001, world: 0, kind: net::entity::TYPE_CREATURE, p18: 3, pos: [3.0, 0.0, 0.0], actor_pos: [3.0, 0.0, 0.0],
        creature: Some(net::entity::Creature { name: "a fallen orc".into(), ..Default::default() }), ..Default::default()
    });
    p.entities.slots[1] = Some(net::entity::Entity {
        id: 9002, world: 0, kind: net::entity::TYPE_CREATURE, p18: 1, pos: [6.0, 0.0, 0.0], actor_pos: [6.0, 0.0, 0.0],
        creature: Some(net::entity::Creature { name: "Merchant Kim".into(), npc_flags: 0x80 | 1, ..Default::default() }), ..Default::default()
    });
    p.entities.slots[2] = Some(net::entity::Entity {
        id: 9003, world: 0, kind: net::entity::TYPE_CREATURE, p18: 1, pos: [8.0, 0.0, 0.0], actor_pos: [8.0, 0.0, 0.0],
        creature: Some(net::entity::Creature { name: "Banker Dan".into(), npc_flags: 0x80 | 2, ..Default::default() }), ..Default::default()
    });

    // 0 (M1). Player-started opens STILL open normal native windows even while holder_mod holds takeover!
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 1, "player-started loot must open native window even under takeover");
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.mode, 1, "sync_hidden_sessions must never hijack native loot window");
    apply(&mut p, opcode(0x0016, vec![]));
    assert_eq!(p.mode, 0);

    apply(&mut p, opcode(0x00B7, shop_bytes(9002, 90, 110, &[sword()], "Shop!")));
    assert_eq!(p.mode, 10, "player-started merchant must open native window even under takeover");
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.mode, 10, "sync_hidden_sessions must never hijack native merchant window");
    p.mode = 0;
    p.npcw.shop = Default::default();

    p.target = 9003;
    apply(&mut p, opcode(0x124D, vec![]));
    assert_eq!(p.mode, 11, "player-started bank must open native window even under takeover");
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.mode, 11, "sync_hidden_sessions must never hijack native bank window");
    p.mode = 0;
    p.bank = Default::default();
    p.target = u32::MAX;

    // 1. Mod-opened Loot (via open_loot): keeps mode == 0, draws 0 native pixels, loot data & action work, and M3 ends session on completion.
    host_a.execute("assert(eqoa.actions.open_loot(9001))").unwrap();
    super::super::begin_iteration(&mut p, true);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 0, "mod-opened taken-over loot must keep mode == 0");
    let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
    let mut raster = CountingRaster(0);
    p.run_npcw(&vui, &mut raster);
    assert_eq!(raster.0, 0, "native loot window must draw 0 calls under takeover");
    p.forge_snapshot(None);
    host_a
        .execute(
            r#"
        local l = assert(eqoa.loot.get())
        assert(l.open == true and #l.items == 1)
        assert(eqoa.windows.find("loot") == nil)
        loot_req = assert(eqoa.actions.loot(l.items[1].ref, 1))
    "#,
        )
        .unwrap();
    super::super::begin_iteration(&mut p, true);
    p.run_npcw(&vui, &mut raster);
    assert_eq!(raster.0, 0, "dispatching loot under takeover draws 0 calls");
    assert!(p.session.sends_lost > 0, "native 0x0017 loot request must still send");
    let mut w19 = net::stream::Writer::default();
    w19.u8(0).signed(0);
    let ev19 = opcode(0x0019, w19.data);
    let cap19 = super::super::begin_game_event(&p, &ev19);
    let _ = p.npcw_opcode(0x0019, match &ev19 { Event::Opcode { data, .. } => data, _ => &[] });
    super::super::end_game_event(&p, cap19);
    super::super::begin_iteration(&mut p, true);
    assert!(!super::super::loot_session_open(&p), "M3: loot action completion must end hidden loot session");

    // 2. Mod-opened Merchant (via talk): keeps mode == 0, draws 0 pixels, merchant.get().open == true & sell ends session (M3).
    let sent_before_merch = p.session.sends_lost;
    host_a.execute("assert(eqoa.actions.talk(9002, { kind = 'merchant' }))").unwrap();
    super::super::begin_iteration(&mut p, true);
    apply(&mut p, opcode(0x00B7, shop_bytes(9002, 90, 110, &[sword()], "Shop!")));
    assert_eq!(p.mode, 0, "mod-opened taken-over merchant must keep mode == 0");
    p.run_npcw(&vui, &mut raster);
    assert_eq!(raster.0, 0, "native merchant window must draw 0 calls under takeover");
    p.forge_snapshot(None);
    host_a
        .execute(
            r#"
        local m = assert(eqoa.merchant.get())
        assert(m.open == true and #m.items == 1)
        local inv = assert(eqoa.inventory.get())
        sell_req = assert(eqoa.actions.sell(inv.items[1].ref, 1))
    "#,
        )
        .unwrap();
    super::super::begin_iteration(&mut p, true);
    p.run_npcw(&vui, &mut raster);
    assert_eq!(raster.0, 0);
    assert!(p.session.sends_lost > sent_before_merch, "native sell request must still send");
    let ev_took = Event::InvTook { slot: 0, sound: 3, n: 1 };
    let cap_took = super::super::begin_game_event(&p, &ev_took);
    crate::inventory::took(&mut p.dump.inventory, 0, 1);
    p.npcw_item_took(3);
    super::super::end_game_event(&p, cap_took);
    super::super::begin_iteration(&mut p, true);
    assert!(!super::super::merchant_session_open(&p), "M3: sell completion must end hidden merchant session");
    assert!(p.npcw.shop.id.is_none());

    // 3 (M2). Bank opened via talk(9003): retargeting BEFORE 0x124D must still preserve banker_id = 9003 (not retargeted id)!
    let sent_before_bank = p.session.sends_lost;
    host_a.execute("assert(eqoa.actions.talk(9003, { kind = 'bank' }))").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 9003);
    apply(&mut p, opcode(0x124D, vec![]));
    assert_eq!(p.mode, 0, "mod-opened taken-over bank must keep mode == 0");
    p.target = 7777; // the player retargets after the open: the hidden session keeps its banker
    assert_eq!(
        window_takeover::hidden_bank_session(),
        Some(9003),
        "M2: hidden banker_id must come from talk request (9003), not retargeted play.target (7777)"
    );
    p.run_bank(&vui, &mut raster);
    assert_eq!(raster.0, 0, "native bank window must draw 0 calls under takeover");
    p.forge_snapshot(None);
    host_a
        .execute(
            r#"
        local b = assert(eqoa.bank.get())
        assert(b.open == true)
        bm_req = assert(eqoa.actions.bank_money("deposit", 25))
    "#,
        )
        .unwrap();
    super::super::begin_iteration(&mut p, true);
    p.run_bank(&vui, &mut raster);
    assert_eq!(raster.0, 0, "bank_taken_tick must draw 0 calls to screen raster");
    assert_eq!(p.bank.state, 1, "bank_money must have sent native 0x1255");
    assert_eq!(p.target, 7777, "bank_taken_tick must restore player's current target");
    assert!(p.session.sends_lost > sent_before_bank);
    let mut w53 = net::stream::Writer::default();
    w53.signed(525);
    let ev53 = opcode(0x1253, w53.data);
    let cap53 = super::super::begin_game_event(&p, &ev53);
    let _ = p.bank_opcode(0x1253, match &ev53 { Event::Opcode { data, .. } => data, _ => &[] });
    super::super::end_game_event(&p, cap53);
    super::super::begin_iteration(&mut p, true);
    assert!(!super::super::bank_session_open(&p), "M3: bank_money completion must end hidden bank session");

    // 3b (M3): Walk-away & idle timeout release lingering hidden sessions so next player open is native.
    host_a.execute("assert(eqoa.actions.open_loot(9001))").unwrap();
    super::super::begin_iteration(&mut p, true);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert!(super::super::loot_session_open(&p));
    p.self_at = Some(([40.0, 0.0, 0.0], None)); // walked > 25u away from [0,0,0] / corpse at [3,0,0]
    super::super::begin_iteration(&mut p, true);
    assert!(!super::super::loot_session_open(&p), "M3: walk-away must end hidden loot session");
    assert!(p.npcw.corpse.is_none());

    p.self_at = Some(([0.0, 0.0, 0.0], None));
    host_a.execute("assert(eqoa.actions.open_loot(9001))").unwrap();
    super::super::begin_iteration(&mut p, true);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert!(super::super::loot_session_open(&p));
    window_takeover::expire_hidden_session_for_test(window_takeover::TakeoverKind::Loot);
    super::super::begin_iteration(&mut p, true);
    assert!(!super::super::loot_session_open(&p), "M3: idle timeout must end hidden loot session");
    assert!(p.npcw.corpse.is_none());
    // Next player-started loot opens normally (mode == 1)!
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 1, "after timeout cleanup, player open must show native window");
    apply(&mut p, opcode(0x0016, vec![]));

    // 4. Auto-release on zoning -> released cleanly; re-take -> auto-release on mod disable/clear.
    let tp = Event::Teleport { world: 2, pos: [0.0; 3], hpr: [0.0; 3], token: 7 };
    let cap = super::super::begin_game_event(&p, &tp);
    super::super::end_game_event(&p, cap);
    host_a.execute("assert(eqoa.windows.taken('loot') == false and eqoa.windows.taken('bank') == false)").unwrap();
    host_a.execute("assert(eqoa.windows.takeover('loot'))").unwrap();
    native_events::clear(&host_a);
    host_b.execute("assert(eqoa.windows.taken('loot') == false and eqoa.windows.takeover('loot'))").unwrap();
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round65_open_loot_talk_npc_flags_and_strict_target_guards() {
    use crate::fork_ext::addons::{
        action_broker,
        discovery::{DiscoveredAddon, EntryMode},
        native_events, window_takeover,
    };
    let root = std::env::temp_dir();
    let host = Host::for_addon(&DiscoveredAddon {
        id: "r65_actions".into(),
        name: "r65_actions".into(),
        entry: root.join("main.lua"),
        script_root: root.clone(),
        package_root: root.clone(),
        module_dirs: vec![root.clone()],
        resource_dirs: vec![root],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: vec!["eqoa.actions.*".into(), "eqoa.windows.takeover".into()],
        extensions: Default::default(),
        initially_enabled: true,
        mods_root: None,
    })
    .unwrap();

    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.player.world = 0;
    p.self_at = Some(([0.0, 0.0, 0.0], None));
    p.entities.slots = vec![
        Some(net::entity::Entity {
            id: 5001,
            world: 0,
            kind: net::entity::TYPE_CREATURE,
            p18: 3, // targetable + corpse
            pos: [20.0, 0.0, 0.0],
            actor_pos: [20.0, 0.0, 0.0],
            creature: Some(net::entity::Creature {
                name: "Far Corpse".into(),
                npc_flags: 0,
                ..Default::default()
            }),
            ..Default::default()
        }),
        Some(net::entity::Entity {
            id: 5002,
            world: 0,
            kind: net::entity::TYPE_CREATURE,
            p18: 3, // targetable + corpse (nearest!)
            pos: [4.0, 0.0, 0.0],
            actor_pos: [4.0, 0.0, 0.0],
            creature: Some(net::entity::Creature {
                name: "Near Corpse".into(),
                npc_flags: 0,
                ..Default::default()
            }),
            ..Default::default()
        }),
        Some(net::entity::Entity {
            id: 5003,
            world: 0,
            kind: net::entity::TYPE_CREATURE,
            p18: 1, // living hostile mob (NOT a corpse, no friendly/merchant/bank npc_flags)
            pos: [2.0, 0.0, 0.0],
            actor_pos: [2.0, 0.0, 0.0],
            creature: Some(net::entity::Creature {
                name: "Hostile Mob".into(),
                npc_flags: 0,
                ..Default::default()
            }),
            ..Default::default()
        }),
        Some(net::entity::Entity {
            id: 5004,
            world: 0,
            kind: net::entity::TYPE_CREATURE,
            p18: 1,
            pos: [8.0, 0.0, 0.0],
            actor_pos: [8.0, 0.0, 0.0],
            creature: Some(net::entity::Creature {
                name: "Merchant Bob".into(),
                npc_flags: 0x80 | 1,
                ..Default::default()
            }),
            ..Default::default()
        }),
        Some(net::entity::Entity {
            id: 5005,
            world: 0,
            kind: net::entity::TYPE_CREATURE,
            p18: 1,
            pos: [12.0, 0.0, 0.0],
            actor_pos: [12.0, 0.0, 0.0],
            creature: Some(net::entity::Creature {
                name: "Banker Alice".into(),
                npc_flags: 0x80 | 2,
                ..Default::default()
            }),
            ..Default::default()
        }),
    ];
    p.forge_snapshot(None);
    host.execute(
        r#"
        results = {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        local m = eqoa.entities.find(5004)
        local b = eqoa.entities.find(5005)
        assert(m.npc_flags == (128 + 1), "merchant npc_flags exported")
        assert(b.npc_flags == (128 + 2), "banker npc_flags exported")
    "#,
    )
    .unwrap();

    // 1. Strict negative control: open_loot(living_mob) & talk(corpse) & talk(hostile_mob) never send packets!
    let sent0 = p.session.sends_lost;
    host.execute(
        r#"
        r_bad_loot = assert(eqoa.actions.open_loot(5003))
        r_bad_talk1 = assert(eqoa.actions.talk(5002))
        r_bad_talk2 = assert(eqoa.actions.talk(5003))
        r_bad_bank = assert(eqoa.actions.talk(5004, { kind = "bank" }))
    "#,
    )
    .unwrap();
    for _ in 0..4 {
        super::super::begin_iteration(&mut p, true);
    }
    native_events::dispatch(&host).unwrap();
    assert_eq!(p.session.sends_lost, sent0, "guards must send zero packets");
    assert!(!p.auto_attack, "open_loot guard must never engage auto_attack");
    host.execute(
        r#"
        assert(eqoa.actions.status(r_bad_loot).state == "refused")
        assert(eqoa.actions.status(r_bad_talk1).state == "refused")
        assert(eqoa.actions.status(r_bad_talk2).state == "refused")
        assert(eqoa.actions.status(r_bad_bank).state == "refused")
    "#,
    )
    .unwrap();

    // 2. open_loot() (nil) picks nearest corpse (5002 at dist 4, not 5001 at 20, not living 5003 at 2).
    host.execute("r_open = assert(eqoa.actions.open_loot())").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 5002, "open_loot() must target nearest corpse");
    assert!(p.session.sends_lost > sent0, "open_loot must send 0x0015");
    // Server refuses with 0x0085 code 1 -> ActionResult gets code="refused" + server text.
    apply(&mut p, opcode(0x0085, vec![1]));
    native_events::dispatch(&host).unwrap();
    host.execute(
        r#"
        local st = assert(eqoa.actions.status(r_open))
        assert(st.state == "refused" and st.ok == false and st.code == "refused")
        assert(type(st.reason) == "string" and #st.reason > 0)
    "#,
    )
    .unwrap();

    // Re-try open_loot(5002) and confirm on 0x0018.
    host.execute("assert(eqoa.windows.takeover('loot'))").unwrap();
    host.execute("r_open2 = assert(eqoa.actions.open_loot(5002))").unwrap();
    super::super::begin_iteration(&mut p, true);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    native_events::dispatch(&host).unwrap();
    host.execute("local st = assert(eqoa.actions.status(r_open2)); assert(st.state == 'completed' and st.ok == true)").unwrap();

    // 3. talk(nil, {kind="merchant"}) targets 5004; mismatched 0x00B7 (id 9999) does NOT confirm it, matching 5004 does!
    host.execute("assert(eqoa.windows.takeover('merchant')); assert(eqoa.windows.takeover('bank'))").unwrap();
    host.execute("r_merch = assert(eqoa.actions.talk(nil, { kind = 'merchant' }))").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 5004);
    apply(&mut p, opcode(0x00B7, shop_bytes(9999, 90, 110, &[sword()], "Wrong")));
    native_events::dispatch(&host).unwrap();
    host.execute("local st = assert(eqoa.actions.status(r_merch)); assert(st.state == 'dispatched' and st.phase == 'dispatch', 'wrong npc_id must not confirm talk')").unwrap();
    p.mode = 0;
    p.npcw.shop = Default::default();
    apply(&mut p, opcode(0x00B7, shop_bytes(5004, 90, 110, &[sword()], "Hi")));
    native_events::dispatch(&host).unwrap();
    host.execute("local st = assert(eqoa.actions.status(r_merch)); assert(st.state == 'completed' and st.ok == true)").unwrap();

    host.execute("r_bank = assert(eqoa.actions.talk(nil, { kind = 'bank' }))").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 5005);
    apply(&mut p, opcode(0x124D, vec![]));
    native_events::dispatch(&host).unwrap();
    host.execute("local st = assert(eqoa.actions.status(r_bank)); assert(st.state == 'completed' and st.ok == true)").unwrap();

    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round65_zero_addon_zero_takeover_probes() {
    use crate::fork_ext::addons::{action_broker, window_takeover};
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    let _ = window_takeover::take_state_borrows_for_test();

    let mut p = ready();
    let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
    let mut raster = crate::pointer::testing::NullRaster;
    for _ in 0..5 {
        super::super::begin_iteration(&mut p, true);
        p.run_npcw(&vui, &mut raster);
        p.run_bank(&vui, &mut raster);
    }
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    apply(&mut p, opcode(0x0016, vec![]));
    apply(&mut p, opcode(0x00B7, shop_bytes(7001, 90, 110, &[sword()], "Shop!")));
    p.npcw.shop = Default::default();
    p.mode = 0;
    apply(&mut p, opcode(0x124D, vec![]));
    p.mode = 0;
    assert_eq!(
        window_takeover::take_state_borrows_for_test(),
        0,
        "zero-addon path must perform zero takeover TLS state borrows"
    );
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
}

#[test]
fn round65_reset_all_for_test_clears_hidden_sessions() {
    use crate::fork_ext::addons::{action_broker, window_takeover};
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();

    let mut p = ready();
    assert!(
        window_takeover::takeover(
            1,
            "test_mod",
            window_takeover::TakeoverKind::Loot,
            window_takeover::TakeoverScope::Own,
        )
        .is_ok()
    );
    assert!(
        window_takeover::takeover(
            1,
            "test_mod",
            window_takeover::TakeoverKind::Merchant,
            window_takeover::TakeoverScope::Own,
        )
        .is_ok()
    );
    assert!(
        window_takeover::takeover(
            1,
            "test_mod",
            window_takeover::TakeoverKind::Bank,
            window_takeover::TakeoverScope::Own,
        )
        .is_ok()
    );
    assert!(window_takeover::activate_hidden_for_test(1, window_takeover::TakeoverKind::Loot, 9001));
    assert!(window_takeover::activate_hidden_for_test(1, window_takeover::TakeoverKind::Merchant, 9002));
    assert!(window_takeover::activate_hidden_for_test(1, window_takeover::TakeoverKind::Bank, 9003));
    p.npcw.corpse = Some(list(&[gem()]));
    p.npcw.shop = crate::merchant::read_shop(&shop_bytes(9002, 90, 110, &[sword()], "Hi"));
    assert!(super::super::loot_session_open(&p));
    assert!(super::super::merchant_session_open(&p));
    assert!(super::super::bank_session_open(&p));
    assert!(window_takeover::has_activity());

    window_takeover::reset_all_for_test();
    assert!(!window_takeover::has_activity());
    assert!(!super::super::loot_session_open(&p));
    assert!(!super::super::merchant_session_open(&p));
    assert!(!super::super::bank_session_open(&p));
    assert_eq!(window_takeover::hidden_bank_session(), None);
}

#[test]
fn round65_loot_release_clears_corpse_visibility_and_master_loot() {
    use crate::fork_ext::addons::{action_broker, window_takeover};
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();

    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    assert!(
        window_takeover::takeover(
            1,
            "test_mod",
            window_takeover::TakeoverKind::Loot,
            window_takeover::TakeoverScope::Own,
        )
        .is_ok()
    );
    assert!(window_takeover::arm_pending_open(window_takeover::TakeoverKind::Loot, 1, 9001));
    crate::fork_ext::loot_requested(9001);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 0);
    let ml = net::raid::MasterLoot { seconds: 60, members: vec![] };
    let ev_ml = Event::MasterLoot(Some(ml.clone()));
    let cap_ml = super::super::begin_game_event(&p, &ev_ml);
    p.master_loot(Some(ml));
    super::super::end_game_event(&p, cap_ml);
    assert!(action_broker::is_master_loot_active());
    assert!(p.npcw.corpse.is_some());

    let sends_before = p.session.sends_lost;
    assert!(window_takeover::release(1, window_takeover::TakeoverKind::Loot));
    super::super::begin_iteration(&mut p, true);
    assert!(p.session.sends_lost > sends_before, "release must send 0x0016");
    assert!(p.npcw.corpse.is_none(), "corpse must be cleared");
    assert!(!action_broker::is_master_loot_active(), "master-loot flag must be cleared on release");

    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
}

#[test]
fn round65_window_takeover_scopes_own_and_all() {
    use crate::fork_ext::addons::{
        action_broker,
        discovery::{DiscoveredAddon, EntryMode},
        window_takeover,
    };
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();

    let root = std::env::temp_dir();
    let host = Host::for_addon(&DiscoveredAddon {
        id: "r65_scopes".into(),
        name: "r65_scopes".into(),
        entry: root.join("main.lua"),
        script_root: root.clone(),
        package_root: root.clone(),
        module_dirs: vec![root.clone()],
        resource_dirs: vec![root.clone()],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: vec!["eqoa.windows.takeover".into(), "eqoa.actions.*".into()],
        extensions: crate::fork_ext::addons::discovery::Extensions {
            replaces: vec!["loot".into(), "merchant".into(), "bank".into()],
            ..Default::default()
        },
        initially_enabled: true,
        mods_root: None,
    })
    .unwrap();

    // Invalid scope rejects cleanly with invalid_argument.
    host.execute(
        r#"
        local ok, err = eqoa.windows.takeover("loot", {scope = "bad"})
        assert(ok == nil and err and err.code == "invalid_argument")
    "#,
    )
    .unwrap();

    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.dump.player.p34 = 500;
    let mut bag_pot = potion();
    bag_pot.equip_slot = -1;
    p.dump.inventory = list(&[bag_pot]);
    p.self_at = Some(([0.0, 0.0, 0.0], None));
    p.entities.slots = vec![
        Some(net::entity::Entity {
            id: 9101,
            world: 0,
            kind: net::entity::TYPE_CREATURE,
            p18: 3,
            pos: [3.0, 0.0, 0.0],
            actor_pos: [3.0, 0.0, 0.0],
            creature: Some(net::entity::Creature {
                name: "a beetle corpse".into(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        Some(net::entity::Entity {
            id: 9102,
            world: 0,
            kind: net::entity::TYPE_CREATURE,
            p18: 1,
            pos: [4.0, 0.0, 0.0],
            actor_pos: [4.0, 0.0, 0.0],
            creature: Some(net::entity::Creature {
                name: "Merchant Scope".into(),
                npc_flags: 0x88,
                ..Default::default()
            }),
            ..Default::default()
        }),
        Some(net::entity::Entity {
            id: 9103,
            world: 0,
            kind: net::entity::TYPE_CREATURE,
            p18: 1,
            pos: [5.0, 0.0, 0.0],
            actor_pos: [5.0, 0.0, 0.0],
            creature: Some(net::entity::Creature {
                name: "Banker Scope".into(),
                npc_flags: 0x82,
                ..Default::default()
            }),
            ..Default::default()
        }),
    ];

    // 1. scope = "own" (explicit and default) leaves player-opened loot, merchant, AND bank normal.
    host.execute(
        r#"
        assert(eqoa.windows.takeover("loot"))
        assert(eqoa.windows.takeover("merchant", {scope = "own"}))
        assert(eqoa.windows.takeover("bank", {scope = "own"}))
    "#,
    )
    .unwrap();
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 1, "scope=own must leave player-opened corpse window showing (mode 1)");
    apply(&mut p, opcode(0x0016, vec![]));

    p.target = 9102;
    apply(&mut p, opcode(0x00B7, shop_bytes(9102, 90, 110, &[sword()], "Welcome")));
    assert_eq!(p.mode, 10, "scope=own must leave player-opened merchant window showing (mode 10)");
    p.npcw.merchant.visible = false;
    p.npcw.shop = Default::default();
    p.mode = 0;

    p.target = 9103;
    apply(&mut p, opcode(0x124D, vec![]));
    assert_eq!(p.mode, 11, "scope=own must leave player-opened bank window showing (mode 11)");
    p.bank = Default::default();
    p.mode = 0;

    // 2. scope = "all" hides every session of that kind, including player-opened ones.
    host.execute(
        r#"
        assert(eqoa.windows.takeover("loot", {scope = "all"}))
        assert(eqoa.windows.takeover("merchant", {scope = "all"}))
        assert(eqoa.windows.takeover("bank", {scope = "all"}))
    "#,
    )
    .unwrap();

    // Player clicks corpse by hand -> hidden (mode == 0), readable via snapshot.
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 0, "scope=all must hide player-opened loot window");
    assert!(super::super::loot_session_open(&p));
    let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
    let mut raster = CountingRaster(0);
    p.run_npcw(&vui, &mut raster);
    assert_eq!(raster.0, 0);
    apply(&mut p, opcode(0x0016, vec![]));
    assert!(!super::super::loot_session_open(&p));

    // Player clicks merchant by hand -> hidden (mode == 0), draws nothing, single sell keeps it open until release.
    p.target = 9102;
    apply(&mut p, opcode(0x00B7, shop_bytes(9102, 90, 110, &[sword()], "Welcome")));
    assert_eq!(p.mode, 0, "scope=all must hide player-opened merchant window");
    assert!(super::super::merchant_session_open(&p));
    p.run_npcw(&vui, &mut raster);
    assert_eq!(raster.0, 0);
    p.forge_snapshot(None);
    host.execute(
        r#"
        local m = assert(eqoa.merchant.get())
        assert(m.open == true and #m.items == 1)
        local inv = assert(eqoa.inventory.get())
        assert(eqoa.actions.sell(inv.items[1].ref, 1))
    "#,
    )
    .unwrap();
    super::super::begin_iteration(&mut p, true);
    let ev_took = Event::InvTook { slot: 0, sound: 3, n: 1 };
    let cap_took = super::super::begin_game_event(&p, &ev_took);
    crate::inventory::took(&mut p.dump.inventory, 0, 1);
    p.npcw_item_took(3);
    super::super::end_game_event(&p, cap_took);
    super::super::begin_iteration(&mut p, true);
    assert!(
        super::super::merchant_session_open(&p),
        "scope=all must keep replacement merchant open across single sell until mod releases or player leaves"
    );
    // Mod closes player's replacement merchant window cleanly via release().
    host.execute("assert(eqoa.windows.release('merchant'))").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert!(!super::super::merchant_session_open(&p), "release() must close scope=all hidden merchant session");

    // Player clicks bank by hand -> hidden (mode == 0), uses play.target (9103) as banker_id.
    p.target = 9103;
    apply(&mut p, opcode(0x124D, vec![]));
    assert_eq!(p.mode, 0, "scope=all must hide player-opened bank window");
    assert_eq!(window_takeover::hidden_bank_session(), Some(9103));
    p.run_bank(&vui, &mut raster);
    assert_eq!(raster.0, 0);
    host.execute("assert(eqoa.windows.release('bank'))").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert!(!super::super::bank_session_open(&p));

    // And if mod holding scope="all" uses talk(9103) then player retargets to 7777 before 0x124D,
    // pending talk target (9103) still wins over retargeted play.target (M2 preserved).
    host.execute(
        r#"
        assert(eqoa.windows.takeover("bank", {scope = "all"}))
        assert(eqoa.actions.talk(9103, {kind = "bank"}))
    "#,
    )
    .unwrap();
    super::super::begin_iteration(&mut p, true);
    p.target = 7777;
    apply(&mut p, opcode(0x124D, vec![]));
    assert_eq!(p.mode, 0);
    assert_eq!(window_takeover::hidden_bank_session(), Some(9103));

    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
}

// ---- Round 6.5 review fixes (M1-M3, L4) -------------------------------------------------------

fn r65fix_creature(id: u32, name: &str, p18: u8, flags: u16, at: [f32; 3]) -> Option<net::entity::Entity> {
    Some(net::entity::Entity {
        id,
        world: 0,
        kind: net::entity::TYPE_CREATURE,
        p18,
        pos: at,
        actor_pos: at,
        creature: Some(net::entity::Creature { name: name.into(), npc_flags: flags, ..Default::default() }),
        ..Default::default()
    })
}

fn r65fix_world() -> Play {
    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    p.self_at = Some(([0.0, 0.0, 0.0], None));
    p.entities.slots = vec![
        r65fix_creature(9001, "a fallen orc", 3, 0, [3.0, 0.0, 0.0]),
        r65fix_creature(9002, "Merchant Kim", 1, 0x81, [6.0, 0.0, 0.0]),
        r65fix_creature(9003, "Banker Dan", 1, 0x82, [8.0, 0.0, 0.0]),
        r65fix_creature(9004, "a fallen gnoll", 3, 0, [4.0, 0.0, 0.0]),
        r65fix_creature(9005, "Merchant Lee", 1, 0x81, [7.0, 0.0, 0.0]),
        r65fix_creature(9007, "Banker Sue", 1, 0x82, [9.0, 0.0, 0.0]),
    ];
    p
}

fn r65fix_own(kind: crate::fork_ext::addons::window_takeover::TakeoverKind) {
    use crate::fork_ext::addons::window_takeover::{self, TakeoverScope};
    assert!(window_takeover::takeover(1, "r65fix_mod", kind, TakeoverScope::Own).is_ok());
}

fn r65fix_host(caps: &[&str]) -> Host {
    use crate::fork_ext::addons::discovery::{DiscoveredAddon, EntryMode};
    let root = std::env::temp_dir();
    Host::for_addon(&DiscoveredAddon {
        id: "r65fix_mod".into(),
        name: "r65fix_mod".into(),
        entry: root.join("main.lua"),
        script_root: root.clone(),
        package_root: root.clone(),
        module_dirs: vec![root.clone()],
        resource_dirs: vec![root.clone()],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: caps.iter().map(|s| (*s).to_string()).collect(),
        extensions: Default::default(),
        initially_enabled: true,
        mods_root: None,
    })
    .unwrap()
}

/// M2: under scope own a pending open claims only the reply it can name: loot and merchant by the
/// observed id, bank by the player's current target (0x124D carries no id). A player-started open
/// beside it stays native and the pending open survives for its own reply.
#[test]
fn round65_fix_m2_pending_open_never_claims_a_player_started_open() {
    use crate::fork_ext::addons::{action_broker, window_takeover::{self, TakeoverKind}};
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let mut p = r65fix_world();
    for k in TakeoverKind::ALL {
        r65fix_own(k);
    }
    let _ = window_takeover::take_log_for_test();

    // loot: the mod's request and the player's overlap (two outstanding): the reply cannot be
    // told apart, so it must stay a native window, never hidden.
    assert!(window_takeover::arm_pending_open(TakeoverKind::Loot, 1, 9001));
    crate::fork_ext::loot_requested(9004); // the player's own
    crate::fork_ext::loot_requested(9001); // the mod's
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 1, "overlapping loot requests: the reply must not be claimed by the pending open");
    let log = window_takeover::take_log_for_test();
    assert!(
        log.iter().any(|l| l.contains("loot window left to the game") && l.contains("overlapped")),
        "a decline is logged with its reason: {log:?}"
    );
    apply(&mut p, opcode(0x0016, vec![]));
    crate::fork_ext::loot_source::clear();
    crate::fork_ext::loot_requested(9001);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 0, "the pending open must still claim its own reply (known matching target)");
    let log = window_takeover::take_log_for_test();
    assert!(
        log.iter().any(|l| l.contains("loot window HIDDEN for the mod")),
        "an accept is logged: {log:?}"
    );
    apply(&mut p, opcode(0x0016, vec![]));
    window_takeover::set_hidden_inactive(TakeoverKind::Loot);

    // merchant: the player's shop (9005) arrives while the mod's talk(9002) is pending
    assert!(window_takeover::arm_pending_open(TakeoverKind::Merchant, 1, 9002));
    apply(&mut p, opcode(0x00B7, shop_bytes(9005, 90, 110, &[sword()], "Lee")));
    assert_eq!(p.mode, 10, "a player-started merchant must stay native beside a pending talk");
    p.mode = 0;
    p.npcw.merchant.visible = false;
    p.npcw.shop = Default::default();
    apply(&mut p, opcode(0x00B7, shop_bytes(9002, 90, 110, &[sword()], "Kim")));
    assert_eq!(p.mode, 0, "the pending talk must still claim its own merchant");
    window_takeover::set_hidden_inactive(TakeoverKind::Merchant);
    p.npcw.shop = Default::default();

    // bank: no id in 0x124D; the player's target is 9007, the mod asked for 9003
    assert!(window_takeover::arm_pending_open(TakeoverKind::Bank, 1, 9003));
    p.target = 9007;
    apply(&mut p, opcode(0x124D, vec![]));
    assert_eq!(p.mode, 11, "a player-started bank (other target) must stay native beside a pending talk");
    p.mode = 0;
    p.bank = Default::default();
    p.target = 9003;
    apply(&mut p, opcode(0x124D, vec![]));
    assert_eq!(p.mode, 0, "the pending bank talk must claim the open for its own banker");
    assert_eq!(window_takeover::hidden_bank_session(), Some(9003));

    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
}

/// M2: a refused (0x0085) or timed-out open leaves no pending claim behind.
#[test]
fn round65_fix_m2_refused_open_clears_pending_claim() {
    use crate::fork_ext::addons::{action_broker, window_takeover};
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let host = r65fix_host(&["eqoa.windows.takeover", "eqoa.actions.*"]);
    let mut p = r65fix_world();
    p.forge_snapshot(None);
    host.execute("assert(eqoa.windows.takeover('merchant'))").unwrap();
    host.execute("local r, e = eqoa.actions.talk(9002, {kind = 'merchant'}); assert(r, e and (e.code .. ': ' .. e.message))")
        .unwrap();
    super::super::begin_iteration(&mut p, true);
    // the server refuses
    apply(&mut p, opcode(0x0085, vec![1]));
    super::super::begin_iteration(&mut p, true);
    // the player now opens a shop by hand: it must be theirs, a native window
    apply(&mut p, opcode(0x00B7, shop_bytes(9002, 90, 110, &[sword()], "Kim")));
    assert_eq!(p.mode, 10, "a refused talk must not leave a pending claim on the player's own open");
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
}

/// M3: a hidden session that ends while a native window of the player's is open is not forgotten:
/// the close is owed and goes out once nothing blocks it, leaving no stale corpse.
#[test]
fn round65_fix_m3_hidden_session_end_while_mode_nonzero_is_deferred_not_dropped() {
    use crate::fork_ext::addons::{action_broker, window_takeover::{self, TakeoverKind}};
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let mut p = r65fix_world();
    r65fix_own(TakeoverKind::Loot);
    assert!(window_takeover::arm_pending_open(TakeoverKind::Loot, 1, 9001));
    crate::fork_ext::loot_requested(9001);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 0);
    assert!(p.npcw.corpse.is_some());
    // the player opens a merchant by hand while the hidden corpse session lives
    apply(&mut p, opcode(0x00B7, shop_bytes(9005, 90, 110, &[sword()], "Lee")));
    assert_eq!(p.mode, 10);
    // the mod lets go: the session must end, but cannot be closed under the player's window
    assert!(window_takeover::release(1, TakeoverKind::Loot));
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.mode, 10, "the player's native window is untouched");
    assert!(window_takeover::has_activity(), "the owed close must keep the takeover state alive");
    // the player closes their window; the owed close goes out now
    p.mode = 0;
    p.npcw.merchant.visible = false;
    p.npcw.shop = Default::default();
    let before = p.session.sends_lost;
    super::super::begin_iteration(&mut p, true);
    assert!(p.npcw.corpse.is_none(), "M3: the hidden corpse must not stay stale after its session ended");
    assert!(p.session.sends_lost > before, "M3: the deferred 0x0016 close must be sent");
    super::super::begin_iteration(&mut p, true);
    assert!(!window_takeover::has_activity(), "once sent nothing is owed");
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
}

/// L4: a window the player already had open natively is never taken over by a reply that arrives
/// while a pending open for the same thing waits.
#[test]
fn round65_fix_l4_window_already_open_is_not_hidden_on_arrival() {
    use crate::fork_ext::addons::{action_broker, window_takeover::{self, TakeoverKind}};
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let mut p = r65fix_world();
    for k in TakeoverKind::ALL {
        r65fix_own(k);
    }
    // loot: native window up, then the mod's request is pending and its reply arrives
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 1);
    assert!(window_takeover::arm_pending_open(TakeoverKind::Loot, 1, 9001));
    crate::fork_ext::loot_requested(9001);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 1, "L4: an already-open native corpse window must not be hidden by the reply");
    apply(&mut p, opcode(0x0016, vec![]));
    crate::fork_ext::loot_source::clear();
    window_takeover::set_hidden_inactive(TakeoverKind::Loot);

    // merchant
    apply(&mut p, opcode(0x00B7, shop_bytes(9002, 90, 110, &[sword()], "Kim")));
    assert_eq!(p.mode, 10);
    assert!(window_takeover::arm_pending_open(TakeoverKind::Merchant, 1, 9002));
    apply(&mut p, opcode(0x00B7, shop_bytes(9002, 90, 110, &[sword()], "Kim")));
    assert_eq!(p.mode, 10, "L4: an already-open native merchant window must not be hidden");
    p.mode = 0;
    p.npcw.shop = Default::default();
    window_takeover::set_hidden_inactive(TakeoverKind::Merchant);

    // bank
    p.target = 9003;
    apply(&mut p, opcode(0x124D, vec![]));
    assert_eq!(p.mode, 11);
    assert!(window_takeover::arm_pending_open(TakeoverKind::Bank, 1, 9003));
    apply(&mut p, opcode(0x124D, vec![]));
    assert_eq!(p.mode, 11, "L4: an already-open native bank window must not be hidden");
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
}

/// M1: a mod's `open_loot`/`talk` moves the player's target; when the mod is done (its hidden
/// session ended) the previous target comes back if it still exists, is not a corpse and the
/// player has not chosen another meanwhile.
#[test]
fn round65_fix_m1_previous_target_restored_when_the_mods_open_is_done() {
    use crate::fork_ext::addons::{action_broker, window_takeover::{self, TakeoverKind}};
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let host = r65fix_host(&["eqoa.windows.takeover", "eqoa.actions.*"]);
    let mut p = r65fix_world();
    p.entities.slots.push(r65fix_creature(9500, "a bristly rat", 1, 0, [10.0, 0.0, 0.0]));
    p.entities.slots.push(r65fix_creature(9600, "a gnoll pup", 1, 0, [11.0, 0.0, 0.0]));
    p.forge_snapshot(None);
    host.execute("assert(eqoa.windows.takeover('loot'))").unwrap();

    // 1. the previous target (9500) is put back after the session ends
    p.target = 9500;
    host.execute("local r, e = eqoa.actions.open_loot(9001); assert(r, e and (e.code .. ': ' .. e.message))").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 9001, "open_loot targets the corpse (documented)");
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 9001, "not restored while the open is pending");
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 0);
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 9001, "not restored while the hidden session is in use");
    let before = p.session.sends_lost;
    host.execute("assert(eqoa.windows.release('loot'))").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 9500, "M1: the player's previous target comes back");
    assert!(p.session.sends_lost > before, "M1: the restore goes through the game's own target request");

    // 2. a player who chose another target meanwhile keeps their choice
    host.execute("assert(eqoa.windows.takeover('loot'))").unwrap();
    p.target = 9500;
    host.execute("assert(eqoa.actions.open_loot(9001))").unwrap();
    super::super::begin_iteration(&mut p, true);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    p.target = 9600; // the player chose
    host.execute("assert(eqoa.windows.release('loot'))").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 9600, "M1: never override a target the player chose");

    // 3. a previous target that became a corpse is not restored
    host.execute("assert(eqoa.windows.takeover('loot'))").unwrap();
    p.target = 9500;
    host.execute("assert(eqoa.actions.open_loot(9001))").unwrap();
    super::super::begin_iteration(&mut p, true);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    if let Some(Some(e)) = p.entities.slots.iter_mut().find(|e| e.as_ref().is_some_and(|e| e.id == 9500)) {
        e.p18 = 3;
    }
    host.execute("assert(eqoa.windows.release('loot'))").unwrap();
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.target, 9001, "M1: a previous target that died is not restored");
    let _ = TakeoverKind::Loot;
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
}

/// Owner test (5): one close per hidden session, and none when there is nothing to close: the mod's end of a
/// session that the server already closed, or a release with no session up, sends no 0x0016.
#[test]
fn round65_fix2_one_close_per_hidden_session() {
    use crate::fork_ext::addons::{
        action_broker,
        window_takeover::{self, TakeoverKind},
    };
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let mut p = r65fix_world();
    let sys = p.session.drdp.system_endpoint;
    let remote = drdp::address_to_endpoint([127, 0, 0, 1], 10071, drdp::PORT_LEADER);
    p.session.conn = p.session.drdp.connection_open(sys, remote);
    assert!(p.session.conn.is_some());
    p.session.record = Some(vec![]);
    let _ = window_takeover::take_log_for_test();
    let closes = |p: &Play| {
        p.session
            .record
            .as_ref()
            .unwrap()
            .iter()
            .filter(|(_, data)| data == &0x0016u16.to_le_bytes())
            .count()
    };

    // a) a live hidden session: the mod's release AND an idle expiry in the same pass send exactly one 0x0016
    r65fix_own(TakeoverKind::Loot);
    assert!(window_takeover::arm_pending_open(
        TakeoverKind::Loot,
        1,
        9001
    ));
    crate::fork_ext::loot_requested(9001);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 0);
    window_takeover::expire_hidden_session_for_test(TakeoverKind::Loot);
    assert!(window_takeover::release(1, TakeoverKind::Loot));
    let before = closes(&p);
    super::super::begin_iteration(&mut p, true);
    assert_eq!(closes(&p) - before, 1, "exactly one 0x0016 for the session");
    super::super::begin_iteration(&mut p, true);
    super::super::begin_iteration(&mut p, true);
    assert_eq!(
        closes(&p) - before,
        1,
        "no further close on later iterations"
    );
    assert!(p.npcw.corpse.is_none());
    let log = window_takeover::take_log_for_test();
    let closed: Vec<_> = log.iter().filter(|l| l.contains("loot hidden session closed")).collect();
    assert_eq!(closed.len(), 1, "one close line per session, with its reason: {log:?}");
    assert!(closed[0].contains("0x0016 sent"), "{closed:?}");

    // b) the server closed the corpse first (its own 0x0016): our end of the session sends nothing
    window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    r65fix_own(TakeoverKind::Loot);
    assert!(window_takeover::arm_pending_open(
        TakeoverKind::Loot,
        1,
        9001
    ));
    crate::fork_ext::loot_requested(9001);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    apply(&mut p, opcode(0x0016, vec![]));
    let before = closes(&p);
    assert!(window_takeover::release(1, TakeoverKind::Loot));
    super::super::begin_iteration(&mut p, true);
    super::super::begin_iteration(&mut p, true);
    assert_eq!(
        closes(&p),
        before,
        "the server already closed it: no second 0x0016"
    );
    let log = window_takeover::take_log_for_test();
    assert!(
        log.iter().any(|l| l.contains("loot hidden session closed") && l.contains("not sent")),
        "the close without a packet is logged too: {log:?}"
    );

    // c) a release with no hidden session up closes nothing and sends nothing
    window_takeover::reset_all_for_test();
    r65fix_own(TakeoverKind::Loot);
    let before = closes(&p);
    assert!(window_takeover::release(1, TakeoverKind::Loot));
    super::super::begin_iteration(&mut p, true);
    assert_eq!(
        closes(&p),
        before,
        "nothing was hidden, so nothing is closed"
    );
    let log = window_takeover::take_log_for_test();
    assert!(
        log.iter().any(|l| l.contains("released with no hidden session")),
        "{log:?}"
    );
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
}

/// Owner test (a): the corpse the server lets you open may be far from you (the client has no range check):
/// a session opened on a corpse beyond 25 units must stay up, ending only when the player walks away from where it
/// opened.
#[test]
fn round65_fix2_far_corpse_session_is_not_closed_at_once() {
    use crate::fork_ext::addons::{
        action_broker,
        window_takeover::{self, TakeoverKind},
    };
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let mut p = r65fix_world();
    p.entities.slots.push(r65fix_creature(
        9400,
        "a far orc corpse",
        3,
        0,
        [40.0, 0.0, 0.0],
    ));
    r65fix_own(TakeoverKind::Loot);
    assert!(window_takeover::arm_pending_open(
        TakeoverKind::Loot,
        1,
        9400
    ));
    crate::fork_ext::loot_requested(9400);
    apply(&mut p, opcode(0x0018, list_bytes(&[gem()])));
    assert_eq!(p.mode, 0);
    for _ in 0..5 {
        super::super::begin_iteration(&mut p, true);
    }
    assert!(
        super::super::loot_session_open(&p),
        "a corpse opened from afar must stay open and show its items"
    );
    assert!(p.npcw.corpse.is_some());
    p.self_at = Some(([0.0, 60.0, 0.0], None)); // the player walks off 60 units
    super::super::begin_iteration(&mut p, true);
    assert!(
        !super::super::loot_session_open(&p),
        "walking away from where it opened still ends it"
    );
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
}

/// Use the native merchant flag, and inspect the actual packet rather than treating any lost
/// send as a successful merchant request. A closer smith must never win merchant selection.
#[test]
fn round65_fix2_merchant_uses_native_flags_and_exact_request() {
    use crate::fork_ext::addons::{action_broker, window_takeover};
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    let host = r65fix_host(&["eqoa.actions.*", "eqoa.windows.takeover"]);
    let mut p = r65fix_world();
    p.entities.slots = vec![
        r65fix_creature(9401, "Merchant", 1, 0x81, [6.0, 0.0, 0.0]),
        r65fix_creature(9402, "Smith", 1, 0x88, [1.0, 0.0, 0.0]),
    ];
    let sys = p.session.drdp.system_endpoint;
    let remote = drdp::address_to_endpoint([127, 0, 0, 1], 10071, drdp::PORT_LEADER);
    p.session.conn = p.session.drdp.connection_open(sys, remote);
    assert!(p.session.conn.is_some());
    p.session.record = Some(vec![]);
    p.forge_snapshot(None);
    let _ = window_takeover::take_log_for_test();
    host.execute("assert(eqoa.windows.takeover('merchant')); req = assert(eqoa.actions.talk(nil, {kind='merchant'}))").unwrap();
    super::super::begin_iteration(&mut p, true);
    let log = window_takeover::take_log_for_test();
    assert!(
        log.iter().any(|l| l.contains("mod talk -> npc 0x24b9") && l.contains("opcode 0x004c")),
        "the request's entity and opcode are logged: {log:?}"
    );
    assert_eq!(
        p.target, 9401,
        "merchant selection must skip the closer smith"
    );
    let mut expected = Writer::op(0x004C);
    expected.u32(9401);
    assert!(
        p.session
            .record
            .as_ref()
            .unwrap()
            .iter()
            .any(|(_, data)| data == &expected.data),
        "native merchant opcode and entity id must go out"
    );
    assert!(
        !p.session
            .record
            .as_ref()
            .unwrap()
            .iter()
            .any(|(_, data)| data.starts_with(&0x0061u16.to_le_bytes())),
        "never ask the smith to repair"
    );
    host.execute(
        "assert(eqoa.actions.status(req).state == 'dispatched', 'sending is not success')",
    )
    .unwrap();
    apply(
        &mut p,
        opcode(0x00B7, shop_bytes(9401, 90, 110, &[sword()], "Hi")),
    );
    assert_eq!(p.mode, 0);
    host.execute("assert(eqoa.actions.status(req).ok == true)")
        .unwrap();
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
}

/// Round 7 Part C fix L1: the "Use the game's windows" switch holds after a restart, and turning it on stops a
/// running batch whose window claim it released, exactly as the Stop button does.
#[test]
fn round7c_game_windows_switch_is_saved_and_stops_a_claimed_batch() {
    use crate::fork_ext::addons::{action_broker, native_events, window_takeover};
    let config = std::env::temp_dir().join(format!("eqoa-r7c-switch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&config);
    std::fs::create_dir_all(&config).unwrap();
    let previous = std::env::var_os("EQOA_CONFIG");
    unsafe { std::env::set_var("EQOA_CONFIG", &config) };
    window_takeover::reset_all_for_test();
    window_takeover::set_use_game_windows(true);
    window_takeover::forget_switch_for_test();
    let kept = window_takeover::use_game_windows();
    window_takeover::set_use_game_windows(false);
    window_takeover::forget_switch_for_test();
    let cleared = !window_takeover::use_game_windows();

    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    let mut p = ready();
    p.session.conn = Some(usize::MAX);
    let mut a = potion();
    a.key = 101;
    a.equip_slot = -1;
    let mut b = gem();
    b.key = 102;
    b.equip_slot = -1;
    p.dump.inventory = list(&[a, b]);
    p.bags_open_merchant(88);
    p.forge_snapshot(None);
    let host = Host::with_test_capabilities(&["eqoa.actions.*", "eqoa.windows.takeover"]).unwrap();
    host.execute(
        r#"
        bres = {}
        eqoa.events.on('batch.result', function(e) bres[#bres + 1] = e.data end)
        assert(eqoa.windows.takeover('merchant'))
        local inv = assert(eqoa.inventory.get())
        assert(eqoa.actions.batch({ steps = {
            { kind = 'sell', item = inv.items[1].ref, count = 1 },
            { kind = 'sell', item = inv.items[2].ref, count = 1 },
        }, require_window = 'merchant' }))
        "#,
    )
    .unwrap();
    p.bags_broker_tick();
    native_events::dispatch(&host).unwrap();
    assert!(p.npcw.shop.pending, "the batch's first sale went out");
    let sent = p.session.sends_lost;
    window_takeover::set_use_game_windows(true);
    for _ in 0..3 {
        p.bags_broker_tick();
        native_events::dispatch(&host).unwrap();
    }
    let result = host.execute(
        "assert(#bres == 1, 'batch results: ' .. #bres); local r = bres[1]; \
         assert(r.state == 'stopped' and r.code == 'cancelled' and r.reason == 'stopped', \
         tostring(r.state) .. ' ' .. tostring(r.code) .. ' ' .. tostring(r.reason))",
    );
    let later_sends = p.session.sends_lost - sent;
    window_takeover::set_use_game_windows(false);
    window_takeover::reset_all_for_test();
    action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    match previous {
        Some(value) => unsafe { std::env::set_var("EQOA_CONFIG", value) },
        None => unsafe { std::env::remove_var("EQOA_CONFIG") },
    }
    let _ = std::fs::remove_dir_all(&config);
    assert!(kept, "the switch must hold after a restart");
    assert!(cleared, "turning it off must hold after a restart too");
    result.unwrap();
    assert_eq!(later_sends, 0, "no sale after the switch stopped the batch");
}
