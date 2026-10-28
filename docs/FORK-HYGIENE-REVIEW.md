# EQOA Enhanced fork hygiene — final independent review

Review date: 2026-10-28
Reviewer scope: read-only source, Git ancestry/scope, policy, documentation and recorded validation. No builds, test suites, source/index/HEAD changes, parser-repository access, or subagent delegation were performed by this reviewer. The only executed check was the read-only footprint checker.

## Verdict

**Spec alignment: PASS for the frozen production change. Quality: approve; no actionable new defects found.**

This verdict applies to the working-tree hygiene delta over merge checkpoint `1bfe228`, considered in the context of the complete enhanced branch against upstream `8d90350`. The root README ownership override and the explicit instruction to disable upstream reporting are binding. The pending final validation prose and final commit/publication bookkeeping remain the coordinating agent's closeout work; they do not change this source verdict.

**Actionable findings: none.** There are no severity-tagged file/line defect findings to report.

## Evidence and review boundaries

Read the owner's Downloads/EQOA Enhanced Fork Hygiene Brief.md first, then FORK.md, docs/FORK-HYGIENE-STATUS.md, docs/FORK-OWNERSHIP.md, docs/COMPANION-LAUNCH.md, docs/UPSTREAM-SYNC.md, docs/DEVELOPMENT.md, docs/LICENSE-OPTIONS.md, the ownership manifest, skip inventory and capture manifests. Reviewed the supplied hygiene-review-package.patch and current working-tree files. Read-only Git access required the existing sandbox escalation workaround; no repository mutation was made.

Independently confirmed:

- `1bfe228c2b8ec514562929b7826a9e6283e3303a` has parents `8aeaf9181f401e4b8b93f92347d407cc81eb4856` and `8d9035096a2deb447854d5fcff4ef25225f80fd1`.
- Local `master` and `origin/master` both resolve to `8d90350`; `enhanced` and the local `looprook/enhanced` remote-tracking ref both resolve to the merge checkpoint. `rerere.enabled` is `true`.
- `fix-animation-oob-panic` resolves to `231f0e9`; the ledger records it as prepared, unsubmitted and unpushed. This reviewer did not query hosting services or repeat the coordinator's remote verification.
- The restored map/quest directories and tracker/groupmark/mapsuggest/statbox/render-map implementations have no differences from `8aeaf91`. The checked incoming admin/fishing/netstat/shadow/clipstack modules have no differences from pinned upstream.
- `python -Xutf8 tools/check_fork_footprint.py` passes against pinned origin/master: **31 shared paths, 12 exact hooks, 108 fork-owned changed paths at review time**. The count includes untracked files. The helper changes no Git state or budgets.

The existing merge/source/capture audit was used for the broad restored map integration. This was not a fresh functional audit of every line of the restored map or a live server playtest.

## Workstream assessment

### A — upstream synchronization

The merge preserves both parents and the reviewed conflict resolutions. The fork retains the upstream options/shadow integration and separate enhanced option IDs. The local branch/ref/rerere state agrees with the execution record. Full upstream ownership is not claimed for the root README, in accordance with the owner override. No evidence in this delta indicates a master release push, stash destruction or history rewriting.

### B — documentation and ownership

The enhanced landing page is concise and links development/reference material instead of copying the upstream reference ledger. CLAUDE.md is enforced as upstream text plus the single scope override. LICENSE and upstream UPDATE-NOTES.md are immutable in the checker; fork notes are separate. The checker also compares package/workspace Cargo license fields, so a dependency-file budget does not silently permit a license change. Existing MIT/OFL notices remain represented. The owner has not selected new licensing terms.

FORK.md and docs/FORK-OWNERSHIP.md describe the actual adapter layout, the inherited shared-file footprint, conflict handling and later isolation candidates. At review time the status ledger still contained a Validation in progress section; the coordinator explicitly identified final result transcription as pending. Final handoff should replace that section with the already completed results and named-skip link.

### C — extension boundary and behavior

Verified actual shared-parent execution, not only the textual hook counter:

- `crates/eqoa/src/play.rs:5196` calls the combat adapter before the no-damage UI early return, preserving event coverage. The adapter retains self/pet/other attribution, the `ts`, `actor`, `target`, `amount`, `ability`, and `kind` contract, and positive heal amounts. JSON escaping and `unsigned_abs` remove malformed-name and minimum-signed-amount hazards. Browser and unit-test paths do not emit native telemetry.
- `crates/eqoa/src/hud.rs:328` calls the label adapter after the stock target gauges. The child restores anchor/vertical state and explicitly uses top alignment for self gauges and bottom alignment for target HP. Centers, scales, colors and setting gates agree with the removed implementation, including the target gauge's extra 25-pixel title offset.
- `crates/eqoa/src/options.rs:718`, `:761`, `:1184`, `:1232` and `:1295` connect change, activation, category, item registration and value text. The existing UI flow remains reachable and IDs remain 1048–1052.
- `crates/eqoa/src/settings.rs:957`, `:1069` and `:1483` preserve the existing setting keys, default omission format, parsing semantics and diagnostic wording.

The common service modules accept plain event/configuration data. App-private child adapters intentionally retain access to their parents' private state; this is the brief's allowed self-contained module-directory alternative, not an unfulfilled requirement to create a separate crate. A new unused frame-tick hook was not invented.

### D — portable companion launch

Rust and Python launch paths read command/args from mod.json, use EQOA_PARSER_DIR for the working directory, resolve explicit relative commands beside the manifest and bare commands through local/PATH lookup, and reject malformed or disabled manifests. The default module is consistently `eqoa_parser`. Windows process launch keeps CREATE_NO_WINDOW. Native UDP/process code is cfg-gated from wasm; the actual browser build succeeded.

The desktop launcher retains a separate plain PLAY EQOA action. A missing/disabled parser preventing the explicit Play with Combat Parser action is therefore appropriate failure reporting, not a regression that prevents ordinary play. The running label deliberately tracks only the native client's own child; this limitation is documented. Native launch errors go to the client log, and the Python launcher exposes an actionable error dialog.

### Reporting override

`crates/eqoa/src/report.rs:57`, `:1145` and `:1249`, plus `hud.rs:367`, disable normal UI/API/F3/crash-offer entry points. The transport itself resolves through report_policy and accepts only an explicit numeric HTTP loopback fixture. It disables proxy use and redirect following before posting. Upstream's historical endpoint is test-only reference data.

The browser policy requires explicit same-origin HTTP loopback selection. `web/report.js` guards direct post/open entry points, hides the gate button, omits send actions from failure/previous-session notices, and retains scrubbed local diagnostics. Fetch uses redirect:error and credentials:omit. The Node harness exercises disabled F3/API/crash/previous-session paths and an enabled fixture control. No newly enabled path to the upstream report service was found.

### E — restored features and footprint enforcement

The inherited restored integrations are documented and retain their intentionally larger budgets. The checker uses an explicit shared-file list with independent insertion/deletion maxima; it includes working-tree, staged and untracked changes, rejects shared binaries and unknown paths, pins the upstream base, verifies the exact CLAUDE exception, and counts named seams. Its tests cover real temporary repositories and both permitted changes and policy failures. Textual hook counting is explicitly documented as a limited guard; source review and compilation remain necessary.

The later map crate/feature-flag isolation remains a separate decision. The final capture manifest records byte-identical minimap, mob-hover and spawn-search cases against the merged baseline. This supports preservation of those deterministic cases, not all live-world behavior.

### F — animation semantics

The new HSprite changes are narrowly scoped: `set_play_frame` returns on nonpositive frame counts; frame advancement requires a positive count while clock/blend progression remains intact. Existing signed clock/frame behavior and upstream node_frame nearest-end sampling remain unchanged. The tests include nonempty positive controls, empty/negative counts and both playback directions.

The enhanced branch's prior Anim::frame fallback and character clip guards are retained. Added character tests cover missing/out-of-range clips with and without an existing play. The separate upstream branch is prepared for owner review without submitting it. This review does not claim all possible malformed asset/layer combinations are now comprehensively handled.

## Validation assessed

The coordinator supplied successful completion status and the reviewer read final log tails and the source-aware skip record:

- Final native release build: successful Finished release result in hygiene-native-final.log; seven compiler warnings remain, not errors.
- Actual web/build.sh: coordinator reports exit 0; final asset listing includes report-policy.js in the completed web output.
- Full locked workspace: exit 0; Cargo totals **1,224 passed, 0 failed, 2 ignored** across 91 summaries.
- docs/FORK-TEST-SKIPS.md identifies **168 Cargo-ok results with fixture checks omitted**, leaving **1,056 without an identified fixture omission**. Those omitted cases must not be represented as successful disc/oracle comparisons. The two explicitly ignored FX benchmarks are separate.
- Supplied focused results: HUD 7; combined adapter/service/HUD 13; options 15; settings 39; attack timing 1; Python parser real-process tests 9; Node report policy/browser harness passed. These overlap the full suite where applicable and must not be added to its total.
- Final deterministic native map captures: three cases match the merged baseline byte for byte, recorded in docs/upstream-sync-captures/hygiene-manifest.json.

No test/build was rerun by this reviewer. The read-only footprint command was the sole verification execution.

## Considered and set aside

- Returning README to upstream text: rejected because the owner's later enhanced-ownership ruling supersedes the original brief.
- Restoring positive modulo/clamping for valid negative animation clocks: rejected because upstream intentionally samples the nearest clip endpoint, and this pass preserves that behavior.
- Applying the GM Goto/Buff PC keyboard proposal: outside the hygiene scope; retained as an unapplied patch, preserving current upstream gameplay.
- Full Forge map parity, map/theme rewrite and crate/feature-flag isolation: explicitly deferred work, not defects introduced by this refactor.
- Parser-side protocol expansion, roster data and companion packaging: separate repository/design work. The current wire contract and launch boundary are retained.
- Detecting independently launched parser processes: not reliable from the native child handle; the documented Running indicator is intentionally narrower.
- Repointing the updater immediately: not required for local no-channel builds. It remains a prerequisite before public fork CI releases, as documented.
- Choosing a new license or submitting the prepared animation branch: owner decisions, not actions authorized by this cleanup.
- Literal zero edits outside hook calls: reporting guards, regression coverage and the animation fix are explicit authorized exceptions; broad restored integrations are workstream E's preserved baseline.

Final handoff should record this review, finish the validation prose, and include the named fixture omissions. No source correction is requested by this review.
