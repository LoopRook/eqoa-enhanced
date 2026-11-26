# EQOA Auction

Off by default. Enable it in Mods & Addons (F10); it shows **Provides: Auction window (Shift+U)**, **Mail and auction:
can search the auction, look at your lots, look at your bids** and **Asks you first: bid at the auction, buy out an
auction lot** before you do.

An Auction window of its own, on **Shift+U** (Shift+A would be the strafe key; the Controls page has an **Auction** row
if you want another key). With **EQOA Start Menu** on,
its Auction entries (Search, My lots, My bids) open this window; the Bag Viewer has an **Auction** button on its
title row too. It works anywhere the server has the auction (the same rule as the game's own Start menu entries); on a
server without one it says so. It sits over play, and the game's own Auction pages are exactly as they were with the
mod off. Putting an item up for sale is done from the Bag Viewer (see its README), at a bank.

**Search tab**

- **Name**, **Seller**, **Level** (from and to), **Price** (from and to), **Type** (the game's wear slots), **Class** and
  **Race** narrow the search (the number boxes take digits only; Tab moves from box to box);
  **Search** (or **Enter** in a box) asks the server. Class and Race are the game's own lists, "Any" first, then **My
  class** / **My race** (your own, when the client knows them), then the names in the game's order; the wheel scrolls
  a list. **< Page** and **Page >** turn the pages.
- **Click a column title** (Name, High bid, Buy now, Time left) to order the rows shown: once for ascending, again for
  descending; a small ^ or v marks the active title, and a lot with no value in that column stays last. The game's own
  search has no sort of its own, so the order is of the page on screen only, and the line **ordered within this page** under the
  list says so (it shows whenever an order is on, because the server does not say whether there are more pages). A
  click on a row opens the lot shown on that row, in any order.
- The list shows each lot: its name and count, the high bid ("no bids"), the buy-now price and the time left, under
  column titles that are each as wide as their title and their biggest number (no title runs into the next). Your own
  lots are in yellow. Click a lot to see the seller and the lowest bid you may make.
- **Bid** takes the amount in the box (it starts at the lowest bid; click the box and type another) and **Buy now**
  buys the lot at its price. Each one asks you first: the client's own "Are you sure?" shows the price, and only your
  answer bids or buys. A bid the game's form could not make (below the minimum, above the buy-now price) is refused
  before any question. On servers that sell by the stack, **Count** sets how many you bid for.

**My lots** (name, buy-now price, time left) and **My bids** (count, price each, the amount held) list what you have
up and what you have bid, under column titles, with **Refresh**.

If the server does not answer a request in 15 seconds the window says so and does not repeat it: look again before
trying it again. It uses the window session of `eqoa/1` (`auction_search`) and the `eqoa.auction.get` read; when you
zone, die, turn the mod off or switch on **Use the game's windows**, the client closes it for you.

## Known gaps

- With the game's full map open this window hides itself and takes no clicks or keys; it comes back as it was.
- A click on another window lying over the box you are typing in does not end the typing.
- A refused "My lots" asks for "My bids" without saying so.
