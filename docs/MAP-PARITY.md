# FrontiersForge Map Parity

## Priority and Ownership

Map parity is the current priority in this chat. Another agent owns parser
implementation and integration. Parser work is not a prerequisite for map work;
this chat maintains parser documentation only. This document is a proposed
walkthrough and acceptance checklist, not a claim that parity is implemented.

The user specifically identified the native map UI, missing NPC search, and
missing tracking as problems. Match Forge's appearance and behavior while
preserving native waypoints, group waypoints, quests, coaches, and community pins.

## Playable Preview — 2026-10-28

The native milestone is implemented on the enhanced line. See MAP-RELEASE-HANDOFF.md for code, design provenance, validation and before/after captures. See `MAP-PLAYTEST.md`
for the separate executable, isolated profile, controls and live-server checks.

- Native temporary NPC / Group / Places search uses the real reference catalog
  and server-visible NPCs; it is independent of the browser study.
- Selecting a result guides on both maps. Gold recorded locations/dashes and
  green live matches coexist. Minimap guidance clips to its rim; Stop clears only
  this guide. Existing quest/coach/camp/community/personal/group marks remain.
- Group guidance uses valid current-world actor/channel-40 positions; unavailable
  positions produce no arrow, and roster departure clears the target.
- Search (F2) and Stop (F4) have persisted Controls rows, button assignment and
  binding-aware labels. Search focus stops movement/look and releases prior holds.
- Personal waypoints can be selected for guidance; updates/arrival/deletion synchronize the guide.
- The minimap is circular, Search clears coordinate chrome, and map labels/XP badges use native theme tokens. Theme-pack loading remains pending.

This is an initial playtest, not full Forge parity. Name fragments are supported;
fuzzy/exact modes, level filters, journal persistence, named location management,
Forge artwork, rotating/indoor height views and theme packs remain outstanding.
The sequence below remains the full-parity acceptance checklist.

## Reference and Current Starting Point

Reference: [FrontiersForge](https://github.com/mmvest/FrontiersForge/tree/4026d5cebcede24943ece8dc34833e584f46cd90),
pinned to `4026d5cebcede24943ece8dc34833e584f46cd90` for repeatable comparisons.

- `scripts/world_map/world_map.lua`: presentation, search panel, result regions,
  selection, locations, and shared tracking.
- `scripts/world_map/modules/entity_search.lua`: fuzzy/exact name search and
  level filtering across journal/database entries.
- `scripts/world_map/modules/entity_db.lua`: recorded spawn database loader.
- `scripts/world_map/modules/journal.lua` and `locations.lua`: personal discovery
  data, saved locations, and sharing.
- `scripts/mini_map/mini_map.lua`: circular presentation, live entities, tracking,
  rotation, and height-aware indoor rendering.

Native already has terrain generation/cache (`map/build.rs`, `store.rs`), camera
and coordinate transforms (`view.rs`), map controls (`chrome.rs`), minimap
(`mini.rs`), markers, and waypoint persistence. Extend these systems in the Rust
client. Lua injection and PCSX2 memory access are not dependencies of this work.

## Proposed Delivery Sequence

### 1. Establish the visual target

Capture Forge and native at matching worlds, locations, zoom levels, and window
sizes. Inventory panel placement, type sizes, spacing, buttons, labels, marker
colors, selection state, and minimap framing. Forge's source has been inspected;
matching in-game visual captures remain outstanding.

Treat interface styling and map artwork as separate tasks. Forge uses handmade
Tunaria art and image maps for the other worlds; native generates terrain tiles.
Choose the corresponding map backgrounds and verify world-to-image calibration
against known landmarks. Record asset sources before bundling anything, respecting
the repository's game-data rules. Keep generated terrain as an available fallback.

Proposed interaction layout: map viewport beside a collapsible search/control
panel, with a stable selected-result and Track/Stop area. Match the reference
before adding stylistic departures. Preserve usable keyboard, controller, and
touch navigation; typing in search must not move the player or trigger map keys.

### 2. Build the searchable NPC catalog

Use three explicitly labeled sources:

- Reference database: NPC names, levels, worlds, and historical spawn locations.
- Personal journal: NPCs this character has encountered, recorded from native
  entity state and persisted locally.
- Nearby entities: what the server currently lets this client see.

The nearby entity list cannot answer where every NPC in the world lives. Broad
search needs a reference dataset or an accumulated journal. Confirm the source,
format, server applicability, and coverage of the reference database before
promising worldwide results. Existing camp markers are not a full NPC database.

Implement the reference's case-insensitive fuzzy/exact names and level filters.
Results show names and level ranges; selecting a result highlights its recorded
spawn areas. Keep world/server distinctions and data provenance when combining
sources. Historical positions are approximate areas, not claims of a live spawn.
Show useful empty, unavailable-data, and other-world states.

### 3. Connect search to tracking

Player workflow:

1. Open the map and search for an NPC.
2. Select a result to see its known spawn areas.
3. Choose Track; keep the active target in shared state, with details on demand
   and a small Stop control on the minimap rim.
4. Close the world map and follow minimap guidance toward the nearest recorded
   location in the current world.
5. Highlight/ping matching live NPCs once they enter the client's entity list.

Keep one shared tracking state for both maps so opening or closing a panel does
not lose the selection. Preserve personal and group waypoints separately.
Stop/replacement, zoning, another-world targets, despawn, multiple matches, and
reconnects need explicit behavior. Persist a search target by its catalog identity,
not a transient network entity ID.

Forge-style guidance is direction/distance to known locations and nearby matches.
Obstacle-aware routing and automatic movement are separate features outside this
parity milestone. The UI must distinguish an approximate recorded location from a
currently observed NPC.

### 4. Integrate minimap search and group-member guidance

Use one minimap with recorded-location guidance and live-entity highlights as
simultaneous layers, as Forge does. Remove the study's permanent tracking card
and nearby notification. Expose target details on hover/focus, with a Stop control
on the rim. A small search control opens a temporary NPC/Group/Places panel;
choosing an eligible result starts guidance and closes the panel.

Native group names/IDs and positions are already decoded in `group.rs` and used
by `Play`'s map-context construction and the existing map/minimap. Follow the
selected member's latest known current-world position. Distinguish current,
stale/unavailable, and other-world positions. Do not guide to default coordinates.
Remove guidance when the selected member leaves the group; retain other group
dots, personal/group waypoints, quests, coaches, and community pins.

The native implementation must carry member identity and position validity into
the map model; the current minimap's position/color pairs alone are insufficient
for a member picker. Data freshness and world checks need explicit tests. This is
new native functionality, not a claim that Forge has an identical group picker.

### 5. Keep the UI themeable

Presentation must be replaceable independently of map data and behavior. Read
`MAP-THEMING.md` for the proposed token/asset contract and parser UI references.
The prototype demonstrates two skins sharing the same controls and guide target;
a native theme-pack loader remains future implementation work.

### 6. Complete the remaining map features

- Personal journal collection, persistence, import/merge, and source selection.
- Named custom locations with notes, search, edit/delete, sharing, and tracking.
- Matching minimap framing and controls, optional player-relative rotation,
  entity filtering, tracking lists, and appearance settings.
- Height-aware cave/dungeon views: ceiling cuts, textured/height-shaded modes,
  and indoor zoom behavior. This needs a geometry-rendering change; current
  cached terrain tiles alone do not provide Forge's indoor view.

Deliver these incrementally after the UI/search/track workflow. They remain part
of full parity, even if the first useful release precedes them.

## Input Bindings

Use the existing Controls page and `binds::ROWS` for every keyboard/controller
shortcut. The current client already remaps Map (`map`, M), Minimap (`minimap`,
N), zoom (`map_in` / `map_out`, + / -), and group waypoint
(`map_group_waypoint`, G). Map actions also reuse gameplay rows: Q waypoint is
`talk`, X markers is `autorun`, T towns is `tell`, J quests is `look_left`, and
Space center is `attack`. Moving a shared row changes its map behavior too.

The native keyboard legend now resolves those labels from `settings::with_binds`
and `Binds::key`; it must never advertise stale defaults after a rebind. The
browser study's M is an illustrative default, not a read of saved client settings.

Ship normal actions with working defaults; unbinding is an explicit player
choice. The current `binds` regression suite checks unique default keys and swap
behavior when a player assigns an occupied primary key. Keep gameplay, map, and
text-entry dispatch contexts exclusive so one press cannot trigger two actions.

Planned defaults for the new functionality (not wired into native dispatch yet):

| Action | Controls ID | Default |
| --- | --- | --- |
| Open/close map | `map` (existing) | M |
| Toggle minimap | `minimap` (existing) | N |
| Map zoom | `map_in` / `map_out` (existing) | +/= and -/_ |
| Open minimap search | `map_search` (new) | F2 |
| Stop guidance | `map_stop_guidance` (new) | F4 |

F2 and F4 are unused by the existing default table. Recheck that at implementation
time; give these actions their own rows and cover duplicate assignment, swapping,
reset-to-defaults, saved bindings, and typing focus before shipping. Confirming a
search result uses the focused panel's normal Confirm action, not a global
shortcut that also centers the map.

When implementing minimap search and Stop Guidance, add dedicated Controls actions
before assigning shortcuts. Use action IDs for dispatch and current binding
labels/glyphs for hints. Keyboard text-entry focus must consume typed letters
without toggling the map or moving the player. Themes supply styling, never
literal key labels. Controller glyphs must likewise resolve the current physical
binding; the existing static controller legend still needs that separate audit.

## Acceptance Checks

| Area | Evidence required |
| --- | --- |
| Presentation | Matched-location Forge/native captures; review controls and background art separately at multiple window sizes. |
| Search | Known NPCs, partial/exact queries, level bounds, duplicates, empty results, and world/source filters. |
| Tracking | Search-to-minimap workflow, stop/replace, multiple spawns, live match, lost match, and world changes. |
| Group guidance | Pick a member, follow position changes, handle unavailable/other-world data and group departure without stale/default guidance. |
| Themes | Change palette/font/frame tokens without resetting tracking, filters, zoom, quests, coaches, or waypoints. |
| Persistence | Journal and saved locations survive restart; imports preserve existing data and report malformed inputs. |
| Inputs | Search typing, mouse selection/drag, controller focus, touch controls, and map-close behavior. |
| Existing features | Quests, coaches, community pins, personal/group waypoints, and player/group positions still work. |
| Indoor/performance | Cave floors shown instead of roofs; incremental rendering/search avoids game-thread stalls. |

First useful milestone: a Forge-style map interface with NPC search and tracking
working together in the native client. Full parity additionally requires the
journal, location management, minimap settings, indoor rendering, and visual checks.
