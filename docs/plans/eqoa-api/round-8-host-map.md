# Round 8 WP8: mod-host map of crates/eqoa/src/fork_ext/ (2026-11-22)

## 1. Summary
- 144 files under `fork_ext/` (recursive), 120,323 lines: HOST 43 files / 35,944 lines, API 55 / 50,427, FORGE 16 / 16,455, FEATURE 19 / 7,611, SEAM 11 / 9,886 (section 6).
- Classes come from each file's module doc comment, first function and the greps below; I did not read every file in full. Test files take the class of the code they test. Borderline calls are in section 7.
- **Linux/macOS blocker (HIGH):** `addons/sandbox.rs:193-199` returns "unsupported: secure rooted file operations are not implemented on this platform" off Windows. File-backed mod features go through it (`file_io.rs`, `sound.rs:944-949`), so the host would build but fail closed on files. It needs a Unix implementation.
- `addons/sound.rs` (FORGE) is Windows-only in practice (MCI, `winmm.dll`); elsewhere it gives `SoundDiagnostic::UnsupportedPlatform` (`sound.rs:115`).
- Other host Windows items already have a non-Windows branch: `fonts.rs:610/623`, `json_save.rs:66`, `player_files.rs:285/321`, `bundled.rs:593`, `mods.rs:128-139`. The 8 MB stack and GUI subsystem sit outside `fork_ext` (`build.rs:34-40`, `main.rs:45`) and need nothing on Linux/macOS.
- Play's private fields are reached through `#[path]` children only by API (`play.rs:15` -> `api_capture.rs` and its six children) and FEATURE (`play.rs:1235` `report_state.rs`); HOST and FORGE do not.
- Dependencies run SEAM -> API/HOST, HOST -> API and FORGE (`host.rs:22-27` embeds Forge snapshot types), FORGE -> API and HOST. No API -> FORGE use found, so Forge is not separable from the host struct today.
- Not checked: any Linux/macOS build (nothing compiled), the `eqoa/1` spec text, wasm32 `cfg`s beyond `mod.rs`, a full call matrix.

## 2. Per-file classification

Format: path | class | lines | evidence (module doc, then one function name in the file); `(test)` marks test files.

### API

- `addons/action_broker.rs` | API | 5818 | Action broker: the per-mod action queue, 30 Hz rate limiting, round-robin; e.g. `allow`
- `addons/actions.rs` | API | 94 | Action and window dispatch for `eqoa.actions` and `eqoa.windows`.; e.g. `queue_assist`
- `addons/bag_viewer_tests.rs` | API | 4816 | (test) A flat colour per icon id (the fixture's art cache); none of them is the worn marker's green.; e.g. `bv_icon_color`
- `addons/bundled_picture_tests.rs` | API | 139 | (test) GPU readback/PNG comparison, following gpu_test.rs and renderer picture tests.; e.g. `pixels`
- `addons/chat_history.rs` | API | 43 | eqoa.ChatHistory()`: the stock chat-history scroll, for addons that draw; e.g. `install`
- `addons/chat_history_tests.rs` | API | 470 | (test) eqoa.ChatHistory()` and the bundled Wereoxxs chat scroller.; e.g. `chat_history`
- `addons/docs_tests.rs` | API | 729 | (test) Round 7 Part D2: the author documentation against the running client.; e.g. `docs`
- `addons/effects.rs` | API | 264 | (no doc comment); e.g. `name_str`
- `addons/gems.rs` | API | 177 | eqoa.gems`: the crafting gems the game's players know by name. Static shipped metadata (name,; e.g. `lookup`
- `addons/gesture.rs` | API | 258 | Physical player gesture gate and synthetic report guard for game actions.; e.g. `enter_gesture`
- `addons/guides.rs` | API | 338 | Mod-owned guidance; no personal/group waypoint writes.; e.g. `guide_count`
- `addons/hud_layer.rs` | API | 713 | Mod HUD Layer: Native HUD pass drawing for Lua mods.; e.g. `drawn_commands`
- `addons/hud_layout.rs` | API | 342 | HUD frame, layout rects and visibility for `eqoa.hud.frame()`, `eqoa.hud.layout()`, `eqoa.hud.visible()`.; e.g. `set_hud_state`
- `addons/input.rs` | API | 1493 | Routes a live logical key after native chat/menu and ImGui focus gating.; e.g. `observe_report`
- `addons/map_migration.rs` | API | 50 | One-time native-to-mod journal copy. The legacy file is always read-only.; e.g. `migrate`
- `addons/map_mod_manager_tests.rs` | API | 550 | (test) (no doc comment); e.g. `r4_begin`
- `addons/map_views.rs` | API | 683 | One native minimap output, shared by held Lua views. No terrain generation here.; e.g. `project`
- `addons/map_waypoint.rs` | API | 123 | The player's own map waypoint as mods move it (`eqoa.map.set_waypoint` / `clear_waypoint`). The pin itself; e.g. `placed`
- `addons/native.rs` | API | 1452 | The versioned native surface. Capability presence is independent of game state.; e.g. `session`
- `addons/native_core.rs` | API | 1048 | Plain owned game values and an ordered commit journal. No Lua at mutation sites.; e.g. `none_windows_json`
- `addons/native_events.rs` | API | 1122 | Per-VM subscriptions, source cursors and timers; dispatch only at Lua boundaries.; e.g. `bus`
- `addons/native_input.rs` | API | 352 | (no doc comment); e.g. `release_vm`
- `addons/native_items.rs` | API | 934 | eqoa.inventory`, `eqoa.bank`, `eqoa.loot`, `eqoa.merchant` and `eqoa.items` (spec section 6):; e.g. `items_prepare`
- `addons/native_media.rs` | API | 410 | (no doc comment); e.g. `imgui`
- `addons/native_r6a.rs` | API | 609 | Round 6 Part A native Lua bindings (`eqoa.quests.*`, `eqoa.mastery.*`, and later camera/windows).; e.g. `resolve_in`
- `addons/native_state.rs` | API | 1483 | The native Stats Box toggle's local diagnostic wording.; e.g. `make_ref`
- `addons/native_tests.rs` | API | 1901 | (test) Events the host's own services emit, driven in `round7c_host_service_events_reach_lua_from_their_producers`.; e.g. `install_field_check`
- `addons/perf_delta_event_tests.rs` | API | 302 | (test) Performance round 2 (entity deltas): what a handler gets from a delivered event is what an eagerly; e.g. `paths`
- `addons/perf_delta_tests.rs` | API | 115 | (test) Performance round 2 (entity deltas): the cheaper ways of doing a thing give exactly what the old way gave.; e.g. `next`
- `addons/pointer_api.rs` | API | 383 | Window pointer observations and frame-lived addon rectangle claims.; e.g. `claim_count`
- `addons/record_raw.rs` | API | 271 | A native record built in one protected Lua call.; e.g. `build`
- `addons/round6_loot_recording.hex` | API | 6 | (no doc comment)
- `addons/round7a_collect_diagnostics.py` | API | 43 | (no doc comment)
- `addons/round7c_fix_tests.rs` | API | 545 | (test) Round 7 Part C, fix round 1: each review finding reproduced by a test before its fix.; e.g. `new`
- `addons/round7e_key_tests.rs` | API | 1631 | (test) What the page says with `page` as it is: every text it draws (the help; e.g. `reset`
- `addons/round7h_tests.rs` | API | 235 | (test) Round 7H: the last corrections before the `eqoa/1` freeze (the D2 spec pass's list), the parts that need; e.g. `status`
- `addons/round_3_picture_tests.rs` | API | 1540 | (test) Round 3 Picture tests and per-frame cost benchmark for the four HUD extra mods.; e.g. `new`
- `addons/round_minimap_manager_tests.rs` | API | 1319 | (test) What the viewer's HUD pass has that the harness did not: the HUD layer drawn through a `Ui` to the GPU.; e.g. `new`
- `addons/weapon_sets.rs` | API | 197 | Native menu 3, not an inferred equipped/active weapon set.; e.g. `from_native`
- `addons/weapon_sets_tests.rs` | API | 170 | (test) (no doc comment); e.g. `weapon_sets_api_is_unavailable_without_native_data`
- `addons/window_actions.rs` | API | 467 | Round 6 window and quest actions (`eqoa.actions.drop_quest`, `mastery_*`, `close_window`,; e.g. `signature`
- `addons/window_kinds.rs` | API | 117 | The game's window kinds: the `Window.kind` names `eqoa.windows.get()` reports (spec Â§7, "Camera and; e.g. `mode_name`
- `addons/window_takeover.rs` | API | 919 | Window takeover registry for native loot, merchant, and bank windows.; e.g. `log`
- `api_capture.rs` | API | 492 | Native state projection, as a child of Play's adapter (access stays private).; e.g. `in_world_now`
- `api_capture_tests.rs` | API | 3687 | (test) Round 7 Part C fix M5b: `eqoa.map.set_waypoint`/`clear_waypoint` move the player's own waypoint through the; e.g. `replay_ui_fixture`
- `api_updates.rs` | API | 996 | Typed, bounded source deltas; JSON conversion occurs only at an addon boundary.; e.g. `key`
- `chat_history.rs` | API | 166 | Chat-history scrolling, published to addons as `eqoa.ChatHistory()`.; e.g. `key`
- `items_capture.rs` | API | 613 | The item containers as plain data (spec section 6): the pack and the bank from the dump, the; e.g. `current_loot_source_id`
- `items_tests.rs` | API | 4700 | (test) Round 5: the items API from Play-shaped state. Items are encoded as the server sends them and read; e.g. `bags_item_page`
- `loot_source.rs` | API | 77 | Which corpse the client asked to loot (0x0015): `Play::attack_press` tells it (one hook call) and the; e.g. `requested`
- `map_mod_tests.rs` | API | 1393 | (test) Bundled map mods through the real Lua host and Play/protocol capture.; e.g. `new`
- `r6a_capture.rs` | API | 1176 | Round 6 Part A native state capture: quests and class mastery (plus camera and windows in subsequent steps).; e.g. `windows_json`
- `r6a_tests.rs` | API | 1454 | (test) Round 6 Part A integration tests built from Play/protocol-shaped state.; e.g. `ready`
- `r6d_tests.rs` | API | 669 | (test) Round 6 window and quest actions, driven from Play-shaped state through the broker.; e.g. `ready`
- `round7h_play_tests.rs` | API | 333 | (test) Round 7H: the last corrections before the `eqoa/1` freeze, driven from a `Play` through the action broker; e.g. `ready`

### FEATURE

- `app_icon.rs` | FEATURE | 139 | The owner's program icon: build.rs chooses one of two .ico files; e.g. `rgba_from`
- `app_icon_ico.rs` | FEATURE | 105 | Which of the owner's two program icons a build carries.; e.g. `choose`
- `crashfile_dirs_tests.rs` | FEATURE | 209 | (test) The fork's local crash reports and upstream's kept crash reports share; e.g. `scratch`
- `map_api.rs` | FEATURE | 336 | Native map adapter. Private cache access stays within Map's module.; e.g. `api_waypoints_loaded`
- `map_render_tests.rs` | FEATURE | 88 | (test) The world map keeps window coordinates when drawn into a Render Scale target.; e.g. `capture`
- `player_stats.rs` | FEATURE | 811 | Fork-owned player stats computation for the native Stats Box and `eqoa.player.stats()`.; e.g. `as_str`
- `release.rs` | FEATURE | 34 | Release routing is fork-owned; shared updater mechanics remain upstream's.; e.g. `base`
- `report.rs` | FEATURE | 1581 | Fork-owned problem and crash report saving to the Desktop.; e.g. `mod_scans_on_this_thread`
- `report_background.rs` | FEATURE | 177 | Report I/O never runs on the per-frame snapshot path.; e.g. `enabled_mods_addons_cached`
- `report_core.rs` | FEATURE | 2308 | Fork-owned local problem and crash reports retained after upstream removed reports.; e.g. `secret`
- `report_crash_dir.rs` | FEATURE | 90 | Where this fork keeps its own local crash reports.; e.g. `in_config`
- `report_state.rs` | FEATURE | 427 | Small diagnostic snapshot; a child of Play's fork adapter, never game logic.; e.g. `describe`
- `report_sync_tests.rs` | FEATURE | 128 | (test) F3 must reach Support's form while the map is up: upstream handles it; e.g. `sync_f3_saves_redacted_zip_through_dialog_hook`
- `report_viewer.rs` | FEATURE | 451 | Native report adapter: input ownership and frame capture.; e.g. `claims_key`
- `scripted_input.rs` | FEATURE | 360 | Play test inputs through the native window's event routes.; e.g. `steps`
- `session_replay.rs` | FEATURE | 62 | Passive playback into a test Session backed by in-memory Datagrams.; e.g. `rows`
- `telemetry.rs` | FEATURE | 122 | Plain combat events and the native localhost broadcaster.; e.g. `json`
- `touch_tests.rs` | FEATURE | 121 | (test) Integration checks for the reviewed upstream touch implementation.; e.g. `upstream_touch_dpad_reaches_quick_chat_and_releases_without_a_click`
- `version.rs` | FEATURE | 62 | The build label players see. Stable and local builds show VERSION as it is;; e.g. `display_version`

### FORGE

- `addons/ability.rs` | FORGE | 2029 | Native FrontiersForge ability adapters (`frontiers_forge.ability`, `ability_list`, `ability_bar`, `icon`).; e.g. `record_lockout`
- `addons/chat.rs` | FORGE | 904 | Native FrontiersForge chat adapter (`frontiers_forge.chat`).; e.g. `queue_outgoing_chat`
- `addons/combat.rs` | FORGE | 736 | Native FrontiersForge combat adapter (`frontiers_forge.combat`).; e.g. `record_combat_event`
- `addons/entity.rs` | FORGE | 1323 | Native FrontiersForge entity and entity_list adapter module (`frontiers_forge.entity`, `frontiers_forge.entity_list`).; e.g. `install_entity_adapter`
- `addons/game_health.rs` | FORGE | 89 | Receipt validity belongs to a Play lifetime, not the HUD atlas lifetime.; e.g. `receive`
- `addons/group.rs` | FORGE | 496 | Native FrontiersForge group adapter module (`frontiers_forge.group`).; e.g. `install_group_adapter`
- `addons/health.rs` | FORGE | 677 | Native health and HUD suppression adapter slice (Stage 2E).; e.g. `new`
- `addons/icon_tests.rs` | FORGE | 1581 | (test) Unit and GPU integration tests for FrontiersForge ability & effect icons.; e.g. `new`
- `addons/icons.rs` | FORGE | 1777 | Decodes game icon textures into ImGui textures for Lua addons.; e.g. `from_lua_table`
- `addons/items_forge.rs` | FORGE | 710 | The Forge item modules (`frontiers_forge.inventory`, `.item`, `.bank`, `.loot`, `.merchant`,; e.g. `source`
- `addons/native_projection.rs` | FORGE | 185 | Forge's bounded legacy records, projected from the native core.; e.g. `apply`
- `addons/object_cache.rs` | FORGE | 41 | The Forge adapters' snapshot objects (an ability, an item, a bar slot) are tables of closures: one; e.g. `memo`
- `addons/player.rs` | FORGE | 1162 | Native FrontiersForge player adapter module (`frontiers_forge.player`).; e.g. `install_player_adapter`
- `addons/quest_log.rs` | FORGE | 131 | Retail journal titles, in the packed zero-based order used by Forge.; e.g. `from_titles`
- `addons/sound.rs` | FORGE | 2089 | Windows MCI-backed sound bank for UiForge compatibility.; e.g. `new`
- `addons/wereoxxs_test.rs` | FORGE | 2525 | (test) Full integration test suite for the unchanged wereoxxs_ui package.; e.g. `modern_slot_positions`

### HOST

- `addons/addon_perf.rs` | HOST | 359 | What each addon costs per frame, printed under `EQOA_PERF` beside upstream's own `perf:` line.; e.g. `enabled`
- `addons/async_json.rs` | HOST | 450 | Bounded read-only JSON decoding off the game thread. Lua table materialization stays on its VM.; e.g. `close`
- `addons/bit.rs` | HOST | 393 | Native LuaJIT BitOp 1.0.2 compatible `bit` library implementation.; e.g. `to_i32`
- `addons/bridge.rs` | HOST | 1523 | (no doc comment); e.g. `drawing`
- `addons/bridge_drawlist.rs` | HOST | 569 | Scoped Lua draw lists; native pointers exist only during validated calls.; e.g. `window_token`
- `addons/bridge_drawlist_tests.rs` | HOST | 420 | (test) Inactive preparation: activate only after compiler handoff; no results claimed.; e.g. `context`
- `addons/bridge_next_tests.rs` | HOST | 767 | (test) (no doc comment); e.g. `forge_review_color_flags_accept_legacy_unused_bits`
- `addons/bridge_widgets.rs` | HOST | 1218 | Bounded native widget bindings audited against pinned sol_ImGui.h.; e.g. `flags`
- `addons/capture.rs` | HOST | 166 | Physical press ownership for native addon UI input routing.; e.g. `is_empty`
- `addons/discovery.rs` | HOST | 1583 | Native-only addon discovery engine.; e.g. `declared_capabilities`
- `addons/exchange.rs` | HOST | 333 | Cross-mod read-only private publications. Authority comes from a live VM,; e.g. `register`
- `addons/file_io.rs` | HOST | 2297 | Small Lua 5.1 text-file subset. Resource paths are a virtual overlay: reads; e.g. `flush`
- `addons/fixtures/widgets.lua` | HOST | 11 | (no doc comment)
- `addons/fonts.rs` | HOST | 1093 | Safe TrueType font loading, atlas lifecycle management, and ImFont userdata.; e.g. `current_context_epoch`
- `addons/gpu_test.rs` | HOST | 353 | (test) Optional local-source/GPU evidence; never connects to a server.; e.g. `forge_gpu_pinned_grid_and_texture_fixture`
- `addons/host.rs` | HOST | 1183 | (no doc comment); e.g. `managed`
- `addons/hud_textures.rs` | HOST | 144 | Owned texture images in the addon HUD pass. Rounded square geometry masks a circle.; e.g. `image`
- `addons/json_save.rs` | HOST | 575 | Private JSON saves: snapshot Lua data, encode and atomically replace on a bounded worker.; e.g. `close`
- `addons/manager.rs` | HOST | 3236 | Backend lifecycle coordinator. The caller supplies an active frame and owns; e.g. `take_mod_log_for_test`
- `addons/manager_profiles.rs` | HOST | 806 | Profile transitions execute outside NewFrame, at the manager's render boundary.; e.g. `class_display`
- `addons/manager_profiles_tests.rs` | HOST | 1755 | (test) The real 0.4 -> 0.5 update: the player's profile store (Default, the seeded; e.g. `mod_profiles_two_characters_restore_save_state_and_layout_on_fresh_vms`
- `addons/manager_scope_recovery_tests.rs` | HOST | 374 | (test) (no doc comment); e.g. `forge_recovery_caught_error_keeps_addon_running_and_warns_once`
- `addons/mod.rs` | HOST | 143 | Native-only, explicitly opted-in Forge feasibility host.; e.g. `request_open`
- `addons/named_profiles.rs` | HOST | 1078 | Named, process-selected mod profiles. No account or credential input exists here.; e.g. `new`
- `addons/native_settings.rs` | HOST | 205 | Settings use the manager's currently selected storage slot and atomic writer.; e.g. `slot`
- `addons/option_buttons.rs` | HOST | 116 | Native option action rows owned by a mod VM; actions run at its next boundary.; e.g. `is_button`
- `addons/options.rs` | HOST | 398 | Mod option toggles for Enhanced HUD options and `eqoa.options.toggle`.; e.g. `set_storage_root`
- `addons/package.rs` | HOST | 1589 | Isolated per-addon package loader and resource resolver for Forge addons.; e.g. `resolve_resource`
- `addons/perf_alloc.rs` | HOST | 58 | Test-only, current-thread allocation meter; disabled outside measured scopes.; e.g. `measure`
- `addons/perf_round_tests.rs` | HOST | 849 | (test) Instruments a scratch entry function. Keep these probes outside the main timing pass.; e.g. `write_journal`
- `addons/player_files.rs` | HOST | 604 | Player-authorized native file selection and bounded copies, away from the frame.; e.g. `close`
- `addons/profile.rs` | HOST | 99 | Profiles contain bounded plain Lua data, never executable serializer output.; e.g. `encode`
- `addons/runtime.rs` | HOST | 2651 | Lazy native app adapter for addon management and physical-pixel rendering.; e.g. `select`
- `addons/sandbox.rs` | HOST | 369 | Shared virtual mounts for legacy and native file/media entry points.; e.g. `key`
- `addons/spike.rs` | HOST | 385 | Explicit opt-in only. No Context, input interception or draw pass by default.; e.g. `from_env`
- `addons/storage.rs` | HOST | 1558 | Bounded plain-data profiles and versioned enabled state for native addons.; e.g. `new`
- `addons/textures.rs` | HOST | 1755 | Bounded, owner-local texture handles. Lua never receives a GPU/native pointer.; e.g. `dimensions`
- `addons/visibility.rs` | HOST | 1357 | Managed desired visibility is separate from the standalone effective registry.; e.g. `new`
- `bundled.rs` | HOST | 1619 | Auto-extracted bundled mod packages and embedded assets.; e.g. `bundled_registry_path`
- `bundled_shipped_hashes.txt` | HOST | 68 | (no doc comment)
- `bundled_update_tests.rs` | HOST | 198 | (test) The previous, unedited Wereoxxs scroller stays a trusted shipped hash, so; e.g. `new`
- `mods.rs` | HOST | 595 | Drag-and-drop mod engine and companion manager for EQOA Native Rust.; e.g. `mods_dir`
- `modtools.rs` | HOST | 642 | Headless author tools, compiled as a child of addons to use its production installers.; e.g. `run`

### SEAM

- `bindpage.rs` | SEAM | 1368 | Mod-owned keybinds on the Controls page and binds file serialization.; e.g. `set_manager_blocked`
- `hud.rs` | SEAM | 1275 | HUD-number adapter; stock HUD layout remains in the parent.; e.g. `health_visible`
- `map.rs` | SEAM | 31 | Native map output routing: the minimap capture and the one overlay seam.; e.g. `draw_minimap`
- `map_toolbar.rs` | SEAM | 107 | Search toolbar placement from the chrome that was actually composed.; e.g. `note`
- `mod.rs` | SEAM | 92 | Fork-owned services. These modules accept plain data and do not depend on the app.; e.g. `run`
- `options.rs` | SEAM | 549 | The fork's option rows and pages; loaded as an options child.; e.g. `categories`
- `play.rs` | SEAM | 5445 | The combat indication adapter; service code only receives plain combat data.; e.g. `begin_game_event`
- `render_font.rs` | SEAM | 25 | (no doc comment); e.g. `at_height`
- `report_label.rs` | SEAM | 177 | The footprint manifest the build was made with: its `reviewed_base` is the; e.g. `synced_commit`
- `report_no_upload.rs` | SEAM | 11 | Enhanced never uploads a crash or bug report (fork policy: local saves only,; e.g. `blocked`
- `settings.rs` | SEAM | 806 | Enhanced default migration and HUD-number keys in their existing format.; e.g. `migrate_configs`

## 3. Crossings between classes (main items; paths under `crates/eqoa/src/fork_ext/`)

| From -> To | What is used (file:line) |
|---|---|
| SEAM -> HOST | `options.rs:47,152-198` (`addons::options::active_mod_items`/`toggle_by_item`/`activate_button`; `addons::request_open` :177, `take_open_request` :214); `bindpage.rs` (mod keybinds on the Controls page); `hud.rs:270,282-291` (`hud_layer::draw_with_target_alpha`, `health::InstanceToken`, `visibility::publish_claims`) |
| SEAM -> API | `play.rs:106,119,146,242-266` (`action_broker::set_master_loot`, `has_in_flight`, `open_loot_in_flight`, `resolve_server_*`); `play.rs:10,274` (`window_takeover`) |
| SEAM -> FORGE | `play.rs:399` (`game_health::GameHealth` field); `hud.rs:282` (`health::InstanceToken`) |
| API -> SEAM (via #[path]) | `api_capture.rs:3,191,252,279,312,342,453` (`native_core::{CORE,View}`, `player::CLASSES/RACES`, `hud_layout::get_window_size`, `window_takeover::release_all`, `native_core::capture_requested`/`unknown_lockout`); `r6a_capture.rs:4` (`native_core::{CORE,Core,MasteryFlags,changed_fields}`) |
| HOST -> API | `addons/manager.rs:259-260,289,312,353` (`native_core::CORE`, `manager_demand`, `native_events::active`/`bus`); `manager.rs:265` (`input::native::release_vm`) |
| HOST -> FORGE | `addons/host.rs:22-27` (`health::HealthSnapshot`, `player::PlayerSnapshot`, `group::GroupSnapshot`, `entity::EntitiesSnapshot`, `ability::AbilitiesSnapshot`, `chat::ChatSnapshot` held in the host struct) |
| FORGE -> API | `chat.rs:90,333`, `combat.rs:29,53,145` (`native_core::CORE.with(..)`, `view.in_world`); `native_projection.rs` projects the native core into Forge records |
| FORGE -> HOST | `icons.rs:8,1690` (`textures::{Handle,Textures}`, `bridge::Bridge::install`); `sound.rs:944-949` (`sandbox::Mounts`, `sandbox::lock_resource`) |
| API -> FORGE | none found by grep (`native*.rs`, `actions.rs`, `action_broker.rs` do not import `player/ability/quest_log/icons`) |
| HOST (bundled/mods) -> HOST | `bundled.rs:740,954` -> `addons::storage::atomic_replace`; `bundled.rs:1430` -> `addons::discovery::discover` |
| FEATURE -> HOST | `report.rs:49` -> `addons::discovery::discover` |
| FEATURE -> API | `map_api.rs:80,114,182,198` (`map_waypoint::hash_owner`/`owner`, `map_views::Projection`); `hud_layer.rs:212,232` (API) uses `player_stats::ROWS` (FEATURE) |
| API tests -> SEAM | `action_broker.rs:4871..5816` call `bindpage::reset_in_memory()` in tests |
| Upstream files -> fork (seam hooks) | `main.rs:225,466` (`report_core as report`, `modtools::run`), `bindpage.rs:53,183,222,351,410`, `crashfile.rs:246,347`, `hud.rs:591`, `icon.rs:43,192`, `logfile.rs:63,100,411`, `options.rs:584,753`, `play.rs:399,6021,6034,6352,6552`, `settings.rs:249`, `statbox.rs:3` (all under `crates/eqoa/src/`) |

Not checked: a full call matrix. Each pair was sampled with grep over `use` and qualified paths, so the lists are not exhaustive.

## 4. Classes that depend on private fields through `#[path]` children

A `#[path]` child module sees its parent's private items, so it reads `Play`'s private fields without `Play` exposing them.

- `play.rs:15` -> `api_capture.rs` (API; child of Play). `api_capture.rs:474` `api_updates.rs`, `:477` `items_capture.rs`, `:479` `r6a_capture.rs`; tests `:482` `items_tests.rs`, `:485` `r6a_tests.rs`, `:488` `r6d_tests.rs`, `:491` `round7h_play_tests.rs`. All API.
- `play.rs:18` `map_mod_tests.rs`, `play.rs:21` `api_capture_tests.rs` (API tests).
- `play.rs:1235` `report_state.rs` (FEATURE: reads Play's private state for report snapshots).
- `report_viewer.rs:7` `scripted_input.rs` (FEATURE test input); `map_api.rs:335` `map_render_tests.rs` (FEATURE test); `app_icon.rs:8` `app_icon_ico.rs` (FEATURE); `bundled.rs:981` `bundled_update_tests.rs` (HOST test). `crashfile_dirs_tests.rs:5` mentions a `#[path]` line that lives in another file (not checked which).
- Inside the host, not Play: `addons/mod.rs:4` -> `../modtools.rs`; `addons/bridge.rs:10,13,16,18` (drawlist, drawlist tests, next tests, widgets); `addons/input.rs:11` `native_input.rs`; `addons/manager.rs:59` `manager_profiles.rs`; `addons/native_tests.rs:1327` `round7e_key_tests.rs`; `addons/wereoxxs_test.rs:2524` `bundled_picture_tests.rs`.
- So Play's private fields are reached only by API (the `api_capture` family and its tests) and FEATURE (`report_state.rs`). HOST and FORGE do not use Play's privates; HOST reads what the API capture publishes into `native_core::CORE`. `loot_source.rs` is a plain public hook call (`Play::attack_press`), not a `#[path]`.

## 5. What Linux and macOS would need (HOST and API; FORGE/FEATURE noted where they gate the host)

| Windows-only item | file:line | Existing cfg for other systems? |
|---|---|---|
| Secure rooted file ops (`share_mode(3)`, flags `0x02200000`, reparse check `0x400`) | `addons/sandbox.rs:172-200`, `207-243` (`lock_parents`, `lock_resource`) | **No working fallback.** `sandbox.rs:193-199` returns "unsupported: secure rooted file operations are not implemented on this platform". HIGH: file-backed mod features go through it (`file_io.rs`, `sound.rs:944-949`) |
| Reparse-point check (`MetadataExt::file_attributes`) | `addons/file_io.rs:310-314`, `named_profiles.rs:679-681`, `storage.rs:87-89`, `bundled.rs:649,832` | Yes: extra `cfg(windows)` check on top of a portable `is_symlink()`. Hidden/system skip (`storage.rs:87`) is Windows-only; Unix skips dot and `$` names only |
| Case-insensitive path keys | `addons/json_save.rs:66-70`, `player_files.rs:118`, `bundled.rs:593-598` | Yes, `cfg(not(windows))` twins |
| Atomic new-file rename (`windows_sys` `MoveFileExW`) | `addons/player_files.rs:285-318` | Yes, `rename_new` at `:321` (hard_link + remove) |
| System fonts (`SystemRoot`, `C:\Windows\Fonts`) | `addons/fonts.rs:610-621` | Yes, `:623` returns false: no system fonts on Linux/macOS (gap, not a crash) |
| MCI sound (`mciSendStringW`, `waveOutGetNumDevs`, `winmm.dll`) | `addons/sound.rs:202-247,293-311,828-902` (FORGE) | Partial: `SoundDiagnostic::UnsupportedPlatform` (`:115,149`) plus `not(windows)` stubs (`:225,293-311,452`); no real audio |
| Path separators (`\`, `\\?\`, `:` rejects) | `package.rs:51-59,105`, `file_io.rs:201-212,569-572`, `async_json.rs:79,164`, `native_media.rs:115,273`, `fonts.rs:440,598-644`, `host.rs:604-612`, `exchange.rs:86`, `bundled.rs:590` | Portable (the rules also reject `\` and `:` elsewhere; the `\\?\` strip is a no-op elsewhere) |
| Directory symlinks in tests | `discovery.rs:1258-1266`, `package.rs:452-460` | Yes, `cfg(unix)` twin |
| Open mods folder (`explorer.exe`) | `mods.rs:128-139` | Yes: macOS `open` (:132), other Unix `xdg-open` |
| Parser child spawn (`CREATE_NO_WINDOW`, `PATHEXT`) | `mods.rs:280-283,509-516` | Windows-only extras; Unix runs without them |
| `cfg(windows)` tests | `modtools.rs:522,628` (`round7b_support_table_matches_the_running_client`, `eqoa1_frozen_surface_only_grows`); `file_io.rs:1318` | Tests only; they would not run on Linux/macOS. Why they are gated: not checked (they need graphics: `with_graphics`) |
| `windows-sys` dependency | `Cargo.toml:104-105`; used by `fork_ext/player_files.rs:309-312` (HOST) and `report.rs:294-334` (FEATURE: `LoadLibraryA("shell32.dll")`, `GetProcAddress`) | `report.rs:352` has a `not(windows)` twin, `:375` macOS |
| DLL loading in HOST/API | none found; only the FEATURE `report.rs:294` | n/a |
| 8 MB main-thread stack | `crates/eqoa/build.rs:34-40` (msvc `/STACK:8388608`, gnu `-Wl,--stack,8388608`, only when target OS is windows) | Linux/macOS main threads already default to 8 MB; nothing needed. Test-only `big_stack` (16 MiB, `addons/mod.rs:92`) is portable |
| GUI subsystem | `src/main.rs:45` `cfg_attr(all(windows, not(test)), windows_subsystem = "windows")`; console handling `main.rs:266,284` | Windows-only by design; nothing needed elsewhere |
| Home-dir env for scrubbing | `report.rs:193,339`, `report_core.rs:402`, `report_state.rs:379` (`USERPROFILE`; `:379` unwraps `HOME` when `USERPROFILE` is unset) | FEATURE. `report_state.rs:379` would panic with neither set (LOW; not run) |
| Mod folder location | `mods.rs` `mods_dir()`, `storage.rs:415` `data_dir` | not checked (whether `APPDATA` or a home path is assumed) |
| Tests that SKIP off Windows | `fonts.rs:858`, `sound.rs:225,1283` | A Linux run would prove less than Windows |

## 6. Summary table

| Class | Files | Lines |
|---|---:|---:|
| HOST | 43 | 35,944 |
| API | 55 | 50,427 |
| FORGE | 16 | 16,455 |
| FEATURE | 19 | 7,611 |
| SEAM | 11 | 9,886 |
| Total | 144 | 120,323 |

Test files count in their subject's class, so production-only totals are smaller (not measured).

## 7. Judgement calls and gaps

- FORGE: `sound.rs` (MCI "for UiForge compatibility"), `icons.rs`, `health.rs`, `game_health.rs`, `quest_log.rs`, `native_projection.rs` ("Forge's bounded legacy records"), `object_cache.rs`, and the `frontiers_forge.*` adapters (`player`, `entity`, `chat`, `combat`, `group`, `ability`, `items_forge`). `wereoxxs_test.rs` and `icon_tests.rs` are FORGE tests.
- Counted API though they could be Forge or host: `weapon_sets.rs`, `effects.rs`, `input.rs`, `map_migration.rs`, `native_input.rs`, `native_media.rs` (`effects.rs`, `native_input.rs`, `native_media.rs` have no doc comment, so the header did not decide).
- Top-level `chat_history.rs` and `loot_source.rs` are API. `player_stats.rs` and `map_api.rs` are FEATURE but also feed `eqoa.player.stats()` and `eqoa.map` (mixed). `map.rs`, `map_toolbar.rs`, `options.rs`, `settings.rs`, `hud.rs`, `bindpage.rs`, `play.rs` are SEAM adapters upstream's files call.
- `bundled.rs`, `bundled_shipped_hashes.txt` and `mods.rs` (drag-and-drop mod engine and companion manager) are HOST. The Mods & Addons window itself is `addons/runtime.rs` and `addons/manager.rs` plus the `options.rs` rows (HOST/SEAM).
- Not checked: whether `window_takeover.rs` and `action_broker.rs` are API or host internals (counted API per their module docs), and the internals of most host files beyond the greps above.
