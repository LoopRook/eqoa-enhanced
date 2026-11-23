# EQOA Spellbook

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: Spellbook window (Shift+K)** before you do.

A Spellbook window of its own, on its own key: **Shift+K** (the Controls page has a **Spellbook** row, so you can
move it; a key the game, a macro or another mod already has leaves the row empty and says why). Press the key again,
press **Escape** or click the **x** to close it. It sits over play, so your character can keep walking, and the
game's own Start menu > Abilities page is exactly as it was.

- The list is every ability your character has learned (the wheel scrolls it), as the game's Abilities page lists
  them. Click one to see its level, power, cast time, range and description. Abilities on cooldown are red, and a
  green **[n]** is the slot it is memorized in.
- The scroll bar beside the list: click it to jump the list to that place (there is no dragging yet: the mod API has
  no button-release event). The wheel works over the list.
- Under the list are the 20 memorized slots. Choose an ability, then click **Memorize** (it goes on the first free
  slot) or click the slot you want; right-click a slot to clear it. As in the game, an ability already memorized is
  refused (Unmemorize it first). Any ability you have learned and not memorized can be memorized. Slots 11 to 20 need Extended > Ability Bars.
- **Cast** casts the chosen memorized ability on your target. **Unmemorize** clears it; **Unmemorize all** clears
  every slot (the game asks nothing before that, and neither does this).
- The window's texts are wrapped to fit it: the hint about slots under the list, the description (three lines, ending in
  "..." when longer) and the last answer from the game (two lines, under the description).
- A key that cannot open the window (you have **Use the game's windows** on, you are dead or out of the world) leaves a
  note in the game's log only (`eqoa.log`), not a warning on screen.
- It does not use a controller yet: no controller button opens it (Start menu > Abilities still works).

It uses the window sessions of `eqoa/1` (`eqoa.windows.open_session`): when you zone, die, turn the mod off or switch on
**Use the game's windows**, the client closes it for you.
