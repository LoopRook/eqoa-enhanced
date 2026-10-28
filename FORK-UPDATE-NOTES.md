# Enhanced fork update notes

Upstream changes remain in UPDATE-NOTES.md.

## 2026-10-28 · Upstream integration and fork hygiene

- Integrate upstream fishing, GM tools, character shadows and housekeeping while preserving enhanced features.
- World keeps Shadows; Map Markers moves to Map & Quests. Saved setting names remain unchanged.
- Enhanced option IDs use a separate transient range, avoiding future upstream collisions.
- Preview archives include the upstream MIT notice.

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

## 2026-10-28 · Character select smooth at Render Scale x2 and x4, and HUD Nearest by default
- With Render Scale at x2 or x4 (Options, Extended, Video), character select and the other
  screens before the world are smooth again. The HUD's Sharp filter (Extended, UI, HUD), which
  draws the menus' art, did far more work for each pixel than it needed, and those screens
  are covered in that art from edge to edge: at the larger scales a browser could fall below
  30 frames a second there while Freeport, crowds and all, kept up. Sharp now does about a
  quarter of the work and looks exactly as before; the start menu's windows and the HUD in
  the world are lighter with it too.
- HUD is back to Nearest by default, the PS2's own way of drawing the menus' and the HUD's
  art, which costs the least to draw. Sharp is still on that row for anyone who wants every
  texel of that art drawn evenly at any window size (if you had chosen it, choose it again).

## 2026-10-27 · A sharp HUD by default, and the world's textures smooth again
- The HUD's and the windows' art is drawn sharp now unless you choose otherwise, every pixel
  the same size at any window size, as the text already was. Options > Extended > UI > HUD
  puts back the game's own look (Nearest).
- Options > Extended > World > Textures is gone: the world's textures are drawn smooth, as
  the game draws them. Crisp did not suit them.

## 2026-10-27 · Render Scale, and a sharper HUD and textures if you want them
- Options > Extended > Video > Render Scale: x2 or x4 draws the whole picture two or four
  times larger and shrinks it to fit your window, which smooths jagged edges, leaves, fences
  and thin things. x1, the default, is as before. x4 needs a good graphics card, and on a big
  window it draws at the most your card allows (the page tells you when it does).
- Options > Extended > UI > HUD: Sharp draws the HUD's and the windows' art with every pixel
  the same size at any window size, the way Text: Sharp draws the letters. Nearest, the
  default, is the game's look.
- Options > Extended > World > Textures: Crisp draws the world's walls, props, characters
  and plants sharp, each pixel of their textures a flat square. The ground, the sky, water
  and glows stay smooth, so roads still blend into grass and snow. Smooth, the default, is
  the game's look.
- Text (Sharp or Smooth) is on the UI page now, beside HUD.

## 2026-10-27 · The map, the minimap and the extra bars are gone
- This client keeps to the game: the additions it never had are taken out. Gone are the map
  (M) with its waypoints, group waypoints, XP camps, coaches, routes and community markers, the
  minimap (N), the quest markers, the quest tracker and the quest guide behind them, the Stats
  Box (B), the power bars under your group, your target and its target, and the Target of
  Target bar.
- What a computer adds stays: widescreen, any frame rate, the mouse, the touch screen, the
  keyboard and the Controls page, controllers, several windows at once and the Extended options.
- F still targets whomever your target is targeting: it is the game's Assist Target.
- Your options, keys and buttons stay as you left them.

## 2026-10-27 · Waypoints and map suggestions on your target
- With someone targeted and in view (a merchant, a quest giver, a creature), □ (Q) on the map
  puts your waypoint right on them; press it again to take it away. A white ring shows your
  target on the map.
- Suggest (N, or L3 on a controller) takes your target's spot too, with their name already
  typed in for you to keep or change.
- What you choose on the map still comes first: once you move the map, □ goes in its middle,
  and Suggest takes the spot you point at or a waypoint you just put. With no target, both
  work as before.

## 2026-10-26 · A Stats Box, and N for the minimap
- A Stats Box: your stats (Str to Cha, AC, max HP and power, the resists) in a small box
  under your effect icons. A buff that raises a stat shows it in blue, a debuff that lowers
  one in red. It is off unless you turn it on: Options > Extended > UI > Stats Box, or press
  B. Click or tap it to open your Summary.
- Press N to show or hide the minimap. Both keys can be moved, or given a controller
  button, on the Controls page.
- Options > Extended: Text (Sharp or Smooth) is on the Video page now.

## 2026-10-26 · Community markers on the map
- The map shows places other players suggested and a GM accepted: a pin each, coloured by what it
  marks (a landmark, a camp, a quest's place, a merchant, a way to travel, a danger), its name
  beside it when you zoom in, and what it is when you point at it. X hides them with the other
  markers. The list comes from eqoaclassic.com whichever server you play on, and is kept for a
  start with no connection.
- Suggest one of your own: put your waypoint on the spot (a click on the map), or point at it, and
  press N (L3 on a controller, or the map's Suggest button). Type its name, then a note if you
  like; Enter sends it for a GM's approval, Escape sends nothing. The map says when it went.

## 2026-10-26 · The map shows where your pointer is
- Under the map's title, the place under the mouse pointer, as the game counts
  it: "x 24611, z 15502" (on a controller, the place under the middle cross once
  you pan the map off yourself). Thanks for the idea.

## 2026-10-26 · Quest markers on the map
- The map shows where your quests are, on a server that sends them (ours does;
  Sandstorm sends none): a gold "!" at each NPC with a quest you can take now,
  for your class, race and level, and a "?" where a quest you are on needs
  you: the NPC to see next, or the nearest places of the creatures to hunt.
- Point at a mark (or tap it, or pan the map's cross onto it with a
  controller) to see its quests: their names and levels, how the offer starts
  or what the step asks, and the rewards or what you still need. Click it, or
  tap it again, to put your waypoint on it.
- The marks sit over the XP camps and coaches, and are on the minimap too, on
  its edge while farther than it shows (point at one for its quests). J on the
  map (R3 on a pad, or the map's Quests button) hides or shows them, and so
  does Options > Extended > Quests > Quest Markers.

## 2026-10-26 · A minimap
- A minimap in the top right, under the name plate of the menus beside it: the map's own
  picture of the land round you, north up, with you as the arrow in the middle, your group in
  their colours, the creatures and players near you as dots in their names' colours, your
  target ringed and your waypoint (on its edge while it is farther than it shows).
- The map's XP camps and coaches are on it too, as the map marks them; point at one with
  the mouse for its card. Map Markers (X on the map) hides them here as well.
- Click or tap it to open the map; the mouse wheel over it zooms it in and out. It goes
  away with the rest of the HUD under Hide UI (H) and while a window is open.
- Turn it off under Options > Extended > UI > Minimap. Loading Screens moved from UI to the
  Video page to make room.

## 2026-10-26 · XP camps, coaches, routes and towns on the map
- The map (M) marks the popular XP camps with a red XP badge and every coach with the
  horseshoe the stables wear (the one on Freeport's stable sign, read from your own disc).
  Point at one, with the mouse, a tap, or the pad's middle cross once the map is panned,
  for its card: a camp's levels, the guide's note, the rare creatures near it and notable
  drops; a coach's town, its coachman and the towns it runs to.
- Click a marker, or tap it a second time, to put your waypoint on it.
- The coach routes are dashed lines between the towns they join; the coach you point at
  has its own drawn bright.
- The towns' names are on the map too, blue for good, red for evil, cream for neutral,
  the bigger town's first where they crowd, smaller as you zoom out.
- X (○ on a pad) hides or shows the markers and routes, and so does Options > Extended >
  World > Map Markers; T (Select) the town names. The camps, routes and towns are from
  EQOA-Data's map guide.

## 2026-10-26 · A waypoint of your own on the map
- The map has a waypoint of your own: click or tap a spot on the map (M) to put a
  magenta marker there, and click it again to take it away. With a controller, pan the
  map and press □ to put it at the middle (Q on the keyboard does the same).
- The compass shows it as a magenta dot on its rim pointing the way to go, however far
  it is, and it goes away when you get there. It is kept for each world, and nobody
  else sees it.

## 2026-10-24 · Controllers: the map on a button, PlayStation pads behind other windows
- Options > Controls: a controller's button can now be put on any of the
  keyboard's own rows: the map, its zoom, targets 1 to 6, the group keys, F9's
  art and the rest. That button then does what the key does (R3 on the map:
  R3 opens and closes the map) instead of what it did in the game, and the
  page says which row lost it. Defaults puts it all back.
- On Windows a DualShock 4 or a DualSense now works in a game window that is
  not in front, as an Xbox controller already did: with two windows open,
  each can have its own controller, PlayStation or Xbox. Over USB, Bluetooth
  or the DualShock 4's wireless adapter. Before, Windows gave a PlayStation
  controller only to the window in front. On Linux and on a Mac every window
  already read every controller.
- A problem report (F3) now lists the controllers connected, how each is read
  and which game window reads it.
