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
    assert_eq!(own.json(false)["data"], json!({}), "data is always a table: empty for every other window");
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
fn a6_chat_line_checks_its_arguments() {
    fresh();
    let h = host_with("sender", &["eqoa.actions.chat", "eqoa.actions.command"], &[]);
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
    // the frozen slash rule: every channel but say goes as a slash command and needs `command`; say needs `chat`
    let chat_only = host_with("chat_only", &["eqoa.actions.chat"], &[]);
    chat_only.execute(
        r#"
        assert(eqoa.actions.chat_line('say', 'hello'), 'say is an ordinary line')
        for _, c in ipairs({'group', 'guild', 'shout', 'reply'}) do
            local r, e = eqoa.actions.chat_line(c, 'hello'); assert(r == nil and e.code == 'permission_denied', c .. ': ' .. tostring(e and e.code))
        end
        local r, e = eqoa.actions.chat_line('tell', 'hello', {to = 'Wil'}); assert(r == nil and e.code == 'permission_denied', e and e.code)
    "#,
    )
    .unwrap();
    let command_only = host_with("command_only", &["eqoa.actions.command"], &[]);
    command_only.execute(
        r#"
        assert(eqoa.actions.chat_line('group', 'hello'))
        local r, e = eqoa.actions.chat_line('say', 'hello'); assert(r == nil and e.code == 'permission_denied', e and e.code)
    "#,
    )
    .unwrap();
    action_broker::reset_all_for_test();
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
    // (a bare channel word such as /group only changes the channel now; a real command is still sent)
    for c in ["w", "h", "o"] {
        key(&host, c, c);
    }
    key(&host, "return", "");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["command"]);
    // disabling gives the Enter key back
    native_events::clear(&host);
    assert!(!window_takeover::is_taken(TakeoverKind::ChatEntry));
}

/// Owner test 2026-11-23: the chat window could not be moved or sized like the other mod windows. Its top strip moves it,
/// its corner sizes it (within limits), and where it ends up is kept in the mod's settings and comes back.
/// Negative controls: a press and drag on the lines moves nothing; the window starts where it always did; a size is
/// clamped to the smallest.
#[test]
fn a6_chat_window_moves_and_sizes_with_the_pointer_and_keeps_its_place() {
    use super::{native_settings, pointer_api};
    use std::{cell::RefCell, rc::Rc};
    fresh();
    ships("eqoa_chat");
    let mut context = context();
    // each frame is a new frame of the client's (a pointer claim lasts one frame, the old one is dropped)
    let ticks = |host: &Host, bridge: &Bridge, context: &mut Context, n: usize| {
        for _ in 0..n {
            CORE.with(|c| c.borrow_mut().frame += 1);
            frames(host, bridge, context, 1);
        }
    };
    let host = Host::for_addon(&addon("eqoa_chat")).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let saved: Rc<RefCell<Vec<serde_json::Value>>> = Rc::default();
    let sink = saved.clone();
    native_settings::configure(&host, None, Rc::new(move |v| {
        sink.borrow_mut().push(v.clone());
        Ok(())
    }));
    host.run_addon(&addon("eqoa_chat")).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    ticks(&host, &bridge, &mut context, 3);
    // the window's box, found by what its pointer claim covers
    let box_of = |vm: u64| -> (f64, f64, f64, f64) {
        pointer_api::position([0.0, 0.0], true, (1024, 768));
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut y = 0.0;
        while y < 768.0 {
            let mut x = 0.0;
            while x < 1024.0 {
                if pointer_api::point_in_vm_claim(vm, x, y) {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
                x += 2.0;
            }
            y += 2.0;
        }
        (x0, y0, x1, y1)
    };
    let boxed = || box_of(vm);
    let drag = |host: &Host, context: &mut Context, bridge: &Bridge, from: (f64, f64), to: (f64, f64)| {
        pointer_api::position([from.0 as f32, from.1 as f32], true, (1024, 768));
        context.io_mut().add_mouse_pos_event([from.0 as f32, from.1 as f32]);
        CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": from.0, "y": from.1, "inside_window": true, "button": "left"})));
        context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
        ticks(host, bridge, context, 2);
        pointer_api::position([to.0 as f32, to.1 as f32], true, (1024, 768));
        context.io_mut().add_mouse_pos_event([to.0 as f32, to.1 as f32]);
        ticks(host, bridge, context, 2);
        context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
        ticks(host, bridge, context, 3);
    };
    let start = boxed();
    assert!(start.2 > start.0 && start.3 > start.1, "the window is up and claims its place: {start:?}");
    // control: a press on the lines moves nothing
    drag(&host, &mut context, &bridge, (start.0 + 40.0, start.1 + 70.0), (start.0 + 240.0, start.1 - 30.0));
    let after = boxed();
    assert_eq!((after.0, after.1, after.2, after.3), (start.0, start.1, start.2, start.3), "control: the lines are not a handle");
    // the top strip moves it: 120 right, 60 up
    drag(&host, &mut context, &bridge, (start.0 + 40.0, start.1 + 3.0), (start.0 + 160.0, start.1 - 57.0));
    let moved = boxed();
    assert!((moved.0 - (start.0 + 120.0)).abs() <= 4.0 && (moved.1 - (start.1 - 60.0)).abs() <= 4.0, "moved: {start:?} -> {moved:?}");
    assert!(((moved.2 - moved.0) - (start.2 - start.0)).abs() <= 2.0, "moving keeps the size");
    // the corner sizes it: 90 wider, 40 taller
    drag(&host, &mut context, &bridge, (moved.2 - 3.0, moved.3 - 3.0), (moved.2 + 87.0, moved.3 + 37.0));
    let sized = boxed();
    assert!(((sized.2 - sized.0) - (moved.2 - moved.0 + 90.0)).abs() <= 4.0, "wider: {moved:?} -> {sized:?}");
    assert!(((sized.3 - sized.1) - (moved.3 - moved.1 + 40.0)).abs() <= 4.0, "taller: {moved:?} -> {sized:?}");
    // control: it cannot be made smaller than the smallest
    drag(&host, &mut context, &bridge, (sized.2 - 3.0, sized.3 - 3.0), (sized.2 - 1500.0, sized.3 - 1500.0));
    let small = boxed();
    assert!((small.2 - small.0) > 100.0 && (small.3 - small.1) > 30.0, "clamped to the smallest: {small:?}");
    // the place is kept in the settings, and a fresh start takes it back
    let last = saved.borrow().last().cloned().expect("saved at the end of a drag");
    let text = last.to_string();
    assert!(["\"x\"", "\"y\"", "\"w\"", "\"h\""].iter().all(|k| text.contains(k)), "the place and size are saved: {text}");
    drop(bridge);
    drop(host);
    fresh();
    let host2 = Host::for_addon(&addon("eqoa_chat")).unwrap();
    let bridge2 = Bridge::install(&host2).unwrap();
    native_settings::configure(&host2, Some(last.clone()), Rc::new(|_| Ok(())));
    host2.run_addon(&addon("eqoa_chat")).unwrap();
    // the manager hands the saved value over when the mod is first enabled
    let value = super::profile::decode(&host2.lua, &last).unwrap();
    host2.callback(3, value).unwrap();
    let vm2 = native_events::bus(&host2.lua).borrow().vm;
    ticks(&host2, &bridge2, &mut context, 3);
    let back = box_of(vm2);
    assert!((back.0 - small.0).abs() <= 4.0 && (back.1 - small.1).abs() <= 4.0 && (back.2 - small.2).abs() <= 4.0 && (back.3 - small.3).abs() <= 4.0, "the saved place came back: {small:?} vs {back:?}");
    assert!((back.0 - start.0).abs() > 20.0 || (back.1 - start.1).abs() > 20.0, "control: not the default place");
}

/// Owner request 2026-11-23 (item 8): tabs are edited inside the chat window. A right click on a tab opens Rename, Move
/// left, Move right, Delete and "Show in this tab" with a check per style; a right click on the empty strip (or the +)
/// makes a New tab that starts with nothing ticked; All can be renamed and moved, never deleted; the tabs are kept in
/// the mod's settings and Reset tabs brings the default set back; the Styles/Mine switch and the Notices tab are gone (an old saved mode is ignored).
/// Negative controls are inline: a left click opens no menu, All has no Delete or style rows, the first tab's Move left
/// is off, Escape drops a rename, a new tab shows nothing while All shows the lines, no Mine or Notices view exists.
#[test]
fn a6_chat_tabs_are_edited_inside_the_window_and_kept() {
    use super::{native_settings, pointer_api};
    use std::{cell::RefCell, rc::Rc};
    fresh();
    ships("eqoa_chat");
    let mut context = context();
    for (text, color) in [("Dab: hi", [1.0f32; 4]), ("You hit a rat for 3 points of damage.", [1.0; 4])] {
        native_core::observe_text(text, color);
    }
    native_core::flush_deferred();
    native_a6::set_chat_info(native_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec!["say", "combat"])], reply_to: String::new() });
    let host = Host::for_addon(&addon("eqoa_chat")).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let saved: Rc<RefCell<Vec<serde_json::Value>>> = Rc::default();
    let sink = saved.clone();
    native_settings::configure(&host, None, Rc::new(move |v| {
        sink.borrow_mut().push(v.clone());
        Ok(())
    }));
    host.run_addon(&addon("eqoa_chat")).unwrap();
    let ticks = |context: &mut Context, n: usize| {
        for _ in 0..n {
            CORE.with(|c| c.borrow_mut().frame += 1);
            frames(&host, &bridge, context, 1);
        }
    };
    ticks(&mut context, 3);
    // wide enough for every tab and some empty strip
    host.execute("chat_probe.state.w = 640").unwrap();
    ticks(&mut context, 2);
    pointer_api::position([0.0, 0.0], true, (1024, 768));
    let names = |host: &Host| -> Vec<String> { host.lua.load("return chat_probe.names()").eval().unwrap() };
    let center = |id: &str| -> Option<(f64, f64)> {
        let (x, y): (f64, f64) = host
            .lua
            .load(format!("local h = chat_probe.hit('{id}'); if not h then return -1, -1 end; return h.x + h.w / 2, h.y + h.h / 2"))
            .eval()
            .unwrap();
        (x >= 0.0).then_some((x, y))
    };
    let enabled = |id: &str| -> Option<bool> { host.lua.load(format!("local h = chat_probe.hit('{id}'); if not h then return nil end; return h.enabled ~= false")).eval().unwrap() };
    let press = |context: &mut Context, at: (f64, f64), button: &str| {
        pointer_api::position([at.0 as f32, at.1 as f32], true, (1024, 768));
        CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": at.0, "y": at.1, "inside_window": true, "button": button})));
        ticks(context, 2);
    };
    let click = |context: &mut Context, id: &str, button: &str| {
        let at = center(id).unwrap_or_else(|| panic!("{id} has a place: {:?}", names(&host)));
        press(context, at, button);
    };
    let visible = || -> i64 { host.lua.load("return chat_probe.visible()").eval().unwrap() };
    let cuts = || -> i64 { host.lua.load("return chat_probe.state.menu_cuts").eval().unwrap() };
    let typed = |context: &mut Context, text: &str| {
        for c in text.chars() {
            key(&host, &c.to_string(), &c.to_string());
        }
        ticks(context, 1);
    };

    // the default set: All and a tab per style (no Notices, no Mine)
    let start = names(&host);
    assert_eq!(start.first().map(String::as_str), Some("All"));
    assert!(!start.iter().any(|n| n == "Notices" || n == "Main"), "no Notices tab and no game tabs: {start:?}");
    assert!(center("mode").is_none(), "no Styles/Mine switch");
    assert!(start.len() >= 4 && start.iter().any(|n| n == "Say"), "{start:?}");
    // control: a left click chooses the tab and opens no menu
    click(&mut context, "tab:say", "left");
    assert!(center("menu:Rename").is_none(), "a left click opens no menu");
    assert_eq!(visible(), 1, "the Say tab shows the one say line");
    // a right click on a style's tab: its menu
    click(&mut context, "tab:say", "right");
    for row in ["Rename", "Move left", "Move right", "Delete", "Show in this tab", "Say", "Group", "Guild", "Shout", "Tell", "Combat", "Loot", "Rolls", "XP", "System", "New tab", "Reset tabs"] {
        assert!(center(&format!("menu:{row}")).is_some(), "the menu has {row}");
    }
    assert_eq!(cuts(), 0, "the menu's text fits (nothing cut)");
    // ticking Combat in Say's tab adds the combat line; the menu stays open for more
    click(&mut context, "menu:Combat", "left");
    assert!(center("menu:Rename").is_some(), "ticking a style keeps the menu open");
    assert_eq!(host.lua.load("return chat_probe.hit('menu:Combat').check").eval::<bool>().unwrap(), true);
    assert_eq!(visible(), 2, "Say now shows the combat line too");
    click(&mut context, "menu:Combat", "left");
    assert_eq!(visible(), 1, "unticked: back to the say line");
    // a click outside closes the menu (and does nothing else)
    press(&mut context, (1000.0, 5.0), "left");
    assert!(center("menu:Rename").is_none(), "a click away closes the menu");
    // All: renamed and moved, never deleted, no styles to tick
    click(&mut context, "tab:all", "right");
    assert!(center("menu:Rename").is_some() && center("menu:Move right").is_some());
    assert!(center("menu:Delete").is_none() && center("menu:Say").is_none(), "control: All has no Delete and no styles");
    assert_eq!(enabled("menu:Move left"), Some(false), "control: the first tab cannot move left");
    click(&mut context, "menu:Move left", "left");
    assert_eq!(names(&host), start, "a disabled row does nothing");
    // move All to the right: it is no longer first, and it still shows everything
    click(&mut context, "menu:Move right", "left");
    let moved = names(&host);
    assert_eq!(moved[1], "All");
    assert_eq!(moved[0], start[1]);
    // and back
    click(&mut context, "tab:all", "right");
    click(&mut context, "menu:Move left", "left");
    assert_eq!(names(&host), start);
    // rename: Escape drops it (control), Enter keeps it, twelve characters at most
    click(&mut context, "tab:say", "right");
    click(&mut context, "menu:Rename", "left");
    typed(&mut context, "xyz");
    key(&host, "escape", "");
    ticks(&mut context, 2);
    assert_eq!(names(&host), start, "Escape keeps the old name");
    click(&mut context, "tab:say", "right");
    click(&mut context, "menu:Rename", "left");
    for _ in 0..6 {
        key(&host, "backspace", "");
    }
    typed(&mut context, "Talking all day long");
    key(&host, "enter", "");
    ticks(&mut context, 2);
    let renamed = names(&host);
    assert_eq!(renamed[start.iter().position(|n| n == "Say").unwrap()], "Talking all", "twelve characters (the trailing space is trimmed): {renamed:?}");
    assert_eq!(renamed.len(), start.len());
    // All renamed too
    click(&mut context, "tab:all", "right");
    click(&mut context, "menu:Rename", "left");
    for _ in 0..4 {
        key(&host, "backspace", "");
    }
    typed(&mut context, "Everything");
    key(&host, "enter", "");
    ticks(&mut context, 2);
    assert_eq!(names(&host)[0], "Everything");
    // delete the renamed tab; the others stay
    click(&mut context, "tab:say", "right");
    click(&mut context, "menu:Delete", "left");
    let after_delete = names(&host);
    assert_eq!(after_delete.len(), start.len() - 1);
    assert!(!after_delete.iter().any(|n| n == "Talking all"));
    // New tab from the +: starts with nothing ticked, so it shows nothing while All shows the lines
    click(&mut context, "tab:new", "left");
    let with_new = names(&host);
    assert_eq!(with_new.len(), after_delete.len() + 1);
    assert!(with_new.iter().any(|n| n == "New"));
    assert_eq!(visible(), 0, "a new tab starts with nothing ticked");
    click(&mut context, "tab:all", "left");
    assert_eq!(visible(), 2, "control: All shows the lines");
    // New tab from a right click on the empty strip: only New tab and Reset tabs
    let strip = center("strip").expect("there is empty strip beside the tabs");
    press(&mut context, strip, "right");
    assert!(center("menu:New tab").is_some() && center("menu:Reset tabs").is_some());
    assert!(center("menu:Rename").is_none(), "control: the strip's menu has no tab rows");
    click(&mut context, "menu:New tab", "left");
    assert_eq!(names(&host).len(), with_new.len() + 1);
    // dragging the window afterwards keeps the tabs in what is saved (the drag's save is the whole snapshot)
    let grip = center("move").expect("the move strip has a place");
    pointer_api::position([grip.0 as f32, grip.1 as f32], true, (1024, 768));
    context.io_mut().add_mouse_pos_event([grip.0 as f32, grip.1 as f32]);
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": grip.0, "y": grip.1, "inside_window": true, "button": "left"})));
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
    ticks(&mut context, 2);
    pointer_api::position([grip.0 as f32 + 30.0, grip.1 as f32 - 20.0], true, (1024, 768));
    context.io_mut().add_mouse_pos_event([grip.0 as f32 + 30.0, grip.1 as f32 - 20.0]);
    ticks(&mut context, 2);
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
    ticks(&mut context, 3);
    assert!(saved.borrow().last().is_some_and(|v| v.to_string().contains("\"tabs\"")), "the drag's save keeps the tabs");
    // the tabs were saved after each change, and come back in a fresh start
    let last = saved.borrow().last().cloned().expect("the tabs are saved");
    let want = names(&host);
    drop(bridge);
    drop(host);
    fresh();
    native_a6::set_chat_info(native_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec!["say", "combat"])], reply_to: String::new() });
    let host2 = Host::for_addon(&addon("eqoa_chat")).unwrap();
    let bridge2 = Bridge::install(&host2).unwrap();
    // a value saved by the old version also holds its mode: ignored, not an error
    let mut last = last;
    last["table"].as_array_mut().unwrap().push(json!(["own_tabs", true]));
    native_settings::configure(&host2, Some(last.clone()), Rc::new(|_| Ok(())));
    host2.run_addon(&addon("eqoa_chat")).unwrap();
    let value = super::profile::decode(&host2.lua, &last).unwrap();
    host2.callback(3, value).unwrap();
    for _ in 0..3 {
        CORE.with(|c| c.borrow_mut().frame += 1);
        frames(&host2, &bridge2, &mut context, 1);
    }
    assert_eq!(names(&host2), want, "the edited tabs come back");
    // Reset tabs brings the default set back (and that is what is kept)
    pointer_api::position([0.0, 0.0], true, (1024, 768));
    let (x, y): (f64, f64) = host2.lua.load("local h = chat_probe.hit('tab:all'); return h.x + h.w / 2, h.y + h.h / 2").eval().unwrap();
    pointer_api::position([x as f32, y as f32], true, (1024, 768));
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": "right"})));
    for _ in 0..2 {
        CORE.with(|c| c.borrow_mut().frame += 1);
        frames(&host2, &bridge2, &mut context, 1);
    }
    let (rx, ry): (f64, f64) = host2.lua.load("local h = chat_probe.hit('menu:Reset tabs'); return h.x + h.w / 2, h.y + h.h / 2").eval().unwrap();
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": rx, "y": ry, "inside_window": true, "button": "left"})));
    for _ in 0..3 {
        CORE.with(|c| c.borrow_mut().frame += 1);
        frames(&host2, &bridge2, &mut context, 1);
    }
    let reset = names(&host2);
    assert_eq!(reset, start, "Reset tabs restores All and one tab per style");
    assert!(host2.lua.load("return chat_probe.hit('mode') == nil").eval::<bool>().unwrap(), "the Styles/Mine switch is gone");
}

// ---- owner test 2026-11-23 evening (lane A): typing, rename, channels, Shift+T, combat -------------------------------

fn tick_n(host: &Host, bridge: &Bridge, context: &mut Context, n: usize) {
    for _ in 0..n {
        CORE.with(|c| c.borrow_mut().frame += 1);
        frames(host, bridge, context, 1);
    }
}

/// A key the way a real keyboard gives it to the mods (`input::route_mod_key`, the viewer's own policy): down and up.
fn real_key(host: &Host, key: winit::keyboard::Key) {
    super::input::route_mod_key(&key, true, false, false, true, false);
    super::input::route_mod_key(&key, false, false, false, true, false);
    native_core::flush_deferred();
    native_events::dispatch(host).unwrap();
}

fn real_text(host: &Host, text: &str) {
    for c in text.chars() {
        let key = if c == ' ' { winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space) } else { winit::keyboard::Key::Character(c.to_string().into()) };
        real_key(host, key);
    }
}

fn named(host: &Host, k: winit::keyboard::NamedKey) {
    real_key(host, winit::keyboard::Key::Named(k));
}

fn loaded() -> (Host, Bridge, Context, u64) {
    fresh();
    super::input::reset_input_state();
    ships("eqoa_chat");
    let context = context();
    native_a6::set_chat_info(native_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec!["say", "combat"])], reply_to: String::new() });
    let host = Host::for_addon(&addon("eqoa_chat")).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon("eqoa_chat")).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    (host, bridge, context, vm)
}

fn probe_str(host: &Host, code: &str) -> String {
    host.lua.load(format!("return tostring({code})")).eval().unwrap()
}

/// Item 3: "Nothing I type works in the rename, and typing fires the other hot keys." The client lets a text capture lapse
/// after one second unless the mod asks again; the mod asked once when the rename began. Now it asks every frame it is
/// wanted. The keys go in by the real route (`route_mod_key`), and while the rename is open the game's hot keys are the
/// mod's (`wants_key_capture`); when it ends they are the game's again.
/// Negative control: with the old Lua the capture is gone after a second of waiting (the first assertion) and nothing typed arrives.
#[test]
fn a6_chat_rename_takes_the_typed_keys_for_longer_than_a_second_and_holds_the_hot_keys() {
    let (host, bridge, mut context, _vm) = loaded();
    tick_n(&host, &bridge, &mut context, 3);
    host.execute("chat_probe.state.w = 640").unwrap();
    tick_n(&host, &bridge, &mut context, 2);
    super::pointer_api::position([0.0, 0.0], true, (1024, 768));
    let press = |context: &mut Context, id: &str, button: &str| {
        let (x, y): (f64, f64) = host.lua.load(format!("local h = chat_probe.hit('{id}'); return h.x + h.w / 2, h.y + h.h / 2")).eval().unwrap();
        super::pointer_api::position([x as f32, y as f32], true, (1024, 768));
        CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": button})));
        tick_n(&host, &bridge, context, 2);
    };
    press(&mut context, "tab:say", "right");
    press(&mut context, "menu:Rename", "left");
    assert!(super::input::is_text_capture_active(), "the rename holds the keyboard");
    // longer than the lapse: the capture is renewed (the old code asked once and lost it)
    std::thread::sleep(std::time::Duration::from_millis(1250));
    tick_n(&host, &bridge, &mut context, 2);
    assert!(super::input::is_text_capture_active(), "still held after more than a second");
    // the hot keys do not reach the game or other mods while it is open
    assert!(super::input::wants_key_capture(false, Some(b'w'), false, true), "W is the rename's, not the game's");
    assert!(super::input::wants_key_capture(false, Some(0x0D), false, true), "Enter is the rename's");
    // letters, a digit and a space, then Backspace, by the real route
    host.execute("chat_probe.state.rename.text = ''").unwrap();
    real_text(&host, "Hi 5x");
    named(&host, winit::keyboard::NamedKey::Backspace);
    assert_eq!(probe_str(&host, "chat_probe.state.rename.text"), "Hi 5", "typed by the real keyboard route");
    // Escape drops it: the name is as it was; the keyboard is let go
    named(&host, winit::keyboard::NamedKey::Escape);
    tick_n(&host, &bridge, &mut context, 2);
    assert!(host.lua.load("return chat_probe.state.rename == nil").eval::<bool>().unwrap());
    assert!(!super::input::is_text_capture_active(), "the keyboard is the game's again");
    assert!(!super::input::wants_key_capture(false, Some(b'w'), false, true), "control: W is the game's again");
    assert!(host.lua.load("return chat_probe.names()[2] == 'Say'").eval::<bool>().unwrap(), "Escape keeps the old name");
    // Enter keeps it
    press(&mut context, "tab:say", "right");
    press(&mut context, "menu:Rename", "left");
    for _ in 0..3 {
        named(&host, winit::keyboard::NamedKey::Backspace);
    }
    real_text(&host, "Talk");
    named(&host, winit::keyboard::NamedKey::Enter);
    tick_n(&host, &bridge, &mut context, 2);
    assert_eq!(probe_str(&host, "chat_probe.names()[2]"), "Talk");
}

fn open_line(host: &Host, bridge: &Bridge, context: &mut Context, vm: u64, text: &str, channel: &str) {
    ws::open_client(vm, "eqoa_chat", "chat_entry", "key", json!({"text": text, "channel": channel, "links": []}));
    native_core::flush_deferred();
    tick_n(host, bridge, context, 2);
}

/// Items 5 and 6: after a channel change by a slash command (/g, /s, /say, /gu, /t name), by Tab or by the buttons, typing
/// keeps going into the line; a line opened by T or Shift+T ("/tell ") takes the keys at once, even a second later.
/// Negative control: before, "/g" sent a command and closed the line, and the capture lapsed after a second.
#[test]
fn a6_chat_line_keeps_taking_keys_after_a_channel_change_and_for_a_tell() {
    let (host, bridge, mut context, vm) = loaded();
    tick_n(&host, &bridge, &mut context, 3);
    let state = |code: &str| probe_str(&host, &format!("chat_probe.state.{code}"));
    // "/g " (the space types the change): group, the line empty and open, then typing goes on
    open_line(&host, &bridge, &mut context, vm, "", "say");
    assert!(super::input::is_text_capture_active());
    real_text(&host, "/g ");
    assert_eq!((state("channel").as_str(), state("text").as_str()), ("group", ""), "/g changes the channel");
    assert!(ws::get(vm, "chat_entry").is_some(), "the line stays open");
    assert!(action_broker::queued_kinds_for_test().contains(&"set_channel"), "the game's chat mode follows: {:?}", action_broker::queued_kinds_for_test());
    real_text(&host, "hello");
    assert_eq!(state("text"), "hello");
    // a bare "/s" and Enter changes the channel too, and keeps the line
    for (cmd, want) in [("/s", "shout"), ("/say", "say"), ("/gu", "guild"), ("/group", "group")] {
        host.execute("chat_probe.state.text = ''").unwrap();
        real_text(&host, cmd);
        named(&host, winit::keyboard::NamedKey::Enter);
        tick_n(&host, &bridge, &mut context, 1);
        assert_eq!((state("channel").as_str(), state("text").as_str()), (want, ""), "{cmd}");
        assert!(ws::get(vm, "chat_entry").is_some(), "{cmd}: the line is still open");
        real_text(&host, "x");
        assert_eq!(state("text"), "x", "{cmd}: typing continues");
    }
    assert!(!action_broker::queued_kinds_for_test().contains(&"command"), "a channel command is not sent as a command");
    // "/t " (the space) changes to Tell; the name and the message are typed after it
    host.execute("chat_probe.state.text = ''").unwrap();
    real_text(&host, "/t Wil");
    assert_eq!((state("channel").as_str(), state("text").as_str()), ("tell", "Wil"));
    real_text(&host, " hi");
    assert_eq!(state("text"), "Wil hi");
    // and a pasted "/t Wil" with Enter keeps the name ready for the message
    host.execute("chat_probe.state.text = '/t Wil'; chat_probe.state.channel = 'say'").unwrap();
    named(&host, winit::keyboard::NamedKey::Enter);
    assert_eq!((state("channel").as_str(), state("text").as_str()), ("tell", "Wil "));
    // Tab goes round the channels and typing continues; so do the buttons
    host.execute("chat_probe.state.text = ''").unwrap();
    named(&host, winit::keyboard::NamedKey::Tab);
    assert_eq!(state("channel"), "reply", "Tab goes on from Tell");
    real_text(&host, "ok");
    assert_eq!(state("text"), "ok", "typing after Tab");
    let (x, y): (f64, f64) = host.lua.load("local h = chat_probe.hit('chan:guild'); return h.x + h.w / 2, h.y + h.h / 2").eval().unwrap();
    super::pointer_api::position([x as f32, y as f32], true, (1024, 768));
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": "left"})));
    tick_n(&host, &bridge, &mut context, 2);
    assert_eq!(state("channel"), "guild");
    real_text(&host, "yo");
    assert_eq!(state("text"), "okyo", "typing after a button");
    // more than a second later the line still takes keys (the capture is renewed)
    std::thread::sleep(std::time::Duration::from_millis(1250));
    tick_n(&host, &bridge, &mut context, 2);
    real_text(&host, "!");
    assert_eq!(state("text"), "okyo!", "typing after a second's wait");
    // control: a real command (not a bare channel word) is still sent, and closes the line
    host.execute("chat_probe.state.text = '/who'").unwrap();
    named(&host, winit::keyboard::NamedKey::Enter);
    assert!(action_broker::queued_kinds_for_test().contains(&"command"), "/who is a command");
    assert!(ws::get(vm, "chat_entry").is_none(), "sent: the line closed");
    // Shift+T: the game opens its line with "/tell "; the mod's line is Tell with nothing to delete, and takes the keys at once
    action_broker::reset_all_for_test();
    open_line(&host, &bridge, &mut context, vm, "/tell ", "say");
    assert_eq!((state("channel").as_str(), state("text").as_str()), ("tell", ""));
    std::thread::sleep(std::time::Duration::from_millis(1250));
    tick_n(&host, &bridge, &mut context, 2);
    real_text(&host, "Wil hello");
    assert_eq!(state("text"), "Wil hello", "typed at once, a second after it opened");
    // R: Reply
    named(&host, winit::keyboard::NamedKey::Escape);
    open_line(&host, &bridge, &mut context, vm, "/reply ", "say");
    assert_eq!(state("channel"), "reply");
}

/// Item 7: the game shows hits as numbers over heads and sends no chat line for them, so nothing reached the Combat tab.
/// The tab is made from the `combat.hit` events (the player's, the pet's, and what hits them), in time order with the
/// chat lines that are combat too. Negative controls: before any hit the tab is empty; someone else's hit is not shown;
/// All does not show the combat log.
#[test]
fn a6_chat_combat_tab_shows_the_hits_the_game_tells_the_client_of() {
    let (host, bridge, mut context, _vm) = loaded();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.player = json!({"id": 256, "name": "Me", "level": 20, "pet_id": 9000});
        c.view.entities = vec![
            json!({"ref": {"$ref": "entity", "key": "8193", "generation": 0, "incarnation": 1}, "id": 8193, "name": "Gnoll", "level": 22, "kind": "npc", "is_creature": true,
                   "position": {"x": 1.0, "y": 0.0, "z": 1.0}, "actor_position": {"x": 1.0, "y": 0.0, "z": 1.0}}),
            json!({"ref": {"$ref": "entity", "key": "9000", "generation": 0, "incarnation": 1}, "id": 9000, "name": "Spirit", "level": 20, "kind": "npc", "is_creature": true,
                   "position": {"x": 1.0, "y": 0.0, "z": 1.0}, "actor_position": {"x": 1.0, "y": 0.0, "z": 1.0}}),
        ];
    });
    host.execute("chat_probe.state.w = 640").unwrap();
    tick_n(&host, &bridge, &mut context, 3);
    let press_tab = |context: &mut Context, id: &str| {
        let (x, y): (f64, f64) = host.lua.load(format!("local h = chat_probe.hit('{id}'); return h.x + h.w / 2, h.y + h.h / 2")).eval().unwrap();
        super::pointer_api::position([x as f32, y as f32], true, (1024, 768));
        CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": "left"})));
        tick_n(&host, &bridge, context, 2);
    };
    let lines = |host: &Host| -> Vec<String> { host.lua.load("return chat_probe.lines()").eval().unwrap() };
    press_tab(&mut context, "tab:combat");
    assert!(lines(&host).is_empty(), "control: nothing before a hit");
    // the player hits the gnoll, the gnoll hits back, the pet hits, a stranger hits another stranger, a heal
    native_core::observe_hit_tagged(256, 8193, -12, None, Some((false, false)));
    native_core::observe_text("You have slain a gnoll.", [1.0, 0.55, 0.15, 1.0]);
    native_core::observe_hit(8193, 256, -7, None);
    native_core::observe_hit(9000, 8193, -4, None);
    native_core::observe_hit(5555, 6666, -99, None);
    native_core::observe_hit_tagged(256, 256, 5, None, Some((false, false)));
    native_core::flush_deferred();
    tick_n(&host, &bridge, &mut context, 2);
    assert_eq!(
        lines(&host),
        vec![
            "You hit Gnoll for 12.",
            "You have slain a gnoll.",
            "Gnoll hits you for 7.",
            "Spirit hits Gnoll for 4.",
            "You heal yourself for 5.",
        ],
        "the combat log in time order, with the chat line that is combat; the stranger's hit is not there"
    );
    // All shows the chat lines only
    press_tab(&mut context, "tab:all");
    assert_eq!(lines(&host), vec!["You have slain a gnoll."], "control: All does not carry the combat log");
}

/// 2026-11-25 owner report: the Combat tab wrote a mercenary's hits and heals as "You". A server that tags its numbers
/// tells a pet's or a merc's figure with the player as the attacker, so the line is told by the tag (`by_player`,
/// `from_merc`). Negative control: the run against
/// the old mod (git stash of `bundled-mods/eqoa_chat/main.lua`) fails with the merc's lines written as "You".
#[test]
fn a6_chat_combat_tab_does_not_write_a_mercs_or_pets_hit_as_yours() {
    let (host, bridge, mut context, _vm) = loaded();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.player = json!({"id": 256, "name": "Me", "level": 20, "pet_id": 9000});
        c.view.entities = vec![
            json!({"ref": {"$ref": "entity", "key": "8193", "generation": 0, "incarnation": 1}, "id": 8193, "name": "Gnoll", "level": 22, "kind": "npc", "is_creature": true,
                   "position": {"x": 1.0, "y": 0.0, "z": 1.0}, "actor_position": {"x": 1.0, "y": 0.0, "z": 1.0}}),
            json!({"ref": {"$ref": "entity", "key": "7000", "generation": 0, "incarnation": 1}, "id": 7000, "name": "Quinor", "level": 20, "kind": "npc", "is_creature": true,
                   "position": {"x": 1.0, "y": 0.0, "z": 1.0}, "actor_position": {"x": 1.0, "y": 0.0, "z": 1.0}}),
        ];
    });
    host.execute("chat_probe.state.w = 640").unwrap();
    tick_n(&host, &bridge, &mut context, 3);
    let (x, y): (f64, f64) = host.lua.load("local h = chat_probe.hit('tab:combat'); return h.x + h.w / 2, h.y + h.h / 2").eval().unwrap();
    super::pointer_api::position([x as f32, y as f32], true, (1024, 768));
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": "left"})));
    tick_n(&host, &bridge, &mut context, 2);
    // the player's own hit, the merc's hit and heal (the server's attacker is the player, the tag says merc), the pet's hit
    // by the same road, the merc told by its own id, another member's hit (not shown)
    native_core::observe_hit_tagged(256, 8193, -12, None, Some((false, false)));
    native_core::observe_hit_tagged(256, 8193, -30, None, Some((false, true)));
    native_core::observe_hit_tagged(256, 256, 40, None, Some((false, true)));
    native_core::observe_hit_tagged(256, 8193, -9, None, Some((true, false)));
    native_core::observe_hit_tagged(7000, 8193, -20, None, Some((false, true)));
    native_core::observe_hit_tagged(5555, 8193, -99, None, None);
    native_core::flush_deferred();
    tick_n(&host, &bridge, &mut context, 2);
    let lines: Vec<String> = host.lua.load("return chat_probe.lines()").eval().unwrap();
    assert_eq!(
        lines,
        vec![
            "You hit Gnoll for 12.",
            "Your mercenary hits Gnoll for 30.",
            "Your mercenary heals you for 40.",
            "Your pet hits Gnoll for 9.",
            "Quinor hits Gnoll for 20.",
        ],
        "only the player's own hit is 'You'; a stranger's hit is not there"
    );
}

/// 2026-11-26: a server that sends no tag (or none agreed yet) cannot tell the player's own hit from its pet's or
/// mercenary's, all credited to the player's id. `by_player` is then nil and the Combat tab does not write "You":
/// "Your side hits Gnoll for 12." Negative controls: the known own hit (tag read) is still "You", a stranger's hit is
/// still named, and against the old mod (`by_player ~= false`) the unknown lines read "You hit ..." (the git stash of
/// `bundled-mods/eqoa_chat/main.lua` run fails on the first line).
#[test]
fn a6_chat_combat_tab_does_not_say_you_when_it_cannot_tell() {
    let (host, bridge, mut context, _vm) = loaded();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.player = json!({"id": 256, "name": "Me", "level": 20, "pet_id": 9000});
        c.view.entities = vec![
            json!({"ref": {"$ref": "entity", "key": "8193", "generation": 0, "incarnation": 1}, "id": 8193, "name": "Gnoll", "level": 22, "kind": "npc", "is_creature": true,
                   "position": {"x": 1.0, "y": 0.0, "z": 1.0}, "actor_position": {"x": 1.0, "y": 0.0, "z": 1.0}}),
        ];
    });
    host.execute("chat_probe.state.w = 640").unwrap();
    tick_n(&host, &bridge, &mut context, 3);
    let (x, y): (f64, f64) = host.lua.load("local h = chat_probe.hit('tab:combat'); return h.x + h.w / 2, h.y + h.h / 2").eval().unwrap();
    super::pointer_api::position([x as f32, y as f32], true, (1024, 768));
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": "left"})));
    tick_n(&host, &bridge, &mut context, 2);
    native_core::observe_hit(256, 8193, -12, None); // no tag: unknown
    native_core::observe_hit(256, 256, 5, None); // no tag: unknown heal on the player
    native_core::observe_hit_tagged(256, 8193, -30, None, Some((false, false))); // the tag says the player's own
    native_core::observe_hit(8193, 256, -7, None); // the gnoll's hit: named by its id
    native_core::flush_deferred();
    tick_n(&host, &bridge, &mut context, 2);
    let lines: Vec<String> = host.lua.load("return chat_probe.lines()").eval().unwrap();
    assert_eq!(
        lines,
        vec![
            "Your side hits Gnoll for 12.",
            "Your side heals you for 5.",
            "You hit Gnoll for 30.",
            "Gnoll hits you for 7.",
        ],
        "unknown is never 'You'; the tagged own hit and the others are as before"
    );
}

/// 2026-11-26 owner report (eqoaclassic, a mercenary and no pet of his own): the Combat tab wrote "Your pet hits" for his
/// mercenary's pets, and wrote the heals on those pets as his. A pet-tagged hit is "Your pet" only while the player has
/// a pet; with none out it is "Your side's pet" (a mercenary's pets are tagged the same way). A tag with both the pet and
/// the merc bits reads as the mercenary's; a heal the server sends without a tag is "Your side". Negative control: the
/// old mod writes "Your pet hits Gnoll for 9." for the first line (git stash of `bundled-mods/eqoa_chat/main.lua`).
#[test]
fn a6_chat_combat_tab_says_your_pet_only_while_the_player_has_one() {
    let (host, bridge, mut context, _vm) = loaded();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.player = json!({"id": 256, "name": "Me", "level": 20, "pet_id": null});
        c.view.entities = vec![
            json!({"ref": {"$ref": "entity", "key": "8193", "generation": 0, "incarnation": 1}, "id": 8193, "name": "Gnoll", "level": 22, "kind": "npc", "is_creature": true,
                   "position": {"x": 1.0, "y": 0.0, "z": 1.0}, "actor_position": {"x": 1.0, "y": 0.0, "z": 1.0}}),
            json!({"ref": {"$ref": "entity", "key": "7001", "generation": 0, "incarnation": 1}, "id": 7001, "name": "Wolf", "level": 20, "kind": "npc", "is_creature": true,
                   "position": {"x": 1.0, "y": 0.0, "z": 1.0}, "actor_position": {"x": 1.0, "y": 0.0, "z": 1.0}}),
        ];
    });
    host.execute("chat_probe.state.w = 640").unwrap();
    tick_n(&host, &bridge, &mut context, 3);
    let (x, y): (f64, f64) = host.lua.load("local h = chat_probe.hit('tab:combat'); return h.x + h.w / 2, h.y + h.h / 2").eval().unwrap();
    super::pointer_api::position([x as f32, y as f32], true, (1024, 768));
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": "left"})));
    tick_n(&host, &bridge, &mut context, 2);
    native_core::observe_hit_tagged(256, 8193, -9, None, Some((true, false))); // pet bit, no pet of the player's out
    native_core::observe_hit_tagged(256, 8193, -11, None, Some((true, true))); // pet and merc bits: the mercenary's
    native_core::observe_hit(256, 7001, 20, None); // a heal on the mercenary's pet, sent with no tag
    native_core::flush_deferred();
    tick_n(&host, &bridge, &mut context, 2);
    CORE.with(|c| c.borrow_mut().view.player = json!({"id": 256, "name": "Me", "level": 20, "pet_id": 9000}));
    native_core::observe_hit_tagged(256, 8193, -13, None, Some((true, false))); // the same bit with a pet of the player's out
    native_core::flush_deferred();
    tick_n(&host, &bridge, &mut context, 2);
    let lines: Vec<String> = host.lua.load("return chat_probe.lines()").eval().unwrap();
    assert_eq!(
        lines,
        vec![
            "Your side's pet hits Gnoll for 9.",
            "Your mercenary hits Gnoll for 11.",
            "Your side heals Wolf for 20.",
            "Your pet hits Gnoll for 13.",
        ],
    );
}

#[test]
fn a6_chat_mod_tabs_are_all_plus_one_per_style_or_the_players_own() {
    fresh();
    let source = std::fs::read_to_string(root("eqoa_chat").join("main.lua")).unwrap();
    for needle in ["id = \"all\"", "STYLE_LABELS", "eqoa.chat.tabs()"] {
        assert!(source.contains(needle), "the chat mod has {needle}");
    }
    for gone in ["Notices", "own_tabs", "drag to move"] {
        assert!(!source.contains(gone), "the chat mod no longer has {gone}");
    }
    for kind in ["say", "tells", "group", "guild", "shouts", "combat", "loot", "rolls", "experience", "system"] {
        assert!(source.contains(&format!("{kind} = \"")), "a label for the {kind} style");
    }
}

#[test]
fn a6_a_failing_handler_releases_its_windows_and_is_told_they_closed() {
    fresh();
    let h = host_with("faulty", TAKEOVER, &["start_menu", "chat_entry"]);
    let other = host_with("steady", TAKEOVER, &["raid_ready"]);
    h.execute(
        r#"
        assert(eqoa.windows.takeover('start_menu', {scope = 'all'}))
        assert(eqoa.windows.takeover('chat_entry', {scope = 'all'}))
        log = {}
        eqoa.events.on('windows.session', function(e) log[#log + 1] = e.data.phase .. ':' .. e.data.kind .. ':' .. tostring(e.data.reason) end)
        eqoa.events.on('windows.session', function(e) if e.data.phase == 'opened' then error('boom') end end)
    "#,
    )
    .unwrap();
    other.execute("assert(eqoa.windows.takeover('raid_ready', {scope = 'all'}))").unwrap();
    let vm = native_events::bus(&h.lua).borrow().vm;
    ws::open_client(vm, "faulty", "start_menu", "key", json!({"entries": []}));
    native_core::flush_deferred();
    native_events::dispatch(&h).unwrap();
    assert!(!window_takeover::is_taken(TakeoverKind::StartMenu), "released with the failure");
    assert!(!window_takeover::is_taken(TakeoverKind::ChatEntry), "every kind it held");
    assert!(window_takeover::is_taken(TakeoverKind::RaidReady), "another mod's claim stays");
    native_core::flush_deferred();
    native_events::dispatch(&h).unwrap();
    h.execute("assert(log[2] == 'closed:start_menu:failed', tostring(log[2]))").unwrap();
    assert!(ws::get(vm, "start_menu").is_none());
    // the control: a healthy mod keeps what it holds
    let fine = host_with("fine", TAKEOVER, &["start_menu"]);
    fine.execute("assert(eqoa.windows.takeover('start_menu', {scope = 'all'})); eqoa.events.on('windows.session', function() end)").unwrap();
    let vm2 = native_events::bus(&fine.lua).borrow().vm;
    ws::open_client(vm2, "fine", "start_menu", "key", json!({"entries": []}));
    native_core::flush_deferred();
    native_events::dispatch(&fine).unwrap();
    assert!(window_takeover::is_taken(TakeoverKind::StartMenu));
    assert!(ws::get(vm2, "start_menu").is_some());
}

include!("perf_mods_tests.rs");

include!("hide_under_map_tests.rs");

/// Owner test 2026-11-26: "being able to paste into it would be nice". The player's paste key (Ctrl+V) while the chat line holds
/// the keyboard hands the clipboard's text to the mod as typed text (single line, control characters left out, the mod's own
/// limit), and the mod never asks for the clipboard. Control: a plain V types the letter. Negative control: without the paste
/// handling Ctrl+V types the letter "v" and nothing is pasted.
#[test]
fn a6_chat_line_takes_a_paste_as_typed_text() {
    let (host, bridge, mut context, vm) = loaded();
    tick_n(&host, &bridge, &mut context, 3);
    let state = |code: &str| probe_str(&host, &format!("chat_probe.state.{code}"));
    open_line(&host, &bridge, &mut context, vm, "hi ", "say");
    assert!(super::input::is_text_capture_active());
    let key_v = || winit::keyboard::Key::Character("v".into());
    // control: a plain V is the letter
    real_key(&host, key_v());
    assert_eq!(state("text"), "hi v");
    host.execute("chat_probe.state.text = 'hi '").unwrap();
    // Ctrl+V: two lines, a tab and a bell on the clipboard
    super::input::TEST_CLIPBOARD.with_borrow_mut(|c| *c = Some("hello\r\nwor\u{7}ld\tend".into()));
    super::input::set_modifiers((true, false, false));
    real_key(&host, key_v());
    super::input::set_modifiers((false, false, false));
    assert_eq!(state("text"), "hi hello world end", "the pasted text was typed in, one line, no control characters");
    // a long paste stops where typing does (the line's own limit)
    host.execute("chat_probe.state.text = ''").unwrap();
    super::input::TEST_CLIPBOARD.with_borrow_mut(|c| *c = Some("a".repeat(300)));
    super::input::set_modifiers((true, false, false));
    real_key(&host, key_v());
    super::input::set_modifiers((false, false, false));
    assert_eq!(state("text").len(), 110, "the line takes no more than it takes typed");
    // nothing on the clipboard: nothing typed, and not the letter either
    host.execute("chat_probe.state.text = 'x'").unwrap();
    super::input::set_modifiers((true, false, false));
    real_key(&host, key_v());
    super::input::set_modifiers((false, false, false));
    assert_eq!(state("text"), "x");
    // with no text entry holding the keyboard a paste goes nowhere
    super::input::reset_input_state();
    super::input::TEST_CLIPBOARD.with_borrow_mut(|c| *c = Some("late".into()));
    super::input::set_modifiers((true, false, false));
    real_key(&host, key_v());
    super::input::set_modifiers((false, false, false));
    assert_eq!(state("text"), "x", "the entry is not holding the keyboard: nothing is typed");
    super::input::TEST_CLIPBOARD.with_borrow_mut(|c| *c = None);
}
