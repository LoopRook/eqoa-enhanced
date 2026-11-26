# EQOA Stats Box

Displays the authentic Summary stats box in the HUD, showing attributes, hit points, power, and resists across two columns with dynamic color shifts. Clicking the stats box opens the Summary window.

The Stats Box replaces the game's own Stat Window, so it answers the game's own key: **the game's Stat Window key (F4 unless you rebound it on the Controls page, in either interface) and `/stats` open and close the Stats Box**; `/stats on` and `/stats off` show and hide it. The game's Stat Window itself never shows while the mod is on, whether the box is up or not, so the two never swap or show the same figures twice. The box is also an option in the Enhanced HUD options. The mod's own key on the Controls page ("Toggle Stats Box") is an extra and is not bound until you bind it (it was `y`).

Disable the mod, or turn on "Use the game's windows", and the game's Stat Window and its key work again as they always did (your own on/off choice for it is kept). Uses `eqoa.hud.hide('stat_window')` and listens for the `hud.piece_requested` event; declared as `hud.stat_window` in `replaces`.
