//! Fork-owned services. These modules accept plain data and do not depend on the app.
pub mod mods;
pub mod telemetry;
pub mod release;
#[cfg(not(target_arch = "wasm32"))]
pub mod app_icon;
