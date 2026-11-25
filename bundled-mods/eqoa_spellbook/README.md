# EQOA Spellbook

Off by default. Enable it in Mods & Addons (Shift+F10); it shows **Provides: Spellbook window (Shift+K)** before you do.

A Spellbook window of its own, on its own key: **Shift+K** (the Controls page has a **Spellbook** row, so you can
move it; a key the game, a macro or another mod already has leaves the row empty and says why). Press the key again,
press **Escape** or click the **x** to close it. It sits over play, so your character can keep walking, and the
game's own Start menu > Abilities page is exactly as it was.

- The list is every ability your character has learned (the wheel scrolls it), as the game's Abilities page lists
  them, grouped under **Levels 1-10**, **11-20** and so on. Click one to see its level, power, cast time, range and
  description. Each has its icon; abilities on cooldown are red, and a green **[n]** is the slot it is memorized in.
- **Search:** click the box above the list and type; the list shows only abilities whose name contains what you typed
  (capitals do not matter). Backspace removes a letter, the small **x** clears the box, **Escape** (or Enter) lets go
  of the keyboard, and the next Escape closes the window. While you type, the game's keys do not move you.
- **Self, Target, Group, Pet, All:** the buttons beside the box show only the abilities that affect you, your
  target, your group or your pet (the ability's `scope`: 0 self, 1 target, 2 group, 3 pet). **All** shows every
  ability, including those with another scope (a corpse, or one the game does not say). A filter and a search work together.
- The scroll bar beside the list: click it to jump the list to that place (the wheel works over the list too).
- Under the list are the 20 memorized slots, each with its ability's icon. **Drag** an ability from the list onto a slot
  to memorize it there; drag it onto a slot that is taken and the ability there is cleared first. Drag a slot onto
  another slot to move its ability, or onto a taken slot to swap the two. **Right-click** a slot to clear it.
  Dropping anywhere that is not a slot does nothing. Or choose an ability, then click **Memorize** (it goes on the
  first free slot) or click an empty slot. As in the game, an ability already memorized is refused when you try to
  memorize it again (Unmemorize it first). Any ability you have learned and not memorized can be memorized. Slots 11 to 20 need Extended > Ability Bars.
- How a move is sent: the mod API has no move or swap action, so the window does what you would do by hand, with the
  game's own actions, one at a time and each only after the game has answered the one before: Unmemorize the slot the
  ability leaves, Unmemorize what is on the slot it goes to, Memorize the ability onto that slot, and (a swap) Memorize
  the other ability onto the slot just left. If the game refuses a step the move stops there; the line under the
  description says why. A second drag made while a move is still going is ignored.
- **Cast** casts the chosen memorized ability on your target. **Unmemorize** clears it; **Unmemorize all** clears
  every slot (the game asks nothing before that, and neither does this).
- The window's texts are wrapped to fit it: the hint about slots under the list, the description (three lines, ending in
  "..." when longer) and the last answer from the game (two lines, under the description). The window is 332 by 419
  pixels of the game's 640 by 448 screen, and keeps its place inside the game window.
- A key that cannot open the window (you have **Use the game's windows** on, you are dead or out of the world) leaves a
  note in the game's log only (`eqoa.log`), not a warning on screen.
- It does not use a controller yet: no controller button opens it (Start menu > Abilities still works).

It uses the window sessions of `eqoa/1` (`eqoa.windows.open_session`): when you zone, die, turn the mod off or switch on
**Use the game's windows**, the client closes it for you.

Uses: `eqoa.abilities.list`, `eqoa.icons.resource` (the ability pictures), `eqoa.input.capture_text` (the search box),
and the ImGui mouse functions `IsMouseDown`, `IsMouseReleased` and `GetMouseDragDelta` (to follow a drag; the press
itself is the `pointer.pressed` event).
