// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Bag Viewer: merchants and bankers in the window (the option "Merchants in Bag Viewer", 2026-11-27).
//! Tested without a window or a GPU: the shipped `bags.lua`, `helpers.lua`, `multi.lua` and `trade.lua` are loaded into a
//! plain Lua state with the fake `eqoa` of `bag_helpers_tests.rs` (its fixture) and a few more calls the module uses. Each
//! test has a negative control: the same scenario against the shipped source with one line of the feature changed
//! (`mutated`), which must give the other answer. The window itself (the drawn panel, real clicks, Esc, the pictures) is
//! `bag_viewer_tests.rs`'s merchant and bank test, which needs a GPU. Set `EQOA_TRADE_TEST_SOURCE` to a copy of the
//! package folder to run these against that instead of the embedded bytes.
use super::bag_helpers_tests::FIXTURE;
use mlua::{Lua, Table};

fn source(file: &str) -> String {
    match std::env::var_os("EQOA_TRADE_TEST_SOURCE") {
        Some(dir) => std::fs::read_to_string(std::path::PathBuf::from(dir).join(file)).unwrap(),
        None => super::bag_helpers_tests::source(&format!("eqoa_bag_viewer/{file}")),
    }
}

const FILES: [&str; 4] = ["modules/bags.lua", "modules/helpers.lua", "modules/multi.lua", "modules/trade.lua"];

fn world_with(mutations: &[(&str, &str)]) -> Lua {
    let lua = Lua::new();
    let mut sources: Vec<String> = FILES.iter().map(|f| source(f)).collect();
    for (from, to) in mutations {
        assert!(sources.iter().any(|s| s.contains(from)), "the mutation matches nothing in the shipped source: {from}");
        for s in sources.iter_mut() {
            *s = s.replace(from, to);
        }
    }
    let names = ["BAGS_LOADER", "HELPERS_LOADER", "MULTI_LOADER", "TRADE_LOADER"];
    for (i, code) in sources.into_iter().enumerate() {
        let name = FILES[i];
        let f = lua
            .create_function(move |lua, ()| -> mlua::Result<Table> { lua.load(code.as_str()).set_name(name).call(()) })
            .unwrap();
        lua.globals().set(names[i], f).unwrap();
    }
    lua.load(FIXTURE).exec().unwrap();
    lua.load(EXTRA).exec().unwrap();
    lua
}
fn world() -> Lua {
    world_with(&[])
}
fn mutated(mutations: &[(&str, &str)]) -> Lua {
    world_with(mutations)
}

fn string(lua: &Lua, expr: &str) -> String {
    lua.load(format!("return tostring({expr})")).eval().unwrap_or_else(|e| panic!("{expr}\n{e}"))
}
fn int(lua: &Lua, expr: &str) -> i64 {
    lua.load(format!("return {expr}")).eval().unwrap_or_else(|e| panic!("{expr}\n{e}"))
}
fn boolean(lua: &Lua, expr: &str) -> bool {
    lua.load(format!("return {expr}")).eval().unwrap_or_else(|e| panic!("{expr}\n{e}"))
}
fn run(lua: &Lua, code: &str) {
    lua.load(code).exec().unwrap_or_else(|e| panic!("{e}\n{code}"));
}

/// What the module needs beyond the helpers' fixture: the option, the keyboard, the held windows' sessions, the two actions,
/// items with a server key, a merchant that is talked to and a banker, and the viewer's own `open` (counted).
const EXTRA: &str = r##"
OPTIONS, OPTION_ROWS, INPUT = {}, {}, {claims = {}, released = {}, text = false}
eqoa.options = {
  toggle = function(o) OPTION_ROWS[#OPTION_ROWS + 1] = o return {id = o.id} end,
  get = function(id) return OPTIONS[id] end,
  set = function(id, v) OPTIONS[id] = v return true end,
}
eqoa.input = {
  claim = function(key) INPUT.claims[#INPUT.claims + 1] = key return #INPUT.claims end,
  release = function(token) INPUT.released[#INPUT.released + 1] = token end,
  capture_text = function(on) INPUT.text = on end,
}
SESSIONS, PASSED, BUYS, MOVES = {}, {}, {}, {}
PASS_ANSWER = true
EVENTS = {}
eqoa.events = {on = function(name, fn) EVENTS[name] = EVENTS[name] or {}; table.insert(EVENTS[name], fn) return #EVENTS[name] end}
function emit(name, data) for _, fn in ipairs(EVENTS[name] or {}) do fn({data = data}) end end
eqoa.windows.session = function(kind) return SESSIONS[kind] end
eqoa.windows.pass = function(ref)
  if PASS_ANSWER ~= true then return nil, PASS_ANSWER end
  PASSED[#PASSED + 1] = ref
  return true
end
-- an item by its handle, as the client answers: the stack as it is now, or stale_handle
eqoa.items.get = function(ref)
  for _, list in ipairs({INV_ITEMS, WARES or {}, BANK.items or {}}) do
    for _, it in ipairs(list) do if it.ref == ref then return it end end
  end
  return nil, {code = "stale_handle"}
end
eqoa.windows.confirm = function() return nil end
eqoa.actions.buy = function(ref, n) BUYS[#BUYS + 1] = {ref = ref, n = n} return "buy" .. #BUYS end
eqoa.actions.bank_move = function(from, ref, n) MOVES[#MOVES + 1] = {from = from, ref = ref, n = n} return "move" .. #MOVES end
X = MULTI_LOADER()
package.loaded["multi"] = X
T = TRADE_LOADER()
package.loaded["trade"] = T
OPENED = 0
-- the viewer's own `open` (main.lua): the window is up on the All tab
T.open_viewer = function() OPENED = OPENED + 1; B.open = true; B.tab = 1; SESSIONS.inventory = {ref = "inv"} end
T.close_viewer = function() B.open = false; SESSIONS.inventory = nil end

function it_(id, name, o)
  local t = item(id, name, o)
  t.inventory_key = id
  return t
end
function bank_it(id, name, q)
  return {id = id, name = name, ref = "bank" .. id, quantity = q or 1, max_stack = 20, equipped = false, tabs = {"misc"},
    list = "bank", inventory_key = id}
end
function ware_(id, name, max)
  return {id = id, name = name, ref = "w" .. id, list = "merchant", inventory_key = id, max_stack = max or 1, quantity = 1}
end
OPTIONS_OF_MERCHANT = {{key = "buy", selected = true}, {key = "sell", selected = false}, {key = "exit", selected = false}}
-- the player talks to a merchant: its list appears (the always-on merchant.changed handler of main.lua calls this first)
function talk_merchant(wares, options)
  WARES = wares
  MERCH = {kind = "merchant", items = wares, revision = (MERCH and MERCH.revision or 0) + 1,
    merchant = {entity_id = ENTITY or 500, mode = "menu", pending = false, options = options or OPTIONS_OF_MERCHANT}}
  T.on_merchant({before = nil, after = MERCH, cause = "open"})
end
function banker_talk(items, money)
  BANK = {kind = "bank", open = true, items = items, money = money or 500, used = #items, capacity = 40, remaining = 40 - #items, revision = 2}
  T.on_bank({before = {open = false}, after = BANK, cause = "open"})
end
function key(k, text) return T.on_key({down = true, key = k, text = text}) end
function type_digits(s) for d in s:gmatch("%d") do key(d, d) end end
function taken_scope(kind)
  local scope
  for _, t in ipairs(TAKEOVERS) do if t.kind == kind then scope = t.scope end end
  return scope
end
function released(kind) for _, k in ipairs(RELEASED) do if k == kind then return true end end return false end
"##;

fn viewer_closed(lua: &Lua) {
    run(lua, "OPTIONS.open_npc = true; T.poll(true); B.open = false; B.tab = 1; SESSIONS.inventory = nil");
}

#[test]
fn trade_option_off_holds_nothing_opens_nothing_and_the_game_keeps_its_windows() {
    let scenario = r#"
        T.poll(true)                                        -- the option was never turned on
        local out = {}
        out[#out + 1] = #TAKEOVERS                          -- nothing is held
        talk_merchant({ware_(1, "Bread")})
        out[#out + 1] = OPENED                              -- the viewer is not opened for a merchant
        out[#out + 1] = tostring(T.visit_merchant())
        out[#out + 1] = tostring(T.mode())
        local tabs = {}
        for _, t in ipairs(B.visible_tabs()) do tabs[#tabs + 1] = t.label end
        out[#out + 1] = table.concat(tabs, ",")
        banker_talk({})
        out[#out + 1] = OPENED
        out[#out + 1] = #OPTION_ROWS .. ":" .. OPTION_ROWS[1].id .. ":" .. tostring(OPTION_ROWS[1].default)
        return table.concat(out, ";")
    "#;
    let got = string(&world(), &format!("(function() {scenario} end)()"));
    assert_eq!(got, "0;0;false;nil;All,Usable,Equipment,Charged,Misc,Bank;0;1:open_npc:false", "off by default: the game's windows");

    // negative control: with the option ignored, the merchant opens the window and the tab appears
    let bad = mutated(&[("T.on = eqoa.options.get(OPTION) == true", "T.on = true")]);
    let got = string(&bad, &format!("(function() {scenario} end)()"));
    assert_ne!(got, "0;0;false;nil;All,Usable,Equipment,Charged,Misc,Bank;0;1:open_npc:false", "negative control: {got}");
    assert!(got.starts_with("2;"), "negative control: it holds both windows ({got})");
}

#[test]
fn trade_option_on_holds_both_windows_whole_and_a_merchant_opens_the_merchant_tab_a_banker_the_bank_tab() {
    let lua = world();
    viewer_closed(&lua);
    assert_eq!(string(&lua, "taken_scope('merchant') .. ',' .. taken_scope('bank')"), "all,all", "the game's windows are held whole");
    // the player talks to a merchant with the viewer closed: it opens on the Merchant tab
    run(&lua, "talk_merchant({ware_(1, 'Bread')})");
    assert_eq!(int(&lua, "OPENED"), 1);
    assert_eq!(int(&lua, "B.tab"), 8);
    assert_eq!(string(&lua, "T.mode()"), "merchant");
    assert!(boolean(&lua, "T.auto_opened"), "the visit opened the window");
    assert_eq!(string(&lua, "(function() local t = {} for _, x in ipairs(B.visible_tabs()) do t[#t + 1] = x.label .. x.idx end return table.concat(t, ',') end)()"), "All1,Usable2,Equipment3,Charged4,Misc5,Bank6,Merchant8");
    // the Merchant tab exists only while a merchant is open
    run(&lua, "MERCH = nil");
    assert!(!boolean(&lua, "T.visit_merchant()"));
    assert!(!string(&lua, "(function() local t = {} for _, x in ipairs(B.visible_tabs()) do t[#t + 1] = x.label end return table.concat(t, ',') end)()").contains("Merchant"));

    // with the viewer already open on another tab it is the tab that changes, and the viewer is not the visit's to close
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "B.open = true; B.tab = 3; talk_merchant({ware_(1, 'Bread')})");
    assert_eq!(int(&lua, "OPENED"), 0, "already open");
    assert_eq!(int(&lua, "B.tab"), 8);
    assert!(!boolean(&lua, "T.auto_opened"));

    // a banker opens the Bank tab, and the tab shows the bank beside the pack
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "banker_talk({bank_it(7, 'Moonstone', 3)})");
    assert_eq!(int(&lua, "OPENED"), 1);
    assert_eq!(int(&lua, "B.tab"), 6);
    assert_eq!(string(&lua, "T.mode()"), "bank");
    // the Bank tab with no bank open is the plain bank list, as before
    run(&lua, "BANK.open = false");
    assert_eq!(string(&lua, "T.mode()"), "nil");

    // a merchant the mod's own helper talked to (Sell junk) is not a visit
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "B.helper_hold.merchant = true; talk_merchant({ware_(1, 'Bread')})");
    assert_eq!(int(&lua, "OPENED"), 0, "a helper's session shows nothing");
    assert!(!boolean(&lua, "T.visit_merchant()"));
    // negative control: without telling the helper's session from the player's, Sell junk opens the Merchant tab
    let bad = mutated(&[("if d.before == nil and not B.helper_hold.merchant then opened(\"merchant\") end", "if d.before == nil then opened(\"merchant\") end")]);
    viewer_closed(&bad);
    run(&bad, "B.helper_hold.merchant = true; talk_merchant({ware_(1, 'Bread')})");
    assert_eq!(int(&bad, "OPENED"), 1, "negative control: a helper's talk opened the window");
}

#[test]
fn trade_a_click_buys_one_a_stackable_ware_asks_how_many_and_shows_the_total() {
    let setup = r#"
        MONEY = 100
        BUYP.w1, BUYP.w2, BUYP.w3 = 10, 12, 5000
        set_pack({})
        talk_merchant({ware_(1, "Bread"), ware_(2, "Healing Potion", 20), ware_(3, "Plate Armor")})
    "#;
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    // one ware that does not stack: bought at once, exactly one
    run(&lua, "T.click_ware(WARES[1])");
    assert_eq!(string(&lua, "#BUYS .. ':' .. BUYS[1].ref .. ':' .. BUYS[1].n"), "1:w1:1");
    assert!(string(&lua, "B.result.text").starts_with("Buy Bread (10t)"), "{}", string(&lua, "B.result.text"));
    // a stackable ware asks first: nothing is bought, the count starts on 1 and is capped by what 100 tunar pay for (8)
    run(&lua, "T.click_ware(WARES[2])");
    assert_eq!(int(&lua, "#BUYS"), 1, "nothing is bought before the answer");
    assert_eq!(string(&lua, "T.prompt.kind .. ':' .. T.prompt.qty .. ':' .. T.prompt.max"), "buy:1:8");
    assert_eq!(int(&lua, "T.prompt_total(T.prompt)"), 12);
    run(&lua, "type_digits('3')");
    assert_eq!(int(&lua, "T.prompt.qty"), 3);
    assert_eq!(int(&lua, "T.prompt_total(T.prompt)"), 36, "the total is the game's price for exactly this count");
    assert!(boolean(&lua, "INPUT.text == false and INPUT.claims[1] == 'Enter'"), "the keyboard is the question's while it is up");
    run(&lua, "key('Enter')");
    assert_eq!(string(&lua, "#BUYS .. ':' .. BUYS[2].ref .. ':' .. BUYS[2].n"), "2:w2:3", "exactly the three the player confirmed");
    assert!(string(&lua, "B.result.text").starts_with("Buy Healing Potion x3 (36t)"));
    assert!(boolean(&lua, "T.prompt == nil"));
    // more than can be paid is capped at what can
    run(&lua, "T.click_ware(WARES[2]); type_digits('50'); key('Enter')");
    assert_eq!(int(&lua, "BUYS[3].n"), 8);
    // Escape cancels the question only: nothing bought, the window stays (the key does not reach main.lua's close)
    run(&lua, "T.click_ware(WARES[2])");
    assert!(boolean(&lua, "key('Escape') == true"), "the first Escape is the question's");
    assert!(boolean(&lua, "T.prompt == nil and #BUYS == 3 and B.open"));
    assert!(boolean(&lua, "key('Escape') == false"), "the next one is the window's");

    // negative control: a stackable ware bought at once, with no question
    let bad = mutated(&[("if (it.max_stack or 0) > 1 then", "if false then")]);
    viewer_closed(&bad);
    run(&bad, setup);
    run(&bad, "T.click_ware(WARES[2])");
    assert_eq!(int(&bad, "#BUYS"), 1, "negative control: bought without asking");
    // negative control: the count confirmed is not the count bought
    let bad = mutated(&[("local n = math.floor(p.qty)", "local n = p.max")]);
    viewer_closed(&bad);
    run(&bad, setup);
    run(&bad, "T.click_ware(WARES[2]); type_digits('3'); key('Enter')");
    assert_eq!(int(&bad, "BUYS[1].n"), 8, "negative control: it bought the cap, not the 3 confirmed");
}

#[test]
fn trade_a_ware_that_costs_more_than_you_carry_is_greyed_and_never_bought() {
    let setup = r#"
        MONEY = 100
        BUYP.w1, BUYP.w3 = 10, 5000
        set_pack({})
        talk_merchant({ware_(1, "Bread"), ware_(3, "Plate Armor")})
    "#;
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    let entries = "T.entries('merchant')";
    assert_eq!(string(&lua, &format!("tostring(T.affordable({entries}[1])) .. ':' .. tostring(T.affordable({entries}[2]))")), "true:false", "the dear one is greyed");
    run(&lua, "T.click_ware(WARES[2])");
    assert_eq!(int(&lua, "#BUYS"), 0, "not tried");
    assert_eq!(string(&lua, "B.result.text"), "Plate Armor: costs 5,000 tunar, you carry 100");
    assert!(boolean(&lua, "B.result.ok == false"));
    // the tunar the player gains makes it buyable again, as the grey follows the money
    run(&lua, "MONEY = 6000; T.click_ware(WARES[2])");
    assert_eq!(int(&lua, "#BUYS"), 1);

    // negative control: with the money check off the dear ware is sent (the client would refuse it, but it is not greyed)
    let bad = mutated(&[
        ("local function affords(unit) return unit == nil or unit <= carried() end", "local function affords(unit) return true end"),
        ("if total and total > have then", "if false then"),
    ]);
    viewer_closed(&bad);
    run(&bad, setup);
    assert!(boolean(&bad, "T.affordable(T.entries('merchant')[2])"), "negative control: not greyed");
    run(&bad, "T.click_ware(WARES[2])");
    assert_eq!(int(&bad, "#BUYS"), 1, "negative control: the dear ware was tried");
}

#[test]
fn trade_right_click_sells_one_at_once_and_a_stack_asks_how_many_the_worn_is_never_offered() {
    let setup = r#"
        SELLP.ref1, SELLP.ref2 = 7, 4
        set_pack({it_(1, "Rusty Sword", {tabs = {"equipment"}}), it_(2, "Rat Pelt", {q = 3}), it_(3, "Worn Ring", {worn = true, tabs = {"equipment"}})})
        talk_merchant({ware_(1, "Bread")})
    "#;
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    run(&lua, "T.sell_item(INV_ITEMS[1])");
    assert_eq!(string(&lua, "#SELLS .. ':' .. SELLS[1].ref .. ':' .. SELLS[1].qty"), "1:ref1:1");
    assert!(string(&lua, "B.result.text").starts_with("Sell Rusty Sword (7t)"), "{}", string(&lua, "B.result.text"));
    // a stack asks, starting on the whole stack; the total is the game's price for the count
    run(&lua, "B.last_req = nil; T.sell_item(INV_ITEMS[2])");
    assert_eq!(int(&lua, "#SELLS"), 1, "nothing is sold before the answer");
    assert_eq!(string(&lua, "T.prompt.kind .. ':' .. T.prompt.qty .. ':' .. T.prompt.max"), "sell:3:3");
    assert_eq!(int(&lua, "T.prompt_total(T.prompt)"), 12);
    run(&lua, "type_digits('2')");
    assert_eq!(int(&lua, "T.prompt_total(T.prompt)"), 8);
    run(&lua, "key('Enter')");
    assert_eq!(string(&lua, "#SELLS .. ':' .. SELLS[2].ref .. ':' .. SELLS[2].qty"), "2:ref2:2", "exactly the two confirmed");
    // what is worn is not offered
    run(&lua, "B.last_req = nil; T.sell_item(INV_ITEMS[3])");
    assert_eq!(int(&lua, "#SELLS"), 2);
    assert_eq!(string(&lua, "B.result.text"), "Worn Ring: cannot sell an equipped item");
    // the hover card says what the stack sells for
    assert_eq!(string(&lua, "T.card({{'Rat Pelt', 'title'}}, INV_ITEMS[2], 'merchant')[2][1]"), "Sells for 12 tunar (the stack)");
    assert_eq!(int(&lua, "#T.card({{'Rat Pelt', 'title'}}, INV_ITEMS[2], nil)"), 1, "only at a merchant");

    // negative control: a stack sold whole at once, with no question
    let bad = mutated(&[("if q > 1 then open_prompt(\"sell\", it, q, q) else T.sell(it, 1) end", "T.sell(it, q)")]);
    viewer_closed(&bad);
    run(&bad, setup);
    run(&bad, "T.sell_item(INV_ITEMS[2])");
    assert_eq!(string(&bad, "#SELLS .. ':' .. SELLS[1].qty"), "1:3", "negative control: the stack went without asking");
}

#[test]
fn trade_sell_n_sells_the_selected_stacks_through_the_games_own_question_and_keeps_the_visit_open() {
    let setup = r#"
        set_pack({it_(1, "Rat Pelt", {q = 3}), it_(2, "Chipped Tooth"), it_(3, "Spider Silk", {q = 2}), it_(4, "Worn Ring", {worn = true, tabs = {"equipment"}})})
        talk_merchant({ware_(1, "Bread")})
        B.rebuild()
        X.click({item = INV_ITEMS[1]}, {ctrl = true})
        X.click({item = INV_ITEMS[3]}, {ctrl = true})
        X.click({item = INV_ITEMS[4]}, {ctrl = true})
    "#;
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    assert_eq!(string(&lua, "#X.sellable()"), "2", "the worn ring is left out of what Sell (N) counts");
    run(&lua, "X.sell_selected()");
    assert_eq!(int(&lua, "#BATCHES"), 1);
    assert!(boolean(&lua, "BATCHES[1].sell_many == true"), "one call to the client's own group sale");
    assert_eq!(string(&lua, "#steps_of(1) .. ':' .. steps_of(1)[1].item .. ':' .. steps_of(1)[1].count .. ':' .. steps_of(1)[2].item .. ':' .. steps_of(1)[2].count"), "2:ref1:3:ref3:2");
    run(&lua, "batch_ends(2)");
    assert!(!boolean(&lua, "released('merchant')"), "the merchant stays open for more");
    assert!(boolean(&lua, "T.visit_merchant()"));

    // negative control: a helper that ends its sale lets the merchant go, which would close the player's Merchant tab
    let bad = mutated(&[("if M.trade and M.trade.keeps(kind) then return end", "if false then return end")]);
    viewer_closed(&bad);
    run(&bad, setup);
    run(&bad, "X.sell_selected(); batch_ends(2)");
    assert!(boolean(&bad, "released('merchant')"), "negative control: the session was let go");
}

#[test]
fn trade_the_merchants_own_menu_choices_are_shown_and_never_skipped() {
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "talk_merchant({ware_(1, 'Bread')}, {{key = 'buy', label = 'Purchase', selected = true}, {key = 'sell', label = 'Trade in'}, {key = 'exit', label = 'Farewell'}})");
    assert_eq!(string(&lua, "T.hint('merchant')"), "Purchase: click a ware.  Trade in: right-click a bag item.");
    assert_eq!(string(&lua, "T.leave_label('merchant')"), "Farewell");
    assert_eq!(string(&lua, "T.leave_label('bank')"), "Done");
    // a merchant that gives no words uses the keys
    run(&lua, "talk_merchant({ware_(1, 'Bread')}, {{key = 'buy'}, {key = 'sell'}, {key = 'exit'}})");
    assert_eq!(string(&lua, "T.hint('merchant')"), "Buy: click a ware.  Sell: right-click a bag item.");
    assert_eq!(string(&lua, "T.leave_label('merchant')"), "Exit");

    // negative control: the merchant's words ignored
    let bad = mutated(&[("if o.key == key and o.label and o.label ~= \"\" then return o.label end", "if false then return o.label end")]);
    viewer_closed(&bad);
    run(&bad, "talk_merchant({ware_(1, 'Bread')}, {{key = 'buy', label = 'Purchase'}, {key = 'sell', label = 'Trade in'}, {key = 'exit', label = 'Farewell'}})");
    assert_eq!(string(&bad, "T.leave_label('merchant')"), "Exit", "negative control: the label is lost");
}

#[test]
fn trade_leaving_the_tab_walking_away_the_option_and_zoning_end_the_visit() {
    // leaving the Merchant tab lets the merchant go (the game's session ends) and the claim is taken again
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "B.open = true; talk_merchant({ware_(1, 'Bread')}); B.select(1)");
    assert!(boolean(&lua, "released('merchant')"), "the session is over");
    assert_eq!(string(&lua, "taken_scope('merchant')"), "all", "and the window is held again for the next one");
    assert_eq!(int(&lua, "B.tab"), 1);
    let bad = mutated(&[("if old == T.TAB and eqoa.merchant.get() ~= nil then end_visit(\"merchant\") end", "if false then end_visit(\"merchant\") end")]);
    viewer_closed(&bad);
    run(&bad, "B.open = true; talk_merchant({ware_(1, 'Bread')}); B.select(1)");
    assert!(!boolean(&bad, "released('merchant')"), "negative control: the player is left talking to the merchant");

    // walking away (the session closed under the tab): the window the visit opened goes with it; one already open stays
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "talk_merchant({ware_(1, 'Bread')}); MERCH = nil; T.tick()");
    assert!(boolean(&lua, "B.step_aside == true"), "the visit opened the window: it closes");
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "B.open = true; talk_merchant({ware_(1, 'Bread')}); MERCH = nil; T.tick()");
    assert!(boolean(&lua, "B.step_aside == nil and B.tab == 1"), "an open window is not closed, the tab goes");
    let bad = mutated(&[("if T.auto_opened then B.step_aside = true -- the visit opened the window: it goes with it", "if false then B.step_aside = true")]);
    viewer_closed(&bad);
    run(&bad, "talk_merchant({ware_(1, 'Bread')}); MERCH = nil; T.tick()");
    assert!(boolean(&bad, "B.step_aside == nil"), "negative control: the empty window stays");

    // the bank walked away from: the window it opened closes too
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "banker_talk({bank_it(7, 'Moonstone', 3)}); T.tick(); BANK.open = false; T.tick()");
    assert!(boolean(&lua, "B.step_aside == true"));

    // the option turned off while shopping: the claims are let go and the visit is over
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "B.open = true; SESSIONS.inventory = {ref = 'inv'}; talk_merchant({ware_(1, 'Bread')}); OPTIONS.open_npc = false; T.poll(true)");
    assert!(boolean(&lua, "released('merchant') and released('bank')"));
    assert!(boolean(&lua, "not T.visit_merchant() and T.mode() == nil"));

    // zoning: the claims ended with the world and are taken again
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "local n = #TAKEOVERS; T.reset(); LATER = #TAKEOVERS - n");
    assert_eq!(int(&lua, "LATER"), 2, "both windows are claimed again in the new world");
}

#[test]
fn trade_a_banker_opens_the_bank_tab_a_click_moves_one_across_and_a_stack_asks_how_many() {
    let setup = r#"
        set_pack({it_(1, "Rusty Sword", {tabs = {"equipment"}}), it_(2, "Rat Pelt", {q = 5}), it_(3, "Worn Ring", {worn = true, tabs = {"equipment"}})})
        banker_talk({bank_it(7, "Moonstone", 4), bank_it(8, "Spare Dagger", 1)})
    "#;
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    assert_eq!(int(&lua, "B.tab"), 6);
    assert_eq!(string(&lua, "T.mode()"), "bank");
    assert_eq!(string(&lua, "T.caption('bank')"), "Bank 2/40   tunar 500");
    assert_eq!(string(&lua, "#T.entries('bank')"), "2");
    // one item: moved at once (a click on the bank's row takes it out, a click in the pack puts it in)
    run(&lua, "T.click_move(BANK.items[2])");
    assert_eq!(string(&lua, "#MOVES .. ':' .. MOVES[1].from .. ':' .. MOVES[1].ref .. ':' .. MOVES[1].n"), "1:bank:bank8:1");
    run(&lua, "B.last_req = nil; T.click_move(INV_ITEMS[1])");
    assert_eq!(string(&lua, "MOVES[2].from .. ':' .. MOVES[2].ref .. ':' .. MOVES[2].n"), "inventory:ref1:1");
    // a stack asks: part of it moves, exactly the part confirmed
    run(&lua, "B.last_req = nil; T.click_move(INV_ITEMS[2])");
    assert_eq!(int(&lua, "#MOVES"), 2, "nothing is moved before the answer");
    assert_eq!(string(&lua, "T.prompt.kind .. ':' .. T.prompt.qty .. ':' .. T.prompt.max"), "deposit:5:5");
    run(&lua, "type_digits('2'); key('Enter')");
    assert_eq!(string(&lua, "#MOVES .. ':' .. MOVES[3].from .. ':' .. MOVES[3].ref .. ':' .. MOVES[3].n"), "3:inventory:ref2:2");
    run(&lua, "B.last_req = nil; T.click_move(BANK.items[1])");
    assert_eq!(string(&lua, "T.prompt.kind"), "withdraw");
    run(&lua, "type_digits('1'); key('Enter')");
    assert_eq!(string(&lua, "MOVES[4].from .. ':' .. MOVES[4].n"), "bank:1");
    // what is worn is not moved
    run(&lua, "B.last_req = nil; T.click_move(INV_ITEMS[3])");
    assert_eq!(int(&lua, "#MOVES"), 4);
    assert_eq!(string(&lua, "B.result.text"), "Worn Ring: cannot move an equipped item");

    // a bank the mod's own helper opened (Bank marked, the tunar buttons) is not a visit
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "B.helper_hold.bank = true; banker_talk({bank_it(7, 'Moonstone', 3)})");
    assert_eq!(int(&lua, "OPENED"), 0);

    // negative control: a stack moved whole at once, with no question
    let bad = mutated(&[("open_prompt(it.list == \"bank\" and \"withdraw\" or \"deposit\", it, q, q)", "T.move(it, q)")]);
    viewer_closed(&bad);
    run(&bad, setup);
    run(&bad, "T.click_move(INV_ITEMS[2])");
    assert_eq!(string(&bad, "#MOVES .. ':' .. MOVES[1].n"), "1:5", "negative control: the whole stack went without asking");
}

#[test]
fn review4_a_window_another_mod_holds_is_left_alone() {
    let lua = world();
    run(&lua, "eqoa.windows.takeover = function(k, o) return nil, {code = 'busy', message = 'held by eqoa_other'} end; OPTIONS.open_npc = true; T.poll(true); B.open = false");
    run(&lua, "talk_merchant({ware_(1, 'Bread')})");
    assert_eq!(int(&lua, "OPENED"), 0, "not our claim: Bag Viewer does not open over the other mod's window");
    assert_eq!(int(&lua, "#PASSED"), 0, "and does not take the session from it");
    assert!(boolean(&lua, "not released('merchant') and not T.visit_merchant()"));
    run(&lua, "banker_talk({bank_it(7, 'Moonstone', 3)})");
    assert_eq!(int(&lua, "OPENED"), 0);
    assert!(boolean(&lua, "T.mode() == nil"));
}

#[test]
fn review1_the_option_turned_off_at_an_npc_hands_the_window_to_the_game_and_lets_the_claims_go() {
    // the option goes off and the NPC is talked to before the once-a-second poll has noticed: the claim is still held
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "OPTIONS.open_npc = false; SESSIONS.merchant = {ref = 'ws-1'}; talk_merchant({ware_(1, 'Bread')})");
    assert_eq!(int(&lua, "OPENED"), 0, "off: Bag Viewer stays shut");
    assert_eq!(string(&lua, "#PASSED .. ':' .. tostring(PASSED[1])"), "1:ws-1", "the game's own merchant window is shown");
    assert!(!boolean(&lua, "released('merchant')"), "the claim is kept until the client has carried the pass out");
    run(&lua, "emit('windows.session', {phase = 'closed', kind = 'merchant', reason = 'passed', ref = 'ws-1'})");
    assert!(boolean(&lua, "released('merchant') and released('bank')"), "and let go when the session's end is told");
    // the same for a banker
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "OPTIONS.open_npc = false; SESSIONS.bank = {ref = 'ws-2'}; banker_talk({bank_it(7, 'Moonstone', 3)})");
    assert_eq!(string(&lua, "#PASSED .. ':' .. tostring(PASSED[1]) .. ':' .. OPENED"), "1:ws-2:0");
    // turned off while the Bank tab shows a visit: the bank session ends with the claim
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "banker_talk({bank_it(7, 'Moonstone', 3)}); OPTIONS.open_npc = false; T.tick()");
    assert!(boolean(&lua, "released('bank')"), "the player is not left talking to a banker with no window");
}

#[test]
fn review2_a_session_the_client_has_not_made_yet_is_passed_when_it_opens_and_a_refused_pass_ends_it() {
    // Bag Viewer cannot open (the player died in flight): the session does not exist yet on the open's own frame
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "T.open_viewer = function() OPENED = OPENED + 1 end; talk_merchant({ware_(1, 'Bread')})");
    assert_eq!(int(&lua, "#PASSED"), 0, "no session yet: nothing to pass on this frame");
    run(&lua, "SESSIONS.merchant = {ref = 'ws-9'}; emit('windows.session', {phase = 'opened', kind = 'merchant', ref = 'ws-9'})");
    assert_eq!(string(&lua, "#PASSED .. ':' .. tostring(PASSED[1])"), "1:ws-9", "passed when the client makes the session");
    // the game will not show it now (not_ready): the session is ended rather than left hidden
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "PASS_ANSWER = {code = 'not_ready'}; T.open_viewer = function() end; SESSIONS.merchant = {ref = 'ws-3'}; talk_merchant({ware_(1, 'Bread')})");
    assert!(boolean(&lua, "released('merchant')"), "a pass that is refused must not leave a hidden session");
}

#[test]
fn review3_before_the_first_draw_a_click_does_not_crash_and_any_error_ends_the_visit_and_keeps_the_claims_off() {
    // (a) no draw has happened yet: the window's hit lists do not exist
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "talk_merchant({ware_(1, 'Bread')}); B.slot_hits = nil");
    assert!(boolean(&lua, "pcall(T.on_pointer, {button = 'right', x = 5, y = 5})"), "a right-click before the first draw is not an error");
    assert!(boolean(&lua, "pcall(T.on_pointer, {button = 'left', x = 5, y = 5})"));
    // (b) an error inside the module: the visit ends, both windows are let go and are not taken again
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "talk_merchant({ware_(1, 'Bread')}); T.hits = {{x = 0, y = 0, w = 10, h = 10, run = function() error('boom') end}}");
    assert!(boolean(&lua, "pcall(T.on_pointer, {button = 'left', x = 1, y = 1})"), "the error stays inside the module");
    assert!(boolean(&lua, "released('merchant') and released('bank')"), "the claims are let go");
    assert!(boolean(&lua, "T.dead == true and not T.visit_merchant() and T.mode() == nil"));
    run(&lua, "local n = #TAKEOVERS; NOW = NOW + 5; T.poll(true); LATER = #TAKEOVERS - n");
    assert_eq!(int(&lua, "LATER"), 0, "a dead module does not claim the windows again");
    // an error in an event handler of the module counts the same
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "eqoa.merchant.get = function() error('boom') end");
    assert!(boolean(&lua, "pcall(T.on_merchant, {before = nil, after = {}})"));
    assert!(boolean(&lua, "T.dead == true and released('merchant')"));
    // (c) the window's drawing died (the client removed its session without telling the mod): not claimed again
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "B.open = true; SESSIONS.inventory = nil; NOW = NOW + 5; local n = #TAKEOVERS; T.poll(true); LATER = #TAKEOVERS - n");
    assert_eq!(int(&lua, "LATER"), 0, "no claim over a window that cannot draw");
    assert!(boolean(&lua, "T.dead == true and not B.open"), "the stale window is closed");
}

#[test]
fn review5_the_controller_keeps_its_row_on_screen_and_a_new_visit_starts_at_the_top() {
    let setup = r#"
        local wares = {}
        for i = 1, 20 do wares[i] = ware_(i, "Ware " .. i) end
        talk_merchant(wares)
        T.page = 9
        T.pad_key("left", true)                              -- from the pack's first column into the list
    "#;
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    run(&lua, "for i = 1, 14 do T.pad_key('down', false) end");
    assert_eq!(int(&lua, "T.pad_row"), 15);
    assert!(
        boolean(&lua, "T.pad_row > T.scroll and T.pad_row <= T.scroll + T.page"),
        "row {} with the list scrolled to {}",
        int(&lua, "T.pad_row"),
        int(&lua, "T.scroll")
    );
    // and back up past the top of the page
    run(&lua, "for i = 1, 14 do T.pad_key('up', false) end");
    assert!(boolean(&lua, "T.pad_row == 1 and T.scroll == 0"));
    // a visit that ended at row 15 does not leave it for the next merchant
    run(&lua, "for i = 1, 14 do T.pad_key('down', false) end; MERCH = nil; T.tick(); B.open = true; talk_merchant({ware_(1, 'A'), ware_(2, 'B')})");
    assert!(
        boolean(&lua, "T.pad_row == 1 and T.scroll == 0 and not T.pad_list"),
        "stale row {} scroll {}",
        int(&lua, "T.pad_row"),
        int(&lua, "T.scroll")
    );
}

#[test]
fn review6_the_answer_acts_on_the_same_stack_merchant_and_price_the_question_showed() {
    let setup = r#"
        MONEY = 1000
        BUYP.w2 = 12
        set_pack({})
        talk_merchant({ware_(1, "Bread"), ware_(2, "Healing Potion", 20)})
        T.click_ware(WARES[2])
        type_digits('3')
    "#;
    // the stack at that place is another stack by the time of the answer (a new handle)
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    run(&lua, "local w = ware_(2, 'Healing Potion', 20); w.ref = 'w2-later'; WARES[2] = w; MERCH.items = WARES; MERCH.revision = MERCH.revision + 1; key('Enter')");
    assert_eq!(int(&lua, "#BUYS"), 0, "the handle the question showed is stale: nothing is bought");
    assert!(string(&lua, "B.result.text").contains("changed"), "the player is told: {}", string(&lua, "B.result.text"));
    // another item at the same place
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    run(&lua, "WARES[2].id = 999; key('Enter')");
    assert_eq!(int(&lua, "#BUYS"), 0, "another item: nothing is bought");
    // the price moved while the question was up
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    run(&lua, "BUYP.w2 = 99; key('Enter')");
    assert_eq!(int(&lua, "#BUYS"), 0, "the price changed: nothing is bought");
    assert!(string(&lua, "B.result.text").contains("price"), "{}", string(&lua, "B.result.text"));
    // another merchant
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    run(&lua, "MERCH.merchant.entity_id = 777; key('Enter')");
    assert_eq!(int(&lua, "#BUYS"), 0, "another merchant: nothing is bought");
    // control: nothing moved, the three are bought
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    run(&lua, "key('Enter')");
    assert_eq!(string(&lua, "#BUYS .. ':' .. BUYS[1].ref .. ':' .. BUYS[1].n"), "1:w2:3");
    // a typed number past the cap shows the number that will be used
    let lua = world();
    viewer_closed(&lua);
    run(&lua, "MONEY = 100; BUYP.w2 = 12; talk_merchant({ware_(2, 'Healing Potion', 20)}); T.click_ware(WARES[1]); type_digits('50')");
    assert_eq!(string(&lua, "T.prompt.text .. ':' .. T.prompt.qty"), "8:8", "the box shows the 8 that Enter buys, not the 50 typed");
}

#[test]
fn review7_the_two_panel_bank_tab_is_only_for_a_bank_the_player_talked_to() {
    let lua = world();
    viewer_closed(&lua);
    // the Bank tab's own refresh session (a helper's talk) near a banker: the tab stays as it was before the feature
    run(&lua, "B.open = true; B.tab = 6; B.helper_hold.bank = true; BANK.open = true");
    assert_eq!(string(&lua, "T.mode()"), "nil", "a helper's session is not a visit");
    // the player's own talk: two panels
    run(&lua, "B.helper_hold.bank = nil");
    assert_eq!(string(&lua, "T.mode()"), "bank");
    // and the claim must be ours: another mod's bank is not shown
    run(&lua, "T.held.bank = false");
    assert_eq!(string(&lua, "T.mode()"), "nil");
}

#[test]
fn review8_items_marked_to_bank_or_keep_are_not_sold_by_sell_n_and_a_right_click_asks_first() {
    let setup = r#"
        set_pack({it_(1, "Rat Pelt", {q = 3}), it_(2, "Moonstone"), it_(3, "Arrows", {q = 10}), it_(4, "Spider Silk", {q = 2})})
        H.bank["2"] = true            -- marked to bank (B)
        H.keep["3"] = 20              -- kept up to 20 for Restock (R)
        talk_merchant({ware_(1, "Bread")})
        B.rebuild()
        for i = 1, 4 do X.click({item = INV_ITEMS[i]}, {ctrl = true}) end
    "#;
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    assert_eq!(string(&lua, "#X.sellable()"), "2", "only the two unmarked stacks");
    run(&lua, "X.sell_selected()");
    assert_eq!(string(&lua, "#steps_of(1) .. ':' .. steps_of(1)[1].item .. ':' .. steps_of(1)[2].item"), "2:ref1:ref4", "the marked stacks stay");
    run(&lua, "batch_ends(2)");
    assert!(string(&lua, "B.result.text").contains("2 marked"), "the line says what was kept: {}", string(&lua, "B.result.text"));
    // a right-click on a marked item asks first; nothing is sold until Yes
    let lua = world();
    viewer_closed(&lua);
    run(&lua, setup);
    run(&lua, "X.clear(); T.sell_item(INV_ITEMS[2], {fx = 8, fy = 46})");
    assert_eq!(int(&lua, "#SELLS"), 0, "not sold by the right-click");
    assert!(boolean(&lua, "B.menu ~= nil and B.menu.asking == true"), "a question is up");
    assert!(string(&lua, "B.menu.title").contains("Moonstone"));
    run(&lua, "B.trigger_menu(B.menu.rows[2])");
    assert_eq!(int(&lua, "#SELLS"), 0, "No keeps it");
    run(&lua, "T.sell_item(INV_ITEMS[2], {fx = 8, fy = 46}); B.trigger_menu(B.menu.rows[1])");
    assert_eq!(string(&lua, "#SELLS .. ':' .. SELLS[1].ref"), "1:ref2", "Yes sells it");
    // an unmarked stack asks how many, not whether
    run(&lua, "B.last_req = nil; T.sell_item(INV_ITEMS[1], {fx = 8, fy = 46})");
    assert_eq!(string(&lua, "T.prompt.kind"), "sell");
}

#[test]
fn third_check_a_poll_while_a_pass_is_carried_keeps_the_claim_and_the_wait_lets_it_go() {
    // the pass is asked (the option went off at an NPC); a player.changed poll arrives before the session's end is told
    let flow = r#"
        OPTIONS.open_npc = false; SESSIONS.merchant = {ref = 'ws-1'}; talk_merchant({ware_(1, 'Bread')})
        T.poll(true)
    "#;
    let lua = world();
    viewer_closed(&lua);
    run(&lua, flow);
    assert_eq!(string(&lua, "#PASSED .. ':' .. tostring(PASSED[1])"), "1:ws-1");
    assert!(boolean(&lua, "T.dead == false and not released('merchant')"), "the poll neither fails nor lets the claim go before the pass is carried out");
    // "closed" is never told: after PASS_WAIT the claim is let go, no error
    run(&lua, "NOW = NOW + 4; T.poll(true)");
    assert!(boolean(&lua, "T.dead == false and released('merchant') and released('bank')"), "the wait ran out: the claims go back");
    // negative control: PASS_WAIT used before it is declared is the global nil: the poll raises, the guard lets everything go
    let bad = mutated(&[("now - since > T.PASS_WAIT", "now - since > PASS_WAIT")]);
    viewer_closed(&bad);
    run(&bad, flow);
    assert!(boolean(&bad, "T.dead == true and released('merchant')"), "negative control: the poll raised and dropped the pass");
}
