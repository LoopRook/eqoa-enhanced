# EQOA Enhanced fork

This is the enhanced client, not the vanilla distribution. The root README.md is
our GitHub landing page; upstream's technical ledger is linked from DEVELOPMENT.md.

## Goals and quick start

Keep upstream engine/gameplay work while retaining the circular minimap, NPC/group
search and guidance, waypoints, quests, Stats Box, HUD options and companion hooks.
The parser repository is separate and remains untouched by this hygiene pass.
Use tools/build-map-preview.ps1 for a complete Windows map bundle, or the prepared
Start-EQOA.bat installation. See docs/MAP-PLAYTEST.md for acceptance checks.

## Ownership and merge rules

- origin is upstream; looprook is this fork; joukop remains unchanged.
- master is a fast-forward-only upstream mirror. Never commit or push fork work
  there. Merge origin/master into enhanced on a short-lived sync branch.
- Fork-owned: AGENTS.md, FORK.md, FORK-UPDATE-NOTES.md, docs/, design/map-parity/,
  mods/, launcher/start scripts, and the enhanced map/guide/HUD helper modules.
- Shared integration: main.rs, play.rs, viewer.rs, hud.rs, options.rs, settings.rs,
  binds.rs, bindpage.rs, net/src/session.rs, render/src/{lib,ui,map}.rs and shaders.
  Existing restored systems stay in place; avoid a map/quest rewrite.
- New fork behavior belongs in fork-owned files. Keep a single call per hook in
  shared code. Mods, telemetry and HUD-number cleanup will share one boundary.
- Preserve both intents on a conflict. Prefer upstream implementations plus the
  documented hook where a hook exists; do not overwrite a shared file wholesale.
- Enhanced-only option IDs use 1000+; saved setting keys do not change.
- LICENSE and third-party notices remain intact. No replacement license selected.

## Current integration record

The pending synchronization joins enhanced 8aeaf91 with upstream 8d90350.
Conflicts: README, update notes, options, settings, Play initialization. Resolutions
preserve enhanced behavior, upstream shadow settings and server timing/fishing/GM
state. README remains enhanced by the owner's explicit clarification; CLAUDE has
only one scope override. Fork player notes are separate from upstream's notes.

A native GM keyboard omission found upstream is retained as a proposed patch in
docs/patches/upstream-gm-keyboard.patch, not introduced as extra gameplay behavior.
Further ownership/footprint details and hygiene acceptance are being recorded in
docs/FORK-HYGIENE-STATUS.md.

## Reference

- docs/MAP-RELEASE-HANDOFF.md, MAP-REFINEMENT.md, MAP-THEMING.md, MAP-PARITY.md.
- docs/MODDING-AND-PARSER-ROADMAP.md and parser-integration-design.md.
- docs/DEVELOPMENT.md and docs/LICENSE-OPTIONS.md.
