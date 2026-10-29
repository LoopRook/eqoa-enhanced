# Forge Sound Stage Report

## Status

**Completed (Fix round 1).** Bounded native Forge sound stage implemented using
real Windows MCI (`winmm`) parity, integrated with the addon instance lifecycle,
and covered by unit/oracle tests. Review feedback addressed for path
containment, negative IDs, old-bank cleanup oracle, diagnostic bounds, lossy
formatting, unused warnings, aggregate/close-retry coverage, and report
accuracy.

## Base / Head

- **Base:** `3ffe587` (`feat(addons): bounded native Forge sound stage via Windows MCI`)
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
  - `UnsupportedBackend` returns explicit diagnostics on non-Windows; no dummy
    production backend.
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
  - Resolved paths must live inside the addon script root, resource dirs, or
    the Windows system media directory.
  - Absolute-path containment uses component-wise `Path::starts_with` plus
    file checks; a prefix-trap file (`addonspoof.wav` next to `addon/`) is
    rejected.
  - Lua sound IDs now accept signed integers and reject negative, zero, and
    overflow values safely (return `false`/`nil`) instead of raising conversion
    errors.
  - Volume clamped to `[0.0, 1.0]`; non-finite volumes rejected.
  - Per-bank handle limit (`MAX_SOUND_HANDLES = 16`), per-file size limit
    (`MAX_SOUND_FILE_BYTES = 16 MiB`), and aggregate budget
    (`MAX_AGGREGATE_BYTES = 64 MiB`).
  - Failed closes retain records so limits cannot be bypassed by open/close
    churn; close retries are capped at `MAX_CLOSE_RETRIES = 3`.
- Lifecycle:
  - Banks start inactive; `set_active(true)` only while the instance is enabled.
  - Ordinary disable stops playback but preserves handles for re-enable.
  - Terminal events (retire, **successful replacement during reload**, eject,
    shutdown) call `close_all()`; a failed reload preserves the old bank.
  - `Manager::suspend_sound()` is wired into runtime pause/error paths alongside
    `suspend_health()`.
- Diagnostics:
  - Bounded ring (`MAX_DIAGNOSTICS = 32`) with deduplication.
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
test result: ok. 130 passed; 0 failed; 0 ignored; 0 measured; 987 filtered out; finished in 19.66s
```

**Saved log:** `C:\Users\hocka\AppData\Local\Temp\opencode\forge-addons-fix1.log`

### Skips

- 4 runtime skips from pre-existing symlink tests:
  `SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))`.
  These are unchanged from the base and unrelated to sound.

### New / updated tests

- 19 `forge_sound_*` tests (was 14):
  - `forge_sound_absolute_path_outside_roots_rejected`
  - `forge_sound_negative_zero_and_overflow_ids_rejected_in_lua`
  - `forge_sound_aggregate_budget_rejects_before_os_open`
  - `forge_sound_close_retry_limit_retains_record`
  - `forge_sound_diagnostics_ring_bounded_and_deduplicated`
  - Existing validation, sanitization, lifecycle, limits, close-failure
    retention, Lua binding contract, and recorder-based MCI command
    syntax/order tests.
- 3 `forge_manager_sound_*` tests (updated):
  - `forge_manager_sound_failed_reload_preserves_old_bank` now asserts the
    old bank is emptied after successful replacement.

### Real-MCI native run

**Command:**

```powershell
$env:EQOA_SOUND_NATIVE_REQUIRED='1'
$env:EQOA_FORGE_SPIKE='C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua'
cargo test --locked -p eqoa forge_sound_real_mci -- --nocapture --test-threads=1
```

**Result:** passed; no `SKIP` printed. The test executed real `open`, `play`,
`IsSoundPlaying` query (`status ... mode` returned `playing`), `SetSoundVolume`,
`StopSound`, second `IsSoundPlaying` query (returned stopped), and `release_sound`
(`close`) against the generated WAV.

**Saved log:** `C:\Users\hocka\AppData\Local\Temp\opencode\forge-sound-native-required.log`

### Other saved logs

- `C:\Users\hocka\AppData\Local\Temp\opencode\forge-sound-fix1.log` — all 19 sound tests.
- `C:\Users\hocka\AppData\Local\Temp\opencode\forge-manager-sound-fix1.log` — manager lifecycle tests.
- `C:\Users\hocka\AppData\Local\Temp\opencode\forge-sound-pre-fix.log` — pre-fix sound run (no failures; evidence preserved).

## Build Verification

- **Native build:** `cargo build --locked -p eqoa` — succeeded (exit 0).
  - Log: `C:\Users\hocka\AppData\Local\Temp\opencode\forge-native-build-fix1.log`
- **WASM check:** `cargo check --locked -p eqoa --target wasm32-unknown-unknown` — succeeded (exit 0).
  - Log: `C:\Users\hocka\AppData\Local\Temp\opencode\forge-wasm-check-fix1.log`
- **Linux:** not executed locally; code is gated with `#[cfg(windows)]` for
  Windows-specific tests/MCI and `#[cfg(not(windows))]` for the unsupported
  backend path.

## Concerns / Known Limitations

- The sound implementation is Windows-only. Non-Windows builds compile the
  `UnsupportedBackend` path and return `nil`/diagnostics for all sound calls.
- MCI device availability is environment-dependent; the real-MCI test will panic
  if `EQOA_SOUND_NATIVE_REQUIRED=1` is set and no device is present.
- `cargo build` still emits pre-existing dead-code warnings from other modules.
  Sound-module warnings are suppressed with `#![allow(dead_code)]` because the
  binary currently only exercises a subset of the API.
- `git diff --check` and `git diff --cached --check` passed. LF/CRLF warnings
  are present because core.autocrlf is converting LF files on this Windows
  checkout; no whitespace errors were reported.
