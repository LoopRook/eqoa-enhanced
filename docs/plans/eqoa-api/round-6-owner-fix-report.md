# Round 6 Owner-Fix Report (`eqoa_bag_viewer`)

- **Branch**: `feat/eqoa-api-r6` (base `2e4fb5fe`, worktree `eqoa-api-r6`)
- **Author**: `LoopRook`
- **Owner-confirmed intact behavior preserved**: Selling, deposit, withdraw, and Mark as junk remain unchanged in behavior and passing in tests.

## Commits on `feat/eqoa-api-r6`

1. `a979eab1` — `fix(bag_viewer,icons): lazy-load item icons once per icon without native windows` (Item 4)
2. `43eedcb8` — `fix(bag_viewer): track container revision on cached grid and verify batch loot and log` (Item 1 + Items 2/3 verification)
3. `73194a8b` — `fix(bag_viewer): bound window height to HUD frame and scroll large packs` (Item 5)
4. `f5a75d05` — `feat(bag_viewer): add Take all button beside Take all but junk on Corpse footer` (Item 6)
5. `6d87f0ce` — `test(bag_viewer): trigger loot source ask before corpse open in round6_item1`
6. `bcabd928` — `docs(eqoa-api): round 6 owner-fix report`
7. `2eae0689` — `fix(bags,loot): support per-take 0x0016 close and 0x0015 batch reopen`
8. `1cae8b1e` — `docs(eqoa-api): update round 6 report with Sandstorm follow-up`
9. `23864f46` — `fix(broker,loot): require corpse item match for LootAll InvAdd confirmation` (M1)
10. `94dcb4e6` — `fix(loot): require InvAdd or 0x0019/0x001A before logging loot_open disappearance` (M3)
11. `64a01518` — `fix(loot): diff InvAdd-confirmed same-corpse loot_open without 0x0016` (M2)
12. `adb9d596` — `fix(broker,loot): stop corpse reopen immediately on map or camera open` (M4)
13. `ab4413c5` — `fix(bag_viewer): keep Corpse tab active during loot batch and summary` (L1)
14. `effa6edd` — `fix(bag_viewer,icons): retry cooled-down icons on change or slow backoff` (L2)
15. `2422bc87` — `test(bag_viewer): add Sandstorm duplicate-id stacks and partial take coverage` (Test gap)

## Per-Item Root Causes, Fixes, and Verification

### 1. Corpse tab contradiction (`"2/40"` shown vs `"Corpse has no items"`)
- **Root cause**: `M.grid` in `bundled-mods/eqoa_bag_viewer/modules/bags.lua` (`rebuild()`) did not store the container revision (`c.revision`) or `tab_key`, and `draw()` in `bundled-mods/eqoa_bag_viewer/main.lua` only checked `if grid == nil then grid = B.rebuild() end`. If `eqoa.loot.get()` changed to an empty container (`#loot.items == 0`) or a different revision without `M.grid` being cleared first, the Corpse tab kept rendering the stale cached `"2/40"` cells while `M.start_loot_non_junk()` read `eqoa.loot.get()` live and saw `total == 0` (`"Corpse has no items"`).
- **Fix**:
  - Added `active_key_and_container()` and `M.is_grid_stale()` in `bundled-mods/eqoa_bag_viewer/modules/bags.lua`, storing `tab_key` and `rev = c.revision` on `M.grid`.
  - Updated `draw()` in `bundled-mods/eqoa_bag_viewer/main.lua` to check `if grid == nil or B.is_grid_stale() then grid = B.rebuild()`.
  - Updated `M.start_loot_non_junk()` (and `M.start_loot_all()`) to call `M.dirty()` when `total == 0`.
- **Verification**: `round6_item1_stale_corpse_grid_and_loot_batch_and_log` (`crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs`) — failed on base `2e4fb5fe` because `M.grid.rev` was missing; passes after fix, including negative controls for genuine `0/40` empty corpses (`"Corpse has no items"`, `ok = false`) and junk-only corpses (`"Corpse has only junk items"`, `ok = false`).

### 2. `"Take all but junk"` taking one item per click (Sandstorm per-take `0x0016` + `0x0015` reopen)
- **Root cause** (confirmed from owner's live `config/eqoa.log`): Sandstorm never sends `0x0019` (`LootOne` reply) or `0x001A` (`LootAll` reply). Instead, after every single `LootOut::One(key, count)` request, the server sends item confirmation (`0x003B Event::InvAdd` + chat `"You receive <item> from the corpse."`) and immediately sends `0x0016` (0 bytes, `LootClose`). Prior to `2eae0689`, `resolve_server_loot_close()` resolved the in-flight `LootOne` as `state = "unknown"` and `step_tick` stopped the batch on `!win.is_open`, resulting in one item per click. Across corpse reopens (`0x0015` -> new `0x0018`), `native_items::items_session_is_new` also rotates all item handles and wire keys re-index (`0, 1, ...`).
- **Fix** (`0` upstream lines added):
  - `crates/eqoa/src/fork_ext/play.rs` & `crates/eqoa/src/fork_ext/addons/action_broker.rs`:
    - `InvAdd` calls `mark_loot_inv_added(item_id)` to mark matching in-flight `LootOne` / `LootAll` actions `inv_confirmed = true`.
    - `resolve_server_loot_close()` treats `inv_confirmed` on `0x0016` as `ok = Some(true)` (`state = "completed"`), clears `LOOT_BUSY`, and if more `ActionKind::Loot` steps remain in the batch for the targeted corpse (`corpse_target_id`), transitions the batch into `corpse_reopen` instead of aborting on closed window.
    - While `corpse_reopen` is active in `mode == 0` with the same corpse still targeted, `begin_iteration` calls `take_corpse_reopen_ask()` and triggers `_play.attack_press()` (the game's own `0x0015` loot-request path).
    - `resolve_server_loot_open` accepts the smaller `0x0018` for the same corpse (or aborts cleanly on 5-byte empty `is_empty` or corpse mismatch), and `dispatch_broker_action` re-resolves stale step handles by `LootItemIdentity` via `native_items::re_resolve_loot_handle()`.
    - The reopen/batch aborts cleanly on `0x0085` refusal (`resolve_server_loot_blocked`), 5-byte empty `0x0018`, zoning (`reset_session`), targeting another entity (`corpse_target_id`), `SERVER_TIMEOUT_TICKS` (`300` ticks = 10s), or clicking `Stop`.
- **What `"Take all"` (`0x001A`) does on Sandstorm**: `eqoa.actions.loot_all()` sends wire `LootOut::All` (`0x001A`). On servers that answer `0x001A`, `resolve_server_loot_all` confirms all items (`round6_item6_take_all_button_on_corpse_tab`). On a server like Sandstorm that never answers `0x001A`: if the server sends `0x0016` after an `InvAdd`, `resolve_server_loot_close` resolves `ok = Some(true)` (only the confirmed item) or `state = "unknown"`; if the server sends no `0x0016` or `0x001A`, the action times out after `SERVER_TIMEOUT_TICKS` (`300` ticks = 10s), resolves `state = "unknown"` (`"Take all: timed out waiting for server reply"`), and `take_loot_timeout_expired()` clears `LOOT_BUSY` in `play.rs` so looting is not left wedged.
- **Verification**: `round6_sandstorm_loot_log_and_batch_reopen_with_negative_controls` (`crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs`) — fails on `bcabd928` (`assertion 'left == right' failed: Sandstorm take must log on InvAdd + 0x0016: [] (left: 0, right: 1)`) and passes after `2eae0689`, replaying the exact `0x0018 -> One(1,1) -> InvAdd -> chat -> 0x0016 -> 0x0015 -> smaller 0x0018 (key 2 -> 0) -> One(0,1) -> InvAdd -> 0x0016` sequence (`"Looted 2 of 2"`, `ok = true`) plus negative controls (`0x0085`, 5-byte empty/tunar-only, retargeting elsewhere, `Stop`, zoning, and unanswered `0x001A` timeout).

### 3. `"Loot log: Nothing looted yet"` on Sandstorm (no `0x0019` / `0x001A`)
- **Root cause**: `M.log_loot(data)` (`bundled-mods/eqoa_bag_viewer/modules/bags.lua`) logs `loot.changed` events with `cause == "loot_one"` or `"loot_all"`, which previously only came from `0x0019`/`0x001A` opcodes (`crates/eqoa/src/fork_ext/items_capture.rs`). On Sandstorm, each take only produces `InvAdd` + `0x0016` (`"loot_close"`) and/or a smaller `0x0018` (`"loot_open"`), so `M.log_loot` never fired.
- **Fix** (`0` upstream lines added):
  - `crates/eqoa/src/fork_ext/items_capture.rs`, `crates/eqoa/src/fork_ext/addons/native_items.rs`, `crates/eqoa/src/fork_ext/play.rs`:
    - `npcw_requests` records every native or broker `LootOut::One` / `LootOut::All` via `items_capture::note_loot_send()`, capturing corpse entity ID, `before_json`, and requested `(inv_index, item_id, qty_before, count)`.
    - `end_game_event` calls `items_capture::note_loot_inv_add(item_id, qty)` on `InvAdd`.
    - When `0x0016` (`"loot_close"`) arrives with `gained_qty > 0` (or `InvAdd` arrives right after `0x0016`), or when the next same-corpse `0x0018` (`"loot_open"`) lacks that record, `native_items::emit_sandstorm_loot_delta()` emits a single synthetic `loot.changed` (`cause = "loot_one"` or `"loot_all"`) with the removed quantity.
    - `PENDING_TAKE` is cleared by `clear_pending_loot()` whenever `0x0019` (`"loot_one"`), `0x001A` (`"loot_all"`), `0x0085` (`"loot_refused"`), native `LootOut::Close`, or zone reset (`forget()`) occurs, preventing double entries when `0x0019`/`0x001A` arrives, preventing entries on refused takes, and ignoring tunar-only / 5-byte empty corpses.
- **Verification**: `round6_sandstorm_loot_log_and_batch_reopen_with_negative_controls` passes both `InvAdd + 0x0016` batch logging (0 duplicates) and closed-window native UI takes observed via smaller same-corpse `0x0018` (`Gnoll Ear x1, from an undead gnoll`), while `round5b_*` and `round6_item1`/`round6_item6` keep all `0x0019`/`0x001A` tests passing with 0 duplicate entries; negative controls confirm `0x0085` refused takes and 5-byte tunar-only corpses log nothing.

### 4. Icons missing until native inventory/bank windows open (and per-frame resource discipline)
- **Root causes**:
  1. `crates/eqoa/src/fork_ext/items_capture.rs:53` used `u32::try_from(icon).ok()`, dropping negative signed 32-bit ESF `dict_id`s to `None`/`nil` in Lua.
  2. `crates/eqoa/src/fork_ext/addons/icons.rs:696-703` (`LiveLibraryProvider::decode_icon`) returns `None` and queues `PENDING_LOOKUPS` when a surface is not yet preloaded in `SAFE_LIBRARY` (as happens for bank/corpse items or before native inventory/bank prefetch), and `icons::update_live_data` decodes it on the next frame — but `bags.lua` only called `eqoa.icons.resource(...)` once during `M.rebuild()` and cached `texture = nil` permanently unless another change event triggered `M.dirty()`.
  3. `queue_pending_lookup` in `icons.rs` did not filter `FAILED_LOOKUPS`, allowing unresolvable IDs to be re-queued if retried.
  4. Loading all slots up front on a 100-slot pack would exceed `MAX_CACHED_ICONS` (64).
- **Fixes**:
  - `crates/eqoa/src/fork_ext/items_capture.rs:53`: changed to `(icon as i32 != 0 && icon as i32 != -1).then_some(icon as u32)` so negative signed 32-bit `dict_id`s reach Lua as `u32`.
  - `crates/eqoa/src/fork_ext/addons/icons.rs`: filtered `FAILED_LOOKUPS` in `queue_pending_lookup`, and added `RESOURCE_CALL_COUNT` + `resource_call_count()` (reset in `reset_test_counters()`) for non-wasm counting tests.
  - `bundled-mods/eqoa_bag_viewer/modules/bags.lua` & `main.lua`: added `TRIM_OPTS` (single module-level table), `M.icon_req`, `M.has_pending_icons`, `M.sync_visible_icons(first_slot, last_slot)` (holding handles only for visible slots `<= 48`), and `M.resolve_pending_icons(first_slot, last_slot)` invoked from `draw()` only via `elseif B.has_pending_icons` (with `ICON_RETRY_LIMIT = 5`, caching `"ok"`/`"failed"` per icon ID across rebuilds and resetting on `"state.reset"`).
- **Verification**: `round6_item4_lazy_icons_load_without_native_windows_and_zero_per_frame_requests` failed on base `2e4fb5fe` (`negative signed dict_id must reach Lua as u32`) and passes after the fix: positive (`40011`), negative signed (`-40012` -> `4294927284`), and bank-tab (`40013`) icons load lazily via `update_live_data` without opening native UI windows; unresolvable icon (`999999`, negative control) stays `nil` (letter fallback); 10 steady frames record `+0` `resource_call_count()`, `+0` `lookup_attempt_count()`, and `<= 1024` bytes delta in `lua.used_memory()`.

### 5. Window goes off-screen on 100-slot pack
- **Root cause**: `M.MAX_ROWS = 8` in `bags.lua` produced `gh = 302`, making total window height `H = 501` (or `519` with footer row 2), exceeding the `640x448` HUD frame and pushing footer controls off screen, with no scroll support on the item grid.
- **Fix**:
  - Reduced `M.MAX_ROWS` from `8` to `6` (`H = 425`, or `443 <= 448` with `has_ctx_btn`; 40-slot pack stays `rows = 5`), keeping `48` visible slots strictly under `MAX_CACHED_ICONS` (64).
  - Added `M.grid_scroll`, `total_rows`, `M.visible_slot_range()`, and `M.scroll_grid(delta)` in `bags.lua`, plus `grid_rect` wheel handling and a 2px scroll track/thumb indicator in `main.lua`.
- **Verification**: `round6_item5_window_fits_hud_and_scrolls_100_slot_pack` passes: 100-slot pack (`capacity = 100`, 100 items, merchant open) fits `H/k <= 448.0`, keeps all `action_hits` within `[0, dh]`, holds `<= 48` live textures, scrolls with mouse wheel to row 13 (`Pack Item 100`), and clicks scrolled slots accurately. Negative control: 40-slot pack keeps `rows = 5, total_rows = 5` and wheel events keep `grid_scroll = 0`.

### 6. Missing `"Take all"` button on Corpse tab
- **Root cause**: `main.lua` (`bar2_y`) only drew `"Take all but junk"` when `eqoa.loot.get() ~= nil`.
- **Fix**:
  - Added `M.start_loot_all()` in `bundled-mods/eqoa_bag_viewer/modules/bags.lua` calling `eqoa.actions.loot_all()`.
  - Updated `main.lua` (`bar2_y` when `eqoa.loot.get() ~= nil`) to draw `"Take all"` (`w = 88`) and `"Take all but junk"` (`w = bw - 88 - 6`) side-by-side within `W - 2 * PAD`, without increasing `H`.
- **Verification**: `round6_item6_take_all_button_on_corpse_tab` passes: verifies `"Take all"` dispatches `0x001A` `LootAll`, clears all 3 items (including junk `Rat Pelt`), logs all 3 in loot log, and shows `"Take all: ok"`. Negative control: clicking `"Take all"` on an empty `0/40` corpse reports `"Corpse has no items"` (`ok = false`) and sends `0` packets.

## Review Fixes (DeepSeek review of `2e4fb5fe..1cae8b1e`)

- **M1 (`action_broker.rs`, `play.rs`)**: Changed `ServerMatcher::LootAll` to `ServerMatcher::LootAll { item_ids: Vec<i64> }` populated from the corpse's non-zero stacks at stage time (`play.rs`); `mark_loot_inv_added()` now confirms `LootAll` only if `item_ids.contains(&added_item_id)`. Verified by `round6_m1_loot_all_ignores_unrelated_inv_add_before_close` (failed before `23864f46` with `"Take all: ok"`; passes after, reporting `"Take all: corpse closed"` for unrelated `InvAdd` and `"Take all: ok"` for a matching corpse item).
- **M2 (`items_capture.rs`)**: Removed `!pending.closed` guard in `capture("loot_open")` so a pending take whose `0x0016` never arrived is still diffed on a subsequent same-corpse `0x0018` (with the M3 `InvAdd` rule) rather than discarded. Verified by `round6_m2_loot_open_without_close_diffs_with_inv_add` (failed before `64a01518` with `left: 0, right: 1`; passes after, plus negative control with no `InvAdd`).
- **M3 (`items_capture.rs`)**: Never-wrong pickup logging rule in `capture("loot_open")`: skips any pending entry with `entry.gained_qty <= 0` and bounds corpse diff by `entry.gained_qty`. A mere disappearance from the next `0x0018` without `InvAdd` (or `0x0019`/`0x001A`) logs nothing. Verified in `round6_sandstorm_loot_log_and_batch_reopen_with_negative_controls` (failed before `94dcb4e6` logging `"Cracked Skull x1"`; passes after).
- **M4 (`play.rs`, `action_broker.rs`)**: Added `abort_corpse_reopen(code, reason)` called in `begin_iteration()` before reopen `0x0015` when `_play.map.open || _play.mode == 38` (`"stopped: map or camera open"`) or `_play.mode != 0 && _play.mode != 1` (`"stopped: non-play mode active"`). Verified by `round6_m4_reopen_stops_immediately_on_map_or_camera_open` (failed before `adb9d596` by sending `0x0015` while `map.open`; passes after for map open, camera mode 38, and `mode == 0` positive control). Also fixed `resolve_server_loot_close` / `resolve_server_loot_open` to refresh `CURRENT_CORPSE_TARGET` so consecutive corpse switches never retain a prior `batch.corpse_target_id`.
- **L1 (`bags.lua`, `main.lua`)**: Added `M.corpse_tab_kept`, `keeping_corpse_tab()`, `EMPTY_CORPSE` (`revision = -1`), and `M.clear_corpse_hold()` so `active_key_and_container()` and `visible_tabs()` keep tab 7 (`"Corpse"`) active across per-take `0x0016 -> 0x0015 -> 0x0018` gaps and after a loot batch finishes to display its summary (`"Looted N of N"`), clearing cleanly on manual tab change, viewer close, or `state.reset`. Verified by `round6_l1_loot_batch_keeps_corpse_tab_across_close_and_summary` (failed before `ab4413c5` dropping to `tab = 1`; passes after, with non-batch corpse close negative control still resetting to `tab = 1`).
- **L2 (`bags.lua`)**: Replaced permanent `"failed"` string with negative backoff timestamp `-(os.clock() + ICON_RETRY_COOLDOWN_S)` (`10s`) cleared on `M.dirty()` (next items/state change) or in `sync_visible_icons()` after cooldown expiry, never on steady frames (`has_pending_icons` remains `false`). Verified by `round6_l2_failed_icon_retries_after_items_change_and_not_per_frame` (failed before `effa6edd`; passes after with `+0` steady-frame resource requests).
- **Test gap (`bag_viewer_tests.rs`)**: Added `round6_sandstorm_duplicate_item_id_stacks_and_partial_take` covering a `"Take 1"` partial stack on `Bone Chip x4 -> x3`, followed by `"Take all but junk"` across two non-junk stacks sharing the same `item_id` (`Spider Silk` qty 3 and qty 5) plus a junk item (`Rat Pelt`) and the remaining partial `Bone Chip x3` across Sandstorm `0x0016`/`0x0015`/shifted-key `0x0018`, with junk-only negative control.

## Validation Summary

- `python tools/update_bundled_manifest.py --check`: `Bundled mod manifests are up to date.`
- `git diff --name-only 2e4fb5fe..HEAD`: touches only `bundled-mods/SOURCE.txt`, `bundled-mods/eqoa_bag_viewer/main.lua`, `bundled-mods/eqoa_bag_viewer/modules/bags.lua`, `crates/eqoa/src/fork_ext/addons/action_broker.rs`, `crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs`, `crates/eqoa/src/fork_ext/addons/icons.rs`, `crates/eqoa/src/fork_ext/addons/native_items.rs`, `crates/eqoa/src/fork_ext/items_capture.rs`, `crates/eqoa/src/fork_ext/items_tests.rs`, `crates/eqoa/src/fork_ext/play.rs`, and this report (0 upstream lines added).
- Focused tests (all run with `-- --test-threads=1`, `EQOA_ISO=run/eqoa.iso`):
  - `cargo test --locked -p eqoa round6`: `58 passed, 0 failed, 2398 filtered out`.
  - `cargo test --locked -p eqoa bag_viewer`: `4 passed, 0 failed, 2452 filtered out` (plus `round6_m1`, `round6_m2`, `round6_m4`, `round6_l1`, `round6_l2`, `round6_sandstorm_*` in `round6`).
  - `cargo test --locked -p eqoa items_tests::`: `40 passed, 0 failed, 2416 filtered out` (includes all `0x0019`/`0x001A` tests).
- `python tools/run_gates.py` (re-run on commit `2422bc87`): **ALL GATES PASSED**
  - `addons`: **PASS** (`495 passed, 0 failed, 25 ignored`; 16 distinct environment skips: Windows symlink privilege, missing `run/eqoa.iso` in worktree path / real disc art, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`).
  - `full`: **PASS** (`2376 passed, 0 failed, 30 ignored`; 30 distinct skips for missing `run/eqoa.iso` in worktree / `EQOA_2002_ISO`, `run/client.bin`, save state, `extracted-assets/sandstorm-patch-20261015/S2`, `EQOA_FORGE_SPIKE`, Windows symlink privilege).
  - `wasm`: **PASS**
  - `footprint`: **PASS**
  - `paths`: **PASS**
  - `player-release`: **PASS**
