# EQOA Stats Box (example mod)

An example for mod authors of **replacing one of the game's HUD pieces through the API**. It is not shipped with the client:
it was bundled until 2026-11-27, when the game's own Stat Window became the one players use. To try it, copy this folder into
your `mods` folder and turn it on in Mods & Addons.

It draws a stats box in the HUD (attributes, hit points, power and resists in two columns, raised values blue and lowered ones
red); a click on it opens the game's Summary window.

What it shows:

- **Holding a game piece back.** While the mod is on it holds the game's own Stat Window with `eqoa.hud.hide("stat_window")`
  (declared as `hud.stat_window` in `replaces`), whether its own box is up or not, so the two never show the same figures twice.
- **Answering the game's own key.** The game's Stat Window key (F4 unless the player rebound it, in either interface) and
  `/stats` do nothing to the held window; the holder hears them as `hud.piece_requested` (`piece = "stat_window"`, `action` =
  `toggle`, `show` or `hide`) and shows or hides its own box.
- **Its own switch and key.** The box's switch is the mod's own option (`stats_box`, a row on the Enhanced page while the mod is
  on, flipped by `eqoa.actions.toggle_stats_box`, an action reserved for this mod's id); its own key on the Controls page,
  "Toggle Stats Box", is unbound until the player binds it.
- **Standing down.** With "Use the game's windows" on the game's window and key are the game's again and the box draws nothing;
  turned off or failing, the mod lets go and the game's window works as it always did (the player's own on/off choice for it is
  kept untouched).

API: `eqoa.player.stats`, `eqoa.hud.panel/text/frame/visible/hide/show`, `eqoa.text.localized`, `eqoa.windows.open`,
`eqoa.options.toggle/get`, `eqoa.input.bind`, `eqoa.actions.toggle_stats_box`, `eqoa.events.on` (`hud.piece_requested`,
`player.stats_changed`, `render`, `state.reset`), `eqoa.imgui`.
