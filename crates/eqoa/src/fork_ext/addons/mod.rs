// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Native-only, explicitly opted-in Forge feasibility host.
mod bridge;
mod capture;
pub(crate) mod game_health;
mod runtime;
pub(crate) mod sound;
pub(crate) mod visibility;
pub(crate) use runtime::Runtime;
static OPEN_REQUEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CLOSE_OPTIONS_ON_OPEN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub(crate) fn request_open() {
    OPEN_REQUEST.store(true, std::sync::atomic::Ordering::Release);
    CLOSE_OPTIONS_ON_OPEN.store(true, std::sync::atomic::Ordering::Release);
}
pub(crate) fn take_open_request() -> bool {
    OPEN_REQUEST.swap(false, std::sync::atomic::Ordering::AcqRel)
}
pub(crate) fn take_close_options_on_open() -> bool {
    CLOSE_OPTIONS_ON_OPEN.swap(false, std::sync::atomic::Ordering::AcqRel)
}
pub(crate) mod ability;
pub(crate) mod bit;
pub(crate) mod chat;
pub(crate) mod chat_history;
#[cfg(test)]
mod chat_history_tests;
pub(crate) mod combat;
pub(crate) mod discovery;
pub(crate) mod effects;
pub(crate) mod entity;
mod file_io;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod fonts;
#[cfg(test)]
mod gpu_test;
pub(crate) mod group;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod health;
pub(crate) mod host;
#[cfg(test)]
mod icon_tests;
pub(crate) mod icons;
pub(crate) mod input;
pub(crate) mod manager;
pub(crate) mod named_profiles;
pub(crate) mod native;
pub(crate) mod native_core;
pub(crate) mod native_events;
mod native_media;
pub(crate) mod native_projection;
pub(crate) mod native_settings;
mod native_state;
#[cfg(test)]
mod native_tests;
pub(crate) mod package;
pub(crate) mod player;
mod profile;
pub(crate) mod quest_log;
mod sandbox;
mod spike;
pub(crate) mod storage;
mod textures;
pub(crate) mod weapon_sets;
#[cfg(test)]
mod weapon_sets_tests;
#[cfg(test)]
mod wereoxxs_test;
