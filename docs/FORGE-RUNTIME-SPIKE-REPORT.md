# Forge runtime Stage 1 spike — 2026-10-28

Constrained native feasibility implementation, not full Forge compatibility or a
release candidate. No commit, push, publication, account login, server action,
emulator, or downloaded injector was performed.

## Implementation

- Exact native dependencies: `mlua = 0.12.2` (`lua51`, `vendored`),
  `dear-imgui-rs = 0.18.0` (`build-from-source`), `dear-imgui-wgpu = 0.18.0`
  (`wgpu-30`), `dear-imgui-winit = 0.18.0`. No render crate changes.
- `EQOA_FORGE_SPIKE` explicitly selects one Lua source file. Without it no
  ImGui context, platform backend, input interception, or overlay pass is made.
- Real Dear ImGui draw lists/widgets and opaque, frame-checked draw-list handles;
  matching scalar/table-plus-changed and InputText text-plus-result returns.
- Settings/Save/Load/DisableScript/OnEject callbacks use last-registration-wins.
  A spike control window offers disable, source reload, and profile save/load.
  Profiles use bounded plain JSON data under the selected client configuration
  directory, keyed by canonical source path. Failed/uninitialized hosts cannot
  overwrite profiles. Writes use a temporary file followed by rename.
- Native snapshots supply numeric in-game and start-menu state. The viewer passes
  session-self/HUD availability and `StartData.menu.visible`; no fixture state is
  substituted in the application.
- Final overlay pass follows render-scale resolve and precedes submission.
  Display dimensions come from the final target texture; framebuffer scale is
  one and winit uses `HiDpiMode::Locked(1.0)` for physical client-area pixels.
- Lua heap: 8 MiB; execution/callback: 200,000 instructions; bridge invocation:
  4,096 native UI calls and 32 stack entries. Source/profile files: 1 MiB.
  Memory textures: at most 16, dimensions 1..256, exact RGBA8 byte length.
  Release is deferred until recorded draw data is consumed; expired handles fail.
- Owned Begin/End and Push/Pop operations are unwound on Lua failure; Lua has no
  native addresses or FFI. Host cleanup releases its resources after failure,
  disable/reload and shutdown. This is not a strong security boundary.

Implementation lives in `crates/eqoa/src/fork_ext/addons/`, excluding separately
owned discovery work. Shared edits are four native dependencies, lock resolution,
native module registration, and small viewer state/init/event/post-resolve hooks.
Both viewer minimap calls also adopt Terra's independently reviewed signature
change. Root must review the exact dependency/hook footprint and policy budgets;
this work did not change ownership policy.

## Source authority

FrontiersForge `4026d5cebcede24943ece8dc34833e584f46cd90`, UiForge
`6f2d125cc686cbc882f8098a939cc03cc23a59a1`, and its actual C++ ImGui bindings
`ec6f9ef7bab73e60f7ed32ef1e95dbee3b7b348e` were inspected locally.
The IDE stub was not executed. `screen_grid.lua` was executed unchanged only in
the isolated GPU test. The additional widgets fixture is authored for this spike.

## Verification commands and evidence

PowerShell prerequisite: prepend `C:/Users/hocka/.cargo/bin` to PATH.

1. `cargo fetch` — passed through authorized escalation; fourteen lock additions.
2. `cargo test --locked -p eqoa forge_host -- --test-threads=1` — red: three
   expected failures against the unimplemented host; green: three passed.
   An isolated scratch harness under ignored `target/forge-host-red` also
   observed the host red state; its temporary manifest is not shipped.
3. `cargo test --locked -p eqoa forge_bridge -- --test-threads=1` — red:
   missing ImGui bridge; green: one passed. It checks real geometry, multireturns,
   expired draw-list rejection, and a surviving second host after stack recovery.
4. Set `EQOA_FORGE_SPIKE` to the pinned local `scripts/screen_grid.lua`, then run
   `cargo test --locked -p eqoa fork_ext::addons -- --test-threads=1 --nocapture`.
   The six-test suite covers host limits, profile roundtrips/invalid data, the
   bridge, and real WGPU readback. GPU fixture game state is explicitly synthetic.
   Capture: ignored `target/forge-spike-grid.png`, visually inspected; grid and
   green RGBA texture render, with pixel assertions for both. The fixture also
   exercises pinned Save/Load callbacks and the InputText return shapes.
5. `cargo build --locked -p eqoa` — passed, Windows development profile.
6. `cargo check --locked -p eqoa --target wasm32-unknown-unknown` — passed; native
   addon code/dependencies stay outside the browser build.

Final focused rerun after cleanup/profile and texture-release controls: six
passed, zero failed, no prerequisite skips. The GPU test additionally rejects
invalid texture byte lengths and released handles while retaining an image
already emitted earlier in that frame. `git diff --check` passed.
New Rust modules were rustfmt formatted. Existing unrelated dead-code warnings
remain. No full workspace suite, release build, web packaging, or player package
verification was run for this bounded spike.

## Remaining gates and deliberate limits

- No live-session, packaged-app, winit input/IME, DPI-monitor transition, resize,
  supersampling, multibox/profile isolation, or logout/re-entry acceptance yet.
  The GPU fixture does not establish those behaviors.
- The application live/offscreen frame loop has the new pass. The standalone
  CLI `shot_with` function does not execute addons; the PNG above is a dedicated
  offscreen GPU fixture, not a full game shot or matched Forge parity capture.
- Profiles roundtrip in tests, but actual window restart/profile file persistence
  and failure/reload controls need interactive acceptance.
- Only one explicitly selected source is loaded. Normal drop-in discovery,
  resource-file lookup, fonts/audio, general HUD/input claims, `bit`/`serpent`,
  broad widgets, and full `ff_example` remain outside this spike.
- `pcall`, `xpcall`, coroutines, OS/IO/debug libraries, native modules and dynamic
  code loading are unavailable. This keeps budget termination bounded for the
  selected script; broader Lua compatibility needs tested protected-call wrappers.
- InputText is bounded, supported flag/format overloads are restricted, and invalid
  texture creation raises a capability error. These are explicit adaptations,
  not universal UiForge equivalence.
- Both native maps remain defaults. Forge maps are optional later proof cases,
  not required replacements or ports.
