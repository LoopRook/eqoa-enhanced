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

pub(crate) mod map_toolbar;
