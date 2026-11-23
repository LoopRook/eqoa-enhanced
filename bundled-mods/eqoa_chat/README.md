# EQOA Chat

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: Chat window with tabs, channels, links (Enter)** and **Replaces game windows: Chat Entry** before you do.

A chat window of this mod's own, bottom left. The game's chat box is hidden while it is on.

- **Move and size.** Drag the thin strip at the top of the window to move it, and its bottom-right corner to size it;
  the mod remembers where you left it. (The game's box also opens instead of this one if you press a controller
  button in the same instant; the log then says why.)
- **Edit the tabs.** In the **Styles** view: right-click a tab for **Rename**, **Move left**, **Move right**, **Delete** and
  **Show in this tab** (a tick per chat style: Say, Group, Guild, Shout, Tell, Combat, Loot, Rolls, XP, System; the menu
  stays open while you tick). Right-click the empty strip beside the tabs, or click the small **+**, for **New tab** (it
  starts with nothing ticked). **All** can be renamed and moved but never deleted and always shows every line. **Reset tabs**
  (in any of these menus) brings back All plus one tab per style. Your tabs are kept in the mod's settings. The **Mine**
  view (the game's own Options > Chat Tabs) cannot be edited here and is unchanged. Rename types in the strip (12 letters at
  most), Enter keeps it, Esc drops it.
- **Tabs.** **All** shows every line. Then one tab per chat style: **Say, Tell, Group, Guild, Shout, Combat, Loot,
  Rolls, XP, System**, each with only that style's lines. The small **Styles / Mine** button switches the strip to your
  own chat tabs from the game's Options (Extended > UI > Chat Tabs): **Main** and the tabs you made, each showing the
  kinds you gave it. **Notices** holds this mod's own messages (a line it could not send, and why). The wheel over the
  window scrolls back.
- **Enter** (and the `/` key, and the **R** and **T** keys) opens the mod's entry line instead of the game's. On a
  **controller**, R3 still opens the game's own line (it has the pad's soft keyboard and a cancel button; a mod's line
  would take the whole pad while you type). Type, press **Enter** to send, **Esc** to cancel. An empty Enter just closes it.
- **Channel picker.** **Say, Group, Guild, Shout, Tell, Reply** under the window while you type; click one or press
  **Tab** to go round. Say, Group, Guild and Shout also set the game's chat mode (what Community > Chat Modes sets),
  so the game and this window agree. **Tell**: type the player's name, a space, then the message. **Reply** goes to
  whoever last sent you a tell (its name is on the button). A line that starts with `/` is sent as a command.
- **Item links.** A link in a chat line is blue; click it and the game's own item card opens (where the server offers
  item links and Item Links is on). The Bag Viewer's **Link** button puts the item into this window's line, as the
  game's own Link Item puts it into the game's line; up to four links go with a line.
- **Not taken.** Passwords, report text, the buddy, guild and ignore name boxes and the alarm box are always the game's own
  boxes. Reports are never sent by a mod.
- Turn the mod off and the game's chat box and Enter key come back at once.

It uses `eqoa.windows.takeover("chat_entry")`, `eqoa.actions.chat_line`, `eqoa.actions.command`,
`eqoa.actions.set_channel` and `eqoa.actions.open_link`; Mods & Addons labels it as able to act on its own for those.
Other mods' warnings are not collected in the **Notices** tab: the mod API has no way to read them yet.
