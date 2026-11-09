# Enhanced fork update notes

Upstream changes remain in UPDATE-NOTES.md.

## Unreleased

- Added the experimental native `eqoa/1` mod API and an EQOA Example Hearts mod. Enable it from the Addon Manager after installing bundled mods. The API remains under review until the Round 5 audit.
- Mods share bundled resources read-only and keep file changes in private overlays. Unsupported calls appear in the Addon Manager and its error log.
- Effects and controller reads use native game state; existing Forge UI mods keep their compatibility interface.
- Mod chat queued while Combat Record is open waits until it closes. World changes cancel unsent mod chat with a visible Addon Manager diagnostic.

## 2026-11-08 · Enhanced 0.4.14

- Nothing changes in how you play. Behind the scenes, Enhanced's own options are kept apart from the vanilla client's, so new vanilla options arrive sooner and with fewer surprises.

## 2026-11-08 · Enhanced 0.4.13

- New from the vanilla client: texture packs made for PCSX2 work here as they are. Put the pack's files in `%APPDATA%\eqoa-rs\textures\SLUS-20744\replacements` (made for you the first time), then turn on Options > Extended > Textures > Texture Packs.
- Dump Textures, on the same page, saves each texture the game draws to `textures\SLUS-20744\dumps` under PCSX2's names, so a pack you make from them works in PCSX2 too.
- Both are off until you turn them on, and the game looks exactly as before.
- On servers with their own races, the races past the game's original twenty (such as Orc, Kobold or Gargoyle on the Heirloom server) now offer every class the server allows at character creation; before, they showed no class to pick.

## 2026-11-08 · Enhanced 0.4.12

- New from the vanilla client, on servers that have them: a Combat Record under Start > Character (your PvP kills, deaths, streaks and duels; /record in chat), and more races to make at character creation, with every class open to every race where the server allows it.
- New characters' names may be three letters long.
- /bug opens the bug report form, as F3 does, on servers that take bug reports.
- The raid list shows each member's power as a thin blue bar under their health.
- The spell book has Unmemorize for one ability, and a long spell description scrolls with R1/R2, the mouse wheel, or PageUp/PageDown.
- New players start with Auto Attack on. If you already saved your options, your choice stays.

## 2026-11-07 · Enhanced 0.4.11

- Nothing changes in how you play. This release is built entirely from the public source code, including the live server connection code that earlier releases kept private. Downloads and auto-updates work as before.

## 2026-11-07 · Enhanced 0.4.10

- Fixed: mod target boxes (Avoid's Modern UI Target Level, ARPG UI, Wereoxxs UI) showed "(none)" or nothing for some targets on servers that show more than 24 people and creatures, such as eqoaclassic. They now show whatever you target, and your pet and group are found the same way.

## 2026-11-07 · Enhanced 0.4.9

- New from the vanilla client: in a raid group on a server that has them, the group's bars become a raid list of everyone in it. Click a name to target that member. Turn it off in Options > Extended > Map & Quests > Raid List.
- Copy and paste: Ctrl+V, Ctrl+C and Ctrl+X work in the game's text boxes. Drag over chat lines with the mouse to select them, and Ctrl+C copies them.
- Controller macros on the Controls page.
- GMs can fly with /fly on servers that have the command.
- A server's own races (such as eqoaclassic's Hill Giant) show under their own name, plus other fixes.
- F3 on a server that takes bug reports now opens the game's Support form. Everywhere else, F3 still saves a problem report to your Desktop, and the HUD's Report button always does.
- EQOA Enhanced is now under the GNU Affero General Public License v3 or later, like the vanilla client. Nothing changes in how you play. Earlier versions stay under MIT (see NOTICE).

## 2026-11-04 · Enhanced 0.4.8

- Minor update.

## 2026-11-04 · Enhanced 0.4.7

- Minor update.

## 2026-11-04 · Enhanced 0.4.6

- LoopRook's additions first published in 0.4.6 or later use the EQOA Enhanced License; earlier code stays MIT, upstream and third-party licenses stay intact, and anyone can still play the official release.

## 2026-11-04 · Enhanced 0.4.5

- Minor client update.

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
