# EQOA Bag Viewer

Off by default. Enable it in the Addon Manager, then bind **Bag viewer** on the
Controls page (it starts unbound). The key opens and closes a read-only Bags
window; Escape closes it too.

The window shows the 40 pack slots as item icons with stack counts and a green
**E** on worn items, the tunar you carry and used/capacity at the bottom.
Tabs: **All**, **Equipment** and **Bank** (the bank's contents and its tunar,
readable anywhere). Hover an item for its card: name, level, slot, stats, HP and
durability, value, and a red "You can't use this" when your class, race or level
rule it out. Under the grid is this session's loot log: the last 50 pickups,
newest first ("12:41 Rusty Sword x1"); the wheel scrolls it. Moving, selling,
looting and banking stay in the game's windows.

It uses only the public `eqoa/1` API (`eqoa.inventory`, `eqoa.bank`, `eqoa.items`,
`eqoa.icons`, the `inventory.changed`, `bank.changed` and `loot.changed` events)
and is the worked example for the items API. While the window is closed it does
nothing but note pickups for the log.

Not shown yet: the game's Usable, Charged and Misc tabs (the API gives no item
type or charge definition to sort by) and the corpse a pickup came from.

AGPL-3.0-or-later.
