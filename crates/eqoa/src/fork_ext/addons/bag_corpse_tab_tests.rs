// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The Bag Viewer's Corpse tab (the owner's yes, 2026-11-28): it shows only while there is a corpse within loot reach that is
//! not known to be empty, or while the game's corpse window is open, or while a Take-all batch keeps it. Tested without a
//! window or a GPU: the shipped `bags.lua` and `helpers.lua` in a plain Lua state with the fake `eqoa` of
//! `bag_helpers_tests.rs` (its fixture); each test has a negative control (the same scenario against the shipped source with
//! one line of the feature changed), which must give the other answer.
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

/// Corpses in the zone list at a distance from the player (at 0, 0, 0), the corpse window, and the open request.
const EXTRA: &str = r##"
OPENS = {}
eqoa.actions.open_loot = function(id, opts) OPENS[#OPENS + 1] = {id = id} return "open" .. #OPENS end
function corpses(list)
  NPCS = {}
  for _, c in ipairs(list) do
    NPCS[#NPCS + 1] = {id = c[1], kind = "npc", corpse = true, name = "a rat", position = {x = c[2], y = 0, z = 0}}
  end
  B.scan_reset()
end
function tabs()
  local t = {}
  for _, x in ipairs(B.visible_tabs()) do t[#t + 1] = x.label end
  return table.concat(t, ",")
end
-- what the client says of a corpse window: the corpse it names and what it lists
function window(id, items)
  LOOT = {open = true, kind = "loot", items = items, source = id and {entity_id = id, name = "a rat"} or nil, revision = 1}
  B.note_corpse_contents({before = nil, after = LOOT, cause = "loot_open"})
end
function window_closes()
  LOOT = nil
  B.note_corpse_contents({before = {}, after = nil, cause = "loot_close"})
end
"##;

const BASE: &str = "All,Usable,Equipment,Charged,Misc,Bank";

#[test]
fn corpse_tab_a_far_corpse_shows_no_tab_and_one_in_reach_shows_it() {
    let lua = world();
    // the zone list holds a corpse 30 units away: not in loot reach
    run(&lua, "corpses({{600, 30}})");
    assert_eq!(string(&lua, "tabs()"), BASE, "a corpse far away: no tab");
    // one 3 units away is in reach
    run(&lua, "corpses({{600, 3}})");
    assert_eq!(string(&lua, "tabs()"), format!("{BASE},Corpse"));
    // the reach is the one the mod has learned from the server's answers: a corpse opened at 12 units makes 10 near enough
    run(&lua, "corpses({{600, 10}})");
    assert_eq!(string(&lua, "tabs()"), BASE, "10 units: past the default reach of 7");
    run(&lua, "B.note_loot_reach(12, true); B.scan_reset()");
    assert_eq!(string(&lua, "tabs()"), format!("{BASE},Corpse"), "the server has opened one at 12");
    // negative control: without the distance check the far corpse shows the tab, as before this change
    let bad = mutated(&[("if c.d2 <= reach * reach and not M.emptied[c.e.id]", "if true and not M.emptied[c.e.id]")]);
    run(&bad, "corpses({{600, 30}})");
    assert_eq!(string(&bad, "tabs()"), format!("{BASE},Corpse"), "negative control: the far corpse showed the tab");
}

#[test]
fn corpse_tab_an_emptied_corpse_in_reach_shows_none_and_one_with_loot_left_keeps_it() {
    let lua = world();
    run(&lua, "corpses({{600, 3}})");
    assert_eq!(string(&lua, "tabs()"), format!("{BASE},Corpse"));
    // the player's own window lists something left: still a corpse with loot (the window is open, the tab shows)
    run(&lua, "window(600, {{name = 'Rat Pelt'}}); window_closes()");
    assert_eq!(string(&lua, "tabs()"), format!("{BASE},Corpse"), "closed with an item left on it: it still has loot");
    // the window then lists nothing: emptied (a Take all, or the last item taken)
    run(&lua, "window(600, {}); window_closes()");
    assert_eq!(string(&lua, "tabs()"), BASE, "emptied: no tab");
    // another corpse in reach that nobody has opened still shows it (unknown is not empty)
    run(&lua, "corpses({{600, 3}, {601, 5}})");
    assert_eq!(string(&lua, "tabs()"), format!("{BASE},Corpse"));
    // the corpse named by the request when the window gave none
    run(&lua, "corpses({{602, 3}}); B.opening_id = 602; LOOT = {open = true, items = {}, revision = 2}; B.note_corpse_contents({after = LOOT}); LOOT = nil; B.note_corpse_contents({before = {}})");
    assert_eq!(string(&lua, "tabs()"), BASE, "the corpse the mod asked for was empty");
    // a corpse that leaves the zone is forgotten: the id may come again for another
    run(&lua, "corpses({}); tabs(); corpses({{602, 3}})");
    assert_eq!(string(&lua, "tabs()"), format!("{BASE},Corpse"), "a new corpse with the same id");
    // negative control: an empty window changes nothing (the corpse is taken for one with loot)
    let bad = mutated(&[("if #(after.items or {}) == 0 then M.emptied[id] = true else M.emptied[id] = nil end", "M.emptied[id] = nil")]);
    run(&bad, "corpses({{600, 3}}); window(600, {}); window_closes()");
    assert_eq!(string(&bad, "tabs()"), format!("{BASE},Corpse"), "negative control: the emptied corpse still showed the tab");
}

#[test]
fn corpse_tab_the_open_corpse_window_and_a_take_all_batch_keep_it_with_no_corpse_in_reach() {
    let lua = world();
    run(&lua, "corpses({})");
    assert_eq!(string(&lua, "tabs()"), BASE);
    // the game's corpse window is open (a corpse far, or already emptied: the window is what is shown)
    run(&lua, "corpses({{600, 30}}); window(600, {})");
    assert_eq!(string(&lua, "tabs()"), format!("{BASE},Corpse"), "the open window keeps the tab");
    run(&lua, "window_closes()");
    assert_eq!(string(&lua, "tabs()"), BASE);
    // a Take-all batch keeps the tab it is on
    run(&lua, "B.tab = 7; B.batch_id = 'batch1'; B.batch_kind = 'loot'");
    assert_eq!(string(&lua, "tabs()"), format!("{BASE},Corpse"), "a batch is keeping it");
    // negative control: without the window clause the open window shows no tab
    let bad = mutated(&[("if eqoa.loot.get() or keeping_corpse_tab() or M.lootable_corpse(M.DRAW_TTL) ~= nil then", "if keeping_corpse_tab() or M.lootable_corpse(M.DRAW_TTL) ~= nil then")]);
    run(&bad, "corpses({}); window(600, {{name = 'Rat Pelt'}})");
    assert_eq!(string(&bad, "tabs()"), BASE, "negative control: the open window showed no tab");
}

#[test]
fn corpse_tab_a_selected_tab_falls_back_to_all_when_it_goes_and_the_buttons_open_the_corpse_in_reach() {
    let lua = world();
    run(&lua, "corpses({{600, 3}}); B.tab = 7; B.rebuild()");
    assert_eq!(int(&lua, "B.tab"), 7, "selected, the corpse is in reach");
    run(&lua, "window(600, {}); window_closes(); B.rebuild()");
    assert_eq!(int(&lua, "B.tab"), 1, "emptied: the tab goes and the selection falls back to All");
    // two corpses in reach: the nearer one is empty, the other has loot: Look at corpse opens the one with loot
    let setup = "corpses({{600, 2}, {601, 5}}); window(600, {}); window_closes()";
    let lua = world();
    run(&lua, setup);
    assert_eq!(int(&lua, "B.lootable_corpse().id"), 601);
    run(&lua, "B.start_loot_button()");
    assert_eq!(string(&lua, "#OPENS .. ':' .. tostring(OPENS[1].id)"), "1:601", "the request names the corpse that has loot, not the nearest");
    // negative control: asking for the nearest (no id) would open the empty one
    let bad = mutated(&[("return M.open_loot_corpse(nil, c and c.id) ~= nil", "return M.open_loot_corpse(nil) ~= nil")]);
    run(&bad, setup);
    run(&bad, "B.start_loot_button()");
    assert_eq!(string(&bad, "tostring(OPENS[1].id)"), "nil", "negative control: no corpse named, the nearest (the empty one) is asked for");
}

#[test]
fn corpse_tab_the_take_buttons_ask_for_the_corpse_with_loot_and_auto_loots_path_is_as_it_was() {
    // the nearer corpse (600) is known empty, the other (601) is not
    let setup = "corpses({{600, 2}, {601, 5}}); window(600, {}); window_closes()";
    let lua = world();
    run(&lua, setup);
    run(&lua, "B.take_all_button()");
    assert_eq!(string(&lua, "#OPENS .. ':' .. tostring(OPENS[1].id)"), "1:601", "the Take all button names the corpse that has loot");
    let lua = world();
    run(&lua, setup);
    run(&lua, "B.take_non_junk_button()");
    assert_eq!(string(&lua, "#OPENS .. ':' .. tostring(OPENS[1].id)"), "1:601", "so does Take all but junk");
    // Auto-loot's finish of an open it asked for (and the player's attack-key path) call `start_loot_all` with no window: as
    // before this change it asks for the nearest corpse the old way (no id), known empty or not
    let lua = world();
    run(&lua, setup);
    run(&lua, "B.start_loot_all()");
    assert_eq!(string(&lua, "#OPENS .. ':' .. tostring(OPENS[1].id)"), "1:nil", "Auto-loot's path is untouched");
    // negative control: if the button asked as Auto-loot's path does, it would ask for the nearest (the empty one)
    let bad = mutated(&[("    if eqoa.loot.get() then return start() end\n    local c = M.lootable_corpse()", "    if true then return start() end\n    local c = M.lootable_corpse()")]);
    run(&bad, setup);
    run(&bad, "B.take_all_button()");
    assert_eq!(string(&bad, "tostring(OPENS[1].id)"), "nil", "negative control: no corpse named");
    // nothing in reach with loot: the button says so and asks for nothing
    let lua = world();
    run(&lua, "corpses({{600, 2}}); window(600, {}); window_closes(); B.take_all_button()");
    assert_eq!(string(&lua, "#OPENS"), "0");
    assert_eq!(string(&lua, "B.result.text"), "Corpse has no items");
}
