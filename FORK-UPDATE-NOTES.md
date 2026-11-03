# Enhanced fork update notes

Upstream changes remain in UPDATE-NOTES.md.

## 2026-11-04 · Enhanced 0.4.4

- Fixed: with Wereoxxs UI's chat on, holding R2 hid the ability bar and scrolled nothing you could see. R2 now scrolls the mod's chat up and down, and letting go returns to the newest line.
- Holding R2 to read chat history now keeps the ability bar and minimap on screen, with or without mods.
- Problem reports (F3 or the HUD's Report a bug, and the offer after a crash) still save a ZIP to your Desktop. They now also include a short note of what the game was doing: zone, target, nearby creatures and the game's mode. Your account name, password and Windows user folder are removed; nothing is uploaded.
- From the vanilla client: Touch Pad Size (Extended > Controller) for the on-screen controller; Class Mastery fixes, so Master Classes that branch (such as Spellbinder's) no longer refuse their second branch, and on eqoaclassic a Master Class's abilities can be bought again after reopening the window; plus speed-ups and fixes.

## 2026-11-02 · Enhanced 0.4.3

- Fixed: the creature journal forgot NPCs above level 60 (and ones of unknown level) every time you restarted the game, and importing a FrontiersForge journal skipped them. They are now kept.

## 2026-11-01 · Enhanced 0.4.2

- Fixed: map search listed other players near you by name. It now lists only NPCs and creatures, never players or their pets. Your creature journal never saved players and is unchanged.
- Fixed: hovering over a player on the minimap called them an NPC; it now says Player (or Pet).

## 2026-11-01 · Enhanced 0.4.1

- Mod profiles: each character loads its own mods, settings and window layout. New characters can use a shared Tank, Healer, Melee or Caster profile (picked from their class) or make their own copy; manage profiles in the Addon Manager.
- ARPG UI and Avoid's Modern UI are now included next to Wereoxxs UI, and the game adds any missing included mods to your mods folder, even if you update automatically. They stay off until you turn them on. If you had removed one before, it is added back once (still off); delete it again and it stays gone. Included mods also update themselves, but only files you haven't edited.
- Wereoxxs UI has a weapon-set bar: turn it on in its settings; click a set to use it.
- The map now learns creatures as you play: every NPC you see goes into your own creature journal, and map search finds them alongside the built-in list. Only creatures are recorded, never players or pets. Export your journal or import a friend's (also works with Wereoxx's PCSX2 map mod files).
- Mods stay visible while you talk to NPCs (coachmen, quest givers) and step aside when the full map is open.
- Fixed: mod ability bars sometimes missing cooldowns, especially short ones such as Feed.
- Fixed: Avoid's Modern UI couldn't be turned on at level 60; its XP bars now hide at max level.

## 2026-11-01 · Enhanced 0.3.0

- Two more FrontiersForge mods now run natively, unchanged: ARPG UI (health and power orbs, action tray) and Avoid's Modern UI (player, target, group and quest panels). Like Wereoxxs UI, they step aside for game menus. Turn on one at a time in the Addon Manager.
- Problem reports: press F3 (or use the crash offer at the next start) and choose Save to Desktop. You get an EQOA-Enhanced-report zip with your note, a screenshot and the logs, ready to send to us. Your account name, password and Windows user folder are removed, and nothing is uploaded anywhere.
- Mods that save files are safer: writes are batched, and mods can no longer freeze the game through file access.

## 2026-11-01 · Enhanced 0.2.2

- Mods now step aside for every game window: the spellbook, quest journal, options, merchant, bank, trainer, blacksmith, auctions, trade, mail and more.
- With a mod hiding the game's ability bars, the game's item bar no longer pops back when you scroll the hotbars.
- With a mod's chat on, holding C or R2 no longer brings up the game's chat history, and the minimap stays put.
- Fixed mods stopping (and the whole mod UI vanishing) after a few hours of busy chat.
- Mod settings now stay with you when you move or re-extract the game folder; existing settings carry over automatically.
- If a mod ever stops with an error, the game's own HUD comes straight back.

## 2026-10-31 · Enhanced 0.2.1

- Brings in the latest improvements from the original vanilla client, with all Enhanced features and mods unchanged.
- New Ground Fog option under Options, Extended, World (the PS2 look by default, or Exact); Shadows moved to the Characters page.
- Duels, with an orange marker on the compass, on servers that turn duels on.
- Sound no longer cuts out in big crowds and busy fights, and far-away or blocked creature swings are quieter.
- Walking while casting matches the PS2, with natural leg movement.
- Glowing weapons, crystals, eyes and spell particles shine as brightly as on the console again.
- Falling from Sandstorm's boon no longer hangs in midair behind a loading screen.
- Master Class rank upgrades can be bought again.
- On servers that offer them: group search (LFG), stack auctions and mail to several people at once.

## 2026-10-31 · Enhanced 0.2.0

- Mods: EQOA Enhanced can now run FrontiersForge UI mods natively, no emulator needed. The game creates the mods folder automatically if missing. Turn mods on in Options, Extended, Mods & Addons, Addon Manager. Mods are off until you turn them on.
- Included: Wereoxxs UI is now bundled directly in the release ZIP inside the mods folder (turn it on in the Addon Manager). It brings customizable ability bars with the game's icons and cooldowns, casting bar, player, target and group frames, buffs, compass, chat and damage meter. Other Forge mods are experimental for now.
- Upgrading from 0.1.x: Players updating automatically get native mod support, but should download the 0.2.0 ZIP once to get the included Wereoxxs files (or copy the mods folder from the ZIP into their game folder).
- Mods can hide parts of the game's own HUD (ability bars, buffs, compass, chat window, target, group, pet, health, power and experience bars), and everything comes back when you turn the mod off.
- Press Enter to type in a mod's chat box; game pop-ups and menus still get Enter first.
- Mod window positions, sizes and settings are kept between sessions.
- If a mod has a problem, the Addon Manager shows it and mods/addons_error.log records it.

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
