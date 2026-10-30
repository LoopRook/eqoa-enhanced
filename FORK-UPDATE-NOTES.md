# Enhanced fork update notes

Upstream changes remain in UPDATE-NOTES.md.

## 2026-10-31 · Enhanced 0.1.2

- NPC search works even if map-entities.json is missing or the game is started from inside the ZIP.
- Minimap search now preserves continuous autorun and follow movement while the search query panel is open.
- The program uses the selected EQOA icon on the window, taskbar and eqoa.exe:
  DEV in development builds, STANDARD in official releases. It no longer changes
  to the memory-card save icon after the disc is chosen.

## 2026-10-28 · Enhanced 0.1.1

- The circular minimap and full world map keep their position, dimensions, backdrop coverage, and UI alignment when Render Scale changes (2x and 4x).

## 2026-10-28 · Enhanced 0.1.0

- Download one Windows ZIP, extract and double-click eqoa.exe. No build tools or BAT required.
- Automatic updates and update notes now come from Enhanced versioned releases.
- Preserve upstream's GM Goto fix: find the floor again after destination zones load.
- Retain the enhanced map, guidance, quests, HUD options and disabled upstream report submission.

## 2026-10-28 · Upstream integration and fork hygiene

- Integrate upstream fishing, GM tools, character shadows and housekeeping while preserving enhanced features.
- World keeps Shadows; Map Markers moves to Map & Quests. Saved setting names remain unchanged.
- Enhanced option IDs use a separate transient range, avoiding future upstream collisions.
- Preview archives include the upstream MIT notice.
- Report button, F3 submission and automatic uploads are disabled. Local crash diagnostics remain available.
- Companion launch uses mod.json and EQOA_PARSER_DIR without machine-specific paths or a Windows console window.
- Mods, telemetry and HUD-number options use fork-owned adapters to reduce upstream conflicts.
- Empty animation clips no longer enter frame division/modulo; valid animation timing follows upstream.

## 2026-10-28 · Minimap hover and compact search

- Hover NPC/player dots for name, level when known, kind and distance.
- The large minimap Search button is now a small magnifier. Mouse-wheel zoom
  remains, and guidance details appear while hovering.
- NPC search says Spawned nearby when a living match is currently observed.
  Not detected means out of range or not spawned; the client cannot tell which.
  Guidance follows a live match first and falls back to recorded locations.
- Returning focus with no controller no longer reports a controller disconnect.

## 2026-10-28 · Map text clarity

- Map names, XP labels and marker tooltips now retain the font's full detail when
  enlarged. Their size, wrapping and layout stay the same.

## 2026-10-28 · Map search and guidance preview

- The minimap is now circular. Search stays below the map coordinates and status messages.
- Set a personal waypoint and choose Track waypoint to follow its pink guide on both maps.
- Map labels and XP camp badges share the compact theme; zoomed-out camps use small neutral markers.

- The native map now carries the compact dark UI treatment with thin borders, red focus and selection, and Space Mono text. Mouse/keyboard Suggest keeps the map visible while naming and annotating a community marker.

- Find NPCs, group members and camp/coach locations in a temporary map panel. F2 opens search; F4 stops guidance. Both can be changed in Controls.
- Follow recorded spawn locations on the minimap while actual nearby matches highlight separately. Group guidance follows valid same-world positions.
- After changing keys in Controls, the map's keyboard legend now shows the keys
  you chose, including zoom, waypoints, markers, towns, and quest markers.
- A control with no keyboard key assigned is shown as unbound.
