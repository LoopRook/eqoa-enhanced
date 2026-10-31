//! Fork-owned services. These modules accept plain data and do not depend on the app.
#[cfg(not(target_arch = "wasm32"))]
pub mod addons;
#[cfg(not(target_arch = "wasm32"))]
pub mod app_icon;
pub mod mods;
pub mod release;
pub mod telemetry;

/// Returns whether the client is in plain gameplay (mode 0 with no dialog,
/// menu, map, editor, or search claiming keyboard input).
pub fn is_plain_play(play: Option<&mut crate::play::Play>) -> bool {
    match play {
        Some(p) => p.mode == 0 && !p.wants_ui(),
        None => false,
    }
}
