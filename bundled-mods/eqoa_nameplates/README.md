# EQOA Name Plates

Your own name plates over heads and floating damage numbers, in place of the game's. Off until you turn it on in
Mods & Addons.

## What you see
- **Plates** over creatures, players and NPCs near you: the level (in its consider colour) and the name, and a
  health bar under them. Your target's plate has a yellow frame; your group members' names are green. A quest
  "!" or "?" (where the server sends them) and a mercenary's "<Owner's Mercenary>" line are drawn over the plate,
  as the game draws them.
- **Damage numbers** float up from what you hit: white for your hits, bigger for a hit well above your usual
  (the game does not mark critical hits, so "well above your usual" is the mod's own guess), and red for
  damage you take.
- The consider colour is the server's own for your **target**; for others the level colour is worked out from
  how far their level is from yours (red: 3 or more above, yellow: 1 to 2 above, white: the same, then blue,
  light blue, green and grey further below).

## Things to know
- **Plates show through walls.** The game hides its own plates behind scenery; a mod cannot ask the client whether
  a point is behind a wall (the game uses the picture's depth, which mods cannot read, and its collision check is
  not offered). Use the Distance option to keep far plates away.
- At most 60 plates are drawn at once: the nearest ones, and always your target's.
- Text size follows the Size option (the text is sized by the window, so it needs no font).

## Options
Mods & Addons, **Name Plates: settings...** opens a small window: show **Players**, **NPCs** and
**Creatures**, a **Distance** (plates farther than this are not drawn or worked out) and a **Size**. **Hide the
game's target rings** also hides the rings round your target (they stay by default).

## Off means the game's
While the mod is on, the game's own plates and damage numbers are hidden (Mods & Addons says "Replaces HUD: Name
plates, Damage numbers"). Turn the mod off, or leave the world, and they are back at once.

## For mod authors
Uses `eqoa.camera.project_entity` (the point the game puts its own plate at), `Entity.head_mark`,
`Entity.merc_tag` and `eqoa.hud.hide("name_plates" | "damage_numbers" | "target_rings")`.
