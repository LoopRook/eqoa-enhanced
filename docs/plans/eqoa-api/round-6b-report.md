# Round 6B/C Report: Game Actions Broker, Batches, and Bag Viewer (Parts B & C)

Branch: `feat/eqoa-api-r6b` (from `develop` `b8d5fbdd`), reviewed footprint base `a76d8d27a1c3f51fca545d7eabaca9e5dc336091`.
Commits on branch before this report:
- `cebd9371`: `feat(actions): add gesture-gated 30 Hz action broker and lifecycle`
- `23ee8428`: `feat(actions): implement non-item game actions and camera seam proposal`
- `ea8174c9`: `feat(actions): implement item and merchant/bank/loot transaction actions`
- `4a7c43b6`: `feat(bag_viewer, addons): add Part B item actions menu and captures`
- `864dd664`: `feat(actions, docs): verify Part B capabilities, GETTING-STARTED guide, and full gate pass`
- `844c266d`: `feat(actions): add gesture-gated broker batches and spec section 8`
- `8ab6319e`: `feat(bag_viewer): add Part C junk batches, persistence, tests, and captures`
- `e4290af6`: `fix(actions): bind gesture token to physical event across subscribers (H1)`
- `16ef9e54`: `fix(actions): route transactions and camera through approved native hooks (H2)`
- `746359a9`: `fix(actions): require exact reply and state change before confirming tx (H3)`
- `2ac99654`: `fix(addons): enter ImGui render gesture scope in frame-mode and Forge callbacks (M4)`
- `547259a8`: `test(actions): restore dispatch, request-kind, and 32-tick drain assertions (Fix 5)`
- `39d9b85f`: `docs(eqoa-api): record approved and applied round 6 seams`

---

## Part B: Action Broker, Actions, Capabilities, and Bag Viewer Menu

### Changes by module
- `crates/eqoa/src/fork_ext/addons/gesture.rs` (new): Single-owner `GestureToken`, `GestureScope`, 3-source gesture arming (`ModKey`, `PointerClaim`, `ImGuiClick`), one-action and one-batch token consumption, and synthetic `/bug` / `#bug` report guard (`assert_not_report_command` blocking `/bug` and `#bug` even inside wrapper commands like `/say` or `/tell`).
- `crates/eqoa/src/fork_ext/addons/action_broker.rs` (new): 30 Hz `VmBroker` (`MIN_TICK_MS = 33`), rate limiter (per-VM bucket: max 6/s, burst 12), non-item actions (`cast`, `attack`, `assist`, `hail`, `target_id`, `clear_target`, `cycle_target`, `sit`, `camp`, `consider`, `command`, `autorun`, `follow`, `stop`, `open_window`, `weapon_set`, `prepare_weapon_set`, `assign_ability`, `unmemorize_ability`, `unmemorize_all_abilities`, `arrange_ability`), item/transaction actions (`use_item`, `equip`, `swap_items`, `quick_slot`, `loot`, `loot_all`, `buy`, `sell`, `bank_move`, `bank_money`), capability descriptors, and `round6b_*` tests.
- `crates/eqoa/src/fork_ext/addons/native_events.rs`, `native.rs`, `native_core.rs`, `native_items.rs`, `native_state.rs`, `pointer_api.rs`, `bridge.rs`, `bridge_widgets.rs`, `chat.rs`, `mod.rs`: Gesture scope entry at input/pointer/ImGui widget call sites, `action.result` event delivery (`RequestId`), stale handle resolution, and report-command filtering.
- `crates/eqoa/src/fork_ext/play.rs` & `crates/eqoa/src/fork_ext/items_tests.rs`: Native dispatch & server reply matching (`BrokerReply`), per-subsystem busy guards, zone/disable cancellation without replay, and `Play::bags_*` helpers routing events through `api_capture` (`items_forge.rs` left unchanged because pinned Forge modules contain no transaction functions).
- `crates/eqoa/src/fork_ext/addons/native_tests.rs`: Restored `bindpage::register("eqoa_stats_box", ...)` after `settings::testing::reset(false)` in `failed_option_saves_leave_lua_and_controls_values_unchanged_and_diagnose`.
- `bundled-mods/eqoa_bag_viewer/{mod.json, main.lua, modules/bags.lua}` & `bundled-mods/SOURCE.txt`: Contextual `Corpse` tab (tab 7), item click popup menu (`Use`, `Equip`/`Unequip`, `Quick slot 1..4`, `Sell 1`/`Sell stack`, `Deposit`/`Withdraw`, `Take`/`Take all`), greyed refusal reasons, result line under Tunar/used, no Destroy entry, zero unclicked actions, kept `"enabled": false`.
- `crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs`: Added `round6b_bag_viewer_*` GPU/Play tests and captures (`docs/plans/eqoa-api/r6-bags-{menu,merchant,bank-menu,corpse}.png`).
- `docs/modding/GETTING-STARTED.md`: Added `## Actions` button/keybind example.

### Gesture design (three physical sources)
Each dispatch enters a synchronous thread-local `GestureScope` bound to `(vm, addon_id)` and restores any enclosing scope on `Drop`, so gestures never leak across VMs, timers, events, or coroutines:
1. **`ModKey` (`eqoa.input.bind`):** `native_events.rs` arms an active `ModKey` scope only when an `input.key` event has `down == true`, `repeat == false`, and `synthetic == false`, delivering only to the addon owning that Controls-page bind.
2. **`PointerClaim` (`pointer.pressed`):** `pointer_api.rs` / `native_events.rs` verifies the event is a physical mouse press inside the VM's currently claimed `eqoa.input.claim_pointer` rectangle and enters an armed `PointerClaim` scope for that VM's listener only.
3. **`ImGuiClick` (`eqoa.imgui` / Forge UI widgets):** `bridge_widgets.rs` / `native_events.rs` wraps `render` and `settings.render` callbacks in an initially unarmed `enter_render_scope`. Clicking/activating an interactive widget calls `gesture::arm_imgui_click()` synchronously for that frame's callback.

Calling `take_single_token` succeeds once per scope (`consumed_single = true`), so a second action in the same callback fails with `needs_player_action`.

---

## Part C: Broker Batches and Bag Viewer Junk Batches

### Changes by module
- `crates/eqoa/src/fork_ext/addons/gesture.rs`: Exposed `take_batch_token`, allowing one gesture to authorize at most one `eqoa.actions.batch(...)` call (`consumed_batch = true`).
- `crates/eqoa/src/fork_ext/addons/action_broker.rs`: Implemented `MAX_BATCH_STEPS = 64`, `BatchId`, `BatchWindow` (`Merchant | Bank | Loot`, automatically inferred from step kinds when omitted), serial execution in `step_tick` (dispatches at most 1 batch step per VM per 30 Hz tick and blocks on `waiting_req_id` until server confirmation/refusal or timeout), automatic abort on any client/server refusal, window close, zoning (`cancel_all_batches("zone")`), mod disable, rate limit, or explicit `eqoa.actions.stop_batch(id?)` (no replay), plus `eqoa.actions.batch_status(id)` and unified `status`/`cancel` support for `BatchId` and `RequestId`. Added `round6c_batch_*` tests.
- `crates/eqoa/src/fork_ext/addons/native.rs` & `native_events.rs`: Capability entries and event delivery for `batch.progress` and `batch.result` (`e.data.{batch_id, state, ok, completed, total, current_step, code, reason, queued_tick, completed_tick, failed_step, detail}`).
- `docs/modding/EQOA-API-SPEC.md`: Documented queued batches in §8 (`Queued batches (Part C)`).
- `bundled-mods/eqoa_bag_viewer/{mod.json, main.lua, modules/bags.lua}` & `bundled-mods/SOURCE.txt`:
  - Added `"eqoa.files.read"` and `"eqoa.files.write"` capabilities (still `"enabled": false`).
  - Added `"Mark as junk"` / `"Unmark"` item menu entries (disabled on equipped items), per-item-type `junk_ids` persistence in `"junk.cfg"` (`eqoa.files.read` / `eqoa.files.write`), and top-right amber `"J"` stack badges on junk stacks.
  - Added `[ ] Also sell grey items` toggle (off by default, `sell_grey = false`, persisted alongside `junk_ids`), excluding equipped, equippable, quick-slotted, usable, charged, and stat-bearing items.
  - Added Merchant footer `"Sell junk (N items, X tunar)"` (using native `eqoa.merchant.price(ref, "sell", qty)` quotes), batch progress (`"Selling junk i of N..."`), terminal summaries (`"Sold X of Y: <reason>"`), Corpse `"Take all but junk"` batch button, and live `"Stop"` button while any batch runs. Zero actions occur without a click.
- `crates/eqoa/src/fork_ext/items_tests.rs` & `crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs`: Added `bags_loot_confirm_one`, `bags_fill_inventory_to`, `round6c_bag_viewer_*` GPU/Play tests, and captures `docs/plans/eqoa-api/r6-junk-{badges,merchant,stopped,corpse}.png`.

---

## Gate Summary

Command run after the fix round (`39d9b85f`):
```text
python tools/run_gates.py --release --lock-wait 120
```
Result: **ALL GATES PASSED**
- **addons**: PASS (318.49s) — `484 passed, 0 failed, 25 ignored` (9 skips: 4 symlink OS privilege `os error 1314`, 2 `EQOA_FORGE_SPIKE`, 3 `EQOA_BUNDLED_UI_GOLDENS`)
- **full**: PASS (1634.40s) — `2338 passed, 0 failed, 30 ignored` (30 distinct skips: missing local `run/client.bin`, `EQOA_2002_ISO`, live S2/savestate assets, symlink privilege, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`)
- **wasm**: PASS (5.19s)
- **footprint**: PASS (2.26s)
- **paths**: PASS (4.99s)
- **player-release**: PASS (44.08s)
- **release**: PASS (125.57s) — `<CARGO_TARGET_DIR>/release/eqoa.exe`

Also verified directly:
- `python tools/update_bundled_manifest.py` + `python tools/test_player_release.py` -> `20 tests ... OK`
- `python tools/check_fork_footprint.py --base a76d8d27a1c3f51fca545d7eabaca9e5dc336091` -> `Changed files: 494 fork-owned, 35 shared; PASS: fork footprint is within reviewed policy` (with three lead-approved 1-line upstream hook calls).

---

## Negative Controls (Break-and-Revert Verified)

1. **Gesture gate bypass (Part B, Step 1/5):**
   Temporarily bypassed the gesture check inside `crates/eqoa/src/fork_ext/addons/gesture.rs::take_single_token`.
   - Command: `cargo test --locked -p eqoa --bin eqoa -- round6b_broker_gesture_gate_enforces_three_sources_and_one_action_per_gesture --test-threads=1 --nocapture`
   - Failure observed: `FAILED` (`panicked at crates\eqoa\src\fork_ext\addons\action_broker.rs:2248:10` / `2249:9`), reverted immediately.
2. **Bag Viewer Destroy entry guard (Part B, Step 4/5):**
   Temporarily injected a `"Destroy"` menu row in an external copy of `bundled-mods/eqoa_bag_viewer` pointed to by `EQOA_R5B_TEST_SOURCE`.
   - Command: `cargo test --locked -p eqoa --bin eqoa -- round6b_bag_viewer_bags_menu_actions_greyed_reasons_no_destroy_and_no_unclicked_actions --test-threads=1 --nocapture`
   - Failure observed: `FAILED` (`panicked at crates\eqoa\src\fork_ext\addons\bag_viewer_tests.rs:818:5: must have no Destroy entry`), scratch copy removed.
3. **Broker second-batch busy guard (Part C, Step 6):**
   Temporarily disabled `v.active_batch.is_some()` check in `crates/eqoa/src/fork_ext/addons/action_broker.rs` line 636.
   - Command: `python tools/run_gates.py --only round --filter round6c_ --lock-wait 120`
   - Failure observed: `FAIL` on `fork_ext::addons::action_broker::tests::round6c_batch_gesture_single_batch_refuses_second_serial_execution_and_capabilities` (`err2.code == 'busy'` check at `action_broker.rs:3887`), reverted immediately and re-passed (`2 passed, 0 failed`).
4. **Bag Viewer grey-items default off guard (Part C, Step 7):**
   Temporarily flipped `sell_grey = false` to `sell_grey = true` in an external copy of `bundled-mods/eqoa_bag_viewer` via `EQOA_R5B_TEST_SOURCE`.
   - Command: `python tools/run_gates.py --only round --filter round6c_bag_viewer_ --lock-wait 120`
   - Failure observed: `FAIL` (`panicked at crates\eqoa\src\fork_ext\addons\bag_viewer_tests.rs:1021:5: assertion failed: !bv.eval::<bool>("require('bags').sell_grey")` and `crates\eqoa\src\fork_ext\addons\bag_viewer_tests.rs:1121:5: Sell junk (4 items, 110 tunar)`), scratch copy removed.

---

## Unsupported, `not_yet`, and Upstream Seam Proposals

- `eqoa.actions.destroy`: **`unsupported`** (`"not offered before the PR (owner)"`) — intentionally omitted from API and Bag Viewer.
- `eqoa.actions.move`, `eqoa.actions.look`, `eqoa.actions.buttons`: **`not_yet`** (round 4, `"continuous controls come after the PR"`).
- `eqoa.actions.drop_quest`, `mastery_share`, `mastery_browse`, `mastery_buy`, `mastery_refund`, `close_window`, `select_row`, `activate_row`, `dialog_answer`, `input_text`, `menu_command`: **`not_yet`** (round 4, `"arrives with the window/quest read API"` — Part A handle follow-up after merge).
- `eqoa.actions.camera`: Now **`supported`** via the approved 1-line `Play::frame` hook (`fork_hooks::apply_camera(self, walker)` at `crates/eqoa/src/play.rs:3326`). See [round-6b-seams.md](round-6b-seams.md) for all three approved and applied seams.

---

## Fix round (2026-11-16)

All findings in [round-6-review-bc.md](round-6-review-bc.md) were resolved and verified with dedicated tests and negative controls:

1. **H1 — One gesture per physical event (`e4290af6`):**
   - Moved `GestureScope` entry in `crates/eqoa/src/fork_ext/addons/native_events.rs::deliver_data` outside the subscriber loop, added `arm_mod_key` (`gesture.rs`) for `eqoa.input.bind` wrappers (`native_state.rs`), and consumed tokens on accepted enqueue (`action_broker.rs`).
   - Test: `action_broker::tests::round6b_one_gesture_per_physical_event_across_multiple_listeners` (two `pointer.pressed` listeners + two `input.bind` listeners; one press grants one action, second returns `needs_player_action`).
   - Negative control: re-introduced per-listener `PointerClaim` scope in `native_events.rs` -> `python tools/run_gates.py --only round --filter round6b_one_gesture_per_physical_event --lock-wait 120` failed at `crates\eqoa\src\fork_ext\addons\action_broker.rs:3527:10`.

2. **H2 — Transactions through native consumers and `apply_camera` (`16ef9e54`):**
   - Removed all copied opcode builders (`0x0019`, `0x001A`, `0x004A`, `0x004B`, `0x1254`, `0x1255`) and direct `shop.pending`/`bank.state` writes from `crates/eqoa/src/fork_ext/play.rs`.
   - Added the three lead-approved 1-line upstream hook calls:
     - `crates/eqoa/src/npcw.rs`: `super::fork_hooks::npcw_requests(self, &mut loot_out, &mut merch_out);` (+1 line; total shared diff `+2 -1`).
     - `crates/eqoa/src/bankplay.rs`: `super::fork_hooks::bank_requests(self, &mut out);` (+1 line; total shared diff `+1 -0`).
     - `crates/eqoa/src/play.rs`: `fork_hooks::apply_camera(self, walker);` (+1 line; total shared diff `+94 -51`).
   - Native consumer loops remain gated by their open window checks (`mode == 1/10/11`); closed windows refuse with `window_closed`. Implemented `eqoa.actions.camera` through `WalkCamera` / `camstyle` setters and clamps.
   - Test: `items_tests::round6b_transactions_route_through_native_consumers_and_camera_hook`.
   - Negative control: skipped pushing `MerchantOut::Sell` in `fork_ext/play.rs::npcw_requests` -> `python tools/run_gates.py --only round --filter round6b_transactions_route_through_native_consumers_and_camera_hook --lock-wait 120` failed at `crates\eqoa\src\fork_ext\items_tests.rs:2044:5` (`npcw.rs loop sent the sell packet`).

3. **H3 — No false confirmations (`746359a9`):**
   - Correlated server confirmations against exact item identity and quantity changes (`LootOne { key, qty_before, count }`, `MerchantBuy { item_id, qty_before, count }`, `MerchantSell { key, qty_before, count }`, `BankMove`, `BankMoney`), treated `0x0016` (`LootClose`), closed windows while in-flight, and unconfirmed pending clears as `unknown` (`ok = nil`) instead of `confirmed`, stopping batches (`"merchant did not confirm"`, `"bank did not confirm"`, `"corpse closed"`).
   - Fixed `items_tests::bags_bank_withdraw_confirm` to encode `0x1250` slot `u8` matching `bankplay.rs`.
   - Test: `items_tests::round6c_no_false_confirmations_unrelated_invadd_or_corpse_close`.
   - Negative control: temporarily resolved `LootClose` as `Some(true)` -> `python tools/run_gates.py --only round --filter round6c_no_false_confirmations_unrelated_invadd_or_corpse_close --lock-wait 120` failed at `crates\eqoa\src\fork_ext\items_tests.rs:2271:6`.

4. **M4 — Frame-mode and Forge widget gesture scope (`2ac99654`):**
   - Added `gesture::enter_render_scope` in `host.rs::frame_entry` when `!once` and in `manager.rs::Manager::invoke` when `rendering`.
   - Test: `manager::tests::round6b_frame_mode_and_forge_widget_click_grants_one_gesture_via_manager` (real `Manager::frame` + `Gpu` button click).
   - Negative control: replaced `_gesture` with `None` in `host.rs::frame_entry` -> `python tools/run_gates.py --only round --filter round6b_frame_mode_and_forge --lock-wait 120` failed at `crates\eqoa\src\fork_ext\addons\manager.rs:2646:9` (`c_code=Some("needs_player_action")`).

5. **Restored weakened tests (`547259a8`):**
   - `native_tests::round_3_stage_a_actions_assist_and_summary_api` verifies accepted assist reaches broker dispatch.
   - `round_3_picture_tests::assert_mod_click` verifies `queued_kinds_for_test() == ["assist"]`.
   - `api_capture_tests::addon_actions_use_live_camera_assist_and_summary_overlay_gates` drains all 32 queued assists across ticks (`8 + 24*1 == 32`, zero extra sends).
   - Negative control: set `TOKEN_BUCKET_REFILL_PER_TICK = 0` -> `python tools/run_gates.py --only round --filter addon_actions_use_live_camera --lock-wait 120` failed at `crates\eqoa\src\fork_ext\api_capture_tests.rs:1830:9`.

6. **Documentation / scope correction (`39d9b85f` + this report):**
   - Removed the inaccurate Forge transaction wiring claim (`items_forge.rs` is unchanged vs `develop` `b8d5fbdd`), updated [round-6b-seams.md](round-6b-seams.md), and updated `docs/fork-footprint.json` with exact approved counts (`494` fork-owned, `35` shared, `PASS`).

---

## Fix round 2 (2026-11-16)

Resolved the remaining contention and reply-correlation findings from [round-6-recheck-bc.md](round-6-recheck-bc.md) with zero new upstream lines:

1. **No silent drop on contention + `Staged` / `native_sent` lifecycle (`9e3e7b36`):**
   - Added `DispatchOutcome::Staged` and `InFlightAction::native_sent` in `crates/eqoa/src/fork_ext/addons/action_broker.rs` so staged consumer transactions (`loot`, `loot_all`, `buy`, `sell`, `bank_move`, `bank_money`, `camera`) remain queued without emitting `"dispatched"` until `mark_deferred_sent` runs upon actual push to `_loot_out`, `_merch_out`, `_out`, or `apply_camera` application.
   - Refactored `crates/eqoa/src/fork_ext/play.rs::npcw_requests` and `bank_requests` to inspect `PENDING_NPCW_TX` and `PENDING_BANK_TX` before `.take()`ing; when native outputs are non-empty or busy (`shop.pending`, `bank.state != 0`, `LOOT_BUSY`), staged requests stay staged to retry on the next tick, or expire with `"refused"` (`code = "busy"`) if continuously contended past `RATE_LIMIT_WAIT_TICKS` (30 ticks).
   - Verified targeted gate: `python tools/run_gates.py --only round,wasm,footprint --filter round6b_ round6c_ --lock-wait 120` -> `round` `20 passed, 0 failed`, `wasm` `PASS`, `footprint` `PASS`.

2. **Strict correlation of sent requests & frame contention guard (`79c2f465`):**
   - Routed all server reply resolvers (`resolve_server_*`, including `resolve_server_d7`) through `resolve_matching_sent` so unsent (`!native_sent`) requests can never match or confirm on any server packet, and flagged frames where native output vectors were already populated (`mark_subsystem_contended` / `clear_frame_contention`) so any reply in a contended frame resolves the sent in-flight action as `"unknown"` (`ok = nil`).
   - Updated `crates/eqoa/src/fork_ext/play.rs` (`begin_game_event` / `end_game_event`) and `action_broker.rs`:
     - Bank deposit (`0x1251`) and withdraw (`0x1250`) confirm only when both item identity and moved quantity match (`added_item == Some((item_id, count))`, `withdrawn == Some((key, item_id, count))` and `bank_qty == qty_before - count`), else `"unknown"`.
     - `LootOne` (`0x0019`) confirms only when corpse stack drops by exact requested count (`after == 0` if `count >= qty_before` else `after == qty_before - count`), else `"unknown"`.
     - `LootAll` (`0x001A`) captures `corpse_before` and confirms only when `corpse_after != corpse_before` (state changed), else `"unknown"`, stopping active batches.
   - Updated `round6b_merchant_buy_sell_bank_move_bank_money_and_loot` bank deposit (`0x1251` qty 1) and loot (`0x0019` 5-byte payload, qty 1) test fixtures to match exact requested counts.
   - Verified targeted gate: `python tools/run_gates.py --only round,wasm,footprint --filter round6b_ round6c_ --lock-wait 120` -> `round` `20 passed, 0 failed`, `wasm` `PASS`, `footprint` `PASS`.

3. **Regression tests & break-and-revert negative controls (`f73d2499`):**
   - Added `items_tests::round6c_contention_retains_staged_deposit_and_never_confirms_on_player_reply`: verifies player `BankOut::Move` contention retains staged broker deposit, prevents player `0x1251` from confirming it, retries and sends once idle, treats mismatched deposit quantity (`2` vs `1`) as `"unknown"` stopping the batch, resolves `"unknown"` on concurrent native contention against an already-sent request, and refuses cleanly with `code == 'busy'` after `RATE_LIMIT_WAIT_TICKS`.
     - Negative control: temporarily switched `PENDING_BANK_TX.with(|b| *b.borrow())` to `.take()` before `!_out.is_empty()` in `fork_ext/play.rs::bank_requests` -> `cargo test --locked -p eqoa --bin eqoa -- round6c_contention_retains_staged_deposit_and_never_confirms_on_player_reply --test-threads=1 --nocapture` failed at `crates\eqoa\src\fork_ext\items_tests.rs:2354:5` (`retained staged deposit must send once contention clears`), then reverted.
   - Added `items_tests::round6c_loot_partial_decrease_and_noop_loot_all_yield_unknown_and_stop_batch`: verifies `LootOne` partial stack decrease (`4 -> 3` for `count = 3`) and no-op `LootAll` (`0x001A` with full pack) both resolve `"unknown"` (`ok == nil`) and stop the batch before step 2 sends.
     - Negative control: temporarily weakened `resolve_server_loot_one` in `action_broker.rs` back to `after <= qty_before.saturating_sub(count) || after < qty_before` -> `cargo test --locked -p eqoa --bin eqoa -- round6c_loot_partial_decrease_and_noop_loot_all_yield_unknown_and_stop_batch --test-threads=1 --nocapture` failed at `crates\eqoa\src\fork_ext\items_tests.rs:2480:6` (`assertion failed` on `state == 'unknown'`), then reverted.
   - Re-verified targeted gate after reverting both negative controls: `round` `22 passed, 0 failed`, `wasm` `PASS`, `footprint` `PASS`.

4. **Final release gates (`python tools/run_gates.py --release --lock-wait 120`):**
   - `addons`: PASS (`484 passed, 0 failed, 25 ignored`; 9 prerequisite skips)
   - `full`: PASS (`2340 passed, 0 failed, 30 ignored`; 30 distinct prerequisite skips)
   - `wasm`: PASS
   - `footprint`: PASS
   - `paths`: PASS
   - `player-release`: PASS
   - `release`: PASS
