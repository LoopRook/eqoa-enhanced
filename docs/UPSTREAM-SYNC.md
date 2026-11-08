# Upstream synchronization record

## How to sync

From the main checkout (on develop, nothing building):

    python tools/sync_upstream.py

It fetches origin, makes `sync/<date>` in `../eqoa-sync-<date>`, merges origin/master and
does the routine parts itself: README.md and CLAUDE.md keep ours (`.gitattributes`),
Git's rerere replays any resolution recorded in an earlier sync, and a conflict inside a
one-line struct literal (play.rs's `Play { ... }`) is merged field by field. A clean merge
then refreshes docs/upstream/CLAUDE.md, moves `reviewed_base` only if every shared budget
still passes, commits the merge, and runs the client/net and Python tool tests. The
report is `target/sync-report.md` in the sync worktree.

It stops for a person at a real conflict (exit 2: resolve, `git add`, then
`python tools/sync_upstream.py --continue` in the worktree) or a budget overrun (exit 3:
fix, or raise the budget with a reason, then `--continue`). Each hand resolution is
recorded by rerere, so the same conflict resolves itself next time. Still by hand: read
upstream's CLAUDE.md diff for rules worth adopting, the sync's entry below, the
independent review, the owner's play-test, and the merge into develop.

## Initial integration

Enhanced baseline: 8aeaf91. Upstream merge target: 8d90350. Common upstream
ancestor: 74d8d7e. All 23 incoming commits are retained through a merge, not copied
or squashed. They add fishing, GM tools, network housekeeping, character shadows,
portal caster selection and corrected rendering order.

Conflicts and resolutions:

- README: enhanced landing page, following the owner's direct clarification.
- UPDATE-NOTES: upstream text; fork notes in FORK-UPDATE-NOTES.md.
- options: preserve Shadows ID 46 and all enhanced rows; enhanced-only IDs move to
  1043..1052 without changing persisted keys. World remains six rows; Map Markers
  moves into the five-row Map & Quests page. Mods & Addons stays reachable.
- settings: combine shadow serialization/description/tests with enhanced settings.
- Play: preserve server timing defaults, fishing/GM state and enhanced map state.

## Evidence before the hygiene refactor

- Locked full workspace suite: 1,205 passed, 2 existing FX benchmarks ignored
  across 91 test/doc-test summaries.
- After narrowing the hygiene scope to leave the optional GM keyboard proposal
  unapplied: 958 client tests passed, 2 existing FX benchmarks ignored.
- Initial release build and native captures passed. Three map captures match the
  enhanced baseline byte for byte; the scene capture reports a drawn shadow.
- Source audit: 18 newly added upstream files and 31 enhanced feature files
  match their respective baselines. Shared integration was reviewed separately.

Prerequisite-based disc/oracle checks can return early. Final hygiene validation
lists skipped names and actual prerequisites in FORK-TEST-SKIPS.md; commands,
final build results and map hashes are in FORK-VERIFICATION.md. These checks are not a claim
of live fishing, GM permissions, moving group, or every indoor shadow behavior.

## Known upstream issue

Native GM Goto/Buff PC keyboard dispatch is absent upstream. A reviewed proposal
is saved in patches/upstream-gm-keyboard.patch, apply with git apply --unidiff-zero if selected later, without changing stock gameplay
outside this hygiene pass. Controller/pointer behavior remains upstream's.

## Next synchronization

Follow FORK.md and the footprint manifest. Merge origin/master into develop
regularly through short-lived sync/<date> branches, preserve full ancestry, and
keep master a pure fast-forward mirror. Keep our CLAUDE.md on conflict; replace
docs/upstream/CLAUDE.md with upstream's new CLAUDE.md and review its diff. Never push master to looprook or publish through its release workflow.
The owner decides the license; existing MIT/OFL notices remain intact.

## Enhanced 0.1.0 follow-up

Merge bfdb12e integrates upstream7912c73. Only the fork README conflicted; keep
our player guide. Upstream GM MoveTo/loading/walker code is retained, and actual
disc regressions exercise the corrected floor placement with negative controls.
Versioned release routing and packaging are documented in RELEASING.md and
RELEASE-0.1.0-VERIFICATION.md. The original hygiene report is historical;0.1.0
subsequently enables fork-owned updates under the owner's explicit request.

## 2026-10-31 sync

Upstream range: 7912c73..ad941f5 (86 commits). Incorporates duels, live group
search, live look dyes, 16-bit ground fog (`fog16`), frame stall diagnostics,
and sound backlogs.

Conflicted files and resolutions:
- README: retained fork landing page and release download links per owner policy.
- binds.rs: retained upstream corpse window description and appended fork map note.
- hud.rs: retained upstream patch closure, duel patch, and `bearing_point`; reapplied
  group compass marks draw gate and fork `HudState` fields.
- options.rs: kept upstream Ground Fog (ID 47) on World and Shadows (ID 46) on
  Characters; preserved fork Map & Quests page and IDs 1043..1047.
- settings.rs: unioned `ps2_fog` with fork map settings across defaults, parsing,
  serialization, and updated `describe()` test.
- viewer.rs: routed addon events in `window_event` before upstream stall timing;
  updated touch pad perf mark to phase 17.
- play.rs: unioned upstream duel/live/trigger fields with fork map and health state.
- session.rs: unioned upstream opcodes 0x7E07..0x7E0D with fork opcodes 0x7E01..0x7E06
  and preserved respective state handlers.
- fans.rs: adopted upstream `draw_fans` device signature cleanly.
- lib.rs: exported both `fog16` and fork `map` modules.

Validation:
- `cargo test --locked -p eqoa --bin eqoa -- --test-threads=1`: 1,356 passed, 0 failures, 2 ignored (`the_discs_patch_s2`, `the_discs_patch_s4` disc skips).
- `cargo test --locked -p net -p render -p esf -p vi -- --test-threads=1`: all suites passed (net: 49 passed; render: 51 passed, 1 ignored benchmark `ground_fog_cost`; esf: 22 passed; vi: 65 passed; 0 failures).
- `cargo check --locked -p eqoa --target wasm32-unknown-unknown`: passed.
- `python tools/check_fork_footprint.py --base ad941f5ca18a9238c5c5e51bdb77b9abb6afd37d`: 19/19 hooks present, all 34 shared files within budget.
- `python -X utf8 -m unittest discover -s tools -p "test_*.py"`: 46 passed, 0 failures.
- `cargo build --release --locked -p eqoa`: finished successfully.
- Note: `forge_texture_early_memory_limit_rejects_before_upload` was GC-timing flaky after the merge and fixed in e16efff (plus the sibling in `forge_texture_memory_early_memory_limit_rejects_before_upload`).

## 2026-11-03 sync

Enhanced start: e3964ff on sync/2026-11-03, branched from develop. Upstream range:
ad941f5..b8e02cd781aac6a1b9e9906f09896956c77afcd6 (eight commits). Real two-parent merge;
no squash, push, tag, or protected branch update.

Incoming changes: SSE-compatible EE four-lane NEON and WebAssembly SIMD paths;
Android APK/source-build support and input plumbing; adjustable Touch Pad Size,
resize recovery and fishing message-box layout; live Class Mastery branch/rank
rules. Upstream 42e41f8 removes reports because its report server closed.

Conflicts and resolutions:

- README.md (four hunks): retained Enhanced's landing page/download links.
- hideui.rs (one): retained upstream wording and Enhanced map visibility; report
  visibility is consumed through the viewer's local report-open hook.
- hud.rs (three): retained upstream implementation and existing Enhanced HUD;
  report button drawing lives in fork_ext/report_core.rs, returning its pick rect.
- main.rs (two): retained upstream logfile lifecycle and Android registration;
  alias the fork-owned report core and retain the CLI credential-noting hook.
- options.rs (four): union Touch Pad Size ID 48 with existing fork rows; Enhanced
  IDs 1043..1047 and all persisted keys stay unchanged. Text/HUD remain reachable
  on Map & Quests; Controller gains Touch Pad Size.
- play.rs (three): retain upstream removal of report-only state/describers and
  keep Enhanced map/addon initialization; HUD report pick calls the local core.
- settings.rs (six): union touch_size field/default/serialization with fork
  settings; keep upstream removal of unused report descriptions and their test.
- touch.rs (two): keep upstream full-size slop and all pad-size behavior, preserve
  Enhanced map long-press suppression/tests, initialize fork dpad/holds fields in
  the upstream Default constructor.
- viewer.rs (four): union upstream resize fields and touch scale with addon/map
  state; retain upstream removal of report diagnostics. Minimal hooks invoke
  fork_ext/report_viewer.rs for native dialog input/capture and fork-owned draw.
- report.rs (modify/delete): accept deletion of the shared path; move retained
  local report/crash core to fork_ext/report_core.rs. Production send always uses
  Save to Desktop; historical HTTP transport is compiled only for loopback tests.
- web/build.sh (one): take upstream's notices.js copy list.
- web/report.js (modify/delete): accept deletion. RELEASING.md and
  enhanced-release.yml ship Windows x64 only; no Enhanced web release is published.
- CLAUDE.md: ours merge driver retained fork rules automatically. Refresh the
  verbatim upstream copy and adopt Android's platform-event/signing-key guidance;
  upstream's report deletion is overridden only for Enhanced's local saves.

Owner's report decision: retain F3, the HUD's Report a bug, the next-start crash
report offer, and Save to Desktop. ZIPs contain the note, screenshot and logs;
known accounts/passwords and the Windows home directory are scrubbed. Nothing is
uploaded. Upstream logfile.rs stays intact around eight added hook lines (startup
before rotation, exact run identity, redaction/ring, Enhanced header, panic and
clean exit); Station api.rs retains four credential hooks. The journal helpers
remain in fork_ext/report.rs and report_core::clean_exit flushes map/journal.

Footprint review: PASS against the complete b8e02cd target with a trial manifest
before advancing reviewed_base. Shared budget changes from the previous review:
logfile +8/-0 (new), Station API +4/-0 (new), main +3 additions, HUD +2 additions,
viewer +20 additions/+3 deletions, touch +2 additions. Reasons: retain local
reports through hooks and reconcile upstream's new touch constructor. The old
shared report.rs/web/report.js budgets disappear, as does web/build.sh's policy
copy override. The settings description hook disappears with upstream's method.
Detailed maxima and reasons are in FORK-OWNERSHIP.md and fork-footprint.json.

Hook-removal controls (each mutation was compiled/tested, then restored):

- sync_f3_saves_redacted_zip_through_dialog_hook: removing viewer's report event
  hook fails the headless wiring guard: assertion failed, viewer contains
  fork_report::event(self, el, &event). With it present the dialog's Enter action
  saves a ZIP in a temporary Desktop, including screenshot/log/note and redaction.
- sync_crash_marker_is_offered_on_next_start_hook: removing logfile startup's
  report::start call fails its wiring guard. The behavior portion checks an
  interrupted log creates an offer; a cleanly closed log creates none.
- sync_logger_close_flushes_journal_through_report_hook: removing logfile close's
  report::clean_exit call fails with "logger close must flush journal", NotFound.
  With the hook present the temporary journal contains its recorded creature.

The first two tests combine source wiring guards with headless behavior tests;
they do not claim a live OS keyboard/window/GPU test. Owner playtest remains due.

Additional merge regression fixes:

- Keep EXTENDED_ITEMS in numeric ID order (48 before 1043), as the existing
  every-row/one-page regression requires; no persisted IDs or keys change.
- logs_begin_with_the_enhanced_line now starts/closes upstream logfile, the
  actual owner of the log, and checks the fork header fed through its hook.
- The fork HUD count fixture has no font: its report button is 28 pixels wide
  with two 16-pixel caps, leaving a -4-pixel middle. Upstream's new VU1 winding
  cull drops that middle. Temporarily disabling only culling made the original
  17/41 quad assertions pass; with upstream restored the correct fixture counts
  are 16/40. Production raster code is unchanged. The first workspace attempt
  stopped at 1,499 client passes / three failures / 26 ignored; these findings
  prompted the fixes and a complete rerun.

The second full attempt passed those three fixes (1,501 client passes / one
failure / 26 ignored), then exposed the existing community-map test's PID-only
cache path. Its log explicitly loaded a one-marker version-9 cache left by an
earlier process before testing a malformed answer against three held markers.
The fixture now creates a unique, freshly owned directory and cleans it on drop;
production community-map code is unchanged. The retained report dialog's name,
picture and crash-offer wording now describes local saving rather than a board.

Final locked workspace suite: 1,775 passed / 0 failed / 27 ignored across 96
summaries; client 1,502 passed / 0 failed / 26 ignored. Explicit SKIP messages: 197
(missing disc/oracle/reference prerequisites and symlink privileges, recorded
separately from ignored). Python tools: 46 passed. Footprint: PASS, 33 shared
paths in budget, reviewed_base b8e02cd781aac6a1b9e9906f09896956c77afcd6.
See SYNC-2026-11-03-VERIFICATION.md for commands, controls and all skip messages.
Locked release build: PASS (4m 26s); PE x64 GUI / 8 MiB stack validated. Preview
folder: main checkout run/sync-2026-11-03-<merge-shortsha>/, with bundled mods and
owner checklist. Copy verification requires the EXE SHA256 to match the built
artifact recorded in SYNC-2026-11-03-VERIFICATION.md. No push/tag/protected branch
merge; live owner QA and Android/browser runtime checks remain pending.


Review follow-up to b5e6310 (2026-11-03): remove the unused client-state feature,
duplicate logger and settings describer, and orphan browser policy; retain one
native panic-hook owner (logfile). Restore the HUD Report pick regression, guard
logger cleanup on test failure, standardize the report-open predicate, register
HUD/surface seams, and tighten slack budgets. Second-window log selection is
recorded in backlog.md without changing it in this round. Follow-up checks: sync_* 5 passed; report filter 44 passed; HUD pick 1 passed.
Locked full workspace: 1,773 passed / 0 failed / 27 ignored over 96
summaries, with 197 explicit prerequisite SKIP messages. Python tools: 46 passed.
Footprint PASS with the new seams and tightened budgets. Locked release build
PASS (1m 40s); x64 GUI / 8 MiB stack validated. Current executable SHA256:
`6b3272c2509f2fdba83105112c72401f97af5026061ac9ab358eee3ec67f8788`. A new main-checkout preview under
run/sync-2026-11-03-<follow-up-shortsha>/ is copied after committing; the old
b5e6310 preview is retained.

The HUD budget changes from +309/-11 to +315/-14 solely for the restored
nonzero-height fixture, Report pick assertion and misplaced-position negative
control. Options is tightened to +194/-42, play to +750/-53 and settings to
+322/-17; the remaining shared budgets already match their measured diff.
Viewer uses the portable is_open_local predicate everywhere, requiring only
two shared-line changes from the merge. Historical browser-policy test records
are marked as historical, and their retired files are removed from ownership.

Client state restored as a fork-owned snapshot at the owner's request (2026-11-03).
`fork_ext/report_state.rs` describes mode/target/nearest eight entities, world/zone,
position/heading, HUD claims, enabled mods, map/search and autorun/follow. The
existing frame_library adapter forwards the walker and calls note_state at most
once per second; snapshot text is scrubbed before state.txt persistence and ZIP
export. Crash files retain the last noted snapshot, and startup clears the old
slot after collecting an interrupted run. Upstream describers remain deleted.
Shared budgets are unchanged (play +750/-53); a named snapshot seam is guarded.
Snapshot follow-up checks: sync filter 5 passed; report filter 48 passed.
Locked workspace suite with scratch EQOA_CONFIG and one test thread: 1,777 passed,
0 failed, 27 ignored, 197 explicit prerequisite SKIP messages. The actual capture
hook removal made the ZIP test fail with "client_state.txt is empty without the
per-frame report hook"; restored hook passes. Fake-clock, Play-fixture, HUD-claim,
redaction, ZIP-content and previous-run crash-state regressions pass. Footprint:
PASS, 303 owned / 33 shared, budgets unchanged.

Locked release build: PASS, optimized (1m 16s), x64 GUI / 8 MiB stack validated.
EXE: 32,745,472 bytes; SHA256 `4a7e85dce82a833b992d5da780db030a2ea8b3206f09cc0c898ed00e0335065b`. A new
main-checkout run/sync-2026-11-03-<snapshot-shortsha>/ preview is copied after the
follow-up commit; its checklist includes client_state.txt and the crash section,
with exact EXE and bundled-mod hash comparisons. Earlier preview folders stay.


Integration snapshot review fix (base 4840b46, integrate/2026-11-03): the per-frame
snapshot now reads an in-memory addon cache. A single bounded report-io worker
refreshes that cache off-thread at most once per 30 seconds and writes state.txt
only when its text changes. Normal exit flushes pending work; panic capture still
uses the latest redacted in-memory STATE, and interrupted-start capture still
reads the last persisted snapshot. Addon/profile changes may take up to 30 seconds
to appear in this diagnostic cache; report export keeps its fresh manifest scan.
Canonical Dot::kind is shared with the minimap (p256 only). Zone lookup reuses
startmenu::zone_name and gWorldInfo bounds with the same committed coordinates as
the Position line. Loading snapshots omit stale character/world information and
say Mode: loading. Modes 7, 9, 36 and 42 have names. The sole new shared change is
zone_name visibility (+1/-1; body unchanged); play remains +752/-54, with the
in-place frame_library edit recorded in its budget reason.

Negative control: restored the old scanning function in report_state.rs and ran
report_state_capture_does_not_scan_mods_on_frame_thread. It failed at runtime
(exit 101, 0 passed / 1 failed): "snapshot performed a filesystem scan on the
frame thread", scan count 4 versus 1. Restored the exact source bytes; the final
report filter passes all 53 tests. The worker test also confirms scans/writes
execute on a different thread, skips unchanged writes, and scans once across
repeated captures. The existing redaction/ZIP/crash/throttle tests remain passing.

Final integration verification (shared mkdir lock with owned EXIT cleanup,
`timeout 2400`, scratch EQOA_CONFIG under integration target/, common
CARGO_TARGET_DIR at sync-2026-11-03/target):

- `cargo test --locked --workspace -- --nocapture --test-threads=1`: PASS,
  **1,789 passed / 0 failed / 27 ignored**, across 96 summaries; client
  **1,516 passed / 0 failed / 26 ignored**. **197 explicit prerequisite SKIP
  messages** (missing disc/oracle/reference resources and symlink privileges).
- `C:/Python314/python.exe tools/check_fork_footprint.py --base b8e02cd`: PASS,
  **308 fork-owned / 34 shared**. New startmenu helper visibility budget +1/-1 is
  justified above; all other budgets unchanged, including play +752/-54.
- `cargo build --release --locked -p eqoa`: PASS, optimized, 1m 14s.
  PE x64 GUI / 8 MiB stack validated. EXE 32,794,624 bytes; SHA256
  `4df0f2907881991a78ad3bb24f3aab149c4dd109b20ba635398426a07edbb5ee`.

A new main-checkout `run/combined-<follow-up-sha>/` preview is copied after the
commit. It uses the same six-entry layout as combined-4840b46, with identical
launcher/checklist bytes except the checklist header's new commit. EXE and all
bundled-mod copies are hash-checked; existing previews are never overwritten.
Live owner QA remains pending. No push or other-branch changes.

## 2026-11-07 sync (AGPL)

Upstream b8e02cd..eb5916bfd25053dcc047c3a050a2529577842306 (35 commits): raid list,
GM /fly, copy/paste and chat selection, controller macros, the group invite opcode,
F3 bug reports to servers that take them, server races, flyer landing/underwater,
WASM SIMD EE, and upstream's relicence from MIT to AGPL-3.0-or-later (ab32f96,
eb5916b). Owner decision: take the full range; the fork is AGPL-3.0-or-later and
LICENSE-ENHANCED is retired. Code published through 0.4.5 and upstream through
1e23e80 stays MIT; NOTICE carries that text and ships in the player ZIP.

Branch sync/2026-11-07 off develop a4c8748: db1332a merge (8 conflicts, union),
d1fe4d6 header sweep and NOTICE credit, 4a47fc9 NOTICE in the player ZIP, then a
review round:
- F3: the local report claimed F3 before upstream's handler, so Support's
  bug-report form was unreachable natively. fork_ext/report_viewer.rs now passes
  F3 on when the server offered bug reports (`Session::bug_reports`); elsewhere,
  and before the world, F3 saves the local report. The HUD Report button always
  saves locally. Test `f3_is_supports_form_where_the_server_takes_bug_reports`;
  negative control: the old predicate fails "an offer: F3 passes on".
- Options: the merge had raised PAGE_MOST to 7 and narrowed the full-page layout
  test. PAGE_MOST is 6 again, the test uses PAGE_MOST + 3, and upstream's Raid
  List row sits in Map & Quests (UI already holds six). Player notes must say
  "Options > Extended > Map & Quests > Raid List" (upstream's UPDATE-NOTES says UI).
- Footprint re-based on eb5916b; render/ui.rs takes upstream's text/text_scaled;
  NOTICE is a shared path; five budgets raised to measured (reasons in the JSON).
- Per-file headers point the MIT text at NOTICE; upstream doc comments the merge
  mangled are restored.

A line-level check (every line either side added since b8e02cd is still present)
found nothing lost except the intended license wording. Reviews: MiniMax M3 on
a4c8748..d1fe4d6 and on the review round, both MERGEABLE WITH FIXES; findings
fixed except the open item below.

Verification (scratch EQOA_CONFIG, --test-threads=1): `cargo test --locked -p
eqoa -p net`: client 1,586 passed / 0 failed / 26 ignored, net 53 / 0. Python
tools 51 passed. Footprint against eb5916b: PASS. Oracle tests skip here (no
disc/oracle on this machine).

Open: with Target of Target on, a raid list of 13+ rows (or 10+ rows and a pet)
reaches y 314 and the fork's Target-of-Target bar (y 300) is drawn over its last
row or the pet bar. Fix awaits the owner's choice of layout.

## 2026-11-08 sync

Upstream eb5916b..72a7d487a5667dc0d6cd0dfd0949ef66cdf6f97e (17 commits): Character >
Combat Record (0x7E20/0x7E21), a server's own races and classes at creation
(0x7E1E/0x7E1F), three-letter names, /bug opening the bug form, power on raid
frames, the spell book's Unmemorize and scrolling description, Auto Attack on from
the start. Branch sync/2026-11-08 off develop 429431c; merge d59c0ae. No new opcode
collides with the fork's (0x7E01-0x7E06 remain the fork's power/quest offers).

Conflicts (3): README.md kept ours (fork landing page); play.rs's one-line `Play`
literal gains upstream's `record` beside the fork's fields, and `record_menu()` runs
beside the fork's power-bar/quest agreements; session.rs, both sides' tests kept
(the fork's other-offers test and upstream's combat-record test).
docs/upstream/CLAUDE.md refreshed: only the combat record's mention, nothing to adopt.

Verification (scratch EQOA_CONFIG, --test-threads=1): `cargo test --locked -p eqoa
-p net`: client 1,600 passed / 0 failed / 26 ignored, net 51 / 0, integration tests
8 / 0. Python tools 63 passed. Footprint against 72a7d48: PASS, budgets unchanged.
Every test name on develop and upstream is present after the merge, except three
upstream tests develop had already dropped before this sync
(a_group_line_is_a_blue_chat_line, a_layout_with_the_maps_rows_still_loads,
options_with_the_lines_of_rows_gone_still_load). Oracle/disc tests skip here (run
without --nocapture, so SKIP lines were not counted). Owner play-test pending.

## 2026-11-08 sync (second)

Upstream 72a7d48..8435f2c (5 commits): PCSX2 texture packs and texture dumps on a new
Extended > Textures page (rows 50/51, `texpack.rs`, `esf::pcsx2`, a renderer pack hook),
and every class for a server's race past the game's twenty. Branch sync/2026-11-08 off
develop 9301c3d; merge 4c27535, fix 6510561.

Conflicts (4, by hand; rerere recorded): main.rs, both modules (`fork_ext`, `texpack`);
settings.rs, both sides' tests; render/ui.rs, the fork's `add_font_at_height` with
upstream's new `add_texture_as` size argument; options.rs, Textures goes last (index 9,
after Map & Quests and Mods & Addons) so the web build's `split_at(9)` drops only it, as
upstream's `split_at(7)` did; the walk test follows it there. 6510561 keeps
EXTENDED_ITEMS in item order (50/51 before 1043+), which
`every_row_on_one_page_and_no_page_scrolls` requires (it failed before the fix).
options.rs budget +5/-5 with its reason. docs/upstream/CLAUDE.md: only texture packs'
description, nothing to adopt.

Verification (sync script: scratch EQOA_CONFIG, --test-threads=1): `cargo test --locked
-p eqoa -p net` 1,667 passed / 0 failed / 26 ignored, 127 SKIP messages (disc/oracle
absent here). Python tools 80 passed. `cargo check -p eqoa --target
wasm32-unknown-unknown` passes. Footprint against 8435f2c: PASS. Light review (Codex
Terra) of the four resolutions: no findings; it noted upstream's row-coverage test
would fail if run on wasm (upstream's, tests run natively). Not tested: texture packs
with a real PCSX2 pack, and their interplay with Forge addon textures in game. Owner
play-test pending.
