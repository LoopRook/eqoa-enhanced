// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Auto-loot follow-ups (2026-11-21), the Bag Viewer's own decisions, tested without a window or a GPU: the bundled
//! `bags.lua` is loaded as shipped into a plain Lua state with a fake `eqoa` and a clock the test moves.
//! The native half (the by-id request, the fallback, the relists) is in `fork_ext/items_tests.rs`
//! (`autoloot_*`).
use mlua::{Lua, Table};

fn bags_source() -> &'static str {
    let file = crate::fork_ext::bundled::EMBEDDED_FILES
        .iter()
        .find(|f| f.path == "eqoa_bag_viewer/modules/bags.lua")
        .expect("the Bag Viewer ships bags.lua");
    std::str::from_utf8(file.data).unwrap()
}

const FIXTURE: &str = r##"
NOW = 100.0
os.clock = function() return NOW end
LOG, OPENS, LOOTALLS, BATCHES, RELEASED = {}, {}, {}, {}, {}
PL = {id = 77, position = {x = 0, y = 0, z = 0}, fighting = false, auto_attacking = false, target_id = nil}
ENT, LIST, LOOT, OPEN_FAILS, NATIVE_MODE, TAKEN = {}, {}, nil, nil, 0, false
INV = {remaining = 5, capacity = 40, used = 35}
eqoa = {
  player = {get = function() return PL end},
  entities = {find = function(id) return ENT[id] end, list = function() return LIST end},
  group = {get = function() return nil end},
  inventory = {get = function() return INV end},
  loot = {get = function() return LOOT end},
  windows = {
    get = function() return {native_mode = NATIVE_MODE} end,
    release = function(k) RELEASED[#RELEASED + 1] = k; TAKEN = false end,
    takeover = function() TAKEN = true return true end,
    taken = function() return TAKEN end,
  },
  actions = {
    open_loot = function(id, opts)
      OPENS[#OPENS + 1] = {id = id, keep = opts and opts.keep_target or false}
      if OPEN_FAILS then return nil, OPEN_FAILS end
      return "open" .. #OPENS
    end,
    loot_all = function()
      if LOOTALL_ERR then return nil, LOOTALL_ERR end
      LOOTALLS[#LOOTALLS + 1] = true
      return "all" .. #LOOTALLS
    end,
    batch = function(spec) BATCHES[#BATCHES + 1] = spec return "batch" .. #BATCHES end,
    cancel = function() end,
    stop_batch = function() return true end,
  },
  log = {write = function(level, line) LOG[#LOG + 1] = line end},
  files = {read = function() return nil end, write = function() return true end},
}
B = BAGS_LOADER()
B.auto_mode = "all"

function logged(needle)
  for _, l in ipairs(LOG) do if l:find(needle, 1, true) then return true end end
  return false
end
function count_logged(needle)
  local n = 0
  for _, l in ipairs(LOG) do if l:find(needle, 1, true) then n = n + 1 end end
  return n
end
-- a creature of yours dies where it stood, 2 units away
function kill(id, x)
  ENT[id] = {id = id, corpse = true, kind = "npc", name = "a gnoll", position = {x = x or 2, y = 0, z = 0}}
  B.mark_engaged(id)
end
function poll() B.auto_poll(true) end
function later(s) NOW = NOW + s end
-- the server's answer to the last open_loot, with the detail the client gives
function open_answered(method)
  B.on_action_result({request_id = "open" .. #OPENS, ok = true, state = "completed", phase = "server", kind = "open_loot",
    detail = method and {method = method} or {}})
end
function corpse_list(n, junk)
  local items = {}
  for i = 1, n do items[i] = {name = "Item " .. i, id = 3000 + i, quantity = 1, ref = "ref" .. i, junk = junk} end
  LOOT = {items = items, open = true, source = {entity_id = 9300}}
  for _, it in ipairs(items) do if it.junk then B.junk_ids[tostring(it.id)] = true end end
end
"##;

fn world() -> Lua {
    let lua = Lua::new();
    let src = bags_source().to_string();
    let loader = lua
        .create_function(move |lua, ()| -> mlua::Result<Table> {
            lua.load(src.as_str()).set_name("bags.lua").call(())
        })
        .unwrap();
    lua.globals().set("BAGS_LOADER", loader).unwrap();
    lua.load(FIXTURE).exec().unwrap();
    lua
}

fn int(lua: &Lua, expr: &str) -> i64 {
    lua.load(format!("return {expr}")).eval().unwrap()
}
fn boolean(lua: &Lua, expr: &str) -> bool {
    lua.load(format!("return {expr}")).eval().unwrap()
}
fn run(lua: &Lua, code: &str) {
    lua.load(code).exec().unwrap();
}

/// A fight in progress: a gnoll is on auto-attack and has you targeted, and another creature of yours just died.
fn in_a_fight(lua: &Lua) {
    run(
        lua,
        r#"
        ENT[9200] = {id = 9200, kind = "npc", name = "a gnoll scout", position = {x = 3, y = 0, z = 0}, target_id = 77, disposition_name = "Hostile"}
        LIST = {ENT[9200]}
        PL.fighting, PL.auto_attacking, PL.target_id = true, true, 9200
        kill(9300)
    "#,
    );
}

#[test]
fn autoloot_opens_the_corpse_by_id_in_the_middle_of_a_fight() {
    let lua = world();
    in_a_fight(&lua);
    run(&lua, "poll()");
    assert_eq!(int(&lua, "#OPENS"), 1, "a fight is no reason to wait when the corpse is opened by id");
    assert!(boolean(&lua, "OPENS[1].id == 9300 and OPENS[1].keep == true"), "asked by id, keep_target");
    assert!(boolean(&lua, "logged('opening it')"));
    assert!(!boolean(&lua, "logged('waiting: in combat')"));
}

#[test]
fn autoloot_waits_out_a_fight_only_once_the_client_says_it_opens_the_old_way() {
    // negative control: the old method retargets, which ends the auto-attack, so the combat wait is still there for it
    let lua = world();
    in_a_fight(&lua);
    run(&lua, "B.loot_method = 'retarget'; poll()");
    assert_eq!(int(&lua, "#OPENS"), 0, "the old method must not open a corpse in a fight");
    assert!(boolean(&lua, "logged('waiting: in combat')"));
    // the fight ends: it goes on
    run(&lua, "PL.fighting, PL.auto_attacking, ENT[9200] = false, false, nil; LIST = {}; later(1); B.scan_reset(); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1);
}

#[test]
fn autoloot_learns_the_method_from_the_open_result() {
    // keep_target: the result says so
    let lua = world();
    run(&lua, "kill(9300); poll(); open_answered('keep_target')");
    assert!(boolean(&lua, "B.loot_method == 'keep_target'"));
    assert!(boolean(&lua, "logged('opened by their id')"));
    // an answer with no method (a client that does not know the option, or a session that fell back): the old way
    let lua = world();
    run(&lua, "kill(9300); poll(); open_answered(nil)");
    assert!(boolean(&lua, "B.loot_method == 'retarget'"), "no method in the detail means the target was changed");
    assert!(boolean(&lua, "logged('the old way')"));
    // and a fallback says so
    let lua = world();
    run(&lua, "kill(9300); poll(); open_answered('retarget')");
    assert!(boolean(&lua, "B.loot_method == 'retarget'"));
    assert_eq!(int(&lua, "count_logged('opened the old way')"), 1, "said once");
}

#[test]
fn a_manual_open_without_the_option_never_changes_what_auto_loot_believes() {
    // the Loot button asks the old way on purpose and its answer carries no method
    let lua = world();
    run(&lua, "kill(9300); ENT[9300].position = {x = 2, y = 0, z = 0}; LIST = {ENT[9300]}; B.open_loot_corpse(nil)");
    assert!(boolean(&lua, "OPENS[1].keep == false"), "no keep_target for a button");
    run(&lua, "open_answered(nil)");
    assert!(boolean(&lua, "B.loot_method == nil"), "a button's open says nothing about the method");
}

#[test]
fn not_ready_is_asked_again_in_half_a_second_and_is_not_a_try() {
    let lua = world();
    run(&lua, "kill(9300); OPEN_FAILS = {code = 'not_ready', message = 'cannot open corpse right now'}; poll()");
    assert_eq!(int(&lua, "#OPENS"), 1);
    run(&lua, "later(0.3); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1, "too soon");
    run(&lua, "later(0.3); poll()");
    assert_eq!(int(&lua, "#OPENS"), 2, "half a second on it asks again, not five seconds");
    assert_eq!(int(&lua, "B.auto_retry[9300].n"), 0, "never a try");
    assert!(boolean(&lua, "logged('trying again in 0.5 s')"));
    // negative control: the old five-second wait is what the setting used to say
    let lua = world();
    run(&lua, "B.AUTO_NOTREADY_SECS = 5.0; kill(9300); OPEN_FAILS = {code = 'not_ready', message = 'cannot open corpse right now'}; poll(); later(0.6); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1, "with the old wait it is still waiting after 0.6 s");
}

/// The corpse opens (its list arrives, then the open's answer), the take is `Take all`, and the server's answer to it
/// leaves `left` items on the corpse.
fn take_all_ends(lua: &Lua, left: usize) {
    run(
        lua,
        &format!(
            "corpse_list({left} == 0 and 1 or {left}); open_answered('keep_target'); \
             if {left} == 0 then LOOT = nil end; \
             B.on_action_result({{request_id = 'all' .. #LOOTALLS, ok = true, state = 'completed', phase = 'server', kind = 'loot_all'}}); LOOT = nil"
        ),
    );
}

#[test]
fn a_take_that_left_items_behind_is_asked_again_twice_and_then_left_alone() {
    let lua = world();
    run(&lua, "kill(9300); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1);
    // try 1 ends with items still on the corpse (the bags were full)
    take_all_ends(&lua, 2);
    assert!(boolean(&lua, "logged('unfinished')"), "said in the log");
    assert!(boolean(&lua, "B.auto_done == nil or not B.auto_done[9300]"), "not finished");
    run(&lua, "later(4); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1, "not at once: it waits a moment");
    run(&lua, "later(7); poll()");
    assert_eq!(int(&lua, "#OPENS"), 2, "ask again once it has been a moment and the player is within reach");
    // try 2 ends the same way
    take_all_ends(&lua, 2);
    run(&lua, "later(11); poll()");
    assert_eq!(int(&lua, "#OPENS"), 3, "two more tries in all");
    // try 3 ends the same way: that was the last
    take_all_ends(&lua, 2);
    assert!(boolean(&lua, "logged('still unfinished after 2 more tries')"));
    run(&lua, "later(60); poll()");
    assert_eq!(int(&lua, "#OPENS"), 3, "never a fourth time");
    // every try is in the log, not swallowed as a repeat
    assert_eq!(int(&lua, "count_logged('opening it')"), 3);
}

#[test]
fn a_take_that_took_everything_is_finished() {
    // negative control for the retry: nothing left on the corpse means done, and no second open ever
    let lua = world();
    run(&lua, "kill(9300); poll()");
    take_all_ends(&lua, 0);
    run(&lua, "later(60); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1);
    assert!(boolean(&lua, "B.auto_done[9300] == true"));
    assert!(boolean(&lua, "logged('done: Take all ok')"));
}

#[test]
fn a_take_cut_short_is_asked_again_when_the_server_closed_it_but_not_when_the_player_did() {
    // (a) the window went away on its own (the server closed it, the player walked off): asked again
    let lua2 = world();
    run(
        &lua2,
        "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(2); B.junk_ids['3002'] = true; open_answered('keep_target')",
    );
    assert_eq!(int(&lua2, "#BATCHES"), 1, "a corpse with junk on it is taken item by item");
    run(
        &lua2,
        "B.note_loot_closed({before = {}, after = nil, closed_by = 'server'}); \
         B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', code = 'window_closed', reason = 'corpse window closed', completed = 0, total = 1}); LOOT = nil",
    );
    assert!(boolean(&lua2, "logged('unfinished')"));
    run(&lua2, "later(11); poll()");
    assert_eq!(int(&lua2, "#OPENS"), 2, "the server's close ended the take early: asked again");

    // (b) the player closed the window: theirs, never asked again (negative control for (a))
    let lua3 = world();
    run(
        &lua3,
        "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(2); B.junk_ids['3002'] = true; open_answered('keep_target'); \
         B.note_loot_closed({before = {}, after = nil, closed_by = 'player'}); \
         B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', code = 'window_closed', reason = 'corpse window closed', completed = 0, total = 1}); LOOT = nil",
    );
    assert!(boolean(&lua3, "logged('you closed it')"), "said why it is left alone");
    run(&lua3, "later(120); kill(9300); poll()");
    assert_eq!(int(&lua3, "#OPENS"), 1, "a corpse the player closed is not asked for again");
    assert!(boolean(&lua3, "B.auto_done[9300] == true"));
}

#[test]
fn the_players_stop_button_and_loot_tab_close_are_theirs_too() {
    for closer in ["B.stop_batch()", "B.tab = 7; B.select(1)"] {
        let lua = world();
        run(
            &lua,
            "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(2); B.junk_ids['3002'] = true; open_answered('keep_target')",
        );
        run(&lua, closer);
        run(
            &lua,
            "B.on_batch_result({batch_id = B.batch_id or 'batch1', ok = false, state = 'stopped', code = 'cancelled', reason = 'stopped', completed = 0, total = 1}); LOOT = nil",
        );
        run(&lua, "later(60); kill(9300); poll()");
        assert_eq!(int(&lua, "#OPENS"), 1, "{closer}: a corpse the player stopped is not asked for again");
    }
}

#[test]
fn junk_left_on_a_corpse_is_not_a_reason_to_ask_again() {
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(2); B.junk_ids['3002'] = true; open_answered('keep_target')");
    assert_eq!(int(&lua, "#BATCHES"), 1);
    // the batch took the one non-junk item: done, with the junk left (today's rule)
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = true, state = 'completed', completed = 1, total = 1}); LOOT = nil");
    assert!(boolean(&lua, "logged('junk left')"));
    run(&lua, "later(120); kill(9300); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1, "the junk stays where it is");
    // a corpse of nothing but junk
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(1); B.junk_ids['3001'] = true; open_answered('keep_target')");
    assert!(boolean(&lua, "logged('only junk on it, left there')"));
    run(&lua, "LOOT = nil; later(120); kill(9300); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1);
}

#[test]
fn a_corpse_with_no_junk_is_one_take_all_not_one_take_and_one_reopen_per_item() {
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(3); open_answered('keep_target')");
    assert_eq!(int(&lua, "#BATCHES"), 0, "no per-item batch");
    assert_eq!(int(&lua, "#LOOTALLS"), 1, "one Take all");
    // negative control: one junk item on it and the per-item take comes back
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(3); B.junk_ids['3003'] = true; open_answered('keep_target')");
    assert_eq!(int(&lua, "#BATCHES"), 1);
    assert_eq!(int(&lua, "#LOOTALLS"), 0);
}

#[test]
fn the_games_loot_window_showing_during_auto_loot_is_said_once() {
    let lua = world();
    run(&lua, "kill(9300); poll(); open_answered('keep_target')");
    assert!(!boolean(&lua, "logged('SHOWN')"), "hidden: nothing to say");
    run(&lua, "NATIVE_MODE = 1; later(0.3); poll(); later(0.3); poll()");
    assert_eq!(int(&lua, "count_logged('SHOWN')"), 1, "one line, however long it stays");
    assert!(boolean(&lua, "logged('game\\'s loot window was SHOWN during Auto-loot')"));
    // negative control: a corpse the player opened themselves is theirs to see
    let lua = world();
    run(&lua, "kill(9300); NATIVE_MODE = 1; corpse_list(1); poll(); later(0.3); poll()");
    assert!(boolean(&lua, "logged('it is open already')"), "taken from the player's own window");
    assert!(!boolean(&lua, "logged('SHOWN')"));
}

#[test]
fn creatures_you_hit_are_followed_every_tenth_of_a_second_and_the_idle_poll_stays_slow() {
    let lua = world();
    run(&lua, "B.AUTO_POLL_SECS = 0.25; B.mark_engaged(9400); ENT[9400] = {id = 9400, kind = 'npc', name = 'a rat', position = {x = 2, y = 0, z = 0}}; poll()");
    let base = int(&lua, "B.poll_count");
    run(&lua, "later(0.05); B.auto_poll()");
    assert_eq!(int(&lua, "B.poll_count"), base, "50 ms: too soon");
    run(&lua, "later(0.06); B.auto_poll()");
    assert_eq!(int(&lua, "B.poll_count"), base + 1, "110 ms on: a fight is followed this closely");
    // negative control: nothing engaged keeps the quarter-second poll
    let lua = world();
    run(&lua, "B.AUTO_POLL_SECS = 0.25; poll()");
    let base = int(&lua, "B.poll_count");
    run(&lua, "later(0.11); B.auto_poll()");
    assert_eq!(int(&lua, "B.poll_count"), base, "idle: still a quarter second");
    run(&lua, "later(0.15); B.auto_poll()");
    assert_eq!(int(&lua, "B.poll_count"), base + 1);
}

// ---- Auto-loot fix round 1 (2026-11-22) ------------------------------------------------------------------------------

#[test]
fn autoloot_fix1_attempts_that_never_opened_the_corpse_do_not_use_up_the_retries() {
    // after an unfinished take, "the game could not open it now" (casting, another window) is asked again every half
    // second and is no attempt: the two more tries are for real reopens
    let lua = world();
    run(&lua, "kill(9300); poll()");
    take_all_ends(&lua, 2);
    assert_eq!(int(&lua, "B.auto_more[9300].left"), 2);
    run(&lua, "OPEN_FAILS = {code = 'not_ready', message = 'cannot open corpse right now'}; later(11); poll(); later(0.6); poll(); later(0.6); poll()");
    assert_eq!(int(&lua, "#OPENS"), 4, "the first open and three refused asks");
    assert_eq!(int(&lua, "B.auto_more[9300].left"), 2, "refusals used none of the retries");
    // the corpse opens at last: that is the first of the two more tries
    run(&lua, "OPEN_FAILS = nil; later(0.6); poll(); corpse_list(2); open_answered('keep_target')");
    assert_eq!(int(&lua, "B.auto_more[9300].left"), 1);
}

#[test]
fn autoloot_fix1_a_corpse_the_player_closed_does_not_block_another_corpse() {
    // the player left the Loot tab of corpse A (not the one Auto-loot is taking): B's take that ends early is still asked again
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(2); B.junk_ids['3002'] = true; open_answered('keep_target')");
    run(&lua, "B.note_loot_closed({before = {source = {entity_id = 9999}}, after = nil, closed_by = 'player'})");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', code = 'window_closed', reason = 'corpse window closed', completed = 0, total = 1}); LOOT = nil");
    assert!(boolean(&lua, "logged('unfinished')"), "another corpse's close is not this one's");
    assert!(!boolean(&lua, "logged('you closed it')"));
    // control: the same close naming this corpse is the player's
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(2); B.junk_ids['3002'] = true; open_answered('keep_target')");
    run(&lua, "B.note_loot_closed({before = {source = {entity_id = 9300}}, after = nil, closed_by = 'player'})");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', code = 'window_closed', reason = 'corpse window closed', completed = 0, total = 1}); LOOT = nil");
    assert!(boolean(&lua, "logged('you closed it')"));
}

#[test]
fn autoloot_fix1_stop_and_the_tab_belong_to_the_corpse_being_taken_only() {
    // Stop while corpse A's batch runs, then corpse B's take ends early within five seconds: B is asked again
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(2); B.junk_ids['3002'] = true; open_answered('keep_target'); B.stop_batch()");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'stopped', code = 'cancelled', reason = 'stopped', completed = 0, total = 1}); LOOT = nil");
    assert!(boolean(&lua, "logged('you closed it')"), "A was stopped by the player");
    run(&lua, "later(1); kill(9301, 3); poll(); corpse_list(2); LOOT.source.entity_id = 9301; B.junk_ids['3002'] = true; open_answered('keep_target')");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', code = 'window_closed', reason = 'corpse window closed', completed = 0, total = 1}); LOOT = nil");
    assert!(boolean(&lua, "B.auto_done == nil or not B.auto_done[9301]"), "B was not the one the player stopped");
    assert!(boolean(&lua, "B.auto_more ~= nil and B.auto_more[9301] ~= nil"), "B is scheduled to be asked again");
}

#[test]
fn autoloot_fix1_a_corpse_the_player_opened_is_guarded_again_when_the_mod_reopens_it() {
    let lua = world();
    run(&lua, "kill(9300); NATIVE_MODE = 1; corpse_list(2); poll(); later(0.3); poll()");
    assert!(!boolean(&lua, "logged('SHOWN')"), "the player's own window is theirs");
    // that take ended with items left: asked again later, this time by the mod, and the game's window must stay hidden
    run(&lua, "B.on_action_result({request_id = 'all1', ok = true, state = 'completed', phase = 'server', kind = 'loot_all'}); LOOT = nil; NATIVE_MODE = 0");
    run(&lua, "later(11); poll(); corpse_list(2); open_answered('keep_target'); NATIVE_MODE = 1; later(0.3); poll()");
    assert!(boolean(&lua, "logged('SHOWN')"), "the mod's own take is guarded: a window showing now is the mod's fault");
}

// ---- Auto-loot, bags full (owner decision 2026-11-22) -----------------------------------------------------------------

/// The take ended with items left and the pack has `free` free slots.
fn take_ends_with_items_left(lua: &Lua, free: usize) {
    run(lua, &format!("INV.remaining = {free}"));
    take_all_ends(lua, 2);
}

#[test]
fn autoloot_bags_full_waits_for_free_space_not_for_a_timer() {
    let lua = world();
    run(&lua, "kill(9300); poll()");
    take_ends_with_items_left(&lua, 0);
    assert!(boolean(&lua, "logged('free bag space')"), "said in the log");
    assert!(boolean(&lua, "B.auto_more == nil or B.auto_more[9300] == nil"), "not on the short-retry budget");
    // however long it takes, with full bags nothing is asked
    run(&lua, "later(300); poll(); later(300); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1, "no timer");
    // space is freed (sold junk, dropped an item): it goes back
    run(&lua, "INV.remaining = 1; poll()");
    assert_eq!(int(&lua, "#OPENS"), 2, "the rest is taken when there is room");
}

#[test]
fn autoloot_other_causes_keep_the_short_timer_with_free_bags() {
    // negative control: the same early end with free bag space is not a bags-full case
    let lua = world();
    run(&lua, "kill(9300); poll()");
    take_ends_with_items_left(&lua, 5);
    assert!(!boolean(&lua, "logged('free bag space')"));
    run(&lua, "later(6); poll()");
    assert_eq!(int(&lua, "#OPENS"), 2, "asked again after a few seconds");
}

#[test]
fn autoloot_bags_full_has_no_try_limit() {
    let lua = world();
    run(&lua, "kill(9300); poll()");
    for round in 1..=5 {
        take_ends_with_items_left(&lua, 0);
        run(&lua, "INV.remaining = 1; later(1); poll()");
        assert_eq!(int(&lua, "#OPENS"), round + 1, "round {round}: back again when there is room");
    }
}

#[test]
fn autoloot_bags_full_does_not_chase_a_player_who_walked_away() {
    let lua = world();
    run(&lua, "kill(9300); poll()");
    take_ends_with_items_left(&lua, 0);
    run(&lua, "PL.position = {x = 30, y = 0, z = 0}; later(1); poll()");
    assert!(boolean(&lua, "logged('you left reach')"));
    // coming back with free space changes nothing: it was dropped
    run(&lua, "PL.position = {x = 0, y = 0, z = 0}; INV.remaining = 3; later(1); kill(9300); poll(); later(10); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1, "dropped, not chased");
    // control: still in reach with free space it goes back (the test above)
}

#[test]
fn autoloot_bags_full_corpse_that_vanished_is_forgotten() {
    let lua = world();
    run(&lua, "kill(9300); poll()");
    take_ends_with_items_left(&lua, 0);
    run(&lua, "ENT[9300] = nil; later(1); poll()");
    assert!(boolean(&lua, "B.auto_bags[9300] == nil"));
}

#[test]
fn autoloot_a_batch_that_stops_on_a_full_pack_waits_for_space_too() {
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(2); B.junk_ids['3002'] = true; open_answered('keep_target'); INV.remaining = 0");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', code = 'not_ready', reason = 'inventory is full', completed = 0, total = 1}); LOOT = nil");
    assert!(boolean(&lua, "logged('free bag space')"));
    run(&lua, "later(100); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1);
    run(&lua, "INV.remaining = 2; poll()");
    assert_eq!(int(&lua, "#OPENS"), 2);
}

#[test]
fn autoloot_a_corpse_the_player_closed_is_left_alone_even_with_full_bags() {
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(2); B.junk_ids['3002'] = true; open_answered('keep_target'); INV.remaining = 0");
    run(&lua, "B.note_loot_closed({before = {source = {entity_id = 9300}}, after = nil, closed_by = 'player'})");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', code = 'not_ready', reason = 'inventory is full', completed = 0, total = 1}); LOOT = nil");
    assert!(boolean(&lua, "logged('you closed it')"));
    run(&lua, "INV.remaining = 5; later(100); kill(9300); poll()");
    assert_eq!(int(&lua, "#OPENS"), 1);
}

// ---- Auto-loot fix round 2 (2026-11-22) ------------------------------------------------------------------------------

/// The pack is already full when Take all runs: the server cannot move anything, and the client's answer to the take is
/// "did not change the corpse" (`ok = nil`), a refusal (`ok = false`) or the take refused at once.
#[test]
fn autoloot_fix2_bags_already_full_before_take_all_wait_for_space_not_for_a_timer() {
    for (how, result) in [
        ("did not change the corpse", "ok = nil, state = 'unknown', code = 'unknown', reason = 'loot_all did not change corpse state'"),
        ("refused by the server", "ok = false, state = 'refused', code = 'refused', reason = 'You cannot loot that corpse.'"),
    ] {
        let lua = world();
        run(&lua, "kill(9300); poll(); INV.remaining = 0; corpse_list(2); open_answered('keep_target')");
        run(
            &lua,
            &format!("B.on_action_result({{request_id = 'all1', {result}, phase = 'server', kind = 'loot_all'}}); LOOT = nil"),
        );
        assert!(boolean(&lua, "logged('free bag space')"), "{how}: the bags-full wait");
        assert!(boolean(&lua, "B.auto_bags ~= nil and B.auto_bags[9300] ~= nil"), "{how}");
        assert!(boolean(&lua, "B.auto_more == nil or B.auto_more[9300] == nil"), "{how}: not the short retries");
        run(&lua, "later(60); poll()");
        assert_eq!(int(&lua, "#OPENS"), 1, "{how}: no timer");
        run(&lua, "INV.remaining = 2; poll()");
        assert_eq!(int(&lua, "#OPENS"), 2, "{how}: back when there is room");
    }
    // control: with free bag space the same answer is an ordinary unfinished take (the short retries)
    let lua = world();
    run(&lua, "kill(9300); poll(); INV.remaining = 5; corpse_list(2); open_answered('keep_target')");
    run(
        &lua,
        "B.on_action_result({request_id = 'all1', ok = nil, state = 'unknown', code = 'unknown', reason = 'loot_all did not change corpse state', phase = 'server', kind = 'loot_all'}); LOOT = nil",
    );
    assert!(!boolean(&lua, "logged('free bag space')"));
    assert!(boolean(&lua, "B.auto_more[9300] ~= nil"));
}

#[test]
fn autoloot_fix2_take_all_refused_and_batches_with_full_bags_wait_for_space() {
    // Take all refused at once (the game busy) with a full pack
    let lua = world();
    run(&lua, "kill(9300); poll(); INV.remaining = 0; LOOTALL_ERR = {code = 'busy', message = 'loot transaction already pending'}; corpse_list(2); open_answered('keep_target')");
    assert!(boolean(&lua, "logged('free bag space')"), "the refused Take all with full bags");
    // a batch cut short (window gone) while the pack is full and non-junk items are left
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(3); B.junk_ids['3003'] = true; open_answered('keep_target'); INV.remaining = 0");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', code = 'window_closed', reason = 'corpse window closed', completed = 1, total = 2}); ");
    assert!(boolean(&lua, "logged('free bag space')"), "the batch cut short with full bags");
    // control: free bag space, the same batch end: the short retries
    let lua = world();
    run(&lua, "B.auto_mode = 'non_junk'; kill(9300); poll(); corpse_list(3); B.junk_ids['3003'] = true; open_answered('keep_target'); INV.remaining = 5");
    run(&lua, "B.on_batch_result({batch_id = B.batch_id, ok = false, state = 'refused', code = 'window_closed', reason = 'corpse window closed', completed = 1, total = 2}); ");
    assert!(!boolean(&lua, "logged('free bag space')"));
    assert!(boolean(&lua, "B.auto_more[9300] ~= nil"));
}

#[test]
fn autoloot_fix2_the_zone_the_window_and_the_disconnect_forget_what_was_waiting() {
    // a corpse waiting for bag space and one on the short retries, then the world changes: a new creature that gets the
    // same entity id is a new corpse
    let lua = world();
    run(&lua, "kill(9300); poll(); INV.remaining = 0; corpse_list(2); open_answered('keep_target'); B.on_action_result({request_id = 'all1', ok = true, state = 'completed', phase = 'server', kind = 'loot_all'}); LOOT = nil");
    run(&lua, "B.auto_more = {[9301] = {left = 0, at = NOW}}; B.last_close = {by = 'player', at = NOW}; B.auto_player_open = 9300");
    run(&lua, "B.clear_corpse_hold()");
    assert!(boolean(&lua, "next(B.auto_bags or {}) == nil and next(B.auto_more or {}) == nil"), "nothing waits for bag space or retries");
    assert!(boolean(&lua, "B.last_close == nil and B.auto_player_open == nil"));
    // the new creature with the same id dies out of reach: it is waited for, not dropped as the old corpse was
    run(&lua, "ENT[9300] = nil; INV.remaining = 0; PL.position = {x = 20, y = 0, z = 0}; kill(9300, 2); later(1); poll()");
    assert!(!boolean(&lua, "logged('you left reach')"));
    assert!(boolean(&lua, "B.auto_done == nil or B.auto_done[9300] == nil"));
}

#[test]
fn autoloot_fix2_a_refused_reopen_of_a_corpse_waiting_for_bag_space_follows_the_open_cooldowns() {
    let lua = world();
    run(&lua, "kill(9300); poll(); INV.remaining = 0; corpse_list(2); open_answered('keep_target'); B.on_action_result({request_id = 'all1', ok = true, state = 'completed', phase = 'server', kind = 'loot_all'}); LOOT = nil");
    // still full over many polls: the wait is said once
    run(&lua, "for i = 1, 20 do later(0.1); poll() end");
    assert_eq!(int(&lua, "count_logged('bags are still full')"), 1, "said once per wait, not every poll");
    // space is freed while the player is casting: the game cannot open it
    run(&lua, "INV.remaining = 1; OPEN_FAILS = {code = 'not_ready', message = 'cannot open corpse right now'}; poll()");
    assert_eq!(int(&lua, "#OPENS"), 2, "one ask");
    run(&lua, "later(0.1); poll(); later(0.1); poll(); later(0.1); poll()");
    assert_eq!(int(&lua, "#OPENS"), 2, "not again every poll");
    run(&lua, "later(0.5); poll()");
    assert_eq!(int(&lua, "#OPENS"), 3, "again after the not-ready wait");
}

#[test]
fn autoloot_fix2_a_corpse_the_player_opens_while_bags_are_full_is_not_taken_until_there_is_room() {
    let lua = world();
    run(&lua, "kill(9300); poll(); INV.remaining = 0; corpse_list(2); open_answered('keep_target'); B.on_action_result({request_id = 'all1', ok = true, state = 'completed', phase = 'server', kind = 'loot_all'}); LOOT = nil");
    let before = int(&lua, "#LOOTALLS");
    // the player opens it themselves, bags still full
    run(&lua, "corpse_list(2); later(1); poll()");
    assert_eq!(int(&lua, "#LOOTALLS"), before, "no take while the bags are full");
    // control: room again, the same open is taken from
    run(&lua, "INV.remaining = 3; later(1); poll()");
    assert_eq!(int(&lua, "#LOOTALLS"), before + 1);
}

#[test]
fn autoloot_fix2_a_disconnect_forgets_which_way_corpses_were_opened() {
    let lua = world();
    run(&lua, "B.loot_method = 'retarget'; B.keep_req = 'open9'; B.on_disconnect()");
    assert!(boolean(&lua, "B.loot_method == nil and B.keep_req == nil"));
}
