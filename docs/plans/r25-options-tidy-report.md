# Round 2.5 options and upstream tidy

Branch: `chore/r25-options-tidy`, from `develop` at `264a5fb`.
Reviewed base remains `91f9e72dd6d111e20a37b73cea461444a18ad79c`.
Comparison with newer `origin/master` is pinned to `138a5a80dffca6c9779c71cde4206dad50c954d0`.
No push, merge, tag, API addition, or budget increase.

## Assigned items

- **2, options:** restored reviewed upstream UI, Map, Textures and all other page tables, row order and IDs 41–45. Two fork pages follow upstream's last page: Enhanced HUD (six rows) and Mods & Addons (four native rows, three on web, including the independent Quest Markers toggle). Construction and fork walk/consistency tests live in `fork_ext/options.rs`. Target of Target and Mana Bars now use private IDs 1031/1042; these UI IDs are not serialized. Every saved key remains supported. The whole upstream `options.rs` test module is byte-identical to the reviewed base: its walk still ends on upstream's `Map, the last category` assertion, and its table tests iterate `EXTENDED_ITEMS`. `fork_options::pages()` returns upstream's tables to upstream fixtures and the complete runtime tables (upstream pages plus the two fork pages) to the fork's own tests and consistency check. `options.rs` differs from the reviewed base only at module, category, row, action/value and test hook lines.
- **4, ordering/formatting:** restored the upstream position of `Pick::Raid`, the compact upstream `mini_view` formatting (retaining the fork's entity-dot data), and reflowed `binds.rs` lines. Restored the Minimap binding description to the Map page. No whole-file formatting of shared code.
- **5, duplicate implementations:** `groupmark.rs`, `mapsuggest.rs` and `touch.rs` now match reviewed upstream exactly. GM map travel is retained in `fork_ext/play.rs`. Suggestions use upstream's name/note edit boxes for keyboard and controller; removed the inline fork form and its routes/screenshot switch. Restored upstream quest-toggle implementation order and documentation; existing master/part toggle behavior remains. Fork-owned tests cover touch → libpad → quick-chat directions and touch hold → map group waypoint.
- **6, defaults:** upstream default constants, serialization comparisons and tests again use 24/100. One production hook runs the fork-owned migration before upstream settings loading. With no `first_run_defaults_applied=on` marker in `enhanced.opt`, missing display/distance keys become explicit 64/650 in `client.opt` and existing per-window files. An out-of-range value counts as missing, so a hand-edited or older `entity_slots=16` / `entity_view=50` migrates to 64/650 instead of silently falling back to upstream's 24/100. Explicit choices in the accepted range, including 24/100, stay exact; unrelated file content is retained. The marker is written after migration writes succeed; a failed marker write is reported once per process, stays absent and retries on the next launch. Later omissions use upstream 24/100. Tests cover new installs, old default-only files, partial values, invalid values, explicit retail choices, secondary windows, unknown keys, marker-present controls and failed-write retries. Upstream unit fixtures seed an already-migrated marker through one test hook; the restored removed-row test disables the fork-only keys through another.

The lead explicitly deferred upstream's newer touch commit `e8d7c7d` on 2026-11-09: do not advance the reviewed base or increase budgets for it. **Item 5 is incomplete for that newer implementation until a sync moves the base.** It supplies separate stick/button sizes (50–200%, constrained by available room), opacity (10–200%), edge inset (0–10% of the shorter window side), migration from `touch_size`, and Controller-page feedback for sizes that cannot fit. The reviewed touch implementation retains the existing 50–100% whole-pad slider. Its unused fork `dpad` field was removed; upstream already keeps all D-pad buttons available over windows.

## Validation

All Cargo commands used `CARGO_TARGET_DIR=%USERPROFILE%/Paseo/targets/shared`, the mkdir build lock (with a 40-minute acquisition timeout), two build jobs, one test thread and a scratch `EQOA_CONFIG`. Native builds/tests used `RUSTFLAGS="-C target-feature=+crt-static"`; WASM used its ordinary target flags. No worktree `target` directory was created.

| Check | Result |
|---|---|
| Focused `cargo test --release --locked -p eqoa --bin eqoa <filter> -- --test-threads=1` | **121 passed, zero failed/ignored**: `settings::` 56, `options::` 25, `binds::` 23, `touch` 17 |
| `cargo test --locked --workspace --no-fail-fast -- --test-threads=1` | **2,168 passed, two known AO failures, 32 ignored** |
| Client target (`eqoa` bin) in that run | **1,813 passed, zero failures, 30 ignored** |
| `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | **PASS** |
| `python -m unittest discover -s tools -p test_fork_footprint.py` | **13 passed** |
| `python tools/check_fork_footprint.py --base 91f9e72dd6d111e20a37b73cea461444a18ad79c` | **PASS** |
| `cargo build --release --locked -p eqoa` | **PASS** (4m 20s; source `5ff1a9c`) |

The focused and workspace runs used the required `--test-threads=1` without `--nocapture`, so fixture SKIP lines were not counted; the 32 ignored tests are statically `#[ignore]`d. Fixture-dependent tests that print SKIP still return success without exercising their fixture; missing disc/module/oracle fixtures and host symlink privileges remain limits.

The full workspace gate is **not green**. The only failing target is `render --test ao`: `the_foot_of_a_wall_darkened_and_open_ground_not` reads `[127; 3]` instead of `[128; 3]`, and `the_same_at_freeport_as_at_the_origin` reports factor difference 54 on NVIDIA GeForce RTX 3070 Ti / Vulkan. These exactly match the untouched-reviewed-upstream control already recorded in [the sync report](sync-2026-11-09-report.md). No renderer code or assertion was changed.

Earlier in the round the unchanged upstream `options_kept_between_runs` fixture failed because its refusing store hid the seeded marker. `testing::reset` now seeds an already-loaded upstream-default client when the store refuses reads, and the real failed-migration retry test still exercises 64/650 with storage refusal and passes. Review item M4 remains: that upstream assertion is satisfied by the fork's test hook rather than by upstream's read path alone.

### Upstream-owned tests changed

All upstream test functions are present. The complete `options.rs` test module is byte-identical to `91f9e72`. The remaining differences are fixture row/key substitutions and one test-only setup hook per restored fixture; no upstream assertion is deleted or inverted.

| File | Test | Before this fix (branch `48d6cc5`) | Now |
|---|---|---|---|
| options | `the_extended_pages` | ended `fork_options::assert_after_upstream_walk(10, 11)`, inverting the end-of-list check | upstream `assert_eq!((w.cat_sel, cues.last()), (9, Some(&9)), "Map, the last category")` |
| options | `every_row_on_one_page_and_no_page_scrolls` | built `all` and checked labels from `fork_options::items()` | checks `EXTENDED_ITEMS` |
| options | `every_extended_row_has_its_own_item` | `fork_options::items()` | `EXTENDED_ITEMS` |
| settings | `options_with_the_lines_of_rows_gone_still_load` | deleted | restored; only added line `let _upstream = fork_settings::upstream_rows();`. Gone keys are still asserted dropped and every kept line retained |
| settings | `client_text_round_trips_and_forgives` | fork appended Target of Target assertions | upstream text; the fork key is covered by `fork_ext/settings.rs::target_of_target_keeps_its_saved_key_and_rejects_invalid_values` |
| settings | `options_with_the_voice_lines_still_load` | `map_markers` substituted for `flora` | upstream `flora` text |
| settings | `options_kept_between_runs` | body unchanged; refusal assertion shadowed by the fork test hook | body unchanged; see M4 above |
| binds | `a_layout_with_the_maps_rows_reads_them` | deleted | restored; only added line `let _upstream = fork_binds::upstream_rows();`, which hides the fork's Stats Box row for this fixture |
| binds | `a_row_moves_or_swaps` | cycle target moved to `f2` | `f10` (F2 is the fork's Map search) |
| binds | `a_key_lets_go_as_what_it_was_pressed_as` | `f2` | `f10` |
| binds | `a_button_on_a_row_of_the_clients_keys` | Hide UI row | Map row; same swap assertions and assertion count |
| binds | `a_rows_button_presses_its_key` | Hide UI row | Map row |
| binds | `keys_opt_keeps_the_rows_buttons` | `button.hide_ui=r3` | `button.map=r3` |
| binds | `space_jumps_and_g_attacks_by_default` | B | Y |
| binds | `space_is_the_attack_where_it_makes_no_jump` | B | Y |
| binds | `a_layout_kept_before_space_jumped_keeps_what_was_moved` | `key.jump=b` | `key.jump=y` |
| binds | `a_captured_press_is_kept_from_the_game` | doc comment: "a key of the client's on R3" | doc comment: "the map on R3" |
| binds | `named` helper | no F2/F4 key names | F2 and F4 named keys added for the changed fixtures |

`row_named` also gains a test-only `fork_binds::visible` gate; production keeps the Stats Box row and its B key.

The new D-pad test exercises the reviewed touch overlay through libpad into all four quick-chat directions, with no-input and release/no-click controls. The map test checks the hold threshold, one request only, no release tap, and the ordinary-tap control when holds are disabled. Upstream group-waypoint and suggestion tests also ran in the green `eqoa` bin target. No physical touch-device or live-server playtest, or deterministic screenshot capture, was performed.

Static checks completed:

- `python tools/check_fork_footprint.py --base 91f9e72dd6d111e20a37b73cea461444a18ad79c`: PASS. The `fork_options::categories` record became the runtime-page/rows/legend records, the `items` count was tightened to 2, and no budget or reviewed base changed.
- `git diff --check`: PASS.
- Compared complete normalized files: touch, group waypoint and suggestion adapter exactly equal reviewed upstream; `options.rs`'s test module byte-identical to the base.
- Rustfmt applied to touched fork adapter/test files; shared files retain upstream formatting.

## Footprint

Measured with `git diff --numstat`, before at `264a5fb`, after at `5ff1a9c`. Figures are added/deleted lines, not the manifest's maximum budgets.

| Shared file | Before vs reviewed base | After vs reviewed base |
|---|---:|---:|
| options.rs | +46 / -89 | +21 / -13 |
| hud.rs | +265 / -44 | +264 / -43 |
| play.rs | +425 / -89 | +381 / -62 |
| binds.rs | +61 / -57 | +62 / -38 |
| settings.rs | +172 / -38 | +52 / -1 |
| **Total** | **+969 / -317 (1,286)** | **+780 / -157 (937)** |

For completeness, the same files against the newer pinned `origin/master`
include upstream changes that are deliberately outside this cleanup:

| Shared file | Before vs origin/master | After vs origin/master |
|---|---:|---:|
| options.rs | +80/-392 | +63/-324 |
| hud.rs | +271/-227 | +270/-226 |
| play.rs | +428/-342 | +384/-315 |
| binds.rs | +61/-57 | +62/-38 |
| settings.rs | +204/-264 | +84/-227 |

The requested five-file footprint drops by **349 changed lines**. Touch additionally drops from +11/-0 to zero; groupmark drops by 24 lines and mapsuggest by 32 lines. The map family's ownership/budget tightening belongs to the other worker.

Dry-run `git merge-tree --write-tree --name-only <revision> origin/master`: **8 conflicts before, 7 after**. Touch no longer conflicts. Remaining: `hud.rs`, `map/chrome.rs`, `map/mini.rs`, `options.rs`, `play.rs`, `settings.rs`, `viewer.rs`. This only creates Git tree objects; it does not merge or move a branch.

## Commits and remaining work

- `1756427`: item 6 initial isolation of first-run defaults.
- `aa6cd8e`: item 2 upstream pages and fork category adapter.
- `b34755c`: item 4 ordering and formatting.
- `6d2666e`: item 6 approved compatibility migration and negative controls.
- `65379c6`: item 2 two appended pages.
- `0537da9`: item 5 reviewed upstream paths, retained GM travel and touch tests.
- `0a074dd`, `96b1e37`: refusing-storage fixture correction; Stats Box binding description names Enhanced HUD.
- `eb6632f`: review H2/M1: upstream `options.rs` pagination and table tests restored; fork pages tested in `fork_ext`.
- `ff80f4d`: review H1: upstream map-layout binds fixture restored with a fork-row control in `fork_ext/binds.rs`.
- `5ff1a9c`: review H3/M2/M3: upstream removed-row settings fixture restored; out-of-range defaults migrate; marker failure logged once.

Pending: independent integration review, owner live input/UI check, and the explicitly deferred newer upstream touch sync. The known AO failures remain outstanding upstream. Items 1, 3 and upstream fix PR preparation are outside this worker's scope. Review item M4 (the refusing-store assertion depends on a fork test hook) remains open.

Raw validation logs are at `%LOCALAPPDATA%/Temp/opencode/r25-workspace-test.log`; the source worktree contains only the final report, not these temporary logs.
