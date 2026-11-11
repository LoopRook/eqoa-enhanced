# Round 3 independent check 2

**Verdict: DISCREPANCIES FOUND.** The claimed proof of native rendering parity is not independent, several named proving tests do not exist, and upstream tests have changed. A passing test count does not establish the claimed fixes.

## Gates

All Cargo commands use `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, a fresh scratch `EQOA_CONFIG`, the requested lock and per-command timeout. Results below are independent reruns; pending means no result was available at report time.

| Gate | Independent result | Builder claim |
|---|---|---|
| Addon suite | 352 passed, 0 failed, 24 ignored, 1602 filtered; 9 real SKIP prints (4 symlink privileges, 2 missing Forge prerequisites, 3 missing reviewed goldens) | 345 passed, 24 ignored, 0 SKIP (earlier report) |
| Full eqoa binary | 1947 passed, 0 failed, 31 ignored; 203 real SKIP prints | Not reported |
| net | 87 passed across targets, 0 failed, 0 ignored, 0 SKIP prints | Not reported |
| tools unittest | 97 run, OK, 0 skips | 97 passed |
| Release packaging | Attempt with pre-existing shared-target executable failed: does not embed current source commit; fresh CI build pending | Bundles all four mods |
| Optimized zero-addon CI gate | 1 passed, 0 SKIP; 0.0 allocs/frame; command and target match CI, but `RUSTFLAGS=-C target-feature=+crt-static` was inadvertently omitted, so not an exact CI reproduction | 1 passed, 0.0 allocations/frame |
| Release benchmark | 1 passed, 0 SKIP; 0.125 ms/frame, but two default-off features are not enabled (see below) | 0.140 ms/frame |
| wasm check | PASS, 0 errors, 0 warnings | 0 warnings, 0 errors |
| Footprint with reviewed base 5d54c0c | PASS, 378 fork-owned, 42 shared, all hooks intact | PASS, 369 fork-owned |
| merge-tree HEAD 5d54c0c | Clean, tree `8a40eb3d4e1fec5fac1ce1556df84539b3f6af2c` | Clean |

## HIGH finding proving tests

Paths below are relative to `crates/eqoa/src/fork_ext/`. “No” means the named test does not establish the complete finding, not necessarily that the implementation is still broken.

| Finding | Assessment and key evidence |
|---|---|
| Review H1 | **No, partial.** `settings.rs:398-403` asserts `get_toggle("eqoa_target_of_target", "target_of_target") == Some(true)` and :470-483 simulates a restart and asserts `register_toggle(...).0 == true`; it exercises write refusal too, but does not prove live Lua menu wiring. Named `addon_options_persists_to_mod_settings_across_reloads` does not exist in the tree. |
| H2 | **No.** Named `disabled_mod_removes_options_from_enhanced_hud_category` does not exist; no assertion to quote. |
| H3 | **No.** `addons/round_3_picture_tests.rs:396` explicitly sets Stats Box true; :417-424 sets it false then asserts blank. The ToT test similarly explicitly sets true at :461 and false at :494-499. Neither asserts fresh defaults. |
| H4 | **No.** The named path is wrong: test is `addons/native_tests.rs:694`, not `bindpage::tests`; :704 asserts `dispatch_keypress("z") == Some(("mod_test", "custom_key"))`. It does not exercise a live event, Stats Box Lua, default B, input suppression, or actual toggle. |
| H5 | **No, partial.** `addons/round_3_picture_tests.rs:412` asserts `assert_pixels_match(...)` for 4:3 and 16:9, but it bypasses the live frame ordering and has no tall viewport. The drawing is a reconstructed reference, not develop execution. |
| H6 | **No.** Benchmark asserts `per_frame_millis < 0.5` at `addons/round_3_picture_tests.rs:740-743`; it neither asserts an ImGui window scope nor hitboxes or window metrics. |
| H7 | **No.** Named `test_group_snapshot_filters_local_player_and_inactive_slots` is absent; the mana picture test at `addons/round_3_picture_tests.rs:564` asserts matching pixels from a hand-built group, not production capture filtering or raid visibility. |
| H8 | **No.** ToT picture assertion at `addons/round_3_picture_tests.rs:489` exercises a named, non-self target with mana enabled (:453-461), not self-targets, empty names, or mana disabled. |
| H9 | **No.** Named `test_hud_visible_in_play_and_hidden_in_menus` is absent; no assertion to quote. |
| H10 | **No.** `addons/round_3_picture_tests.rs:412` compares a reconstructed drawing using synthetic assets (:62-85), not localized labels or develop's real render. |
| H11 | **No.** Named namespace is wrong; actual `addons/wereoxxs_test.rs:765` asserts Forge modules are tables (:800-802), not the new EQOA capability/spec/strict-record contract. It can skip without GPU (:774-776). |
| H12 | **No.** `addons/round_3_picture_tests.rs:357-359` asserts under 2% whole-frame pixels exceed channel tolerance 8, against same-test reconstruction (:399-403), not a checked-in develop capture. |
| Check H1 | **Yes for tooling fixture.** `tools/test_player_release.py:208` asserts the archive member list equals the expected allowlist; independent tools suite passed 97. |
| Check H2 | **Partial.** `tools/test_player_release.py:217` checks archive members against fixture hashes; rerun: 20 tests OK. It does not necessarily validate the real repository's SOURCE.txt; packaging gate remains decisive. |
| Check H3 | **Yes for count.** `bundled.rs:1240` asserts `ids.len() == 8`; full suite passed. |
| Check H4 | **No.** Same as H12: `addons/round_3_picture_tests.rs:412` compares against a reconstruction, not the committed reference PNG. |
| Check H5 | **Partial.** `settings.rs:382-385` asserts legacy keys survive refused writes, and :398-403 asserts migrated ID/value and :470-483 simulates reload through `register_toggle(...).0 == true`; the named migration test alone does not demonstrate a Lua addon observes restored settings after a process restart. |
| Check H6 | **No.** Same as H3: `addons/round_3_picture_tests.rs:396` sets the option true before rendering, so a default-on regression survives. |

## Other findings

### HIGH: Reference pictures are not captured from develop

Eight PNGs were added in `c5d083a` under `docs/round3-captures`. The test's `save_png` (:183-192) writes them only when `EQOA_R3_CAPTURE` is set; its input is drawn by reference code recreated in the branch (e.g. :283 and :399-403, :464-476). It never reads the PNGs. Assets are a 1x1 white texture and solid white synthetic glyphs (:62-85). This is not a capture of develop's native drawing. Four picture tests skip when no GPU (:371-374, :433-436, :507-510, :582-585); the benchmark also skips (:665-668). The tests do compare pixels at 640x480 and 1280x720, but permit 2% of all screen pixels to differ by over 8 per channel (:355-360), which can hide a substantial fraction of small HUD elements.

### HIGH: Upstream test assertions changed

Comparison to `git show 5d54c0c:<file>` found no missing upstream test function names in changed Rust files, but upstream assertions differ in `crates/eqoa/src/bindpage.rs:645`, `binds.rs:1621`, `hud.rs:994`, and `play.rs:11777`. Examples include F2 changed to F10, hide-UI controller rows changed to map rows, jump-key B changed to Y, and the hidden HUD menu click test changed to a compass/map test. Some precede this branch and reflect fork bindings, but the checker instruction explicitly classifies any upstream test change HIGH; the original coverage has not been demonstrated intact.

### MEDIUM: Benchmark does not enable all four HUD features

The benchmark loads four hosts but never turns Stats Box or Target of Target on; their defaults are off. `addons/round_3_picture_tests.rs:681-685` starts them, while :714-728 times rendering without setting their options. Its result therefore does not establish the cost of all four visible mods.

### MEDIUM: Removed or weakened develop tests

`git diff develop...HEAD` deletes the two fork bind tests in `fork_ext/binds.rs` (F2 fixture and retained-key round trip), three native HUD/ToT tests (`hud.rs` and `play.rs`), six legacy setting persistence/default/invalid-input tests (`fork_ext/settings.rs`), and four stat-box math/layout tests (`statbox.rs`). Removal of native implementations explains the deleted subjects, but the replacement picture and migration tests do not cover all former negative controls, key round trips, geometry, colors, math, invalid values, and restart behavior. `fork_ext/hud.rs` weakens populated fixture and expected quad count from 40/38 to 29/27 because native extras were removed; this no longer checks them. `fork_ext/api_capture_tests.rs:216` replaces the `position==nil` assertion with count/name assertions; the lost position assertion is not justified by the changed roster. `fork_ext/options.rs` replaces legacy number-toggle checks with mod toggle checks; unknown-item negative control was restored. The minimap pick test in `hud.rs` moved intact.

### MEDIUM: Release build warning claim

The independent non-static release build passed but produced 75 compiler warnings, contrary to the report's zero-warning claim. The static-CRT CI rerun and packaging were still running when this report was finalized.

### LOW: footprint and stray output

`target` does not exist in the worktree. The only raised budget versus develop is `bindpage.rs` added 7 to 10 (+3), within the approved +4; its deleted budget falls 7 to 5. The claim that zero budgets were raised is inaccurate.








