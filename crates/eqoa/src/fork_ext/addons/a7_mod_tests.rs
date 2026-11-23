// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A7: the bundled `eqoa_social` mod (Friends, Ignore, Guild, Who, Group Search + incoming Friend Request
//! and Guild Invitation prompts). Verifies shipping off, exact capability declarations, Mods & Addons disclosures,
//! Shift+F / Shift+G keys on Controls page, session lifecycle, mod-off control, and a headless "no text exceeds
//! the window" check across every tab and prompt box at default size.
use super::{
    action_broker,
    bridge::Bridge,
    discovery::{DiscoveredAddon, EntryMode},
    host::Host,
    native_a7::{
        self, FriendEntry, GuildMemberSnap, GuildSnap, IgnoredEntry, LfgEntrySnap, LfgSnap, WhoEntrySnap, WhoSnap,
    },
    native_core::{CORE, Core},
    native_events, runtime, window_sessions as ws,
    window_takeover::{self, TakeoverKind},
};
use dear_imgui_rs::{Context, sys};
use serde_json::json;
use std::path::PathBuf;

const ID: &str = "eqoa_social";

fn root(id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bundled-mods")
        .join(id)
        .canonicalize()
        .unwrap()
}

fn manifest(id: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root(id).join("mod.json")).unwrap()).unwrap()
}

fn addon(id: &str) -> DiscoveredAddon {
    let m = manifest(id);
    let mut ext = super::discovery::Extensions::default();
    if let Some(arr) = m["provides"].as_array() {
        ext.provides = arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
    }
    if let Some(arr) = m["replaces"].as_array() {
        ext.replaces = arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
    }
    DiscoveredAddon {
        id: id.into(),
        name: m["name"].as_str().unwrap_or(id).into(),
        entry: root(id).join("main.lua"),
        script_root: root(id),
        package_root: root(id),
        module_dirs: vec![root(id)],
        resource_dirs: vec![root(id)],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: m["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap().to_string())
            .collect(),
        extensions: ext,
        initially_enabled: m["enabled"].as_bool().unwrap_or(false),
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

fn fixture() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    native_a7::forget();
    crate::fork_ext::bindpage::reset_in_memory();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = json!({"id": 100, "name": "Hero", "level": 35});
    });
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

fn press(host: &Host, key: &str, combo: &str) {
    CORE.with(|c| {
        c.borrow_mut().emit(
            "input.key",
            json!({"key": key, "combo": combo, "down": true, "repeat": false, "synthetic": false, "text": ""}),
        )
    });
    native_events::dispatch(host).unwrap();
}

fn used_names(src: &str, prefix: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find(prefix) {
        rest = &rest[i + prefix.len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() && !found.contains(&name) {
            found.push(name);
        }
    }
    found
}

#[test]
fn a7_social_manifest_capabilities_and_disclosures_are_exact_and_ships_off() {
    fixture();
    let m = manifest(ID);
    assert_eq!(m["enabled"], false, "eqoa_social must ship disabled");
    assert!(crate::fork_ext::bundled::PACKAGES.contains(&ID));
    for file in ["mod.json", "main.lua", "README.md"] {
        assert!(
            crate::fork_ext::bundled::EMBEDDED_FILES
                .iter()
                .any(|f| f.path == format!("{ID}/{file}")),
            "{file} must be embedded"
        );
    }

    // Every action / who / lfg capability used in main.lua is declared, and every declared action is used.
    let src = std::fs::read_to_string(root(ID).join("main.lua")).unwrap();
    let declared: Vec<String> = m["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    for name in used_names(&src, "eqoa.actions.") {
        assert!(
            declared.contains(&format!("eqoa.actions.{name}")),
            "main.lua calls eqoa.actions.{name} without declaring it"
        );
    }
    for cap in declared.iter().filter(|c| c.starts_with("eqoa.actions.")) {
        let name = cap.trim_start_matches("eqoa.actions.");
        assert!(
            used_names(&src, "eqoa.actions.").iter().any(|u| u == name),
            "mod.json declares {cap} without calling it"
        );
    }
    for cap in ["eqoa.who.request", "eqoa.lfg.search", "eqoa.lfg.register", "eqoa.lfg.unregister"] {
        assert!(src.contains(cap) && declared.contains(&cap.to_string()), "{cap} must be both used and declared");
    }

    // Mods & Addons disclosure lines show Provides, Replaces, autonomous actions, and Asks-you-first items.
    let a = addon(ID);
    let lines = runtime::disclosure_lines(&a);
    let joined = lines.join("\n");
    assert!(
        joined.contains("Provides: Friends (Shift+F), Guild (Shift+G)"),
        "provides disclosure missing: {joined}"
    );
    assert!(
        joined.contains("Replaces game windows: Friend Request, Guild Invite"),
        "replaces disclosure missing: {joined}"
    );
    assert!(
        joined.contains("Asks you first: kick a guild member, accept a friend request, accept a guild invitation"),
        "asks-you-first disclosure missing: {joined}"
    );
    // Negative control: confirmed actions must NOT appear in "can act on its own"
    let auto_line = lines.iter().find(|l| l.starts_with("Actions: can act on its own")).unwrap();
    for asked in ["guild_kick", "friend_accept", "guild_accept"] {
        assert!(!auto_line.contains(asked), "{asked} must not be listed as autonomous: {auto_line}");
    }
}

#[test]
fn a7_social_shift_f_and_shift_g_lifecycle_and_mod_off_control() {
    fixture();
    // Mod-off control: before eqoa_social loads, no prompt takeover is held
    assert!(!window_takeover::is_taken(TakeoverKind::FriendRequest));
    assert!(!window_takeover::is_taken(TakeoverKind::GuildInvite));

    let mut context = context();
    let a = addon(ID);
    let host = Host::for_addon(&a).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&a).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;

    // Default keys land on the Controls page (not colliding with game keys)
    assert_eq!(crate::fork_ext::bindpage::get_key(ID, "friends").as_deref(), Some("shift+f"));
    assert_eq!(crate::fork_ext::bindpage::get_key(ID, "guild").as_deref(), Some("shift+g"));

    // Closed control: draws nothing, holds no window session, claims request takeovers once in world
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0);
    assert!(ws::get(vm, "buddy_list").is_none());
    assert!(ws::get(vm, "guild").is_none());
    assert!(window_takeover::is_taken(TakeoverKind::FriendRequest));
    assert!(window_takeover::is_taken(TakeoverKind::GuildInvite));

    // Shift+F opens Friends tab and draws without diagnostics
    press(&host, "f", "shift+f");
    let s = ws::get(vm, "buddy_list").expect("Shift+F opened buddy_list session");
    assert_eq!((s.origin, s.opened_from.as_str()), ("mod", "key"));
    let vtx = frames(&host, &bridge, &mut context, 3);
    let logs = host.drain_logs().unwrap();
    assert!(vtx > 0, "draws Social window");
    assert!(logs.is_empty(), "no diagnostics: {logs:?}");
    assert!(bridge.drain_diagnostics().is_empty(), "no ImGui recovery diagnostics");

    // Shift+F again closes it
    press(&host, "f", "shift+f");
    assert!(ws::get(vm, "buddy_list").is_none());
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0);

    // Shift+G opens Guild tab
    press(&host, "g", "shift+g");
    assert!(ws::get(vm, "guild").is_some(), "Shift+G opened guild session");
    assert!(frames(&host, &bridge, &mut context, 2) > 0);
    // Escape closes it
    press(&host, "escape", "escape");
    assert!(ws::get(vm, "guild").is_none(), "Escape closes Social");

    // Client session end (death / zoning) closes it cleanly
    press(&host, "f", "shift+f");
    assert!(ws::get(vm, "buddy_list").is_some());
    ws::set_env(ws::Env { dead: true, ..ws::Env::default() });
    frames(&host, &bridge, &mut context, 2);
    assert!(ws::get(vm, "buddy_list").is_none());
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0);
    ws::set_env(ws::Env::default());

    // "Use the game's windows" on: Shift+F logs a leveled note, NOT a mod error
    host.drain_logs().unwrap();
    window_takeover::set_use_game_windows(true);
    press(&host, "f", "shift+f");
    let logs = format!("{:?}", host.drain_logs().unwrap());
    window_takeover::set_use_game_windows(false);
    assert!(!logs.contains("invalid_argument"), "no mod error: {logs}");
    assert!(logs.contains("did not open"), "says why it did not open: {logs}");

    // Disabling releases sessions and prompt takeovers
    press(&host, "f", "shift+f");
    native_events::clear(&host);
    window_takeover::release_vm(vm);
    assert!(ws::get(vm, "buddy_list").is_none());
    assert!(!window_takeover::is_taken(TakeoverKind::FriendRequest));
    assert!(!window_takeover::is_taken(TakeoverKind::GuildInvite));
}

fn click_imgui(host: &Host, bridge: &Bridge, context: &mut Context, x: f32, y: f32) {
    context.io_mut().add_mouse_pos_event([x, y]);
    frames(host, bridge, context, 1);
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
    frames(host, bridge, context, 1);
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
    frames(host, bridge, context, 2);
}

fn tab_button_centers(win_name: &str, labels: &[&str]) -> Vec<(f32, f32)> {
    let cname = std::ffi::CString::new(win_name).unwrap();
    unsafe {
        let w = sys::igFindWindowByName(cname.as_ptr());
        assert!(!w.is_null(), "window {win_name:?} must exist");
        let style = &*sys::igGetStyle();
        let mut x = (*w).Pos.x + style.WindowPadding.x;
        let title_h = sys::igGetFontSize() + style.FramePadding.y * 2.0;
        let frame_h = sys::igGetFrameHeight();
        let y = (*w).Pos.y + title_h + style.WindowPadding.y + frame_h * 0.5;
        let mut out = Vec::new();
        for &lbl in labels {
            let cs = std::ffi::CString::new(lbl).unwrap();
            let sz = sys::igCalcTextSize(cs.as_ptr(), std::ptr::null(), true, -1.0);
            let bw = sz.x + style.FramePadding.x * 2.0;
            out.push((x + bw * 0.5, y));
            x += bw + style.ItemSpacing.x;
        }
        out
    }
}

fn child_was_drawn(parent: &str, child_id: &str) -> bool {
    let full = format!("{parent}/{child_id}_");
    // Dear ImGui names child windows "<parent>/<child>_<hash>"
    unsafe {
        let ctx = &*sys::igGetCurrentContext();
        let windows = &ctx.Windows;
        for i in 0..windows.Size {
            let w = *windows.Data.add(i as usize);
            if !w.is_null() && (*w).WasActive {
                let name = std::ffi::CStr::from_ptr((*w).Name).to_string_lossy();
                if name.starts_with(&full) {
                    return true;
                }
            }
        }
    }
    false
}
fn assert_window_fits_horizontally(name: &str, label: &str) {
    let cname = std::ffi::CString::new(name).unwrap();
    let w = unsafe { sys::igFindWindowByName(cname.as_ptr()) };
    assert!(!w.is_null(), "{label}: window {name:?} was not drawn");
    let (size_x, content_x, inner_w) = unsafe {
        let win = &*w;
        let inner_w = win.InnerRect.Max.x - win.InnerRect.Min.x;
        (win.Size.x, win.ContentSize.x, inner_w)
    };
    assert!(
        content_x <= inner_w + 2.0,
        "{label}: text/content width ({content_x:.1}px) exceeds window inner width ({inner_w:.1}px, window size {size_x:.1}px) in {name:?}"
    );
}

/// Owner test 2026-11-23: a group search that came back empty looked the same as one never made ("No group search
/// results yet"), and "Searching..." was hidden once registration was known. The tab now hears the answer (even an
/// empty one) and says "Nobody found". Negative control: before any answer the note is the old "No group search results
/// yet"; the same events with players give a count, not "nobody".
#[test]
fn a7_social_group_search_says_nobody_found_for_an_empty_answer() {
    fixture();
    let mut context = context();
    let a = addon(ID);
    let host = Host::for_addon(&a).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&a).unwrap();
    let lfg = |entries: Vec<LfgEntrySnap>, busy: bool, hash: u64, result: bool| {
        native_a7::sync_lfg(
            hash,
            move || LfgSnap {
                available: true,
                busy,
                registered: Some(false),
                invites_open: None,
                members_invite: None,
                default_min_level: 30,
                default_max_level: 40,
                entries,
            },
            result,
        );
    };
    lfg(Vec::new(), false, 1, false);
    frames(&host, &bridge, &mut context, 2);
    let probe = || -> (Option<i64>, Option<String>) { host.lua.load("return social_probe()").eval().unwrap() };
    assert_eq!(probe().0, None, "control: nothing heard before a search");
    host.execute("assert(social_lfg_empty_note(false, nil, nil) == 'No group search results yet. Click Search LFG.')").unwrap();
    // a search is out: the note says so (also when registration is known)
    host.execute("assert(social_lfg_empty_note(true, nil, nil) == 'Searching...')").unwrap();
    // the empty answer arrives
    lfg(Vec::new(), false, 2, true);
    frames(&host, &bridge, &mut context, 2);
    assert_eq!(probe().0, Some(0), "an empty answer is heard");
    host.execute(
        "local n = social_lfg_empty_note(false, 0, nil); assert(n:find('Nobody found', 1, true), n)
         assert(social_lfg_empty_note(false, 0, 'players'):find('No player', 1, true))
         assert(social_lfg_empty_note(false, 0, 'groups'):find('No group', 1, true))",
    )
    .unwrap();
    // control: an answer with someone in it is a count, never "nobody"
    let one = vec![LfgEntrySnap {
        id: 9,
        name: "Wil".into(),
        race_id: 0,
        race_name: Some("Human".into()),
        class_id: 0,
        class_name: Some("Warrior".into()),
        level: 30,
        world_id: 1,
        position: [0.0; 3],
    }];
    lfg(one, false, 3, true);
    frames(&host, &bridge, &mut context, 2);
    assert_eq!(probe().0, Some(1));
}

#[test]
fn a7_social_no_text_exceeds_window_at_default_size_across_all_tabs_and_prompts() {
    fixture();
    // Populate realistic long data for every tab + both incoming request boxes
    native_a7::sync_friends(1, || {
        vec![
            FriendEntry {
                id: 1,
                name: "MaximilianTheLongName".into(),
                online: true,
                class_id: 1,
                class_name: Some("Shadowknight".into()),
                race_id: 2,
                race_name: Some("Dark Elf".into()),
                level: 60,
                world: "Castle Lightwolf".into(),
            },
            FriendEntry {
                id: 2,
                name: "Bram".into(),
                online: false,
                class_id: 0,
                class_name: Some("Warrior".into()),
                race_id: 0,
                race_name: Some("Human".into()),
                level: 12,
                world: String::new(),
            },
        ]
    });
    native_a7::sync_ignored(1, || {
        vec![IgnoredEntry { id: 9, name: "VeryLongIgnoredCharacterName".into() }]
    });
    native_a7::sync_guild(1, || GuildSnap {
        in_guild: true,
        name: "Order of the Eternal Silver Crescent".into(),
        motd: "Welcome adventurers! Guild raid starts at 8:00 PM EST outside Castle Lightwolf, bring potions.".into(),
        show_chat: true,
        self_rank: Some(3),
        self_rank_name: Some("Leader".into()),
        can_invite: true,
        can_set_motd: true,
        members: vec![
            GuildMemberSnap {
                id: 100,
                name: "Hero".into(),
                class_id: 1,
                class_name: Some("Paladin".into()),
                race_id: 0,
                race_name: Some("Human".into()),
                level: 60,
                rank: 3,
                rank_name: "Leader".into(),
                online: true,
                can_promote: false,
                can_demote: false,
                can_kick: false,
            },
            GuildMemberSnap {
                id: 101,
                name: "SeraphinaLongName".into(),
                class_id: 2,
                class_name: Some("Necromancer".into()),
                race_id: 2,
                race_name: Some("Dark Elf".into()),
                level: 58,
                rank: 1,
                rank_name: "Senior".into(),
                online: true,
                can_promote: true,
                can_demote: true,
                can_kick: true,
            },
        ],
    });
    native_a7::set_who(
        WhoSnap {
            pending: false,
            received: 1,
            entries: vec![WhoEntrySnap {
                id: 301,
                name: "AlaricStormbringer".into(),
                race_id: 3,
                race_name: Some("Barbarian".into()),
                class_id: 1,
                class_name: Some("Shadowknight".into()),
                level: 55,
            }],
        },
        false,
    );
    native_a7::sync_lfg(
        1,
        || LfgSnap {
            available: true,
            busy: false,
            registered: Some(true),
            invites_open: Some(true),
            members_invite: Some(false),
            default_min_level: 30,
            default_max_level: 40,
            entries: vec![LfgEntrySnap {
                id: 401,
                name: "ValeriaDawnbreaker".into(),
                race_id: 1,
                race_name: Some("High Elf".into()),
                class_id: 4,
                class_name: Some("Enchanter".into()),
                level: 37,
                world_id: 1,
                position: [0.0, 0.0, 0.0],
            }],
        },
        false,
    );

    let mut context = context();
    let a = addon(ID);
    let host = Host::for_addon(&a).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&a).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;

    // Claim prompt takeovers and simulate active held friend_request + guild_invite sessions
    frames(&host, &bridge, &mut context, 2);
    native_a7::on_incoming_friend_request(701, "LongRequestPlayerName".into());
    native_a7::on_incoming_guild_invite(702, "LongGuildInviterName".into(), "Keepers of the Endless Horizon".into());
    ws::sync_held_request(TakeoverKind::FriendRequest, Some((vm, ID.into())), "ended");
    ws::sync_held_request(TakeoverKind::GuildInvite, Some((vm, ID.into())), "ended");

    // Open Social on Friends tab and check Friends + both prompt boxes
    press(&host, "f", "shift+f");
    frames(&host, &bridge, &mut context, 3);
    assert!(child_was_drawn("Social##eqoa_social", "##friends_list"), "Friends tab active");
    assert_window_fits_horizontally("Social##eqoa_social", "Friends tab");
    assert_window_fits_horizontally("Friend Request##eqoa_social_fr", "Friend Request prompt");
    assert_window_fits_horizontally("Guild Invitation##eqoa_social_gi", "Guild Invitation prompt");

    // Click Ignore tab and verify
    let centers = tab_button_centers(
        "Social##eqoa_social",
        &["[Friends]", "Ignore", "Guild", "Who", "Group Search"],
    );
    click_imgui(&host, &bridge, &mut context, centers[1].0, centers[1].1);
    assert!(child_was_drawn("Social##eqoa_social", "##ignore_list"), "Ignore tab active");
    assert_window_fits_horizontally("Social##eqoa_social", "Ignore tab");

    // Switch to Guild via Shift+G and verify
    press(&host, "g", "shift+g");
    frames(&host, &bridge, &mut context, 3);
    assert!(child_was_drawn("Social##eqoa_social", "##guild_roster"), "Guild tab active");
    assert_window_fits_horizontally("Social##eqoa_social", "Guild tab");

    // Click Who tab and verify
    let centers = tab_button_centers(
        "Social##eqoa_social",
        &["Friends", "Ignore", "[Guild]", "Who", "Group Search"],
    );
    click_imgui(&host, &bridge, &mut context, centers[3].0, centers[3].1);
    assert!(child_was_drawn("Social##eqoa_social", "##who_list"), "Who tab active");
    assert_window_fits_horizontally("Social##eqoa_social", "Who tab");

    // Click Group Search tab and verify
    let centers = tab_button_centers(
        "Social##eqoa_social",
        &["Friends", "Ignore", "Guild", "[Who]", "Group Search"],
    );
    click_imgui(&host, &bridge, &mut context, centers[4].0, centers[4].1);
    assert!(child_was_drawn("Social##eqoa_social", "##lfg_list"), "Group Search tab active");
    assert_window_fits_horizontally("Social##eqoa_social", "Group Search tab");

    // Verify all Lua logs & ImGui bridge diagnostics remain empty
    let logs = host.drain_logs().unwrap();
    assert!(logs.is_empty(), "unexpected logs during layout render: {logs:?}");
    let diag = bridge.drain_diagnostics();
    assert!(diag.is_empty(), "unexpected ImGui recovery diagnostics: {diag:?}");
}
