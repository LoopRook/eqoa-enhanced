# Bundled mods as examples

The client ships these mods in `bundled-mods/`. Read the source of the one closest to what you want.
API names below are the ones each `main.lua` calls (see the spec for signatures).

## Native `eqoa/1` mods

| Mod | What it demonstrates | API areas it uses |
|---|---|---|
| `eqoa_example_hearts` | The smallest complete mod: ten hearts for health, a settings save, clean release of a claim. Start here. | `player`, `hud.hide`, `events` (`render`, `disabling`, `world.leaving`, `state.reset`), `settings`, `imgui` draw lists |
| `eqoa_hud_numbers` | Text drawn over the game's own bars, with an on/off option and a movable layout. | `player`, `target`, `hud.text`, `hud.visible`, `hud.layout`, `options.toggle/get` |
| `eqoa_mana_bars` | A gauge under the target's health bar. | `target`, `group`, `hud.gauge`, `hud.layout`, `options` |
| `eqoa_target_of_target` | A target-of-target panel you can click to assist (a game action). | `target`, `entities.find`, `hud.image/gauge/text/frame`, `actions.assist`, `imgui` |
| `eqoa_stats_box` | A stats panel with a key bind; a click opens the game's Summary window. | `player.stats`, `hud.panel/text`, `input.bind`, `text.localized`, `windows.open`, `actions.toggle_stats_box` (reserved for this mod) |
| `eqoa_round_minimap` | A round minimap replacing the square one, with hover names and wheel zoom. | `map.view/get/markers`, `hud.hide/image`, `input.pointer`, `input.claim_pointer`, `input.minimap_wheel`, `entities`, `group`, `fonts` |
| `eqoa_map_search` | Map search with guidance to a creature or place; shares data with another mod. | `map`, `entities`, `input.bind`, `input.capture_text`, `files.load_json`, `mods.send`, `fonts`, `binds.get` |
| `eqoa_creature_journal` | A background data file: records discoveries, imports/exports, shares its file read-only. Declares `large_data`. | `entities.list`, `files.load_json`, `files.save_json_async`, `files.import/export`, `options.button`, `mods.send`, manifest `shares`, `commands`, `permissions` |
| `eqoa_bag_viewer` | A bags window: icons, tabs, a click menu of item actions, a loot log. Off by default. | `inventory`, `bank`, `loot`, `merchant`, `items.can_use`, `icons`, `actions.*` and `action.result`, `input.bind/claim/claim_pointer`, `hud.frame`, `files` |

## Forge-style packages (run unchanged on this client)

`arpg_ui`, `avoids_modern_ui` and `wereoxxs_ui` are FrontiersForge/UiForge packages, kept as they came
(with small noted changes, see `bundled-mods/SOURCE.txt`). They show the Forge compatibility layer, not
the `eqoa` API. Their own `README.md` files describe what they draw. Do not copy their layout for a new
mod; use the native examples above.

## Which one for what

- First mod, health or text: `eqoa_example_hearts`, then `eqoa_hud_numbers`.
- Sit beside the game's HUD: `eqoa_mana_bars`, `eqoa_target_of_target`.
- Your own window with clicks: `eqoa_bag_viewer`.
- Saved data and sharing between mods: `eqoa_creature_journal`, `eqoa_map_search`.
- Game actions and their results: `eqoa_bag_viewer` (many), `eqoa_target_of_target` (one).
