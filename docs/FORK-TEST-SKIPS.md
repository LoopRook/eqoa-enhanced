# Workspace fixture skips — 2026-10-28

This is the original hygiene run. The newer 0.1.0 totals, two added fixture tests
and separate disc-enabled controls are in RELEASE-0.1.0-VERIFICATION.md.

The final Windows run of `cargo test --locked --workspace -- --nocapture
--test-threads=1` completed with exit 0. Evidence is the local
`hygiene-workspace.log` retained in this task's staging directory. No second
workspace run was performed for this inventory.

Cargo reports **1,224 passed, 0 failed, and 2 ignored**. Source and serial-log review
identify **168 `ok` results with fixture-dependent checks omitted**: 167 emit a
skip message and one returns silently. This includes two S2 tests whose conditional
fixture branches never run and one disc-picker test that exercises rejection
controls before skipping its real-disc acceptance check. These 168 results are
not evidence of successful disc, oracle, capture, or decomp comparisons. The other
1,056 `ok` results have no identified fixture omission in this review.

The run used a scratch `EQOA_CONFIG`, no `EQOA_ISO`, no `run/eqoa.iso`, no decrypted
`run/client.bin`/`run/support.bin`, and no local oracle/decomp/reference/capture or
server-patch fixtures. The 2002 disc and fixture override variables were absent.
Some tests hard-code their checkout-relative disc path; supplying `EQOA_ISO`
alone does not enable every test in this list.

| Crate | Cargo `ok` | With fixture checks omitted | Cargo ignored |
| --- | ---: | ---: | ---: |
| `dnas` | 6 | 6 | 0 |
| `drdp` | 5 | 3 | 0 |
| `ee` | 23 | 0 | 0 |
| `eqoa` | 975 | 77 | 2 |
| `esf` | 21 | 11 | 0 |
| `iso` | 2 | 2 | 0 |
| `net` | 36 | 2 | 0 |
| `render` | 57 | 1 | 0 |
| `snd` | 17 | 5 | 0 |
| `vi` | 80 | 61 | 0 |
| `webicons` | 2 | 0 | 0 |

The remaining renderer/GPU checks did run: the final log contains no missing-GPU
skip. `render` has one disc-dependent skeleton inventory omission, listed below.
The EE libm message about operations "unported in both" describes individual
comparison cases inside an exercised test, not additional skipped tests.

## Explicitly ignored benchmarks

Cargo reports these two separately as `ignored`; they are not part of the 168
fixture omissions. Both are in `eqoa`'s `src/main.rs` test binary, declared with
`#[ignore]` in `crates/eqoa/src/fx.rs`:

- `fx::bench::fans_cover` — optional world/effect coverage benchmark; needs local disc data (and GPU for its rendering stage).
- `fx::bench::volumes_cost` — optional effect-volume timing benchmark; needs local disc data.

## Fixture checks omitted

All test names below were present in the final run and reported `ok`. Names are
grouped by crate and Cargo test binary. A missing first prerequisite can prevent
later prerequisite checks from being reached. In particular, some oracle tests
also need a disc, and ESF reference tests need both a disc and reference files.

### `dnas`

**Test binary: `tests/discs.rs`**

- `every_other_disc_on_hand` — Extracted reference discs absent (`EQOA_BUILDS`/sibling `decomp-builds/extracted`).
- `one_flipped_bit_is_refused` — Neither the Frontiers disc nor extracted reference disc data is available.
- `the_frontiers_disc_through_its_iso` — Frontiers disc absent.

**Test binary: `tests/natives.rs`**

- `a_wrong_native_is_caught` — Frontiers disc absent.
- `every_call_of_an_unpack` — Frontiers disc absent.
- `pure_natives_on_random_arguments` — Frontiers disc absent.

### `drdp`

**Test binary: `tests/oracle_drdp.rs`**

- `a_planted_bug_is_caught` — Oracle executable absent: `eqoa-port/build/oracle_drdp`.
- `a_real_sandstorm_session` — PS2/Sandstorm capture absent (`pcaps/ps2_sandstorm.pcap`).
- `two_instances_through_a_bad_network` — Oracle executable absent: `eqoa-port/build/oracle_drdp`.

### `eqoa`

**Test binary: `src/main.rs`**

- `bgm::tests::the_page_streams_what_the_disc_does` — Frontiers disc absent.
- `client_image::tests::a_patch_gives_the_group_menu_its_entries` — Decrypted CLIENT image absent (`run/client.bin`); patch tests also need local S2 data.
- `discs::tests::the_disc_judged_off_the_file` — **Partial exercise:** non-disc/missing-file rejection controls ran; real-disc acceptance check omitted.
- `fishing::tests::find_water_on_freeports_docks` — Frontiers disc absent.
- `fishing::tests::the_splash_texture_is_spellfxs` — Frontiers disc absent.
- `fx::tests::neriak_fog_draws_round_the_plaza` — Frontiers disc absent.
- `fx::tests::night_fog_shows_at_night` — Frontiers disc absent.
- `fx::tests::the_flock_flies_as_boids` — Frontiers disc absent.
- `fxview::tests::a_visual_plays_and_plays_again` — Frontiers disc absent.
- `fxview::tests::both_casts_at_a_human_and_at_itself` — Frontiers disc absent.
- `fxview::tests::the_visual_list_is_every_visual_the_stage_plays` — Frontiers disc absent.
- `loading::tests::the_gears_off_the_disc` — Frontiers disc absent.
- `map::build::tests::a_zone_is_made_a_piece_a_frame` — Frontiers disc absent.
- `mouse::tests::the_left_button_steers_while_the_player_moves` — Frontiers disc absent.
- `particles::tests::a_creatures_own_emitters_run_at_their_nodes` — Frontiers disc absent.
- `particles::tests::a_held_items_particles_follow_the_hand` — Frontiers disc absent.
- `particles::tests::the_creatures_own_emitters_off_the_disc` — Frontiers disc absent.
- `particles::tests::the_items_particle_sprites_off_the_disc` — Frontiers disc absent.
- `play::commands::a_far_teleport_lands_on_the_floor_once_its_zones_are_in` — Frontiers disc absent.
- `play::commands::a_move_is_answered_with_its_first_position_from_the_arrival` — Frontiers disc absent.
- `play::commands::the_live_patchs_search_window` — Local S2 patch fixture(s) absent: `live`.
- `play::entity_actors::an_entity_has_an_actor_while_it_is_drawn` — Frontiers disc absent.
- `pregame::client::races::tests::the_tables_are_the_decomps` — Decomp sources `virace.cpp` and `viclass.cpp` absent (`EQOA_DECOMP`/sibling `decomp`).
- `pregame::figure::tests::a_figure_fidgets_from_the_start` — Frontiers disc absent.
- `pregame::figure::tests::a_figure_is_its_model_in_the_art_drawn` — Frontiers and 2002 disc fixtures absent.
- `pregame::host::tests::a_bgm_fades_out_in_the_games_time` — Frontiers disc absent.
- `pregame::host::tests::a_click_in_station_is_heard_whole` — Frontiers disc absent.
- `pregame::stage::room_tests::the_arena_is_twelve_closed_sets` — Frontiers disc absent.
- `pregame::stage::room_tests::the_camera_in_a_set_draws_that_set_alone` — Frontiers disc absent.
- `pregame::station::page::tests::every_page_on_the_disc_parses` — Frontiers disc absent.
- `pregame::station::pak::tests::the_discs_pak` — Frontiers disc absent.
- `pregame::station::renderer::tests::a_login_goes_out_as_the_game_sends_it_and_its_answer_moves_on` — Frontiers disc absent.
- `pregame::station::renderer::tests::a_new_account_from_the_pages` — Frontiers disc absent.
- `pregame::station::renderer::tests::a_password_change_from_the_pages` — Frontiers disc absent.
- `pregame::station::renderer::tests::play_shows_the_news_then_plays` — Frontiers disc absent.
- `pregame::station::renderer::tests::save_account_keeps_the_login_for_the_next_run` — Frontiers disc absent.
- `pregame::station::renderer::tests::triangle_on_the_first_page_disconnects` — Frontiers disc absent.
- `pregame::station::tests::a_pointer_on_the_pages` — Frontiers disc absent.
- `pregame::wide::tests::the_cuts_are_the_pictures` — Frontiers disc absent.
- `rings::tests::the_rings_sprite_off_the_disc` — Frontiers disc absent.
- `s2::tests::a_world_out_of_the_servers_folder` — Frontiers disc and local server art `ss-art.csf` absent.
- `s2::tests::set_9s_art_out_of_char_esf` — Local S2 patch fixture(s) absent: `live`.
- `s2::tests::the_hello_and_the_record_of_the_pad` — Local S2 patch fixture(s) absent: `live`, `july`, `ours`. **All conditional fixture branches omitted; not an early return.**
- `s2::tests::the_live_s2s_features` — Local S2 patch fixture(s) absent: `live`.
- `s2::tests::the_other_builds` — Local S2 patch fixture(s) absent: `july`, `ours`. **All conditional fixture branches omitted; not an early return.**
- `s2::tests::the_words_out_of_the_servers_folder` — Local server patch `WORDS.TXT` absent (`EQOA_S2_BUILDS`).
- `saveicon::tests::supports_own_icon_is_a_face_not_noise` — SUPPORT image absent (`EQOA_SUPPORT`/`run/support.bin`).
- `searchwnd::tests::the_windows_words_are_read_out_of_the_patch` — Local S2 patch fixture(s) absent: `live`.
- `shadows::tests::behind_a_wall_casts_no_shadow` — Frontiers disc absent.
- `sound::tests::a_creatures_loop_stops_when_it_goes` — Frontiers disc absent.
- `sound::tests::the_ambience_is_the_effects_class` — Frontiers disc absent.
- `sound::tests::the_options_volumes_go_on_their_classes` — Frontiers disc absent.
- `sound::tests::the_titles_music_plays` — Frontiers disc absent.
- `spelling::tests::the_discs_words_lose_every_klick` — **Silent early return:** disc open/read fails; no skip message is emitted.
- `spells::tests::a_bows_arrow_flies_to_its_target` — Frontiers disc absent.
- `startmenu::tests::the_start_tables_read_out_of_client` — Decrypted CLIENT image absent (`run/client.bin`).
- `startmenu::tests::the_test_entry_while_the_gm_channel_is_on` — Decrypted CLIENT image absent (`run/client.bin`).
- `statbox::tests::the_widest_row_fits_its_column` — Frontiers disc absent.
- `stream::tests::a_2002_disc_gone_plays_frontiers` — Frontiers disc absent; this control requires Frontiers present and no explicit 2002-disc override.
- `stream::tests::a_world_swapped_under_the_walk_keeps_it_standing` — Frontiers and 2002 disc fixtures absent.
- `stream::tests::the_2002_tunaria_keeps_frontiers_volumes_and_streams` — Frontiers and 2002 disc fixtures absent.
- `viewer::tests::a_listed_model_follows_the_art` — Frontiers and 2002 disc fixtures absent.
- `walker::tests::a_character_loaded_again_keeps_one_set_of_clips` — Frontiers disc absent.
- `walker::tests::a_landing_is_told_once` — Frontiers disc absent.
- `walker::tests::a_login_stands_where_it_logged_out_not_on_what_is_over_it` — Frontiers disc absent.
- `walker::tests::a_move_lands_on_the_floor_under_it_not_a_roof_over_it` — Frontiers disc absent.
- `walker::tests::a_place_given_between_frames_stands` — Frontiers disc absent.
- `walker::tests::a_stall_is_one_long_frame` — Frontiers disc absent.
- `walker::tests::drawn_at_any_rate_the_walk_moves_every_frame` — Frontiers disc absent.
- `walker::tests::the_cameras_no_turn_is_kept_with_the_frame` — Frontiers disc absent.
- `walker::tests::the_player_stops_at_a_creature_in_its_way` — Frontiers disc absent.
- `walker::tests::the_view_ends_at_the_games_far_plane` — Frontiers disc absent.
- `walker::tests::the_walk_keeps_the_consoles_frames_at_any_display_rate` — Frontiers disc absent.
- `walker::tests::zoomed_out_the_players_body_moves_as_at_the_games_camera` — Frontiers disc absent.
- `weather::tests::a_ports_sky_is_set_at_once_and_the_weather_after_it_is_not_held` — Frontiers disc absent.
- `weather::tests::the_orders_after_a_ports_sky_wait_for_its_world` — Frontiers disc absent.
- `wholist::tests::the_patchs_sort_is_read_out_of_the_live_s2` — Local S2 patch fixture(s) absent: `live`.

### `esf`

**Test binary: `tests/deferred.rs`**

- `a_zone_on_its_own_is_the_whole_parse_subtree` — Frontiers disc absent.
- `tunaria_opens_with_its_zones_on_the_disc` — Frontiers disc absent.

**Test binary: `tests/era.rs`**

- `the_2002_disc_stands_in_for_char_esf` — Frontiers and 2002 disc fixtures absent.

**Test binary: `tests/flora.rs`**

- `every_flora_sprite_builds_inside_its_box` — Frontiers disc absent.

**Test binary: `tests/library.rs`**

- `first_loaded_file_wins` — Frontiers disc absent; reference comparisons also require `EQOA_EXPECT`/sibling `esfviewer` fixtures.
- `group_members_resolve` — Frontiers disc absent; reference comparisons also require `EQOA_EXPECT`/sibling `esfviewer` fixtures.

**Test binary: `tests/lod.rs`**

- `an_lod_body_builds_its_levels_apart` — Frontiers disc absent.

**Test binary: `tests/meshes.rs`**

- `meshes_match_reference_parser` — Frontiers disc absent; reference comparisons also require `EQOA_EXPECT`/sibling `esfviewer` fixtures.

**Test binary: `tests/mips.rs`**

- `the_disc_carries_every_level_the_game_keeps` — Frontiers disc absent.

**Test binary: `tests/surfaces.rs`**

- `surfaces_match_reference_parser` — Frontiers disc absent; reference comparisons also require `EQOA_EXPECT`/sibling `esfviewer` fixtures.

**Test binary: `tests/terrain_alone.rs`**

- `a_zones_terrain_builds_from_its_record_alone` — Frontiers disc absent.

### `iso`

**Test binary: `tests/sparse.rs`**

- `listing_through_the_cache_equals_the_file` — Frontiers disc absent.
- `reads_through_the_cache_equal_the_file` — Frontiers disc absent.

### `net`

**Test binary: `tests/dump_capture.rs`**

- `sandstorms_memory_dumps_read_whole` — PS2/Sandstorm capture absent (`pcaps/ps2_sandstorm.pcap`).

**Test binary: `tests/selfstate_capture.rs`**

- `the_ps2s_own_self_states_re_encode` — PS2/Sandstorm capture absent (`pcaps/ps2_sandstorm.pcap`).

### `render`

**Test binary: `tests/skin_palette.rs`**

- `every_skeleton_on_the_disc_fits_the_palette` — Frontiers disc absent.

### `snd`

**Test binary: `tests/device_upload.rs`**

- `an_ambient_tracks_stop_that_finds_the_packet_full_still_stops` — Frontiers disc absent.

**Test binary: `tests/oracle_snd.rs`**

- `a_planted_difference_fails` — Oracle executable absent: `eqoa-port/build/oracle_snd`.
- `snddrv_matches_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle_snd`.

**Test binary: `tests/oracle_snddev.rs`**

- `a_planted_difference_fails` — Oracle executable absent: `eqoa-port/build/oracle_snd`.
- `visounddevice_matches_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle_snd`.

### `vi`

**Test binary: `tests/atlas.rs`**

- `supports_atlas_reads_whole` — SUPPORT image absent; no disc available for the atlas helper fallback.

**Test binary: `tests/csprite_items.rs`**

- `forms_hold_no_items` — Frontiers disc absent.

**Test binary: `tests/emitters.rs`**

- `music_regions_equal_brute_force` — Frontiers disc absent.

**Test binary: `tests/lighting_fit.rs`**

- `colour_buffers_fit_their_sprites` — Frontiers disc absent.

**Test binary: `tests/oracle.rs`**

- `camera_effect_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `coll_sphere_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `layout_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `libm_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `walk_camera_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `walk_control_matches` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_anim.rs`**

- `character_animation_matches` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_atmos.rs`**

- `a_mutated_constant_fails` — Oracle executable absent: `eqoa-port/build/oracle_atmos`.
- `a_planted_difference_fails` — Oracle executable absent: `eqoa-port/build/oracle_atmos`.
- `the_atmosphere_matches_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle_atmos`.

**Test binary: `tests/oracle_clip.rs`**

- `calc_planes_as_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle`.
- `the_clip_stack_as_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_collide.rs`**

- `collide_obj_on_real_buffers_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `matrices_and_primitives_match` — Oracle executable absent: `eqoa-port/build/oracle`.
- `query_obj_on_real_buffers_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `triangle_edge_vertex_match` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_effect.rs`**

- `a_mutated_constant_fails` — Oracle executable absent: `eqoa-port/build/oracle_effect`.
- `a_planted_difference_fails` — Oracle executable absent: `eqoa-port/build/oracle_effect`.
- `effect_volumes_match_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle_effect`.

**Test binary: `tests/oracle_flora.rs`**

- `flora_matches_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle_flora`.

**Test binary: `tests/oracle_pad.rs`**

- `every_call_matches_over_scripted_libpad` — Oracle executable absent: `eqoa-port/build/oracle`.
- `pad_layout_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `planted_mistakes_are_seen` — Oracle executable absent: `eqoa-port/build/oracle`.
- `scale_stick_matches_over_every_byte` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_particle.rs`**

- `a_planted_difference_fails` — Oracle executable absent: `eqoa-port/build/oracle_particle`.
- `particles_match_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle_particle`.

**Test binary: `tests/oracle_pick.rs`**

- `a_planted_hit_colour_fails` — Oracle executable absent: `eqoa-port/build/oracle`.
- `pick_leaves_match` — Oracle executable absent: `eqoa-port/build/oracle`.
- `pick_pack_vunc_matches` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_ray.rs`**

- `a_planted_difference_fails` — Oracle executable absent: `eqoa-port/build/oracle`.
- `capsule_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `collide_ray_obj_on_real_buffers_matches` — Oracle executable absent: `eqoa-port/build/oracle`.
- `ray_leaves_match` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_shadow.rs`**

- `shadows_match_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle_shadow`.

**Test binary: `tests/oracle_spell.rs`**

- `a_changed_world_rule_fails` — Oracle executable absent: `eqoa-port/build/oracle_spell`.
- `a_planted_record_fails` — Oracle executable absent: `eqoa-port/build/oracle_spell`.
- `an_effect_with_a_longer_wait_fails` — Oracle executable absent: `eqoa-port/build/oracle_spell`.
- `spell_effects_match_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle_spell`.

**Test binary: `tests/oracle_swing.rs`**

- `the_swing_tables_match` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_textset.rs`**

- `the_quick_chat_tree_matches` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_tod.rs`**

- `clip_planes_and_far_plane` — Oracle executable absent: `eqoa-port/build/oracle`.
- `init_time_of_day` — Oracle executable absent: `eqoa-port/build/oracle`.
- `profiles_and_blend` — Oracle executable absent: `eqoa-port/build/oracle`.

**Test binary: `tests/oracle_trail.rs`**

- `a_planted_difference_fails` — Oracle executable absent: `eqoa-port/build/oracle_trail`.
- `trails_match_the_decomp` — Oracle executable absent: `eqoa-port/build/oracle_trail`.

**Test binary: `tests/oracle_ui.rs`**

- `the_ui_layer_records_what_the_decomp_records` — Oracle executable absent: `eqoa-port/build/oracle_ui`.

**Test binary: `tests/outdoors_probe.rs`**

- `roofed_ground_is_darker_than_open_ground` — Frontiers disc absent.

**Test binary: `tests/world_2002.rs`**

- `tunaria_off_the_2002_disc_is_the_same_ground` — Frontiers and 2002 disc fixtures absent.

**Test binary: `tests/world_actors.rs`**

- `a_creature_stops_a_walk_as_its_capsule` — Frontiers disc absent.
- `creatures_are_in_the_rooms_they_reach_as_they_move` — Frontiers disc absent.

**Test binary: `tests/world_flora.rs`**

- `flora_on_tunaria` — Frontiers disc absent.

**Test binary: `tests/world_rooms.rs`**

- `every_portal_as_its_record_has_it` — Frontiers disc absent.
- `the_walk_from_inside_closed_rooms` — Frontiers disc absent.
- `the_walk_from_outdoors` — Frontiers disc absent.

**Test binary: `tests/world_shadow.rs`**

- `shadow_query_on_tunaria` — Frontiers disc absent.

**Test binary: `tests/world_walk.rs`**

- `bsp_walk_equals_brute_force` — Frontiers disc absent.

**Test binary: `tests/world_zones.rs`**

- `zone_by_zone_equals_whole` — Frontiers disc absent.

## Source audit and scope

The review followed test-level early returns and helper-level `Option`/`Result`
propagation, not just uppercase `SKIP:` lines. The silent case is
`spelling::tests::the_discs_words_lose_every_klick` in `crates/eqoa/src/spelling.rs`:
failed disc open/read returns immediately. The save-icon test logs a sentence
ending in "test skipped", and the race/class table check logs lowercase
"skipped:"; both are included. Oracle negative controls that accept `None` after
missing fixture setup are also included even when their bodies do not use
`return`.

The VI list was reconciled by test name and binary with the earlier
`animation-staging/vi-skip-report.md`: all 61 fixture omissions match. In this
workspace run VI reports 80 `ok`, of which 19 have no fixture omission and 61 do.
The animation guard regression tests are among the exercised checks.

Platform-disabled code is not a runtime fixture skip. For example,
`discs::tests::game_mode_finds_the_disc_where_a_deck_keeps_it` is `#[cfg(unix)]`
and was not compiled into this Windows run. Browser/WASM builds, launcher helper
checks, Python/Node tests, and live game/parser acceptance are separate evidence
and are not counted in these Cargo totals.

After the workspace run, the only source follow-up was a test-only options lookup
correction. Its focused regression passed; this report records the completed
workspace invocation rather than claiming another whole-suite run after that edit.
