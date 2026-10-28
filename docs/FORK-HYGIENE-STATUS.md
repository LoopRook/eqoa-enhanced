# Fork hygiene execution record

Spec: the owner's EQOA Enhanced Fork Hygiene Brief, October 28, 2026.

This records the original hygiene checkpoint. The owner subsequently requested
versioned releases and fork-owned updates: v0.1.0 is now published. See
RELEASE-0.1.0-VERIFICATION.md and RELEASING.md for current delivery status.

## Rulings

- README.md remains the enhanced GitHub landing page: the owner's later direct
  clarification overrides brief B's upstream README restoration. Detailed upstream
  prose is referenced rather than duplicated.
- CLAUDE.md uses upstream text plus one FORK.md override; UPDATE-NOTES.md is upstream
  text, with fork notes separated into FORK-UPDATE-NOTES.md.
- Keep the validated merge resolutions, but do not introduce additional stock-game
  behavior in this cleanup. The reviewed GM keyboard proposal is saved as a patch.
- Do not prune worktrees, discard the documentation stash, force-push shared
  history, edit the parser repository, or choose a license.

## Work ledger

- A: merge 1bfe228 complete; enhanced and codex/upstream-sync-2026-10-28 pushed
  to looprook. Local master exactly 8d90350; rerere enabled. No master push.
- B: enhanced root README retained by owner override; CLAUDE single override;
  upstream UPDATE-NOTES and LICENSE exact; fork change log no longer duplicates
  upstream entries. FORK.md and ownership record provide the handoff.
- C/D: services moved to fork_ext with one named call per adapter hook. Portable
  parser manifest, one EQOA_PARSER_DIR setting, native no-console launch, browser
  process/UDP gating. Separate parser repository untouched.
- Reporting: button/F3/API/automatic upload paths disabled by default; local
  diagnostics retained; only explicit loopback fixture uploads permitted.
- E: restored map/quest implementations retained. Shared touch points and exact
  +/- budgets documented and checked by tools/check_fork_footprint.py.
- F: clean upstream branch fix-animation-oob-panic prepared at 231f0e9, directly
  atop 8d90350, unpushed/unsubmitted. Original branch saved at
  refs/backup/fix-animation-oob-panic-20261028. Upstream already fixed the original
  negative-clock bug; preserve node_frame nearest-end behavior. Apply only missing
  empty-clip frame guards to enhanced, plus character regression tests; preserve
  enhanced Anim::frame fallback. Synthetic vi library checks: 17 passed.

## Validation

Native release and actual web/build.sh completed with exit0 after final source
edits. Full locked workspace: 1224 Cargo-reported passed, 0 failed, 2 ignored;
168 results omitted fixture checks, all named in docs/FORK-TEST-SKIPS.md.
Additional Python 9, footprint 11, Node report-policy, targeted Rust and three
byte-identical native map capture checks passed. Exact commands/output and limits
are in docs/FORK-VERIFICATION.md. Shared budgets were reviewed and reduced after
extraction; the checker passes all 31 paths and 12 named hooks.

Independent whole-branch review found no actionable defects and confirmed spec
alignment, merge ancestry, preserved map files and all footprint checks. Its full
report is retained as docs/FORK-HYGIENE-REVIEW.md. Source cleanup 73915b0 is committed and pushed to both enhanced and the sync
branch on looprook. The primary enhanced checkout fast-forwarded to that exact
source. Both normal EXEs and the preview ZIP are refreshed and hash-verified.
Licensing and parser sources remain unchanged.

## Remaining owner decisions

- Keep current MIT, or select terms for contributions/distribution after reviewing
  docs/LICENSE-OPTIONS.md. No licensing change is made automatically.
- Whether and when to submit the prepared animation branch to upstream.
- Later isolation of restored features behind a crate/feature flag; outside this
  pass. Full Forge parity and runtime themes remain planned map work.
- Resolved in 0.1.0: versioned fork CI releases and updater URLs now point to
  Enhanced, with verified assets. Plain local no-channel builds still do not update.

## Delivery closeout

- GitHub default branch is enhanced, so the repository landing page shows this
  fork's player README. master remains upstream-only and was not pushed.
- Root README and bundled playtest instructions lead with direct EXE launch.
  BAT/CMD files are optional development or isolated-profile shortcuts.
- Installed root eqoa.exe and target/release/eqoa.exe match the verified source
  binary SHA256 5555caebd557dc923cdcadf3910f39ff21a259832fc92627855b0698ad33912c.
  Previous binaries are retained under ignored run/pre-hygiene-* directories.
- Prepared bundle: run/map-parity-preview/map-preview-5555caebd557.zip. Its manifest
  pins source 73915b0 and the catalog; nine allowlisted files pass CRC/hash checks.
  No profile, account or disc files are included. The direct packaged EXE finds
  its adjacent catalog without CMD environment and matches the search capture.
- No GitHub release or CI build was published. Worktrees and the documentation
  stash remain available. The animation proposal remains unpushed/unsubmitted.
