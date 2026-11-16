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

---

## Part B: Action Broker, Actions, Capabilities, and Bag Viewer Menu

### Changes by module
- `crates/eqoa/src/fork_ext/addons/gesture.rs` (new): Single-owner `GestureToken`, `GestureScope`, 3-source gesture arming (`ModKey`, `PointerClaim`, `ImGuiClick`), one-action and one-batch token consumption, and synthetic `/bug` / `#bug` report guard (`assert_not_report_command` blocking `/bug` and `#bug` even inside wrapper commands like `/say` or `/tell`).
- `crates/eqoa/src/fork_ext/addons/action_broker.rs` (new): 30 Hz `VmBroker` (`MIN_TICK_MS = 33`), rate limiter (per-VM bucket: max 6/s, burst 12), non-item actions (`cast`, `attack`, `assist`, `hail`, `target_id`, `clear_target`, `cycle_target`, `sit`, `camp`, `consider`, `command`, `autorun`, `follow`, `stop`, `open_window`, `weapon_set`, `prepare_weapon_set`, `assign_ability`, `unmemorize_ability`, `unmemorize_all_abilities`, `arrange_ability`), item/transaction actions (`use_item`, `equip`, `swap_items`, `quick_slot`, `loot`, `loot_all`, `buy`, `sell`, `bank_move`, `bank_money`), capability descriptors, and `round6b_*` tests.
- `crates/eqoa/src/fork_ext/addons/native_events.rs`, `native.rs`, `native_core.rs`, `native_items.rs`, `native_state.rs`, `pointer_api.rs`, `bridge.rs`, `bridge_widgets.rs`, `chat.rs`, `mod.rs`: Gesture scope entry at input/pointer/ImGui widget call sites, `action.result` event delivery (`RequestId`), stale handle resolution, and report-command filtering.
- `crates/eqoa/src/fork_ext/play.rs` & `crates/eqoa/src/fork_ext/items_tests.rs`: Native dispatch & server reply matching (`BrokerReply`), per-subsystem busy guards, zone/disable cancellation without replay, Forge transaction wiring (`frontiers_forge`), and `Play::bags_*` helpers routing events through `api_capture`.
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

Command run after final code/test/capture commits (`8ab6319e`):
```text
python tools/run_gates.py --filter round6b_ round6c_ --release --lock-wait 120
```
Result: **ALL GATES PASSED**
- **round**: PASS (6.37s) — `16 passed, 0 failed, 0 ignored`
- **addons**: PASS (319.20s) — `482 passed, 0 failed, 25 ignored` (9 skips: 4 symlink OS privilege `os error 1314`, 2 `EQOA_FORGE_SPIKE`, 3 `EQOA_BUNDLED_UI_GOLDENS`)
- **full**: PASS (1625.56s) — `2334 passed, 0 failed, 30 ignored` (30 distinct skips: missing local `run/client.bin`, `EQOA_2002_ISO`, live S2/savestate assets, symlink privilege, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`)
- **wasm**: PASS (2.96s)
- **footprint**: PASS (1.67s)
- **paths**: PASS (5.33s)
- **player-release**: PASS (45.14s)
- **release**: PASS (128.99s) — `C:\Users\hocka\Paseo\targets\shared\release\eqoa.exe`

Also verified directly:
- `python tools/update_bundled_manifest.py` + `python tools/test_player_release.py` -> `20 tests ... OK`
- `python tools/check_fork_footprint.py --base a76d8d27a1c3f51fca545d7eabaca9e5dc336091` -> `Changed files: 492 fork-owned, 34 shared; PASS: fork footprint is within reviewed policy` (no budget increases; zero upstream lines added).

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
- `eqoa.actions.camera`: **`not_yet`** (round 4, `"requires &mut Walker hook in Play::frame (proposed in round-6b-seams.md)"`). See [round-6b-seams.md](round-6b-seams.md) for the exact 1-line upstream `Play::frame` hook proposal (`crates/eqoa/src/play.rs:3326`).
