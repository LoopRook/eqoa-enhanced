# Forge Sound Stage Report

## Status

**Completed (Fix round 2).** Bounded native Forge sound stage with review
feedback incorporated: platform-gated tests, strict active-`script_root`
containment, signed Lua handle handling, deterministic native MCI playback
query, bounded diagnostics, and old-bank cleanup oracle.

## Base / Head

- **Base:** `ada9092` (`fix(addons): review round 1 for Forge sound stage`)
- **Head:** local commit on `forge-compatibility` after this report (not pushed).

## Files Changed

- `crates/eqoa/src/fork_ext/addons/sound.rs`
- `crates/eqoa/src/fork_ext/addons/manager.rs`
- `crates/eqoa/src/fork_ext/addons/runtime.rs`
- `crates/eqoa/src/fork_ext/addons/mod.rs`
- `docs/FORGE-SOUND-REPORT.md` (this file)
- `FORK-UPDATE-NOTES.md`

## What Was Implemented

- `SoundBank` per addon instance backed by `MciBackend`.
  - `WindowsMciBackend` uses `mciSendStringW` / `mciGetErrorStringW` on Windows.
  - `UnsupportedBackend` is never reached in production; non-Windows calls short-
    circuit in `load_sound` with `SoundDiagnostic::UnsupportedPlatform`.
- Lua API surface matching the pinned FrontiersForge reference:
  - `UiForge.LoadSound`
  - `UiForge.PlaySound`
  - `UiForge.StopSound`
  - `UiForge.IsSoundPlaying`
  - `UiForge.SetSoundVolume`
  - `UiForge.ReleaseSound`
- Safety and bounds:
  - Path sanitization rejects injection characters, nulls, control characters,
    parent traversal, and drive-relative paths.
  - **Containment:** resolved paths must be regular files inside the canonical
    active `script_root`, or inside a canonical resource directory that is itself
    inside `script_root`. The Windows system Media folder is the only external
    exception. Absolute paths are not widened by resource directories, and the
    relative branch no longer accepts a resource root that escapes `script_root`.
  - Lua sound IDs accept signed integers; negative, zero, and values outside the
    positive `i32` range return `false`/`nil` instead of raising Lua errors.
  - Volume clamped to `[0.0, 1.0]`; non-finite volumes rejected.
  - Per-bank handle limit (`MAX_SOUND_HANDLES = 16`), per-file size limit
    (`MAX_SOUND_FILE_BYTES = 16 MiB`), and aggregate budget
    (`MAX_AGGREGATE_BYTES = 64 MiB`).
  - Failed closes retain records so limits cannot be bypassed by open/close
    churn; close retries are capped at `MAX_CLOSE_RETRIES = 3`.
- Lifecycle:
  - Banks start inactive; `set_active(true)` only while the instance is enabled.
  - Ordinary disable stops playback but preserves handles for re-enable.
  - Terminal events (retire, **successful** reload, runtime failure, eject,
    shutdown) call `close_all()`; a **failed-compile** reload preserves the old
    bank/playing state.
  - `Manager::suspend_sound()` is wired into runtime pause/error paths alongside
    `suspend_health()`.
- Diagnostics:
  - Bounded ring (`MAX_DIAGNOSTICS = 32`) with deduplication.
  - Each diagnostic message capped at 2048 characters.
  - Stop, query, volume, and terminal close failures record actionable
    diagnostics.
  - Drains into the existing manager diagnostic channel.
- MCI command strings use the validated Unicode path string, avoiding lossy
  `Path::display()` for canonical paths.

## Test Evidence

### Full addon suite

**Command:**

```powershell
$env:EQOA_FORGE_SPIKE='C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua'
cargo test --locked -p eqoa fork_ext::addons -- --nocapture --test-threads=1
```

**Result:**

```text
test result: ok. 133 passed; 0 failed; 0 ignored; 0 measured; 987 filtered out; finished in 18.09s
```

### Skips

- 4 runtime skips from pre-existing symlink tests:
  `SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))`.
  These are unchanged from the base and unrelated to sound.

### New / updated tests

- 22 `forge_sound_*` tests (was 19):
  - `forge_sound_resource_root_outside_script_root_rejected` — deterministic
    regression for external resource directories without symlink privileges.
  - `forge_sound_absolute_path_outside_roots_rejected` — prefix-trap coverage.
  - `forge_sound_negative_zero_and_overflow_ids_rejected_in_lua`.
  - `forge_sound_aggregate_budget_rejects_before_os_open`.
  - `forge_sound_close_retry_limit_retains_record`.
  - `forge_sound_diagnostics_ring_bounded_and_deduplicated`.
  - `forge_sound_stop_query_volume_failure_records_diagnostic`.
  - `forge_sound_diagnostic_message_truncation`.
  - `forge_sound_load_returns_unsupported_platform_on_non_windows` (compiles
    only on non-Windows).
  - Existing validation, sanitization, lifecycle, limits, close-failure
    retention, Lua binding contract, and recorder-based MCI command
    syntax/order tests.
- 3 `forge_manager_sound_*` tests:
  - `forge_manager_sound_failed_reload_preserves_old_bank` now asserts the old
    bank is emptied after a successful replacement.

### RED evidence

- **Resource-root containment:** `forge_sound_resource_root_outside_script_root_rejected`
  was run against the buggy resolver (absolute branch allowed external resource
  dirs; relative branch did not require `script_root`). It failed; the log is
  preserved. After the containment fix the same test passes.
  - RED: `run/forge-sound-stage/forge-res-escape-red.log`
  - GREEN: `run/forge-sound-stage/forge-res-escape-green.log`
- **Historical RED for negative Lua IDs:** not preserved. The fix was made
  before capturing a failing log. This gap is recorded honestly below.

### Real-MCI native run

**Command:**

```powershell
$env:EQOA_SOUND_NATIVE_REQUIRED='1'
$env:EQOA_FORGE_SPIKE='C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua'
cargo test --locked -p eqoa forge_sound_real_mci -- --nocapture --test-threads=1
```

**Result:** passed; `waveOutGetNumDevs()` returned > 0 on this host, so the test
proceeded and executed real MCI operations against a 2-second looping generated
WAV: `open`, cached `load_sound`, `play` (looping), `status … mode` returned
`playing`, `setaudio volume`, `stop`, `status … mode` returned `stopped`, and
`release`/`close`. No `SKIP` was printed.

## Build Verification

- **Native build:** `cargo build --locked -p eqoa` — succeeded (exit 0).
- **WASM check:** `cargo check --locked -p eqoa --target wasm32-unknown-unknown`
  — succeeded (exit 0).
- **Linux:** not executed locally. Platform-specific tests are gated with
  `#[cfg(windows)]`; the non-Windows `load_sound` path returns
  `UnsupportedPlatform`. The `forge_sound_monotonic_id_exhaustion_guard`,
  `forge_sound_handle_and_file_size_limits`, and
  `forge_sound_aggregate_budget_rejects_before_os_open` tests are now
  `#[cfg(windows)]`.

## Fixed / Deferred Findings

| Finding | Status | Notes |
|---|---|---|
| C1 — Linux-breaking ungated tests | Fixed | Three platform-specific tests now `#[cfg(windows)]`; added non-Windows `UnsupportedPlatform` test. |
| I2 — Absolute-path resource-root symlink escape | Fixed | Absolute branch only allows `script_root` or system Media; resource dirs no longer widen it. |
| N2 — Relative branch resource-root escape | Fixed | Relative branch requires both `script_root` and canonical resource-dir containment. |
| I3 — Negative/invalid IDs raised Lua errors | Fixed | `parse_lua_sound_id` maps out-of-range signed values to safe no-ops. |
| I4 — Real-MCI masking / missing query | Fixed | `waveOutGetNumDevs` preflight, looping clip, `query_sound_mode` assertions; required-native mode cannot skip. |
| I5 — Vacuous reload-close assertion | Fixed | Old bank `len() == 0` and `is_empty()` asserted after successful replacement. |
| M6 — Stop/query/volume diagnostics | Fixed | Failures record `MciCommandFailed`. |
| M7 — Diagnostic message size | Fixed | Capped at 2048 chars. |
| M8 — Lossy `Path::display()` | Fixed | Uses validated `canonical_str`. |
| N4 — Silent terminal close retry | Fixed | Records bounded `MciCloseFailed` diagnostic. |
| M10 — Dead code | Fixed | Removed `SoundRecord.id`, `SoundId::as_i32`, `RecorderBackend::push_response`, `clear_diagnostic`, `with_backend`; test accessors gated `#[cfg(test)]`. |
| M9 — Resolver deduplication | Deferred | Per lead ruling; containment is enforced without unifying resolvers. |
| M12 — Per-VM handle namespace | Documented adaptation | Each addon has an isolated Lua state and bank; handle `1` is not shared across VMs. |
| RED→GREEN historical evidence | Not preserved | Only the resource-root RED was captured before fix; negative-ID RED was not. |

## Concerns / Known Limitations

- The sound implementation is Windows-only. Non-Windows builds return `nil` with
  an explicit `UnsupportedPlatform` diagnostic.
- MCI device availability is environment-dependent. The default native test skips
  only when `waveOutGetNumDevs() == 0`; `EQOA_SOUND_NATIVE_REQUIRED=1` turns any
  MCI error into a hard failure.
- If MCI itself fails to close an alias (e.g., device removed), the bank retains
  the counted record and makes a final silent attempt in `Drop`. Full OS resource
  reclamation cannot be guaranteed when the underlying MCI layer rejects the
  close command.
- Pre-existing dead-code warnings remain in unrelated modules.
- `git diff --check` and `git diff --cached --check` passed. LF/CRLF warnings
  are present because core.autocrlf is converting LF files on this Windows
  checkout; no whitespace errors were reported.

## Durable Log Paths

All logs are copied to:
`C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-sound-stage/`

- `forge-addons-fix2.log` — full pinned addon suite.
- `forge-sound-native-required-fix2.log` — focused required-native MCI run.
- `forge-native-build-fix2.log` — native build.
- `forge-wasm-check-fix2.log` — WASM check.
- `forge-res-escape-red.log` / `forge-res-escape-green.log` — containment RED.
- `forge-sound-fix2.log` — focused sound tests.
