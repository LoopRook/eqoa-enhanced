# Round 7 Part B: generated mod API reference

Branch: `feat/eqoa-api-r7b`, based on develop `9ea6b435`.

## Sonnet review follow-up (2026-11-18)

Requested changes are being verified; the original delivery record below is
historical. `develop` (including Part A merge `8301b288`) merged cleanly as
`b2144f62`. Both generated references will be rebuilt from this merged code.

The production texture bank requires a device, queue and renderer. Avoiding
those would require changing the production service rather than using its real
install path, so this fix uses the approved GPU-test skip convention instead.
Adapter and device acquisition failures have a distinct error type. Only that
type prints `SKIP` in the GPU-dependent tests; Lua/discovery/install failures
still fail. The CLI still returns exit 1 and writes no incomplete reference.
The command-output test now owns its temporary directory through `Scratch`,
including early skips and assertion unwinding. DEV-TOOLS identifies `--out` as
the reliable export and explains terminal-only stdout visibility.

Test-first no-adapter/cleanup run:
`python tools/run_gates.py --only round --filter round7b_without_adapter round7b_scratch_cleans`
reported **1 passed, 1 failed, 0 ignored** (225.28 seconds): cleanup passed; the
real empty-backend adapter request failed at the old unwrap, proving the CI
failure. The corrected focused run including both command tests reported
**4 passed, 0 failed, 0 ignored** (343.57 seconds). Its one printed `SKIP` is
the deliberate empty-backend regression test, which asserts the skip result;
the command-output test used the real available GPU and completed both exports.
The merged static-CRT release build at `db67e26c` passed (223.17 seconds).
Both DEV-TOOLS `--format ... --out ...` exports were regenerated from that
executable. The terminal checks again passed four exports and both expected
exit-1 refusals, with an empty working directory, scratch config and nonexistent
disc path. The merged registry now has **1,723 descriptors**: 1,623 supported,
50 not_yet, 27 partial and 23 unsupported. Generation metadata records
`db67e26ceb48a2737fd86a443fdb1a10a8e3136c`, Windows x86_64, client 0.5.3.
The regenerated baseline `--only round --filter round7b_` passed **5 tests,
0 failed, 0 ignored** (221.14 seconds). The only printed SKIP was the deliberate
empty-backend test; the real-GPU drift comparison passed on the merged registry.
Review negative controls were then executed together with
`python tools/run_gates.py --only round --filter round7b_support_table_matches round7b_scratch_cleans`:

- Removing the actual `eqoa.log.write` bind block from `native::install` made
  the drift equality assertion fail, printing both regeneration commands.
- Temporarily suppressing `Scratch::drop` for the `partial-output` fixture made
  the unwind test fail with `scratch directory survived unwinding`. The test
  removed its deliberately leaked fixture before reporting failure.

Result: **0 passed, 2 failed, 0 ignored, 0 SKIP lines**, 1177.12 seconds
(including waiting for the shared Cargo lock). Both files were restored byte for
byte in `finally`; git confirms no remaining source diff. Thus GPU skip handling
did not hide a real registry mismatch. The full restored release gates remain
to be recorded here.

## Original delivery record

**Initial Part B delivery completed; review follow-up is recorded above.**
The full release gates finished **ALL GATES PASSED** at 01:47:33 local time on
2026-11-18, covering HEAD `69c2d41f`. The last code commit is `95d1066b`
(01:00:21); HEAD's generated-document refresh is at 01:03:12. No source or
generated reference changed after the successful run. Only this report is
updated afterward, so the full gates were not repeated.

## Scope and boundaries

Implements Part B of `docs/round-7-audit:docs/plans/eqoa-api/round-7-plan.md`.
The owner approved one dispatch line in upstream `main.rs` (question 6).
Part A and round 6.5 are separate, unmerged branches: this reference describes
this branch's installed registry. Part C must regenerate it after integration.
No registry claims are corrected here, and the surface is not frozen yet.

The new tooling source is a child of `addons`, declared with a path attribute in
`addons/mod.rs`, and re-exported from `fork_ext/mod.rs`. This gives it access to
the existing private production services without changing their visibility.
The browser keeps an inert dispatcher; no native addon tools are compiled there.

No Part A or 6.5 implementation file is edited. The lead should retain the module
declarations when integrating their branches. The generated files intentionally
need regeneration after either branch changes the capability registry.

Read-only comparison against the concurrent branch tips found **no Part A file
overlap**. Round 6.5 overlaps in `addons/mod.rs` (independent module declarations)
and `docs/fork-footprint.json` (its own hook budgets versus this main dispatch
budget). Preserve both sets of additions when integrating; this branch changes
none of 6.5's budgets or implementation files.

## Changes

- `fork_ext/modtools.rs`: strict command dispatch and option parsing, UTF-8 stdout
  or file output, scratch native manifest discovery, production Host/managed
  visibility/bridge/texture/sound/font installers, public capability-list query,
  stable path ordering, complete JSON and grouped Markdown writers, three tests.
- `fork_ext/addons/mod.rs` and `fork_ext/mod.rs`: module declaration and native
  re-export, plus the browser's inert dispatcher.
- `main.rs`: the approved single line at the start of `run`, before world-service
  installation. Existing main handles errors on stderr with exit code 1.
- `docs/fork-footprint.json`: main budget 6 to 7, approval reason, exact-once hook.
- `docs/DEV-TOOLS.md`: CLI usage, platform/GPU prerequisites, regeneration commands
  and the distinction between API data and generation provenance.
- `SUPPORT-TABLE.md` and `eqoa-1-surface.json`: release-generated references.

The table covers every non-Forge registry entry; Forge is grouped by module,
counts all registered members and lists every entry not marked supported. JSON
retains every descriptor including revisions and children. Unsupported methods
missing from the registry are not invented by the generator. Existing registry
accuracy, event/field claims and DrawList coverage remain Part C's audit.

## Validation

Test-first run: `python tools/run_gates.py --only round --filter round7b_`:
0 passed, 3 failed, 0 ignored, no SKIP lines. Expected failures:

| Test | Failing assertion before implementation |
|---|---|
| `round7b_command_dispatch_and_output` | `Result::unwrap()` got the old `usage: eqoa [boot [server]] ...` error from `main.rs` |
| `round7b_command_refuses_invalid_options` | `["mod-api", "--format", "xml"]: mod-api not implemented` (no proper usage parser) |
| `round7b_support_table_matches_the_running_client` | `Result::unwrap()` got `mod-api not implemented` |

The first implementation compile reported `E0277` because `SoundDiagnostic`
implements Display rather than Error; the tool now converts it to text, matching
the manager's existing error boundary. This was a compile correction, not a test
failure used as a negative control.

The next actual command run had 1 passed / 1 failed: the parser test passed;
the output test caught `invalid module name 'eqoa/1': slashes and colons are
forbidden`. `eqoa/1` is the manifest API declaration; the package loader's public
module is `eqoa`. The scratch entry and capability query now use `require('eqoa')`.

Preflight `--only footprint,paths`: both PASS. `main.rs` has one inserted line,
with its budget explicitly increased from 6 to 7 under the owner's question 6
approval. The named hook is checked exactly once. No other budget grows.

### Release command and generated files

Generated from the Windows static-CRT release executable at code commit
`95d1066b`, client 0.5.3, 2026-11-18 UTC.
The release build passed. A private copy of that executable was used so another
worktree could not overwrite it between exports.

The terminal check ran from an empty scratch directory with `EQOA_ISO` pointing
to a nonexistent file and a scratch `EQOA_CONFIG`. Both file exports and both
stdout exports (default Markdown and explicit JSON) returned 0. JSON stdout
equals the saved JSON; default Markdown stdout equals the saved Markdown.
`--format xml` and writing `--out` to a directory each returned 1 with an error
on stderr. No disc, login, game window or addon entry execution was needed.

The JSON contains **1,686 descriptors**: 1,596 supported, 54 not_yet, 26 partial,
10 unsupported. These are this branch's claims, not new claims of implementation.

### Executed negative controls

After all three tests had passed, the following mutations were applied together,
then `python tools/run_gates.py --only round --filter round7b_` was executed:

| Temporary mutation | Test and observed failure |
|---|---|
| Remove the single `main.rs` dispatch hook | `round7b_command_dispatch_and_output`: `unwrap()` received the old `usage: eqoa [boot [server]] ...` error |
| Accept `--format xml` with success | `round7b_command_refuses_invalid_options`: `unwrap_err()` received `Ok(())` |
| Remove the actual `eqoa.log.write` bind block from `native::install` | `round7b_support_table_matches_the_running_client`: equality assertion failed and printed both exact regeneration commands |

**0 passed, 3 failed, 0 ignored, 0 SKIP lines**, 89.70 seconds. The temporary
edits were restored in a `finally` block, byte for byte; `git diff --exit-code`
for all three source files then passed. The native file has no committed edit.
An earlier mutation-helper attempt used the Windows default text encoding and
failed to compile main.rs; it was restored and is explicitly **not counted** as
a negative-control result. The successful attempt wrote UTF-8 explicitly.

### Final gates

Command: `python tools/run_gates.py --release --filter round7b_`, with the
shared-target safeguard described below, the shared Cargo lock/cache, the owned
Frontiers disc at `run/eqoa.iso`, and the runner's scratch configuration.

| Gate | Result | Passed | Failed | Ignored | Printed SKIP lines | Seconds |
|---|---|---:|---:|---:|---:|---:|
| round | PASS | 3 | 0 | 0 | 0 | 91.84 |
| addons | PASS | 498 | 0 | 25 | 9 (9 distinct) | 425.12 |
| full | PASS | 2379 | 0 | 30 | 50 (30 distinct) | 1918.48 |
| wasm | PASS | | | | | 34.39 |
| footprint | PASS | | | | | 2.31 |
| paths | PASS | | | | | 4.31 |
| player-release | PASS | | | | | 47.50 |
| release | PASS | | | | | 136.61 |

Counts and timings are copied from the gate summary. Printed SKIPs can return
success and therefore appear among passed tests; they are not claimed as
fixture coverage. Addon skips cover four symlink checks lacking Windows
privilege, two pinned Forge fixture checks lacking `EQOA_FORGE_SPIKE`, and
three bundled UI picture comparisons lacking reviewed `EQOA_BUNDLED_UI_GOLDENS`.
The full run repeats those and skips fixture branches requiring `run/client.bin`,
a live save state, live/July/local-server S2 assets, the 2002 disc, or server-world
art. The 50 printed lines include repeated missing-S2 reports; the summary lists
30 distinct lines. Ignored tests are separate opt-in tests, not counted as run.

Existing bundled load/lifecycle and zero-manager-diagnostic tests passed. The
existing zero-addon frame/packet allocation-bound test ran and logged zero menu
frame allocations. The reviewed golden-image comparisons did not run, as noted
above. No live owner playtest or external decomp/oracle evidence is claimed;
this gate runs the eqoa binary tests, not the separate oracle workspace suite.

Coverage was checked against git timestamps and a clean `git diff 69c2d41f`.
Round, wasm and release logs show compilation from this worktree. The round log
executes the three Part B tests, with 2,456 filtered out, matching this branch's
2,459-test binary rather than the other branch's cached 2,475-test binary.
The final footprint passes with main at +7/-0 and its new hook occurring once.
Path checks, rustfmt on the new source, and the complete branch whitespace check
pass. There is no worktree-local `target` directory. No existing test was
deleted, skipped, or weakened by this change; nothing was pushed, merged or tagged.

No Part B implementation remains outstanding. Part C must regenerate both
references after merging the other API branches and correcting registry claims.

### Earlier verification and corrections

The first launch was cancelled before completing the round step to remove
Markdown hard-break trailing spaces and the final extra blank line found by
`git diff --check`. The writer now uses a metadata list and exactly one final
newline; both files were regenerated from the corrected release executable.

Command tests after the loader correction:
`python tools/run_gates.py --only round --filter round7b_command`: **2 passed,
0 failed, 0 ignored, 0 SKIP lines** (44.09 seconds). This is an actual current-tree
compile using the cache mitigation below.

Visual text inspection of the first generated table found `eqoa.Color` grouped
as a module. A test assertion was added before correcting the classifier:
`--only round --filter round7b_command_dispatch` failed (0 passed, 1 failed,
0 ignored, 0 SKIP) with `root record fields must be in the record section`.
The classifier now recognizes uppercase type segments directly under `eqoa`
as well as under modules; aliases remain in their own section.

### Shared target cache caveat

A queued command-test rerun returned 0 passed / 0 failed, with 2,475 tests
filtered out. Its log showed Cargo finished in 0.71 seconds and reused the
executable another worktree had just built. This is **not counted as validation**.
The initial red run had 2,459 total tests, including all three new tests.

For subsequent runs, a scratch Python wrapper (then a scratch `sitecustomize`
audit hook for the exact standard command) refreshes `main.rs`'s modification
time immediately before Cargo is spawned, after the runner acquires its lock.
It changes no source bytes, compiler flags or target directory. This forces
Cargo to compile this worktree's eqoa crate rather than reuse another branch's
newer binary. All Cargo operations still use the runner and shared cache/lock.
Reviewers sharing this target directory should force a current-worktree compile
inside the lock and check that the expected tests actually execute.

### Final release gates after review follow-ups (2026-11-18)

Command: `python tools/run_gates.py --release` on c07f6b3d (full run, no filter).
Result: **ALL GATES PASSED**; wasm, footprint, paths and player-release PASS;
release PASS (3544.65 s), executable in the shared target. The `--out` note in
`docs/DEV-TOOLS.md` and the scratch-directory cleanup (with its unwinding test)
were already in place. This machine has no graphics adapter, so the mod-api
reference tests print an explicit SKIP (`round7b_without_adapter...`); other
SKIPs are missing discs/client.bin/live S2, symlink privilege and UI goldens.
Oracle tests skip here. An earlier attempt was cut off by the tool time limit and
is not counted.
