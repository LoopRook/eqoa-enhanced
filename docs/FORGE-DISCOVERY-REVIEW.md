# Independent Review: Forge Addon Discovery Engine (Stage 2A)

**Worktree**: `C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility`
**Target Code**: [`crates/eqoa/src/fork_ext/addons/discovery.rs`](../crates/eqoa/src/fork_ext/addons/discovery.rs)
**Contract Specification**: `.superpowers/sdd/2026-10-28-forge-compatibility/discovery-brief.md`
**Report Reviewed**: [`docs/FORGE-DISCOVERY-REPORT.md`](FORGE-DISCOVERY-REPORT.md)
**Reviewer**: Antigravity (Independent Read-Only Reviewer)
**Date**: 2026-10-28

---

## 1. Executive Summary

This independent review assesses the Stage 2A native addon discovery engine implemented in `crates/eqoa/src/fork_ext/addons/discovery.rs`.

The implementation satisfies the core architectural invariants of the discovery brief:
- **Pure Data Interfaces**: Clean, read-only data structs (`DiscoveredAddon`, `DiscoveryDiagnostic`, `Discovery`) with public fields designed for consumption by later lifecycle and host stages.
- **Zero Runtime Side Effects**: Performs no script execution, Lua VM instantiation, network calls, environment mutation, or filesystem writes.
- **Deterministic Output**: Implements deterministic sorting across discovered addons (case-insensitive ID, then exact ID, then canonical entry path) and diagnostics (canonical path, then message).
- **Graceful Isolation**: Errors in individual manifests or directories produce actionable diagnostics and do not crash or prevent valid neighboring addons from being discovered.
- **Companion Preservation**: Correctly ignores companion manifests with `command` and no Lua runtime (e.g. `mods/eqoa_parser/mod.json`) without emitting spurious diagnostics.

The review identified **3 Medium-severity findings** relating to symlink containment/diagnostics, **3 Low-severity semantic/code findings**, and **4 notable Unit Test gaps**.

---

## 2. Concrete Findings by Severity

### Medium Severity: Symlink & Directory Containment

#### [MED-01] Module and Resource Directory Symlink Escapes Silently Dropped Without Diagnostic
- **Location**: `crates/eqoa/src/fork_ext/addons/discovery.rs:246-250`, `255-259`, `581-585`, `590-594`, `599-603`, `608-612`, `680-684`, `689-693`, `698-702`, `707-711`
- **Specification**: Brief line 21 requires:
  > *"Canonicalize paths. Reject absolute/drive/UNC/parent traversal manifest entries, entry/module/resource paths escaping the canonical script root through symlinks, and invalid native types/API/schema... Return actionable diagnostics for missing root, unreadable candidates or bad files without losing valid neighbors."*
- **Issue**: In loose scripts, native manifests, and legacy packages, when `modules` or `resources` directories are checked, the code performs:
  ```rust
  if let Ok(cm) = pkg_mod.canonicalize() {
      if cm.starts_with(canonical_root) {
          module_dirs.push(cm);
      }
  }
  ```
  If `pkg_mod` or `shared_mod` is a symlink pointing outside `canonical_root`, the canonicalized path `cm` fails `starts_with(canonical_root)`. However, the failure is **silently ignored**, pushing nothing and emitting no `DiscoveryDiagnostic`. Similarly, if `canonicalize()` fails (e.g. broken symlink or access error), it is silently ignored.
- **Impact**: Escaped or broken module/resource symlinks fail closed (the directory is excluded), but operators and addon developers receive zero diagnostic explaining why the module/resource path was rejected.
- **Recommendation**: Emit a `DiscoveryDiagnostic` when an existing `modules` or `resources` directory path escapes the canonical root or fails canonicalization.

---

#### [MED-02] Scope of `starts_with` Containment Check Checks `canonical_root` Instead of `script_root`
- **Location**: `crates/eqoa/src/fork_ext/addons/discovery.rs:247`, `256`, `582`, `591`, `600`, `609`, `681`, `690`, `699`, `708`
- **Specification**: Brief line 21 specifies rejecting:
  > *"entry/module/resource paths escaping the canonical script root through symlinks"*
- **Issue**: In Layout 3, an imported child scripts tree (such as `root/FrontiersForge/scripts/`) is established as its own `script_root`. However, the symlink escape checks for module and resource directories compare against `canonical_root` (the top-level root) rather than `script_root`. Consequently, a package inside `FrontiersForge/scripts/` whose `modules` directory is symlinked to another folder under `root` outside `FrontiersForge/scripts` passes `starts_with(canonical_root)`.
- **Impact**: Cross-script-root containment leakage across different imported script roots.
- **Recommendation**: For `shared_mod`/`shared_res`, verify `cm.starts_with(script_root)`. For `pkg_mod`/`pkg_res`, verify `cm.starts_with(package_root)` or `cm.starts_with(script_root)`.

---

#### [MED-03] Unhandled Canonicalization Failures on Child `scripts` Subdirectories
- **Location**: `crates/eqoa/src/fork_ext/addons/discovery.rs:122-132`
- **Specification**: Brief line 21:
  > *"Return actionable diagnostics for missing root, unreadable candidates or bad files without losing valid neighbors."*
- **Issue**: In Layout 3 child detection:
  ```rust
  let scripts_subdir = path.join("scripts");
  if scripts_subdir.is_dir() {
      if let Ok(canon_scripts) = scripts_subdir.canonicalize() {
          ...
      }
  }
  ```
  If `scripts_subdir` is an unreadable directory, permission-denied directory, or broken symlink, `scripts_subdir.canonicalize()` returns an `Err`, which is silently skipped with `if let Ok`. No diagnostic is recorded.
- **Impact**: A broken or inaccessible child script directory leaves no trace in `diagnostics`.
- **Recommendation**: Add an `else` branch emitting an actionable diagnostic indicating that the child scripts directory could not be resolved or canonicalized.

---

### Low Severity: Semantic & Code Findings

#### [LOW-01] Semantic Divergence on Legacy Packaged Addon ID
- **Location**: `crates/eqoa/src/fork_ext/addons/discovery.rs:715-716` vs `discovery-brief.md:19`
- **Specification**: Brief line 19 states:
  > *"Legacy id is entry filename stem; name same unless native name supplied."*
- **Analysis**: For legacy packages matching `package_name/main.lua` or `package_name/init.lua`, the literal filename stem of the entry file would be `"main"` or `"init"`. If the implementation used the entry filename stem, every legacy package utilizing `main.lua` or `init.lua` would share the ID `"main"` or `"init"`, triggering duplicate ID exclusions across the entire installation.
- **Implementation Behavior**: The author assigned `id: dir_name.to_string()` (e.g. `"main_pack"`, `"init_pack"`), verified in test `addon_discovery_loose_file_precedence_and_shared_dirs` (lines 806-810).
- **Assessment**: The implementation's choice (`dir_name`) is practically essential to prevent catastrophic ID collisions. However, it technically diverges from the literal wording of the brief.
- **Recommendation**: Retain the current implementation and amend the brief wording in Stage 2B documentation to clarify: *"For loose files, legacy ID is entry filename stem; for packaged directories, legacy ID is package directory name unless native manifest supplies an ID."*

---

#### [LOW-02] Missing ID Character Set Validation for Legacy Addons
- **Location**: `crates/eqoa/src/fork_ext/addons/discovery.rs:263`, `715`
- **Issue**: While `parse_native_manifest` enforces `is_valid_id` (`[a-zA-Z0-9_.-]+`) on `mod.json` entries, loose script stems and legacy package directory names are accepted directly as IDs without character validation.
- **Impact**: If a user creates a legacy folder `mods/My Mod (v2)/main.lua`, the discovered ID becomes `"My Mod (v2)"`. If subsequent host runtime subsystems assume IDs are strictly canonical ASCII tokens, unexpected parsing or lookup errors could occur.
- **Recommendation**: Consider validating or sanitizing legacy IDs against `is_valid_id`, or documenting that legacy IDs may contain spaces/arbitrary characters.

---

#### [LOW-03] Unused Variable `_file_name` in Loose Script Loop
- **Location**: `crates/eqoa/src/fork_ext/addons/discovery.rs:217`
- **Issue**: `let _file_name = file.file_name().unwrap_or_default().to_string_lossy();` is defined but never used. Minor dead code.
- **Recommendation**: Remove the variable.

---

## 3. Unit Test Coverage & Gap Analysis

The existing test suite contains 6 tests in `crates/eqoa/src/fork_ext/addons/discovery.rs`:
1. `addon_discovery_missing_root_returns_diagnostic`
2. `addon_discovery_loose_file_precedence_and_shared_dirs`
3. `addon_discovery_child_scripts_directory_imported`
4. `addon_discovery_native_manifest_success_and_companion_skip`
5. `addon_discovery_manifest_diagnostics_and_path_traversal`
6. `addon_discovery_duplicate_ids_excluded`

While these tests cover the happy paths and primary error conditions, the following gaps against the brief specification were identified:

1. **[TEST-01] Missing Symlink Tests with Privilege Skips**:
   Brief line 26 requires: *"Include explicit skip diagnostic if symlink test lacks privileges."* No symlink creation or symlink escape tests are present in `discovery.rs`.
2. **[TEST-02] Incomplete Manifest Path Traversal Variants**:
   `addon_discovery_manifest_diagnostics_and_path_traversal` only tests `../outside.lua`. It does not exercise:
   - Absolute Unix paths (`/opt/bad.lua`)
   - Windows absolute/drive paths (`C:\bad.lua`, `\bad.lua`)
   - UNC paths (`\\server\share\bad.lua`)
3. **[TEST-03] Incomplete Manifest Schema and Type Error Coverage**:
   The test suite does not exercise invalid field types:
   - Non-integer `schemaVersion` (e.g. `"1"`, `1.5`)
   - Non-string or unknown `runtime` (e.g. `123`, `"python"` with schemaVersion)
   - Non-string or unsupported `api` (e.g. `"eqoa/2"`)
   - Non-array `capabilities` or array with non-string elements (e.g. `["hud", 42]`)
   - Non-boolean `enabled` (e.g. `"true"`)
   - Invalid `id` characters (e.g. spaces `"bad id"`, `@`, `$`)
4. **[TEST-04] Hidden Directory & Support Directory Coverage**:
   Only `profiles` is tested as an ignored support directory. Dot-prefixed directories (e.g. `.git/`, `.hidden/`) and non-script directories in imported child structures are not tested.

---

## 4. Verification Limits & Boundaries

- **Read-Only Inspection**: This review was conducted by static source inspection and contract analysis. No files were compiled or modified in the `forge-compatibility` worktree during the review phase.
- **Discovery Scope Only**: The discovery module handles finding and cataloging addon files on disk. It does not load Lua scripts, execute code, bind ImGui widgets, or provide runtime compatibility.
- **Default Map Invariant**: Both native maps remain the active default systems. Forge `world_map` and `mini_map` remain purely optional test cases.

---

## Appendix: Review Resolutions & Verification (Post-Review Implementation)

Following review approval, the following bounded fixes and test expansions were implemented and verified in [`crates/eqoa/src/fork_ext/addons/discovery.rs`](../crates/eqoa/src/fork_ext/addons/discovery.rs):

1. **MED-01 Resolved (`resolve_support_dir`)**:
   - Replaced scattered raw directory canonicalizations with `resolve_support_dir`.
   - Absent optional directories (`modules`, `resources`) remain silent `None`.
   - Dangling symlinks or unreadable directories emit `DiscoveryDiagnostic` (`Failed to resolve...`) without dropping valid neighboring addons.
   - Symlinks escaping canonical root emit `DiscoveryDiagnostic` (`...directory escapes canonical root via symlink...`).
2. **MED-02 Confirmed False Positive & Proved**:
   - In Layout 3 child detection, `discover_script_root(&canon_scripts, &canon_scripts, &mut ctx)` resets `canonical_root` to the active child script root.
   - Legitimate private $\to$ shared links within the *same* script root succeed.
   - Cross-scriptroot links escaping into a different tree fail `starts_with(canonical_root)` and are excluded with a diagnostic.
   - Proved with behavioral test `addon_discovery_cross_scriptroot_symlink_stays_excluded`.
3. **MED-03 Resolved (Child `scripts/` Failures)**:
   - Added checks for `scripts_subdir.exists() || scripts_subdir.is_symlink()`.
   - Broken symlinks or unreadable child `scripts` directories emit `DiscoveryDiagnostic` (`Failed to resolve script directory...`).
4. **LOW-01 & LOW-02 Clarified (Legacy Opaque IDs vs Strict Native IDs)**:
   - Legacy package directories use directory name as opaque ID; loose scripts use entry filename stem.
   - Legacy IDs intentionally preserve drop-in names verbatim as opaque strings (including spaces and Unicode, e.g. `日本語_overlay` and `My Custom UI (v2)`), verified via `addon_discovery_legacy_names_with_spaces_and_unicode_preserved`.
   - Native `mod.json` strictly enforces `[a-zA-Z0-9_.-]+`. Future host persistence must use safe encoded keys.
5. **LOW-03 Resolved**:
   - Cleaned up unused variable `_file_name` and parameter `_dir_name`.
6. **ReadDir Diagnostics**:
   - Replaced `.flatten()` with explicit error logging for directory entry read failures.
7. **Test Suite Expansions (TEST-01 through TEST-04)**:
   - Added `addon_discovery_dangling_and_escaping_support_dirs_emit_diagnostics` (TEST-01 / MED-01).
   - Added `addon_discovery_cross_scriptroot_symlink_stays_excluded` (MED-02).
   - Added `addon_discovery_dangling_child_scripts_directory_emits_diagnostic` (TEST-01 / MED-03).
   - Added `addon_discovery_legacy_names_with_spaces_and_unicode_preserved` (LOW-02).
   - Added `addon_discovery_manifest_schema_type_and_absolute_path_failures` (TEST-02 / TEST-03).
   - Added `addon_discovery_hidden_and_support_dirs_skipped` (TEST-04).
   - Symlink tests check `is_privilege_error` and log explicit skip on Windows unprivileged execution (`os error 1314`).
   - All 12 focused tests passed cleanly (`12 passed; 0 failed; 0 ignored; finished in 0.05s`).
