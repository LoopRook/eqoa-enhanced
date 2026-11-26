# EQOA Start Menu

Off by default. Enable it in Mods & Addons (Shift+F10); it shows **Provides: Start menu (the Start key and button)** and
**Replaces game windows: Start Menu** before you do.

The Start key (the keyboard's and the controller's Start button) opens this mod's menu instead of the game's. It lists
the same entries the game's menu would show on the server you are on (the game's own tables, with the duel, combat
record, raid and mercenary entries where the server offers them, and the bank's and expansion's tables), and clicking
one does what the game's entry does. Windows that return to the Start menu when you close them (the buddy list, the
spell book, ...) return to this one.

- Click an entry, or move with the arrow keys (the controller's d-pad) and press Enter (the controller's cross). An
  entry with `>` opens a sub-menu; **Esc** (the controller's triangle) goes up one menu, and closes the menu at the top.
  The **x** closes it too, and so does pressing the Start key again.
- **Entries that an EQOA mod does the job of open that mod's window** when the mod is on: Character > Summary, Class
  Mastery, Factions and Combat Record open **EQOA Character**; Quest opens **EQOA Quest Journal**; the skill books under
  Tradeskills > Recipe Book open **EQOA Crafting**; Who, Buddy List > View List, Ignore > View List, Guild > Guild List
  and Group > Group Search open **EQOA Social** on that page; Inventory opens **Bag Viewer**; Ability opens **EQOA Spellbook**; Mail > Inbox and Compose open **EQOA Mail** (on that tab); Auction > Search, My lots and My bids open **EQOA Auction**. With the mod off, the
  game's own window opens, as before. A sub-menu is always yours to browse (Tradeskills, Guild, Buddy List and the rest
  open as sub-menus). The menu knows an entry by the game's own command, not by its word, so the chat mode called Guild
  stays a chat mode. The menu asks the mod with `eqoa.mods.send` (the command `open`, which each of those mods accepts);
  it can ask for nothing but "open your window".
- **Ability** opens **EQOA Spellbook** when that mod is on; with it off it is the game's own Ability page, and Escape (or its
  Close) there brings you back to this menu.
- **Enter** is this menu's while it is up: the game's chat line does not open with it.
- **Support**: **Request Help** and **Help Status** open the game's own Support window in one click, and the menu closes;
  the form, its topics and everything you send are the game's and yours (the menu only opens the window). **Send Report**
  is the game's own report editor, which no mod may open, so it opens the game's own Start menu for you to press it there.
- **Inventory** with Bag Viewer off opens the game's own Start menu: the item page has no copy here.
- The menu closes when you run an entry, zoom, die, turn the mod off or switch on **Use the game's windows**. Your
  character can keep walking while it is up; the controller's sticks do not move the character while it holds the pad.
- Turn the mod off and the Start key is the game's again at once.

It uses `eqoa.windows.takeover("start_menu")` and `eqoa.actions.menu_command` (the label **Actions: can act on its
own (menu_command)** says so): the entries it runs are the game's own, one press each.
