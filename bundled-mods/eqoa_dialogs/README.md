# EQOA Dialog Window

Off by default. Enable it in Mods & Addons (Shift+F10); it shows **Provides: NPC dialog window**, **Replaces game windows:
Dialog, Group Invitation, Resurrection Offer, Summons** and **Can answer NPC dialogs** before you do.

A cleaner window for what NPCs say. While the mod is on, the words of an NPC (hail, quest offers, trainers, coaches)
appear in the mod's window instead of the game's dialog window.

- **The NPC's words** are wrapped to fit the window. Answers are buttons: click one or press its number, **1** to
  **6**. With more than six answers, **7** (More) and **8** (Back) turn the page. A plain message (no answers)
  has a **Done** button (click it, or press Enter or 1).
- **Enter** is the window's while a dialog is up: on a plain message it presses Done, on a dialog with answers it does
  nothing (the number keys and clicks answer). It never reaches the game, so it cannot open the chat line over the dialog.
- **Escape or the x** leaves the conversation, as pressing Escape in the game's own dialog does: one press, and
  nothing else opens (not the game's dialog, not the Start menu). On an invitation, summons or resurrection offer it
  is the game's own No. The **Game window** button at the bottom of the window gives the dialog to the game
  instead: its own dialog window opens with the same words and answers, and you can answer it there.
- A dialog that cannot be shown here always ends up in the game's window: if you turn the mod off, switch on
  **Use the game's windows**, or the window sits unused for two minutes, the game's own window shows it.
- The window sits over play, so your character can keep walking. If you walk far from the NPC (about 25 units) the
  dialog is closed with no answer, as pressing Escape in the game would.
- The title is the NPC you last had targeted when the words arrived. That is a best guess, not a promise that the
  NPC spoke.
- **Invitations box** (option **Invitations and offers**, on by default, in the mod's
  options): a group invitation, a summons or a resurrection offer comes in its own small box with **Accept**,
  **Decline** and **Game window**. Accepting a group invitation is sent at once. Accepting a summons or a
  resurrection never is: the game's own **Are you sure?** appears and only your own Yes there sends it. Declining
  always goes at once. Turn the option off and the game shows these itself.

- **Quest hand-ins** (option **Highlight hand-ins**, on by default, in the mod's options): an answer whose words
  name a quest in your journal (the whole title, any capitals, the longest title wins) has a green frame and a
  **Hand in** button beside it. Clicking either sends that answer, as the number key does. Only the answer's own words
  count, never its position.
- **Speed through hand-ins** (option, off by default; it replaces the old "Auto-answer hand-ins" rule, and a value saved
  for that one does nothing): when you talk to someone your journal sends you to (a "?" quest step mark on their name in
  the world you are in), the window steps through the talk by itself, straight to the end: it presses Done on a plain
  message and picks the only real answer (the game's own More, Back and Close, and a plain Goodbye or Farewell, do not
  count as answers), with no pause to show each line. It also carries on through a quest that is offered with a
  single answer. It **stops and waits for you** the moment there is a real choice (two or more answers): that choice
  has a green frame and a line saying where it stopped. Everything the person said stays in the window, in a record
  above the live words (scroll it with the wheel or the ^ / v marks). Someone without a "?" for you is talked to as
  usual, and nothing runs with the option off.
  **To stop it:** press Escape (between two steps too: the late step is then shown, not answered), click an answer
  yourself, click the x, or walk away. It also stops by itself after 12 steps, when the person who speaks is not
  known, or when the server sends the very same step again. After you have answered a step yourself, the rest of that
  talk is yours. **Who counts as "a "?" for you":** the creature you had targeted when the words arrived (a best guess, as for the
  title), when the client has a quest step mark ("?") of the same name within 60 units of where that creature stands, in
  the world you are in. **It does not run** when Map Markers is off, in the first seconds after entering a world (the
  client builds its quest marks a moment later), when the creature cannot be found, or when the journal has no step
  there. **It could run on the wrong person** when the speaker was not the creature you had targeted, or when a kill,
  loot or reach step's creature of the same name stands at the mark; the cost is only that it presses Done or a
  single answer for you (never a Goodbye, never one of two or more answers), and Escape stops it. After you leave a
  talk with Escape or the x, or about two seconds after a stopped talk ends, the next talk starts afresh.

It uses the window sessions of `eqoa/1` (`eqoa.windows.takeover` with `scope = "all"` for `text_dialog`,
`response_dialog`, `group_invite`, `revive_offer`, `summon_offer`; `eqoa.windows.session`, `pass`, and
`eqoa.actions.dialog_answer`). An answer is reported as sent, never as confirmed. The mod holds the kinds again each
time you enter a world, because the game lets go of every hold when a world is left.
