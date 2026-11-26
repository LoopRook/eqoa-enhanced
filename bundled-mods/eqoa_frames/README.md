# EQOA Frames (demo)

A small demonstration of what the client gives a unit-frame mod. It is off until you switch it on in Mods & Addons (Shift+F10).
Three draggable windows:

- **Effects**: your effects as a row of icons, each with the time left under it in the game's short form (seconds under a
  minute, then minutes, then hours: "45s", "3m", "3h"). The row wraps to more rows at the window's width (drag the window's
  corner to change it) and its sizes follow the UI scale. An effect with no icon art is a framed square with the first letters
  of its name. A green frame marks an effect the server lets you take off (it only allows beneficial ones; the record does not say
  which of the others are harmful). Tick **Show as list** for the older list of labelled buttons (the choice is not kept
  between sessions). Click an icon to take the effect off, where the game lets you (only effects the server
  calls removable). Needs a server that offers the buff window and **Buff Window** on in Options; otherwise it lists the
  game's own effects with no times. Your pet's effects are never buttons: they are the dimmer **Pet:** lines at the bottom, once each, when the server sends them (from its buff list, or from the pet strip). Hold the pointer on an
  effect to see what it does (the description of the ability you have with that name; an effect of someone else's spell
  that you do not have says the game does not tell), the time left, and "click to take it off" where that is allowed.
- **Casting**: a bar for your own cast and one for your target's. Melee rounds are not casts and show no bar. The client
  is told when a cast starts and how long it takes; it is not told when one is interrupted or lands, so a bar that fills is a clock, not a result.
- **Target**: the target's name in its consider colour, its title and guild where the server sends them, and buttons to
  invite it to your group, send your pet in and call it back off. **Invite to group** shows only for another player who
  is not already in your group: not for a monster, an NPC or yourself.

It declares only the reads and the four actions it uses (`group_invite`, `pet_attack`, `pet_back_off`, `remove_effect`), so
Mods & Addons shows them. It is meant to be read and copied: see `docs/modding/EQOA-API-SPEC.md`, section 4 ("Frames").
