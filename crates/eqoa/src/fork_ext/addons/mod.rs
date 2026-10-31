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
pub(crate) mod combat;
pub(crate) mod discovery;
pub(crate) mod effects;
pub(crate) mod entity;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod fonts;
#[cfg(test)]
mod gpu_test;
pub(crate) mod group;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod health;
pub(crate) mod host;
pub(crate) mod icons;
#[cfg(test)]
mod icon_tests;
pub(crate) mod input;
pub(crate) mod manager;
pub(crate) mod package;
pub(crate) mod player;
mod profile;
mod spike;
mod storage;
mod textures;
#[cfg(test)]
mod wereoxxs_test;
