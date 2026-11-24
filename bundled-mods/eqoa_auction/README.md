# EQOA Auction

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: Auction window (Shift+A)**, **Mail and auction:
can search the auction, look at your lots, look at your bids** and **Asks you first: bid at the auction, buy out an
auction lot** before you do.

An Auction window of its own, on **Shift+A** (the Controls page has an **Auction** row). With **EQOA Start Menu** on,
its Auction entries (Search, My lots, My bids) open this window; the Bag Viewer has an **Auction** button on its
title row too. It works anywhere the server has the auction (the same rule as the game's own Start menu entries); on a
server without one it says so. It sits over play, and the game's own Auction pages are exactly as they were with the
mod off. Putting an item up for sale is done from the Bag Viewer (see its README), at a bank.

**Search tab**

- **Name**, **Level** (from and to) and **Type** (the game's wear slots) narrow the search; **Search** (or **Enter**
  in a box) asks the server. **< Page** and **Page >** turn the pages.
- The list shows each lot: its name and count, the high bid ("no bids"), the buy-now price and the time left. Your own
  lots are in yellow. Click a lot to see the seller and the lowest bid you may make.
- **Bid** takes the amount in the box (it starts at the lowest bid; click the box and type another) and **Buy now**
  buys the lot at its price. Each one asks you first: the client's own "Are you sure?" shows the price, and only your
  answer bids or buys. A bid the game's form could not make (below the minimum, above the buy-now price) is refused
  before any question. On servers that sell by the stack, **Count** sets how many you bid for.

**My lots** and **My bids** list what you have up and what you have bid, with **Refresh**.

If the server does not answer a request in 15 seconds the window says so and does not repeat it: look again before
trying it again. It uses the window session of `eqoa/1` (`auction_search`) and the `eqoa.auction.get` read; when you
zone, die, turn the mod off or switch on **Use the game's windows**, the client closes it for you.
