# Native Map Playtest

This is the playable native map milestone on the enhanced line. The preview
uses a separate executable and profile. No public release was published; parser
code is untouched. See MAP-RELEASE-HANDOFF.md for implementation and validation.

## Launch

Extract the complete build folder and double-click its EQOA executable. In this
preview its filename starts with eqoa-map-preview-. Select your locally owned
Frontiers disc if prompted, then sign in normally. No build commands are needed.
Keep map-entities.json and the supporting files beside the executable.

For a separate test profile, optionally run Play Map Preview.cmd instead.
Play Map UI Preview.cmd is an equivalent shortcut. These select the same EXE
with an isolated profile/; account files are not copied from your normal client.
Developer build and packaging instructions belong in docs/DEVELOPMENT.md.

## Controls and behavior

- **M**: open/close the existing world map. **N**: toggle minimap.
- **F2** or the **magnifier** beside the minimap: open a temporary search panel.
- Type part of an NPC name. **Tab** switches NPCs / Group / Places. Arrow keys
  select; **Enter** or a result click starts guidance and closes search.
- Set a personal pin with the map click/waypoint control, then choose **Track waypoint**, or **Personal waypoint** under Places. Its pink guide follows pin changes and stops on arrival/deletion.
- **F4** or the minimap's **X**: stop guidance. Personal/group waypoints remain.
- Rebind these actions in **Controls**; displayed shortcut labels follow saved
  bindings. Controller buttons can be assigned to Search/Stop there too.

NPC search combines Forge's recorded locations with actual nearby NPCs received
by the client. Gold marks/dashes show historical locations; green marks show
matching live NPCs. Both can appear simultaneously. A historical location never
proves a creature is spawned. Sightings disappear when the server removes them. Search shows **Spawned nearby**
for a current living NPC observation, and **Not detected (range/spawn unknown)**
for an unobserved recorded name. Not detected never means confirmed dead or
despawned. A result guides to the nearest live match when available, then falls
back to historical spawn coordinates. This is not a server-wide spawn query.

Hover an NPC/player dot for its name, reported level (or unknown), kind and
distance. Corpse/inactive/other-world entities do not produce living dots.
The minimap search button is a compact magnifier; hover it for its saved binding.
Mouse-wheel zoom remains. Guide details appear on hover rather than in a
permanent strip; the small X still stops guidance. The yellow ? is a quest step.

Group guidance follows a selected member's actor position, or an active group
position update in the same world. Unavailable or other-world positions produce
no direction arrow; leaving the group clears selection. Guidance indicates a
direction and distance; it does not find paths around obstacles or move you.

## Native UI and Suggest

The preview now includes Track Level Companion / parser-inspired dark surfaces, thin borders, red
selection/focus, compact controls and an embedded SIL OFL Space Mono font. These
are rendered natively, including map labels and compact XP camp badges. The minimap
is circular, with markers on its rim and search outside the circle. The normal game HUD and other windows retain their styling.

Mouse/keyboard Suggest keeps the map open with Name, Note, Submit and Cancel in
one form. The selected marker position, server submission and approval feedback
are preserved. Escape cancels without submitting. Controller use retains the
existing two-step soft-keyboard editor. No real community submissions were made
by the automated checks; its test HTTP endpoint runs on loopback.

## Acceptance checks in the game

1. Search for a nearby NPC; guide, close M, walk and verify the minimap line.
2. Approach a recorded spawn; confirm live matches highlight independently.
3. Group with another player; guide to them, move apart, zone, and leave group.
4. Type names containing movement/combat keys; verify no gameplay action fires.
5. Rebind Search/Stop, restart, and verify the keys and displayed labels agree.
6. Set a personal pin, Track waypoint, move it, arrive and clear it; confirm guidance updates or stops appropriately.
7. Move the pointer across the world map: Search must stay below coordinates/notices. Check circular minimap edges, marker hover, and wheel input.
8. Hover ordinary NPC/player dots and verify name, level, kind and distance; check blank terrain and clipped corners show no entity tooltip.
9. Alt-Tab with no controller and return/click search: there must be no controller-disconnected warning. With a real controller, confirm a genuine unplug still warns.
10. Check quests, coaches, camps, community pins and group waypoints. Open Suggest, check location/name/note, then cancel without submitting.

## Current limits

This is a playtest build, not a declaration of full Forge parity. Search currently
uses case-insensitive name fragments. Fuzzy/level filters, a persisted personal
journal, saved named locations, Forge artwork, rotating/height-aware indoor maps,
and loadable theme packs remain in `MAP-PARITY.md`. The shipped reference dataset
is a historical snapshot and may differ from the connected server.

Automated tests and deterministic native map renders are recorded separately;
live-server play, moving group members and real despawn behavior require the
in-game acceptance checks above.

## Reference data

`map-entities.json` is external, untracked preview data from FrontiersForge at
`4026d5cebcede24943ece8dc34833e584f46cd90`,
`scripts/resources/entity_database.json`. Its MIT notice accompanies the preview.
Native searches read `EQOA_MAP_DATABASE` when set, otherwise `map-entities.json`
beside the executable. Never commit the local database or disc image.
