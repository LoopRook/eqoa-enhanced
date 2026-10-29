# Forge Sound Stage Report

## Status

**Completed.** Bounded native Forge sound stage implemented using real Windows
MCI (`winmm`) parity, integrated with the addon instance lifecycle, and covered
by unit/oracle tests.

## Base / Head

- **Base:** `5757646` (`docs(addons): finalize clean checkpoint and budget-stop handoff`)
- **Head:** local commit on `forge-compatibility` after this report (not pushed).

## Files Changed

- `crates/eqoa/src/fork_ext/addons/sound.rs` (new)
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
  - Volume clamped to `[0.0, 1.0]`; non-finite volumes rejected.
  - Per-bank handle limit (`MAX_SOUND_HANDLES = 16`), per-file size limit
    (`MAX_SOUND_FILE_BYTES = 16 MiB`), and aggregate budget
    (`MAX_AGGREGATE_BYTES = 64 MiB`).
  - Failed closes retain records so limits cannot be bypassed by open/close
    churn.
- Lifecycle:
  - Banks start inactive; `set_active(true)` only while the instance is enabled.
  - Ordinary disable stops playback but preserves handles for re-enable.
  - Terminal events (retire, failed reload, eject, shutdown) call `close_all()`.
  - `Manager::suspend_sound()` is wired into runtime pause/error paths alongside
    `suspend_health()`.
- Diagnostics drain into the existing manager diagnostic channel.

## Test Evidence

### Command

```powershell
$env:EQOA_FORGE_SPIKE='C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua'
cargo test --locked -p eqoa fork_ext::addons -- --nocapture --test-threads=1
```

### Result

```text
test result: ok. 125 passed; 0 failed; 0 ignored; 0 measured; 987 filtered out; finished in 19.14s
```

### Skips

- 4 runtime skips from pre-existing symlink tests:
  `SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))`.
  These are unchanged from the base and unrelated to sound.

### New Tests

- 14 `forge_sound_*` tests covering validation, sanitization, lifecycle, limits,
  close-failure retention, Lua binding contract, and recorder-based MCI command
  syntax/order assertions.
- 3 `forge_manager_sound_*` tests covering disable/enable handle retention,
  inactive-callback playback blocking, and failed-reload bank preservation.

## Build Verification

- `cargo build --locked -p eqoa` — succeeded.
- `cargo check --locked -p eqoa --target wasm32-unknown-unknown` — succeeded.

## Concerns / Known Limitations

- The sound implementation is Windows-only. Non-Windows builds compile the
  `UnsupportedBackend` path and return `nil`/diagnostics for all sound calls.
- MCI device availability is environment-dependent; the real-MCI test skips
  gracefully when no device is present.
- `cargo build` emits pre-existing dead-code warnings plus a handful of new
  dead-code warnings for `SoundBank` accessors that are currently only exercised
  in tests. These are harmless but should be cleaned up if the API stabilizes
  without external callers.
- `git diff --check` and `git diff --cached --check` passed. LF/CRLF warnings
  are present because core.autocrlf is converting LF files on this Windows
  checkout; no whitespace errors were reported.
