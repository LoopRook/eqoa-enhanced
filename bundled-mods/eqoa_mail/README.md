# EQOA Mail

Off by default. Enable it in Mods & Addons (Shift+F10); it shows **Provides: Mail window (Shift+M)** and, because it can send
mail, **Mail and auction: can send plain mail, take what a mail carries** and **Asks you first: delete a mail, send
mail that carries tunar or an item** before you do.

A mail window of its own. The game has no Mail key (its Mail is Start menu > Community > Mail), so the mod adds one:
**Shift+M** (the Controls page has a **Mail** row, so you can move it; a key the game, a macro or another mod already
has leaves the row empty and says why). With **EQOA Start Menu** on, its Community > Mail entries (Inbox and Compose)
open this window too. Press the key again, press **Escape** or click the **x** to close it. It sits over play, so
your character can keep walking, and the game's own Mail windows are exactly as they were with the mod off.

**Inbox tab**

- Your mails: who sent it, the subject, the date, and a **+** for a mail that carries tunar or an item. Unread mails
  are brighter. The mouse wheel scrolls past eight. The game keeps 100 mails.
- Click a mail to read it (the game asks the server; it takes a moment). What it carries is listed under the text.
  A long mail scrolls with the mouse wheel over its text (the line numbers show where you are).
- **Refresh** asks for the inbox again. **Reply** opens the Write tab to the sender. **Take contents** takes the tunar
  and items a mail carries. **Delete** asks you first ("Are you sure?" with the mail's name); only your answer deletes.

**Write tab**

- **To** takes one name, **Subject** 32 letters, **Text** up to 500. Click a box and type. **Tab** moves to the next
  box, **Enter** adds a line in the text (and moves on from To and Subject), **Escape** or a click anywhere else
  (a right-click, another window) stops typing. Letters with accents count as one letter each.
- On servers whose mail carries things, **Tunar** and **Attach an item** (one item from your pack, with - and + for
  how many) appear (Attach an item only where the server takes items). A mail that carries something goes to **one name only**, and **Send** asks you first, showing
  the name, what goes and the postage. Plain text mail sends at once, as the game's own Compose does.
- What the server says (sent, no such name, mailbox full...) shows at the bottom.

It uses the window sessions of `eqoa/1` (`eqoa.windows.open_session`, kinds `mail_inbox` and `mail_compose`) and the
`eqoa.mail.get` read; when you zone, die, turn the mod off or switch on **Use the game's windows**, the client closes
it for you. Nothing is sent twice, and the game's own Mail windows cannot be open at the same time (the buttons
say the game's window is busy while one is). After a send the window waits for the server's answer (the form empties only
when the mail went, and Send is off meanwhile), so a mail is never sent twice by a click on a stale message. It
has no controller use (mouse and keyboard only).

## Known gaps

- With the game's full map open this window hides itself and takes no clicks or keys; it comes back as it was.
- Closing the window while a mail is on its way empties the form (the mail may have gone) and says so; look at the server's answer before sending it again.
- A click on another window lying over the box you are typing in does not end the typing.
- The Item picker's rows also take a right-click.
- Waits for the server's answer count drawn frames, so they are shorter at high frame rates.
