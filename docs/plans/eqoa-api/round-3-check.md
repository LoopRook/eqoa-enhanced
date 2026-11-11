# Round 3 independent check

Checker: independent review of `feat/eqoa-api-r3` (HEAD `9bdb80d`) against
`docs/plans/eqoa-api/round-3-brief.md` and `round-3-report.md`.
Every number below comes from a rerun in this worktree with
`CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, a scratch `EQOA_CONFIG`,
the `C:/Users/hocka/Paseo/cargo-build.lock` protocol, and the shared target dir only.

## Verdict: DISCREPANCIES FOUND

The render removal, footprint reduction and the zero-addon allocation gate are real, but the
branch does not pass CI as it stands (4 client tests, the tools test suite, and release
packaging all fail), the "picture tests" prove nothing about pixels, and the settings
migration neither persists nor reaches the mods it migrates into.

## Gates (mine vs the builder's claim)

| # | Gate | My result | Builder's claim |
|---|---|---|---|
| 1 | `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1` | **352 passed, 0 failed, 24 ignored**, 1602 filtered. `grep -c SKIP` = 3, and all three hits are test *names* containing "skip"; **0** real `SKIP:` prints | 345 passed / 0 failed / 24 ignored / 0 SKIP, 1615 filtered |
| 2a | `cargo test --locked -p eqoa --bin eqoa -- --test-threads=1` | **1943 passed, 4 failed, 31 ignored** — failures: `fork_ext::bundled::tests::{test_flat_registry_listing_all_three_packages_installs_all_three_on_fresh_mods_folder, test_fresh_mods_folder_installs_all_packages, test_from_env_setup_discovers_all_bundled_packages_on_fresh_mods_folder, test_second_mods_folder_gets_all_packages_when_first_folder_has_records}` | not reported ("full client tests at the end") |
| 2b | `cargo test --locked -p net` | **9 passed, 0 failed** (5 targets) | not reported |
| 3a | zero-addon allocation gate (release, `.github/workflows/enhanced-dev.yml:88`) | **PASS**, 1 passed, `allocs/frame=0.0` | PASS, 0.0 allocs/frame |
| 3b | `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | **OK**, 0 errors, **2 warnings** (`fork_ext/report_core.rs:12`, unused `account`/`password`) | "0 errors, 0 warnings" |
| 3c | `python -X utf8 -m unittest discover -s tools -p "test_*.py"` | **97 run, 16 failures, 9 errors** — all one root cause (H1) | not reported |
| 3d | packaging (`tools/build_player_release.py`, crt-static build) | **FAIL**: `eqoa_stats_box/main.lua` hash mismatch (H2) | "bundles all 4 mods into `eqoa-enhanced.zip`" |
| 4 | Round 3 picture tests | **FAIL as "proof of identical"**: quad counts only, no reference image, no pixel comparison (H4) | "exact match: 160 / 11 / 6 / 165 quads", "reference capture PNGs" |
| 5 | `git diff develop...HEAD --stat` + test audit | See H4/M2 — 10 tests deleted, 2 weakened, 1 assertion dropped; no upstream test missing or changed by this branch | report §C.4 lists the deletions |
| 6a | `python tools/check_fork_footprint.py --base 5d54c0c…` | **PASS** — 368 fork-owned, 42 shared, all hooks 1/1 | PASS, 369 fork-owned |
| 6b | `git merge-tree --write-tree --name-only HEAD 5d54c0c` | **clean**, tree `c39ca704d7c8593a8753facb488fd885ba38b538`, no conflicting files | clean |
| 6c | budgets vs `develop:docs/fork-footprint.json` | No unapproved raise. `bindpage.rs` added 7→10 (+3, inside the approved +4), deleted 7→5; every other file lowered | "Zero budgets were raised" |
| 7 | settings migration | 7/7 keys asserted **once** in one test, but no second-start persistence and the value never reaches the mod (H5) | "full test with negative controls" |
| — | release benchmark | **0.508 ms/frame → its own `< 0.5` assertion fails** | 0.478 ms/frame, passed |

## Findings

### HIGH

**H1 — CI's tools test step fails: the release fixture was not updated for the four new mods.**
`tools/test_player_release.py:16-75` still hard-codes the pre-round-3 `BUNDLED_MOD_FILES`
(no `eqoa_stats_box`/`eqoa_target_of_target`/`eqoa_mana_bars`/`eqoa_hud_numbers`). Every
fixture `SOURCE.txt` therefore lacks them, and `tools/build_player_release.py:160-161` raises
`ValueError("SOURCE.txt file allowlist mismatch with expected bundled mods")`. Result:
16 failures + 9 errors of 97 (`test_missing_bundled_mod_file_fails`,
`test_wrong_catalog_is_rejected_before_output`, the whole `build()` family, and all 12
`test_executable_rejects_regular_and_delayed_visual_cpp_runtime_imports` subtests). This is
the workflow step `Test release tooling` (`.github/workflows/enhanced-dev.yml:70-71`).

**H2 — release packaging is refused: a bundled mod's recorded checksum is stale.**
`bundled-mods/SOURCE.txt:59` records `603d0ba091265e606429b665b7f862f0582b95f4ed9e33e617821ac75d113a92`
for `eqoa_stats_box/main.lua`; the file hashes to
`cf3972ab5b67b7fd30eb1fe4eac2897e6f85b016a29ee77250c45ded3de9f239` (edits landed after the
checksum was written). I reproduced this on a clean detached worktree of HEAD with the CI
environment (`RUSTFLAGS=-C target-feature=+crt-static`, `EQOA_BUILD_COMMIT`, `EQOA_UPDATE_CHANNEL`):
`build_release` aborts on that hash. All other recorded hashes match.

**H3 — the full client suite fails four tests that the branch made stale.**
`crates/eqoa/src/fork_ext/bundled.rs` still asserts the old four-package list at lines
923, 1208, 1246 and 1336:
`left: [wereoxxs_ui, arpg_ui, avoids_modern_ui, eqoa_example_hearts, eqoa_stats_box, eqoa_target_of_target, eqoa_mana_bars, eqoa_hud_numbers]`
vs `right: [wereoxxs_ui, arpg_ui, avoids_modern_ui, eqoa_example_hearts]`.
CI's `cargo test --workspace` step would stop here.

**H4 — the picture tests do not compare pictures and have no reference image.**
`round_3_picture_tests.rs` asserts only quad *counts* against a drawing reimplemented inside
the test file, and the "reference capture" is a blank RGBA buffer with a text tag:

- `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:299` — `assert_eq!(mod_quads, native_quads, "mod and native stats box quad counts match at {size:?}");`
- `…:367` — `assert_eq!(mod_tot_quads, native_tot_quads, "mod and native ToT quad counts match at {size:?}");`
- `…:436` — `assert_eq!(mod_mana_quads, native_mana_quads, "mod and native mana bars quad counts match at {size:?}");`
- `…:513` — `assert_eq!(mod_num_quads, native_num_quads, "mod and native hud numbers quad counts match at {size:?}");`
- Negative controls only count `0`: `…:307`, `…:375`, `…:444`, `…:521`.
- `…:233-251` `save_reference_capture` writes `img[i] = b` where `b` is the bytes of
  `format!("{name}:{quads_count}")`; the rest of the image stays zero. The PNGs in the target
  dir really do start with `stats_box:160`, `target_of_target:11`, `mana_bars:6`,
  `hud_numbers:165` — a tag, not a rendering.
- No reference image is in the repo: `git log --all --oneline -- '*r3-native*'` is empty.
- The "native" side is not the native code either. At Stage B (`7ebf6a1`) the stats box called
  `crate::statbox::draw`, but Stage C (`8985f38`) replaced it with `native_statbox_draw`
  (`…:161-188`) and `native_hud_numbers_draw` (`…:190-231`) written into the test after the
  native code was deleted, and ToT was hand-drawn from the start (`…:346-355`). So even the
  quad counts compare a mod against a test-local copy, not against the shipped native drawing.

**H5 — the settings migration is lossy and never reaches the mods.**
- Mod option values are a thread-local in-memory list with no file behind them:
  `crates/eqoa/src/fork_ext/addons/options.rs:17-19` (`static TOGGLES: RefCell<Vec<ModToggle>>`),
  `:80-99` `set_toggle` only mutates that vector. Nothing writes or reads a mod-settings file.
- The Lua API always uses the addon id `"mod"` (`crates/eqoa/src/fork_ext/addons/native_state.rs:833,849,858`),
  so the bundled mods read their toggle as `("mod","stats_box")`
  (`bundled-mods/eqoa_stats_box/main.lua:10,25`) while migration writes
  `("eqoa_stats_box","stats_box")` (`crates/eqoa/src/fork_ext/settings.rs:167-190`). The
  migrated value is unreachable even in the same process.
- Migration then strips the legacy keys from `client.opt` (`fork_ext/settings.rs:199-205`), so
  the player's saved choice is destroyed and the next start falls back to the mod defaults.
- The test `migration_moves_legacy_options_and_binds_into_mods_and_strips_files`
  (`fork_ext/settings.rs:372-423`) asserts all seven old keys land (lines 384-392), the strip
  (396-406) and an in-process "second run" (409-411) — but nothing reloads the toggles from
  disk, because there is nowhere to reload them from. So gate 7's "survives a second start" is
  not proven, and by the code it cannot hold.

**H6 — the Stats Box and Target of Target now default ON, changed from today's behaviour.**
`bundled-mods/eqoa_stats_box/main.lua:4` (`local is_open = true`) with
`eqoa.options.toggle({ …, default = true })` at `:10`, and
`bundled-mods/eqoa_target_of_target/main.lua:6` (`default = true`). Native defaults were
`stats_box = false` and `target_of_target = false` (`git show develop:crates/eqoa/src/settings.rs`,
`ClientOptions` defaults). The brief says both are "off until turned on".

### MEDIUM

**M1 — the Enhanced HUD rows are static, not per-mod.** `crates/eqoa/src/fork_ext/options.rs:11-14`
hard-codes `[2000..=2005]` and the labels at `:67-77`; `active_mod_items`/`active_mod_item_ids`
and `unregister_addon_toggles` (`addons/options.rs:58,139-149`) have no caller. So disabling a
mod in the Addon Manager does not remove its option rows, and a non-bundled mod can never get a
row — contrary to brief A5/§6 and report A5. `set_toggle` also keys everything under `"mod"`
(`native_state.rs:833`), so two mods with the same option id collide.

**M2 — tests removed without equivalent coverage.**
`crates/eqoa/src/hud.rs`: deleted `mana_bars_sit_under_the_health_they_go_with` and
`the_target_of_target_bar_sits_above_the_targets`; `crates/eqoa/src/play.rs`: deleted
`the_target_of_target_is_the_targets_channels`; `crates/eqoa/src/fork_ext/settings.rs`: six
tests deleted (`stats_box_kept_only_when_on`, `mana_bars_kept_only_when_off`,
`target_of_target_keeps_its_saved_key_and_rejects_invalid_values`,
`combined_old_save_keeps_fork_and_upstream_keys_and_drops_unknowns`,
`default_number_settings_write_nothing_and_off_keeps_legacy_keys`,
`unknown_keys_and_invalid_number_values_leave_defaults_alone`) and one added;
`crates/eqoa/src/fork_ext/binds.rs`: file and its round-trip test deleted (replaced by two
tests in `fork_ext/bindpage.rs`). The geometry/data checks (bar sits above the target, bars
under their health, ToT is the target's channels) have **no** replacement — the quad-count
tests cannot catch a moved bar or swapped data. `fork_ext/hud.rs` `forge_hud_*` tests keep
their asserts but move expected quads 40/38 → 29/27; `fork_ext/options.rs`
`number_toggles_change_only_their_own_saved_setting` drops its final assertion
(`!(target_hp_percent, xp_details, hud_numbers)` unchanged after `activate(9999)`).

**M3 — the release benchmark fails its own gate here.** `round_3_all_four_mods_per_frame_performance_benchmark`
measured **0.508 ms/frame** (507.6 µs) vs the builder's 0.478 ms, tripping
`assert!(per_frame_millis < 0.5)` (`round_3_picture_tests.rs:594-599`). The loop
(`:578-583`) only clears and runs Lua `render` callbacks; `hud_layer::draw` (the actual
drawing) is never timed, so even a passing number would not be "per-frame cost of the four mods".

**M4 — report overstates build hygiene.** wasm `cargo check` emits 2 warnings and the release
build emits 72 warnings; report claims "0 errors, 0 warnings" for both.

### LOW

- `crates/eqoa/src/fork_ext/settings.rs:96-110`: `UPSTREAM_ROWS` is now written and never read
  (the two upstream tests that used `fork_settings::upstream_rows()` lost that line), dead code.
- `crates/eqoa/src/fork_ext/bundled.rs` test expectations aside, the four new packages are
  embedded correctly and the bundled Wereoxxs/ARPG/Modern picture tests still pass.
- An untracked `docs/plans/eqoa-api/round-3-review.md` in this worktree makes
  `build_player_release.py` refuse with "Release source must be clean"; not a branch defect,
  but it is why packaging has to be checked from a temporary clean worktree.

## Upstream test integrity (gate 5, `git show 5d54c0c:<file>`)

All upstream test functions in `crates/eqoa/src/{hud,play,settings,binds,bindpage,viewer,options}.rs`
are present in HEAD (9/82/46/26/7/4/20), and **this branch changed none of them**: comparing
test bodies against `5d54c0c` shows no round-3 edits, and two fork-modified upstream bodies were
actually restored (`settings.rs options_with_the_lines_of_rows_gone_still_load`,
`binds.rs a_layout_with_the_maps_rows_reads_them` lost their `fork_*::upstream_rows()` line).
The remaining divergences from upstream (`hud.rs a_pointer_finds_the_pieces_where_a_tall_window_put_them`
+`Pick::Report`; `play.rs npc_pc_display_agrees_with_the_server` and
`hidden_the_huds_buttons_take_no_click`; `binds.rs` hide_ui→map renames; `bindpage.rs` two tests)
already exist on `develop` and predate round 3 — reported for the record, not as a round-3 defect.

## Bottom line

Blocking before merge: H1, H2, H3 (CI red), H4 (the brief's "proof of identical" is absent),
H5/H6 (player-visible settings regression). The footprint, merge cleanliness and the
zero-addon allocation gate are genuinely good.
