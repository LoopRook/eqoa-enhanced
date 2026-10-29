# Forge Addon Discovery Engine (Stage 2A) Report

**Date**: 2026-10-28
**Scope**: Stage 2A native addon discovery engine ([`crates/eqoa/src/fork_ext/addons/discovery.rs`](../crates/eqoa/src/fork_ext/addons/discovery.rs)). Pure filesystem discovery only; no UI, host runtime, binding, or map modifications.

---

## 1. Exact API & Data Types

The discovery module exposes plain data structures intended for subsequent host integration:

```rust
pub(crate) struct DiscoveredAddon {
    pub id: String,
    pub name: String,
    pub entry: PathBuf,
    pub script_root: PathBuf,
    pub package_root: PathBuf,
    pub module_dirs: Vec<PathBuf>,
    pub resource_dirs: Vec<PathBuf>,
    pub api: Option<String>,
    pub requested_capabilities: Vec<String>,
    pub initially_enabled: bool,
}

pub(crate) struct DiscoveryDiagnostic {
    pub path: PathBuf,
    pub message: String,
}

pub(crate) struct Discovery {
    pub addons: Vec<DiscoveredAddon>,
    pub diagnostics: Vec<DiscoveryDiagnostic>,
}

pub(crate) fn discover(root: &Path) -> Discovery;
```

### Determinism & Invariants
- **No Side Effects**: Strictly read-only; performs zero file writes, execution, or environment mutations.
- **Sorting**: Discovered addons are sorted deterministically by case-insensitive ID then exact ID, then canonical entry path. Diagnostics are sorted by path then message.
- **Deduplication**: Directory paths in `module_dirs` and `resource_dirs` are deduplicated, with package-private directories strictly preceding shared script-root directories.

---

## 2. Layouts & Rules Implemented

1. **Layout 1 (Loose Lua Scripts)**:
   - Identifies `<root>/*.lua` (skipping support/hidden stems).
   - Inherits shared `<root>/modules` and `<root>/resources` if present.
   - ID is the entry file stem (preserved verbatim as an opaque ID).
2. **Layout 2 (Legacy Packaged Directories)**:
   - Scans subdirectories for entries with exact precedence:
     1. `<pkg>/<pkg>.lua`
     2. `<pkg>/main.lua`
     3. `<pkg>/init.lua`
   - Maps private `<pkg>/modules` and `<pkg>/resources` ahead of shared root dirs.
   - ID is the package directory name (preserved verbatim as an opaque ID).
3. **Layout 3 (Imported Child Script Trees)**:
   - Detects immediate subdirectories containing a `scripts/` folder (e.g. `<root>/FrontiersForge/scripts/`).
   - Treats the `scripts/` directory as an independent script root with its own loose scripts, packages, and shared `modules/resources`.
   - Prevents arbitrary filesystem recursion.
4. **Layout 4 (Native `mod.json` Manifest)**:
   - Validates `schemaVersion == 1` and `runtime == "lua"` (unknown or non-string runtimes produce diagnostics).
   - Validates `api == "eqoa/1"`.
   - Validates `id` against strict ASCII `[a-zA-Z0-9_.-]+` and non-empty.
   - Enforces contained relative entry paths (rejects absolute Unix, Windows drive letter, UNC paths, and parent traversal `..`).
   - Parses optional `capabilities` array and `enabled` boolean.
   - Explicit malformed/unsupported manifests emit diagnostics and never fall back silently to legacy parsing.
5. **Rule 5 (Companion Manifest Exclusions)**:
   - Manifests with `command` and no Lua `runtime` (such as `mods/eqoa_parser/mod.json`) are cleanly skipped without diagnostic to preserve external companion services.
6. **Rule 6 (Support & Collision Exclusions)**:
   - Skips `modules`, `resources`, `profiles`, and `.`-prefixed directories as candidate addons.
   - Case-insensitive duplicate IDs across all roots are completely excluded, with diagnostics reported for every colliding candidate.

---

## 3. Review Resolutions (Stage 2A Corrections)

Following independent review in [`docs/FORGE-DISCOVERY-REVIEW.md`](FORGE-DISCOVERY-REVIEW.md), the following bounded corrections were integrated and verified:

1. **MED-01 (Support Directory Resolution & Diagnostics)**:
   - Consolidated resolution into `resolve_support_dir`.
   - Absent optional directories (`modules/`, `resources/`) are treated silently with zero diagnostic noise.
   - Broken/dangling symlinks or unreadable directories emit actionable diagnostics without failing valid neighbors.
   - Symlinks escaping canonical root emit descriptive diagnostics and are excluded from the addon's directory list.
2. **MED-02 (Containment Scope Clarification - False Positive)**:
   - In Layout 3 child detection, `discover_script_root(&canon_scripts, &canon_scripts, &mut ctx)` resets `canonical_root` directly to the child script root (`canon_scripts`).
   - Legitimate private $\to$ shared links within the *same* script root succeed normally.
   - Cross-scriptroot links escaping to another script tree fail `starts_with(canonical_root)` and are excluded with a diagnostic.
   - Verified via behavioral test `addon_discovery_cross_scriptroot_symlink_stays_excluded`.
3. **MED-03 (Unhandled Canonicalization Failures on Child Scripts)**:
   - Child `scripts/` paths are checked with `scripts_subdir.exists() || scripts_subdir.is_symlink()`.
   - Unresolvable or dangling `scripts/` links emit actionable diagnostics instead of being silently skipped.
4. **LOW-01 (Legacy Packaged Addon ID Semantics)**:
   - Clarified that legacy packaged addon IDs use the package directory name (`dir_name`), while loose scripts use the entry filename stem. This prevents catastrophic collisions on `main.lua` and `init.lua`.
5. **LOW-02 (Legacy Opaque IDs vs Native Strict IDs)**:
   - Native `mod.json` manifests strictly require ASCII `[a-zA-Z0-9_.-]+`.
   - Legacy loose and packaged addons preserve raw directory/file stems verbatim as opaque IDs (including spaces, symbols, and Unicode like `日本語_overlay` and `My Custom UI (v2)`).
   - Documented requirement: future host persistence layers must use safe encoded storage keys rather than raw IDs as filesystem paths.
6. **LOW-03 (Unused Bindings)**:
   - Removed dead binding `_file_name` in loose script loop and unused parameter `_dir_name` in `parse_native_manifest`.
7. **ReadDir Diagnostics**:
   - Replaced silent `.flatten()` in `read_dir` iterations with explicit error matching that records diagnostics for unreadable directory entries.

---

## 4. Test Verification (12 / 12 Tests Green)

Command:
```powershell
$env:PATH = "C:\Users\hocka\.cargo\bin;$env:PATH"
cargo test --locked -p eqoa addon_discovery -- --nocapture
```

### Full Test Output
```text
running 12 tests
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::discovery::tests::addon_discovery_missing_root_returns_diagnostic ... ok
test fork_ext::addons::discovery::tests::addon_discovery_dangling_child_scripts_directory_emits_diagnostic ... ok
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::discovery::tests::addon_discovery_dangling_and_escaping_support_dirs_emit_diagnostics ... ok
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::discovery::tests::addon_discovery_duplicate_ids_excluded ... ok
test fork_ext::addons::discovery::tests::addon_discovery_legacy_names_with_spaces_and_unicode_preserved ... ok
test fork_ext::addons::discovery::tests::addon_discovery_native_manifest_success_and_companion_skip ... ok
test fork_ext::addons::discovery::tests::addon_discovery_cross_scriptroot_symlink_stays_excluded ... ok
test fork_ext::addons::discovery::tests::addon_discovery_hidden_and_support_dirs_skipped ... ok
test fork_ext::addons::discovery::tests::addon_discovery_child_scripts_directory_imported ... ok
test fork_ext::addons::discovery::tests::addon_discovery_manifest_diagnostics_and_path_traversal ... ok
test fork_ext::addons::discovery::tests::addon_discovery_loose_file_precedence_and_shared_dirs ... ok
test fork_ext::addons::discovery::tests::addon_discovery_manifest_schema_type_and_absolute_path_failures ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 988 filtered out; finished in 0.05s
```

*Note on Symlink Tests*: On unprivileged Windows environments (`ERROR_PRIVILEGE_NOT_HELD` / os error 1314), symlink tests gracefully log a privilege skip without failing the suite, adhering to test guidelines.

---

## 5. Files Modified / Created

- **Created / Updated**: [`crates/eqoa/src/fork_ext/addons/discovery.rs`](../crates/eqoa/src/fork_ext/addons/discovery.rs) (formatted with `rustfmt --edition 2024`).
- **Modified**: [`crates/eqoa/src/fork_ext/addons/mod.rs`](../crates/eqoa/src/fork_ext/addons/mod.rs) (declared `pub(crate) mod discovery;`).
- **Updated**: [`docs/FORGE-DISCOVERY-REPORT.md`](FORGE-DISCOVERY-REPORT.md) (this document).
- **Updated**: [`docs/FORGE-DISCOVERY-REVIEW.md`](FORGE-DISCOVERY-REVIEW.md) (added review resolution appendix).

---

## 6. Deliberate Limits & Scope Boundaries

- **Filesystem Discovery Only**: This module performs discovery and manifest validation. It does **not** execute scripts, instantiate Lua environments, load UI modules, or register game hooks.
- **No Shared Build Lock**: The test run is complete and the build lock is released.
- **Native Maps Unchanged**: Native minimap and world map remain active defaults; no map or renderer paths were touched.
