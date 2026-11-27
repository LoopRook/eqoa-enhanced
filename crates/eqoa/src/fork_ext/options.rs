// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
// Upstream portions up to 1e23e80 remain MIT (NOTICE).
//! The fork's option rows and pages; loaded as an options child.
//!
//! Enhanced rows follow all upstream pages. Map and quest rows keep upstream IDs.
use super::{Category, Note};
use crate::settings::ClientOptions;
use vi::ui::{Vui, wide};

/// The fork's one page, after upstream's last: the Enhanced HUD rows first, then the Mods & Addons
/// rows (the page was two until 2026-11-13: the Options list ran off the screen).
const fn categories<const M: usize>(upstream: &[Category]) -> [Category; M] {
    let n = upstream.len();
    assert!(M == n + 1);
    let mut all = [CATEGORY; M];
    let mut i = 0;
    while i < n {
        all[i] = upstream[i];
        i += 1;
    }
    all[n] = CATEGORY;
    all
}
const PAGES: [Category; super::CATEGORIES.len() + 1] = categories(super::CATEGORIES);
// every page holds its rows (`MENU_MOST`); since upstream's sync 2026-11-25 a page longer than
// `STANDARD_PAGE_ITEMS` (the Enhanced page) takes the menu's place with its own title
// (`WndOptions::extended_window`)
const _: () = assert!(super::pages_fit(&PAGES));
// the Enhanced page joins the Other group's menu (upstream's UI menu is full at `MENU_MOST`, sync
// 2026-11-26): every group's menu must still fit (`menus_fit`)
const _: () = assert!(super::menus_fit(&PAGES));
/// The rows of the Enhanced page ahead of the Mods & Addons rows: one per Enhanced HUD toggle (the Stats Box's row,
/// 2000, went with the mod when it left the bundle on 2026-11-27).
const HUD_ROWS: usize = 5;
#[cfg(test)]
thread_local! { static FORK_PAGES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(not(target_arch = "wasm32"))]
type DynamicHudEntry = (
    Vec<(i32, String)>,
    &'static [Category],
    &'static [(i32, &'static str)],
);
#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static DYNAMIC_HUD_CACHE: std::cell::RefCell<Vec<DynamicHudEntry>> =
        const { std::cell::RefCell::new(Vec::new()) };
}
#[cfg(not(target_arch = "wasm32"))]
fn dynamic_hud_tables() -> (&'static [Category], &'static [(i32, &'static str)]) {
    let active = crate::fork_ext::addons::options::active_mod_items();
    let is_default = active.len() == HUD_ROWS
        && active
            .iter()
            .zip(&ITEMS[ITEMS.len() - HUD_ROWS..])
            .all(|((id, l), (def_id, def_l))| id == def_id && l == def_l);
    if is_default {
        return (&PAGES, ITEMS);
    }
    DYNAMIC_HUD_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, pages, items)) = cache.iter().find(|(k, _, _)| *k == active) {
            return (*pages, *items);
        }
        let mut pages_vec = PAGES.to_vec();
        let hud_idx = pages_vec.len() - 1;
        // the Enhanced HUD rows the active mods offer, then the Mods & Addons rows
        let page: Vec<i32> = active.iter().map(|(id, _)| *id).chain(CATEGORY.items[HUD_ROWS..].iter().copied()).collect();
        pages_vec[hud_idx].items = Box::leak(page.into_boxed_slice());
        let pages_slice: &'static [Category] = Box::leak(pages_vec.into_boxed_slice());
        let mut items_vec = ITEMS[..ITEMS.len() - HUD_ROWS].to_vec();
        for (id, label) in &active {
            let leaked_label: &'static str = Box::leak(label.clone().into_boxed_str());
            items_vec.push((*id, leaked_label));
        }
        let items_slice: &'static [(i32, &'static str)] = Box::leak(items_vec.into_boxed_slice());
        cache.push((active, pages_slice, items_slice));
        (pages_slice, items_slice)
    })
}
/// Production always uses appended pages. Upstream fixtures retain their own
/// tables; the fork's walker explicitly opts into the complete runtime view.
pub(super) fn pages() -> &'static [Category] {
    #[cfg(test)]
    if !FORK_PAGES.with(|enabled| enabled.get()) {
        return super::CATEGORIES;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        dynamic_hud_tables().0
    }
    #[cfg(target_arch = "wasm32")]
    {
        &PAGES
    }
}
#[cfg(test)]
struct ForkPages;
#[cfg(test)]
impl ForkPages {
    fn enter() -> Self {
        FORK_PAGES.with(|enabled| enabled.set(true));
        Self
    }
}
#[cfg(test)]
impl Drop for ForkPages {
    fn drop(&mut self) {
        FORK_PAGES.with(|enabled| enabled.set(false));
    }
}
pub(super) const ITEMS: &[(i32, &str)] = &[
    (1051, "Open Mods Folder"),
    (1052, "Combat Parser"),
    #[cfg(not(target_arch = "wasm32"))]
    (1053, "Addon Manager"),
    (2001, "Target of Target"),
    (2002, "Mana Bars"),
    (2003, "HUD Numbers"),
    (2004, "Target HP %"),
    (2005, "XP Details"),
];
pub(super) const CATEGORY: Category = Category {
    name: "Enhanced",
    items: &[
        2001,
        2002,
        2003,
        2004,
        2005,
        1051,
        1052,
        #[cfg(not(target_arch = "wasm32"))]
        1053,
    ],
    note: Note::None,
    // listed in upstream's Other menu (Extended's groups): its UI menu holds `MENU_MOST` categories since
    // sync 2026-11-26, so Options > Other > Enhanced
    group: super::Group::Other,
};
/// The registered rows: upstream's and the fork's own.
pub(super) fn items() -> impl Iterator<Item = &'static (i32, &'static str)> {
    #[cfg(not(target_arch = "wasm32"))]
    let fork_items = dynamic_hud_tables().1;
    #[cfg(target_arch = "wasm32")]
    let fork_items = ITEMS;
    super::EXTENDED_ITEMS.iter().chain(fork_items.iter())
}
/// The fork's toggles: the server and the
/// frame follow their setting as upstream's rows do.
pub(super) fn change(c: &mut ClientOptions, item: i32) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    if crate::fork_ext::addons::options::is_button(item) { return false; }
    match item {
        #[cfg(not(target_arch = "wasm32"))]
        item if item >= crate::fork_ext::addons::options::MOD_OPTION_BASE => {
            return crate::fork_ext::addons::options::toggle_by_item(item);
        }
        // An action row (Open Mods Folder, Combat Parser, Addon Manager) acts
        // on ✕ (`activate`), not on left or right.
        _ => return false,
    }
    true
}
pub(super) fn activate(item: i32) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    if crate::fork_ext::addons::options::activate_button(item) { return true; }
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
    #[cfg(not(target_arch = "wasm32"))]
    if crate::fork_ext::addons::options::is_button(item) { return Some(wide("Open")); }
    let on_off = |on| wide(vui.words.get(if on { 551 } else { 552 }, vui.language));
    Some(match item {
        1051 => wide("Open"),
        #[cfg(not(target_arch = "wasm32"))]
        1053 => wide("Open"),
        1052 => wide(if crate::fork_ext::mods::is_parser_running() {
            "Running"
        } else {
            "Launch"
        }),
        #[cfg(not(target_arch = "wasm32"))]
        item if item >= crate::fork_ext::addons::options::MOD_OPTION_BASE => {
            crate::fork_ext::addons::options::value_by_item(item).map(on_off)?
        }
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
    /// The action rows act on ✕ (`activate`), not on a left or right step:
    /// stepping over "Addon Manager" must not open it.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn left_and_right_do_not_run_the_action_rows() {
        let mut c = ClientOptions::default();
        crate::fork_ext::addons::take_open_request();
        assert!(
            !change(&mut c, 1053),
            "the Addon Manager acts on ✕, not on a step"
        );
        assert!(
            !crate::fork_ext::addons::take_open_request(),
            "no open requested by a step"
        );
        assert!(
            !change(&mut c, 9999),
            "an unknown row has nothing to change"
        );
    }
    #[test]
    fn number_toggles_change_only_their_own_saved_setting() {
        #[cfg(not(target_arch = "wasm32"))]
        crate::fork_ext::addons::options::reset_in_memory();
        let defaults = ClientOptions::default();
        let mut c = defaults;
        assert!(change(&mut c, 2004));
        assert_eq!(
            (
                crate::fork_ext::addons::options::value_by_item(2004),
                crate::fork_ext::addons::options::value_by_item(2005),
                crate::fork_ext::addons::options::value_by_item(2003)
            ),
            (Some(false), Some(true), Some(true))
        );
        assert!(change(&mut c, 2005));
        assert!(change(&mut c, 2003));
        assert_eq!(
            (
                crate::fork_ext::addons::options::value_by_item(2004),
                crate::fork_ext::addons::options::value_by_item(2005),
                crate::fork_ext::addons::options::value_by_item(2003)
            ),
            (Some(false), Some(false), Some(false))
        );
        assert_eq!(c.minimap, defaults.minimap);
        assert_eq!(c.shadows, defaults.shadows);
        assert!(!change(&mut c, 9999));
        assert!(!activate(9999));
        assert_eq!(
            (
                crate::fork_ext::addons::options::value_by_item(2004),
                crate::fork_ext::addons::options::value_by_item(2005),
                crate::fork_ext::addons::options::value_by_item(2003)
            ),
            (Some(false), Some(false), Some(false)),
            "activating an unknown item does not alter existing toggles"
        );
        #[cfg(not(target_arch = "wasm32"))]
        crate::fork_ext::addons::options::reset_in_memory();
    }
    #[test]
    fn number_values_use_game_words_and_unknown_rows_have_no_value() {
        #[cfg(not(target_arch = "wasm32"))]
        crate::fork_ext::addons::options::reset_in_memory();
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
        assert_eq!(value_text(&vui, 2004, &c), Some(wide("Localized On")));
        crate::fork_ext::addons::options::set_toggle("eqoa_hud_numbers", "xp_details", false);
        assert_eq!(value_text(&vui, 2005, &c), Some(wide("Localized Off")));
        assert_eq!(value_text(&vui, 1051, &c), Some(wide("Open")));
        assert_eq!(value_text(&vui, 9999, &c), None);
        #[cfg(not(target_arch = "wasm32"))]
        crate::fork_ext::addons::options::reset_in_memory();
    }

    #[test]
    fn combined_tables_preserve_the_upstream_prefix_and_own_every_row_once() {
        #[cfg(not(target_arch = "wasm32"))]
        crate::fork_ext::addons::options::reset_in_memory();
        let _pages = ForkPages::enter();
        let upstream = super::super::CATEGORIES;
        for (actual, original) in pages().iter().zip(upstream) {
            assert_eq!(
                (actual.name, actual.items, actual.note),
                (original.name, original.items, original.note)
            );
        }
        let mut listed: Vec<_> = pages()
            .iter()
            .flat_map(|c| c.items.iter().copied())
            .collect();
        let mut registered: Vec<_> = items()
            .map(|row| row.0)
            .filter(|&id| !(cfg!(target_os = "android") && id == 55))
            .collect();
        listed.sort_unstable();
        registered.sort_unstable();
        let count = listed.len();
        listed.dedup();
        assert_eq!(listed.len(), count);
        // upstream's per-interface pages (sync 2026-11-25): a few rows (Spell Bar Size, Chat Fade Delay,
        // Wake on New Chat) are listed only by the PC interface's `category(i)`; every registered row is
        // on some page in one interface or the other
        crate::settings::testing::reset(false);
        for pc in [true, false] {
            crate::settings::set_ui_mode(pc).unwrap();
            let mut page: Vec<_> = (0..pages().len()).flat_map(|i| super::super::category(i).items.iter().copied()).collect();
            if pc {
                listed.extend(page.iter().copied());
            }
            // each interface lists a row once (a duplicate in the PC pages would not show in the union)
            page.sort_unstable();
            let n = page.len();
            page.dedup();
            assert_eq!(page.len(), n, "a row is listed twice in the interface with pc={pc}");
        }
        listed.sort_unstable();
        listed.dedup();
        assert_eq!(listed, registered);
        // every page fits (upstream's `pages_fit` since sync 2026-11-25); the fork's one page is longer
        // than a standard page (HUD rows, then Mods & Addons rows), so it opens in the menu's place
        let (fork, upstream_pages) = pages().split_last().unwrap();
        assert!(super::super::pages_fit(upstream_pages) && super::super::pages_fit(pages()));
        assert_eq!(fork.name, "Enhanced");
        assert_eq!(fork.items.len(), if cfg!(target_arch = "wasm32") { 8 } else { 9 }, "six HUD rows and the Mods & Addons rows");
        assert_eq!(pages().iter().filter(|c| c.name == "Enhanced HUD" || c.name == "Mods & Addons").count(), 0, "no second fork page");
        assert!(fork.items.len() > super::super::STANDARD_PAGE_ITEMS, "control: the fork's page is more than a standard page's rows");
        assert!(
            items().all(|&(id, label)| !label.is_empty() && super::super::label_of(id) == label)
        );
        let without_fork: Vec<_> = upstream
            .iter()
            .flat_map(|c| c.items.iter().copied())
            .collect();
        assert_ne!(
            without_fork.len(),
            registered.len(),
            "control: missing fork rows detected"
        );
    }
    #[test]
    fn the_fork_pages_and_rows() {
        testing::reset(false);
        #[cfg(not(target_arch = "wasm32"))]
        crate::fork_ext::addons::options::reset_in_memory();
        let _pages = ForkPages::enter();
        let vui = vui();
        let mut w = WndOptions::default();
        let mut o = GameOptions::default();
        let (mut cues, mut ups) = (Vec::new(), Vec::new());
        w.open(&vui, &NullRaster, &o);
        w.sel = 3;
        w.key(
            &vui,
            &mut NullRaster,
            &mut o,
            key::CROSS,
            0,
            &mut cues,
            &mut ups,
        );
        let cats = pages();
        let first = cats.len() - 1;
        assert_eq!(cats[first].name, "Enhanced", "one fork page, after upstream's last");
        assert_eq!(&cats[first].items[..5], &[2001, 2002, 2003, 2004, 2005], "Enhanced HUD rows first");
        assert_eq!(&cats[first].items[5..7], &[1051, 1052], "then the Mods & Addons rows");
        assert!(!cats[first].items.contains(&2000) && !items().any(|i| i.0 == 2000), "the Stats Box row left with the mod (2026-11-27)");
        assert!(!cats[first].items.contains(&1045) && !items().any(|i| i.0 == 1045), "the Quest Markers row is gone (owner 2026-11-22)");
        // Extended's groups (upstream, sync 2026-11-15): the Enhanced page closes the Other group's
        // menu and is in no other group's
        let groups = &super::super::GROUPS;
        let other = groups.iter().position(|g| g.0 == super::super::Group::Other).unwrap();
        let in_other = super::super::group_categories(other);
        assert_eq!(in_other.last(), Some(&first), "Enhanced last in the Other group's menu");
        assert!((0..groups.len()).filter(|&g| g != other).all(|g| !super::super::group_categories(g).contains(&first)), "in no other group");
        w.grp_sel = other as i32;
        w.key(
            &vui,
            &mut NullRaster,
            &mut o,
            key::CROSS,
            0,
            &mut cues,
            &mut ups,
        );
        assert_eq!(w.grp, Some(other), "the Other group's menu open");
        for (index, category) in [(in_other.len() - 1, &cats[first])] {
            w.cat_sel = index as i32;
            w.key(
                &vui,
                &mut NullRaster,
                &mut o,
                key::CROSS,
                0,
                &mut cues,
                &mut ups,
            );
            // the page and the list of pages stay on the 448-high screen (the list ran off it
            // with two fork pages); the window past eight rows has no title bar
            let (top, bottom) = (w.window2.top_left()[1], w.window2.top_left()[1] + w.window2.size[1]);
            assert!(top >= 0 && bottom <= 448, "the page's window on the screen: {top}..{bottom}");
            let list = w.window5.top_left()[1];
            assert!(list >= 0 && list + w.window5.size[1] <= 448, "the list of pages on the screen: {list}");
            for &item in category.items {
                w.draw(&vui, &mut NullRaster, &o, &mut ups);
                assert_eq!(w.item, item);
                assert!(item >= 1000);
                let before = settings::client();
                #[cfg(not(target_arch = "wasm32"))]
                let mod_before = crate::fork_ext::addons::options::value_by_item(item);
                w.key(
                    &vui,
                    &mut NullRaster,
                    &mut o,
                    key::RIGHT,
                    0,
                    &mut cues,
                    &mut ups,
                );
                let after = settings::client();
                if item < 1051 {
                    assert_ne!(after, before, "toggle {item}");
                    w.key(
                        &vui,
                        &mut NullRaster,
                        &mut o,
                        key::LEFT,
                        0,
                        &mut cues,
                        &mut ups,
                    );
                    assert_eq!(settings::client(), before);
                } else if item >= 2000 {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let mod_after = crate::fork_ext::addons::options::value_by_item(item);
                        assert_ne!(mod_after, mod_before, "mod toggle {item}");
                        w.key(
                            &vui,
                            &mut NullRaster,
                            &mut o,
                            key::LEFT,
                            0,
                            &mut cues,
                            &mut ups,
                        );
                        assert_eq!(
                            crate::fork_ext::addons::options::value_by_item(item),
                            mod_before
                        );
                    }
                } else {
                    assert_eq!(after, before, "action rows do not toggle");
                }
                w.key(
                    &vui,
                    &mut NullRaster,
                    &mut o,
                    key::DOWN,
                    0,
                    &mut cues,
                    &mut ups,
                );
            }
            assert_eq!(cues.last(), Some(&9), "stops at last row");
            w.key(
                &vui,
                &mut NullRaster,
                &mut o,
                key::TRIANGLE,
                0,
                &mut cues,
                &mut ups,
            );
        }
        w.key(
            &vui,
            &mut NullRaster,
            &mut o,
            key::DOWN,
            0,
            &mut cues,
            &mut ups,
        );
        assert_eq!(
            (w.cat_sel, cues.last()),
            (in_other.len() as i32 - 1, Some(&9)),
            "fork category end stops"
        );
        assert!(
            !change(&mut ClientOptions::default(), 42),
            "upstream IDs belong to upstream"
        );
        testing::reset(false);
    }

    /// Upstream's Shadows (46) and Quest Tracker (45) toggle
    /// independently and keep their own saved keys.
    #[test]
    fn upstream_shadows_and_enhanced_quest_tracker_toggle_independently() {
        settings::testing::reset(true);
        let (mut w, mut o, mut cues, mut ups) = (
            WndOptions::default(),
            GameOptions::default(),
            Vec::new(),
            Vec::new(),
        );
        assert!(settings::client().shadows && settings::client().quest_tracker);
        w.item = 46;
        w.change(&mut o, 1, &mut cues, &mut ups);
        assert!(!settings::client().shadows && settings::client().quest_tracker);
        w.item = 45;
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

    #[test]
    fn fork_toggle_keeps_all_legacy_saved_keys() {
        // The Quest Markers row is gone from the menu; the saved setting still loads and no row toggles it.
        let mut c = ClientOptions::from_text(
            "map=off
minimap=off
map_markers=off
quest_helper=off
quest_markers=off
quest_tracker=off
",
        );
        assert!(!change(&mut c, 1045), "no row 1045 any more");
        assert!(!c.quest_markers, "a saved quest_markers=off still hides the marks");
        assert!(!c.map && !c.minimap && !c.map_markers && !c.quest_helper && !c.quest_tracker);
        assert_eq!(ClientOptions::from_text(&c.to_text()), c);
        assert!(!change(&mut c, 41));
        assert!(!change(&mut c, 9999));
    }

    use vi::ui::{Raster, Window};

    /// The game's font for the measure: ARIALUI.CSF off the disc (`$EQOA_ISO`, else `run/eqoa.iso`), as the options page draws
    /// with it. With no disc (the CI runners have none) glyphs are 9.5 a character: the disc's font draws the longer labels at
    /// about 9 (a capital-heavy short one at 10), so this is an estimate that keeps labels near the limit honest; the run with
    /// the disc is the exact one.
    struct GameFont(Option<esf::font::Font>);
    impl crate::ui::Layout for GameFont {}
    impl Raster for GameFont {
        fn blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
        fn stretch_blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
        fn fill_fan(&mut self, _: [f32; 4], _: [[i32; 2]; 4]) {}
        fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) {}
        fn draw_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16]) {}
        fn draw_centered_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16], _: u32) {}
        fn string_width(&self, s: &[u16]) -> i32 {
            match &self.0 {
                Some(f) => s.iter().map(|&c| f.glyph(c).or_else(|| f.glyph(b'?' as u16)).map_or(0, |g| g.width as i32 + 1)).sum(),
                None => s.len() as i32 * 19 / 2,
            }
        }
        fn point_size(&self) -> i32 {
            self.0.as_ref().map_or(21, |f| f.height as i32)
        }
        fn viewport(&self) -> [i32; 4] {
            [0, 0, 639, 447]
        }
        fn set_viewport(&mut self, _: [i32; 4]) {}
    }
    fn game_font() -> GameFont {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = std::env::var_os("EQOA_ISO").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("run/eqoa.iso"));
        let font = iso::Iso::open(&path).ok().and_then(|mut d| d.read("ARIALUI.CSF").ok()).and_then(|b| esf::Container::open(b).ok()).and_then(|c| {
            (0..c.nodes.len()).filter(|&i| c.nodes[i].ty == 0x7000).find_map(|i| esf::font::read(&c, i).filter(|f| f.id == vi::ui::font_id() as i32))
        });
        GameFont(font)
    }

    /// Every label the fork puts on the Enhanced page: its own rows and the labels of the bundled mods' option rows
    /// (`eqoa.options.toggle` / `button` / ... in each `main.lua`), the rows that page lists when those mods are on.
    fn fork_row_labels() -> Vec<String> {
        let mut labels: Vec<String> = ITEMS.iter().map(|(_, l)| l.to_string()).collect();
        for file in crate::fork_ext::bundled::EMBEDDED_FILES.iter().filter(|f| f.path.ends_with("/main.lua")) {
            let text = String::from_utf8_lossy(file.data);
            for line in text.lines().filter(|l| l.contains("eqoa.options.") && l.contains("label")) {
                let Some(at) = line.find("label") else { continue };
                let rest = &line[at + 5..];
                let Some(open) = rest.find('"') else { continue };
                let Some(close) = rest[open + 1..].find('"') else { continue };
                labels.push(rest[open + 1..open + 1 + close].to_string());
            }
        }
        labels
    }

    /// The label column of an options page, as `WndOptions::draw_rows_painted` lays it out: the labels start at x 40 and the
    /// value box is at `width - 176`, `width` being the page window's own (440 for every page, `page_window`), and the game's
    /// own label hit area stops 8 short of the box (`width - 176 - 48` wide). A label wider than that is drawn under the box.
    fn label_column(r: &dyn Raster) -> i32 {
        let vui = vi::ui::Vui::new(Default::default(), Default::default(), Vec::new());
        let mut w = Window::default();
        for rows in [9, 12] {
            w.init(&vui, r, [320, 246], 440, rows, 0);
            assert_eq!(w.size[0], 440, "the page's window width");
        }
        (w.size[0] - 176) - 40 - 8
    }

    /// Owner's test 2026-11-27: three labels ran under the On/Off box. Every label of the fork's pages fits the column in the
    /// game's font. Negative control: the labels before the shortening ("Show invitations, summons and resurrection offers",
    /// "Answer quest hand-ins automatically", "World Map: text and marker size...") do not (asserted below).
    #[test]
    fn every_fork_row_label_fits_its_column_in_the_games_font() {
        let font = game_font();
        let column = label_column(&font);
        let vui = vi::ui::Vui::new(Default::default(), Default::default(), Vec::new());
        let measure = |label: &str| Window::default().text_width(&vui, &font, &wide(label));
        let labels = fork_row_labels();
        assert!(labels.len() >= 15, "the scan found the fork's rows and the bundled mods' rows: {labels:?}");
        let too_wide: Vec<String> = labels.iter().filter(|l| measure(l) > column).map(|l| format!("{l:?} is {} wide, the column {column}", measure(l))).collect();
        assert!(too_wide.is_empty(), "labels that run under the value box: {too_wide:#?}");
        for old in ["Show invitations, summons and resurrection offers", "Answer quest hand-ins automatically", "World Map: text and marker size..."] {
            assert!(measure(old) > column, "control: {old:?} ({}) does not fit {column}", measure(old));
        }
    }
}
