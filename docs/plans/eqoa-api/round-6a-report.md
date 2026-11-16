# Round 6 Report: The Read API + Approved Seams (Part A, 2026-11-16)

Branch: `feat/eqoa-api-r6` from `develop` (`83ef435f`), reviewed footprint base `a76d8d27a1c3f51fca545d7eabaca9e5dc336091`. Commits are LoopRook's; nothing was pushed, merged, or tagged.

## Commits

| Commit | Summary |
|---|---|
| `d623b83a` | `feat(eqoa-api): implement quests and class mastery read API (round 6a step 1)` |
| `09831145` | `docs(eqoa-api): propose minimal read-only round 6 seams` |
| `61b23e29` | `feat(eqoa-api): implement camera read API and tick-coalesced camera.changed (round 6a step 2)` |
| `a9f06c6e` | `feat(eqoa-api): implement windows read API, hints helpers, and windows.changed (round 6a step 3)` |
| `4b12a877` | `feat(eqoa-api): implement Forge CMs/icons/hints, HotbarSlot.item, and camera/window helpers (round 6a steps 5-6)` |
| `61e796ea` | `feat(eqoa-api): add Round 6 Part A tests and getting-started examples` |
| `dffb0d1a` | `docs(eqoa-api): add Round 6 Part A completion report` |
| `bc14ca34` | `feat(eqoa-api): apply lead-approved read-only seams and wire window rows/dialogs` |

## What Was Built (Read-Only Observations, Seams & Events)

| Where | What |
|---|---|
| `crates/eqoa/src/fork_ext/r6a_capture.rs` & `crates/eqoa/src/fork_ext/addons/native_r6a.rs` | Native modules `eqoa.quests` (`list`, `get`), `eqoa.mastery` (`get`, `skills`, `selected`), `eqoa.camera` (`get`, `compass`), and `eqoa.windows` (`get`, `find`, `get_window`, `format_hint`, `parse_hint`). Events `quests.changed`, `mastery.changed`, `camera.changed`, and `windows.changed`. Exact handle lifetimes: `QuestRef` tracks 1-based journal revisions and turns `stale_handle` on zoning/removal; `WindowRef` invalidates on zone/reset or when a window closes/reopens (`incarnation`). Wired `Window.rows`, `selected_row`, `scroll_top`, `tabs`, `tab`, `MenuRow.*` (`index`, `text`, `enabled`, `item`, `command`), and `Dialog` (`text`, `options`, `selection`) for `text_dialog` (PrivChat), `response_dialog` (visible 1-based page positions including `More`/`Back`/`Close`), and `confirm` (`yes_no` `sel + 1`, defaulting to No = `2`). |
| `crates/eqoa/src/fork_ext/addons/native_core.rs`, `native.rs`, `native_events.rs`, `api_capture.rs`, `api_updates.rs` | `MasteryFlags` per-counter initialization tracking (`WorldEntry` vs `0x1402` sub-ops 1–10 so uninitialized counters stay `nil`), 30 Hz camera tick coalescing (`camera_tick` + `pending_camera_before` + `flush_camera_motion`), `eqoa.player.Player.mastery`, `eqoa.hotbar.HotbarSlot.item` (`resolve_hotbar_items`), `MenuRow.item` resolution inside `assign_window_refs`, and capability descriptors (`supported` / `not_yet`). |
| `crates/eqoa/src/fork_ext/addons/icons.rs`, `visibility.rs` | `eqoa.icons.class(class_id)` and `eqoa.icons.disposition(disposition)` returning GPU texture handles (`32x44` UI atlas textures) plus `frontiers_forge.ui.{GetClassIconTexture,GetDispositionIconTexture,DrawClassIcon,DrawDispositionIcon}`. |
| `crates/eqoa/src/fork_ext/addons/player.rs`, `fork_ext/play.rs` | `frontiers_forge.player.{GetCMs,GetCMsSpent,GetCMPct}` driven from live `CORE.mastery_flags` / snapshot (`nil` until initialized). |
| `crates/eqoa/src/fork_ext/addons/items_forge.rs`, `items_capture.rs` | `eqoa.merchant.MerchantState.button_hint` (`|t<Exit>`) and `frontiers_forge.merchant.GetButtonHint()`, upgrading capability status to `supported`. |
| `crates/eqoa/src/fork_ext/addons/host.rs`, `native.rs` | `frontiers_forge.camera.{GetCoordinates,GetFacingRadians,GetFacingDegrees}`, `frontiers_forge.window.{FormatButtonHint,ParseButtonHint,IsTextDialogOpen}`, and `frontiers_forge.util.{GetCompassRadians,GetCompassDegrees}` matching pinned Forge reference semantics. |
| `docs/modding/GETTING-STARTED.md` | Updated merchant button hint note and added examples for `eqoa.quests`, `eqoa.windows`, `eqoa.camera`, and `eqoa.mastery`. |

## Approved Upstream Seams Applied & Footprint Before/After

Lead-approved (owner-delegated 2026-11-16) read-only `&self` accessors only; **zero field visibility changes** and zero gameplay/behavior changes (`NpcWindows.loot`, `BankPlay.wnd`, and `SpellBook.top` remain private). Documented in [round-6-seams.md](round-6-seams.md) and budgeted in `docs/fork-footprint.json`:

| File | Accessor(s) Added | Before (+ / -) | After (+ / -) |
|---|---|---:|---:|
| `crates/eqoa/src/dialog.rs` | `PrivChat::read_text`, `Response::read_state`, `MBox::sel` | 0 / 0 | 9 / 0 |
| `crates/eqoa/src/npcw.rs` | `NpcWindows::loot_lists` | 1 / 1 | 5 / 1 |
| `crates/eqoa/src/loot.rs` | `Loot::read_list` | 0 / 0 | 3 / 0 |
| `crates/eqoa/src/bankplay.rs` | `BankPlay::read_lists` | 0 / 0 | 4 / 0 |
| `crates/eqoa/src/bank.rs` | `Bank::read_lists` | 0 / 0 | 3 / 0 |
| `crates/eqoa/src/questlog.rs` | `QuestLog::read_state` | 0 / 0 | 3 / 0 |
| `crates/eqoa/src/startmenu.rs` | `MainMenu::read_state` | 1 / 1 | 7 / 1 |
| `crates/eqoa/src/spellbook.rs` | `SpellBook::top` | 0 / 0 | 3 / 0 |
| `crates/eqoa/src/trainer.rs` | `Trainer::read_state` | 0 / 0 | 3 / 0 |
| `crates/eqoa/src/classmastery.rs` | `ClassMastery::read_state` | 0 / 0 | 4 / 0 |
| **Total (10 seam files)** | **+42 / -0 newly added** | **2 / 2** | **44 / 2** |

Workspace footprint: `486 fork-owned, 42 shared` (`PASS: fork footprint is within reviewed policy`).

## Excluded or Deferred Items

- **`eqoa.textures.formats.jpeg`**: stays `not_yet` (no JPEG decoder crate exists in workspace `Cargo.toml`/`Cargo.lock`; only `png` and `bcdec_rs` are present).
- **Write/transaction calls**: deferred to Round 6 Part B / after-PR backlog per the Round 6 scope decision (`83ef435f`).

## Modified Existing Tests

1. `crates/eqoa/src/fork_ext/addons/native_tests.rs` (`native_capabilities_are_probeable_and_stubs_diagnose_even_when_caught`, commit `61b23e29`): updated `eqoa.camera.get` expectation from `not_yet` to `supported` and switched the stub probe/diagnostic check to `eqoa.actions.cast` (`not_yet`).
2. `crates/eqoa/src/fork_ext/addons/player.rs` (`forge_player_populated_stats`, commit `4b12a877`): test fixture populates `cms: 15`, `cms_spent: 10`, `cm_pct: 75`; updated assertions from `nil` to `15`, `10`, and `75`.
3. `crates/eqoa/src/fork_ext/items_tests.rs` (`round5_forge_item_modules_follow_the_pinned_shapes`, commit `4b12a877`): updated `eqoa.capabilities.get('frontiers_forge.merchant.GetButtonHint').status` expectation from `not_yet` to `supported`.
4. `crates/eqoa/src/fork_ext/r6a_tests.rs` (`round6a_windows_modes_dialogs_hints_and_outside_world`, `round6a_forge_icons_merchant_hint_hotbar_items_and_zero_addon`, commit `bc14ca34`): updated pre-seam `nil`/`not_yet` assertions for `text_dialog.dialog`, `confirm.yes_no.selection` (defaults to No = `2`), `Window.rows`, `Window.selected_row`, `Window.scroll_top`, `MenuRow.command`, `Dialog.options`, and `Dialog.selection` after wiring the approved seams.

## Tests & Executed Negative Controls

All 8 focused tests in `crates/eqoa/src/fork_ext/r6a_tests.rs` pass (plus `9/9` in `addons::visibility::` and `21/21` `round5_` regression checks):

| Test (`play::fork_hooks::api_capture::r6a_tests::...`) | Coverage |
|---|---|
| `round6a_quests_dump_add_remove_stale_and_zoning` | Dump quests, `0x007C` add, `0x007D` remove, `quests.changed`, `QuestRef` `stale_handle` after removal & zoning |
| `round6a_mastery_uninitialized_dump_and_reply_subops` | Uninitialized nil fields, `0x1402` sub-ops 1/3/4/5/8/9, `WorldEntry` zero-init, `experience_share` scaling, Forge `GetCMs*` |
| `round6a_camera_third_and_first_person_and_tick_coalescing` | Third/first-person camera, compass & `ee::libm::atan2f`, 30 Hz `camera.changed` tick coalescing, Forge `camera.*` & `util.GetCompass*` |
| `round6a_windows_modes_dialogs_hints_and_outside_world` | §7 mode table (`normal`, `none`, unknown -> `other`), `EditBox` (`password` redaction), `MBox`, `WindowRef` invalidation, hints roundtrip, quiet frames |
| `round6a_forge_icons_merchant_hint_hotbar_items_and_zero_addon` | GPU `eqoa.icons.class/disposition`, Forge UI icon textures/draw, `merchant.GetButtonHint`, `HotbarSlot.item` + compaction/stale ref, capability statuses, zero-addon zero-work check |
| `round6a_seam_dialogs_privchat_response_pages_and_confirm_yes_no` | `PrivChat` (`text_dialog`), 2-page 14-option `Response` dialog (`More`/`Back`/`Close` and visible `selected_row` index vs native `sel`), `MBox` `yes_no` No (`2`) -> Left -> Yes (`1`) |
| `round6a_seam_loot_bank_questlog_and_start_menu_rows` | `Loot` (`0x0018`, Leave All / corpse `ItemRef` rows / Take All / `selected_row` / `scroll_top`), `Bank` (`0x124D`, focus on bank items + `ItemRef`), `QuestLog` rows/selection/scroll, `StartMenu` entries + dim `enabled=false` + commands |
| `round6a_seam_trainer_mastery_and_spellbook_rows` | `Trainer` stats/`Train`/`Details` selection across modes `0..=2`, `ClassMastery` tabs/columns/unknown `???` vs owned skill + `selected_row`/`scroll_top`, `SpellBook` rows/tabs/selection/scroll |

Each test had an executed negative control (implementation temporarily broken, test run, failure recorded, clean code restored):

| Test | Command | Injected Fault | Observed Failure |
|---|---|---|---|
| `round6a_quests_...` | `cargo test -p eqoa round6a_quests_dump_add_remove_stale_and_zoning` | `quests_json` returning `Vec::new()` | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:132:6: ... RuntimeError("[string \"Forge spike\"]:3: assertion failed!")` |
| `round6a_mastery_...` | `cargo test -p eqoa round6a_mastery_uninitialized_dump_and_reply_subops` | Unscaled `experience_share` (`m.x08 as f32`) | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:275:6: ... RuntimeError("[string \"Forge spike\"]:4: assertion failed!")` |
| `round6a_windows_...` | `cargo test -p eqoa round6a_windows_modes_dialogs_hints_and_outside_world` | Omitting `!play.edit.password` redaction | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:659:6: ... RuntimeError("[string \"Forge spike\"]:3: assertion failed!")` |
| `round6a_camera_...` | `cargo test -p eqoa round6a_camera_third_and_first_person_and_tick_coalescing` | Bypassing `self.camera_tick != self.tick` guard | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:502:10: ... RuntimeError("[string \"Forge spike\"]:1: same tick emits once immediately; second render buffers")` |
| `round6a_forge_...` | `cargo test -p eqoa round6a_forge_icons_merchant_hint_hotbar_items_and_zero_addon` | Early return in `resolve_hotbar_items` | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:809:6: ... RuntimeError("[string \"Forge spike\"]:12: assertion failed!")` |
| `round6a_seam_dialogs_...` | `cargo test -p eqoa round6a_seam_dialogs_privchat_response_pages_and_confirm_yes_no` | Naive `sel + 1` instead of visible page position in `Response` | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:924:6: ... RuntimeError("[string \"Forge spike\"]:8: assertion failed!")` |
| `round6a_seam_loot_...` | `cargo test -p eqoa round6a_seam_loot_bank_questlog_and_start_menu_rows` | Dropping `NpcWindows::loot_lists()` selection/scroll to `(None, None)` | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:1010:6: ... RuntimeError("[string \"Forge spike\"]:8: loot sel=nil top=nil")` |
| `round6a_seam_trainer_...` | `cargo test -p eqoa round6a_seam_trainer_mastery_and_spellbook_rows` | Dropping `cat` from `ClassMastery::read_state()` (`tab = None`) | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:1212:6: ... RuntimeError("[string \"Forge spike\"]:3: tab=nil ntabs=5 ...")` |

## Workspace Gates (`tools/run_gates.py --filter round6a_ --release`)

Executed with `EQOA_ISO` pointing to `run/eqoa.iso` after applying seams (`bc14ca34`):

- **round**: PASS (`111.15s`) — `8 passed; 0 failed; 0 ignored`
- **addons**: PASS (`308.89s`) — `471 passed; 0 failed; 25 ignored` (9 skip notes: 4 symlink-privilege skips on Windows, 2 GPU spike tests without `EQOA_FORGE_SPIKE`, 3 bundled UI golden comparisons without `EQOA_BUNDLED_UI_GOLDENS`)
- **full**: PASS (`1592.23s`) — `2326 passed; 0 failed; 30 ignored` (30 distinct skips for missing optional external fixtures: `run/client.bin`, symlink privileges, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`, `EQOA_2002_ISO`, live save state / S2 patches)
- **wasm**: PASS (`8.03s`)
- **footprint**: PASS (`1.64s`, `486 fork-owned, 42 shared`)
- **paths**: PASS (`5.00s` — re-verified clean after removing the local target path from this report)
- **player-release**: PASS (`47.13s`)
- **release**: PASS (`269.25s`, release `eqoa.exe` built)
