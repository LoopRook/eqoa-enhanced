# Forge Host Standard Library & Sandbox Hardening Report

**Date:** 2026-10-29
**Status:** Completed & Pinned Verified
**Scope Boundary:** Owned active `crates/eqoa/src/fork_ext/addons/{host.rs, package.rs, bit.rs, mod.rs}` and `docs/FORGE-STDLIB-REPORT.md` ONLY. No bridge, health, runtime, shared, maps, dependencies, policy, or ledger changes.

---

## 1. Executive Summary

Under the authorized `BUILD NOW` directive, the Host standard library hardening and native `bit` module integration was compiled, verified via TDD (RED $\to$ GREEN), validated through real controlled negative oracle execution, and confirmed across the entire pinned serial addons test suite (99 passed; 0 failed).

Key architecture highlights:
- **Native `bit` Module (8 Scoped Functions)**: `tobit`, `bnot`, `band`, `bor`, `bxor`, `lshift`, `rshift`, and `arshift` implemented with IEEE-754 mantissa-shift float bias normalization ($2^{52} + 2^{51} = 6755399441055744.0$), string coercion, minimum arity checks, shift count masking (`& 31`), and safe non-finite mapping (`!val.is_finite() => 0`).
- **Protected `pcall` Wrapper**: Retains original Lua 5.1 C closure while enforcing instruction budget escape prevention. The instruction hook sets a per-host `fatal` flag upon budget exhaustion, and the wrapper immediately rethrows, preventing Lua scripts from trapping the error. Error object identity and arbitrary multiple returns (including `nil`) are preserved for ordinary application errors.
- **Top-Level Boundary Isolation**: The `fatal` flag is reset *only* at fresh top-level entrypoints (`Host::run` and `Host::callback`) when `remaining` is replenished to 200,000; it is never reset inside `pcall` or `require`.
- **Whitelist `clean_os`**: Exposes genuine native C closures `clock` and `date` from vendored `loslib.c`. Dangerous functions (`execute`, `exit`, `remove`, `rename`, `getenv`, `tmpname`) are purged from both `_G.os` and `package.loaded["os"]`.
- **Root Coroutine Correction**: Explicitly stripped `coroutine` from `_G` and `package.loaded` (correcting for Lua 5.1 `luaopen_base` always opening `coroutine`), and blocked disk `require("coroutine")`.
- **Anti-Shadowing Precedence**: `require("bit")` and `require("os")` take absolute precedence over disk lookup in `package.rs`, ensuring private addon files cannot shadow native modules.
- **Clean Architecture**: Refactored `create_base_host()` to return a private `BaseHost` struct instead of an unwieldy tuple.

---

## 2. Baseline Source Checksums

Verified against untouched baseline before applying patches:
- `crates/eqoa/src/fork_ext/addons/host.rs`: `BCA8494B266C335C832C8F03AD0D7C35ED041FD9E9F8A897CC6BB2C270EDB500`
- `crates/eqoa/src/fork_ext/addons/package.rs`: `91B61F8C518B65046A08B039B39E921FC1C5687BF87756531CCC1FC8A44521FF`
- `crates/eqoa/src/fork_ext/addons/mod.rs`: `81C7E991EE8B45BD82701D9DCD1871C67E7D9E0BDD0C30DEBC7DEBFDB8BE4D91`

---

## 3. RED Phase Verification

Applied `host-stdlib-red.patch` and executed focused tests with `--locked --nocapture`:

### A. Host Suite (`cargo test --locked -p eqoa fork_ext::addons::host -- --nocapture`)
- **Result**: `4 passed; 4 failed; 0 ignored` (0.07s).
- **Legitimate RED Failures**:
  - `forge_host_clean_os_clock_date_and_blocks_unsafe_functions`: **FAILED** (`attempt to index global 'os' (a nil value)`).
  - `forge_host_coroutine_and_dangerous_libraries_strictly_absent`: **FAILED** (assertion failed: `assert(coroutine == nil)` because Lua 5.1 `luaopen_base` opened `coroutine`).
  - `forge_host_pcall_cannot_swallow_budget_exhaustion_finite_control_and_recovers`: **FAILED** (`attempt to call global 'pcall' (a nil value)`).
  - `forge_host_pcall_catches_ordinary_errors_and_preserves_multireturns_and_identity`: **FAILED** (`attempt to call global 'pcall' (a nil value)`).
- **Baseline Controls Passing**:
  - `forge_host_keeps_globals_and_replaces_callbacks`: ok
  - `forge_host_stops_runaway_code_without_poisoning_next_call`: ok
  - `forge_host_limits_memory_and_rejects_native_modules`: ok
  - `forge_host_callback_top_boundary_fatal_reset_and_recovery`: ok

### B. Package Suite (`cargo test --locked -p eqoa fork_ext::addons::package -- --nocapture`)
- **Result**: `12 passed; 2 failed; 0 ignored` (0.13s).
- **Legitimate RED Failures**:
  - `forge_package_require_bit_and_os_cannot_be_shadowed_by_disk`: **FAILED** (assertion failed: `matches!(b.get::<Value>("band").unwrap(), Value::Function(_))` because private `modules/bit.lua` shadowed the module).
  - `forge_package_table_identity_and_per_host_isolation`: **FAILED** (`module 'bit' not found in addon module search paths`).
- **Baseline Controls Passing**: All 12 prior tests passed ok.

---

## 4. GREEN Phase Implementation & Verification

Applied `host-stdlib-green.patch` on top of RED state. All tests passed cleanly:

### A. Host Suite (`cargo test --locked -p eqoa fork_ext::addons::host -- --nocapture`)
- **Result**: `8 passed; 0 failed; 0 ignored` (0.03s). All 8 tests passed GREEN.

### B. Package Suite (`cargo test --locked -p eqoa fork_ext::addons::package -- --nocapture`)
- **Result**: `14 passed; 0 failed; 0 ignored` (0.06s). All 14 tests passed GREEN.

### C. Native Bit Suite (`cargo test --locked -p eqoa fork_ext::addons::bit -- --nocapture`)
- **Result**: `6 passed; 0 failed; 0 ignored` (0.00s). All 6 unit tests passed GREEN:
  - `test_to_i32_direct_boundary_and_rounding`: ok
  - `test_module_table_isolation`: ok
  - `test_lua_surface_variadics_and_extra_arguments`: ok
  - `test_lua_surface_operations_and_multireturns`: ok
  - `test_lua_surface_string_numeric_coercion_and_rounding`: ok
  - `test_lua_surface_zero_args_and_type_errors`: ok

---

## 5. Controlled Negative Oracle Execution

To prove that the `pcall` wrapper actively defends against instruction budget swallowing (and is not trivially satisfied by the top-level boundary check alone), a real negative modification was compiled and executed:

1. **Modification**: Disabled only the wrapper's post-fatal check in `crates/eqoa/src/fork_ext/addons/host.rs:146-148`:
   ```rust
   let call_res = orig_pcall.call::<mlua::MultiValue>(args);
   // DISABLED FOR CONTROLLED NEGATIVE:
   // if fatal_pcall.get() { return Err(mlua::Error::runtime("Forge instruction budget exceeded")); }
   call_res
   ```
2. **Execution**:
   ```powershell
   cargo test --locked -p eqoa forge_host_pcall_cannot_swallow_budget_exhaustion_finite_control_and_recovers -- --nocapture
   ```
3. **Observed Failure**:
   ```text
   thread 'fork_ext::addons::host::tests::forge_host_pcall_cannot_swallow_budget_exhaustion_finite_control_and_recovers' panicked at crates\eqoa\src\fork_ext\addons\host.rs:420:9:
   assertion `left == right` failed
     left: Some(true)
    right: Some(false)
   test fork_ext::addons::host::tests::forge_host_pcall_cannot_swallow_budget_exhaustion_finite_control_and_recovers ... FAILED
   ```
4. **Conclusion**: When the wrapper check is bypassed, the inner `pcall` traps the quota error and resumes execution, setting `outer_marker = true`. With the wrapper active, the error is immediately rethrown across nested levels, keeping `outer_marker == false`.
5. **Restoration**: Production source was restored to its exact verified state and confirmed GREEN (`1 passed; 0 failed`).

---

## 6. Pinned Serial Addons Test Suite

Executed the full serial addons test suite with command-local `EQOA_FORGE_SPIKE`:
```cmd
set EQOA_FORGE_SPIKE=C:\Users\hocka\Github Projects\EQOA Native Rust\run\forge-compat-reference\scripts\screen_grid.lua&& cargo test --locked -p eqoa fork_ext::addons -- --test-threads=1 --nocapture
```

### Results:
- **Suite Summary**: `ok. 99 passed; 0 failed; 0 ignored; 0 measured; 987 filtered out; finished in 15.01s`
- **Prerequisite Skips Captured**:
  - `SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))` in:
    - `addon_discovery_cross_scriptroot_symlink_stays_excluded`
    - `addon_discovery_dangling_and_escaping_support_dirs_emit_diagnostics`
    - `addon_discovery_dangling_child_scripts_directory_emits_diagnostic`
    - `forge_package_resource_precedence_and_containment`
  - (These 4 tests skip cleanly on non-elevated Windows environments by design).
- **GPU & Spike Tests**: `forge_gpu_pinned_grid_and_texture_fixture` and `forge_manager_unchanged_pinned_hearts_real_png_lifecycle_and_hp_pixels` ran to completion without skip under the local `EQOA_FORGE_SPIKE` path.
- **Prior Pass Count Preservation**: All prior 85 tests remained GREEN alongside the 14 new/extended tests (total 99 passed).

---

## 7. Formatting & Build Slot Release

- `rustfmt --edition 2024` executed on all touched files: `host.rs`, `package.rs`, `bit.rs`, `mod.rs`.
- `git diff` verified: zero unintended edits or dependency alterations.
- **Cargo/Compiler Slot**: Formally released. Worker standing by and idle.
