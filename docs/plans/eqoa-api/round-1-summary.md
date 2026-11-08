# Round 1: the mod API design

The proposed **eqoa** API gives mod authors dependable access to what your client actually knows: your character, nearby creatures, target, group, abilities, buffs, chat and combat. Mods can build replacement interfaces, bags, loot logs, merchant tools, quest-title panels and map overlays. They receive notifications when things change. Missing information stays unknown; an unfinished feature produces a visible explanation in the Addon Manager and error log instead of a convincing zero or false success.

Wereoxxs UI, ARPG UI and Modern UI keep their existing drawing tools, settings, names and counting conventions. Their compatibility layer uses the same native information as new mods. Existing scripts keep running each frame; new authors can choose to register their notifications once. Improvements such as real buff lists, controller input and complete chat capture therefore benefit both kinds of mod. Class Mastery will show points available, points spent and the chosen experience allocation accurately.

Mods may act for you without a click: target, cast, equip, loot, buy, sell, bank, send chat, move, follow, operate game menus and control the camera. This includes full bots. Installing and enabling such a mod means trusting it with your character's actions, including purchases and item destruction. Disabling it stops its queued actions and held controls. Your physical movement takes priority; losing focus, zoning or disconnecting releases its movement. Actions still obey the normal game's cooldowns, collision, window rules, transaction locks and server permissions. A queued purchase is never reported as a completed purchase.

That freedom does not include access to another mod's private files, arbitrary internet connections or launching programs. Mods may read the shared fonts, icons and support files shipped with the client. Every write stays private to that mod; existing interfaces can keep saving notes and migrating old settings without changing shared files. Normal game commands use your existing game connection, but mods cannot submit bug reports for you. Drawing, memory use and action queues have limits so one mod cannot monopolize the client.

The client cannot reveal creatures the server never sent, exact unknown buff timers, complete quest objectives, or emulator memory. The Forge minimap's memory-based terrain renderer cannot carry over. Mods instead get calibrated native map images, world and zone information, and a way to place a destination on the native map. This supplies direction and distance, not obstacle-aware route planning.

The build order remains:

1. **Round 2:** character and combat information, notifications, drawing, media, settings, diagnostics and the shared Forge compatibility core; a small hearts example.
2. **Round 3:** bags, item details, bank, loot, merchants and gem helpers; a bag viewer and loot log.
3. **Round 4:** quests, camera, dialogs, map information, Class Mastery, remaining icons and game actions.
4. **Round 5:** audit every promise, test the existing interfaces in play, finish author guidance and freeze the first stable API after your approval.

No owner policy questions remain. The build lead must review the small additional native integration points needed for reliable notifications and actions. This round changes documentation only; nothing has been built or released.
