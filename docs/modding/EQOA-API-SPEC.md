# EQOA Enhanced mod API: `eqoa/1`

This is the reference for the native Lua mod API of EQOA Enhanced, **as built**: every module, function, record, event and error a mod can rely on. It describes API version `eqoa/1`, revision 1.

- **New here?** Read [GETTING-STARTED.md](GETTING-STARTED.md) first and copy [the template](template/).
- **Frozen.** `eqoa/1` is frozen with the Stable release that first carries rounds 4 to 7 of the API. From that release on it only grows: nothing described here is renamed, removed or changed in meaning. [VERSIONING.md](VERSIONING.md) says exactly what that covers.
- **The client is the authority.** [SUPPORT-TABLE.md](SUPPORT-TABLE.md) lists every name with its status. It is printed by the client itself (`eqoa mod-api`) and checked against the client by a test, so it cannot drift; a second test checks this page against it. Where this page and the table ever disagree, the table is right.
- **Feature-test, do not assume.** `eqoa.revision` rises when something is added. `eqoa.capabilities.has("eqoa.map.set_waypoint")` asks about one name (§2).
- A few numbers below are marked **design intent, not part of the promise**: they describe how the client is tuned today and may change. Section 8 has a short list marked **under review**. Everything else is the contract.

## 1. Conventions

**Reads are snapshots, changes are events, writes are queued actions.** A getter returns a read-only copy of the client's state at this frame. To notice a change, subscribe to its event (§3) instead of comparing getters every frame. To make the game do something, call an action (§8); it is carried out at the next game step and its outcome comes back as an event.

**Names.** The API is the global `eqoa`; `require("eqoa")` returns the same table and `require("eqoa.player")` a module of it. Functions are called with a dot and named in snake_case: `eqoa.player.get()`. Modules and records are read-only: assigning to one raises an error.

**Counting starts at 1.** Every position in the API (bars, slots, tabs, rows, list positions) is 1-based. IDs are not positions: entity ids, item and ability ids, world ids, icon hashes and the game's enum numbers are passed through unchanged.

**Units.** Positions are game units, x and z on the ground and y up; north is -z and west is -x. Headings are radians in [0, 2π), 0 = north, increasing toward west. Durations are seconds. `tick` counts 30 Hz game steps; `frame` counts drawn frames. Do not time anything by counting frames.

**Types used on this page**

| Type | Meaning |
|---|---|
| `int`, `uint`, `number`, `bool`, `string` | A whole number, a whole number ≥ 0, a finite number, a boolean, a string. A wrong type or a non-finite number is refused with `invalid_argument`. |
| `T?`, `T[]` | A value that may be nil; a list of T (1-based, no gaps; an empty list is `{}`). |
| `Id` | An item-definition or ability id as a decimal string (`"1234"`), because these are 64-bit and do not fit a Lua number exactly. |
| `Vec3`, `Vec2`, `Color` | `{x, y, z}`, `{x, y}`, `{r, g, b, a}` with components 0..1. Their capability paths are `eqoa.Vec3`, `eqoa.Vec2`, `eqoa.Color`. |
| `EntityRef`, `AbilityRef`, `ItemRef`, `QuestRef`, `WindowRef` | Opaque handles to one particular thing. Compare with `==`. They cannot be built by hand or saved, and they expire (below). |
| `Token`, `RequestId`, `BatchId`, `Texture`, `Font`, `Sound` | Opaque handles that belong to this mod: a subscription or claim, a queued action, a batch, a loaded resource. |
| `Error` | `{code:string, message:string, capability:string}` (capability path `eqoa.Error`): what went wrong, a sentence for a log, and the name of the function that refused. (`retry_after` is declared `unsupported`: no refusal in this client knows how long to wait.) |

**What a call returns.** Unless a row says otherwise:

- a query returns `value, nil`, or `nil, Error`;
- "there is nothing there" (no target, no such record, a closed window) is `nil, nil`, not an error;
- a field the client does not know is nil. Absent is never reported as 0, -1 or an empty string: zero health is a real value, unknown health is nil;
- a function that changes something returns `true, nil` or `nil, Error`;
- nothing in `eqoa` raises a Lua error for an ordinary failure. (Passing a wrong type where a table or function is required can raise in a few functions; ImGui misuse raises as ImGui does.)
- these take no arguments, and refuse one with `invalid_argument`: `eqoa.client.get`, `eqoa.client.display`, `eqoa.player.get`, `eqoa.player.stats`, `eqoa.target.get`, `eqoa.group.get`, `eqoa.entities.list`, `eqoa.abilities.list`, `eqoa.hotbar.list`, `eqoa.effects.list`, `eqoa.chat.get_channel`, `eqoa.combat.get_counters`, `eqoa.input.get`, `eqoa.fonts.default`, `eqoa.settings.load`, `eqoa.actions.assist`, `eqoa.actions.target_target`, `eqoa.actions.toggle_stats_box`. Other functions ignore arguments they do not take.

**Error codes**

| Code | Meaning |
|---|---|
| `invalid_argument`, `out_of_range` | The call was wrong: a bad type or value, or a number outside its range. |
| `not_in_world` | The function needs the character to be in the game world. |
| `unavailable` | The data or service is not there yet (not loaded, not sent by the server, switched off by the player). Try later. |
| `not_found` | The thing asked for does not exist (a file, an icon, an entity, a dropped result). |
| `stale_handle` | A handle that has expired or belongs to another mod. |
| `not_ready` | An action cannot be done in the game's present state (dead, a window is up, no valid target, not loaded). |
| `busy` | Something else holds what is needed: another request, another mod's claim, a full queue. |
| `rate_limited`, `quota`, `limit_exceeded` | A protective limit was reached. |
| `not_declared` | `mod.json` lacks the declaration the call needs: a `capabilities` list (§8), or for a full window takeover a matching `replaces` entry (§7). |
| `permission_denied` | The mod is not allowed: the capability is not in its list, the path is outside its area, or the operation is never open to mods (reports, passwords). |
| `requires_player` | The call works only inside the player's own key press, click or option row (§5). |
| `window_closed`, `wrong_window` | The window or session an action or query needs is not open. |
| `refused`, `cancelled`, `unknown` | An action's outcome: the server said no; it was cancelled; no answer came (§8). |
| `unsupported` | The name exists so that code can load, but does nothing in `eqoa/1` on this platform. |
| `not_yet` | Planned and not built (in `eqoa/1`: `eqoa.actions.move`, `look` and `buttons`), or, from `load_json`, a shared file whose owner is still starting. |
| `unknown_symbol` | A diagnostic only: the mod read a name that is not part of the API. |
| `io_error`, `failed` | A file could not be read or written; any other failure. |

More codes may be added; treat one you do not know as a failure.

**Unknown names never crash a mod.** Reading a name that is not part of a module or record gives nil and writes an `unknown_symbol` diagnostic (§2). Calling a name that is `unsupported` or `not_yet` returns `nil, Error` with that code and writes a diagnostic. So a typing mistake shows up in Mods & Addons instead of failing silently, and a mod written for a newer client degrades instead of breaking.

**Handles expire.** A handle names one thing in one world. All handles go stale when the world changes (zoning, respawn, logout, disconnect); an `EntityRef` also when its entity leaves or its slot is reused; an `ItemRef` when the stack is gone or its corpse or merchant session ends; every `QuestRef` when the journal changes; a `WindowRef` when its window closes or changes page. A record you already hold stays readable as a copy; using its expired handle returns `stale_handle`. After `state.reset` (§3), fetch everything again.

**Where functions work.** Functions that read the world or act in it need the character in the world and otherwise return `not_in_world`; this is the default for anything about the player, entities, items, windows and actions. Pure helpers (`eqoa.math`, `eqoa.capabilities`, `eqoa.settings`, `eqoa.files`, `eqoa.log`, `eqoa.gems`) work at any time. Rows say so where it is not obvious. `eqoa.client.get().in_world` tells which state you are in.

## 2. Packages, versions, capabilities and diagnostics

### The package and `mod.json`

A mod is a folder in the client's `mods` folder (beside the program) that holds a manifest named `mod.json` and its Lua files. The folder name is free; the mod is known by its `id`. Sub-folders `modules/` (Lua files for `require`) and `resources/` (images, fonts, sounds) are found automatically.

```json
{
  "schemaVersion": 1,
  "runtime": "lua",
  "api": "eqoa/1",
  "id": "my_first_mod",
  "name": "My First Mod",
  "entry": "main.lua",
  "entryMode": "once"
}
```

Every field the client reads:

| Field | Required | Value | Meaning |
|---|---|---|---|
| `schemaVersion` | yes | `1` | The manifest format. |
| `runtime` | yes | `"lua"` | |
| `api` | yes | `"eqoa/1"` | The API version the mod is written for. |
| `id` | yes | Letters, digits, `_`, `.`, `-` | The mod's identity: in settings, in other mods' `@id/` paths and `eqoa.mods.send`, and in logs. Keep it unique and never change it. |
| `entry` | yes | A relative path inside the folder | The Lua file to run. |
| `name` | no | string | The name shown to the player. Defaults to `id`. |
| `entryMode` | no | `"once"` or `"frame"` | How the entry is run (§3). Defaults to `"frame"`, which exists for Forge-style code; **new mods should set `"once"`**. |
| `enabled` | no | bool | Whether the mod starts enabled the first time the client sees it. Defaults to `false`. Once the player has chosen, the player's choice is used. |
| `capabilities` | no | list of strings | What the mod declares it uses. Informational for reading functions; **required for acting**: `eqoa.actions.<name>` or `eqoa.actions.*` (§8), `eqoa.windows.takeover` (§7), `eqoa.map.waypoint` (§7). `"large_data"` asks for the larger Lua memory (§5). Shown to the player. |
| `replaces` | no | list of strings (at most 32) | The game windows the mod replaces completely (window kinds, §7) and the HUD pieces it replaces (`hud.<piece>`). Needed for a scope `"all"` takeover; shown to the player. A name that is neither is an error. |
| `shares` | no | list of paths under `data/` (at most 32) | Private files other mods may read (§5). |
| `commands` | no | list of identifiers (at most 32) | Commands other mods may send (§5). |
| `permissions` | no | list of `"import"`, `"export"` | Asking the player for a file, writing one to the Desktop (§5). Shown to the player. |

A manifest with a missing or wrong required field, an unknown `entryMode`, or an invalid entry in one of the lists is rejected: the mod does not appear, and Mods & Addons shows why. Unknown extra fields are ignored. If a folder holds both `manifest.json` and `mod.json`, `manifest.json` is read.

A folder or a single `.lua` file without a manifest is loaded as a FrontiersForge package (§11), not as an `eqoa/1` mod.

### What the player is shown

Mods & Addons lists each mod with its name and id, its state (Disabled, Enabled, or Failed), its `capabilities` and `permissions`, and these lines, taken from the manifest alone:

| Line | Shown when |
|---|---|
| `Actions: can act on its own (...)` | The mod declares action capabilities. The list names them: `all actions`, or each action; chat reads `chat (ordinary lines)` and `command (slash lines)`; `eqoa.map.waypoint` reads `map waypoint`. Every Forge package shows `chat (ordinary lines), command (slash lines), weapon_set`. |
| `Replaces game windows: ...`, `Not replaceable in this version: ...`, `Replaces HUD: ...` | From `replaces` (§7). |
| `Can answer NPC dialogs` | The mod declares `eqoa.actions.dialog_answer` or `eqoa.actions.activate_row`. |
| `Text boxes: can type into and submit the game's open text boxes` | The mod declares `eqoa.actions.input_text`. |

**A mod that changes what it may do is asked about again.** The client remembers, for each mod the player enabled, its kind (native or Forge) and what it declared. If an update changes the kind or widens the declarations, the mod comes back **disabled** with the line "its permissions changed; enable it again to accept", and stays off until the player enables it again. A mod the player has switched off just shows its new lines. The mods that ship with the client, unmodified and in their shipped place, take up their own new declarations silently.

### Versions

`eqoa.api` is the string `"eqoa/1"`. `eqoa.revision` is a whole number, 1 today, raised whenever something is added. A change that would break a mod written against this page can only ship as `eqoa/2`, chosen per mod in `mod.json`, running beside `eqoa/1` mods. The full promise is in [VERSIONING.md](VERSIONING.md).

### Asking the client what it supports

| Function | Returns |
|---|---|
| `eqoa.capabilities.get(path:string)` | A `Capability` for that exact dotted name. An unknown name gives a record with `status = "unknown"`. |
| `eqoa.capabilities.has(path:string)` | `true` only if the status is `supported`. |
| `eqoa.capabilities.list(prefix:string="")` | Every `Capability` whose path is `prefix` or below it, sorted by path. |

`Capability = {path:string, status:string, revision:uint, round:uint?, reason:string?, children:string[]}`

- `status` is `"supported"`, `"partial"` (a module whose members differ), `"not_yet"`, `"unsupported"` or `"unknown"`.
- `revision` is 1 for a name that is supported or partial and 0 otherwise.
- `round` is the development round the name came from (§12); it is history, not something to test. Test `status`, or `eqoa.revision`.
- `reason` says why a name is not supported. `children` lists the names directly below.

Paths are the names on this page: a module (`eqoa.player`), a function (`eqoa.player.get`), a field of a record (`eqoa.player.Player.level`), an event (`eqoa.events.cooldown.started`) and its fields (`eqoa.events.cooldown.started.data.lockout`), an ImGui function (`ImGui.Begin`, also `eqoa.imgui.Begin`), a Forge name (`frontiers_forge.player.GetLevel`).

Asking is free: it never writes a diagnostic. `supported` means the name works; it does not mean there is data right now (no target, a closed window and an unloaded icon still give their ordinary empty answers).

### Diagnostics: where problems show

When a mod reads an unknown name, calls something unsupported, passes a bad argument, fails in a handler or is refused a permission, the client records a diagnostic, also when the mod's own code caught the failure with `pcall`. Quiet outcomes are not diagnosed: `not_in_world`, `unavailable`, `not_found`, `busy` and `stale_handle` are ordinary answers.

- **In the game:** Mods & Addons shows `[Warnings: N]` beside the mod and the messages at the bottom of the window. A mod whose code failed outright shows "Failed — reload to retry".
- **On disk:** `addons_error.log` in the `mods` folder holds the same messages. For an API diagnostic the line is a small JSON object with the mod's id, the name (`symbol`), the error code, the message and how often it happened.
- The game's own log, `eqoa.log`, holds what the mod wrote with `eqoa.log.write`.

Repeats of the same diagnostic (same name, same code) are counted, not repeated. Design intent, not part of the promise: a repeat is written again at most once a minute; a mod may raise a limited number of distinct diagnostics (256, and at most 16 new ones a second after a burst of 32) before the rest are counted in one overflow line; the window keeps the latest 128; the log file is rotated at 5 MiB.

## 3. Running, events and the life of a mod

### How the entry runs

- **`"entryMode": "once"`** The entry file runs once each time the mod is enabled. It registers handlers and returns. Drawing happens in a `render` handler. This is the mode for new mods.
- **`"entryMode": "frame"`** (the default) The entry file runs every drawn frame and may draw directly; there is no `render` event, and subscribing to it is refused with `invalid_argument`. All other events work. Code in this mode must guard anything it wants to do only once.

Life cycle:

1. **Enable.** The entry runs. On the mod's first enable in a session its saved settings are loaded (`on_load`). Then `enabled` fires; if the character is already in the world, `world.entered` and `state.reset` follow with `initial = true`. Nothing from before (chat, combat) is replayed.
2. **Each client frame.** Events gathered since the last frame are delivered in the order they happened, then (in once mode) `render`.
3. **Leaving a world** (zoning, respawn, disconnect). `world.leaving` fires; all handles go stale; queued actions are cancelled; HUD and key claims are released. Subscriptions stay, so the mod carries on in the next world: `world.entered`, then `state.reset`.
4. **Disable.** `disabling` fires, the settings are saved (`on_save`), then everything the mod holds is dropped: subscriptions, timers, claims, queued actions, option rows, sounds. Lua globals survive a disable: on the next enable the entry runs again with the old globals and no subscriptions, so initialise deliberately (`state = state or {}`).
5. **Reload** (the button in Mods & Addons) starts the mod in a fresh Lua state: globals are gone, saved settings return through `on_load`.

A mod that raises an error in its entry, runs out of memory or exceeds its instruction budget is stopped and marked Failed until it is reloaded.

### Subscribing

| Function | Returns |
|---|---|
| `eqoa.events.on(name:string, callback:function)` | A `Token`. `callback(event)` runs for each such event. An unknown event name is refused (`unknown_symbol`). |
| `eqoa.events.once(name:string, callback:function)` | The same, for one event only. |
| `eqoa.events.off(token:Token)` | `true` if removed, `false` if it was already gone; `stale_handle` for anything that is not this mod's token. |
| `eqoa.events.after(seconds:number, callback:function)` | A `Token` for a one-shot timer. The callback gets a `timer` event at the first delivery after the time has passed. `off` cancels it. |

A mod may hold 128 subscriptions and timers at a time (`quota`; design intent, not part of the promise). Handlers for one event run in the order they were registered. Removing a subscription inside a handler takes effect at once; one added inside a handler starts with the next event.

If a handler raises an error, that subscription is removed, the error is reported as a diagnostic, and the mod keeps running.

Every event is an `Event = {name:string, seq:uint, time:number, tick:uint, frame:uint, generation:uint, data:table}`: `seq` rises with every event (a mod does not see every number), `time` is seconds since the client started, taken when the thing happened, `tick` and `frame` are as in §1, `generation` is the world generation (`Client.generation`). `data` is described per event below. Ignore fields you do not know: more may be added.

### The events

"Changed" events fire once per change with `before` and `after` copies and `changed`, the sorted list of the record's fields that differ (for a list: `{"records"}`). They do not fire every frame.

| Event | `data` | When |
|---|---|---|
| `enabled`, `disabling` | `{}` | The mod was enabled; it is about to be disabled. |
| `render` | `{dt:number}` | Each drawn frame, in once mode: draw here. `dt` is seconds since the last one (0 the first time). |
| `settings.render` | `{dt:number}` | Each frame while the mod's Settings panel in Mods & Addons is open: draw the settings UI here. Both modes. |
| `timer` | `{token:Token}` | A timer from `eqoa.events.after` is due. |
| `world.leaving` | `{reason:string, world_id:int?}` | The world is being left; `reason` is `"zone"`, `"respawn"` or `"disconnect"`. |
| `world.entered` | `{world_id:int, initial:bool}` | The character is in a world and its state is loaded. |
| `state.reset` | `{in_world:bool, initial:bool}` | Everything the mod read before is out of date: fetch it again (or, with `in_world = false`, there is no world state). Fired after entering and leaving a world, and after `events.dropped`. |
| `events.dropped` | `{first_seq:uint?, last_seq:uint?, count:uint, streams:string[]}` | Some events were discarded; `streams` names what was lost and `count` how many. When the mod fell behind, `first_seq` and `last_seq` are the numbers of the first and last discarded event. When the client discarded changes before they became events (`streams` is `source.capture` or `source.generation`), they had no numbers and both are nil. Chat and combat lines that were dropped are gone; state can be read again. |
| `player.changed` | `{before:Player?, after:Player?, changed:string[]}` | Anything in the `Player` record changed. Movement is reported at most once per game step. |
| `player.stats_changed` | `{}` | The Character Summary numbers changed (§4). |
| `target.changed` | `{before:EntityRef?, after:EntityRef?}` | The player's target changed. |
| `entity.added`, `entity.changed`, `entity.removed` | `{ref:EntityRef, before:Entity?, after:Entity?, changed:string[]}` | An entity came into view (`after` only), changed, or left (`before` only: its last known state). |
| `group.changed` | `{before:Group?, after:Group?, changed:string[]}` | |
| `abilities.changed`, `hotbar.changed` | `{before, after, changed}` with lists of `Ability` / `Hotbar` | |
| `cooldown.started`, `cooldown.updated`, `cooldown.ended` | `{ability:AbilityRef?, ability_id:Id?, display_key:uint, lockout:number}` | An ability's lockout went from zero to a value, changed value, or returned to zero. |
| `effects.changed` | `{before:Effect[]?, after:Effect[]?, changed:{"records"}}` | |
| `chat.message` | a `ChatMessage` (§4) | A line arrived in chat. |
| `chat.history_scroll` | `{active:bool, steps:int}` | The player pressed the chat-history keys while the game's own chat window is hidden; a mod that draws chat scrolls by `steps`. |
| `combat.hit` | a `CombatHit` (§4) | |
| `input.key`, `input.pad`, `pointer.pressed`, `pointer.wheel` | §5 | |
| `inventory.changed`, `bank.changed`, `loot.changed`, `merchant.changed` | `{before:Container?, after:Container?, changed:string[], cause:string}` | §6 |
| `quests.changed`, `mastery.changed`, `camera.changed`, `windows.changed`, `map.changed` | `{before, after, changed}` | §7 |
| `guide.lost` | §7 | A map guide ended by itself. |
| `action.result`, `batch.progress`, `batch.result` | an `ActionResult` / `BatchResult` | §8 |
| `resource.ready`, `resource.failed` | `{kind:string, request?, handle?, value?, path?, error?}` | Something loaded or saved in the background finished. `kind` is `"texture"`, `"font"`, `"sound"`, `"json"`, `"json_saved"`, `"import"` or `"export"` (§5). `resource.ready` carries `handle` for a texture, font or sound (and the decoded value for `json`), `request` for the file kinds (`json`, `json_saved`, `import`, `export`), `value` only for `json` and `path` only for `import` and `export`. `resource.failed` carries `error`, whose `capability` is the function that made the request (`eqoa.textures.load`, `eqoa.textures.rgba`, `eqoa.fonts.load`, `eqoa.sound.load`, `eqoa.files.load_json`, ...). More kinds may be added. |
| `shared.changed` | `{mod:string, path:string}` | A file another mod shares was saved or removed (§5). |
| `mod.command` | `{from:string, command:string, args}` | Another mod sent a command (§5). |

Delivery limits, as built and tested: the client keeps up to 1,024 undelivered events (2 MiB) per mod and delivers up to 256 per frame; beyond that the oldest are discarded and `events.dropped` then `state.reset` tell the mod. All of a mod's Lua in one frame shares one budget of 4 million instructions; exceeding it stops the mod. A disabled mod receives nothing.

## 4. Core game state (round 2)

These functions read the client's own state. They need no capability. Unless a row says otherwise they return `nil, Error{code="not_in_world"}` outside the world and reject any argument they do not take with `invalid_argument`.

### Client, player, entities and target

| Function | Returns |
|---|---|
| `eqoa.client.get()` | `Client`. Always answers, also outside the world. |
| `eqoa.client.display()` | `DisplayMetrics = {width:uint, height:uint, render_scale:uint}`: the size in pixels of the surface mods draw on, and the player's render-scale setting (1, 2 or 4). `unavailable` before the drawing context exists. |
| `eqoa.player.get()` | `Player`. |
| `eqoa.player.stats()` | `Stats`: the Character Summary's numbers. `nil, nil` while the client does not know them yet. |
| `eqoa.player.experience_for_level(level:uint)` | The experience needed to reach `level` (1..59), by the game's own formula; nil for any other level. Works outside the world. To ask about the next level pass `player.level + 1`. |
| `eqoa.entities.list()` | `Entity[]`: everything the client currently sees, in its own order, with no fixed limit. Empty means nothing seen, not nothing there. |
| `eqoa.entities.get(ref:EntityRef)` | That `Entity` now, or `stale_handle`. |
| `eqoa.entities.find(id:uint)` | The `Entity` with that id, or `nil, nil`. |
| `eqoa.entities.named(name:string, exact:bool=true)` | `Entity[]` whose name matches, ignoring case; with `exact = false`, names that contain `name`. Plain text, no patterns. |
| `eqoa.entities.dots()` | `{ids:uint[], xs:number[], zs:number[]}`: the creatures the game's minimap draws as dots this frame (living, named creatures, and the target), as three parallel arrays so that a per-frame scan builds no record per creature. Read it each frame; do not keep it. |
| `eqoa.entities.pick_dot(query)` | The minimap dot nearest a pointer position: `{id, x, y, world_x, world_z}` or `nil, nil`. `query = {x, y, center_x, center_y, radius, scale_x, scale_y, reach, exclude?}`: the pointer and the circle of a mod's minimap picture in pixels relative to its origin, the pixel scale per 112-unit view, the hit radius, and ids to skip. `unavailable` until the minimap has been drawn; `invalid_argument` for a missing or non-finite number. |
| `eqoa.entities.distance(a:Vec3, b:Vec3)` | The straight-line distance in game units. A pure calculation; works outside the world. |
| `eqoa.target.get()` | The targeted `Entity`, or `nil, nil` with no target or when the client has no record of it (`Player.target_id` still gives the id). |

`Client = {in_world:bool, phase:string, generation:uint, tick:uint, frame:uint, time:number, world_id:int?}`. `phase` is `"pregame"`, `"loading"`, `"world"` or `"disconnected"`. `generation` goes up each time a world is entered or left (handles from an older generation are stale). `tick` counts 30 Hz game steps, `frame` drawn frames, `time` seconds since the client started.

`Player` fields:

| Fields | Meaning |
|---|---|
| `entity:EntityRef?`, `id:uint`, `name:string`, `level:uint` | The character. `entity` is nil while the client has no entity record of itself. |
| `class_id:int`, `class_name:string?`, `race_id:int`, `race_name:string?` | The game's class and race IDs and their names (nil for an ID the client does not know). |
| `experience:uint`, `experience_debt:uint` | Raw counters, not percentages. |
| `hp:Vital?`, `power:Vital?` | `Vital = {current:int, maximum:int, base:int}`. nil until the server has sent the character's stats. |
| `hp_fraction:number?` | The health bar's own value, 0..1 (nil without an entity record). |
| `attributes:Attributes?` | `{base:SevenStats, total:SevenStats}`, where `SevenStats = {strength, stamina, agility, dexterity, wisdom, intelligence, charisma}` (ints). |
| `armor:int?`, `resists:Resists?` | Armor, and `Resists = {base, poison, disease, fire, cold, lightning, arcane}`: `base` is the base resistance, the others the bonuses on top of it. |
| `position:Vec3?`, `velocity:Vec3?`, `speed:number?`, `horizontal_speed:number?`, `heading:number?` | Where the character is and how it moves: game units, units per second, radians. nil until the character exists in the world. |
| `target_id:uint?`, `pet_id:uint?`, `has_pet:bool` | The selected target's and the pet's ids. |
| `auto_attacking:bool`, `dead:bool`, `fighting:bool?` | `fighting` is the game's in-combat flag (the one that starts battle music); nil while unknown. |
| `location:Location?` | `{world_id:int, place_id:int?, compass_id:int?, name:string?, cell_x:int, cell_z:int}`: the world, the named place and the map cell. |
| `mastery:Mastery?` | Class Mastery counters (§7). |

`Stats` holds one `StatItem` per line of the Character Summary, by name: `strength`, `stamina`, `agility`, `dexterity`, `wisdom`, `intelligence`, `charisma`, `armor`, `hp_max` (also as `max_hp`), `power` (also as `max_power`; this is maximum power, not current), `fire`, `cold`, `lightning`, `arcane`, `poison`, `disease`. The same items are also given as `rows` (all sixteen in the window's order), `columns` (the window's two columns of eight) and `resists` (the six resistances by name).

`StatItem = {name, short, word, stat_id, current, base, min, max, without_min, without_max, change, effect_change, shift}`: `name` is the key above, `short` the window's label (`"Str"`, `"AC"`, `"HPMax"`, ...), `word` the game's text id for it and `stat_id` the game's stat number; `current` is the value now; `min`/`max` are the range the game shows and `without_min`/`without_max` that range without effects (all four nil for power); `base` is the value without effects; `change` (and `effect_change`, the same number) is how far effects moved it; `shift` is `"same"`, `"raised"` or `"lowered"`.

`player.stats_changed` fires (with empty data) whenever these numbers change, including changes that alter no `Player` field; read `eqoa.player.stats()` again when it does.

`Entity = {ref, id, name, kind, level, position, actor_position, world_id, hp_fraction, power, class_code, entity_type, is_creature, corpse, target_id, status_flags, poisoned, diseased, npc_flags, search_flags, disposition, disposition_name}`

| Fields | Meaning |
|---|---|
| `ref:EntityRef`, `id:uint`, `name:string`, `level:uint?` | `name` is empty for a record with no creature data. |
| `kind:string` | `"player"`, `"npc"`, `"pet"` or `"other"`. More values may be added. |
| `position:Vec3`, `actor_position:Vec3`, `world_id:int` | `position` is the last position the server sent; `actor_position` is where the client draws the creature now (it moves between updates). |
| `hp_fraction:number?` | Health 0..1. nil when the client has no measured value: a zero from the server is ambiguous (dead or out of range), so it is reported only for the current target. Do not conclude death from nil. |
| `power:number?` | Power 0..1 when the server sends it. |
| `class_code:uint?`, `entity_type:int`, `is_creature:bool`, `corpse:bool` | The game's raw kind codes (`class_code` is not the player class) and whether this is a corpse. |
| `target_id:uint?` | What it is targeting, when known. |
| `status_flags:uint?`, `poisoned:bool?`, `diseased:bool?` | Status bits; `eqoa.entities.status` names them: `nameplate_hidden` = 1, `poisoned` = 4, `diseased` = 8. |
| `npc_flags:uint?` | NPC service bits: 1 merchant, 2 banker, 8 smith, 128 ordinary NPC. nil without creature data. |
| `search_flags:uint?` | The game's raw search flags, nil when unknown. |
| `disposition:uint?`, `disposition_name:string?` | How it regards you: known only for the current target, after the server's reply, and cleared when the target changes. `disposition_name` is `"Hostile"` (0..1), `"Neutral"` (2..3) or `"Friendly"` (4..6). |

### Group

`eqoa.group.get()` returns `Group = {in_group:bool, self_leader:bool?, members:Member[], raid:RaidMember[], invites:Invite[]}`. Outside a group it is still a record, with `in_group = false`, no members and `self_leader` nil. `members` never includes the player.

- `Member = {entity:EntityRef?, id:uint, name:string, active:bool, hp_fraction:number?, power:number?, position:Vec3?, world_id:int?}`. A member the client does not see keeps its place in the list with `active = false`; its position and world are nil unless the server reports them. A member can be in another world: compare `world_id` before measuring a distance.
- `RaidMember = {id:uint, name:string, hp_fraction:number, power_fraction:number}`; `Invite = {id:uint, name:string}`.

### Abilities, hotbars, cooldowns and effects

| Function | Returns |
|---|---|
| `eqoa.abilities.list()` | `Ability[]`, every ability the character knows, ordered by id. |
| `eqoa.abilities.get(ref:AbilityRef)` | That `Ability`, or `stale_handle`. |
| `eqoa.abilities.find(id:Id)` | The `Ability` with that id (a decimal string, as in `Ability.id`), or `nil, nil`. |
| `eqoa.abilities.in_range(ref, from:Vec3, to:Vec3)` | `RangeResult = {in_range:bool, distance:number}`: distance against the ability's range (always in range for a self ability or one with no range). Geometry only: not line of sight, not a promise the cast works. |
| `eqoa.hotbar.list()` | `Hotbar[]`: the bars the player has turned on, in the game's order. |
| `eqoa.cooldowns.get(ref:AbilityRef)` | `Cooldown = {active:bool, lockout:number, started_at:number?, estimated_remaining:number?}`. `lockout` is the server's lockout in seconds; `active` is `lockout > 0`. The last two are the client's own estimate from when it saw the lockout start, nil when it did not see that. |
| `eqoa.effects.list()` | `Effect[]`, at most eight. `unavailable` until the server has sent the effect list for this world; an empty list after that means no effects. |

`Ability = {ref:AbilityRef, id:Id, name:string, description:string, display_key:int, spellbook_slot:uint?, usable:bool, level:int, range:number, cast_time:number, power_cost:int, icon_background:uint?, icon_foreground:uint?, scope:int, cooldown:number, equipment_mask:uint, lockout:number}`. Times are seconds, `range` game units. `spellbook_slot` is the 1-based memorized slot (nil when not memorized). `display_key` is the server's lookup key, which can change; use `id` or `ref` for identity. `scope` is an `eqoa.abilities.scope` value: `self` 0, `target` 1, `group` 2, `pet` 3, `corpse` 4, `unknown` 5. The icon fields are resource hashes for `eqoa.icons.resource`.

`Hotbar = {index:uint, kind:string, selected:bool, selected_slot:uint?, slots:HotbarSlot[]}`; `kind` is `"abilities"`, `"items"` or `"weapon_sets"`. `HotbarSlot = {index:uint, kind:string, ability:AbilityRef?, item:ItemRef?, weapon_set:uint?, ui_icon:uint?}`; `kind` is `"empty"`, `"ability"`, `"item"`, `"weapon_set"` or `"command"`. `index` values are 1-based. (`HotbarSlot.command` and `HotbarSlot.icon` are declared `unsupported`: the game's bars do not expose a command's text; use `ui_icon` or the ability's or item's own icon.)

`Effect = {index:uint, icon:uint?, name:string}`. `index` is the 1-based slot and changes when effects shift. The server sends no duration, caster or stack count.

### Chat and combat

| Function | Returns |
|---|---|
| `eqoa.chat.history(limit:uint=100, before_seq:uint?)` | `ChatMessage[]`, oldest first: the latest `limit` messages (0..1000, `invalid_argument` beyond), optionally only those before `before_seq`. The client keeps the last 1,000 messages of the current world. |
| `eqoa.chat.get_channel()` | The channel the player's chat line sends to: `"say"`, `"shout"`, `"group"`, `"guild"` or `"other"`. |
| `eqoa.chat.channel_name(channel:string)` | A display label: Say, Party, Guild, Shout, Tell, System; "Unknown Message Type" for anything else. Works outside the world. |
| `eqoa.combat.get_counters()` | `Counters = {received:uint, dropped:uint}`: combat events delivered to this mod and lost to a full queue, since it was enabled. Works outside the world. |

`ChatMessage = {seq:uint, time:number, text:string, color:Color, channel:string}`. `text` is the whole logical line, not wrapped. `channel` is `"group"`, `"system"` or `"unknown"` in this client (`unknown` covers say, shout, guild and tells, which the client's chat observer does not tell apart); more values may be added. (`channel_code`, `sender` and `outgoing` are declared `unsupported` and read as nil.) The `chat.message` event carries one `ChatMessage` per line as it arrives; a mod enabled later can read the retained history but gets events only for new lines.

`CombatHit = {attacker_id:uint, defender_id:uint, amount:uint, kind:"damage"|"heal", color, outgoing:bool, incoming:bool, from_pet:bool, to_pet:bool}`, carried by `combat.hit` for every hit the client is told about, including other people's. `amount` is the size; `kind` says which way. `color` is `{r, g, b}` as three integers 0..255 at positions 1..3 when the server sends a colour, else nil (black is a colour, not nil). The four flags compare the two ids with the player and the pet.

## 5. Drawing, the HUD, input and resources (rounds 2 to 4)

### The HUD

A mod can hide pieces of the game's HUD, ask where they are, and draw in the game's own HUD style.

| Function | Returns |
|---|---|
| `eqoa.hud.hide(element:string)` | A `Token` for this mod's claim to hide that piece. Only in the world (`not_in_world`); `invalid_argument` for an unknown element. |
| `eqoa.hud.show(token:Token)` | `true` if the claim was released, `false` if it was already gone; `stale_handle` for a value that is not this mod's token. It never releases another mod's claim. |
| `eqoa.hud.hidden_by(element:string)` | The ids of the mods hiding any part of that element, sorted. |
| `eqoa.hud.visible(element:string?)` | Whether the piece (or, with no argument, the HUD as a whole) is being drawn now, counting the player's hide-UI key and every mod's claims. `false` outside the world. |
| `eqoa.hud.frame(size?)` | `Frame`: how the game's 640 x 448 HUD frame maps onto the window. `size = {width, height}` asks about another window size. |
| `eqoa.hud.layout(size?)` | `Layout`: where the game's HUD pieces are now, including the player's Move HUD changes. |

Elements for `hide`, `show` and `hidden_by`: `health`, `power`, `experience_main`, `experience_secondary`, `compass`, `target`, `group_panel`, `group_markers`, `pet`, `effects`, `hotbar`, `chat`, `minimap`, and `all` (every piece except `minimap`, which is hidden only by name). Hiding the minimap also turns off its clicks.

A piece stays hidden while at least one enabled mod holds a claim on it. Claims end when the mod releases them, is disabled or fails, and when the world is left; a token from before is then answered with `false`. Hiding never changes the player's saved HUD options and never hides the game's dialogs, the full map or Mods & Addons.

`Frame = {width, height, offset_x, offset_y, scale_x, scale_y, side, top, window_width, window_height}`: `width` and `height` are 640 and 448; a frame coordinate maps to window pixels as `offset + coordinate * scale`; `side` and `top` are the margins of the 4:3 area.

`Layout = {player:PlayerLayout, target:TargetLayout, pet:SinglePanelLayout, group_members:GroupMemberLayout[], minimap:Rect}` with `PlayerLayout = {panel, health, power, exp}`, `TargetLayout = {panel, health, power}`, `SinglePanelLayout = {panel}`, `GroupMemberLayout = {index, panel, health, power}` (four entries), each part a `Rect = {x, y, width, height, frame_x, frame_y, frame_width, frame_height}`. `x`, `y`, `width`, `height` are window pixels (the same units as the pointer, below); the `frame_` fields are the same rectangle in the game's HUD frame, for use with the drawing calls below.

Drawing in the HUD's own style, in the game's HUD pass (under the game's windows). Each call queues one thing for this frame, so call them every frame from `render`. All four are for the world only (`not_in_world`), return `true`, and give `limit_exceeded` beyond 256 calls per mod per frame (tested).

| Function | Draws |
|---|---|
| `eqoa.hud.text(spec)` | Text in the game's own HUD font. `spec = {x, y, text, color?, scale?, align?, outline?, shadow?, anchor?, vertical?, clip_width?, target_fade?}`: position in HUD-frame coordinates; `color = {r, g, b, a?}` 0..1; `align`, `anchor` are `"left"`, `"center"` or `"right"`; `vertical` is `"top"`, `"center"` or `"bottom"`; `outline` and `shadow` add a black edge or drop shadow. Instead of `x`, `y`: `col` (1..2) and `row` (1..8) place the text in the Stats Box's grid. |
| `eqoa.hud.gauge(spec)` | One of the game's bar gauges. `spec = {x, y, art, fill, lit?, anchor?, vertical?}`: `art` is the game's gauge art number (7 health, 9 power), `fill` 0..1. |
| `eqoa.hud.image(spec)` | A piece of the game's UI art, or a texture of the mod's. Game art: `{ui_id:int, x, y, w?, h?, color?, anchor?, vertical?, target_fade?}`; `ui_id = "panel"` draws a window backing. A texture: `{texture:TextureInfo, x, y, width?, height?, shape?}` with `shape = "square"` (default) or `"circle"`, which cuts the picture to a circle (it must be square). |
| `eqoa.hud.panel(spec)` | The game's window backing and border: `{x, y, w, h, anchor?, vertical?}`. |

`target_fade = true` on text and images fades them with the game's target plate when the target changes.

### Options and keys shown to the player

| Function | Returns |
|---|---|
| `eqoa.options.toggle({id:string, label:string?, default:bool?})` | Adds an on/off row for this mod in the game's options and returns `Toggle = {id, label, default, value, item_id}`. `value` is the saved setting. |
| `eqoa.options.get(id:string, addon_id:string?)` | The saved value of this mod's switch, or `nil` if there is none. With `addon_id`, another mod's switch; it reads `false` while that mod is disabled. |
| `eqoa.options.set(id:string, value:bool)` | Saves the switch. `io_error` if it cannot be saved (the value is then unchanged). |
| `eqoa.options.button({id:string, label:string}, callback:function)` | Adds an action row for this mod in Mods & Addons; the player choosing it runs `callback` at the next safe moment, and counts as the player's own act (see "Files the player chooses"). Returns `Button = {id:string, label:string, item_id:int}` (`item_id` is the row's number in the client's menu). Calling it again with the same `id` relabels that row. Rows go when the mod is disabled. At most 32 per mod (`quota`; design intent). |
| `eqoa.input.bind({id:string, label:string?, default_key:string?}, callback:function?)` | Gives the mod a key the player can change on the game's Controls page, and (with `callback`) runs it on each press of that key. Returns `true`. See §10 for which presses reach it. `default_key` defaults to `"b"`. Calling it again with the same `id` replaces the earlier call's callback (a press runs one callback); a later call without a callback leaves the key with none. |
| `eqoa.binds.register(id:string, default_key:string, description:string)` | The same Controls row without a callback. Returns `true`. |
| `eqoa.binds.get(id:string)` | The key the row has now, as a key name (§10); `""` when it has none; `nil` when this mod registered no such row. |

### Raw input

| Function | Returns |
|---|---|
| `eqoa.input.get()` | `InputState = {focused:bool, pad:Pad?, text_capture:bool}`: whether the window has focus, the controller state (nil with no controller) and whether a mod's text editor has the keyboard. Always answers. |
| `eqoa.input.claim(key:string, suppress:bool=false)` | A `Token` for one key: `"Enter"`, `"Tab"`, `"Backspace"`, `"Escape"`, `"Space"` or a single printable character. With `suppress = true` the game does not see that key in plain play. `busy` if the key is already claimed. Claims end when the world is left. |
| `eqoa.input.release(token:Token)` | `true` if released, `false` if already gone. |
| `eqoa.input.capture_text(active:bool)` | Takes the keyboard for a text editor the mod draws: while it holds, plain-play keys do not reach the game and `input.key` also delivers repeats and a `text` field. Call it with `true` every frame the editor is open: the hold lapses about a second after the last call (design intent). Returns `true`. |
| `eqoa.input.rotate_dpad(active:bool)` | Turns the controller's D-pad a quarter turn (up becomes right) for layouts that need it. World only. |
| `eqoa.input.pointer()` | `Pointer = {x:number, y:number, inside_window:bool}`: the mouse in window pixels. |
| `eqoa.input.claim_pointer(rect)` | Keeps clicks and the wheel inside `rect = {x, y, width, height}` (window pixels) from the game for this frame and the next; call it every frame for each area. A `Rect` from `eqoa.hud.layout()` can be passed as it is. At most 32 areas per mod per frame (`quota`; design intent). Claims end when the pointer leaves the window or the window loses focus. The player's Move HUD drag goes first. |
| `eqoa.input.minimap_wheel(steps:number)` | Passes wheel steps (at most 20 either way) to the game's minimap zoom. |

`Pad = {buttons:string[], left:Vec2, right:Vec2, raw_left:RawAxes, raw_right:RawAxes}`: `buttons` lists the pressed buttons by name (`select`, `l3`, `r3`, `start`, `up`, `right`, `down`, `left`, `l2`, `r2`, `l1`, `r1`, `triangle`, `circle`, `cross`, `square`); the sticks are -1..1 with right and down positive; `RawAxes = {x, y}` are the game's own 0..255 values (128 at rest).

Input events:

- `input.key`: `{key:string, combo:string, down:bool, repeat:bool, synthetic:bool, text:string?}` for a key press or release in plain play. `key` is the key's name; `combo` is the full name of the press, with the modifiers held (§10); a release carries the names its press had. `text` is the character typed, only while this mod holds `capture_text`. Keys the game or ImGui has taken (a game window, the chat line, a focused ImGui text field) are not delivered.
- `input.pad`: `{before:Pad?, after:Pad?}` when the controller state changes.
- `pointer.pressed`: `{x, y, inside_window, button}` with `button` `"left"`, `"right"`, `"middle"` or `"other"`; `pointer.wheel`: `{x, y, inside_window, steps}`. Both go to every listening mod for input anywhere in the window, claimed or not. Touch is not reported.

### Drawing: ImGui is the API

Mods draw windows and shapes with [Dear ImGui](https://github.com/ocornut/imgui) through the global `ImGui` (also reachable as `eqoa.imgui`; the two are the same object). Drawing is legal only inside a `render` or `settings.render` handler (or, in frame mode, the entry). Positions and sizes are window pixels; `eqoa.client.display()` gives the size. The functions keep ImGui's own names, argument order, overloads, enum numbers and return order as FrontiersForge's Lua binding has them; `ImVec2.new(x, y)` and `ImVec4.new(x, y, z, w)` make vectors, and `eqoa.imgui.vec2(x, y)` and `eqoa.imgui.vec4(x, y, z, w)` are the same constructors.

The bridge is a fixed set of functions, not all of Dear ImGui. These are supported:

| Area | Names |
|---|---|
| Windows and layout | Begin, End, BeginChild, EndChild, BeginMenuBar, EndMenuBar, BeginMainMenuBar, EndMainMenuBar, BeginMenu, EndMenu, MenuItem, BeginTabBar, EndTabBar, BeginTabItem, EndTabItem, BeginGroup, EndGroup, Indent, Unindent, SameLine, Dummy, SetCursorPos, SetCursorScreenPos, GetCursorPos, GetCursorScreenPos, GetContentRegionAvail, GetWindowPos, SetWindowPos, SetNextWindowPos, SetNextWindowSize, GetScrollY, GetScrollMaxY, SetScrollY, SetScrollHereY, IsWindowFocused, IsWindowHovered, PushID, PopID, PushClipRect, PopClipRect, SetNextItemWidth |
| Text and widgets | Text, TextColored, TextDisabled, TextWrapped, TextUnformatted, BulletText, Separator, SetTooltip, CalcTextSize, Button, InvisibleButton, Selectable, Checkbox, RadioButton, SliderInt, SliderFloat, ColorEdit3, ColorEdit4, InputText, InputTextWithHint, InputInt, Combo, BeginCombo, EndCombo, BeginPopup, EndPopup, OpenPopup, TreeNode, TreePop, CollapsingHeader, ProgressBar, Image, PushTextWrapPos, PopTextWrapPos |
| Style and queries | PushStyleColor, PopStyleColor, PushFont, PopFont, SetWindowFontScale, GetColorU32, GetDisplaySize, GetTime, IsItemActive, IsItemHovered, IsMouseDragging, IsMouseHoveringRect, SetKeyboardFocusHere, GetFrameHeight, GetTextLineHeight, GetTextLineHeightWithSpacing, GetBackgroundDrawList, GetWindowDrawList |

These exist only as refusals (`unsupported`, with a diagnostic): BeginTooltip, EndTooltip, BeginItemTooltip, GetForegroundDrawList, PushStyleVar, PopStyleVar, PushItemWidth, PopItemWidth, SetNextWindowFocus, SetWindowFocus, and the mouse functions GetMousePos, GetMousePosOnOpeningCurrentPopup, GetMouseWheel, GetMouseDragDelta, ResetMouseDragDelta, IsMouseDown, IsMouseClicked, IsMouseReleased, IsMouseDoubleClicked, IsMousePosValid, SetMousePos and SetMouseCursor. For the mouse use `eqoa.input.pointer()` and the pointer events. Any other ImGui name is an unknown key (nil, with a diagnostic).

A draw list from `GetWindowDrawList()` or `GetBackgroundDrawList()` is valid for the current frame only. Its methods (called with `:`): `AddLine`, `AddRect`, `AddRectFilled`, `AddTriangle`, `AddTriangleFilled`, `AddCircle`, `AddCircleFilled`, `AddNgon`, `AddNgonFilled`, `AddText` (as `(pos, color, text)` or `(font, size, pos, color, text)`), `AddImage`, `AddImageRounded`, `PushClipRect`, `PopClipRect`. Colours are ImGui's packed numbers (`ImGui.GetColorU32`); textures are the handles from `eqoa.textures`, `eqoa.icons` and `eqoa.map.view`. Each method is in the capability registry and the support table as `ImDrawList.<Name>` (for example `eqoa.capabilities.has("ImDrawList.AddNgon")`).

Every `Begin...` needs its `End...`. An error inside a frame unwinds the open scopes for you, and is reported.

### Icons, text and small helpers

| Function | Returns |
|---|---|
| `eqoa.icons.resource(hash:uint, options:IconOptions?)` | `TextureInfo` for a game icon by its resource hash (`Item.icon`, `Ability.icon_foreground`, `Effect.icon`, ...). `not_found` if the game has not loaded that art. `IconOptions = {trim:bool?, trim_transparent:bool?, trim_color:Color?}` crops the border. World only. |
| `eqoa.icons.ui(id:uint)` | `TextureInfo` for a piece of the game's UI art by number. World only. |
| `eqoa.icons.info(hash:uint)` | `SurfaceInfo = {width:uint, height:uint}` of an icon (`source_format` is declared `unsupported`). |
| `eqoa.icons.class(class_id:int)` | The class's icon as a `TextureInfo` (`class_id` 0..14). |
| `eqoa.icons.disposition(value:uint)` | The icon the game uses for that `Entity.disposition`. |
| `eqoa.text.localized(id:int)` | A string from the game's text table; `nil, nil` for an id with no text; `unavailable` before the table is loaded. |
| `eqoa.math.bearing(from:Vec3, to:Vec3)` | The compass direction from one point to another, radians in [0, 2π), north 0, increasing toward west. |
| `eqoa.math.degrees(radians)`, `eqoa.math.radians(degrees)` | Conversions. |
| `eqoa.math.normalize(angle)`, `eqoa.math.signed_angle(a, b)` | An angle brought into [0, 2π); the shortest turn from `a` to `b`, in [-π, π). |
| `eqoa.math.facing(position:Vec3, heading:number, destination:Vec3, tolerance:number)` | Whether someone at `position` facing `heading` looks at `destination` within `tolerance` radians (0..π). |
| `eqoa.math.is_bit_zero(value:uint, mask:uint)` | `(value & mask) == 0`. The global `bit` library (`tobit`, `bnot`, `band`, `bor`, `bxor`, `lshift`, `rshift`, `arshift`) is also available. |

`TextureInfo = {texture:Texture, width:uint, height:uint}`. Pass `texture` to `ImGui.Image`, the draw list's image methods, or `eqoa.hud.image`.

### Textures, fonts and sound

Paths are relative and are looked up in the mod's own package, then the shared resources (§9).

| Function | Returns |
|---|---|
| `eqoa.textures.load(path:string)` | `TextureInfo` from a PNG file. Any failure (missing, not a PNG, too large) is `not_found`, with the reason as a diagnostic. JPEG is not decoded: `eqoa.textures.formats.jpeg` is `unsupported`, `eqoa.textures.formats.png` and `eqoa.textures.formats.rgba` are supported. |
| `eqoa.textures.rgba(bytes:string, width:uint, height:uint)` | `TextureInfo` from raw pixels: exactly `width * height * 4` bytes, each side 1..2048. |
| `eqoa.textures.release(texture:Texture?)` | `true`. Releasing nil or twice is fine. |
| `eqoa.fonts.load(path:string, size:number=16)` | `FontInfo = {font:Font, status:string, reason:string?}` for a TTF or OTF file at a pixel size up to 256. `status` is `"pending"` (it will be ready on a later frame), `"ready"`, or `"fallback"`: the file could not be used and `font` is the default font. Use it with `ImGui.PushFont`. `unavailable` before the drawing context exists. |
| `eqoa.fonts.default()` | The default font's `FontInfo`. |
| `eqoa.fonts.release(font:Font?)` | `true`. |
| `eqoa.sound.load(path:string)` | A `Sound` from a WAV or MP3 file; `failed` if it cannot be loaded. |
| `eqoa.sound.play(sound:Sound, {loop:bool=false, volume:number=1}?)` | `true` when playback started. |
| `eqoa.sound.stop(sound?)`, `eqoa.sound.set_volume(sound, volume)`, `eqoa.sound.is_playing(sound?)`, `eqoa.sound.release(sound?)` | `true`, `true`, a bool, `true`. Volume is 0..1. |

`eqoa.fonts.game` (the game's own bitmap font as an ImGui font) is `unsupported`; `eqoa.hud.text` draws with that font. Sound and fonts from files work on Windows, the published platform; elsewhere the functions answer `unsupported`. A mod's sounds stop when it is disabled.

Loading a texture, font or sound also fires `resource.ready` or `resource.failed` (§3) with `kind` `"texture"`, `"font"` or `"sound"`.

### Settings and the log

| Function | Returns |
|---|---|
| `eqoa.settings.load()` | The mod's saved settings value, or nil if none was saved. |
| `eqoa.settings.save(value)` | `true` once the value is safely on disk. |
| `eqoa.settings.on_save(callback)` | Registers the function the client calls to get the value to save (on Save in Mods & Addons and when the mod is disabled). One per mod; a second call replaces the first. |
| `eqoa.settings.on_load(callback)` | Registers the function the client calls with the saved value (when the mod is first enabled, and on Load or a profile switch in Mods & Addons). |
| `eqoa.log.write(level:string, message:string)` | Writes a line to the game's log (`eqoa.log`) as `addon <id>: [level] message`. `level` is `"debug"`, `"info"`, `"warn"` or `"error"`; `warn` and `error` also appear in Mods & Addons. Long messages are cut. `quota` if the mod logs faster than the client drains (64 lines; design intent). |

A settings value is nil, a boolean, a finite number, a string, or a table of these whose keys are either all strings or 1..n, nested at most 32 deep and at most 1 MiB when saved. No functions, handles, metatables or cycles. Settings are kept per mod and per player profile; the mod never names a file.

### Private files

`eqoa.files` reads and writes a small private area that belongs to the mod. Paths are relative, with `/`; see §9 for what the prefixes mean.

| Function | Returns |
|---|---|
| `eqoa.files.read(path:string)` | The file's bytes as a string. `not_found`, `permission_denied`, or `quota` for a file over 1 MiB. |
| `eqoa.files.write(path:string, bytes:string)` | `true`. The file is replaced whole or not at all. At most 1 MiB a file and 16 MiB for the mod in total, settings included (tested; `quota`). |
| `eqoa.files.remove(path:string)` | `true` if something was removed, `false` if there was nothing. |
| `eqoa.files.list(dir:string="", glob:string="*")` | `FileEntry[] = {path:string, kind:"file"\|"directory", size:uint?}` for the entries directly in `dir`, sorted by name; `glob` filters names with `*` and `?`. `quota` rather than a partial list when there are more than 1,024. |

#### Large JSON files

For data too big to handle in one frame, the client reads, parses and writes JSON off the frame thread.

| Function | Returns |
|---|---|
| `eqoa.files.load_json(path:string)` | A request number. The result arrives as `resource.ready {kind="json", request, value}` or `resource.failed {kind="json", request, error}`. `not_found` at once for a missing private file. At most two loads at a time per mod (`quota`). |
| `eqoa.files.save_json_async(path:string, value)` | A request number. The value is copied at the call and written later; the result is `resource.ready {kind="json_saved", request}` or `resource.failed`. A private path only. Saves to one path are merged: if you save again before the last one was written, only the newest is written and reported, so track the latest request. Saving to a second path while one is in flight is `quota`. |
| `eqoa.files.null` | The value that stands for JSON `null` in loaded data and in values to save. |

A loaded value is an ordinary, changeable Lua table; arrays (also empty ones) remember that they are arrays and are saved as arrays. A value to save follows the settings rules above, plus `eqoa.files.null`. A file may be up to 16 MiB, 32 levels deep and two million values to load, and 14 MiB to save (tested). A large value is handed to Lua over several frames; how it is sliced is design intent, not part of the promise.

An ordinary mod has 8 MiB of Lua memory. A mod that lists `"large_data"` in its `capabilities` gets 128 MiB (tested); without it a large load fails with `resource.failed` and a message that says so. The mod is not disabled.

#### Sharing a file with other mods

A mod may let others read some of its private files. In `mod.json`, `"shares": ["data/journal.json"]` lists them (paths under `data/`). Another mod reads one with `eqoa.files.load_json("@<mod id>/data/journal.json")`:

- `not_yet` (quietly) while the owner is enabled but has not started yet: try again shortly;
- `permission_denied` if the owner is missing or disabled, or the path is not in its `shares`.

`shared.changed {mod, path}` fires for every mod when a shared file is saved or removed. Sharing is read-only.

#### Messages between mods

A mod lists the commands it accepts in `mod.json`: `"commands": ["import_journal"]`. `eqoa.mods.send(mod_id:string, command:string, args?)` queues one; the receiver gets `mod.command {from, command, args}` at its next event delivery. `args` is plain JSON-like data, small (about 4 KiB, 8 levels, 256 values; `quota` beyond). `unavailable` if the target is missing or disabled; `invalid_argument` if it does not declare that command. Each sender may have 4 commands waiting per receiver (tested).

#### Files the player chooses

With `"permissions": ["import"]` or `["export"]` in `mod.json` a mod may ask the player for a file or put one on the player's Desktop. These two calls are the only ones that need **the player's own act**: they work only inside the handler of a key press, a pointer press, or an `eqoa.options.button` row (`requires_player` otherwise), and each such act allows one import or export. A handler may pass the act on once with `eqoa.mods.send`, so one mod's button can start another mod's import; it cannot be passed further, and timers and `render` never have it.

| Function | Returns |
|---|---|
| `eqoa.files.import({title:string?, extensions:string[]})` | A request number. The system's file picker opens (its title starts with the mod's name); the chosen file is copied into the mod's private area and reported as `resource.ready {kind="import", request, path}` with a path to pass to `load_json` or `read`. Cancelling gives `resource.failed` with code `cancelled`. The copies are staging: the client keeps the newest three and the mod cannot write there. |
| `eqoa.files.export(private_path:string, file_name:string)` | A request number. Copies one of the mod's private files to the player's Desktop as `file_name` (one plain name ending in `.json`, `.txt`, `.csv` or `.png`). It never replaces a file: a taken name gets ` (2)`, ` (3)`, ... The result is `resource.ready {kind="export", request, path}` with the name used. |

Both give `permission_denied` without the manifest permission and `busy` while the mod's previous import or export is still open.

## 6. Items (round 5)

An item **definition ID** (`Item.id`) names a kind of item. An **`ItemRef`** (`Item.ref`) names one particular stack in one particular list. A list position (`Item.index`) is display order only and changes when the list compacts. Keep the `ref` to follow a stack: it stays valid while that stack exists in that list, through compaction, and is stale once the stack is gone, the corpse or merchant session changes, or you zone.

Reading items needs no capability. Buying, selling, looting and moving items are actions (§8).

### Containers

| Function | Returns |
|---|---|
| `eqoa.inventory.get()` | The pack: a `Container`. Always available in the world. |
| `eqoa.bank.get()` | The bank: a `Container`. The contents arrive with the world, so they are readable away from a banker; `open` says whether the bank window (or a hidden bank session, §7) is up. `nil, nil` if the client holds no bank list. |
| `eqoa.loot.get()` | The corpse being looted: a `Container`, or `nil, nil` when none is open. Never a corpse from earlier. |
| `eqoa.merchant.get()` | The merchant being talked to: a `Container`, or `nil, nil` when none is open. |

All four return `nil, Error{code="not_in_world"}` outside the world and `invalid_argument` if given an argument. They also answer during a hidden session (§7 "Taking over a window"): what matters is the session with the server, not whether the game's window is drawn.

`Container = {kind, revision, open, items, capacity, used, remaining, full, money, view, merchant, source}`

| Field | Type | Meaning |
|---|---|---|
| `kind` | string | `"inventory"`, `"bank"`, `"loot"` or `"merchant"`. |
| `revision` | uint | Goes up when the container changes. |
| `open` | bool | Inventory: always true. Bank: its window or session is up. Loot and merchant: true (they exist only while open). |
| `items` | `Item[]` | The stacks, in the list's own order. For a merchant these are the wares, also while selling. |
| `capacity` | uint? | Inventory and bank: the game's limit, 40 unless the server's list is larger. Loot: 40. Merchant: nil. |
| `used` | uint | Number of stacks. |
| `remaining`, `full` | uint?, bool? | `capacity - used` (never below 0) and `used >= capacity`; nil when `capacity` is nil. |
| `money` | uint? | Tunar carried (inventory) or banked (bank); nil for loot and merchant. |
| `view` | `ContainerView?` | What the game's own window shows; see below. nil for bank and loot. |
| `merchant` | `MerchantState?` | Merchant only. |
| `source` | `LootSource?` | Loot only: `{entity_id:uint, name:string}`, the corpse the client asked to loot. It is given only when it cannot be wrong, so a missing `source` is normal (for example after two loot requests overlapped). |

`ContainerView = {tab, tabs, rows, selected, selected_row}`. For the pack it exists only while the start menu's item page is up: `tabs` is four `Tab = {index:uint, key:string, label:string?}` rows with keys `"usable"`, `"equipment"`, `"charged"`, `"misc"` (index 1 to 4), `tab` the index of the one showing, `rows` the `ItemRef`s under that tab in the page's order, `selected` the lit stack and `selected_row` its 1-based row (both nil when no stack is lit). For a merchant it always exists: `tabs` is empty, `tab` nil, and `rows` are the wares while buying, the pack's stacks while selling, and empty in the merchant's menu.

`MerchantState = {entity_id, mode, greeting, selected_option, options, button_hint, pending, sell_rate, buy_rate}`: the merchant's entity id; `mode` is `"menu"`, `"sell"` or `"buy"`; `greeting` its text; `options` is `MerchantOption[] = {key:"buy"|"sell"|"exit", label:string?, selected:bool}` and `selected_option` the lit key (menu mode only); `button_hint` the window's hint text in the game's `|glyph label` form (see `eqoa.windows.parse_hint`), nil until the game's words are loaded; `pending` true while a purchase or sale waits for the server; the two rates are the game's price multipliers (use `eqoa.merchant.price` for a price).

### Items

`Item` fields:

| Fields | Meaning |
|---|---|
| `ref:ItemRef`, `list:string`, `index:uint`, `inventory_key:uint` | The handle, the list it is in (`"inventory"`, `"bank"`, `"loot"`, `"merchant"`), its current 1-based position there, and the server's key for the stack (an identifier, not a position). |
| `id:Id`, `name:string`, `description:string`, `icon:uint?` | The definition ID (a decimal string, §1), texts, and the icon's resource hash for `eqoa.icons.resource` (nil if none). |
| `quantity:uint`, `charges:int`, `max_stack:uint`, `value:uint` | Stack size, charges, largest stack, and base value in tunar (not a merchant's price). |
| `equipped:bool`, `equipped_slot:uint?`, `quick_slot:uint?` | Whether it is worn, the 1-based equipment position, the 1-based quick slot; nil when not worn / not in a quick slot. |
| `wear_slot:int`, `slot_name:string?`, `equippable:bool` | Where it can be worn: an `eqoa.items.slots` value (an ID, not a position), its localized name, and whether it can be worn at all. |
| `level_required:int`, `class_mask:uint`, `race_mask:uint`, `allowed_classes:int[]`, `allowed_races:int[]` | Requirements. In the masks bit n means class or race ID n; the lists are those IDs, ascending. |
| `current_hp:int`, `maximum_hp:int`, `durability:int` | Item hit points and the definition's durability, as integers (not a percentage). |
| `damage:int`, `attack_type:int`, `attack_name:string?`, `range:number` | Weapon numbers; `attack_type` is an `eqoa.items.attack_types` value; `range` is in game units. |
| `no_trade:bool`, `no_rent:bool`, `lore:bool` | The item's flags. |
| `stats:ItemStat[]` | `{id:int, name:string?, value:int}` in the order received; `id` is an `eqoa.items.stats` value; penalties are negative; an unknown stat keeps its `id` and has a nil `name`. |
| `tabs:string[]` | The item-page tab keys (`"usable"`, `"equipment"`, `"charged"`, `"misc"`) this stack appears under. Available whether or not the page is open. |

| Function | Returns |
|---|---|
| `eqoa.items.get(ref:ItemRef)` | The current `Item`. `stale_handle` if the stack is gone or the handle is from another list session or world; `invalid_argument` if `ref` is not an `ItemRef`. |
| `eqoa.items.can_use(ref, class_id:int?, race_id:int?, level:int?)` | `CanUse = {allowed:bool, reasons:string[]}`. Omitted arguments default to the player's own class, race and level (`invalid_argument` if the client does not know them yet). `reasons` lists what fails, in the order `"class"`, `"race"`, `"level"`. It is the client's knowledge, not a promise that the server accepts the item. |
| `eqoa.items.stat(ref, stat_id:int)` | The value of the first stat with that ID, or 0 if the item has none. |
| `eqoa.items.slot_info(wear_slot:int)` | `SlotInfo = {id:int, name:string?, order:uint?}`: the slot's localized name and its place in the paperdoll order; both nil for an unknown ID. |
| `eqoa.items.attack_name(attack_type:int)`, `eqoa.items.stat_name(stat_id:int)` | The localized name, `nil, nil` for an unknown ID, `unavailable` before the game's words are loaded. |
| `eqoa.merchant.price(ref, direction:"buy"\|"sell", quantity:uint=1)` | Tunar for that many, by the game's own rounding: the unit price times the quantity. `"buy"` takes a ref from the merchant's wares, `"sell"` one from the pack (`invalid_argument` otherwise). `wrong_window` when no merchant is open. It does not check your money or the stack size; the action does. |

`eqoa.items.slots`, `eqoa.items.stats` and `eqoa.items.attack_types` are read-only tables of the game's IDs by snake_case name (for example `eqoa.items.slots.head`, `eqoa.items.stats.str`, `eqoa.items.attack_types.bow`). The names are listed in the [support table](SUPPORT-TABLE.md). `eqoa.items.slots.none` is 4294967295.

### Gems

`eqoa.gems.get(name:string)` returns `Gem = {type:string, rarity:string, stat:string}` for a gem in the client's built-in list, by exact, case-sensitive name, or `nil, nil`. `eqoa.gems.is_gem(name)` is the same test as a bool. `eqoa.gems.format(name, amount:uint, formatter:function?)` returns a display string: by default `name .. " x" .. amount`, followed by `" (type, rarity, stat)"` for a known gem; a `formatter(name, amount)` supplies the first part instead (an error inside it is returned as `failed`). The list is shipped data, not something the server confirms. All three work outside the world.

### Item events

`inventory.changed`, `bank.changed`, `loot.changed` and `merchant.changed` each carry `{before:Container?, after:Container?, changed:string[], cause:string}` and fire once per change. `before` is nil when the container appears and `after` is nil when it goes. `changed` names the container fields that differ. `cause` is:

- `open`, `close`: the container appeared or went, or the bank window's flag flipped;
- `reset`: zoning (with `before` or `after` nil);
- `frame`: a change the client noticed at a frame with no event of its own (a tab, a lit row, a merchant's mode);
- `refresh`: the client rebuilt the container;
- or the game's own event: `inv_add`, `inv_took`, `inv_move`, `inv_use`, `inv_equip`, `inv_unequip`, `quick_slot`, `gear_set`, `item_hp`, `loot_open`, `loot_close`, `loot_one`, `loot_all`, `loot_refused`, `merchant_open`, `merchant_ready`, `money`, `bank_open`, `bank_took`, `bank_added`, `bank_money`, `auction`, `fishing`.

More `cause` values may be added; treat one you do not know as a plain change.

## 7. Quests, Class Mastery, camera, windows and the map (rounds 4 and 6)

### Quests

| Function | Returns |
|---|---|
| `eqoa.quests.list()` | `Quest[]`: the journal's titles in the game's order, at most eight. `Quest = {ref:QuestRef, index:uint, title:string}`. |
| `eqoa.quests.get(ref:QuestRef)` | That `Quest`, or `stale_handle`. |

The server sends titles only: there are no objective or progress fields. Every `QuestRef` goes stale when the journal changes. `quests.changed` carries `{before:Quest[]?, after:Quest[]?, changed:{"records"}}`.

### Class Mastery

| Function | Returns |
|---|---|
| `eqoa.mastery.get()` | `Mastery`: the counters, readable with the Class Mastery window closed. `Player.mastery` is the same record. |
| `eqoa.mastery.skills()` | `MasterySkill[]` once the game has received its skill list; `unavailable` before that. |
| `eqoa.mastery.selected()` | The `MasteryNode` being inspected in the window, or `nil, nil`. |

`Mastery = {available:uint?, spent:uint?, experience_share:number?, progress_raw:int?, pending:bool, stats:int[], bonuses:int[]}`. `available` and `spent` are the window's "Points Available" and "Points Spent". `experience_share` is the share of experience going to mastery, 0..1 (it is not progress toward the next point). `progress_raw` is a raw counter with no verified meaning. `pending` is true while a mastery request waits for the server. `stats` and `bonuses` are the game's raw arrays. A counter the server has not sent is nil.

`MasterySkill = {id:int, parent_key:int, rank_key:int, name:string, category:uint, cost:int, known:bool, owned:bool, master_class:bool}`.
`MasteryNode = {id:int, skill_id:int, parent_key:int, name:string, title:string, category:uint, cost:int?, text:string, known:bool, owned:bool, level_required:uint, points_spent_required:int, minimum_stats:int[]}`.

`mastery.changed` carries `{before:Mastery?, after:Mastery?, changed:string[]}`. Changing the share, inspecting, buying and refunding are actions (§8).

### Camera

`eqoa.camera.get()` returns `Camera = {position:Vec3, heading:number, pitch:number, distance:number, mode:int}`: the eye's position, the camera's own heading and pitch in radians (not the character's), its distance from the character and the game's camera mode number. `eqoa.camera.compass()` returns the direction the eye looks along the ground, in radians with the §1 convention. Both give `not_in_world` outside the world and `unavailable` before the camera exists. `camera.changed` carries `{before:Camera?, after:Camera?, changed:string[]}`, at most once per game step. Moving the camera is an action (§8).

### Reading the game's windows

`eqoa.windows.get()` always answers, also outside the world (then `mode = "none"`, `native_mode = 0`, nothing open):

`Windows = {mode:string, native_mode:int, busy:bool, start_menu_open:bool, any_window_open:bool, active:WindowRef?, windows:Window[]}`

- `mode` names what has the game's controls; `native_mode` is the game's own number for it (table below).
- `windows` lists what is up: the window of the mode, then any overlays (`inventory`, `map`, `search`, `typing`). The last one has `focused = true`.
- `any_window_open` is true when `windows` is not empty. `busy` is true when the mode is not plain play (`normal` or `manual_camera`), the start menu is up or any window is open: plain-play keys do not reach the game then.
- `active` is the `WindowRef` of the focused window (the last one in `windows`), nil when nothing is open.

`windows.changed` carries `{before:Windows, after:Windows, changed:string[]}` when the final state of a client iteration differs from the last.

| `native_mode` | `mode` and `Window.kind` | | `native_mode` | `mode` and `Window.kind` |
|---|---|---|---|---|
| 0 | `normal` | | 27 | `gm_buff_pc` |
| 1 | `loot` | | 28 | `mail_tell` |
| 2 | `input` | | 29 | `mail_reply` |
| 4 | `gm_goto` | | 30 | `quick_chat` |
| 5 | `spellbook` (the Abilities window) | | 31 | `support_request` |
| 6 | `recipe_book` | | 32 | `support_inbox` |
| 7 | `response_dialog` | | 33 | `chat_history` |
| 9 | `text_dialog` | | 34 | `options` |
| 10 | `merchant` | | 35 | `who` |
| 11 | `bank` | | 36 | `fishing` |
| 12 | `trainer` (Character Summary) | | 37 | `experiment` |
| 13 | `start_menu` | | 38 | `manual_camera` |
| 14 | `confirm` | | 39 | `auction_search` |
| 15 | `trade` | | 40 | `mastery` |
| 16 | `trade_ask` | | 41 | `factions` |
| 17 | `buddy_request` | | 42 | `group_search` |
| 18 | `buddy_list` | | 43 | `duel_invite` |
| 19 | `blacksmith` | | 44 | `duel` |
| 20 | `guild_invite` | | 60 | `combat_record` |
| 21 | `guild` | | 61 | `mercenary_items` |
| 22 | `quest_log` | | 62 | `mercenary_spells` |
| 23 | `mail_compose` | | 63 | `dye_picker` |
| 24 | `mail_inbox` | | any other | `other` |
| 26 | `ignore_list` | | | |

`normal` and `manual_camera` put no window in the list. Besides the modes, four overlay kinds can appear as windows: `inventory` (the start menu's item page), `map` (the full map), `search` (player search) and `typing` (the chat line). A hidden session (below) leaves the mode at `normal` and adds no window.

`Window = {ref:WindowRef, kind:string, title:string?, visible:bool, focused:bool, rows:MenuRow[]?, selected_row:uint?, scroll_top:uint?, tabs:string[]?, tab:uint?, hints:ButtonHint[], dialog:Dialog?}`

- `rows` are the window's rows as `MenuRow = {index:uint, text:string, enabled:bool, item:ItemRef?, command:string?}`; `selected_row` and `scroll_top` are 1-based positions in it (nil when nothing is lit or the window has no list). A row's `command`, when present, is a name `eqoa.actions.menu_command` accepts (for example `loot.take_all`, `merchant.buy`). Rows are given only for windows the client can read; nil does not mean an empty window.
- `tabs` and `tab` (the labels and the 1-based tab showing) are given for the Abilities window, Class Mastery and the item page only.
- `hints` are the button hints the window shows, as `ButtonHint = {button:string, label:string}`; many windows give none.
- A `WindowRef` goes stale when that window closes or is replaced, or its page changes.

`Dialog` is one of three shapes, told apart by `kind`:

| `kind` | Fields |
|---|---|
| `"text"` | `text:string`, `options:string[]`: an NPC's text. For a response dialog, `options` are the choices shown (including the game's More, Back and Close entries) and `Window.rows` mirrors them; for a plain text dialog `options` is empty and the only answer is Done (`dialog_answer` option 1). |
| `"confirm"` | `text:string`, `style:"ok"\|"yes_no"\|"cancel"\|"other"`, `style_id:int`, `tag:int`, `prompt_kind:string`, `selection:uint?`: a message box. `selection` is the lit button (1 = Yes, 2 = No in a `yes_no` box; 1 in a one-button box). `prompt_kind` says what is being asked: `"group_invite"`, `"pet_dismiss"`, `"quit"`, `"guild"`, `"report"` or `"other"`. **More values may be added**: a prompt that reads `other` today may get its own value later, so treat any value you do not know as `other`. |
| `"input"` | `text:string?`, `title:string`, `tag:uint`, `chat:bool`, `password:bool`: a text box. `text` is always nil for a password box. `chat` is true for the chat line. `tag` identifies the box's purpose. |

| Function | Returns |
|---|---|
| `eqoa.windows.find(kind:string)` | The first window of that kind, or `nil, nil`. |
| `eqoa.windows.get_window(ref:WindowRef)` | That window now, or `stale_handle`. |
| `eqoa.windows.format_hint(button:string, label:string)` | The game's hint text for a button, in its `\|glyph label` form. `invalid_argument` for an unknown button name. Button names: `select`, `l3`, `r3`, `start`, `up`, `right`, `down`, `left`, `l2`, `r2`, `l1`, `r1`, `triangle`, `circle`, `cross`, `square`. |
| `eqoa.windows.parse_hint(text:string)` | `ButtonHint[]` from such text, in order; plain text between glyphs comes back with `button = "text"`. |
| `eqoa.windows.open(name:string)` | Opens the Character Summary: `name` must be `"summary"`. Returns `true`; needs no capability; `not_in_world` outside the world. Other windows are opened with `eqoa.actions.open_window` (§8). |

Operating a window (rows, dialogs, text boxes) is done with actions (§8).

### Taking over a window

A mod can keep the game's corpse, merchant or bank window from appearing while it shows the same thing its own way, or while it works unseen (selling junk, taking loot). The session with the server is real and unchanged; only the game's window is hidden. `eqoa.loot.get`, `eqoa.merchant.get`, `eqoa.bank.get` and the item actions work on a hidden session; the window actions do not.

| Function | Returns |
|---|---|
| `eqoa.windows.takeover(kind:string, {scope="own"\|"all"}?)` | `true` when the mod holds the claim. `kind` is `"loot"`, `"merchant"` or `"bank"`. Calling it again while holding the claim keeps it and renews a running hidden session's idle time. |
| `eqoa.windows.release(kind:string?)` | `true` if the mod held it. With no argument, releases every kind the mod holds. |
| `eqoa.windows.taken(kind:string?)` | Whether any mod holds a claim on `kind`. With no argument: `{loot:bool, merchant:bool, bank:bool, holders:table}`, where `holders` maps each held kind to the holding mod's id. |

**Scopes.**

- `"own"` (the default) hides only the sessions the mod itself starts with `eqoa.actions.open_loot` or `eqoa.actions.talk` (or a batch). A corpse, merchant or banker the player clicks still opens the game's window. This is the scope for helpers.
- `"all"` hides every session of that kind, also the ones the player starts. It is for a mod that draws a complete replacement window: the mod must draw it and give the player a way to close it (`release`).

**What `takeover` checks, in this order.** The first failure is the answer:

| Check | Error code |
|---|---|
| The mod declares `eqoa.windows.takeover` | `not_declared` (no `capabilities` list) or `permission_denied` (not in the list) |
| `kind` names one of the game's windows (a `Window.kind` from the table above, or an overlay) and `scope` is `"own"` or `"all"` | `invalid_argument` |
| Scope `"own"` is not asked for a window that exists only in the client (`inventory`, `spellbook`, `quest_log`, `trainer`, `mastery`, `recipe_book`, `experiment`, `start_menu`, `map`): there is no session to start, so such a window can only be replaced whole | `invalid_argument` |
| This build can replace that window: `loot`, `merchant` or `bank` | `unsupported` for every other window |
| For scope `"all"`: the manifest's `replaces` list names that window | `not_declared` |
| For scope `"all"`: the player's **Use the game's windows** switch is off | `unavailable` |
| No other mod holds that kind | `busy` (with a diagnostic naming the holder) |

So `not_declared` has two meanings for takeover: no `capabilities` list at all, or no matching `replaces` entry for a full replacement. `release` and `taken` need no capability and answer `invalid_argument` or `unsupported` for a kind the same way.

**Declaring it.** A mod that uses scope `"all"` lists the windows in `replaces` in `mod.json` (§2). `replaces` accepts every window kind, so a mod written for a later client can already say what it means to replace, and the `hud.<piece>` names (`hud.target`, `hud.minimap`, ... : the `eqoa.hud.hide` element names with `hud.` in front), which are only shown to the player; HUD pieces are hidden with `eqoa.hud.hide`, not `takeover`. A mod that only uses scope `"own"` needs no `replaces`.

**What the player is shown** (Mods & Addons, from the manifest, before enabling):

- `Replaces game windows: Loot, Merchant, Bank` for the windows in `replaces` this build can replace (only when the mod also declares `eqoa.windows.takeover`);
- `Not replaceable in this version: ...` for the other windows in `replaces`, by the game's own titles;
- `Replaces HUD: ...` for `hud.<piece>` entries, by token (`hud.target` reads `Target frame`). For a mod that also declares `eqoa.windows.takeover` these are listed in the `Replaces game windows` line instead.

**The player's switch.** Mods & Addons has a checkbox, **Use the game's windows**. It is an emergency override for full replacements only, and it is remembered across restarts. Turning it on releases every scope `"all"` claim at once (the game's windows come back) and stops batches that held or used one; while it is on, `takeover(kind, {scope="all"})` is refused with `unavailable`. Scope `"own"` claims are not affected: helpers keep working.

**A hidden session**

- begins only when the server's open is known to answer the holding mod's own request. An open that cannot be tied to the request, or a window that was already open, stays the game's.
- ends when the mod's action or batch is finished, when the mod releases the claim, is disabled or fails, on zoning, when the player walks more than 25 units from where the session began, and, for scope `"own"`, after 15 seconds without use. When it ends the client closes the session with the server once, quietly.
- A request the server never answers ends as `unknown` after 10 seconds.

The 25-unit, 15-second and 10-second values are design intent, not part of the promise.

**The server lists a corpse or merchant again by itself.** After each take the live server closes the corpse and lists it again; after each sale or purchase it sends the merchant's list again. While a hidden session runs, such a list stays hidden. The last one can arrive after the session has ended: for 3 seconds after a hidden session ends, a list from the same corpse or merchant is still held hidden and closed quietly. This applies only when the player has not asked for that window themselves. Known limit: if the player talks to the same merchant within those 3 seconds, or while the mod's sale is still unanswered, that first window is hidden too; talking again opens it. (The 3 seconds are design intent, not part of the promise.)

Two further limits: the game's window-open sound can play for a session that is then hidden, and a mod that changes scope while its hidden session is running does not change that session.

### The map

Reading the map needs no capability.

| Function | Returns |
|---|---|
| `eqoa.map.get()` | `MapState`, below. |
| `eqoa.map.markers(world_id:int)` | `Marker[]` for that world (0..255): what the game's map can show. |
| `eqoa.map.view({size?, layers?})` | A held minimap picture the mod can draw; see "Minimap views". |
| `eqoa.map.set_guide(spec)`, `eqoa.map.clear_guide()` | The mod's own guide line; see "Guides". |
| `eqoa.map.open()` | Opens the full map, as the player's map key does. Returns `true`. `unavailable` while the player has Map turned off, `busy` if the request cannot be queued. Needs no capability. |
| `eqoa.map.set_waypoint(point)`, `eqoa.map.clear_waypoint()` | Move or clear the player's own waypoint; see "The player's waypoint". |

`MapState = {enabled:bool, open:bool, minimap:bool, markers_on:bool, world_id:int, world_name:string?, zone:Location?, bounds:Bounds?, revision:uint, waypoint:Waypoint?, search_anchor:Rect?}`

- `enabled`, `markers_on`: the player's Map and Map Markers settings. `open`: the full map is up. `minimap`: the minimap is in view (setting on, in play, full map closed, HUD shown), whether or not a mod hides the game's picture.
- `bounds = {x_min, z_min, x_max, z_max}`: the current world's extent in game units, nil until its map is loaded.
- `waypoint = {world_id:int, x:number, z:number, label:string?, owner:string?}`: the personal waypoint; `owner` is the id of the mod that placed it, nil when it is the player's.
- `search_anchor = {x, y, width, height}`: the free band of the full map's header in window pixels, for a mod that puts a control there; nil before the full map has been drawn once.
- `revision` goes up when the state changes; `map.changed` carries `{before:MapState?, after:MapState?, changed:string[]}`.

`Marker = {kind, name, x, z, level_min, level_max, label, step}`: `kind` is `"camp"`, `"coach"`, `"town"`, `"quest"`, `"community"`, `"waypoint"` or `"group_waypoint"`; `x`, `z` are game units; `level_min` and `level_max` are given for camps and quests; `label` is the game's search label when it has one (a camp's "(XP camp)" label, a coach's town and coachman, a community marker's note); `step` is given for quests: true for a step to do, false for a quest on offer.

#### The player's waypoint (round 7)

`eqoa.map.set_waypoint({world_id:int, x:number, z:number, label:string?})` and `eqoa.map.clear_waypoint()` return a `RequestId`; the outcome is an `action.result` (§8) of kind `set_waypoint` or `clear_waypoint`. Both need the capability **`eqoa.map.waypoint`**, listed by its own name (`eqoa.actions.*` does not cover it); Mods & Addons then shows "map waypoint" among what the mod can do on its own. No click from the player is needed.

- The call itself fails with `not_in_world`, `unavailable` (Map turned off), the capability errors, or `invalid_argument` (a malformed point, a non-finite number, a label over 128 bytes).
- The request is then `refused` with `invalid_argument` for a world the game does not know or a point outside that world's ground, and with `not_ready` for a known world whose map is not loaded yet. The pin lands only on a loaded map's ground.
- The pin is the game's own: one per world, saved, drawn and reached as if the player had placed it.
- **Ownership.** A mod owns a pin only while that exact pin is still there. Every game step the client drops ownership, for good, of a pin that is gone or differs (the player moved or cleared it, arrived at it, or cleared and re-placed it at the same spot), and another mod's `set_waypoint` takes it over. `clear_waypoint()` clears the pin this mod placed most recently and still owns; if it owns none the result is `refused` with `busy`. Disabling a mod clears only the pins it still owns, in every world; the player's pins and another mod's stay.
- The game keeps one pin file per client window, shared by every character played in it: a mod's pin and its ownership carry across a logout and a character change for the rest of the session. Ownership is not saved: after a restart every pin is the player's.
- Known limit: a player who clears and re-places the very same point within one game step (under 34 ms) is not noticed.

#### Guides

`eqoa.map.set_guide({world_id:int, x?, z?, entity_id?, dots?, label?})` sets this mod's guide: a dashed line with optional dots (`dots = {{x, z}, ...}`), drawn on the full map, the minimap and minimap views. It is separate from the waypoint and needs no capability. Give a point (`x`, `z`) or an `entity_id`; an entity guide follows the live entity (`not_found` if the client does not see it in that world). A mod has one guide; setting another replaces it. `eqoa.map.clear_guide()` removes it. Both return `true`. A guide is cleared when the mod is disabled.

`guide.lost` fires when a guide ends by itself: `{owner:uint, world_id:int, entity_id:uint?, label:string?, reason:"entity_left"|"world_left"}` (`owner` is a number identifying the mod's running instance). An entity guide is lost when its entity goes; every guide is lost when the world is left.

#### Minimap views

`eqoa.map.view({size:number?})` returns a view: the game's minimap, centred on the player, without the game's frame art, as a picture the mod draws itself (for example cut to a circle with `eqoa.hud.image`). `size` is in pixels, 16..1024 (`out_of_range` otherwise). A mod may hold four views (`quota` for a fifth). The game draws the picture only while its own minimap is hidden (`eqoa.hud.hide("minimap")`) and in view.

| Member | Meaning |
|---|---|
| `view.texture` | A `TextureInfo`, or nil while there is no picture: before the first capture, while the game's own minimap is shown, after a resize until the next capture, after `close`. |
| `view.error` | nil, or the `Error` that keeps the picture from being made. |
| `view.get_texture()` | The `TextureInfo`, nil, or `nil, Error`. |
| `view.project(x, z)` | `px, py, inside`: a world position in the picture's pixels, and whether it falls inside the visible map. |
| `view.unproject(px, py)` | `x, z`: the world position under a pixel. |
| `view.close()` | Releases the view now. A view the mod drops is released when Lua collects it. |

There is no centre or zoom argument: the view follows the game's zoom, the wheel and the indoor view. `eqoa.input.minimap_wheel` (§5) zooms it.

## 8. Game actions (round 6)

A mod may make the game do things: target, attack, cast, use and move items, loot, buy and sell, talk in chat, open and operate the game's windows. **Automation is allowed**: an action may be called from any event, timer, key or render callback, with no click from the player. Actions go through the same paths as the player's own controls, with the client's ordinary checks. There is no packet API, no teleport, no stat or cooldown edit, and nothing that gives a mod rights the player's character does not have.

### Who may act (permissions)

1. **Enabling a mod is the player's permission.** Before enabling, Mods & Addons shows what the mod declares (§2 "What the player is shown").
2. **Every action needs its capability in `mod.json`.** `eqoa.actions.<name>` grants one action; `eqoa.actions.*` grants them all. A mod whose `mod.json` has no `capabilities` list is refused with `not_declared`; a mod with a list that lacks the capability is refused with `permission_denied`. Either refusal also shows as a diagnostic in Mods & Addons.
3. `eqoa.actions.*` covers only names under `eqoa.actions.`. Window takeover needs `eqoa.windows.takeover` and moving the map waypoint needs `eqoa.map.waypoint`, each listed by its own name (§7).
4. **Chat lines are judged by the line that is sent, at every entry point.** A line that starts with `/` needs `eqoa.actions.command`; any other line needs `eqoa.actions.chat`. This holds for `eqoa.actions.chat`, `eqoa.actions.command`, a `chat` or `command` step in a batch, a line typed into the game's chat box with `eqoa.actions.input_text`, and (for a native mod) Forge's `SendChatText`.
5. **Forge packages** (no `mod.json`) cannot call `eqoa.actions` (`not_declared`). Their own Forge helpers that act (sending chat lines and slash commands, switching weapon sets) are not gated, so every Forge package is labelled in Mods & Addons as able to do those three things.
6. A native mod that uses the legacy alias `eqoa.WeaponSets.Use` needs `eqoa.actions.weapon_set`.
7. **Mods act only in the game world.** Outside it (login, character select, loading) every action is refused with `not_in_world`. Mods are not restricted on servers that ask for the game as it was: a player who wants the plain game does not enable mods.
8. **Reports are never sent by a mod.** A chat or command line whose command is `/bug`, `#bug`, `/feature` or `#feature` (in any letter case, also behind a say, tell, group, guild, shout, reply or similar wrapper) is refused with `permission_denied`, and so is any attempt to operate the game's support and report windows, its report text box or its report confirmation.

The capability is checked before the arguments, except for `chat` and `command`, which check the text first.

### Requests and results

An action call returns `RequestId, nil` once the request is accepted into the mod's queue, or `nil, Error` if it is not. Accepted is not done: the outcome arrives later as an `action.result` event and can be read with `eqoa.actions.status`. A `RequestId` is an opaque handle; compare with `==`, do not store it across sessions.

`ActionResult = {request_id, kind, state, phase, ok, code, reason, queued_tick, dispatched_tick, completed_tick, detail}`

| Field | Type | Meaning |
|---|---|---|
| `request_id` | `RequestId` | The request. |
| `kind` | string | The action's name, for example `"sell"`. |
| `state` | string | `"queued"`, `"dispatched"`, then one final state: `"completed"`, `"refused"`, `"cancelled"` or `"unknown"`. |
| `phase` | string | How far it got: `"queued"`, `"dispatch"` (the client acted or refused) or `"server"` (it concerns the server's answer). |
| `ok` | bool? | nil while the outcome is not known: `queued`, `dispatched` (sent, waiting for the server), `unknown`, and `cancelled` after the request was sent (the server may still act). true for `completed`; false for `refused` and for `cancelled` before anything was sent. Do not use it to tell whether a request is finished: read `state`. |
| `code`, `reason` | string? | Why, when refused, cancelled or unknown: an error code (§1) and a sentence. |
| `queued_tick`, `dispatched_tick`, `completed_tick` | uint, uint?, uint? | 30 Hz game steps at each stage. |
| `detail` | table? | Extra facts for some actions (for example `{typed, submitted}` for `input_text`). |

- `completed` means the client did it (a local action such as targeting) or the server's answer confirmed it (loot, buy, sell, bank, opening a corpse or a merchant).
- `refused` means a check failed or the server said no. `reason` carries the server's words when it gave any.
- `unknown` means the request was sent and no answer the client can tie to it came within 10 seconds. The client does not guess and does not retry. Show it; do not treat it as success or failure.
- `cancelled` means it was cancelled by the mod, or by zoning, logging out, or the mod being disabled.

The `action.result` event carries the `ActionResult` and fires when a request is dispatched to the server (`state = "dispatched"`) and again at its final state; an action with no server step fires once, with its final state. Nothing fires when the request is queued.

| Function | Returns |
|---|---|
| `eqoa.actions.status(id:RequestId\|BatchId)` | The latest `ActionResult` (or `BatchResult`). `not_found` once the record has been dropped: the client keeps the latest 256 results per mod (tested). |
| `eqoa.actions.cancel(id:RequestId\|BatchId)` | `true` if it cancelled something. A queued request is never sent. A request already sent stops being waited for (its result becomes `cancelled`); the server may still carry it out. `not_found` if the request is no longer pending. With a `BatchId` it is `eqoa.actions.stop_batch`. |
| `eqoa.actions.stop()` | Queues a stop: ends auto-attack, auto-run, follow and the pending cast. See the note below. |

Limits, as built and tested: 32 requests may wait per mod (one more is refused with `busy`); at most 8 are dispatched per mod per game step, refilling at one per step (30 a second), and 64 per step for all mods together, shared fairly; a request that waits more than 1 second for its turn is refused with `rate_limited`. Only one transaction at a time runs against each of the pack, a corpse, a merchant and the bank, for all mods together: a second one is refused with `busy`.

Most actions need the character alive and the game in plain play (no game window owning the controls); otherwise the result is `refused` with code `not_ready`. An entity, item, quest or window handle from an earlier world or session gives `stale_handle`.

### The actions

All of these are functions of `eqoa.actions`, need their capability, and return a `RequestId`.

| Function | What it does |
|---|---|
| `eqoa.actions.target(entity)` | Targets an entity (`EntityRef` or entity id); `nil` clears the target. `not_found` if it cannot be targeted now. |
| `eqoa.actions.cycle_target(filter:string?)` | The game's next-target key. `"group"` cycles group members, `"corpse"` cycles corpses in view; with no filter (or `"any"`, `"nearby"`) it cycles as the key does. The game has no hostile or friendly cycling: `"hostile"` and `"friendly"` are refused with `unsupported`; any other filter is `invalid_argument`. |
| `eqoa.actions.assist()`, `eqoa.actions.target_target()` | Targets your target's target (the game's Assist). The two names are the same action; a `target_target` call needs `eqoa.actions.target_target`, an `assist` call `eqoa.actions.assist`. `refused` if there is no target to assist. |
| `eqoa.actions.interact()` | The game's interact with the current target: talks to an NPC, opens a corpse, as the player's key does. |
| `eqoa.actions.attack(on:bool?)` | Auto-attack on or off; with no argument, toggles. On needs a living target that can be attacked. |
| `eqoa.actions.cast(ability)` | Casts an ability on the current target (`AbilityRef`, ability id or name). It must be memorized and usable. `not_ready` for no power, no valid target or out of range; `busy` while casting or while the ability is on its lockout; `not_found` if you do not have it. `completed` means the request was sent, not that the cast landed. |
| `eqoa.actions.use_hotbar(slot)` or `(bar, slot)` | Uses a hotbar slot as its key does: `slot` 1..20 across the ability bars, or `bar` 1..4 and `slot` 1..5. `not_found` for an empty slot. |
| `eqoa.actions.use_item(item:ItemRef)` | Uses one item from the pack. |
| `eqoa.actions.equip(item:ItemRef)` | The item page's equip command for a pack item: wears it if it is not worn, takes it off if it is. See the note below. |
| `eqoa.actions.swap_items(a:ItemRef, b:ItemRef)` | Swaps two pack stacks. |
| `eqoa.actions.quick_slot(item:ItemRef, slot:uint)` | Puts a pack item in quick slot 1..4. |
| `eqoa.actions.weapon_set(set)` | Switches weapon set: 1..5 (5 is bare hands) or `"bare_hands"`. `unsupported` on a server without weapon sets. |
| `eqoa.actions.prepare_weapon_set(set:uint, name:string)` | Saves the worn weapons as set 1..4 under a name of at most 16 characters. |
| `eqoa.actions.assign_ability(slot:uint, ability)` | Memorizes an ability into hotbar slot 1..20, as the Abilities window does. |
| `eqoa.actions.unmemorize_ability(x)`, `eqoa.actions.unmemorize_all()` | Clears one memorized ability (a slot number 1..20, or an ability) or all of them. The game itself asks nothing before Unmemorize All, and neither does this. |
| `eqoa.actions.arrange_ability(from, to)` | Moves an ability in the Abilities window's order. |
| `eqoa.actions.loot(item:ItemRef, count:uint?)`, `eqoa.actions.loot_all()` | Takes one stack (or `count` of it) or everything from the open corpse. `window_closed` if no corpse is open. |
| `eqoa.actions.buy(item:ItemRef, count:uint=1)`, `eqoa.actions.sell(item:ItemRef, count:uint=1)` | Buys from the open merchant's wares, sells a pack stack to it. `window_closed` if no merchant is open. |
| `eqoa.actions.bank_move(item:ItemRef, count:uint?)` | Moves a stack between pack and bank; the direction follows from the list the item is in. An optional first argument (`"bank"` or `"withdraw"`, `"inventory"` or `"deposit"`) names the list it comes from. `window_closed` if no bank is open. |
| `eqoa.actions.bank_money(direction:"deposit"\|"withdraw", amount:uint)` | Moves tunar. |
| `eqoa.actions.open_loot(entity?)` | Asks the server to open a corpse: the given one (`EntityRef` or id) or the nearest. Refuses anything that is not a corpse (`invalid_argument`); `not_found` if there is none. The result is `completed` when the corpse opens, `refused` with the server's reason (for example `out_of_range`), or `unknown`. The client targets the corpse for this and puts your previous target back when the mod is done, unless you chose another meanwhile. |
| `eqoa.actions.talk(entity?, {kind="merchant"\|"bank"}?)` | Talks to an NPC: the given one or the nearest of that kind (any friendly NPC when `kind` is omitted). Refuses corpses and NPCs that are not of the kind asked for. `completed` when the merchant or bank opens. The target is put back as for `open_loot`. |
| `eqoa.actions.chat(text:string)` | Sends a line as if typed into chat, in the current channel. The line must be shorter than 128 UTF-16 units and contain no line break (`invalid_argument`). A second argument is refused with `unsupported`: to speak in a particular channel use `command` with that channel's slash command. Grants: rule 4 above. |
| `eqoa.actions.command(text:string)` | The same, for a command: a leading `/` is added if the text has none (and does not start with `#`). The game's own rules for each command still apply. |
| `eqoa.actions.autorun(on:bool?)` | Auto-run on or off; toggles with no argument. |
| `eqoa.actions.follow(entity)` | Follows an entity in view (it is targeted first). `not_ready` if the game will not follow it. `follow(nil)` only stops following: when nothing is being followed it completes and does nothing. |
| `eqoa.actions.camera({heading?, pitch?, distance?, mode?})` | Sets the camera: radians, game units, `mode` 0..3. At least one field. `busy` when the game has the camera locked. |
| `eqoa.actions.mastery_share(fraction:number)` | Sets the share of experience going to Class Mastery: 0..1 in steps of 0.01. |
| `eqoa.actions.mastery_browse(skill_id:int)` | Inspects a mastery skill, as the Class Mastery window's list does. It needs that window open with its list loaded (`not_ready` otherwise); `not_found` for a skill not in the list. Buying and refunding are not available (below). |
| `eqoa.actions.open_window(kind:string, arg:int=0)` | Opens one of the game's windows; see the list below. |
| `eqoa.actions.close_window(window:WindowRef)` | Closes that window, as the back button does. |
| `eqoa.actions.select_row(window, row:uint)`, `eqoa.actions.activate_row(window, row:uint)` | Moves the lit row to `row`; `activate_row` then confirms it. Rows are the 1-based `Window.rows`. `out_of_range` for a row that does not exist, `not_ready` for a row that is not enabled. |
| `eqoa.actions.dialog_answer(window, option:uint)` | Answers a dialog: the 1-based position in `Dialog.options` for a text dialog (1 for its single Done), or 1 or 2 for a confirm (§7 `Dialog`). |
| `eqoa.actions.menu_command(window, command:string)` | Runs a command that an enabled row of that window offers (`MenuRow.command`, for example `"loot.take_all"`). Any other string is `unsupported`. |
| `eqoa.actions.input_text(window, text:string, submit:bool=false)` | Types into the game's open text box; see "Text boxes" below. |

**Not available:**

- `eqoa.actions.destroy` is `unsupported`. A mod cannot destroy items.
- `eqoa.actions.move`, `eqoa.actions.look` and `eqoa.actions.buttons` (held movement, camera stick and controller buttons) are `not_yet`: they wait for a change in the upstream client. Calling one returns `nil, Error{code="not_yet"}`.
- `eqoa.actions.toggle_stats_box` belongs to the bundled Stats Box mod; any other mod gets `permission_denied`.
- `eqoa.actions.drop_quest`, `eqoa.actions.mastery_buy` and `eqoa.actions.mastery_refund` are `not_yet`. The game asks the player Yes or No before dropping a quest and before buying or refunding a Class Mastery skill, and these calls would take the step without that answer. They come back with the confirm contract of the window-replacement roadmap, which lets a mod draw the confirmation its own way while the step still waits for the player's own choice. Calling one returns `nil, Error{code="not_yet"}` and sends nothing. A mod can still lead the player there: `open_window("quests")` or `open_window("mastery")`, and the game's own window asks.

**The game's confirmations cannot be skipped.** Where the game asks the player to confirm a step that cannot be undone (its Yes/No box: quitting, dismissing a pet, leaving a guild and the like), the box is still raised when a mod leads the game there, and the step does not happen until it is answered. A mod that replaces a window may read the box (`Dialog`, §7) and draw it its own way; it answers with `eqoa.actions.dialog_answer`, which needs its own capability and is shown to the player as "Can answer NPC dialogs". The API has no way to make the game leave the box out, and no action takes a step the game would have asked about (see `drop_quest` and the mastery purchases above).

**Under review, not part of the promise.** These behave as described today, and may be corrected before or after the freeze; do not build on them:

- `stop()` ends auto-attack, auto-run and follow whoever started them, the player included.
- `equip` toggles; there is no "wear" or "take off" form.

### Windows the game can be asked to open

`eqoa.actions.open_window(kind, arg)` accepts these kinds (several names for one window are aliases):

| Kind | Opens |
|---|---|
| `summary`, `trainer`, `skills` | Character Summary |
| `start`, `menu` | The start menu |
| `inventory`, `bags` | The start menu's item page |
| `abilities`, `spellbook` | Abilities |
| `quests` | Quest Journal |
| `mastery` | Class Mastery |
| `factions` | Factions |
| `options` | Options |
| `map` | The full map (`unsupported` while the player has Map turned off) |
| `friends`, `buddies` | Friends list |
| `ignore` | Ignore list |
| `guild` | Guild (`not_ready` if the character is in no guild) |
| `mail`, `mail_compose` | Mail inbox, new mail |
| `tell`, `reply` | The tell and reply text boxes |
| `auction`, `auction_browse`, `auction_search` | Auction search (`not_ready` where the server offers none) |
| `support`, `support_inbox` | The support windows (a mod can open them but never operate them) |
| `who` | Who list |
| `search` | Player search (`unsupported` where the server has none) |
| `recipes` | Recipe book; `arg` is the trade skill's id |
| `experiment` | Experiment |

Any other kind is `invalid_argument`. `arg` is used only by `recipes`. A corpse, a merchant and the bank are not opened this way: use `open_loot`, `talk` or `interact`. Where a window's assets are not loaded yet the result is `not_ready`.

The window actions (`close_window`, `select_row`, `activate_row`, `dialog_answer`, `menu_command`, `input_text`) work by pressing the game's own keys in the window that has the keyboard, over the next few frames. `completed` means the keys were delivered. They need a window the client can read (`unsupported` otherwise) that currently has the keyboard (`busy` otherwise), and they do not work on a session a mod has hidden (§7 "Taking over a window"): there, use `loot`, `loot_all`, `buy`, `sell`, `bank_move` and `bank_money`.

### Text boxes

`eqoa.actions.input_text(window, text, submit)` needs `eqoa.actions.input_text`, which Mods & Addons shows as "Text boxes: can type into and submit the game's open text boxes". It types `text` (at most 512 characters, only characters the game's keyboard can type) into the text box that is open, and with `submit = true` confirms it, exactly as the player would. A password box is always refused (`permission_denied`).

What else the mod needs depends on what the box sends. It is checked when the call is made and again when the text is typed, whether the text is submitted or left for the player to send:

| Text box | Also needs |
|---|---|
| The chat line | The grant for the line: `eqoa.actions.command` for a line starting with `/`, `eqoa.actions.chat` otherwise |
| The Who list's Tell (it sends `/t <name> <text>`) | `eqoa.actions.command` |
| Guild Say, the guild's message of the day | `eqoa.actions.chat` |
| Guild Create, the alarm, a GM command box | `eqoa.actions.command` |
| A player's name for a friend request, a guild invitation, the ignore list (add or remove) | nothing more |
| Report a player | always refused, `permission_denied` |
| A community map marker's name or note | always refused, `unsupported` |
| Any other text box | refused, `unsupported` |

A missing grant is refused with `permission_denied`.

### Batches

A batch is a list of actions the client runs one at a time, each waiting for the one before it (and for the server's answer where there is one).

`eqoa.actions.batch(spec)` returns `BatchId, nil`. `spec` is a list of steps, or `{steps = {...}, require_window = "merchant"|"bank"|"loot"}`. There may be 1 to 64 steps (design intent, not part of the promise). Each step is a table `{kind = "<action>", ...}` with that action's arguments by name. It needs `eqoa.actions.batch` (or `eqoa.actions.*`) and each step's own capability (`chat` and `command` steps by rule 4; a `target_target` step counts as `assist`). A mod runs one batch at a time (`busy`).

Steps may be: `target`, `assist`, `target_target`, `interact`, `attack`, `autorun`, `stop`, `cast`, `use_item`, `equip`, `quick_slot`, `weapon_set`, `unmemorize_all`, `open_loot`, `talk`, `loot`, `loot_all`, `buy`, `sell`, `bank_move`, `bank_money`, `chat`, `command`. Any other kind is `invalid_argument`.

`BatchResult = {batch_id, state, ok, completed, total, current_step, code, reason, queued_tick, completed_tick, failed_step}`: `state` is `"running"`, then `"completed"`, `"refused"`, `"stopped"`, `"cancelled"` or `"unknown"`; `ok` is nil while `running` and for `unknown`, true for `completed`, false for `refused`, `stopped` and `cancelled`; `completed` counts the steps done out of `total`; `current_step` is the 1-based step in hand; `failed_step` the step that ended it. (`detail` is declared and never filled; each step's own facts are in that step's `action.result`.)

- `batch.progress` fires as the batch advances and `batch.result` once when it ends, both with the `BatchResult`. Each step also fires its own `action.result`.
- `eqoa.actions.batch_status(id:BatchId)` returns the `BatchResult`; `eqoa.actions.status` does too.
- `eqoa.actions.stop_batch(id:BatchId?)` returns `true` if it stopped the running batch, `false` if none was running. The batch ends `stopped` (code `cancelled`, reason `stopped`). A step not yet sent is never sent. A step already sent finishes: the one `batch.result` waits for the server's answer to it (at most 10 seconds) and counts the step if the server confirmed it.
- A batch ends at once, and its remaining steps are dropped and never replayed, when: a step is refused; the window it needs closes (`window_closed`, reason `merchant closed`, `bank closed` or `corpse closed`); the server does not answer a step within 10 seconds (`unknown`); it waits more than 1 second for its turn (`rate_limited`); the player zones or logs out (`cancelled`); it is stopped; or the player turns on **Use the game's windows** while the batch holds or used a full replacement of a window (§7). Disabling or reloading the mod cancels its requests and its batch without an event: a disabled mod receives nothing.

## 9. The sandbox and its limits

A mod runs in its own Lua 5.1 state, apart from other mods and from the game's memory.

**What Lua has.** The base functions (`pairs`, `ipairs`, `pcall`, `type`, `tostring`, `setmetatable`, `select`, `unpack`, ...), `string`, `table`, `math`, and `require`. From `os` only `clock`, `date`, `remove` and `rename`; from `io` only `io.open` in text modes `"r"` and `"w"` (both work on the mod's private files, like `eqoa.files`). There is no `debug`, `coroutine`, `load`, `loadstring`, `dofile`, `loadfile`, `xpcall`, `collectgarbage`, FFI or native library loading. An instruction or memory limit cannot be swallowed with `pcall`.

**`require`** loads Lua source text from the mod's own folder and its `modules/` folder (and the shared `modules/` folder of the `mods` folder), up to 1 MiB a file, never bytecode. `require("eqoa")`, `require("eqoa.<module>")` and `require("bit")` return the built-in tables.

**No network, no processes, no other files.** A mod cannot open sockets or URLs, start programs, read the clipboard or reach anything on disk outside the places below. Chat lines and game actions travel over the game's own connection, as the player's do.

**Files a mod can reach.** Paths are relative, use `/`, and may not contain `..`, a drive or a leading slash (`permission_denied`).

| Path | Is | Access |
|---|---|---|
| `package_resources/...` | The mod's own `resources/` folder | Read. A write goes to a private copy that only this mod sees from then on. |
| `resources/...` | The shared `resources/` folder of the `mods` folder (fonts and art shipped with the client) | Read. A write goes to a private copy, as above. |
| anything else, for example `notes.txt` or `data/journal.json` | The mod's private data | Read and write. |
| `@<mod id>/data/...` | Another mod's shared file | Read, with `eqoa.files.load_json` only (§5). |

The files in the `mods` folder itself are never changed by a mod: its private data and private copies live in the player's configuration folder, keyed by the mod's `id`. Removing a file that came from `resources/` or `package_resources/` hides it for this mod only (and it stays hidden after a restart); writing it again brings back a private copy. A bare file name given to `eqoa.textures.load`, `eqoa.fonts.load` or `eqoa.sound.load` is looked for in `package_resources/`, then `resources/`. Links and junctions are refused. Listing, private writes and font and sound loading work on Windows, the published platform, and answer `unsupported` elsewhere.

**Limits.** Tested values are part of the contract in this sense: a mod within them works.

| Limit | Value | |
|---|---|---|
| Lua memory per mod | 8 MiB; 128 MiB with `"large_data"` | tested |
| Lua instructions per mod per frame, all handlers together | 4 million | tested |
| A private file; all private data of a mod, settings included | 1 MiB; 16 MiB | tested |
| Files open at once through `io.open` | 16 | tested |
| HUD draw calls per mod per frame (`eqoa.hud.text` and the like) | 256 | tested |
| Queued actions, dispatch rate, retained results | §8 | tested |
| Undelivered events per mod; events delivered per frame | 1,024 (2 MiB); 256 | tested |
| JSON load: file size, depth, values, loads at once | 16 MiB, 32, two million, 2 | tested |
| Commands waiting per sender (`eqoa.mods.send`) | 4 | tested |
| Fonts and sounds held | 16 each | tested |

Design intent, not part of the promise (the client is tuned this way today; no test pins the number, or the number is expected to move): 16 mods running at once; ImGui calls and drawn geometry per mod per frame (131,072 calls; a geometry budget of about 20,000 shape points); 64 nested ImGui scopes; textures up to 2,048 pixels a side, 128 per mod, 64 MiB decoded; media files up to 16 MiB; fonts and sounds up to 64 MiB together; `require` nested 32 deep; settings 32 deep and 1 MiB; 1,024 entries per listing; 64 log lines between frames; 128 subscriptions; 32 option buttons; 32 pointer areas per frame; four map views; the timing of diagnostics (§2); the timings of hidden window sessions (§7).

Going over a limit is refused with `quota`, `limit_exceeded` or `out_of_range` and reported; exceeding the memory or instruction limit stops the mod.

## 10. Keys

A mod gets keys in two ways: a row on the game's Controls page that the player can change (`eqoa.input.bind`, `eqoa.binds.register`, §5), or by listening to `input.key` itself. The rules below are the same for both and are settled by the client, not by the order mods load in.

**A key is its full name.** The name is the modifiers held, then the key, in lower case and in this order: `super+ctrl+alt+shift+key`. `u`, `ctrl+u` and `ctrl+shift+u` are three different keys; `shift+space` is not `space`. Key names are the client's: letters and digits as themselves, `space`, `enter`, `escape`, `backspace`, `tab`, `up`, `down`, `left`, `right`, `pageup`, `pagedown`, `home`, `end`, `insert`, `delete`, `f1` to `f12`. A name written with the modifiers in another order is read as the same key. The name comes from the key's place and the modifiers, never from what the press types on the player's keyboard layout.

The `input.key` event carries both: `key` is the plain key, `combo` the full name of the press. A binding fires when its key equals `combo`: a binding on `ctrl+u` fires on Ctrl+U only, a binding on `u` fires on U alone.

**One press does one thing.**

- **A modifier combination over a game key belongs to the mod for that press.** If the game uses Space for Jump and a mod has `shift+space`, then Shift+Space fires the mod and the game does not jump for that press; Space alone still jumps. A mod never takes a bare key away from the game this way.
- **A real conflict is the same name on both sides**: the mod's key is exactly a key a game control has (a bare key, or the very combination bound in the game) or a key in one of the player's macro triggers. Then **the game's control wins, then the macro, then the mod**: the mod's binding does not fire, and the Controls page marks its row with `!` and says who has the key.
- **Between mods on the same name**, one fires: a key the player chose beats a default key; then the lower addon id; then the lower binding id.

**On the Controls page** a mod's row behaves like the game's own rows. Giving a mod a key that a game control has frees that control or swaps the two (the page says which), and the reverse; a key in a macro trigger is refused for a mod. **A default key never takes a key in use**: if the default belongs to a game control, a macro or a mod that wins, the row starts empty and says who has the key. Cancelling Options puts mod keys back together with the game's.

**Keys that can never be mod keys.** The client keeps these for itself, with or without further modifiers: `f3`, `f11`, `f12`, `escape`, `alt+enter`, `ctrl+c`, `ctrl+v`, `ctrl+x`, and `super+c`, `super+v`, `super+x`, `super+,`. A default or saved key among them leaves the row empty, with the reason.

**When a mod's key does not fire.** Mod keys work in plain play. They do not fire while the player types in chat or a text box, while a game window or menu has the keyboard, while an ImGui text field has focus, while the character is dead, or while another mod's text editor holds the keyboard (`eqoa.input.capture_text`). Held-key repeats do not fire a binding.

**The Mods & Addons key.** The client itself has one row of this kind, **Mods & Addons**, default `f10`: it opens and closes the Mods & Addons window, follows every rule above, and exists with no mod enabled. It does nothing on the loading screen, with the full map up, or while the world changes.

Mod keys are saved by the client per player, separately from the game's own keys.

## 11. FrontiersForge compatibility

The client also runs packages written for FrontiersForge and UiForge (the `frontiers_forge.*` modules, the `UiForge` global, `ImGui`), so that existing UI packages work unchanged. That layer is kept up, and it is **not part of the `eqoa/1` promise** ([VERSIONING.md](VERSIONING.md)): its names, units and index bases are Forge's, not this page's. New mods should use `eqoa`.

What an author needs to know:

- A package without a `mod.json` (a folder with `<name>.lua`, `main.lua` or `init.lua`, or a single `.lua` file) is loaded as a Forge package, always in frame mode. It also sees the `eqoa` global for reading.
- [SUPPORT-TABLE.md](SUPPORT-TABLE.md) lists every Forge name the client answers and marks the ones that only refuse. Nothing that needs the emulator's memory exists: `frontiers_forge.cave`, `ffi`, raw addresses, window-object memory reads, pointer-based text conversion. Calling one returns nil in Forge's own shape and writes a diagnostic, so a package that probes for them keeps loading.
- Forge packages are not asked for capabilities. The three things they can do on their own (send chat lines, send slash commands, switch weapon sets) are shown to the player on every Forge package (§2).
- Forge's conventions stay Forge's: for example its bars and slots count from 0, its health is 0..255, and `Util.IsStartMenuOpen` is true for any game window.

Two older Enhanced extensions are kept as permanent aliases, with their old shapes: `eqoa.WeaponSets` (`eqoa.WeaponSets.IsAvailable`, `eqoa.WeaponSets.GetCount`, `eqoa.WeaponSets.GetSelected`, `eqoa.WeaponSets.GetIcon`, `eqoa.WeaponSets.GetBadge`, `eqoa.WeaponSets.GetName`, `eqoa.WeaponSets.IsEmpty`, `eqoa.WeaponSets.Use`; sets counted from 0; `Use` returns whether the switch was queued) and `eqoa.ChatHistory()` (returns `{active, steps}`, the polled form of the `chat.history_scroll` event). New mods use `eqoa.actions.weapon_set`, `eqoa.hotbar` and the event instead.

## 12. Round labels

`Capability.round` and the "Since round" column of the support table say in which development round a name was built. They are history: a mod should test `status` or `eqoa.revision`, never the round.

| Round | What it added |
|---|---|
| 2 | The core: client, player, entities, target, group, abilities, hotbars, cooldowns, effects, chat, combat, input; events and the life cycle; capabilities and diagnostics; ImGui, textures, fonts, sound, settings, log, private files, math helpers; the sandbox. |
| 3 | The HUD: `eqoa.hud` layout and drawing, `eqoa.options`, mod keys (`eqoa.binds`, `eqoa.input.bind`), `eqoa.player.stats`, `eqoa.windows.open`, the first two actions (`assist`, `target_target`). |
| 4 | The map: `eqoa.map` views, markers and guides, the pointer, large JSON files, sharing between mods (`shares`, `eqoa.mods.send`), import and export, option buttons. |
| 5 | Items: `eqoa.inventory`, `eqoa.bank`, `eqoa.loot`, `eqoa.merchant`, `eqoa.items`, `eqoa.gems` and their events. |
| 6 | Actions and the rest of the game's state: `eqoa.actions` and batches, `eqoa.quests`, `eqoa.mastery`, `eqoa.camera`, `eqoa.windows` and window takeover, class and disposition icons. (Round 6.5, counted as 6: declared capabilities for acting, takeover scopes, `open_loot`, `talk`, `Entity.npc_flags`.) |
| 7 | The finish before the freeze: `eqoa.files.list`, the player's waypoint (`eqoa.map.set_waypoint`, `eqoa.map.clear_waypoint`, `eqoa.map.waypoint`), the `combo` field of `input.key` and the key rules of §10, the manifest's `replaces` and the labels shown to the player, the rule that a slash line needs `eqoa.actions.command`, and the generated support table. |

## 13. What the client cannot give

The client can only report what the server sends it. There is no list of everything in a zone, only what is in view; no health, disposition or target for entities it has not been told about; no duration, stack count or caster for effects; no quest objectives or progress, only titles; no exact time remaining on a cooldown, only the lockout and the client's own estimate. Such things are nil, never guessed.

There is no access to the game's memory, no FFI, no packet API. Mods do not run in the browser build. Automation is allowed, but there is no route-finding: a mod that wants to navigate has only positions and, once built, the movement controls of §8.
