# Round 2.5: CI and bundled UI gates

Branch: `chore/r25-gates`, from develop `264a5fb`. Scope: item 1 only.
Items 3 and 8 await the options work/integration; no options, HUD, binds or shared Play files changed.

## CI

`public-source.yml` runs on every develop push, including documentation-only pushes, and on
develop/enhanced pull requests. Its mandatory optimized allocation step runs before the release
build. `enhanced-dev.yml` also runs it before building/publishing Nightly. Existing tests,
footprint, tooling, build and publishing checks remain in place.

The step runs exactly
`play::fork_hooks::native_tests::performance_busy_world_zero_addons_frame_and_packet_allocation_bound`
with `cargo +1.99.1 test --release --locked -p eqoa --bin eqoa --target x86_64-pc-windows-msvc`,
`--exact --test-threads=1 --nocapture`, and a scratch `EQOA_CONFIG`. A failed command or a result
other than exactly one passing test fails the step. There is no timing threshold. Repository
branch-protection settings are outside this worktree; no claim is made that they were changed.

The busy-world fixture covers 200 entities, 200 abilities and six group members. After warmup,
300 world frames, 300 menu frames, 600 entity packets and 600 chat lines must each allocate
**zero** times; the old per-frame/per-packet allowance of one allocation is removed. The test also
requires zero full native-view builds. Its allocation-meter negative control allocates a value
and requires the meter to detect it.

The workflow regression test checks both jobs and rejects debug-only or failure-swallowing
mutations. It also checks the exact single-test selector and the nonempty-result guard.

## Game-shaped fixtures and pictures

The shared bundled UI helper constructs a native `Play`, supplies stats-channel health bytes,
native player/entity/group/effect/ability/hotbar/journal/chat data, and calls the production
`forge_snapshot` path with normal manager demand. It disables the test-only demand override,
which would otherwise force full views and bypass the cache. The existing Wereoxxs, ARPG and
Modern fixtures now use this helper instead of hand-building mod snapshots. The regression test
checks cache reuse and changed entity data after the begin/end packet hooks; disconnect is its
unavailable-state control.

There is one GPU picture-comparison test per bundled UI. Each renders four frames of the embedded
package, reads back a 1024x768 RGBA image, and compares it byte for byte with a separately reviewed
PNG from `EQOA_BUNDLED_UI_GOLDENS/<addon-id>.png`. It never automatically blesses output. Current
captures go to the shared Cargo target as `r25-<addon-id>.png`. A second capture drops the addon
layer and must differ; when a golden is present, that blank capture must fail comparison too.
Readback follows `addons/gpu_test.rs`; picture equality and changed-image controls follow the
renderer picture-test pattern.

Reviewed baseline images are not supplied in this checkout. Missing baselines emit explicit
`SKIP` lines, counted below; generated captures alone are **not** evidence of visual parity.
GPU absence also emits a `SKIP`. A configured missing/invalid/mismatched baseline fails.
These captures contain the bundled packages' file textures and fonts, but no retail disc HUD
atlas; blank ability icons are an explicit fixture limit, not verified game-art parity.

## Validation

Validation is in progress. Final counts and remaining limitations will be filled after the runs.
Tooling: **97 passed** on the final run, including both workflow negative controls. Footprint:
**PASS** against the pinned base, with no policy or budget changes.

The initial optimized eqoa run was **1806 passed / 2 failed / 30 ignored**. Both failures were
in new test setup: Wereoxxs initializes before drawing, so visible geometry is now required
after the four-frame warmup. The workspace run showed that warmup alone was insufficient:
the bundled Wereoxxs package intentionally defaults every replacement window off. The picture
fixture now explicitly enables its eight visible panels after initialization, matching the
existing Wereoxxs menu/pause GPU fixture. The allocation fixture warmed only the snapshot, leaving input
initialization inside the measured first frame. That produced exactly one 1024-byte allocation
across 300 frames (the first `observe_report` calls `is_dpad_rotated`, initializing
`InputState::default` and its preallocated key-event ring). The fixture now warms the complete
frame path once. Zero remains the steady-state allocation assertion; no tolerance was added.
These adjustments require the subsequent final runs; the initial failures are retained here.

First full workspace run (`cargo test --locked --workspace --target
x86_64-pc-windows-msvc --no-fail-fast -- --test-threads=1 --nocapture`): **2162 passed /
3 failed / 32 ignored**, **268 explicit SKIP lines**. The corrected allocation test reports
zero allocations **and bytes** for world/menu/entity/chat paths; the Play cache/packet test passes.
Failures: the new Wereoxxs picture fixture described above, plus
`the_foot_of_a_wall_darkened_and_open_ground_not` and
`the_same_at_freeport_as_at_the_origin` in `render/tests/ao.rs`. The latter two are the known
NVIDIA/upstream failures already documented in the sync handoff; this run leaves them visible
instead of filtering them. No renderer code or assertions were changed.

At 23:21 Eastern the run released the shared build lock. Per the lead's instruction,
further Cargo commands are paused until options-tidy finishes its full workspace and release
validation. The pending Wereoxxs fixture adjustment and CI output-path fallback still need
verification, followed by the optimized allocation gate, wasm check and final release build.

All Cargo commands use `C:/Users/hocka/Paseo/targets/shared`, the shared mkdir build lock with
a bounded acquisition wait, and scratch configuration. No worktree target directory is created.
Footprint base: `91f9e72dd6d111e20a37b73cea461444a18ad79c`; no budgets changed.
