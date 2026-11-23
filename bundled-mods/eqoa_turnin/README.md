# EQOA Quest Turn-in Helper

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: Quest hand-in box**, **Replaces game windows:
Dialog** and **Can answer NPC dialogs** before you do.

When an NPC talks to you and one of the answers names a quest in your journal, a small **Quest hand-in** box
appears with a **Hand in: <quest>** button. Click it and that answer is sent, the same one you would pick in the
game's own window.

- **It matches on the answer's own words.** An answer matches when the quest's title appears in its text (whole
  words, upper or lower case, punctuation ignored). The answer's number or position is never used. If two answers
  name quests, you get a button for each and nothing is chosen for you.
- **Every dialog it does not recognise goes straight back to the game's own window**, unchanged: plain messages,
  dialogs with no quest in them, everything. You will not notice the mod for those.
- **Not now (game window)** or **Escape** in the box also hands the dialog to the game's window.
- **Answer quest hand-ins automatically** (in the mod's options, **off** by default): when exactly one answer names a
  quest, it is answered without a click. With two matching answers it still waits for you.
- Handing in is a plain answer to the NPC. The mod says "sent": it cannot tell you the server accepted it. Watch
  your journal.
- The dialog does not hold your character in place while the box is up. If you walk away (about 25 units) it is closed
  with no answer, as pressing Escape would.

**Do not run it together with EQOA Dialog Window.** Only one mod can hold the NPC dialogs. Whichever starts second
is told it is busy (a warning in the game's log, naming the mod that holds them) and does nothing until the other is
turned off. To use only this helper, turn **EQOA Dialog Window** off in Mods & Addons, then turn this one on (or
restart). To go back, do the reverse.

It uses `eqoa.windows.takeover` with `scope = "all"` for `text_dialog` and `response_dialog`, `eqoa.windows.session`,
`eqoa.windows.pass`, `eqoa.actions.dialog_answer` and `eqoa.quests.list`. The matching is in `modules/match.lua`
and calls nothing of the game's.
