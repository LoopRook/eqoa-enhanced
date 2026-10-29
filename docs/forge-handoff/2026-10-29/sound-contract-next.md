# UiForge Sound Contract: Windows MCI Architecture & Integration Specification

**Status**: Proposal / Implementation Prep (Read-Only Stage)
**Target Files**:
- Source Implementation: `target/forge-preview-prep/sound.rs`
- Specification & Review Log: `target/forge-preview-prep/sound-contract-next.md`
**Referenced Worktrees**:
- Forge Compatibility: `C:\Users\hocka\.paseo\worktrees\1usktejc\forge-compatibility`
- Pinned FrontiersForge Reference: `run/forge-compat-reference/UiForge/src/core/audio_manager.cpp` & `core.cpp`
- Pinned Script Fixture: `run/forge-compat-reference/scripts/uiforge_example/uiforge_example.lua`

---

## 1. Executive Summary & Root Review Corrections

Following Root's review of `audio_manager.cpp` and `sound.rs`, the Windows MCI implementation has been finalized with exact parity and defensive boundaries:

1. **Finite Volume Clamping vs Non-Finite Rejection**:
   - Matches C++ `ToMciVolume` by clamping finite values outside `[0.0, 1.0]` (e.g. `-0.5` $\rightarrow 0.0$, `1.5` $\rightarrow 1.0$).
   - Rejects non-finite values (`NaN`, `+Inf`, `-Inf`) with `SoundDiagnostic::InvalidVolume`, returning `false`.
   - Validated via `validate_volume` and tested on valid mock bank records as well as Lua scripts.
2. **Comprehensive Absolute and Relative Path Resolution**:
   - Relative paths are resolved against `resource_dirs` and must canonicalize strictly inside `script_root` (matching `package::resolve_resource` containment).
   - Absolute paths are accepted if they canonicalize to regular files within:
     1. The active `script_root`
     2. Any active directory in `resource_dirs` (e.g. legacy `UiForge.resources_path` absolute references)
     3. The system media directory (`C:\Windows\Media` / `%SystemRoot%\Media`)
   - Pre-sanitization and post-canonicalization sanitization strictly reject quotes (`"`), NUL (`\0`), control characters (`< 0x20`), parent traversal (`..`), and Windows drive-relative paths (`C:foo`).
   - Native Windows UTF-16 commands are encoded directly from wide OS strings (`OsStrExt::encode_wide`), avoiding lossy conversions.
3. **Private Test Constructor**:
   - `SoundBank::with_owner_id` is private and `#[cfg(test)]`-gated. Production code can only use `SoundBank::new()`, which allocates monotonic tokens via `AtomicU64::fetch_update`.
4. **Conservative Close Failure Retention**:
   - If an MCI `close` fails, the record is **retained** in the bank rather than silently dropped, ensuring open aliases remain counted against handle limits (`MAX_SOUND_HANDLES`) and aggregate memory bounds (`MAX_AGGREGATE_BYTES`).
   - Retries back off after `MAX_CLOSE_RETRIES` (3 attempts) until final terminal `Drop`.
   - Explicitly notes: native MCI close failure cannot guarantee OS device release (no fake cleanup claim).
5. **Real `mlua` Integration & Tests**:
   - Implemented `install_sound_bindings` matching the FrontiersForge C++ `UiForge` table.
   - Tested using `mlua` covering nil safety, arity, options tables with clamping, and security rejections.

---

## 2. API Contract & Lua Signatures

| Lua API Signature | Reference C++ Return | Native Rust Implementation | Nil-Safety & Behavior |
| :--- | :--- | :--- | :--- |
| `UiForge.LoadSound(path: string)` | `sol::optional<int>` | `Option<u32>` (`SoundId`) | Resolves, canonicalizes, checks bounds, caches path. Returns positive integer `sound_id`, or `nil` on missing/invalid file or non-Windows. |
| `UiForge.PlaySound(sound_id, options)` | `bool` | `bool` | Nil-safe (`sound_id == nil` returns `false`). Options: `{ volume = 1.0, loop = false }`. Clamps finite volume; rejects NaN/Inf. Seeks to start and plays. Replaying restarts sound. Returns `false` if addon is disabled. |
| `UiForge.StopSound(sound_id)` | `bool` | `bool` | Nil-safe (`sound_id == nil` returns `false`). Sends `stop <alias>`. |
| `UiForge.IsSoundPlaying(sound_id)` | `bool` | `bool` | Nil-safe (`sound_id == nil` returns `false`). Queries `status <alias> mode` == `"playing"`. |
| `UiForge.SetSoundVolume(sound_id, vol)`| `bool` | `bool` | Nil-safe (`sound_id == nil` returns `false`). Clamps finite `vol`; rejects NaN/Inf. Scales to MCI `0..1000`. |
| `UiForge.ReleaseSound(sound_id)` | `void` | `()` | Nil-safe no-op. Sends `close <alias>`. Retains record and tracks limits if close fails. |

---

## 3. Security Hardening & Resource Bounds

### 3.1 Input Sanitization & Traversal Defense
- **Pre-Sanitization**: Rejects empty strings, `"`, `\0`, control chars, `..` traversal, and drive-relative paths (`C:foo`).
- **Canonical Containment**:
  - Relative paths resolved against `resource_dirs` must start with `canonical_script_root`.
  - Absolute paths must start with `canonical_script_root`, a canonical `resource_dir`, or `canonical_system_media`.
- **Post-Canonicalization Sanitization**: The canonical path string is re-sanitized before string construction.
- **Native Wide UTF-16 Formatting**: Wide commands are constructed directly from `OsStrExt::encode_wide` on Windows.

### 3.2 Strict Resource Bounds
- **Maximum Asset Size**: Single sound file $\le 16\text{ MiB}$ (`MAX_SOUND_FILE_BYTES`).
- **Maximum Open Handles**: Up to 16 sound handles per addon bank (`MAX_SOUND_HANDLES`).
- **Aggregate Asset Memory**: Cumulative file size across the bank $\le 64\text{ MiB}$ (`MAX_AGGREGATE_BYTES`).
- **Monotonic ID Range**: Sound IDs are checked positive `i32` values (`1..=0x7FFF_FFFF`). Returns `nil` with `SoundDiagnostic::IdExhausted` prior to any MCI command if exhausted.

---

## 4. Lifecycle & State Machine

```
[Bank Construction]
       │
       ▼
  SoundBank::new() (active: false)
       │
       ├───> Inactive: play_sound() returns false (SoundDiagnostic::InactiveAddon)
       │
 [Addon Activation]
       │
       ▼
  bank.set_active(true)
       │
       ├───> LoadSound() -> canonical resolution, resource bounds, MCI open
       ├───> PlaySound() -> sets volume, seeks start, plays
       │
  [Addon Ordinary Disable]
       │
       ▼
  bank.set_active(false)
       │
       ├───> stop_all() (halts all playing audio immediately)
       └───> Handles and MCI aliases PRESERVED across ordinary disable
       │
  [Addon Re-enable]
       │
       ▼
  bank.set_active(true) (all existing handles remain valid and playable)
       │
  [Addon Reload / Failure / Eject / Shutdown / Drop]
       │
       ▼
  bank.close_all() (idempotent / Drop-safe)
       │
       ├───> Sends "close <alias>" for all open handles
       ├───> Failed closes retained in bank to prevent cap bypass
       └───> Drops make terminal final-chance close attempt
```

---

## 5. Seam Integration Plan

1. **Module Placement**: Place `sound.rs` into `crates/eqoa/src/fork_ext/addons/sound.rs`.
2. **Addon Instance Ownership**:
   ```rust
   pub struct AddonInstance {
       // ... existing fields ...
       pub sound_bank: Rc<RefCell<SoundBank>>,
   }
   ```
3. **Registration**: In addon bridge installation:
   ```rust
   sound::install_sound_bindings(
       &lua,
       &uiforge_table,
       instance.sound_bank.clone(),
       instance.discovered.script_root.clone(),
       instance.discovered.resource_dirs.clone(),
   )?;
   ```
4. **Lifecycle Hooks in Manager**:
   - Activation before entrypoint: `instance.sound_bank.borrow_mut().set_active(true)`.
   - On disable: `instance.sound_bank.borrow_mut().set_active(false)`.
   - On reload/drop: `instance.sound_bank.borrow_mut().close_all()`.

---

## 6. Unit & Oracle Test Coverage

The test suite in `sound.rs` exercises:
1. `volume_validation_and_clamping`: Clamping finite values outside `[0, 1]` to `[0, 1]`, rejecting `NaN`/`Inf`, and verifying rejection on existing bank records.
2. `path_sanitization_rejects_dangerous_inputs`: Rejection of `"`, `\0`, control characters, `..` traversal, and drive-relative paths.
3. `bank_defaults_inactive_and_blocks_playback`: Verifies default inactive state and blocking of playback.
4. `monotonic_sound_id_exhaustion_guard`: Verifies rejection before MCI allocation when `0x7FFF_FFFF` is exceeded.
5. `nil_and_invalid_handles_are_safe_noops`: Verifies exact C++ nil-safety returns for all APIs.
6. `handle_and_file_size_limits`: Enforces 16-handle limit and 16 MiB single file limit.
7. `close_failure_retains_record_and_bounds`: Verifies failed close records remain counted in `len()` and `aggregate_bytes()` so caps cannot be bypassed.
8. `real_mci_generated_wav_playback_or_explicit_device_skip`: Generates a valid 100 ms PCM WAV and verifies `LoadSound`, path caching, `PlaySound`, `SetSoundVolume`, `StopSound`, and `ReleaseSound`, with explicit skip reporting in headless CI without a sound device.
9. `lua_bindings_contract_and_nil_safety`: Uses `mlua::Lua` to execute actual Lua scripts verifying nil-safety, finite volume clamping, option tables, and path sanitization.
