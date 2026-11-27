# EQOA Chat

Off by default. Enable it in Mods & Addons (Shift+F10); it shows **Provides: Chat window with tabs, channels, links (Enter)** and **Replaces game windows: Chat Entry** before you do.

A chat window of this mod's own, bottom left. The game's chat box is hidden while it is on.

- **Move and size.** Drag the thin strip at the top of the window (the faint grip lines) to move it, and its bottom-right
  corner to size it; the mod remembers where you left it. (The game's box also opens instead of this one if you press a
  controller button in the same instant; the log then says why.)
- **Edit the tabs.** Right-click a tab for **Rename**, **Move left**, **Move right**, **Delete** and
  **Show in this tab** (a tick per chat style: Say, Group, Guild, Shout, Tell, Combat, Loot, Rolls, XP, System; the menu
  stays open while you tick). Right-click the empty strip beside the tabs, or click the small **+**, for **New tab** (it
  starts with nothing ticked). **All** can be renamed and moved but never deleted and always shows every line. **Reset tabs**
  (in any of these menus) brings back All plus one tab per style. Your tabs are kept in the mod's settings (the game's own Options
  chat tabs are not used by this window). Rename types in the strip (12 characters at most): Enter keeps it, Esc drops it,
  and no other key does anything while you type.
- **Tabs.** **All** shows every line of chat. Then one tab per chat style: **Say, Group, Guild, Shout, Tell, Combat, Loot,
  Rolls, XP, System**, each with only that style's lines. The mod's own messages (a line it could not send, and why) come
  as red System lines. The wheel over the window scrolls back, and so do Up and Down while the game's chat-history key
  is held (C, or R2 on a controller). That key's Tab (the game's chat panes) does nothing here.
- **Combat.** The game shows hits as numbers over heads, not as lines of chat, so this window writes them: "You hit
  Gnoll for 12.", "Gnoll hits you for 7.", your pet's hits and what hits it, heals. They show in the tabs that show
  Combat (and not in All), in time order with the chat lines that are combat too (kills, misses, resists).
- **Enter** (and the `/` key, and the **R** and **T** keys) opens the mod's entry line instead of the game's. On a
  **controller**, R3 still opens the game's own line (it has the pad's soft keyboard and a cancel button; a mod's line
  would take the whole pad while you type). Type, press **Enter** to send, **Esc** to cancel. An empty Enter just closes it.
  A long line **wraps inside the window** (words kept whole, a long word broken inside it) and the window grows upward by a
  row for each wrapped row, up to four rows; past that the last four show, so the end you are typing at stays in view.
- **Channel picker.** **Say, Group, Guild, Shout, Tell, Reply** under the window while you type; click one or press
  **Tab** to go round (typing keeps going into the line). Say, Group, Guild and Shout also set the game's chat mode (what Community > Chat Modes sets),
  so the game and this window agree. **Tell**: type the player's name, a space, then the message. **Reply** goes to
  whoever last sent you a tell (its name is on the button). A line that starts with `/` is sent as a command, except
  a bare channel command: `/g`, `/s`, `/gu`, `/say` (and their long names) change the channel and keep the line open, `/t name`
  or `/tell name` switch to Tell with the name ready, `/r` to Reply.
- **Item links.** A link in a chat line is blue; click it and the game's own item card opens (where the server offers
  item links and Item Links is on). The Bag Viewer's **Link** button puts the item into this window's line, as the
  game's own Link Item puts it into the game's line; up to four links go with a line.
- **Not taken.** Passwords, report text, the buddy, guild and ignore name boxes and the alarm box are always the game's own
  boxes. Reports are never sent by a mod.
- Turn the mod off and the game's chat box and Enter key come back at once.

It uses `eqoa.windows.takeover("chat_entry")`, `eqoa.actions.chat_line`, `eqoa.actions.command`,
`eqoa.actions.set_channel` and `eqoa.actions.open_link`; Mods & Addons labels it as able to act on its own for those.
Other mods' warnings are not collected here: the mod API has no way to read them yet. While the line is open your character keeps
running (autorun and follow too), as in the game's own chat box.

Long lines wrap at the window's inner width on word boundaries (a word longer than a row breaks inside the word), on every tab; each row keeps the message's colour, and an item link split across two rows opens from either part. The wheel scrolls a row at a time. Resizing the window wraps again. Only the messages the shown rows come from are measured.
