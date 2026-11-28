// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The owner's play on build 4936f2c0 (2026-11-27): the Bag Viewer's item tooltip (a long line that ran off the edge, stats
//! shown as "Stat 0 +40") and the Corpse tab. Tested without a window or a GPU: the shipped `bags.lua` and `helpers.lua` in a
//! plain Lua state with the fake `eqoa` of `bag_helpers_tests.rs` (its fixture); each test has a negative control (the same
//! scenario against the shipped source with one line changed). The drawn window (the wrapped tooltip, the Auction line, the
//! target after a helper's pass) is in `bag_viewer_tests.rs`, `owner_h_*`.
use super::bag_helpers_tests::{FIXTURE, source};
use mlua::{Lua, Table};

fn world_with(mutations: &[(&str, &str)]) -> Lua {
    let lua = Lua::new();
    let mut bags = source("eqoa_bag_viewer/modules/bags.lua");
    let mut helpers = source("eqoa_bag_viewer/modules/helpers.lua");
    for (from, to) in mutations {
        assert!(bags.contains(from) || helpers.contains(from), "the mutation matches nothing in the shipped source: {from}");
        bags = bags.replace(from, to);
        helpers = helpers.replace(from, to);
    }
    let loader = |code: String, name: &'static str| {
        lua.create_function(move |lua, ()| -> mlua::Result<Table> { lua.load(code.as_str()).set_name(name).call(()) }).unwrap()
    };
    lua.globals().set("BAGS_LOADER", loader(bags, "bags.lua")).unwrap();
    lua.globals().set("HELPERS_LOADER", loader(helpers, "helpers.lua")).unwrap();
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

/// What the tooltip reads beyond the helpers' fixture: the game's stat names (as the client gives them when its words are
/// loaded), the table of stat ids by key, and the wear slots' names.
const EXTRA: &str = r##"
WORDS = {}
STATS = {str = 0, sta = 1, agi = 2, dex = 3, wis = 4, int = 5, cha = 6, hp = 8, pow = 10, ac = 14}
eqoa.items.stat_name = function(id) if WORDS.stats then return WORDS.stats[id] end return nil, {code = "unavailable"} end
eqoa.items.stats = STATS
eqoa.items.slot_info = function(slot) if WORDS.slots then return {id = slot, name = WORDS.slots[slot]} end return nil, {code = "unavailable"} end
function gear(stats)
  local it = item(1, "Fists of Might", {tabs = {"equipment"}})
  it.level_required, it.wear_slot, it.allowed_classes, it.allowed_races = 59, 12, {}, {}
  it.stats, it.maximum_hp, it.current_hp, it.durability = stats, 11000, 11000, 60
  it.description, it.value = "", 4792
  return it
end
function card_text(it)
  B.hover = nil
  local t = {}
  for _, l in ipairs(B.card(it)) do t[#t + 1] = l[1] end
  return table.concat(t, "|")
end
"##;

#[test]
fn owner_h_a_stat_without_a_name_in_the_item_is_named_from_the_games_words_or_the_stat_table() {
    let lua = world();
    // the item came without the stats' names (the client read it before its words were loaded)
    run(&lua, "IT = gear({{id = 0, value = 40}, {id = 8, value = 75}})");
    // the words are there now: the card asks the client for the name
    run(&lua, "WORDS.stats = {[0] = 'Strength', [8] = 'Hit Points'}");
    let card = string(&lua, "card_text(IT)");
    assert!(card.contains("Strength +40|Hit Points +75"), "{card}");
    // the words are not loaded (the client says unavailable): the name comes from the stat table, not "Stat 0"
    let lua = world();
    run(&lua, "IT = gear({{id = 0, value = 40}, {id = 8, value = 75}})");
    let card = string(&lua, "card_text(IT)");
    assert!(card.contains("Strength +40|Hit points +75"), "{card}");
    assert!(!card.contains("Stat 0") && !card.contains("Stat 8"), "{card}");
    // a name the item brings is kept; an id nobody knows stays "Stat N"
    run(&lua, "IT = gear({{id = 2, name = 'Agility', value = 5}, {id = 77, value = 3}})");
    let card = string(&lua, "card_text(IT)");
    assert!(card.contains("Agility +5|Stat 77 +3"), "{card}");
    // the slot's name too, when the item came without it
    run(&lua, "WORDS.slots = {[12] = 'Hands'}; IT = gear({})");
    assert!(string(&lua, "card_text(IT)").contains("Slot: Hands"));
}

#[test]
fn owner_h_the_tooltip_wraps_a_long_line_to_its_width_and_keeps_every_word() {
    let lua = world();
    // a text 6 units a character; the tooltip is 200 wide
    run(
        &lua,
        "WIDTH = function(s) return #s * 6 end
         LINES = {{'Fists of Might', 'title'}, {'Class: Warrior, Ranger, Paladin, Shadowknight, Monk, Bard, Rogue, Druid', 'text'}, {'HP 11000/11000', 'text'}}
         OUT = B.wrap_card(LINES, 200, WIDTH)",
    );
    assert!(int(&lua, "#OUT") > 3, "the long line became several");
    assert!(boolean(&lua, "(function() for _, l in ipairs(OUT) do if WIDTH(l[1]) > 200 then return false end end return true end)()"), "no line is wider than the tooltip");
    // every word is still there, in order, and the lines after the first keep the line's colour kind
    let joined = string(&lua, "(function() local t = {} for _, l in ipairs(OUT) do t[#t + 1] = l[1] end return table.concat(t, ' ') end)()");
    assert_eq!(joined, "Fists of Might Class: Warrior, Ranger, Paladin, Shadowknight, Monk, Bard, Rogue, Druid HP 11000/11000");
    assert_eq!(string(&lua, "OUT[2][2] .. OUT[3][2]"), "texttext");
    // a short card is untouched (the same lines)
    run(&lua, "SHORT = B.wrap_card({{'Rat Pelt', 'title'}, {'Level 3', 'text'}}, 200, WIDTH)");
    assert_eq!(int(&lua, "#SHORT"), 2);
    // a word wider than the tooltip is cut, never left to run off
    run(&lua, "LONGWORD = B.wrap_card({{'Abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz', 'text'}}, 120, WIDTH)");
    assert!(boolean(&lua, "#LONGWORD > 1 and WIDTH(LONGWORD[1][1]) <= 120"));
    // negative control: without the wrapping the long line stays as it was (it runs off the edge)
    let bad = mutated(&[("function M.wrap_card(lines, maxw, width)", "function M.wrap_card(lines, maxw, width) if true then return lines end")]);
    run(&bad, "WIDTH = function(s) return #s * 6 end; OUT = B.wrap_card({{'Class: Warrior, Ranger, Paladin, Shadowknight, Monk, Bard, Rogue, Druid', 'text'}}, 200, WIDTH)");
    assert_eq!(int(&bad, "#OUT"), 1, "negative control: one wide line");
}

#[test]
fn owner_h_the_corpse_tab_shows_while_a_corpse_is_in_reach_and_is_left_when_there_is_none() {
    // the facts the owner asked about: a corpse in the zone list showed the tab whatever its distance (the owner's yes of
    // 2026-11-28 limits it to a corpse in loot reach that is not known to be empty: `bag_corpse_tab_tests.rs`); with none the
    // tab is gone and a selection of it falls back to All
    let lua = world();
    run(&lua, "NPCS = {{id = 600, kind = 'npc', corpse = true, name = 'a rat', position = {x = 3, y = 0, z = 0}}}; B.scan_reset()");
    assert_eq!(string(&lua, "(function() local t = {} for _, x in ipairs(B.visible_tabs()) do t[#t + 1] = x.label end return table.concat(t, ',') end)()"), "All,Usable,Equipment,Charged,Misc,Bank,Corpse");
    run(&lua, "B.tab = 7; B.rebuild()");
    assert_eq!(int(&lua, "B.tab"), 7, "selected, empty: the corpse is near and can be looked at");
    run(&lua, "NPCS = {}; B.scan_reset(); B.rebuild()");
    assert_eq!(int(&lua, "B.tab"), 1, "the corpse is gone: the tab is not left selected");
    assert!(!string(&lua, "(function() local t = {} for _, x in ipairs(B.visible_tabs()) do t[#t + 1] = x.label end return table.concat(t, ',') end)()").contains("Corpse"));
}
