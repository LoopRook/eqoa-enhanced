# Round 5 report: the items API (Part A, 2026-11-15)

Branch `feat/eqoa-api-r5` from develop `8efdc147` (after sync 2026-11-15), reviewed footprint base
`a76d8d27a1c3f51fca545d7eabaca9e5dc336091`. Builder: Claude Sonnet. Commits are LoopRook's; nothing was pushed, merged
or tagged. Part B (the example mod) is not started.

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
| Full client suite, same flags, skipping the timing tests (`--skip round4_ --skip frame_time --skip benchmark --skip scrollback_10000 --skip measure`) | 2295 passed, 0 failed, 30 ignored, 50 SKIP lines (the 50 skipped by name are the timing tests) |
| round 5 tests (`round5_`) | 17 passed, plus `items_forge::tests::every_function_of_each_forge_item_module_is_listed` |
| Zero-addon gate (optimized frame, menu, entity packet, chat tests, in the full run) | passed; the item hooks run only after the "an addon wants state" check, and `no_addon_means_no_item_copies_and_no_events` asserts it |
| Bundled Wereoxxs/ARPG/Modern UI, round 3 and round 4 mods | their tests are in the 461/2295 above and pass; no bundled-mod session reported an Addon Manager diagnostic (the new `not_yet` fields are only reported when a mod reads them, and none does) |
| `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | PASS |
| `python tools/check_fork_footprint.py --base a76d8d27...` | PASS; 31 shared files before and after, no upstream file touched (`git diff` of upstream files is empty); 456 to 461 fork-owned files |
| `git merge-tree --write-tree --name-only HEAD origin/master` | clean before and after (no conflicts) |
| `python -X utf8 tools/test_player_release.py` | 20 tests OK |
| `cargo build --locked --release -p eqoa` after the last code commit | PASS (2 min 51 s) |

SKIP lines (50 in the full run): 14 no `run/client.bin`; 8 + 3 + 1 no 2002 or Frontiers disc variants (`EQOA_2002_ISO` not set; the Frontiers disc is at
`run/eqoa.iso`); 6 + 2 + 2 + 1 no live/ours/july S2 patch files or WORDS.TXT in sibling folders not on this machine; 1 no `EQOA_WORLDS_DIR`;
1 no live save state; 1 no `ss-art.csf`. The 9 in the addon suite are the same set as round 4 (Windows symlink privilege, pinned Forge fixtures,
reviewed picture goldens). No item test skips: none needs the disc or the decomp oracle (oracle tests skip on this machine as before).

