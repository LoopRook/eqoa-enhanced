# FAIL — Part C fix independent check

Reviewed `d20328e..fd77676`; test run includes later head `6ff7992`. The develop-native round-minimap baseline remains absent (M4), and card sizing remains unbounded for an oversized single word (M3). M5 has a moved-placement coverage gap at higher render scales. These prevent a full pass.

## Findings from round-4-review-c.md

- **M1 fixed for the tested cases:** `dot_under` calls the current-frame `pick_dot` and resolves the selected entity; the real-manager hover test covers movement, removal, nearest selection and ID ties. The stale-pick mutation below fails the movement assertion.
- **M2 fixed for the tested visible marks:** marker hover checks `markers_on`, excludes towns, processes marks in draw order and places steps on the rim; `round_minimap_marker_hover_follows_what_the_minimap_draws` checks the stated cases. The documented diagonal rim limitation remains.
- **M3 partial:** fallback placement and axis clamping work for the tested multiword name and moved HUD, but `main.lua:168-176` expressly leaves an oversized word whole; at `:186-192` the computed card can exceed the window, so clamping its origin cannot contain it. Arbitrarily many wrapped lines similarly can exceed the height. The regression uses a long name with spaces, not an oversized word.
- **M4 partial / not fixed:** the fixture supplies 200 dots and quest marks, reuses targets and waits for GPU completion, but the required develop-native round-minimap-plus-search baseline is still unavailable. The stock square is not that baseline. No timing conclusion is made here.
- **M5 partial:** the new real-manager matrix checks 1x/2x/4x, wide/tall windows, image, ring and hit positions with a synthetic picture. Move HUD is separately checked at 1x, not in the scale matrix; real indoor terrain is unavailable. The controls below demonstrate ring sensitivity at both 2x and 4x, not moved-placement sensitivity at those scales.

## Tests and negative controls

The five original `round_minimap_*` tests and the five named fix tests exist. The benchmark test also exists but was explicitly excluded from the completed suite. The ordinary suite output captures passing test stdout; the deliberate GPU assertion failures below establish that the controlled cases actually executed on a GPU rather than returning at fixture setup.

- Hover: cached the first `pick_dot` result; `round_minimap_hover_follows_this_frames_creatures_and_picks_the_nearest_dot` failed at `no card where the creature was` (exit 101). This control demonstrates stale movement detection; it did not independently reach the later despawn and nearest assertions.
- Move HUD card: removed the right-side fallback and horizontal clamp; `round_minimap_card_stays_inside_the_window_wherever_the_minimap_is_moved` failed with `left edge: card [-402.4, 645, 398, 70]` (exit 101).
- Render scale: made the ring ignore HUD scale and restricted the matrix temporarily to 2x; the scale test failed at the diagonal ring assertion (exit 101). The same mutation passed at 4x, where the 640-wide fixture has a unit HUD scale. A separate 4x-only control displaced the ring by 10 pixels and failed the diagonal assertion (exit 101). These controls do not prove detection of every render-scale-specific regression.
- Each mutation was followed by `git checkout -- .`; byte restoration and a clean `git status --short` were confirmed before the report was created. No worktree target directory exists.

## Changed pre-existing tests in d20328e..6ff7992

- `hud_specs_with_metatable_defaults_and_plain_specs_both_read_their_fields`: formatting only; both assertions retained unchanged.
- `round_minimap_circle_follows_move_hud_and_clips_to_a_circle`, `round_minimap_dot_card_wheel_zoom_and_n_toggle`, and `round_minimap_rim_arrow_only_where_the_circle_crops_the_square_mark`: use the revised shared `Rm` fixture or rectangle helper. In the dot-card test the explicit `view.project` returned `inside` assertion was removed; the subsequent geometric within-circle assertion remains, but no longer checks that API return value. This is an assertion removal, not merely formatting.
- `round4c_frame_time_with_all_map_mods_in_a_busy_world`: workload/measurement changes and two assertions added (at least 200 dots and exactly one draw); no assertion removed or loosened.
- The shared `Rm` fixture also affects `round_minimap_hides_stock_blocks_its_clicks_and_returns_when_off` and `round_minimap_carries_the_search_button_and_box_when_the_search_mod_is_on`, whose assertions are unchanged. The other test-file changes add tests or test helpers.

## Commands

- Footprint: `python tools/check_fork_footprint.py --base 11aa5f088bb52d5f306b0224e37b782b1bf4503d` — PASS, 31 shared files.
- Release tests: `python -X utf8 tools/test_player_release.py` — 20 passed.
- The requested exact `fork_ext::` command unexpectedly includes `round4c_frame_time_with_all_map_mods_in_a_busy_world`. It started; I terminated the process as soon as I observed it, so the command did not complete and no benchmark result was obtained. Subsequent runs use `--skip round4c_frame_time_with_all_map_mods_in_a_busy_world` to respect the no-benchmark instruction.
- First benchmark-excluded run: 534 passed, 1 failed, 26 ignored. `disabled_bundled_hud_mod_removes_option_key_rows_and_drawing` failed creating a PID-based scratch directory with `AlreadyExists` at `manager.rs:1013` in the shared system temp folder.
- Fresh-temp rerun with that same benchmark exclusion: 535 passed, 0 failed, 26 ignored; the previously failing scratch-directory test passed. The 10 round-minimap tests all reported `ok`.

All Cargo commands used the shared target, a fresh empty `EQOA_CONFIG`, the shared directory lock and a foreground subprocess with a 30-minute timeout. The lock was removed after each command. Long output was written to system temp logs. No commit was made.
