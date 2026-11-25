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

/// A Play on a session that records what it sends (the connection of `ready()` is gone: nothing is kept), so the
/// bytes of a mod's action can be compared with the game's own control on the same state.
fn recording() -> Play {
    struct Nowhere;
    impl net::session::Datagrams for Nowhere {
        fn send_to(&mut self, _: &[u8], _: [u8; 4], _: u16) {}
        fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
            None
        }
    }
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    native_a6::reset_for_test();
    window_takeover::reset_all_for_test();
    let mut s = net::session::Session::connect_with(Box::new(Nowhere), ([127, 0, 0, 1], 5000), ([127, 0, 0, 1], 10070), "test", [0; 32]).unwrap();
    s.self_id = Some(0x100);
    s.record = Some(Vec::new());
    s.item_links_offer = true;
    let mut p = Play::new(s, net::session::Player::default(), Box::<net::dump::MemoryDump>::default());
    let _ = p.loading.wait.take();
    p.agree_item_links();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
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
    let data = s.data;
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
    p.death = Some(crate::death::DeathCam::new([0.; 3], [0., 0., 1.], [0.; 3]));
    p.open_start_menu(true);
    step(&mut p, &h);
    assert_eq!(p.mode, 13, "dead: the game's");
    p.close_start_menu(0);
    p.death = None;
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
    // (a step with the line shut: the line open before the claim must not count as opened since)
    super::super::begin_iteration(&mut p, true);
    let vm = hold(&h, TakeoverKind::ChatEntry);
    p.channel = 4;
    p.chat_key("", true);
    step(&mut p, &h);
    assert_eq!(p.mode, 0, "the game's box was undone");
    assert!(p.edit.text.is_empty() && p.edit.purpose == 0);
    let s = ws::get(vm, "chat_entry").expect("the mod has a session");
    let d = s.data;
    assert_eq!((d["text"].as_str(), d["channel"].as_str()), (Some(""), Some("guild")));
    // R and T open the line with the command typed
    p.reply_name = "Dab".into();
    ws::reset_for_test();
    p.chat_key("/reply ", false);
    step(&mut p, &h);
    let d = ws::get(vm, "chat_entry").unwrap().data;
    assert_eq!((d["text"].as_str(), d["reply_to"].as_str()), (Some("/reply "), Some("Dab")));
    // a line kept with an item link comes with it
    ws::reset_for_test();
    let name: Vec<u16> = "Rusty Sword".encode_utf16().collect();
    p.link_draft = Some((name.clone(), vec![crate::editbox::TextLink { start: 0, len: name.len(), list: 0, key: 11 }]));
    p.open_chat_line();
    step(&mut p, &h);
    let d = ws::get(vm, "chat_entry").unwrap().data;
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
    let (mut mine, mut native) = (recording(), recording());
    let h = host();
    for p in [&mut mine, &mut native] {
        p.reply_name = "Dab".into();
        p.dump.inventory.items = vec![item("Rusty Sword", 11)];
        sent(p);
    }
    let ask = |p: &mut Play, code: &str| {
        h.execute(&format!("req, err = {code}; assert(req ~= nil, err and err.message)")).unwrap();
        step(p, &h);
    };
    // each channel against the game's line: the channel plain text already goes to sends the text alone, as the
    // game's Enter does (one message); another goes as the game's command for it
    for (route, line) in [("group", "/g hello"), ("guild", "/gu hello"), ("shout", "/s hello"), ("reply", "/r hello")] {
        ask(&mut mine, &format!("eqoa.actions.chat_line('{route}', 'hello')"));
        native.chat_line(line);
        let (a, b) = (sent(&mut mine), sent(&mut native));
        assert!(!a.is_empty(), "{route} sent something");
        assert_eq!(a, b, "{route}: the bytes are the game's own line's");
    }
    // on the current channel (say here): the text alone, one message, as the Enter key's line sends it; the control is
    // that the command form (`/say`) is three messages
    ask(&mut mine, "eqoa.actions.chat_line('say', 'hello')");
    native.chat_line("hello");
    let (a, b) = (sent(&mut mine), sent(&mut native));
    assert_eq!(a, b, "say on the say channel is the game's plain line");
    assert_eq!(a.len(), 1, "one message");
    native.chat_line("/say hello");
    assert_eq!(sent(&mut native).len(), 3, "the control: the command form is the channel switch around the line");
    for (route, code) in [("group", 2u8), ("shout", 3), ("guild", 4)] {
        mine.channel = code;
        native.channel = code;
        ask(&mut mine, &format!("eqoa.actions.chat_line('{route}', 'hello')"));
        native.chat_line("hello");
        let (a, b) = (sent(&mut mine), sent(&mut native));
        assert_eq!(a, b, "{route} on its own channel is the game's plain line");
        assert_eq!(a.len(), 1);
    }
    mine.channel = 0;
    native.channel = 0;
    // text is taken as passed: a leading space does not move a link
    ask(&mut mine, "eqoa.actions.chat_line('say', '  look Rusty Sword', {links = {{start = 8, len = 11, list = 'inventory', key = 11}}})");
    native.chat_line_linked("  look Rusty Sword", &[crate::editbox::TextLink { start: 7, len: 11, list: 0, key: 11 }]);
    assert_eq!(sent(&mut mine), sent(&mut native), "the link is where the text says");
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
    // the channel switch (0x000E) goes first and last; the linked line is the longest message
    let line = a.iter().max_by_key(|m| m.len()).unwrap();
    let mut r = Reader::new(&line[2..]);
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
    let (mut mine, mut native) = (recording(), recording());
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
    let d = ws::get(vm, "chat_entry").unwrap().data;
    assert_eq!((d["offer"]["name"].as_str(), d["offer"]["key"].as_u64(), d["offer"]["seq"].as_u64()), (Some("Rusty Sword"), Some(11), Some(1)));
    // a second offer into the open line is a new seq on the same session
    let again = offer_link(&mut p, 0);
    assert!(again.is_some());
    assert_eq!(ws::get(vm, "chat_entry").unwrap().data["offer"]["seq"], 2);
    // a server with no item links refuses
    p.session.item_links_offer = false;
    p.agree_item_links();
    assert!(matches!(offer_link(&mut p, 0), Some(DispatchOutcome::Refused { code: "not_ready", .. })));
}

#[test]
fn a6_a_link_in_a_chat_line_opens_the_games_item_card() {
    let (mut p, mut native) = (recording(), recording());
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
    // the game's own click on the link: the same card, the same question to the server
    native.open_link(4242, "Rusty Sword");
    let (a, b) = (sent(&mut p), sent(&mut native));
    assert!(!a.is_empty(), "the server was asked for the item");
    assert_eq!(a, b, "as a click on the link asks it");
}

// ---- fix round ----

#[test]
fn a6_a_pad_press_opens_the_games_line_with_its_soft_keyboard_and_never_a_mod_line() {
    let mut p = ready();
    let h = host();
    let vm = hold(&h, TakeoverKind::ChatEntry);
    // the pad's R3 (key 147): its button went down this frame (`prepare_controls` notes it before the game reads it)
    native_a6::note_pad(crate::input::bit::R3);
    p.open_chat_line();
    step(&mut p, &h);
    assert!(p.mode == 2 && p.edit.purpose == CHAT_PURPOSE, "the game's own line (the pad can type and cancel in it)");
    assert!(ws::get(vm, "chat_entry").is_none(), "a mod's line would have taken the whole pad away while it types");
    // the same for the Start menu's chat entry reached with the pad
    p.mode = 0;
    p.edit.clear();
    step(&mut p, &h);
    native_a6::note_pad(0);
    // the control: no pad button, a keyboard's Enter, a few steps later: the mod's line
    for _ in 0..5 {
        step(&mut p, &h);
    }
    p.open_chat_line();
    step(&mut p, &h);
    assert_eq!(p.mode, 0);
    assert!(ws::get(vm, "chat_entry").is_some(), "the keyboard's Enter opens the mod's line");
}

#[test]
fn a6_a_chat_line_already_open_when_the_mod_claims_the_kind_stays_the_games() {
    let mut p = ready();
    let h = host();
    p.open_chat_line();
    step(&mut p, &h);
    assert_eq!(p.mode, 2, "no holder: the game's line");
    let vm = hold(&h, TakeoverKind::ChatEntry);
    step(&mut p, &h);
    step(&mut p, &h);
    assert!(p.mode == 2 && p.edit.purpose == CHAT_PURPOSE, "typing goes on in the game's box");
    assert!(ws::get(vm, "chat_entry").is_none());
    // closed, and opened again: the mod's
    p.mode = 0;
    p.edit.clear();
    step(&mut p, &h);
    p.open_chat_line();
    step(&mut p, &h);
    assert_eq!(p.mode, 0);
    assert!(ws::get(vm, "chat_entry").is_some());
}

#[test]
fn a6_a_holder_whose_handler_fails_gives_the_keys_back_to_the_game_at_once() {
    for kind in [TakeoverKind::StartMenu, TakeoverKind::ChatEntry] {
        let mut p = ready();
        with_menus(&mut p);
        let h = host();
        let vm = hold(&h, kind);
        h.execute("eqoa.events.on('windows.session', function(e) if e.data.phase == 'opened' then error('boom') end end)").unwrap();
        let open = |p: &mut Play| match kind {
            TakeoverKind::StartMenu => p.open_start_menu(true),
            _ => p.open_chat_line(),
        };
        open(&mut p);
        step(&mut p, &h);
        native_core::flush_deferred();
        native_events::dispatch(&h).unwrap();
        assert!(!window_takeover::is_taken(kind), "{kind:?}: the failed mod no longer holds it");
        assert!(ws::get(vm, kind.name()).is_none(), "{kind:?}: and has no window");
        // the next press is the game's
        p.mode = 0;
        p.edit.clear();
        step(&mut p, &h);
        open(&mut p);
        step(&mut p, &h);
        let want = if kind == TakeoverKind::StartMenu { 13 } else { 2 };
        assert_eq!(p.mode, want, "{kind:?}: the game's own window opens");
        // the negative control is the same press with the handler healthy: it stays the mod's (the tests above)
    }
}

/// Owner decision 2026-11-23: `eqoa.chat.get_channel` named group and shout the wrong way round (the game's `/g` is
/// channel 2, `/s` channel 3). It reports the channel the line really goes to, as the new session data and
/// `set_channel` always did. Negative control: the old table answered "shout" for 2 and "group" for 3.
#[test]
fn a6_get_channel_names_the_channel_the_line_goes_to() {
    let mut p = ready();
    let host = Host::with_test_capabilities(&["eqoa.chat.get_channel"]).unwrap();
    for (code, name) in [(0u8, "say"), (2, "group"), (3, "shout"), (4, "guild"), (7, "other")] {
        p.channel = code;
        host.snapshot.set(p.forge_snapshot(None));
        host.execute(&format!("local c = eqoa.chat.get_channel(); assert(c == '{name}', 'channel {code}: ' .. tostring(c))")).unwrap();
    }
    // the path that follows the player each frame (`api_updates`) uses the same table, not a copy of the old one
    for file in ["api_capture.rs", "api_updates.rs"] {
        let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fork_ext").join(file)).unwrap();
        assert!(!src.contains("2 => \"shout\""), "{file} still names channel 2 shout");
    }
    // the line's own channels: what `set_channel` sets is what is read back
    for (name, code) in [("group", 2u8), ("shout", 3)] {
        assert_eq!(native_a6::channel_name(code), Some(name));
    }
}

/// Owner test 2026-11-23 (lane A, item 4): "Hitting Enter makes me stop running." A mod's text capture cleared the
/// character's autorun, follow and attack on every step; the game's own chat box does not. While the mod's chat line is
/// open the run goes on. Negative control: any other text capture (a mod's own text box, no chat line) still stops them.
#[test]
fn a6_a_mods_chat_line_keeps_the_character_running() {
    let mut p = ready();
    let h = host();
    let vm = hold(&h, TakeoverKind::ChatEntry);
    p.autorun = true;
    p.following = true;
    p.follow_id = 7;
    // control: a text capture with no chat line stops them, as it always did
    crate::fork_ext::addons::input::reset_input_state();
    crate::fork_ext::addons::input::testing_text_capture(true);
    step(&mut p, &h);
    assert!(!p.autorun && !p.following, "another text box takes the stops");
    // the chat line is open: the run keeps going
    p.autorun = true;
    p.following = true;
    ws::open_client(vm, "a6_mod", "chat_entry", "key", json!({"text": "", "channel": "say", "links": []}));
    step(&mut p, &h);
    assert!(p.autorun && p.following, "the chat line keeps the run, as the game's box does");
    // closing the line: the capture alone stops them again
    let id = ws::get(vm, "chat_entry").unwrap().id;
    let _ = ws::close(vm, &id, "closed");
    step(&mut p, &h);
    assert!(!p.autorun, "no chat line, the capture stops the run");
    crate::fork_ext::addons::input::testing_text_capture(false);
}

// ---- The real Start Menu mod on a real Play with the disc's own menu tables (owner test 2026-11-24: choosing
// Ability left the player stuck: the same Enter also opened the game's chat line, the Chat mod took it, and its
// box held the keyboard under the game's Ability page). Keys go the way the viewer sends them.

fn iso_path() -> std::path::PathBuf {
    std::env::var_os("EQOA_ISO").map(Into::into).unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../run/eqoa.iso"))
}

/// The disc's CLIENT image and word table (None without the disc: the test says SKIP and counts as one).
fn disc_menu() -> Option<(crate::client_image::ClientImage, vi::strtab::StringTable)> {
    let iso = iso_path();
    let image = crate::modules::load_now(crate::modules::Module::Client, &iso)?;
    let img = crate::client_image::ClientImage::from_bytes((*image).clone())?;
    let words = iso::Iso::open(&iso).ok().and_then(|mut d| d.read("WORDS.TXT").ok()).and_then(|b| crate::spelling::words(&b))?;
    Some((img, words))
}

fn bundled_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").canonicalize().unwrap()
}

type Edit<'a> = Option<&'a dyn Fn(String) -> String>;

/// The bundled mod `id`, its `main.lua` passed through `edit` first (the negative controls run the old Lua): an
/// edited mod is copied to a scratch folder.
fn bundled_edited(id: &str, edit: Edit) -> crate::fork_ext::addons::discovery::DiscoveredAddon {
    let mut a = crate::fork_ext::addons::discovery::discover(&bundled_root()).addons.into_iter().find(|a| a.id == id).unwrap_or_else(|| panic!("{id} is bundled"));
    if let Some(edit) = edit {
        let to = std::env::temp_dir().join(format!("eqoa-a6-{id}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&to);
        std::fs::create_dir_all(&to).unwrap();
        let to = to.canonicalize().unwrap();
        let from = a.entry.parent().unwrap().to_path_buf();
        for f in std::fs::read_dir(&from).unwrap().flatten().filter(|f| f.path().is_file()) {
            std::fs::copy(f.path(), to.join(f.file_name())).unwrap();
        }
        let main = to.join("main.lua");
        let text = std::fs::read_to_string(&main).unwrap();
        let edited = edit(text.clone());
        assert_ne!(text, edited, "the edit changed the Lua");
        std::fs::write(&main, edited).unwrap();
        a.entry = main;
        a.script_root = to.clone();
        a.package_root = to.clone();
        a.module_dirs = vec![to.clone()];
        a.resource_dirs = vec![to];
    }
    a
}

struct Running {
    host: Host,
    _bridge: crate::fork_ext::addons::bridge::Bridge,
    vm: u64,
}

fn start_mod(a: &crate::fork_ext::addons::discovery::DiscoveredAddon) -> Running {
    let host = Host::for_addon(a).unwrap();
    let bridge = crate::fork_ext::addons::bridge::Bridge::install(&host).unwrap();
    host.run_addon(a).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    Running { host, _bridge: bridge, vm }
}

/// A Play with the disc's start menu tables, the Start Menu mod, the Chat mod (which holds the chat line, as it does
/// for the owner) and a hotkey window's mod running: the Spellbook (Shift+K) when `with_book`, else the Quest Journal
/// (Shift+J), so the game's own Ability page is what the Ability entry opens.
struct Rig {
    p: Play,
    menu: Running,
    chat: Running,
    hot: Running,
    /// the hotkey's key, its combo, its code and the kind of window it opens
    hotkey: (&'static str, &'static str, u8, &'static str),
}

impl Rig {
    fn new(img: &crate::client_image::ClientImage, words: &vi::strtab::StringTable, edit: Edit, with_book: bool) -> Rig {
        let mut p = ready();
        crate::fork_ext::bindpage::reset_in_memory();
        window_takeover::forget_switch_for_test();
        p.words = Some(words.clone());
        p.client_data(img);
        p.session.expansion = 2;
        let menu = start_mod(&bundled_edited("eqoa_start_menu", edit));
        let chat = start_mod(&bundled_edited("eqoa_chat", None));
        let (id, hotkey) = if with_book { ("eqoa_spellbook", ("k", "shift+k", b'K', "spellbook")) } else { ("eqoa_quest_journal", ("j", "shift+j", b'J', "quest_log")) };
        let hot = start_mod(&bundled_edited(id, None));
        assert!(window_takeover::is_taken(TakeoverKind::StartMenu), "the Start Menu mod holds the Start key");
        assert!(window_takeover::is_taken(TakeoverKind::ChatEntry), "the Chat mod holds the chat line");
        Rig { p, menu, chat, hot, hotkey }
    }

    /// One game step, and what each mod is told after it.
    fn step(&mut self) {
        super::super::begin_iteration(&mut self.p, true);
        native_core::flush_deferred();
        for m in [&self.menu, &self.chat, &self.hot] {
            native_events::dispatch(&m.host).unwrap();
        }
    }

    /// A key down as the viewer routes it (`runtime::route_key`): a mod hears it only in plain play, and the game gets
    /// it unless a mod's claim (or an open text box) holds it there. Returns whether the game got it.
    fn key(&mut self, name: &str, combo: &str, code: u8) -> bool {
        let plain = self.p.addon_input_available();
        // the claim is read as the key arrives; the mods hear the key a step later (`native_core::defer`), so a claim a
        // mod lets go of in answer to this very key still held it
        let to_game = !crate::fork_ext::addons::input::wants_key_capture(false, Some(code), false, plain);
        if plain {
            native_core::CORE.with(|c| {
                c.borrow_mut().emit("input.key", json!({"key": name, "combo": combo, "down": true, "repeat": false, "synthetic": false, "text": ""}))
            });
            for m in [&self.menu, &self.chat, &self.hot] {
                native_events::dispatch(&m.host).unwrap();
            }
        }
        to_game
    }

    /// The game's own Escape: in play it opens the Start menu; in its spell book it backs out of the book (`Out::Close`,
    /// back to the Start menu); in its Start menu or any other window it closes.
    fn escape(&mut self) {
        if !self.key("escape", "escape", 0x1b) {
            return;
        }
        match self.p.mode {
            0 => self.p.open_start_menu(true),
            5 => {
                let bars = self.p.ability_bars();
                let out = self.p.book.key(&self.p.abilities, bars, 27, true, 0);
                self.p.book_actions(out);
            }
            13 => self.p.start_actions(vec![(0, 0)]),
            // another window: it closes the way each does, back to the Start menu
            _ => {
                self.p.mode = 0;
                self.p.open_start_menu(false);
            }
        }
    }

    /// The game's own Enter: in play it opens the chat line (`Play::chat_key`), the key the Start menu must not leak.
    fn enter(&mut self) {
        if self.key("return", "return", 0x0d) && self.p.mode == 0 {
            self.p.chat_key("", true);
        }
    }

    fn arrow(&mut self, name: &str) {
        let _ = self.key(name, name, 0);
    }

    fn menu_session(&self) -> Option<ws::Session> {
        ws::get(self.menu.vm, "start_menu")
    }

    fn chat_line(&self) -> Option<ws::Session> {
        ws::get(self.chat.vm, "chat_entry")
    }

    /// Down to the entry at `path` (indexes from 0 through the sub-menus), and Enter on it.
    fn choose(&mut self, path: &[usize]) {
        for (depth, &index) in path.iter().enumerate() {
            for _ in 0..index {
                self.arrow("down");
            }
            if depth + 1 < path.len() {
                self.enter();
            }
        }
        self.enter();
    }
}

/// The leaf entries (commands) of the menu data: each one's path of indexes, label and command.
fn leaves(entries: &Value, at: &mut Vec<usize>, out: &mut Vec<(Vec<usize>, String, String)>) {
    for (i, e) in entries.as_array().unwrap().iter().enumerate() {
        at.push(i);
        if let Some(sub) = e.get("entries") {
            leaves(sub, at, out);
        } else if let Some(c) = e["command"].as_str() {
            out.push((at.clone(), e["text"].as_str().unwrap().to_string(), c.to_string()));
        }
        at.pop();
    }
}

/// After an entry ran and its window closed: nothing of the menu or the chat holds a key or the keyboard, Escape opens
/// the Start menu again and closes it, and another window opens on its hotkey.
fn the_player_is_free(r: &mut Rig, what: &str) {
    // a window a route opened is closed first (its hotkey would close it)
    if let Some(open) = ws::get(r.hot.vm, r.hotkey.3) {
        ws::close(r.hot.vm, &open.id, "test").unwrap();
        r.step();
    }
    assert!(r.menu_session().is_none(), "{what}: the menu is closed");
    assert!(r.chat_line().is_none(), "{what}: no chat line of the Chat mod is open");
    assert!(!crate::fork_ext::addons::input::is_key_claimed(0x1b), "{what}: Escape is the game's again");
    assert!(!crate::fork_ext::addons::input::is_key_claimed(0x0d), "{what}: Enter is the game's again");
    r.escape();
    r.step();
    assert!(r.menu_session().is_some(), "{what}: Escape opens the Start menu again (mode {})", r.p.mode);
    r.escape();
    assert!(r.menu_session().is_none(), "{what}: Escape closes the menu");
    r.step();
    let (key, combo, code, kind) = r.hotkey;
    assert!(r.key(key, combo, code));
    assert!(ws::get(r.hot.vm, kind).is_some(), "{what}: {combo} opens the {kind} window (mode {})", r.p.mode);
    let id = ws::get(r.hot.vm, kind).unwrap().id;
    ws::close(r.hot.vm, &id, "test").unwrap();
    r.step();
}

/// Owner test 2026-11-24 through the real key path: Escape for the menu, down to Ability, Enter. The negative control
/// runs the same keys on the old Lua (no claim on Enter).
fn ability_walk(edit: Edit, with_book: bool) -> Option<Rig> {
    let Some((img, words)) = disc_menu() else {
        eprintln!("SKIP a6 ability walk: no disc");
        return None;
    };
    let mut r = Rig::new(&img, &words, edit, with_book);
    r.escape();
    r.step();
    assert!(r.menu_session().is_some(), "Escape opened the mod's Start menu");
    r.choose(&[3]);
    r.step();
    r.step();
    Some(r)
}

#[test]
fn a6_ability_opens_the_games_page_and_the_player_gets_every_key_back() {
    let Some(mut r) = ability_walk(None, false) else { return };
    assert_eq!(r.p.mode, 5, "the game's Ability page (the spell book) is up");
    assert!(r.chat_line().is_none(), "the Enter that chose Ability did not also open the chat line");
    // Escape in the page goes back to the Start menu, which is the mod's
    r.escape();
    r.step();
    assert!(r.menu_session().is_some(), "the page's Escape returns to the mod's Start menu (mode {})", r.p.mode);
    assert_eq!(r.p.mode, 0);
    // Escape closes the menu
    r.escape();
    r.step();
    the_player_is_free(&mut r, "after Ability");
}

/// With EQOA Spellbook on, Ability opens its window and the game's page stays shut; the Enter that chose it opens no
/// chat line; everything works after. (The negative control is the old Lua, which had no Ability route, in the run
/// recorded in the report; and the test above, with the Spellbook off, is the game's page.)
#[test]
fn a6_ability_opens_the_spellbook_mod_when_it_is_on() {
    let Some(mut r) = ability_walk(None, true) else { return };
    assert_eq!(r.p.mode, 0, "the game's Ability page did not open");
    assert!(r.menu_session().is_none(), "the menu closed");
    assert!(ws::get(r.hot.vm, "spellbook").is_some(), "EQOA Spellbook opened");
    assert!(r.chat_line().is_none(), "no chat line");
    the_player_is_free(&mut r, "after Ability into the Spellbook");
}

/// The negative control: the Start Menu Lua without the claim on Enter. The same keys leave the Chat mod holding a chat
/// line (its text box takes every key) under the game's page.
#[test]
fn a6_ability_without_the_enter_claim_leaves_a_chat_line_open_the_old_bug() {
    let strip = |s: String| s.replace("    state.enter = eqoa.input.claim(\"Enter\", true)\n", "");
    let Some(r) = ability_walk(Some(&strip), false) else { return };
    assert!(r.chat_line().is_some(), "the old Lua: Enter also opened the chat line and the Chat mod holds it");
    assert_eq!(r.p.mode, 5, "under the game's Ability page");
}

/// Every entry of the disc's menu, through its real keys: after it the game's page (if any) closes and the player can
/// open the menu again and a hotkey window. The mods an entry may route to are off here, so each runs the game's own
/// command (the routes are in `menu_routes_tests`).
#[test]
fn a6_every_entry_of_the_discs_menu_leaves_the_player_able_to_go_on() {
    let Some((img, words)) = disc_menu() else {
        eprintln!("SKIP a6 walk of every entry: no disc");
        return;
    };
    let data = {
        let mut p = ready();
        p.words = Some(words.clone());
        p.client_data(&img);
        p.session.expansion = 2;
        start_menu_data(&mut p).unwrap()
    };
    let mut all = Vec::new();
    leaves(&data["entries"], &mut Vec::new(), &mut all);
    assert!(all.len() > 40, "the disc's menu has its entries: {}", all.len());
    let mut walked = Vec::new();
    for (path, label, command) in all {
        let what = format!("{label} ({command}) at {path:?}");
        let mut r = Rig::new(&img, &words, None, true);
        r.escape();
        r.step();
        assert!(r.menu_session().is_some(), "{what}: the menu opens");
        r.choose(&path);
        r.step();
        r.step();
        // the entries that are the chat line (Message > Compose, Tell, Reply) open it, as the game's own do: the Chat
        // mod holds it and the player closes it; no other entry may bring one up with its Enter
        let is_chat_entry = ["0x3010000:", "0xb0d0000:", "0xb0d0001:"].iter().any(|c| command.starts_with(c));
        if is_chat_entry {
            if let Some(line) = r.chat_line() {
                ws::close(r.chat.vm, &line.id, "test").unwrap();
                r.step();
            }
        }
        assert!(r.chat_line().is_none(), "{what}: the Enter did not also open the chat line");
        assert!(r.menu_session().is_none(), "{what}: the menu ended when the entry ran");
        // with EQOA Spellbook on, Ability is its window and not the game's page
        if command.starts_with("0x8010000:") {
            assert!(ws::get(r.hot.vm, "spellbook").is_some() && r.p.mode == 0, "{what}: EQOA Spellbook opened, not the game's page (mode {})", r.p.mode);
        }
        // Support (the game's report routes) is the player's to press in the game's own menu: the hand-off opens it
        if command.starts_with("0xb0b1") {
            assert_eq!(r.p.mode, 13, "{what}: the game's own Start menu opened for the Support forms");
        }
        // the game's window (if any) closes the way the game's own do
        match r.p.mode {
            0 => {}
            13 => r.p.start_actions(vec![(0, 0)]),
            _ => {
                r.p.mode = 0;
                r.p.open_start_menu(false);
                r.step();
                assert!(r.menu_session().is_some(), "{what}: the window's return opens the mod's menu (mode {})", r.p.mode);
                r.escape();
                r.step();
            }
        }
        assert_eq!(r.p.mode, 0, "{what}: play again");
        the_player_is_free(&mut r, &what);
        walked.push(label);
        window_takeover::reset_all_for_test();
        ws::reset_for_test();
    }
    eprintln!("walked {} entries: {walked:?}", walked.len());
}

impl Rig {
    /// A pad button pressed and let go, as the viewer hands the report to the fork (`apply_pad`): the button must be
    /// taken out of what the game sees. Returns the report the game would have seen at the press.
    fn pad(&mut self, bit: u16) -> crate::input::Report {
        let mut press = crate::input::Report { buttons: bit, sticks: [0.5; 4], ..Default::default() };
        ws::apply_pad(&mut press);
        self.step();
        let mut release = crate::input::Report::default();
        ws::apply_pad(&mut release);
        self.step();
        press
    }
}

/// Owner test 2026-11-24 (pad): a controller player's Start opens the mod's menu and its d-pad and cross are the menu's;
/// the character does not walk on the sticks while it is up. The game opens the menu in mode 13 and the mod's session is
/// made a step later in mode 0; the pad focus the mod asks for on opening was refused because the client's idea of "plain
/// play" was still the one from before.
#[test]
fn a6_the_pad_drives_the_mods_menu_and_keeps_the_character_still() {
    use crate::input::bit;
    let Some((img, words)) = disc_menu() else {
        eprintln!("SKIP a6 pad: no disc");
        return;
    };
    let mut r = Rig::new(&img, &words, None, false);
    // the pad's Start is the game's key 144, as the keyboard's Escape is the game's `open_start_menu`
    r.p.open_start_menu(true);
    r.step();
    let id = r.menu_session().expect("the mod's menu opened").id;
    assert!(ws::is_focused(&id), "the mod holds the pad as its menu opens");
    // down three rows (Ability), cross: the game's page
    for _ in 0..3 {
        let seen = r.pad(bit::DOWN);
        assert_eq!(seen.buttons & bit::DOWN, 0, "the d-pad is the menu's, not the game's");
        assert_eq!(seen.sticks, [0.0; 4], "the character does not walk on the sticks while the menu holds the pad");
    }
    r.pad(bit::CROSS);
    r.step();
    assert_eq!(r.p.mode, 5, "the pad's cross chose Ability: the game's page (mode {})", r.p.mode);
    assert!(r.menu_session().is_none());
    // triangle closes the menu at the top
    r.p.mode = 0;
    r.p.open_start_menu(true);
    r.step();
    assert!(r.menu_session().is_some());
    r.pad(bit::TRIANGLE);
    assert!(r.menu_session().is_none(), "the pad's triangle closed the menu");
    assert!(ws::sessions_for_test().iter().all(|s| !ws::is_focused(&s.id)), "no focus is left behind");
}

impl Rig {
    /// A world is left and the next one entered, as the client does it (`api_capture`: `Core::reset`, then the new
    /// world's state published), and the mods are told.
    fn change_world(&mut self, reason: &str) {
        native_core::CORE.with(|c| c.borrow_mut().reset(reason));
        self.step();
        native_core::CORE.with(|c| {
            c.borrow_mut().publish(native_core::View { in_world: true, phase: "world", world_id: Some(7), ..Default::default() })
        });
        self.step();
    }
}

/// Owner audit 2026-11-24 (World Map review, C1): after a zone, the Start key opened the game's menu and Enter the game's
/// chat line, because the Start Menu and Chat mods claimed once at load and the client lets go of every claim when a
/// world is left. Through the real keys: Escape and Enter still go to the mods after each kind of world change, and so
/// does a mod's hotkey. The negative control is the old Lua on disk (run recorded in the report).
#[test]
fn a6_start_key_and_enter_still_go_to_the_mods_after_each_kind_of_world_change() {
    let Some((img, words)) = disc_menu() else {
        eprintln!("SKIP a6 world change: no disc");
        return;
    };
    for reason in ["zone", "respawn", "disconnect"] {
        let mut r = Rig::new(&img, &words, None, true);
        // before: the mods have both
        r.escape();
        r.step();
        assert!(r.menu_session().is_some(), "{reason}: Escape opens the mod's menu before the change");
        r.escape();
        r.step();
        r.change_world(reason);
        assert_eq!(r.p.mode, 0);
        // Escape: the mod's menu, not the game's
        r.escape();
        r.step();
        assert!(r.menu_session().is_some(), "{reason}: Escape opens the mod's Start menu in the new world (mode {})", r.p.mode);
        assert_eq!(r.p.mode, 0, "{reason}: the game's own menu did not open");
        r.escape();
        r.step();
        assert!(r.menu_session().is_none());
        // Enter: the chat mod's line, not the game's
        r.enter();
        r.step();
        assert!(r.chat_line().is_some(), "{reason}: Enter opens the Chat mod's line in the new world (mode {})", r.p.mode);
        assert_eq!(r.p.mode, 0, "{reason}: the game's own chat box did not open");
        let line = r.chat_line().unwrap();
        ws::close(r.chat.vm, &line.id, "test").unwrap();
        r.step();
        // another mod's hotkey
        assert!(r.key("k", "shift+k", b'K'));
        assert!(ws::get(r.hot.vm, "spellbook").is_some(), "{reason}: Shift+K opens the Spellbook in the new world");
    }
}
