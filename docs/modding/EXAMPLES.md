# Bundled mods as examples

The client ships these mods in `bundled-mods/`. Read the source of the one closest to what you want.
API names below are the ones each `main.lua` calls (see the spec for signatures).

## Native `eqoa/1` mods

| Mod | What it demonstrates | API areas it uses |
|---|---|---|
| `eqoa_example_hearts` | The smallest complete mod: ten hearts for health, a settings save, clean release of a claim. Start here. | `player`, `hud.hide`, `events` (`render`, `disabling`, `world.leaving`, `state.reset`), `settings`, `imgui` draw lists |
| `eqoa_hud_numbers` | Text drawn over the game's own bars, with an on/off option and a movable layout. | `player`, `target`, `hud.text`, `hud.visible`, `hud.layout`, `options.toggle/get` |
| `eqoa_mana_bars` | A gauge under the target's health bar. | `target`, `hud.gauge`, `hud.layout`, `hud.visible`, `options` |
| `eqoa_target_of_target` | A target-of-target panel you can click to assist: the smallest example of a game action and its declared capability. | `target`, `entities.find`, `hud.image/gauge/text/frame`, `actions.assist`, `imgui` |
| `eqoa_stats_box` | A stats panel with a key bind; a click opens the game's Summary window. | `player.stats`, `hud.panel/text`, `input.bind`, `text.localized`, `windows.open`, `actions.toggle_stats_box` (reserved for this mod) |
| `eqoa_round_minimap` | A round minimap replacing the square one, with hover names and wheel zoom. | `map.view/get/markers`, `hud.hide/image/layout`, `input.pointer`, `input.claim_pointer`, `input.minimap_wheel`, `entities.pick_dot/find`, `group`, `fonts` |
| `eqoa_map_search` | Map search with a guide line to a creature or place; a text box of its own; reads another mod's shared file. | `map.get/markers/set_guide/clear_guide/open`, `entities.list/named`, `input.bind`, `input.capture_text`, `input.claim_pointer`, `files.load_json` (an `@mod/` path), `mods.send`, `fonts`, `binds.get`; manifest `commands` |
| `eqoa_creature_journal` | A background data file: records discoveries, imports/exports, shares its file read-only. Declares `large_data`. | `entities.list`, `files.load_json`, `files.save_json_async`, `files.import/export`, `options.button`, `mods.send`, manifest `shares`, `commands`, `permissions` |
| `eqoa_bag_viewer` | A bags window: icons, tabs, a click menu of item actions, a loot log; selling junk, restocking and banking marked items at a merchant or banker (by a button or once on approach), taking loot and banking without the game's windows. Off by default. | `inventory`, `bank`, `loot`, `merchant`, `items.can_use`, `merchant.price`, `icons`, many `actions` (`use_item`, `equip`, `sell`, `buy`, `loot`, `bank_move`, `open_loot`, `talk`, `batch`) with `action.result`, `windows.takeover` (scope `own`), `input.bind/claim/claim_pointer`, `hud.frame`, `files` |
| `eqoa_frames` | A demo of frame data: your effects with the time left (click one to take it off), a cast bar for you and your target, the target's title and consider colour, and buttons to invite it, send your pet in and call it off. Off by default. | `effects.buffs/pet`, `casts.get`, `target.get/info`, `player.get`, `actions.remove_effect/group_invite/pet_attack/pet_back_off`, `imgui` windows |
| `eqoa_spellbook` | A spellbook window on its own key (Shift+K): your abilities, the 20 memorized slots, cast and (un)memorize. A window session that the client ends for you (zoning, death, the player's switch). Off by default. | `abilities.list`, `cooldowns.get`, `actions.cast/assign_ability/unmemorize_ability/unmemorize_all`, `windows.open_session/close_session`, `input.bind/claim/pointer/claim_pointer`, `imgui` |
| `eqoa_crafting` | A crafting window on its own key (Shift+R): your six tradeskills, searchable recipes with how many of each ingredient you hold, Fill tray and one Combine per click, an Experiment tab, and the server's keep-this-recipe question answered by the player's click. Off by default. | `craft.get`, `actions.craft_list/craft_inspect/craft_tray_add/craft_tray_remove/craft_tray_clear/craft_fill/craft_combine/craft_keep`, `windows.open_session/close_session` (`recipe_book`, `experiment`), `input.bind/claim/pointer/claim_pointer/capture_text`, `imgui` |
| `eqoa_quest_journal` | A quest journal window on its own key (Shift+J): your quests and their goals, and a Drop quest button that asks the player first (`confirm.closed`). Off by default. | `quests.list/goals`, `actions.drop_quest`, `windows.open_session/close_session`, `input.bind/claim/pointer/claim_pointer`, `imgui` |
| `eqoa_social` | A social window on Shift+F (Friends) and Shift+G (Guild): Friends, Ignore, Guild roster/MOTD, Who, Group Search, plus incoming Friend Request and Guild Invitation prompts with 45-second timeout and client-drawn confirms for kick and accept. Off by default. | `social`, `guild`, `who`, `lfg`, `actions` (`friend_*`, `ignore_*`, `guild_*`, `group_invite`, `command`), `windows.open_session/close_session/takeover/pass`, `input.bind/claim/capture_text`, `imgui` |
| `eqoa_character` | A Character window on its own key (Shift+C): stats, resists and training points with a spend control per stat, Class Masteries (browse, buy, refund), factions and the combat record. Spending a point and buying or refunding a skill always ask the player first (the client's own question). A window session that the client ends for you, and a Class Mastery session held while the Masteries tab is up. Off by default. | `player.get/stats/training`, `mastery.get/skills/selected`, `factions.list`, `combat.record`, `actions.train_stats/mastery_browse/mastery_buy/mastery_refund/request_factions/request_combat_record`, `windows.open_session/close_session`, `input.bind/claim/pointer/claim_pointer`, `imgui` |

## Forge-style packages (run unchanged on this client)

`arpg_ui`, `avoids_modern_ui` and `wereoxxs_ui` are FrontiersForge/UiForge packages, kept as they came
(with small noted changes, see `bundled-mods/SOURCE.txt`). They show the Forge compatibility layer, not
the `eqoa` API. Their own `README.md` files describe what they draw. Do not copy their layout for a new
mod; use the native examples above.

## Which one for what

- First mod, health or text: `eqoa_example_hearts`, then `eqoa_hud_numbers`.
- Sit beside the game's HUD: `eqoa_mana_bars`, `eqoa_target_of_target`.
- Your own window with clicks: `eqoa_bag_viewer`; a window on its own key with a session: `eqoa_spellbook`, `eqoa_quest_journal`, `eqoa_character` (which also asks the player first before it spends anything).
- A step that must ask the player first (destroy, drop quest): `eqoa_quest_journal`.
- A server's reply read for a mod window, one request at a time, a question answered by a click: `eqoa_crafting`.
- Saved data and sharing between mods: `eqoa_creature_journal`, `eqoa_map_search`.
- Game actions and their results: `eqoa_target_of_target` (one), `eqoa_bag_viewer` (many, with batches).
- Timers, cast bars and unit-frame data: `eqoa_frames`.
- Working at a merchant, banker or corpse without the game's window: `eqoa_bag_viewer`.
