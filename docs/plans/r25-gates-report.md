# Round 2.5: CI and bundled UI gates

Branch: `chore/r25-gates`, from develop `264a5fb`. Scope: items 1, 3 and 8.
The lead authorized merging options-tidy source `96b1e37` for integrated validation.
Shared gameplay edits in this branch come from that merge; this worker adds tests, CI and policy records.

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

Wereoxxs enables all eight panel toggles using a fresh ImGui layout; no player profile is loaded.
Reviewed baseline images are not supplied in this checkout. Missing baselines emit explicit
`SKIP` lines, counted below; generated captures alone are **not** evidence of visual parity.
GPU absence also emits a `SKIP`. A configured missing/invalid/mismatched baseline fails.
These captures contain the bundled packages' file textures and fonts, but no retail disc HUD
atlas; blank ability icons are an explicit fixture limit, not verified game-art parity.

## Validation

Final integrated checks:

| Check | Result |
| --- | --- |
| Workspace (unfiltered, no-fail-fast) | 2165 passed / 2 known AO failures / 32 ignored; 269 explicit SKIP lines |
| Client binary within that suite | 1810 passed / 0 failed / 30 ignored |
| Bundled picture capture and missing-layer controls | 3 passed; 3 golden-comparison SKIPs; 0 GPU skips |
| Tooling | 97 passed |
| Pinned footprint | PASS, 344 owned / 42 shared |
| Optimized zero-addon gate | Running |
| Wasm check | Pending |
| Release build | Pending |

Rust pass counts include fixture tests that print SKIP and return; the 269 SKIP lines are
reported separately and must not be read as verified disc/oracle/golden coverage.

Earlier validation history (kept to document the negative cases and their fixes):
Tooling: **97 passed** on the final run, including both workflow negative controls. Footprint:
**PASS** against the pinned base. The subsequent item-3 policy changes are detailed below.

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

The lead released the options-tidy validation hold on November 10. The pending picture fix
was committed before merging that branch, as requested. On the merged source, all three
picture fixtures render and their missing-layer controls pass: **3 Rust tests passed,
3 golden-comparison SKIPs**, no GPU skips. Final integrated suite/optimized/wasm/release
results are recorded in the validation table above.

## Footprint records and round counters

Pinned policy base remains `91f9e72dd6d111e20a37b73cea461444a18ad79c`.
The map family no longer has fork-owned exemptions. Twelve differing paths now have exact
shared budgets, including new files under that family counted in full. Identical tracker,
groupmark, mapsuggest and other map-family paths have no dormant allowance: an edit to an
unlisted shared file fails review. Ten obsolete budgets were removed and twelve existing
budgets tightened; **no existing budget increased**. Both options hook-record corrections
(`aa6cd8e`, `6d2666e`) survive unchanged. FORK.md and FORK-OWNERSHIP.md now agree with the manifest.
The checker passes with **344 fork-owned / 42 shared changed files**. A negative control
reducing map/mini.rs's allowance by one line fails with `exceeds budget`.

Line counts use the checker's exact `git diff --no-renames --numstat` semantics against the
pinned base; the reported totals sum its `SHARED` rows. Before = develop `264a5fb`, after =
merged source `c9dd806` plus the policy-only correction `ce814af`.

| Measurement | Before | After |
| --- | ---: | ---: |
| Actual checker totals under the policy then in force | +1995 / -424 (31 files) | +6249 / -394 (42 files) |
| Same corrected classification applied to both snapshots | +6657 / -509 (46 files) | +6249 / -394 (42 files) |
| Total changed shared lines, same classification | 7166 | 6643 |

The raw checker total rises because the old policy hid map-family code. On the same
classification the round removes **408 added lines and 115 deleted-line divergences**,
**523 fewer changed lines** overall. Classification changes do not add gameplay code.

Dry-run merges used `git merge --no-commit --no-ff` in a disposable shared clone, first at
`264a5fb`, then at `c9dd806`, against pinned `origin/master`
`986206b69cd432b6d9be21b9a1a08c2b0051d6bb`. The scratch clone used the repository's
`merge.ours.driver=true` for its fork-owned README/CLAUDE rules. Both merges were aborted and
clean state verified. Conflicted files: **8 before -> 7 after**, matching the options worker.
The touch.rs conflict disappears. Remaining conflicts: hud.rs, map/chrome.rs, map/mini.rs,
options.rs, play.rs, settings.rs and viewer.rs. No merge into develop or upstream was performed.

All Cargo commands use `C:/Users/hocka/Paseo/targets/shared`, the shared mkdir build lock with
a bounded acquisition wait, and scratch configuration. No worktree target directory is created.
No worktree target folder was created. The scratch merges changed no protected branch.
