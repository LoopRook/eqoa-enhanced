# Round 6 Report: The Read API (Part A, 2026-11-16)

Branch: `feat/eqoa-api-r6` from `develop` (`83ef435f`), reviewed footprint base `a76d8d27a1c3f51fca545d7eabaca9e5dc336091`. Commits are LoopRook's; nothing was pushed, merged, or tagged, and zero upstream files were modified.

## Commits

| Commit | Summary |
|---|---|
| `d623b83a` | `feat(eqoa-api): implement quests and class mastery read API (round 6a step 1)` |
| `09831145` | `docs(eqoa-api): propose minimal read-only round 6 seams` |
| `61b23e29` | `feat(eqoa-api): implement camera read API and tick-coalesced camera.changed (round 6a step 2)` |
| `a9f06c6e` | `feat(eqoa-api): implement windows read API, hints helpers, and windows.changed (round 6a step 3)` |
| `4b12a877` | `feat(eqoa-api): implement Forge CMs/icons/hints, HotbarSlot.item, and camera/window helpers (round 6a steps 5-6)` |
| `61e796ea` | `feat(eqoa-api): add Round 6 Part A tests and getting-started examples` |

## What Was Built (Read-Only Observations & Events)

| Where | What |
|---|---|
| `crates/eqoa/src/fork_ext/r6a_capture.rs` (new) & `crates/eqoa/src/fork_ext/addons/native_r6a.rs` (new) | Native modules `eqoa.quests` (`list`, `get`), `eqoa.mastery` (`get`, `skills`, `selected`), `eqoa.camera` (`get`, `compass`), and `eqoa.windows` (`get`, `find`, `get_window`, `format_hint`, `parse_hint`). Events `quests.changed`, `mastery.changed`, `camera.changed`, and `windows.changed`. Exact handle lifetimes: `QuestRef` tracks 1-based journal revisions and turns `stale_handle` on zoning/removal; `WindowRef` invalidates on zone/reset or when a window closes/reopens (`incarnation`). |
| `crates/eqoa/src/fork_ext/addons/native_core.rs`, `native.rs`, `native_events.rs`, `api_capture.rs`, `api_updates.rs` | `MasteryFlags` per-counter initialization tracking (`WorldEntry` vs `0x1402` sub-ops 1–10 so uninitialized counters stay `nil`), 30 Hz camera tick coalescing (`camera_tick` + `pending_camera_before` + `flush_camera_motion`), `eqoa.player.Player.mastery`, `eqoa.hotbar.HotbarSlot.item` item-ref resolution (`resolve_hotbar_items`), and capability descriptors (`supported` / `not_yet`). |
| `crates/eqoa/src/fork_ext/addons/icons.rs`, `visibility.rs` | `eqoa.icons.class(class_id)` and `eqoa.icons.disposition(disposition)` returning GPU texture handles (`32x44` UI atlas textures) plus `frontiers_forge.ui.{GetClassIconTexture,GetDispositionIconTexture,DrawClassIcon,DrawDispositionIcon}`. |
| `crates/eqoa/src/fork_ext/addons/player.rs`, `fork_ext/play.rs` | `frontiers_forge.player.{GetCMs,GetCMsSpent,GetCMPct}` driven from live `CORE.mastery_flags` / snapshot (`nil` until initialized). |
| `crates/eqoa/src/fork_ext/addons/items_forge.rs`, `items_capture.rs` | `eqoa.merchant.MerchantState.button_hint` (`|t<Exit>`) and `frontiers_forge.merchant.GetButtonHint()`, upgrading capability status to `supported`. |
| `crates/eqoa/src/fork_ext/addons/host.rs`, `native.rs` | `frontiers_forge.camera.{GetCoordinates,GetFacingRadians,GetFacingDegrees}`, `frontiers_forge.window.{FormatButtonHint,ParseButtonHint,IsTextDialogOpen}`, and `frontiers_forge.util.{GetCompassRadians,GetCompassDegrees}` matching pinned Forge reference semantics. |
| `docs/modding/GETTING-STARTED.md` | Updated merchant button hint note and added examples for `eqoa.quests`, `eqoa.windows`, `eqoa.camera`, and `eqoa.mastery`. |

## Upstream Seams Proposed (Not Applied)

No upstream files were touched (`python tools/check_fork_footprint.py --base a76d8d27a1c3f51fca545d7eabaca9e5dc336091` passes within all existing budgets). [round-6-seams.md](round-6-seams.md) proposes **+33 / -3 lines** of read-only accessors across 10 upstream files (`dialog.rs`, `npcw.rs`, `loot.rs`, `bankplay.rs`, `bank.rs`, `questlog.rs`, `startmenu.rs`, `spellbook.rs`, `trainer.rs`, `classmastery.rs`) to expose window rows/selections/tabs and `PrivChat`/`Response`/`MBox.sel` dialog fields.

## Excluded or Deferred Items

- **`eqoa.textures.formats.jpeg`**: stays `not_yet` (no JPEG decoder crate exists in workspace `Cargo.toml`/`Cargo.lock`; only `png` and `bcdec_rs` are present).
- **Seam-blocked `eqoa.windows` fields**: `Window.rows`, `Window.selected_row`, `Window.tabs`, `Window.tab`, and full `text_dialog` / `response_dialog` / `confirm.selection` dialog contents stay `nil` with capability status `not_yet` and reason `"needs an upstream read accessor the owner approves first"` until [round-6-seams.md](round-6-seams.md) is approved.
- **Write/transaction calls**: deferred to Round 6 Part B / after-PR backlog per the Round 6 scope decision (`83ef435f`).

## Modified Existing Tests

Three existing fork tests were updated because previously `not_yet` APIs are now `supported`:

1. `crates/eqoa/src/fork_ext/addons/native_tests.rs` (`native_capabilities_are_probeable_and_stubs_diagnose_even_when_caught`, commit `61b23e29`): updated `eqoa.camera.get` expectation from `not_yet` to `supported` and switched the stub probe/diagnostic check to `eqoa.actions.cast` (which remains `not_yet`).
2. `crates/eqoa/src/fork_ext/addons/player.rs` (`forge_player_populated_stats`, commit `4b12a877`): test fixture populates `cms: 15`, `cms_spent: 10`, `cm_pct: 75`; updated assertions from `nil` to `15`, `10`, and `75`.
3. `crates/eqoa/src/fork_ext/items_tests.rs` (`round5_forge_item_modules_follow_the_pinned_shapes`, commit `4b12a877`): updated `eqoa.capabilities.get('frontiers_forge.merchant.GetButtonHint').status` expectation from `not_yet` to `supported`.

## Tests & Executed Negative Controls

All 5 focused tests in `crates/eqoa/src/fork_ext/r6a_tests.rs` pass (plus `9/9` in `addons::visibility::` and `21/21` `round5_` regression checks):

| Test (`play::fork_hooks::api_capture::r6a_tests::...`) | Coverage |
|---|---|
| `round6a_quests_dump_add_remove_stale_and_zoning` | Dump quests, `0x007C` add, `0x007D` remove, `quests.changed`, `QuestRef` `stale_handle` after removal & zoning |
| `round6a_mastery_uninitialized_dump_and_reply_subops` | Uninitialized nil fields, `0x1402` sub-ops 1/3/4/5/8/9, `WorldEntry` zero-init, `experience_share` scaling, Forge `GetCMs*` |
| `round6a_camera_third_and_first_person_and_tick_coalescing` | Third/first-person camera, compass & `ee::libm::atan2f`, 30 Hz `camera.changed` tick coalescing, Forge `camera.*` & `util.GetCompass*` |
| `round6a_windows_modes_dialogs_hints_and_outside_world` | §7 mode table (`normal`, `none`, unknown -> `other`), `EditBox` (`password` redaction), `MBox`, `WindowRef` invalidation, hints roundtrip, quiet frames |
| `round6a_forge_icons_merchant_hint_hotbar_items_and_zero_addon` | GPU `eqoa.icons.class/disposition`, Forge UI icon textures/draw, `merchant.GetButtonHint`, `HotbarSlot.item` + compaction/stale ref, capability statuses, zero-addon zero-work check |

Each covered area had a real negative control executed by temporarily breaking the implementation, running the targeted test, observing failure, and restoring the clean code:

| Test | Command | Injected Fault | Observed Failure |
|---|---|---|---|
| `round6a_quests_...` | `cargo test -p eqoa round6a_quests_dump_add_remove_stale_and_zoning` | `quests_json` in `r6a_capture.rs` returning `Vec::new()` | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:132:6: ... RuntimeError("[string \"Forge spike\"]:3: assertion failed!")` (`#list == 1`) |
| `round6a_mastery_...` | `cargo test -p eqoa round6a_mastery_uninitialized_dump_and_reply_subops` | Unscaled `experience_share` (`m.x08 as f32`) in `mastery_json` (`r6a_capture.rs`) | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:275:6: ... RuntimeError("[string \"Forge spike\"]:4: assertion failed!")` (`m.experience_share - 0.4`) |
| `round6a_windows_...` | `cargo test -p eqoa round6a_windows_modes_dialogs_hints_and_outside_world` | Omitting `!play.edit.password` redaction in `r6a_capture.rs` | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:659:6: ... RuntimeError("[string \"Forge spike\"]:3: assertion failed!")` (`inp.dialog.text == nil`) |
| `round6a_camera_...` | `cargo test -p eqoa round6a_camera_third_and_first_person_and_tick_coalescing` | Bypassing `self.camera_tick != self.tick` check in `native_core.rs` | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:502:10: ... RuntimeError("[string \"Forge spike\"]:1: same tick emits once immediately; second render buffers")` |
| `round6a_forge_...` | `cargo test -p eqoa round6a_forge_icons_merchant_hint_hotbar_items_and_zero_addon` | Early return in `resolve_hotbar_items` (`native_core.rs`) | `panicked at crates\eqoa\src\fork_ext\r6a_tests.rs:809:6: ... RuntimeError("[string \"Forge spike\"]:12: assertion failed!")` (`slots[1].item == inv.items[1].ref`) |

## Workspace Gates (`tools/run_gates.py --filter round6a_ --release`)

Executed with `EQOA_ISO` pointing to `run/eqoa.iso`:

- **round**: PASS (`284.54s`) — `5 passed; 0 failed; 0 ignored`
- **addons**: PASS (`303.10s`) — `471 passed; 0 failed; 25 ignored` (9 skip notes: 4 symlink-privilege skips on Windows, `EQOA_FORGE_SPIKE` missing for 2 GPU spike tests, 3 bundled UI golden comparisons skipped without `EQOA_BUNDLED_UI_GOLDENS`)
- **full**: PASS (`1606.01s`) — `2323 passed; 0 failed; 30 ignored` (30 distinct skips for missing optional external fixtures: `run/client.bin`, symlink privileges, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`, `EQOA_2002_ISO`, live save state / S2 patches)
- **wasm**: PASS (`27.23s`)
- **footprint**: PASS (`1.63s`, `485 fork-owned, 34 shared`, within budget)
- **paths**: PASS (`4.81s`)
- **player-release**: PASS (`38.56s`)
- **release**: PASS (`284.86s`, built `C:\Users\hocka\Paseo\targets\shared\release\eqoa.exe`)
