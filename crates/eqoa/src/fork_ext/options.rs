//! Mods & Addons page registration and behavior; loaded as an options child.
use super::{Category, Note};
use crate::settings::ClientOptions;
use vi::ui::{Vui, wide};

pub(super) const ITEMS: [(i32, &str); 5] = [
    (1048, "Target HP %"),
    (1049, "XP Details"),
    (1050, "HUD Numbers"),
    (1051, "Open Mods Folder"),
    (1052, "Combat Parser"),
];
pub(super) const CATEGORY: Category = Category {
    name: "Mods & Addons",
    items: &[1048, 1049, 1050, 1051, 1052],
    note: Note::None,
};
pub(super) fn change(c: &mut ClientOptions, item: i32) -> bool {
    match item {
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
        _ => return false,
    }
    true
}
pub(super) fn value_text(vui: &Vui, item: i32, c: &ClientOptions) -> Option<Vec<u16>> {
    let on_off = |on| wide(vui.words.get(if on { 551 } else { 552 }, vui.language));
    Some(match item {
        1048 => on_off(c.target_hp_percent),
        1049 => on_off(c.xp_details),
        1050 => on_off(c.hud_numbers),
        1051 => wide("Open"),
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
        assert!(!change(&mut c, 1044));
        assert!(!activate(1044));
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
        assert_eq!(value_text(&vui, 1044, &c), None);
    }
}
