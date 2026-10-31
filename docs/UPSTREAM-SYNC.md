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
