# Executed negative-control failure transcript

These runs deliberately reintroduce bugs. Failures are the expected control result, not final failures. Every original source byte was restored; the complete client suite was rerun after the final controls. See the report for commands, mutations, green results and qualifications.

## mutation-batch

```text
running 14 tests
thread 'fork_ext::addons::hud_layer::tests::hud_layer_queue_bounds_and_clears' (27940) panicked at crates\eqoa\src\fork_ext\addons\hud_layer.rs:395:9:
assertion failed: queue_command(addon,
        HudCommand::Gauge {
            x: 0,
            y: 0,
            art: 9,
            fill: 1.0,
            lit: true,
            anchor: render::ui::Anchor::Left,
            vertical: render::ui::Vertical::Top,
        }).is_ok()
FAILED
thread 'fork_ext::addons::manager::tests::disabled_bundled_hud_mod_removes_option_key_rows_and_drawing' (3080) panicked at crates\eqoa\src\fork_ext\addons\manager.rs:2500:13:
assertion failed: items.iter().all(|item|
        !super::super::options::active_mod_item_ids().contains(item))
FAILED
thread 'fork_ext::addons::round_3_picture_tests::bundled_hud_numbers_picture_comparison' (10556) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:324:9:
hud_numbers (640, 480) element [0.0, 35.357143, 200.0, 12.85714]: 174/2800 differ
FAILED
thread 'fork_ext::addons::round_3_picture_tests::bundled_mana_bars_picture_comparison' (10144) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:324:9:
mana_bars (640, 480) element [28.0, 135.0, 140.0, 10.714279]: 184/1540 differ
FAILED
thread 'fork_ext::addons::round_3_picture_tests::bundled_stats_box_picture_comparison' (30636) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:324:9:
stats_box (640, 480) element [198.0, 55.714283, 152.0, 145.71428]: 1742/22344 differ
FAILED
thread 'fork_ext::addons::round_3_picture_tests::bundled_target_of_target_picture_comparison' (32992) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:324:9:
target_of_target (640, 480) element [28.0, 346.0714, 140.0, 10.714294]: 112/1540 differ
FAILED
thread 'fork_ext::addons::round_3_picture_tests::fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring' (13976) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:819:135:
called `Result::unwrap()` on an `Err` value: RuntimeError("[string \"Forge spike\"]:1: assertion failed!\nstack traceback:\n\t[C]: in ?\n\t[C]: in function 'assert'\n\t[string \"Forge spike\"]:1: in main chunk")
FAILED
thread 'fork_ext::addons::round_3_picture_tests::group_mana_does_not_draw_over_raid_or_claimed_stock_panel' (37916) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:1102:5:
raid already has native mana gauges
FAILED
thread 'fork_ext::addons::round_3_picture_tests::hud_layout_uses_native_wide_tall_transforms_and_dynamic_pet_rect' (38536) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:1164:5:
assertion failed: !hud_layout::hud_element_visible("typo")
FAILED
thread 'fork_ext::addons::round_3_picture_tests::menu_values_persist_on_disk_and_reach_reloaded_bundled_lua' (29788) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:1046:10:
called `Result::unwrap()` on an `Err` value: RuntimeError("[string \"Forge spike\"]:1: assertion failed!\nstack traceback:\n\t[C]: in ?\n\t[C]: in function 'assert'\n\t[string \"Forge spike\"]:1: in main chunk")
FAILED
thread 'fork_ext::addons::round_3_picture_tests::round3_capabilities_and_stats_records_match_public_contract' (34924) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:1076:9:
called `Result::unwrap()` on an `Err` value: RuntimeError("[string \"Forge spike\"]:6: assertion failed!\nstack traceback:\n\t[C]: in ?\n\t[C]: in function 'assert'\n\t[string \"Forge spike\"]:6: in main chunk")
FAILED
thread 'play::fork_hooks::native_tests::addon_actions_use_live_camera_assist_and_summary_overlay_gates' (32700) panicked at crates\eqoa\src\fork_ext\api_capture_tests.rs:1739:9:
assertion `left == right` failed: assist dispatch in mode 38
  left: 0
 right: 1
FAILED
thread 'play::fork_hooks::native_tests::group_capture_excludes_player_and_inactive_slots_before_lua' (24784) panicked at crates\eqoa\src\fork_ext\api_capture_tests.rs:1541:154:
called `Result::unwrap()` on an `Err` value: RuntimeError("[string \"Forge spike\"]:1: assertion failed!\nstack traceback:\n\t[C]: in ?\n\t[C]: in function 'assert'\n\t[string \"Forge spike\"]:1: in main chunk")
FAILED
thread 'viewer::fork_report::addon_order_tests::addon_overlay_recorded_first_is_submitted_after_the_scene' (24008) panicked at crates\eqoa\src\fork_ext\report_viewer.rs:259:9:
assertion `left == right` failed: overlay must remain above the later-recorded scene
  left: [255, 0, 0, 255]
 right: [0, 0, 255, 255]
FAILED
test result: FAILED. 0 passed; 14 failed; 0 ignored; 0 measured; 1987 filtered out; finished in 10.02s
```

## reintroduced-controls

```text
running 7 tests
thread 'fork_ext::addons::round_3_picture_tests::fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring' (33456) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:782:10:
called `Result::unwrap()` on an `Err` value: RuntimeError("[string \"Forge spike\"]:1: assertion failed!\nstack traceback:\n\t[C]: in ?\n\t[C]: in function 'assert'\n\t[string \"Forge spike\"]:1: in main chunk")
FAILED
thread 'fork_ext::addons::round_3_picture_tests::mod_click_rect_owns_mouse_inside_and_passes_it_outside' (35448) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:1005:9:
native action at corner of eqoa_stats_box's drawn rect
FAILED
thread 'fork_ext::addons::round_3_picture_tests::tot_click_name_bar_routes_assist_and_passes_outside_clicks' (30636) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:1005:9:
native action at corner of eqoa_target_of_target's drawn rect
FAILED
thread 'fork_ext::bindpage::tests::rebind_write_failure_keeps_old_key_and_reports_failure' (30776) panicked at crates\eqoa\src\fork_ext\bindpage.rs:351:9:
assertion failed: !set_key("key_refusal", "toggle", "f8")
FAILED
thread 'play::fork_hooks::native_tests::live_pet_layout_follows_group_raid_and_native_effect_strip' (12916) panicked at crates\eqoa\src\fork_ext\api_capture_tests.rs:1590:9:
assertion `left == right` failed
  left: 285
 right: 93
FAILED
thread 'play::fork_hooks::native_tests::mod_hud_visibility_tracks_live_menu_map_loading_and_hide_ui' (37052) panicked at crates\eqoa\src\fork_ext\api_capture_tests.rs:1558:5:
menu hides mod HUD
FAILED
thread 'play::fork_hooks::native_tests::player_numeric_hp_uses_native_gauge_byte_not_exact_stat_ratio' (34416) panicked at crates\eqoa\src\fork_ext\api_capture_tests.rs:1650:9:
128/255 rounds to 50%, not stat ratio 90%
FAILED
test result: FAILED. 0 passed; 7 failed; 0 ignored; 0 measured; 1996 filtered out; finished in 1.21s
```

## negative-default-tot

```text
running 2 tests
thread 'fork_ext::addons::round_3_picture_tests::fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring' (38692) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:604:69:
called `Result::unwrap()` on an `Err` value: RuntimeError("[string \"Forge spike\"]:1: assertion failed!\nstack traceback:\n\t[C]: in ?\n\t[C]: in function 'assert'\n\t[string \"Forge spike\"]:1: in main chunk")
FAILED
thread 'fork_ext::addons::round_3_picture_tests::tot_uses_targets_channels_and_rejects_self_and_nameless_and_honors_mana' (38108) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:663:9:
invalid ToT 2/"Luck"
FAILED
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 1982 filtered out; finished in 0.71s
```

## multibox-red

```text
running 1 test
thread 'settings::fork_settings::tests::hud_migration_rejects_invalid_values_and_reloads_secondary_windows_in_real_lua' (17468) panicked at crates\eqoa\src\fork_ext\settings.rs:650:13:
assertion `left == right` failed
  left: [98, 114, 105, 103, 104, 116, 110, 101, 115, 115, 61, 57, 10, 115, 116, 97, 116, 115, 95, 98, 111, 120, 61, 111, 110, 10, 116, 97, 114, 103, 101, 116, 95, 111, 102, 95, 116, 97, 114, 103, 101, 116, 61, 111, 110, 10, 109, 97, 110, 97, 95, 98, 97, 114, 115, 61, 111, 102, 102, 10, 104, 117, 100, 95, 110, 117, 109, 98, 101, 114, 115, 61, 111, 102, 102, 10, 120, 112, 95, 100, 101, 116, 97, 105, 108, 115, 61, 111, 102, 102, 10, 116, 97, 114, 103, 101, 116, 95, 104, 112, 95, 112, 101, 114, 99, 101, 110, 116, 61, 111, 102, 102, 10]
 right: [98, 114, 105, 103, 104, 116, 110, 101, 115, 115, 61, 57, 10]
FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2002 filtered out; finished in 0.84s
```

## live-order-red

```text
running 1 test
thread 'viewer::fork_report::addon_order_tests::live_redraw_prepares_mods_before_native_hud_and_submits_after_resolve' (32416) panicked at crates\eqoa\src\fork_ext\report_viewer.rs:196:9:
Lua preparation must precede native HUD consumption; overlay submits after scene resolve
test viewer::fork_report::addon_order_tests::live_redraw_prepares_mods_before_native_hud_and_submits_after_resolve ... FAILED

failures:

failures:
    viewer::fork_report::addon_order_tests::live_redraw_prepares_mods_before_native_hud_and_submits_after_resolve

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2001 filtered out; finished in 0.04s
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2001 filtered out; finished in 0.04s

```

## stats-cache-red

```text
running 1 test
thread 'fork_ext::addons::round_3_picture_tests::stats_box_updates_cached_rows_when_only_native_stat_bonuses_change' (35112) panicked at crates\eqoa\src\fork_ext\addons\round_3_picture_tests.rs:1156:5:
stat-only change must invalidate cached drawing
FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1997 filtered out; finished in 0.21s
```

## pet-red

```text
running 1 test
thread 'play::fork_hooks::native_tests::live_pet_layout_follows_group_raid_and_native_effect_strip' (7928) panicked at crates\eqoa\src\fork_ext\api_capture_tests.rs:1601:5:
assertion `left == right` failed: 48px native offset plus 20px effects row
  left: 66
 right: 68
FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2000 filtered out; finished in 0.01s
```

## packaging-allowlist-red

```text
E
======================================================================
ERROR: test_exact_player_archive_and_hash_bound_update_assets (tools.test_player_release.PlayerReleaseTests.test_exact_player_archive_and_hash_bound_update_assets)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "C:\Users\hocka\.paseo\worktrees\1usktejc\eqoa-api-r3\tools\test_player_release.py", line 203, in test_exact_player_archive_and_hash_bound_update_assets
    self.build()
    ~~~~~~~~~~^^
  File "C:\Users\hocka\.paseo\worktrees\1usktejc\eqoa-api-r3\tools\test_player_release.py", line 200, in build
    return self.packager.build_release(**options)
           ~~~~~~~~~~~~~~~~~~~~~~~~~~~^^^^^^^^^^^
  File "C:\Users\hocka\.paseo\worktrees\1usktejc\eqoa-api-r3\tools\build_player_release.py", line 318, in build_release
    expected_mod_hashes = load_bundled_mod_sources(bundled_dir)
  File "C:\Users\hocka\.paseo\worktrees\1usktejc\eqoa-api-r3\tools\build_player_release.py", line 161, in load_bundled_mod_sources
    raise ValueError("SOURCE.txt file allowlist mismatch with expected bundled mods")
ValueError: SOURCE.txt file allowlist mismatch with expected bundled mods

----------------------------------------------------------------------
Ran 1 test in 1.570s

FAILED (errors=1)
```

## packaging-checksum-red

```text
F
======================================================================
FAIL: test_tampered_bundled_mod_file_fails (tools.test_player_release.PlayerReleaseTests.test_tampered_bundled_mod_file_fails)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "C:\Users\hocka\.paseo\worktrees\1usktejc\eqoa-api-r3\tools\test_player_release.py", line 416, in test_tampered_bundled_mod_file_fails
    with self.assertRaisesRegex(ValueError, "checksum|tampered|mismatch"):
         ~~~~~~~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AssertionError: ValueError not raised

----------------------------------------------------------------------
Ran 1 test in 2.164s

FAILED (failures=1)
```
