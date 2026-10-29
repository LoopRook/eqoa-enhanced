# Forge Addon UI PNG Texture Resources (Stage 2C) Report

**Date**: 2026-10-28
**Scope**: Stage 2C native UI-addon PNG file texture loading and GPU resource management ([`crates/eqoa/src/fork_ext/addons/textures.rs`](../crates/eqoa/src/fork_ext/addons/textures.rs)). Bundled UI images only; standalone PCSX2 replacement texture packs remain deferred and untouched.

---

## 1. Executive Summary

Stage 2C delivers native file-backed PNG texture loading for Forge UI addons. It extends the existing memory texture subsystem to load bundled UI graphics with strict containment, memory budgeting, and safe GPU resource lifecycle management.

Key capabilities delivered:
- **Addon Texture Installation ([`Textures::install_for_addon`](../crates/eqoa/src/fork_ext/addons/textures.rs#L249-L393))**:
  - Configures the addon texture bank with extended budget caps: max 2048 dimension (clamped to GPU hardware limits), 128 handles, and 64 MiB total decoded pixel memory.
  - Exposes `UiForge.LoadTexture(path)` and legacy alias `UiForge.IGraphicsApi.CreateTextureFromFile(path)`.
  - Exposes `UiForge.resources_path` pointing to the shared `<script_root>/resources` directory (no profiles path exposed).
  - Preserves existing `CreateTextureFromMemory` and `ReleaseTexture` functions.
- **Dual-Mode Path Resolution**:
  - **Relative paths**: Resolved using [`package::resolve_resource`](../crates/eqoa/src/fork_ext/addons/package.rs#L34-L88), prioritizing private package resources ahead of shared script-root resources.
  - **Legacy absolute paths**: e.g. `UiForge.resources_path .. "\\retro_health_hearts\\heart.png"`. Allowed only when the canonical file lies inside a discovered `resource_dir` AND the active `script_root` (rechecked on every load). Normalizes `\` to `/`, rejects `..` parent traversal prior to canonicalization, and rejects drive letters or UNC paths outside allowed roots. Absolute paths pointing directly to shared resources do not prefer private files.
- **Strict PNG Decoding & Normalization**:
  - Decoded strictly via existing `png 0.18.1` crate dependency.
  - Reads bounded to 16 MiB (`Read::take(16 * 1024 * 1024 + 1)`).
  - Inspects header dimensions and verifies byte arithmetic before allocation; rejects zero dimensions, dimensions exceeding bank caps, and byte overflow.
  - Color conversion: accurately transforms RGBA8, RGB8 (alpha set to 255), Grayscale, Grayscale+Alpha, and Indexed palette images into standard 32-bit RGBA8.
  - Unsupported formats (e.g. JPEG) and corrupt files return `nil` in Lua with bounded diagnostics (matching upstream UiForge `LoadTexture` returning `nullptr`), avoiding fake or broken handles.
- **GPU Handle Management & Accounting**:
  - Consolidates GPU upload between memory and file paths (`upload_rgba8`).
  - Opaque owner-scoped handles prevent cross-host handle confusion.
  - Tracks allocated decoded bytes, counting released but pending textures until `collect`.
  - `collect(false)` unregisters dead textures and reclaims byte accounting; `collect(true)` cleans all handles and resets accounting to zero.
  - Bounded diagnostic buffer: capped at 32 entries, deduplicated to prevent per-frame spam; exposed via `drain_diagnostics`.

---

## 2. Architecture & Data Structures

```rust
#[derive(Clone, Copy, Debug)]
pub struct TexturesBudget {
    pub max_dimension: u32,
    pub max_count: usize,
    pub max_total_bytes: usize,
}

pub const ADDON_BUDGET: TexturesBudget = TexturesBudget {
    max_dimension: 2048,
    max_count: 128,
    max_total_bytes: 64 * 1024 * 1024, // 64 MiB
};

impl Textures {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, renderer: Rc<RefCell<WgpuRenderer>>) -> Rc<RefCell<Self>>;
    pub fn install(owner: &Rc<RefCell<Self>>, lua: &Lua) -> Result<()>;
    pub fn install_for_addon(owner: &Rc<RefCell<Self>>, lua: &Lua, addon: &DiscoveredAddon) -> Result<()>;
    pub fn set_budget(&mut self, budget: TexturesBudget);
    pub fn add_diagnostic(&mut self, msg: String);
    pub fn drain_diagnostics(&mut self) -> Vec<String>;
    pub fn raw(handle: &AnyUserData) -> Result<sys::ImTextureRef_c>;
    pub fn collect(&mut self, all: bool);
}
```

---

## 3. Test Verification (Red / Green)

### RED Baseline
All 6 `forge_texture_` tests initially failed with planned panics:
```text
running 6 tests
test fork_ext::addons::textures::tests::forge_texture_cumulative_budget_and_release_cleanup ... FAILED
test fork_ext::addons::textures::tests::forge_texture_explicit_shared_absolute_path_alias ... FAILED
test fork_ext::addons::textures::tests::forge_texture_grayscale_rgb_and_palette_conversion_and_dimension_limits ... FAILED
test fork_ext::addons::textures::tests::forge_texture_nil_and_diagnostics_on_missing_corrupt_unsupported_and_traversal ... FAILED
test fork_ext::addons::textures::tests::forge_texture_private_and_shared_png_distinct_colors ... FAILED
test fork_ext::addons::textures::tests::forge_texture_wgpu_headless_render_and_pixel_readback ... FAILED

test result: FAILED. 0 passed; 6 failed; 0 ignored; 0 measured; 1008 filtered out; finished in 0.00s
```

### GREEN State (Focused Texture Suite)
Command:
```powershell
$env:PATH = "C:\Users\hocka\.cargo\bin;$env:PATH"
cargo test --locked -p eqoa forge_texture_ -- --nocapture --test-threads=1
```
Output:
```text
running 6 tests
test fork_ext::addons::textures::tests::forge_texture_cumulative_budget_and_release_cleanup ... ok
test fork_ext::addons::textures::tests::forge_texture_explicit_shared_absolute_path_alias ... ok
test fork_ext::addons::textures::tests::forge_texture_grayscale_rgb_and_palette_conversion_and_dimension_limits ... ok
test fork_ext::addons::textures::tests::forge_texture_nil_and_diagnostics_on_missing_corrupt_unsupported_and_traversal ... ok
test fork_ext::addons::textures::tests::forge_texture_private_and_shared_png_distinct_colors ... ok
test fork_ext::addons::textures::tests::forge_texture_wgpu_headless_render_and_pixel_readback ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 1008 filtered out; finished in 2.96s
```

### Stage 2C Remediation Verification (Red / Green)

Following the independent review findings ([HIGH-01], [MED-01], [MED-02], and [LOW-01]), 6 targeted regression tests and strengthened color assertions were integrated into [`crates/eqoa/src/fork_ext/addons/textures.rs`](../crates/eqoa/src/fork_ext/addons/textures.rs):
- `forge_texture_diagnostics_dedup_full_ring_preserves_all`
- `forge_texture_userdata_creation_failure_rolls_back_gpu_and_budget`
- `forge_texture_memory_userdata_failure_rolls_back_gpu_and_budget`
- `forge_texture_byte_memory_budget_exhaustion_separate_from_count`
- `forge_texture_id_counter_exhaustion_rejects_without_leak`
- `forge_texture_platform_path_normalization_preserves_windows_verbatim_and_normalizes_unix`
- `forge_texture_explicit_shared_absolute_path_alias` (strengthened with WGPU headless rendering and framebuffer pixel readback)

#### RED Baseline (4 tests failing against un-remediated code)
Command:
```powershell
cargo test --locked -p eqoa --manifest-path Cargo.toml forge_texture_ -- --nocapture --test-threads=1
```
Output:
```text
running 12 tests
test fork_ext::addons::textures::tests::forge_texture_byte_memory_budget_exhaustion_separate_from_count ... ok
test fork_ext::addons::textures::tests::forge_texture_cumulative_budget_and_release_cleanup ... ok
test fork_ext::addons::textures::tests::forge_texture_diagnostics_dedup_full_ring_preserves_all ... FAILED
test fork_ext::addons::textures::tests::forge_texture_explicit_shared_absolute_path_alias ... FAILED
test fork_ext::addons::textures::tests::forge_texture_grayscale_rgb_and_palette_conversion_and_dimension_limits ... ok
test fork_ext::addons::textures::tests::forge_texture_id_counter_exhaustion_rejects_without_leak ... FAILED
test fork_ext::addons::textures::tests::forge_texture_memory_userdata_failure_rolls_back_gpu_and_budget ... ok
test fork_ext::addons::textures::tests::forge_texture_nil_and_diagnostics_on_missing_corrupt_unsupported_and_traversal ... ok
test fork_ext::addons::textures::tests::forge_texture_platform_path_normalization_preserves_windows_verbatim_and_normalizes_unix ... FAILED
test fork_ext::addons::textures::tests::forge_texture_private_and_shared_png_distinct_colors ... ok
test fork_ext::addons::textures::tests::forge_texture_userdata_creation_failure_rolls_back_gpu_and_budget ... ok
test fork_ext::addons::textures::tests::forge_texture_wgpu_headless_render_and_pixel_readback ... ok

failures:
    fork_ext::addons::textures::tests::forge_texture_diagnostics_dedup_full_ring_preserves_all
    fork_ext::addons::textures::tests::forge_texture_explicit_shared_absolute_path_alias
    fork_ext::addons::textures::tests::forge_texture_id_counter_exhaustion_rejects_without_leak
    fork_ext::addons::textures::tests::forge_texture_platform_path_normalization_preserves_windows_verbatim_and_normalizes_unix

test result: FAILED. 8 passed; 4 failed; 0 ignored; 0 measured; 1011 filtered out; finished in 6.40s
```

> [!NOTE]
> **RED Baseline Coverage Caveats & Analysis**:
> 1. **Shared Absolute Path Alias RED Failure**: The initial RED failure in `forge_texture_explicit_shared_absolute_path_alias` was caused by fixture framebuffer dimensions (64x64), where ImGui's default window titlebar and borders clipped the 16x16 image quad, leaving only titlebar pixels. Adjusting the framebuffer to 320x240 allowed the image to render. Windows `canonicalize` natively accepts `\` separators, so this test on Windows does not directly prove the Unix path bug; the Unix backslash separator defect is verified deterministically by `forge_texture_platform_path_normalization_preserves_windows_verbatim_and_normalizes_unix`.
> 2. **Early OOM vs. Post-Upload Rollback Coverage**: Both `forge_texture_userdata_creation_failure_rolls_back_gpu_and_budget` and `forge_texture_memory_userdata_failure_rolls_back_gpu_and_budget` passed in the RED run (8 passed / 4 failed). Setting `set_memory_limit` prior to `lua.load(...).eval()` caused chunk compilation/evaluation to fail *before* texture upload occurred, verifying that early Lua failures do not allocate textures, but leaving the post-upload `lua.create_userdata` failure rollback branch unexercised in production Lua execution. Deterministic post-upload failure coverage has been prepared via an injection seam in `.superpowers/sdd/2026-10-28-forge-compatibility/texture-remediation/oom-coverage/`.

#### GREEN State (Remediated Texture Suite - 13 Tests with Post-Upload Seam)
Command:
```powershell
cargo test --locked -p eqoa --manifest-path Cargo.toml forge_texture_ -- --nocapture --test-threads=1
```
Output:
```text
running 13 tests
test fork_ext::addons::textures::tests::forge_texture_byte_memory_budget_exhaustion_separate_from_count ... ok
test fork_ext::addons::textures::tests::forge_texture_cumulative_budget_and_release_cleanup ... ok
test fork_ext::addons::textures::tests::forge_texture_diagnostics_dedup_full_ring_preserves_all ... ok
test fork_ext::addons::textures::tests::forge_texture_early_memory_limit_rejects_before_upload ... ok
test fork_ext::addons::textures::tests::forge_texture_explicit_shared_absolute_path_alias ... ok
test fork_ext::addons::textures::tests::forge_texture_grayscale_rgb_and_palette_conversion_and_dimension_limits ... ok
test fork_ext::addons::textures::tests::forge_texture_id_counter_exhaustion_rejects_without_leak ... ok
test fork_ext::addons::textures::tests::forge_texture_memory_early_memory_limit_rejects_before_upload ... ok
test fork_ext::addons::textures::tests::forge_texture_nil_and_diagnostics_on_missing_corrupt_unsupported_and_traversal ... ok
test fork_ext::addons::textures::tests::forge_texture_platform_path_normalization_preserves_windows_verbatim_and_normalizes_unix ... ok
test fork_ext::addons::textures::tests::forge_texture_post_upload_userdata_failure_rolls_back_gpu_and_budget ... [oracle] Real allocator memory quota failure triggered: MemoryError("not enough memory")
ok
test fork_ext::addons::textures::tests::forge_texture_private_and_shared_png_distinct_colors ... ok
test fork_ext::addons::textures::tests::forge_texture_wgpu_headless_render_and_pixel_readback ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 1021 filtered out; finished in 6.16s
```

#### Controlled Negative Oracle Verification
To conclusively verify that `forge_texture_post_upload_userdata_failure_rolls_back_gpu_and_budget` validates the post-upload rollback branch and would fail without it, `bank.rollback_upload(id)` was temporarily commented out in `upload_and_create_userdata_with`.
- **Negative Control Run**:
  `test forge_texture_post_upload_userdata_failure_rolls_back_gpu_and_budget ... FAILED`
  `assertion left == right failed: post-upload userdata failure must roll back bank entries: left: 1, right: 0`
- **Restoration**: `bank.rollback_upload(id)` was restored, and the test returned cleanly to GREEN.

### Full Serial Addons Suite (Post-Remediation with Stage 2D Suite and Pinned GPU Fixture)
Command:
```powershell
$env:PATH = "C:\Users\hocka\.cargo\bin;$env:PATH"
$env:EQOA_FORGE_SPIKE = "C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua"
cargo test --locked -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1
```
Output:
```text
running 52 tests
test fork_ext::addons::bridge::tests::forge_bridge_renders_real_lines_preserves_returns_and_rejects_expired_handles ... ok
test fork_ext::addons::discovery::tests::addon_discovery_child_scripts_directory_imported ... ok
test fork_ext::addons::discovery::tests::addon_discovery_cross_scriptroot_symlink_stays_excluded ... SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
ok
test fork_ext::addons::discovery::tests::addon_discovery_dangling_and_escaping_support_dirs_emit_diagnostics ... SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
ok
test fork_ext::addons::discovery::tests::addon_discovery_dangling_child_scripts_directory_emits_diagnostic ... SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
ok
test fork_ext::addons::discovery::tests::addon_discovery_duplicate_ids_excluded ... ok
test fork_ext::addons::discovery::tests::addon_discovery_hidden_and_support_dirs_skipped ... ok
test fork_ext::addons::discovery::tests::addon_discovery_legacy_names_with_spaces_and_unicode_preserved ... ok
test fork_ext::addons::discovery::tests::addon_discovery_loose_file_precedence_and_shared_dirs ... ok
test fork_ext::addons::discovery::tests::addon_discovery_manifest_diagnostics_and_path_traversal ... ok
test fork_ext::addons::discovery::tests::addon_discovery_manifest_schema_type_and_absolute_path_failures ... ok
test fork_ext::addons::discovery::tests::addon_discovery_missing_root_returns_diagnostic ... ok
test fork_ext::addons::discovery::tests::addon_discovery_native_manifest_success_and_companion_skip ... ok
test fork_ext::addons::gpu_test::forge_gpu_pinned_grid_and_texture_fixture ... Forge GPU fixture captured C:\Users\hocka\.paseo\worktrees\1usktejc\forge-compatibility\crates\eqoa\../../target/forge-spike-grid.png
ok
test fork_ext::addons::host::tests::forge_host_keeps_globals_and_replaces_callbacks ... ok
test fork_ext::addons::host::tests::forge_host_limits_memory_and_rejects_native_modules ... ok
test fork_ext::addons::host::tests::forge_host_stops_runaway_code_without_poisoning_next_call ... ok
test fork_ext::addons::manager::tests::forge_manager_callback_commands_observe_snapshot_and_retire_disabled_and_eject_resources ... ok
test fork_ext::addons::manager::tests::forge_manager_disabled_defaults_missing_root_and_sixteen_vm_cap ... ok
test fork_ext::addons::manager::tests::forge_manager_gpu_failure_isolation_and_after_render_retirement ... ok
test fork_ext::addons::manager::tests::forge_manager_order_private_modules_toggle_globals_callbacks_and_persisted_false ... ok
test fork_ext::addons::manager::tests::forge_manager_reload_compile_failure_keeps_old_vm_and_rescan_retires_only_removed ... ok
test fork_ext::addons::package::tests::forge_package_cached_mutable_module_survives_frames ... ok
test fork_ext::addons::package::tests::forge_package_cycles_and_error_retry ... ok
test fork_ext::addons::package::tests::forge_package_hostile_metatable_does_not_poison_loading_stack ... ok
test fork_ext::addons::package::tests::forge_package_nested_module_runaway_stops_original_budget ... ok
test fork_ext::addons::package::tests::forge_package_nil_and_false_reload_standard_behavior ... ok
test fork_ext::addons::package::tests::forge_package_private_shared_precedence_and_init_fallback ... ok
test fork_ext::addons::package::tests::forge_package_registered_host_submodules_precedence_and_ffi_lockdown ... ok
test fork_ext::addons::package::tests::forge_package_reserved_host_precedence_unsupported_reserved_ffi_bytecode_and_path_rejection ... ok
test fork_ext::addons::package::tests::forge_package_resource_precedence_and_containment ... SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
ok
test fork_ext::addons::package::tests::forge_package_return_overwrites_package_loaded_and_explicit_nil_preserves_nil ... ok
test fork_ext::addons::package::tests::forge_package_two_addons_requiring_same_config_independently ... ok
test fork_ext::addons::profile::tests::forge_profile_roundtrips_numeric_keys_and_rejects_cycles_and_functions ... ok
test fork_ext::addons::storage::tests::forge_storage_identity_is_safe_path_sensitive_and_length_delimited ... ok
test fork_ext::addons::storage::tests::forge_storage_invalid_save_and_failed_replace_preserve_last_good_target ... ok
test fork_ext::addons::storage::tests::forge_storage_numeric_profile_repeated_replace_and_scratch_isolation ... ok
test fork_ext::addons::storage::tests::forge_storage_rejects_corrupt_oversized_schema_and_unavailable_config ... ok
test fork_ext::addons::storage::tests::forge_storage_windows_rename_replaces_existing_without_destination_delete ... ok
test fork_ext::addons::textures::tests::forge_texture_byte_memory_budget_exhaustion_separate_from_count ... ok
test fork_ext::addons::textures::tests::forge_texture_cumulative_budget_and_release_cleanup ... ok
test fork_ext::addons::textures::tests::forge_texture_diagnostics_dedup_full_ring_preserves_all ... ok
test fork_ext::addons::textures::tests::forge_texture_early_memory_limit_rejects_before_upload ... ok
test fork_ext::addons::textures::tests::forge_texture_explicit_shared_absolute_path_alias ... ok
test fork_ext::addons::textures::tests::forge_texture_grayscale_rgb_and_palette_conversion_and_dimension_limits ... ok
test fork_ext::addons::textures::tests::forge_texture_id_counter_exhaustion_rejects_without_leak ... ok
test fork_ext::addons::textures::tests::forge_texture_memory_early_memory_limit_rejects_before_upload ... ok
test fork_ext::addons::textures::tests::forge_texture_nil_and_diagnostics_on_missing_corrupt_unsupported_and_traversal ... ok
test fork_ext::addons::textures::tests::forge_texture_platform_path_normalization_preserves_windows_verbatim_and_normalizes_unix ... ok
test fork_ext::addons::textures::tests::forge_texture_post_upload_userdata_failure_rolls_back_gpu_and_budget ... [oracle] Real allocator memory quota failure triggered: MemoryError("not enough memory")
ok
test fork_ext::addons::textures::tests::forge_texture_private_and_shared_png_distinct_colors ... ok
test fork_ext::addons::textures::tests::forge_texture_wgpu_headless_render_and_pixel_readback ... ok

test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 982 filtered out; finished in 9.32s
```

---

## 4. Limits & Boundary Invariants

- **Execution Serial Invariant**: Due to Dear ImGui thread-local/global context handling, addon test runs involving multiple ImGui contexts must run with `--test-threads=1`.
- **Pinned Spike Grid Fixture Active**: With `EQOA_FORGE_SPIKE` passed locally, `forge_gpu_pinned_grid_and_texture_fixture` ran to completion, verified center-line rendering, and wrote `target/forge-spike-grid.png`.
- **Windows Privilege 1314**: 4 symlink tests in the discovery and package suites continue to gracefully skip on non-elevated Windows environments. Symlink link behavior is not claimed runtime-verified on this host.
- **Frozen Stage 2B Code**: `host.rs`, `package.rs`, `discovery.rs`, `spike.rs`, `bridge.rs`, and `mod.rs` were left untouched, honoring the concurrent Stage 2B review boundaries.
- **Deferred Work**: Standalone PCSX2 replacement texture packs remain deferred; no profile config paths are exposed. Native minimap and world map remain active defaults.
