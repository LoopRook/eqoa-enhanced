# EQOA Enhanced fork

This is the enhanced native game client. README.md is our GitHub landing page;
useful upstream setup and reference material is linked from docs/DEVELOPMENT.md.
The client is licensed under GNU AGPL-3.0-or-later (see LICENSE). Code
published through 0.4.5 and upstream code up to commit 1e23e80 was released
under MIT and remains available under MIT at those commits (see NOTICE).
Third-party code retains its own license. Anyone may play an unmodified official release.
See docs/LICENSE-OPTIONS.md for details.

## Current freeze

This session's client/map/release work is frozen at published 0.1.0. Read
[docs/DEVELOPMENT-FREEZE.md](docs/DEVELOPMENT-FREEZE.md) for audited data sources,
optional companion-folder behavior and retained limits before starting more work.
Do not treat those follow-ups as authorization to continue this work. Other
agents' already-authorized work is unaffected.

The owner explicitly authorized this 0.1.1 patch release to resolve minimap and
world map scaling under render scales 2x and 4x. This targeted patch is not a
broad unfreeze; the client/map scope limits and freeze policies remain in effect.

## Goals and ownership

Keep upstream engine/gameplay improvements while retaining the circular minimap,
NPC/group search, guidance, waypoints, quest guide, Stats Box and optional HUD
numbers. The separate parser project is outside this repository's scope.

The complete file ownership map, exact shared-file budgets and hook table are in
[docs/FORK-OWNERSHIP.md](docs/FORK-OWNERSHIP.md). Its executable policy is
[docs/fork-footprint.json](docs/fork-footprint.json). Fork-owned code includes
questguide/, statbox.rs and
fork_ext/ (including report_core.rs, report_state.rs, report_viewer.rs and report_sync_tests.rs,
the retained local report/crash core, adapter and regression tests) under crates/eqoa/src; mods/, launcher
scripts, AGENTS.md, the fork README/changelog, design/map-parity/ and docs/.

Existing restored systems still touch hud.rs, play.rs, viewer.rs, settings.rs,
options.rs, binds.rs, bindpage.rs, input.rs, touch.rs, ui.rs, hideui.rs, lod.rs,
client_image.rs, main.rs, net/src/session.rs and render/src/{lib,ui,fans}.rs and
ui.wgsl. The exact allowlist also records dependency, mesh, animation and browser
touch points. No map/quest rewrite is part of this cleanup. Isolating restored
features behind a crate or feature flag is a later option requiring its own plan.

The app map family (map/**, tracker.rs, groupmark.rs, mapsuggest.rs) and the render
map module/shader are shared since upstream restored them. Round 2.5 removes their
old fork-owned exemptions; changed files have exact measured budgets and identical
files have no reserved budget. New fork implementation belongs in fork-owned files.

## Extension seams and new features

fork_ext/mod.rs owns the plain-data mods and telemetry services. The app-private
adapters fork_ext/{play,hud,options,settings}.rs are children of their shared
parent modules. Each named hook occurs once: combat event, HUD labels, option
category/items/change/activate/value_text, and settings write/parse.
See the ownership table for exact symbols and call sites.

Put new implementation in fork-owned files. Shared code should only gather app
state, invoke the hook and consume its result. Preserve saved setting keys;
enhanced-only option IDs use 1000+. Process launch and UDP are native-only. Follow
docs/COMPANION-LAUNCH.md for the portable manifest contract. Add behavior tests
with positive and negative controls and update FORK-UPDATE-NOTES.md.

F3 and the HUD report button open the local Save to Desktop dialog (F3 opens
upstream's Support bug-report form instead on a server that takes bug reports); crash reports
are offered locally at the next start. Upstream also keeps a panic for a supporting
server the player logs into (0x7E64-0x7E66). Every crash this build sends carries
`fork_ext::report::enhanced_label()` through the fork-owned send hook in
`fork_ext/report_label.rs`; a server whose limit cannot fit the whole identity
gets no report, and it stays kept. F3's server Support petitions remain upstream's. The retained report core is
fork-owned at fork_ext/report_core.rs; upstream logfile keeps logging through hooks.
The small fork_ext/report_state.rs snapshot is a child of the Play adapter and is
noted at most once a second through its existing frame_library hook. Upstream
state describers remain deleted. Snapshot files are scrubbed before persistence
and export; character names remain visible. A bounded background worker owns
changed-only state writes and the snapshot's enabled-addon cache (refreshed at
most once per 30 seconds, off the frame thread). The crash handler can also read
the latest redacted in-memory snapshot.
Historical HTTP transport exists only in tests against numeric loopback fixtures;
the fork dialog only saves locally; the upstream game-server crash path uses the Enhanced identity hook. The obsolete browser report policy
was removed after upstream deleted browser reporting. Keep map/community
suggestions distinct from bug-report uploads.

## Upstream sync and conflicts

origin is upstream, looprook is this fork, and joukop remains unchanged. Keep
master a fast-forward-only mirror of origin/master; GitHub defaults to enhanced
so its landing page shows the fork README. Never commit fork changes or
push master to looprook. Since 2026-10-30, enhanced is the stable branch and
develop the development branch (CLAUDE.md "Branches and worktrees"): features
merge into develop, merge origin/master into develop through a short-lived
sync/<date> branch, and develop merges into enhanced only for an approved release.
Keep rerere.enabled on. Do not rebase shared enhanced or develop history, force
push it, prune protected worktrees or discard the saved documentation stash.

On a shared-file conflict, retain upstream's implementation and reapply the
named hook. Where restored code still integrates inline, review both sides and
preserve both intents. Run the footprint checker/tests and native/browser checks.
Budgets require explicit review; never increase them merely to silence a failure.

The 2026-10-28 merge at 1bfe228 combines enhanced 8aeaf91 and upstream 8d90350.
Conflicts: README (enhanced landing page, owner override); UPDATE-NOTES (upstream,
fork notes separate); options (retain Shadows ID46, fork IDs1043..1052); settings
(preserve both serialized groups); Play initialization (retain upstream timing,
fishing/GM and fork map state). See docs/UPSTREAM-SYNC.md for validation.
CLAUDE.md is the fork's own (since 2026-10-30); upstream's is kept verbatim in
docs/upstream/CLAUDE.md, which the checker holds equal to upstream at the
reviewed base. On a sync, keep our CLAUDE.md, refresh the copy and review
upstream's changes to it for rules worth adopting.

Local builds have no updater channel. Published builds use channel enhanced and
follow LoopRook/eqoa-enhanced/releases/latest/download/. Official versioned
builds come from this repository: official-release.yml builds a vX.Y.Z tag on
enhanced, tests it and publishes complete verified runtime assets before marking
the release latest. Source CI (public-source.yml) publishes nothing. The private
build repository was retired on 2026-11-07 (docs/PRIVATE-BUILDS.md).
Never use the upstream dev channel or force-move a published version tag. Read
docs/RELEASING.md before publishing; the original hygiene pass itself made no release.

## Handoff references

- docs/FORK-HYGIENE-STATUS.md: execution record, validation and remaining decisions.
- docs/MAP-RELEASE-HANDOFF.md, MAP-PLAYTEST.md, MAP-REFINEMENT.md, MAP-THEMING.md,
  MAP-PARITY.md: map behavior, theme boundaries, known limits and player checks.
- docs/MODDING-AND-PARSER-ROADMAP.md and parser-integration-design.md: broader goals.
- docs/patches/upstream-gm-keyboard.patch: optional upstream proposal, unapplied.

The clean fix-animation-oob-panic branch is prepared for the owner to review,
without pushing or submitting it. Upstream already corrected negative-clock
sampling; the remaining fix guards empty clips and missing character animations.
The fork also retains its existing Anim::frame fallback. Details and named
fixture skips are in the hygiene verification report.
