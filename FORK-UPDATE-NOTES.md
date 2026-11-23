# Enhanced fork update notes

Upstream changes remain in UPDATE-NOTES.md.

## Unreleased

- **Spellbook 2: search, groups, icons and drag and drop.** In the Spellbook (Shift+K): a **Search** box above the list filters abilities by name as you type (click it, type; Backspace removes a letter, the small x clears it, Escape lets go of the keyboard). The buttons **Self**, **Target**, **Group**, **Pet** and **All** show only the abilities that affect that; All shows everything. The list is grouped under **Levels 1-10**, **11-20** and so on. Each ability shows its icon in the list and in the 20 memorized slots. **Drag** an ability from the list onto a slot to memorize it there (an ability already on that slot is cleared first); drag a slot onto another to move it, or onto a taken slot to swap the two; **right-click** a slot to clear it. A move is the same Unmemorize and Memorize the game does by hand, sent one at a time; if the game refuses one, the move stops and the line under the description says why. The window is a little taller (419 instead of 416 pixels) and nothing is cut off at its default size. The **Memorize** and **Cast** buttons stay as they were.
- For mod authors (`eqoa/1`, additions only): the ImGui bridge now has `IsMouseDown`, `IsMouseReleased` and `GetMouseDragDelta` (with `IsMouseDragging`, they follow a press to its release), listed in docs/modding. Nothing that existed changed.
- **Owner-test fixes: Bag Viewer, Spellbook, Quest Journal.** With **Use the game's windows** on, pressing a window's key no longer pops a warning on screen (the note goes to the game's log only). The Bag Viewer dims an icon only for gear you cannot use: crafting, misc, junk and other items are never dimmed (the red line on an item's card is unchanged). The Spellbook now memorizes any ability you have learned and not yet memorized, onto the first free slot (it refused every one, such as Return Home, before), and lists only abilities you have learned, as the game's Abilities page does. Its texts (the slots hint, the description, the game's answers, the key hint beside the x) are wrapped to fit the window, which is a little taller.
- For mod authors (`eqoa/1`, additions only): `eqoa.abilities` records have a new field `learned` (the character has the ability; `usable` stays the game's "on a slot" flag, which is false for an ability not yet memorized). `eqoa.actions.assign_ability` now refuses (`not_ready`) an ability already on a slot or not learned, as the Abilities page's Memorize does, and no longer refuses one that is merely not yet memorized.
- **EQOA Social window mod (off until you enable it in Mods & Addons).** One window with tabs for **Friends**, **Ignore**, **Guild**, **Who** and **Group Search**, sitting over play so you can keep walking while the game's own windows stay untouched. **Shift+F** opens it on Friends (see who is online, add or remove friends, send a tell or group invite from a row, or manage your Ignore list); **Shift+G** opens it on Guild (roster with ranks and message of the day; invite, promote, demote, edit MOTD, and kick with the game's own "Are you sure?" box; disband and quit keep the game's own prompt). **Who** and **Group Search** let you search who is online, sign up for or leave group search on servers that offer it, and tell or invite from a row. Incoming **Friend Requests** and **Guild Invitations** appear in the mod's prompt box, refuse automatically after 45 seconds like the game, and ask for your own Yes click before accepting. With the mod off, the game's windows are unchanged.
- For mod authors (`eqoa/1`, additions only): `eqoa.social` (`friends`, `ignored`, `friend_request`), `eqoa.guild` (`get`, `invite`), `eqoa.who` (`get`, `request`), `eqoa.lfg` (`get`, `search`, `register`, `unregister`), events `friends.changed`, `ignored.changed`, `guild.changed`, `who.result`, `lfg.result`, `lfg.changed`, `friend_request`/`guild_invite` takeover with 45-second timeout and `pass`, optional target arguments on `eqoa.actions.group_invite`, and community actions (`friend_add`, `friend_remove`, `friend_accept`, `friend_refuse`, `ignore_add`, `ignore_remove`, `guild_invite`, `guild_promote`, `guild_demote`, `guild_kick`, `guild_set_motd`, `guild_accept`, `guild_refuse`, with client-drawn confirmation for `guild_kick`, `friend_accept` and `guild_accept`). See docs/modding.
- **Bag Viewer item cards say who can use the item.** Hover an item: a Class line ("All classes" or the classes that can use it), a Race line when some races cannot, and the item's description at the bottom in grey. If your class, race or level rule the item out, just that line turns red (the item's name stays as it was) and the red "You can't use this" stays. In the grid, the icon of an item you cannot use is dimmed, and an item marked to bank (blue **B**) or kept up to a number for Restock (blue **R**) shows that letter in its slot's bottom-left corner.
- **Bags, Spellbook and Quest Journal as windows of their own, on their own keys.** Three bundled mods (off until you enable each in Mods & Addons, which now says **Provides: ...** before you do): **Bag Viewer** on **B**, **EQOA Spellbook** on **Shift+K** and **EQOA Quest Journal** on **Shift+J**. Press the key again, press Escape or click the x to close one. Each opens over the game, so your character can keep walking; the game's own Start menu and its Items, Abilities and Quests pages are exactly as they were. Each key has a row on the Controls page and can be moved; a key the game, a macro or another mod already uses leaves the row empty and says why (the game's key always wins). No controller button opens these windows yet: Start menu > Items still works.
- **The Stats Box key moved from B to Y**, and the Bag Viewer opens on **Shift+B** (plain B belongs to the game's own key-binding page). If you had set a key yourself, it keeps that key, and a Bag Viewer left without a key says why on its Controls row.
- **A mod can destroy an item or drop a quest, but only if you say yes.** The game puts its own question on the screen ("Destroy ...?" or "Drop the quest ...?"), starting on **No**. Left or Right (or Tab) chooses, Enter answers, Escape is No, or click Yes or No. A mod cannot answer for you, skip it or hold it open: it ends as No after 30 seconds, when you zone or die, or when the window that asked closes. Mods & Addons shows **Asks you first: destroy an item, drop a quest** for a mod that can ask.
- **Turning on "Use the game's windows", or turning a mod off, closes its windows at once.** Corpse, merchant and bank are the game's own windows again, and a mod window you had open closes. Leaving the world (zoning, logging out) and dying close them too.
- For mod authors (`eqoa/1`, additions only): window sessions (`eqoa.windows.open_session`, `session`, `close_session`, `pass`, `focus`, `release_focus`, `confirm`), the events `windows.session`, `windows.key` and `confirm.closed`, the actions `destroy`, `link_item` and `hide_look`, `drop_quest` (now supported, behind the question above), and the manifest's `provides` label. See docs/modding. Nothing that existed changed except `destroy` and `drop_quest`, which were `unsupported` / `not_yet`.

- The Quest Markers switch is gone from Options > Enhanced: U turns the Quest Helper off, and J on the map shows or hides just the marks.
- **`Start EQOA (performance log).bat`** is now in the download beside the game. It starts the game with timing lines added to the log (`%APPDATA%\eqoa-rs\eqoa.log`); use it when we ask for a performance log.
- **Frames demo mod (off until you enable it in Mods & Addons).** Three small windows you can drag: your effects with the time left on each (click one to take it off, where the server lets you), a cast bar for you and for your target, and the target's name in its consider colour with its title and guild, plus buttons to invite it to your group, send your pet in and call it back off. With the mod off nothing changes. The effect times need a server that offers the buff window and **Buff Window** on in Options; the cast bars are a clock from the cast's announced length, because the game is never told when a cast is interrupted or lands.
- **Frames demo mod fixes (owner test 2026-11-22).** "click to take it off" no longer follows your cursor: a tip shows only while the pointer is on an effect. Hold the pointer on an effect to see what it does (the description of the ability you have with that name; an effect of someone else's spell says the game does not tell), the time left, and the take-off line where it applies. The target's cast bar no longer shows while an enemy is only fighting you: the game announces a melee round the same way as a spell, and the demo (and `eqoa.casts`) now ignore melee rounds, so the bar shows real casts only. The game's log gets a `frames:` line for each cast of your target (and your own), and one for each melee round it ignored, saying what the server sent.
- For mod authors (`eqoa/1`, additions only): effects with time left, whose and removable (`eqoa.effects.buffs`, `eqoa.effects.pet`), casts with start, length and end (`eqoa.casts`, the events `cast.started` and `cast.ended`), the quest tracker's goals with how many you have (`eqoa.quests.goals`, on servers that send them), training points, breath, the target's title, guild and consider colour, and each raid member's group, role and marks. New actions, each behind its own permission that Mods & Addons shows: `group_invite`, `pet_attack`, `pet_back_off`, `emote` (wave, bow, point, cheer), `jump` and `remove_effect`. Each reports that it was sent, never that it worked.
- **Complete HUD hiding for mods.** Mods can now hide any HUD piece (including the whole player frame, raid list, quest tracker, DPS meter, breath gauge, spell bars, and utility bars in both the stacked and carousel bar layouts, or all pieces except the minimap via `all_pieces`) using the existing HUD hide controls, without changing what `all` hides for older mods. Clicking or scrolling where a hidden piece sat does nothing, while Move HUD, Hide UI, and the local report button keep working. With no mod enabled, the HUD is unchanged.
- **Raid list, ready checks, loot rolls, and master loot for mods.** Mods can read the raid list and open ready-check or loot-roll strips, replace the ready-check and loot-roll strips (keeping their timers running and falling back to the game's strips if the mod is turned off or **Use the game's windows** is enabled), answer ready checks and loot rolls, and assign or abandon master loot.
- **Target Frame Demo mod** (off by default in Mods & Addons): a small bundled example that hides the game's target frame and draws its own.
- **Bag Viewer helpers at merchants and bankers.** Near a merchant the Bag Viewer's **Sell junk** button sells every item you marked as junk with one click, out of sight, and says "Sold N junk items for X tunar". Marking is your decision: **Mark as junk** in an item's menu marks an ordinary Misc item at once, and asks once ("Mark <item> as junk?", Yes or No) for equipment, potions and other usables, quick-slotted items and no-trade items. Sell junk then sells everything marked, gear included; the only exception is an item you are wearing right now, which it keeps and says so ("Kept 1 junk item: you are wearing it"). Mark an item **Keep up to N (Restock)** in its menu and, near a merchant who sells it, **Restock** buys up to N (counting what your bags already hold, never more than N, stopping when money or bag space runs out). Mark items **Bank this** and, near a banker, **Bank marked** deposits them (worn items never go). Each of the three has a switch in the row **Do it when I get near**, off by default and kept for each character: switched on, it acts once when you walk up, never while a game window is open or while you fight, and again only after you have walked away and come back.
- **Talking to a merchant or banker yourself is unchanged.** The game's own window opens as always; the helpers only act on their button or on their switch.
- **Selling by hand asks first for the same items the game's shop window asks about.** The Bag Viewer's Sell 1 and Sell stack now ask "Sell it?" for an item that is quick-slotted, no-trade or not an ordinary item, and say which; Yes sells it, No keeps it.
- The lines the helpers say ("Sold 3 junk items for 120 tunar", "Restocked: ...", "Banked 2 items") appear in the Bag Viewer's result line and the game's log for now: a line in the chat window itself needs a small addition to the mod API that is not made yet.
- **Auto-loot works in a fight and leaves your target alone.** When a creature you hit dies, Auto-loot now asks for its corpse by name instead of targeting it, so it loots straight away even while you are still fighting, your target stays on the next enemy and your auto-attack keeps swinging. If the server turns out not to accept that, the game falls back to the old way (target the corpse, loot it, put your target back) for the rest of the session, writes one line to the game's log saying which method is in use, and Auto-loot waits for the end of a fight again. It never loots while you are casting.
- **Auto-loot is quicker.** It notices a death within a tenth of a second during a fight, asks again half a second after "the game could not open it just now" (it used to wait five seconds), and takes a corpse with nothing marked as junk in one go instead of item by item.
- **Auto-loot goes back for what did not fit.** When a take leaves items behind because your bags were full, Auto-loot waits until you have free bag space, then returns for the rest if you are still in reach of the corpse and it is still there, as often as needed. If you walk away it lets the corpse go and does not chase you. A take that ended early for another reason (the window went away, the server was not ready) is asked again when you are in reach and five seconds later, at most two more times. A corpse you closed or stopped yourself, and junk you chose to leave, are left alone.
- If the game's own loot window ever shows during Auto-loot, one line in the game's log says so (it has never been seen; this is a safety check).
- For mod authors: `eqoa.actions.open_loot(entity, {keep_target = true})` and `loot.changed`'s new `closed_by` field are described in docs/modding. Nothing that existed changed.
- **Character window (bundled mod, off until you enable it in Mods & Addons, then Shift+C).** It shows your stats as `now / cap (with gear)`, armor and resists, HP, power, Class Mastery points and your training points, with a **+** and **-** beside each stat. Place points, press **Train**, and the game asks **Are you sure?** (starting on **No**): your points are spent only when you answer Yes yourself, and spent points cannot be taken back. Its **Masteries** tab lists your Class Mastery skills; click one to read it, then **Buy** or **Refund**, which ask **Are you sure?** the same way. Its **Factions** tab shows your standings a page at a time and its **Combat** tab your combat record (on a server that keeps one). With the mod off, the game's own Character Summary, Class Mastery, Factions and Combat Record windows are exactly as they were.
- For mod authors (`eqoa/1`, additions only): `eqoa.actions.train_stats`, `request_factions` and `request_combat_record`; `eqoa.actions.mastery_buy` and `mastery_refund` are now supported and, like `destroy` and `drop_quest`, always ask the player first; the window sessions `"trainer"` and `"mastery"` (a `"mastery"` session gets the Class Mastery list with the game's own window closed); `eqoa.factions.list`, `eqoa.combat.record` and `Training.caps`; Mods & Addons says **Asks you first: ...** for them. See docs/modding.

## 2026-11-21 · Enhanced 0.7.0

- **Documentation for mod authors.** `docs/modding/` now describes the mod API (`eqoa/1`) as it is built: a getting-started guide, a starter mod to copy, the full reference, what each bundled mod shows, and what `eqoa/1` promises. **`eqoa/1` is a stable candidate:** the guard is on and we do not expect breaking changes; the final freeze comes when the API is offered upstream (the PR), after upstream has had its say.
- For mod authors, corrections made before the guard went on: a mod may decline any of the game's own Yes/No boxes but may accept only a group invite (quit, dismissing a pet, leaving or disbanding a guild need your own answer); NPC conversations still answer. Cancelling a request the game has already sent now returns false and its real result still arrives. Dropping a quest and buying or refunding masteries are `not_yet` for mods until a later round brings them back with a confirm you see.
- **Bag Viewer sells single items near a merchant.** Sell 1 and Sell stack work whenever a merchant is in reach: with no merchant window open, Bag Viewer talks to the merchant for you and sells without opening the game's window. After a sale the game's merchant window no longer pops up.
- Mods need the map waypoint permission to place or clear a personal pin. New pins must be on the loaded map's ground.
- Disabling a mod removes only the map pins it placed and still owns, in every world. A pin you clear and place again, even at the same spot, stays yours.
- When a mod's selling or banking run stops (its Stop, or **Use the game's windows** for a mod that replaces the window), a sale already sent still counts once the server answers, and a step that was only queued is never sent. Runs that reached a merchant or bank by talking to them stop too.
- An enabled addon that changes package type or adds permissions stays disabled until you approve it again in Mods & Addons.
- Mods with chat permission can send ordinary chat. Lines starting with a slash require the command permission, including chat submitted through the game's editor.
- Mods & Addons shows **Replaces game windows** and **Can answer NPC dialogs** when a mod declares those abilities. **Use the game's windows** is an emergency override: it turns off every mod that replaces a whole game window, and the game's own windows come back. Mods that only help (Bag Viewer's selling, Take all, Auto-loot) keep working.
- Map Search waits for the Creature Journal to start before reading its shared data.
- Mods opening Mail, Auction, or Who now open the right window.
- **A key for the Mods & Addons window.** Controls has a new row, **Mods & Addons**, beside the mod keys; it starts on **F10** (unless something already uses F10: then it starts empty and says what has it). Pressed in the world it opens the Mods & Addons window, pressed again it closes it; it works with no mod enabled, follows the same rules as a mod key (swaps, the client's own keys, cancel, restart) and is not fired while you type or a menu has the keyboard. On Windows F10 is also the system menu key; if it ever sticks or eats the next key, rebind the row and tell us. The key does nothing on the loading screen, with the full map up, or while the world changes (where the window would not be seen or usable), and a combination you give it (say Shift+Space) is the row's alone even with no mod enabled.
- **A key does one thing.** A key is its name: the modifiers you hold plus the key, so Shift+Space is not Space. A mod may use a modifier plus a key the game already uses (a mod on Shift+Space, with Jump still on Space): while it does, that press belongs to the mod alone, and Space by itself still jumps. A real conflict is only the same name used by both: a bare key the game uses, or the very combination bound to a game control or a macro. Giving a mod such a key frees or swaps the game control (the same line you get between two game controls says which; a macro's key is refused), and giving a game control a key a mod uses does the same to the mod. A mod's default key never takes a key already in use: its row starts empty and says which control has it, and it says so again after a restart. Which mod gets a shared default no longer depends on the order mods load in. If your saved keys already share a name, the game control wins, the mod's row on Controls is marked with ! and says why, and nothing you saved is changed until you pick a key. F3, F11, F12, Escape, Alt+Enter, Ctrl+C, V and X (Cmd+C, V, X and Cmd+, on a Mac) belong to the client and can no longer be mod keys (an old saved one stays in the file but does nothing, and the row says why). A known consequence: on a German keyboard a mod on Shift+7 owns that press, so the command line no longer opens from Shift+7 there; you chose the binding and can rebind either side. One thing not changed: the Macros page still lets you put a macro trigger on a key a mod has; the macro wins and the mod's row is marked. Cancelling Options (Triangle on the menu) puts mod keys back together with the game's. One gap: with Auto Save Options off, leaving Options some other way without saving, then restarting, can leave a mod's new key saved beside the game's old one; the game control wins and the mod's row is marked.

## 2026-11-19 · Enhanced 0.6.0

The mod API (`eqoa/1`) is not frozen yet. Bundled mods and client updates ship in this release; third-party mod authors should wait for the freeze before building on it.

- **Bag Viewer works without the game's windows.** It never opens the game's corpse, merchant or bank windows for its own buttons. **Look at corpse** shows what is on the nearest corpse without taking anything; **Take all** and **Take all but junk** take it. **Sell junk** works near merchants, and **Open bank**, **Deposit** and **Withdraw** work near bankers. Buttons grey out with "too far from the merchant" or "too far from the banker" when you are out of reach, and a window you open yourself is never blocked. Server refusal messages show in the Bag Viewer's result line.
- **Junk marking.** Mark items as junk in the Bag Viewer (shared by item type); only marked items are sold or skipped. The old "Also sell grey items" option is gone.
- **Auto-loot** is a choice in the Bag Viewer window: **Off**, **Everything** or **Everything but junk**, kept for each character. It loots creatures you killed once you are out of combat (never another player's kill or a player corpse), asks again when you walk closer or a few seconds later (three tries), and brings your previous target back. Every decision is a line in eqoa.log ("auto-loot: corpse ...: queued / waiting: in combat / too far / skipped, not your kill / done"). Opening the Bags window no longer slows the game in a crowded zone.
- **Mods can replace the corpse, merchant and bank windows** (the mod API for this is not frozen yet). The game's own windows still show when you click a corpse, merchant or banker yourself.
- **Wereoxxs UI is much faster**: long chat histories and custom fonts no longer cost a frame each; on a recorded session it went from 11.8 to 1.9 ms per frame. Third-party Forge mods that call a missing camera or icon service now show a clear diagnostic instead of silently returning nothing.
- **Item and action APIs for bundled mods**: mods can read items and bags, and act on loot, merchants and banks, without a fresh click or key press per action. Mods & Addons labels any mod that declares such automatic actions.
- **From the vanilla client** (192 changes in the last sync, plus the newest): auras and armour looks, a buff window and **stacked spell bars** (movable on their own in Move HUD, with both click paths following), raid lists, Touch Controls, texture packs that **precache by default** (large packs show textures sooner after zoning or a restart), and on servers that support them: mercenary windows that take the keyboard (arrows, Enter or E, Escape; Page Up and Page Down in the spell window), Hardcore characters with a fall notice and memorial on the character list, [OG] account restrictions (locked map and item-link rows), and a dye picker at dyers and the Pet Groomer.
- Options -> Extended is now the vanilla client's five groups (Graphics, UI, Other, Touch Controls, Multi-boxing). The **Enhanced** page (HUD extras and Mods & Addons) is in the **UI** group; every row keeps its saved setting. The path to the Addon Manager is Options -> Extended -> UI -> Enhanced -> Addon Manager.
- Group members' and your pet's power bars are now the game's own (Options -> Extended -> UI -> Power Bars, on by default). Mana Bars keeps your target's power bar. A mod that hides the effects strip or ability bars now also hides the buff window and stacked spell bars.
- **Round Minimap** is a bundled mod, on by default: a clean circle with a ring, in the same place and size, following Move HUD. N toggles it, the wheel zooms it, and hovering a creature, player or pet shows its name, level, kind and distance. Turn the mod off and the square minimap returns. Clicking it does not open the map (press M).
- **The M map uses the game's own look**, with Map Search (F2, F4 clears the guide) and Creature Journal as bundled mods; your journal and key choices carry over. Search stays responsive while creatures move, keeps stable result order, filters as you type, and its button is back at the built-in size. Known creature guides upgrade to live creatures, Track waypoint follows moved pins, and import/export report their result.
- **Bag Viewer** (off by default; turn it on in Mods & Addons, then bind **Bag viewer** under Controls): your 40 pack slots, tunar, tabs (All, Usable, Equipment, Charged, Misc, Bank), a hover card with stats and a red "You can't use this" when it does not fit your character, and this session's loot log.
- Behind the scenes: faster and safer testing.

## 2026-11-11 · Enhanced 0.5.3

- Fix: bundled mods that are on by default (HUD numbers and Mana Bars) are now on for players who already had a mod profile.

## 2026-11-11 · Enhanced 0.5.2

- The first published release since 0.4.14: it contains everything listed under 0.5.0 and 0.5.1 below (those two were prepared but not published, because a release check failed).

## 2026-11-11 · Enhanced 0.5.1

- Creature recording now has an enabled-by-default Creature Journal addon, with its own import/export rows, bounded Forge journal imports, and quiet/periodic saves.
- Map Search is an enabled-by-default addon with rebindable F2/F4, creature/group/place tabs, a minimap button that follows Move HUD, shared journal results, and live/known guidance.

- Fixed live addon map updates and lost-guide notifications. Existing UI mods keep the minimap unless they hide it explicitly. Private journal JSON can reload and flushes on disable or quit; large Lua heaps require the declared `large_data` capability. Catalogue tables load in bounded slices.
- Addon authors: map state, markers, native minimap views, owned guides, minimap visibility/layout and pointer claims are available. Addon drawing and input can run over the open map. Bounded packaged JSON loading and private asynchronous JSON saving support the coming search and journal mods.
- Added upstream Move HUD and Reset HUD. The round minimap, its hover cards and search control follow its saved position; HUD numbers, mana bars and Target of Target follow their stock bars. Upstream also restores a lost graphics device in place and improves mercenary tags.

## 2026-11-11 · Enhanced 0.5.0

- Added upstream item links, Q-tap monster information, map places and indoor minimap floors. Crash reports sent to a supporting game server identify EQOA Enhanced; the local crash-save offer remains available.

- Rebound mod keys survive later changes to native controls. Failed HUD option saves report an error, and disabling Mana Bars stops requesting group power updates. Target of Target fades with the native target plate.

- Four HUD extras (Stats Box, Target of Target, Mana Bars, and HUD numbers) are now bundled Lua mods (`eqoa_stats_box`, `eqoa_target_of_target`, `eqoa_mana_bars`, and `eqoa_hud_numbers`), enabled by default. Stats Box and Target of Target start off; Mana Bars and numeric readouts start on. Existing settings carry over, and the Stats Box key can be rebound on Controls. Disabling a mod removes its HUD options and key rows.
- Added upstream raid leadership marks, loot-roll strips, raid menus and master-looter controls on servers that offer them. Hat D-pads and WebGL2 context-loss handling include upstream fixes.

- Added vanilla's Number Colors, Pet Effects, optional server DPS meter and Macros page. Enhanced HUD and Mods & Addons follow every vanilla Extended page.
- Touch controls now have separate stick/button sizes, opacity and edge spacing; existing Touch Pad Size settings carry over.
- Integrated upstream server worlds, race bodies, capes and light-shadow improvements.

- Kept the Enhanced NPC display and distance defaults (64/650) when upgrading older settings; explicit choices stay as saved.
- Community marker suggestions use the vanilla name-and-note edit boxes for keyboard and controller.
- Extended keeps the vanilla UI, Map and Textures pages in their original order. Enhanced HUD and Mods & Addons follow them; existing settings are kept.

- Wereoxxs Enter opens its own chat again after switching away from and back to the game. Optional legacy-settings probes no longer clutter the Addon Manager.
- Restored signed game icon IDs for Forge UIs, fixing overlapping ability icons in Avoids Modern UI.
- Added Map and Quest Helper master switches while keeping map search, guidance, the creature journal and circular minimap.
- Group waypoints now use O; G attacks, and Space jumps on servers that offer jumps. Saved custom bindings remain supported.
- GMs can Ctrl-click the map to travel when their server offers it. Distant quest offers no longer crowd the minimap rim.
- Integrated upstream lighting, shadows, camera, character and input updates.
- Quest tracking stays silent when turned off before the server's first request, as the vanilla client's does.
- Pressing Tab no longer reopens the mod chat input after closing it with an empty Enter. Tab targets once, and the game keeps the key.
- Mod UIs keep receiving combat and cooldown events when another mod listens to player movement; waiting or disabled mods keep native state capture idle.

- Fixed severe in-world slowdown from addon state capture, including with every addon disabled.
- Nightly builds now say they are Nightly and name the build commit, so they no longer look like the Stable version. Stable is unchanged.
- Added the experimental native `eqoa/1` mod API and an EQOA Example Hearts mod. Enable it from the Addon Manager after installing bundled mods. The API remains under review until the Round 5 audit.
- Mods share bundled resources read-only and keep file changes in private overlays. Unsupported calls appear in the Addon Manager and its error log.
- Effects and controller reads use native game state; existing Forge UI mods keep their compatibility interface.
- Mod chat queued while Combat Record is open waits until it closes. World changes cancel unsent mod chat with a visible Addon Manager diagnostic.

## 2026-11-08 · Enhanced 0.4.14

- Nothing changes in how you play. Behind the scenes, Enhanced's own options are kept apart from the vanilla client's, so new vanilla options arrive sooner and with fewer surprises.

## 2026-11-08 · Enhanced 0.4.13

- New from the vanilla client: texture packs made for PCSX2 work here as they are. Put the pack's files in `%APPDATA%\eqoa-rs\textures\SLUS-20744\replacements` (made for you the first time), then turn on Options > Extended > Textures > Texture Packs.
- Dump Textures, on the same page, saves each texture the game draws to `textures\SLUS-20744\dumps` under PCSX2's names, so a pack you make from them works in PCSX2 too.
- Both are off until you turn them on, and the game looks exactly as before.
- On servers with their own races, the races past the game's original twenty (such as Orc, Kobold or Gargoyle on the Heirloom server) now offer every class the server allows at character creation; before, they showed no class to pick.

## 2026-11-08 · Enhanced 0.4.12

- New from the vanilla client, on servers that have them: a Combat Record under Start > Character (your PvP kills, deaths, streaks and duels; /record in chat), and more races to make at character creation, with every class open to every race where the server allows it.
- New characters' names may be three letters long.
- /bug opens the bug report form, as F3 does, on servers that take bug reports.
- The raid list shows each member's power as a thin blue bar under their health.
- The spell book has Unmemorize for one ability, and a long spell description scrolls with R1/R2, the mouse wheel, or PageUp/PageDown.
- New players start with Auto Attack on. If you already saved your options, your choice stays.

## 2026-11-07 · Enhanced 0.4.11

- Nothing changes in how you play. This release is built entirely from the public source code, including the live server connection code that earlier releases kept private. Downloads and auto-updates work as before.

## 2026-11-07 · Enhanced 0.4.10

- Fixed: mod target boxes (Avoid's Modern UI Target Level, ARPG UI, Wereoxxs UI) showed "(none)" or nothing for some targets on servers that show more than 24 people and creatures, such as eqoaclassic. They now show whatever you target, and your pet and group are found the same way.

## 2026-11-07 · Enhanced 0.4.9

- New from the vanilla client: in a raid group on a server that has them, the group's bars become a raid list of everyone in it. Click a name to target that member. Turn it off in Options > Extended > Map & Quests > Raid List.
- Copy and paste: Ctrl+V, Ctrl+C and Ctrl+X work in the game's text boxes. Drag over chat lines with the mouse to select them, and Ctrl+C copies them.
- Controller macros on the Controls page.
- GMs can fly with /fly on servers that have the command.
- A server's own races (such as eqoaclassic's Hill Giant) show under their own name, plus other fixes.
- F3 on a server that takes bug reports now opens the game's Support form. Everywhere else, F3 still saves a problem report to your Desktop, and the HUD's Report button always does.
- EQOA Enhanced is now under the GNU Affero General Public License v3 or later, like the vanilla client. Nothing changes in how you play. Earlier versions stay under MIT (see NOTICE).

## 2026-11-04 · Enhanced 0.4.8

- Minor update.

## 2026-11-04 · Enhanced 0.4.7

- Minor update.

## 2026-11-04 · Enhanced 0.4.6

- LoopRook's additions first published in 0.4.6 or later use the EQOA Enhanced License; earlier code stays MIT, upstream and third-party licenses stay intact, and anyone can still play the official release.

## 2026-11-04 · Enhanced 0.4.5

- Minor client update.

## 2026-11-04 · Enhanced 0.4.4

- Fixed: with Wereoxxs UI's chat on, holding R2 hid the ability bar and scrolled nothing you could see. R2 now scrolls the mod's chat up and down, and letting go returns to the newest line.
- Holding R2 to read chat history now keeps the ability bar and minimap on screen, with or without mods.
- Problem reports (F3 or the HUD's Report a bug, and the offer after a crash) still save a ZIP to your Desktop. They now also include a short note of what the game was doing: zone, target, nearby creatures and the game's mode. Your account name, password and Windows user folder are removed; nothing is uploaded.
- From the vanilla client: Touch Pad Size (Extended > Controller) for the on-screen controller; Class Mastery fixes, so Master Classes that branch (such as Spellbinder's) no longer refuse their second branch, and on eqoaclassic a Master Class's abilities can be bought again after reopening the window; plus speed-ups and fixes.

## 2026-11-02 · Enhanced 0.4.3

- Fixed: the creature journal forgot NPCs above level 60 (and ones of unknown level) every time you restarted the game, and importing a FrontiersForge journal skipped them. They are now kept.

## 2026-11-01 · Enhanced 0.4.2

- Fixed: map search listed other players near you by name. It now lists only NPCs and creatures, never players or their pets. Your creature journal never saved players and is unchanged.
- Fixed: hovering over a player on the minimap called them an NPC; it now says Player (or Pet).

## 2026-11-01 · Enhanced 0.4.1

- Mod profiles: each character loads its own mods, settings and window layout. New characters can use a shared Tank, Healer, Melee or Caster profile (picked from their class) or make their own copy; manage profiles in the Addon Manager.
- ARPG UI and Avoid's Modern UI are now included next to Wereoxxs UI, and the game adds any missing included mods to your mods folder, even if you update automatically. They stay off until you turn them on. If you had removed one before, it is added back once (still off); delete it again and it stays gone. Included mods also update themselves, but only files you haven't edited.
- Wereoxxs UI has a weapon-set bar: turn it on in its settings; click a set to use it.
- The map now learns creatures as you play: every NPC you see goes into your own creature journal, and map search finds them alongside the built-in list. Only creatures are recorded, never players or pets. Export your journal or import a friend's (also works with Wereoxx's PCSX2 map mod files).
- Mods stay visible while you talk to NPCs (coachmen, quest givers) and step aside when the full map is open.
- Fixed: mod ability bars sometimes missing cooldowns, especially short ones such as Feed.
- Fixed: Avoid's Modern UI couldn't be turned on at level 60; its XP bars now hide at max level.

## 2026-11-01 · Enhanced 0.3.0

- Two more FrontiersForge mods now run natively, unchanged: ARPG UI (health and power orbs, action tray) and Avoid's Modern UI (player, target, group and quest panels). Like Wereoxxs UI, they step aside for game menus. Turn on one at a time in the Addon Manager.
- Problem reports: press F3 (or use the crash offer at the next start) and choose Save to Desktop. You get an EQOA-Enhanced-report zip with your note, a screenshot and the logs, ready to send to us. Your account name, password and Windows user folder are removed, and nothing is uploaded anywhere.
- Mods that save files are safer: writes are batched, and mods can no longer freeze the game through file access.

## 2026-11-01 · Enhanced 0.2.2

- Mods now step aside for every game window: the spellbook, quest journal, options, merchant, bank, trainer, blacksmith, auctions, trade, mail and more.
- With a mod hiding the game's ability bars, the game's item bar no longer pops back when you scroll the hotbars.
- With a mod's chat on, holding C or R2 no longer brings up the game's chat history, and the minimap stays put.
- Fixed mods stopping (and the whole mod UI vanishing) after a few hours of busy chat.
- Mod settings now stay with you when you move or re-extract the game folder; existing settings carry over automatically.
- If a mod ever stops with an error, the game's own HUD comes straight back.

## 2026-10-31 · Enhanced 0.2.1

- Brings in the latest improvements from the original vanilla client, with all Enhanced features and mods unchanged.
- New Ground Fog option under Options, Extended, World (the PS2 look by default, or Exact); Shadows moved to the Characters page.
- Duels, with an orange marker on the compass, on servers that turn duels on.
- Sound no longer cuts out in big crowds and busy fights, and far-away or blocked creature swings are quieter.
- Walking while casting matches the PS2, with natural leg movement.
- Glowing weapons, crystals, eyes and spell particles shine as brightly as on the console again.
- Falling from Sandstorm's boon no longer hangs in midair behind a loading screen.
- Master Class rank upgrades can be bought again.
- On servers that offer them: group search (LFG), stack auctions and mail to several people at once.

## 2026-10-31 · Enhanced 0.2.0

- Mods: EQOA Enhanced can now run FrontiersForge UI mods natively, no emulator needed. The game creates the mods folder automatically if missing. Turn mods on in Options, Extended, Mods & Addons, Addon Manager. Mods are off until you turn them on.
- Included: Wereoxxs UI is now bundled directly in the release ZIP inside the mods folder (turn it on in the Addon Manager). It brings customizable ability bars with the game's icons and cooldowns, casting bar, player, target and group frames, buffs, compass, chat and damage meter. Other Forge mods are experimental for now.
- Upgrading from 0.1.x: Players updating automatically get native mod support, but should download the 0.2.0 ZIP once to get the included Wereoxxs files (or copy the mods folder from the ZIP into their game folder).
- Mods can hide parts of the game's own HUD (ability bars, buffs, compass, chat window, target, group, pet, health, power and experience bars), and everything comes back when you turn the mod off.
- Press Enter to type in a mod's chat box; game pop-ups and menus still get Enter first.
- Mod window positions, sizes and settings are kept between sessions.
- If a mod has a problem, the Addon Manager shows it and mods/addons_error.log records it.

## 2026-10-31 · Enhanced 0.1.2

- NPC search works even if map-entities.json is missing or the game is started from inside the ZIP.
- Minimap search now preserves continuous autorun and follow movement while the search query panel is open.
- The program uses the selected EQOA icon on the window, taskbar and eqoa.exe:
  DEV in development builds, STANDARD in official releases. It no longer changes
  to the memory-card save icon after the disc is chosen.

## 2026-10-28 · Enhanced 0.1.1

- The circular minimap and full world map keep their position, dimensions, backdrop coverage, and UI alignment when Render Scale changes (2x and 4x).

## 2026-10-28 · Enhanced 0.1.0

- Download one Windows ZIP, extract and double-click eqoa.exe. No build tools or BAT required.
- Automatic updates and update notes now come from Enhanced versioned releases.
- Preserve upstream's GM Goto fix: find the floor again after destination zones load.
- Retain the enhanced map, guidance, quests, HUD options and disabled upstream report submission.

## 2026-10-28 · Upstream integration and fork hygiene

- Integrate upstream fishing, GM tools, character shadows and housekeeping while preserving enhanced features.
- World keeps Shadows; Map Markers moves to Map & Quests. Saved setting names remain unchanged.
- Enhanced option IDs use a separate transient range, avoiding future upstream collisions.
- Preview archives include the upstream MIT notice.
- Report button, F3 submission and automatic uploads are disabled. Local crash diagnostics remain available.
- Companion launch uses mod.json and EQOA_PARSER_DIR without machine-specific paths or a Windows console window.
- Mods, telemetry and HUD-number options use fork-owned adapters to reduce upstream conflicts.
- Empty animation clips no longer enter frame division/modulo; valid animation timing follows upstream.

## 2026-10-28 · Minimap hover and compact search

- Hover NPC/player dots for name, level when known, kind and distance.
- The large minimap Search button is now a small magnifier. Mouse-wheel zoom
  remains, and guidance details appear while hovering.
- NPC search says Spawned nearby when a living match is currently observed.
  Not detected means out of range or not spawned; the client cannot tell which.
  Guidance follows a live match first and falls back to recorded locations.
- Returning focus with no controller no longer reports a controller disconnect.

## 2026-10-28 · Map text clarity

- Map names, XP labels and marker tooltips now retain the font's full detail when
  enlarged. Their size, wrapping and layout stay the same.

## 2026-10-28 · Map search and guidance preview

- The minimap is now circular. Search stays below the map coordinates and status messages.
- Set a personal waypoint and choose Track waypoint to follow its pink guide on both maps.
- Map labels and XP camp badges share the compact theme; zoomed-out camps use small neutral markers.

- The native map now carries the compact dark UI treatment with thin borders, red focus and selection, and Space Mono text. Mouse/keyboard Suggest keeps the map visible while naming and annotating a community marker.

- Find NPCs, group members and camp/coach locations in a temporary map panel. F2 opens search; F4 stops guidance. Both can be changed in Controls.
- Follow recorded spawn locations on the minimap while actual nearby matches highlight separately. Group guidance follows valid same-world positions.
- After changing keys in Controls, the map's keyboard legend now shows the keys
  you chose, including zoom, waypoints, markers, towns, and quest markers.
- A control with no keyboard key assigned is shown as unbound.
