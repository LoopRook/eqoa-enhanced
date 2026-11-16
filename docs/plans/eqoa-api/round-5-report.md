# Round 5 report: the items API (Part A, 2026-11-15)

## For the owner: what to test

Package: `Downloads\EQOA-Enhanced-Round5-Test`, start it with "Play EQOA Round5 Test.bat" (your settings copy,
updater off; the "(fps log)" bat also writes the per-mod timing log). Build: commit 7152fba8.

1. In the Addon Manager turn on **EQOA Bag Viewer** (it is off by default). Controls page: give "Bag viewer" a
   key (it starts unbound). Press it: a Bags window opens; Escape or the key closes it.
2. The grid shows your 40 slots with the right icons, stack counts, an E on worn items, tunar and "used/40".
   Try each tab: All, Usable, Equipment, Charged, Misc should match the game's own item page tabs; Bank shows
   your bank and bank tunar anywhere, without visiting a banker.
3. Hover items: name, level, slot, stats, HP, value; a red "You can't use this" on items your class, race or
   level can't use.
4. Loot a few corpses: lines appear under the grid, newest first, with the time. "from <corpse>" shows only
   when you had the corpse targeted a second before looting; clicking a corpse to loot it usually gives no
   "from" (the one-line upstream hook below would fix that).
5. Buy/sell, bank and zone with the window open: it should update, never move or sell anything itself.
   Also check your usual mods (Wereoxxs, the HUD mods, map mods) still work and fps is as before with the
   Bag Viewer off.

Tested by agents: 2312 client tests pass (0 failed; 50 skipped because the decomp oracle is not on this
machine), 35 round 5 tests with protocol-shaped data and negative controls, two Terra reviews (Part A REJECT
then fixed; Part B approved with one fix, done), packaging dry run. Not tested: real play on a server, real
loot/merchant/bank traffic, the look at your screen size; mods' per-frame cost in a busy place.

Your decisions (none block testing): (a) the one upstream line for exact "from <corpse>" (`play.rs`, budget +1,
section "Part B fix round"); (b) the two read-only accessor seams for the item page's tab/selection and the
merchant's mode/selection (section "What is not given", about 11 lines). Both would also go into the PR.

Branch `feat/eqoa-api-r5` from develop `8efdc147` (after sync 2026-11-15), reviewed footprint base
`a76d8d27a1c3f51fca545d7eabaca9e5dc336091`. Builder: Claude Sonnet. Commits are LoopRook's; nothing was pushed, merged
or tagged. Part B (the example mod) is at the end.

## What was built (observations and events only; no transactions)

| Where | What |
|---|---|
| `fork_ext/addons/native_items.rs` (new) | `eqoa.inventory.get`, `eqoa.bank.get`, `eqoa.loot.get`, `eqoa.merchant.get` (one `Container` shape, spec section 6); `eqoa.items.get/can_use/stat/slot_info/attack_name/stat_name`; `eqoa.items.slots/stats/attack_types` (protected maps); `eqoa.merchant.price` (native `merchant_price`/`sell_price` rounding, times the quantity). The core's handle rules: one `ItemRef` per stack, by wire key, kept through compaction, new for a new corpse or merchant session, stale after the stack is gone or the world changed. Container revisions. The four `*.changed` events and their causes. Reads are cached per (generation, revision), so a mod that reads every frame gets the same read-only table until the bag changes. |
| `fork_ext/items_capture.rs` (new, child of Play's adapter) | The four containers from Play's own state: `dump.inventory`, `dump.bank.items`, the corpse (`npcw.corpse`, only while the corpse window is up, mode 1), the shop's wares (`npcw.shop`, only while its window is up, mode 10), tunar (`p34`) and bank tunar (`p38`), icons through `inventory::item_icon`, capacities through `pack_cap`/`bank_cap`. Copies are made only for an addon: when a game event that changes an item list is seen (the same `begin_game_event`/`end_game_event` pair round 2 uses: `InvAdd`, `InvTook`, `InvMove`, `InvUse`, `InvEquip`, `InvUnequip`, `QuickSlot`, `GearSet`, `ItemHp`, and the opcodes 0x0018, 0x0016, 0x0019, 0x001A, 0x00B7, 0x00D7, 0x0052, 0x124D, 0x1250, 0x1251, 0x1253, 0x1206), and on a frame where a cheap key of the lists moved (a window closed by a key, a sale's tunar). |
| `fork_ext/addons/gems.rs` (new) | `eqoa.gems.get/is_gem/format`: the 80 pinned entries as static data; `format` takes the optional formatter and reports its error as `failed`. |
| `fork_ext/addons/items_forge.rs` (new) | `frontiers_forge.inventory`, `.item`, `.bank`, `.loot`, `.merchant`, `.gems` over the same core: Forge's 1-based positions, `Tab` enum 0..3 with the game's own labels, dense `bank.GetItems` `{idx, slot, level, name, amount}`, item objects with live methods and properties (their `.record` is the `ItemRef`), `SLOT`/`STAT`/`ATTACK` names, `Gems.Format` calling the real `Get` (the reference called a `Gems.get` that does not exist). Built on the first call (a host's instruction budget is not spent at creation). |
| `native_core.rs`, `native_events.rs`, `native.rs`, `api_capture.rs`, `api_updates.rs`, `fork_ext/play.rs` | The view carries the four containers; reset events on zoning; the capture hooks; the attack-word table for `attack_name`; capability rows. All fork-owned. |

Capabilities: every new function is `supported`; `eqoa.capabilities` lists them and the new record fields. What the client
cannot give without a seam is `not_yet` (round 6) with the reason, and reading it diagnoses (see below).

## What is not given, and the exact lines it would take

Nothing in an upstream file was changed or added (gate: footprint equal to before). Two groups of values live in private
fields of upstream windows and cannot be reached from fork code:

1. **The item page's tab and selection** (`Container.view`; Forge `inventory.IsWindowOpen/GetOpenTab/GetTabs/GetTabList/
   GetSelectedSlot/GetSelectedItem`): `ItemMenu.tab`, `.menu`, `.item` are private (`itemmenu.rs:284..`). Proposed seam
   (about 5 lines): in `impl ItemMenu` a read-only accessor
   `pub fn read_state(&self) -> (i32, i32, i32) { (self.tab, self.menu, self.item) }`
   (`Play.items` is already reachable from the adapter; `.visible` is already public). The rows would be derived on the
   fork side from the pack with `infotext`'s four tab predicates.
2. **The merchant window's mode, highlighted option and selected row** (`MerchantState.mode/selected_option/options/
   button_hint`; Forge `merchant.GetMode/GetSelectedOption/GetOptions/GetButtonHint/GetSelectedItemIndex/GetSelectedItem`):
   `NpcWindows.merchant` (`npcw.rs:21`) is private and `Merchant.menu`/`.list` are private. Proposed seam (about 6 lines):
   `npcw.rs`: `merchant: Merchant,` becomes `pub merchant: Merchant,`; `merchant.rs`, in `impl Merchant`:
   `pub fn read_state(&self) -> (i32, i32, i32) { (self.mode, self.menu.sel, self.list.sel) }`.
   The greeting, entity id, rates and `pending` are given already (they are public on `Shop`).

Until the owner approves them these fields are absent (nil), `eqoa.capabilities.get("eqoa.items.Container.view")` is
`not_yet` round 6 with the reason, reading the field diagnoses once per mod, and the listed Forge calls diagnose and return
nothing, never a made-up value. Also not built (the brief's scope): buy, sell, loot and bank-move transactions, in both
families; `windows.format_hint` (so `merchant.GetButtonHint`).

Decisions to confirm (small; the code follows the brief where it was silent): an inventory's `open` is always true (it has
no window of its own); `Container.capacity` follows the native list sizes (40, or the server's) rather than a fixed 40;
Forge `item.GetStats` values keep their sign (the reference read unsigned words); Forge `GetEquippedSlotIndex` returns the
paperdoll's own 0-based index, as the reference does.

## Tests

Test names are suffixes under `play::fork_hooks::api_capture::items_tests` (file `fork_ext/items_tests.rs`). Items are encoded as
the server sends them (`Writer`) and read back with the client's own `read_net_item`/`read_item_list`/`read_shop`; lists are
changed by the same native functions the session pump calls (`inventory::add_item`, `took`, `set_hp`, `equip`); the corpse,
merchant and bank open from their real opcodes (`npcw_opcode`, `bank_opcode`).

| Test (`round5_...`) | Covers |
|---|---|
| `pack_and_bank_come_from_the_dump_with_real_item_fields` | zone-in dump with pack and bank; slot -1 is nil, class/race masks expanded, `no_rent` is `rent==0`, negative stats keep their sign, an unknown stat id keeps its value with a nil name, a quick slot is native+1, `wear_slot` 0xFFFFFFFF, bank readable closed |
| `an_empty_pack_and_bank_are_empty_lists_not_missing`, `empty_corpse_is_open_with_no_items`, `a_kept_corpse_and_a_shop_behind_another_window_are_closed` | each container's empty and closed case; a corpse the dump kept and a shop stored behind another window are not given |
| `full_pack_counts_remaining_from_the_native_capacity` | 40 of 40 is full; a server cap of 60 is followed |
| `taking_an_item_compacts_the_list_and_handles_follow_their_stacks` | an item taken, list compacts, an earlier ref resolves to the same stack, the removed one is `stale_handle`, a re-added stack is a new handle |
| `stack_quantity_item_hp_and_tunar_each_change_once_with_their_cause` | item added to a stack, item HP (0x0060), tunar (0x0052); an event that changes nothing says nothing |
| `bank_opens_by_0x124d_moves_items_and_money_and_is_readable_closed` | bank open by 0x124D, 0x1251 add, 0x1250 take, 0x1253 money, closing keeps the contents |
| `corpse_window_opens_loses_rows_and_closes_to_nothing` | 0x0018 open, the Leave All row is no item, 0x0019 loot (pack and corpse each one event), 0x0016 close: `get` is nil, nil, old refs stale |
| `merchant_opens_by_0x00b7_prices_natively_and_close_discards_the_wares` | 0x00B7 open, native buy and sell rounding (17 and 34 for two; 79 for the worn sword, equal to `sell_price`), close discards wares and refs, a new session never revives an old ref |
| `zoning_resets_every_container_and_stales_every_handle` | each container goes with cause `reset`, `get` is `not_in_world`, refs stale after re-entry, the new world's containers come with cause `reset` |
| `a_frame_sees_a_window_closed_and_tunar_changed_with_no_event_of_their_own` | the incremental frame path (fixture demand off): bank window closed by a key, tunar changed, nothing changed |
| `pure_item_helpers_use_the_native_rules`, `gems_use_the_shipped_lookup`, `capabilities_list_every_new_function_and_say_what_is_missing` | `can_use` reasons in order, constants, `slot_info`, `stat_name`/`attack_name` with and without the game's words, gems, capability rows |
| `no_addon_means_no_item_copies_and_no_events` | with no addon: no capture, no copy, no event |
| `forge_item_modules_follow_the_pinned_shapes` and `items_forge::tests::every_function_of_each_forge_item_module_is_listed` | one test for all six Forge modules against the pinned semantics (1-based positions, tab enum, dense bank array, item objects, `IsValid`, loot iterator, gems, the unsupported calls), and that the lazy proxies list every function |

### Negative controls run (each: the code broken on purpose, the tests run, the failures counted, the code put back)

| Break | Result |
|---|---|
| native events no longer named (`items::cause` returns none for adds, takes and opcodes) | 8 of 16 tests FAILED (run before the frame test was added) |
| the frame refresh disabled (`items::refresh` returns at once) | 1 FAILED (`a_frame_sees_a_window_closed...`); this showed the earlier tests all ran through the test build's full publish, so that test was added |
| handles by list position instead of wire key | 2 FAILED (`taking_an_item...`, `corpse_window...`) |
| `no_rent` inverted | 2 FAILED |
| negative stat value made unsigned | 2 FAILED |
| corpse and shop given whatever the window mode | 1 FAILED (`a_kept_corpse...`) |
| item cause computed with no addon listening | 1 FAILED (`no_addon_means_no_item_copies...`) |

## Test changes (no test deleted or weakened)

- `fork_ext/addons/package.rs` (`forge_package_...` unsupported-module check): the module it required to be unsupported,
  `frontiers_forge.inventory`, is supported now; the same check (an unsupported `frontiers_forge.*` module fails naming the API)
  now names `frontiers_forge.quest`, which is still unsupported. Assertion text otherwise unchanged.
- `fork_ext/addons/native_tests.rs` (`native_capabilities_are_probeable_and_stubs_diagnose_even_when_caught`): it used
  `eqoa.inventory.get` as its example of a `not_yet` stub; it now uses `eqoa.camera.get` (still `not_yet`, round 4) for the
  same assertions (status `not_yet`, the call returns `not_yet`, the diagnostic names the symbol).

## Gates

| Gate | Result |
|---|---|
| `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1` (scratch `EQOA_CONFIG`) | 461 passed, 0 failed, 25 ignored, 9 SKIP lines |
| Full client suite, same flags, skipping the timing tests (`--skip round4_ --skip frame_time --skip benchmark --skip scrollback_10000 --skip measure`) | 2295 passed, 0 failed, 30 ignored, 50 SKIP lines (50 further tests were filtered out by name: the timing tests) |
| round 5 tests (`round5_`) | 17 passed, plus `items_forge::tests::every_function_of_each_forge_item_module_is_listed` |
| Zero-addon gate (optimized frame, menu, entity packet, chat tests, in the full run) | passed; the item hooks run only after the "an addon wants state" check, and `no_addon_means_no_item_copies_and_no_events` asserts it |
| Bundled Wereoxxs/ARPG/Modern UI, round 3 and round 4 mods | their existing tests are in the 461/2295 above and pass (those that assert zero Addon Manager diagnostics still do); I added no new bundled-mod session test |
| `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | PASS |
| `python tools/check_fork_footprint.py --base a76d8d27...` | PASS; 31 shared files before and after, no upstream file touched (`git diff` of upstream files is empty); 456 to 461 fork-owned files |
| `git merge-tree --write-tree --name-only HEAD origin/master` | clean before and after (no conflicts) |
| `python -X utf8 tools/test_player_release.py` | 20 tests OK |
| `cargo build --locked --release -p eqoa` after the last code commit | PASS (2 min 51 s) |

SKIP lines (50 in the full run): 14 no `run/client.bin`; 8 + 3 + 1 no 2002 or Frontiers disc variants (`EQOA_2002_ISO` not set; the Frontiers disc is at
`run/eqoa.iso`); 6 + 2 + 2 + 1 no live/ours/july S2 patch files or WORDS.TXT in sibling folders not on this machine; 1 no `EQOA_WORLDS_DIR`;
1 no live save state; 1 no `ss-art.csf`. The 9 in the addon suite are the same set as round 4 (Windows symlink privilege, pinned Forge fixtures,
reviewed picture goldens). No item test skips: none needs the disc or the decomp oracle (oracle tests skip on this machine as before).


## Fix round (review `round-5-review-a.md`, Terra; the lead's triage)

| Finding | Fix (commit "round 5 fix round...") | Test | Negative control (run) |
|---|---|---|---|
| H1: a second 0x0018 while a corpse is up, or a re-sent 0x00B7 from the same merchant, kept old ItemRefs alive when the wire keys repeated | `items_session_is_new` also returns true for the native open causes `loot_open` (loot) and `merchant_open` (merchant), so every earlier handle of that list is `stale_handle`. The bank's `bank_open` only flips the window; the bank's stacks are not replaced, so its handles stay | `round5_a_corpse_opened_again_with_the_same_wire_keys_is_a_new_session`, `round5_a_merchant_sent_again_by_the_same_merchant_is_a_new_session` (real 0x0018 / 0x00B7 bytes; old ref stale, new ref resolves to the new list's item) | the open-cause clause removed: both FAILED |
| H2 (MEDIUM): the frame check hashed every stack | `items_capture::key` is O(1) scalars: window modes 1/10/11, shop id and pending, the two money values, the four list lengths. No stack is read per frame. Stack contents change only through the events in `cause()`. One content change had no event of its own and is handled narrowly: a fishing catch (`fishplay.rs` `fishing_caught`, opcode 0x0F02, `add_item` into the pack) is now cause `fishing`. Other writers of `dump.inventory` (crafting, trade, auction 0x1206, loot 0x0019/0x001A, bank 0x1250/0x1251) already arrive as events in `cause()` | `round5_a_quiet_frame_does_no_stack_work_and_events_fire_once`: five frames with nothing changed copy no stacks (a test-build counter, `items::take_builds`), item HP by its event is one copy and one event, a window closed by a key is exactly one `close`, 0x0F02 maps to `fishing`. The earlier `a_frame_sees_a_window_closed_and_tunar_changed...` still passes | frame refresh made to copy every frame: FAILED |
| M5: Forge `IsValid` | `GetSlotId(record) ~= 0 and GetName(record) == self._name`, with the fetch-nil guard kept (`item.lua:539`) | `round5_forge_is_valid_needs_a_slot_id_as_the_reference_does` (a record whose wear slot is 0) | the slot test removed: FAILED |
| M3, M4 | No change (per the lead): `Container.view` and merchant mode/selection stay `not_yet` pending the owner's decision on the two seams above; `pack_cap`/`bank_cap` stay as the native rules | - | - |

Test-file changes: only additions (4 tests in `items_tests.rs`); no existing test changed or removed in this round.

The exact filter that runs every round 5 test (they sit outside `fork_ext::addons::`):
`cargo test --locked -p eqoa --bin eqoa -- round5_ every_function_of_each_forge --test-threads=1` (22 tests).

Gates after the fix round: round 5 filter above 22 passed; addon suite 461 passed, 0 failed, 25 ignored, 9 SKIP; full client suite
(timing skips as before) 2299 passed, 0 failed, 30 ignored; wasm check PASS; footprint PASS (31 shared files, no upstream file
touched); `tools/test_player_release.py` 20 OK; release build PASS after the last code commit. SKIP lines are the same set as Part A
(no disc oracle, sibling S2 files, `run/client.bin`).


## Part B: the bag viewer example mod (2026-11-15)

Builder: Claude Sonnet. Same branch, commits are LoopRook's, nothing pushed, merged or tagged. Tests are `round5b_...` under `fork_ext::addons::manager::tests` (the round 5 filter `round5_` does not match them: use `round5b_` too).

### What a player gets (`bundled-mods/eqoa_bag_viewer`, off by default)

Turn it on in Mods & Addons; its row **Bag viewer** appears on Controls, unbound (the game's keys are unchanged). The key opens and closes a read-only Bags window, centred in the HUD frame, scaled as the HUD is, always on screen; Escape closes it. It shows the 40 pack slots as item icons with a stack count (when more than one) and a green E on worn items, your tunar and used/40 under the grid, and tabs **All**, **Equipment** and **Bank** (the bank's items and bank tunar, readable anywhere). Hover an item: card with name, level, slot, stats (signed), HP, durability and value, and a red "You can't use this" when class, race or level rule it out (the API's own `eqoa.items.can_use`). Under the grid, the loot log: this session's pickups, newest first ("12:41 Rusty Sword x1"), last 50, the wheel scrolls it. Kept through zoning; "Leave All" adds nothing. While the window is closed it does nothing except note pickups for the log.

### Not in the window, and why (items 1, 2 and 4 are closed in "Completion round" below)

1. **Usable, Charged and Misc tabs are missing.** The game puts an item in those by its item type (`Item.p4c` in 7, 2, 5, 8 is Usable) and its charge definition (`Item.p54 > 0`, type not 8, not worn is Charged; Misc is the rest; `infotext::is_usable/is_charged/is_misc`). The `Item` record carries neither, and `Container.view` (the game's own tab rows) is the `not_yet` seam. Equipment is exact (`wear_slot ~= 0xFFFFFFFF`, which is the game's `wearable`; the test compares with `infotext::is_wearable` over the same pack and has a wear-slot-0 item that `equippable` would get wrong). Smallest fix: two fork-owned `Item` fields, `item_type` (= `p4c`) and `max_charges` (= `p54`), in `items_capture.rs` `item_json` plus the spec row and capabilities (about 6 lines, no upstream file), then the mod adds three predicates and tab labels (about 10 Lua lines). Not built: you decide whether the API grows. Nothing in the window pretends to have these tabs.
2. **"from <corpse>" is not on the log lines.** The corpse window's data (0x0018) is the item list only; `Container` for loot has no corpse name or entity, and `loot.changed` carries none. Entries read "13:36 Spider Silk x4". To give the name the API would need a corpse entity (the entity the player targeted for the loot request) on the loot container; your call.
3. Escape: the mod claims it (`eqoa.input.claim("Escape", true)`) only while the window is open so the game menu does not open too. If another mod owns Escape the claim is refused and the Addon Manager shows one `busy` entry; the window still closes on Escape.
4. The captures use flat stand-in icon art (a test art cache); the real art path is `eqoa.icons.resource(item.icon)` and was not run against the disc's art here. The mod falls back to the item's first letter when an icon is missing.

### Files

`bundled-mods/eqoa_bag_viewer/{mod.json, main.lua, modules/bags.lua, README.md}` (LF, `-text`); registered in `fork_ext/bundled.rs` (`PACKAGES`, `EMBEDDED_FILES`); `bundled-mods/SOURCE.txt` (four SHA-256 rows + a provenance line); `tools/build_player_release.py` and `tools/test_player_release.py` (the four files in `BUNDLED_MOD_FILES`); `crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs` (new, included from `manager.rs`'s test module); fixture methods `Play::bags_*` and `bags_native_tab` in `fork_ext/items_tests.rs`; `FORK-UPDATE-NOTES.md`, `docs/modding/GETTING-STARTED.md`. No upstream file, no budget change. Uses only `eqoa.inventory/bank/items/icons/hud/input/events/imgui` (no Forge layer, no private hook); the mod reads `loot.changed`, `inventory.changed`, `bank.changed`, `state.reset`.

### Tests (real Manager, the bundled files, GPU readback; protocol-shaped state)

The world is the zone-in dump (pack of eight with a worn sword, one stack of five, class/race/level-locked items and a wear-slot-0 item; bank of two; tunar 1,234 and bank 99,000) and the client's own handlers: loot window by real 0x0018/0x0019/0x001A/0x0016 bytes, 0x003B/0x003C item changes, 0x0052 tunar, zoning by the core's reset. Each test asserts zero Addon Manager diagnostics.

| Test (`round5b_...`) | Covers |
|---|---|
| `off_by_default_loads_clean_has_an_unbound_controls_row_and_its_key_opens_and_closes_it` | package present but Disabled, no Controls row until enabled; after enable zero diagnostics, row exists and is unbound (`""`), the unbound row and another key open nothing; bound to K opens and closes; Escape closes; a disabled mod's key fires nothing |
| `the_grid_shows_the_pack_as_the_dump_gave_it_and_follows_its_changes` | 8 items in dump order, 5 rows of 8 = 40 slots, used 8, capacity 40, tunar 1,234; icons and counts from the records; one worn; a quiet frame reads nothing; a taken stack, an added stack and a tunar change each update the grid after their event, one read per event |
| `equipment_follows_the_games_own_tab_and_the_bank_reads_away_from_a_banker` | tab strip All/Equipment/Bank; Equipment equals the game's `is_wearable` list over the same pack (6 stacks, includes wear slot 0); used/capacity still the whole pack; Bank shows the two bank items and 99,000 with the bank window closed; back to All |
| `the_card_has_the_items_fields_and_the_red_line_only_when_the_character_cannot_use_it` | sword card lines (level, slot, signed stats, HP, value) and no red line; the ring (class/race/level all allowed) no red line; robe (class), cloak (race), helm (level) each red, each with exactly that one reason from `can_use` |
| `the_loot_log_is_fed_by_pickups_newest_first_and_keeps_fifty` | window closed: three pickups (one, one, all) newest first as `HH:MM Name xN`, no grid read; Leave All with items left adds nothing; zoning and a new world keep the log; 51 pickups keep the newest 50 |
| `a_closed_window_does_no_work_and_an_open_one_rebuilds_only_on_change` | closed: 30 frames and a tunar change give 0 grid reads and 0 draws; open: one read, one draw per frame, no more reads in 30 frames; closed again: no draws, no reads |
| `the_window_follows_the_ui_scale_and_stays_on_screen` | 640x480, 1024x768, 1600x900 and 320x240: the window is on screen, 286 frame pixels at the HUD frame's scale, larger in a larger window |
| `picture_the_window_is_drawn_where_it_says_and_closed_it_is_not` | through the real renderer: panel drawn where the mod says and absent closed (scene shows through), nothing changes outside the panel, three slots show their icons' colours, the worn marker's green and the active tab's red underline, the hover card's red frame, log text; a control pixel (an unworn slot's edge) is not green. Writes the captures below when `EQOA_R5B_CAPTURE_DIR` is set |

### Negative controls (each: the mod's Lua changed in a copy, the tests run against it via `EQOA_R5B_TEST_SOURCE`, the copy removed)

| Break | Failed |
|---|---|
| `loot.changed` handler removed | 2 (loot log, picture) |
| Equipment tab by `equippable` instead of wear slot | 1 (equipment) |
| red line never added / red line on every item | 1 each (card) |
| `close()` leaves its listeners on | 1 (closed window) |
| grid read again every frame | 2 (closed/open, grid) |
| log keeps 51 | 1 (loot log) |
| Leave All logs what is left on the corpse | 1 (loot log) |
| zoning clears the log | 1 (loot log) |
| default key K / enabled by default | 1 each (off by default) |
| window never drawn | 6 |
| Escape does not close | 1 (off by default) |
| Bank tab reads the pack | 1 (equipment/bank) |

14 breaks, every one made tests fail; the baseline is 8 of 8 passing.

### Captures for the owner (flat stand-in icon art)

`docs/plans/eqoa-api/r5-bags-all.png` (All tab, sword hovered, two pickups in the log), `r5-bags-bank.png`, `r5-bags-loot.png`.

### Test file changes (no test deleted or weakened)

- `fork_ext/addons/round_minimap_manager_tests.rs`: `every_bundled_mod_runs_a_play_session_with_zero_manager_diagnostics` asserted the bundled set is 11 packages; it is 12 with the bag viewer (the message says nine `eqoa_*` mods). The session now also runs the bag viewer through enable, disable, zoning and the rest, with zero diagnostics.
- `fork_ext/bundled.rs` tests: the five expected package lists gain `eqoa_bag_viewer` after `eqoa_round_minimap`.
- `fork_ext/items_tests.rs`: `Spec` gained an `icon` field (default 4001, what every earlier test read) and the Part B fixture methods; additions only.
- `fork_ext/addons/manager.rs`: one `include!` line for the new test file.

### Gates

| Gate | Result |
|---|---|
| `round5_ round5b_ every_function_of_each_forge` | 30 passed (22 Part A + 8 Part B) |
| `fork_ext::addons::` (scratch `EQOA_CONFIG`) | 468 passed, 1 failed (the bundled-set count, 11 to 12, fixed as listed above; that test then passes alone and the full-suite run below includes it), 25 ignored |
| Full client suite, timing skips as before | 2307 passed, 0 failed, 30 ignored (2299 + 8) |
| Bundled Wereoxxs, ARPG, Modern UI and the round 3/4 mods | in the runs above, pass; the zero-diagnostics play-session gate runs each of the 12 packages |
| `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | PASS |
| `check_fork_footprint.py --base a76d8d27...` | PASS; 31 shared files as before, no upstream file touched, no budget change |
| `git merge-tree` against `origin/master` | clean |
| `tools/test_player_release.py` | 20 OK |
| Release build, `RUSTFLAGS=-C target-feature=+crt-static`, after the last code commit | PASS (2 min 22 s) |
| Packaging dry run (`tools/build_player_release.py --channel enhanced-dev` with the local catalogue) | PASS: the zip carries the four `mods/eqoa_bag_viewer/` files and the checksum check accepts them. The executable was built with crt-static, `EQOA_BUILD_COMMIT` and `EQOA_UPDATE_CHANNEL=enhanced-dev` at e1f2ebcc (1 min 38 s) and packaged from a clean clone of that commit, because the packager refuses a checkout holding your untracked `round-5-review-a.md`. Temp output and clone deleted |

SKIP lines: not recounted this round (passing tests' output is captured). The Part A set stands (no disc oracle, sibling S2 files, `run/client.bin`, disc variants); Part B added none: its 8 tests all ran on this machine's GPU adapter and wrote the pictures (without an adapter they print a SKIP line and return).

### Completion round (the lead's decisions on the three gaps)

**1. Tabs.** New `Item.tabs: string[]` (fork-owned: `items_capture.rs` `tab_keys`, capability row in `native_items.rs`, spec row in section 6): the item-page tab keys the stack is under (`"usable"`, `"equipment"`, `"charged"`, `"misc"`, in the page's order), by calling `infotext::is_usable/is_wearable/is_charged/is_misc`, the four predicates `itemmenu.rs` sorts the page with. No raw `p4c`/`p54` is exposed and no rule is copied into Lua. The mod's tabs are now All, Usable, Equipment, Charged, Misc, Bank; the four game tabs filter on `item.tabs`. No Forge mapping (no pinned call needs it).

**2. "from <corpse>".** New `Container.source: {entity_id, name}?` on the loot container (`items_capture.rs` `note_source`; capability rows for `Container.source` and `LootSource`; spec row and build-status note). The client's loot request (0x0015, `play.rs` `attack_press`) names the targeted corpse, but recording it there would be a line in an upstream file, so the source is taken when the corpse's list arrives (0x0018): the entity the player has targeted, only if it is a named corpse (`p18 & 2`). (Superseded by the Part B fix round below: it now also requires the target to have been steady for a second.) It is kept while that window is up even if the target moves, and cleared when it closes. A log line is "HH:MM Name xN, from <name>" with a source and "HH:MM Name xN" without. Footprint unchanged (31 shared files, no upstream line).

**3. Captures with the real art.** `r5-bags-all.png` (sword hovered), `r5-bags-bank.png`, `r5-bags-loot.png` are redrawn by `round5b_captures_for_the_owner_draw_the_discs_own_item_art_and_the_corpse_in_the_log`: the disc's ITEMICON.ESF icons (the first 12 with real pictures) are given to the live art cache with `update_live_data` and drawn through `eqoa.icons.resource`; the log shows "from a decaying skeleton". The test asserts each of the eight pack slots shows more than a few colours, so a flat square fails it. It prints SKIP and returns without `$EQOA_ISO`. The icons are the disc's, which have their own grey plate and black count strip; the item names are the fixture's. The window grew to 318 frame pixels wide (36 pixel slots) so a "from" line fits.

New tests: `every_stack_has_the_tabs_the_games_item_page_gives_it_and_each_tab_lists_exactly_those` (nine stacks: usable by types 2, 7 and 8, a charged wand, a worn-slot ring with a charge definition that is Equipment but not Charged, a wear-slot-0 item, misc; `item.tabs` equals the native predicates for every stack and each of the five tabs lists exactly the native list), the captures test above, and additions to the loot log test (source while open and after the target moves, no source for a living target, line with and without "from"). The old tab test now clicks Equipment as tab 3 and Bank as tab 6.

Negative controls run this round: charged by `is_usable` (Rust): 1 failed (tabs); any targeted entity is the source (Rust): 1 failed (loot log); source never noted (Rust): 1 failed (loot log); every stack under every tab: 2 failed; tab by quantity instead of `Item.tabs`: 2 failed; no "from" on the line: 1 failed; "from a corpse" when unknown: 1 failed (loot log). The earlier 14 controls were not rerun after the layout change (they concern behaviour, not layout).

Test file changes this round: `items_tests.rs` `Spec` gained `p4c` and `p54` (default 0) and the `bags_target_corpse`/`bags_target` fixtures; `bag_viewer_tests.rs` slot geometry (36 pixel slots, 318 wide) and the tab numbers. No test deleted or weakened.

**4. SKIP lines.** The full run was made with `--nocapture`: **50 SKIP lines**, the same count as Part A, none from the bag viewer (its 10 tests ran, including the real-art captures): 14 no `run/client.bin`; 8 no 2002 disc (`EQOA_2002_ISO`); 6 no live S2 patch; 4 symlink privilege not held; 3 no Frontiers disc or no 2002 disc; 2 no ours S2; 2 no july S2; 1 no Frontiers or 2002 disc; 1 no `run/eqoa.iso` or `EQOA_WORLDS_DIR`; 1 no live save state; 1 no disc or `ss-art.csf`; 1 no `WORDS.TXT`; 3 bundled-UI picture comparisons with no reviewed `EQOA_BUNDLED_UI_GOLDENS` (wereoxxs_ui, arpg_ui, avoids_modern_ui); 2 pinned Forge hearts/`forge_gpu` (`EQOA_FORGE_SPIKE`); 1 no `world10.json`.

Gates after the completion round: round 5 filters (`round5_ round5b_ every_function_of_each_forge`) 32 passed (22 Part A, 10 Part B); `fork_ext::addons::` 471 passed, 0 failed, 25 ignored; full client suite with the timing skips 2309 passed, 0 failed, 30 ignored; wasm check PASS; footprint PASS (31 shared files, no upstream file touched); merge-tree clean; `tools/test_player_release.py` 20 OK; release build and packaging dry run below.

Release build (crt-static, `EQOA_BUILD_COMMIT` and `EQOA_UPDATE_CHANNEL=enhanced-dev`) at 050ea6d0: PASS (2 min 7 s). Packaging dry run from a clean clone of that commit (your untracked review file stays in the worktree): PASS, the zip carries the four `mods/eqoa_bag_viewer/` files; temp output and clone deleted.

## Part B fix round (review `round-5-review-b.md`, Terra; the lead's triage)

**1. Loot `source` race (MEDIUM).** The target at the 0x0018 reply is not proof of which corpse was asked: the request names the target when it is sent, and the player may retarget before the reply. Fix, no upstream line: `items_capture.rs` tracks the target id and the game's-clock time (`session.now()`, milliseconds) it was first seen, O(1), once a frame while an addon wants state (`refresh`, `note`) and again when a corpse arrives; `forget` clears it. `Container.source` is given only when the current target is a named corpse AND it has been the target for at least 1 second (`STEADY_MS`) before the reply; otherwise nil. A repeated 0x0018 follows the same rule. The zero-addon path is unchanged. Wording fixed in the spec row (now "the corpse you had targeted, when that is certain; nil when it is not"), the mod README and the update notes; "never a guess" is gone. One consequence for the owner: clicking a corpse targets and asks in one action, so its source is usually nil; a corpse targeted a moment before the loot key is pressed has one.

Tests (`items_tests.rs`, protocol-shaped, real 0x0018/0x0016 bytes; the clock is moved on by a test-only skew): `round5b_a_corpse_targeted_steadily_is_the_source_of_its_loot_window` (name and entity id; the window going away takes it); `round5b_a_target_that_changed_before_the_reply_is_not_the_source` (request on A, target B just before the reply: nil; a steady living target: nil); `round5b_a_repeated_reply_after_a_recent_target_change_is_not_the_source` (steady A has a name; B targeted, a frame sees it, reply 200 ms later: nil; 1.5 s later the repeated reply: B). The manager-level loot tests now let the target be steady before the corpse arrives. Negative controls run: steadiness not required: 2 failed; the target not looked at when the reply arrives: 1 failed.

For the owner's decision (not added): the exact fix. `crates/eqoa/src/play.rs:5804`, in `attack_press`, the only place the client builds the loot request (0x0015; `click` reaches it through the same function), inside `if e.p18 & 2 != 0 {`, one line before `let mut w = net::stream::Writer::op(0x0015);`:
`crate::fork_ext::loot_requested(self.target);`
Fork side: `loot_requested` stores `(id, clock)` in `items_capture.rs`; at 0x0018 the source is that id when it was asked for within a few seconds and the entity is a named corpse, and the steadiness rule goes away. Footprint cost: `play.rs` +1 added, 0 deleted (budget 92/51 to 93/51), one hook call, a reviewed reason ("the loot request names its corpse").

**2. Black strip under icons (looks).** Cause: the disc's item icons are 128x64 surfaces holding a 114x44 plate; the rest is padding, transparent in some icons and opaque black in others. Trimming transparent pixels left the black padding on five of the twelve (slots 1, 2 and 9 among them). The mod now asks `eqoa.icons.resource(icon, {trim = true, trim_color = {r = 0, g = 0, b = 0, a = 1}})`, which trims both, so every icon is the 114x44 plate and is drawn filling its slot (stretched to the square, as the others always were; keeping the plate's 2.6:1 shape would make a 36 by 14 strip). The real-art capture test now asserts for each of the eight pack slots that the point near the slot's bottom-left is not black; control: with the trim colour removed it fails ("slot 1: black strip under the icon"). `r5-bags-{all,bank,loot}.png` redrawn.

Test file changes: `items_tests.rs` additions only (three tests, `bags_advance_clock`); `bag_viewer_tests.rs` (target steady before the corpse; the new strip assertion). No test deleted or weakened.

Gates after the fix round: `round5_ round5b_ every_function_of_each_forge` 35 passed (22 Part A, 13 Part B); `fork_ext::addons::` 471 passed, 0 failed, 25 ignored; full client suite with the timing skips 2312 passed, 0 failed, 30 ignored; **SKIP lines 50** (`--nocapture`), the same set as listed above, none from the bag viewer; wasm check PASS; footprint PASS (31 shared files, no upstream file touched, no budget change); merge-tree clean; `tools/test_player_release.py` 20 OK; SOURCE.txt hashes regenerated for the changed mod files.

Release build (crt-static, `EQOA_BUILD_COMMIT` and `EQOA_UPDATE_CHANNEL=enhanced-dev`) at `7152fba8` (the last code-and-gates commit; this note is a docs-only commit after it): PASS (3 min 51 s), left at the shared target's `release/eqoa.exe` for the owner package. Packaging dry run from a clean clone of `7152fba8`: PASS, the zip carries the four `mods/eqoa_bag_viewer/` files; temp output and clone deleted.

## Seams (the three owner-approved upstream seams, 2026-11-15)

The owner approved exactly the lines proposed above. Upstream files carry only these lines (neutral, in upstream's style, nothing reordered or reformatted):

| File | Lines | Budget in `docs/fork-footprint.json` |
|---|---|---|
| `crates/eqoa/src/play.rs` (`attack_press`, inside `if e.p18 & 2 != 0 {`, before the 0x0015 request is built, line 5804) | `crate::fork_ext::loot_requested(self.target);` | 92/51 to 93/51 (+1/-0); hook entry "loot request" count 1 |
| `crates/eqoa/src/itemmenu.rs` (`impl ItemMenu`, the first item) | `pub fn read_state(&self) -> (i32, i32, i32)` with its one-line doc: `(self.tab, self.menu, self.menus.get(self.tab as usize).map_or(-1, \|m\| m.list.sel))` | new, +4/-0 |
| `crates/eqoa/src/merchant.rs` (`impl Merchant`, the first item) | `pub fn read_state(&self) -> (i32, i32, i32)` with its one-line doc: `(self.mode, self.menu.sel, self.list.sel)` | new, +5/-0 |
| `crates/eqoa/src/npcw.rs` (`NpcWindows`) | `merchant: Merchant,` becomes `pub merchant: Merchant,` | new, +1/-1 |

Reason on each budget: "owner-approved 2026-11-15, round 5: <seam>". Footprint PASS (34 shared files now, three new). No other upstream line was touched.

**Loot request hook.** `fork_ext::loot_requested` (fork-owned, `fork_ext/mod.rs`) records the corpse id and a clock reading in `fork_ext/loot_source.rs` (plain data, no `Play`; a native-only module with a no-op on wasm). When the corpse's list (0x0018) arrives, `items_capture::note_source` takes that request: `Container.source` is the requested corpse when the reply comes within 5 seconds and the entity is a named corpse; a reply uses its request up. The 1-second steadiness rule is gone, and so is its consequence that a clicked corpse had no name: clicking a corpse targets and requests in one action, and the log now says "from <name>" for it. What the player targets before the reply does not matter. Tests (real 0x0018/0x0016 bytes and the real `attack_press`): `round5b_a_corpse_clicked_and_looted_at_once_is_the_source_of_its_loot_window`, `round5b_a_target_switched_before_the_reply_is_still_the_requested_corpse`, `round5b_a_reply_with_no_recent_request_for_a_corpse_has_no_source` (no request; a 6-second-old request; attacking a living creature); the manager-level loot log tests press loot through the same path. Controls run: the hook call removed: 4 failed; the source taken from the target at the reply (the old rule): 2 failed.

**Item page.** `Container.view` for the pack while the start menu's item page is up (`items.visible`): `tab` (native + 1), `tabs` (four rows with the game's labels, words 437, 438, 1473, 439), `rows` (the stacks under the tab by the same four predicates, in the page's order), `selected` and `selected_row` (the lit row when the page is on a tab, nil in the quick-slot and weapon-set modes), built in `items_capture.rs` `pack_view` from `ItemMenu::read_state`; the core turns the wire keys into handles (`native_items.rs` `resolve_view_refs`). Forge: `inventory.IsWindowOpen/GetOpenTab/GetTabs/GetTabList/GetSelectedSlot/GetSelectedItem`. The reference's exact return shapes are not on this machine; they follow the Part A notes and the spec section 11 wording: `IsWindowOpen` a bool; `GetOpenTab` the tab enum 0..3 or nil; `GetTabs` `{tab, label}` rows or nil; `GetTabList(tab?)` the Item objects under a tab (the open one by default), in the page's order; `GetSelectedSlot` the lit stack's 1-based position in the pack, not its row; `GetSelectedItem` its Item object. Flag these shapes for review against the reference when it is available.

**Merchant window.** `MerchantState.mode` (`menu`, `sell`, `buy`), `selected_option` and `options` (Buy, Sell, Leave with the game's labels, the lit one in the menu) and `Container.view` for the merchant (wares when buying, the pack's stacks when selling, the lit row), from `Merchant::read_state` through `merchant_window`. Forge: `merchant.GetMode/GetSelectedOption/GetOptions/GetSelectedItemIndex/GetSelectedItem`; `GetButtonHint` stays `not_yet` (it needs `windows.format_hint`), as the brief allowed. Capability rows: `Container.view`, `MerchantState.mode/selected_option/options` and the new `ContainerView`, `Tab`, `MerchantOption`, `LootSource` types are supported; `MerchantState.button_hint` and `frontiers_forge.merchant.GetButtonHint` are still `not_yet` round 6.

**Events.** The frame's cheap key now includes the two `read_state` tuples and the item page's visibility (scalars, no stack hashing); a change reads the lists once and fires `inventory.changed` / `merchant.changed` once (cause `frame`, `changed` naming `view` or `merchant`).

Tests (Play-shaped; the game's own `ItemMenu` and `Merchant` driven by their keys and drawn with a raster that draws nothing, `items_tests.rs`): `round5b_the_item_pages_tab_and_lit_row_are_the_pack_containers_view_and_each_change_is_one_event` (closed: nil; opens on the usable tab; Right then Down: equipment tab, second row; a frame with nothing moved: no event; charged and misc tabs; closing: nil and one event; Forge's calls) and `round5b_the_merchant_windows_mode_option_and_row_are_the_containers_state_and_each_change_is_one_event` (menu with Buy lit; Down lights Sell; Cross: selling lists the pack with the first row lit, Down the second; buying lists the wares; Forge's calls; closing: nil). Controls run: rows not filtered by the tab's predicate: 1 failed; the lit row never reported: 1; the frame key ignoring the windows' state: 2; the item page's view given while closed: 1; selling lists the wares: 1.

Test file changes: `items_tests.rs`: the capability test now expects `Container.view` and `MerchantState.mode` supported and `button_hint` `not_yet` (they were `not_yet`); `round5_merchant_opens_by_0x00b7...` asserts the window's menu state where it asserted `mode == nil and options == nil`; `round5_forge_item_modules_follow_the_pinned_shapes` asserts the Forge calls' new answers (a merchant on its menu with Buy lit, the item page closed as `false`/nil) where it asserted they were nil; the loot source tests of the fix round (steadiness) are replaced by the request-based ones above (the steady-target rule they tested is gone). New helpers: `Quiet`, `bags_item_page`, `bags_merchant_open`, `bags_merchant_keys`, `bags_press_loot`. No test was deleted without its behaviour being replaced.

Gates after the seams: round 5 filters (`round5_ round5b_ every_function_of_each_forge`) 39 passed; `fork_ext::addons::` 471 passed, 0 failed, 25 ignored; full client suite with the timing skips 2314 passed, 0 failed, 30 ignored; **SKIP lines 50** (`--nocapture`), the same set as listed above, none from the bag viewer or the seams; wasm check PASS (the loot hook has a wasm no-op); footprint PASS (34 shared files; `play.rs` +93/-51, `itemmenu.rs` +4/-0, `merchant.rs` +5/-0, `npcw.rs` +1/-1; one new hook); merge-tree against `origin/master` clean; `tools/test_player_release.py` 20 OK; SOURCE.txt hashes regenerated.

Release build (crt-static, `EQOA_BUILD_COMMIT` and `EQOA_UPDATE_CHANNEL=enhanced-dev`) at `42ea2da9` (the last code-and-gates commit; this note is a docs-only commit after it): PASS (2 min 59 s), left at the shared target's `release/eqoa.exe` for the owner package. Packaging dry run from a clean clone of `42ea2da9`: PASS, the zip carries the four `mods/eqoa_bag_viewer/` files; temp output and clone deleted.
