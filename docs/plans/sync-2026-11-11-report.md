# Upstream sync ? 2026-11-11

Prepared on `sync/2026-11-11` in `C:/Users/hocka/Github Projects/eqoa-sync-2026-11-11`.
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
`C:/Users/hocka/Paseo/cargo-build.lock`, with a bounded lock wait and 3,600-second
command timeout. All used `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`.
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
`C:/Users/hocka/AppData/Local/Temp/eqoa-sync-2026-11-11-gates`.
The worktree's script-created `target` held only logs/configuration, never Cargo
artifacts; its evidence was archived and that temporary folder removed before
handoff. No private game data or credentials are committed.
