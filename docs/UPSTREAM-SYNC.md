# Upstream synchronization record

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
