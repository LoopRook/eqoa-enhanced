EQOA Native - Mods & Addons
===========================

This folder is where you install add-ons and UI modifications for the game.

HOW TO INSTALL A MOD:
1. Place the mod's folder or Lua file directly inside this "mods" folder.
   For example:
     mods/
       wereoxxs_ui/
         wereoxxs_ui.lua
         modules/
       resources/
         gear-icon.png
         fonts/

2. Launch the game and go to:
     Options -> Extended -> Mods & Addons -> Addon Manager

3. New mods are turned off by default. Select your mod in the Addon Manager
   and click "Enable" to activate it.

TROUBLESHOOTING:
If a mod fails to load or encounters an error, details are logged to:
  mods/addons_error.log
You can also view errors and diagnostics live inside the in-game Addon Manager.

---
FOR MOD AUTHORS:
Addons can be standalone .lua scripts or package directories containing
a mod.json manifest and an entry script (e.g., wereoxxs_ui.lua).
