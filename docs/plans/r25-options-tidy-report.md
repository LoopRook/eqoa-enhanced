# Round 2.5 options and upstream tidy

Branch: `chore/r25-options-tidy`, from `develop` at `264a5fb`.
Reviewed base remains `91f9e72dd6d111e20a37b73cea461444a18ad79c`.
Comparison with newer `origin/master` is pinned to `138a5a80dffca6c9779c71cde4206dad50c954d0`.
No push, merge, tag, API addition, or budget increase.

## Assigned items

- **2, options:** restored reviewed upstream UI, Map, Textures and all other page tables, row order and IDs 41–45. Two fork pages follow upstream's last page: Enhanced HUD (six rows) and Mods & Addons (four native rows, three on web, including the independent Quest Markers toggle). Construction and fork walk tests live in `fork_ext/options.rs`. Target of Target and Mana Bars now use private IDs 1031/1042; these UI IDs are not serialized. Every saved key remains supported. The upstream walk text is restored except one adapter assertion for stepping past upstream's last page; registry tests use adapter hooks to include fork rows. `options.rs` differs from the reviewed base only at module, category, row, action/value and test hook lines.
- **4, ordering/formatting:** restored the upstream position of `Pick::Raid`, the compact upstream `mini_view` formatting (retaining the fork's entity-dot data), and reflowed `binds.rs` lines. Restored the Minimap binding description to the Map page. No whole-file formatting of shared code.
- **5, duplicate implementations:** `groupmark.rs`, `mapsuggest.rs` and `touch.rs` now match reviewed upstream exactly. GM map travel is retained in `fork_ext/play.rs`. Suggestions use upstream's name/note edit boxes for keyboard and controller; removed the inline fork form and its routes/screenshot switch. Restored upstream quest-toggle implementation order and documentation; existing master/part toggle behavior remains. Fork-owned tests cover touch → libpad → quick-chat directions and touch hold → map group waypoint.
- **6, defaults:** upstream default constants, serialization comparisons and tests again use 24/100. One production hook runs the fork-owned migration before upstream settings loading. With no `first_run_defaults_applied=on` marker in `enhanced.opt`, missing display/distance keys become explicit 64/650 in `client.opt` and existing per-window files. Explicit choices, including 24/100, stay exact; unrelated file content is retained. The marker is written after migration writes succeed. Later omissions use upstream 24/100. Tests cover new installs, old default-only files, partial values, explicit retail choices, secondary windows, unknown keys, marker-present controls and failed-write retries. Upstream unit fixtures seed an already-migrated marker through one test hook.

The lead explicitly deferred upstream's newer touch commit `e8d7c7d` on 2026-11-09: do not advance the reviewed base or increase budgets for it. **Item 5 is incomplete for that newer implementation until a sync moves the base.** It supplies separate stick/button sizes (50–200%, constrained by available room), opacity (10–200%), edge inset (0–10% of the shorter window side), migration from `touch_size`, and Controller-page feedback for sizes that cannot fit. The reviewed touch implementation retains the existing 50–100% whole-pad slider. Its unused fork `dpad` field was removed; upstream already keeps all D-pad buttons available over windows.

## Validation

All Cargo commands used `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, the mkdir build lock (with a 30-minute acquisition timeout), two build jobs, one test thread and a scratch `EQOA_CONFIG`. Native builds/tests used `RUSTFLAGS="-C target-feature=+crt-static"`; WASM used its ordinary target flags. No worktree `target` directory was created.

| Check | Result |
|---|---|
| Initial `cargo test --locked -p eqoa --bin eqoa -- --test-threads=1 --nocapture` | 1,805 reported passes, one fixture failure, 30 ignored; 174 SKIP lines |
| Corrected `cargo test --locked -p eqoa --bin eqoa settings:: -- --test-threads=1 --nocapture` | **52 passed, zero failed/ignored/SKIPs** |
| `cargo test --locked --workspace --no-fail-fast -- --test-threads=1 --nocapture` | **2,161 reported passes, two known AO failures, 32 ignored; 268 SKIP lines** |
| Client subset of the full workspace run | **1,806 reported passes, zero failures, 30 ignored** |
| `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | **PASS** |
| `python -m unittest discover -s tools -p test_fork_footprint.py` | **13 passed** |
| `cargo build --release --locked -p eqoa` | **PASS** (14m 20s; source `96b1e37`) |

SKIP counts are lines printed by fixture-dependent tests, many of which return success without exercising their intended fixture. They are not equivalent to successful coverage. Missing disc/module/oracle fixtures and host symlink privileges remain limits.

The full workspace gate is **not green**. The only failing target is `render --test ao`: `the_foot_of_a_wall_darkened_and_open_ground_not` reads `[127; 3]` instead of `[128; 3]`, and `the_same_at_freeport_as_at_the_origin` reports factor difference 54 on NVIDIA GeForce RTX 3070 Ti / Vulkan. These exactly match the untouched-reviewed-upstream control already recorded in [the sync report](sync-2026-11-09-report.md). That control was not repeated here; no renderer code or assertion was changed.

The initial client failure was the unchanged upstream `options_kept_between_runs` test: its refusing store hid the seeded marker, so the fixture looked like a first run. The fork-owned fixture now seeds an already-loaded upstream-default client when it refuses reads. The real failed-migration retry test still exercises 64/650 with storage refusal and passes. No upstream assertion was weakened. A cache-shape error in that fixture correction was caught by the compiler and corrected through the existing setter before the successful targeted/workspace runs.

The new D-pad test exercises the reviewed touch overlay through libpad into all four quick-chat directions, with no-input and release/no-click controls. The map test checks the hold threshold, one request only, no release tap, and the ordinary-tap control when holds are disabled. Upstream group-waypoint and suggestion tests also ran in the green client subset. No physical touch-device or live-server playtest, or deterministic screenshot capture, was performed.

Static checks completed:

- `python tools/check_fork_footprint.py --base 91f9e72dd6d111e20a37b73cea461444a18ad79c`: PASS. Only hook records changed; no budgets or reviewed base changed.
- `git diff --check`: PASS.
- Compared complete normalized files: touch, group waypoint and suggestion adapter exactly equal reviewed upstream.
- Rustfmt applied to touched fork adapter/test files; shared files retain upstream formatting.

## Footprint

Measured with `git diff --numstat`, before at `264a5fb`, after implementation at `96b1e37`. Figures are added/deleted lines, not the manifest's maximum budgets.

| Shared file | Before vs reviewed base | After vs reviewed base |
|---|---:|---:|
| options.rs | +46 / -89 | +18 / -11 |
| hud.rs | +265 / -44 | +264 / -43 |
| play.rs | +425 / -89 | +381 / -62 |
| binds.rs | +61 / -57 | +55 / -54 |
| settings.rs | +172 / -38 | +162 / -32 |
| **Total** | **+969 / -317 (1,286)** | **+880 / -202 (1,082)** |

For completeness, the same files against the newer pinned `origin/master`
include upstream changes that are deliberately outside this cleanup:

| Shared file | Before vs origin/master | After vs origin/master |
|---|---:|---:|
| options.rs | +80/-392 | +58/-320 |
| hud.rs | +271/-227 | +270/-226 |
| play.rs | +428/-342 | +384/-315 |
| binds.rs | +61/-57 | +55/-54 |
| settings.rs | +204/-264 | +194/-258 |

The requested five-file footprint drops by **204 changed lines**. Touch additionally drops from +11/-0 to zero; groupmark drops by 24 lines and mapsuggest by 32 lines. The map family's ownership/budget tightening belongs to the other worker.

Dry-run `git merge-tree --write-tree --name-only <revision> origin/master`: **8 conflicts before, 7 after**. Touch no longer conflicts. Remaining: `hud.rs`, `map/chrome.rs`, `map/mini.rs`, `options.rs`, `play.rs`, `settings.rs`, `viewer.rs`. This only creates Git tree objects; it does not merge or move a branch.

## Commits and remaining work

- `1756427`: item 6 initial isolation of first-run defaults.
- `aa6cd8e`: item 2 upstream pages and fork category adapter.
- `b34755c`: item 4 ordering and formatting.
- `6d2666e`: item 6 approved compatibility migration and negative controls.
- `65379c6`: item 2 two appended pages.
- `0537da9`: item 5 reviewed upstream paths, retained GM travel and touch tests.
- `0a074dd`, `96b1e37`: refusing-storage fixture correction; Stats Box binding description names Enhanced HUD.

Pending: independent integration review, owner live input/UI check, and the explicitly deferred newer upstream touch sync. The known AO failures remain outstanding upstream. Items 1, 3 and upstream fix PR preparation are outside this worker's scope.

Raw validation logs, command runner, counts and footprint/merge evidence are archived locally at
`C:/Users/hocka/AppData/Local/Temp/eqoa-r25-options-validation-20261110-000231.zip`. The source worktree contains only the final report, not these temporary logs.
