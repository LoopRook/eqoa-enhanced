// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Batch 8 second review (2026-11-26): the Map Search and Bag Viewer fixes, tested without a window or a GPU. The mods'
//! shipped Lua is loaded into a plain Lua state with a fake `eqoa` (the pattern of `bag_helpers_tests.rs`); `main.lua`
//! itself runs, its event handlers recorded and fired by the test as the client fires them. Each test has a negative
//! control: the same check against the shipped source with the fix's lines put back as they were (the mutations).
use super::bag_helpers_tests::{source, FIXTURE};
use mlua::Lua;

/// Events are recorded and fired by `emit`; anything else the mod reaches for through `AUTO_MT` is callable and returns
/// nothing, so a draw call the test does not care about does nothing.
const FAKE: &str = r##"
HANDLERS, NEXT_TOKEN, TOKENS = {}, 0, {}
AUTO_MT = {}
AUTO_MT.__call = function() return nil end
AUTO_MT.__index = function(self, k) local v = setmetatable({}, AUTO_MT); rawset(self, k, v); return v end
function emit(name, data, time)
  for _, h in ipairs(HANDLERS[name] or {}) do
    if h.live then h.fn({data = data, time = time or 0}) end
  end
end
function events_on(name, fn)
  NEXT_TOKEN = NEXT_TOKEN + 1
  local h = {fn = fn, live = true}
  HANDLERS[name] = HANDLERS[name] or {}
  table.insert(HANDLERS[name], h)
  TOKENS[NEXT_TOKEN] = h
  return NEXT_TOKEN
end
function events_off(token) if TOKENS[token] then TOKENS[token].live = false end end
"##;

fn loader(lua: &Lua, src: String, name: &'static str) -> mlua::Function {
    lua.create_function(move |lua, ()| -> mlua::Result<mlua::Value> { lua.load(src.as_str()).set_name(name).call(()) }).unwrap()
}

/// The shipped source of `path` with each mutation applied; a mutation that matches none of `paths` is a mistake.
fn patched(path: &str, mutations: &[(&str, &str)]) -> String {
    let mut src = source(path);
    for (from, to) in mutations {
        src = src.replace(from, to);
    }
    src
}

fn check_mutations(paths: &[&str], mutations: &[(&str, &str)]) {
    for (from, _) in mutations {
        assert!(paths.iter().any(|p| source(p).contains(from)), "the mutation matches nothing in the shipped source: {from}");
    }
}

fn flag(lua: &Lua, expr: &str) -> bool {
    lua.load(format!("return {expr}")).eval().unwrap()
}
fn int(lua: &Lua, expr: &str) -> i64 {
    lua.load(format!("return {expr}")).eval().unwrap()
}
fn text(lua: &Lua, expr: &str) -> String {
    lua.load(format!("return tostring({expr})")).eval().unwrap()
}

// ---- Map Search ----

const SEARCH_ENV: &str = r##"
LOADS, LIVE = {}, {}
eqoa = setmetatable({
  events = {on = events_on, off = events_off},
  input = {bind = function() end, capture_text = function() end, claim_pointer = function() end},
  files = {load_json = function(path)
    LOADS[#LOADS + 1] = path
    if path:find("^@") then return nil, {code = "not_yet"} end
    return "req" .. #LOADS
  end},
  fonts = {load = function() return nil end},
  map = {get = function() return {open = false, minimap = true, world_id = 0, enabled = true} end, markers = function() return {} end, clear_guide = function() end},
  player = {get = function() return {id = 1, name = "Hero", position = {x = 0, y = 0, z = 0}} end},
  entities = {list = function() return LIVE end},
  group = {get = function() return nil end},
  hud = {visible = function() return true end},
  binds = {get = function() return "" end},
  mods = {send = function() return true end},
  imgui = false, -- no drawing: the panel's state is what the test reads
}, AUTO_MT)
package.loaded["search"], package.loaded["spawns"] = nil, nil
package.preload["spawns"] = function() return SPAWNS_LOADER() end
package.preload["search"] = function() return SEARCH_LOADER() end
"##;

fn map_world(mutations: &[(&str, &str)]) -> Lua {
    check_mutations(&["eqoa_map_search/modules/search.lua"], mutations);
    let lua = Lua::new();
    lua.globals().set("SEARCH_LOADER", loader(&lua, patched("eqoa_map_search/modules/search.lua", mutations), "search.lua")).unwrap();
    lua.globals().set("SPAWNS_LOADER", loader(&lua, source("eqoa_map_search/modules/spawns.lua"), "spawns.lua")).unwrap();
    lua.load(FAKE).exec().unwrap();
    lua.load(SEARCH_ENV).exec().unwrap();
    lua
}

/// One load replaces a name table while the index is part-way through the old one and another load is still queued:
/// the index must not carry its place in the old table into the new one (Lua's "invalid key to 'next'").
const INDEX_REPLACED: &str = r##"
S = require("search")
local old = {}
for i = 1, 3200 do old["old" .. i] = {world = {0}, pos = {{i, 0, i}}} end
S.load(old, "catalog")
local guard = 0
while #S.jobs > 0 do S.step(); guard = guard + 1; assert(guard < 2000, "the load did not end") end
S.step() -- starts the index
assert(S.index_job and S.index_job.phase == 1 and S.index_job.key ~= nil, "the index is part-way through the old table")
-- two more loads are queued: a catalogue with other names, and a journal
S.load({new1 = {world = {0}, pos = {{1, 0, 1}}}}, "catalog")
S.load({}, "journal")
STEP_ERRORS = {}
for _ = 1, 200 do
  local ok, err = pcall(S.step)
  if not ok then STEP_ERRORS[#STEP_ERRORS + 1] = tostring(err); break end
end
"##;

#[test]
fn batch8_h2a_a_load_that_replaces_a_name_table_does_not_leave_the_index_walking_the_old_one() {
    let lua = map_world(&[]);
    lua.load(INDEX_REPLACED).exec().unwrap();
    assert_eq!(int(&lua, "#STEP_ERRORS"), 0, "{}", text(&lua, "STEP_ERRORS[1]"));
    assert!(flag(&lua, "S.npc_cache ~= nil and S.index_job == nil"), "the index finished");
    assert_eq!(int(&lua, "#S.npc_cache"), 1, "the new names only");
    assert_eq!(text(&lua, "S.npc_cache[1].name"), "new1");
    // negative control: the walk is kept after a load replaces its table, as it was
    let bad = map_world(&[("M.index_job=nil -- its walk", "local _kept=nil -- its walk"), ("if not j or #M.jobs>0 then return end", "if not j then return end")]);
    bad.load(INDEX_REPLACED).exec().unwrap();
    assert_eq!(int(&bad, "#STEP_ERRORS"), 1, "negative control: the stale walk raises an error");
    assert!(text(&bad, "STEP_ERRORS[1]").contains("invalid key to 'next'"), "{}", text(&bad, "STEP_ERRORS[1]"));
}

/// main.lua running: the catalogue's read fails and the journal is never there. The panel may say "Searching..." while the
/// read is out; once it failed, a typed query finds the live creature and the panel stops saying it.
const NOTHING_LOADS: &str = r##"
S = require("search")
local rat = {id = 9, name = "Test Rat", is_creature = true, corpse = false, hp_fraction = 1, search_flags = 0, class_code = 0,
  position = {x = 5, y = 0, z = 5}, world_id = 0, level = 3}
LIVE = {rat}
T = 0
function frames(n) for _ = 1, n do T = T + 0.1; emit("render", nil, T) end end
S.toggle()
S.query = "rat"
S.dirty = true
frames(30)
WAITING_WHILE_READING = S.npc_cache == nil and (S.dirty or S.build ~= nil)
emit("resource.failed", {kind = "json", request = "req1"}, T)
frames(60)
"##;

fn run_nothing_loads(lua: &Lua) {
    // the mod's main.lua as shipped: `require("search")` gives it the module the test also holds
    lua.load("S = require('search')").exec().unwrap();
    lua.load(source("eqoa_map_search/main.lua").as_str()).set_name("main.lua").exec().unwrap();
    lua.load(NOTHING_LOADS).exec().unwrap();
}

#[test]
fn batch8_h2b_with_no_catalogue_and_no_journal_a_live_creature_is_still_found_and_searching_ends() {
    let lua = map_world(&[]);
    run_nothing_loads(&lua);
    assert!(flag(&lua, "WAITING_WHILE_READING"), "while the catalogue is being read the panel waits for the index");
    assert!(flag(&lua, "not S.dirty and S.build == nil"), "the panel stopped saying Searching...");
    assert_eq!(int(&lua, "#S.rows"), 1, "the live creature is found");
    assert_eq!(text(&lua, "S.rows[1].name"), "Test Rat");
    assert_eq!(int(&lua, "#S.catalog"), 0, "(there is no catalogue, so the panel's NPC list line can show for an empty result)");
    // negative control: the wait without the way out (the index is waited for for ever)
    let bad = map_world(&[(
        "if #M.jobs>0 or M.index_job or M.index_stale or (M.loads_pending and M.loads_pending()) then M.want_index=true;return end",
        "do M.want_index=true;return end",
    )]);
    run_nothing_loads(&bad);
    assert!(flag(&bad, "S.dirty"), "negative control: still Searching...");
    assert_eq!(int(&bad, "#S.rows"), 0, "negative control: no creature found");
}

// ---- Bag Viewer ----

const BAG_FILES: [&str; 3] = ["eqoa_bag_viewer/modules/bags.lua", "eqoa_bag_viewer/modules/helpers.lua", "eqoa_bag_viewer/main.lua"];

fn bag_world(mutations: &[(&str, &str)]) -> Lua {
    check_mutations(&BAG_FILES, mutations);
    let lua = Lua::new();
    lua.globals().set("BAGS_LOADER", loader(&lua, patched(BAG_FILES[0], mutations), "bags.lua")).unwrap();
    lua.globals().set("HELPERS_LOADER", loader(&lua, patched(BAG_FILES[1], mutations), "helpers.lua")).unwrap();
    lua.globals().set("MULTI_LOADER", loader(&lua, source("eqoa_bag_viewer/modules/multi.lua"), "multi.lua")).unwrap();
    lua.globals().set("AUCTION_LIST_LOADER", loader(&lua, source("eqoa_bag_viewer/modules/auction_list.lua"), "auction_list.lua")).unwrap();
    lua.globals().set("REPAIR_LOADER", loader(&lua, source("eqoa_bag_viewer/modules/repair.lua"), "repair.lua")).unwrap();
    lua.load(FAKE).exec().unwrap();
    lua.load(FIXTURE).exec().unwrap();
    lua.load(
        r##"
        eqoa.events = {on = events_on, off = events_off}
        for _, v in pairs(eqoa) do if type(v) == "table" then setmetatable(v, AUTO_MT) end end
        setmetatable(eqoa, AUTO_MT)
        package.loaded["helpers"] = H
        package.preload["multi"] = function() return MULTI_LOADER() end
        package.preload["auction_list"] = function() return AUCTION_LIST_LOADER() end
        package.preload["repair"] = function() return REPAIR_LOADER() end
        -- character Alice saved "off" for all three helpers; Bob has no saved choice
        FILES["helpers.cfg"] = "auto:Alice:sell=0\nauto:Alice:restock=0\nauto:Alice:bank=0\n"
        POLLS = 0
        function world_changes_to(name)
          PL.name = name
          NOW = NOW + 2
          emit("player.changed", {after = {name = name}}, 0)
        end
        function polls_in_three_frames()
          POLLS = 0
          for i = 1, 3 do emit("render", nil, i) end
          return POLLS
        end
        "##,
    )
    .exec()
    .unwrap();
    // main.lua as shipped; the poll is counted instead of run (main.lua calls HP.auto_poll() through the table)
    lua.load(patched(BAG_FILES[2], mutations).as_str()).set_name("main.lua").exec().unwrap();
    lua.load("H.auto_poll = function() POLLS = POLLS + 1 end").exec().unwrap();
    lua
}

const NO_HOOK: (&str, &str) = ("if M.on_character then M.on_character(name) end", "local _unused = nil");

#[test]
fn batch8_m2_the_helpers_poll_follows_the_character_in_the_world_both_ways() {
    let lua = bag_world(&[]);
    // character Alice (saved off for every helper) comes into the world: the poll that waited for a character stops
    lua.load("world_changes_to('Alice')").exec().unwrap();
    assert_eq!(int(&lua, "polls_in_three_frames()"), 0, "Alice has all three off: no poll");
    assert!(flag(&lua, "not H.auto_on('bank')"), "her saved off stays off");
    // the player logs in as Bob, who has no saved choice: Bank is on, and the poll runs
    lua.load("world_changes_to('Bob')").exec().unwrap();
    assert!(flag(&lua, "H.auto_on('bank')"), "Bank is on by default for Bob");
    assert!(int(&lua, "polls_in_three_frames()") > 0, "Bob's Bank needs the poll: nothing waits for a click");
    // and back to Alice: the poll stops, her off is not overridden
    lua.load("world_changes_to('Alice')").exec().unwrap();
    assert_eq!(int(&lua, "polls_in_three_frames()"), 0, "back to Alice: no poll");
    assert!(flag(&lua, "not H.auto_on('bank') and not H.auto_on('sell') and not H.auto_on('restock')"), "her saved off was never overridden");
    // the order of events in the review: the poll was already off (a switch was clicked) when Bob came
    let late = bag_world(&[]);
    late.load("world_changes_to('Alice'); H.on_auto_change()").exec().unwrap();
    assert_eq!(int(&late, "polls_in_three_frames()"), 0);
    late.load("world_changes_to('Bob')").exec().unwrap();
    assert!(int(&late, "polls_in_three_frames()") > 0, "Bob after an Alice whose poll was off");
    // negative control: without the hook the poll is only re-read by a switch click
    let bad = bag_world(&[NO_HOOK]);
    bad.load("world_changes_to('Alice'); H.on_auto_change()").exec().unwrap();
    assert_eq!(int(&bad, "polls_in_three_frames()"), 0);
    bad.load("world_changes_to('Bob')").exec().unwrap();
    assert!(flag(&bad, "H.auto_on('bank')"), "(negative control) Bank shows as on for Bob");
    assert_eq!(int(&bad, "polls_in_three_frames()"), 0, "negative control: nothing is ever banked");
}
