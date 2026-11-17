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

## Per-Item Root Causes, Fixes, and Verification

### 1. Corpse tab contradiction (`"2/40"` shown vs `"Corpse has no items"`)
- **Root cause**: `M.grid` in `bundled-mods/eqoa_bag_viewer/modules/bags.lua` (`rebuild()`) did not store the container revision (`c.revision`) or `tab_key`, and `draw()` in `bundled-mods/eqoa_bag_viewer/main.lua` only checked `if grid == nil then grid = B.rebuild() end`. If `eqoa.loot.get()` changed to an empty container (`#loot.items == 0`) or a different revision without `M.grid` being cleared first, the Corpse tab kept rendering the stale cached `"2/40"` cells while `M.start_loot_non_junk()` read `eqoa.loot.get()` live and saw `total == 0` (`"Corpse has no items"`).
- **Fix**:
  - Added `active_key_and_container()` and `M.is_grid_stale()` in `bundled-mods/eqoa_bag_viewer/modules/bags.lua`, storing `tab_key` and `rev = c.revision` on `M.grid`.
  - Updated `draw()` in `bundled-mods/eqoa_bag_viewer/main.lua` to check `if grid == nil or B.is_grid_stale() then grid = B.rebuild()`.
  - Updated `M.start_loot_non_junk()` (and `M.start_loot_all()`) to call `M.dirty()` when `total == 0`.
- **Verification**: `round6_item1_stale_corpse_grid_and_loot_batch_and_log` (`crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs`) — failed on base `2e4fb5fe` because `M.grid.rev` was missing; passes after fix, including negative controls for genuine `0/40` empty corpses (`"Corpse has no items"`, `ok = false`) and junk-only corpses (`"Corpse has only junk items"`, `ok = false`).

### 2. `"Take all but junk"` taking one item per click (live-server reproducibility note)
- **Offline observation**: Both before and after edits, in the offline test harness (`bv.play.bags_loot_confirm_one` injecting opcode `0x0019` followed by `bv.play.bags_broker_tick()`), clicking `"Take all but junk"` on a 3-item corpse (`Rat Pelt` marked junk, `Spider Silk`, `Bone Chip`) submits all non-junk steps in a single `eqoa.actions.batch({steps = steps, require_window = "loot"})`, resolves Step 1 on `0x0019`, and automatically dispatches and completes Step 2 (`"Looted 2 of 2"`, `ok = true`) in one click. Verified in `round6_item1_stale_corpse_grid_and_loot_batch_and_log`.
- **Live-server gap stated plainly**: Reproducing "one item per click" requires observing the live server's packet sequence on loot. In code, `crates/eqoa/src/fork_ext/addons/native_items.rs:140` (`items_session_is_new`) resets all item handle incarnations whenever `0x0018` (`loot_open`) arrives (`stale_handle` on subsequent queued batch steps if a live server re-sends `0x0018` after `LootOut::One`), and `crates/eqoa/src/fork_ext/play.rs:115-124` + `crates/eqoa/src/fork_ext/addons/action_broker.rs:1985-2014` require `0x0019` whose first byte indexes `corpse.items` matching `key` and exact remaining stack count. No artificial passing test was invented for live-server packet ordering not reproducible offline.

### 3. `"Loot log: Nothing looted yet"` after looting by hand and batch
- **Offline observation**: `M.log_loot(data)` (`bundled-mods/eqoa_bag_viewer/modules/bags.lua`) logs `loot.changed` events with `cause == "loot_one"` (`0x0019`) or `cause == "loot_all"` (`0x001A`) when both `data.before` and `data.after` are present (verified in `round6_item1_stale_corpse_grid_and_loot_batch_and_log`, `round6_item6_take_all_button_on_corpse_tab`, and `round5b_bag_viewer_*`).
- **Live-server gap stated plainly**: In `crates/eqoa/src/fork_ext/items_capture.rs:341-373`, only opcodes `0x0019` (`"loot_one"`) and `0x001A` (`"loot_all"`) produce causes logged by `M.log_loot`, and `0x0016` (`"loot_close"`) produces `data.after == nil` (intentionally ignored by `M.log_loot` so `"Leave All"` on an unemptied corpse does not log false pickups, per existing Round 5 tests). Live-server packet traces that omit `0x0019`/`0x001A` or replace corpse state via `0x0018`/`0x0016` cannot be reproduced offline without live captures.

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

## Validation Summary

- `python tools/update_bundled_manifest.py --check`: `Bundled mod manifests are up to date.`
- `git diff --name-only 2e4fb5fe..HEAD`: touches only `bundled-mods/SOURCE.txt`, `bundled-mods/eqoa_bag_viewer/main.lua`, `bundled-mods/eqoa_bag_viewer/modules/bags.lua`, `crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs`, `crates/eqoa/src/fork_ext/addons/icons.rs`, `crates/eqoa/src/fork_ext/items_capture.rs` (0 upstream lines added).
- Focused tests (all run with `-- --test-threads=1`, `EQOA_ISO=run/eqoa.iso`):
  - `cargo test --locked -p eqoa round6_item`: `4 passed, 0 failed` (covered in `cargo test --locked -p eqoa round6`: `51 passed, 0 failed, 2398 filtered out`).
  - `cargo test --locked -p eqoa bag_viewer`: `4 passed, 0 failed` (after setting `bags_press_loot()` before `bags_corpse` in `round6_item1` -> re-verified in `round6` and `run_gates.py`).
  - `cargo test --locked -p eqoa forge_icon`: `23 passed, 0 failed`.
- `python tools/run_gates.py` (default gate suite): **ALL GATES PASSED**
  - `addons`: **PASS** (`488 passed, 0 failed, 25 ignored`; 16 distinct environment skips: Windows symlink privilege, missing `run/eqoa.iso` in worktree path / real disc art, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`).
  - `full`: **PASS** (`2369 passed, 0 failed, 30 ignored`; 30 distinct skips for missing `run/eqoa.iso` in worktree / `EQOA_2002_ISO`, `run/client.bin`, save state, `extracted-assets/sandstorm-patch-20261015/S2`, `EQOA_FORGE_SPIKE`, Windows symlink privilege).
  - `wasm`: **PASS**
  - `footprint`: **PASS**
  - `paths`: **PASS**
  - `player-release`: **PASS**
