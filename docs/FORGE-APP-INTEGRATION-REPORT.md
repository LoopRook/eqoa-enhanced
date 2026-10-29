# Stage 2E native application integration

Implemented the native application adapter and Addon Manager controls. This is
source integration, not a packaged preview or full Forge compatibility acceptance.

## Delivered behavior and ownership

- `fork_ext/addons/runtime.rs` owns lazy discovery/activation, the single managed
  Context/platform/shared renderer, controls, physical-target rendering and shutdown.
  The explicitly selected standalone spike is an alternative, never a second Context.
  Missing/empty/disabled-only roots create no graphics state until requested. Discovery
  is cached; rescan is explicit. Native manifests and saved enabled intent can activate
  the runtime. Legacy scripts remain initially disabled.
- `capture.rs` records initial physical key/button ownership. Repeats/releases follow
  it across UI capture changes. Game-held drags remain game-owned; focus loss and
  cursor exit preserve existing game bookkeeping. Lifecycle events reach the client.
- The native-only fork options action uses checked ID1053 and a request consumed on
  the app thread. Controls collect commands; Manager executes once per frame.
  Settings callbacks run in an intentional settings window, including collapse/close
  handling. Disabled instances still count toward the 16-VM cap; the backend's
  instruction to remove an addon first is preserved.
- `fork_ext/mods.rs` now selects `mods` beside the native executable even before that
  folder exists. The exceptional current-executable lookup failure logs a diagnostic
  and uses a temporary unavailable root. Companion launcher tests remain intact.
  Configuration/profile storage stays in the existing client configuration directory.
- Small native-only `viewer.rs` hooks cover Gfx state/initialization, window events,
  raw motion capture, both pregame and live post-resolve frames, and exiting cleanup.
  Live snapshots use real session/self readiness and native start-menu visibility.
  Managed display size comes from the final physical target, with Locked(1.0) input
  mapping and framebuffer scale [1,1]. `after_render` runs after overlay attempts;
  shutdown drops manager/resources before renderer/platform/Context.

No backend, Host/Bridge/Textures, dependencies, render crate, gameplay, maps or policy
changes were made for this phase. No commit, merge, push, release or account/server
action was performed. Installation instructions are in `FORGE-ADDONS-TESTING.md`.

## Verification

Meaningful RED was observed against the prepared stubs for capture ownership,
lazy activation/path selection, command dispatch and common GPU render behavior.
The native directory test failed against the previous cwd fallback; the option
action test failed before action1053 was handled. A further cursor-exit negative
control failed before clearing canceled game drags. Each was then implemented.

Commands use the existing native Cargo installation on PATH and `--locked`:

```powershell
$env:EQOA_FORGE_SPIKE='C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua'
cargo test --locked -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1
cargo test --locked -p eqoa fork_ext::mods:: -- --nocapture --test-threads=1
cargo test --locked -p eqoa options::fork_options:: -- --nocapture --test-threads=1
cargo build --locked -p eqoa
cargo check --locked -p eqoa --target wasm32-unknown-unknown
git diff --check
```

- Final serial addon suite: **61 passed, 0 failed**. Four symlink cases explicitly
  skipped their privileged operations for Windows error1314 (reported as passing
  tests, not ignored). Pinned screen-grid GPU fixture executed and wrote
  `target/forge-spike-grid.png`; no pinned-GPU skip. Existing texture post-upload
  allocator rollback oracle executed.
- Companion/path suite: **5 passed, 0 failed**.
- Fork options suite: **3 passed, 0 failed**.
- Runtime's actual common render helper GPU test verified a deliberate settings
  window, a single settings callback recorded in a saved profile, and green pixel
  readback at physical x128 on a 256x192 final target with x2/x4 scene attachments.
  Five pure capture tests cover ownership changes, repeats/releases, focus loss,
  game-held mouse drags and cursor-exit cancellation. Lazy tests verify no Context
  creation; selectors avoid mutating global cwd/env.
- Native development build: **passed**, exit0 (13 warnings; no warning suppression
  or automatic fixes). WASM target check: **passed**, exit0 (one unused-variable
  warning). No release build or exhaustive workspace suite was run.
- `git diff --check`: passed (line-ending notices only).

The footprint checker was run and **failed with 14 findings** against the newly
advanced `origin/master` (4c3242bc), including shared files outside this phase and
the aggregate viewer budget (+242/-37 versus +221/-12). Policy was not changed.
The integration lead owns exact diff/budget review against the reviewed upstream
base `7912c7393df7d85655d5816980fae1217cc48a32`; this is an outstanding gate.

## Remaining acceptance gates

No live native window/gameplay/IME or monitor-DPI playtest was performed. Pure
ownership tests and GPU readbacks do not establish that acceptance. Check manager
open/close in pregame and world, held inputs across focus changes, focus loss,
resize/DPI, x1/x2/x4 scene scale, restart/profile persistence, unrelated-cwd launch,
and existing app offscreen/report capture behavior in a concrete preview.
Standalone CLI `shot_with` does not execute addons.

Before packaging, the integration lead must integrate already-published0.1.1 map
fixes: this Forge branch's old base would otherwise regress the world map. Native
maps remain defaults. FF_Example, health hearts, broader native data/events/actions
and the full supported compatibility matrix remain later phases, not passed gates.

## Bounded frame-recovery follow-up (2026-10-29)

Review identified an open native ImGui frame when platform preparation or renderer
consumer acquisition failed after frame begin. Reopening reused that Context.
The common helper now calls the backend's idempotent `Context::end_frame()` after
its fallible closure has dropped UI/pass/PendingFrame borrows, before resource
retirement. Already-rendered frames remain idle. Fallible `try_frame`/`try_render`
also propagate documented recoverable errors through the existing pause path;
native programmer assertions/panics are not claimed recoverable.

Three regression tests inject errors at the real helper's post-frame platform
preparation and consumer-acquisition boundaries, plus after actual WGPU render.
These inject boundary errors; they do **not** provoke a real window-context
mismatch, backend ContextNotBound or GPU device-loss error. They use the real
Context/Manager/renderer and reuse the same Context for a subsequent successful
frame, including green GPU pixel readback emitted only by the second Lua entry.
Assertions also cover idempotent cleanup, resumed command dispatch and completed
manager retirement. No duplicate fake renderer path or capture ownership change.

Command: `cargo test --locked -p eqoa forge_runtime_ -- --nocapture --test-threads=1`.
Before the fix: **5 passed, 2 failed**, both pre-render cases reported
`left a native ImGui frame open`; post-render control passed. After the fix:
**7 passed, 0 failed**, all three recovery GPU tests and the existing physical
target/settings GPU test executed (no prerequisite skips). `git diff --check`
passed with line-ending notices only. No repeated native/WASM build or
exhaustive/full addon suite was requested for this native-only helper follow-up.
