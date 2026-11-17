# Round 6 B/C recheck 2

**Verdict: REJECT**

Reviewed `5d4d6f62..baeaa159`, the fix brief, and the Fix round 2 report. The requested quantity and state-change checks are present, but actual frame ordering leaves reply correlation unsafe.

## Remaining items

- **Staged requests dropped on contention — FIXED for the original collision.** `crates/eqoa/src/fork_ext/play.rs:524` and `:659` inspect without taking; `:543`, `:598`, `:626`, and `:671` retain a request while its native output or busy flag blocks it. `action_broker.rs:993` refuses an unsent request as busy after its deadline; `play.rs:425` clears the staged slot. Actual append and sent marking are paired at `play.rs:553-555`, `:585-587`, `:610-611`, `:637-638`, and `:686-703`, subject to the merchant caveat below.
- **Player transaction falsely confirms broker — PARTLY FIXED.** `action_broker.rs:1685-1694` excludes unsent requests, so a player's reply cannot confirm the still-staged request in the original scenario. However the frame contention guard is not effective for normal frame ordering (HIGH below); a sent request can still be confirmed by a player's matching reply. The test at `items_tests.rs:2379-2380` calls the output hook before delivering a reply, unlike the real frame.
- **Bank identity and exact quantity — FIXED as matching criteria.** `action_broker.rs:2110-2127` requires item ID and quantity for deposit, and key, item ID, quantity and remaining bank stack for withdrawal. This does not eliminate the independent origin ambiguity above.
- **Loot partial decrease — FIXED.** `action_broker.rs:1886-1905` requires exactly the requested decrease, or disappearance for the whole stack; a mismatch resolves unknown. `items_tests.rs:2400` exercises partial decrease and batch termination.
- **LootAll opcode alone — FIXED.** `play.rs:125-132` captures corpse items before the event and `:239-252` compares afterward; `action_broker.rs:1942-1948` returns unknown without a state change. The test at `items_tests.rs:2400` also checks a no-op LootAll and batch stop. These checks do not establish who caused a changed state.

## New or remaining defects

**HIGH — contention marker is cleared before replies can normally use it.** `play.rs:3325` invokes `begin_iteration`, which clears contention at `fork_ext/play.rs:367-368` on a game step; server events are processed afterward (`play.rs:3365-3367`, `:3964`), while native outputs are collected later during drawing (`play.rs:9828-9830`; `npcw.rs:474`, `bankplay.rs:216`). A player can queue a loot action while a broker loot action is already sent; the native loop sends it (`npcw.rs:475-485`). If the player's matching response arrives on the next game step, the marker from the prior draw has already been cleared, and the matching corpse decrease can confirm the broker request (`action_broker.rs:1869-1895`) even if the broker's response has not arrived. The batch may proceed on the wrong reply. Track ambiguity across outstanding native transactions until safely resolved; the test needs the real event/draw/next-step ordering.

**MEDIUM — a delayed merchant sale may be marked sent when no packet is sent.** While a broker sale waits behind a player merchant transaction, that transaction may reduce the same inventory stack below the staged count. The retry checks only that the key exists and marks it sent after appending (`fork_ext/play.rs:626-638`), but the native loop sends only if the stack still has at least the requested quantity (`npcw.rs:507-513`). The broker then waits for a reply to a packet that was never sent, eventually reporting unknown instead of refusing the stale request; an unrelated matching reply can also be misattributed. Revalidate quantity before marking sent or report actual native-loop acceptance.

## Verification

- `python tools/run_gates.py --only round --filter round6b_ round6c_ --lock-wait 120`: PASS, 22 passed, 0 failed, 0 ignored. Initial sandbox run could not create the shared cargo lock; the completed run used approved access to the shared lock and target cache.
- Contention negative control: temporarily changed the bank staging read to `take()` before the output/busy check; focused gate FAIL, 0 passed and 1 failed (`round6c_contention_retains_staged_deposit_and_never_confirms_on_player_reply`). Restored with `git checkout -- crates/eqoa/src/fork_ext/play.rs`; file status was clean. The focused test then PASSed (1 passed, 0 failed).
- No live server or owner playtest was performed.
