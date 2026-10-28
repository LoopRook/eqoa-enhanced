# Enhanced 0.1.0 verification

## Upstream and preserved features

Reviewed upstream 7912c7393df7d85655d5816980fae1217cc48a32, one commit since 8d90350.
Regular merge bfdb12e retains its GM ground-arrival correction. Only README
conflicted; kept the fork landing page and upstream UPDATE-NOTES verbatim.
Local master fast-forwarded to upstream; no master push or tag replacement.
Restored map/quest modules remain unchanged by this release integration.

A temporary hard link to the owned disc enabled the new Goto regressions; the
link was removed afterward, without changing the original disc. All 6 Goto tests
passed. Actual MoveTo stood at 53.72 against ground 53.58 after 90 frames; the old
Respawn negative control fell to -70.39. The loading-state control also passed.

## Tests and builds

- Full locked workspace with release channel enabled: 1,228 reported passed,
  0 failed, 2 ignored across 91 summaries. Existing 168 fixture omissions are named in
  FORK-TEST-SKIPS.md; two added full-suite omissions are
  play::adminplay::tests::a_goto_lands_on_the_ground_once_the_zones_are_in and
  walker::tests::goto_stands_on_the_ground_of_the_square. Those two were exercised
  separately by the disc-enabled run above. Do not count fixture omissions as
  successful oracle/disc comparisons.
- Actual web/build.sh completed successfully. This is browser build evidence,
  not a claim of live browser gameplay.
- Routing regression failed before implementation; only channel enhanced selects
  the fork URL. Local/dev/unknown channels return no release URL. Existing updater
  tests cover failed hashes/downloads, atomic rollback, second-window behavior,
  update notes and Restart/Later input. An old test expecting a vanilla fallback
  was corrected to require enhanced and explicitly reject vanilla.
- Release packager uses real ZIP/PE fixtures and checks exact members, source,
  version, notes, catalog, every hash and byte length. Corruption controls rewrite
  outer checksums to ensure inner validation actually detects the change.
- Initial native dependency audit found VCRUNTIME140.dll. The static-CRT build
  succeeded and its import table contains only Windows system DLLs; the packager
  rejects both ordinary and delayed Visual C++ runtime imports.

## Independent review and publication gates

Initial review found cross-tag latest-version races, unexpected assets retained
in retried drafts, and the unbundled CRT dependency. The fix wave serializes all
publication, checks monotonic history inside the lock, rejects extra draft assets
before upload, verifies exact remote names and downloaded bytes afterward, and
statically links the Windows CRT. Scoped re-review marks all three addressed, with no new blocker. Forty Python
tooling tests passed after the fixes, including remote history/draft guards,
regular/delayed PE imports and a real 0.1.1 README version fixture. YAML, Bash and
PowerShell syntax checks also passed. Exact stamped artifacts and public CI are
final gates after this reviewed source checkpoint.

Release builds use semantic version 0.1.0 and their exact source commit, follow
LoopRook/eqoa-enhanced/releases/latest/download/, and compile the fork's own notes.
Published vMAJOR.MINOR.PATCH tags are retained. The player ZIP has seven runtime
files; separate raw EXE/update manifest/checksums/notes support automatic updating.
No parser repository changes, player data, development tools, BATs or disc data
are part of the package. Existing license notices are retained without selecting
new terms. Moving groups, zoning, despawns and server-specific features retain
the live-player checks described in MAP-PLAYTEST.md.

## First CI attempt and recovery

Original tag source remains c7fddfc35a2f2d4185793af95b2e595a12e6a0e9. The initial
[CI run 40738499391](https://github.com/LoopRook/eqoa-enhanced/actions/runs/40738499391)
passed source/ancestry/toolchain/footprint gates, then stopped on one Python test:
Windows reported the same directory as RUNNER~1 versus runneradmin. No release
was published. The original tag test was reproduced failing under an 8.3 temporary
path and passing under a canonical path; this was a fixture assertion problem.

The recovery workflow uses a canonical runner temp directory, supports dispatch
for an existing tag, and stamps/publishes the checked-out tag's commit rather than
the control workflow revision. Future fixtures use samefile with exact argv and
a wrong-directory negative control. 41 Python tests pass; the unchanged tag's
original assertion passes with the canonical environment. YAML/Bash/PowerShell
and executed source-output fixtures pass. Independent recovery review found no
new defect. Game/Rust/packager code and v0.1.0 were not changed for this correction.
