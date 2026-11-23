# EQOA Spellbook

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: Spellbook window (Shift+K)** before you do.

A Spellbook window of its own, on its own key: **Shift+K** (the Controls page has a **Spellbook** row, so you can
move it; a key the game, a macro or another mod already has leaves the row empty and says why). Press the key again,
press **Escape** or click the **x** to close it. It sits over play, so your character can keep walking, and the
game's own Start menu > Abilities page is exactly as it was.

- The list is every ability your character knows (the wheel scrolls it). Click one to see its level, power, cast
  time, range and description. Abilities on cooldown are red, ones you cannot use are grey, and a green **[n]** is
  the slot it is memorized in.
- Under the list are the 20 memorized slots. Choose an ability, then click a slot to memorize it there; right-click
  a slot to clear it.
- **Cast** casts the chosen memorized ability on your target. **Unmemorize** clears it; **Unmemorize all** clears
  every slot (the game asks nothing before that, and neither does this).
- It does not use a controller yet: no controller button opens it (Start menu > Abilities still works).

It uses the window sessions of `eqoa/1` (`eqoa.windows.open_session`): when you zone, die, turn the mod off or switch on
**Use the game's windows**, the client closes it for you.
