# Modern UI Mods

Author - Avoids

A pack of ten UI replacements for EverQuest Online Adventures, running under UiForge on
top of the FrontiersForge API. Each mod has its own Enable checkbox and settings panel,
all under this one script's Settings tab.

Enable `avoids_modern_ui_mods.lua` in the UiForge script list, then open Settings and
expand a mod to turn it on.

## The mods

| Mod | Replaces |
|---|---|
| Health Bar | the native health bar |
| Mana Bar | the native power bar |
| Experience Bars | both native exp bars |
| Ability Bar | the native hotbar, with icons and cooldowns |
| Chat Log | the native chat window, with channel colors and its own input |
| Group Frames | the native group display |
| Pet Frame | the native pet panel |
| Quest Log | the native quest log, plus your own per quest notes |
| Effects | the native buff/debuff display |
| Target Level | adds a level to the target nameplate, no native panel |

Turning a mod off restores whatever native panel it was hiding, immediately.

## Layout

```
avoids_modern_ui_mods.lua   the dispatcher, and the only file UiForge loads
modules/
  avoids_<mod>.lua          one file per mod, returning its mod table
  avoids_persist.lua        settings persistence, shared
  avoids_common.lua         helpers shared by more than one mod
resources/                  data files the pack writes, see below
```

The dispatcher holds the mod list and the master lifecycle. Every mod is a module
returning `{ display_name, Update, Settings, OnDisable, Save?, Load? }`, and the
dispatcher is the only thing that calls into them, so adding or removing a mod means
touching `MOD_ORDER` and the `ModPack` table and nothing else.

Module names carry an `avoids_` prefix because requires are bare. A plain `chat_log` could
collide with a same named module belonging to another script.

## Settings

Settings live in the UiForge profile, like every other script in this project. **File then
Save Profile** commits them, **File then Select Profile** restores them. Nothing here
writes settings to disk on its own, so a change you never save into a profile is lost when
the game closes.

Which mods are enabled is saved the same way, alongside them.

Earlier builds kept a `.cfg` file per mod under `scripts\resources`. Those are read once so
an existing setup carries over, then deleted. That migration is marked in the code for
removal in a later release, so anyone upgrading should launch once and save a profile.

## resources/

Free-form user data that does not fit a profile. Only one file lives here.

- `quest_log_notes.cfg` - your per quest notes, one line per note as `name|note text`.
  Written as you type rather than on profile save. Earlier builds kept it directly under
  `scripts\resources`, and it moves here automatically the first time Quest Log loads.

The directory ships with the pack because scripts cannot create one. Deleting it stops
notes from saving.

## Working on it

`avoids_modern_ui_mods.lua` re-executes every frame, which is how UiForge drives a script.
The files under `modules/` do not. `require` caches them, so a module body runs once and
only the `Update` function it returns runs per frame. **Editing a file under `modules/`
needs a full reload to take effect**, since the cached copy is what keeps running.

Each mod keeps its state in a global table behind the `x = x or {}` idiom. That is what
carries settings across a script reload.

Two rules worth knowing before changing lifecycle code, both learned from bugs:

- Anything released in `OnDisable` must be re-acquired on a path that runs again
  afterwards. A one time init guard is only safe if teardown also resets it, otherwise the
  mod comes back silently dead. `AvoidsCommon.ReleaseNativeToggle` exists to keep a panel
  and its tracking flag falling together.
- Teardown restores native UI but must not clear the setting that asked for it, or the
  preference will not survive a disable and re-enable.
