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
will list skipped names and actual prerequisites. These checks are not a claim
of live fishing, GM permissions, moving group, or every indoor shadow behavior.

## Known upstream issue

Native GM Goto/Buff PC keyboard dispatch is absent upstream. A reviewed proposal
is saved in patches/upstream-gm-keyboard.patch, apply with git apply --unidiff-zero if selected later, without changing stock gameplay
outside this hygiene pass. Controller/pointer behavior remains upstream's.

## Next synchronization

Follow FORK.md and the footprint manifest. Merge origin/master regularly through
short-lived branches, preserve full ancestry, and keep master a pure fast-forward
mirror. Never push master to looprook or publish through its release workflow.
The owner decides the license; existing MIT/OFL notices remain intact.
