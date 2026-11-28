// LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE).
//! Owner's test 2026-11-28: a long quest line in the Quest Journal ran off the window's right edge ("Sign Coachman Ronks's
//! ledger at the coach on the coa" and the rest cut). Now a quest's text wraps inside the list onto as many lines as it needs.
//! The wrapping rule is `bundled-mods/eqoa_quest_journal/modules/wrap.lua` (pure Lua, tested alone); the window test clicks the
//! SECOND line of a wrapped quest and shows that this quest, not the one under it, is selected.
use super::{
    a1_mod_tests::{addon, click, context, fixture, frames, press},
    bridge::Bridge,
    host::Host,
};
use serde_json::json;

const LONG: &str = "Sign Coachman Ronks's ledger at the coach on the coast road near the old watchtower and bring it back to Captain Aldric";

fn wrap_module() -> mlua::Lua {
    let lua = mlua::Lua::new();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods/eqoa_quest_journal/modules/wrap.lua");
    let src = std::fs::read_to_string(path).unwrap();
    let m: mlua::Table = lua.load(&src).eval().unwrap();
    lua.globals().set("W", m).unwrap();
    lua
}

/// Seven pixels a letter, like a fixed font.
const WIDTH: &str = "function(t) return #t * 7 end";

#[test]
fn wrap_puts_a_long_line_on_several_all_inside_the_width_and_loses_no_word() {
    let lua = wrap_module();
    let lines: Vec<String> = lua
        .load(&format!("local out = W.wrap([[{LONG}]], 300, {WIDTH}) return out"))
        .eval::<Vec<String>>()
        .unwrap();
    assert!(lines.len() >= 2, "a long line goes onto several: {lines:?}");
    for l in &lines {
        assert!(l.len() * 7 <= 300, "inside the width: {l:?}");
    }
    assert_eq!(lines.join(" "), LONG, "every word is kept, in order");
    // a word wider than the line is cut, never dropped
    let cut: Vec<String> = lua.load(&format!("return W.wrap('Supercalifragilisticexpialidocious', 100, {WIDTH})")).eval().unwrap();
    assert!(cut.len() >= 2 && cut.iter().all(|l| l.len() * 7 <= 100), "{cut:?}");
    assert_eq!(cut.concat(), "Supercalifragilisticexpialidocious");
    // controls: a short line stays one, an empty one is one empty line
    let short: Vec<String> = lua.load(&format!("return W.wrap('Bat Eyes', 300, {WIDTH})")).eval().unwrap();
    assert_eq!(short, vec!["Bat Eyes".to_string()]);
    let empty: Vec<String> = lua.load(&format!("return W.wrap('', 300, {WIDTH})")).eval().unwrap();
    assert_eq!(empty, vec![String::new()]);
}

struct Journal {
    host: Host,
    bridge: Bridge,
    context: dear_imgui_rs::Context,
}

impl Journal {
    fn frames(&mut self, n: usize) {
        frames(&self.host, &self.bridge, &mut self.context, n);
    }

    /// The window's place, as the mod works it out (HUD scale and the window's corner, in screen pixels).
    fn place(&self) -> (f64, f64, f64) {
        self.host
            .lua
            .load(
                "local f = eqoa.hud.frame() local k = math.min(f.scale_x, f.scale_y)
                 local dw, dh = 1024, 768
                 local H = 278
                 local x0 = math.max(0, math.min(dw - 332 * k, f.offset_x + (640 - 332) / 2 * f.scale_x))
                 local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
                 return x0, y0, k",
            )
            .eval::<(f64, f64, f64)>()
            .unwrap()
    }
}

fn journal(titles: &[&str]) -> Journal {
    fixture();
    // the process-wide input state an earlier test may have left (a click needs the window focused and the pointer known)
    super::input::reset_input_state();
    super::input::focus(true);
    super::pointer_api::position([0.0, 0.0], true, (1024, 768));
    super::native_core::fixture_demand(true);
    // the window size the HUD frame is worked out from (a process-wide value an earlier test may have left at zero)
    super::hud_layout::set_window_size(1024, 768);
    set_quests(titles);
    let context = context();
    let host = Host::for_addon(&addon("eqoa_quest_journal")).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon("eqoa_quest_journal")).unwrap();
    let mut j = Journal { host, bridge, context };
    press(&j.host, "j", "shift+j");
    j.frames(3);
    j
}

fn set_quests(titles: &[&str]) {
    super::native_core::CORE.with(|c| {
        c.borrow_mut().view.quests = titles
            .iter()
            .enumerate()
            .map(|(i, t)| json!({"ref": {"$ref": "quest", "key": (i + 1).to_string(), "generation": 0, "incarnation": 1}, "index": i + 1, "title": t}))
            .collect();
    });
}

/// The quest the journal reports selected after a click `row_y` HUD pixels down the window (the mod's debug line).
fn selected_after_clicking(titles: &[&str], row_y: f64) -> Option<String> {
    let mut j = journal(titles);
    let (x0, y0, k) = j.place();
    click(&j.host, x0 + 30.0 * k, y0 + row_y * k, "left");
    j.frames(2);
    let logs = j.host.drain_logs().unwrap();
    let found = logs.iter().rev().find_map(|l| l.split("Quest Journal: selected ").nth(1).map(str::to_string));
    if found.is_none() {
        eprintln!("no selection after the click at {row_y}; place {:?}; logs {logs:?}", (x0, y0, k));
    }
    found
}

/// A click on the SECOND line of a wrapped quest selects that quest. Negative control (red before the change): the journal
/// drew each quest on one cut line in a 13-pixel row, so the same click, 19 pixels into the list, was the next quest's.
#[test]
fn a_click_on_the_second_line_of_a_wrapped_quest_selects_that_quest() {
    // the list starts 24 HUD pixels down; a line is 13: 19 pixels in is the second line of the first quest
    let titles = [LONG, "Bat Eyes"];
    assert_eq!(selected_after_clicking(&titles, 24.0 + 13.0 + 6.0).as_deref(), Some(LONG), "the second line belongs to the long quest");
    // controls: the first line is the same quest, and the quest below the wrapped one is still reachable under it
    assert_eq!(selected_after_clicking(&titles, 24.0 + 6.0).as_deref(), Some(LONG), "the first line");
    // a wrapped quest below a one-line one: the one-line row is 13 pixels, the second line of the long one is 26 + 6 down
    assert_eq!(
        selected_after_clicking(&["Bat Eyes", LONG], 24.0 + 13.0 + 13.0 + 6.0).as_deref(),
        Some(LONG),
        "the second line of the second quest"
    );
    assert_eq!(selected_after_clicking(&["Bat Eyes", LONG], 24.0 + 6.0).as_deref(), Some("Bat Eyes"), "the one-line quest above");
}
