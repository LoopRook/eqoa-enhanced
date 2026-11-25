// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Fork-owned services. These modules accept plain data and do not depend on the app.
#[cfg(not(target_arch = "wasm32"))]
pub mod addons;
#[cfg(not(target_arch = "wasm32"))]
pub use addons::modtools;
#[cfg(target_arch = "wasm32")]
pub mod modtools {
    pub fn run(_: &[String]) -> Option<crate::Result<()>> {
        None
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub mod app_icon;
#[cfg(not(target_arch = "wasm32"))]
pub mod bundled;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod loot_source;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod loot_by_id;
pub mod chat_history;
pub mod mods;
pub mod player_stats;
pub mod release;
#[cfg(not(target_arch = "wasm32"))]
pub mod report;
#[cfg(not(target_arch = "wasm32"))]
mod report_background;
#[cfg(not(target_arch = "wasm32"))]
pub mod report_crash_dir;
pub mod report_label;
pub mod report_no_upload;
#[cfg(target_arch = "wasm32")]
pub mod report {
    pub use super::report_label::enhanced_label;
}
pub mod report_core;
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod session_replay;
pub mod telemetry;
pub mod version;

/// The frame's HUD phase in stages, for the stall probe (`addons::stall_probe`): ends the stage before and starts
/// `label`'s; a stage that takes long is said in the log. Nothing on a page.
pub(crate) fn hud_stage(_label: &'static str) {
    #[cfg(not(target_arch = "wasm32"))]
    addons::stall_probe::stage(_label);
}

/// The last HUD stage of the frame ends.
pub(crate) fn hud_stage_end() {
    #[cfg(not(target_arch = "wasm32"))]
    addons::stall_probe::stage_end();
}

/// The client asked to loot corpse `id` (0x0015): the loot window's source, when its list arrives.
pub(crate) fn loot_requested(_id: u32) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        loot_source::requested(_id);
        addons::window_takeover::note_loot_request(_id);
    }
}

/// Returns whether the client is in plain gameplay (mode 0 or manual camera 38
/// with no dialog, menu, map, editor, or search claiming keyboard input).
pub fn is_plain_play(play: Option<&mut crate::play::Play>) -> bool {
    match play {
        #[cfg(not(target_arch = "wasm32"))]
        Some(p) => p.addon_input_available(),
        #[cfg(target_arch = "wasm32")]
        Some(p) => matches!(p.mode, 0 | 38) && !p.wants_ui(),
        None => false,
    }
}
pub(crate) fn prepare_controls(
    _report: &mut crate::input::Report,
    _connected: bool,
    _gameplay: bool,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        addons::native_core::presentation();
        addons::input::observe_report(_report, _connected, _gameplay);
        // round A1: a mod window's pad focus takes the d-pad, confirm and back (after the report is seen, before the game)
        if _gameplay {
            addons::native_a6::note_pad(_report.buttons);
            addons::window_sessions::apply_pad(_report);
        }
        if _gameplay && addons::input::is_text_capture_active() {
            *_report = crate::input::Report::default();
        }
    }
}

#[cfg(test)]
mod touch_tests;

pub(crate) mod map;

pub(crate) mod bindpage;

/// Stock minimap hit routing; addon replacement does not leave invisible click targets.
pub(crate) fn minimap_contains(map: &crate::map::Map, point: [f32; 2]) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    if addons::visibility::effective_visibility().minimap {
        return false;
    }
    map.mini_contains(point)
}

/// Blanks the quads of a HUD piece hidden by a mod (seam 3 in `hudlayout::end` and `seal`) in place
/// so hidden pieces draw nothing and are not outlined by Move HUD, while keeping `ui.quads.len()`
/// intact so `Ui::end()`'s `sided`/`risen`/`opaque` indices stay valid on widescreen and tall windows.
pub(crate) fn piece_hidden(_ui: &mut render::ui::Ui, _piece: crate::hudlayout::Piece, _from: usize) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::hudlayout::Piece;
        let v = addons::visibility::effective_visibility();
        let hidden = match _piece {
            Piece::Player => v.player_hidden(),
            Piece::Group => {
                let is_raid = crate::hud::fork_hud::take_last_group_was_raid()
                    .unwrap_or_else(|| addons::hud_layout::get_hud_state().in_raid);
                if is_raid { v.raid_list } else { v.group_panel }
            }
            Piece::Pet => v.pet,
            Piece::Target => v.target,
            Piece::Minimap => v.minimap,
            Piece::Quests => v.quest_tracker,
            Piece::Meter => v.dps_meter,
            Piece::Effects => v.active_effects,
            // upstream's PC interface (sync 2026-11-25) splits chat into up to four panes
            Piece::Chat | Piece::Chat1 | Piece::Chat2 | Piece::Chat3 => v.chat,
            Piece::Breath => v.breath,
            Piece::QuickMenus => v.ability_bar || (v.spell_bars && v.utility_bars),
            Piece::SpellBars => v.ability_bar || v.spell_bars,
            // the PC weapon-set bar was part of the Utility Bars before upstream gave it its own piece
            Piece::UtilityBars | Piece::EquipmentBar => v.ability_bar || v.utility_bars,
            // upstream's pieces with no mod visibility flag: always drawn
            Piece::Settings | Piece::TargetInfo | Piece::RaidPrompts | Piece::Stats | Piece::Focus => false,
        };
        if hidden {
            let start = _from.min(_ui.quads.len());
            for q in &mut _ui.quads[start..] {
                q.pos = [[0.0; 2]; 4];
                q.color = [0.0; 4];
                q.blend = 3;
                q.clip = Some([0, 0, 0, 0]);
            }
            return true;
        }
    }
    false
}

pub(crate) mod map_toolbar;
