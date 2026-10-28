# Enhanced upstream synchronization implementation plan

> For agentic workers: use superpowers:executing-plans to execute task by task.

**Goal:** Include upstream master's work without losing the enhanced map or
companion-facing behavior; provide a brief README and license options.

**Architecture:** Merge the complete pinned upstream history into an isolated
branch based on enhanced. Resolve overlapping changes by preserving both intents,
then fast-forward local enhanced only after checks. Make the root README a brief enhanced guide; reuse upstream technical references
by linking them rather than duplicating large ledgers.

**Tech stack:** Rust workspace, WGPU, PowerShell, Git.
**Spec:** User request to review master's commits and remain mergeable; existing
AGENTS.md and docs/MAP-RELEASE-HANDOFF.md govern retained functionality.

## Constraints and review focus

- Pin upstream 8d90350 and enhanced 8aeaf91 before integration.
- Preserve native search, circular minimap, entity hover, waypoint/group guidance,
  font detail, remappable input, inline Suggest and virtual-controller correction.
- Retain fishing, GM tools, housekeeping, shadow settings, caster portal walk and
  upstream draw order. Check new windows own input and render beneath overlays.
- Parser implementation stays with its agent. Preserve inherited telemetry/launcher
  behavior; do not redesign it here.
- No master push, release publication or license change. Existing MIT notices stay.
- Never replace an in-use executable destructively or copy saved accounts into ZIPs.

## Tasks

- [x] Review all 23 upstream commits and changed subsystems against their tests.
- [x] Merge origin/master with full ancestry, resolve every conflict by intent,
      and record resolutions and retained feature anchors.
- [x] Run locked workspace tests, new GPU tests and release build; inspect
      deterministic native map captures and report fixture skips/live limits.
- [x] Obtain independent source review; address actionable integration findings.
- [x] Write the brief enhanced README.md and link its detailed development reference. Add sourced license choices
      without changing LICENSE or Cargo metadata.
- [x] Commit validated integration, fast-forward local enhanced, refresh normal
      executables and preview bundle, and record upstream ancestry/provenance.

## Hygiene continuation

The owner supplied EQOA Enhanced Fork Hygiene Brief after the first merge.
FORK-HYGIENE-STATUS.md is the continuation ledger; FORK-VERIFICATION.md records
final command results and FORK-TEST-SKIPS.md names every omitted fixture check.
The later owner instruction keeps an enhanced root README and makes direct EXE
launch the player path. Developer build commands stay in DEVELOPMENT.md.
