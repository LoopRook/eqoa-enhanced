# Round 7 Part B: generated mod API reference

Branch: `feat/eqoa-api-r7b`, based on develop `9ea6b435`.

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
`955fe5a84c5432c19dc6a5e01890f98e6a99e9df`, client 0.5.3, 2026-11-18 UTC.
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

The full standard release gate run is pending; results will be appended after it.

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
