# Forge Addon Package Loader & Resource Resolver (Stage 2B) Report

**Date**: 2026-10-28
**Scope**: Stage 2B native per-addon package loader and resource resolver ([`crates/eqoa/src/fork_ext/addons/package.rs`](../crates/eqoa/src/fork_ext/addons/package.rs) and [`crates/eqoa/src/fork_ext/addons/host.rs`](../crates/eqoa/src/fork_ext/addons/host.rs)). Pure loader and resolver engine; no manager, UI, or map modifications.

---

## 1. Executive Summary

Stage 2B implements isolated, per-addon module loading (`require`) and contained resource path resolution. Every addon is assigned its own dedicated Lua 5.1 virtual machine with independent globals, callback registry, and `package.loaded` cache.

Key capabilities delivered:
- **Per-Addon VM Isolation**: Distinct `Host` instances instantiated via [`Host::for_addon(&DiscoveredAddon)`](../crates/eqoa/src/fork_ext/addons/host.rs#L24-L100), coexisting with [`Host::new()`](../crates/eqoa/src/fork_ext/addons/host.rs#L102-L177) without breaking existing spike tests.
- **Ordered Module Resolution**: Resolves conventional dotted Lua module names using [`DiscoveredAddon.module_dirs`](../crates/eqoa/src/fork_ext/addons/discovery.rs#L16) (private package modules first, then shared script-root modules), checking `<name.replaceDotsSlash>.lua` followed by `<name>/init.lua`.
- **Reserved Native Modules**: Native host registry controls `frontiers_forge.*` (snapshot-backed `util`, and registered native submodules like `frontiers_forge.player`), `imgui`, and `uiforge`. Deceptive or mock definitions on disk (such as autocomplete definitions or memory-backed stubs) are strictly ignored. Unknown `frontiers_forge.*` imports fail naming the unsupported API; `ffi` and native OS libraries remain unavailable.
- **Lua 5.1 Caching & Reload Semantics**: Full fidelity `package.loaded` caching conforming to Lua 5.1 `loadlib.c` (481–493): non-nil loader returns overwrite `package.loaded[name]`; chunks returning `nil` retain what was written to `package.loaded[name]`, falling back to boolean `true` if still the sentinel; explicit `package.loaded[name] = nil` plus `nil` return preserves `nil`. Chunks setting or returning `false` trigger standard reload. Chunks receive module name as `...`.
- **Containment & Security**:
  - Bounded file reads: strictly stream-capped at 1 MiB (`Read::take(1024 * 1024 + 1)`), preventing memory exhaustion.
  - Active script root containment verified on *every* module load to protect against post-discovery symlink tampering.
  - Recursion limit: nesting depth capped at 32.
  - Cycle detection: records loading chains and reports formatted cycle errors (e.g. `cycle_a -> cycle_b -> cycle_a`).
  - Sentinel cleanup & RAII guard: `loading_stack` is guarded by an RAII guard so hostile metatable errors or runtime exceptions cannot poison the loading stack or exhaust the 32-call recursion limit.
  - Shared instruction quota: nested `require` calls do not reset `Host.run`'s 200,000 instruction budget.
- **Contained Resource Resolver**: Rust-only API [`resolve_resource`](../crates/eqoa/src/fork_ext/addons/package.rs#L34-L88) supporting relative path lookup across private then shared [`DiscoveredAddon.resource_dirs`](../crates/eqoa/src/fork_ext/addons/discovery.rs#L17), rejecting absolute, drive letter, UNC, and `..` path traversals across both separators on all OS platforms.
- **VM Setup Deduplication**: Extracted shared host VM initialization ([`create_base_host`](../crates/eqoa/src/fork_ext/addons/host.rs)) unifying base environment creation, memory quota, and callback state across [`Host::new`](../crates/eqoa/src/fork_ext/addons/host.rs) and [`Host::for_addon`](../crates/eqoa/src/fork_ext/addons/host.rs).

---

## 2. Architecture & Data Interfaces

### Resource Resolver API
```rust
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResourceError {
    InvalidPath(String),
    NotFound(String),
    EscapesScriptRoot(PathBuf),
    NotAFile(PathBuf),
    BrokenFile(String),
}

pub(crate) fn resolve_resource(
    addon: &DiscoveredAddon,
    relative_path: &str,
) -> Result<PathBuf, ResourceError>;
```

### Host Addon Integration
```rust
impl Host {
    pub fn for_addon(addon: &DiscoveredAddon) -> Result<Self>;
    pub fn compile_addon_entry(&self, addon: &DiscoveredAddon) -> Result<Function>;
    pub fn run_addon(&self, addon: &DiscoveredAddon) -> Result<()>;
    pub fn register_host_module(&self, name: impl Into<String>, value: Value) -> Result<()>;
    pub fn resolve_resource(&self, relative_path: &str) -> Result<PathBuf, ResourceError>;
}
```

---

## 3. Test Verification (Red / Green)

### RED Baseline (Initial Test Run)
All 8 behavioral tests were initially created with panics and failed as expected:
```text
running 8 tests
test fork_ext::addons::package::tests::forge_package_cached_mutable_module_survives_frames ... FAILED
test fork_ext::addons::package::tests::forge_package_cycles_and_error_retry ... FAILED
test fork_ext::addons::package::tests::forge_package_nested_module_runaway_stops_original_budget ... FAILED
test fork_ext::addons::package::tests::forge_package_nil_and_false_reload_standard_behavior ... FAILED
test fork_ext::addons::package::tests::forge_package_private_shared_precedence_and_init_fallback ... FAILED
test fork_ext::addons::package::tests::forge_package_reserved_host_precedence_unsupported_reserved_ffi_bytecode_and_path_rejection ... FAILED
test fork_ext::addons::package::tests::forge_package_resource_precedence_and_containment ... FAILED
test fork_ext::addons::package::tests::forge_package_two_addons_requiring_same_config_independently ... FAILED

test result: FAILED. 0 passed; 8 failed; 0 ignored; 0 measured; 1000 filtered out; finished in 0.00s
```

### GREEN State (Implementation Complete)
Command:
```powershell
$env:PATH = "C:\Users\hocka\.cargo\bin;$env:PATH"
cargo test --locked -p eqoa forge_package_ -- --nocapture
```
Output:
```text
running 8 tests
test fork_ext::addons::package::tests::forge_package_cached_mutable_module_survives_frames ... ok
test fork_ext::addons::package::tests::forge_package_nil_and_false_reload_standard_behavior ... ok
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::package::tests::forge_package_nested_module_runaway_stops_original_budget ... ok
test fork_ext::addons::package::tests::forge_package_reserved_host_precedence_unsupported_reserved_ffi_bytecode_and_path_rejection ... ok
test fork_ext::addons::package::tests::forge_package_resource_precedence_and_containment ... ok
test fork_ext::addons::package::tests::forge_package_cycles_and_error_retry ... ok
test fork_ext::addons::package::tests::forge_package_two_addons_requiring_same_config_independently ... ok
test fork_ext::addons::package::tests::forge_package_private_shared_precedence_and_init_fallback ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 1000 filtered out; finished in 0.05s
```

### Full Addon Module Suite (Initial)
Command:
```powershell
$env:PATH = "C:\Users\hocka\.cargo\bin;$env:PATH"
cargo test --locked -p eqoa fork_ext::addons:: -- --nocapture
```
Output:
```text
running 26 tests
...
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 982 filtered out; finished in 0.06s
```

### Stage 2B Remediation Verification (Red / Green)

Following the independent review findings (HIGH-01, MED-01, MED-02, and LOW-01), 3 targeted regression tests were added to `crates/eqoa/src/fork_ext/addons/package.rs`:
- `forge_package_registered_host_submodules_precedence_and_ffi_lockdown`
- `forge_package_return_overwrites_package_loaded_and_explicit_nil_preserves_nil`
- `forge_package_hostile_metatable_does_not_poison_loading_stack`

#### RED Baseline (3 new tests failing against un-remediated code)
Command:
```powershell
cargo test -p eqoa --manifest-path Cargo.toml forge_package_registered_host_submodules forge_package_return_overwrites forge_package_hostile_metatable -- --nocapture
```
Output:
```text
running 3 tests
test fork_ext::addons::package::tests::forge_package_registered_host_submodules_precedence_and_ffi_lockdown ... FAILED
test fork_ext::addons::package::tests::forge_package_return_overwrites_package_loaded_and_explicit_nil_preserves_nil ... FAILED
test fork_ext::addons::package::tests::forge_package_hostile_metatable_does_not_poison_loading_stack ... FAILED

failures:
    RuntimeError("unsupported FrontiersForge API: 'frontiers_forge.player'")
    assertion `left == right` failed: left: 1, right: 2
    expected hostile error, got: runtime error: module require nesting depth exceeded limit (32)

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 1008 filtered out; finished in 0.01s
```

#### GREEN State (Remediated Package Suite - 11 Tests)
Command:
```powershell
cargo test -p eqoa --manifest-path Cargo.toml forge_package_ -- --nocapture
```
Output:
```text
running 11 tests
test fork_ext::addons::package::tests::forge_package_cached_mutable_module_survives_frames ... ok
test fork_ext::addons::package::tests::forge_package_cycles_and_error_retry ... ok
test fork_ext::addons::package::tests::forge_package_hostile_metatable_does_not_poison_loading_stack ... ok
test fork_ext::addons::package::tests::forge_package_nested_module_runaway_stops_original_budget ... ok
test fork_ext::addons::package::tests::forge_package_nil_and_false_reload_standard_behavior ... ok
test fork_ext::addons::package::tests::forge_package_private_shared_precedence_and_init_fallback ... ok
test fork_ext::addons::package::tests::forge_package_registered_host_submodules_precedence_and_ffi_lockdown ... ok
test fork_ext::addons::package::tests::forge_package_reserved_host_precedence_unsupported_reserved_ffi_bytecode_and_path_rejection ... ok
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::package::tests::forge_package_resource_precedence_and_containment ... ok
test fork_ext::addons::package::tests::forge_package_return_overwrites_package_loaded_and_explicit_nil_preserves_nil ... ok
test fork_ext::addons::package::tests::forge_package_two_addons_requiring_same_config_independently ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 1000 filtered out; finished in 0.12s
```

#### Full Addon Module Suite (Post-Remediation with Pinned GPU Fixture)
Executed serially with the command-local pinned GPU fixture reference:
Command:
```powershell
$env:EQOA_FORGE_SPIKE = "C:\Users\hocka\Github Projects\EQOA Native Rust\run\forge-compat-reference\scripts\screen_grid.lua"
cargo test -p eqoa --manifest-path Cargo.toml fork_ext::addons:: -- --nocapture --test-threads=1
```
Output:
```text
running 35 tests
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::discovery::tests::addon_discovery_missing_root_returns_diagnostic ... ok
test fork_ext::addons::discovery::tests::addon_discovery_dangling_child_scripts_directory_emits_diagnostic ... ok
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::bridge::tests::forge_bridge_renders_real_lines_preserves_returns_and_rejects_expired_handles ... ok
test fork_ext::addons::discovery::tests::addon_discovery_legacy_names_with_spaces_and_unicode_preserved ... ok
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::discovery::tests::addon_discovery_dangling_and_escaping_support_dirs_emit_diagnostics ... ok
test fork_ext::addons::discovery::tests::addon_discovery_duplicate_ids_excluded ... ok
test fork_ext::addons::discovery::tests::addon_discovery_cross_scriptroot_symlink_stays_excluded ... ok
test fork_ext::addons::discovery::tests::addon_discovery_hidden_and_support_dirs_skipped ... ok
test fork_ext::addons::discovery::tests::addon_discovery_native_manifest_success_and_companion_skip ... ok
test fork_ext::addons::discovery::tests::addon_discovery_manifest_diagnostics_and_path_traversal ... ok
test fork_ext::addons::discovery::tests::addon_discovery_child_scripts_directory_imported ... ok
test fork_ext::addons::discovery::tests::addon_discovery_loose_file_precedence_and_shared_dirs ... ok
test fork_ext::addons::discovery::tests::addon_discovery_manifest_schema_type_and_absolute_path_failures ... ok
[forge_gpu_test] captured frame saved to target\forge-spike-grid.png
test fork_ext::addons::gpu_test::forge_gpu_pinned_grid_and_texture_fixture ... ok
test fork_ext::addons::host::tests::forge_host_keeps_globals_and_replaces_callbacks ... ok
test fork_ext::addons::host::tests::forge_host_limits_memory_and_rejects_native_modules ... ok
test fork_ext::addons::host::tests::forge_host_stops_runaway_code_without_poisoning_next_call ... ok
test fork_ext::addons::package::tests::forge_package_cached_mutable_module_survives_frames ... ok
test fork_ext::addons::package::tests::forge_package_cycles_and_error_retry ... ok
test fork_ext::addons::package::tests::forge_package_hostile_metatable_does_not_poison_loading_stack ... ok
test fork_ext::addons::package::tests::forge_package_nested_module_runaway_stops_original_budget ... ok
test fork_ext::addons::package::tests::forge_package_nil_and_false_reload_standard_behavior ... ok
test fork_ext::addons::package::tests::forge_package_private_shared_precedence_and_init_fallback ... ok
test fork_ext::addons::package::tests::forge_package_registered_host_submodules_precedence_and_ffi_lockdown ... ok
test fork_ext::addons::package::tests::forge_package_reserved_host_precedence_unsupported_reserved_ffi_bytecode_and_path_rejection ... ok
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::package::tests::forge_package_resource_precedence_and_containment ... ok
test fork_ext::addons::package::tests::forge_package_return_overwrites_package_loaded_and_explicit_nil_preserves_nil ... ok
test fork_ext::addons::package::tests::forge_package_two_addons_requiring_same_config_independently ... ok
test fork_ext::addons::profile::tests::forge_profile_roundtrips_numeric_keys_and_rejects_cycles_and_functions ... ok
test fork_ext::addons::texture_tests::forge_texture_manifest_roundtrip_and_relative_path_containment ... ok
test fork_ext::addons::texture_tests::forge_texture_png_dimensions_and_containment ... ok
test fork_ext::addons::texture_tests::forge_texture_unsupported_format_and_corrupt_payload_diagnostics ... ok
test fork_ext::addons::texture_tests::forge_texture_virtual_allocation_and_index_recycling ... ok
test fork_ext::addons::texture_tests::forge_texture_write_rgba_stride_and_bounds_validation ... ok
test fork_ext::addons::texture_tests::forge_texture_zero_dimension_and_oversized_allocation_rejected ... ok

test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 976 filtered out; finished in 3.97s
```

All 35 tests pass. Exact skips: 4 Windows symlink privilege skips (error 1314) and 0 fixture skips.

---

## 4. Environment Notes & Verified Limits

1. **Windows 1314 Symlink Privileges**:
   On non-elevated Windows environments (`ERROR_PRIVILEGE_NOT_HELD` / os error 1314), directory symlinks cannot be created by normal user processes. 4 symlink tests in the suite explicitly detect this error and log a graceful skip. Because these tests skip locally, directory symlink containment behavior is not claimed to be runtime-verified in this local environment.
2. **Constrained Spike Invariants Retained**:
   - `pcall` and `xpcall` remain nil in the Lua global environment for this stage to guarantee instruction quotas cannot be caught and bypassed by untrusted scripts. Broader compatibility is scheduled for later gates.
   - No general filesystem I/O (`io.*`) or OS process libraries are exposed to Lua.
   - Resource resolver is currently exposed only as a native Rust API; no Lua texture decoding or raw asset loading is exposed.
   - No `profiles_path` is exposed until an owned configuration directory manager is implemented.
   - Legacy opaque IDs are preserved and never joined directly into directory path strings.
   - Native minimap and world map remain active defaults; no map or renderer paths were touched.
