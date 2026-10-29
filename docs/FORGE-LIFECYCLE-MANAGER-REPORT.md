# Native addon backend lifecycle — Stage 2D

Implemented in `fork_ext/addons/manager.rs` and `storage.rs`, with declarations in
`addons/mod.rs`. No viewer/options wiring, dependencies, frozen host/bridge/texture
APIs, map code, or ownership policy were changed. No commit, merge, push or release.

## Concrete backend API and call order

`manager` is crate-visible. Its concrete API is:

```rust
Manager::new(root: PathBuf, storage_root: Option<PathBuf>,
             device: &wgpu::Device, queue: &wgpu::Queue,
             renderer: Rc<RefCell<WgpuRenderer>>) -> Manager
Manager::entries(&self) -> Vec<EntryView>
Manager::diagnostics(&self) -> &[Diagnostic]
Manager::queue(&mut self, command: Command)
Manager::frame(&mut self, snapshot: Snapshot)
Manager::after_render(&mut self)
Manager::shutdown(&mut self)
```

Commands are `Enable(String, bool)`, `Reload(String)`, `Rescan`, `Settings(String)`,
`Save(String)`, and `Load(String)`. They execute at the next frame boundary; queueing
does not execute Lua. Entries expose discovered metadata, enabled intent, status
(`Disabled`, `Enabled`, `Failed`) and initialization state. Diagnostics distinguish
primary frame/callback errors, cleanup/eject errors, storage and discovery failures.
The queue is capped at 1,024 commands and diagnostics at 128 messages of 2,048
characters each.

The caller owns the **one Context/platform/shared renderer** and must:

1. Begin its actual ImGui frame, then call `frame(actual_snapshot)`.
2. Consume the Context's draw data through that shared backend and record the pass.
3. Call `after_render()`. A second `frame()` before this boundary is rejected with
   a diagnostic. Textures retired by failure, replacement or removal stay alive
   until this boundary. All retained banks, including disabled instances, collect
   ordinary explicit releases; live handles survive ordinary disable.
4. Call `shutdown()` after its final pass is recorded, while the renderer/Context
   remain alive. Shutdown is idempotent and callbacks cannot render outside a
   bridge invocation. Drop performs best-effort nonrendering shutdown. If the caller
   omitted the render boundary, Drop avoids unregistering possibly referenced
   textures; backend-held references then live until renderer teardown.

## Lifecycle behavior

- Construction discovers metadata but creates no VMs and executes no addon code.
  Legacy defaults remain disabled; explicit persisted intent overrides a native
  manifest's initial enabled value. Missing/empty roots instantiate nothing.
- At most 16 VMs, including disabled retained environments, are instantiated.
  Fresh compile-before-replace reload also respects this cap: at 16 existing VMs,
  it reports that an instantiated addon must be removed before staging a replacement.
- First enabled entry executes once, registers callbacks/draws, then receives saved
  Load data once before subsequent entry frames. Each entry has its own Host,
  Bridge, compiled entry function and texture bank. Snapshot is updated before
  entries and queued callbacks; no native game values are fabricated.
- Normal disable saves healthy initialized state where possible, calls DisableScript
  once for the active transition, and preserves globals/module cache/callbacks.
  Successful ordinary disable retains its live texture bank too. Re-enable resumes
  those globals and cached handles without rerunning initialization or Load.
  Successful inactive Save/Load/Settings callbacks likewise retain cached textures.
  Any new Lua callback error marks the owner Failed and forces retirement, including
  errors during disable or inactive callbacks; re-enable requires Reload.
- Reload prepares and parses a fresh environment before touching the old one.
  Read/syntax failure leaves the old active instance running. A valid replacement
  initializes on a subsequent frame, preserving disabled intent. Runtime failure
  in the replacement is isolated, with no claim of side-effect rollback.
- A Lua frame/callback failure marks only its addon Failed, invokes DisableScript
  once if active, and retires its resources even when cleanup fails. No automatic
  per-frame retry; explicit Reload is required. Invalid profile encoding or IO
  failures produce diagnostics and do not trap an addon in the enabled state.
- OnEject is separate from disable and runs once at shutdown for every initialized
  retained environment, including previously disabled/failed ones. Final cleanup
  also covers textures allocated by OnEject before an error.
- Rescan preserves unchanged instances and enabled intent. Removed or duplicate-
  invalidated entries retire only their banks; unaffected neighbors keep state.

## Storage

### 2026-10-29 retained-texture correction

Ordinary disable now explicitly selects retained disposition. Terminal failure,
successful reload (including an already-disabled old VM), removed/changed rescan
and eject select forced retirement. A cleanup exception overrides retention and
keeps Failed/reload-needed semantics. Failed syntax reload preserves old handles.
Successful inactive Save/Load/Settings no longer retire their cached allocations;
explicit ReleaseTexture still collects through the existing post-render boundary.
Disabled allocations remain charged to the same budgets and 16-VM cap.

This corrects a native compatibility defect: pinned UiForge
`ForgeScript::Disable` (forgescript_manager.cpp391-401) retains its environment and
textures. Unchanged hearts caches four handles and skips Initialize after reenable.
Its OnDisable releases native-bar suppression, while ordinary upstream Enable386-388
only sets enabled=true. Plain reenable therefore resumes images with the native
bar visible. No callback/claim restoration is invented. Hearts Load131-150 or an
explicit settings toggle reasserts suppression; C++ profile application defers Load
until after its script pass and only invokes enabled scripts with saved state.
Native per-addon profile actions remain existing host extensions. Health claim
installation/publication is still separate app plumbing; this correction does not
claim that failed owners' HUD claims have been integrated or live hearts accepted.
Saved-enabled pre-login startup also needs a separate bounded pending-intent layer:
hearts initializes before its Render in-game guard. Availability may suppress an
effective claim without discarding the requested intent. Disable/failure/eject or
an explicit EnableHealthBar must clear that intent, with no restoration on plain
reenable. That adaptation and real stats freshness/order remain future gates.

Regression work changed the incorrect disabled-resource expiry assertions and
added same-userdata/same-external-texture reenable GPU readback; inactive callback
errors (Save/Load/Settings) and failing DisableScript must instead expire both old
and newly allocated handles after consumption and reject plain reenable. Reload
tests cover syntax failure and replacement for active and disabled old instances.
Existing failure-isolation, OnEject and explicit-release controls remain exercised.

RED: the disabled callback retention assertion failed. The first GPU fixture edit
overlapped the victim window; after correcting that fixture, focused GPU RED failed
at the intended healthy cached handle expiry after ordinary disable. No production
change preceded these intended failures. Final command results follow below.

- `cargo test --locked -p eqoa forge_manager_ -- --nocapture --test-threads=1`:
  **6 passed, 0 failed**, all GPU prerequisites present. During verification, the
  fixture also needed a warmup for the newly appearing ImGui image window and a
  retained strong Lua reference to inspect replaced userdata; these were fixture
  fixes, not additional production behavior changes.
- With command-local `EQOA_FORGE_SPIKE` pointing to the pinned reference
  `scripts/screen_grid.lua`, `cargo test --locked -p eqoa fork_ext::addons:: --
  --nocapture --test-threads=1`: **73 passed, 0 failed**, four explicit Windows1314
  symlink-operation skips (reported as passing tests, not ignored). Pinned grid,
  cached-handle reenable/failure-isolation, runtime recovery and texture GPU tests
  executed, including real post-upload allocator failure rollback.
- `git diff --check`: passed with line-ending notices only. No repeated native,
  WASM, map or exhaustive workspace builds; no source changes outside manager.rs.

Caller-supplied configuration is separated from source under `<config>/addons`.
The state registry is bounded JSON with `schemaVersion: 1` and boolean enabled
values keyed by digest. Profiles reuse the bounded plain-data Lua JSON encoder,
including numeric table keys, and are validated/decoded before Load callbacks.

Filenames use SHA-256 of a versioned domain plus separately length-delimited
canonical entry path, canonical package path and opaque ID bytes. Native encoded
path bytes avoid lossy Unicode collisions. Moving sources changes storage identity;
cross-platform/path portability is not promised. Raw IDs never form paths.

Reads use `take(1 MiB + 1)`; oversized or malformed data, unsupported registry
schemas, unavailable configuration and failed writes are diagnosed. Saves validate
before touching the destination, create a unique same-directory temporary file
with `create_new`, write/flush/sync it, then use `std::fs::rename` to replace the
destination. The old destination is never deleted first. Failed replacement leaves
it intact and removes only that operation's temporary file. This does not claim
atomicity/durability for every remote filesystem or multi-process registry locking.

## Verification

PowerShell PATH was prepended with `C:/Users/hocka/.cargo/bin`.

- RED: `cargo test --locked -p eqoa forge_storage_ -- --nocapture --test-threads=1`
  observed five expected stub failures; the corresponding manager command with
  `forge_manager_` observed four expected stub failures.
- GREEN storage: the same focused storage command passed five tests. They cover
  numeric-key profiles, repeated replacement on Windows, independent configs,
  digest framing/path identity, invalid save preserving old bytes, injected rename
  failure preserving the target/removing temp, malformed/oversized data and schema
  errors.
- Manager testing first exposed a re-enable delay; the retained environment now
  resumes immediately when no retirement boundary is pending. A further regression
  test failed on resources allocated by callbacks in a disabled VM, then passed
  after callback retirement/final-eject cleanup was corrected.
- GREEN manager: `cargo test --locked -p eqoa forge_manager_ -- --nocapture
  --test-threads=1` passed five tests, including actual WGPU readback. Mod A emits
  a magenta texture then errors with an open Begin and failing cleanup callback;
  B's green pixels still render. A's handle is valid before pass recording and
  invalid after `after_render()`. A normal disable also proves deferred reclamation.
  Callback transcripts prove ordering, replacement semantics, preserved globals,
  independent module caches, persisted false, and final OnEject after disable.
  Reload/rescan and 16-VM controls are exercised with generated scratch packages.
- Final command uses command-local `EQOA_FORGE_SPIKE` pointing at the original
  checkout's pinned `scripts/screen_grid.lua`, then:
  `cargo test --locked -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1`.
  Final result: **51 passed, 0 failed**, with the pinned GPU fixture executed and
  captured. Four Windows symlink privilege checks printed explicit skips (os error
  1314); no manager or pinned-GPU prerequisite skip occurred. Runtime was 73.17 s.
- Touched Rust was rustfmt formatted; `git diff --check` passed before this report.

No account/server actions, user-profile changes, pack changes, broad workspace
suite, release build or player package were performed. GPU manager tests use
generated scratch scripts and explicit fixture snapshots. This is a backend slice;
app integration, interactive input/DPI/restart acceptance, native data adapters,
full FF_Example/non-map-suite compatibility and player preview packaging remain.
