# Owner-fix independent check

**PASS** — requested verification passed with the timing workloads excluded. Branch `feat/eqoa-api-r4`, HEAD `8111306a`; review range `18f1d4b5..8111306a`.

## Negative controls

- Replaced the minimap's positive-area guard with `if true then`. `every_bundled_mod_runs_a_play_session_with_zero_manager_diagnostics` failed: 44 `claim_pointer` invalid rectangle diagnostics, 0 passed / 1 failed. This is the expected failure.
- Restored the Lua file, replaced `capture_without_frame` with a no-op, and ran `round_minimap_shows_no_square_frame_inside_the_circle_with_the_discs_art`. It failed on grey frame pixels inside the outdoor circle, 0 passed / 1 failed. This is the expected failure.
- Ran `git checkout -- .` after the controls and confirmed `git status --short` empty before this report was created.

## Shared seam

`git diff 11aa5f0 -- crates/eqoa/src/map/mini.rs`:

```diff
@@ -557,13 +557,16 @@ impl Map {
                 plain(ui, white, diamond(q, rr).map(f), color, cut);
             }
         }
+        crate::fork_ext::map::overlay(ui, white, cx.world, |at| f(to_px(at)), [a[0], a[1], b[0], b[1]]);
         let mut r = l.assets.raster(ui);
         if let Some(vui) = cx.vui {
             // the border, a window's frame round it, placed as the HUD's right side is
             r.ui.anchor = Anchor::Right;
             r.ui.vertical = Vertical::Top;
             r.ui.shift = moved;
+            let framed = r.ui.quads.len();
             vui.blit_frame(&mut r, [FRAME[0], FRAME[1]], [FRAME[2] - FRAME[0], FRAME[3] - FRAME[1]], &vi::ui::frame_sets()[0], 3, 1.0, true);
+            crate::fork_ext::map::capture_without_frame(&mut r.ui.quads, framed);
             // north, up: the compass turns with the camera, the minimap does not
```

Three added lines, no removals. The calls describe generic overlay and frame capture operations, with no Lua or mod names; they are neutral and minimal for these two hooks. The existing `fork_ext` namespace is fork-specific.

## Changes to pre-existing tests

- `forge_icon_real_disc_ui_and_item_icon_pixel_match` and `forge_icon_real_disc_enumerated_ability_icons_resolve`: disc path now comes from `EQOA_ISO`; no assertions removed. When the variable is absent they skip rather than using a hard-coded local disc; this reduces default fixture coverage, but this check sets it.
- `round4b_manager_map_mods_large_journal_gpu_captures_and_frame_cost`: Export/Import click coordinates track the scaled panel; no assertions removed. Filtered out of our suite runs because it measures frame cost.
- `round_minimap_dot_card_wheel_zoom_and_n_toggle`: added inside and far-point projection assertions; shared `Rm` fixture now supports zoning, default off.
- `round4c_frame_time_with_all_map_mods_in_a_busy_world`: real art selection and CPU/GPU timing added; assertions retained. Filtered out of suite runs as requested.
- `combined_tables_preserve_the_upstream_prefix_and_own_every_row_once`: the prior `fits(pages())` assertion was removed and replaced with `fits(upstream_pages)` plus an explicit assertion that the combined pages do not fit the old row limit and new page/content checks. The fit assertion is narrower, intentionally reflecting the taller merged fork page. The compile-time equivalent is likewise narrowed.
- `the_fork_pages_and_rows`: added merged-page ordering and screen-window bounds assertions; retained row traversal and checks.
- `tools/test_check_no_local_paths.py` is new in this range, not a changed pre-existing test.

## Gates

Both cargo suites use the linked disc, fresh empty per-run configuration, one test thread, the shared target and mkdir lock, and `--nocapture` for SKIP counting. They add `--skip round4c_frame_time_with_all_map_mods_in_a_busy_world --skip round4b_manager_map_mods_large_journal_gpu_captures_and_frame_cost` to respect the instruction not to run frame-time benchmarks. Accordingly these are filtered suites, not the literally unfiltered commands.

| Check | Passed | Failed | Ignored | SKIP lines |
|---|---:|---:|---:|---:|
| `fork_ext::` | 541 | 0 | 26 | 9 |
| Full client, timing tests excluded | 2098 | 0 | 32 | 49 |

Footprint against reviewed base `11aa5f088bb52d5f306b0224e37b782b1bf4503d`: PASS, 31 shared paths. `check_no_local_paths.py`: PASS. `test_player_release.py`: PASS, 20 tests. No worktree target directory was created.

The fork suite filtered 1,565 tests in total, including 2 timing tests; the full suite filtered exactly 2 timing tests. The 49 full-suite SKIP lines include unavailable 2002-disc, client module, live S2 and save-state prerequisites as well as the nine fork fixture/privilege skips. SKIP lines are not successful coverage. The full run took 1,346.61 seconds.

Indoor disc-art capture and indoor zoom are not established by these checks; oracle and other missing prerequisites remain represented by skips. Owner live playtest and actual frame-rate comparison are outside this check.


