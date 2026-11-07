// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see LICENSE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Fork-owned services. These modules accept plain data and do not depend on the app.
#[cfg(not(target_arch = "wasm32"))]
pub mod addons;
#[cfg(not(target_arch = "wasm32"))]
pub mod app_icon;
#[cfg(not(target_arch = "wasm32"))]
pub mod bundled;
pub mod chat_history;
pub mod mods;
pub mod release;
#[cfg(not(target_arch = "wasm32"))]
pub mod report;
#[cfg(not(target_arch = "wasm32"))]
mod report_background;
pub mod report_core;
pub mod telemetry;

/// Returns whether the client is in plain gameplay (mode 0 with no dialog,
/// menu, map, editor, or search claiming keyboard input).
pub fn is_plain_play(play: Option<&mut crate::play::Play>) -> bool {
    match play {
        Some(p) => p.mode == 0 && !p.wants_ui(),
        None => false,
    }
}
