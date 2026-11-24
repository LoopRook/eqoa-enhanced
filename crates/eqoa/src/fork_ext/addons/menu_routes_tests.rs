// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Owner test 2026-11-23: the Start menu's entries open the EQOA mods that do their job. With the target mod on,
//! choosing the entry sends it the command "open" (`eqoa.mods.send`) and the menu closes, so the mod's window opens
//! and the game's does not; with the mod off the entry runs the game's own command, as before (the control).
use super::{
    a1_mod_tests::{context, fixture, frames, press, root},
    action_broker,
    bridge::Bridge,
    discovery::{self, DiscoveredAddon},
    host::Host,
    native_core, native_events, window_sessions as ws,
};
use dear_imgui_rs::Context;
use serde_json::json;

fn found(id: &str) -> DiscoveredAddon {
    discovery::discover(root("eqoa_start_menu").parent().unwrap()).addons.into_iter().find(|a| a.id == id).unwrap_or_else(|| panic!("{id} is bundled"))
}

struct Mod {
    host: Host,
    bridge: Bridge,
    vm: u64,
}

fn run(id: &str) -> Mod {
    let host = Host::for_addon(&found(id)).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&found(id)).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    Mod { host, bridge, vm }
}

/// A Start menu whose first entry is `entry` running `command`, opened as the client does it; `entry` is what the game
/// labels it on the disc's tables (the Lua routes by the command, so each row names the game's own).
fn open_menu(menu: &Mod, context: &mut Context, entry: &str, command: &str) {
    let data = json!({"entries": [
        {"text": entry, "enabled": true, "command": command},
        {"text": "Quit", "enabled": true, "command": "0xa010000:0"},
    ]});
    ws::open_client(menu.vm, "eqoa_start_menu", "start_menu", "key", data);
    native_core::flush_deferred();
    frames(&menu.host, &menu.bridge, context, 2);
}

// (the game's entry text on the disc's tables, its command, the mod that does its job, the session kind that mod opens)
const ROUTES: [(&str, &str, &str, &str); 17] = [
    ("Summary", "0x3020000:0", "eqoa_character", "trainer"),
    ("Class Mastery", "0x2080000:0", "eqoa_character", "trainer"),
    ("Quest", "0xb0a0000:0", "eqoa_quest_journal", "quest_log"),
    ("Weapons", "0xa7010000:0", "eqoa_crafting", "recipe_book"),
    ("View List", "0xb050000:0", "eqoa_social", "buddy_list"),
    ("Guild List", "0xb051001:0", "eqoa_social", "guild"),
    ("Who", "0xb030004:0", "eqoa_social", "buddy_list"),
    ("Group Search", "0x2090000:0", "eqoa_social", "buddy_list"),
    ("Inventory", "0x2000000:0", "eqoa_bag_viewer", "inventory"),
    ("Ability", "0x8010000:0", "eqoa_spellbook", "spellbook"),
    ("Factions", "0xaa000001:0", "eqoa_character", "trainer"),
    ("Combat Record", "0x7e20001:0", "eqoa_character", "trainer"),
    ("Inbox", "0xb0b0002:0", "eqoa_mail", "mail_inbox"),
    ("Compose", "0xb0b0000:0", "eqoa_mail", "mail_compose"),
    ("Search", "0xa8010000:0", "eqoa_auction", "auction_search"),
    ("My lots", "0xa8020000:0", "eqoa_auction", "auction_search"),
    ("My bids", "0xa8030000:0", "eqoa_auction", "auction_search"),
];

#[test]
fn start_menu_entries_open_the_eqoa_mod_when_it_is_on_and_the_games_window_when_it_is_off() {
    let mut ctx = context();
    for (entry, command, id, kind) in ROUTES {
        // the mod is on: the entry asks it to open, and the game's own command is not run
        fixture();
        let menu = run("eqoa_start_menu");
        let target = run(id);
        open_menu(&menu, &mut ctx, entry, command);
        press(&menu.host, "return", "return");
        assert!(action_broker::queued_kinds_for_test().is_empty(), "{entry}: the game's entry is not run; logs {:?}", menu.host.drain_logs());
        assert!(ws::get(menu.vm, "start_menu").is_none(), "{entry}: the menu closes");
        frames(&target.host, &target.bridge, &mut ctx, 2);
        assert!(ws::get(target.vm, kind).is_some(), "{entry}: {id} opened a {kind} window; logs {:?}", target.host.drain_logs());
        drop(target);
        drop(menu);
        // the control: the mod is off, the same entry runs the game's own command
        fixture();
        let menu = run("eqoa_start_menu");
        open_menu(&menu, &mut ctx, entry, command);
        press(&menu.host, "return", "return");
        assert_eq!(action_broker::queued_kinds_for_test(), vec!["menu_command"], "{entry}: with {id} off the game's entry runs");
    }
}

#[test]
fn start_menu_social_entries_open_the_page_they_name_and_other_entries_stay_the_games() {
    let mut ctx = context();
    fixture();
    let menu = run("eqoa_start_menu");
    let target = run("eqoa_social");
    open_menu(&menu, &mut ctx, "Guild List", "0xb051001:0");
    press(&menu.host, "return", "return");
    frames(&target.host, &target.bridge, &mut ctx, 2);
    assert!(ws::get(target.vm, "guild").is_some(), "Guild opens the guild page");
    assert!(ws::get(target.vm, "buddy_list").is_none());
    drop(target);
    drop(menu);
    // an entry no mod does the job of is the game's, even with every mod on
    fixture();
    let menu = run("eqoa_start_menu");
    let _social = run("eqoa_social");
    open_menu(&menu, &mut ctx, "Options", "0xb0d0002:0");
    press(&menu.host, "return", "return");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["menu_command"], "an unmapped entry runs the game's command");
}

/// Owner test 2026-11-24, the disc's own labels: the chat mode called Guild and the Guild sub-menu are not the guild page, and a sub-menu is browsed, never
/// taken. The old Lua matched the label word alone, so each of these went to a mod, or never opened.
#[test]
fn start_menu_routes_by_the_games_command_not_by_a_word_that_other_entries_share() {
    let mut ctx = context();
    for (entry, command) in [("Guild", "0xb030002:0"), ("Group", "0xb030000:0"), ("Say", "0xb010000:0"), ("Options", "0xb0d0002:0"), ("Experiment", "0xa7020000:0")] {
        fixture();
        let menu = run("eqoa_start_menu");
        let _mods = ["eqoa_spellbook", "eqoa_social", "eqoa_crafting", "eqoa_character"].map(run);
        open_menu(&menu, &mut ctx, entry, command);
        press(&menu.host, "return", "return");
        assert_eq!(action_broker::queued_kinds_for_test(), vec!["menu_command"], "{entry}: the game's own entry runs");
        assert!(ws::sessions_for_test().iter().all(|s| s.vm == menu.vm), "{entry}: no mod window opened");
    }
    // a sub-menu called Guild opens as a sub-menu: Enter goes into it
    fixture();
    let menu = run("eqoa_start_menu");
    let social = run("eqoa_social");
    ws::open_client(
        menu.vm,
        "eqoa_start_menu",
        "start_menu",
        "key",
        json!({"entries": [{"text": "Guild", "enabled": true, "entries": [{"text": "Guild List", "enabled": true, "command": "0xb051001:0"}]}]}),
    );
    native_core::flush_deferred();
    frames(&menu.host, &menu.bridge, &mut ctx, 2);
    press(&menu.host, "return", "return");
    assert!(ws::get(menu.vm, "start_menu").is_some(), "the sub-menu opened, the menu is still up");
    assert!(ws::get(social.vm, "guild").is_none(), "and the guild page did not");
}
