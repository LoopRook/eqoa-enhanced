// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A5: the bundled Character mod (`bundled-mods/eqoa_character`). It ships off, on Shift+C, says what it
//! provides and what it asks the player first, opens and closes like the other windows, draws every tab inside its
//! window at the default size (long names and many rows included), and each control asks for the right thing:
//! placing points and Train, a Class Mastery session while its tab is up with Buy and Refund, the factions' pages and
//! the combat record. The questions themselves (what the player's Yes sends) are in `fork_ext/a5_tests.rs`.
//!
//! Each test says what its negative control is.
use super::{
    a1_mod_tests::{addon, click, context, fixture, frames, press, run},
    action_broker::{self, ActionKind},
    bridge::Bridge,
    host::Host,
    native_a3, native_a5,
    native_a5::A5Action,
    native_core::CORE,
    native_events, window_sessions as ws,
};
use dear_imgui_rs::Context;
use serde_json::json;

const ID: &str = "eqoa_character";

#[test]
fn a5_character_opens_on_shift_c_draws_and_closes_every_way_it_should() {
    run(ID, "c", "shift+c", "trainer");
}

/// Shift+C is free: no game row, macro or other mod has it (a game key always wins and would leave the row empty).
/// Controls: plain C is the game's (chat history), so a mod default on it gets an empty row.
#[test]
fn a5_character_default_key_is_free_on_the_controls_page() {
    use crate::fork_ext::bindpage;
    bindpage::reset_in_memory();
    let main = std::fs::read_to_string(super::a1_mod_tests::root(ID).join("main.lua")).unwrap();
    assert!(main.contains("default_key = \"shift+c\""), "the mod declares Shift+C");
    for (addon, key) in [("eqoa_stats_box", "y"), ("eqoa_bag_viewer", "shift+b"), ("eqoa_spellbook", "shift+k"), ("eqoa_quest_journal", "shift+j")] {
        bindpage::register(addon, if addon == "eqoa_stats_box" { "toggle" } else { "open" }, key, addon);
    }
    bindpage::register(ID, "open", "shift+c", "Character");
    assert_eq!(bindpage::get_key(ID, "open").as_deref(), Some("shift+c"));
    bindpage::register("eqoa_plain_c", "open", "c", "Plain C");
    assert_eq!(bindpage::get_key("eqoa_plain_c", "open").as_deref(), Some(""), "plain C is the game's chat-history key");
}

fn discovered() -> super::discovery::DiscoveredAddon {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods");
    let found = super::discovery::discover(&root);
    found.addons.into_iter().find(|a| a.id == ID).unwrap_or_else(|| panic!("{ID} is bundled"))
}

/// What Mods & Addons says before the mod is enabled: what it provides, which steps ask first, and what it asks of
/// the server on its own. Control: a mod that declares none of these says none of them.
#[test]
fn a5_mods_and_addons_says_what_the_character_mod_can_do() {
    let lines = super::runtime::disclosure_lines(&discovered());
    assert!(lines.contains(&"Provides: Character window (Shift+C)".to_string()), "{lines:?}");
    assert!(
        lines.contains(&"Asks you first: spend training points, buy a Class Mastery skill, refund a Class Mastery skill".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"Actions: can act on its own (ask for faction standings, ask for the combat record)".to_string()),
        "{lines:?}"
    );
    // the label never says the three steps act on their own
    assert!(!lines.iter().any(|l| l.starts_with("Actions:") && (l.contains("train_stats") || l.contains("mastery_"))), "{lines:?}");
    let other = super::runtime::disclosure_lines(&super::a1_mod_tests::addon("eqoa_spellbook"));
    assert!(!other.iter().any(|l| l.contains("training points") || l.contains("Class Mastery")), "{other:?}");
}

struct Window {
    host: Host,
    bridge: Bridge,
    context: Context,
    vm: u64,
}

impl Window {
    fn frames(&mut self, n: usize) {
        frames(&self.host, &self.bridge, &mut self.context, n);
    }

    fn eval<T: mlua::FromLuaMulti>(&self, code: &str) -> T {
        self.host.lua.load(code).eval().unwrap()
    }

    /// The centre of the control `label` on the tab now showing, in screen pixels.
    fn find(&self, label: &str) -> Option<(f64, f64)> {
        let (x, y): (Option<f64>, Option<f64>) = self.eval(&format!(
            "local S = require('character').state
             for _, list in ipairs({{S.hits, S.row_hits}}) do
                 for _, h in ipairs(list) do if h.label == '{label}' then return h.x + h.w / 2, h.y + h.h / 2 end end
             end"
        ));
        x.zip(y)
    }

    /// Clicks the control `label` and draws again; false when there is no such control (a disabled button has none).
    fn tap(&mut self, label: &str) -> bool {
        let Some((x, y)) = self.find(label) else { return false };
        click(&self.host, x, y, "left");
        self.frames(2);
        true
    }

    fn open(&mut self) {
        press(&self.host, "c", "shift+c");
        self.frames(2);
        assert!(ws::get(self.vm, "trainer").is_some(), "the window is open: {:?}", self.host.drain_logs());
    }
}

fn window() -> Window {
    fixture();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.player = json!({
            "id": 256, "name": "Dabran", "level": 20, "race_name": "Human", "class_name": "Warrior",
            "hp": {"current": 100, "maximum": 120, "base": 100}, "power": {"current": 10, "maximum": 20, "base": 10},
            "attributes": {
                "base": {"strength": 50, "stamina": 50, "agility": 50, "dexterity": 50, "wisdom": 50, "intelligence": 50, "charisma": 50},
                "total": {"strength": 60, "stamina": 55, "agility": 50, "dexterity": 50, "wisdom": 50, "intelligence": 50, "charisma": 50},
            },
        });
        c.view.mastery = Some(json!({"available": 3, "spent": 2, "experience_share": 0.1, "progress_raw": 0, "pending": false, "stats": [], "bonuses": []}));
    });
    native_a3::forget();
    native_a5::forget();
    native_a3::set_training(Some((2, 100, [100; 7])));
    let context = context();
    let host = Host::for_addon(&addon(ID)).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon(ID)).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    Window { host, bridge, context, vm }
}

fn skill(id: i64, name: &str, known: bool, owned: bool, category: i64) -> serde_json::Value {
    json!({"id": id, "parent_key": 0, "rank_key": id, "name": name, "category": category, "cost": 3,
        "known": known, "owned": owned, "master_class": category == 4})
}

fn node(id: i64, owned: bool, known: bool) -> serde_json::Value {
    json!({"id": 0, "skill_id": id, "parent_key": 0, "name": "N", "title": "Title", "category": 1, "cost": 3,
        "text": "It does a thing", "known": known, "owned": owned, "level_required": 10, "points_spent_required": 2, "minimum_stats": []})
}

fn points_asked() -> Vec<[i32; 7]> {
    action_broker::queued_actions_for_test()
        .into_iter()
        .filter_map(|k| match k {
            ActionKind::A5(A5Action::TrainStats { points }) => Some(points),
            _ => None,
        })
        .collect()
}

/// Placing points and Train: the stat stops at the points held and at its cap; Train asks for exactly what was placed
/// and only once something is placed. Controls: no Train control before a point is placed; a stat at its cap takes none.
#[test]
fn a5_character_places_points_and_trains_what_was_placed() {
    let mut w = window();
    w.open();
    assert!(w.find("Train").is_none(), "nothing placed: no Train button");
    assert!(w.tap("+strength") && w.tap("+strength"));
    assert!(w.tap("+stamina"), "the click lands; the window refuses the third point");
    assert_eq!(w.eval::<i64>("require('character').state.add.strength"), 2);
    assert_eq!(w.eval::<i64>("require('character').state.add.stamina"), 0, "only two points were held");
    assert!(w.tap("-strength"));
    assert!(w.tap("+stamina"));
    assert!(w.find("Train").is_some());
    assert!(points_asked().is_empty(), "placing points asks nothing");
    assert!(w.tap("Train"));
    assert_eq!(points_asked(), vec![[1, 1, 0, 0, 0, 0, 0]], "Train asks for exactly what was placed");
    assert_eq!(w.host.drain_logs().unwrap().len(), 0, "no diagnostics");
}

/// The cap: a stat at its cap takes no point. Control: the stat one below its cap takes one.
#[test]
fn a5_character_a_stat_at_its_cap_takes_no_point() {
    let mut w = window();
    native_a3::set_training(Some((5, 100, [51, 100, 100, 100, 100, 100, 100])));
    w.open();
    assert!(w.tap("+strength") && w.tap("+strength"));
    assert_eq!(w.eval::<i64>("require('character').state.add.strength"), 1, "Strength is 50 and its cap 51");
    assert!(w.tap("+stamina") && w.tap("+stamina"));
    assert_eq!(w.eval::<i64>("require('character').state.add.stamina"), 2, "Stamina is 50 and its cap 100");
    assert!(points_asked().is_empty());
}

/// The Masteries tab holds a Class Mastery session while it is up and lets it go on leaving; a click on a skill
/// inspects it; Buy and Refund ask for the right action and only when the skill is in the state that allows it.
/// Controls: the session is gone after leaving the tab; Buy is not offered for an owned skill, Refund not for one that
/// is not owned, and neither before the skill has been read.
#[test]
fn a5_character_masteries_tab_holds_a_session_and_buys_and_refunds() {
    let mut w = window();
    CORE.with(|c| {
        c.borrow_mut().view.mastery_skills =
            Some(vec![skill(1, "Ironhide", true, false, 1), skill(2, "Quickstep", true, true, 1), skill(3, "Locked", false, false, 1)])
    });
    w.open();
    assert!(ws::get(w.vm, "mastery").is_none(), "the session is the tab's, not the window's");
    assert!(w.tap("tab:masteries"));
    assert!(ws::get(w.vm, "mastery").is_some(), "the Masteries tab holds a Class Mastery session");
    assert!(w.find("Buy").is_none() && w.find("Refund").is_none(), "nothing chosen: neither is offered");
    // choose Ironhide: inspected through the game's own request, its node read; Buy is offered, Refund is not
    assert!(w.tap("skill:1"));
    assert!(action_broker::queued_kinds_for_test().contains(&"mastery_browse"));
    assert!(w.find("Buy").is_none(), "the skill has not been read yet");
    CORE.with(|c| c.borrow_mut().view.mastery_node = Some(node(1, false, true)));
    w.frames(2);
    assert!(w.find("Buy").is_some() && w.find("Refund").is_none());
    assert!(w.tap("Buy"));
    assert!(action_broker::queued_actions_for_test().contains(&ActionKind::MasteryBuy { skill_id: 1 }));
    // the owned skill: Refund, never Buy
    assert!(w.tap("skill:2"));
    CORE.with(|c| c.borrow_mut().view.mastery_node = Some(node(2, true, true)));
    w.frames(2);
    assert!(w.find("Buy").is_none() && w.find("Refund").is_some());
    assert!(w.tap("Refund"));
    assert!(action_broker::queued_actions_for_test().contains(&ActionKind::MasteryRefund { skill_id: 2 }));
    // a locked skill cannot be bought
    assert!(w.tap("skill:3"));
    CORE.with(|c| c.borrow_mut().view.mastery_node = Some(node(3, false, false)));
    w.frames(2);
    assert!(w.find("Buy").is_none(), "a skill that is not yet known cannot be bought");
    // leaving the tab lets the session go
    assert!(w.tap("tab:summary"));
    assert!(ws::get(w.vm, "mastery").is_none(), "the session ended with the tab");
    assert!(ws::get(w.vm, "trainer").is_some(), "the window itself is still open");
    assert_eq!(w.host.drain_logs().unwrap().len(), 0);
}

/// Closing the window closes the Class Mastery session with it, and the client ending the window's session closes
/// the mod's window. Control: the session was open just before.
#[test]
fn a5_character_closing_ends_every_session() {
    let mut w = window();
    w.open();
    assert!(w.tap("tab:masteries"));
    assert!(ws::get(w.vm, "mastery").is_some());
    press(&w.host, "escape", "escape");
    assert!(ws::get(w.vm, "mastery").is_none() && ws::get(w.vm, "trainer").is_none(), "both sessions ended");
    // the client ends the window's session (zoning, death): the mod's window closes and its tab's session with it
    ws::set_env(ws::Env::default());
    w.open();
    assert!(w.tap("tab:masteries"));
    ws::set_env(ws::Env { away: true, ..ws::Env::default() });
    w.frames(2);
    assert!(ws::get(w.vm, "mastery").is_none() && ws::get(w.vm, "trainer").is_none());
    assert_eq!(w.eval::<bool>("require('character').is_open()"), false);
    ws::set_env(ws::Env::default());
}

fn page<S: ToString>(n: i32, pages: i32, entries: &[(S, i32)]) -> serde_json::Value {
    native_a5::page_json(n - 1, pages, entries.iter().map(|(a, b)| (a.to_string(), *b)).collect())
}

/// The Factions tab asks for the first page on entering, shows what arrives and pages with its buttons; the Combat tab
/// asks for the record. Controls: no Previous on the first page, no Next on the last; no list before the reply.
#[test]
fn a5_character_factions_and_combat_tabs_ask_and_show() {
    let mut w = window();
    w.open();
    assert!(w.tap("tab:factions"));
    let asked = |name: &str| action_broker::queued_actions_for_test().iter().filter(|k| k.name() == name).count();
    assert_eq!(asked("request_factions"), 1, "entering the tab asks for page 1");
    assert!(w.find("Next page").is_none(), "no page has arrived: nothing to turn");
    native_a5::set_factions(1, || Some(page(1, 3, &[("Qeynos Guard", 1500), ("Dark Elves", -2500)])));
    w.frames(2);
    assert!(w.find("Previous page").is_none(), "the first page has no Previous");
    assert!(w.find("Next page").is_some());
    assert!(w.tap("Next page"));
    assert_eq!(asked("request_factions"), 2);
    assert!(
        action_broker::queued_actions_for_test().contains(&ActionKind::A5(A5Action::RequestFactions { page: 1 })),
        "Next asks for page 2 (the game's number 1)"
    );
    native_a5::set_factions(2, || Some(page(3, 3, &[("Last", 1)])));
    w.frames(2);
    assert!(w.find("Next page").is_none() && w.find("Previous page").is_some(), "the last page has no Next");
    // the combat record
    assert!(w.tap("tab:combat"));
    assert_eq!(asked("request_combat_record"), 1, "entering the tab asks for the record");
    assert!(w.tap("Refresh"));
    assert_eq!(asked("request_combat_record"), 2);
    native_a5::set_record(1, true, || Some(json!({"name": "Dabran", "rows": [
        {"heading": true, "label": "Duels", "value": ""}, {"heading": false, "label": "Won", "value": "5"}]})));
    w.frames(2);
    assert_eq!(w.host.drain_logs().unwrap().len(), 0, "no diagnostic");
}

/// Every text of every tab lies inside the window at the default size, with the longest names and as many rows as the
/// server may send (the lists scroll). Controls: the same data really was too long for the lines (texts were cut); a
/// normal name is not cut; and the check itself fails for a text drawn outside the window.
#[test]
fn a5_character_no_text_exceeds_the_window_on_any_tab() {
    let long = "Very Long Faction Name That Keeps Going And Going Past Any Sensible Column Width Indeed";
    let mut w = window();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.player["name"] = json!(long);
        c.view.mastery_skills = Some((1..=40).map(|n| skill(n, &format!("{long} {n}"), true, n % 3 == 0, 1 + n % 4)).collect());
        c.view.mastery_node = Some({
            let mut n = node(1, false, true);
            n["title"] = json!(long);
            n["text"] = json!(long.repeat(4));
            n
        });
    });
    native_a3::set_training(Some((99999, 100000, [100000; 7])));
    native_a5::set_factions(1, || {
        Some(page(2, 12, &(1..=60).map(|n| (format!("{long} {n}"), if n % 2 == 0 { 2147483647 } else { -2147483648 })).collect::<Vec<_>>()))
    });
    native_a5::set_record(1, true, || {
        Some(json!({"name": long, "rows": (1..=60).map(|n| json!({"heading": n % 7 == 0, "label": format!("{long} {n}"), "value": long})).collect::<Vec<_>>()}))
    });
    w.open();
    w.eval::<()>("require('character').record = true");
    let check = |w: &Window| -> (bool, i64, i64) {
        let (inside, drawn, cuts): (bool, i64, i64) = w.eval(
            "local M = require('character')
             local r, ok = M.rect, true
             for _, d in ipairs(M.drawn) do
                 if d.x < r.x - 0.5 or d.y < r.y - 0.5 or d.x + d.w > r.x + r.w + 0.5 or d.y + d.h > r.y + r.h + 0.5 then ok = false end
             end
             return ok, #M.drawn, M.cuts",
        );
        (inside, drawn, cuts)
    };
    let mut cut_total = 0;
    for tab in ["summary", "masteries", "factions", "combat"] {
        w.eval::<()>(&format!("require('character').state.tab = '{tab}'; require('character').state.sel = 1"));
        w.frames(2);
        let (inside, drawn, cuts) = check(&w);
        assert!(drawn > 5, "{tab}: something was drawn ({drawn})");
        assert!(inside, "{tab}: a text lies outside the window");
        cut_total += cuts;
    }
    assert!(cut_total > 0, "control: the long names really were cut to fit");
    // control: the check fails for a text outside the window
    w.eval::<()>(
        "local M = require('character'); M.drawn[#M.drawn + 1] = { x = M.rect.x + M.rect.w, y = M.rect.y, w = 40, h = 10 }",
    );
    assert!(!check(&w).0, "control: a text past the window's edge is caught");
    // control: ordinary names are not cut at all
    let mut w = window();
    CORE.with(|c| c.borrow_mut().view.mastery_skills = Some(vec![skill(1, "Ironhide", true, false, 1)]));
    native_a5::set_factions(1, || Some(page(1, 1, &[("Qeynos Guard", 1500)])));
    w.open();
    w.eval::<()>("require('character').record = true");
    for tab in ["summary", "masteries", "factions"] {
        w.eval::<()>(&format!("require('character').state.tab = '{tab}'"));
        w.frames(2);
        let (inside, _, cuts) = check(&w);
        assert!(inside && cuts == 0, "{tab}: ordinary text fits whole (cut {cuts} times)");
    }
}
