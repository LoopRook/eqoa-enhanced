FAIL â€” verification incomplete; full client suite running.

Reviewed range `8111306a..969739e4`; reviewed footprint base `11aa5f088bb52d5f306b0224e37b782b1bf4503d`.

## Behavioral checks

- Lazy event `change_event_before_and_after_are_built_only_when_read`: passed in focused suite; changing the lazy lookup to always return `after` made it fail (1 failed, exit 101). Original bytes restored.
- `forge_adapter_objects_are_reused_only_while_their_snapshot_is_unchanged`: passed in focused suite; removing snapshot equality from cache reuse made it fail (1 failed, exit 101). Original bytes restored. Test checks changed cooldown and icon as well as identity.
- `entity_updates_keep_the_list_right_through_removals_and_the_changed_keys_are_exact`: passed in the full suite, including shifted entries, removal, and no-duplicate assertions.
- After restoration, Git status was clean. The requested broad `git checkout -- .` was rejected by automatic approval review due to risk of discarding unrelated tracked changes; both mutated files were independently restored byte-for-byte instead.

## Existing tests and bundled mods

Existing test files changed: `addons/native_tests.rs` and `api_capture_tests.rs` only append tests; `addons/round_3_picture_tests.rs` exposes two helpers; `addons/round_minimap_manager_tests.rs` extends/instruments a harness and adds a test; `map_mod_tests.rs` wraps event data in `Arc`. `addons/perf_round_tests.rs` is new. No existing assertions were removed or loosened.

The requested bundled-mods/mods diff changes journal, map search, and SOURCE.txt, but no Wereoxxs, ARPG, or Avoids Modern UI files. Existing available integration tests passed, including bundled ARPG/Modern lifecycle and Wereoxxs-specific checks; reference-dependent tests were ignored. All three picture comparisons reported SKIP because reviewed goldens were unavailable.

## Checks

- Focused `fork_ext::`: 549 passed, 0 failed, 24 ignored, 1570 filtered; nine SKIP lines: four symlink privilege, pinned Forge grid, pinned hearts, and three missing UI picture goldens.
- Full client: 2,107 passed, 0 failed, 30 ignored, 6 filtered; 49 explicit SKIP lines plus one lowercase skipped diagnostic. See the complete normalized skip lines below.
- Footprint against reviewed base: PASS.
- `check_no_local_paths.py`: PASS.
- `test_player_release.py`: PASS (20 tests).

Both suites use the shared Cargo target and lock, disc, fresh empty configuration, one test thread and captured output. I attempted to filter `frame_time`, `benchmark`, `perf_round_tests::`, `round4_play_phase_hooks_do_nothing_when_no_addon_is_enabled`, `round4_measure_lua_journal_save_14_mib`, and `forge_wereoxxs_worst_case_scrollback_10000_messages_and_budget_measurements`. However, the five tests in `perf_round_tests.rs` are included under `addons::manager::tests`, so the module filter did not match. `round4_guide_costs_under_half_a_millisecond`, `round4_record_build_micro`, `round4_seven_mods_in_a_real_shaped_scene`, `round4_forge_uis_split_between_our_layer_and_their_lua`, and `round4_snapshot_read_cost_micro` ran in both suites. This violated the explicit no-benchmark instruction; the timings were not analyzed. The other named filters applied.

## Full-suite SKIP lines

Paths have been omitted. Repeated lines represent separate tests.

```text
SKIP: no run/client.bin
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
SKIP forge_gpu: set EQOA_FORGE_SPIKE to the pinned local screen_grid.lua
SKIP pinned hearts: EQOA_FORGE_SPIKE prerequisite missing
SKIP symlink test: privilege not held (A required privilege is not held by the client. (os error 1314))
SKIP arpg_ui picture comparison: no reviewed EQOA_BUNDLED_UI_GOLDENS
SKIP avoids_modern_ui picture comparison: no reviewed EQOA_BUNDLED_UI_GOLDENS
SKIP wereoxxs_ui picture comparison: no reviewed EQOA_BUNDLED_UI_GOLDENS
SKIP: no live save state (fixture missing)
SKIP: no run/client.bin
SKIP: no run/client.bin
SKIP: no run/client.bin
SKIP: no live S2 (fixture missing)
SKIP: no run/client.bin
SKIP: no run/client.bin
SKIP: no run/client.bin
SKIP: no run/client.bin
SKIP: no run/client.bin
skipped: the decomp's virace.cpp/viclass.cpp are not beside the repository (EQOA_DECOMP)
SKIP: no Frontiers or no 2002 disc
SKIP: no 2002 disc (set EQOA_2002_ISO)
SKIP: no 2002 disc (set EQOA_2002_ISO)
SKIP: no 2002 disc (set EQOA_2002_ISO)
SKIP: no disc or no ss-art.csf
SKIP: no live S2 (fixture missing)
SKIP: no live S2 (fixture missing)
SKIP: no july S2 (fixture missing)
SKIP: no ours S2 (fixture missing)
SKIP: no live S2 (fixture missing)
SKIP: no july S2 (fixture missing)
SKIP: no ours S2 (fixture missing)
SKIP: no WORDS.TXT (fixture missing)
SKIP: no live S2 (fixture missing)
SKIP (the rocks): no world10.json in EQOA_WORLDS_DIR
SKIP: no run/eqoa.iso or EQOA_WORLDS_DIR
SKIP: no 2002 disc (set EQOA_2002_ISO)
SKIP: no 2002 disc (set EQOA_2002_ISO)
SKIP: no 2002 disc (set EQOA_2002_ISO)
SKIP: no 2002 disc (set EQOA_2002_ISO)
SKIP: no 2002 disc (set EQOA_2002_ISO)
SKIP: no run/client.bin
SKIP: no run/client.bin
SKIP: no run/client.bin
SKIP: no run/client.bin
SKIP: no Frontiers disc or no 2002 disc (set EQOA_2002_ISO)
SKIP: no Frontiers disc or no 2002 disc (set EQOA_2002_ISO)
SKIP: no Frontiers disc or no 2002 disc (set EQOA_2002_ISO)
SKIP: no live S2 (fixture missing)
```

