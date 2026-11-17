# Round 6 B/C independent review

**Verdict: REJECT**

## HIGH

1. **One physical press can authorize multiple actions or batches.** `crates/eqoa/src/fork_ext/addons/native_events.rs:604` iterates subscribers and `:653-659` creates a fresh armed gesture for each listener to the same `pointer.pressed` event. A mod registers two listeners over its claimed rectangle; one click gives each a fresh token, and each can enqueue an action (or start a batch). Consumption must belong to the physical event, not an individual callback. The existing one-action test exercises two calls in one scope, not two subscribers.

2. **Transaction actions bypass the required native player path.** `crates/eqoa/src/fork_ext/play.rs:1304`, `:1409`, `:1465`, `:1537`, and `:1574` build and send loot, buy, sell, bank item and money packets directly; `:1414`, `:1470`, `:1543`, and `:1577` also write native pending state. Loot-all directly sends an opcode at `:1344`. This explicitly violates the owner brief and spec §8, which require the native output consumers and prohibit copied packet logic and direct state writes. For example a merchant transaction can diverge from native validations or pending handling as that consumer changes. Leave these actions not_yet until an approved native consumer seam exists.

3. **Unrelated replies and window closure can be reported as successful transactions.** `crates/eqoa/src/fork_ext/play.rs:76,118-120` treats any inventory-add event as confirmation of an outstanding buy, without correlating an item or transaction; `:134-139` reports a loot close as a successful loot reply. `crates/eqoa/src/fork_ext/addons/action_broker.rs:1532-1538` ignores the stored loot key. For example an unrelated inventory addition while a purchase is pending can advance a sell/buy batch despite no merchant confirmation; closing a corpse while loot is pending can count an unconfirmed step as completed. Ambiguous responses must remain unknown and must not advance the batch.

## MEDIUM

4. **Frame-mode widget actions never receive a gesture scope.** `crates/eqoa/src/fork_ext/addons/manager.rs:314-316` executes the frame entry directly; the only production `enter_render_scope` call is in the event subscriber path at `native_events.rs:645-646`. Default frame-mode and Forge addons draw in their entry, not a render event. A physical click on their button calls `arm_imgui_click` without an active render scope and an action returns `needs_player_action`. Cover the actual manager/Forge frame-mode path.

## Scope and evidence

The reference `frontiers_forge` bank, inventory, loot and merchant modules contain observation functions but no transaction functions; `items_forge.rs:11` still says transactions are unbuilt. I did not find newly invented Forge transaction functions, but the builder report's assertion that Forge transaction wiring was implemented is unsupported. The bag viewer remains disabled by default; the selected tests pass, but the transaction and confirmation defects above make its batch operations unsafe to approve. This review does not certify every native check, lifecycle edge, widget input source, visual capture, or live server behavior.

Existing tests changed relative to develop:

- `native_tests.rs::round_3_stage_a_actions_assist_and_summary_api`: gesture setup and request ID replace the legacy queue assertions. **Weakened:** no longer verifies that the accepted assist reaches the old queue; the separate new Play test checks dispatch and reply, but this specific test only checks acceptance.
- `native_tests.rs::failed_option_saves_leave_lua_and_controls_values_unchanged_and_diagnose`: calls moved to key handlers, plus Controls re-registration after reset. No evident weakening of the original persistence and permission checks.
- `native_tests.rs::option_and_bind_names_never_redirect_caller_identity`: permission check moved to a key handler. No evident weakening.
- `round_3_picture_tests.rs::assert_mod_click`: assist assertion changes from legacy queue flag to broker queue count; reset added. **Weakened:** count alone does not establish the queued request is assist, although it checks that a request was queued by the click.
- `api_capture_tests.rs::addon_actions_use_live_camera_assist_and_summary_overlay_gates`: legacy 16-request queue assertion replaced by gesture, 32-request limit and first-tick count. **Weakened:** the old one-request-per-success drain assertion is gone; the replacement only counts eight first-tick sends and does not verify all 32 are eventually dispatched once.
- `bag_viewer_tests.rs` and `items_tests.rs`: additions to existing files; no existing test assertions removed in the diff.

## Test reruns

- `python tools/run_gates.py --only round --filter round6b_ round6c_ --lock-wait 120`: PASS, 16 passed, 0 failed, 0 ignored. This is the requested filtered round gate, not a fresh full workspace, release, GPU acceptance, or live server run.
- Negative control 1: changed the active-batch busy predicate to always false, then ran `python tools/run_gates.py --only round --filter round6c_ --lock-wait 120`: FAIL, 3 passed, 1 failed, specifically `round6c_batch_gesture_single_batch_refuses_second_serial_execution_and_capabilities`. Restored with `git checkout -- crates/eqoa/src/fork_ext/addons/action_broker.rs`; status clean for that file.
- Negative control 2: changed the grey-rule initial and load-time false defaults to true, then ran `python tools/run_gates.py --only round --filter round6c_bag_viewer_ --lock-wait 120`: FAIL, 0 passed, 2 failed (junk persistence/default and merchant batch tests). Restored with `git checkout -- bundled-mods/eqoa_bag_viewer/modules/bags.lua`; status clean for that file. An initial edit of only the initial field passed because loading resets it to false; this was corrected in the control.
- Initial sandbox run could not create the shared lock and was interrupted; the actual gate runs used approved access to the shared lock and cache. No owner playtest was performed.
