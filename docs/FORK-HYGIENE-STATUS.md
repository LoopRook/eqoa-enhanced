# Fork hygiene execution record

Spec: the owner's EQOA Enhanced Fork Hygiene Brief, October 28, 2026.

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
report is retained as docs/FORK-HYGIENE-REVIEW.md. Final commit/push and normal
executable/preview refresh follow this verified state. Licensing and parser
sources remain unchanged.

## Remaining owner decisions

- Keep current MIT, or select terms for contributions/distribution after reviewing
  docs/LICENSE-OPTIONS.md. No licensing change is made automatically.
- Whether and when to submit the prepared animation branch to upstream.
- Later isolation of restored features behind a crate/feature flag; outside this
  pass. Full Forge parity and runtime themes remain planned map work.
- Before public fork CI releases, repoint updater URLs to the fork. Current local
  no-channel builds do not self-update; no release workflow is triggered here.
