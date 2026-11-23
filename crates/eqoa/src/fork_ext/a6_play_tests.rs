// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A6 from Play-shaped state: the Start key's menu and the chat line held by a mod, what the mod's actions
//! send compared with the game's own controls on the same state, and the cases that must stay the game's.
//!
//! Each test says what its negative control is: the behaviour without the change would fail the assertion named.
use super::*;
use crate::fork_ext::addons::{
    action_broker,
    host::Host,
    native_core::{self, CORE, Core},
    native_events, native_a6, window_sessions as ws,
    window_takeover::{self, TakeoverKind, TakeoverScope},
};
use net::stream::{Reader, Writer};

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    native_a6::reset_for_test();
    window_takeover::reset_all_for_test();
    PASS_ONCE.with(|p| p.set(false));
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p.session.conn = Some(usize::MAX);
    p.session.self_id = Some(0x100);
    p.session.record = Some(Vec::new());
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    p
}

/// The start menu as a server's tables make it: a sub-menu with one entry, Items, and Quit.
fn with_menus(p: &mut Play) {
    use crate::client_image::{Label, Menu, MenuEntry};
    p.bags_start_built();
    let s = p.start.as_mut().unwrap();
    let sub = s.menus.push(Menu {
        x00: -1,
        x04: -1,
        entries: vec![MenuEntry { msg: 0x0B0A_0000, id: 0, ..Default::default() }],
    });
    let root = s.menu.main.menu;
    s.menus.list[root].entries = vec![
        MenuEntry { msg: 0, id: 5, sub: Some(sub), ..Default::default() },
        MenuEntry { msg: 0x0200_0000, id: 0, ..Default::default() },
        MenuEntry { msg: 0x0A01_0000, id: 0, ..Default::default() },
    ];
    for ((msg, id), text) in [((0, 5), "Character"), ((0x0B0A_0000, 0), "Quest Journal"), ((0x0200_0000, 0), "Items"), ((0x0A01_0000, 0), "Quit")] {
        s.menus.labels.insert((msg, id), Label { text: crate::ui::wide(text), ..Default::default() });
    }
}

fn host() -> Host {
    Host::with_test_capabilities(&[
        "eqoa.actions.*",
        "eqoa.windows.session",
        "eqoa.windows.focus",
        "eqoa.windows.takeover",
        "eqoa.events.on",
    ])
    .unwrap()
}

fn hold(host: &Host, kind: TakeoverKind) -> u64 {
    let vm = native_events::bus(&host.lua).borrow().vm;
    window_takeover::takeover(vm, "a6_mod", kind, TakeoverScope::All).unwrap();
    vm
}

fn step(p: &mut Play, host: &Host) {
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn sent(p: &mut Play) -> Vec<Vec<u8>> {
    std::mem::take(p.session.record.as_mut().unwrap()).into_iter().map(|(_, m)| m).collect()
}

fn item(name: &str, key: u32) -> net::dump::NetItem {
    let mut it = net::dump::NetItem { inv_index: key, ..Default::default() };
    it.item.name = name.to_string();
    it
}

#[test]
fn a6_the_start_key_opens_the_mods_menu_and_the_games_never_shows() {
    let mut p = ready();
    with_menus(&mut p);
    // control: nobody holds the Start menu, so the game's own opens (the behaviour this round must not change)
    p.open_start_menu(true);
    super::super::begin_iteration(&mut p, true);
    assert!(p.mode == 13 && p.start_open.is_some(), "the game's menu waits for its draw when no mod holds it");
    p.close_start_menu(0);
    p.race_frame_due.clear();

    let h = host();
    let vm = hold(&h, TakeoverKind::StartMenu);
    p.open_start_menu(true);
    assert!(p.mode == 13 && p.start_open.is_some(), "the key opens the game's menu first: the diversion is the next step");
    step(&mut p, &h);
    assert_eq!(p.mode, 0, "undone: play again");
    assert!(p.start_open.is_none() && !p.start.as_ref().unwrap().menu.visible, "the game's menu is never drawn");
    assert!(p.race_frame_due.is_empty(), "the character was never framed, so no camera is put back");
    let s = ws::get(vm, "start_menu").expect("the mod has a session");
    assert_eq!((s.origin, s.opened_from.as_str()), ("player", "key"));
    let data = s.data.unwrap();
    let entries = data["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["text"], "Character");
    assert_eq!(entries[0]["entries"][0]["text"], "Quest Journal");
    assert_eq!(entries[0]["entries"][0]["command"], "0xb0a0000:0");
    assert!(entries[0].get("command").is_none(), "a sub-menu has no command");
    assert_eq!(entries[1]["native"], true, "Items is the game's own page");
    assert_eq!(entries[2]["command"], "0xa010000:0");
    // the mod reads it as the session's data
    h.execute("s = eqoa.windows.session('start_menu'); assert(s.data.entries[1].entries[1].text == 'Quest Journal'); assert(s.opened_from == 'key' and s.origin == 'player')").unwrap();
    // a window returning to the Start menu on close opens the mod's, not the game's
    p.close_start_menu(0);
    p.race_frame_due.clear();
    let again = ws::get(vm, "start_menu").unwrap().id;
    p.open_start_menu(false);
    step(&mut p, &h);
    assert_eq!(p.mode, 0);
    assert!(ws::get(vm, "start_menu").is_none(), "the second open is the toggle: the mod's menu closed (the game's Start key closes the game's)");
    assert_ne!(again, "");
}

#[test]
fn a6_the_pads_start_button_reaches_the_mods_menu_too() {
    // the pad's Start is the game's key 144, like the keyboard's: `set_key` is the one entry for both. Needs the disc
    // (a walk on Freeport's ground); a run without it says so
    let (x, z) = (25275.115f32, 15766.627f32);
    let Some(world) = crate::walker::tests::tunaria_round(x, z) else {
        eprintln!("SKIP a6_the_pads_start_button_reaches_the_mods_menu_too: no disc");
        return;
    };
    let mut walker = crate::walker::Walker::new(world, x, z, 0.0);
    let mut p = ready();
    with_menus(&mut p);
    let h = host();
    let vm = hold(&h, TakeoverKind::StartMenu);
    p.set_key(144, true, &mut walker, 4.0 / 3.0);
    p.set_key(144, false, &mut walker, 4.0 / 3.0);
    step(&mut p, &h);
    assert!(ws::get(vm, "start_menu").is_some(), "the pad's Start opened the mod's menu");
    assert_eq!(p.mode, 0);
    // control: with no holder the same press opens the game's menu
    window_takeover::reset_all_for_test();
    ws::reset_for_test();
    p.set_key(144, true, &mut walker, 4.0 / 3.0);
    super::super::begin_iteration(&mut p, true);
    assert_eq!(p.mode, 13);
}

#[test]
fn a6_a_menu_command_does_what_the_games_entry_does() {
    let (mut mine, mut native) = (ready(), ready());
    for p in [&mut mine, &mut native] {
        with_menus(p);
    }
    let h = host();
    let vm = hold(&h, TakeoverKind::StartMenu);
    mine.open_start_menu(true);
    step(&mut mine, &h);
    let before = mine.race_frame_due.clone();
    h.execute("s = eqoa.windows.session('start_menu'); req, err = eqoa.actions.menu_command(s.ref, '0xa010000:0'); assert(req ~= nil, err and err.message)").unwrap();
    step(&mut mine, &h);
    // the game's own press of Quit: its menu's entry goes to `start_actions` and the Yes/No box comes up
    native.open_start_menu(true);
    native.start_actions(vec![(0x0A01_0000, 0)]);
    assert_eq!(mine.mode, native.mode, "the same window: {}", mine.mode);
    assert_eq!(mine.mode, 14, "Quit asks Yes or No");
    assert_eq!(mine.mbox.kind, native.mbox.kind);
    assert_eq!(mine.race_frame_due.len(), before.len() + 0, "no stray camera restore: the mod's menu never framed the character");
    assert!(ws::get(vm, "start_menu").is_none(), "the menu ends when the game's entry runs");
    // a command that is not in the menu is refused; so is one of the report routes
    let mut other = ready();
    with_menus(&mut other);
    let h2 = host();
    hold(&h2, TakeoverKind::StartMenu);
    other.open_start_menu(true);
    step(&mut other, &h2);
    h2.execute(
        r#"s = eqoa.windows.session('start_menu')
        local r, e = eqoa.actions.menu_command(s.ref, '0xdead:0'); assert(r ~= nil, 'queued, refused at dispatch')
        local r2, e2 = eqoa.actions.menu_command(s.ref, '0xb0b1000:0'); assert(r2 == nil and e2.code == 'permission_denied', e2 and e2.code)
        local r3, e3 = eqoa.actions.menu_command('ws-999', '0xa010000:0'); assert(r3 == nil and e3.code == 'stale_handle', e3 and e3.code)"#,
    )
    .unwrap();
    step(&mut other, &h2);
    assert_eq!(other.mode, 0, "nothing ran");
    assert!(ws::get(native_events::bus(&h2.lua).borrow().vm, "start_menu").is_some(), "the menu is still up");
}

#[test]
fn a6_items_hands_the_start_key_back_to_the_games_menu() {
    let mut p = ready();
    with_menus(&mut p);
    let h = host();
    let vm = hold(&h, TakeoverKind::StartMenu);
    p.open_start_menu(true);
    step(&mut p, &h);
    h.execute("s = eqoa.windows.session('start_menu'); assert(eqoa.actions.menu_command(s.ref, '0x2000000:0'))").unwrap();
    step(&mut p, &h);
    assert_eq!(p.mode, 13, "the game's own Start menu is up for its item page");
    assert!(ws::get(vm, "start_menu").is_none(), "the mod's session ended (passed)");
    step(&mut p, &h);
    assert!(p.mode == 13 && p.start_open.is_some(), "and the next step leaves it the game's");
    // once: the key after that is the mod's again
    p.close_start_menu(0);
    p.race_frame_due.clear();
    p.open_start_menu(true);
    step(&mut p, &h);
    assert_eq!(p.mode, 0);
    assert!(ws::get(vm, "start_menu").is_some());
}

#[test]
fn a6_the_menu_stays_the_games_when_the_player_uses_the_games_windows_or_is_dead() {
    let mut p = ready();
    with_menus(&mut p);
    let h = host();
    hold(&h, TakeoverKind::StartMenu);
    ws::set_env(ws::Env { dead: true, ..ws::Env::default() });
    p.open_start_menu(true);
    step(&mut p, &h);
    assert_eq!(p.mode, 13, "dead: the game's");
    p.close_start_menu(0);
    ws::set_env(ws::Env::default());
    window_takeover::set_use_game_windows(true);
    p.open_start_menu(true);
    super::super::begin_iteration(&mut p, true);
    window_takeover::set_use_game_windows(false);
    assert_eq!(p.mode, 13, "the player's switch: the game's");
    // control: neither, and it is the mod's
    p.close_start_menu(0);
    p.race_frame_due.clear();
    hold(&h, TakeoverKind::StartMenu);
    p.open_start_menu(true);
    step(&mut p, &h);
    assert_eq!(p.mode, 0);
}

#[test]
fn a6_a_menu_already_up_when_the_mod_claims_the_kind_stays_the_games() {
    let mut p = ready();
    with_menus(&mut p);
    p.open_start_menu(true);
    // the game draws it (its `run_start_menu` takes the open)
    p.start_open = None;
    p.start.as_mut().unwrap().menu.visible = true;
    let h = host();
    hold(&h, TakeoverKind::StartMenu);
    step(&mut p, &h);
    assert_eq!(p.mode, 13, "a window already open stays native until it closes");
    assert!(p.start.as_ref().unwrap().menu.visible);
}

#[test]
fn a6_losing_the_claim_closes_the_mods_menu() {
    let mut p = ready();
    with_menus(&mut p);
    let h = host();
    let vm = hold(&h, TakeoverKind::StartMenu);
    p.open_start_menu(true);
    step(&mut p, &h);
    assert!(ws::get(vm, "start_menu").is_some());
    h.execute("log = {}; eqoa.events.on('windows.session', function(e) log[#log + 1] = e.data.phase .. ':' .. e.data.reason end)").unwrap();
    assert!(window_takeover::release(vm, TakeoverKind::StartMenu));
    step(&mut p, &h);
    native_core::flush_deferred();
    native_events::dispatch(&h).unwrap();
    assert!(ws::get(vm, "start_menu").is_none());
    h.execute("assert(log[1] == 'closed:released', tostring(log[1]))").unwrap();
}

/// The drift test the design asked for: the entries the mod is given are the ones the game's own opening lists,
/// run into nothing (`run_start_menu`), for an expansion server with the TEST entry on.
#[test]
fn a6_the_entry_list_is_the_one_the_games_own_opening_makes() {
    let mut p = ready();
    with_menus(&mut p);
    p.session.expansion = 2;
    p.admin.part.enabled = 1;
    // the mod's list
    let data = start_menu_data(&mut p).unwrap();
    let root_mine = p.start.as_ref().unwrap().menu.main.menu;
    // the game's: the opening of `run_start_menu`, drawn into nothing
    p.start.as_mut().unwrap().menu.main.menu = usize::MAX - 1;
    p.start_open = Some(true);
    let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
    let loc = crate::startmenu::Location { world: 0, pos: [0.0; 3], cols: 1, rows: 1, cell: 1.0, origin: [0.0; 3], bounds: None };
    let mut r = super::super::NoopRaster;
    let _ = p.run_start_menu(&vui, &mut r, &loc);
    let root_game = p.start.as_ref().unwrap().menu.main.menu;
    assert_eq!(root_mine, root_game, "the same table: the expansion's with TEST");
    assert!(p.start.as_ref().unwrap().menu.visible, "the game's opening showed its menu");
    let theirs = entries_of(&p, root_game, &mut vec![root_game]);
    assert_eq!(data["entries"], json!(theirs));
    // the negative control: without TEST the table is another
    p.admin.part.enabled = 0;
    p.start.as_mut().unwrap().menu.visible = false;
    let plain = start_menu_data(&mut p).unwrap();
    assert_ne!(p.start.as_ref().unwrap().menu.main.menu, root_game, "TEST off picks the table without it");
    let _ = plain;
}

// ---- the chat line ----

#[test]
fn a6_the_enter_key_opens_the_mods_chat_line_and_only_for_chat() {
    let mut p = ready();
    // control: nobody holds it, so the game's own line opens
    p.chat_key("", true);
    super::super::begin_iteration(&mut p, true);
    assert!(p.mode == 2 && p.edit.purpose == CHAT_PURPOSE, "the game's line when no mod holds it");
    p.mode = 0;
    p.edit.clear();
    let h = host();
    let vm = hold(&h, TakeoverKind::ChatEntry);
    p.channel = 4;
    p.chat_key("", true);
    step(&mut p, &h);
    assert_eq!(p.mode, 0, "the game's box was undone");
    assert!(p.edit.text.is_empty() && p.edit.purpose == 0);
    let s = ws::get(vm, "chat_entry").expect("the mod has a session");
    let d = s.data.unwrap();
    assert_eq!((d["text"].as_str(), d["channel"].as_str()), (Some(""), Some("guild")));
    // R and T open the line with the command typed
    p.reply_name = "Dab".into();
    ws::reset_for_test();
    p.chat_key("/reply ", false);
    step(&mut p, &h);
    let d = ws::get(vm, "chat_entry").unwrap().data.unwrap();
    assert_eq!((d["text"].as_str(), d["reply_to"].as_str()), (Some("/reply "), Some("Dab")));
    // a line kept with an item link comes with it
    ws::reset_for_test();
    let name: Vec<u16> = "Rusty Sword".encode_utf16().collect();
    p.link_draft = Some((name.clone(), vec![crate::editbox::TextLink { start: 0, len: name.len(), list: 0, key: 11 }]));
    p.open_chat_line();
    step(&mut p, &h);
    let d = ws::get(vm, "chat_entry").unwrap().data.unwrap();
    assert_eq!(d["links"][0]["name"], "Rusty Sword");
    assert_eq!((d["links"][0]["start"].as_u64(), d["links"][0]["list"].as_str(), d["links"][0]["key"].as_u64()), (Some(1), Some("inventory"), Some(11)));
    assert!(p.link_draft.is_none(), "the mod's line owns it now");
    // every other text box is the game's: a buddy's name, an ignore's, the report's, an alarm's, a who list's tell
    for purpose in [0x0B04_0000u32, 0x0B05_3001, 0x0B0B_1002, 0x0B0E_000A, 0x0B05_1009] {
        ws::reset_for_test();
        p.mode = 0;
        p.open_edit(purpose, 24, -1, false);
        step(&mut p, &h);
        assert_eq!(p.mode, 2, "purpose {purpose:#x} stays the game's box");
        assert_eq!(p.edit.purpose, purpose);
        assert!(ws::get(vm, "chat_entry").is_none());
        p.mode = 0;
        p.edit.clear();
    }
    p.who_tell = Some("Dab".into());
    p.open_chat_line();
    step(&mut p, &h);
    assert_eq!(p.mode, 2, "a Who list's Tell goes back to the list: the game's");
}

#[test]
fn a6_a_chat_line_goes_as_the_games_own_line_does() {
    let (mut mine, mut native) = (ready(), ready());
    let h = host();
    for p in [&mut mine, &mut native] {
        p.reply_name = "Dab".into();
        p.session.item_links_offer = true;
        p.agree_item_links();
        p.dump.inventory.items = vec![item("Rusty Sword", 11)];
        sent(p);
    }
    let ask = |p: &mut Play, code: &str| {
        h.execute(&format!("req, err = {code}; assert(req ~= nil, err and err.message)")).unwrap();
        step(p, &h);
    };
    // each channel against the game's line
    for (route, line) in [("say", "/say hello"), ("group", "/g hello"), ("guild", "/gu hello"), ("shout", "/s hello"), ("reply", "/r hello")] {
        ask(&mut mine, &format!("eqoa.actions.chat_line('{route}', 'hello')"));
        native.chat_line(line);
        let (a, b) = (sent(&mut mine), sent(&mut native));
        assert!(!a.is_empty(), "{route} sent something");
        assert_eq!(a, b, "{route}: the bytes are the game's own line's");
    }
    ask(&mut mine, "eqoa.actions.chat_line('tell', 'hello', {to = 'Wil'})");
    native.chat_line("/t Wil hello");
    assert_eq!(sent(&mut mine), sent(&mut native), "a tell");
    // a line with an item link: the link's place counts in the text that goes
    ask(
        &mut mine,
        "eqoa.actions.chat_line('shout', 'look: Rusty Sword', {links = {{start = 7, len = 11, list = 'inventory', key = 11}}})",
    );
    let at = "/s look: ".encode_utf16().count();
    native.chat_line_linked("/s look: Rusty Sword", &[crate::editbox::TextLink { start: at, len: 11, list: 0, key: 11 }]);
    let (a, b) = (sent(&mut mine), sent(&mut native));
    assert_eq!(a, b, "the linked line is the game's own linked line");
    // the link is on the wire: the control that shows the comparison is not of two plain lines
    let mut r = Reader::new(&a[0][2..]);
    let (_route, _target, _text, n) = (r.u8(), r.u32(), r.wide(1024), r.u8());
    assert_eq!(n, 1, "one link went with the line");
    // a link to an item the player does not have is refused
    h.execute("req, err = eqoa.actions.chat_line('say', 'x Gone', {links = {{start = 3, len = 4, list = 'inventory', key = 99}}}); assert(req ~= nil)").unwrap();
    step(&mut mine, &h);
    assert!(sent(&mut mine).is_empty(), "nothing is sent for an item that is not there");
    // Reply with nobody to reply to sends nothing
    mine.reply_name.clear();
    ask(&mut mine, "eqoa.actions.chat_line('reply', 'hi')");
    assert!(sent(&mut mine).is_empty());
    let _ = Writer::default();
}

#[test]
fn a6_set_channel_is_the_games_chat_mode() {
    let (mut mine, mut native) = (ready(), ready());
    let h = host();
    h.execute("req, err = eqoa.actions.set_channel('shout'); assert(req ~= nil, err and err.message)").unwrap();
    step(&mut mine, &h);
    native.group_changed(3);
    assert_eq!(mine.channel, 3);
    assert_eq!(sent(&mut mine), sent(&mut native), "0x000E with the channel, as the game's Chat Modes sends it");
    // the group's channel needs a group of two; the guild's needs a guild (as the game's own channel step)
    h.execute("req, err = eqoa.actions.set_channel('group'); assert(req ~= nil)").unwrap();
    step(&mut mine, &h);
    assert_eq!(mine.channel, 3, "not in a group: unchanged");
    assert!(sent(&mut mine).is_empty());
    h.execute("req, err = eqoa.actions.set_channel('tell'); assert(req == nil and err.code == 'invalid_argument')").unwrap();
}

#[test]
fn a6_link_item_goes_to_the_mods_line_when_it_holds_the_chat_and_to_the_games_otherwise() {
    let mut p = ready();
    p.session.item_links_offer = true;
    p.agree_item_links();
    p.dump.inventory.items = vec![item("Rusty Sword", 11)];
    // control: no holder, the game's own Link Item opens its line with the link in it
    assert!(offer_link(&mut p, 0).is_none());
    assert!(p.link_item(0, 0));
    assert_eq!(p.mode, 2);
    assert_eq!(p.edit.links.len(), 1);
    p.mode = 0;
    p.edit.clear();
    let h = host();
    let vm = hold(&h, TakeoverKind::ChatEntry);
    let out = offer_link(&mut p, 0).expect("held: the mod's");
    assert!(matches!(out, DispatchOutcome::Completed { .. }));
    assert_eq!(p.mode, 0, "the game's line stayed shut");
    let d = ws::get(vm, "chat_entry").unwrap().data.unwrap();
    assert_eq!((d["offer"]["name"].as_str(), d["offer"]["key"].as_u64(), d["offer"]["seq"].as_u64()), (Some("Rusty Sword"), Some(11), Some(1)));
    // a second offer into the open line is a new seq on the same session
    let again = offer_link(&mut p, 0);
    assert!(again.is_some());
    assert_eq!(ws::get(vm, "chat_entry").unwrap().data.unwrap()["offer"]["seq"], 2);
    // a server with no item links refuses
    p.session.item_links_offer = false;
    p.agree_item_links();
    assert!(matches!(offer_link(&mut p, 0), Some(DispatchOutcome::Refused { code: "not_ready", .. })));
}

#[test]
fn a6_a_link_in_a_chat_line_opens_the_games_item_card() {
    let mut p = ready();
    p.session.item_links_offer = true;
    p.agree_item_links();
    let h = host();
    // the line's links come with the event before it
    native_a6::stash_links(&[(10, 11, 4242)]);
    native_core::observe_text("Dab: look Rusty Sword", [1.0; 4]);
    native_core::flush_deferred();
    h.execute(
        r#"local m = eqoa.chat.history(1)[1]
        assert(m.kind == 'say', m.kind)
        assert(m.links[1].id == 4242 and m.links[1].name == 'Rusty Sword' and m.links[1].start == 11 and m.links[1].len == 11, 'links')
        req, err = eqoa.actions.open_link(m.links[1].id, m.links[1].name); assert(req ~= nil, err and err.message)
        local r2, e2 = eqoa.actions.open_link(777, 'Nothing'); assert(r2 == nil and e2.code == 'not_found', e2 and e2.code)"#,
    )
    .unwrap();
    assert!(!p.links.is_open());
    step(&mut p, &h);
    assert!(p.links.is_open(), "the game's own card for the link is open");
    assert!(!sent(&mut p).is_empty(), "and the server was asked for the item");
}
