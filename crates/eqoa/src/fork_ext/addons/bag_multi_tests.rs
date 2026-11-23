// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Bag Viewer multi-select (2026-11-23): Ctrl+click and Shift+click pick slots, one menu acts on all of them. Tested without a
//! window or a GPU: the shipped `bags.lua`, `helpers.lua` and `multi.lua` are loaded into a plain Lua state with the fake
//! `eqoa` of `bag_helpers_tests.rs` (its fixture), and each test has a negative control: the same scenario against the shipped
//! source with one line of the feature changed (`mutated`), which must give the other answer. The window wiring (real clicks
//! with the modifiers, the blue outline, closing and zoning) is `bag_viewer_tests.rs`'s multi-select test, which needs a GPU.
use super::bag_helpers_tests::{FIXTURE, source};
use mlua::{Lua, Table};

fn world_with(mutations: &[(&str, &str)]) -> Lua {
    let lua = Lua::new();
    let mut bags = source("eqoa_bag_viewer/modules/bags.lua");
    let mut helpers = source("eqoa_bag_viewer/modules/helpers.lua");
    let mut multi = source("eqoa_bag_viewer/modules/multi.lua");
    for (from, to) in mutations {
        let hit = bags.contains(from) || helpers.contains(from) || multi.contains(from);
        assert!(hit, "the mutation matches nothing in the shipped source: {from}");
        bags = bags.replace(from, to);
        helpers = helpers.replace(from, to);
        multi = multi.replace(from, to);
    }
    let loader = |code: String, name: &'static str| {
        lua.create_function(move |lua, ()| -> mlua::Result<Table> { lua.load(code.as_str()).set_name(name).call(()) }).unwrap()
    };
    lua.globals().set("BAGS_LOADER", loader(bags, "bags.lua")).unwrap();
    lua.globals().set("HELPERS_LOADER", loader(helpers, "helpers.lua")).unwrap();
    lua.globals().set("MULTI_LOADER", loader(multi, "multi.lua")).unwrap();
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
    lua.load(format!("return {expr}")).eval().unwrap_or_else(|e| panic!("{expr}
{e}"))
}
fn int(lua: &Lua, expr: &str) -> i64 {
    lua.load(format!("return {expr}")).eval().unwrap_or_else(|e| panic!("{expr}
{e}"))
}
fn boolean(lua: &Lua, expr: &str) -> bool {
    lua.load(format!("return {expr}")).eval().unwrap_or_else(|e| panic!("{expr}
{e}"))
}
fn run(lua: &Lua, code: &str) {
    lua.load(code).exec().unwrap_or_else(|e| panic!("{e}\n{code}"));
}

/// What the multi-select module needs beyond the helpers' fixture: items with a server key, the new action, the clicks as
/// `main.lua` makes them (`X.click` first, the item's own menu when it was a plain click).
const EXTRA: &str = r##"
X = MULTI_LOADER()
DESTROYS = {}
eqoa.actions.destroy_many = function(refs) DESTROYS[#DESTROYS + 1] = refs return "destroy" .. #DESTROYS end
BANK.open = false
function it_(id, name, o)
  local t = item(id, name, o)
  t.inventory_key = id
  return t
end
function bank_it(id, name, q)
  return {id = id, name = name, ref = "bank" .. id, quantity = q or 1, max_stack = 20, equipped = false, tabs = {"misc"}, list = "bank", inventory_key = id}
end
-- the grid for the pack as set
function view(items)
  if items then set_pack(items) end
  B.tab = 1
  return B.rebuild()
end
function G() if not B.grid then B.rebuild() end return B.grid end
function click(i, d)
  local e = G().items[i]
  local h = {item = e.item, fx = 8, fy = 46}
  if not X.click(h, d or {}) then B.build_menu(e.item, 8, 46) end
end
function ctrl(i) click(i, {ctrl = true}) end
function shift(i) click(i, {shift = true}) end
function selected()
  local names = {}
  for _, e in ipairs(G().items) do if X.is_selected(e.item) then names[#names + 1] = e.item.name end end
  return table.concat(names, ",")
end
function group_menu(i)
  local e = G().items[i]
  X.open_menu({item = e.item, fx = 8, fy = 46})
  return B.menu
end
function labels()
  local t = {}
  for _, r in ipairs(B.menu and B.menu.rows or {}) do t[#t + 1] = r.label end
  return table.concat(t, "|")
end
function row(prefix)
  for _, r in ipairs(B.menu.rows) do
    if r.label:sub(1, #prefix) == prefix then return r end
  end
end
function choose(prefix)
  local r = row(prefix)
  assert(r, "no row " .. prefix .. " in " .. labels())
  B.trigger_menu(r)
  return r
end
SIX = function()
  return {it_(1, "Rat Pelt"), it_(2, "Chipped Tooth"), it_(3, "Torn Cloth"), it_(4, "Cracked Fang"), it_(5, "Spider Silk"), it_(6, "Bone Chip")}
end
"##;

#[test]
fn multi_ctrl_click_adds_and_removes_shift_click_takes_the_range_and_a_plain_click_clears() {
    let scenario = r#"
        view(SIX())
        local out = {}
        ctrl(2); ctrl(4)
        out[#out + 1] = selected()                       -- Chipped Tooth,Cracked Fang
        out[#out + 1] = tostring(B.menu)                 -- no menu opened by selecting
        ctrl(2)
        out[#out + 1] = selected()                       -- Cracked Fang
        shift(6)                                         -- from the slot clicked last (2) to 6
        out[#out + 1] = selected()
        click(1)                                         -- a plain click clears and opens the item's own menu
        out[#out + 1] = "[" .. selected() .. "]" .. tostring(B.menu and B.menu.title)
        X.clear()
        shift(3)                                         -- no anchor: just that one
        out[#out + 1] = selected()
        click(1, {ctrl = true}); click(3, {ctrl = true, shift = true})   -- Ctrl+Shift adds the range
        out[#out + 1] = selected()
        return table.concat(out, ";")
    "#;
    let real = world();
    let got = string(&real, &format!("(function() {scenario} end)()"));
    assert_eq!(
        got,
        "Chipped Tooth,Cracked Fang;nil;Cracked Fang;Chipped Tooth,Torn Cloth,Cracked Fang,Spider Silk,Bone Chip;[]Rat Pelt;Torn Cloth;Rat Pelt,Chipped Tooth,Torn Cloth",
        "a ctrl-click toggles one; a shift-click takes the slots from the last one clicked to this one (ctrl-click 2 was the last, so 2..6); a plain click clears"
    );
    // negative controls: without the ctrl branch a ctrl-click is a plain click (it opens a menu, selects nothing); without
    // the range a shift-click selects nothing
    let no_ctrl = mutated(&[("    elseif d.ctrl then", "    elseif false then")]);
    assert_ne!(string(&no_ctrl, &format!("(function() {scenario} end)()")), got);
    assert_eq!(int(&no_ctrl, "(function() view(SIX()) ctrl(2) return X.n end)()"), 0, "negative control: Ctrl+click selected nothing");
    let no_range = mutated(&[("for i = math.min(a, b), math.max(a, b) do add(grid.items[i].item) end", "")]);
    assert_eq!(int(&no_range, "(function() view(SIX()) ctrl(2) shift(5) return X.n end)()"), 0, "negative control: no range");
    assert_eq!(int(&real, "(function() view(SIX()) ctrl(2) shift(5) return X.n end)()"), 4);
    let keeps = mutated(&[("    X.drop_selection()\n    X.anchor = k\n    return false", "    X.anchor = k\n    return false")]);
    assert_eq!(int(&keeps, "(function() view(SIX()) ctrl(2) click(1) return X.n end)()"), 1, "negative control: a plain click kept the selection");
    assert_eq!(int(&real, "(function() view(SIX()) ctrl(2) click(1) return X.n end)()"), 0);
}

#[test]
fn multi_the_group_menu_is_titled_with_the_count_and_greys_what_cannot_apply_with_the_reason() {
    let pack = r#"
        view({it_(1, "Rat Pelt", {q = 3}), it_(2, "Chipped Tooth"), it_(3, "Rusty Sword", {worn = true, tabs = {"equipment"}}), it_(4, "Torn Cloth")})
        ctrl(1); ctrl(2); ctrl(3)
        group_menu(2)
    "#;
    let lua = world();
    run(&lua, pack);
    assert_eq!(string(&lua, "B.menu.title"), "3 items selected");
    assert_eq!(string(&lua, "labels()"), "Mark as junk|Unmark|Mark to bank|Sell (2)|Put in bank (2)|Destroy...");
    // nothing near: Sell and Put in bank are greyed, each with its reason; the marks work anywhere
    assert!(boolean(&lua, "row('Sell').enabled == false and row('Sell').reason == 'no merchant nearby'"));
    assert!(boolean(&lua, "row('Put in bank').enabled == false and row('Put in bank').reason == 'no banker nearby'"));
    assert!(boolean(&lua, "row('Mark as junk').enabled and row('Mark to bank').enabled and row('Destroy').enabled"));
    assert!(boolean(&lua, "row('Unmark').enabled == false"), "nothing is marked yet");
    // a merchant in reach: enabled; one beyond the learned reach: greyed with the window's own words, a second click tries
    run(&lua, "put_npcs(3.0, 4.0); view(); X.clear(); ctrl(1); ctrl(2); group_menu(1)");
    assert!(boolean(&lua, "row('Sell').enabled and row('Put in bank').enabled"));
    run(&lua, "put_npcs(30.0, 30.0); X.clear(); ctrl(1); group_menu(1)");
    assert!(boolean(&lua, "row('Sell').enabled == false and row('Sell').reason == 'too far from the merchant' and row('Sell').far_kind == 'merchant'"));
    assert!(boolean(&lua, "row('Put in bank').reason == 'too far from the banker'"));
    // one item: "1 item selected"; only the worn one: nothing to sell, bank or destroy
    run(&lua, "put_npcs(3.0, 3.0); X.clear(); ctrl(3); group_menu(3)");
    assert_eq!(string(&lua, "B.menu.title"), "1 item selected");
    assert!(boolean(&lua, "row('Sell').reason == 'all selected items are worn' and not row('Sell').enabled and not row('Destroy').enabled"));

    // negative controls
    let no_reason = mutated(&[("(not merch_open and not near_merch) and \"no merchant nearby\"", "false")]);
    run(&no_reason, pack);
    assert!(boolean(&no_reason, "row('Sell').enabled == true"), "negative control: Sell is offered with no merchant near");
    let no_count = mutated(&[("string.format(\"%d %s selected\", n, plural(n, \"item\", \"items\"))", "\"selected\"")]);
    run(&no_count, pack);
    assert_eq!(string(&no_count, "B.menu.title"), "selected");
}

#[test]
fn multi_sell_goes_in_pack_order_skips_the_worn_reports_a_refusal_and_finds_each_item_again() {
    let setup = r#"
        view({it_(1, "Rusty Sword", {worn = true, tabs = {"equipment"}}), it_(2, "Rat Pelt", {q = 2}), it_(3, "Chipped Tooth"),
              it_(4, "Torn Cloth"), it_(5, "Cracked Fang")})
        put_npcs(3.0, nil)
        open_merchant({})
        ctrl(1); shift(5)
        group_menu(3)
    "#;
    let lua = world();
    run(&lua, setup);
    assert_eq!(string(&lua, "B.menu.title"), "5 items selected");
    run(&lua, "choose('Sell (4)')");
    // one batch, four steps (the worn sword is not among them), in pack order, each the whole stack, through the merchant
    assert_eq!(int(&lua, "#BATCHES"), 1);
    assert_eq!(string(&lua, "BATCHES[1].require_window"), "merchant");
    assert_eq!(int(&lua, "#steps_of(1)"), 4);
    assert_eq!(
        string(&lua, "(function() local t = {} for _, s in ipairs(steps_of(1)) do t[#t + 1] = s.kind .. ':' .. s.item .. ':' .. s.count end return table.concat(t, ' ') end)()"),
        "sell:ref2:2 sell:ref3:1 sell:ref4:1 sell:ref5:1"
    );
    assert!(boolean(&lua, "B.batch_kind == 'multi' and B.batch_id ~= nil"));
    assert_eq!(string(&lua, "B.result.text"), "Selling 0 of 4...");
    // step 2 is refused by the time its turn comes, and the Cracked Fang is gone from the pack: the rest goes on with what
    // is found now (the Torn Cloth), the vanished one is skipped and counted
    run(&lua, "set_pack({it_(1, 'Rusty Sword', {worn = true, tabs = {'equipment'}}), it_(4, 'Torn Cloth')})");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', completed = 1, total = 4, failed_step = 2, code = 'not_ready', reason = 'inventory is full'})");
    assert_eq!(int(&lua, "#BATCHES"), 2, "the rest of the group goes on in a new batch");
    assert_eq!(int(&lua, "#steps_of(2)"), 1, "only the Torn Cloth is still there to sell");
    assert_eq!(string(&lua, "steps_of(2)[1].item"), "ref4");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = true, state = 'completed', completed = 1, total = 1})");
    assert_eq!(string(&lua, "B.result.text"), "Sold 2 of 4: Chipped Tooth was refused; 1 moved or gone; 1 worn item kept");
    assert!(boolean(&lua, "(function() for _, l in ipairs(LOG) do if l:find('Chipped Tooth was refused (inventory is full)', 1, true) then return true end end return false end)()"), "the reason is in the log");
    assert!(boolean(&lua, "B.result.ok == false"), "a refusal makes it a problem line");
    assert_eq!(int(&lua, "X.n"), 0, "the selection is cleared when the action finishes");
    assert!(boolean(&lua, "B.batch_id == nil and RELEASED[#RELEASED] == 'merchant'"), "the merchant session is let go at the end");

    // gear, a potion or a quick-slotted item in the group: the game's merchant would ask, so the group asks once first
    let ask = world();
    run(&ask, "view({it_(1, 'Rusty Sword', {tabs = {'equipment'}}), it_(2, 'Rat Pelt')}); put_npcs(3.0, nil); open_merchant({}); ctrl(1); ctrl(2); group_menu(1); choose('Sell (2)')");
    assert_eq!(int(&ask, "#BATCHES"), 0, "nothing is sold before the player says Yes");
    assert_eq!(string(&ask, "B.menu.title"), "Sell these items?");
    assert_eq!(string(&ask, "B.result.text"), "1 is gear, a potion, quick-slotted or no-trade: sell them all?");
    run(&ask, "choose('No')");
    assert_eq!(int(&ask, "#BATCHES"), 0, "No sells nothing");
    run(&ask, "group_menu(1); choose('Sell (2)'); choose('Yes')");
    assert_eq!(int(&ask, "#steps_of(1)"), 2);
    // negative control: without the question the gear is sold at the first click
    let blind_sell = mutated(&[("if not row.confirmed and row.ask and (row.enabled or row.far_kind) then", "if false then")]);
    run(&blind_sell, "view({it_(1, 'Rusty Sword', {tabs = {'equipment'}}), it_(2, 'Rat Pelt')}); put_npcs(3.0, nil); open_merchant({}); ctrl(1); ctrl(2); group_menu(1); choose('Sell (2)')");
    assert_eq!(int(&blind_sell, "#BATCHES"), 1, "negative control: sold with no question");

    // all goes well: "Sold 3 of 3"
    let ok = world();
    run(&ok, "view({it_(1, 'A'), it_(2, 'B'), it_(3, 'C')}); put_npcs(3.0, nil); open_merchant({}); ctrl(1); ctrl(2); ctrl(3); group_menu(1); choose('Sell (3)')");
    assert_eq!(int(&ok, "#steps_of(1)"), 3);
    run(&ok, "B.on_batch_progress({batch_id = B.batch_id, completed = 1, total = 3})");
    assert_eq!(string(&ok, "B.result.text"), "Selling 1 of 3...");
    run(&ok, "B.on_batch_result({batch_id = B.batch_id, ok = true, state = 'completed', completed = 3, total = 3})");
    assert_eq!(string(&ok, "B.result.text"), "Sold 3 of 3");
    assert!(boolean(&ok, "B.result.ok == true"));
    // stopped by the player (Stop): the rest is not tried
    let stop = world();
    run(&stop, "view({it_(1, 'A'), it_(2, 'B'), it_(3, 'C')}); put_npcs(3.0, nil); open_merchant({}); ctrl(1); ctrl(2); ctrl(3); group_menu(1); choose('Sell (3)')");
    run(&stop, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'stopped', completed = 1, total = 3, code = 'cancelled', reason = 'stopped'})");
    assert_eq!(int(&stop, "#BATCHES"), 1, "a stopped group is not started again");
    assert_eq!(string(&stop, "B.result.text"), "Sold 1 of 3: stopped");

    // negative controls
    // 1. a worn item is not left out of the sell list
    let worn = mutated(&[
        ("if it.equipped then worn[#worn + 1] = it else ok[#ok + 1] = it end", "ok[#ok + 1] = it"),
        ("if not cur or cur.equipped then", "if not cur then"),
    ]);
    run(&worn, setup);
    run(&worn, "choose('Sell (5)'); choose('Yes')"); // the sword is gear, so the group asks first
    assert_eq!(int(&worn, "#steps_of(1)"), 5, "negative control: the worn sword would be sold");
    // 2. a refusal ends the whole group instead of going on
    let no_go_on = mutated(&[("if entry and not STOP_CODES[code] and d.state == \"refused\" then", "if false then")]);
    run(&no_go_on, setup);
    run(&no_go_on, "choose('Sell (4)'); set_pack({it_(4, 'Torn Cloth')}); B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', completed = 1, total = 4, failed_step = 2, code = 'not_ready', reason = 'x'})");
    assert_eq!(int(&no_go_on, "#BATCHES"), 1, "negative control: the rest was dropped");
    // 3. an item is not found again before its step: its old reference is sent whether or not it is still there
    let blind = mutated(&[("if not cur or cur.equipped then", "if false then")]);
    run(&blind, setup);
    run(&blind, "choose('Sell (4)'); set_pack({it_(4, 'Torn Cloth')})");
    let failed = blind.load("B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', completed = 1, total = 4, failed_step = 2, code = 'not_ready', reason = 'x'})").exec();
    assert!(failed.is_err() || int(&blind, "#steps_of(2)") != 1, "negative control: a vanished item is not skipped");
}

#[test]
fn multi_a_merchant_too_far_asks_to_click_again_then_talks_and_sells_when_it_opens() {
    let lua = world();
    run(&lua, "view({it_(1, 'A'), it_(2, 'B')}); put_npcs(30.0, nil); ctrl(1); ctrl(2); group_menu(1)");
    assert!(boolean(&lua, "row('Sell (2)').enabled == false"));
    run(&lua, "choose('Sell (2)')");
    assert_eq!(int(&lua, "#TALKS"), 0, "the first click only says why");
    assert!(string(&lua, "B.result.text").contains("too far from the merchant"), "{}", string(&lua, "B.result.text"));
    run(&lua, "group_menu(1); choose('Sell (2)')");
    assert_eq!(int(&lua, "#TALKS"), 1, "the second click tries: it talks to the merchant");
    assert_eq!(int(&lua, "#BATCHES"), 0, "nothing is sold before the merchant answers");
    run(&lua, "open_merchant({})");
    assert_eq!(int(&lua, "#BATCHES"), 1);
    assert_eq!(int(&lua, "#steps_of(1)"), 2);
}

#[test]
fn multi_marking_asks_once_for_gear_and_never_marks_a_worn_item() {
    let setup = r#"
        view({it_(1, "Rat Pelt"), it_(2, "Rusty Sword", {tabs = {"equipment"}}), it_(3, "Worn Ring", {worn = true, tabs = {"equipment"}})})
        ctrl(1); ctrl(2); ctrl(3); group_menu(1)
    "#;
    let lua = world();
    run(&lua, setup);
    run(&lua, "choose('Mark as junk')");
    assert_eq!(string(&lua, "B.menu.title"), "Mark these items as junk?");
    assert!(boolean(&lua, "next(B.junk_ids) == nil"), "nothing is marked until the player says Yes");
    run(&lua, "choose('No')");
    assert!(boolean(&lua, "next(B.junk_ids) == nil and X.n == 3"), "No leaves the marks and the selection");
    run(&lua, "group_menu(1); choose('Mark as junk'); choose('Yes')");
    assert!(boolean(&lua, "B.junk_ids['1'] == true and B.junk_ids['2'] == true and B.junk_ids['3'] == nil"));
    assert_eq!(string(&lua, "B.result.text"), "Marked 2 items as junk; 1 worn item kept");
    assert_eq!(int(&lua, "X.n"), 0);
    // Unmark, and Mark to bank (instant: neither asks)
    run(&lua, "view(); ctrl(1); ctrl(2); group_menu(1); choose('Unmark')");
    assert!(boolean(&lua, "next(B.junk_ids) == nil"));
    assert_eq!(string(&lua, "B.result.text"), "Unmarked 2 items");
    run(&lua, "view(); ctrl(1); ctrl(3); group_menu(1); choose('Mark to bank')");
    assert!(boolean(&lua, "H.is_banked({id = 1}) and not H.is_banked({id = 3})"));
    assert_eq!(string(&lua, "B.result.text"), "Marked 1 item to bank; 1 worn item kept");

    // negative control: without the question the gear is marked at the first click
    let no_ask = mutated(&[("if not row.confirmed and row.ask and (row.enabled or row.far_kind) then", "if false then")]);
    run(&no_ask, setup);
    run(&no_ask, "choose('Mark as junk')");
    assert!(boolean(&no_ask, "B.junk_ids['2'] == true"), "negative control: marked with no question");
    // and a worn item is marked when the worn filter is off
    let worn = mutated(&[("if flag and it.equipped then", "if false then")]);
    run(&worn, setup);
    run(&worn, "choose('Mark as junk'); choose('Yes')");
    assert!(boolean(&worn, "B.junk_ids['3'] == true"), "negative control: the worn ring was marked");
}

#[test]
fn multi_destroy_sends_one_question_for_the_whole_list_without_the_worn_and_nothing_else() {
    let setup = r#"
        view({it_(1, "Rusty Sword", {worn = true, tabs = {"equipment"}}), it_(2, "Healing Potion", {q = 5, quick = 1}), it_(3, "Rat Pelt", {q = 3})})
        put_npcs(3.0, 3.0)
        ctrl(1); ctrl(2); ctrl(3); group_menu(2)
    "#;
    let lua = world();
    run(&lua, setup);
    run(&lua, "choose('Destroy')");
    assert_eq!(int(&lua, "#DESTROYS"), 1, "one call, one question");
    assert_eq!(string(&lua, "table.concat(DESTROYS[1], ',')"), "ref2,ref3", "the two that are not worn, in pack order");
    assert_eq!(int(&lua, "#BATCHES + #SELLS + #TALKS"), 0, "the mod sends nothing but the question: the client asks, the player answers");
    assert_eq!(string(&lua, "B.result.text"), "Destroy: answer the question on the screen");
    // No: kept, selection cleared
    run(&lua, "B.on_confirm_closed({kind = 'destroy_many', answer = 'no'})");
    assert_eq!(string(&lua, "B.result.text"), "Destroy: kept");
    assert_eq!(int(&lua, "X.n"), 0);
    // Yes: what the client sent, with what was skipped
    run(&lua, "view(); ctrl(1); ctrl(2); ctrl(3); group_menu(2); choose('Destroy'); B.on_confirm_closed({kind = 'destroy_many', answer = 'yes', detail = {sent = true, sent_count = 1, total = 2, skipped = {'Rat Pelt'}}})");
    assert_eq!(string(&lua, "B.result.text"), "Destroyed 1 of 2: Rat Pelt had moved; 1 worn item kept");
    run(&lua, "view(); ctrl(2); ctrl(3); group_menu(2); choose('Destroy'); B.on_confirm_closed({kind = 'destroy_many', answer = 'yes', detail = {sent = true, sent_count = 2, total = 2, skipped = {}}})");
    assert_eq!(string(&lua, "B.result.text"), "Destroyed 2 of 2");
    assert!(boolean(&lua, "B.result.ok == true"));
    // the single Destroy's answer is not taken for the group's
    run(&lua, "B.result = nil; B.on_confirm_closed({kind = 'destroy', answer = 'yes', detail = {sent = true}})");
    assert_eq!(string(&lua, "B.result.text"), "Destroy: sent");

    // negative control: a worn item is not left out of the question
    let worn = mutated(&[("row(\"Destroy...\", #usable > 0, \"all selected items are worn\", function() destroy(usable, worn) end)", "row(\"Destroy...\", #usable > 0, \"x\", function() destroy(items, worn) end)")]);
    run(&worn, setup);
    run(&worn, "choose('Destroy')");
    assert_eq!(int(&worn, "#DESTROYS[1]"), 3, "negative control: the worn sword would be on the list");
}

#[test]
fn multi_the_bank_tab_takes_items_out_the_same_way() {
    let setup = r#"
        BANK.open = true
        BANK.items = {bank_it(21, "Moonstone", 3), bank_it(22, "Spare Dagger", 1)}
        BANK.revision = 2
        B.tab = 6
        B.rebuild()
        ctrl(1); ctrl(2)
        group_menu(2)
    "#;
    let lua = world();
    run(&lua, setup);
    assert_eq!(string(&lua, "B.menu.title"), "2 items selected");
    assert_eq!(string(&lua, "labels()"), "Take out of bank (2)");
    run(&lua, "choose('Take out of bank')");
    assert_eq!(string(&lua, "BATCHES[1].require_window"), "bank");
    assert_eq!(
        string(&lua, "(function() local t = {} for _, s in ipairs(steps_of(1)) do t[#t + 1] = s.kind .. ':' .. s.item .. ':' .. s.count .. ':' .. s.from end return table.concat(t, ' ') end)()"),
        "bank_move:bank21:3:bank bank_move:bank22:1:bank"
    );
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = true, state = 'completed', completed = 2, total = 2})");
    assert_eq!(string(&lua, "B.result.text"), "Took out 2 of 2");
    assert!(boolean(&lua, "RELEASED[#RELEASED] ~= 'bank'"), "on the Bank tab the bank session stays for the window");
    // no banker, bank closed: greyed with the reason
    let shut = world();
    run(&shut, setup);
    run(&shut, "BANK.open = false; X.clear(); ctrl(1); group_menu(1)");
    assert!(boolean(&shut, "row('Take out').enabled == false and row('Take out').reason == 'no banker nearby'"));
    // Put in bank (from the pack), at an open bank: bank_move from the pack
    let dep = world();
    run(&dep, "BANK.open = true; view({it_(1, 'A', {q = 2}), it_(2, 'B')}); ctrl(1); ctrl(2); group_menu(1); choose('Put in bank (2)')");
    assert_eq!(string(&dep, "steps_of(1)[1].kind .. ':' .. steps_of(1)[1].from .. ':' .. steps_of(1)[1].count"), "bank_move:inventory:2");
    run(&dep, "B.on_batch_result({batch_id = B.batch_id, ok = true, state = 'completed', completed = 2, total = 2})");
    assert_eq!(string(&dep, "B.result.text"), "Banked 2 of 2");

    // negative control: a bank item is not moved from the bank
    let wrong = mutated(&[("from = kind == \"deposit\" and \"inventory\" or \"bank\"", "from = \"inventory\"")]);
    run(&wrong, setup);
    run(&wrong, "choose('Take out of bank')");
    assert_eq!(string(&wrong, "steps_of(1)[1].from"), "inventory", "negative control: the direction is wrong");
}

#[test]
fn multi_the_selection_goes_with_the_tab_the_stack_and_the_end_of_an_action() {
    let lua = world();
    run(&lua, "view(SIX()); ctrl(1); ctrl(3)");
    assert_eq!(int(&lua, "X.n"), 2);
    // a stack that leaves the pack leaves the selection (the grid is rebuilt on every change)
    run(&lua, "set_pack({it_(1, 'Rat Pelt'), it_(2, 'Chipped Tooth'), it_(4, 'Cracked Fang')}); B.rebuild()");
    assert_eq!(string(&lua, "selected()"), "Rat Pelt");
    assert_eq!(int(&lua, "X.n"), 1);
    // another tab: the selection is left behind
    run(&lua, "B.select(3)");
    assert_eq!(int(&lua, "X.n"), 0, "a selection belongs to the tab it was made on");
    // the hint line says how many are selected
    run(&lua, "view(SIX()); ctrl(2); ctrl(5)");
    assert_eq!(string(&lua, "X.hint()"), "2 items selected: right-click one for actions");
    // a group action that is already running refuses a second
    run(&lua, "put_npcs(3.0, nil); open_merchant({}); group_menu(2); choose('Sell (2)')");
    assert_eq!(int(&lua, "#BATCHES"), 1);
    run(&lua, "view(); ctrl(1); ctrl(2); group_menu(1); choose('Sell (2)')");
    assert_eq!(int(&lua, "#BATCHES"), 1, "one batch at a time");
    assert!(string(&lua, "B.result.text").contains("another action is still running"));

    // negative controls
    let tab = mutated(&[("if M.multi and tab ~= M.tab then M.multi.clear() end", "")]);
    run(&tab, "view(SIX()); ctrl(1); B.select(3)");
    assert_eq!(int(&tab, "X.n"), 1, "negative control: the selection stayed on another tab");
    let prune = mutated(&[("if M.multi then M.multi.prune(items) end", "")]);
    run(&prune, "view(SIX()); ctrl(1); ctrl(3); set_pack({it_(1, 'Rat Pelt')}); B.rebuild()");
    assert_eq!(int(&prune, "X.n"), 2, "negative control: a gone stack stayed selected");
}
