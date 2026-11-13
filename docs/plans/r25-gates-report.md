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

Final integrated checks at `c7a5498` (the options merge), all run in this worktree with
`CARGO_TARGET_DIR=%USERPROFILE%/Paseo/targets/shared` and a scratch `EQOA_CONFIG`:

| Check | Result |
| --- | --- |
| Workspace (`cargo test --locked --workspace --no-fail-fast -- --test-threads=1`) | **2172 passed / 2 known AO failures / 32 ignored**; 271 SKIP lines |
| Client binary within that suite | 1817 passed / 0 failed / 30 ignored |
| Bundled picture capture and missing-layer controls | 3 passed; **3 golden-comparison SKIPs**; 0 GPU skips |
| Tooling (`python -X utf8 -m unittest discover -s tools -p "test_*.py"`) | 97 passed |
| Pinned footprint (`--base 91f9e72…`) | **FAIL**: +6149/-349 in 42 shared files, 346 owned; 2 budgets exceeded |
| Optimized zero-addon gate (exact CI command) | **PASS**: `1 passed`, zero allocations and bytes |
| `cargo check -p eqoa --target wasm32-unknown-unknown` | PASS (two unused-parameter warnings only) |
| `cargo build --release --locked -p eqoa` | PASS |
| Release packaging allowlist (CI env: `+crt-static`, commit stamp, `enhanced-dev`) | PASS |
| Notes heading check | PASS |
| Dry-run merge against `origin/master` | 7 conflicted files |

The two workspace failures are the known upstream/NVIDIA AO tests and are left visible:
`the_foot_of_a_wall_darkened_and_open_ground_not` (grey `[127,127,127]` where `[128,128,128]`
was drawn) and `the_same_at_freeport_as_at_the_origin` (factor 54, 56 at half the radius,
1227 pixels under 200), both on an NVIDIA RTX 3070 Ti Vulkan adapter. No renderer code or
assertion was changed.

The optimized gate ran exactly as `public-source.yml` does:
`cargo +1.99.1 test --release --locked -p eqoa --bin eqoa --target x86_64-pc-windows-msvc
play::fork_hooks::native_tests::performance_busy_world_zero_addons_frame_and_packet_allocation_bound
-- --exact --test-threads=1 --nocapture`, with the nonempty-result guard satisfied by
`test result: ok. 1 passed;`. Measured: menu 11.655 µs/frame, world 7.310 µs/frame,
0.069 µs/event, 0.018 µs/chat, with `0.0` allocations and bytes on every path.

**Picture SKIPs.** The SKIP count is 271 lines: 260 `SKIP: …`, 9 other `SKIP …` lines and
2 `SKIPPED: …` lines. Three of them are exactly the bundled picture tests
(`arpg_ui`, `avoids_modern_ui`, `wereoxxs_ui`), and they skip the golden comparison because
the reviewed golden PNGs are absent from this checkout (`EQOA_BUNDLED_UI_GOLDENS` is unset
and empty). Their captures are written to the shared target as `r25-*.png`, and the
missing-addon-layer controls pass, but a capture is not visual parity: no picture was
verified against a reviewed baseline here.

The packaging step was re-run on a CI-equivalent build (`RUSTFLAGS=-C
target-feature=+crt-static`, `EQOA_BUILD_COMMIT` set, `EQOA_UPDATE_CHANNEL=enhanced-dev`)
because the plain release build has no static CRT and no commit stamp; it produced the
expected `EQOA-Enhanced-Nightly-Windows-x64.zip`, stamped exe, `SHA256SUMS`, `update.txt`
and `update-notes.md` in a scratch output directory. `dev_release.py preflight` itself cannot
run on this branch: it requires the commit to be an ancestor of `refs/remotes/origin/develop`,
and this checkout's `origin` is upstream (develop is `looprook/develop`), while a feature
branch is deliberately not on develop. Its notes check (`unreleased_heading_problem`) passes,
and the tooling suite covers it.

Rust pass counts include fixture tests that print SKIP and return; the 271 SKIP lines are
reported separately and must not be read as verified disc/oracle/golden coverage. The two AO
failures are the only failures; no other target failed.

Earlier validation history (kept to document the negative cases and their fixes):
Tooling: **97 passed** on that run, including both workflow negative controls. Footprint:
**PASS** at the policy-only snapshot (`c9dd806` + `ce814af`) against the pinned base, before
the options merge. The item-3 policy changes are detailed below.

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
(`aa6cd8e`, `6d2666e`) survive unchanged. FORK.md and FORK-OWNERSHIP.md agree with the manifest.
That policy snapshot passed at `c9dd806` + `ce814af` with **344 fork-owned / 42 shared changed
files**; a negative control reducing map/mini.rs's allowance by one line failed with
`exceeds budget`.

**The merged branch HEAD does not pass its own footprint gate.** Measured at `c7a5498` with
`python -X utf8 tools/check_fork_footprint.py --base 91f9e72…`:

| Shared path | Measured | Budget | Result |
| --- | ---: | ---: | --- |
| `crates/eqoa/src/binds.rs` | +62 / -38 | +55 / -54 | exceeds budget |
| `crates/eqoa/src/options.rs` | +21 / -13 | +18 / -11 | exceeds budget |

Both paths received options-branch code at the merge (`c7a5498` adds 23 lines to
`binds.rs` and 33 to `options.rs`), while the merge kept the first parent's tightened
budgets. The second parent's manifest values (`binds.rs` 100/87, `options.rs` 46/142) were
measured on a different base and are not a valid replacement. **No budget was raised and no
source or test was changed to satisfy the gate.** A fresh measurement would raise the added
allowance by 7 lines (`binds.rs`) and 3 lines (`options.rs`) and lower both deleted allowances;
that is an owner policy decision and remains open.

The checker still reports **346 fork-owned / 42 shared changed files**. Line counts use its
exact `git diff --no-renames --numstat` semantics against the pinned base and sum its `SHARED`
rows.

| Measurement | Before (`264a5fb`) | After (`c7a5498`) |
| --- | ---: | ---: |
| Same corrected classification applied to both snapshots | +6657 / -509 (46 files) | +6149 / -349 (42 files) |
| Total changed shared lines, same classification | 7166 | 6498 |

The policy-only snapshot before the options merge measured +6249 / -394 (6643 changed lines);
the merge's restored upstream settings/fixtures account for the further drop. Against the
develop snapshot the merged round removes **508 added lines, 160 deleted-line divergences and
668 changed lines** under one classification. Classification changes do not add gameplay code.

The dry-run merge was repeated in this worktree rather than a clone: repository config already
held `merge.ours.driver=true`, so `git merge --no-commit --no-ff origin/master`
(`986206b69cd432b6d9be21b9a1a08c2b0051d6bb`) was run on a clean tree. Conflicted files:
**7** — `hud.rs`, `map/chrome.rs`, `map/mini.rs`, `options.rs`, `play.rs`, `settings.rs` and
`viewer.rs` (the options worker's 8 -> 7, with `touch.rs` gone). The merge was aborted
immediately; HEAD stayed at `c7a5498` and `git status` is clean. No merge into develop or
upstream was performed.

All Cargo commands use `%USERPROFILE%/Paseo/targets/shared`, one at a time, with a scratch
`EQOA_CONFIG`; no worktree target directory was created. The packaging check wrote only to
`%TEMP%`. The dry-run merge changed no protected branch.
