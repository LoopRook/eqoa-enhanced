# EQOA Character

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: Character window (Shift+C)** before you do.

A Character window of its own, on its own key: **Shift+C** (the Controls page has a **Character** row, so you can
move it; a key the game, a macro or another mod already has leaves the row empty and says why). Press the key again,
press **Escape** or click the **x** to close it. It sits over play, so your character can keep walking, and the
game's own Start menu pages (Character Summary, Class Mastery, Factions, Combat Record) are exactly as they were.

Four tabs:

- **Summary**: name, level, race and class, HP, power, your seven stats as `now / cap (with gear)`, armor and the six
  resists, your Class Mastery points, and your **training points**. Click **+** or **-** beside a stat to place points
  (it stops at the stat's cap, as the game's trainer does, and at the points you have), then **Train**. The client then
  asks **Are you sure?**, starting on **No**: your points are spent only when you answer Yes yourself (Left or Right to
  choose, Enter to answer, Escape for No, or click). Spent points cannot be taken back.
- **Masteries**: the skills of your Class Mastery list, with their cost and whether you own them (`owned`), can buy them
  (`buy`) or cannot yet (`locked`); `[M]` marks a Master Class. Click a skill to read it (the server sends its
  description and what it needs). **Buy** and **Refund** wait while the game is still waiting for the server's last answer (they come back when it is in; a Yes given in that moment waits for it and then goes), and ask **Are you sure?** the same way; only your own Yes sends the
  request. One request goes to the server at a time. The wheel scrolls the list.
- **Factions**: your standings, a page at a time (**Previous page** and **Next page**). The bar runs from the middle,
  red to the left and blue to the right, as in the game.
- **Combat**: your combat record, on a server that keeps one (**Refresh** asks again).

It uses the window sessions of `eqoa/1` (`eqoa.windows.open_session`): when you zone, die, turn the mod off or switch on
**Use the game's windows**, the client closes it for you. While the Masteries tab is up the mod holds a Class Mastery
session, so the skill list is fetched when you open the tab and let go when you leave it, as the game's own window does.
It does not use a controller yet: no controller button opens it.
