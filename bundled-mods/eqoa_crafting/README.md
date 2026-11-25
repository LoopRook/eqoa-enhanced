# EQOA Crafting

Off by default. Enable it in Mods & Addons (Shift+F10); it shows **Provides: Crafting window (Shift+R)** and a
**Crafting: can fill the crafting tray, combine what is in it, answer the keep-this-recipe question** line before you do.

A crafting window of its own, on its own key: **Shift+R** (the Controls page has a **Crafting** row, so you can move
it; a key the game, a macro or another mod already has leaves the row empty and says why). Press the key again,
press **Escape** or click the **x** to close it. It sits over play, so your character can keep walking, and the game's
own Start menu > Tradeskills pages are exactly as they were. Nothing here combines by itself.

**Recipes tab**

- Click one of your six tradeskills to read its recipes (the game asks the server, as its own recipe book does; the
  list takes a moment). Each skill shows your level and rank.
- **Find a recipe**: click the box and type; it matches any part of the name. **Escape** or a click elsewhere leaves it.
- Click a recipe to see what it makes and what it needs. Each ingredient shows how many you hold: green when you
  have enough, red when you do not. Tools you are missing say so.
- **Fill tray** puts the recipe's ingredients in the tray from your bags (the tray holds four items at most, as in
  the game). It tells you which ingredient is missing when it cannot.
- **Combine** does one combine of what is in the tray, once per click. Click it again for the next one: it never
  repeats by itself.

**Experiment tab**

- Your crafting components are listed. Click one to put it in the tray; click a tray slot to take it out. **Combine**
  tries the tray, like the game's Experiment window.

**The question after a combine.** When a combine finds a recipe you did not have, the server asks whether to keep it.
The window shows the question with **Keep recipe** and **Leave it**; only your click answers it, and nothing else can
combine until you have. **Empty tray** clears the tray without sending anything.

What the server says about a combine shows under the buttons. The window does not remove, create or rearrange
recipes: use the game's own recipe book (Start menu > Tradeskills > Recipes) for that.

It uses the window sessions of `eqoa/1` (`eqoa.windows.open_session`, kinds `recipe_book` and `experiment`) and the
`eqoa.craft.get` read: when you zone, die, turn the mod off or switch on **Use the game's windows**, the client closes
it for you. While it is open the client reads the server's crafting replies for it; with it closed or off, the
game's own crafting windows are exactly as they were.
