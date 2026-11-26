# EQOA Bag Viewer

Off by default. Enable it in Mods & Addons (it shows **Provides: Bags window (B)**), then press **B**
(the Controls page has a **Bag viewer** row, so you can move it; a key the game, a macro or another mod
already has leaves the row empty and says why). The key opens and closes the Bags window; Escape and
turning the mod off close it too, and so do zoning, dying and the **Use the game's windows** switch (the
client ends the window's session). It opens over play: you can keep walking.

On a controller the window is usable once it is open (a controller button does not open it yet): the d-pad
moves over the slots, confirm opens the item's menu (and presses a row), back closes the menu, then the window.

The window shows the 40 pack slots as item icons with stack counts and a green
**E** on worn items, the tunar you carry and used/capacity at the bottom.
Tabs: **All**, **Usable**, **Equipment**, **Charged**, **Misc** (the game's own
item-page tabs) and **Bank** (the bank's contents and its tunar, readable anywhere; with a banker in reach the tab refreshes itself when you show it, through the mod's own hidden bank session, with no button to press: out of reach, a plain line says "too far from the banker"). Hover an item for its card: name, level, and for gear (items with a wear slot) class ("All classes" or "Class: Paladin, Cleric"),
race (only when the item is shut to some races), slot, stats, HP and
durability, value, and its description at the bottom in grey (wrapped, at most three lines).
When your class, race or level rule the item out, only that line turns red, with a red "You can't use this"
under it; the name keeps its colour. The icon of a piece of gear (an item with a wear slot) you cannot use is dimmed in the grid; crafting, misc, junk and other non-gear items are never dimmed.
Items you set aside for a helper carry a small letter in the bottom-left corner of their slot: a blue **B** (marked to bank) or **R** (kept up to a number for Restock); junk keeps its yellow **J** at the top right. Under the grid is this session's loot log: the last 50 pickups,
newest first ("12:41 Rusty Sword x1, from a decaying skeleton"; the corpse is named
the corpse you asked to loot); the wheel scrolls it. Moving, selling,
looting and banking stay in the game's windows.

The **Auction** button (top right) opens the EQOA Auction mod's window and closes the Bags window so the two do not
sit on top of each other. Bags does not come back by itself when Auction closes: `eqoa/1` gives a mod no way to read
another mod's window, so press the Bags key (Shift+B) again.

Buttons (this version): **Look at corpse** only shows the nearest corpse's items;
**Take all** and **Take all but junk** take them. Mark items as junk from an item's
menu; **Sell junk** sells them at a merchant. **Auto-loot** is a choice under the
loot log, **Off**, **Everything** or **Everything but junk**, kept for each
character (the junk marks are shared: they are per item type). The mod's own opens never show the game's corpse, shop or bank window. A
button for a merchant or banker that is beyond the reach the server last answered at
is greyed out ("too far from the banker"); the reach is learned from real answers
(two silences at a distance grey it, an answer clears it) and a greyed button still
tries on a second click. One more than 40 units away shows nothing at all.

Auto-loot opens the corpse of a creature you hit as soon as it dies, in the middle of a fight
too: it asks for the corpse by its id (`eqoa.actions.open_loot(id, {keep_target = true})`), so
your target and your auto-attack stay as they are. If the server turns out not to accept that, the
client falls back to targeting the corpse, which ends an auto-attack, and Auto-loot then waits for
the end of a fight (the game's combat flag, auto-attack on a live creature, or a hostile within 30
units that has you targeted; your pet, group members such as a mercenary, and merchants or bankers
do not count). It opens corpses within the server's loot reach, which it learns from the server's
answers (it starts at 7 units; "Out of range." only means walk closer). A corpse you opened
yourself (the attack key on a dead target loots it) is taken from that window. A corpse the
server does not answer is asked again 2 units closer or after 5 seconds, three tries in all (half
a second when the game could not open it just then, as while you cast). A take that ends early
(the window went away, the server was not ready) is asked again when you are within reach and five
seconds on, at most two more times (only real reopens of the corpse count). A take that left items
because your bags were full waits until you have free bag space (`eqoa.inventory.get().remaining`),
then goes back for the rest if you are still within reach and the corpse is still there, as often as
needed; if you walk out of reach it is dropped, not chased. A corpse you closed or stopped yourself,
and junk left on purpose, are never asked for again. With Auto-loot on the mod looks around four times a
second, ten times a second while a creature you hit is being followed, not on every creature update.
Every decision is a line in the game's log (`eqoa.log`), starting
`addon eqoa_bag_viewer: [info] auto-loot:` with the corpse id and the reason.

**Helpers at a merchant or a banker** (`modules/helpers.lua`). They work through hidden sessions of the mod's own
(`eqoa.windows.takeover(kind)`, scope `own`), so a merchant or banker you talk to yourself still opens the game's window
exactly as before, and a helper acts only on its button or on its own switch.

- **Mark as junk** (item menu) is the decision to sell: an ordinary **Misc** item is marked at once; equipment, a potion or
  other usable, a quick-slotted item or a no-trade item asks once, "Mark <item> as junk?" (Yes marks it, No does not; Escape
  is No).
- **Sell junk** (near a merchant, or with the merchant open): one click sells every stack marked as junk, gear and potions
  included. The one exception is an item you are wearing right now: it is kept, and a "Kept 1 junk item: you are wearing
  it" line says so. The count on the button is what will really be sold. When some of the junk is something the game's own
  merchant window would ask about (a quick-slotted or no-trade stack, or one that is not an ordinary Misc item), the game's
  own question comes up ONCE for the whole sale ("Sell these N items for T tunar?", a line for each such stack, starting on
  **No**); Yes sells everything, No sells nothing. When none is, it sells at once. The line "Sold N junk items for X tunar"
  gives the game's own quote for the stacks that were sent.
- **Restock**: an item's menu has **Keep up to N (Restock)** (N starts at what the bags hold; the menu steps it by 5). Near
  a merchant, **Restock (N)** buys up to N of each kept item that merchant sells, counting everything of that kind in the
  pack, never more than N. It stops, and says why, when money or bag space runs out.
- **Junk and bank marks are exclusive**: an item is either set aside to sell or set aside to bank, never both. Marking an item
  (or a group) to bank takes its junk mark off, and marking it as junk takes its bank mark off; the menu then offers the other
  mark ("Mark as junk" / "Unmark bank" after banking, "Bank this" / "Unmark junk" after junking). Saved marks from an older
  version that carry both are settled when the mod loads: the **bank mark is kept and the junk mark dropped** (the safe one:
  nothing gets sold).
- **Bank marked**: **Bank this** in an item's menu; near a banker, **Bank marked (N)** deposits the marked stacks (never worn
  ones) and says so when the bank is full. **Taking an item out of the bank takes its bank mark off**, however you take it out
  (Withdraw 1, Withdraw stack, the multi-select's Take out of bank, or the game's own bank window), so it is not banked
  again at the next visit; a deposit never marks anything, and you mark an item again with **Bank this** if you want it
  banked again. A mark belongs to the item type (the same item id, as every mark here does), so it comes off for every stack
  of that item; a different item with the same name has its own mark and keeps it.
- **Do it when I get near**: three switches under the buttons (Sell junk, Restock, Bank), kept for each character. **Bank starts ON** for a
  character with no saved choice (an item marked to bank is a clear instruction); **Sell junk and Restock start OFF** (they
  sell items and spend tunar). A choice you have made, off included, is kept as it is; a character saved by an older version
  with only some switches on keeps those, and Bank stays off for it. A helper acts once per approach (it arms again after you have been out of the NPC's reach by 5 more units),
  never while any game window or session of any kind is up, a batch is running, or you are in a fight.
- **Sell 1 / Sell stack** in an item's menu: for a quick-slotted, no-trade or not ordinary item the game's own question
  comes up (the cases in which the game's own merchant window asks; `eqoa.actions.sell` asks the player itself since the
  client's 2026-11-25 change, so the mod no longer asks a second time), starting on **No**, naming the item and what you
  get. An ordinary Misc item sells at once, as in the game.
- Marks and the switches are saved in `helpers.cfg` (private files of the mod): `keep:<id>=<n>`, `bank:<id>`,
  `auto:<character>:<helper>=1`. The lines the helpers say go to the result line and the game's log (`addon eqoa_bag_viewer:
  [info] bag helpers: ...`); `eqoa/1` has no way yet to put a line in the chat window without sending it to the server.

It uses only the public `eqoa/1` API (`eqoa.inventory`, `eqoa.bank`, `eqoa.items`,
`eqoa.icons`, the `inventory.changed`, `bank.changed` and `loot.changed` events)
and is the worked example for the items API. While the window is closed it does
nothing but note pickups for the log.

AGPL-3.0-or-later.

## Item menu: Link in chat, Hide look, Destroy

An item's menu also has **Link in chat** (puts its link in the chat line), **Hide look** and **Show look** (for
worn items, where the server offers them) and **Destroy...**. Destroy never destroys at once: the game puts its own
**"Destroy ...?"** question on the screen, starting on **No**; Left or Right chooses, Enter answers, Escape is No, or
click Yes or No. Only your own Yes destroys the item; the window's result line then says what you answered.

## Several items at once (Ctrl+click, Shift+click)

**Ctrl+click** a slot to select it (it gets a blue outline); Ctrl+click it again to take it out of the selection. **Shift+click**
selects every slot from the one you clicked last to this one (**Ctrl+Shift+click** adds that range to what is already selected).
A plain click works as before and clears the selection. The line under the grid says how many are selected. **Right-click** any
selected item for one menu titled **"N items selected"**:

- **Mark as junk** and **Unmark junk** (marking asks once, with the same Yes or No as a single item, when some of the items are
  equipment, potions or other things the game's merchant would ask about), **Mark to bank**.
- **Sell (N)** at a merchant and **Put in bank (N)** at a banker: greyed with the reason when none is near ("no merchant nearby",
  "too far from the merchant"; the same reach rules as the other buttons, and a second click tries anyway). **Sell (N)** is one call to the client (`eqoa.actions.sell_many`): where
  any selected item is one the game's merchant window would ask about, the game's own question comes up once for them all
  (starting on **No**), and the mod asks nothing of its own. The stacks then go one at a time at the game's own pace, each
  looked up again by its own identity before its turn, so a stack that moved, emptied or changed while the run goes is
  skipped, not acted on by its slot. A stack that grew since you selected it is skipped too (the client sells whole stacks
  only). **Put in bank (N)** still goes item by item as a batch, and a refused item does not stop the rest. The result line
  says what happened, for example "Sold 4 of 5: 1 moved or gone; 1 worn item kept" (the reasons go to the game's log). The
  count is what was sent to the merchant.
- **Destroy...** puts the game's own question on the screen once, starting on **No**, listing the items (the first 12, then
  "...and N more", so Yes and No stay on the screen); nothing is destroyed without your own Yes. The items are then destroyed
  one at a time. It is greyed while a merchant window is open ("close the merchant first") and while another group action is
  running. The result line counts what the server really removed ("Destroyed 3 of 5"), and if zoning or closing the window stopped
  it, says how far it got and why.
- On the **Bank** tab the menu has **Take out of bank (N)**, which works the same way.

A worn item is never sold, banked or destroyed by a group action (the client refuses a worn stack in a group destroy for every mod, and skips one put on while it runs): it is skipped and the result line says so. The selection clears when
you close the window, zone, change tab, or when the action finishes. The controller does not multi-select (its d-pad and confirm
work as before). The mod uses `eqoa.actions.destroy_many` and the `ctrl` and `shift` fields of `pointer.pressed` (both
additions to `eqoa/1`).

## Bank tunar (Bank tab)

On the Bank tab the line under the grid shows **Bank tunar** and **Carried** (the tunar in your pack), and a row of its
own has a box, **Deposit**, **All**, **Withdraw** and **All**. Click the box and type an amount (digits only; Enter,
Escape or a click elsewhere ends the typing, and only while you type does the mod take the keyboard). **Deposit** and
**Withdraw** move that amount; each **All** moves everything you carry or everything in the bank at once. A button is
greyed when it cannot work (the box is empty, the amount is more than you carry or the bank holds, or the banker is too far or not near) and a click on it says why in the result line, which also shows the game's own answer, for example "Deposit 500 tunar: ok". You do not open the bank first: with a banker near, **Deposit** and **Withdraw** open the mod's own hidden bank themselves, move the tunar and let go, as **Put in bank** and **Bank marked** do. The amounts follow the game's own money updates. The controller's d-pad still moves over
the items only. It uses `eqoa.actions.bank_money`; with the game's full map open the box lets go of the keyboard.

## List on auction (at a bank)

On the Bank tab, an item's menu has **List on auction...** and the group menu of several selected bank items has
**List on auction (N)**. They are there whenever a banker is near or the bank is open, like Withdraw and Take out of
bank, and they open the bank themselves the same way (if the game's own bank window is open, close it first: listing
goes through this window). On a server without the auction the form says so.

A small form opens over the Bags window for each item: **Price** and **Buy now** are boxes you type a number in (click
a box, type, Tab moves on, Enter or a click elsewhere stops; the lowest price the game's own sell form allows, the
item's value, is filled in, and buy now starts at twice that and must be at least the price), **Time** is 12, 24, 48 or
72 hours, and on servers that sell by the stack a **Count** appears. **List it** only asks: the client's own "Are you
sure?" shows the item and its price, and only your answer lists it. **Cancel** (or Escape) leaves.

A group is listed one item at a time: the form comes again for each, with its own price box and **Skip** and **Stop**;
the next waits until the server has answered the last. A line at the end says how many went up. Mods & Addons shows
**Asks you first: put an item up at the auction**. The **Auction** button on the title row opens the EQOA Auction
window when that mod is on.
