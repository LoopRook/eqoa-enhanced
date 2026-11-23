# EQOA Start Menu

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: Start menu (the Start key and button)** and
**Replaces game windows: Start Menu** before you do.

The Start key (the keyboard's and the controller's Start button) opens this mod's menu instead of the game's. It lists
the same entries the game's menu would show on the server you are on (the game's own tables, with the duel, combat
record, raid and mercenary entries where the server offers them, and the bank's and expansion's tables), and clicking
one does what the game's entry does. Windows that return to the Start menu when you close them (the buddy list, the
spell book, ...) return to this one.

- Click an entry, or move with the arrow keys (the controller's d-pad) and press Enter (the controller's cross). An
  entry with `>` opens a sub-menu; **Esc** (the controller's triangle) goes up one menu, and closes the menu at the top.
  The **x** closes it too, and so does pressing the Start key again.
- **Items** opens the game's own Start menu so you can use its item page (the bag windows have no copy here).
- The menu closes when you run an entry, zoom, die, turn the mod off or switch on **Use the game's windows**. Your
  character can keep walking while it is up; the controller's sticks do not move the character while it holds the pad.
- Turn the mod off and the Start key is the game's again at once.

It uses `eqoa.windows.takeover("start_menu")` and `eqoa.actions.menu_command` (the label **Actions: can act on its
own (menu_command)** says so): the entries it runs are the game's own, one press each.
