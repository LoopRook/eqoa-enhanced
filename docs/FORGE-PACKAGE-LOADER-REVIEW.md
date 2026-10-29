# Independent Review: Forge Addon Package Loader & Resource Resolver (Stage 2B)

**Worktree**: `C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility`
**Target Code**:
- [`crates/eqoa/src/fork_ext/addons/package.rs`](../crates/eqoa/src/fork_ext/addons/package.rs)
- [`crates/eqoa/src/fork_ext/addons/host.rs`](../crates/eqoa/src/fork_ext/addons/host.rs)
**Report Reviewed**: [`docs/FORGE-PACKAGE-LOADER-REPORT.md`](FORGE-PACKAGE-LOADER-REPORT.md)
**Date**: 2026-10-28
**Reviewer**: Antigravity (Independent Read-Only Reviewer)

---

## 1. Executive Summary

Stage 2B introduces the native per-addon package loader (`require`) and contained resource path resolver. Each discovered addon receives an isolated Lua 5.1 virtual machine with dedicated globals, callbacks, and `package.loaded` cache.

The implementation successfully fulfills several key architectural requirements:
- **VM Isolation**: Separate `Host` instances per addon (`Host::for_addon(&DiscoveredAddon)`), ensuring addon state, globals, and module mutations cannot leak across addons.
- **Module Path Resolution**: Ordered search across private then shared module directories with dotted-to-slash replacement and `<name>.lua` before `<name>/init.lua`.
- **Resource Resolver**: Rust-only `resolve_resource` API validating relative paths against directory traversal, absolute paths, drive letters, and UNC paths, strictly enforcing containment against `addon.script_root`.
- **Quotas & Guards**: 1 MiB bounded source reads, 32-level recursion limit, cycle detection, and shared 200,000 instruction budget preservation.

However, detailed inspection reveals **1 High-Severity architectural defect**, **2 Medium-Severity Lua 5.1 compliance/poisoning issues**, **1 Low-Severity maintenance item**, and **specific test suite gaps**.

---

## 2. Concrete Findings by Severity

### High Severity: Host Registration & API Seam

#### [HIGH-01] `register_host_module` is Blocked by Hardcoded `frontiers_forge.*` Check
- **Location**: [`crates/eqoa/src/fork_ext/addons/package.rs:220-235`](../crates/eqoa/src/fork_ext/addons/package.rs#L220-L235) and [`crates/eqoa/src/fork_ext/addons/host.rs:226-229`](../crates/eqoa/src/fork_ext/addons/host.rs#L226-L229)
- **Code**:
  ```rust
  // package.rs:220-235
  if name == "frontiers_forge.util" {
      return Ok(Value::Table(util_table.clone()));
  }
  if name.starts_with("frontiers_forge.") {
      return Err(mlua::Error::runtime(format!(
          "unsupported FrontiersForge API: '{name}'"
      )));
  }
  if name == "imgui" || name == "uiforge" {
      if let Some(val) = host_reg.borrow().get(&name) {
          return Ok(val.clone());
      }
      return Err(mlua::Error::runtime(format!(
          "unsupported reserved module: '{name}'"
      )));
  }
  ```
- **Defect Analysis**:
  1. `Host::register_host_module(&self, name, value)` stores arbitrary module names in `host_registry`.
  2. When Lua executes `require("frontiers_forge.player")` or any other native capability module (e.g. `frontiers_forge.target`, `frontiers_forge.inventory`), line 223 matches `name.starts_with("frontiers_forge.")` and **unconditionally returns an error**. It never queries `host_reg`.
  3. Furthermore, lines 228-235 restrict `host_reg` lookups strictly to `"imgui"` and `"uiforge"`. Any other module registered via `register_host_module` is completely bypassed.
- **Impact**: Native submodules registered for future Forge compatibility (e.g. `frontiers_forge.player`) cannot be loaded via `require`.
- **Recommendation**: Query `host_reg` before the fallback error check:
  ```rust
  // 1. Check registered host modules first
  if let Some(val) = host_reg.borrow().get(&name) {
      return Ok(val.clone());
  }
  if name == "frontiers_forge.util" {
      return Ok(Value::Table(util_table.clone()));
  }
  if name.starts_with("frontiers_forge.") {
      return Err(mlua::Error::runtime(format!(
          "unsupported FrontiersForge API: '{name}'"
      )));
  }
  if name == "imgui" || name == "uiforge" {
      return Err(mlua::Error::runtime(format!(
          "unsupported reserved module: '{name}'"
      )));
  }
  ```

---

### Medium Severity: Lua 5.1 Require Semantics & State Integrity

#### [MED-01] Non-Nil Loader Return Value Ignored if Module Modifies `package.loaded`
- **Location**: [`crates/eqoa/src/fork_ext/addons/package.rs:294-307`](../crates/eqoa/src/fork_ext/addons/package.rs#L294-L307)
- **Standard Lua 5.1 Specification (`loadlib.c:ll_require`)**:
  ```c
  lua_call(L, 1, 1);  /* run loader */
  if (!lua_isnil(L, -1))  /* non-nil return? */
      lua_setfield(L, 2, name);  /* _LOADED[name] = returned value */
  else if (lua_getfield(L, 2, name) == sentinel) {
      lua_pushboolean(L, 1);
      lua_setfield(L, 2, name);  /* _LOADED[name] = true */
  }
  ```
- **Existing Implementation**:
  ```rust
  let current: Value = package_loaded.get(name.as_str())?;
  let final_val = if current == Value::Table(sentinel.clone()) {
      if res == Value::Nil {
          Value::Boolean(true)
      } else {
          res
      }
  } else {
      current
  };
  package_loaded.set(name.as_str(), final_val.clone())?;
  Ok(final_val)
  ```
- **Defect Analysis**:
  In standard Lua 5.1, if the module returns any non-nil value (`res != nil`), that returned value **always overwrites** `package.loaded[name]`.
  In `package.rs`, if a module assigns an intermediate value to `package.loaded[name]` during execution and subsequently returns a final table (`return FinalModule`), `current != sentinel` evaluates to true. The loader chooses `current` and **discards the returned `res`**.
- **Impact**: Modules that stage an initial table in `package.loaded` and return an enriched or wrapped object will have their return value silently dropped.
- **Recommendation**: Invert the evaluation logic to match `loadlib.c`:
  ```rust
  let final_val = if res != Value::Nil {
      res
  } else {
      let current: Value = package_loaded.get(name.as_str())?;
      if current == Value::Table(sentinel.clone()) {
          Value::Boolean(true)
      } else {
          current
      }
  };
  ```

---

#### [MED-02] Potential `loading_stack` Poisoning on Sentinel Store Failure
- **Location**: [`crates/eqoa/src/fork_ext/addons/package.rs:269-271`](../crates/eqoa/src/fork_ext/addons/package.rs#L269-L271)
- **Code**:
  ```rust
  loading_stack.borrow_mut().push(name.clone());
  package_loaded.set(name.as_str(), Value::Table(sentinel.clone()))?;
  ```
- **Defect Analysis**:
  `loading_stack.push(name)` is invoked before `package_loaded.set`. If `package_loaded.set` fails (e.g. memory allocation error or a hostile/read-only metatable on `package.loaded`), the `?` operator immediately exits the closure without popping `loading_stack`.
  The unpopped entry permanently consumes a slot in `loading_stack`. Repeated occurrences will exhaust the 32-call recursion limit or trigger false-positive cycle errors on subsequent `require` calls.
- **Recommendation**: Use an RAII scope guard (`struct LoadingGuard<'a>(&'a RefCell<Vec<String>>)`) implementing `Drop` to guarantee that `pop()` is executed on every exit path, or ensure `push` occurs only after successful sentinel assignment.

---

### Low Severity: Maintenance & Code Duplication

#### [LOW-01] ~70 Lines of Setup Duplicated Between `Host::for_addon` and `Host::new`
- **Location**: [`crates/eqoa/src/fork_ext/addons/host.rs:26-108`](../crates/eqoa/src/fork_ext/addons/host.rs#L26-L108) vs [`110-197`](../crates/eqoa/src/fork_ext/addons/host.rs#L110-L197)
- **Issue**:
  Both constructors independently configure standard libraries, memory limits, stripped global functions (`dofile`, `loadstring`, `pcall`, etc.), package tables, snapshot hooks, and instruction budget hooks.
  `Host::new` retains a legacy spike `require` closure while `Host::for_addon` installs the full package loader.
- **Assessment**: Maintainability risk. Future changes to security baselines (such as instruction limits or stripped functions) must be synchronized in two places or risk silent divergence.
- **Recommendation**: Extract shared Lua VM setup into a private helper function `Host::create_base_vm()`.

---

## 3. Path Containment Verification

Tracing of callers confirms that path containment is enforced:
1. In `find_module_file` ([`package.rs:176`](../crates/eqoa/src/fork_ext/addons/package.rs#L176)):
   ```rust
   if !canon.starts_with(&addon.script_root) {
       return Err(format!("module '{name}' at '{}' escapes active script root", ...));
   }
   ```
2. In `resolve_resource` ([`package.rs:85`](../crates/eqoa/src/fork_ext/addons/package.rs#L85)):
   ```rust
   if !canon.starts_with(&addon.script_root) {
       return Err(ResourceError::EscapesScriptRoot(canon));
   }
   ```
3. In `compile_addon_entry` ([`host.rs:206`](../crates/eqoa/src/fork_ext/addons/host.rs#L206)):
   ```rust
   if !canon.starts_with(&addon.script_root) {
       return Err(mlua::Error::runtime(...));
   }
   ```
All module candidates, entrypoints, and resource queries canonicalize paths and verify `starts_with(&addon.script_root)`. When a child tree is imported under Layout 3 (`FrontiersForge/scripts/`), `addon.script_root` is set to that child script directory. Escapes outside the script root are correctly rejected.

---

## 4. Test Suite Execution & Recorded Skips

**Baseline Test Execution (Worker-Reported Historical Evidence)**:
- `cargo test --locked -p eqoa fork_ext::addons::` was reported as passing 26 tests cleanly (static review baseline prior to remediation).
- `forge_package_` suite was reported as passing 8 tests in 0.05s.

**Recorded Skips (All Expected)**:
1. **Windows Symlink Privilege Skips (4 tests)**:
   - `addon_discovery_dangling_child_scripts_directory_emits_diagnostic` (corrected from initial review draft)
   - `addon_discovery_dangling_and_escaping_support_dirs_emit_diagnostics`
   - `addon_discovery_cross_scriptroot_symlink_stays_excluded`
   - `forge_package_resource_precedence_and_containment`
   Each logs: `SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))`. This occurs because standard Windows user tokens lack `SeCreateSymbolicLinkPrivilege`.
2. **GPU Fixture Skip (Prior to Pinned Fixture Run)**:
   - `forge_gpu_pinned_grid_and_texture_fixture` logged: `SKIP forge_gpu: set EQOA_FORGE_SPIKE to the pinned local screen_grid.lua` when unconfigured.

**Identified Test Gaps**:
1. No test for `register_host_module` with a `frontiers_forge.*` prefix (which currently fails due to [HIGH-01]).
2. No test verifying that a module return value overwrites an intermediate value stored in `package.loaded` (affected by [MED-01]).
3. No test verifying error recovery when `package.loaded` has a custom or erroring metatable (affected by [MED-02]).

---

## 5. Review Gating Decision

- **Gate Status**: **CONDITIONAL APPROVAL (REMEDIATION REQUIRED BEFORE STAGE 3 HOST EXPANSION)**.
- **Justification**: The core VM isolation and package loading logic are solid and regression-free against Stage 1. However, **[HIGH-01]** will directly block native Player/Target host module registration in Stage 3, and **[MED-01]** introduces subtle incompatibilities with standard Lua 5.1 module conventions.
- Both defects can be cleanly remediated within `package.rs` in a focused implementation turn without breaking existing tests.


---

## 6. Appendix: Stage 2B Remediation Resolution

Following root review, bounded remediation was executed directly in `package.rs` and `host.rs`:

1. **[HIGH-01] Resolved**:
   - `package.rs` now checks `host_reg.borrow().get(&name)` *first* (after `ffi` lockdown), returning registered tables directly.
   - Deceptive disk files (e.g. `modules/frontiers_forge/player.lua`) cannot shadow registered native modules.
   - Registered `frontiers_forge.*` APIs (such as `frontiers_forge.player`) resolve seamlessly.
   - `ffi` remains strictly unavailable and cannot be registered.
   - Unregistered `frontiers_forge.*` modules still fail with clear unsupported API errors.
2. **[MED-01] Resolved (Strict Lua 5.1 `loadlib.c` 481-493 Parity)**:
   - Non-nil loader returns (`res != Value::Nil`) now strictly overwrite whatever intermediate value a module assigned to `package.loaded[name]`.
   - When a loader returns `nil`, if `package.loaded[name]` remains the loading sentinel, it evaluates to `true`.
   - **Root Correction Verified**: When a module explicitly sets `package.loaded[name] = nil` and returns `nil`, stock Lua 5.1 returns `nil`. This stock behavior is preserved and verified by `forge_package_return_overwrites_package_loaded_and_explicit_nil_preserves_nil`.
3. **[MED-02] Resolved**:
   - Implemented Rust RAII guard `LoadingGuard` that automatically pops `loading_stack` on drop across any return path or error.
   - Hostile metatables on `package.loaded` throwing during sentinel store can no longer poison `loading_stack` or exhaust recursion limits.
   - Failed loads cleanly reset `package.loaded[name]` if still containing the sentinel, preserving retry semantics without clearing unrelated entries.
4. **[LOW-01] Resolved**:
   - Extracted `create_base_host()` helper in `host.rs`, sharing memory limit, stripped globals, snapshot/util setup, callback registry, and instruction budget hook between `Host::for_addon` and `Host::new`.
5. **Post-Remediation Verification**:
   - Focused package suite: 11 passed, 0 failed.
   - Serial addon suite with command-local `EQOA_FORGE_SPIKE` pointing to pinned `screen_grid.lua`: **35 passed, 0 failed** in 3.97s.
   - GPU fixture ran and passed (`Forge GPU fixture captured target/forge-spike-grid.png`).
   - 4 expected Windows symlink tests skipped with privilege error 1314.
