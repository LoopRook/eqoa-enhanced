# Upstream sync 2026-11-11b

Status: required automated gates PASS; independent lead review and owner playtest pending.

## Scope and ancestry

Started from develop `b1d277f` (the 0.5.0 preparation commit after `acafc33`).
Merged all 11 commits through upstream `11aa5f088bb52d5f306b0224e37b782b1bf4503d`
with a two-parent merge, `5ae723d`, on `sync/2026-11-11b` in
`C:/Users/hocka/Github Projects/eqoa-sync-2026-11-11b`.
Used `python tools/sync_upstream.py --date 2026-11-11b` and the script's continue path.
The upstream history also includes two merge commits; the brief's nine named non-merge commits remain intact.

## Hand resolutions

| File | Kept from upstream | Kept from Enhanced | Reason |
| --- | --- | --- | --- |
| `hud.rs` (3 hunks) | Every Move HUD begin/end, minimap rectangle note, moved picking, chat-strip placement | HUD visibility hooks, local report button, mod HUD submission after the Target end | The stock pieces move exactly as upstream; addon commands use their own reported coordinates rather than inheriting Target's shift. |
| `map/mini.rs` (2 hunks) | Moved outer/inner window rectangles, frame shift for north and reset before world-space marks; indoor floors/lighting unchanged | Round terrain/UI mask, border, adjusted north label, search and hover cards | The circle is inscribed in the same moved upstream rectangle. Ring drawing moved into fork-owned `fork_ext/map.rs`; tests live in `fork_ext/map_move_tests.rs`. |
| `play.rs` (6 hunks) | Compass/menu/chat moved hit coordinates, Effects/Chat/QuickMenus begin/end, HUD frame end and Move HUD overlay | Existing visibility, chat state and input hooks | Preserve both stock movement and addon suppression; no upstream implementation replaced. |
| `viewer.rs` (2 hunks) | Device-loss recovery; upstream's own `web::graphics_lost` exit removed (deleted upstream in bc7754c, not a fork guard); focus-loss HUD release | Local report and addon input routing | Upstream now owns recovery decisions. Upstream's old unconditional exit would defeat its own recovery; upstream deleted it in bc7754c and the fork takes that deletion. |
| `viewer.rs` new recovery constructor | Upstream renderer/window/world/session reconstruction | Addon runtime recreated through a fork-owned hook after saving/shutting down the old runtime | GPU objects belonging to the lost device must be discarded with it. |

`hudlayout.rs` is byte-identical (normalized line endings) to upstream. All 12 new or
changed upstream Rust test functions across this incoming range remain verbatim.
`fork_ext/addons/hud_layout.rs` now applies each stock piece's live HUD shift to
all player, group-member, target and pet rectangles, including frame coordinates.
`eqoa.hud.frame()` retains its window transform; those metrics map moved frame
coordinates to their actual window positions. No API fields were removed or renamed.
Bundled HUD Numbers, Mana Bars and Target of Target consume those moved coordinates;
the ToT assist hit region follows too. Six previous shipped Lua/manifest hashes were
recorded so safe updates recognize their old unedited copies. Player edits remain protected.
Layout JSON is reused while window size, live HUD state and the four pieces' shifts
remain equal; a move, resize, raid/pet change or reset invalidates it. This avoids
rebuilding the same JSON for three bundled mods on every frame.

## Crash reports

Skipped the crash-report work as instructed. Existing Enhanced crash/Support
identity hooks and local offers were retained without changes in this sync.
The earlier sync report records their findings and follow-up fixes.

## Footprint and upstream instructions

`reviewed_base` moved from `a3cedec7d8bb2688cf8ed56d34a280f4a459b0ef` to
`11aa5f088bb52d5f306b0224e37b782b1bf4503d` only after the script's footprint PASS.
No budgets raised. New implementation and regressions live in fork-owned files.
Upstream `CLAUDE.md` is unchanged across this range; the refreshed copy matches
upstream and there are no new rules to adopt. The fork's `CLAUDE.md` rule on browser event loops was corrected afterwards (see Review and fixes).

## Validation

The final sync-script run (`script-7`) passed unchanged, with all Rust tests
serialized and a scratch configuration. Aggregate: **2,157 passed / 0 failed /
31 ignored**, 210 SKIP messages; Python **97 passed / 0 failed**.

| Suite | Passed | Failed | Ignored | SKIP messages |
| --- | ---: | ---: | ---: | ---: |
| eqoa binary | 2,056 | 0 | 31 | 208 |
| net unit | 92 | 0 | 0 | 0 |
| net dump_capture | 1 | 0 | 0 | 1 |
| net effects | 4 | 0 | 0 | 0 |
| net newchar | 3 | 0 | 0 | 0 |
| net selfstate_capture | 1 | 0 | 0 | 1 |
| net doc tests | 0 | 0 | 0 | 0 |
| Python tools | 97 | 0 | 0 | 0 |
| Separate addons (fresh config; subset of eqoa) | 377 | 0 | 24 | 9 |

The reported Rust pass counts include tests that return early after printing SKIP;
those messages are not claims of fixture validation. Missing inputs include the
owned-disc/decrypted CLIENT/oracle/capture/S2/reference resources. Exact ignored
names and per-suite SKIP counts are archived in `test-skips.json`. No tests were
newly ignored. No unresolved failure was classified as pre-existing; the optional
untouched-upstream GPU comparison was consequently not needed.

The actual moved-minimap GPU composition test passed at 640x480, 1280x720 and
768x1024, checking the inscribed circle, old-center and corner negative hits,
border/north/card quad translations and the moved search button versus its old
inactive position. No disc was needed for its synthetic map fixture.
Addon tests verify moved/reset frame and window coordinates, bundled readout
commands and recovery runtime path/server/open intent. Upstream's five HUD-layout
tests also pass. The four-mod benchmark passed at **2.966 ms/frame** against the
unchanged 5 ms debug limit, with zero post-warmup UTF-8/queue/text-draw allocations.
`cargo check --locked -p eqoa --target wasm32-unknown-unknown`: **PASS** (28.47 s).
Final footprint and the 12-test upstream source audit **PASS**. Every budget is
unchanged: HUD +52/-14, mini +266/-14, Play +262/-60 and viewer +101/-11 fit their
existing maxima. No crash-report path changed relative to develop's starting commit.

The separate addon suite is a repeated subset, not 377 additional unique tests.
Evidence: `C:/Users/hocka/AppData/Local/Temp/eqoa-sync-2026-11-11b-gates`
(`script-7.json`, `script-7-cargo.log`, `addons.json/log`, `wasm.json/log`,
`footprint-final.log`, `source-audit.json`, `test-skips.json`). Earlier measured
failures are retained in script receipts and the archived cargo logs where available.

Rework: first native compile found the extracted ring helper's fork test import
and the new recovery constructor's addon-runtime field; both corrected. The Python
notes test was hardcoded to an older Unreleased bullet and now asserts this sync's
new bullet. The next native run had 2,052 passes / 2 failures / 31 ignored: the
new circle test used exact float radius equality (60 vs 60.000015), corrected to a
0.001-pixel tolerance, and the fork's options fixture had not accounted for upstream's
HUD Layout page. Its expected stock rows now include HUD Layout (76/77), Chat and
Combat in upstream order. All upstream tests stayed untouched.
The next native run had 2,054 passes / 1 failure / 31 ignored: its four-mod cost
was 8.327 ms against the unchanged 5 ms debug limit after adding layout reads.
The initial JSON cache still measured 6.976 ms (2,055 passes / 1 failure / 31 ignored).
The remaining cause was the layout binding calling `record` directly, rebuilding
the readonly Lua tree each call. It now uses the existing `record_cached` path, as
`eqoa.hud.frame()` already did, while geometry JSON is cached by its complete input key.
No performance threshold
or picture-test tolerance was relaxed. An intermediate cache-wrapper delimiter typo
was caught by formatting/compile and corrected before the final run.
Python tooling passed 97 tests in the second and subsequent gates (the first
run had 96 passes / 1 stale-notes failure). One later native run had 2,055 passes /
1 failure / 31 ignored: `an_own_hit_reaches_the_swing_timer` differed by one
millisecond between its captured clock and the handler's clock. No source/assertion
changed; the next full run passed. No untouched-upstream reproduction or
pre-existing classification is claimed for that transient result.

## Limits and outstanding checks

No push, tag, merge into develop/enhanced, public release, or installation performed.
The main checkout remains on develop. No Cargo cache was created inside the worktree;
all Cargo commands use the shared target, shared mkdir lock, foreground subprocess,
1800-second command timeout and isolated EQOA_CONFIG. The sync script's target folder
contains only logs/configuration; those are archived as `target-archive` in the
evidence directory and the worktree target folder is removed after validation.

Manual resolutions require the lead's independent review before integration.
The upstream ancestry dry-run `git merge-tree --write-tree --name-only HEAD
origin/master` returned exit 0 and no conflicts (tree
`beab9778bfd8cba194fe5368580747165fa8c05c`, at merge `5ae723d`). No protected-branch
integration or publishing command was executed. The lead can review this branch, then merge it into develop under the
owner's normal policy; no release/tag/push is part of this task. Full workspace,
native release build, web asset packaging and deterministic disc-based captures
were not requested by this brief and are not claimed; their commands remain the
repository's documented commands, using the same shared lock/target if run later.

Owner playtest remains: mouse and touch Move HUD/Lock/Reset and per-window persistence;
round minimap outdoors/indoors, hover cards and search after moving; addon replacements
and HUD numbers following stock bars; real graphics-device recovery and browser reload
fallback. The software tests do not establish those live workflows.

## Review and fixes

Independent review: `docs/plans/sync-2026-11-11b-review.md`, verdict APPROVE WITH CHANGES.
Correction to this report: the `web::graphics_lost` exit removed from `viewer.rs` was upstream's
own code (a3cedec), deleted upstream in bc7754c; it was never a fork browser guard, and HEAD's
`viewer.rs` matches 11aa5f0 there. Upstream's own `docs/upstream/CLAUDE.md` still says each event loop
ends on `web::graphics_lost`; that file stays verbatim and is stale on this point.

Done:
- MEDIUM 1, stale `CLAUDE.md` rule and misattributed report row: `CLAUDE.md` now says a lost graphics
  device is made again in place (commit 4de3552); the report's viewer.rs rows are corrected above.
- MEDIUM 2, weak recovery-hook test: replaced by
  `device_recovery_flushes_a_changed_mod_option_and_the_new_runtime_reads_it_back` in
  `fork_ext/addons/runtime.rs`. It launches an active runtime with a mod through `launch_headless`,
  runs three frames (the mod changes its option on frame 3), calls `for_new_device`, and checks that the
  old runtime flushed the option to the profile exactly once (the saved `saves` count is 1) and that a
  new runtime, which only runs two frames, reads `changed` back and saves it. Negative control: with
  `for_new_device` rewritten to skip shutdown and the flush (`std::mem::forget(self)`), the test fails at
  "device recovery must flush the old runtime's addon state"; restored afterwards. Skipping only the
  explicit `self.shutdown()` is not detectable, because `Drop` runs the same guarded shutdown when the
  consumed value ends. The older test (paths, server, open intent, lazy attach) is kept.

Not done (LOW):
- Finding 3, no screenshot or GPU line in a local report after a native device recovery: recorded in
  `docs/backlog.md`.
- Finding 2b, a `FORK-UPDATE-NOTES.md` line saying recovery restarts addons' in-memory state: not done.
- Finding 4, direct assertions for layout-cache hit and raid/pet invalidation: not done.
- Finding 5, recording the pre-sync four-mod frame cost and skipping the JSON compare on a cache hit:
  not done.
- Finding 6, update-notes mention of an edited manifest with an unedited script: not done.
- Finding 7, the two pre-existing fork-modified tests and the misplaced doc comment in `map/mini.rs`:
  not done (not from this sync).
- Finding 8, the `unreachable!()` in `hud_layout.rs::to_rect`: not done.
