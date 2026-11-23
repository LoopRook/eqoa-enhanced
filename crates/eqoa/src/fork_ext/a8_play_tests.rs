// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A8 from Play-shaped state: the server's crafting replies read for a mod's window, the recipe list and
//! detail, the tray, one combine, the keep-this-recipe question, and the game's own windows unchanged with the mod off.
//!
//! Each test says what its negative control is: the behaviour without the change would fail the assertion named.
use super::*;
use crate::fork_ext::addons::{
    action_broker,
    host::Host,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws, window_takeover,
};
use net::stream::{Reader, Writer};
use vi::strtab::StringTable;

fn words() -> StringTable {
    let mut lines = vec![String::new(); 1700];
    for (id, s) in [
        (1583, "Weaponsmithing"),
        (1584, "Armorsmithing"),
        (1585, "Jewel Crafting"),
        (1598, "The items you chose do not make anything."),
        (1599, "The items you chose do not make anything (2)."),
        (1600, "You discovered %S! %S Keep this recipe?"),
        (1601, "This item is already in the combine list."),
        (1602, "The combine list is full."),
        (1648, "C"),
    ] {
        lines[id - 1] = s.to_string();
    }
    StringTable::parse(lines.join("\n").as_bytes()).unwrap()
}

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    super::dropped();
    SENT.with(|s| s.borrow_mut().clear());
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p.session.conn = Some(usize::MAX);
    p.words = Some(words());
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    p
}

/// An item as the server sends it, read back with the client's own reader. `component` sets the flag the tray checks.
fn item_bytes(id: i64, name: &str, key: u32, quantity: i64) -> Vec<u8> {
    let mut w = Writer::default();
    w.signed(quantity).signed(0).signed(0).signed(-1).signed(-1).u32(key);
    for v in [id, 10, 0, 4001, 0, -1, 0, 0, 1, 0, 0, 0, 0, 1, 20, 0, 0, -1, -1, 0, 0, 0, 0] {
        w.signed(v);
    }
    w.wide(name).wide("a thing").signed(0);
    w.data
}

fn item(id: i64, name: &str, key: u32, quantity: i64, component: bool) -> net::dump::NetItem {
    let mut it = net::dump::read_net_item(&mut Reader::new(&item_bytes(id, name, key, quantity)));
    it.item.p54 = component as i64;
    it
}

fn rig_play() -> Play {
    let mut p = ready();
    p.craft.token = 0x0102_0304;
    p.craft.slots[3] = net::dump::CraftSlot([0, 7, 2, 0]);
    p.dump.inventory = net::dump::ItemList {
        p04: 0,
        items: vec![
            item(900_001, "Rusty Sword", 11, 1, false),
            item(900_002, "Iron Ore", 12, 5, true),
            item(900_003, "Pine Log", 13, 2, true),
            item(900_004, "Silk", 14, 3, true),
        ],
    };
    p
}

fn rig() -> (Play, Host) {
    let p = rig_play();
    let host = Host::with_test_capabilities(&[
        "eqoa.actions.*",
        "eqoa.craft.get",
        "eqoa.windows.session",
        "eqoa.events.on",
    ])
    .unwrap();
    host.execute(
        r#"
        results = {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        "#,
    )
    .unwrap();
    (p, host)
}

fn step(p: &mut Play, host: &Host) {
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

/// The mod opens its window and the game steps (the window's opening reads the bags, as the game's experiment does).
fn open(p: &mut Play, host: &Host, kind: &str) {
    host.execute(&format!("s_{kind} = assert(eqoa.windows.open_session('{kind}'))")).unwrap();
    step(p, host);
}

/// The mod asks, the game steps, the mod hears. The last result's `ok`, `code` and `reason`.
fn act(p: &mut Play, host: &Host, code: &str) -> (bool, String, String) {
    host.execute(&format!("results = {{}}; last_err = nil; local r, e = (function() {code} end)(); last_err = e")).unwrap();
    step(p, host);
    let (ok, c, r): (Option<bool>, Option<String>, Option<String>) = host
        .lua
        .load("local r = results[#results]; if r == nil then return nil, last_err and last_err.code, last_err and last_err.message end return r.ok, r.code, r.reason")
        .eval()
        .unwrap();
    (ok.unwrap_or(false), c.unwrap_or_default(), r.unwrap_or_default())
}

fn sent() -> Vec<Vec<u8>> {
    SENT.with(|s| s.borrow().clone())
}

fn recipe_list(token: u32, recipes: &[(i32, &str, u8, u8)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(15).u32(token).u32(recipes.len() as u32);
    for (i, (id, name, skill, comp)) in recipes.iter().enumerate() {
        w.signed(0).signed(i as i64).signed(*id as i64).u8(5).u8(*skill).u8(*comp).u8(0).wide(name);
    }
    w.data
}

fn detail_reply(token: u32) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(16).u32(token);
    w.signed(7).signed(-1).signed(-1).u8(0).u8(3).u8(0).u8(2);
    w.data.extend(4u16.to_le_bytes());
    w.u8(9).wide("Iron Ring").wide("Shiny.").signed(1).signed(0).signed(3).signed(2).wide("Iron Ore");
    w.data.extend(2u16.to_le_bytes());
    w.wide("Silk");
    w.data.extend(1u16.to_le_bytes());
    w.signed(1).wide("Hammer").u8(0);
    w.data
}

/// The server's answer to an experiment that found a recipe (Recv12 sub-op 7): the report and the item made.
fn discovery_reply(token: u32, recipe: i32) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(12).u8(7).u32(token);
    for v in [5i64, 3, 2, 4, 1, 25, 0, recipe as i64] {
        w.u32(v as i32 as u32);
    }
    w.u32(0);
    w.data.extend(item_bytes(900_100, "Iron Ring", 99, 1));
    w.data
}

fn snapshot_json() -> serde_json::Value {
    native_a8::current().expect("the window's state is read")
}

fn game_hook_reads(p: &mut Play, data: &[u8]) {
    // what the game's own arm does (`play.rs`, the 0x1102 arm) when the fork's hook does not take the reply
    let modes = Modes { recipe: p.mode == 6, experiment: p.mode == 37 };
    let words = words_of(p);
    let out = p.craft.recv(data, modes, &words, 0);
    p.craft_outs.extend(out);
}

#[test]
fn a8_the_book_and_the_detail_are_read_while_a_mod_window_is_open_and_missed_without_the_hook() {
    let (mut p, host) = rig();
    let book = recipe_list(0x0A0B_0C0D, &[(11, "Iron Ring", 3, 0), (12, "Silk Hood", 3, 1)]);
    // control: the game's own reading, as `play.rs` does it with its mode at 0, drops the book. This is what the
    // mod's window would get with no hook: nothing
    game_hook_reads(&mut p, &book);
    assert!(p.craft.recipes.is_empty(), "the game reads the book only while its own window (mode 6) is up");
    assert_eq!(p.craft.token, 0x0A0B_0C0D, "the token is read either way");
    // with the mod's window open the hook reads it
    p.craft.token = 0x0102_0304;
    open(&mut p, &host, "recipe_book");
    assert!(reply(&mut p, &book), "taken for the mod's window");
    assert_eq!(p.craft.recipes.iter().map(|r| r.id).collect::<Vec<_>>(), vec![11, 12]);
    let state = snapshot_json();
    assert_eq!(state["recipes"].as_array().unwrap().len(), 2);
    assert_eq!(state["recipes"][0]["name"], "Iron Ring");
    // the detail the same way, with how many of each ingredient the bags hold
    assert!(reply(&mut p, &detail_reply(1)));
    let d = &snapshot_json()["detail"];
    assert_eq!(d["name"], "Iron Ring");
    assert_eq!(d["ingredients"][0], serde_json::json!({"name": "Iron Ore", "need": 2, "have": 5}));
    assert_eq!(d["ingredients"][1], serde_json::json!({"name": "Silk", "need": 1, "have": 3}));
    assert_eq!(d["tools"][0], serde_json::json!({"name": "Hammer", "have": false}));
    // and a mod reads it through the API
    host.execute(
        r#"
        local c = assert(eqoa.craft.get())
        assert(#c.recipes == 2 and c.recipes[2].name == "Silk Hood" and c.pending == "none")
        assert(#c.skills == 6 and c.skills[4].level == 7 and c.skills[4].rank == 2)
        "#,
    )
    .unwrap();
    // the game's own window up: it reads its own replies, the hook leaves them
    p.mode = 6;
    assert!(!reply(&mut p, &book), "the game's window has it");
    p.mode = 0;
    // with the window gone there is nothing to read and the game is as it was
    host.execute("assert(eqoa.windows.close_session(s_recipe_book.ref))").unwrap();
    step(&mut p, &host);
    assert!(!reply(&mut p, &book), "no mod window, no hook: the game's own rule");
    assert!(native_a8::current().is_none());
    host.execute("assert(eqoa.craft.get() == nil)").unwrap();
}

#[test]
fn a8_with_the_mod_off_the_games_own_crafting_windows_work_as_they_did() {
    let (mut p, _host) = rig();
    let book = recipe_list(1, &[(11, "Iron Ring", 3, 0)]);
    // no mod window: the hook never takes a reply, and the game's own recipe book (mode 6) reads the list as always
    assert!(!reply(&mut p, &book));
    p.mode = 6;
    game_hook_reads(&mut p, &book);
    assert_eq!(p.craft.recipes.len(), 1, "the game's recipe book still gets its list");
    // the game's experiment: its notice still goes to its own window (the out the window draws)
    p.mode = 37;
    let mut notice = Writer::default();
    notice.u8(12).u8(3).u32(5);
    assert!(!reply(&mut p, &notice.data));
    game_hook_reads(&mut p, &notice.data);
    assert!(
        p.craft_outs.iter().any(|o| matches!(o, CraftOut::Notify(t, _) if wide(t).contains("do not make anything"))),
        "the game's window is told: {:?}",
        p.craft_outs
    );
    // and its Use still opens the window and copies the bags (the start menu's Experiment)
    let inv = p.dump.inventory.clone();
    let out = p.craft.use_id(0xA702_0000, 0, &inv);
    assert_eq!(out, vec![CraftOut::OpenExperiment]);
    assert_eq!(p.craft.items.items.len(), 4);
    // nothing of ours was sent
    assert!(sent().is_empty());
}

#[test]
fn a8_actions_need_the_mods_own_window_and_wait_for_the_games_own_to_go() {
    let (mut p, host) = rig();
    // no window: refused, nothing sent (the control for the line below, where the same call works)
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.craft_list(3)");
    assert!(!ok && code == "not_ready", "{code}");
    assert!(sent().is_empty());
    open(&mut p, &host, "recipe_book");
    let (ok, code, reason) = act(&mut p, &host, "return eqoa.actions.craft_list(3)");
    assert!(ok, "{code} {reason}");
    assert_eq!(sent().len(), 1);
    // the game's own crafting window up: refused busy
    p.craft.pending = 0;
    p.mode = 37;
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.craft_list(2)");
    assert!(!ok && code == "busy", "{code}");
    assert_eq!(sent().len(), 1);
}

#[test]
fn a8_list_and_inspect_send_the_games_own_requests_one_at_a_time() {
    let (mut p, host) = rig();
    open(&mut p, &host, "recipe_book");
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_list(3)");
    assert!(ok);
    // sub-op 6: the token, the skill; the game waits (5)
    assert_eq!(sent()[0], vec![0x01, 0x11, 6, 4, 3, 2, 1, 3, 0, 0, 0]);
    assert_eq!(p.craft.pending, 5);
    host.execute("assert(eqoa.craft.get() ~= nil)").unwrap();
    assert_eq!(snapshot_json()["pending"], "list");
    assert_eq!(snapshot_json()["listed_skill"], 3);
    // a second request while the first is out: refused, never queued behind it, never sent
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.craft_inspect(11)");
    assert!(!ok && code == "stale_handle", "the recipe is not in the book yet: {code}");
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.craft_list(2)");
    assert!(!ok && code == "busy", "{code}");
    assert_eq!(sent().len(), 1, "nothing was sent for either");
    // the list arrives, then the inspect goes (sub-op 7)
    assert!(reply(&mut p, &recipe_list(0x0A0B_0C0D, &[(11, "Iron Ring", 3, 0)])));
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_inspect(11)");
    assert!(ok);
    assert_eq!(sent()[1], vec![0x01, 0x11, 7, 0x0D, 0x0C, 0x0B, 0x0A, 11, 0, 0, 0]);
    // the control for 'busy': once the detail arrives the next request is accepted again
    assert!(reply(&mut p, &detail_reply(1)));
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_list(2)");
    assert!(ok);
    assert_eq!(sent().len(), 3);
}

#[test]
fn a8_the_tray_keeps_the_games_limits_and_a_fill_is_all_or_nothing() {
    let (mut p, host) = rig();
    open(&mut p, &host, "experiment");
    host.execute("c = assert(eqoa.craft.get()); assert(#c.items == 3, 'components only'); iron, pine, silk = c.items[1], c.items[2], c.items[3]").unwrap();
    // the rows are the bag copy's: Iron Ore is the second item of the bags, so row 1 of the components... by slot
    let (ok, c, r) = act(&mut p, &host, "return eqoa.actions.craft_tray_add({slot = iron.slot, id = iron.id})");
    assert!(ok, "{c} {r}");
    assert_eq!(snapshot_json()["tray"].as_array().unwrap().len(), 1);
    assert_eq!(snapshot_json()["items"][0]["in_tray"], 1);
    assert!(sent().is_empty(), "the tray is the client's own list until a combine: nothing is sent");
    // a wrong id (the bags changed under the mod) is refused
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_tray_add({slot = iron.slot, id = iron.id + 1})");
    assert!(!ok && c == "stale_handle", "{c}");
    // not a component
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_tray_add({slot = 1, id = 900001})");
    assert!(!ok && c == "invalid_argument", "{c}");
    // the stack holds two logs: a third is the game's own "already in the combine list"
    host.execute("assert(eqoa.actions.craft_tray_add({slot = pine.slot, id = pine.id}))").unwrap();
    step(&mut p, &host);
    host.execute("assert(eqoa.actions.craft_tray_add({slot = pine.slot, id = pine.id}))").unwrap();
    step(&mut p, &host);
    let (ok, c, r) = act(&mut p, &host, "return eqoa.actions.craft_tray_add({slot = pine.slot, id = pine.id})");
    assert!(!ok && c == "not_ready" && r.contains("already in the combine list"), "{c} {r}");
    // silk makes four; a fifth is the game's "combine list is full" (iron has more in the stack, so it is the limit)
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_tray_add({slot = silk.slot, id = silk.id})");
    assert!(ok);
    let (ok, c, r) = act(&mut p, &host, "return eqoa.actions.craft_tray_add({slot = iron.slot, id = iron.id})");
    assert!(!ok && c == "not_ready" && r.contains("is full"), "{c} {r}");
    assert_eq!(p.craft.combine.len(), 4, "iron, pine, pine, silk");
    // taking one out
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_tray_remove({slot = iron.slot, id = iron.id})");
    assert!(ok);
    assert_eq!(p.craft.combine.len(), 3);
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_tray_remove({slot = iron.slot, id = iron.id})");
    assert!(!ok && c == "not_ready", "it is not in the tray: {c}");
    // a fill replaces the tray; a fill with a pick the bags cannot give changes nothing (the control)
    let before = p.craft.combine.clone();
    assert_eq!(before.len(), 3);
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_fill({{slot = iron.slot, id = iron.id}, {slot = silk.slot, id = silk.id + 5}})");
    assert!(!ok && c == "stale_handle", "{c}");
    assert_eq!(p.craft.combine.len(), before.len(), "a refused fill leaves the tray as it was");
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_fill({{slot = iron.slot, id = iron.id}, {slot = iron.slot, id = iron.id}, {slot = silk.slot, id = silk.id}})");
    assert!(ok);
    assert_eq!(p.craft.combine.iter().map(|i| i.item.id).collect::<Vec<_>>(), vec![900_002, 900_002, 900_004]);
    // more than four picks never reaches the game
    host.execute("big = {iron, iron, silk, silk, pine}; r, e = eqoa.actions.craft_fill({{slot=1,id=1},{slot=1,id=1},{slot=1,id=1},{slot=1,id=1},{slot=1,id=1}})").unwrap();
    host.execute("assert(r == nil and e.code == 'out_of_range')").unwrap();
    // clear
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_tray_clear()");
    assert!(ok && p.craft.combine.is_empty());
    assert!(sent().is_empty(), "none of this sent anything");
}

#[test]
fn a8_one_combine_goes_once_and_is_never_replayed() {
    let (mut p, host) = rig();
    open(&mut p, &host, "experiment");
    host.execute("c = assert(eqoa.craft.get()); iron = c.items[1]").unwrap();
    // an empty tray: nothing to combine
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_combine()");
    assert!(!ok && c == "not_ready", "{c}");
    assert!(sent().is_empty());
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_fill({{slot = iron.slot, id = iron.id}, {slot = iron.slot, id = iron.id}})");
    assert!(ok);
    let (ok, c, r) = act(&mut p, &host, "return eqoa.actions.craft_combine()");
    assert!(ok, "{c} {r}");
    // sub-op 4: the token, a zigzag count of 2, and the ids 900002 twice (zigzag), as the game's Combine builds them
    let mut expect = vec![0x01, 0x11, 4, 4, 3, 2, 1];
    let mut w = Writer::default();
    w.signed(2).signed(900_002).signed(900_002);
    expect.extend(&w.data);
    assert_eq!(sent(), vec![expect], "exactly the bytes the game's own Combine sends");
    assert_eq!(p.craft.pending, 4);
    assert!(p.craft.combine.is_empty(), "the tray is spent");
    assert_eq!(snapshot_json()["pending"], "combine");
    // no answer comes: many steps later nothing has been sent again, and a second combine is refused (the tray is
    // empty, and a request is out). This is the "uncertain combine is never replayed" rule.
    for _ in 0..50 {
        step(&mut p, &host);
    }
    assert_eq!(sent().len(), 1, "never replayed");
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_combine()");
    assert!(!ok && c == "not_ready");
    p.craft.combine = vec![item(900_002, "Iron Ore", 12, 1, true)];
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_combine()");
    assert!(!ok && c == "busy", "a request is still out: {c}");
    assert_eq!(sent().len(), 1);
    // the server answers (nothing made): the game may combine again, once per call
    let mut none = Writer::default();
    none.u8(12).u8(2).u32(9);
    assert!(reply(&mut p, &none.data));
    assert_eq!(p.craft.pending, 0);
    assert!(wide_notice().contains("do not make anything"));
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_combine()");
    assert!(ok);
    assert_eq!(sent().len(), 2);
}

fn wide_notice() -> String {
    snapshot_json()["notice"]["text"].as_str().unwrap_or_default().to_string()
}

#[test]
fn a8_the_keep_recipe_question_shows_and_only_an_explicit_answer_sends_anything() {
    let (mut p, host) = rig();
    open(&mut p, &host, "experiment");
    host.execute("c = assert(eqoa.craft.get()); iron = c.items[1]").unwrap();
    assert!(act(&mut p, &host, "return eqoa.actions.craft_fill({{slot = iron.slot, id = iron.id}})").0);
    assert!(act(&mut p, &host, "return eqoa.actions.craft_combine()").0);
    let combine_bytes = sent();
    assert_eq!(combine_bytes.len(), 1);
    // the server found a recipe: the question is the mod's, not the game's window
    assert!(reply(&mut p, &discovery_reply(0x1111_2222, 4242)));
    let q = &snapshot_json()["question"];
    assert!(q["text"].as_str().unwrap().contains("Keep this recipe?"), "{q}");
    assert_eq!(q["recipe"], 4242);
    host.execute("local c = eqoa.craft.get(); assert(c.question and c.question.recipe == 4242 and c.pending == 'none')").unwrap();
    assert!(p.craft_outs.is_empty() || p.craft_outs.iter().all(|o| !matches!(o, CraftOut::Ask(..))), "the game's window gets no question");
    // nothing was answered by merely asking
    assert_eq!(sent().len(), 1);
    // so are a list and a detail (the game's box holds its whole window)
    let before = sent().len();
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_list(3)");
    assert!(!ok && c == "busy", "{c}");
    assert_eq!(sent().len(), before);
    // a combine is held until the player answers, as the game's box holds its window
    p.craft.combine = vec![item(900_002, "Iron Ore", 12, 1, true)];
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_combine()");
    assert!(!ok && c == "busy", "{c}");
    p.craft.combine.clear();
    // Yes: the game's own Send5, sub-op 5 with the token and the recipe, which does not wait
    let (ok, c, r) = act(&mut p, &host, "return eqoa.actions.craft_keep(true)");
    assert!(ok, "{c} {r}");
    assert_eq!(sent()[1], vec![0x01, 0x11, 5, 0x22, 0x22, 0x11, 0x11, 0x92, 0x10, 0, 0]);
    assert!(snapshot_json()["question"].is_null());
    assert_eq!(p.craft.pending, 0, "scribing does not wait");
    // answered once: a second Yes has nothing to answer
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_keep(true)");
    assert!(!ok && c == "not_ready", "{c}");
    assert_eq!(sent().len(), 2);
    // No: nothing is sent, the question goes
    assert!(reply(&mut p, &discovery_reply(0x1111_2222, 4243)));
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_keep(false)");
    assert!(ok);
    assert_eq!(sent().len(), 2, "No sends nothing");
    assert!(snapshot_json()["question"].is_null());
}

#[test]
fn a8_a_closed_window_drops_the_tray_and_the_snapshot_and_a_reopen_copies_the_bags_again() {
    let (mut p, host) = rig();
    open(&mut p, &host, "experiment");
    host.execute("c = assert(eqoa.craft.get()); iron = c.items[1]").unwrap();
    assert!(act(&mut p, &host, "return eqoa.actions.craft_fill({{slot = iron.slot, id = iron.id}})").0);
    assert_eq!(p.craft.combine.len(), 1);
    host.execute("assert(eqoa.windows.close_session(s_experiment.ref))").unwrap();
    step(&mut p, &host);
    assert!(p.craft.combine.is_empty(), "the tray goes with the window");
    assert!(native_a8::current().is_none());
    // the bags change meanwhile; a reopen sees them, and clears a request the game never answered
    p.dump.inventory.items.push(item(900_005, "Tin", 15, 4, true));
    p.craft.pending = 4;
    open(&mut p, &host, "experiment");
    assert_eq!(p.craft.pending, 0, "as the game's Experiment opening does");
    host.execute("c = assert(eqoa.craft.get()); assert(#c.items == 4, #c.items)").unwrap();
    assert!(sent().is_empty());
}

#[test]
fn a8_the_snapshot_reads_skills_difficulty_and_the_game_words_for_components() {
    let (mut p, host) = rig();
    open(&mut p, &host, "recipe_book");
    assert!(reply(&mut p, &recipe_list(1, &[(11, "Iron Ring", 3, 0)])));
    let s = snapshot_json();
    assert_eq!(s["skills"][3], serde_json::json!({"id": 3, "name": "Jewel Crafting", "level": 7, "rank": 2}));
    assert_eq!(s["skills"][0]["name"], "Armorsmithing");
    assert_eq!(s["skills"][1]["name"], "Weaponsmithing");
    assert_eq!(s["recipes"][0]["difficulty_name"], "C");
    assert_eq!(component_word(3, 0), Some(1482));
    assert_eq!(component_word(2, 18), Some(1481));
    assert_eq!(component_word(2, 19), None);
    assert_eq!(component_word(9, 0), None);
    let revision = s["revision"].as_u64().unwrap();
    // nothing changed: a step leaves the revision alone; a change bumps it
    step(&mut p, &host);
    assert_eq!(snapshot_json()["revision"].as_u64().unwrap(), revision);
    p.craft.slots[3] = net::dump::CraftSlot([0, 8, 2, 0]);
    step(&mut p, &host);
    assert!(snapshot_json()["revision"].as_u64().unwrap() > revision);
}

#[test]
fn a8_the_component_tables_match_the_games_own() {
    // the arrays copied from `craftwnd.rs` are the same numbers (a text check: they are private there)
    let src = include_str!("../craftwnd.rs");
    for (name, table) in [
        ("COMPONENTS01", COMPONENTS01.to_vec()),
        ("COMPONENTS2", COMPONENTS2.to_vec()),
        ("COMPONENTS3", COMPONENTS3.to_vec()),
        ("COMPONENTS4", COMPONENTS4.to_vec()),
        ("COMPONENTS5", COMPONENTS5.to_vec()),
    ] {
        let line = src.lines().find(|l| l.starts_with(&format!("const {name}:"))).unwrap_or_else(|| panic!("{name} in craftwnd.rs"));
        let nums: Vec<i32> = line.split('=').nth(1).unwrap().split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
        assert_eq!(nums, table, "{name}");
    }
    let skill = src.find("fn skill_name").expect("skill_name");
    let body = &src[skill..skill + 300];
    for (skill, word) in SKILL_WORDS.iter().enumerate() {
        assert!(body.contains(&format!("{skill} => {word}")) || body.contains(&format!("{skill} => {word},")), "skill {skill} -> {word}");
    }
}

#[test]
fn a8_dead_or_out_of_the_world_nothing_is_sent() {
    let (mut p, host) = rig();
    open(&mut p, &host, "recipe_book");
    p.death = Some(crate::death::DeathCam::new([0.; 3], [0., 0., 1.], [0.; 3]));
    // the death ends the window (A1) and the action is refused either way
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_list(3)");
    assert!(!ok && (c == "not_ready"), "{c}");
    assert!(sent().is_empty());
    // the world is let go: nothing is left to read
    super::dropped();
    assert!(native_a8::current().is_none());
}

#[test]
fn a8_a_mod_window_opening_or_closing_leaves_the_games_own_crafting_state_alone() {
    let (mut p, host) = rig();
    // the game's own Experiment is up with a combine out and a filled tray
    p.mode = 37;
    p.craft.pending = 4;
    p.craft.combine = vec![item(900_002, "Iron Ore", 12, 1, true)];
    p.craft.items = p.dump.inventory.clone();
    host.execute("s = assert(eqoa.windows.open_session('experiment'))").unwrap();
    step(&mut p, &host);
    assert_eq!((p.craft.pending, p.craft.combine.len()), (4, 1), "opening a mod window under the game's own does not clear its request or its tray");
    assert!(native_a8::current().is_none(), "and nothing is read for the mod yet");
    // the game's window closes: now the mod's window opens, as the game's own Use would
    p.mode = 0;
    step(&mut p, &host);
    assert_eq!((p.craft.pending, p.craft.combine.len()), (0, 0), "control: with no game window up the opening does clear them");
    assert!(native_a8::current().is_some());
    // closing under the game's window: the tray the game's window holds stays
    p.mode = 37;
    p.craft.pending = 4;
    p.craft.combine = vec![item(900_002, "Iron Ore", 12, 1, true)];
    host.execute("assert(eqoa.windows.close_session(s.ref))").unwrap();
    step(&mut p, &host);
    assert_eq!((p.craft.pending, p.craft.combine.len()), (4, 1));
    p.mode = 0;
    step(&mut p, &host);
    assert!(p.craft.combine.is_empty(), "control: with the game's window gone the close empties the tray");
}

#[test]
fn a8_only_the_rows_the_games_experiment_lists_are_offered() {
    let (mut p, host) = rig();
    let mut tool = item(900_010, "Tool Kit", 20, 1, true);
    tool.item.p54 = 2;
    tool.item.p4c = 8;
    let mut odd = item(900_011, "Odd Thing", 21, 1, true);
    odd.item.p54 = -1;
    p.dump.inventory.items.push(tool);
    p.dump.inventory.items.push(odd);
    open(&mut p, &host, "experiment");
    let names: Vec<String> = snapshot_json()["items"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, vec!["Iron Ore", "Pine Log", "Silk"], "no tool (type 8), no negative p54; the control rows are there");
    for (slot, id) in [(5, 900_010), (6, 900_011)] {
        let (ok, c, _) = act(&mut p, &host, &format!("return eqoa.actions.craft_tray_add({{slot = {slot}, id = {id}}})"));
        assert!(!ok && c == "invalid_argument", "{c}");
        let (ok, c, _) = act(&mut p, &host, &format!("return eqoa.actions.craft_fill({{{{slot = {slot}, id = {id}}}}})"));
        assert!(!ok && c == "invalid_argument", "{c}");
    }
    assert!(p.craft.combine.is_empty());
    // the control: a listed row goes in
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_tray_add({slot = 2, id = 900002})");
    assert!(ok);
}

#[test]
fn a8_the_tray_follows_the_real_bags_while_the_window_is_open() {
    let (mut p, host) = rig();
    open(&mut p, &host, "experiment");
    host.execute("c = assert(eqoa.craft.get()); iron, silk = c.items[1], c.items[3]").unwrap();
    assert!(act(&mut p, &host, "return eqoa.actions.craft_fill({{slot = iron.slot, id = iron.id}, {slot = silk.slot, id = silk.id}})").0);
    assert_eq!(p.craft.combine.len(), 2);
    // the character sells the Silk and loots some Tin with the window open: no reopening
    p.dump.inventory.items.retain(|i| i.item.name != "Silk");
    p.dump.inventory.items.push(item(900_005, "Tin", 15, 4, true));
    step(&mut p, &host);
    let names: Vec<String> = snapshot_json()["items"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, vec!["Iron Ore", "Pine Log", "Tin"]);
    assert_eq!(p.craft.combine.iter().map(|i| i.item.id).collect::<Vec<_>>(), vec![900_002], "the sold Silk left the tray");
    // adding what is gone is stale, what was just looted works
    let (ok, c, _) = act(&mut p, &host, "return eqoa.actions.craft_tray_add({slot = silk.slot, id = silk.id})");
    assert!(!ok && c == "stale_handle", "{c}");
    let (ok, ..) = act(&mut p, &host, "return eqoa.actions.craft_tray_add({slot = 4, id = 900005})");
    assert!(ok);
    // the control: with a request out the copy is left as the game's replies updated it
    p.craft.pending = 4;
    p.dump.inventory.items.retain(|i| i.item.name != "Tin");
    step(&mut p, &host);
    assert!(snapshot_json()["items"].as_array().unwrap().iter().any(|i| i["name"] == "Tin"));
    p.craft.pending = 0;
    step(&mut p, &host);
    assert!(!snapshot_json()["items"].as_array().unwrap().iter().any(|i| i["name"] == "Tin"));
    assert_eq!(p.craft.combine.len(), 1);
}

#[test]
fn a8_the_one_line_in_play_rs_offers_the_reply_to_the_mod_before_the_games_own_arm() {
    // the arm is not reachable without a whole game loop, so the wiring is checked in the source: the fork's guard
    // arm comes first, upstream's arm is unchanged after it, and nothing else mentions the hook. Without the guard
    // line the mod's session would miss every reply (the first test above shows what the game's arm alone does).
    let src = include_str!("../play.rs").replace("\r\n", "\n");
    let hook = "Event::Opcode { op: 0x1102, data } if fork_hooks::craft_reply(self, &data) => {}";
    let games = "Event::Opcode { op: 0x1102, data } => {\n                    let modes = crate::craft::Modes { recipe: self.mode == 6, experiment: self.mode == 37 };";
    let (h, g) = (src.find(hook).expect("the hook arm is in play.rs"), src.find(games).expect("the game's own arm is unchanged"));
    assert!(h < g, "the hook arm comes before the game's own");
    assert_eq!(src.matches("fork_hooks::craft_reply").count(), 1, "one line");
}

// ---- the bundled mod, driven by clicks against the real game state ----

mod e2e {
    use super::*;
    use crate::fork_ext::addons::{
        bridge::Bridge,
        discovery::{DiscoveredAddon, EntryMode},
    };
    use dear_imgui_rs::Context;
    use serde_json::json;
    use std::path::PathBuf;

    const ID: &str = "eqoa_crafting";
    // the window's size and the rows it lays out, in HUD pixels (main.lua)
    const W: f64 = 360.0;
    const H: f64 = 350.0;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(ID).canonicalize().unwrap()
    }

    fn addon() -> DiscoveredAddon {
        let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root().join("mod.json")).unwrap()).unwrap();
        DiscoveredAddon {
            id: ID.into(),
            name: ID.into(),
            entry: root().join("main.lua"),
            script_root: root(),
            package_root: root(),
            module_dirs: vec![root()],
            resource_dirs: vec![root()],
            api: Some("eqoa/1".into()),
            entry_mode: EntryMode::Once,
            capabilities_declared: true,
            requested_capabilities: m["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect(),
            extensions: Default::default(),
            initially_enabled: false,
            mods_root: None,
        }
    }

    fn context() -> Context {
        let mut context = Context::create();
        context.set_ini_filename::<PathBuf>(None).unwrap();
        context.io_mut().set_display_size([1024.0, 768.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context.font_atlas().try_claim_legacy_renderer().unwrap().build();
        context
    }

    struct Rig {
        p: Play,
        host: Host,
        bridge: Bridge,
        context: Context,
        k: f64,
        x0: f64,
        y0: f64,
    }

    impl Rig {
        fn frames(&mut self, n: usize) {
            for _ in 0..n {
                // the client's frame counter, which the pointer claims' per-frame quota counts by
                CORE.with(|c| c.borrow_mut().frame += 1);
                self.context.frame();
                self.bridge
                    .invoke(|| {
                        native_events::dispatch(&self.host)?;
                        native_events::render(&self.host)
                    })
                    .unwrap();
                drop(self.context.render_legacy());
            }
        }

        /// The game steps (the broker carries out what the mod asked), the mod hears, the window is drawn.
        fn settle(&mut self) {
            step(&mut self.p, &self.host);
            self.frames(2);
        }

        /// What the mod and the broker say (for a failed assertion's message).
        fn why(&self) -> String {
            let results: String = self
                .host
                .lua
                .load("local out = {}; for _, r in ipairs(results or {}) do out[#out + 1] = tostring(r.kind) .. ':' .. tostring(r.ok) .. ':' .. tostring(r.code) .. ':' .. tostring(r.reason) end return table.concat(out, ' | ')")
                .eval()
                .unwrap_or_else(|e| format!("(no results: {e})"));
            let frame: String = self.host.lua.load("local f = eqoa.hud.frame(); return string.format('%g %g %g %g', f.scale_x, f.scale_y, f.offset_x, f.offset_y)").eval().unwrap_or_default();
            format!("snapshot {}; hud frame [{frame}] vs geometry k {} x0 {} y0 {}; queued {:?}; results [{results}]; logs {:?}; mode {}; pending {}", native_a8::current().is_some(), self.k, self.x0, self.y0, action_broker::queued_kinds_for_test(), self.host.drain_logs(), self.p.mode, self.p.craft.pending)
        }

        /// The window's place as main.lua puts it now (the HUD frame can move between steps).
        fn relayout(&mut self) {
            let (k, sx, sy, ox, oy): (f64, f64, f64, f64, f64) = self
                .host
                .lua
                .load("local f = eqoa.hud.frame(); return math.min(f.scale_x, f.scale_y), f.scale_x, f.scale_y, f.offset_x, f.offset_y")
                .eval()
                .unwrap();
            self.k = k;
            self.x0 = (1024.0 - W * k).min(ox + (640.0 - W) / 2.0 * sx).max(0.0);
            self.y0 = (768.0 - H * k).min(oy + (448.0 - H) / 2.0 * sy).max(0.0);
        }

        fn tap(&mut self, x: f64, y: f64) {
            self.relayout();
            let (px, py) = (self.x0 + x * self.k, self.y0 + y * self.k);
            CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": px, "y": py, "inside_window": true, "button": "left"})));
            native_events::dispatch(&self.host).unwrap();
            self.settle();
        }
    }

    fn rig_with_mod() -> Rig {
        let p = rig_play();
        // the ImGui context first: the bridge binds to the one that exists when it is installed
        let context = context();
        let host = Host::for_addon(&addon()).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        host.run_addon(&addon()).unwrap();
        host.execute("results = {}; eqoa.events.on('action.result', function(e) results[#results + 1] = e.data end)").unwrap();
        let mut r = Rig { p, host, bridge, context, k: 1.0, x0: 0.0, y0: 0.0 };
        CORE.with(|c| {
            c.borrow_mut().emit("input.key", json!({"key": "r", "combo": "shift+r", "down": true, "repeat": false, "synthetic": false, "text": ""}))
        });
        native_events::dispatch(&r.host).unwrap();
        r.settle();
        let (k, sx, sy, ox, oy): (f64, f64, f64, f64, f64) = r
            .host
            .lua
            .load("local f = eqoa.hud.frame(); return math.min(f.scale_x, f.scale_y), f.scale_x, f.scale_y, f.offset_x, f.offset_y")
            .eval()
            .unwrap();
        r.k = k;
        r.x0 = (1024.0 - W * k).min(ox + (640.0 - W) / 2.0 * sx).max(0.0);
        r.y0 = (768.0 - H * k).min(oy + (448.0 - H) / 2.0 * sy).max(0.0);
        r
    }

    #[test]
    fn a8_a_recipe_picked_fills_the_tray_and_each_click_on_combine_does_one_combine() {
        let mut r = rig_with_mod();
        assert!(mod_window_open(), "Shift+R opened a recipe_book session");
        // the skill (the fourth of six): the game is asked for its list, as its own recipe book asks
        let sw = (W - 16.0) / 6.0;
        r.tap(8.0 + 3.0 * sw + 10.0, 50.0);
        assert_eq!(sent(), vec![vec![0x01, 0x11, 6, 4, 3, 2, 1, 3, 0, 0, 0]], "list of skill 3: {}", r.why());
        // the server's list comes in: the window shows it, a recipe row is a click away
        assert!(reply(&mut r.p, &recipe_list(0x0A0B_0C0D, &[(11, "Iron Ring", 3, 0), (12, "Silk Hood", 3, 1)])));
        r.frames(2);
        r.tap(60.0, 94.0);
        assert_eq!(sent().len(), 2);
        assert_eq!(&sent()[1][..3], &[0x01, 0x11, 7], "the detail of the recipe, sub-op 7");
        assert!(reply(&mut r.p, &detail_reply(0x0A0B_0C0E)));
        r.frames(2);
        // Fill tray: the ingredients from the bags (two Iron Ore and a Silk), nothing sent yet
        r.tap(30.0, 304.0);
        assert_eq!(r.p.craft.combine.iter().map(|i| i.item.id).collect::<Vec<_>>(), vec![900_002, 900_002, 900_004]);
        assert_eq!(sent().len(), 2, "filling sends nothing");
        // Combine: one request
        r.tap(100.0, 304.0);
        assert_eq!(sent().len(), 3, "one combine");
        let mut expect = vec![0x01, 0x11, 4, 0x0E, 0x0C, 0x0B, 0x0A];
        let mut w = Writer::default();
        w.signed(3).signed(900_002).signed(900_002).signed(900_004);
        expect.extend(&w.data);
        assert_eq!(sent()[2], expect);
        // the button is not there while the server has not answered: more clicks, more steps, no more requests
        for _ in 0..5 {
            r.tap(100.0, 304.0);
        }
        assert_eq!(sent().len(), 3, "never repeated by itself, and a click on a waiting window does nothing");
        // the server asks about the recipe it found: the mod's window shows it; clicks elsewhere do not answer it
        assert!(reply(&mut r.p, &discovery_reply(0x1111_2222, 4242)));
        r.frames(2);
        assert!(snapshot_json()["question"]["text"].as_str().is_some());
        r.tap(100.0, 304.0);
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), 3, "the question is answered only by its own buttons");
        // Keep recipe: the game's own scribe request, once
        r.tap(40.0, 190.0);
        assert_eq!(sent().len(), 4);
        assert_eq!(sent()[3], vec![0x01, 0x11, 5, 0x22, 0x22, 0x11, 0x11, 0x92, 0x10, 0, 0]);
        r.tap(40.0, 190.0);
        assert_eq!(sent().len(), 4, "answered once");
        let logs = r.host.drain_logs().unwrap();
        assert!(logs.is_empty(), "no diagnostics: {logs:?}");
    }

    #[test]
    fn a8_a_fill_that_the_bags_cannot_give_says_what_is_missing_and_sends_nothing() {
        let mut r = rig_with_mod();
        let sw = (W - 16.0) / 6.0;
        r.tap(8.0 + 3.0 * sw + 10.0, 50.0);
        assert!(reply(&mut r.p, &recipe_list(1, &[(11, "Iron Ring", 3, 0)])));
        r.frames(2);
        r.tap(60.0, 94.0);
        // the bags hold five Iron Ore; make the recipe want two Silk and take the Silk away (the control: with it, the
        // tray fills in the test above)
        r.p.dump.inventory.items.retain(|i| i.item.name != "Silk");
        assert!(reply(&mut r.p, &detail_reply(2)));
        // no hand copy of the tray's list: the client follows the bags itself (the next step)
        r.settle();
        let before = sent().len();
        r.tap(30.0, 304.0);
        assert!(r.p.craft.combine.is_empty());
        assert_eq!(sent().len(), before);
    }
}
