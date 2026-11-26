# EQOA World Map

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: World map window with pins (the map key)** and
**Replaces game windows: Map** before you do.

The map key (and the compass and the minimap click) opens this mod's map instead of the game's: the game's own map
picture, drawn with the same detail tiles the game's map uses, in a dark and red window you can **move** (drag the top strip)
and **resize** (drag the corner). Its place and size are kept.

- **Wheel** zooms (in to a quarter of a world unit a pixel, about a street in a city: the ground is the game's finest picture drawn up to 4 times larger, so it turns to soft blocks at the closest), **dragging** pans. The round minimap (and the game's own) goes while this window is up and comes back when it closes. **W, A, S, D** pan too, **+** and **-** zoom, and **Esc** closes the map (the keys of the
  game's own map that a mod can take; the arrow keys stay the game's). These keys are the map's only while it is up: with it
  closed, W, A, S and D walk your character as always.
- **Hover** a dot or a mark (creatures and players near you, camps, coaches, towns, quests, pins, your waypoint) for a card
  with its name, its level (camps and quests give their level range) and how far away it is. It shows you as a dart facing
  your way, your group in their colours, your target's ring, quests to take (!) and steps to do (?), camps (bigger for an XP
  camp), coaches and town names. The Map Markers setting hides camps and coaches here as it does on the game's map.
- **Right-click** the map: **Set waypoint here** (the game's own waypoint, shown on this map with its distance and on the minimap), **Drop a pin here...** (type a short note, Enter saves, Esc cancels), or **Clear waypoint**. Right-click a pin or
  the waypoint to remove it (a waypoint you placed in the game's own map is taken over first, then cleared). A waypoint you
  set here stays when you turn the mod off. Pins are kept for each world (up to 60 in a world, the whole of Tunaria for
  instance).
- **Find a place...** looks through camps, coaches, towns, quests, community marks and your pins by name; click a result (or press Enter for the
  first) to go there. It does not need Map Search to be on.
- **Use the game's map** gives you the game's own map for that opening (Places, community markers, the list of places). The
  next press of the map key opens this one again. Turning the mod off, or switching on **Use the game's windows**, gives the
  game's map back (switch it off again and the mod takes the map key back by itself).
- The map closes when you die or leave the world. **Enter** is this window's while it is up (it saves a note or picks the first
  search result), so the game's chat line does not open with it.

**Center** follows you again after you pan; it follows you until you move the map. The window keeps its zoom between openings.

Not here (what the game's map shows and this one does not yet, because the mod API gives no read for it): zone names, the
coach routes and horseshoes, the guide lines other mods draw, and the towns' names switch (T). The controller is not
supported: a controller player uses the game's own map (the button above, or the mod off). Play goes on under this window like
under any mod window: Space jumps, the number keys and Tab target, and the arrow keys move your quick menus.

**Sizes.** The map's text, buttons, hover card and marks are drawn at the same size as the other mod windows (they follow the
game's UI scale, so they grow with the game window), and the font is made at the size drawn, so large text stays sharp. To
change them, open **Mods & Addons (F10)** and choose **World Map: text and marker size...**: **Text size** (Small, Normal, Large,
Very large) sizes the title bar, the search box, the buttons, the hover card, the place names and the help line, and **Marker
size** (the same four) sizes the dots, the camps, coaches, pins, your dart and their hit areas. Both apply at once and are kept.
The window's smallest size grows with the text, and the buttons and the help line shorten themselves rather than overlap.
