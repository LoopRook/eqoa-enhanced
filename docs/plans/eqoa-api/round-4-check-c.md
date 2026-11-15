# FAIL — Part C independent check

Reviewed `3a096b7..c9a2dca`, head `c9a2dca`. The owner brief's develop-native frame-time comparison was not measured, as the Part C report acknowledges; quest-step diagonal rim marks and real indoor/terrain parity remain unverified or incomplete. The executable checks below do not remove those gaps.

## Named tests and controls

The fresh-config addon run listed all five named `round_minimap_*` tests as passing: `hides_stock_blocks_its_clicks_and_returns_when_off`, `circle_follows_move_hud_and_clips_to_a_circle`, `dot_card_wheel_zoom_and_n_toggle`, `rim_arrow_only_where_the_circle_crops_the_square_mark`, and `carries_the_search_button_and_box_when_the_search_mod_is_on`. The wheel and N requirements use the third test. The world-map render-scale test is outside this filter. Cargo output captures passing tests' diagnostic output, so the suite's status alone does not establish whether GPU tests skipped internally.

## Changed pre-existing tests

Comparing the two commits (deleted tests necessarily lose their assertions):

- `fork_ext/addons/async_json.rs`: `round4_shared_catalogue_retains_forge_pin` changes equality syntax, no assertion weakened.
- `fork_ext/addons/map_mod_manager_tests.rs`: `round4b_manager_map_mods_large_journal_gpu_captures_and_frame_cost` adds minimap setting setup; no assertion removed.
- `fork_ext/map_mod_tests.rs`: `round4b_search_selects_live_with_dots_follows_and_falls_back_to_known` and `round4b_search_group_departure_places_and_unicode_editor_limit` add setup; no assertion removed.
- `fork_ext/bindpage.rs`: `fixture_frees_f2_while_production_keeps_map_search_on_it` deleted and replaced by `controls_no_longer_list_the_map_search_keys_natively`; the F2/default/fixture assertions are removed.
- `fork_ext/bundled.rs`: `test_fresh_mods_folder_installs_all_packages`, `test_from_env_setup_discovers_all_bundled_packages_on_fresh_mods_folder`, `test_second_mods_folder_gets_all_packages_when_first_folder_has_records`, and `test_flat_registry_listing_all_three_packages_installs_all_three_on_fresh_mods_folder` extend expected package lists; no assertion loosened.
- `fork_ext/map.rs`: `a_gms_travel_click_is_named_once` deleted; its assertions removed.
- `fork_ext/map_move_tests.rs`: `moved_minimap_moves_circle_border_hover_and_search` deleted; its movement, hit, and visual assertions removed. New mod movement test covers some, not necessarily all, of these behaviors.
- `fork_ext/map_sync_tests.rs`: `an_indoor_picture_is_clipped_by_the_round_minimap_at_every_render_scale` deleted; indoor clipping assertions removed.
- `fork_ext/report_state.rs`: `report_state_includes_claimed_hud_elements_and_movement_flags` changes the expected label; no assertion removed.
- `map/chrome.rs`: `a_claimed_shifted_alternate_is_not_advertised_for_zoom`, `keyboard_hints_follow_the_controls_bindings`, `center_legend_follows_the_space_row_after_attack_moves_to_g`, and `rebound_map_key_dispatches_the_advertised_action_but_typing_stays_text` deleted, removing their assertions. `a_narrow_window_breaks_the_legend_into_lines` removes its rebound-label fit assertion and changes the raster fixture; `the_chrome_says_where_it_goes` and `the_notice_goes_under_the_title` only reformat closures, with assertions retained.
- `map/community.rs`: `a_bad_answer_keeps_the_list` retains assertions but replaces an isolated, cleaned-up cache directory with a PID-only temp path; this weakens fixture isolation.
- `map/mini.rs`: `minimap_stays_in_window_coordinates_at_render_scales`, `minimap_card_calls_other_players_players_and_pets_pets`, `map_refinement_hover_chooses_the_nearest_visible_entity_and_keeps_level`, `border_outer_points_match_the_gpu_circle_at_every_aspect`, and `diagonal_guidance_uses_a_circle_rim_and_corners_are_outside` deleted; their assertions removed.
- `map/tests.rs`: `world_map_stays_in_window_coordinates_at_render_scales` moved to `fork_ext/map_render_tests.rs`, retaining the four pixel checks with adapted fixture; `master_off_releases_world_and_closes_search_without_losing_saved_waypoints` deleted and its assertions removed.
- `map/theme.rs`: `bundled_font_is_readable_and_has_no_missing_ascii_glyphs` deleted; assertions removed.
- `play.rs`: `minimap_entity_tooltips_keep_server_names_levels_and_validity` deleted; assertions removed.
- `render/src/ui.rs`: `high_resolution_font_keeps_detail_at_the_original_label_size` deleted; assertions removed.
- `render/tests/map_circle.rs`: `map_and_ui_circle_masks_preserve_regular_views_and_outside_controls` deleted; assertions removed.
- `tools/test_player_release.py` adds five expected bundled files; no assertions removed or loosened. The `map/tool.rs` diff removes fixture plumbing, not a test assertion.

## Map seam diff

`git diff --stat 11aa5f0 --` the requested path set: exactly `map/mini.rs` +1/−0, `map/mod.rs` +4/−0, `render/src/ui.rs` +1/−1 (6 insertions, 1 deletion). Inspected hunks: the minimap overlay, native-only private accessor declaration and world-map overlay, and public `Font::find`; no other listed paths differ.

## Benchmark

The source asserts requested packages loaded/enabled, 300 samples per configuration, and at least 200 entities when the round minimap is on. However `Rm::with` returns early when GPU setup fails, so the test can pass without executing any workload. Require actual timing lines as evidence. Builder table: none 1.20/0.78/3.68/7.25 ms, seven mods 5.98/5.23/11.96/29.64 ms (mean/p50/p95/max). Develop native comparison omitted.

## Other gates

- `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1`, with fresh empty `EQOA_CONFIG` and shared target: 432 passed, 0 failed, 25 ignored, 1645 filtered; 304.73 s. Passing output captured, so explicit SKIP count was not available. Ignored Forge reference tests require `EQOA_FORGE_REFERENCE`.
- `python tools/check_fork_footprint.py --base 11aa5f088bb52d5f306b0224e37b782b1bf4503d`: PASS.
- `python -X utf8 tools/test_player_release.py`: 20 tests, OK.

Independent negative controls (each `cargo test --locked -p eqoa --bin eqoa <name> -- --test-threads=1 --nocapture`):

- Replaced the `hud.hide` lease with a truthy token: exit 101, failed at `the mod hides the stock minimap` (line 181).
- Kept the image at its initial rectangle after the HUD moves: exit 101, failed at `picture at the new place` (line 251).
- Disabled dot/marker hit selection: exit 101, failed at `a card on the dot` (line 291).

After each, `git checkout -- .` restored the tracked files. After the third, `git diff --quiet` confirmed a clean tracked tree; the only untracked file was this report. The earlier unmodified addon run was GREEN for each. `world_map_stays_in_window_coordinates_at_render_scales` ran separately with `--nocapture`: 1 passed, 0 failed, no SKIP message.

Both independent release runs passed and printed all eight timing lines, each reporting 300 frames; therefore they passed the workload assertions rather than taking the GPU early return. Values below are mean / p50 / p95 / max in milliseconds, with the builder's values beside each run. The spread, especially run 1's max, shows substantial noise.

| Configuration | Report | Check run 1 | Check run 2 |
|---|---|---|---|
| none | 1.20 / 0.78 / 3.68 / 7.25 | 1.653 / 0.773 / 3.704 / 128.056 | 1.037 / 0.678 / 3.576 / 16.831 |
| round 3 four | 2.83 / 2.40 / 5.99 / 9.31 | 2.382 / 2.096 / 4.853 / 6.841 | 2.370 / 2.138 / 4.708 / 9.163 |
| creature journal | 1.80 / 1.27 / 5.36 / 8.49 | 1.493 / 1.196 / 3.571 / 7.060 | 1.979 / 1.486 / 5.184 / 13.923 |
| journal + search | 3.61 / 3.18 / 6.77 / 18.97 | 2.863 / 2.596 / 5.721 / 14.252 | 4.873 / 4.494 / 9.166 / 15.282 |
| minimap, pointer away | 2.84 / 2.23 / 6.53 / 12.22 | 2.603 / 1.744 / 5.477 / 150.486 | 3.789 / 3.326 / 7.242 / 19.665 |
| minimap, pointer on circle | 2.91 / 2.13 / 6.30 / 25.63 | 2.966 / 2.263 / 5.637 / 24.670 | 4.549 / 3.355 / 8.594 / 54.078 |
| round 4 three | 5.30 / 4.17 / 14.82 / 49.33 | 4.322 / 3.893 / 6.984 / 22.472 | 5.256 / 4.618 / 9.704 / 30.629 |
| round 3 four + round 4 three | 5.98 / 5.23 / 11.96 / 29.64 | 5.338 / 4.878 / 7.607 / 23.418 | 5.753 / 5.108 / 8.451 / 27.941 |

Command for each: `cargo test --release --locked -p eqoa --bin eqoa round4c_frame_time_with_all_map_mods_in_a_busy_world -- --test-threads=1 --nocapture`, using the specified shared target and lock. Run 1 and run 2 each returned 0; the test took 16.58 s and 20.46 s respectively after compilation. The report's missing develop-native comparison remains missing.
