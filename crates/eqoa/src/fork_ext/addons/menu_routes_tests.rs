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

/// A Start menu whose first entry is `entry`, opened as the client does it; `entry` is what the game would label it.
fn open_menu(menu: &Mod, context: &mut Context, entry: &str) {
    let data = json!({"entries": [
        {"text": entry, "enabled": true, "command": "0xb0a0000:0"},
        {"text": "Quit", "enabled": true, "command": "0xa010000:0"},
    ]});
    ws::open_client(menu.vm, "eqoa_start_menu", "start_menu", "key", data);
    native_core::flush_deferred();
    frames(&menu.host, &menu.bridge, context, 2);
}

// (the game's entry text, the mod that does its job, the session kind that mod opens)
const ROUTES: [(&str, &str, &str); 9] = [
    ("Summary", "eqoa_character", "trainer"),
    ("Quest Journal", "eqoa_quest_journal", "quest_log"),
    ("Tradeskills", "eqoa_crafting", "recipe_book"),
    ("Spell Book", "eqoa_spellbook", "spellbook"),
    ("Friends", "eqoa_social", "buddy_list"),
    ("Guild", "eqoa_social", "guild"),
    ("Who", "eqoa_social", "buddy_list"),
    ("Group Search", "eqoa_social", "buddy_list"),
    ("Bags", "eqoa_bag_viewer", "inventory"),
];

#[test]
fn start_menu_entries_open_the_eqoa_mod_when_it_is_on_and_the_games_window_when_it_is_off() {
    let mut ctx = context();
    for (entry, id, kind) in ROUTES {
        // the mod is on: the entry asks it to open, and the game's own command is not run
        fixture();
        let menu = run("eqoa_start_menu");
        let target = run(id);
        open_menu(&menu, &mut ctx, entry);
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
        open_menu(&menu, &mut ctx, entry);
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
    open_menu(&menu, &mut ctx, "Guild");
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
    open_menu(&menu, &mut ctx, "Options");
    press(&menu.host, "return", "return");
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["menu_command"], "an unmapped entry runs the game's command");
}
