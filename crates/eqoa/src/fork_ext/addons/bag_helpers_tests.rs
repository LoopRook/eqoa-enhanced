// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! A0 helper mods (2026-11-22): Sell junk, Restock and Bank marked in the Bag Viewer, tested without a window or a GPU.
//! The bundled `bags.lua` and `helpers.lua` are loaded as shipped into a plain Lua state with a fake `eqoa` and a clock the
//! test moves (the pattern of `autoloot_logic_tests.rs`). Each test has a negative control: the same check run against
//! the shipped source with one line of the feature changed (`mutated`), which must give the other answer.
use mlua::{Lua, Table};

pub(super) fn source(path: &str) -> String {
    let file = crate::fork_ext::bundled::EMBEDDED_FILES
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("the Bag Viewer ships {path}"));
    std::str::from_utf8(file.data).unwrap().to_string()
}

pub(super) const FIXTURE: &str = r##"
NOW = 100.0
os.clock = function() return NOW end
TALKS, BATCHES, SELLS, TAKEOVERS, RELEASED, LOG, CHAT, FILES, SENT_CHAT = {}, {}, {}, {}, {}, {}, {}, {}, {}
PL = {id = 77, name = "Hero", position = {x = 0, y = 0, z = 0}, fighting = false, auto_attacking = false, target_id = nil}
NPCS, MERCH, BANK, WINDOWS, LOOT = {}, nil, {open = false, items = {}, capacity = 40, used = 0, remaining = 40, revision = 1}, {any_window_open = false}, nil
INV_ITEMS, MONEY, CAPACITY, REV = {}, 1000, 40, 1
SELLP, BUYP, WARES = {}, {}, {}
TAKEN = {}
function inv_container()
  return {items = INV_ITEMS, money = MONEY, capacity = CAPACITY, used = #INV_ITEMS, remaining = math.max(CAPACITY - #INV_ITEMS, 0), revision = REV}
end
eqoa = {
  player = {get = function() return PL end},
  entities = {list = function() return NPCS end, find = function() return nil end},
  group = {get = function() return nil end},
  inventory = {get = inv_container},
  bank = {get = function() return BANK end},
  loot = {get = function() return LOOT end},
  merchant = {
    get = function() return MERCH end,
    price = function(ref, dir, qty)
      qty = qty or 1
      if dir == "sell" then return (SELLP[ref] or 5) * qty end
      return (BUYP[ref] or 10) * qty
    end,
  },
  items = {can_use = function() return {allowed = true, reasons = {}} end},
  windows = {
    get = function() return WINDOWS end,
    release = function(k) RELEASED[#RELEASED + 1] = k; TAKEN[k] = nil; return true end,
    takeover = function(k, opts) TAKEOVERS[#TAKEOVERS + 1] = {kind = k, scope = opts and opts.scope or "own"}; TAKEN[k] = true; return true end,
    taken = function(k) return TAKEN[k] == true end,
  },
  actions = {
    talk = function(entity, opts) TALKS[#TALKS + 1] = {kind = opts and opts.kind} return "talk" .. #TALKS end,
    sell = function(ref, qty) SELLS[#SELLS + 1] = {ref = ref, qty = qty} return "sell" .. #SELLS end,
    batch = function(spec) BATCHES[#BATCHES + 1] = spec return "batch" .. #BATCHES end,
    chat = function(text) SENT_CHAT[#SENT_CHAT + 1] = text return "chat" end,
    cancel = function() end,
    stop_batch = function() return true end,
  },
  log = {write = function(level, line) LOG[#LOG + 1] = line end},
  files = {read = function(name) return FILES[name] end, write = function(name, data) FILES[name] = data return true end},
}
eqoa.capabilities = {has = function(name) return WITH_CHAT == true and name == "eqoa.chat.local" end}
if WITH_CHAT then eqoa.chat = {["local"] = function(text, opts) CHAT[#CHAT + 1] = text return true end} end
package.loaded["bags"] = nil
B = BAGS_LOADER()
package.loaded["bags"] = B
H = HELPERS_LOADER()

function item(id, name, o)
  o = o or {}
  return {id = id, name = name, ref = "ref" .. id, quantity = o.q or 1, max_stack = o.max or 20, equipped = o.worn or false,
    quick_slot = o.quick, no_trade = o.nt or false, tabs = o.tabs or {"misc"}, list = "inventory", value = 10}
end
function ware(id, name, max) return {id = id, name = name, ref = "w" .. id, list = "merchant", max_stack = max or 20} end
function set_pack(items) INV_ITEMS = items; REV = REV + 1 end
function junk(...) for _, id in ipairs({...}) do B.junk_ids[tostring(id)] = true end end
-- a merchant and a banker, at a distance from the player (0,0,0)
function put_npcs(merchant_at, bank_at)
  NPCS = {}
  if merchant_at then NPCS[#NPCS + 1] = {id = 500, kind = "npc", name = "Merchant", npc_flags = 1, position = {x = merchant_at, y = 0, z = 0}} end
  if bank_at then NPCS[#NPCS + 1] = {id = 501, kind = "npc", name = "Banker", npc_flags = 2, position = {x = bank_at, y = 0, z = 0}} end
  B.scan_reset()
end
function later(s) NOW = NOW + s; B.scan_reset() end
-- the merchant's window opens (the server answered the talk)
function open_merchant(wares) WARES = wares or WARES; MERCH = {items = WARES, revision = 1, kind = "merchant"}; B.on_merchant_changed({cause = "open"}) end
function close_merchant() MERCH = nil; B.on_merchant_changed({cause = "close"}) end
function open_bank() BANK.open = true; B.on_bank_changed({cause = "open"}) end
function batch_ends(n, ok, extra)
  local d = {batch_id = B.batch_id, ok = ok ~= false, state = ok == false and "refused" or "completed", completed = n, total = #BATCHES[#BATCHES].steps}
  for k, v in pairs(extra or {}) do d[k] = v end
  B.on_batch_result(d)
end
function said(needle)
  for _, l in ipairs(CHAT) do if l:find(needle, 1, true) then return l end end
  return nil
end
function steps_of(i) return BATCHES[i or #BATCHES].steps end
"##;

fn world_with(extra: &str, mutations: &[(&str, &str)]) -> Lua {
    let lua = Lua::new();
    let mut bags = source("eqoa_bag_viewer/modules/bags.lua");
    let mut helpers = source("eqoa_bag_viewer/modules/helpers.lua");
    for (from, to) in mutations {
        let hit = bags.contains(from) || helpers.contains(from);
        assert!(hit, "the mutation matches nothing in the shipped source: {from}");
        bags = bags.replace(from, to);
        helpers = helpers.replace(from, to);
    }
    let bags_loader = lua
        .create_function(move |lua, ()| -> mlua::Result<Table> { lua.load(bags.as_str()).set_name("bags.lua").call(()) })
        .unwrap();
    let helpers_loader = lua
        .create_function(move |lua, ()| -> mlua::Result<Table> {
            lua.load(helpers.as_str()).set_name("helpers.lua").call(())
        })
        .unwrap();
    lua.globals().set("BAGS_LOADER", bags_loader).unwrap();
    lua.globals().set("HELPERS_LOADER", helpers_loader).unwrap();
    lua.load(extra).exec().unwrap();
    lua.load(FIXTURE).exec().unwrap();
    lua
}
fn world() -> Lua {
    world_with("", &[])
}
fn mutated(mutations: &[(&str, &str)]) -> Lua {
    world_with("", mutations)
}
fn chat_world() -> Lua {
    world_with("WITH_CHAT = true", &[])
}

fn int(lua: &Lua, expr: &str) -> i64 {
    lua.load(format!("return {expr}")).eval().unwrap()
}
fn boolean(lua: &Lua, expr: &str) -> bool {
    lua.load(format!("return {expr}")).eval().unwrap()
}
fn string(lua: &Lua, expr: &str) -> String {
    lua.load(format!("return {expr}")).eval().unwrap()
}
fn run(lua: &Lua, code: &str) {
    lua.load(code).exec().unwrap_or_else(|e| panic!("{e}\n{code}"));
}

/// A pack with marked junk of every kind (ordinary Misc, a quick-slotted potion, a no-trade charm, a sword, a worn ring) and
/// one stack that is not junk; a merchant within reach.
const JUNK_PACK: &str = r#"
    set_pack({
      item(1, "Rat Pelt", {q = 3}), item(2, "Chipped Tooth"),
      item(3, "Healing Potion", {q = 5, quick = 1, tabs = {"usable"}}),
      item(4, "Old Charm", {nt = true}),
      item(5, "Rusty Sword", {tabs = {"equipment"}}),
      item(6, "Worn Ring", {worn = true, tabs = {"equipment"}}),
      item(7, "Spider Silk", {q = 2}),
    })
    junk(1, 2, 3, 4, 5, 6)
    SELLP.ref1, SELLP.ref2 = 4, 20   -- Rat Pelt 4 each (x3 = 12), Chipped Tooth 20
    put_npcs(3.0, nil)
"#;

#[test]
fn line1_sell_junk_sells_everything_marked_gear_included_except_what_is_worn_and_says_what_it_sold() {
    let lua = chat_world();
    run(&lua, JUNK_PACK);
    // near the merchant, nothing open: one click talks to the merchant (a hidden session: scope own) ...
    assert!(boolean(&lua, "B.start_sell_junk()"), "the click starts");
    assert_eq!(int(&lua, "#TALKS"), 1, "it talks to the merchant itself");
    assert_eq!(string(&lua, "TALKS[1].kind"), "merchant");
    assert_eq!(string(&lua, "TAKEOVERS[1].kind .. ':' .. TAKEOVERS[1].scope"), "merchant:own", "hidden, and only this mod's own session");
    assert_eq!(int(&lua, "#BATCHES"), 0, "nothing is sold before the merchant answers");
    // ... and when it has opened, sells every marked stack (the potion, the charm and the sword too) but not the worn ring
    run(&lua, "open_merchant({})");
    assert_eq!(int(&lua, "#BATCHES"), 1);
    assert_eq!(string(&lua, "BATCHES[1].require_window"), "merchant");
    assert_eq!(int(&lua, "#steps_of(1)"), 5, "marking is the decision: pelts, tooth, potion, charm and sword");
    assert_eq!(string(&lua, "steps_of(1)[1].item .. steps_of(1)[2].item .. steps_of(1)[3].item .. steps_of(1)[4].item .. steps_of(1)[5].item"), "ref1ref2ref3ref4ref5");
    assert_eq!(int(&lua, "steps_of(1)[1].count"), 3, "the whole stack");
    run(&lua, "batch_ends(5)");
    assert_eq!(string(&lua, "said('Sold') "), "Sold 5 junk items for 67 tunar", "12 + 20 + 25 + 5 + 5 tunar, the game's quote");
    assert_eq!(string(&lua, "said('Kept')"), "Kept 1 junk item: you are wearing it");
    // the window's result line (what the player reads: `eqoa/1` has no chat line) carries the real final line, not "Sold 5 of 5"
    assert_eq!(
        string(&lua, "B.result.text"),
        "Sold 5 junk items for 67 tunar; Kept 1 junk item: you are wearing it",
        "fix round 2: the result line says the tunar total and the kept worn item"
    );
    assert!(boolean(&lua, "B.result.ok == true"));
    assert!(boolean(&lua, "RELEASED[#RELEASED] == 'merchant'"), "the hidden session ends with the batch");
    // the button counts what will really be sold
    assert_eq!(int(&lua, "select(2, B.junk_sell_list())"), 5);

    // negative control: with the worn-item exception off the worn ring is sold too (six steps, no "Kept" line)
    let bad = mutated(&[("if it.equipped then
                skipped[#skipped + 1]", "if false then
                skipped[#skipped + 1]")]);
    run(&bad, JUNK_PACK);
    run(&bad, "B.start_sell_junk(); open_merchant({})");
    assert_eq!(int(&bad, "#steps_of(1)"), 6, "negative control: the worn ring would be sold");
    // negative control (fix round 2): with the result line left as bags.lua set it, the player reads "Sold 5 of 5" only
    let bad = mutated(&[("B.result = {text = table.concat(parts, \"; \"), ok = ok}", "local _unused = 0")]);
    run(&bad, JUNK_PACK);
    run(&bad, "B.start_sell_junk(); open_merchant({}); batch_ends(5)");
    assert_eq!(string(&bad, "B.result.text"), "Sold 5 of 5", "negative control: the old line stays on the window");
    // negative control: the old rule (leave quick-slotted, no-trade and non-Misc items) sells only two of them
    let old = mutated(&[("if M.is_junk(it) then
            if it.equipped then", "if M.is_junk(it) and M.sell_risk(it) == nil then
            if it.equipped then")]);
    run(&old, JUNK_PACK);
    run(&old, "B.start_sell_junk(); open_merchant({})");
    assert_eq!(int(&old, "#steps_of(1)"), 2, "negative control: the previous rule sold only the two ordinary stacks");
}

/// Marking an item as junk: an ordinary Misc item is marked at once; equipment, a usable, a quick-slotted or a no-trade item
/// asks once ("Mark <name> as junk?", Yes / No); No leaves it unmarked.
const MARK_PACK: &str = r#"
    set_pack({
      item(1, "Rat Pelt", {q = 3}),
      item(2, "Rusty Sword", {tabs = {"equipment"}}),
      item(3, "Healing Potion", {q = 5, tabs = {"usable"}}),
      item(4, "Quick Pelt", {quick = 2}),
      item(5, "Old Charm", {nt = true}),
    })
    function mark_row(idx)
      B.build_menu(INV_ITEMS[idx])
      for _, r in ipairs(B.menu.rows) do if r.label == "Mark as junk" then return r end end
    end
"#;

#[test]
fn marking_gear_usables_quick_slotted_and_no_trade_items_as_junk_asks_once_and_misc_does_not() {
    let lua = world();
    run(&lua, MARK_PACK);
    // an ordinary Misc item is marked at once, with no question
    run(&lua, "B.trigger_menu(mark_row(1))");
    assert!(boolean(&lua, "B.is_junk(INV_ITEMS[1]) and not (B.menu and B.menu.asking)"), "Misc: marked at once");
    for (idx, what) in [(2, "equipment"), (3, "a usable"), (4, "a quick-slotted item"), (5, "a no-trade item")] {
        run(&lua, &format!("B.trigger_menu(mark_row({idx}))"));
        assert!(!boolean(&lua, &format!("B.is_junk(INV_ITEMS[{idx}])")), "{what}: not marked by the first click");
        assert!(boolean(&lua, "B.menu ~= nil and B.menu.asking == true"), "{what}: the question is up");
        let title = string(&lua, "B.menu.title");
        assert_eq!(title, format!("Mark {} as junk?", string(&lua, &format!("INV_ITEMS[{idx}].name"))));
        assert_eq!(int(&lua, "#B.menu.rows"), 2, "Yes and No");
        // No leaves it unmarked
        run(&lua, "B.trigger_menu(B.menu.rows[2])");
        assert!(!boolean(&lua, &format!("B.is_junk(INV_ITEMS[{idx}])")), "{what}: No leaves it unmarked");
        // Yes marks it
        run(&lua, &format!("B.trigger_menu(mark_row({idx})); B.trigger_menu(B.menu.rows[1])"));
        assert!(boolean(&lua, &format!("B.is_junk(INV_ITEMS[{idx}])")), "{what}: Yes marks it");
    }
    // unmarking never asks
    run(&lua, "B.build_menu(INV_ITEMS[2]); for _, r in ipairs(B.menu.rows) do if r.label == 'Unmark' then ROW = r end end; B.trigger_menu(ROW)");
    assert!(!boolean(&lua, "B.is_junk(INV_ITEMS[2])"));

    // negative control: without the question the sword is marked by the first click
    let bad = mutated(&[("if row.mark_ask and not row.confirmed and row.enabled and row.run then", "if false then")]);
    run(&bad, MARK_PACK);
    run(&bad, "B.trigger_menu(mark_row(2))");
    assert!(boolean(&bad, "B.is_junk(INV_ITEMS[2])"), "negative control: marked at once with the question off");
    // negative control: with every item asked, a Misc item would also ask
    let bad = mutated(&[("rows[#rows].mark_ask = M.sell_risk(it) ~= nil or nil", "rows[#rows].mark_ask = true")]);
    run(&bad, MARK_PACK);
    run(&bad, "B.trigger_menu(mark_row(1))");
    assert!(!boolean(&bad, "B.is_junk(INV_ITEMS[1])"), "negative control: the Misc item would ask too");
}

#[test]
fn line1_without_chat_support_the_line_goes_to_the_result_line_and_the_log_never_to_the_server() {
    // `eqoa/1` has no local chat line yet: the line must not be sent as chat (that would say it to the world)
    let lua = world();
    run(&lua, JUNK_PACK);
    run(&lua, "B.start_sell_junk(); open_merchant({}); batch_ends(5)");
    assert!(boolean(&lua, "H.chat[1] == 'Sold 5 junk items for 67 tunar'"));
    assert!(boolean(&lua, "(function() for _, l in ipairs(LOG) do if l:find('Sold 5 junk items for 67 tunar', 1, true) then return true end end end)()"));
    assert_eq!(int(&lua, "#SENT_CHAT"), 0, "never sent to the server as a chat line: that would say it to the world");
}

#[test]
fn line2_restock_buys_up_to_the_mark_counting_the_bags_only_what_the_merchant_sells_and_stops_on_money_or_room() {
    let setup = r#"
        set_pack({item(10, "Healing Potion", {q = 5, max = 20}), item(11, "Bread", {q = 12}), item(12, "Rat Pelt", {q = 1})})
        H.keep["10"], H.keep["11"], H.keep["13"] = 20, 10, 100     -- 13 is arrows: this merchant does not sell them
        BUYP.w10 = 10
        put_npcs(3.0, nil)
        WARES = {ware(10, "Healing Potion", 20), ware(11, "Bread", 20), ware(14, "Boots", 1)}
    "#;
    let lua = chat_world();
    run(&lua, setup);
    assert!(boolean(&lua, "H.start_restock()"));
    assert_eq!(string(&lua, "TALKS[1].kind"), "merchant");
    run(&lua, "open_merchant()");
    assert_eq!(int(&lua, "#BATCHES"), 1);
    assert_eq!(string(&lua, "BATCHES[1].require_window"), "merchant");
    // 20 kept, 5 held: 15 potions (one stack's worth); bread is held 12 >= 10 (nothing); boots and arrows are not marked / not sold
    assert_eq!(int(&lua, "#steps_of(1)"), 1);
    assert_eq!(string(&lua, "steps_of(1)[1].kind .. steps_of(1)[1].item"), "buyw10");
    assert_eq!(int(&lua, "steps_of(1)[1].count"), 15, "up to the mark, never more");
    run(&lua, "batch_ends(1)");
    assert_eq!(string(&lua, "said('Restocked')"), "Restocked: bought 15 items for 150 tunar");
    assert!(boolean(&lua, "RELEASED[#RELEASED] == 'merchant'"));

    // money runs out: 60 tunar buys 6 potions, and the line says so
    let lua = chat_world();
    run(&lua, setup);
    run(&lua, "MONEY = 60; H.start_restock(); open_merchant()");
    assert_eq!(int(&lua, "steps_of(1)[1].count"), 6);
    run(&lua, "batch_ends(1)");
    let line = string(&lua, "said('Restock stopped')");
    assert!(line.contains("out of money") && line.contains("Healing Potion"), "{line}");

    // no room: the potion stack is full except for 2, and the bags have no free slot
    let lua = chat_world();
    run(&lua, setup);
    run(&lua, "CAPACITY = 3; set_pack({item(10, 'Healing Potion', {q = 18, max = 20}), item(11, 'Bread', {q = 12}), item(12, 'Rat Pelt')}); H.keep['10'] = 40; H.start_restock(); open_merchant()");
    assert_eq!(int(&lua, "steps_of(1)[1].count"), 2, "only what fits");
    run(&lua, "batch_ends(1)");
    assert!(string(&lua, "said('Restock stopped')").contains("bags are full"));

    // a merchant that sells none of the kept items: nothing bought, said
    let lua = chat_world();
    run(&lua, setup);
    run(&lua, "H.start_restock(); open_merchant({ware(99, 'Gem', 5)})");
    assert_eq!(int(&lua, "#BATCHES"), 0);
    assert!(boolean(&lua, "said('sells none of the items you keep') ~= nil"));
    assert!(boolean(&lua, "RELEASED[#RELEASED] == 'merchant'"), "the hidden session is closed again");

    // negative control: without counting the bags the whole mark is bought
    let bad = mutated(&[("local need = keep - (owned[k] or 0)", "local need = keep")]);
    run(&bad, setup);
    run(&bad, "H.start_restock(); open_merchant()");
    assert_eq!(int(&bad, "steps_of(1)[1].count"), 20, "negative control: the bags' stack is not counted");
}

#[test]
fn line3_bank_marked_deposits_only_marked_stacks_near_a_banker_and_says_so() {
    let setup = r#"
        set_pack({item(20, "Moonstone", {q = 4}), item(21, "Rat Pelt"), item(22, "Gold Ring", {worn = true, tabs = {"equipment"}}), item(23, "Silk", {q = 9})})
        H.bank["20"], H.bank["22"], H.bank["23"] = true, true, true  -- the ring is worn: never banked
        put_npcs(nil, 4.0)
        BANK.items, BANK.used, BANK.remaining = {}, 0, 40
    "#;
    let lua = chat_world();
    run(&lua, setup);
    assert_eq!(int(&lua, "H.bank_count()"), 2, "the button counts the two stacks");
    assert!(boolean(&lua, "H.start_bank()"));
    assert_eq!(string(&lua, "TALKS[1].kind"), "bank");
    assert_eq!(string(&lua, "TAKEOVERS[1].kind .. ':' .. TAKEOVERS[1].scope"), "bank:own");
    run(&lua, "open_bank()");
    assert_eq!(int(&lua, "#BATCHES"), 1);
    assert_eq!(string(&lua, "BATCHES[1].require_window"), "bank");
    assert_eq!(int(&lua, "#steps_of(1)"), 2);
    assert_eq!(string(&lua, "steps_of(1)[1].kind .. steps_of(1)[1].item .. steps_of(1)[1].count .. steps_of(1)[1].from"), "bank_moveref204inventory");
    assert_eq!(string(&lua, "steps_of(1)[2].item"), "ref23");
    run(&lua, "batch_ends(2)");
    assert_eq!(string(&lua, "said('Banked')"), "Banked 2 items");
    assert!(boolean(&lua, "RELEASED[#RELEASED] == 'bank'"));

    // the bank has room for one more stack only: one is banked, the other said to be left
    let lua = chat_world();
    run(&lua, setup);
    run(&lua, "BANK.remaining = 1; H.start_bank(); open_bank()");
    assert_eq!(int(&lua, "#steps_of(1)"), 1);
    run(&lua, "batch_ends(1)");
    assert!(boolean(&lua, "said('The bank is full: 1 item left in your bags') ~= nil"));

    // negative control: with the marks ignored, every unworn stack goes
    let bad = mutated(&[("if H.is_banked(it) and not it.equipped", "if not it.equipped")]);
    run(&bad, setup);
    run(&bad, "H.start_bank(); open_bank()");
    assert_eq!(int(&bad, "#steps_of(1)"), 3, "negative control: the unmarked Rat Pelt is banked too");
}

#[test]
fn line4_a_merchant_you_talk_to_yourself_opens_the_games_window_and_helpers_act_only_on_their_button() {
    let lua = world();
    run(&lua, JUNK_PACK);
    run(&lua, "H.keep['1'] = 9; H.bank['1'] = true");
    // all three automatic settings are off by default, for every character: standing at the merchant does nothing
    assert!(!boolean(&lua, "H.any_auto()") && !boolean(&lua, "H.auto_on('sell') or H.auto_on('restock') or H.auto_on('bank')"));
    run(&lua, "for i = 1, 8 do later(1); H.auto_poll(true) end");
    assert_eq!(int(&lua, "#TALKS + #BATCHES + #TAKEOVERS"), 0, "nothing is done unasked");
    // the player talks to the merchant: the game's own window opens, and no helper takes it over or acts on it
    run(&lua, "open_merchant({ware(1, 'Rat Pelt')})");
    run(&lua, "for i = 1, 8 do later(1); H.auto_poll(true) end");
    assert_eq!(int(&lua, "#TALKS + #BATCHES + #TAKEOVERS + #RELEASED"), 0, "a player-opened merchant window is left alone");
    // every claim the helpers ever make is scope own (it hides only the mod's own sessions)
    let lua = world();
    run(&lua, JUNK_PACK);
    run(&lua, "H.keep['1'] = 9; B.start_sell_junk(); H.start_restock()");
    assert!(boolean(&lua, "(function() for _, t in ipairs(TAKEOVERS) do if t.scope ~= 'own' then return false end end return #TAKEOVERS > 0 end)()"));

    // negative control: with the automatic choice on, standing near the merchant does talk to it
    let lua = world();
    run(&lua, JUNK_PACK);
    run(&lua, "H.set_auto('sell', true); later(1); H.auto_poll(true)");
    assert_eq!(int(&lua, "#TALKS"), 1, "negative control: the helper acts once the player chose it");
}

#[test]
fn line4_automatic_helpers_default_off_act_once_per_approach_and_wait_for_a_quiet_moment() {
    let lua = world();
    run(&lua, JUNK_PACK);
    run(&lua, "H.set_auto('sell', true)");
    assert!(boolean(&lua, "H.auto_on('sell') and not H.auto_on('restock') and not H.auto_on('bank')"));
    // standing near: it acts once ...
    run(&lua, "later(1); H.auto_poll(true)");
    assert_eq!(int(&lua, "#TALKS"), 1);
    run(&lua, "open_merchant({}); batch_ends(2)");
    assert_eq!(int(&lua, "#BATCHES"), 1);
    run(&lua, "close_merchant()");
    // ... and not again while the player keeps standing there (junk marked again does not make it loop)
    run(&lua, "set_pack({item(1, 'Rat Pelt')}); junk(1); for i = 1, 20 do later(1); H.auto_poll(true) end");
    assert_eq!(int(&lua, "#TALKS"), 1, "once per approach");
    // walking off beyond the reach and coming back arms it again
    run(&lua, "put_npcs(60.0, nil); later(1); H.auto_poll(true); put_npcs(3.0, nil); later(1); H.auto_poll(true)");
    assert_eq!(int(&lua, "#TALKS"), 2, "a new approach");

    // never while a game window is up, never in a fight, never with a window of the game's own open
    for (setup, why) in [
        ("WINDOWS = {any_window_open = true}", "a game window"),
        ("WINDOWS = {start_menu_open = true}", "the start menu"),
        ("MERCH = {items = {}, revision = 1}", "a merchant window"),
        ("LOOT = {items = {}, open = true}", "a corpse window"),
        ("NPCS[#NPCS + 1] = {id = 900, kind = 'npc', name = 'Gnoll', target_id = 77, position = {x = 5, y = 0, z = 0}, disposition_name = 'Hostile'}", "a fight"),
    ] {
        let lua = world();
        run(&lua, JUNK_PACK);
        run(&lua, "H.set_auto('sell', true)");
        run(&lua, setup);
        run(&lua, "later(1); H.auto_poll(true)");
        assert_eq!(int(&lua, "#TALKS"), 0, "not while {why}");
    }
    // negative control: the same state without any of those lets it act
    let lua = world();
    run(&lua, JUNK_PACK);
    run(&lua, "H.set_auto('sell', true); later(1); H.auto_poll(true)");
    assert_eq!(int(&lua, "#TALKS"), 1);
    // negative control: without the once-per-approach mark it talks again on every look
    let bad = mutated(&[("                        H.armed[key] = false\n                        H.note_forget(key .. \":wait\")", "                        H.armed[key] = nil\n                        H.note_forget(key .. \":wait\")")]);
    run(&bad, JUNK_PACK);
    run(&bad, "H.set_auto('sell', true); later(1); H.auto_poll(true); B.pending_open = nil; later(1); H.auto_poll(true)");
    assert_eq!(int(&bad, "#TALKS"), 2, "negative control: it would loop while standing there");
}

#[test]
fn line4_marks_and_the_automatic_choices_are_saved_and_come_back() {
    let lua = world();
    run(&lua, "set_pack({item(1, 'Rat Pelt', {q = 3})}); H.start_keep(INV_ITEMS[1]); H.set_bank(INV_ITEMS[1], true); H.set_auto('restock', true); H.set_auto('bank', true)");
    assert_eq!(int(&lua, "H.keep['1']"), 3, "the first mark is what the bags hold");
    run(&lua, "H.change_keep(INV_ITEMS[1], 5)");
    assert_eq!(int(&lua, "H.keep['1']"), 8);
    let file = string(&lua, "FILES['helpers.cfg']");
    assert!(file.contains("keep:1=8\n") && file.contains("bank:1\n"), "{file}");
    assert!(file.contains("auto:Hero:restock=1") && file.contains("auto:Hero:bank=1") && !file.contains("sell"), "{file}");
    // a fresh start reads the same file back; another character has none of the automatic choices
    run(&lua, "H.keep, H.bank, H.auto_by_char = {}, {}, {}; H.load()");
    assert_eq!(int(&lua, "H.keep['1']"), 8);
    assert!(boolean(&lua, "H.is_banked({id = 1}) and H.auto_on('restock') and H.auto_on('bank') and not H.auto_on('sell')"));
    run(&lua, "B.character = 'Other'");
    assert!(!boolean(&lua, "H.auto_on('restock')"), "the automatic choice belongs to a character");
    // a file with unknown lines (a newer version) is read without harm
    run(&lua, "FILES['helpers.cfg'] = 'keep:5=2\\nfuture:thing\\nauto:Hero:time_travel=1\\nbank:7\\n'; H.load()");
    assert_eq!(int(&lua, "H.keep['5']"), 2);
    assert!(boolean(&lua, "H.is_banked({id = 7}) and not H.any_auto()"));
    // negative control: without saving nothing comes back
    let bad = mutated(&[("eqoa.files.write(FILE,", "(function() end)(FILE,")]);
    run(&bad, "set_pack({item(1, 'Rat Pelt', {q = 3})}); H.start_keep(INV_ITEMS[1]); H.keep = {}; H.load()");
    assert_eq!(int(&bad, "H.keep['1'] or 0"), 0, "negative control: an unsaved mark is gone after a reload");
}

#[test]
fn line5_sell_by_hand_asks_first_for_quick_slotted_no_trade_and_non_ordinary_items() {
    let setup = r#"
        set_pack({
          item(30, "Healing Potion", {q = 5, quick = 2, tabs = {"usable"}}),
          item(31, "Old Charm", {nt = true}),
          item(32, "Rusty Sword", {tabs = {"equipment"}}),
          item(33, "Rat Pelt", {q = 3}),
          item(34, "Warded Ring", {nt = true, quick = 1, tabs = {"usable"}}),
        })
        put_npcs(3.0, nil)
        open_merchant({})
    "#;
    // each risky item: the first click asks (nothing is sold), No keeps it, Yes sells it
    for (idx, id, why) in [
        (1, 30, "quick-slotted"),
        (2, 31, "no-trade"),
        (3, 32, "not an ordinary item"),
        (5, 34, "quick-slotted, no-trade and not an ordinary item"),
    ] {
        let lua = world();
        run(&lua, setup);
        run(&lua, &format!("local it = INV_ITEMS[{idx}]; B.build_menu(it); ROW = nil; for _, r in ipairs(B.menu.rows) do if r.label:find('^Sell 1') then ROW = r end end"));
        assert!(boolean(&lua, "ROW ~= nil and ROW.enabled"), "{id}: Sell 1 is offered");
        run(&lua, "B.trigger_menu(ROW)");
        assert_eq!(int(&lua, "#SELLS"), 0, "{id}: the first click sells nothing");
        assert_eq!(int(&lua, "#TALKS"), 0);
        assert!(boolean(&lua, "B.menu ~= nil and B.menu.asking == true"), "{id}: a question is up");
        let title = string(&lua, "B.menu.title");
        let yes = string(&lua, "B.menu.rows[1].text");
        assert!(title.contains(&string(&lua, &format!("INV_ITEMS[{idx}].name"))) && title.ends_with('?'), "{title}");
        assert!(yes.contains(why), "the question says why: {yes}");
        // No keeps it
        run(&lua, "B.trigger_menu(B.menu.rows[2])");
        assert_eq!(int(&lua, "#SELLS"), 0, "{id}: No sells nothing");
        // Yes sells it, by the one action
        run(&lua, &format!("B.build_menu(INV_ITEMS[{idx}]); for _, r in ipairs(B.menu.rows) do if r.label:find('^Sell 1') then ROW = r end end; B.trigger_menu(ROW); B.trigger_menu(B.menu.rows[1])"));
        assert_eq!(int(&lua, "#SELLS"), 1, "{id}: Yes sells it");
        assert_eq!(int(&lua, "SELLS[1].qty"), 1);
    }
    // an ordinary item sells at once, with no question
    let lua = world();
    run(&lua, setup);
    run(&lua, "B.build_menu(INV_ITEMS[4]); for _, r in ipairs(B.menu.rows) do if r.label:find('^Sell stack') then ROW = r end end; B.trigger_menu(ROW)");
    assert_eq!(int(&lua, "#SELLS"), 1, "a misc item that is neither quick-slotted nor no-trade sells at once, as in the game");
    assert_eq!(int(&lua, "SELLS[1].qty"), 3);

    // negative control: with the question switched off the first click sells the quick-slotted potion
    let bad = mutated(&[("if row.confirm and not row.confirmed and row.run", "if false and row.run")]);
    run(&bad, setup);
    run(&bad, "B.build_menu(INV_ITEMS[1]); for _, r in ipairs(B.menu.rows) do if r.label:find('^Sell 1') then ROW = r end end; B.trigger_menu(ROW)");
    assert_eq!(int(&bad, "#SELLS"), 1, "negative control: no question, the potion is sold on the first click");
}

#[test]
fn the_automatic_helpers_do_not_listen_to_the_world_unless_one_is_on() {
    // the render poll is only registered by main.lua while some character has an automatic helper on
    let lua = world();
    assert!(!boolean(&lua, "H.any_auto()"));
    run(&lua, "H.set_auto('bank', true)");
    assert!(boolean(&lua, "H.any_auto()"));
    run(&lua, "H.set_auto('bank', false)");
    assert!(!boolean(&lua, "H.any_auto()"), "turning it off again leaves nothing on");
    assert_eq!(string(&lua, "FILES['helpers.cfg']"), "", "and nothing saved");
}

const RESTOCK_SETUP: &str = r#"
    set_pack({item(10, "Healing Potion", {q = 5, max = 20}), item(11, "Bread", {q = 12}), item(12, "Rat Pelt", {q = 1})})
    H.keep["10"] = 20
    BUYP.w10 = 10
    put_npcs(3.0, nil)
    WARES = {ware(10, "Healing Potion", 20)}
"#;

const DROP_RESULT: (&str, &str) = ("B.result = {text = table.concat(parts, \"; \"), ok = ok}", "local _unused = 0");

#[test]
fn fix2_item1_a_partial_restock_or_a_full_bank_shows_its_reason_on_the_result_line_not_a_green_bought() {
    let lua = world();
    run(&lua, RESTOCK_SETUP);
    run(&lua, "MONEY = 60; H.start_restock(); open_merchant(); batch_ends(1)");
    let text = string(&lua, "B.result.text");
    assert!(text.contains("out of money") && text.contains("Healing Potion"), "{text}");
    assert!(boolean(&lua, "B.result.ok == false"), "the not-ok colour, never a green 'Bought 1 of 1'");
    // the bank
    let lua = world();
    run(
        &lua,
        r#"
        set_pack({item(20, "Moonstone", {q = 4}), item(23, "Silk", {q = 9})})
        H.bank["20"], H.bank["23"] = true, true
        put_npcs(nil, 4.0)
        BANK.items, BANK.used, BANK.remaining = {}, 0, 1
        H.start_bank(); open_bank(); batch_ends(1)
    "#,
    );
    assert_eq!(string(&lua, "B.result.text"), "Banked 1 item; The bank is full: 1 item left in your bags");
    assert!(boolean(&lua, "B.result.ok == false"));
    // negative control: without the joined line the window keeps "Bought 1 of 1" in green
    let bad = mutated(&[DROP_RESULT]);
    run(&bad, RESTOCK_SETUP);
    run(&bad, "MONEY = 60; H.start_restock(); open_merchant(); batch_ends(1)");
    assert_eq!(string(&bad, "B.result.text"), "Bought 1 of 1");
    assert!(boolean(&bad, "B.result.ok == true"), "negative control: green, with the reason missing");
}

#[test]
fn fix2_item2a_sell_junk_during_another_batch_refuses_and_leaves_that_batchs_session_alone() {
    let run_it = |lua: &Lua| {
        run(lua, JUNK_PACK);
        run(lua, "B.start_sell_junk(); open_merchant({})"); // a Sell junk batch is running (merchant session held)
        run(lua, "N_RELEASED, N_BATCHES = #RELEASED, #BATCHES; RESULT = B.start_sell_junk()");
    };
    let lua = world();
    run_it(&lua);
    assert!(!boolean(&lua, "RESULT"), "the second click is refused");
    assert_eq!(int(&lua, "#RELEASED - N_RELEASED"), 0, "the running batch's merchant session is not released");
    assert_eq!(int(&lua, "#BATCHES - N_BATCHES"), 0, "no second batch");
    assert!(boolean(&lua, "TAKEN.merchant == true"));
    // negative control: without the guard the second click starts another batch over the running one
    let bad = mutated(&[(
        "    if M.batch_id then return false end\n    local merch = eqoa.merchant.get()",
        "    local merch = eqoa.merchant.get()",
    )]);
    run_it(&bad);
    assert_eq!(int(&bad, "#BATCHES - N_BATCHES"), 1, "negative control: a second batch is started");
}

#[test]
fn fix2_item2b_restock_and_bank_marked_during_another_batch_never_talk_or_open_a_hidden_session() {
    let setup = r#"
        set_pack({item(10, "Healing Potion", {q = 5}), item(20, "Moonstone")})
        H.keep["10"] = 20; H.bank["20"] = true
        put_npcs(3.0, 4.0)
        B.batch_id, B.batch_kind = "loot-batch", "loot"
    "#;
    let lua = world();
    run(&lua, setup);
    run(&lua, "RES = {H.start_restock(), H.start_bank()}");
    assert!(!boolean(&lua, "RES[1] or RES[2]"));
    assert_eq!(int(&lua, "#TALKS + #TAKEOVERS"), 0, "nothing is talked to or held while the other batch runs");
    assert!(boolean(&lua, "B.pending_open == nil"));
    // negative control: with the checks after the talk, both talk and hold a session nothing would close
    let bad = mutated(&[
        (
            "    if B.batch_id then return false end -- before any talk: a hidden session nothing would close\n    local merch",
            "    local merch",
        ),
        (
            "    if B.batch_id then return false end -- before any talk: a hidden session nothing would close\n    local bank",
            "    local bank",
        ),
    ]);
    run(&bad, setup);
    run(&bad, "H.start_restock(); B.pending_open = nil; H.start_bank()");
    assert_eq!(int(&bad, "#TALKS"), 2, "negative control: both talk");
}

#[test]
fn fix2_item2c_closing_the_bag_viewer_forgets_a_pending_talk_so_a_late_reply_does_nothing() {
    let main = source("eqoa_bag_viewer/main.lua");
    let drop_at = main.find("B.drop_pending_npc_open()").expect("close() drops the pending talk");
    let release_at = main.find("eqoa.windows.release(\"merchant\")").unwrap();
    assert!(drop_at < release_at, "the pending talk is cleared before the sessions are released");
    let flow = r#"
        set_pack({item(10, "Healing Potion", {q = 5}), item(20, "Moonstone")})
        H.keep["10"] = 20
        put_npcs(3.0, nil)
        H.start_restock()                    -- talk sent, the reply not here yet
        PENDING_BEFORE = B.pending_open ~= nil
        B.drop_pending_npc_open()            -- what close() does first
        eqoa.windows.release("merchant")
        open_merchant({ware(10, "Healing Potion", 20)}) -- the late reply
    "#;
    let lua = world();
    run(&lua, flow);
    assert!(boolean(&lua, "PENDING_BEFORE"));
    assert!(boolean(&lua, "B.pending_open == nil"));
    assert_eq!(int(&lua, "#BATCHES"), 0, "the late reply does not run the restock step");
    // negative control: without the clearing the late reply runs the step
    let bad = mutated(&[("if not p or p.kind == \"loot\" then return end", "do return end")]);
    run(&bad, flow);
    assert_eq!(int(&bad, "#BATCHES"), 1, "negative control: the step runs after the window was closed");
}

#[test]
fn fix2_item3_one_ware_that_cannot_be_bought_does_not_stop_the_others_and_a_cut_plan_costs_what_it_buys() {
    let setup = r#"
        set_pack({item(12, "Rat Pelt", {q = 1})})
        H.keep["10"], H.keep["11"] = 20, 5
        BUYP.w10, BUYP.w11 = 100, 10          -- potions cost more than the money; bread does not
        MONEY = 60
        WARES = {ware(10, "Healing Potion", 20), ware(11, "Bread", 20)}
        PLAN = H.restock_plan({items = WARES}, eqoa.inventory.get())
    "#;
    let lua = world();
    run(&lua, setup);
    assert_eq!(int(&lua, "#PLAN.steps"), 1, "the bread is still planned");
    assert_eq!(string(&lua, "PLAN.steps[1].item"), "w11");
    assert_eq!(string(&lua, "table.concat(PLAN.short, ',')"), "Healing Potion");
    assert_eq!(string(&lua, "PLAN.limit"), "out of money");
    // negative control: stopping at the first limit drops the bread
    let bad = mutated(&[("        if keep then\n", "        if keep and not plan.limit then\n")]);
    run(&bad, setup);
    assert_eq!(int(&bad, "#PLAN.steps"), 0, "negative control: the later ware is never planned");

    // a plan cut at 64 steps is costed for the 64 it keeps
    let cut = r#"
        set_pack({item(12, "Rat Pelt", {q = 1})})
        CAPACITY, MONEY = 500, 100000
        H.keep["10"] = 70
        BUYP.w10 = 10
        PLAN = H.restock_plan({items = {ware(10, "Arrow", 1)}}, eqoa.inventory.get())
    "#;
    let lua = world();
    run(&lua, cut);
    assert_eq!(int(&lua, "#PLAN.steps"), 64);
    assert_eq!(int(&lua, "PLAN.total"), 64);
    assert_eq!(int(&lua, "PLAN.cost"), 640, "64 arrows at 10, not 70");
    let bad = mutated(&[(
        "                        cost = eqoa.merchant.price(ware.ref, \"buy\", n) or ((unit or 0) * n)\n",
        "",
    )]);
    run(&bad, cut);
    assert_eq!(int(&bad, "PLAN.cost"), 700, "negative control: the unclipped cost");
}
