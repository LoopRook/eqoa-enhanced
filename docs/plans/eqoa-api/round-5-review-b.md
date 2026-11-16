# APPROVE WITH CHANGES

## MEDIUM

- `crates/eqoa/src/fork_ext/items_capture.rs:199` and `:217`: `source` is inferred from the current target when 0x0018 arrives, not from the corpse named in the request. If the player requests corpse A, then targets named corpse B before the response, A's loot is attributed to B; another 0x0018 can likewise overwrite the source with the current target. The packet contains only a list, so a named corpse at response time is not evidence that it supplied that list. Omit the source when it cannot be correlated reliably, or track the actual request with a reliable association. Add a protocol-shaped regression for a target change before the reply and for a repeated 0x0018. The existing test changes target after the reply, which does not cover this race. The API documentation and report should not claim this inference is never a guess.

## Review notes

The mod is disabled and unbound by default, supplies the six requested tabs, pack and bank views, hover card, and event-fed bounded loot log. It is read-only. The window subscribes to rendering and inventory changes only while open, and its grid is rebuilt on opening, tab selection, or change; the loot handler remains active while closed. `Item.tabs` calls the same four predicates used by `itemmenu.rs`; the API fields and capability rows are present. The four package files are in both release lists and their SHA-256 values match `SOURCE.txt`. The diff makes no change to `docs/fork-footprint.json` and adds no lines to non-fork-owned files.

Tests exercise the real Manager and protocol-shaped handlers, with zero diagnostics asserted by the session fixture. The report describes negative controls; I did not rerun mutations. GPU and disc-dependent tests can return early with a SKIP message, and ordinary captured test output does not establish whether those paths ran in this rerun.

## Changed pre-existing tests

- `crates/eqoa/src/fork_ext/addons/round_minimap_manager_tests.rs`: bundled package count changes from 11 to 12 to include the new package; does not weaken the test.
- `crates/eqoa/src/fork_ext/bundled.rs`: five expected package lists include the new package; does not weaken the tests.
- `crates/eqoa/src/fork_ext/items_tests.rs`: fixture `Spec` gains icon and tab predicate fields with defaults matching the previous serialized values, and new fixture helpers are added; no pre-existing assertion removed or weakened.
- `tools/test_player_release.py`: adds four package paths to the expected release list; does not weaken the test.
- `crates/eqoa/src/fork_ext/addons/manager.rs`: includes the new test module; no pre-existing test weakened. `bag_viewer_tests.rs` is new, not a changed pre-existing test. No pre-existing test was deleted.

## Rerun

Prescribed command with shared target and lock: **32 passed, 0 failed, 0 ignored, 2357 filtered out**. No SKIP lines appeared in captured output; skips printed inside passing tests may be captured, so the number of actually executed GPU/disc paths is unverified. No other suite was rerun.
