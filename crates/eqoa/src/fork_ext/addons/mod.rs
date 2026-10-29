//! Native-only, explicitly opted-in Forge feasibility host.
mod bridge;
mod capture;
pub(crate) mod game_health;
pub(crate) mod visibility;
mod runtime;
pub(crate) use runtime::Runtime;
static OPEN_REQUEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(crate) fn request_open() {
    OPEN_REQUEST.store(true, std::sync::atomic::Ordering::Release);
}
pub(crate) fn take_open_request() -> bool {
    OPEN_REQUEST.swap(false, std::sync::atomic::Ordering::AcqRel)
}
pub(crate) mod discovery;
#[cfg(test)]
mod gpu_test;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod health;
pub(crate) mod host;
pub(crate) mod manager;
pub(crate) mod package;
mod profile;
mod spike;
mod storage;
mod textures;
