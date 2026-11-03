# 2026-11-03 upstream sync verification

The original merge evidence is retained below. The review-follow-up section at
the end records the current source and refreshed validation.

Source: sync/2026-11-03, Enhanced e3964ff + upstream b8e02cd781aac6a1b9e9906f09896956c77afcd6.
Commands ran in this worktree with CARGO_TARGET_DIR=target and scratch
EQOA_CONFIG=target/sync-test-config. Every cargo invocation acquired the shared
C:/Users/hocka/Paseo/cargo-build.lock in Git Bash and released it on exit.

## Final workspace result

`cargo test --locked --workspace -- --nocapture --test-threads=1`: **PASS**.
96 test/doc-test summaries: **1,775 passed, 0 failed, 27 ignored**. The client
summary is **1,502 passed, 0 failed, 26 ignored**. There are **197 explicit SKIP
messages**, listed below; these successful early returns are not claims that the
missing disc/oracle/reference behavior was exercised. The 27 ignored tests are
listed separately. Some available local addon/disc icon and headless GPU checks
did execute; this is not a blanket disc or GPU pass.

`python -X utf8 -m unittest discover -s tools -p "test_*.py"`: **46 passed**.

`python tools/check_fork_footprint.py --base b8e02cd`: **PASS**; 33 shared paths
within reviewed budgets. Trial-manifest PASS preceded advancing reviewed_base;
the checked-in manifest and verbatim upstream CLAUDE copy pass at the full SHA.
See UPSTREAM-SYNC.md and FORK-OWNERSHIP.md for each budget reason.

## Hook-removal evidence

Each hook was temporarily removed, its focused test compiled and ran, and the
source restored in a finally block. Every control returned 101 due to a test
failure (not a compiler failure); the complete suite then passed with hooks back.

| Test | Hook removed | Observed failure |
| --- | --- | --- |
| sync_f3_saves_redacted_zip_through_dialog_hook | viewer fork_report::event | assertion failed: viewer.contains("fork_report::event(self, el, &event)") |
| sync_crash_marker_is_offered_on_next_start_hook | logfile report::start | assertion failed: source contains report::start() |
| sync_logger_close_flushes_journal_through_report_hook | logfile report::clean_exit | logger close must flush journal: NotFound |

The F3 and crash tests combine source wiring guards with behavior checks. The
F3 behavior follows open -> screenshot/note -> Enter -> local save worker and
inspects the ZIP in a temporary Desktop for note, PNG, log, manifest and credential/
home redaction. Crash behavior checks interrupted vs clean logs; journal behavior
calls the real logfile close and reloads its temporary file. These do not replace
live OS-window keyboard/capture or next-start owner checks.

The first workspace run exposed three merge regressions (option ID sort, the log
header test's old startup path, and synthetic HUD counts after upstream culling).
The second run exposed the PID-reused community-map test cache. All were fixed;
production raster/community behavior was preserved. Temporarily disabling only
upstream culling restored both old HUD quad expectations, confirming that cause.

## Release and preview

`cargo build --release --locked -p eqoa`: **PASS** (optimized, 4m 26s).
Executable: 32,649,216 bytes; SHA256 `c964c5d61fdd1c7a1398bf019ec171a0a2459e7203498a7ec6fc6ea9beecae6d`.
PE inspection: x86_64, GUI subsystem 2, 8,388,608-byte reserved main-thread stack.

The preview copy is generated after the merge commit under the main checkout's
`run/sync-2026-11-03-<merge-shortsha>/`. It contains the identical EXE, bundled
mods, tracked map catalog, license/readme, launcher and TEST-CHECKLIST.txt. The
launcher uses its own profile directory. No game data or preview files are
committed. The copy's EXE hash must match the build above.

Owner QA remains pending: live F3 input/screen capture and crash offer, Touch Pad
Size, server Class Mastery, map/journal, Wereoxxs. Android/browser runtime builds
were not exercised. This sync includes no other worktree's in-flight fixes.

## Ignored tests

```text
test fork_ext::addons::manager::tests::forge_recovery_modern_legacy_level59 ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::manager::tests::forge_recovery_modern_legacy_level60 ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::manager::tests::forge_recovery_modern_profiles_level59 ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::manager::tests::forge_recovery_modern_profiles_level60 ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_addon_focused_typing_does_not_leak_keys_to_game ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_addon_runtime_error_releases_all_claims_and_unhides_game_hud ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_claimed_enter_focuses_addon_input_and_say_window_stays_closed ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_claimed_unsuppressed_key_delivers_to_addon_and_reaches_game ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_dialog_receives_enter_even_when_addon_suppresses_enter ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_escape_and_history_keys_with_chat_claimed_do_not_draw_or_open_game_chat ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_modern_arpg_unchanged_package ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_modern_avoids_unchanged_package ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_modern_quest_log_and_util_surface ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_modern_scripts_path_and_file_io_surface ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_unclaimed_enter_opens_say_window_as_before ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_wereoxxs_all_windows_rendered_without_drawlist_nil_errors ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_wereoxxs_complete_symbol_surface_contract ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_wereoxxs_disabled_startup_profile_enable_toggle_disable_reenable_all_elements ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_wereoxxs_discovery_and_manifest ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_wereoxxs_headless_gpu_manager_pipeline ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_wereoxxs_in_game_full_execution_and_lifecycle ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_wereoxxs_pauses_and_hides_in_all_menu_modes ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::addons::wereoxxs_test::forge_wereoxxs_worst_case_scrollback_10000_messages_and_budget_measurements ... ignored, needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)
test fork_ext::bundled::update_tests::bundled_update_legacy_registry_updates_reference_wereoxxs ... ignored, needs EQOA_FORGE_REFERENCE (pinned original Wereoxxs package)
test fx::bench::fans_cover ... ignored
test fx::bench::volumes_cost ... ignored
test ground_fog_cost ... ignored
```

## Explicit prerequisite SKIP messages (197)

```text
test every_other_disc_on_hand ... SKIP: no discs at <worktree>\crates\dnas\../..\../decomp-builds ($EQOA_BUILDS)
test one_flipped_bit_is_refused ... SKIP: no disc on hand
test the_frontiers_disc_through_its_iso ... SKIP: no disc image at run/eqoa.iso ($EQOA_ISO)
test a_wrong_native_is_caught ... SKIP: no disc image at <worktree>\crates\dnas\../..\run/eqoa.iso
test every_call_of_an_unpack ... SKIP: no disc image at <worktree>\crates\dnas\../..\run/eqoa.iso
test pure_natives_on_random_arguments ... SKIP: no disc image at <worktree>\crates\dnas\../..\run/eqoa.iso
test a_planted_bug_is_caught ... SKIP: no oracle at <worktree>\crates\drdp\../../../eqoa-port/build/oracle_drdp (make oracle in eqoa-port)
test a_real_sandstorm_session ... SKIP: no capture at <worktree>\crates\drdp\../../../pcaps/ps2_sandstorm.pcap
test two_instances_through_a_bad_network ... SKIP: no oracle at <worktree>\crates\drdp\../../../eqoa-port/build/oracle_drdp (make oracle in eqoa-port)
test bgm::tests::the_native_reader_keeps_the_disc_off_the_drivers_lock ... SKIP: no run/eqoa.iso
test bgm::tests::the_page_streams_what_the_disc_does ... SKIP: no run/eqoa.iso
test client_image::tests::a_patch_gives_the_group_menu_its_entries ... SKIP: no run/client.bin
test discs::tests::the_disc_judged_off_the_file ... SKIP: no run/eqoa.iso
test fishing::tests::find_water_on_freeports_docks ... SKIP: no run/eqoa.iso
test fishing::tests::the_splash_texture_is_spellfxs ... SKIP: no run/eqoa.iso
test fork_ext::addons::discovery::tests::addon_discovery_cross_scriptroot_symlink_stays_excluded ... SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::discovery::tests::addon_discovery_dangling_and_escaping_support_dirs_emit_diagnostics ... SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::discovery::tests::addon_discovery_dangling_child_scripts_directory_emits_diagnostic ... SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fork_ext::addons::gpu_test::forge_gpu_pinned_grid_and_texture_fixture ... SKIP forge_gpu: set EQOA_FORGE_SPIKE to the pinned local screen_grid.lua
test fork_ext::addons::manager::tests::forge_manager_unchanged_pinned_hearts_real_png_lifecycle_and_hp_pixels ... SKIP pinned hearts: EQOA_FORGE_SPIKE prerequisite missing
test fork_ext::addons::package::tests::forge_package_resource_precedence_and_containment ... SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
test fx::tests::neriak_fog_draws_round_the_plaza ... SKIP: no run/eqoa.iso
test fx::tests::night_fog_shows_at_night ... SKIP: no run/eqoa.iso
test fx::tests::only_the_ground_fog_goes_as_a_ps2_frame_keeps_it ... SKIP: no run/eqoa.iso
test fx::tests::the_flock_flies_as_boids ... SKIP: no run/eqoa.iso
test fxview::tests::a_visual_plays_and_plays_again ... SKIP: no run/eqoa.iso
test fxview::tests::both_casts_at_a_human_and_at_itself ... SKIP: no run/eqoa.iso
test fxview::tests::the_visual_list_is_every_visual_the_stage_plays ... SKIP: no run/eqoa.iso
test live::tests::the_words_are_the_live_clients ... SKIP: no live save state at <worktree>\crates\eqoa\../../..\1.6-savestates/EEEE1FCC.001.p2s
test loading::tests::the_gears_off_the_disc ... SKIP: no run/eqoa.iso
test map::build::tests::a_zone_is_made_a_piece_a_frame ... SKIP: no disc at <worktree>\crates\eqoa\../../run/eqoa.iso
test mouse::tests::the_left_button_steers_while_the_player_moves ... SKIP: no run/eqoa.iso
test particles::tests::a_creatures_own_emitters_run_at_their_nodes ... SKIP: no run/eqoa.iso
test particles::tests::a_held_items_particles_follow_the_hand ... SKIP: no run/eqoa.iso
test particles::tests::the_creatures_own_emitters_off_the_disc ... SKIP: no run/eqoa.iso
test particles::tests::the_items_particle_sprites_off_the_disc ... SKIP: no run/eqoa.iso
test play::adminplay::tests::a_goto_lands_on_the_ground_once_the_zones_are_in ... SKIP: no run/eqoa.iso
test play::adminplay::tests::the_trigger_labels_meet_the_test_menus_entries ... SKIP: no run/client.bin
test play::commands::a_coach_taken_with_the_inventory_up_leaves_the_camera_behind_the_player ... SKIP: no run/client.bin
test play::commands::a_far_teleport_lands_on_the_floor_once_its_zones_are_in ... SKIP: no run/eqoa.iso
test play::commands::a_move_is_answered_with_its_first_position_from_the_arrival ... SKIP: no run/eqoa.iso
test play::commands::the_boons_move_across_freeport_is_not_held ... SKIP: no run/eqoa.iso
test play::commands::the_live_patchs_search_window ... SKIP: no live S2 at <worktree>\crates\eqoa\../../..\extracted-assets/sandstorm-patch-20261015/S2
test play::duelplay::tests::the_duel_opponent_may_be_fought ... SKIP: no run/eqoa.iso
test play::duelplay::tests::the_feature_word_puts_duel_in_community ... SKIP: no run/client.bin
test play::entity_actors::a_creature_behind_the_camera_trips_no_sound ... SKIP: no run/eqoa.iso
test play::entity_actors::a_creature_not_heard_is_hushed ... SKIP: no run/eqoa.iso
test play::entity_actors::an_actor_is_of_the_body_drawn ... SKIP: no run/eqoa.iso
test play::entity_actors::an_entity_has_an_actor_while_it_is_drawn ... SKIP: no run/eqoa.iso
test play::entity_actors::the_player_stops_at_a_blocker_that_draws_nothing ... SKIP: no run/eqoa.iso
test play::entity_actors::the_players_body_is_drawn_at_its_entitys_scale ... SKIP: no run/eqoa.iso
test play::liveplay::tests::the_group_menu_gains_the_live_entries ... SKIP: no run/client.bin
test play::npcw::tests::an_accepted_trade_puts_the_start_menu_away ... SKIP: no run/client.bin
test pregame::figure::tests::a_figure_fidgets_from_the_start ... SKIP: no run/eqoa.iso
test pregame::figure::tests::a_figure_is_its_model_in_the_art_drawn ... SKIP: no Frontiers or no 2002 disc
test pregame::figure::tests::a_figures_epic_weapon_glows_at_character_select ... SKIP: no run/eqoa.iso
test pregame::host::tests::a_bgm_fades_out_in_the_games_time ... SKIP: no run/eqoa.iso
test pregame::host::tests::a_click_in_station_is_heard_whole ... SKIP: no run/eqoa.iso
test pregame::stage::particle_tests::the_arenas_braziers_burn_in_the_room_drawn ... SKIP: no run/eqoa.iso
test pregame::stage::room_tests::the_arena_is_twelve_closed_sets ... SKIP: no run/eqoa.iso
test pregame::stage::room_tests::the_camera_in_a_set_draws_that_set_alone ... SKIP: no run/eqoa.iso
test pregame::station::page::tests::every_page_on_the_disc_parses ... SKIP: no run/eqoa.iso
test pregame::station::pak::tests::the_discs_pak ... SKIP: no run/eqoa.iso
test pregame::station::renderer::tests::a_login_goes_out_as_the_game_sends_it_and_its_answer_moves_on ... SKIP: no run/eqoa.iso
test pregame::station::renderer::tests::a_new_account_from_the_pages ... SKIP: no run/eqoa.iso
test pregame::station::renderer::tests::a_password_change_from_the_pages ... SKIP: no run/eqoa.iso
test pregame::station::renderer::tests::play_shows_the_news_then_plays ... SKIP: no run/eqoa.iso
test pregame::station::renderer::tests::save_account_keeps_the_login_for_the_next_run ... SKIP: no run/eqoa.iso
test pregame::station::renderer::tests::triangle_on_the_first_page_disconnects ... SKIP: no run/eqoa.iso
test pregame::station::tests::a_pointer_on_the_pages ... SKIP: no run/eqoa.iso
test pregame::wide::tests::the_cuts_are_the_pictures ... SKIP: no disc at <worktree>\crates\eqoa\../../run/eqoa.iso
test rings::tests::the_rings_sprite_off_the_disc ... SKIP: no run/eqoa.iso
test s2::tests::a_world_out_of_the_servers_folder ... SKIP: no disc or no ss-art.csf
test s2::tests::set_9s_art_out_of_char_esf ... SKIP: no live S2 at <worktree>\crates\eqoa\../../..\extracted-assets/sandstorm-patch-20261015/S2
test s2::tests::the_hello_and_the_record_of_the_pad ... SKIP: no live S2 at <worktree>\crates\eqoa\../../..\extracted-assets/sandstorm-patch-20261015/S2
SKIP: no july S2 at <worktree>\crates\eqoa\../../..\go-eqoa-server/patches/S2-stock
SKIP: no ours S2 at <worktree>\crates\eqoa\../../..\go-eqoa-server/patches/serve/S2
test s2::tests::the_live_s2s_features ... SKIP: no live S2 at <worktree>\crates\eqoa\../../..\extracted-assets/sandstorm-patch-20261015/S2
test s2::tests::the_other_builds ... SKIP: no july S2 at <worktree>\crates\eqoa\../../..\go-eqoa-server/patches/S2-stock
SKIP: no ours S2 at <worktree>\crates\eqoa\../../..\go-eqoa-server/patches/serve/S2
test s2::tests::the_words_out_of_the_servers_folder ... SKIP: no WORDS.TXT at <worktree>\crates\eqoa\../../..\extracted-assets/sandstorm-patch-20261015
test searchwnd::tests::the_windows_words_are_read_out_of_the_patch ... SKIP: no live S2 at <worktree>\crates\eqoa\../../..\extracted-assets/sandstorm-patch-20261015/S2
test shadows::tests::behind_a_wall_casts_no_shadow ... SKIP: no run/eqoa.iso
test sound::tests::a_creatures_loop_stops_when_it_goes ... SKIP: no run/eqoa.iso
test sound::tests::the_ambience_is_the_effects_class ... SKIP: no run/eqoa.iso
test sound::tests::the_options_volumes_go_on_their_classes ... SKIP: no run/eqoa.iso
test sound::tests::the_titles_music_plays ... SKIP: no run/eqoa.iso
test spells::tests::a_bows_arrow_flies_to_its_target ... SKIP: no run/eqoa.iso
test spells::tests::an_effects_animated_sprites_are_posed_and_drawn ... SKIP: no run/eqoa.iso
test startmenu::tests::the_start_tables_read_out_of_client ... SKIP: no run/client.bin
test startmenu::tests::the_test_entry_while_the_gm_channel_is_on ... SKIP: no run/client.bin
test statbox::tests::the_widest_row_fits_its_column ... SKIP: no run/eqoa.iso
test stream::tests::a_2002_disc_gone_plays_frontiers ... SKIP: no Frontiers disc, or EQOA_2002_ISO names a disc
test stream::tests::a_world_swapped_under_the_walk_keeps_it_standing ... SKIP: no Frontiers disc or no 2002 disc (set EQOA_2002_ISO)
test stream::tests::the_2002_tunaria_keeps_frontiers_volumes_and_streams ... SKIP: no Frontiers disc or no 2002 disc (set EQOA_2002_ISO)
test viewer::tests::a_listed_model_follows_the_art ... SKIP: no Frontiers disc or no 2002 disc (set EQOA_2002_ISO)
test walker::tests::a_character_loaded_again_keeps_one_set_of_clips ... SKIP: no run/eqoa.iso
test walker::tests::a_landing_is_told_once ... SKIP: no run/eqoa.iso
test walker::tests::a_login_stands_where_it_logged_out_not_on_what_is_over_it ... SKIP: no run/eqoa.iso
test walker::tests::a_move_lands_on_the_floor_under_it_not_a_roof_over_it ... SKIP: no run/eqoa.iso
test walker::tests::a_place_given_between_frames_stands ... SKIP: no run/eqoa.iso
test walker::tests::a_run_steered_by_the_mouse_keeps_the_consoles_frames ... SKIP: no run/eqoa.iso
test walker::tests::a_run_steered_by_the_mouse_keeps_to_the_ground_of_a_slope ... SKIP: no run/eqoa.iso
test walker::tests::a_sidestep_steered_by_the_mouse_keeps_the_camera_smooth ... SKIP: no run/eqoa.iso
test walker::tests::a_stall_is_one_long_frame ... SKIP: no run/eqoa.iso
test walker::tests::drawn_at_any_rate_the_walk_moves_every_frame ... SKIP: no run/eqoa.iso
test walker::tests::goto_stands_on_the_ground_of_the_square ... SKIP: no run/eqoa.iso
test walker::tests::the_cameras_no_turn_is_kept_with_the_frame ... SKIP: no run/eqoa.iso
test walker::tests::the_player_stops_at_a_creature_in_its_way ... SKIP: no run/eqoa.iso
test walker::tests::the_view_ends_at_the_games_far_plane ... SKIP: no run/eqoa.iso
test walker::tests::the_walk_keeps_the_consoles_frames_at_any_display_rate ... SKIP: no run/eqoa.iso
test walker::tests::zoomed_out_the_players_body_moves_as_at_the_games_camera ... SKIP: no run/eqoa.iso
test weather::tests::a_ports_sky_is_set_at_once_and_the_weather_after_it_is_not_held ... SKIP: no run/eqoa.iso
test weather::tests::the_orders_after_a_ports_sky_wait_for_its_world ... SKIP: no run/eqoa.iso
test wholist::tests::the_patchs_sort_is_read_out_of_the_live_s2 ... SKIP: no live S2 at <worktree>\crates\eqoa\../../..\extracted-assets/sandstorm-patch-20261015/S2
test a_zone_on_its_own_is_the_whole_parse_subtree ... SKIP: no disc at <worktree>\crates\esf\../../run/eqoa.iso
test tunaria_opens_with_its_zones_on_the_disc ... SKIP: no disc at <worktree>\crates\esf\../../run/eqoa.iso
test the_sword_of_virtues_materials_are_emissive ... SKIP: no run/eqoa.iso
test the_2002_disc_stands_in_for_char_esf ... SKIP: no Frontiers disc or no 2002 disc (set EQOA_ISO and EQOA_2002_ISO)
test every_flora_sprite_builds_inside_its_box ... SKIP: no disc (set EQOA_ISO)
test first_loaded_file_wins ... SKIP: <worktree>\crates\esf\../..\run/eqoa.iso not found (set EQOA_ISO)
test group_members_resolve ... SKIP: <worktree>\crates\esf\../..\run/eqoa.iso not found (set EQOA_ISO)
test an_lod_body_builds_its_levels_apart ... SKIP: no disc at <worktree>\crates\esf\../..\run/eqoa.iso
test meshes_match_reference_parser ... SKIP: <worktree>\crates\esf\../..\run/eqoa.iso not found (set EQOA_ISO)
test the_disc_carries_every_level_the_game_keeps ... SKIP: no disc (set EQOA_ISO)
test surfaces_match_reference_parser ... SKIP: <worktree>\crates\esf\../..\run/eqoa.iso not found (set EQOA_ISO)
test a_zones_terrain_builds_from_its_record_alone ... SKIP: no disc at <worktree>\crates\esf\../../run/eqoa.iso
test sandstorms_memory_dumps_read_whole ... SKIP: no capture at <worktree>\crates\net\../../../pcaps/ps2_sandstorm.pcap
test the_ps2s_own_self_states_re_encode ... SKIP: no capture at <worktree>\crates\net\../../../pcaps/ps2_sandstorm.pcap
test every_skeleton_on_the_disc_fits_the_palette ... SKIP: no disc at run/eqoa.iso
test an_ambient_tracks_stop_that_finds_the_packet_full_still_stops ... SKIP: no disc at <worktree>\crates\snd\../../run/eqoa.iso
test a_planted_difference_fails ... SKIP: no oracle at <worktree>\crates\snd\../../../eqoa-port/build/oracle_snd (make oracle in eqoa-port)
test snddrv_matches_the_decomp ... SKIP: no oracle at <worktree>\crates\snd\../../../eqoa-port/build/oracle_snd (make oracle in eqoa-port)
test a_planted_difference_fails ... SKIP: no oracle at <worktree>\crates\snd\../../../eqoa-port/build/oracle_snd (make oracle in eqoa-port)
test visounddevice_matches_the_decomp ... SKIP: no oracle at <worktree>\crates\snd\../../../eqoa-port/build/oracle_snd (make oracle in eqoa-port)
test supports_atlas_reads_whole ... SKIP: needs a SUPPORT image (run/support.bin, or the disc at run/eqoa.iso)
test forms_hold_no_items ... SKIP: no disc at run/eqoa.iso
test music_regions_equal_brute_force ... SKIP: no disc
test colour_buffers_fit_their_sprites ... SKIP: no disc
test camera_effect_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test coll_sphere_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test layout_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test libm_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test walk_camera_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test walk_control_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_play_once_idle_plays_again_at_each_fidget ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test character_animation_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_mutated_constant_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_atmos (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_planted_difference_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_atmos (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test the_atmosphere_matches_the_decomp ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_atmos (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test calc_planes_as_the_decomp ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test the_clip_stack_as_the_decomp ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test collide_obj_on_real_buffers_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test matrices_and_primitives_match ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test query_obj_on_real_buffers_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test triangle_edge_vertex_match ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_mutated_constant_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_effect (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_planted_difference_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_effect (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test effect_volumes_match_the_decomp ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_effect (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test flora_matches_the_decomp ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_flora (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test every_call_matches_over_scripted_libpad ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test pad_layout_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test planted_mistakes_are_seen ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test scale_stick_matches_over_every_byte ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_planted_difference_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_particle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test particles_match_the_decomp ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_particle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_planted_hit_colour_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test pick_leaves_match ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test pick_pack_vunc_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_planted_difference_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test capsule_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test collide_ray_obj_on_real_buffers_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test ray_leaves_match ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test shadows_match_the_decomp ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_shadow (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_changed_world_rule_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_spell (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_planted_record_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_spell (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test an_effect_with_a_longer_wait_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_spell (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test spell_effects_match_the_decomp ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_spell (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test the_swing_tables_match ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test the_quick_chat_tree_matches ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test clip_planes_and_far_plane ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test init_time_of_day ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test profiles_and_blend ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build/oracle (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test a_planted_difference_fails ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_trail (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test trails_match_the_decomp ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_trail (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test the_ui_layer_records_what_the_decomp_records ... SKIP: no oracle at <worktree>\crates\vi\../../../eqoa-port/build\oracle_ui (set EQOA_ORACLE, or `make oracle` in eqoa-port)
test roofed_ground_is_darker_than_open_ground ... SKIP: no disc
test tunaria_off_the_2002_disc_is_the_same_ground ... SKIP: no Frontiers disc or no 2002 disc (set EQOA_ISO and EQOA_2002_ISO)
test a_creature_stops_a_walk_as_its_capsule ... SKIP: no disc
test creatures_are_in_the_rooms_they_reach_as_they_move ... SKIP: no disc
test flora_on_tunaria ... SKIP: no disc
test every_portal_as_its_record_has_it ... SKIP: no disc
test the_walk_from_inside_closed_rooms ... SKIP: no disc
test the_walk_from_outdoors ... SKIP: no disc
test shadow_query_on_tunaria ... SKIP: no disc
test bsp_walk_equals_brute_force ... SKIP: no disc
test zone_by_zone_equals_whole ... SKIP: no disc
```


## Independent review follow-up to b5e6310

- Removed unused client-state capture, state-file reads/writes, the ZIP attachment,
  crash section and dialog promise. A legacy snapshot fixture is ignored; the ZIP
  contents assertion now checks the exact note/screenshot/log/manifest set and
  the dialog's matching promises. Enhanced version/mod headers stay intact.
- Removed the second logger implementation and its duplicate tests. Native stderr,
  file logging, caught-panic handling and the single native panic-hook install
  belong to upstream logfile.rs. The report core keeps its bounded tail and hooks.
- Restored Report pick coverage with the 28px atlas row and misplaced-position
  negative controls. A drop guard closes the logger on test unwind.
- Removed the settings describer and orphan web policy files; aligned seam docs,
  registered HUD-button/surface hooks, trimmed budgets, and standardized the
  portable open predicate with two shared-line changes.
- Deferred the pre-existing second-window report log issue to backlog.md.

`cargo test --locked -p eqoa sync_ -- --nocapture --test-threads=1`: **5 passed**.
`cargo test --locked -p eqoa report -- --nocapture --test-threads=1`: **44 passed**.
Restored HUD pick regression: **1 passed** (GPU available; no skip).
`cargo test --locked --workspace -- --nocapture --test-threads=1`: **PASS**,
**1,773 passed, 0 failed, 27 ignored** over 96 summaries; client
**1,500 passed / 0 failed / 26 ignored**. Explicit prerequisite SKIP messages:
**197**. The two removed duplicate logger tests explain the pass-count decrease.
Python tools: **46 passed**. Footprint: **PASS**, 33 shared files within policy;
options +194/-42, play +750/-53, settings +322/-17, HUD +315/-14 (restored test).
The native source audit confirms logfile.rs is the sole native panic-hook owner.

`cargo build --release --locked -p eqoa`: **PASS**, optimized, 1m 40s.
Current EXE: 32,701,440 bytes; SHA256 `6b3272c2509f2fdba83105112c72401f97af5026061ac9ab358eee3ec67f8788`. PE x64 GUI / 8 MiB stack: PASS.
The new review preview is copied after the follow-up commit under the main
checkout's `run/sync-2026-11-03-<follow-up-shortsha>/`, with identical EXE, bundled
mods, catalog, launcher and checklist. The checklist lists note/screenshot/logs/
manifest only. The old b5e6310 preview remains unchanged. Live owner QA remains
pending; the second-window log selection issue is deferred.


## Owner-requested small client snapshot follow-up

Client state is restored as a fork-owned snapshot at the owner's request. The
upstream describers remain deleted. `fork_ext/report_state.rs` is a child of the
Play adapter, fed by its existing frame_library hook (now forwarding the walker).
One closure is accepted at most once per second before formatting or filesystem
work; state.txt is scrubbed before writing and client_state.txt before ZIP export.
The crash section includes the last noted state. Startup collects the old state
with an interrupted log, then clears it so a new run cannot borrow it.

Coverage and actual negative control:

- `report_state_describes_play_mode_target_and_nearest_entities`: Play fixture
  covers mode 33, named target with level/distance/id, nearest-eight ordering and
  cap, remote player and both own/remote pets; character names stay visible.
- `report_state_includes_claimed_hud_elements_and_movement_flags`: published HUD
  claims, map/search and autorun/follow are present; releasing claims removes them.
- `report_state_frame_hook_populates_saved_zip_with_redaction`: invokes the real
  frame_library adapter, saves to a temporary directory, extracts client_state.txt,
  and checks account/password/home redaction in both persisted state and ZIP.
  Temporarily removed the production `crate::report::note_state(...)` line from
  fork_ext/play.rs, ran this test and observed exit 101 / 0 passed / 1 failed:
  **"client_state.txt is empty without the per-frame report hook"**. Restored the
  exact source bytes; the final report filter and full workspace run pass it.
- `report_state_fake_clock_limits_describing_to_once_per_second`: nine frame
  timestamps (including repeated and 999/1000 ms boundary values) invoke the
  describer exactly three times at 0, 1000 and 2000 ms.
- `a_crash_carries_its_own_runs_version_and_log`: confirms the previous run's
  snapshot appears under `[client, as last noted]`, and startup removes the old
  state file. A clean-close log creates no crash report.
- The ZIP contents test again checks the exact available attachments and the
  dialog's matching client-state promise. The earlier F3/crash/journal hook
  removal controls remain recorded above; no shared reporting logic is restored.

Every cargo command uses the shared mkdir lock, an EXIT cleanup trap that is
installed only after acquisition, `timeout 2400`, this worktree's target directory,
and scratch EQOA_CONFIG under target/. Test commands use --test-threads=1.

- `cargo test --locked -p eqoa sync_ -- --nocapture --test-threads=1`: 5 passed.
- `cargo test --locked -p eqoa report -- --nocapture --test-threads=1`: 48 passed.
- `cargo test --locked --workspace -- --nocapture --test-threads=1`: PASS,
  **1,777 passed / 0 failed / 27 ignored**, across 96 summaries; client
  **1,504 passed / 0 failed / 26 ignored**. **197 explicit SKIP messages**, exactly
  the same prerequisite messages recorded for the previous review run. Missing
  disc/oracle/reference resources and symlink privileges remain unavailable.
- `C:/Python314/python.exe tools/check_fork_footprint.py --base b8e02cd`: PASS,
  303 fork-owned / 33 shared paths. The new named capture seam is guarded once.
  Shared budgets remain unchanged, including play +750/-53. reviewed_base stays
  b8e02cd781aac6a1b9e9906f09896956c77afcd6.

Live owner QA is pending. The pre-existing second-window log selection issue
stays deferred. No other branch is changed; no push, tag or protected merge.

Locked release build: PASS, optimized (1m 16s), x64 GUI / 8 MiB stack validated.
EXE: 32,745,472 bytes; SHA256 `4a7e85dce82a833b992d5da780db030a2ea8b3206f09cc0c898ed00e0335065b`. A new
main-checkout run/sync-2026-11-03-<snapshot-shortsha>/ preview is copied after the
follow-up commit; its checklist includes client_state.txt and the crash section,
with exact EXE and bundled-mod hash comparisons. Earlier preview folders stay.
