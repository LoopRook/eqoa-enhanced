// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
// Upstream portions up to 1e23e80 remain MIT (NOTICE).
//! The fork's option rows and pages; loaded as an options child.
//!
//! Target of Target (31), Mana Bars (42), Minimap (1044) and Stats Box
//! (1047) sit on upstream's UI page; Map & Quests holds Quest Markers
//! (1045), Quest Tracker (1046), Map Markers (1043), and upstream's Text
//! (30), HUD (40) and Raid List (49); Mods & Addons holds the addon rows.
//! Upstream IDs stay below 1000, so the fork's map IDs 1043 and up avoid
//! future collisions. They are transient UI identities, not saved keys.
use super::{Category, Note};
use crate::settings::ClientOptions;
use vi::ui::{Vui, wide};

pub(super) const UI: &[i32] = &[22, 31, 42, 1044, 1047, 29];
pub(super) const QUESTS_CATEGORY: Category = Category {
    name: "Map & Quests",
    items: &[1045, 1046, 1043, 30, 40, 49],
    note: Note::None,
};
pub(super) const ITEMS: &[(i32, &str)] = &[
    (31, "Target of Target"),
    (42, "Mana Bars"),
    (1043, "Map Markers"),
    (1044, "Minimap"),
    (1045, "Quest Markers"),
    (1046, "Quest Tracker"),
    (1047, "Stats Box"),
    (1048, "Target HP %"),
    (1049, "XP Details"),
    (1050, "HUD Numbers"),
    (1051, "Open Mods Folder"),
    (1052, "Combat Parser"),
    #[cfg(not(target_arch = "wasm32"))]
    (1053, "Addon Manager"),
];
pub(super) const CATEGORY: Category = Category {
    name: "Mods & Addons",
    items: &[
        1048,
        1049,
        1050,
        1051,
        1052,
        #[cfg(not(target_arch = "wasm32"))]
        1053,
    ],
    note: Note::None,
};
/// The registered rows: upstream's and the fork's own.
pub(super) fn items() -> impl Iterator<Item = &'static (i32, &'static str)> {
    super::EXTENDED_ITEMS.iter().chain(ITEMS.iter())
}
/// The fork's toggles, on upstream's pages and its own: the server and the
/// frame follow their setting as upstream's rows do.
pub(super) fn change(c: &mut ClientOptions, item: i32) -> bool {
    match item {
        // Target of Target (`Play::target_of_target`)
        31 => c.target_of_target = !c.target_of_target,
        // Mana Bars: the server told from the next frame
        // (`Play::agree_power_bars`)
        42 => c.mana_bars = !c.mana_bars,
        // Map Markers: the map's camps and coaches from its next frame
        // (`map::markers`)
        1043 => c.map_markers = !c.map_markers,
        // Minimap: drawn or not from the next frame (`map::mini`)
        1044 => c.minimap = !c.minimap,
        // Quest Markers: the server told from the next frame
        // (`Play::agree_quest_marks`), the map's marks drawn or not
        1045 => c.quest_markers = !c.quest_markers,
        // Quest Tracker: the server told from the next frame
        // (`Play::agree_quest_progress`), the HUD's list drawn or not
        1046 => c.quest_tracker = !c.quest_tracker,
        // Stats Box: drawn or not from the next frame (`statbox`)
        1047 => c.stats_box = !c.stats_box,
        1048 => c.target_hp_percent = !c.target_hp_percent,
        1049 => c.xp_details = !c.xp_details,
        1050 => c.hud_numbers = !c.hud_numbers,
        _ => return activate(item),
    }
    true
}
pub(super) fn activate(item: i32) -> bool {
    match item {
        1051 => crate::fork_ext::mods::open_mods_folder(),
        1052 => {
            if let Err(error) = crate::fork_ext::mods::launch_parser_result() {
                crate::say(&format!("Combat parser: {error}"));
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        1053 => crate::fork_ext::addons::request_open(),
        _ => return false,
    }
    true
}
pub(super) fn value_text(vui: &Vui, item: i32, c: &ClientOptions) -> Option<Vec<u16>> {
    let on_off = |on| wide(vui.words.get(if on { 551 } else { 552 }, vui.language));
    Some(match item {
        31 => on_off(c.target_of_target),
        42 => on_off(c.mana_bars),
        1043 => on_off(c.map_markers),
        1044 => on_off(c.minimap),
        1045 => on_off(c.quest_markers),
        1046 => on_off(c.quest_tracker),
        1047 => on_off(c.stats_box),
        1048 => on_off(c.target_hp_percent),
        1049 => on_off(c.xp_details),
        1050 => on_off(c.hud_numbers),
        1051 => wide("Open"),
        #[cfg(not(target_arch = "wasm32"))]
        1053 => wide("Open"),
        1052 => wide(if crate::fork_ext::mods::is_parser_running() {
            "Running"
        } else {
            "Launch"
        }),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{GameOptions, WndOptions, key};
    use crate::pointer::testing::{NullRaster, vui};
    use crate::settings::{self, testing};

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn addon_manager_action_requests_open_without_changing_settings_or_companion_rows() {
        crate::fork_ext::addons::take_open_request();
        assert!(activate(1053));
        assert!(crate::fork_ext::addons::take_open_request());
        assert!(!crate::fork_ext::addons::take_open_request());
        assert!(ITEMS.iter().any(|(id, _)| *id == 1052));
        assert!(CATEGORY.items.contains(&1053));
    }
    #[test]
    fn number_toggles_change_only_their_own_saved_setting() {
        let defaults = ClientOptions::default();
        let mut c = defaults;
        assert!(change(&mut c, 1048));
        assert_eq!(
            (c.target_hp_percent, c.xp_details, c.hud_numbers),
            (false, true, true)
        );
        assert!(change(&mut c, 1049));
        assert!(change(&mut c, 1050));
        assert_eq!(
            (c.target_hp_percent, c.xp_details, c.hud_numbers),
            (false, false, false)
        );
        assert_eq!(c.minimap, defaults.minimap);
        assert_eq!(c.shadows, defaults.shadows);
        assert!(!change(&mut c, 9999));
        assert!(!activate(9999));
        assert_eq!(
            (c.target_hp_percent, c.xp_details, c.hud_numbers),
            (false, false, false)
        );
    }
    #[test]
    fn number_values_use_game_words_and_unknown_rows_have_no_value() {
        let words = (1..=552)
            .map(|i| match i {
                551 => "Localized On",
                552 => "Localized Off",
                _ => "",
            })
            .collect::<Vec<_>>()
            .join("\n");
        let vui = Vui::new(
            vi::strtab::StringTable::parse(words.as_bytes()).unwrap(),
            Default::default(),
            Vec::new(),
        );
        let c = ClientOptions::default();
        assert_eq!(value_text(&vui, 1048, &c), Some(wide("Localized On")));
        let off = ClientOptions {
            xp_details: false,
            ..c
        };
        assert_eq!(value_text(&vui, 1049, &off), Some(wide("Localized Off")));
        assert_eq!(value_text(&vui, 1051, &c), Some(wide("Open")));
        assert_eq!(value_text(&vui, 9999, &c), None);
    }

    /// The fork's rows on upstream's pages and its own two pages, walked as
    /// upstream's test walks them.
    // Native only: the web build has no Addon Manager row and no Textures page.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_fork_pages_and_rows() {
        let item_named = |label| ITEMS.iter().find(|&&(_, name)| name == label).unwrap().0;
        assert_eq!(UI, &[22, 31, 42, 1044, 1047, 29], "UI: the fork's four between Ability Bars and Update Notices");
        assert_eq!(QUESTS_CATEGORY.name, "Map & Quests");
        assert_eq!(QUESTS_CATEGORY.items, &[1045, 1046, 1043, 30, 40, 49], "Map & Quests' rows");
        testing::reset(false);
        let vui = vui();
        let mut r = NullRaster;
        let mut o = GameOptions { auto_attack: 0, ..GameOptions::default() };
        let mut w = WndOptions::default();
        let (mut cues, mut ups) = (Vec::new(), Vec::new());
        let mut press = |w: &mut WndOptions, o: &mut GameOptions, k: i32, cues: &mut Vec<i32>| {
            w.key(&vui, &mut r, o, k, 0, cues, &mut ups);
            w.draw(&vui, &mut NullRaster, o, &mut ups);
        };
        w.open(&vui, &NullRaster, &o);
        for k in [key::DOWN, key::DOWN, key::DOWN, key::CROSS] {
            press(&mut w, &mut o, k, &mut cues);
        }
        assert_eq!((w.page, w.cat), (3, None), "Extended's categories");
        // UI: Ability Bars, Target of Target, Mana Bars, Minimap, Stats Box,
        // Update Notices
        w.cat_sel = 2;
        press(&mut w, &mut o, key::CROSS, &mut cues);
        assert_eq!((w.cat, w.item), (Some(2), 22), "UI, Ability Bars");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 31, "Target of Target");
        assert!(!settings::client().target_of_target, "off: the game's target bar alone");
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!((settings::client().target_of_target, cues.last()), (true, Some(&11)), "on");
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert!(!settings::client().target_of_target, "and back: two values");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 42, "Mana Bars");
        assert!(settings::client().mana_bars, "on: the power gauges where the server sends power");
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!((settings::client().mana_bars, cues.last()), (false, Some(&11)), "off");
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert!(settings::client().mana_bars, "and back: two values");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 1044, "Minimap");
        assert!(settings::client().minimap, "on: the map round the player under the quick menus");
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!((settings::client().minimap, cues.last()), (false, Some(&11)), "off");
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert!(settings::client().minimap, "and back: two values");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 1047, "Stats Box");
        assert!(!settings::client().stats_box, "off: no box, the default");
        assert_eq!(crate::options::value_text(&vui, 1047), Some(wide(vui.words.get(552, vui.language))), "Off, the game's word");
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!((settings::client().stats_box, cues.last()), (true, Some(&11)), "on");
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert!(!settings::client().stats_box, "and back: two values");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 29, "Update Notices, back on upstream's page");
        press(&mut w, &mut o, key::TRIANGLE, &mut cues);
        // Map & Quests: Quest Markers, Quest Tracker, Map Markers, Text, HUD,
        // Raid List
        w.cat_sel = 7;
        press(&mut w, &mut o, key::CROSS, &mut cues);
        assert_eq!((w.cat, w.item), (Some(7), 1045), "Map & Quests, Quest Markers");
        assert!(settings::client().quest_markers, "on: the map's marks where the server sends them");
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!((settings::client().quest_markers, cues.last()), (false, Some(&11)), "off");
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert!(settings::client().quest_markers, "and back: two values");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 1046, "Quest Tracker");
        assert!(settings::client().quest_tracker, "on: the HUD's list where the server sends it");
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!((settings::client().quest_tracker, cues.last()), (false, Some(&11)), "off");
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert!(settings::client().quest_tracker, "and back: two values");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 1043, "Map Markers, in Map & Quests");
        let was = settings::client().map_markers;
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!(settings::client().map_markers, !was);
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert_eq!(settings::client().map_markers, was);
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 30, "Text");
        assert_eq!(settings::client().text, render::ui::TextFilter::Sharp, "Sharp, the default");
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!((settings::client().text, cues.last()), (render::ui::TextFilter::Smooth, Some(&11)), "Smooth");
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert_eq!(settings::client().text, render::ui::TextFilter::Sharp, "and back: two values");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 40, "HUD");
        assert_eq!((settings::client().hud, crate::options::value_text(&vui, 40)), (render::ui::HudFilter::Nearest, Some(wide("Nearest"))), "Nearest, the game's and the default");
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!((settings::client().hud, crate::options::value_text(&vui, 40), cues.last()), (render::ui::HudFilter::Sharp, Some(wide("Sharp")), Some(&11)), "Sharp");
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert_eq!(settings::client().hud, render::ui::HudFilter::Nearest, "and back: two values");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!(w.item, 49, "Raid List");
        assert!(settings::raid_list(), "on: a raid group's every name and health");
        assert_eq!(crate::options::value_text(&vui, 49), Some(wide(vui.words.get(551, vui.language))));
        press(&mut w, &mut o, key::RIGHT, &mut cues);
        assert_eq!((settings::raid_list(), cues.last()), (false, Some(&11)), "off: the game's bars");
        press(&mut w, &mut o, key::LEFT, &mut cues);
        assert!(settings::raid_list(), "and back: two values");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!((w.row, cues.last()), (5, Some(&9)), "Map & Quests' last row");
        press(&mut w, &mut o, key::TRIANGLE, &mut cues);
        // Mods & Addons, in its registered order
        w.cat_sel = 8;
        press(&mut w, &mut o, key::CROSS, &mut cues);
        assert_eq!((w.cat, w.row, w.item), (Some(8), 0, item_named("Target HP %")), "Mods & Addons page: Target HP % first");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!((w.row, w.item), (1, item_named("XP Details")), "XP Details");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!((w.row, w.item), (2, item_named("HUD Numbers")), "HUD Numbers");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!((w.row, w.item), (3, item_named("Open Mods Folder")), "Open Mods Folder");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!((w.row, w.item), (4, item_named("Combat Parser")), "Combat Parser");
        press(&mut w, &mut o, key::DOWN, &mut cues);
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!((w.row, cues.last()), (5, Some(&9)), "Mods & Addons last row");
        press(&mut w, &mut o, key::TRIANGLE, &mut cues);
        press(&mut w, &mut o, key::DOWN, &mut cues);
        assert_eq!((w.cat_sel, cues.last()), (9, Some(&11)), "Textures, after Mods & Addons");
        testing::reset(false);
    }

    /// Upstream's Shadows (46) and the fork's Quest Tracker (1046) toggle
    /// independently and keep their own saved keys.
    #[test]
    fn upstream_shadows_and_enhanced_quest_tracker_toggle_independently() {
        settings::testing::reset(true);
        let (mut w, mut o, mut cues, mut ups) = (WndOptions::default(), GameOptions::default(), Vec::new(), Vec::new());
        assert!(settings::client().shadows && settings::client().quest_tracker);
        w.item = 46;
        w.change(&mut o, 1, &mut cues, &mut ups);
        assert!(!settings::client().shadows && settings::client().quest_tracker);
        w.item = 1046;
        w.change(&mut o, 1, &mut cues, &mut ups);
        assert!(!settings::client().shadows && !settings::client().quest_tracker);
        let text = settings::client().to_text();
        let restored = ClientOptions::from_text(&text);
        assert!(!restored.shadows && !restored.quest_tracker);
        assert!(text.contains("shadows=off") && text.contains("quest_tracker=off"));
        w.item = 46;
        w.change(&mut o, 1, &mut cues, &mut ups);
        assert!(settings::client().shadows && !settings::client().quest_tracker);
        settings::testing::reset(true);
    }
}
