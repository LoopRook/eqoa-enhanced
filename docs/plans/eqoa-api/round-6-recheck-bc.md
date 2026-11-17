# Round 6 B/C recheck

**Verdict: REJECT**

Reviewed `65cc7de2..e095109d` against the fix brief and `develop` (`b8d5fbdd`). The native consumer seams and gesture changes address their original findings, but confirmation remains unsafe when native and broker transactions overlap.

## Original findings

- **H1 — FIXED.** `crates/eqoa/src/fork_ext/addons/native_events.rs:608` creates one scope outside the listener loop at `:639`; `gesture.rs:150` rejects another action or batch once either allowance is consumed. Key bindings arm that shared scope and consumption occurs on accepted enqueue. The multiple-listener regression is `action_broker.rs:round6b_one_gesture_per_physical_event_across_multiple_listeners`.
- **H2 — FIXED.** The transaction branches in `crates/eqoa/src/fork_ext/play.rs:1643` through `:2020` stage native outputs and camera inputs; I found no copied loot, merchant or bank packet builders or writes to `shop.pending` or `bank.state` there. Exactly one new hook call is present in each shared file: `npcw.rs:474` immediately before the loot loop and before the merchant loop, `bankplay.rs:216` immediately before the bank loop, and `play.rs:3326` in `Play::frame` for camera (the latter is not a consumer loop). The native loops build packets and set pending state. The adapters check the corresponding window/mode, with closed-window refusal. The staged-request collision defect below is separate from packet ownership.
- **H3 — PARTLY FIXED.** `fork_ext/play.rs:192` correlates buy additions with item identity and quantity, `:214` checks loot identity, and `:229` treats corpse close as unknown. Timeouts produce unknown and abort a batch (`action_broker.rs:948`). However, `fork_ext/play.rs:555-570` removes a staged bank request before checking whether native output already exists. If a player deposits the same item in that frame, the broker request is never sent, but the player's `0x1251` clears native state (`bankplay.rs:107-114`) and `action_broker.rs:1783-1785` confirms the broker deposit solely on item ID, even if its requested quantity differs. A batch can therefore advance on an unrelated reply. Also `action_broker.rs:1624` accepts *any* decrease of the right corpse stack, even when less than the requested count, so a partial or unrelated decrease can be reported as full confirmation. `LootAll` confirms any `0x001A` without checking a state change (`fork_ext/play.rs:225`, `action_broker.rs:1645-1654`); an unrelated same-opcode reply can confirm it. No claim of exhaustive protocol correlation is made.
- **M4 — FIXED.** Frame entry and manager rendering enter render scopes at `host.rs:585` and `manager.rs:561`; the real manager/frame widget regression is `manager.rs:round6b_frame_mode_and_forge_widget_click_grants_one_gesture_via_manager`.

## Restored tests

Compared with `develop`: the assist test now checks the queued kind and broker dispatch (`native_tests.rs:523-528`); the picture test checks the actual kind (`round_3_picture_tests.rs:1256`); the capture test checks 32 sends across ticks and no extra send (`api_capture_tests.rs:1819-1845`). These restore the three lost assertions, with a stronger drain count than the original 16-request test.

## New defect introduced by the fix

**HIGH — staged transactions silently dropped on contention.** The new hooks call `take()` before checking `_loot_out`, `_merch_out` or bank `out`/busy (`fork_ext/play.rs:429-445`, `:500-501`, `:528-529`, `:555-570`). When a native action is queued in the same frame, the broker request disappears without being sent or refused, but remains in flight. It can time out as unknown, or the native action's reply can falsely confirm it as described under H3. Retain or explicitly refuse the staged request before consuming it and only correlate replies to requests actually sent.

## Verification

- `python tools/run_gates.py --only round --filter round6b_ round6c_ --lock-wait 120`: **PASS**, 20 passed, 0 failed, 0 ignored. Initial sandbox attempt could not create the shared lock; the completed run used approved access to the shared lock and target cache.
- H3 negative control: temporarily changed the LootClose resolution for loot-one from unknown to success in `fork_ext/play.rs`; `python tools/run_gates.py --only round --filter round6c_no_false_confirmations --lock-wait 120` **FAIL**, 0 passed, 1 failed (`round6c_no_false_confirmations_unrelated_invadd_or_corpse_close`). Restored using `git checkout -- crates/eqoa/src/fork_ext/play.rs`; status for that file was clean afterward. Checkout required approved access to the worktree Git index.
- No live server or owner playtest was performed. Filtered tests do not prove absence of the contention and reply-correlation scenarios above.
