# Review: run_gates.py streamed progress (fix/run-gates-progress)

**Verdict: APPROVE WITH CHANGES** (no HIGH defects; two MEDIUM hang paths that only occur on odd child behaviour, plus one doc slip).
`python -m unittest tools/test_run_gates.py`: 19 tests OK. No cargo run.

## Confirmed fine
- No pipe deadlock in the normal path: a reader thread drains `proc.stdout` (stderr is merged via `stderr=STDOUT`), the main loop never blocks on the pipe.
- Output is complete: every raw line goes into `chunks`; leftover queue items are drained after `reader.join`; the same `safe_decode` runs once on the joined bytes, so the step log still holds the full output and non-UTF-8 bytes are handled as before. Per-line decode is only used for counting.
- Partial lines / `\r`-only progress: `readline` returns the partial chunk at EOF, nothing is lost; counting is the only thing affected.
- Timeout path: kill tree, `returncode` forced to -1 and message appended (unchanged lines ~216). Exit codes, `step_passed = returncode == 0`, summary pass/fail and SKIP counting are untouched. Ctrl+C: the `BaseException` branch kills the tree and re-raises to the existing `KeyboardInterrupt` handler.
- No progress thread exists; only the daemon reader, which is joined before return, so nothing prints after the summary.
- Stale-lock code (`_clear_if_holder_dead`, `process_alive`) is untouched; only a new read-only `_holder_pid` and a `flush=True` on the wait message were added.

## Findings
1. **MEDIUM** `tools/run_gates.py` (`_stream_subprocess_output`, `proc.wait(timeout=5 if timed_out else None)`): after EOF the wait has no timeout and the deadline is no longer checked. A child that closes stdout and then keeps running (or a wrapper that exits its pipe early) is never killed; the old `communicate(timeout=)` covered this. Fix: `proc.wait(timeout=max(0.1, deadline - time.monotonic()))`, on `TimeoutExpired` set `timed_out = True`, kill the tree, wait 5 s.
2. **MEDIUM** same function, `proc.stdout.close()` after `reader.join(timeout=2)`: after a timeout kill, if an orphaned grandchild still holds the pipe, the reader thread stays blocked in `readline` holding the BufferedReader lock and `close()` from the main thread can block on it, hanging the gate run where the old code would at worst hang in `communicate`. Fix: only close if `not reader.is_alive()` (daemon thread can be left behind).
3. **LOW-MEDIUM** `_reader` swallows any exception and enqueues `None`; the main loop then treats it as EOF and does `proc.wait()` with no timeout while the child may be blocked on a full pipe: deadlock. Fix: on reader failure, kill the tree and report failure (or fall through to finding 1's bounded wait).
4. **LOW** `docs/DEV-TOOLS.md`: the "Progress Output" section is also pasted under section 2 (`update_bundled_manifest.py`), which does not stream anything. Remove the second copy.
5. **LOW** (test gap): no test covers a timeout while streaming or a child that exits with non-zero code; findings 1-3 would be caught by a timeout test with a child that closes stdout then sleeps.
