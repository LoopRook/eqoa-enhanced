# Forge Addon UI PNG Texture Resources (Stage 2C) Review

**Date**: 2026-10-28
**Worktree**: `C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility`
**Review Target**: [`crates/eqoa/src/fork_ext/addons/textures.rs`](../crates/eqoa/src/fork_ext/addons/textures.rs) & [`docs/FORGE-TEXTURE-RESOURCES-REPORT.md`](FORGE-TEXTURE-RESOURCES-REPORT.md)
**Primary Authorities**:
- Upstream UiForge implementation: `run/forge-compat-reference/UiForge/src/core/core.cpp` and `graphics_api.cpp`
- Local crate dependency: `png 0.18.1` specification and decoder source
- Repository guidelines: `FORK.md`, `CLAUDE.md`, and memory/containment invariants

---

## 1. Executive Summary

Stage 2C introduces file-backed PNG decoding, GPU uploading via `dear_imgui_wgpu`, and dual-mode path resolution (`UiForge.LoadTexture` and `UiForge.IGraphicsApi.CreateTextureFromFile`).

Overall, the core architecture is well-designed:
- It adheres to containment principles by capping raw file reads at 16 MiB (`Read::take(16 * 1024 * 1024 + 1)`).
- Decoded dimensions are constrained to `ADDON_BUDGET.max_dimension` (2048) and clamped to the adapter's hardware limits.
- The WGPU headless rendering test demonstrates valid end-to-end rendering into Dear ImGui with byte-accurate color readback.
- Standalone PCSX2 replacement texture packs remain correctly deferred; maps and core rendering paths are untouched.

However, static analysis identified **one high-severity cross-platform bug**, **two medium-severity resource/diagnostic defects**, and **specific test gaps** that require remediation before Stage 2C is integrated.

---

## 2. Review Findings & Defect Catalog

### [HIGH-01] Absolute Path Canonicalization Uses Raw Backslash Path Instead of Normalized Path
- **File**: [`crates/eqoa/src/fork_ext/addons/textures.rs#L278-L306`](../crates/eqoa/src/fork_ext/addons/textures.rs#L278-L306)
- **Severity**: HIGH (Portability / Platform Defect)
- **Description**:
  In `textures.rs` lines 278–286, `normalized = path_str.replace('\\', "/")` is calculated to sanitize and validate parent traversal (`..`). However, in the absolute path branch at line 304:
  ```rust
  let pb = PathBuf::from(&path_str);
  let canon = match pb.canonicalize() {
      Ok(p) => p,
      Err(e) => {
          bank.borrow_mut()
              .add_diagnostic(format!("texture file unresolvable '{path_str}': {e}"));
          return Ok(Value::Nil);
      }
  };
  ```
  Legacy Forge scripts (such as `retro_health_hearts`) construct absolute paths using Windows-style backslashes with the shared resources directory, e.g.:
  `local p = UiForge.resources_path .. "\\retro_health_hearts\\heart.png"` (see test at line 704).
  On Windows, `canonicalize()` tolerates backslashes because `\` is the native separator. However, on Linux and macOS, `\` is **not** a separator; it is treated as a literal character in the filename.
  Consequently, on Unix systems, `PathBuf::from(&path_str).canonicalize()` looks for a file with literal backslashes in its name, fails with `ENOENT`, logs a diagnostic, and returns `nil`.
- **Remediation**:
  Construct `PathBuf` from `normalized` instead of `path_str`:
  ```rust
  let pb = PathBuf::from(&normalized);
  let canon = match pb.canonicalize() { ... };
  ```

---

### [MED-01] GPU Texture and Allocation Accounting Leak on UserData Creation Failure
- **File**: [`crates/eqoa/src/fork_ext/addons/textures.rs#L386-L395`](../crates/eqoa/src/fork_ext/addons/textures.rs#L386-L395) and [`#L212-L222`](../crates/eqoa/src/fork_ext/addons/textures.rs#L212-L222)
- **Severity**: MEDIUM (Resource Lifecycle / Accounting Leak)
- **Description**:
  In `upload_rgba8` (lines 150–197), the texture is allocated on the GPU device, written via the queue, registered with `WgpuRenderer` (generating an `ExternalTextureId`), `self.allocated_bytes` is incremented, and an entry is inserted into `self.entries` with `alive = true`.
  In `install_for_addon`:
  ```rust
  let mut bank_mut = bank.borrow_mut();
  match bank_mut.upload_rgba8(&bank, &label, width, height, &rgba) {
      Ok(handle) => Ok(Value::UserData(lua.create_userdata(handle)?)),
      Err(e) => { ... }
  }
  ```
  If `lua.create_userdata(handle)?` fails (e.g. if the Lua VM hits its memory limit or an allocation error occurs), the `?` returns early out of the closure.
  The `handle` is dropped without being returned to Lua. Because Lua never receives the handle, script code cannot call `UiForge.ReleaseTexture(handle)`.
  Crucially, because `entry.alive` was set to `true`, standard per-frame cleanup (`collect(false)`) will **never** reclaim it (it only reclaims entries where `!entry.alive`).
  The GPU texture and Dear ImGui external texture registration remain leaked in memory, and `self.allocated_bytes` permanently counts towards the 64 MiB limit until full host teardown (`collect(true)`).
- **Remediation**:
  Wrap the user data creation or unroll the bank insertion if userdata creation fails:
  ```rust
  match bank_mut.upload_rgba8(&bank, &label, width, height, &rgba) {
      Ok(handle) => match lua.create_userdata(handle) {
          Ok(ud) => Ok(Value::UserData(ud)),
          Err(err) => {
              // Roll back entry in bank_mut: unregister external texture and decrement allocated_bytes
              bank_mut.rollback_last_upload();
              Err(err)
          }
      },
      Err(e) => { ... }
  }
  ```

---

### [MED-02] Diagnostic Ring Eviction Flaw Breaks Deduplication When Full
- **File**: [`crates/eqoa/src/fork_ext/addons/textures.rs#L96-L103`](../crates/eqoa/src/fork_ext/addons/textures.rs#L96-L103)
- **Severity**: MEDIUM (Diagnostic Integrity / Spam Vulnerability)
- **Description**:
  `Textures::add_diagnostic` manages a bounded buffer capped at 32 entries:
  ```rust
  pub fn add_diagnostic(&mut self, msg: String) {
      if self.diagnostics.len() >= 32 {
          self.diagnostics.remove(0);
      }
      if !self.diagnostics.contains(&msg) {
          self.diagnostics.push(msg);
      }
  }
  ```
  Because `remove(0)` occurs **before** checking `!self.diagnostics.contains(&msg)`:
  1. When the ring is full (32 entries) and an addon triggers the same diagnostic each frame (e.g., repeatedly calling `LoadTexture("missing.png")` in its draw loop), `remove(0)` pops the oldest message.
  2. If the duplicate `msg` was already in the buffer (not at index 0), `self.diagnostics.contains(&msg)` evaluates to `true`, so `msg` is not pushed. The buffer size drops from 32 to 31.
  3. If `msg` was at index 0, it is removed, `contains(&msg)` evaluates to `false`, and it is re-appended at the end, constantly rotating out legitimate older diagnostics.
- **Remediation**:
  Check `contains(&msg)` before evicting:
  ```rust
  pub fn add_diagnostic(&mut self, msg: String) {
      if !self.diagnostics.contains(&msg) {
          if self.diagnostics.len() >= 32 {
              self.diagnostics.remove(0);
          }
          self.diagnostics.push(msg);
      }
  }
  ```

---

### [LOW-01] `self.next` Handle ID Overflow
- **File**: [`crates/eqoa/src/fork_ext/addons/textures.rs#L51`](../crates/eqoa/src/fork_ext/addons/textures.rs#L51), [`#L186-L187`](../crates/eqoa/src/fork_ext/addons/textures.rs#L186-L187)
- **Severity**: LOW (Theoretical Limit)
- **Description**:
  `self.next` is a `u64` starting at 0 and incremented per texture allocation. Under realistic workloads, overflowing $2^{64}$ is impossible. However, using `self.next = self.next.checked_add(1).ok_or(...)` or documenting wrapping avoids unchecked arithmetic assumptions.

---

### [INFO-01] PNG Decoder Limits Verification
- **File**: [`crates/eqoa/src/fork_ext/addons/textures.rs#L446-L510`](../crates/eqoa/src/fork_ext/addons/textures.rs#L446-L510)
- **Severity**: INFORMATIONAL (Verified Bounded)
- **Description**:
  The PNG reader from `png 0.18.1` sets a default memory buffer limit of 64 MiB (`Limits { bytes: 64 * 1024 * 1024 }`).
  The file stream read is strictly capped prior to decoding at 16 MiB (`Read::take(16 * 1024 * 1024 + 1)` at line 352).
  Immediately following header parsing (`read_info()`), `textures.rs` checks `width > max_dim || height > max_dim` (where `max_dim <= 2048`), and `expected = width * height * 4` is validated against `budget.max_total_bytes` before raw buffer allocation.
  Therefore, memory consumption during decoding is strictly bounded. This is not an unbounded vulnerability.

---

## 3. Test Suite Gaps

1. **Color Discrimination in Shared vs Private Absolute Path Test**:
   [`forge_texture_explicit_shared_absolute_path_alias`](../crates/eqoa/src/fork_ext/addons/textures.rs#L669-L711) writes a shared blue `heart.png` and a private yellow `heart.png`. However, the test only asserts `matches!(res, Value::UserData(_))` and does not verify that the returned texture is actually BLUE. The test should verify pixel content or assert the canonical file path to prove that the shared asset was selected over the private asset.
2. **Cross-Platform Separator Test**:
   Existing tests only exercise Windows-style backslashes (`"\\retro_health_hearts\\heart.png"`). Test cases should verify forward slashes and mixed separators (`"/retro_health_hearts/heart.png"`).
3. **Byte Accounting Budget Exhaustion Test**:
   [`forge_texture_cumulative_budget_and_release_cleanup`](../crates/eqoa/src/fork_ext/addons/textures.rs#L808-L877) tests `max_count: 2`, but the two 2x2 textures consume only 32 bytes against `max_total_bytes: 64`. A test should verify rejection when `max_total_bytes` is exceeded while `max_count` still has capacity.

---

## 4. Remediation Checklist for Implementation Worker

- [x] In [`textures.rs`](../crates/eqoa/src/fork_ext/addons/textures.rs#L304), platform-aware path normalization (`normalize_texture_path_for_platform`) preserves Windows `\\?\` verbatim and `\\` UNC paths while converting legacy Unix script `\` to `/`.
- [x] In [`textures.rs`](../crates/eqoa/src/fork_ext/addons/textures.rs#L96-L103), `!self.diagnostics.contains(&msg)` is checked prior to length check and `remove(0)`.
- [x] In [`textures.rs`](../crates/eqoa/src/fork_ext/addons/textures.rs#L386-L395), transactional rollback helper `upload_and_create_userdata` unregisters external texture, removes entry, and decrements allocated bytes on `lua.create_userdata` failure for both file and memory APIs.
- [x] In [`textures.rs`](../crates/eqoa/src/fork_ext/addons/textures.rs#L669-L711), `forge_texture_explicit_shared_absolute_path_alias` performs WGPU headless rendering and readback asserting pixels are shared BLUE (`[0, 0, 255]`) and not private YELLOW (`[255, 255, 0]`).
- [x] Added unit test `forge_texture_byte_memory_budget_exhaustion_separate_from_count` verifying that `max_total_bytes` memory budget limit triggers `nil` and logs diagnostics when byte capacity is exceeded while handle count has capacity.

---

## 5. Remediation Resolution & Verification Summary

### RED Baseline Verification
The 4 defect-targeted tests failed against un-remediated code:
- `forge_texture_diagnostics_dedup_full_ring_preserves_all`: failed (`left: 31, right: 32`).
- `forge_texture_explicit_shared_absolute_path_alias`: failed (`shared blue texture [0, 0, 255] must be rendered`).
- `forge_texture_id_counter_exhaustion_rejects_without_leak`: failed (`attempt to add with overflow`).
- `forge_texture_platform_path_normalization_preserves_windows_verbatim_and_normalizes_unix`: failed (`unnormalized Unix path`).

### GREEN Resolution & Post-Upload Oracle
All 13 focused texture tests and 52 total addon tests passed cleanly:
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

test result: ok. 13 passed; 0 failed; 0 ignored; finished in 6.16s
```

#### Controlled Negative Oracle
To verify that `forge_texture_post_upload_userdata_failure_rolls_back_gpu_and_budget` actually validates the post-upload rollback branch and would fail if rollback were missing:
- `bank.rollback_upload(id)` was temporarily disabled in `upload_and_create_userdata_with`.
- Running the test failed with: `assertion left == right failed: post-upload userdata failure must roll back bank entries: left: 1, right: 0`.
- Restoring `bank.rollback_upload(id)` returned the suite to 100% GREEN.

Full serial addon suite: **52 passed; 0 failed; 0 ignored** in 9.32s (4 symlink 1314 skips, 0 fixture skips).
