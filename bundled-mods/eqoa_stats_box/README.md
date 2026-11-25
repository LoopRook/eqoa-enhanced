# EQOA Stats Box

Displays the authentic Summary stats box in the HUD, showing attributes, hit points, power, and resists across two columns with dynamic color shifts. Toggled via the Controls page keybind (default `y`, moved from `b` so the Bag Viewer can have it) or the Enhanced HUD options toggle. Clicking the stats box opens the Summary window.

While the Stats Box is on, it replaces the game's own Stat Window (F4, `/stats`): that window is not drawn, takes no click, and F4 and `/stats` do nothing, so the two never show the same figures twice. Turn the Stats Box off, disable the mod, or use "Use the game's windows", and the game's Stat Window works again as it always did (the player's own on/off choice for it is kept). Uses `eqoa.hud.hide('stat_window')`; declared as `hud.stat_window` in `replaces`.
