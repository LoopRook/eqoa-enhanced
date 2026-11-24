# EQOA Dialog Window

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: NPC dialog window**, **Replaces game windows:
Dialog, Group Invitation, Resurrection Offer, Summons** and **Can answer NPC dialogs** before you do.

A cleaner window for what NPCs say. While the mod is on, the words of an NPC (hail, quest offers, trainers, coaches)
appear in the mod's window instead of the game's dialog window.

- **The NPC's words** are wrapped to fit the window. Answers are buttons: click one or press its number, **1** to
  **6**. With more than six answers, **7** (More) and **8** (Back) turn the page. A plain message (no answers)
  has a **Done** button (click it, or press Enter or 1).
- **Enter** is the window's while a dialog is up: on a plain message it presses Done, on a dialog with answers it does
  nothing (the number keys and clicks answer). It never reaches the game, so it cannot open the chat line over the dialog.
- **Escape or the x** gives the dialog back to the game: its own dialog window opens with the same words and
  answers, and you can answer it there. Nothing is dropped and nothing is answered for you.
- A dialog that cannot be shown here always ends up in the game's window: if you turn the mod off, switch on
  **Use the game's windows**, or the window sits unused for two minutes, the game's own window shows it.
- The window sits over play, so your character can keep walking. If you walk far from the NPC (about 25 units) the
  dialog is closed with no answer, as pressing Escape in the game would.
- The title is the NPC you last had targeted when the words arrived. That is a best guess, not a promise that the
  NPC spoke.
- **Invitations box** (option **Show invitations, summons and resurrection offers**, on by default, in the mod's
  options): a group invitation, a summons or a resurrection offer comes in its own small box with **Accept**,
  **Decline** and **Game window**. Accepting a group invitation is sent at once. Accepting a summons or a
  resurrection never is: the game's own **Are you sure?** appears and only your own Yes there sends it. Declining
  always goes at once. Turn the option off and the game shows these itself.

- **Quest hand-ins** (options **Highlight quest hand-ins**, on by default, and **Answer quest hand-ins
  automatically**, off by default, in the mod's options): an answer whose words name a quest in your journal (the
  whole title, any capitals, the longest title wins) has a green frame and a **Hand in** button beside it. Clicking
  either sends that answer, as the number key does. Only the answer's own words count, never its position. With
  the automatic option on, a dialog whose only real answer (the game's own More, Back and Close, and a plain Goodbye or Farewell, do not count) names
  a quest is answered at once. A dialog that offers any choice is only marked: your click answers it.

It uses the window sessions of `eqoa/1` (`eqoa.windows.takeover` with `scope = "all"` for `text_dialog`,
`response_dialog`, `group_invite`, `revive_offer`, `summon_offer`; `eqoa.windows.session`, `pass`, and
`eqoa.actions.dialog_answer`). An answer is reported as sent, never as confirmed. The mod holds the kinds again each
time you enter a world, because the game lets go of every hold when a world is left.
