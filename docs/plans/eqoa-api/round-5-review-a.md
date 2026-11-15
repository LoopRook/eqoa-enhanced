# REJECT

## HIGH

1. `crates/eqoa/src/fork_ext/addons/native_items.rs:110` — A second 0x0018 received while a corpse window is open replaces `npcw.corpse` (`npcw.rs:149-158`), but `items_session_is_new` only recognizes a previously null container or a different merchant ID. If the new corpse reuses a wire key, an old ItemRef resolves to the new corpse's item instead of `stale_handle`. The same problem occurs for a replacement 0x00B7 for the same merchant while mode 10 remains active. Session changes must invalidate handles even when keys and entity ID match.

2. `crates/eqoa/src/fork_ext/items_capture.rs:196` — Item state is polled every frame and changes without a recognized game event are coalesced with cause `frame`. For example, a bank window closed and reopened between refreshes produces no close or open event and an addon retaining state cannot observe either transition. This violates the requested event-boundary, once-per-change behavior and the catalog's native-event/open/close/reset causes. Hashing every stack each frame also violates the explicit no-per-frame-polling requirement.

## MEDIUM

3. `crates/eqoa/src/fork_ext/addons/native_items.rs:461` — `Container.view` and merchant mode/selection fields are deferred even though section 6 and the brief require them this round. A bag addon cannot display the active tab, its rows or selected item; merchant addons cannot determine buy versus sell mode. The added build-status note documents the deviation but does not implement the contract; inventory view changes also cannot produce the required event.

4. `crates/eqoa/src/fork_ext/items_capture.rs:113` — Capacity follows `pack_cap()` and `bank_cap()`, whereas the reviewed contract specifies inventory capacity 40 and bank capacity nil absent a native rule. With a server list size of 60, the API reports 60 instead of 40, and bank clients receive a numeric capacity where the contract says nil. The added spec addendum changes the contract without a decision in the brief.

5. `crates/eqoa/src/fork_ext/addons/items_forge.rs:185` — Forge `IsValid` omits the pinned `GetSlotId(record) ~= 0` condition. A live item whose wear slot is zero is reported valid here, while pinned Forge reports false. Mods using that guard can process a record the reference rejects.

## LOW

None.

## Existing tests changed or deleted

- `crates/eqoa/src/fork_ext/addons/native_tests.rs`, `native_capabilities_are_probeable_and_stubs_diagnose_even_when_caught`: substitutes the still-unimplemented camera stub for inventory, now implemented. The same status, error and diagnostic assertions remain; does not weaken the stub test.
- `crates/eqoa/src/fork_ext/addons/package.rs`, unsupported Forge module assertion: substitutes quest for the newly supported inventory module. The unsupported-module error and exact symbol assertion remain; does not weaken the test.
- No pre-existing test deleted. New tests use protocol/Play-shaped records; the single Forge integration test combines all six modules rather than the brief's one test per module. Builder reports negative controls, not independently rerun in this read-only review.

## Ownership and validation

No added or changed lines in non-fork-owned files relative to `8efdc147...HEAD`; no change to `docs/fork-footprint.json` or its budgets. Footprint checker against `a76d8d27a1c3f51fca545d7eabaca9e5dc336091`: PASS (31 shared files, within budgets).


Requested addon rerun (scratch EQOA_CONFIG, shared target and lock): 426 passed, 0 failed, 23 ignored, 1,926 filtered out; 0 visible SKIP lines (the harness captures passing-test output). The round5_ Play-shaped tests are outside the requested fork_ext::addons:: filter and were not rerun.
