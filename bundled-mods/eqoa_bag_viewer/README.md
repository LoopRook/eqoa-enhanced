# EQOA Bag Viewer

Off by default. Enable it in the Addon Manager, then bind **Bag viewer** on the
Controls page (it starts unbound). The key opens and closes a read-only Bags
window; Escape closes it too.

The window shows the 40 pack slots as item icons with stack counts and a green
**E** on worn items, the tunar you carry and used/capacity at the bottom.
Tabs: **All**, **Usable**, **Equipment**, **Charged**, **Misc** (the game's own
item-page tabs) and **Bank** (the bank's contents and its tunar, readable anywhere). Hover an item for its card: name, level, slot, stats, HP and
durability, value, and a red "You can't use this" when your class, race or level
rule it out. Under the grid is this session's loot log: the last 50 pickups,
newest first ("12:41 Rusty Sword x1, from a decaying skeleton"; the corpse is named
the corpse you asked to loot); the wheel scrolls it. Moving, selling,
looting and banking stay in the game's windows.

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
  it" line says so. The count on the button is what will really be sold. The line "Sold N junk items for X tunar" gives the
  game's own quote for the stacks the server confirmed.
- **Restock**: an item's menu has **Keep up to N (Restock)** (N starts at what the bags hold; the menu steps it by 5). Near
  a merchant, **Restock (N)** buys up to N of each kept item that merchant sells, counting everything of that kind in the
  pack, never more than N. It stops, and says why, when money or bag space runs out.
- **Bank marked**: **Bank this** in an item's menu; near a banker, **Bank marked (N)** deposits the marked stacks (never worn
  ones) and says so when the bank is full.
- **Do it when I get near**: three switches under the buttons (Sell junk, Restock, Bank), kept for each character, off by
  default. A helper acts once per approach (it arms again after you have been out of the NPC's reach by 5 more units),
  never while any game window or session of any kind is up, a batch is running, or you are in a fight.
- **Sell 1 / Sell stack** in an item's menu ask first (Yes / No) for a quick-slotted, no-trade or not ordinary item (the four
  cases in which the game's own merchant window asks; `eqoa.actions.sell` does not ask, so the mod does), naming the item
  and why.
- Marks and the switches are saved in `helpers.cfg` (private files of the mod): `keep:<id>=<n>`, `bank:<id>`,
  `auto:<character>:<helper>=1`. The lines the helpers say go to the result line and the game's log (`addon eqoa_bag_viewer:
  [info] bag helpers: ...`); `eqoa/1` has no way yet to put a line in the chat window without sending it to the server.

It uses only the public `eqoa/1` API (`eqoa.inventory`, `eqoa.bank`, `eqoa.items`,
`eqoa.icons`, the `inventory.changed`, `bank.changed` and `loot.changed` events)
and is the worked example for the items API. While the window is closed it does
nothing but note pickups for the log.

AGPL-3.0-or-later.
