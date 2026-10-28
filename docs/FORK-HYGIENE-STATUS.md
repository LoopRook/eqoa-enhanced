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

- Pending: finish pinned 8d90350 merge, then inspect any new upstream delta.
- Pending: disable upstream reporting; keep local diagnostics usable.
- Pending: isolate mods/telemetry/HUD/option helpers and portable companion launch.
- Pending: enforce shared-file ownership/footprint and browser compatibility.
- Pending: audit animation guards, prepare the clean upstream branch, verify and
  push enhanced/sync branches to looprook.
