# Forge draw-list slice

This bounded slice adds real native window draw lists and eight shape methods.
It is not full unchanged ff_example/uiforge_example acceptance or a packaged
playtest. No new phase, font/audio/game adapter, dependency, policy, map, Host,
package, manager, runtime, shared hook, commit, push or release is included.

## Source and behavior

Changed `crates/eqoa/src/fork_ext/addons/bridge.rs`; added private child modules
`bridge_drawlist.rs` and `bridge_drawlist_tests.rs`, declared inside bridge only.
The parent delegates existing AddLine/getters and owns frame accounting. The child
owns targeting, argument validation, geometry accounting and primitive calls.

Authority is pinned UiForge binding ec6f9ef7 `sol_ImGui.h`79/2552–2563 plus
ImGui gitlink37b7a7a9 header3307–3317 (1.92.7WIP), fetched by root into original
repository `run/forge-preview-prep/imgui-37b7a7a9.h`, SHA256
`b642eaa18a010ba6b9e5c12fa4829ddba328eb1e7cb94571b0f6cf3d07912905`.

- GetWindowDrawList uses the actual current native window/child draw list.
  GetBackgroundDrawList and existing AddLine use the same guarded target helper.
- AddRect/AddRectFilled, AddCircle/AddCircleFilled, AddTriangle/AddTriangleFilled,
  AddNgon/AddNgonFilled accept native ImVec2 userdata and return no values.
- Pinned Lua AddRect order remains `(pmin,pmax,color,rounding,flags,thickness)`.
  The adapter swaps flags/thickness at the current native FFI boundary.
- Native defaults: rounding0, flags0, thickness1; circle segments0 selects native
  auto tessellation. Ngon requires segments. Permitting omitted trailing arguments
  is an explicit convenience extension matching pinned native defaults and
  observed scripts: direct C++ member-pointer registration itself carries no C++
  default-argument metadata.

## Lifetime and work limits

Handles store opaque numeric identities, never a native draw-list pointer or a
Lua-visible address. Every use first validates the existing active Context/frame
epoch. Window handles also validate current window address/ID, BeginCount, native
window-stack depth and bridge invocation. Saved addresses are never dereferenced.
Only a newly fetched native pointer reaches the primitive call.

A window handle is deliberately scoped more strictly than the upstream raw pointer:
it cannot be used after End, on a reopened same-name window, while another child
is current, or in another callback. A parent handle may resume after its child ends
within the same callback. Background handles retain active-frame behavior and the
existing app teardown contract: bridges/manager die before their Context. Numeric
background Context identity does not claim to solve allocator-address reuse across
destroyed Contexts. Destroyed userdata is rejected by mlua; no new Lua release API
is invented.

Coordinates must be finite with absolute value<=1,000,000; radius/rounding0..4096;
thickness0..64; segments3..512, with circle-only auto0. Rectangle flags are limited
to corner bits0x1f0. Packed color accepts signed i32 or unsigned u32 range and
preserves identical bits, including signed GetColorU32/bit output.

Geometry budget stays **196,608 units per addon per actual native frame** across
repeated bridge invocations. Each estimated path point reserves24 units, above
native thick AA stroke18 indices/point. Explicit polygons reserve segments+2;
lines/triangles/plain rectangles reserve their point count. Rounded rectangles
reserve520, correcting the preparation's insufficient64-point estimate. Auto
circles query the guarded native auto-segment calculator using the unshrunk radius
and reserve `max(calculated+8,50)` points: the50 floor covers rounding of native
48-sample ArcFast steps. Only validated handles reach this metadata query; quota
is charged before any primitive emission, including transparent/clipped calls.

Thus 100 circles at radius20 (the unchanged sample's maximum ball count and worst
radius from its10..20 range) fit without raising the budget; repeated radius4096
geometry fails. The ledger covers these bridge primitives, not native widgets.
Existing call/depth and Host execution limits remain in force.

## Verification

Commands used VS18 BuildTools process-local environment and worktree target cache:

```powershell
cargo test --locked -p eqoa forge_drawlist_ -- --nocapture --test-threads=1
cargo test --locked -p eqoa forge_drawlist_child_clip_differs_from_background_gpu -- --nocapture --test-threads=1
cargo test --locked -p eqoa forge_drawlist_all_shape_families_have_distinct_fill_outline_exterior_gpu -- --nocapture --test-threads=1
$env:EQOA_FORGE_SPIKE='C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua'
cargo test --locked -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1
git diff --check
```

Initial test-only RED exposed missing AddRect/GetWindowDrawList and accepted
out-of-range packed color. That first run aborted at a test-fixture native
"Missing End" assertion: its host-owned window was not closed when the missing
getter failed. The fixture now closes that host window before propagating failure;
production scope unwind was not implicated. No claim that all9 RED tests completed.
Raw output is ignored `target/drawlist-prep/red.log`.

Focused clean-source GREEN: **9 passed, 0 failed**, no prerequisite skips. Tests
cover prior-frame, mismatched/ended/reopened window/child/callback handles,
destroyed userdata, restored Context mismatch, bounds/signed colors, same-frame
quota across callbacks, 100 realistic auto balls and excessive large geometry.

Three actual WGPU readback tests execute: every shape family in outlined and
filled form with distinct interior/outline/exterior coordinates; child clip versus
background at native cursor-derived coordinates; failed owner's child handle
unwinds before a separate healthy Host/Bridge draws its green rectangle in the
same Context/target. These are native GPU tests, not OS input/DPI screenshots.

Both deliberate wrong-source controls actually failed as intended:

- Window list redirected to background: child-clip test failed **child interior**.
- AddRectFilled emission omitted: family test failed **filled interior26,76**.

Each negative ran one test, with0 passes/1 failure. Clean source was restored after
each, byte-identical SHA256 to the clean snapshot before the final suite.
Logs: `negative-target.log`, `negative-fill.log`, `green.log` in ignored prep path.

An initial restored-source serial run reused the omitted-fill negative binary:
`Copy-Item` preserved the clean snapshot's older file timestamp, and Cargo reported
Finished in0.54s without compiling. That invalid clean-verification attempt had
**105 passed/3 failed**, precisely the three new GPU tests using AddRectFilled.
Source hashes and direct inspection confirmed both negative mutations were absent.
The source timestamp was refreshed to force recompilation before the final serial
run. Do not treat `addons-final.log` as clean-source acceptance. The authoritative
restored run is `target/drawlist-prep/addons-restored.log`.

Final pinned serial suite after explicit recompilation: **108 passed, 0 failed,
0 ignored**,132.08s. This includes all9 draw-list tests and both restored negative
oracle tests GREEN. Four Windows symlink operations manually skipped with error1314
(reported by the harness as passing tests, not ignored). No GPU/pinned-source/PNG
prerequisite skips. Existing pinned grid, unchanged hearts/four PNG GPU cases,
runtime recovery and real allocator post-upload rollback all executed.

Touched Rust passed `rustfmt --check`; `git diff --check` passed. No compiler/test
process from this slice remains. Compiler is released and this worker is idle.

No separate native/WASM/full-workspace/release builds: all changes remain within
the existing native-only bridge with no external cfg/signature change. Complete
unchanged UI suites, remaining fonts/audio/draw-list APIs, packaged layout/input/DPI
and live server/data acceptance remain subsequent gates. Budget steering stops
work at this drawing checkpoint; no next phase is started.
