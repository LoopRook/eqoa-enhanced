# Round 4 independent check: performance deltas

**Verdict: PASS WITH CHANGES.** The reviewed implementation retains owned event snapshots and the stat memo covers the inputs read by the calculation; no definite stale-value regression was found. The requested performance target remains unmet and important behavioral guarantees need regression coverage before acceptance. The addon suite was not completed in this check.

## HIGH

- `docs/plans/eqoa-api/perf-deltas-report.md:7`: The report itself states that the seven-mod overhead exceeds the brief's 2 ms goal. In the described busy scene, a player still has higher overhead than the agreed target. The report identifies this accurately; meeting the goal needs further work or an explicit decision to accept the shortfall. No performance measurement was validated in this review.

## MEDIUM

- `crates/eqoa/src/fork_ext/addons/perf_delta_event_tests.rs:63`: The comparison checks only the type of userdata and tables, and line 80 checks `next`, not `pairs` or `ipairs`. The test does not check reference equality/resolution, second-handler or second-VM reads, or an unread field of an event retained across frames after the entity changes. For example, a future implementation that creates a reference to a different incarnation could still pass the userdata type comparison. Add direct eager-versus-lazy assertions for these cases, including iteration and nested read-only errors. The ten sample shapes are useful, but do not establish every event kind or every possible field.
- `crates/eqoa/src/fork_ext/addons/perf_delta_tests.rs:86`: The changed-fields test compares with the old walk but has no independent negative-control mutation. The event test's different-payload control checks its comparator, not that reverting lazy delivery makes the test fail. The requested local revert-and-restore spot check was not performed; consequently the report's negative-control claims were not independently confirmed. No source was modified for this review.

## LOW

- `docs/plans/eqoa-api/perf-deltas-report.md:46`: The comparison supports equality for sampled scalar paths, but saying it proves the same values for references and all ten shapes overstates its assertions: userdata are compared by type only. A wrong reference key could pass. Qualify the claim until reference semantics and the remaining cases are checked.

## Code assessment

`Event.data` is an `Arc` of owned JSON captured by emit; lazy closures retain that snapshot, not a live entity view. The reference constructor uses captured kind, key, generation and incarnation; resolving a reference consults live state, as before. Each delivery creates its own Lua envelope and caches fields in that VM. This supports later-frame and multi-handler snapshot stability by inspection, but those cases were not independently exercised. Ordinary records are guarded proxies, and arrays remain ordinary Lua tables; the new test does not establish all iteration or equality behavior. The memo key includes stat totals and bases, masteries, bonuses, cap, and each worn item's stat IDs and values, matching `view` and `stat_total`; no omitted input was found. The one-line adjustment at `crates/eqoa/src/fork_ext/addons/native_tests.rs:1324` legitimately warms fields newly built on demand before asserting no further builds; it preserves the repeated-read assertion, while no longer asserting those fields were already built during delivery.

## Checks performed

- Five new tests: 5 passed, 0 failed, 0 ignored (four addon delta tests and the stat memo test, single-threaded; disc and scratch configuration set).
- Addon suite: **incomplete**. The first run was interrupted after 246 passing test lines, 5 ignored lines and 7 lines containing SKIP; a second interrupted run had 245 passing test lines, 4 ignored lines and 7 SKIP lines. These are partial log counts, not suite totals. Both runs unexpectedly reached timing tests; they were stopped, and their measurements are not used here. The second run excluded identified benchmarks, but another timing test was reached. The builder's earlier 503/0/25 count cannot be independently confirmed by these runs.
- Fork footprint against the configured reviewed base: PASS (455 fork-owned, 31 shared). No-local-paths check: PASS. The count was reported before this review document was written.
- No full client suite, benchmark rerun, wasm check or release build was requested for this review. Cargo ran with four build jobs, the shared target and the build lock, but an explicit timeout was not configured. The lock was removed after the runs. No source mutation negative control was run.
- The builder's report correctly acknowledges the missed performance target and describes its own full-suite and skip counts; those historical results and timing claims were not rerun here. Its addon count predates the final test as stated.

