# Native Addon Backend Lifecycle (Stage 2D) — Independent Review

**Review Date**: 2026-10-28
**Scope**: Static review of `crates/eqoa/src/fork_ext/addons/manager.rs`, `storage.rs`, and `docs/FORGE-LIFECYCLE-MANAGER-REPORT.md` against root lifecycle brief `.superpowers/sdd/2026-10-28-forge-compatibility/lifecycle-implementation-brief.md`.
**Review Mode**: Independent STATIC review only. Zero source edits, zero compiler/build invocations.

---

## Executive Summary

The Stage 2D backend lifecycle implementation in `crates/eqoa/src/fork_ext/addons/manager.rs` and `crates/eqoa/src/fork_ext/addons/storage.rs` strictly adheres to the root implementation brief. The architecture maintains a clean separation of concerns:
- **Caller Ownership Model**: The caller retains sole ownership of the Dear ImGui `Context`, `WinitPlatform`, and the single shared `Rc<RefCell<WgpuRenderer>>`. `Manager` never creates or prematurely drops rendering contexts.
- **Strict Frame Boundary Discipline**: Commands (`Enable`, `Reload`, `Rescan`, `Settings`, `Save`, `Load`) are strictly queued and only drained at frame boundaries.
- **Resource Lifetime & Isolation**: Resource retirement (`collect(true)`) is deferred until `after_render()`, ensuring GPU textures emitted during a frame are not destroyed before the render pass finishes recording. Neighbor addons continue running smoothly even when a peer fails during an open `ImGui.Begin` block.
- **Storage Durability**: Bounded plain JSON profiles and enabled registries use length-delimited SHA-256 keys, `take(1 MiB + 1)` read bounds, and `std::fs::rename` atomic replacement without pre-deleting destination files.

No critical defects or semantic blockers were found. A few minor design boundaries and limits are documented below for future app-layer and UI integration.

---

## Detailed Findings

### 1. Strengths and Conformance

1. **Deferred Texture Retirement (`manager.rs:225-235`, ` manager.rs:297-304`)**
   - Textures belonging to disabled, reloaded, or failed addons are placed into `self.retired` rather than being collected immediately.
   - `collect(true)` is executed exclusively inside `after_render()`.
   - The GPU readback test (`forge_manager_gpu_failure_isolation_and_after_render_retirement`) provides concrete proof: a texture emitted by a failing addon survives render pass recording and appears in rendered pixel readback before being freed.

2. **Frame-Active Invocation Seam (`manager.rs:203-214`, `manager.rs:280-295`)**
   - All render-capable Lua calls (`entry`, `Settings`, etc.) execute through `Bridge::invoke`.
   - Any Lua error or call budget violation isolates the fault, records diagnostics, unwinds ImGui stacks, and prevents per-frame retry without crashing the host or impacting healthy neighbors.

3. **Lifecycle & Cleanup Separation (`manager.rs:349-368`, `manager.rs:519-551`)**
   - `DisableScript` (callback type 1) is invoked once on active->disabled transition.
   - `OnEject` (callback type 4) is invoked once during `shutdown()` for all initialized retained environments, including previously disabled ones.
   - Resource retirement is forced independently of callback success.
   - If an `OnEject` callback attempts to allocate new resources before terminating, `manager.rs:541-545` resets `resources_retired = false` and re-queues them for retirement.

4. **Atomic File Replacement & Integrity (`storage.rs:166-207`)**
   - File replacement uses same-directory unique temporary files (`.addon-<pid>-<seq>.tmp`) created with `create_new(true)`, followed by `write_all()`, `flush()`, `sync_all()`, dropping the handle, and `fs::rename()`.
   - The destination is never deleted first.
   - Injected rename failure tests confirm that the original target file remains untouched and temporary files are removed.

5. **Collision-Proof Storage Key Derivation (`storage.rs:106-129`)**
   - Identity hashes use `Sha256` with a versioned domain prefix (`eqoa-native-addons/storage/v1\0`), followed by separate 8-byte little-endian length prefixes for the canonical entry path, canonical package path, and opaque ID.
   - Raw IDs never form filesystem paths, eliminating path-traversal vulnerabilities.

---

### 2. Concrete Findings & Operational Limits

#### Finding 2.1: 16-VM Cap Precludes In-Place Reload at Maximum Capacity
- **Severity**: Low / Documented Operational Limit
- **Location**: `crates/eqoa/src/fork_ext/addons/manager.rs:419-428`
- **Details**:
  ```rust
  let count = self.entries.iter().filter(|e| e.instance.is_some()).count()
      + usize::from(entry.instance.is_some());
  if count >= MAX_INSTANCES {
      self.diagnostic(
          Some(&entry.metadata.id),
          "reload",
          "16 VM limit prevents staging a fresh replacement; remove an addon first".into(),
      );
  }
  ```
  Because `Reload` strictly compiles and stages the replacement VM *before* destroying the old active VM (to guarantee that syntax or compilation errors keep the old active instance intact), reloading an already instantiated addon requires a 17th temporary VM. When 16 addons are instantiated, `count` equals 16, and `Reload` is rejected with a diagnostic.
- **Impact**: Expected behavior per brief design. Because normal disable retains the initialized VM/globals/callbacks, disabling an addon cannot free a VM staging slot. When the management UI is implemented in Stage 2E, users should be informed that if 16 addons are currently instantiated/retained, an addon must be removed and rescanned before another can be reloaded.

#### Finding 2.2: Strict Key Count in Registry Deserialization
- **Severity**: Low / Forward Compatibility Note
- **Location**: `crates/eqoa/src/fork_ext/addons/storage.rs:85-88`
- **Details**:
  ```rust
  let object = value
      .as_object()
      .filter(|o| o.len() == 2)
      .ok_or("invalid addon state registry")?;
  ```
  The registry deserializer requires `object.len() == 2` (matching `schemaVersion` and `enabled`).
- **Impact**: While clean and strict for `schemaVersion: 1`, any external metadata or future optional field added to `state.json` would cause `registry()` to reject the file as invalid. If additional fields are introduced in future schemas, this length check should be generalized.

#### Finding 2.3: Deterministic Entry Sort Key
- **Severity**: Informational
- **Location**: `crates/eqoa/src/fork_ext/addons/manager.rs:511-516`
- **Details**:
  `rescan()` sorts entries deterministically by `metadata.entry` path followed by `metadata.id`.
- **Impact**: This fulfills the brief's requirement for deterministic frame execution order. Higher-level management UIs may present entries in alternative sort orders (e.g., localized display name or enabled status) without altering the backend execution sequence.

#### Finding 2.4: Immediate Re-enable Deferred Frame Guard
- **Severity**: Informational / Positive Design Observation
- **Location**: `crates/eqoa/src/fork_ext/addons/manager.rs:409-415`
- **Details**:
  ```rust
  } else if entry.instance.as_ref().is_some_and(|i| {
      self.retired
          .iter()
          .any(|bank| Rc::ptr_eq(bank, &i.textures))
  }) {
      entry.not_before = self.frame_number + 1;
  }
  ```
  If an addon is disabled and re-enabled within the same frame before `after_render()` executes, its bank is still in `self.retired`. Running it immediately would result in `after_render()` purging its newly created textures. Setting `entry.not_before = self.frame_number + 1` cleanly defers its first re-enabled frame until the pending retirement pass completes.

---

## Conclusion & Next Phase Readiness

The implementation in `crates/eqoa/src/fork_ext/addons/manager.rs` and `storage.rs` is sound, thoroughly covered by behavioral tests (including real WGPU rendering and readback), and ready to serve as the backend coordinator for Stage 2E (management UI and app integration).

**Status**: APPROVED.
**Build Slot**: FREE.
