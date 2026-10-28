# EQOA Enhanced 0.1.0 release implementation plan

**Goal:** integrate upstream master and publish a small Windows player download
with automatic updates from LoopRook/eqoa-enhanced and semantic versions.
**Spec:** owner requests current master review, direct EXE distribution, fork
updates and initial version 0.1.0. Previous fork hygiene and map handoff remain binding.
**Architecture:** regular upstream merge; fork-owned release packager/policy and
versioned GitHub release pipeline. Updater follows the fork's latest release.
**Tech stack:** Rust, Python standard library, GitHub Actions and GitHub CLI.

## Constraints and design

- Initial version 0.1.0; immutable tag v0.1.0. Future MAJOR.MINOR.PATCH tags; no
  moving version tags, master pushes, license selection or parser-repository edits.
- Windows x64 player ZIP: eqoa.exe, pinned map catalog, notices, player README and
  manifest. No source, BATs, development tools, accounts, profiles or disc data.
- Public releases enable channel enhanced with their exact build commit. Plain
  local builds retain no channel. Update URLs/notes point only to the fork.
- Existing updater size/SHA256 checks, loopback tests and atomic replacement stay.
- Preserve map/quest behavior and upstream gameplay fixes; shared edits remain
  small with explicit reviewed budgets. Failed CI never publishes a release.
- GitHub Actions builds/tests a semantic tag and publishes completed assets
  together; latest release becomes visible after upload. No rolling dev tag.

## Tasks

- [x] Review and merge upstream 7912c73; keep enhanced README and upstream changelog.
- [x] Add tested fork update policy and player package/manifest generation.
- [x] Add tag-triggered Windows release workflow, player download README and notes.
- [x] Run focused upstream disc regressions, full available suite, native/browser
      build, footprint checks and packaged EXE/corruption/account exclusion checks.
- [x] Independent review, resolve findings, commit/push enhanced and immutable tag.
- [x] Confirm CI/release assets, updater manifest and player download. Record
      source/version/hashes/skips and refresh the normal local EXEs.

## Review ledger

- Initial review: cross-tag latest downgrade race, extra remote draft assets and
  missing Windows runtime dependency. One fix wave addressed all three.
- Scoped re-review: all findings addressed, no new blocker.40Python tests passed;
  static EXE contains only system DLL imports and passes the real PE validator.
- Full workspace1228reported passed/0failed/2ignored; browser build passed. Named
  fixture omissions and real disc controls are recorded in release verification.
- Source commit, exact stamped native rebuild/package, tag CI and public download
  verification remain the final gates. No release has been published yet.

## Completion

- Source c7fddfc tagged v0.1.0; release-control recovery 1d5554b preserves that tag.
- CI 40740158035 passed and published all assets after remote byte verification.
- Public ZIP/manifest were fetched anonymously and matched checksums. Exact
  published EXE passed all three native map captures and refreshed local launchers.
- README has the direct player download. Detailed provenance, test limits and
  recovery audit trail are in RELEASE-0.1.0-VERIFICATION.md.
