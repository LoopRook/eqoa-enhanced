// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Native-only, explicitly opted-in Forge feasibility host.
mod bridge;
#[path = "../modtools.rs"]
pub mod modtools;
mod capture;
pub(crate) mod game_health;
pub(crate) mod runtime;
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
static TOGGLE_REQUEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The manager key was pressed in the world: the window opens, or closes if it is open. (Unlike
/// `request_open`, which Options' Addon Manager row uses and which closes Options, nothing else
/// is touched: the world is already showing.)
pub(crate) fn request_toggle() {
    TOGGLE_REQUEST.store(true, std::sync::atomic::Ordering::Release);
}
pub(crate) fn take_toggle_request() -> bool {
    TOGGLE_REQUEST.swap(false, std::sync::atomic::Ordering::AcqRel)
}
pub(crate) fn take_open_request() -> bool {
    OPEN_REQUEST.swap(false, std::sync::atomic::Ordering::AcqRel)
}
pub(crate) fn take_close_options_on_open() -> bool {
    CLOSE_OPTIONS_ON_OPEN.swap(false, std::sync::atomic::Ordering::AcqRel)
}
pub(crate) mod ability;
pub(crate) mod addon_perf;
mod object_cache;
mod record_raw;
pub(crate) mod action_broker;
pub(crate) mod actions;
pub(crate) mod window_actions;
pub(crate) mod window_kinds;
pub(crate) mod window_takeover;
pub(crate) mod raid_strips;
pub(crate) mod gesture;
mod async_json;
pub(crate) mod bit;
pub(crate) mod chat;
pub(crate) mod chat_history;
#[cfg(test)]
mod autoloot_logic_tests;
#[cfg(test)]
mod bag_helpers_tests;
#[cfg(test)]
mod chat_history_tests;
#[cfg(test)]
mod docs_tests;
pub(crate) mod combat;
pub(crate) mod discovery;
pub(crate) mod effects;
pub(crate) mod entity;
mod file_io;
mod exchange;
mod player_files;
mod map_migration;
mod option_buttons;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod fonts;
#[cfg(test)]
mod gpu_test;
mod gems;
pub(crate) mod group;
pub(crate) mod guides;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod health;
pub(crate) mod host;
pub(crate) mod hud_layer;
pub(crate) mod hud_layout;
mod hud_textures;
#[cfg(test)]
mod icon_tests;
pub(crate) mod icons;
pub(crate) mod input;
mod items_forge;
mod json_save;
pub(crate) mod manager;

/// Runs a test's body on its own thread with a 16 MiB stack. The GPU addon harnesses hold a `Play`
/// (about 110 KB), a `Manager` and more by value (`Bv` about 170 KB, `Rm` about 190 KB), and a debug
/// build copies them several times while building one, so these tests sit near the 2 MiB default
/// test-thread stack; CI's build went over it (2026-11-19, STATUS_STACK_OVERFLOW, which aborts the
/// whole test binary). A panic in the body fails the test as before.
#[cfg(test)]
pub(crate) fn big_stack(body: fn()) {
    let joined = std::thread::Builder::new()
        .name(std::thread::current().name().unwrap_or("big-stack test").to_owned())
        .stack_size(16 << 20)
        .spawn(body)
        .expect("spawn the test's thread")
        .join();
    if let Err(panic) = joined {
        std::panic::resume_unwind(panic);
    }
}
pub(crate) mod map_views;
pub(crate) mod map_waypoint;
pub(crate) mod named_profiles;
pub(crate) mod native;
pub(crate) mod native_core;
pub(crate) mod native_events;
pub(crate) mod native_items;
mod native_media;
pub(crate) mod native_projection;
pub(crate) mod native_r6a;
pub(crate) mod native_a3;
pub(crate) mod frame_actions;
#[cfg(test)]
mod frames_mod_tests;
pub(crate) mod native_settings;
mod native_state;
#[cfg(test)]
pub(crate) mod native_tests;
#[cfg(test)]
mod perf_delta_tests;
#[cfg(test)]
mod perf_delta_event_tests;
pub(crate) mod options;
pub(crate) mod package;
pub(crate) mod player;
pub(crate) mod pointer_api;
mod profile;
pub(crate) mod quest_log;
#[cfg(test)]
mod round_3_picture_tests;
#[cfg(test)]
mod round7c_fix_tests;
#[cfg(test)]
mod round7h_tests;
mod sandbox;
mod spike;
pub(crate) mod storage;
mod textures;
pub(crate) mod weapon_sets;
#[cfg(test)]
mod weapon_sets_tests;
#[cfg(test)]
mod wereoxxs_test;
#[cfg(test)]
pub(crate) use wereoxxs_test::modern_slot_positions;

#[cfg(test)]
pub(crate) mod perf_alloc;
