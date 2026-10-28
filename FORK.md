# EQOA Enhanced fork

This is the enhanced native game client. README.md is our GitHub landing page;
useful upstream setup and reference material is linked from docs/DEVELOPMENT.md.
The existing MIT license and third-party notices remain intact. The owner has
not selected different licensing terms; compare docs/LICENSE-OPTIONS.md.

## Goals and ownership

Keep upstream engine/gameplay improvements while retaining the circular minimap,
NPC/group search, guidance, waypoints, quest guide, Stats Box and optional HUD
numbers. The separate parser project is outside this repository's scope.

The complete file ownership map, exact shared-file budgets and hook table are in
[docs/FORK-OWNERSHIP.md](docs/FORK-OWNERSHIP.md). Its executable policy is
[docs/fork-footprint.json](docs/fork-footprint.json). Fork-owned code includes
map/, questguide/, tracker.rs, groupmark.rs, mapsuggest.rs, statbox.rs and
fork_ext/ under crates/eqoa/src; the render map module/shader; mods/, launcher
scripts, AGENTS.md, the fork README/changelog, design/map-parity/ and docs/.

Existing restored systems still touch hud.rs, play.rs, viewer.rs, settings.rs,
options.rs, binds.rs, bindpage.rs, input.rs, touch.rs, ui.rs, hideui.rs, lod.rs,
client_image.rs, main.rs, net/src/session.rs and render/src/{lib,ui,fans}.rs and
ui.wgsl. The exact allowlist also records dependency, mesh, animation and browser
touch points. No map/quest rewrite is part of this cleanup. Isolating restored
features behind a crate or feature flag is a later option requiring its own plan.

## Extension seams and new features

fork_ext/mod.rs owns the plain-data mods and telemetry services. The app-private
adapters fork_ext/{play,hud,options,settings}.rs are children of their shared
parent modules. Each named hook occurs once: combat event, HUD labels, option
category/items/change/activate/value_text, and settings write/parse/description.
See the ownership table for exact symbols and call sites.

Put new implementation in fork-owned files. Shared code should only gather app
state, invoke the hook and consume its result. Preserve saved setting keys;
enhanced-only option IDs use 1000+. Process launch and UDP are native-only. Follow
docs/COMPANION-LAUNCH.md for the portable manifest contract. Add behavior tests
with positive and negative controls and update FORK-UPDATE-NOTES.md.

Report submission is disabled by default, including button/F3/API/crash paths.
Local diagnostics remain usable. EQOA_REPORTS accepts only an explicitly selected
numeric HTTP loopback fixture; it cannot route reports to upstream. Browser
fixture selection is documented in the report policy tests. Keep map/community
suggestions distinct from bug-report uploads.

## Upstream sync and conflicts

origin is upstream, looprook is this fork, and joukop remains unchanged. Keep
master a fast-forward-only mirror of origin/master; GitHub defaults to enhanced
so its landing page shows the fork README. Never commit fork changes or
push master to looprook. Merge origin/master into enhanced through a short-lived
sync branch. Keep rerere.enabled on. Do not rebase shared enhanced history, force
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
CLAUDE.md retains upstream text plus one FORK.md scope override.

Local builds have no updater channel. Before publishing fork CI builds, change
the updater release URLs from upstream to this fork; otherwise enhanced players
would receive vanilla replacements. No release workflow is triggered here.

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
