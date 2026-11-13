# Upstream sync ? 2026-11-11

Prepared on `sync/2026-11-11` in `<repo:eqoa-sync-2026-11-11>`.
Starting develop: `cbcf86b`. Pinned upstream: `a3cedec7d8bb2688cf8ed56d34a280f4a459b0ef`
(41 commits since reviewed base `5d54c0c`). Merge: `3419bce`; tested adapter fixes:
`7f13540`. Full upstream ancestry retained. Main checkout stays on develop;
no push, protected-branch merge, tag or release was made. Independent lead review
of the hand resolutions and new budget, and owner live QA, remain pending.

## Hand resolutions

| File | Kept from upstream | Kept from Enhanced / why |
|---|---|---|
| `logfile.rs` | Panic persistence through `keep_crash`, native crash lifecycle, logging and tests. | Existing `crate::report::crash` call also runs, preserving the local crash-save offer. |
| `map/chrome.rs` | Complete places list, Where I am, places-only legend and controls, quest/group/travel structure. | Rebound and wrapped keyboard hints through owned helpers in `fork_ext/map.rs`; one final overlay hook replaces only normal keyboard hints. Suggest addresses the final entry, preserving the new P Places entry. Travel remains named once. |
| `map/mini.rs` | New `half` state, indoor zoom/picture/quads, ceiling-cut integration and upstream quest rules/tests. | Circle/mask/border/hits and existing search/journal/guidance stay; defaults initialize both circle fields and half. Indoor pictures already enter the same map layer drawn through the circle mask. |
| `map/mod.rs` | `indoor` and `places`, new map state and routing. | `journal` and existing Enhanced search/guidance hooks remain alongside them. |
| `play.rs` | Automatic field-wise initializer merge; event sequence increment, Q-tap mob information, item links, `open_chat_line`, and upstream `name_color` (including mercenary tags). | Existing event hook follows the sequence increment; native chat visibility gates the upstream chat-opening call; `Dot::of` receives upstream's name color while retaining creature hover cards. |
| `viewer.rs` | Indoor ceiling probe before drawing, plus non-conflicting upstream crash/render behavior. | Existing addon overlay preparation runs beside the probe; report/addon hooks remain. |
| `net/session.rs` | Crash report sender, 0x7E64?0x7E66, all upstream limits/UTF-8 truncation/ack handling and new tests. | Existing power/quest/shard protocol behavior remains. No fork crash encoding was added to net. |

`map/indoor.rs`, `map/places.rs`, `itemlink.rs` and `linkplay.rs` match the pinned
upstream files after newline normalization. Indoor ambient/sun lighting,
`EQOA_MAP_TRACE`, ceiling probes and quest rules were not rewritten.

Additional fork adapter integration: `fork_ext/options.rs` now copies upstream's
`CATEGORIES` slice and derives its size, including the new Chat page and browser's
existing Textures exclusion. Enhanced HUD and Mods & Addons still follow every
upstream page. Only the fork fixture's expected predecessor/indexes changed.

## Crash identity and F3 finding

One hook at `crashfile::send_pending` calls fork-owned
`fork_ext::report_label::pending`. It prefixes every outgoing kept report with
`crate::fork_ext::report::enhanced_label()`: current native reports, browser reports
and the legacy browser last-crash fallback all cross this boundary. A server whose
byte limit cannot fit the complete identity gets no report; the original stays
kept. Numbers, source text, persistence, upstream wire limits, UTF-8 cutting and
acknowledgements stay upstream's. The existing label function moved to a common
owned module; its native path is re-exported unchanged and the browser exposes the
same path. Local ZIP reports and the existing local crash offer remain in place.
`FORK.md` and the fork update notes describe the authorized server-report path.

**F3's server Support petitions do not currently carry an Enhanced label.**
`Play::send_bug_note` sends `bugnote::compose`, whose build line is `dev-<sha>`,
`page <build>` or `local`; neither that note nor the upstream petition route inserts
`enhanced_label`. The local-save form does identify Enhanced. F3 and its server
petition behavior were left unchanged, as requested; `CLAUDE.md` was not edited.

## Footprint and source audit

`reviewed_base` advanced only after the script's footprint gate passed, to
`a3cedec7d8bb2688cf8ed56d34a280f4a459b0ef`.

| Shared budget | Before | After | Reason |
|---|---|---|---|
| `crates/eqoa/src/crashfile.rs` | Unlisted / +0 ?0 | +1 ?1 | The single send-boundary identity hook; implementation and tests are fork-owned. Registered as an exactly-once footprint hook. |

No existing budget increased, including map files. Measured shared changes fell
**5,815 ? 5,704 lines** (additions + deletions against each snapshot's reviewed
base). Chrome is +195/?12 within +304/?19; mini is +273/?14 within +313/?44,
including four lines loading the owned GPU regression module.

All **16 newly incoming upstream tests in resolved/hooked files remain verbatim**.
Inherited differences in chrome's three fork-aware layout/legend assertions,
Play's two fork-aware fixtures, and the already-replaced unanswered-power test in
net remain inherited; this sync removes or weakens no new upstream test.

## Verification

Every Cargo invocation ran in the foreground behind the atomic shared mkdir lock
`%USERPROFILE%/Paseo/cargo-build.lock`, with a bounded lock wait and 3,600-second
command timeout. All used `CARGO_TARGET_DIR=%USERPROFILE%/Paseo/targets/shared`.
The script ran client/net and Python tests serially through
`--continue --tests-only`, with its scratch configuration and one test thread.
The required addon run used a separate scratch EQOA_CONFIG and one thread.

| Suite | Result | Passed | Failed | Ignored | SKIP messages |
|---|---|---:|---:|---:|---:|
| Script: eqoa binary | PASS | 2036 | 0 | 31 | 208 |
| Script: net library | PASS | 92 | 0 | 0 | 0 |
| Script: net dump_capture | PASS | 1 | 0 | 0 | 1 |
| Script: net effects | PASS | 4 | 0 | 0 | 0 |
| Script: net newchar | PASS | 3 | 0 | 0 | 0 |
| Script: net selfstate_capture | PASS | 1 | 0 | 0 | 1 |
| Script: net doc tests | PASS | 0 | 0 | 0 | 0 |
| Script: Python tools | PASS | 97 | 0 | 0 | 0 |
| Separate addon suite | PASS | 374 | 0 | 24 | 9 |
| WASM cargo check | PASS | n/a | 0 | n/a | 0 |

Reported Rust passes include prerequisite SKIP returns; they are not all exercised
assertions. Missing ISO/module/oracle/capture data, optional Forge fixtures,
symlink privileges and unapproved picture goldens remain explicit skips. Addon
SKIPs: four symlink-privilege, two optional Forge-spike and three picture-golden
prerequisites. Ignored tests remain opt-in. Per-invocation totals overlap.

The owned indoor-picture regression passed on the GPU at 1?, 2? and 4?: a rendered
texture is visible centrally and clipped at its corner; disabling the circle
reveals that same corner (negative control). This tests the indoor render-target
texture path, not a live cave scene. The owned crash test covers native-shaped,
legacy-page-shaped and empty reports, identity preservation, and short-offer and
empty-pending negative controls. Existing local report/crash tests also passed.

Rework recorded honestly: run 1 stopped at compile errors (missing extracted
`wide` import and outdated fork page count); tools still passed 97. Run 2 had
2,035 passed / 1 failed / 31 ignored in the client, and tools 97 passed: the fork's
fixed legend index overwrote Places. Both adapter errors were corrected. The final
script run passed 2,137 total / 0 failed / 31 ignored, with 210 SKIP messages, and
Python 97. No assertion was relaxed to obtain the pass.

No remaining scoped-gate failure was classified as pre-existing. The render
crate's known PC-only AO/particles16 failures were not invoked by these client/net
commands, and no new untouched-upstream control was needed. A full workspace run,
release build/package and browser runtime play were not requested gates and were
not run. No native indoor scene, live server crash transmission, real panic,
physical controller/touch or owner playtest was claimed.

## Dry-run, upstream rules and handoff

`git merge-tree --write-tree --name-only HEAD origin/master`: exit 0, no conflict
paths, at tested `7f13540` (upstream is already an ancestor). This is the integration
dry-run; develop was not moved. Live QA and independent review are the remaining
handoff steps. Future workspace/release checks, if requested, must use the same
shared lock/target; no build is left running.

The refreshed `docs/upstream/CLAUDE.md` equals upstream at the reviewed base. Its
diff adds feature scope descriptions and two rules worth carrying forward:

- Server worlds: wait for composition only when the server says the selected
  character needs it; reacquire remaining worlds at promotion, and retell an
  answer sent between character pick and game-connection promotion.
- Crash reports: only a server that offers them gets them; retain bounded storage,
  scrub player paths, and let reports go at acknowledgement. Adopt this with the
  Enhanced identity hook and retained local-save behavior, rather than copying
  vanilla's identity/report policy verbatim. The debug panic switch is debug-only.

No changes to our CLAUDE.md were made. Logs, command/exit JSON, source audit and
footprint evidence are archived at
`%LOCALAPPDATA%/Temp/eqoa-sync-2026-11-11-gates`.
The worktree's script-created `target` held only logs/configuration, never Cargo
artifacts; its evidence was archived and that temporary folder removed before
handoff. No private game data or credentials are committed.


## Fix round (review of 2026-11-11: APPROVE WITH CHANGES)

The review is kept as `docs/plans/sync-2026-11-11-review.md`. Commits are on `sync/2026-11-11`
after `e0fc130`; nothing was pushed, merged or tagged.

| Finding | Fix | Commit |
|---|---|---|
| H1: upstream's kept-crash sender and the fork's local crash reports shared `<config>/crashes/crash-*.txt` | The fork's local crashes (panic, unclosed run, GPU lost) now go to `<config>/enhanced-crashes/` (`fork_ext/report_crash_dir.rs`; `report_core.rs` paths, offer, pruning; the Problem zip reads it). `report::start` calls `migrate_legacy` first, once (marker `.legacy-crashes-moved`): every `crash-*.txt` an older build left in `crashes/` moves to the fork's folder, so no old report with a log tail or state snapshot is sent at a login. Upstream's `crashfile` behaviour is untouched. | 28d8352 |
| M2: a limit just above the label length sent the label plus a few bytes, and the file was let go | `report_label::pending` sends a report only if the limit holds label + newline + min(256, report length) bytes; else nothing goes and the file stays kept. L1 folded in: one log line says how many reports were withheld and why. | d6a2006 |
| M1: F3's server Support note had no Enhanced label | One hook in `play.rs` `send_bug_note` wraps the note in `fork_ext::report_label::bug_note`: a `client: <label>` line directly after upstream's `build:` line (upstream's `support.rs` test passes unchanged: `starts_with("build: ")`). The note stays within `BUG_NOTE_MAX` (1000 bytes; whole lines go from the end if needed); the petition's own text and its 2,047-character limit are untouched. | cc502ef |
| L4 (partly) and L7 | Removed the two fork-only duplicate comment blocks in `net/session.rs` (quest marks; the quest tracker's offer; upstream has each once, above its test). The stray doc comment above `the_info_window_says_a_race_label` in `play.rs` is upstream's own at a3cedec, so it stays. Footprint `review_state` refreshed. | 2a03f85 |
| Owner decision (lead): the label says which upstream commit the build is synced to | `enhanced_label()` now reads "EQOA Enhanced X (abc1234), synced to upstream a3cedec, a fork of DabDavis's vanilla client: ..." with the 7-character `reviewed_base` taken at compile time by `include_str!` of `docs/fork-footprint.json` and a small parse (no build.rs, no network, no git at runtime). M2's minimum body applies to the longer label. | 1ab7f75 |

### Tests added and their negative controls

| Test | What it shows | Negative control |
|---|---|---|
| `crashfile::fork_dirs_tests::a_fork_crash_is_in_its_own_folder_and_never_pending_for_a_server` | a fork crash lands in `enhanced-crashes/`; upstream's `files_in`/`pending_in` see nothing; the fork's offer lists it | the same file placed in `crashes/` (the old layout) is pending for the server with its log tail; and with the folder name temporarily set back to `crashes` the test failed |
| `crashfile::fork_dirs_tests::old_local_crash_files_move_out_of_the_senders_folder_once` | legacy files move, a non-crash file stays, a second start moves nothing, upstream's later report stays | before the move both files are pending; failed with the folder set back to `crashes` |
| `crashfile::fork_dirs_tests::pruning_or_answering_one_sides_reports_leaves_the_other` | upstream's pruning and an acknowledgement leave the fork's files; the fork's pruning leaves upstream's | in one shared folder upstream's pruning deletes the fork's file; failed with the folder set back to `crashes` |
| `crashfile::fork_dirs_tests::a_limit_barely_over_the_label_sends_nothing_and_keeps_the_report` | real kept file, real `Session::send_crash_report`, datagrams inspected: limits of label, label+20 and label+256 send nothing and keep the file; label+1+256 and 12,000 send it labelled; pins the one hook line in `send_pending` by source text | the larger limits do send; failed with the old rule restored |
| `report_label::tests::every_kept_report_gets_the_enhanced_identity_before_its_body` (extended) | one byte short of label + useful body keeps the report | failed with the old rule |
| `report_label::tests::the_support_note_names_enhanced_within_its_limit` | `client:` line second, upstream lines intact, never twice, within 1000 bytes, tail lines go first | `bugnote::compose` alone has no label |
| `play::fork_hooks::tests::the_note_on_the_wire_is_labelled` | the F3 path sends the labelled note | no offer sends no note; failed with the `play.rs` hook line reverted |
| `report_label::tests::the_label_says_the_upstream_commit_it_is_synced_to` | label contains `, synced to upstream <7 chars of reviewed_base>, a fork of ` | another or missing base gives another or no commit; failed with the label text reverted |

Existing fork tests that named `crashes/` for the fork's files now name `enhanced-crashes/` (path updates only; assertions unchanged).
`crashfile.rs` gained one test-only `#[path]` line so the new tests can reach its private functions (see Footprint).

Limits of the tests: a unit test has no config directory, so `crashfile::send_pending` itself is not run end to end; the tests
run its real pieces (`pending_in`, `report_label::pending`, `Session::send_crash_report`) and pin the hook line by source text.
No live server crash transmission, real panic, upgrade from a build with old files, or owner playtest was done.

### Test counts

| Suite | Result | Passed | Failed | Ignored | SKIP messages |
|---|---|---:|---:|---:|---:|
| Script (`--continue --tests-only`), final run: eqoa binary | PASS | 2043 | 0 | 31 | 210 (client and net together) |
| Script: net library / dump_capture / effects / newchar / selfstate_capture / doc | PASS | 92 / 1 / 4 / 3 / 1 / 0 | 0 | 0 | included above |
| Script: Python tools | PASS | 97 | 0 | 0 | 0 |
| `cargo test -p eqoa --bin eqoa fork_ext::` (scratch `EQOA_CONFIG`, 1 thread) | PASS | 468 | 0 | 25 | 0 |
| `cargo check -p eqoa --target wasm32-unknown-unknown` | PASS | n/a | 0 | n/a | n/a |

Script totals: 2144 passed / 0 failed / 31 ignored (2137 before this round plus 7 new test functions; the extended one is not counted).
Rework recorded: the first script run of this round had 1 failure, `round_3_all_four_mods_per_frame_performance_benchmark`
(5.034 ms against a 5 ms debug-mode limit; a timing test on a loaded machine, unrelated to these changes). The unchanged
rerun passed with every test green, and the fork_ext run, which includes it, passed. No assertion was relaxed.

### Footprint

`check_fork_footprint.py --base a3cedec7d8bb2688cf8ed56d34a280f4a459b0ef`: PASS.
- `crashfile.rs`: budget +1/-1 to +2/-1 (the test-only `#[path]` line).
- `play.rs`: deletions 61 to 62 (`send_bug_note` line changed, +1/-1); added stays 278 (measured 263); new exactly-once hook "Enhanced Support note".
- `net/session.rs`: shared lines fell (11 fork-only comment lines removed).
- `review_state` text refreshed for this sync. `support.rs` unchanged.

### Not done (LOW items skipped as asked)

L2 (a loopback test through `send_pending` itself: partly met by the new wire test), L3 (viewer.rs upstream comment text and
`chat_let_go` order), L5 (chrome.rs keyboard legend line), L6 (F2 beside the places list), L8 (F4 rebinding), and the `play.rs`
stray doc comment (it is upstream's own, kept). Archived logs: `%LOCALAPPDATA%/Temp/fx-evidence` (script runs 1 to 3).

## Check and lead fix

Codex Terra checked the fix round (`docs/plans/sync-2026-11-11-check.md`): FAIL on one point. If moving
a legacy crash report failed (folder not creatable, rename/copy failing) start-up continued and the
report stayed in `crashes/` for upstream's sender; a directory-read error still marked the migration
done. Lead fix in `fork_ext/report_crash_dir.rs`: a report that cannot move is renamed in place to
`enhanced-crash-*.txt` (outside the sender's `crash-*.txt` pattern; removed if even that fails), and the
migration is marked done only when the folder was read and every report moved, so it retries at the
next start. New test `crashfile::fork_dirs_tests::a_failed_move_still_keeps_old_reports_from_the_server_and_retries`
(blocks the fork folder with a file; the previous code returned before touching `crashes/`, so its
"none left for a server" assertion would fail: negative control by reading, not run). Reruns:
`crashfile` 8 passed, `report` 71 passed, footprint PASS.
