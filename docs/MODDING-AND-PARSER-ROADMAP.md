# EQOA Enhanced Client, Parser & Map Roadmap

## Goals and Current Status

This branch (`enhanced`) extends the native client while retaining upstream engine updates. Goals are usable QoL features, a working combat parser, community addons, and world-map/minimap parity with FrontiersForge in both appearance and function.

**Current ownership:** another agent handles parser implementation and integration. This chat documents parser findings and prioritizes map parity now, independently of parser completion. See `MAP-PARITY.md` for the map walkthrough and acceptance checklist.

**Status reviewed October 28, 2026: incomplete.** The user reports that the parser does not work and the maps do not match Forge. Earlier "all core deliverables completed" and "zero-configuration" claims described code added, not verified player workflows. A compiled executable or a passing receiver test is insufficient to close either goal.

**Map milestone delivered:** native NPC / Group / Places search, shared guidance,
personal-pin tracking, circular minimap, measured toolbar placement, themed labels
and XP badges, remappable controls and inline Suggest. See MAP-RELEASE-HANDOFF.md
for source, screenshots, packaging, tests and live acceptance checks. Full Forge
parity remains incomplete. Parser findings below are historical review evidence;
another agent may have changed the companion since that review.

## Implementation Inventory

| Area | Present | Remaining acceptance work |
| --- | --- | --- |
| QoL restoration | World map, minimap, quest markers/tracker, Stats Box (`bd50b49`) | Verify after upstream merges; restoration does not establish Forge parity. |
| Combat telemetry | Rust localhost UDP sender and Python `ClientSocketSource` (`2007dad`; parser `0dd87f4`) | Verify live combat end to end, attribution, event coverage, and connection status. |
| HUD enhancements | Target HP %, XP Details, HUD Numbers (`5e64004`) | Validate settings and displayed values. |
| Addon infrastructure | Manifest discovery, mods folder, parser launch shortcut (`5e64004`) | Define a real addon contract; arbitrary resources and enable/disable controls are not integrated into a general runtime. |
| Distribution | Local executable, batch scripts (`c2ac4aa`), desktop launcher (`482abb1`, `8cd81aa`) | Portable installation, consistent launching, actionable failures, and release packaging. |

## Parser: Architecture and Confirmed Gaps

The implementation is currently an **external Python companion**, reading JSON events from the Rust client at `127.0.0.1:28015`. It is not an in-process game mod. The file `mods/eqoa_parser/mod.json` is a descriptor; the Rust launch function hard-codes commands instead of using the manifest's command and arguments.

The user selected external integration with [LoopRook/EQOA-Parser](https://github.com/LoopRook/EQOA-Parser). Treat it as a companion tool, with consistent installation, launch controls, and status. The proposed implementation is in `parser-integration-design.md`.

Evidence from the October 28 review:

- The desktop launcher invokes `eqoa_parser/app.py` directly. Reproducing this command fails with `ImportError: attempted relative import with no known parent package`. The package entry point is `python -m eqoa_parser --client`.
- Desktop, batch, in-game, and manifest launch paths disagree. Several hard-code `C:\Users\hocka\Github Projects\EQOA Parser`; Python discovery is also machine-specific. A successful process spawn does not prove parser readiness.
- `ClientSocketSource` reports active traffic unconditionally. It cannot currently distinguish waiting for a client from receiving combat events.
- The Rust event hook is in `Play::damage_indication`; ability is always `melee`. JSON is assembled manually. Ability attribution, group damage classification, and multibox session separation need explicit validation and contracts.
- Existing CI archives omit the enhanced launcher, mods folder, and companion dependencies. Local startup scripts are not a portable distribution.
- The parser's existing packaged GUI executable requests administrator privileges for packet capture. Native-client mode needs a windowed build without that requirement.
- `packaging/entry.py` omits `--client` from its source flags, so the packaged live build prepends `--live` to `--client`; CLI validation rejects that combination. This was reproduced by checking the packaged argument-selection helper and CLI validator without starting the UI.

Verification: `python -m pytest -q -p no:cacheprovider tests/test_client_source.py` passes (1 test) in the parser checkout outside the execution sandbox. The sandbox blocked even a raw localhost UDP control, so its timeout is not evidence of a product defect. The earlier 1,023-test claim is historical and was not reverified. No live game-to-meter success has been verified.

## FrontiersForge Map Parity

Reference: [mmvest/FrontiersForge](https://github.com/mmvest/FrontiersForge), reviewed at commit `4026d5cebcede24943ece8dc34833e584f46cd90`. Inspect `scripts/world_map/world_map.lua`, its modules, and `scripts/mini_map/mini_map.lua` when implementing parity. This requirement was missing from the earlier roadmap.

| Capability | Forge reference | Native client baseline / gap |
| --- | --- | --- |
| World-map presentation | Handmade Tunaria art; per-world maps; configurable labels, tint, indicators, and control panel | Generated terrain retained; compact native theme delivered. Forge artwork/calibration and matched Forge visual verification remain. |
| Creature search | Name and level filters, recorded spawn regions, selected results | Native case-insensitive name search delivered across recorded/live NPCs; level/fuzzy filters remain. |
| Discovery journal | Persist encountered creatures; merge/share journal data | No equivalent map journal found. |
| Saved locations | Named custom locations, search, persistence, sharing | Personal/group waypoints and community pins exist; no equivalent location manager found. |
| Shared tracking | World-map selection guides minimap toward a spawn and highlights nearby matches | Shared NPC/group/personal-pin guidance delivered. Moving group and zoning checks remain. |
| Indoor minimap | Height-aware geometry, automatic ceiling cuts, textured/height-shaded modes | Reuses terrain tiles; no equivalent cave/ceiling rendering found. |
| Minimap controls | Optional player rotation, entity filters, tracking lists, appearance settings | Circular north-up HUD minimap with zoom, search and guide delivered; rotation/filter/list parity remains. |

Completion requires both a behavior checklist and visual comparisons against the pinned Forge reference, covering outdoor terrain, towns, caves/dungeons, multiple worlds, search, saved data, and tracking. A map opening successfully is not parity. Keep game data out of the repository; assess provenance before reusing external map art or databases.

## Next Milestones

1. Prioritize Forge-style map presentation, NPC search, minimap-integrated search, and NPC/group-member guidance. Keep themes separate from behavior (`MAP-THEMING.md`). Follow `MAP-PARITY.md`; parser work does not block this.
2. Complete map parity with discovery journals, saved locations, minimap controls, indoor rendering, and matched visual verification.
3. Parser implementation/integration remains with the other agent. Preserve this document's diagnostic evidence for that work; this chat makes no parser code changes.
4. Revisit a general addon framework separately from map parity and built-in HUD features.

## Upstream Synchronization

Merge upstream engine changes into `enhanced`, preserving enhanced features:

```sh
git fetch origin master
git merge origin/master
```

Run relevant regression checks after each merge. Release workflows currently target `master`; verify enhanced packaging explicitly before distributing it.
