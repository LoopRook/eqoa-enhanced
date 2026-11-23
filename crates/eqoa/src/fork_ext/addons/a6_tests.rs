// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A6 without a running game: the two new takeover kinds and their rules, the session data a mod reads, the
//! chat reads (`tabs`, `reply_to`, a line's `kind`), the three actions' arguments and capabilities, and the two
//! bundled mods (Start Menu, Chat) loaded and driven the way a player does.
//!
//! Each test names its negative control: what it would see if the change were not there.
use super::{
    action_broker,
    bridge::Bridge,
    discovery::{DiscoveredAddon, EntryMode, Extensions},
    host::Host,
    native_a6,
    native_core::{self, CORE, Core},
    native_events,
    window_sessions as ws,
    window_takeover::{self, TakeoverKind},
};
use dear_imgui_rs::Context;
use serde_json::json;
use std::path::PathBuf;

fn fresh() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    native_a6::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    CORE.with(|c| c.borrow_mut().view.in_world = true);
}

fn host_with(id: &str, caps: &[&str], replaces: &[&str]) -> Host {
    let host = Host::new().unwrap();
    {
        let sess = super::native::session(&host.lua);
        let mut s = sess.borrow_mut();
        s.addon = id.to_string();
        s.native_manifest = true;
        s.capabilities = Some(caps.iter().map(|c| (*c).to_string()).collect());
        s.replaces = replaces.iter().map(|r| (*r).to_string()).collect();
    }
    host
}

const TAKEOVER: &[&str] = &["eqoa.windows.takeover", "eqoa.windows.session", "eqoa.events.on"];

#[test]
fn a6_start_menu_and_chat_entry_are_held_whole_and_declared() {
    fresh();
    let h = host_with("holder", TAKEOVER, &["start_menu", "chat_entry"]);
    h.execute(
        r#"
        for _, kind in ipairs({'start_menu', 'chat_entry'}) do
            local ok, e = eqoa.windows.takeover(kind, {scope = 'own'})
            assert(ok == nil and e.code == 'invalid_argument', kind .. ' own: ' .. tostring(e and e.code))
            local ok2, e2 = eqoa.windows.takeover(kind)
            assert(ok2 == nil and e2.code == 'invalid_argument', 'the default scope is own')
            assert(eqoa.windows.takeover(kind, {scope = 'all'}) == true, kind)
            assert(eqoa.windows.taken(kind) == true)
        end
        local all = eqoa.windows.taken()
        assert(all.start_menu == true and all.chat_entry == true and all.holders.start_menu ~= nil)
        assert(eqoa.windows.release('start_menu') == true)
        assert(eqoa.windows.taken('start_menu') == false and eqoa.windows.taken('chat_entry') == true)
    "#,
    )
    .unwrap();
    // `all` needs the manifest to say it replaces the window (the negative control: the old answer was 'unsupported')
    let bare = host_with("bare", TAKEOVER, &[]);
    bare.execute(
        r#"local ok, e = eqoa.windows.takeover('start_menu', {scope = 'all'}); assert(ok == nil and e.code == 'not_declared', e and e.code)"#,
    )
    .unwrap();
    // one holder: a second mod is told who has it
    let rival = host_with("rival", TAKEOVER, &["chat_entry"]);
    rival
        .execute(r#"local ok, e = eqoa.windows.takeover('chat_entry', {scope = 'all'}); assert(ok == nil and e.code == 'busy', e and e.code)"#)
        .unwrap();
    // the player's switch refuses it and releases it
    window_takeover::set_use_game_windows(true);
    assert!(!window_takeover::is_taken(TakeoverKind::ChatEntry), "the switch released the claim");
    h.execute(r#"local ok, e = eqoa.windows.takeover('chat_entry', {scope = 'all'}); assert(ok == nil and e.code == 'unavailable', e and e.code)"#).unwrap();
    window_takeover::set_use_game_windows(false);
}

#[test]
fn a6_a_mod_cannot_open_these_two_itself() {
    fresh();
    let h = host_with("opener", &["eqoa.windows.session"], &[]);
    h.execute(
        r#"
        for _, kind in ipairs({'start_menu', 'chat_entry'}) do
            local s, e = eqoa.windows.open_session(kind)
            assert(s == nil and e.code == 'invalid_argument', kind .. ': ' .. tostring(e and e.code))
        end
    "#,
    )
    .unwrap();
}

#[test]
fn a6_a_client_session_carries_its_data_and_changes_are_told() {
    fresh();
    let h = host_with("reader", TAKEOVER, &["start_menu"]);
    h.execute("log = {}; eqoa.events.on('windows.session', function(e) log[#log + 1] = e.data.phase .. ':' .. e.data.kind .. ':' .. e.data.revision end)").unwrap();
    let vm = native_events::bus(&h.lua).borrow().vm;
    let data = json!({"entries": [{"text": "Character", "enabled": true, "entries": [{"text": "Summary", "enabled": true, "command": "0x3020000:0"}]}]});
    let s = ws::open_client(vm, "reader", "start_menu", "key", data.clone());
    native_core::flush_deferred();
    native_events::dispatch(&h).unwrap();
    h.execute(
        r#"
        s = eqoa.windows.session('start_menu')
        assert(s.kind == 'start_menu' and s.origin == 'player' and s.opened_from == 'key' and s.title == 'Start Menu')
        assert(s.data.entries[1].text == 'Character' and s.data.entries[1].entries[1].command == '0x3020000:0')
        assert(log[1]:find('^opened:start_menu:'), log[1])
    "#,
    )
    .unwrap();
    // opened again: the same session, a new revision, told as changed
    let again = ws::open_client(vm, "reader", "start_menu", "key", data);
    assert_eq!(again.id, s.id);
    assert!(again.revision > s.revision);
    native_core::flush_deferred();
    native_events::dispatch(&h).unwrap();
    h.execute("assert(log[2]:find('^changed:start_menu:'), tostring(log[2]))").unwrap();
    // the mod closes it
    h.execute("assert(eqoa.windows.close_session(s.ref) == true); assert(eqoa.windows.session('start_menu') == nil)").unwrap();
    // control: a session the mod's window opened itself has no data
    let own = ws::open(vm, "reader", "inventory", "key").unwrap();
    assert!(own.json(false).get("data").is_none());
}

#[test]
fn a6_chat_reads() {
    fresh();
    let h = host_with("reader", &["eqoa.events.on"], &[]);
    h.execute("local t, e = eqoa.chat.tabs(); assert(t == nil and e.code == 'not_in_world', e and e.code); local r, e2 = eqoa.chat.reply_to(); assert(r == nil and e2.code == 'not_in_world')").unwrap();
    native_a6::set_chat_info(native_a6::ChatInfo {
        tabs: vec![(0, "Main".into(), vec!["say", "system"]), (2, "Loot".into(), vec!["loot", "rolls"])],
        reply_to: "Dab".into(),
    });
    h.execute(
        r#"
        local t = eqoa.chat.tabs()
        assert(#t.tabs == 2 and t.tabs[1].name == 'Main' and t.tabs[1].index == 0 and t.tabs[2].index == 2)
        assert(t.tabs[2].kinds[1] == 'loot' and t.tabs[2].kinds[2] == 'rolls')
        assert(#t.kinds == 10 and t.kinds[1] == 'say' and t.kinds[10] == 'system')
        assert(eqoa.chat.reply_to() == 'Dab')
    "#,
    )
    .unwrap();
    native_a6::set_chat_info(native_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec![])], reply_to: String::new() });
    h.execute("assert(eqoa.chat.reply_to() == nil, 'nobody has sent a tell')").unwrap();
}

#[test]
fn a6_a_chat_message_says_what_kind_of_line_it_is() {
    fresh();
    let h = host_with("reader", &["eqoa.events.on"], &[]);
    for (text, color) in [
        ("Dab tells you, 'hello'", [1.0, 0.5, 1.0, 1.0]),
        ("Dab shouts: hello", [1.0; 4]),
        ("You gain experience!", [1.0; 4]),
        ("Dab tells the group, 'pull'", [0.5, 0.5, 1.0, 1.0]),
    ] {
        native_core::observe_text(text, color);
    }
    native_core::flush_deferred();
    h.execute(
        r#"
        local m = eqoa.chat.history(10)
        assert(m[1].kind == 'tells' and m[2].kind == 'shouts' and m[4].kind == 'group', m[1].kind .. m[2].kind .. m[4].kind)
        assert(#m[1].links == 0)
    "#,
    )
    .unwrap();
}

#[test]
fn a6_chat_line_checks_its_arguments_and_needs_only_the_chat_capability() {
    fresh();
    let h = host_with("sender", &["eqoa.actions.chat"], &[]);
    h.execute(
        r#"
        local function bad(code, ...)
            local r, e = eqoa.actions.chat_line(...)
            assert(r == nil and e.code == code, tostring(code) .. ' got ' .. tostring(e and e.code) .. ' for ' .. tostring((...)))
        end
        bad('invalid_argument', 'nowhere', 'hi')
        bad('invalid_argument', 'say', '')
        bad('invalid_argument', 'say', '/quit', nil)
        bad('invalid_argument', 'say', '#x')
        bad('invalid_argument', 'tell', 'hi')
        bad('invalid_argument', 'tell', 'hi', {to = 'two words'})
        bad('invalid_argument', 'say', 'hi', {to = 'Dab'})
        bad('invalid_argument', 'say', '/bug it broke')
        bad('invalid_argument', 'tell', '#bug it broke', {to = 'Dab'})
        bad('out_of_range', 'say', string.rep('x', 126))
        bad('out_of_range', 'say', 'abcdefgh', {links = {{start = 5, len = 9, list = 'inventory', key = 1}}})
        bad('out_of_range', 'say', 'abcdefghij', {links = {
            {start = 1, len = 1, list = 'inventory', key = 1}, {start = 2, len = 1, list = 'inventory', key = 2},
            {start = 3, len = 1, list = 'inventory', key = 3}, {start = 4, len = 1, list = 'inventory', key = 4},
            {start = 5, len = 1, list = 'inventory', key = 5}}})
        bad('invalid_argument', 'say', 'abc', {links = {{start = 1, len = 1, list = 'garden', key = 1}}})
        -- accepted: each channel, a tell with a name, a link
        for _, c in ipairs({'say', 'group', 'guild', 'shout', 'reply'}) do assert(eqoa.actions.chat_line(c, 'hello')) end
        assert(eqoa.actions.chat_line('tell', 'hello', {to = 'Wil'}))
        assert(eqoa.actions.chat_line('say', 'look Sword', {links = {{start = 6, len = 5, list = 'inventory', key = 11}}}))
    "#,
    )
    .unwrap();
    assert_eq!(action_broker::queued_count_for_test(), 7);
    assert!(action_broker::queued_kinds_for_test().iter().all(|k| *k == "chat_line"));
    // the negative control: without the chat capability nothing is queued, and the refusal says which
    let none = host_with("mute", &["eqoa.actions.command"], &[]);
    none.execute("local r, e = eqoa.actions.chat_line('say', 'hi'); assert(r == nil and e.code == 'permission_denied', e and e.code)").unwrap();
    let undeclared = Host::new().unwrap();
    undeclared.execute("local r, e = eqoa.actions.chat_line('say', 'hi'); assert(r == nil and e.code == 'not_declared', e and e.code)").unwrap();
}

#[test]
fn a6_set_channel_and_open_link_and_menu_command_need_their_own_capability() {
    fresh();
    let none = host_with("none", &["eqoa.actions.chat"], &[]);
    none.execute(
        r#"
        local r, e = eqoa.actions.set_channel('say'); assert(r == nil and e.code == 'permission_denied', e and e.code)
        local r2, e2 = eqoa.actions.open_link(1, 'x'); assert(r2 == nil and e2.code == 'permission_denied', e2 and e2.code)
        local r3, e3 = eqoa.actions.menu_command('ws-1', 'x'); assert(r3 == nil and e3.code == 'permission_denied', e3 and e3.code)
    "#,
    )
    .unwrap();
    let h = host_with("all", &["eqoa.actions.set_channel", "eqoa.actions.open_link", "eqoa.actions.menu_command"], &[]);
    h.execute(
        r#"
        for _, c in ipairs({'say', 'group', 'shout', 'guild'}) do assert(eqoa.actions.set_channel(c)) end
        local r, e = eqoa.actions.set_channel('tell'); assert(r == nil and e.code == 'invalid_argument')
        local r2, e2 = eqoa.actions.open_link(4242, 'Rusty Sword'); assert(r2 == nil and e2.code == 'not_found', e2 and e2.code)
        local r3, e3 = eqoa.actions.menu_command('ws-1', '0xa010000:0'); assert(r3 == nil and e3.code == 'stale_handle', e3 and e3.code)
    "#,
    )
    .unwrap();
    // a link a chat line carried can be opened (queued: the client opens the card)
    native_a6::stash_links(&[(0, 5, 4242)]);
    native_core::observe_text("Sword here", [1.0; 4]);
    h.execute("assert(eqoa.actions.open_link(4242, 'Sword'))").unwrap();
    h.execute("local r, e = eqoa.actions.open_link(4242, ''); assert(r == nil and e.code == 'invalid_argument')").unwrap();
    // a link stashed for an event that made no line is not kept for the next line
    native_a6::stash_links(&[(0, 4, 9999)]);
    native_a6::event_boundary();
    native_a6::event_boundary();
    native_core::observe_text("Plain line", [1.0; 4]);
    h.execute("local r, e = eqoa.actions.open_link(9999, 'Plai'); assert(r == nil and e.code == 'not_found', e and e.code)").unwrap();
}

// ---- the two bundled mods ----

fn root(id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(id).canonicalize().unwrap()
}

fn manifest(id: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root(id).join("mod.json")).unwrap()).unwrap()
}

fn addon(id: &str) -> DiscoveredAddon {
    let m = manifest(id);
    let list = |name: &str| -> Vec<String> {
        m[name].as_array().map(|a| a.iter().map(|c| c.as_str().unwrap().to_string()).collect()).unwrap_or_default()
    };
    DiscoveredAddon {
        id: id.into(),
        name: id.into(),
        entry: root(id).join("main.lua"),
        script_root: root(id),
        package_root: root(id),
        module_dirs: vec![root(id)],
        resource_dirs: vec![root(id)],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: list("capabilities"),
        extensions: Extensions { replaces: list("replaces"), provides: list("provides"), ..Default::default() },
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

fn frames(host: &Host, bridge: &Bridge, context: &mut Context, n: usize) -> usize {
    let mut vertices = 0;
    for _ in 0..n {
        context.frame();
        bridge
            .invoke(|| {
                native_events::dispatch(host)?;
                native_events::render(host)
            })
            .unwrap();
        vertices = context.render_legacy().draw_data().total_vtx_count();
    }
    vertices
}

fn key(host: &Host, name: &str, text: &str) {
    CORE.with(|c| {
        c.borrow_mut().emit(
            "input.key",
            json!({"key": name, "combo": name, "down": true, "repeat": false, "synthetic": false, "text": text}),
        )
    });
    native_events::dispatch(host).unwrap();
}

/// Every `eqoa.actions.<name>` and `eqoa.windows.<name>` the script calls is declared, and the other way round.
fn declared_covers_used(id: &str) {
    let src = std::fs::read_to_string(root(id).join("main.lua")).unwrap();
    let declared: Vec<String> = manifest(id)["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
    let used = |prefix: &str| -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = src.as_str();
        while let Some(i) = rest.find(prefix) {
            rest = &rest[i + prefix.len()..];
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !found.contains(&name) {
                found.push(name);
            }
        }
        found
    };
    // `chat_line` is an ordinary chat line: it needs `eqoa.actions.chat`
    let need = |name: &str| if name == "chat_line" { "chat".to_string() } else { name.to_string() };
    let calls: Vec<String> = used("eqoa.actions.").iter().map(|n| need(n)).collect();
    for name in &calls {
        assert!(declared.contains(&format!("eqoa.actions.{name}")), "{id} calls eqoa.actions.{name} and must declare it");
    }
    for cap in declared.iter().filter(|c| c.starts_with("eqoa.actions.")) {
        let name = cap.trim_start_matches("eqoa.actions.");
        assert!(calls.iter().any(|u| u == name), "{id} declares {cap} and never calls it");
    }
    assert!(used("eqoa.windows.").iter().any(|n| n == "takeover"), "{id} holds its window");
    assert!(declared.contains(&"eqoa.windows.takeover".to_string()));
}

fn ships(id: &str) {
    let m = manifest(id);
    assert_eq!(m["enabled"], false, "{id} ships off");
    assert!(m["provides"].as_array().is_some_and(|p| p.len() == 1), "{id} says what it provides");
    declared_covers_used(id);
    assert!(crate::fork_ext::bundled::PACKAGES.contains(&id));
    for file in ["mod.json", "main.lua", "README.md"] {
        assert!(crate::fork_ext::bundled::EMBEDDED_FILES.iter().any(|f| f.path == format!("{id}/{file}")), "{id}/{file} ships");
    }
    // LF line endings (the mutation tests read the source)
    for file in ["mod.json", "main.lua", "README.md"] {
        let bytes = std::fs::read(root(id).join(file)).unwrap();
        assert!(!bytes.contains(&b'\r'), "{id}/{file} is LF");
    }
}

#[test]
fn a6_start_menu_mod_shows_the_session_and_runs_a_command_on_the_keys() {
    fresh();
    ships("eqoa_start_menu");
    let mut context = context();
    let host = Host::for_addon(&addon("eqoa_start_menu")).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon("eqoa_start_menu")).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    assert!(window_takeover::is_taken(TakeoverKind::StartMenu), "it holds the Start key: {:?}", host.drain_logs());
    // shut it draws nothing
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0);
    let data = json!({"entries": [
        {"text": "Character", "enabled": true, "entries": [{"text": "Quest Journal", "enabled": true, "command": "0xb0a0000:0"}]},
        {"text": "Quit", "enabled": true, "command": "0xa010000:0"},
    ]});
    ws::open_client(vm, "eqoa_start_menu", "start_menu", "key", data);
    native_core::flush_deferred();
    let vertices = frames(&host, &bridge, &mut context, 3);
    let logs = host.drain_logs().unwrap();
    assert!(vertices > 0, "it draws the menu: {logs:?}");
    assert!(logs.is_empty(), "no diagnostic: {logs:?}");
    // the pad: the mod holds it while its menu is up
    assert!(ws::is_focused(&ws::get(vm, "start_menu").unwrap().id), "the mod took the d-pad");
    // enter a sub-menu, run its command: one menu_command request, naming the session
    key(&host, "return", "");
    key(&host, "return", "");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["menu_command"], "logs {:?}", host.drain_logs());
    // the client ends the menu when the game's entry runs (`a6_play`)
    let id = ws::get(vm, "start_menu").unwrap().id;
    ws::close(vm, &id, "command").unwrap();
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 2);
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0, "the mod drew nothing once it was told");
    // Escape goes up a menu and then closes it, ending the session
    action_broker::reset_all_for_test();
    ws::open_client(vm, "eqoa_start_menu", "start_menu", "key", json!({"entries": [{"text": "Character", "enabled": true, "entries": [{"text": "A", "enabled": true, "command": "0x1:0"}]}]}));
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 2);
    key(&host, "return", "");
    key(&host, "escape", "");
    assert!(ws::get(vm, "start_menu").is_some(), "the first Escape goes up one menu");
    key(&host, "escape", "");
    assert!(ws::get(vm, "start_menu").is_none(), "the second closes it");
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0, "and it draws nothing");
    // a disabled entry does nothing; disabling the mod gives the Start key back
    ws::open_client(vm, "eqoa_start_menu", "start_menu", "key", json!({"entries": [{"text": "Duel", "enabled": false, "command": "0x2:0"}]}));
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 2);
    key(&host, "return", "");
    assert_eq!(action_broker::queued_count_for_test(), 0, "a dimmed entry is not run");
    native_events::clear(&host);
    assert!(!window_takeover::is_taken(TakeoverKind::StartMenu), "disabled: the game's Start key again");
}

#[test]
fn a6_chat_mod_has_all_and_a_tab_per_style_and_sends_on_the_picked_channel() {
    fresh();
    ships("eqoa_chat");
    let mut context = context();
    for (text, color) in [("Dab: hi", [1.0f32; 4]), ("You hit a rat for 3 points of damage.", [1.0; 4])] {
        native_core::observe_text(text, color);
    }
    native_core::flush_deferred();
    native_a6::set_chat_info(native_a6::ChatInfo {
        tabs: vec![(0, "Main".into(), vec!["say", "combat"])],
        reply_to: "Wil".into(),
    });
    let host = Host::for_addon(&addon("eqoa_chat")).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon("eqoa_chat")).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    assert!(window_takeover::is_taken(TakeoverKind::ChatEntry), "it holds the Enter key: {:?}", host.drain_logs());
    let vertices = frames(&host, &bridge, &mut context, 3);
    let logs = host.drain_logs().unwrap();
    assert!(vertices > 0, "the chat window is always drawn: {logs:?}");
    assert!(logs.is_empty(), "{logs:?}");
    // the player presses Enter: the client opens the mod's line (what `a6_play::chat_entry` does)
    ws::open_client(vm, "eqoa_chat", "chat_entry", "key", json!({"text": "", "channel": "group", "links": []}));
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 2);
    for c in ["h", "e", "y"] {
        key(&host, c, c);
    }
    key(&host, "return", "");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["chat_line"], "logs {:?}", host.drain_logs());
    assert!(ws::get(vm, "chat_entry").is_none(), "sent: the line closed");
    // an empty Enter just closes it; Escape cancels without sending
    action_broker::reset_all_for_test();
    ws::open_client(vm, "eqoa_chat", "chat_entry", "key", json!({"text": "", "channel": "say", "links": []}));
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 2);
    key(&host, "return", "");
    assert!(ws::get(vm, "chat_entry").is_none());
    ws::open_client(vm, "eqoa_chat", "chat_entry", "key", json!({"text": "", "channel": "say", "links": []}));
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 2);
    key(&host, "x", "x");
    key(&host, "escape", "");
    assert!(ws::get(vm, "chat_entry").is_none());
    assert_eq!(action_broker::queued_count_for_test(), 0, "nothing was sent");
    // a link offered by the Bag Viewer goes into the line, and a slash line is a command
    ws::open_client(vm, "eqoa_chat", "chat_entry", "key", json!({"text": "", "channel": "say", "links": [], "offer": {"seq": 1, "name": "Rusty Sword", "list": "inventory", "key": 11}}));
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 2);
    key(&host, "return", "");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["chat_line"], "the linked line goes: {:?}", host.drain_logs());
    action_broker::reset_all_for_test();
    ws::open_client(vm, "eqoa_chat", "chat_entry", "key", json!({"text": "/", "channel": "say", "links": []}));
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 2);
    for c in ["g", "r", "o", "u", "p"] {
        key(&host, c, c);
    }
    key(&host, "return", "");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["command"]);
    // disabling gives the Enter key back
    native_events::clear(&host);
    assert!(!window_takeover::is_taken(TakeoverKind::ChatEntry));
}

#[test]
fn a6_chat_mod_tabs_are_all_plus_one_per_style_or_the_players_own() {
    fresh();
    let source = std::fs::read_to_string(root("eqoa_chat").join("main.lua")).unwrap();
    for needle in ["id = \"all\"", "STYLE_LABELS", "eqoa.chat.tabs()", "Notices", "own_tabs"] {
        assert!(source.contains(needle), "the chat mod has {needle}");
    }
    for kind in ["say", "tells", "group", "guild", "shouts", "combat", "loot", "rolls", "experience", "system"] {
        assert!(source.contains(&format!("{kind} = \"")), "a label for the {kind} style");
    }
}
