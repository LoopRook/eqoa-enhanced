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

It uses only the public `eqoa/1` API (`eqoa.inventory`, `eqoa.bank`, `eqoa.items`,
`eqoa.icons`, the `inventory.changed`, `bank.changed` and `loot.changed` events)
and is the worked example for the items API. While the window is closed it does
nothing but note pickups for the log.

AGPL-3.0-or-later.
