# EQOA Enhanced mod API: `eqoa/1`

This is the reference for the native Lua mod API of EQOA Enhanced, **as built**: every module, function, record, event and error a mod can rely on. It describes API version `eqoa/1`, revision 1.

- **New here?** Read [GETTING-STARTED.md](GETTING-STARTED.md) first and copy [the template](template/).
- **Stable candidate.** From Enhanced 0.7.0 `eqoa/1` is a stable candidate: a guard test stops any supported name from disappearing or changing, and we do not expect breaking changes. The final freeze comes when the API is offered to the upstream client and upstream has had its say; any change before then is listed in the release notes. From the freeze on it only grows: nothing described here is renamed, removed or changed in meaning. [VERSIONING.md](VERSIONING.md) says exactly what that covers.
- **The client is the authority.** [SUPPORT-TABLE.md](SUPPORT-TABLE.md) lists every name with its status. It is printed by the client itself (`eqoa mod-api`) and checked against the client by a test, so it cannot drift. A second test checks the names on this page against it, both ways: every name here exists there, and every name it calls supported is described here. That test does not check the arguments, results and behaviour described here; those were read from the code. Where this page and the table ever disagree, the table is right.
- **Feature-test, do not assume.** `eqoa.revision` rises when something is added. `eqoa.capabilities.has("eqoa.map.set_waypoint")` asks about one name (§2).
- A few numbers below are marked **design intent, not part of the promise**: they describe how the client is tuned today and may change. Everything else is the contract.

## 1. Conventions

**Reads are snapshots, changes are events, writes are queued actions.** A getter returns a read-only copy of the client's state as of this frame. **Freshness (owner-accepted, additive):** the client rebuilds the state mods read on the game's 30 Hz step, not on every drawn frame, so a value other than the ones below can be up to one game step (33 ms) older than the frame that reads it; at 60 or 165 frames a second several frames in a row see the same value. The player's own position, velocity, heading and speed (`eqoa.player.get()`, the minimap and camera-driven reads) are refreshed on every drawn frame, so anything that follows the player moves smoothly. Events are unchanged: they are delivered in order with `tick` and `frame` as before. To notice a change, subscribe to its event (§3) instead of comparing getters every frame. To make the game do something, call an action (§8); it is carried out at the next game step and its outcome comes back as an event.

**Names.** The API is the global `eqoa`; `require("eqoa")` returns the same table and `require("eqoa.player")` a module of it. Functions are called with a dot and named in snake_case: `eqoa.player.get()`. Modules and records are read-only: assigning to one raises an error.

**Counting starts at 1.** Every position in the API (bars, slots, tabs, rows, list positions) is 1-based. IDs are not positions: entity ids, item and ability ids, world ids, icon hashes and the game's enum numbers are passed through unchanged.

**Units.** Positions are game units, x and z on the ground and y up; north is -z and west is -x. Headings are radians in [0, 2π), 0 = north, increasing toward west. Durations are seconds. `tick` counts 30 Hz game steps; `frame` counts drawn frames. Do not time anything by counting frames.

**Types used on this page**

| Type | Meaning |
|---|---|
| `int`, `uint`, `number`, `bool`, `string` | A whole number, a whole number ≥ 0, a finite number, a boolean, a string. A wrong type or a non-finite number is refused with `invalid_argument`. |
| `T?`, `T[]` | A value that may be nil; a list of T (1-based, no gaps; an empty list is `{}`). |
| `Id` | An item-definition or ability id as a decimal string (`"1234"`), because these are 64-bit and do not fit a Lua number exactly. Every field that holds one has this type, in every round (`Item.id`, `Ability.id`, the `id` of a row of `eqoa.craft`, `eqoa.trade`, `eqoa.blacksmith`, `eqoa.mail` and `eqoa.auction`, a mercenary's `item_id` and spell ids, `Buff.spell_id`), so the same item compares equal wherever it is read. An action that takes one back (a `{slot, id}` row, `merc_spell_set`) takes the string as it was read; a Lua number there is `invalid_argument`. Other ids are whole numbers: entity, player and world ids, class, race, stat and skill numbers, recipe numbers, and the keys of a server's list (a mail, a lot, a mercenary's record, a roll, a link, a question). |
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
- these take no arguments, and refuse one with `invalid_argument`: `eqoa.client.get`, `eqoa.client.display`, `eqoa.player.get`, `eqoa.player.stats`, `eqoa.target.get`, `eqoa.group.get`, `eqoa.raid.get`, `eqoa.raid.ready_check`, `eqoa.raid.loot_rolls`, `eqoa.entities.list`, `eqoa.abilities.list`, `eqoa.hotbar.list`, `eqoa.effects.list`, `eqoa.chat.get_channel`, `eqoa.combat.get_counters`, `eqoa.input.get`, `eqoa.fonts.default`, `eqoa.settings.load`, `eqoa.actions.assist`, `eqoa.actions.target_target`, `eqoa.actions.toggle_stats_box`. Other functions ignore arguments they do not take.

**Error codes**

| Code | Meaning |
|---|---|
| `invalid_argument`, `out_of_range` | The call was wrong: a bad type or value, or a number outside its range. |
| `not_in_world` | The function needs the character to be in the game world. |
| `unavailable` | The data or service is not there yet (not loaded, not sent by the server yet, switched off by the player). Try later. |
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
| `unsupported` | The name exists so that code can load, but does nothing in `eqoa/1` on this platform. Also a well-formed request for something that is not offered: a feature this server does not have, a row, command, answer or filter the window or game does not have, or a window or session of a kind the call does not work on (the table below). |
| `not_yet` | Planned and not built (in `eqoa/1`: `eqoa.actions.move`, `look` and `buttons`, and accepting a game Yes/No box that is not yet answerable, §8), or, from `load_json`, a shared file whose owner is still starting. |
| `unknown_symbol` | A diagnostic only: the mod read a name that is not part of the API. |
| `io_error`, `failed` | A file could not be read or written; any other failure. |

**Which code for which situation.** One situation answers with one code in every round (added 2026-11-25, before the freeze; the rounds after 0.7.0 were aligned to it). What 0.7.0 shipped is kept: where a 0.7.0 function answers a situation with another code, it keeps that answer, and the last column lists it.

| Situation | Code | Kept as 0.7.0 shipped it |
|---|---|---|
| A bad argument: a wrong type (a Lua number where an `Id` string is read back, for example), a malformed value, a value outside a documented list; a number outside its range | `invalid_argument`; `out_of_range` | |
| A well-formed request for something that is not offered: a feature this server does not have (jumping, weapon sets, group search, the mercenary windows, the auction, mail that carries items, item links), a row, command, answer or filter the window or game does not have, a window or session of a kind the call does not work on | `unsupported` | `open_window("auction")` answers `not_ready` where the server offers no auction (§8). |
| The player has switched it off: **Use the game's windows**, Map, Item Links, the Buff Window | `unavailable` | `open_window("map")` answers `unsupported` while Map is turned off. |
| A read whose data has not arrived or loaded yet | `unavailable` | |
| The window or session the call needs is not open, and the call names none (a call that names one by handle is the `stale_handle` row) | `window_closed` | `eqoa.merchant.price` answers `wrong_window`; `mastery_browse` answers `not_ready`, and `mastery_buy` and `mastery_refund` share its check. |
| The game's present state does not allow the action now: dead, a game window or box is up, no valid target, data the action needs not sent yet, the game's own rule (a full list, a worn item) | `not_ready` | The window actions (`close_window`, `select_row`, `activate_row`, `dialog_answer`, `menu_command`, `input_text`) answer `busy` when the window does not have the keyboard. |
| A handle, a session `ref`, a token or a `{slot, id}` row that no longer names what it named | `stale_handle` | `eqoa.actions.status` answers `permission_denied` for another mod's `RequestId` or `BatchId`. |
| Something else holds what is needed for now: a request not yet answered, another mod's claim, the one question the client asks at a time, a full request queue, a key or controller another window holds | `busy` | |
| A protective limit: how many of a thing a mod may hold (subscriptions, windows, views); how fast; how large | `quota`; `rate_limited`; `limit_exceeded` | |
| The client never lets a mod do this: report routes, passwords, a path outside its area, a capability it lacks | `permission_denied` | Accepting a game Yes/No box that is not answerable is `not_yet` (§8). |
| Planned and not built | `not_yet` | |

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
| `replaces` | no | list of strings (at most 32) | The game windows the mod replaces completely (window kinds, §7) and the HUD pieces it replaces (`hud.<piece>`). Needed for a scope `"all"` takeover; shown to the player. A name that is neither is an error. Naming a game window without also declaring `eqoa.windows.takeover` is reported as a warning and the mod still loads. |
| `provides` | no | list of short labels (at most 32, each 1 to 48 plain characters) | The windows and keys the mod adds, as the mod names them (`"Spellbook window (Shift+K)"`). Shown to the player before the mod is enabled; it grants nothing and is not checked against what the mod does. |
| `shares` | no | list of paths under `data/` (at most 32) | Private files other mods may read (§5). |
| `commands` | no | list of identifiers (at most 32) | Commands other mods may send (§5). |
| `permissions` | no | list of `"import"`, `"export"` | Asking the player for a file, writing one to the Desktop (§5). Shown to the player. |

A manifest with a missing or wrong required field, an unknown `entryMode`, or an invalid entry in one of the lists is rejected: the mod does not appear, and Mods & Addons shows why. Unknown extra fields are ignored. If a folder holds both `manifest.json` and `mod.json`, `manifest.json` is read.

A folder or a single `.lua` file without a manifest is loaded as a FrontiersForge package (§11), not as an `eqoa/1` mod.

### What the player is shown

Mods & Addons lists each mod with its name and id, its state (Disabled, Enabled, or Failed), its `capabilities` and `permissions`, and these lines, taken from the manifest alone:

| Line | Shown when |
|---|---|
| `Actions: can act on its own (...)` | The mod declares action capabilities. The list names them: `all actions`, or each action; chat reads `chat (ordinary lines)` and `command (slash lines)`; `eqoa.map.waypoint` reads `map waypoint`; `request_factions` reads `ask for faction standings` and `request_combat_record` reads `ask for the combat record`. Every Forge package shows `chat (ordinary lines), command (slash lines), weapon_set`. |
| `Provides: ...` | From `provides` (§2): the windows and keys the mod adds. Shown before `Replaces game windows`. |
| `Replaces game windows: ...`, `Not replaceable in this version: ...`, `Replaces HUD: ...` | From `replaces` (§7). |
| `Asks you first: destroy an item, destroy several items, drop a quest, give a corpse item to a raid member (master looter), abandon a corpse's loot (master looter), spend training points, buy a Class Mastery skill, refund a Class Mastery skill` | The mod declares `eqoa.actions.destroy`, `destroy_many`, `drop_quest`, `loot_assign`, `loot_abandon`, `train_stats`, `mastery_buy` or `mastery_refund` (or `eqoa.actions.*`); the line names those it declares, in this order. They never appear in `Actions: can act on its own`: the player's own Yes is asked first (§8). |
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
3. **Leaving a world** (zoning, respawn, disconnect). `world.leaving` fires first. Then come the ends the world's going causes, each told as its own event with `reason = "zoning"`: every held window session (`windows.session`, `phase = "closed"`) and the question the mod had open (`confirm.closed`), in the order they ended. Then `state.reset` (with `in_world = false`). All handles go stale; queued actions are cancelled; HUD and key claims are released. **Every takeover claim the mod holds, of every kind (§7, "Taking over a window"), ends with the world.** Subscriptions stay, so the mod carries on in the next world: `world.entered`, then `state.reset`, and the mod claims what it wants again (the bundled mods do it on `world.entered` and `state.reset`).
4. **Disable.** `disabling` fires, the settings are saved (`on_save`), then everything the mod holds is dropped: subscriptions, timers, claims, queued actions, option rows, sounds. Lua globals survive a disable: on the next enable the entry runs again with the old globals and no subscriptions, so initialise deliberately (`state = state or {}`).
5. **Reload** (the button in Mods & Addons) starts the mod in a fresh Lua state: globals are gone, saved settings return through `on_load`.

A mod that raises an error in its entry, runs out of memory or exceeds its instruction budget is stopped and marked Failed until it is reloaded.

### Subscribing

| Function | Returns |
|---|---|
| `eqoa.events.on(name:string, callback:function)` | A `Token`. `callback(event)` runs for each such event. An unknown event name is refused (`unknown_symbol`). |
| `eqoa.events.once(name:string, callback:function)` | The same, for one event only. |
| `eqoa.events.off(token:Token)` | `true` if removed, `false` if it was already gone; `stale_handle` for anything that is not this mod's token. |
| `eqoa.events.after(seconds:number, callback:function)` | A `Token` for a one-shot timer. The callback gets a `timer` event at the first delivery after the time has passed. `off` cancels it. A negative or non-finite delay is `invalid_argument`; a delay too large for the clock is refused with `failed`. |

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
| `entity.added` | `{ref:EntityRef, after:Entity, changed:string[]}` | An entity came into view. There is no `before`. |
| `entity.changed` | `{ref:EntityRef, before:Entity, after:Entity, changed:string[]}` | Something about an entity in view changed. |
| `entity.removed` | `{ref:EntityRef, before:Entity, changed:string[]}` | An entity left; `before` is its last known state. There is no `after`. |
| `group.changed` | `{before:Group?, after:Group?, changed:string[]}` | |
| `abilities.changed`, `hotbar.changed` | `{before, after, changed}` with lists of `Ability` / `Hotbar` | |
| `cooldown.started`, `cooldown.updated`, `cooldown.ended` | `{ability:AbilityRef?, ability_id:Id?, display_key:uint, lockout:number}` | An ability's lockout went from zero to a value, changed value, or returned to zero. |
| `effects.changed` | `{before:Effect[]?, after:Effect[]?, changed:{"records"}}` | |
| `buffs.changed` | `{before:Buff[]?, after:Buff[]?, changed:{"records"}}` | The buff window's set came, lost or gained a line, or a line's time ran out (section 4, "Frames"). A re-sent set with the same lines is not a change. |
| `cast.started`, `cast.ended` | `{caster_id:uint, caster:EntityRef?, target_id:uint?, targets:uint[], effect:uint, duration:number, own:bool}`; `cast.ended` adds `reason:string` | A cast was announced; it ended (section 4, "Frames"). |
| `quests.goals_changed` | `{before:QuestGoals[]?, after:QuestGoals[]?, changed:{"records"}}` | The quest tracker's goals changed (section 4, "Frames"). |
| `chat.message` | a `ChatMessage` (§4) | A line arrived in chat. |
| `chat.history_scroll` | `{active:bool, steps:int}` | The player pressed the chat-history keys while the game's own chat window is hidden; a mod that draws chat scrolls by `steps`. |
| `combat.hit` | a `CombatHit` (§4) | |
| `input.key`, `input.pad`, `pointer.pressed`, `pointer.wheel` | §5 | |
| `inventory.changed`, `bank.changed`, `loot.changed`, `merchant.changed` | `{before:Container?, after:Container?, changed:string[], cause:string}` | §6 |
| `quests.changed`, `mastery.changed`, `camera.changed`, `windows.changed`, `map.changed` | `{before, after, changed}` | §7 |
| `guide.lost` | §7 | A map guide ended by itself. |
| `windows.session` | `{owner, phase:"opened"\|"closed"\|"changed", ref:string?, kind:string?, origin:string?, opened_from:string?, revision:uint?, reason:string?, data:table}` | A window session of this mod (§7, "Window sessions, pass and focus") opened or ended. `reason` is set when it ended or changed: one of the end reasons in the table "What ends a session and what an event says" (§7; `"focus_lost"` comes with `phase = "changed"`). `data` is the session's `data` at that moment on all three phases: the same table as `Session.data` (§7, "What a session's `data` holds"). Only the mod that owns the session receives it; a disabled or reloaded mod receives nothing. |
| `windows.key` | `{owner, ref:string, key:"up"\|"down"\|"left"\|"right"\|"confirm"\|"back", down:bool}` | A controller button a focused window holds (§7, "Window sessions, pass and focus"). |
| `confirm.closed` | `{owner, confirm:uint, kind:"destroy"\|"destroy_many"\|"sell"\|"sell_many"\|"loot_assign"\|"loot_abandon"\|"drop_quest"\|"train_stats"\|"mastery_buy"\|"mastery_refund"\|"guild_kick"\|"friend_accept"\|"guild_accept"\|"trade_accept"\|"duel_accept"\|"revive_offer"\|"summon_offer"\|"merc_dismiss"\|"dye_pick"\|"mail_delete"\|"mail_send"\|"auction_bid"\|"auction_buyout"\|"auction_sell", answer:string, detail:table?}` | A question the client put to the player (§8; any of the kinds listed, `"dye_pick"` also for a "No dye") ended. `answer` is `"yes"`, `"no"`, `"expired"`, `"not_asked"` (a `sell_many` that needed no question; `confirm` is 0), or an end reason (the table "What ends a session and what an event says", §7). On `"yes"`, `detail = {sent:bool, reason:string?}`: whether the client then sent the step (it checks the item, quest, points or skill is still the one asked about). `"yes"` means the player said yes, not that the server did it: watch `inventory.changed`, `quests.changed`, `eqoa.player.training()` or `mastery.changed`. | For a resurrection or summons a mod accepted (`kind` `"revive_offer"` or `"summon_offer"`), the question is the one the client asks first (§7 "NPC dialogs and offers (round A2)").
| `friends.changed`, `ignored.changed` | `{before, after, changed}` | The character's friend list or ignore list changed (§7, "Friends, ignore, guild, who and group search"). |
| `guild.changed` | `{before:Guild?, after:Guild?, changed:string[]}` | Guild membership, roster, ranks or MOTD changed (§7). |
| `who.result` | `{entries:WhoEntry[], count:uint, received:uint}` | A reply to `eqoa.who.request()` arrived from the server (§7). |
| `lfg.result`, `lfg.changed` | `lfg.result`: `{entries:LfgEntry[], count:uint}`; `lfg.changed`: `{before:LfgState?, after:LfgState?, changed:string[]}` | Group search results arrived, or server LFG availability / registration state changed (§7). |
| `trade.request`, `trade.accepted`, `trade.closed` | `{kind, id, name}`; `{kind, side}`; `{kind, reason}` | A held trade request arrived; a trade's accept was sent, given or refused; a held trade or wager ended (§7, "Trade, duels and the blacksmith"). |
| `trade.changed`, `duel.changed`, `blacksmith.changed` | `{before, after, changed:string[]}` | What `eqoa.trade.get`, `eqoa.duel.get` or `eqoa.blacksmith.get` returns changed (§7). |
| `merc.changed`, `dye.changed`, `fishing.changed` | `{revision:uint, open:bool}` | What `eqoa.merc.get()`, `eqoa.dye.get()` or `eqoa.fishing.get()` returns changed (round A11, §7). `open` is whether the game's own window is up, so one stream tells a window's opening, its changes and its closing; a closing is `open = false` with the last revision. Read the state with the matching `get`. |
| `action.result`, `batch.progress`, `batch.result` | an `ActionResult` / `BatchResult` | §8 |
| `resource.ready`, `resource.failed` | `{kind:string, request?, handle?, value?, path?, error?}` | Something loaded or saved in the background finished. `kind` is `"texture"`, `"font"`, `"sound"`, `"json"`, `"json_saved"`, `"import"` or `"export"` (§5). `resource.ready` carries `handle` for a texture, font or sound (and the decoded value for `json`), `request` for the file kinds (`json`, `json_saved`, `import`, `export`), `value` only for `json` and `path` only for `import` and `export`. `resource.failed` carries `error`, whose `capability` is the function that made the request (`eqoa.textures.load`, `eqoa.textures.rgba`, `eqoa.fonts.load`, `eqoa.sound.load`, `eqoa.files.load_json`, ...). More kinds may be added. |
| `shared.changed` | `{mod:string, path:string}` | A file another mod shares was saved or removed (§5). |
| `mod.command` | `{from:string, command:string, args}` | Another mod sent a command (§5). |

Delivery limits (§9): the client keeps up to 1,024 undelivered events (2 MiB) per mod and delivers up to 256 per frame; beyond that the oldest are discarded and `events.dropped` then `state.reset` tell the mod. All of a mod's Lua in one frame shares one budget of 4 million instructions; exceeding it stops the mod. A disabled mod receives nothing.

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
| `hp_fraction:number?` | The health bar's own value, 0..1 (nil without an entity record). It can read 0 where `Entity.hp_fraction` of the same character is nil: this one is the bar, that one only a measured value. |
| `attributes:Attributes?` | `{base:SevenStats, total:SevenStats}`, where `SevenStats = {strength, stamina, agility, dexterity, wisdom, intelligence, charisma}` (ints). |
| `armor:int?`, `resists:Resists?` | Armor, and `Resists = {base, poison, disease, fire, cold, lightning, arcane}`: `base` is the base resistance, the others the bonuses on top of it. |
| `position:Vec3?`, `velocity:Vec3?`, `speed:number?`, `horizontal_speed:number?`, `heading:number?` | Where the character is and how it moves: game units, units per second, radians. nil until the character exists in the world. |
| `target_id:uint?`, `pet_id:uint?`, `has_pet:bool` | The selected target's and the pet's ids. |
| `auto_attacking:bool`, `dead:bool`, `fighting:bool?` | `fighting` is the game's in-combat flag (the one that starts battle music); nil while unknown. |
| `location:Location?` | `{world_id:int, place_id:int?, compass_id:int?, name:string?, cell_x:int, cell_z:int}`: the world, the named place and the map cell. |
| `mastery:Mastery?` | Class Mastery counters (§7). |

`Stats` holds one `StatItem` per line of the Character Summary, by name: `strength`, `stamina`, `agility`, `dexterity`, `wisdom`, `intelligence`, `charisma`, `armor`, `hp_max` (also as `max_hp`), `power` (also as `max_power`; this is maximum power, not current), `fire`, `cold`, `lightning`, `arcane`, `poison`, `disease`. The same items are also given as `rows` (all sixteen in the window's order), `columns` (the window's two columns of eight) and `resists` (the six resistances by name).

`StatItem = {name, short, word, stat_id, current, base, min, max, without_min, without_max, change, effect_change, shift}`: `name` is the key above, `short` the window's label (`"Str"`, `"AC"`, `"HPMax"`, ...), `word` the game's text id for it and `stat_id` the game's stat number; `current` is the value now; `min`/`max` are the range the game shows and `without_min`/`without_max` that range without effects (all four nil for power); `base` is the value without effects; `change` (and `effect_change`, the same number) is how far effects moved it; `shift` is `"same"`, `"raised"` or `"lowered"`.

**Heal and power over time, and the combat figures (round 8, added to `Stats`; nothing above changed).** `Stats` also holds `hot` and `pot` (whole numbers: heal over time and power over time, what the six-second tick gives back; the stat channel's numbers 12 and 13, always present when `Stats` is) and `figures`, `StatFigures = {melee:StatRange?, ranged:StatRange?, defence:StatRange?}` with `StatRange = {low:int, high:int, text:string}`. `figures` is nil unless the server has sent the Stat Window's offence and defence figures: that message is go-eqoa-server's own, a PS2 server and any server without it send none, and a server sends them only while the player's own Stat Window setting is on (F4, `/stats` or Extended > UI > HUD Extras), even when a mod is holding the window back with `hud.hide("stat_window")`. A range a server left out is nil (a range is kept only with both ends). Each range is the damage of a hit on a creature of the player's own level, as whole numbers, nothing rounded: `melee` the main hand, `ranged` the arrow of a worn ranged weapon (the game's window shows `ranged` in place of `melee` when both are present), `defence` a creature's hit on the player after AC and AGI (the window's "Taken"). `text` is the Stat Window's own text for the range: `low` alone when both ends agree, otherwise `low-high` ("12-31").

A mod that hides `stat_window` and draws its own can show every line the game's window does, in the same units, from the API alone: health and power, now and most, are `player.get().hp` and `.power` (`current`, `maximum`); HoT and PoT are `Stats.hot` and `Stats.pot`; AC is `Stats.armor.current`; each of the seven stats is `Stats.<name>.current`, with what items and spells add over the base (the window's green "+25" or "-5", blank at 0) the difference of `player.get().attributes.total` and `.base`; the six resists are `Stats.fire`, `.cold`, `.lightning`, `.arcane`, `.poison`, `.disease` (`current`, the figure the Character Summary shows: 40, plus the resist, plus a seventh of Wisdom, at most 315); the offence and defence row is `Stats.figures`. The game's tests walk the window's own lines against these fields, so a line upstream adds to the window is not released until the API gives it.

`player.stats_changed` fires (with empty data) whenever these numbers change, including changes that alter no `Player` field; read `eqoa.player.stats()` again when it does.

`Entity = {ref, id, name, kind, level, position, actor_position, world_id, hp_fraction, power, class_code, entity_type, is_creature, corpse, target_id, status_flags, poisoned, diseased, npc_flags, search_flags, disposition, disposition_name, head_mark, merc_tag}`

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
| `head_mark:string?`, `merc_tag:string?` | What the game's name plate carries besides the name (round A12): `head_mark` is the quest mark over an NPC's head, `"!"` (has a quest to take) or `"?"` (a step or hand-in), from a server that sends them and only while Extended's Quest Markers is on; `merc_tag` is a mercenary's hirer line, such as `<Lonewulf's Mercenary>`, where the server names the hirer. nil otherwise. They are the values `Play::plates` uses, for a mod that hides the plates (`eqoa.hud.hide('name_plates')`) and draws its own. |

### Group

`eqoa.group.get()` returns `Group = {in_group:bool, self_leader:bool?, members:Member[], raid:RaidMember[], invites:Invite[]}`. Outside a group it is still a record, with `in_group = false`, no members and `self_leader` nil. `members` never includes the player. `raid` is filled from the server's raid list whether or not `in_group` is true.

- `Member = {entity:EntityRef?, id:uint, name:string, active:bool, hp_fraction:number?, power:number?, position:Vec3?, world_id:int?}`. A member the client does not see keeps its place in the list with `active = false`; its position and world are nil unless the server reports them. A member can be in another world: compare `world_id` before measuring a distance. For a member the client does not see, `world_id` is the server's byte read as signed (-128..127).
- `RaidMember = {id:uint, name:string, hp_fraction:number, power_fraction:number}`; `Invite = {id:uint, name:string}`.

### Abilities, hotbars, cooldowns and effects

| Function | Returns |
|---|---|
| `eqoa.abilities.list()` | `Ability[]`, every ability the character knows, ordered by id. |
| `eqoa.abilities.get(ref:AbilityRef)` | That `Ability`, or `stale_handle`. |
| `eqoa.abilities.find(id:Id)` | The `Ability` with that id (a decimal string, as in `Ability.id`), or `nil, nil`. |
| `eqoa.abilities.in_range(ref, from:Vec3, to:Vec3)` | `RangeResult = {in_range:bool, distance:number}`: distance against the ability's range (always in range for a self ability or one with no range). Geometry only: not line of sight, not a promise the cast works. Outside the world every ability handle is stale: `stale_handle`. |
| `eqoa.hotbar.list()` | `Hotbar[]`: the bars the player has turned on, in the game's order. |
| `eqoa.cooldowns.get(ref:AbilityRef)` | `Cooldown = {active:bool, lockout:number, started_at:number?, estimated_remaining:number?}`. `lockout` is the server's lockout in seconds; `active` is `lockout > 0`. The last two are the client's own estimate from when it saw the lockout start, nil when it did not see that. |
| `eqoa.effects.list()` | `Effect[]`, at most eight. `unavailable` until the server has sent the effect list for this world; an empty list after that means no effects. |

`Ability = {ref:AbilityRef, id:Id, name:string, description:string, display_key:int, spellbook_slot:uint?, usable:bool, learned:bool, level:int, range:number, cast_time:number, power_cost:int, icon_background:uint?, icon_foreground:uint?, scope:int, cooldown:number, equipment_mask:uint, lockout:number}`. Times are seconds, `range` game units. `spellbook_slot` is the 1-based memorized slot (nil when not memorized). `usable` is the game's own "on a slot" flag (true once memorized, so it is false for an ability that is only learned: it does not mean "may be memorized"; its name is kept, eqoa/1 never renames a field). `learned` is the server's word that the character has the ability (the Abilities page lists only these, and Memorize refuses the rest); it is the test for "can be memorized". `display_key` is the server's lookup key, which can change; use `id` or `ref` for identity. `scope` is an `eqoa.abilities.scope` value: `self` 0, `target` 1, `group` 2, `pet` 3, `corpse` 4, `unknown` 5. The icon fields are resource hashes for `eqoa.icons.resource`.

`Hotbar = {index:uint, kind:string, selected:bool, selected_slot:uint?, slots:HotbarSlot[]}`; `kind` is `"abilities"`, `"items"` or `"weapon_sets"`. `HotbarSlot = {index:uint, kind:string, ability:AbilityRef?, item:ItemRef?, weapon_set:uint?, ui_icon:uint?}`; `kind` is `"empty"`, `"ability"`, `"item"`, `"weapon_set"` or `"command"`. `index` values are 1-based. (`HotbarSlot.command` and `HotbarSlot.icon` are declared `unsupported`: the game's bars do not expose a command's text; use `ui_icon` or the ability's or item's own icon.)

`Effect = {index:uint, icon:uint?, name:string}`. `index` is the 1-based slot and changes when effects shift. The server sends no duration, caster or stack count.

### Frames: effects with time left, casts, goals and more (round A3)

What a unit-frame mod needs and the client already holds. Everything here is read-only, needs no capability and, unless a row says otherwise, answers `not_in_world` outside the world. Times are seconds counted on the game's 30 Hz clock (`Client.tick`), never on frames drawn.

| Function | Returns |
|---|---|
| `eqoa.effects.buffs()` | `Buff[]`: the buff window's set, the player's effects first and then the pet's, each with its time left. `unavailable` when the server sends no buff window, when **Buff Window** is off in the player's Options, and before the first set has arrived. An empty list means no effects. The game's own effects strip is `eqoa.effects.list()`, which has no times. |
| `eqoa.effects.pet()` | `PetEffect[]`: the pet's effects strip, from servers that send it (**Pet Effects** on in Options). `unavailable` otherwise. With the buff window on, the pet's effects are also lines of `eqoa.effects.buffs()`. |
| `eqoa.casts.list()` | `Cast[]`: the casts in progress that the server announced, yours, other players' and monsters'. |
| `eqoa.casts.get(who)` | The `Cast` of that caster (an `EntityRef` or an entity id), or `nil, nil` when it is not casting. |
| `eqoa.quests.goals()` | `QuestGoals[]`: the quest tracker's goals. `unavailable` unless the server sends them and **Quest Tracker** is on in Options. |
| `eqoa.player.training()` | `Training`: your training points. |
| `eqoa.player.breath()` | `Breath`: the breath gauge. |
| `eqoa.target.info()` | `TargetInfo` for the current target, or `nil, nil` with no target. |
| `eqoa.group.raid_roles()` | `RaidRole[]`: every member of the raid list with its group, role and marks. Empty outside a raid. |

- `Buff = {index:uint, whose:string, name:string, icon:uint, spell_id:Id?, description:string?, time_left:number?, removable:bool, ref:BuffRef}`. `whose` is `"player"` or `"pet"`. `time_left` is nil where the server keeps no end for the effect. `removable` is the server's word that the player may take it off (it only allows beneficial effects). `spell_id` is nil where the server names no spell. `description` is what the client's own ability list says the effect does: the description of an ability the player has with the effect's name, so it is nil for an effect of someone else's spell the player does not have (the server's buff window carries no description). It is read when the server's set arrives. A `BuffRef` goes stale when the list changes (a line comes, goes, or its time runs out) and with the world; `eqoa.actions.remove_effect` takes one (section 8).
- `PetEffect = {index:uint, name:string, icon:uint, time_left:number}`.
- `Cast = {caster_id:uint, caster:EntityRef?, target_id:uint?, targets:uint[], effect:uint, duration:number, remaining:number, fraction:number, own:bool}`. `effect` is the game's effect id; `duration` and `remaining` are seconds, `fraction` is how much of it has passed (0..1). `own` is true for the player's own cast. **What the client is told**: a cast's start and its length. It is not told when a cast is interrupted, resisted or lands, and it does not tell the cast's ability from the effect. So a cast ends in the client's records when its time is up, when the caster begins another, when the caster leaves view, or when the player dies; `fraction` is a clock, not a result. A cast with no time (`duration` 0) is announced and not listed. **Melee rounds are not casts**: the server announces a round of fighting in the same packet as a spell (its time is the swing's delay, 3 to 6 seconds), but its effect is one of the swing effects (`Attack1` to `Attack4`, `Range1` to `Range4`), so the client reports no cast and no event for it, for the player or any other creature.
- `QuestGoals = {title:string, ability:bool, quest:QuestRef?, goals:QuestGoal[]}` and `QuestGoal = {text:string, have:uint, need:uint, done:bool}`. `need` is 0 for a thing to do with nothing to count (`done` is then false). `quest` is the journal's entry with the same title, or nil. The journal itself (`eqoa.quests.list`) still holds titles only.
- `Training = {points:int, stat_cap:int, caps:SevenStats}`: the points the Character Summary's trainer spends, the cap the level sets, and `caps`, the cap each stat is held under (the level's cap plus what Class Mastery raises that stat by: the number the trainer checks a point against). Zero points is a real value. Spending points is `eqoa.actions.train_stats` (§8).
- `Breath = {fraction:number, underwater:bool?}`: the server's breath value as 0..1 (the game draws the gauge only under water). `underwater` is nil when the client has no body to ask.
- `TargetInfo = {id:uint, title:string?, guild:string?, consider:Consider?}` and `Consider = {code:int, name:string?, color:Color?}`. The title and guild are the server's words for a player target and come with the server's description of the target (nil before it arrives, and for targets that have none). `consider` is the colour the game's own target ring takes: `name` is `"red"`, `"yellow"`, `"grey"`, `"blue"`, `"light_blue"` or `"green"` for codes 0 to 5 (`name` and `color` are nil for a code the client does not know), and nil entirely before the server has described the target. Where the colours fall on levels is the server's decision.
- `RaidRole = {id:uint, name:string, group:uint?, role:string, class_id:int, leader:bool, helper:bool, main_assist:bool, merc:bool}`. `group` is the member's group in the raid from 1, nil where the server sends none. `role` is `"tank"` (Warrior, Paladin, Shadow Knight), `"healer"` (Druid, Shaman, Cleric), `"damage"` (every other class) or `"merc"` for a mercenary, as the raid list's icons name them. The three flags are the raid's marks and need a server that sends the raid's state (all false otherwise). This is a read only; the raid's commands are not part of `eqoa/1` yet.

`cast.started` and `cast.ended` carry the fields of a `Cast` that are known then (`ended` adds `reason`: `"elapsed"`, `"replaced"`, `"gone"` or `"dead"`). A caster that begins a second cast first produces `cast.ended` with `"replaced"` for the old one. `buffs.changed` and `quests.goals_changed` carry whole lists as the other `changed` events do.

### Factions and the combat record (round A5)

Two reads of what the server sends, with an ask for each (section 8). Both answer `not_in_world` outside the world and `unavailable` until the server's answer has come.

| Function | Returns |
|---|---|
| `eqoa.factions.list()` | `FactionPage`: the page of faction standings the server last sent for the last `eqoa.actions.request_factions`. `unavailable` before a page has come, and again after a new request until its answer. A reply for an older request is dropped, as the game drops it. |
| `eqoa.combat.record()` | `CombatRecord`: the combat record as the server last sent it (on a server that keeps one; `unavailable` says which: it keeps none, or none has come yet). |

- `FactionPage = {page:uint, pages:uint, entries:Faction[]}` and `Faction = {name:string, standing:int}`. `page` counts from 1; `pages` is the page count the server gave (0 if it gave none). `standing` is the server's number; the game draws it clamped to -2000..2000, red below zero and blue above.
- `CombatRecord = {name:string, rows:CombatRow[]}` and `CombatRow = {heading:bool, label:string, value:string}`. A heading row has only a label. The rows are the server's words and figures as it sent them; the client does not interpret them. A count that changes while a record is held comes again from the server and is read again.

Neither read has an event: a mod reads them each frame, or after it has asked.

### Chat and combat

| Function | Returns |
|---|---|
| `eqoa.chat.history(limit:uint=100, before_seq:uint?)` | `ChatMessage[]`, oldest first: the latest `limit` messages (0..1000; a larger limit is refused with `invalid_argument`, not cut down), optionally only those before `before_seq`. The client keeps the last 1,000 messages of the current world (design intent, §9). |
| `eqoa.chat.tabs()`, `eqoa.chat.reply_to()` | The player's chat tabs and the last tell's sender; see "The Start menu and the chat line" (§7). |
| `eqoa.chat.get_channel()` | The channel the player's chat line sends to: `"say"`, `"group"`, `"shout"`, `"guild"` or `"other"`. Corrected 2026-11-23 as a bug fix: builds before it reported group and shout the wrong way round (the game's `/g` is channel 2 and `/s` channel 3); it now names the channel the line really goes to, as `set_channel` and the `chat_entry` session's `channel` always did. The names are unchanged; only the wrong pairing is fixed. |
| `eqoa.chat.channel_name(channel:string)` | A display label: Say, Party, Guild, Shout, Tell, System; "Unknown Message Type" for anything else. Works outside the world. |
| `eqoa.combat.get_counters()` | `Counters = {received:uint, dropped:uint}`: combat events delivered to this mod and lost to a full queue, since it was enabled. Works outside the world. |

`ChatMessage = {seq:uint, time:number, text:string, color:Color, channel:string, kind:string, links:ChatLink[]}` (`kind` and `links` are round A6: the line kind the chat tabs sort by, and the item links it carried; §7). `text` is the whole logical line, not wrapped. `channel` is `"group"`, `"system"` or `"unknown"` in this client (`unknown` covers say, shout, guild and tells, which the client's chat observer does not tell apart). **More values may be added**: a line that reads `unknown` today may get its own channel later, so treat any value you do not know as `unknown`. (`channel_code`, `sender` and `outgoing` are declared `unsupported` and read as nil.) The `chat.message` event carries one `ChatMessage` per line as it arrives; a mod enabled later can read the retained history but gets events only for new lines.

`CombatHit = {attacker_id:uint, defender_id:uint, amount:uint, kind:"damage"|"heal", color, outgoing:bool, incoming:bool, from_pet:bool, to_pet:bool, by_player:bool, from_merc:bool}`, carried by `combat.hit` for every hit the client is told about, including other people's. `amount` is the size; `kind` says which way. `color` is `{r, g, b}` as three integers 0..255 at positions 1..3 when the server sends a colour, else nil (black is a colour, not nil). The four flags compare the two ids with the player and the pet. `outgoing` is true when `attacker_id` is the player's id, and that is all it says: a server that tags its damage numbers (the Number Colors offer, on by default) tells its own pet's or mercenary's hit or heal with the player as attacker, so `outgoing` is true for those too. `by_player` (added after 0.7.0, additive) is true only when the player made it: `outgoing` and the server's tag does not say pet or mercenary. `from_merc` is true when the server's tag says the player's mercenary made it. Both are false or follow `outgoing` when the server sends no tags (the client cannot then tell them apart). A mod that writes "you" for a hit or heal should test `outgoing and by_player ~= false` so it also reads well on a client that predates the fields.

## 5. Drawing, the HUD, input and resources (rounds 2 to 4)

### The HUD

A mod can hide pieces of the game's HUD, ask where they are, and draw in the game's own HUD style.

| Function | Returns |
|---|---|
| `eqoa.hud.hide(element:string)` | A `Token` for this mod's claim to hide that piece. Only in the world (`not_in_world`); `invalid_argument` for an unknown element. |
| `eqoa.hud.show(token:Token)` | `true` if the claim was released, `false` if it was already gone; `stale_handle` for a value that is not this mod's token. It never releases another mod's claim. |
| `eqoa.hud.hidden_by(element:string)` | The ids of the mods hiding any part of that element, sorted. |
| `eqoa.hud.visible(element:string?)` | Whether a piece is being drawn now, counting the player's hide-UI key and every mod's claims. `element` is one of the names below, or `"hud"` (the default: the HUD as a whole), `"all"` (true while none of the original 14 HUD leaves `all` covers is hidden), `"all_pieces"` (true while none of the HUD pieces except `minimap` is hidden) or `"stats_box"` (also `"plain_play"`: the HUD's right-hand side is up, as in plain play). `"mana_bars"` always answers true. `false` outside the world and for a name it does not know. |
| `eqoa.hud.frame(size?)` | `Frame`: how the game's 640 x 448 HUD frame maps onto the window. `size = {width, height}` asks about another window size (a field left out counts as 640 or 448); with no argument, the window as it is. |
| `eqoa.hud.layout(size?)` | `Layout`: where the game's HUD pieces are now, including the player's Move HUD changes. |

Elements for `hide`, `show` and `hidden_by`: `player` (the whole player frame: health, power, both experience bars and compass together), `health`, `power`, `experience_main`, `experience_secondary`, `compass`, `target`, `group_panel`, `group_markers`, `raid_list`, `pet`, `effects`, `hotbar`, `spell_bars`, `utility_bars`, `chat`, `quest_tracker`, `dps_meter`, `breath`, `minimap`, `name_plates`, `damage_numbers`, `stat_window` (the game's own Stat Window, upstream's panel of the stat channel opened with F4 or `/stats`; rules below the table; not part of `all` or `all_pieces`), `target_rings` (round A12: the game's name plates over heads (with the quest marks "!" and "?" and a mercenary's hirer tag that belong to them), floating damage numbers and selection rings; not part of `all` or `all_pieces`, so hiding the HUD leaves them to the game unless a mod asks), `all` (the original set: `health`, `power`, `experience_main`, `experience_secondary`, `compass`, `target`, `group_panel`, `group_markers`, `pet`, `effects`, `hotbar`, and `chat`), and `all_pieces` (every HUD piece except `minimap`, which is hidden only by name — adding `raid_list`, `quest_tracker`, `dps_meter`, `breath`, `spell_bars`, and `utility_bars` to `all`). `spell_bars` hides the ability/spell bars and `utility_bars` hides the item quick slots and weapon sets in both the stacked bar layout and the classic carousel layout (drawing and clicks). Hiding a HUD piece suppresses its drawn output, clears its Move HUD bounds (except `minimap`, which keeps Move HUD dragging while hidden), removes its click areas, and stops mouse-wheel capture (`raid_list`, `dps_meter`); clicking or scrolling where a hidden piece sat does nothing. The local report button is never hidden.

A piece stays hidden while at least one enabled mod holds a claim on it. Claims end when the mod releases them, is disabled or fails, and when the world is left; a token from before is then answered with `false`. Hiding never changes the player's saved HUD options and never hides the game's dialogs, the full map or Mods & Addons.

**The game's Stat Window (`stat_window`).** The game's own Stat Window is meant to be replaced by a mod's (the bundled Stats Box does). While any enabled mod holds a claim on `stat_window`, in the PC interface and in the Controller interface alike: the window is not drawn; it takes no click (a click where it was is the world's); and its own keys do nothing: F4 (or the key the player gave the Controls page row "Stat Window") and `/stats`, with `on`, `off` or none, change nothing, play no sound and say nothing. A press of F4 is not consumed by the hidden window, so a mod the player has given F4 to (a player's own capture; a mod's default key never takes a key the game has, §10) receives it; with no mod on it, nothing happens. If the window was open when the claim began, it stops being drawn at once (no stuck focus, no half-open state) and the player's own on/off choice for it is kept untouched, so it is open again, where it was, the moment the claim ends. The claim ends when the mod releases it with `eqoa.hud.show`, is disabled or fails, when the world is left, and when the player turns on "Use the game's windows"; the game's window then works as vanilla. With no claim the client behaves exactly as the game's own. Declare `hud.stat_window` in `replaces` (§2). The game's Focus window is not an element and is not affected.

`Frame = {width, height, offset_x, offset_y, scale_x, scale_y, side, top, window_width, window_height}`: `width` and `height` are 640 and 448; a frame coordinate maps to window pixels as `offset + coordinate * scale`; `side` and `top` are the margins of the 4:3 area.

`Layout = {player:PlayerLayout, target:TargetLayout, pet:SinglePanelLayout, group_members:GroupMemberLayout[], minimap:Rect, hud_scale:number, pc_mode:bool, hud_unlocked:bool, quests:Rect?, meter:Rect?, effects:Rect?, chat:Rect?, chat1:Rect?, chat2:Rect?, chat3:Rect?, settings:Rect?, breath:Rect?, quick_menus:Rect?, spell_bars:Rect?, utility_bars:Rect?, equipment_bar:Rect?, target_info:Rect?, raid_prompts:Rect?, stats:Rect?, focus:Rect?}` with `PlayerLayout = {panel, health, power, exp}`, `TargetLayout = {panel, health, power}`, `SinglePanelLayout = {panel}`, `GroupMemberLayout = {index, panel, health, power}` (four entries), each part a `Rect = {x, y, width, height, frame_x, frame_y, frame_width, frame_height}`. `x`, `y`, `width`, `height` are window pixels (the same units as the pointer, below); the `frame_` fields are the same rectangle in the game's HUD frame, for use with the drawing calls below. The rectangles are where the pieces are drawn now: moved by Move HUD and, since the 2026-11-25 sync, grown about the piece's corner by HUD Scale (both interfaces) and the PC interface's per-piece size, so a mod that places things on a piece measures from the rectangle (its `frame_width` over the piece's width at 100% is the growth) and does not add fixed offsets to its corner. Round 8 added to `Layout` (nothing above changed): `hud_scale` is the HUD Scale of the interface in use as a factor (1.0 at 100%, 1.5 at 150%; it is what the rectangles above are already grown by, so a mod that draws with `scale` below gives the rectangle's own `frame_width / width-at-100%`); `pc_mode` is true in the PC interface and false in the Controller's; `hud_unlocked` is true while Move HUD is on (the player is dragging pieces). The other pieces of the game's HUD are named as the game names them, in lower case with underscores: `quests` (quest tracker), `meter` (DPS meter), `effects`, `chat` and `chat1` to `chat3` (the main chat box and its three panes), `settings`, `breath`, `quick_menus`, `spell_bars`, `utility_bars`, `equipment_bar` (the PC weapon-set bar), `target_info`, `raid_prompts`, `stats` (the game's Stat Window) and `focus` (the focus target's window). Each is a `Rect` of where the game drew that piece in its last frame, moved and grown as drawn, and is there only while the game draws it (a piece that is not showing has no key, so test for nil; a piece hidden by `hud.hide` is not drawn and has none). Their `frame_` fields are in the piece's own drawing coordinates, the same as the five above. No field is ever nil or NaN for the fields listed without `?`, including for a window of no size.

Drawing in the HUD's own style, in the game's HUD pass (under the game's windows). Each call queues one thing for this frame, so call them every frame from `render`. All four are for the world only (`not_in_world`), return `true`, and give `limit_exceeded` beyond 256 calls per mod per frame (§9).

| Function | Draws |
|---|---|
| `eqoa.hud.text(spec)` | Text in the game's own HUD font. `spec = {x, y, text, color?, scale?, align?, outline?, shadow?, anchor?, vertical?, clip_width?, target_fade?}`: position in HUD-frame coordinates; `color` is a list of three or four numbers 0..1 (red, green, blue, alpha; default white); `scale` defaults to 1; `align` and `anchor` are `"left"` (the default), `"center"` or `"right"`; `vertical` is `"top"` (the default), `"center"` or `"bottom"`; `outline` and `shadow` (default false) add a black edge or drop shadow. Instead of `x`, `y`: `col` (1..2) and `row` (1..8) place the text in the Stats Box's grid. |
| `eqoa.hud.gauge(spec)` | One of the game's bar gauges. `spec = {x, y, art, fill, lit?, scale?, anchor?, vertical?}`: `art` is the game's gauge art number (7 health, 9 power), `fill` 0..1, `lit` (default true) draws the gauge lit; `scale` (round 8; a number above 0, default 1; anything else is `invalid_argument`) draws it grown about its own top left corner `x`, `y`, so it is `136 * scale` wide: to sit on a piece the player has scaled, pass the piece's rectangle (`eqoa.hud.layout()`) corner as `x`, `y` and its `frame_width / 136` as `scale`. Without `scale` the draw is exactly what it was. |
| `eqoa.hud.image(spec)` | A piece of the game's UI art, or a texture of the mod's. Game art: `{ui_id:int, x, y, w?, h?, color?, scale?, anchor?, vertical?, target_fade?}`; `w` and `h` default to the art's own size; `scale` (round 8; a number above 0, default 1; otherwise `invalid_argument`) draws the art, sized by `w` and `h` as before, grown about its own top left corner `x`, `y` (game art only: a texture is sized with `w` and `h`); `ui_id = "panel"` draws a window backing (148 by 132 unless `w`, `h` are given). A texture: `{texture:TextureInfo, x, y, w?, h?, shape?, anchor?, vertical?}` (`width`, `height` are accepted for `w`, `h`; the size defaults to the texture's own) with `shape = "square"` (default) or `"circle"`, which cuts the picture to a circle (it must be square). `anchor` and `vertical` are as for text. |
| `eqoa.hud.panel(spec)` | The game's window backing and border: `{x, y, w, h, anchor?, vertical?}`. |

`target_fade = true` on text and images fades them with the game's target plate when the target changes.

### Options and keys shown to the player

| Function | Returns |
|---|---|
| `eqoa.options.toggle({id:string, label:string?, default:bool?})` | Adds an on/off row for this mod in the game's options and returns `Toggle = {id, label, default, value, item_id}`. `label` defaults to `id` and `default` to false. `value` is the saved setting, or `default` when nothing is saved. |
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
| `eqoa.input.pointer()` | `Pointer = {x:number, y:number, inside_window:bool}`: the mouse in window pixels. `inside_window` is `false` while the pointer is outside the game window, and also while another window is in front of this mod's claimed window at the pointer (see `claim_pointer`): a mod draws no hover or tooltip there. |
| `eqoa.input.claim_pointer(rect)` | Keeps clicks and the wheel inside `rect = {x, y, width, height}` (window pixels) from the game for this frame and the next; call it every frame for each area. A `Rect` from `eqoa.hud.layout()` can be passed as it is. At most 32 areas per mod per frame (`quota`; design intent). Claims end when the pointer leaves the window or the window loses focus. The player's Move HUD drag goes first. **Windows have an order.** Claimed windows are ordered by Dear ImGui's own display order, the order the windows are drawn in (a window that opens later is in front; a press on a claimed window brings it to the front). A click or wheel turn at a point goes to the front-most claimed window there and to nothing behind it (other mods, the game); a real ImGui window (`ImGui.Begin` with inputs) in front of a claimed window takes the click alone, and a claimed window in front of a real ImGui window hides the pointer from it. Call `claim_pointer` from inside the window's own `ImGui.Begin` ... `End` so the claim takes that window's place. |
| `eqoa.input.minimap_wheel(steps:number)` | Passes wheel steps to the game's minimap zoom. More than 20 either way in one call is `invalid_argument` (design intent, §9). |

`Pad = {buttons:string[], left:Vec2, right:Vec2, raw_left:RawAxes, raw_right:RawAxes}`: `buttons` lists the pressed buttons by name (`select`, `l3`, `r3`, `start`, `up`, `right`, `down`, `left`, `l2`, `r2`, `l1`, `r1`, `triangle`, `circle`, `cross`, `square`); the sticks are -1..1 with right and down positive; `RawAxes = {x, y}` are the game's own 0..255 values (128 at rest).

Input events:

- `input.key`: `{key:string, combo:string, down:bool, repeat:bool, synthetic:bool, text:string?}` for a key press or release in plain play. `key` is the key's name; `combo` is the full name of the press, with the modifiers held (§10); a release carries the names its press had. `text` is the character typed, only while this mod holds `capture_text`. Keys the game or ImGui has taken (a game window, the chat line, a focused ImGui text field) are not delivered.
- `input.pad`: `{before:Pad?, after:Pad?}` when the controller state changes.
- `pointer.pressed`: `{x, y, inside_window, button, ctrl, shift, alt}` with `button` `"left"`, `"right"`, `"middle"` or `"other"`; `pointer.wheel`: `{x, y, inside_window, steps, ctrl, shift, alt}`. Both go to every listening mod for input anywhere in the window, except that a click or wheel turn on a window some mod claimed with `eqoa.input.claim_pointer` goes only to the mod whose claimed window is in front there (the one behind it and every other mod hear nothing); a point no mod claimed goes to every listening mod. Touch is not reported. `ctrl`, `shift` and `alt` (added 2026-11-23 for the Bag Viewer's multi-select) are booleans: whether that modifier was held when the press or wheel step happened, as the window last said (a Ctrl+click is a left press with `ctrl = true`).

### Drawing: ImGui is the API

Mods draw windows and shapes with [Dear ImGui](https://github.com/ocornut/imgui) through the global `ImGui` (also reachable as `eqoa.imgui`; the two are the same object). Drawing is legal only inside a `render` or `settings.render` handler (or, in frame mode, the entry). Positions and sizes are window pixels; `eqoa.client.display()` gives the size. The functions keep ImGui's own names, argument order, overloads, enum numbers and return order as FrontiersForge's Lua binding has them; `ImVec2.new(x, y)` and `ImVec4.new(x, y, z, w)` make vectors, and `eqoa.imgui.vec2(x, y)` and `eqoa.imgui.vec4(x, y, z, w)` are the same constructors.

The bridge is a fixed set of functions, not all of Dear ImGui. These are supported:

| Area | Names |
|---|---|
| Windows and layout | Begin, End, BeginChild, EndChild, BeginMenuBar, EndMenuBar, BeginMainMenuBar, EndMainMenuBar, BeginMenu, EndMenu, MenuItem, BeginTabBar, EndTabBar, BeginTabItem, EndTabItem, BeginGroup, EndGroup, Indent, Unindent, SameLine, Dummy, SetCursorPos, SetCursorScreenPos, GetCursorPos, GetCursorScreenPos, GetContentRegionAvail, GetWindowPos, SetWindowPos, SetNextWindowPos, SetNextWindowSize, GetScrollY, GetScrollMaxY, SetScrollY, SetScrollHereY, IsWindowFocused, IsWindowHovered, PushID, PopID, PushClipRect, PopClipRect, SetNextItemWidth |
| Text and widgets | Text, TextColored, TextDisabled, TextWrapped, TextUnformatted, BulletText, Separator, SetTooltip, CalcTextSize, Button, InvisibleButton, Selectable, Checkbox, RadioButton, SliderInt, SliderFloat, ColorEdit3, ColorEdit4, InputText, InputTextWithHint, InputInt, Combo, BeginCombo, EndCombo, BeginPopup, EndPopup, OpenPopup, TreeNode, TreePop, CollapsingHeader, ProgressBar, Image, PushTextWrapPos, PopTextWrapPos |
| Style and queries | PushStyleColor, PopStyleColor, PushFont, PopFont, SetWindowFontScale, GetColorU32, GetDisplaySize, GetTime, IsItemActive, IsItemHovered, IsMouseDragging, IsMouseDown, IsMouseReleased, GetMouseDragDelta, IsMouseHoveringRect, SetKeyboardFocusHere, GetFrameHeight, GetTextLineHeight, GetTextLineHeightWithSpacing, GetBackgroundDrawList, GetWindowDrawList |

These exist only as refusals (`unsupported`, with a diagnostic): BeginTooltip, EndTooltip, BeginItemTooltip, GetForegroundDrawList, PushStyleVar, PopStyleVar, PushItemWidth, PopItemWidth, SetNextWindowFocus, SetWindowFocus, and the mouse functions GetMousePos, GetMousePosOnOpeningCurrentPopup, GetMouseWheel, ResetMouseDragDelta, IsMouseClicked, IsMouseDoubleClicked, IsMousePosValid, SetMousePos and SetMouseCursor. For the mouse's position use `eqoa.input.pointer()` and the pointer events. Any other ImGui name is an unknown key (nil, with a diagnostic).

**Mouse buttons and drags.** `IsMouseDragging(button?, threshold?)`, and (added 2026-11-23 for the Spellbook's drag and drop) `IsMouseDown(button?)`, `IsMouseReleased(button?)` and `GetMouseDragDelta(button?, lock_threshold?)`, read ImGui's own mouse state for the frame being drawn, so they work only between `Begin` and `End` of a mod's render (outside a frame they refuse like every ImGui call). `button` is 0 left, 1 right, 2 middle, 3 and 4 the extra buttons (the default is 0); any other value is an argument error. `IsMouseDown` is true while the button is held; `IsMouseReleased` is true for the one frame after it was let go; `GetMouseDragDelta` returns two numbers, how far the pointer has moved in window pixels since the button went down (still readable on the release frame, `0, 0` otherwise), and with a `lock_threshold` (default ImGui's own, about 6 pixels) it returns `0, 0` until the pointer has moved that far (`0` reports every movement). The threshold must be finite. The pointer's position is `eqoa.input.pointer()`; a press is the `pointer.pressed` event, so a mod follows a press to its release with these.

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
| `eqoa.text.localized(id:int)` | A string from the game's text table, always from its first language column (column 0), whatever language the client is set to; `nil, nil` for an id with no text; `unavailable` before the table is loaded. |
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
| `eqoa.textures.load(path:string)` | `TextureInfo` from a PNG file. Any failure (missing, not a PNG, too large) is `not_found`, with the reason as a diagnostic. JPEG is not decoded. The formats are capability flags, not Lua values (there is no `eqoa.textures.formats` table): `eqoa.capabilities.has("eqoa.textures.formats.png")` and `...rgba` are true, `...jpeg` is false (`unsupported`). |
| `eqoa.textures.rgba(bytes:string, width:uint, height:uint)` | `TextureInfo` from raw pixels: exactly `width * height * 4` bytes; each side at least 1 and at most the texture size limit (2,048; design intent, §9), `out_of_range` otherwise. |
| `eqoa.textures.release(texture:Texture?)` | `true`. Releasing nil or twice is fine. |
| `eqoa.fonts.load(path:string, size:number=16)` | `FontInfo = {font:Font, status:string, reason:string?}` for a TTF or OTF file at a pixel size above 0 and up to 256 (design intent, §9). `status` is `"pending"` (it will be ready on a later frame), `"ready"`, or `"fallback"`: the file could not be used and `font` is the default font. Use it with `ImGui.PushFont`. `unavailable` before the drawing context exists. |
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
| `eqoa.settings.save(value)` | `true` once the value is accepted. The value is checked and counted against the mod's quota during the call (a refusal comes back as `nil` plus an error, as before); `eqoa.settings.load`, the `on_save` result and a later enable see it at once. The file is written to disk in the background a moment later, so the call does not wait for the disk; disabling the mod, logging out, switching profile and quitting wait for the write first, and several saves in a row write only the last value. If the background write fails, the client records it as a diagnostic for the mod in Mods & Addons (and the log); the call had already returned `true`. |
| `eqoa.settings.on_save(callback)` | Registers the function the client calls to get the value to save (when the mod is disabled, and for the **Save profile** button in Mods & Addons). Returns a `Token`. One per mod; a second call replaces the first. |
| `eqoa.settings.on_load(callback)` | Registers the function the client calls with the saved value (when the mod is first enabled, on a profile switch, and for the **Load profile** button in Mods & Addons). Returns a `Token`. The two buttons are shown only while no named profile is active. |
| `eqoa.log.write(level:string, message:string)` | Writes a line to the game's log (`eqoa.log`) as `addon <id>: [level] message`. `level` is `"debug"`, `"info"`, `"warn"` or `"error"`; `warn` and `error` also appear in Mods & Addons. Long messages are cut. `quota` if the mod logs faster than the client drains (64 lines; design intent). |

A settings value is nil, a boolean, a finite number, a string, or a table of these whose keys are either all strings or 1..n, nested at most 32 deep and at most 1 MiB when saved (both design intent, §9). No functions, handles, metatables or cycles. Settings are kept per mod and per player profile; the mod never names a file.

### Private files

`eqoa.files` reads and writes a small private area that belongs to the mod. Paths are relative, with `/`; see §9 for what the prefixes mean.

| Function | Returns |
|---|---|
| `eqoa.files.read(path:string)` | The file's bytes as a string. `not_found`, `permission_denied`, or `quota` for a file over 1 MiB. |
| `eqoa.files.write(path:string, bytes:string)` | `true`. The file is replaced whole or not at all if the game crashes. At most 1 MiB a file and 16 MiB for the mod in total, settings included (§9; `quota`). The new file is in place when the call returns, so `read`, `exists`, `list` and `open` see it at once; the disk's own sync of it finishes in the background a moment later (a power cut or operating-system crash in the moment just after a write can lose that write or leave that file empty; the old contents are not brought back). Disabling or reloading the mod (including what its `DisableScript` writes and the files it left open), logging out, switching profile and quitting wait for it; a sync that fails is a diagnostic for the mod in Mods & Addons (and the log). The same holds for `remove` and for `open("w")` / `flush` / `close`. |
| `eqoa.files.remove(path:string)` | `true` if something was removed, `false` if there was nothing. |
| `eqoa.files.list(dir:string="", glob:string="*")` | `FileEntry[] = {path:string, kind:"file"\|"directory", size:uint?}` for the entries directly in `dir`, sorted by name; `glob` filters names with `*` and `?`. `quota` rather than a partial list when there are more than 1,024 (design intent, §9). |

#### Large JSON files

For data too big to handle in one frame, the client reads, parses and writes JSON off the frame thread.

| Function | Returns |
|---|---|
| `eqoa.files.load_json(path:string)` | A request number. The result arrives as `resource.ready {kind="json", request, value}` or `resource.failed {kind="json", request, error}`. `not_found` at once for a missing private file. At most two loads at a time per mod (`quota`). |
| `eqoa.files.save_json_async(path:string, value)` | A request number. The value is copied at the call and written later; the result is `resource.ready {kind="json_saved", request}` or `resource.failed`. A private path only. Saves to one path are merged: if you save again before the last one was written, only the newest is written and reported, so track the latest request. Saving to a second path while one is in flight is `quota`. |
| `eqoa.files.null` | The value that stands for JSON `null` in loaded data and in values to save. |

A loaded value is an ordinary, changeable Lua table; arrays (also empty ones) remember that they are arrays and are saved as arrays. A value to save follows the settings rules above, plus `eqoa.files.null`. A file may be up to 16 MiB, the top value and up to 32 levels of nesting below it, and two million values to load, and 14 MiB to save (§9). A large value is handed to Lua over several frames; how it is sliced is design intent, not part of the promise.

An ordinary mod has 8 MiB of Lua memory. A mod that lists `"large_data"` in its `capabilities` gets 128 MiB (§9); without it a large load fails with `resource.failed` and a message that says so. The mod is not disabled.

#### Sharing a file with other mods

A mod may let others read some of its private files. In `mod.json`, `"shares": ["data/journal.json"]` lists them (paths under `data/`). Another mod reads one with `eqoa.files.load_json("@<mod id>/data/journal.json")`:

- `not_yet` (quietly) while the owner is enabled but has not started yet: try again shortly;
- `permission_denied` if the owner is missing or disabled, or the path is not in its `shares`.

`shared.changed {mod, path}` fires for every mod when a shared file is saved or removed. Sharing is read-only.

#### Messages between mods

A mod lists the commands it accepts in `mod.json`: `"commands": ["import_journal"]`. `eqoa.mods.send(mod_id:string, command:string, args?)` queues one; the receiver gets `mod.command {from, command, args}` at its next event delivery. `args` is plain JSON-like data and must be small: at most 4 KiB (§9), and not deeply nested or long (the top value and up to 8 levels of nesting below it, 256 values; design intent); `quota` beyond. `unavailable` if the target is missing or disabled; `invalid_argument` if it does not declare that command. Each sender may have 4 commands waiting per receiver (§9).

#### Files the player chooses

With `"permissions": ["import"]` or `["export"]` in `mod.json` a mod may ask the player for a file or put one on the player's Desktop. These two calls are the only ones that need **the player's own act**: they work only inside the handler of a key press, a pointer press, or an `eqoa.options.button` row (`requires_player` otherwise), and each such act allows one import or export. They check the permission and the player's act before their arguments. A handler may pass the act on once with `eqoa.mods.send`, so one mod's button can start another mod's import; it cannot be passed further, and timers and `render` never have it.

| Function | Returns |
|---|---|
| `eqoa.files.import({title:string?, extensions:string[]})` | A request number. The system's file picker opens (its title is the mod's name, then `title`, which defaults to "Import file"), offering files with the given extensions (a list of 1 to 16, letters and digits only). The chosen file is copied into the mod's private area and reported as `resource.ready {kind="import", request, path}` with a path to pass to `load_json` or `read`. The copy is always named `imports/import-<n>.json`, whatever the chosen file's extension. Cancelling gives `resource.failed` with code `cancelled`. The copies are staging: the client keeps the newest three (§9) and the mod cannot write there. |
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

`Container = {kind, revision, open, items, capacity, used, remaining, full, money, view, merchant, source, master_loot}`

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
| `master_loot` | `MasterLoot?` | Loot only, while the corpse is under Master Looter rules: `MasterLoot = {seconds:uint, members:MasterLootMember[]}` with `MasterLootMember = {id:uint, name:string}` listing the eligible recipients. |

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

When `loot.changed` reports a corpse window going (`cause = "close"`) it also carries `closed_by:string`, who closed it as far as the client knows: `"player"` (the player closed the game's loot window with its own control), `"mod"` (the client ended a mod's hidden session when the mod finished, released the takeover or went away), `"server"` (the server closed the corpse: it does so after every take, then lists it again) or `"other"` (anything else seen at a frame: the player walked away, zoned, another window took over). The field is absent on every other change. It is how a mod tells a corpse the player gave up on from one that was cut short; a corpse that the mod closed itself is `"mod"`, never `"player"`. A hidden corpse window cannot be closed by the player at all, so `"player"` only happens for a window the game itself is showing.

## 7. Quests, Class Mastery, camera, windows and the map (rounds 4 and 6)

### Quests

| Function | Returns |
|---|---|
| `eqoa.quests.list()` | `Quest[]`: the journal's titles in the game's order, at most eight (the game's journal holds eight; were the client ever to hold more, the list stops at eight and the Forge quest log reports itself unavailable). `Quest = {ref:QuestRef, index:uint, title:string}`. |
| `eqoa.quests.get(ref:QuestRef)` | That `Quest`, or `stale_handle`. |

The journal holds titles only: there are no objective or progress fields in `Quest`. Where the server sends the quest tracker, `eqoa.quests.goals()` (section 4, "Frames") has the goals with how many the player has. Every `QuestRef` goes stale when the journal changes. `quests.changed` carries `{before:Quest[]?, after:Quest[]?, changed:{"records"}}`.

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

**World overlays (round A12).** A mod that draws its own name plates, damage numbers or target rings needs three things: where a world point falls on the screen, the means to hold back the game's own, and the data to draw. 

`eqoa.camera.project(position:Vec3)` returns `Projection = {x:number?, y:number?, depth:number, on_screen:bool, behind:bool}`: where the point falls on the surface the mod draws on, in the pixels of that surface (the same pixels ImGui's `GetDisplaySize` and `GetCursorScreenPos` use, y down), through the view the renderer used for the frame being drawn over, so a plate follows a head with no frame of lag. `depth` is the distance along the view's axis. `behind` is true when the point is behind the eye (then `x` and `y` are nil and `on_screen` is false); a point too large to compute (`1e308`) or so close to the eye's plane that the maths overflows is reported the same way, as behind with `depth` 0, so `depth` is always a finite number and `x` and `y` are nil only when `behind` is true; `on_screen` is true when it lies inside the view's sides and between its near and far planes (a point just off a side still gets its `x` and `y`, so a mod can pin an arrow to the edge). Positions are the game's world units, the same as `Entity.actor_position` (§4); the character's head is above its `actor_position`, and a mod chooses its own height. It is a calculation, cheap enough to call about a hundred times a frame. Refusals: `invalid_argument` (not a `{x, y, z}` of numbers, or not one argument), `not_in_world`, `unavailable` (no world frame has been drawn yet). `eqoa.camera.project_entity(entity, lift?)` projects the point the game itself puts that creature's name plate at (`entity` is an `Entity` (any table with its `id`) or the entity's id; `lift` is an optional height in game units added above that point), so a plate mod needs no head height of its own per race. It returns the same `Projection`, and refuses like `project` plus `invalid_argument` for a bad entity or `lift`, and `not_found` when the game draws no plate for it this frame (not in the scene, past its draw distance, no name, or still loading). The point is the game's anchor for that frame, 0.6 units above the creature's name node (a prop's own top), so it follows the animation. It is presentation, not game logic: the maths is plain double precision, not the game's `ee` arithmetic, and nothing the game does depends on it.

The game's own name plates, floating damage numbers and target rings are hidden with `eqoa.hud.hide('name_plates')`, `eqoa.hud.hide('damage_numbers')` and `eqoa.hud.hide('target_rings')` (§5, The HUD): a claim like any other, released with `eqoa.hud.show`, when the mod is turned off or when its Lua fails. Hiding the plates also hides the quest marks over heads and a mercenary's hirer tag, which are part of the plate; a mod that hides them draws them itself from `Entity.head_mark` and `Entity.merc_tag` (§4). Only the drawing stops: the numbers still count their time and the rings still ease. What a mod needs to draw its own: `eqoa.entities` (name, level, `hp_fraction`, `disposition`, `kind`, `actor_position`, §4), `eqoa.target.info()` for the target's title, guild and consider colour (round A3), and `combat.hit` for damage numbers (§4, Chat and combat). The head's height is given through `eqoa.camera.project_entity`, not as a field.

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

Three more kinds name the game's yes/no offers (round A2): `group_invite`, `revive_offer` and `summon_offer`. The game shows all three in its message box (mode 14, `confirm` above); they are kinds only for `eqoa.windows.takeover` and sessions (§7, "NPC dialogs and offers (round A2)"), and `Window.kind` still reads `confirm` for the box itself.

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
| `"confirm"` | `text:string`, `style:"ok"\|"yes_no"\|"cancel"\|"other"`, `style_id:int`, `tag:int`, `prompt_kind:string`, `selection:uint?`: a message box. `selection` is the lit button (1 = Yes, 2 = No in a `yes_no` box; 1 in a one-button box). `prompt_kind` says what is being asked, and what a mod may do with it (§8): `"group_invite"` (accept or decline), `"pet_dismiss"`, `"quit"`, `"guild"` (disband or leave) and `"other"` (decline only: accepting is `not_yet`), `"report"` (no synthetic input at all: `permission_denied`). **More values may be added**: a prompt that reads `other` today may get its own value later, so treat any value you do not know as `other`. |
| `"input"` | `text:string?`, `title:string`, `tag:uint`, `chat:bool`, `password:bool`: a text box. `text` is always nil for a password box. `chat` is true for the chat line. `tag` identifies the box's purpose. |

| Function | Returns |
|---|---|
| `eqoa.windows.find(kind:string)` | The first window of that kind, or `nil, nil`. |
| `eqoa.windows.get_window(ref:WindowRef)` | That window now, or `stale_handle`. |
| `eqoa.windows.format_hint(button:string, label:string)` | The game's hint text for a button, in its `\|glyph label` form. `invalid_argument` for an unknown button name. Button names: `select`, `l3`, `r3`, `start`, `up`, `right`, `down`, `left`, `l2`, `r2`, `l1`, `r1`, `triangle`, `circle`, `cross`, `square`. |
| `eqoa.windows.parse_hint(text:string)` | `ButtonHint[]` from such text, in order; plain text between glyphs comes back with `button = "text"`. |
| `eqoa.windows.open(name:string)` | Opens the Character Summary: `name` must be `"summary"`. Returns `true`; needs no capability; `not_in_world` outside the world. Other windows are opened with `eqoa.actions.open_window` (§8). |

Operating a window (rows, dialogs, text boxes) is done with actions (§8).

### Raid, ready checks and loot rolls

`eqoa.raid` reads the raid roster, marks, loot rule, and open ready-check or loot-roll strips. Reading needs no capability; answering strips and assigning master loot are actions (§8).

| Function | Returns |
|---|---|
| `eqoa.raid.get()` | `RaidState`: raid membership, marks, loot rule, roster and any open strips. |
| `eqoa.raid.ready_check()` | `ReadyCheckSession?` for the open ready check, or `nil, nil` when none is up. |
| `eqoa.raid.loot_rolls()` | `LootRollSession?` for open loot rolls, or `nil, nil` when none are up. |

`RaidState = {in_raid:bool, leader_id:uint?, helper_id:uint?, assist_id:uint?, loot_rule:string, in_combat:bool, leads:bool, helps:bool, members:RaidMember[], ready_check:ReadyCheckSession?, loot_rolls:LootRollSession?}` where `loot_rule` is `"free_for_all"`, `"need_before_greed"` or `"master_looter"`.

`RaidMember = {id:uint, name:string, hp_fraction:number, power_fraction:number, group:uint, merc:bool, leader:bool, helper:bool, assist:bool}`.

`ReadyCheckSession = {kind:"raid_ready", origin:"server", revision:uint, data:ReadyCheckData}` with `ReadyCheckData = {id:uint, asker:string, seconds_remaining:number, duration:number, deadline_ms:int, choices:string[]}`.

`LootRollSession = {kind:"raid_roll", origin:"server", revision:uint, data:LootRollData}` with `LootRollData = {rolls:LootRollItem[]}` and `LootRollItem = {id:uint, item_name:string, item_id:Id, icon:uint?, may_need:bool, choices:string[], seconds_remaining:number, duration:number, deadline_ms:int}`.

### Friends, ignore, guild, who and group search (round A7)

`eqoa.social`, `eqoa.guild`, `eqoa.who` and `eqoa.lfg` read community lists and incoming prompts. Reading needs no capability; sending searches or changing registration (`eqoa.who.request`, `eqoa.lfg.search`, `eqoa.lfg.register`, `eqoa.lfg.unregister`) requires each function's own capability and returns a `RequestId`.

| Function | Returns |
|---|---|
| `eqoa.social.friends()` | `Friend[] = {id:int, name:string, online:bool, class_id:int, class_name:string?, race_id:int, race_name:string?, level:int, world:string}`. |
| `eqoa.social.ignored()` | `Ignored[] = {id:uint, name:string}`. |
| `eqoa.social.friend_request()` | `FriendRequest = {id:uint, name:string, seconds_remaining:number, duration:number}` for an active incoming friend request (held or native), or `nil, nil`. |
| `eqoa.guild.get()` | `Guild = {in_guild:bool, name:string, motd:string, show_chat:bool, self_rank:int?, self_rank_name:string?, can_invite:bool, can_set_motd:bool, members:GuildMember[]}`. Each `GuildMember = {id:int, name:string, class_id:int, class_name:string?, race_id:int, race_name:string?, level:int, rank:int, rank_name:string, online:bool, can_promote:bool, can_demote:bool, can_kick:bool}`. |
| `eqoa.guild.invite()` | `GuildInvite = {inviter_id:uint, inviter:string, guild:string, seconds_remaining:number, duration:number}` for an active incoming guild invitation (held or native), or `nil, nil`. |
| `eqoa.who.get()` | `WhoState = {pending:bool, received:uint, entries:WhoEntry[]}` with `WhoEntry = {id:uint, name:string, race_id:int, race_name:string?, class_id:int, class_name:string?, level:int}`. |
| `eqoa.who.request()` | `RequestId`. Sends the game's Who request; results arrive via `who.result` and `eqoa.who.get()`. Needs `eqoa.who.request`. |
| `eqoa.lfg.get()` | `LfgState = {available:bool, busy:bool, registered:bool?, invites_open:bool?, members_invite:bool?, default_min_level:int, default_max_level:int, entries:LfgEntry[]}` with `LfgEntry = {id:uint, name:string, race_id:int, race_name:string?, class_id:int, class_name:string?, level:int, world_id:int, position:Vec3}`. `available` is false on servers that do not offer group search. |
| `eqoa.lfg.search(opts?)` | `RequestId`. `opts` may set `kind` (`"players"` or `"groups"`, inferred from group state when omitted), `races` (bitmask, default -1 for any), `classes` (bitmask, default -1 for any), `min_level` and `max_level` (defaulting to the game's level range). Refuses with `unsupported` on servers without group search. Needs `eqoa.lfg.search`. |
| `eqoa.lfg.register()`, `eqoa.lfg.unregister()` | `RequestId`. Registers or unregisters for group search (`unsupported` on servers without group search). Need `eqoa.lfg.register` / `eqoa.lfg.unregister`. |

### Taking over a window

A mod can keep the game's corpse, merchant or bank window from appearing while it shows the same thing its own way, or while it works unseen (selling junk, taking loot), or hide the raid ready-check (`raid_ready`), loot-roll (`raid_roll`), incoming friend request (`friend_request`) and guild invitation (`guild_invite`) prompts while it draws and answers them itself. The session with the server is real and unchanged; only the game's window or strip is hidden. `eqoa.loot.get`, `eqoa.merchant.get`, `eqoa.bank.get`, `eqoa.raid.ready_check`, `eqoa.raid.loot_rolls`, `eqoa.social.friend_request`, `eqoa.guild.invite`, and the item/raid/social actions work while hidden; the window actions do not.

| Function | Returns |
|---|---|
| `eqoa.windows.takeover(kind:string, {scope="own"\|"all"}?)` | `true` when the mod holds the claim. `kind` is `"loot"`, `"merchant"`, `"bank"`, `"raid_ready"`, `"raid_roll"`, (round A2) `"text_dialog"`, `"response_dialog"`, `"group_invite"`, `"revive_offer"`, `"summon_offer"`, (round A7) `"friend_request"` or `"guild_invite"`, (round A9) `"trade_ask"`, `"trade"`, `"duel_invite"`, `"duel"` or `"blacksmith"`, (round A11) `"mercenary_items"`, `"mercenary_spells"`, `"dye_picker"` or `"fishing"`, (round A6) `"start_menu"` or `"chat_entry"`, or (round A13) `"map"`. `"raid_ready"`, `"raid_roll"`, the five A2 kinds, the two A7 kinds, the five A9 kinds, the four A11 kinds, the two A6 kinds and `"map"` accept only `scope = "all"` (`invalid_argument` for `"own"`). Calling it again while holding the claim keeps it and renews a running hidden session's idle time. Calling it again with the other scope changes the claim's scope for sessions that start later; a hidden session already running keeps the scope it started with. |
| `eqoa.windows.release(kind:string?)` | `true` if the mod held it. With no argument, releases every kind the mod holds. |
| `eqoa.windows.taken(kind:string?)` | Whether any mod holds a claim on `kind`. With no argument: `{loot:bool, merchant:bool, bank:bool, raid_ready:bool, raid_roll:bool, text_dialog:bool, response_dialog:bool, group_invite:bool, revive_offer:bool, summon_offer:bool, friend_request:bool, guild_invite:bool, trade_ask:bool, trade:bool, duel_invite:bool, duel:bool, blacksmith:bool, mercenary_items:bool, mercenary_spells:bool, dye_picker:bool, fishing:bool, start_menu:bool, chat_entry:bool, map:bool, holders:table}`, where `holders` maps each held kind to the holding mod's id. |

While a mod holds `"raid_ready"` or `"raid_roll"`, that strip is not drawn and takes no native keys or clicks, while its expiration timer keeps running. If the holder releases the claim, is disabled or fails, or the player turns on **Use the game's windows**, the native strip draws and responds again at once.

While a mod holds `"friend_request"` or `"guild_invite"`, an incoming request is hidden before the native prompt draws and opens a server-origin window session of that kind. The client tracks the game's 45-second timeout: if the player has not answered within 45 seconds, the client refuses the request with the game's own refusal message and closes the session with `reason = "timeout"`. Releasing the claim, turning on **Use the game's windows**, or calling `eqoa.windows.pass` hands the request back to the game's own prompt (preserving remaining time) if in plain play, or refuses it cleanly if another window is up, on death, or when leaving the world so a request is never left hanging. Zoning, a teleport, a new world entry or a lost connection refuse a held request to the server too, with the game's own refusal. A mod whose `render`, `windows.key` or `windows.session` handler fails loses its claims (as when it is disabled), so a held request goes back to the game's own prompt with its remaining time and later requests are the game's.

**A claim lasts until the world changes.** Every claim, of every kind and scope (the hidden corpse, merchant and bank, the raid strips, the NPC dialogs and offers, the requests, trade, duel and blacksmith, the server windows, the Start menu, the chat line and the map), ends when the character leaves a world: a zone, a teleport, a new world entry, a lost connection. So does the hidden session it held. No event of its own says so: `world.leaving` is the signal. A mod that wants a kind in the next world claims it again on `world.entered` (or on `state.reset` with `in_world = true`); asking for a kind it already holds is a no-op that succeeds. The bundled mods do this.

**Scopes.**

- `"own"` (the default) hides only the sessions the mod itself starts with `eqoa.actions.open_loot` or `eqoa.actions.talk` (or a batch). A corpse, merchant or banker the player clicks still opens the game's window. This is the scope for helpers.
- `"all"` hides every session or strip of that kind, also the ones the player starts. It is for a mod that draws a complete replacement window or strip: the mod must draw it and give the player a way to close or answer it (`release`).

**What `takeover` checks, in this order.** The first failure is the answer:

| Check | Error code |
|---|---|
| The mod declares `eqoa.windows.takeover` | `not_declared` (no `capabilities` list) or `permission_denied` (not in the list) |
| `kind` names one of the game's windows (a `Window.kind` from the table above, an overlay, or `"raid_ready"` / `"raid_roll"`) and `scope` is `"own"` or `"all"` | `invalid_argument` |
| Scope `"own"` is not asked for a window that exists only in the client (`inventory`, `spellbook`, `quest_log`, `trainer`, `mastery`, `recipe_book`, `experiment`, `start_menu`, `chat_entry`, `map`) or a server-pushed strip (`raid_ready`, `raid_roll`) or an NPC dialog or offer (`text_dialog`, `response_dialog`, `group_invite`, `revive_offer`, `summon_offer`: the wire names no speaker, so a dialog cannot be proven to answer the mod's own `talk`): there is no session of the mod's own to start, so it can only be replaced whole | `invalid_argument` |
| This build can replace that window or strip: `loot`, `merchant`, `bank`, `raid_ready`, `raid_roll`, `text_dialog`, `response_dialog`, `group_invite`, `revive_offer`, `summon_offer`, `friend_request`, `guild_invite`, `trade_ask`, `trade`, `duel_invite`, `duel`, `blacksmith`, `mercenary_items`, `mercenary_spells`, `dye_picker`, `fishing`, `start_menu`, `chat_entry` or `map` | `unsupported` for every other window |
| For scope `"all"`: the manifest's `replaces` list names that window | `not_declared` |
| For scope `"all"`: the player's **Use the game's windows** switch is off | `unavailable` |
| No other mod holds that kind | `busy` (with a diagnostic naming the holder) |

So `not_declared` has two meanings for takeover: no `capabilities` list at all, or no matching `replaces` entry for a full replacement. `release` and `taken` need no capability and answer `invalid_argument` or `unsupported` for a kind the same way.

**Declaring it.** A mod that uses scope `"all"` lists the windows in `replaces` in `mod.json` (§2). `replaces` accepts every window kind (including `raid_ready` and `raid_roll`), so a mod written for a later client can already say what it means to replace, and the `hud.<piece>` names (`hud.target`, `hud.minimap`, `hud.player`, `hud.raid_list`, `hud.quest_tracker`, `hud.dps_meter`, `hud.breath`, `hud.spell_bars`, `hud.utility_bars`, `hud.name_plates`, `hud.damage_numbers`, `hud.target_rings`, `hud.stat_window`, ... : the `eqoa.hud.hide` element names with `hud.` in front), which are only shown to the player; HUD pieces are hidden with `eqoa.hud.hide`, not `takeover`. A mod that only uses scope `"own"` needs no `replaces`.

**What the player is shown** (Mods & Addons, from the manifest, before enabling):

- `Replaces game windows: Loot, Merchant, Bank, Ready Check, Loot Roll, Dialog, Group Invitation, Resurrection Offer, Summons` for the windows or strips in `replaces` this build can replace (`text_dialog` and `response_dialog` are both titled `Dialog`) (only when the mod also declares `eqoa.windows.takeover`);
- `Not replaceable in this version: ...` for the other windows in `replaces`, by the game's own titles;
- `Replaces HUD: ...` for `hud.<piece>` entries, by token (`hud.target` reads `Target frame`). For a mod that also declares `eqoa.windows.takeover` these are listed in the `Replaces game windows` line instead.

**The player's switch.** Mods & Addons has a checkbox, **Use the game's windows**. It is an emergency override for full replacements only, and it is remembered across restarts. Turning it on releases every scope `"all"` claim at once (the game's windows come back) and stops batches that held or used one; while it is on, `takeover(kind, {scope="all"})` is refused with `unavailable`. Scope `"own"` claims are not affected: helpers keep working.

**A hidden session**

- begins only when the server's open is known to answer the holding mod's own request. An open that cannot be tied to the request, or a window that was already open, stays the game's.
- ends when the mod's action or batch is finished, when the mod releases the claim, is disabled or fails, on zoning, and when the player walks away from where the session began. When it ends the client closes the session with the server once, quietly.
- **A scope `"own"` session that is not used ends by itself after an idle time.** Calling `eqoa.windows.takeover(kind)` again while it runs starts that time afresh. A mod that keeps showing a session (a loot list the player is reading) therefore calls `takeover` again regularly, every frame or every second, for as long as it shows it; the bundled Bag Viewer does. Both halves, the idle end and the renewal, are covered by tests and part of the contract. The idle time is 15 seconds in this client; no test pins that number and it may change, so renew far more often than that instead of counting on it. Scope `"all"` sessions have no idle time.
- A request the server never answers ends as `unknown`.

The distance (25 units) and the wait for an answer (10 seconds) are design intent, not part of the promise (§9).

**The server lists a corpse or merchant again by itself.** After each take the live server closes the corpse and lists it again; after each sale or purchase it sends the merchant's list again. While a hidden session runs, such a list stays hidden. The last one can arrive after the session has ended: for 3 seconds after a hidden session ends, a list from the same corpse or merchant is still held hidden and closed quietly. This applies only when the player has not asked for that window themselves. Known limit: if the player talks to the same merchant within those 3 seconds, or while the mod's sale is still unanswered, that first window is hidden too; talking again opens it. (The 3 seconds are design intent, not part of the promise.)

Two further limits: the game's window-open sound can play for a session that is then hidden, and a mod that changes scope while its hidden session is running does not change that session.

### Window sessions, pass and focus (round A1)

A **session** is one opening of a window a mod shows. It is the handle that belongs to the opening: a question the mod asks (§8) and a hold on the controller belong to the session and end with it. Fifteen windows are opened by the mod itself, over play, with the character free to walk and the game's own windows untouched (the `kind` list of `open_session` below): `"inventory"` (Bags), `"spellbook"`, `"quest_log"` (the Journal), `"trainer"` (the Character Summary), `"mastery"` (Class Mastery), the friends, ignore, guild, who and group search lists, the two crafting windows `"recipe_book"` and `"experiment"` (see Crafting, below) and the mail and auction windows `"mail_inbox"`, `"mail_compose"` and `"auction_search"` (see Mail and the auction, below). The `"mastery"` window has an effect of its own: while a mod holds a `"mastery"` session the client fetches the Class Mastery skill list as the game's window does when it opens (so `eqoa.mastery.skills()` has it and `mastery_browse`, `mastery_buy` and `mastery_refund` work with the game's own window closed), and lets it go when the session ends. The corpse, merchant and bank a mod holds hidden (above) are sessions too, derived by the client, and so are the NPC dialogs and offers a mod holds (round A2, below).

| Function | Returns |
|---|---|
| `eqoa.windows.open_session(kind:string, {opened_from="mod"\|"key"}?)` | An `eqoa.windows.Session = {ref:string, kind, title, origin:"mod"\|"server"\|"player", opened_from:"mod"\|"key"\|"npc", revision:uint, focused:bool, data:table}`. `origin` says who started it: the mod (`"mod"`: a window it opened itself), the server (`"server"`: a corpse, merchant, bank, dialog, offer, request or window the server opened) or the player's own key (`"player"`: the Start menu and the chat line). `ref` is a plain string that names this one opening, for this mod only: every call that takes a `ref` refuses (`stale_handle`) one that is not this mod's open session. `data` is always a table: the fields of its kind, in the table "What a session's `data` holds" below (empty for every kind that carries none). One session per mod and kind: opening again returns the same one. `kind` is `"inventory"`, `"spellbook"`, `"quest_log"`, `"trainer"`, `"mastery"`, `"buddy_list"`, `"ignore_list"`, `"guild"`, `"who"`, `"group_search"`, `"recipe_book"`, `"experiment"`, `"mail_inbox"`, `"mail_compose"` or `"auction_search"`; `invalid_argument` for a name that is no window or for `loot`, `merchant`, `bank`, `friend_request`, `guild_invite`, `start_menu`, `chat_entry`, `map`, an NPC dialog or offer kind, a trade, duel or blacksmith kind (round A9), or `mercenary_items`, `mercenary_spells`, `dye_picker` or `fishing` (round A11) (hold those with `takeover`), `unsupported` for any other window, `unavailable` while the player's **Use the game's windows** switch is on, `not_ready` while dead or out of the world, `quota` past 8 open windows. Needs `eqoa.windows.session`. |
| `eqoa.windows.session(kind:string)` | This mod's session of that window (also the held corpse, merchant, bank, friend request, guild invite, NPC dialog or offer), or nil. For a dialog or offer session the call also tells the client the mod is still looking at it (round A2, "Keeping a session"). |
| `eqoa.windows.close_session(ref)` | `true` if it closed, `false` if there was nothing to close. Ends the session's question and focus. A held corpse, merchant, bank, friend request or guild invite is closed with `release`, not here (`unsupported`). |
| `eqoa.windows.pass(ref)` | `true`: "not mine": the game's own window opens for this held session as if no mod held the kind (the server's corpse, merchant, bank list, friend request or guild invite is shown by the game; the claim stays for the next opening). Only a held corpse, merchant, bank, friend request or guild invite, or a held NPC dialog or offer (round A2: its own window opens with the same words and answers), can be passed: for a window a mod opens itself (a view of the mod's own), which hide nothing of the game's, `unsupported`. `not_ready` while a game window or box is up, the full World Map is open, or the camera is manual (the game's window cannot open from there): the call returns `nil, err`, never `true` and then nothing. The session ends with `reason = "passed"`. |
| `eqoa.windows.focus(ref, {movement=true}?)` | `true`: the window takes the controller's d-pad, confirm (cross) and back (triangle). They arrive as `windows.key` events and do not reach the game; with `movement = false` the sticks are held too. One window holds the controller at a time (`busy`); `not_ready` while a game window or box is up. Needs `eqoa.windows.focus`. |
| `eqoa.windows.release_focus(ref)` | `true` if this session held it. |
| `eqoa.windows.confirm()` | This mod's open question (§8) as an `eqoa.windows.Confirm = {id:uint, kind:"destroy"\|"destroy_many"\|"sell"\|"sell_many"\|"loot_assign"\|"loot_abandon"\|"drop_quest"\|"train_stats"\|"mastery_buy"\|"mastery_refund"\|"guild_kick"\|"friend_accept"\|"guild_accept"\|"trade_accept"\|"duel_accept"\|"revive_offer"\|"summon_offer"\|"merc_dismiss"\|"dye_pick"\|"mail_delete"\|"mail_send"\|"auction_bid"\|"auction_buyout"\|"auction_sell", text:string}` (the kinds are the same as `confirm.closed` carries; a dye picker's "No dye" is asked as `"dye_pick"`), or nil. A mod can read it; it has no way to answer it. |

**The client's question is the client's.** `"Are you sure?"` (§8) is drawn after every mod window and kept in front each frame; a mod window cannot cover it, and `ImGui.Begin` refuses any window id beginning `###eqoa-client-` (an ordinary Lua error), so a mod can neither open nor add to the client's own windows. The controller focus is suspended, too, while the full World Map is open.

**What a session's `data` holds.** `data` is always a table. These are the fields each kind's `data` has; every other kind (the windows a mod opens itself, a held corpse, merchant, bank, friend request, guild invitation, trade, duel, blacksmith and map, and the raid strips) has an empty one. All the field names below are declared on `eqoa.windows.SessionData`, so **reading a field the kind does not carry is `nil`, never a diagnostic**; a name that is in no row is `unknown_symbol`. `data` is told on `"opened"`, `"changed"` and `"closed"` events alike (a copy as it was then).

<!-- session-data-table -->
| Kind | `data` fields | Types and where they are described |
|---|---|---|
| `start_menu` | `entries` | `StartEntry[]` (The Start menu and the chat line, round A6) |
| `chat_entry` | `text`, `channel`, `reply_to`, `links`, `offer` | `text`, `channel` and `reply_to` strings (`reply_to` may be nil), `links` as `ChatLink[]`, `offer` as `LinkOffer?` (round A6) |
| `text_dialog` | `text`, `talked_to` | string; `eqoa.windows.TalkedTo = {id:uint, name:string}`, or nil (round A2) |
| `response_dialog` | `text`, `options`, `talked_to` | string; `string[]`; `eqoa.windows.TalkedTo`, or nil (round A2) |
| `group_invite` | `name`, `text` | strings: who invites, and the game's sentence (round A2) |
| `revive_offer`, `summon_offer` | `text`, `from` | strings (round A2) |
| `mercenary_items` | `available`, `own_out`, `waiting`, `inventory` | what `eqoa.merc.get()` returns, without `revision` and `spells` (round A11) |
| `mercenary_spells` | `available`, `own_out`, `waiting`, `spells` | what `eqoa.merc.get()` returns, without `revision` and `inventory` (round A11) |
| `dye_picker` | `token`, `target`, `title`, `slots`, `current`, `alpha`, `open`, `no_dye` | what `eqoa.dye.get()` returns, without `revision` (round A11) |
| `fishing` | `open`, `state`, `casting`, `ready`, `perfect_from`, `perfect_to`, `fair_from`, `fair_to`, `list`, `catch`, `message` | what `eqoa.fishing.get()` returns, without `revision` and `power` (round A11) |
<!-- /session-data-table -->

**What ends a session and what an event says.** One list of the values these fields take. A mod must treat a value it does not know as "ended" (more may be added, VERSIONING.md). None of these events or values was in a release before the one that freezes this list.

| Event and field | Values |
|---|---|
| `world.leaving.reason` | `"zone"` (a zone or a teleport), `"respawn"` (a new entry into the same world, as when the server restarts under the player), `"disconnect"` |
| `windows.session.reason`, `confirm.closed.answer` (after `"yes"`, `"no"`, `"expired"`) and `trade.closed.reason` | the end reasons below |

| End reason | The session or question ended because | Told for |
|---|---|---|
| `"closed"` | the mod closed it (`close_session`, `close_window`, a button) | every kind |
| `"released"` | the mod released the kind, or the claim went | held kinds |
| `"game_windows"` | the player turned on **Use the game's windows** | every kind |
| `"passed"` | the mod passed it to the game (`pass`) | held kinds |
| `"failed"` | a handler of the mod that holds it raised an error | the Start menu and the chat line |
| `"zoning"` | the world changed under it: a zone, a teleport, a new world entry, a lost connection | held kinds, questions |
| `"left_world"` | the character is out of the world (a logout, the game's own screens) | windows the mod opened itself, held kinds, questions |
| `"death"` | the character died | held kinds, questions |
| `"walked_away"` | the player moved 25 units from where it began | dialogs, hidden corpse, merchant and bank, fishing |
| `"idle"` | the mod had not looked at it for 120 seconds | dialogs, offers, A11 windows |
| `"answered"` | an answer was sent through `dialog_answer` or `close_window` | dialogs with several answers, plain closes, A11 windows |
| `"server"` | the server closed the window | A11 windows |
| `"withdrawn"` | the server withdrew the invitation | `group_invite` |
| `"ended"` | a newer session of the same kind took its place, or its holder went | held kinds |
| `"key"` | the Start key or the chat key was pressed again | `start_menu`, `chat_entry` |
| `"command"` | a menu command ran | `start_menu` |
| `"timeout"` | nobody answered within 45 seconds and the client refused it | friend request, guild invitation, trade request, duel invitation |
| `"in_guild"` | the invitation was refused: the player is already in a guild | guild invitation |
| `"accepted"`, `"declined"` | the player (or the mod, for an invitation it may answer) said Yes or No | group invitation, resurrection and summons offers, friend request, guild invitation, trade request, duel invitation |
| `"cancelled"`, `"completed"`, `"gone"` | a trade request or wager was cancelled, a wager was completed, or a duel invitation was withdrawn | trade, duel |
| `"focus_lost"` | the controller is the game's again (a `phase = "changed"` event, not an end) | focused windows |

**Two words for an answer, on purpose.** Every request whose whole answer is Yes or No ends `"accepted"` or `"declined"`: the group invitation, the resurrection and summons offers, the friend request, the guild invitation, the trade request and the duel invitation. `"answered"` is for the windows whose answer is a choice among options or a close (a dialog's option, a mercenary or dye window's close): it says that an answer was sent, not which. One word each, and a mod handles both in the one place it handles an ended session. (Before this was settled, a group invitation and the two offers ended `"answered"` and a refused friend request or guild invitation `"refused"`; none of that was in a release.)

**What ends a session.** The mod (`close_session`), the character leaving the world (a zone, a logout: `reason = "left_world"`), death, the player's switch **Use the game's windows** (every mod window closes at once, a question it asked is answered No, and the held corpse, merchant and bank are the game's again), the mod being disabled or reloaded or stopped by an error (nobody is told; a mod whose `render` or `windows.key` handler fails also loses the controller and its windows at once, and every mod window ends when the addon runtime stops on an error), the client leaving the world for the game's own screens (a logout, a lost connection: `reason = "left_world"`), and a `pass`. A world change (a zone, a teleport, a new world entry, a lost connection) also ends every held session (a corpse, merchant, bank, dialog, offer, request, trade or other window the mod holds), a question and the controller's focus (`reason = "zoning"`), and every takeover claim (see "Taking over a window"). Each is told to the owning mod as `windows.session` with `phase = "closed"` and its `reason`, after `world.leaving`; a mod closes its window when it hears it.

**The controller.** Focus is suspended, not ended, while a game window, a box, the update question or Mods & Addons is up: the buttons are the game's, and a button still held when the focus comes back is not a press. It ends when the window closes, the mod goes, the world changes, the character dies, or the system takes the window's focus. No controller button opens a mod window in this version: Start menu > Items, Abilities and Quests still open the game's own pages.

### The Start menu and the chat line (round A6)

Two more windows a mod can hold whole: the menu the Start key opens (`"start_menu"`) and the chat line the Enter key opens (`"chat_entry"`). Both are client-side windows with no server session, so they are held with scope `"all"` only (`"own"` is `invalid_argument`) and the manifest must list them in `replaces` (§2). The claim, the **Use the game's windows** switch, `busy` for a second mod and the release when the mod goes work as for every other kind above.

| What | How |
|---|---|
| `eqoa.windows.takeover("start_menu", {scope="all"})` | The Start key (the keyboard's and the controller's), and a game window returning to the Start menu when it closes, open the mod's menu instead of the game's. |
| `eqoa.windows.takeover("chat_entry", {scope="all"})` | Enter, the `/` key, R and T, the Start menu's chat entry and the game's own Link Item open the mod's chat line instead of the game's. **A line opened with a controller button (R3, the Start menu's chat entry) stays the game's own**: its soft keyboard and cancel button are the pad's only way to type, and a mod's text capture takes the whole pad. **Only the chat purpose of the game's edit box**: passwords, report text, and the buddy, guild, ignore and alarm boxes, and a Who list's Tell, are never given to a mod. |
| `eqoa.windows.session(kind)` / event `windows.session` | The client opens the session when the player presses the key: `Session = {ref, kind, title, origin = "player", opened_from = "key", revision, focused, data}`. `data` is the kind's own record, below (every `Session` has `data`, always a table, empty for the windows that carry none). The same key pressed while the mod's Start menu is up closes it (`reason = "key"`), as the game's own Start key closes the game's. A session ends when the mod closes it (`eqoa.windows.close_session`), when it releases the kind or is disabled (`reason = "released"`), and at every end in "What ends a session" above. |
| `eqoa.windows.open_session("start_menu")` and `("chat_entry")` | `invalid_argument`: the player's key opens them. |

A menu or a line already open when a mod claims the kind stays the game's until it closes. When a mod's handler for its window fails (`render`, `windows.key` or `windows.session` raises), every kind it holds is released at once (the Start key and Enter open the game's windows again) and the mod is told its Start menu and chat line closed (`reason = "failed"`). While the player's **Use the game's windows** switch is on, or the character is dead or out of the world, the keys open the game's own windows.

**`SessionData` of a `start_menu` session** `= {entries: StartEntry[]}`: the entries the game's own menu would list now on this server (its tables for the bank and the expansion, the TEST entry while the GM's channel is on, and the entries the client adds: Duel, Combat Record, Raid, Dismiss Mercenary), as the game builds them when it opens the menu. `StartEntry = {text:string, enabled:bool, command:string?, native:bool?, entries:StartEntry[]?}`. An entry with `entries` is a sub-menu (it has no `command`); any other has a `command`, an opaque string. `enabled = false` is an entry the game draws dimmed. `native = true` marks Items: the game's item page has no copy for a mod, so running it opens the game's own Start menu once instead (the session ends with `reason = "passed"`).

**`eqoa.actions.menu_command(ref:string, command:string)`** (capability `eqoa.actions.menu_command`) runs one entry of this mod's Start menu session, with the game's own `start_actions`: what the game's menu does when the player picks that entry. `ref` is the session's `ref` (a string; the first argument of the same action for a window row is a `WindowRef`). It completes with `detail.sent = true`; the menu ends (`reason = "command"`). `stale_handle` for a session that has closed or is not this mod's, `unsupported` for a command the session does not list, `not_ready` for a dimmed entry, a dead character or another game window being up, `permission_denied` for a report route. The entries open the game's own windows (the spell book, the quest journal, ...); those are the game's, unchanged.

**`SessionData` of a `chat_entry` session** `= {text:string, channel:string, reply_to:string?, links:ChatLink[], offer:LinkOffer?}`. `text` is what the game would have put in its box: empty for Enter, `"/reply "` for R, `"/tell "` for T, a line kept with item links. `channel` is the channel plain text goes to: `"say"`, `"group"`, `"shout"` or `"guild"`. (`eqoa.chat.get_channel` reports the same names for the same channels since the 2026-11-23 correction.) `reply_to` is who last sent the player a tell. `ChatLink = {start:uint, len:uint, list:string, key:uint, name:string}`: a link in `text`, `start` counting characters from 1; `list` is `"inventory"` or `"bank"`. `LinkOffer = {seq:uint, name:string, list:string, key:uint}`: **a link the player asked to put in the line** (the Bag Viewer's Link button, the item menu's Link Item); `seq` goes up with each, and the session is told as changed. The mod puts `name` in its own text and keeps the link; it is the mod's line, not the game's.

| Function | What it does |
|---|---|
| `eqoa.actions.chat_line(channel:string, text:string, {to:string?, links:Link[]?})` | Sends a line on a channel: `channel` is `"say"`, `"group"`, `"guild"`, `"shout"`, `"tell"` (needs `to`, one player's name) or `"reply"` (to whoever last sent a tell: `not_ready` if nobody did). `links` are up to four item links, `{start, len, item}` with an inventory or bank item handle, or `{start, len, list, key}` as a `LinkOffer` carries; `start` and `len` count characters of `text` exactly as passed (nothing is trimmed) from 1 and must lie inside it (`out_of_range`). On the channel the player's plain text already goes to, the text is sent alone, as the game's Enter line sends it (one message); on any other channel it goes as the game's command for it (`/g`, `/gu`, `/s`, `/t name`, `/r`, `/say`), the same bytes, the links counted where the game counts them. A link to an item the player no longer has refuses with `stale_handle` and sends nothing. **Capability, by the line that is sent (rule 4):** `"say"` needs `eqoa.actions.chat`; every other channel goes as a slash command and needs `eqoa.actions.command`. `text` must not start with `/` or `#` (`invalid_argument`: use `eqoa.actions.command`); the whole line, with its command word, must be shorter than 128 characters (`out_of_range`). Completes with `detail = {sent=true, links=n}`. Where the server offers no item links, the line goes without them, as the game's does. |
| `eqoa.actions.set_channel(channel:string)` | Sets the channel plain text goes to (`"say"`, `"group"`, `"shout"`, `"guild"`), as Community > Chat Modes does: the game's own message and its line in the chat. `not_ready` for group without a group of two or more, guild without a guild, or while dead. Needs `eqoa.actions.set_channel`. |
| `eqoa.actions.open_link(id:uint, name:string)` | Opens the game's own item card for a link a chat line carried (`ChatMessage.links`), as a click on the link does; the server is asked for the item. `not_found` for an id no recent line carried, `unsupported` where the server offers no item links, `unavailable` while the player has Item Links turned off. Needs `eqoa.actions.open_link`. |
| `eqoa.chat.tabs()` | `ChatTabs = {tabs:ChatTab[], kinds:string[]}`: the player's chat tabs as they set them (Extended > UI > Chat Tabs). `ChatTab = {index:uint, name:string, kinds:string[]}`; index 0 is Main, which is always there; another tab is there while a kind goes to it. `kinds` lists every line kind: `"say"`, `"tells"`, `"group"`, `"guild"`, `"shouts"`, `"combat"`, `"loot"`, `"rolls"`, `"experience"`, `"system"`. |
| `eqoa.chat.reply_to()` | Who last sent the player a tell, or nil. |

`ChatMessage` (§4) gains two fields: `kind`, the line kind the game's own chat tabs sort it by (the names above), and `links:ChatLink[]`, the item links the line carried as `{start, len, name, id}` (`start` and `len` count characters of `text` from 1; `id` is the server's id for the item, what `open_link` takes). Both are also in the `chat.message` event.

Every one of these reports what the client did, never what happened next: `sent`, never `confirmed`. Nothing is replayed.
### NPC dialogs and offers (round A2)

A mod can take over what NPCs say and the game's yes/no offers, and show them in a window of its own. Five more kinds for `eqoa.windows.takeover` (§7 above), all additive: `"text_dialog"` (an NPC's plain words, answered with Done), `"response_dialog"` (words with answers to pick), `"group_invite"`, `"revive_offer"` (a resurrection) and `"summon_offer"` (a summons). All five accept only `scope = "all"` (`invalid_argument` for `"own"`: the wire names no speaker, so the client cannot prove a dialog answers the mod's own `talk`, and an offer is never the mod's own act) and need a matching `replaces` entry in `mod.json`. The second mod to ask for a kind held by another is told `busy` with the holder's name, as for every kind. The claims end with every zoning and every world left, as every claim does ("A claim lasts until the world changes", above): a mod asks again at the start and on `world.entered` or `state.reset`.

**What happens.** While a mod holds a kind, a dialog or offer of that kind does not open the game's window. The holder gets a session instead, told as `windows.session` (`phase = "opened"`, `origin = "server"`, `opened_from = "npc"`) and readable with `eqoa.windows.session(kind)`. The session's `data` table:

| Kind | `data` |
|---|---|
| `text_dialog` | `{text:string, talked_to:{id:uint, name:string}?}` |
| `response_dialog` | `{text:string, options:string[], talked_to:{id:uint, name:string}?}`. `options` is **every** answer the server sent, in order (a server may send more than six), without the game's own More, Back and Close entries: a mod pages by itself. |
| `group_invite` | `{name:string, text:string}`: who invites, and the game's sentence. |
| `revive_offer`, `summon_offer` | `{text:string, from:string}`. |

`talked_to` is a best guess: the player's target when the words arrived. A dialog on the wire names no speaker, so it is **not proven** to be who spoke; show it as a hint, never act on it. Match an answer by its own text, never by its position.

**The game's limits are kept.** A plain-text dialog arriving while another is up waits, eleven at most (a further one is dropped, as the game drops it). A dialog with answers: one waits, and a further one is refused to the server exactly as the game does. One session per kind is up at a time; the next waiting one starts when that one ends and the game is in plain play (no window of the game up).

**Answering.** `eqoa.actions.dialog_answer(ref, option)` (§8) takes the session's `ref` as well as a `WindowRef`:

- `text_dialog`: option 1 is Done. `response_dialog`: option 1 to `#data.options`, the absolute position in `data.options`. The client puts the dialog back into the game's own dialog for that one call and presses the game's own keys, so what goes to the server is exactly what pressing the answer in the game's window sends.
- Offers: 1 is Yes, 2 is No. Declining is always sent at once. **Accepting a `group_invite` is sent at once** (a mod may accept it today). **Accepting a `revive_offer` or `summon_offer` is not sent**: the call completes with `detail = {asked=true, confirm=<id>}` once the client has put its own "Are you sure?" on the screen (§8, the same question as `destroy`), and only the player's own Yes there sends it. The mod hears the end as `confirm.closed` with `kind = "revive_offer"` or `"summon_offer"`.
- The result is `sent`, never `confirmed`. An answer is never replayed. `stale_handle` for a `ref` of a session that has ended, `out_of_range` for an option outside the dialog, `not_ready` while another game window is up and the game cannot take the answer (try again when it closes).
- `eqoa.actions.close_window(ref)` ends a session without choosing: Done for a `text_dialog`, the game's own "no choice" (Escape) for a `response_dialog`, No for an offer.
- The session ends once an answer was sent: `reason = "answered"` for a `text_dialog` or `response_dialog`, `"accepted"` for an invitation or offer answered Yes (1) and `"declined"` for one answered No (2) or closed.

**Pass.** `eqoa.windows.pass(ref)` hands the dialog or offer to the game: its own window opens with the same dialog, as if no mod held the kind, and the player answers it there (`reason = "passed"`). The claim stays for the next one. A mod that does not recognise a dialog passes it at once: the player sees the game's window, unchanged. If the game has another window up, the dialog waits for it in the game's own queue.

**A session that ends without an answer goes back to the game's window, never dropped.** That is what happens when the mod releases the kind, is disabled or reloaded or stopped by an error, when the player turns on **Use the game's windows**, and when the session is idle (below). The waiting dialogs go with it.

**Keeping a session (idle).** The client ends a dialog or offer session the mod has not touched for 120 seconds and gives the dialog to the game's window (`reason = "idle"`). A touch is `eqoa.windows.session(kind)` (cheap: call it every frame while showing the dialog), `eqoa.windows.focus`, or `eqoa.windows.takeover` again. The 120 seconds are design intent, not part of the promise (§9): touch far more often.

**Walking away and zoning.** A dialog does not hold the player in place while a mod shows it. If the player moves away from where it opened (the same 25 units as a hidden corpse or merchant session), the session ends with `reason = "walked_away"`; a `response_dialog` is answered "no choice" to the server, as the game's Escape does, and the other kinds send nothing. A zoning or world left ends it with `reason = "zoning"` or `"left_world"` and sends nothing (the conversation belongs to the world left). Both are told to the mod. **Death:** when the character dies the game closes an open dialog or box without answering it and refuses every invitation (with 3); the held dialogs end the same way (`reason = "death"`, nothing sent, nothing given back to the game's window, each waiting invitation refused with 3), and `dialog_answer` or `close_window` on a dialog session of a dead character is refused `not_ready`. A summons or resurrection offer is the exception: it is offered to the dead and is answered as before (Yes through the client's own question). A dialog or invitation that arrives while the character is dead is the game's own window, not held. **A mod that fails** (its `render`, `windows.key` or `windows.session` handler raises an error) loses its claims at once, as a disable does: what it held goes back to the game's window and the next dialog shows there immediately.

**Known limits.** The game's open sound still plays for a dialog that is held (it is queued before the client diverts it, as for a corpse or merchant). A dialog the game opens from a path that is not a server packet (the class-mastery point message) is not diverted and shows in the game's own window. What the live server does with a late or missing answer is known only from live play, so a mod that holds `all` and answers by text, and passes what it does not know, is the safe shape; a wrong match can hand in the wrong quest, which is why the answer is made by matching the dialog's own words.

**What the player is shown.** Mods & Addons shows `Replaces game windows: Dialog, ...` from `replaces` (above) and, for a mod that declares `eqoa.actions.dialog_answer`, `Can answer NPC dialogs` (§2). The buddy request, guild invitation, trade and duel prompts are not part of this round, and the quit, report and GM prompts can never be taken.
### Crafting (round A8)

A mod can show its own recipe book and experiment tray. It opens a `"recipe_book"` or `"experiment"` window session (above), reads the game's crafting state with `eqoa.craft.get`, and asks for what the game's own crafting windows ask for with the actions whose names begin `craft_` (listed below). The game's own Start menu > Tradeskills windows are unchanged, and nothing here exists while no mod holds one of the two sessions.

**Why the session matters.** The game reads the server's crafting replies (the recipe list, a recipe's detail, a combine's result and its "keep this recipe?" question) only while its own recipe book or experiment is up. While a mod holds a `"recipe_book"` or `"experiment"` session the client reads them for the mod's window instead, and hands the mod the words the game's window would have shown. The actions below are refused (`window_closed`) without such a session of the calling mod, and (`not_ready`) while the game's own crafting window is up. Opening the session copies the bags into the crafting tray's list and empties the tray, as the game's Experiment does; closing it empties the tray. While the game's own recipe book or experiment is up, the client changes nothing of the game's crafting state for a mod (the transition waits until the game's window closes), the actions are refused `not_ready`, and the replies are the game's. The tray's list follows the real bags while the window is open and no request is out: an item sold, destroyed or looted shows at the next step, and a tray entry no longer held is dropped. Only the rows the game's Experiment lists are offered (a component that is not a tool). Limits: if the player opens the game's own recipe book while a mod's combine is unanswered, the game's window takes that reply and its keep-this-recipe question is not shown to the mod; the question is one per client, so another mod holding a crafting window can see and answer it; the list and detail requests are refused `busy` while the question is unanswered.

`eqoa.craft.get()` returns an `eqoa.craft.State`, or nil when no mod crafting window is open (the client reads the state at its next game step after the session opens, so read again). Needs `eqoa.craft.get`.

| Field | Meaning |
|---|---|
| `revision` | Changes whenever any other field does. |
| `pending` | `"none"`, or the one request the server has not answered: `"list"`, `"detail"`, `"combine"`, `"create"`, `"remove"`, `"move"`. The game allows one at a time, so the actions below that send are refused `busy` while it is not `"none"`. Closing and reopening the window clears it, as the game's own does. |
| `listed_skill` | The skill whose recipe list was asked for last (0 to 5), or nil. |
| `skills` | The six tradeskills, each an `eqoa.craft.Skill = {id:0..5, name:string, level:int, rank:int}`. |
| `recipes` | The recipes of `listed_skill` as the book holds them, each an `eqoa.craft.Recipe = {id:int, name:string, level:int, skill:int, component:int, component_name:string, difficulty:0..3, difficulty_name:string, order:int}` (`component` is the book's grouping, with the game's word for it; `difficulty` is Common, Uncommon, Rare or Ultra Rare). Empty until the list the mod asked for arrives. |
| `detail` | The recipe asked about last (`craft_inspect`), or nil until its reply arrives: an `eqoa.craft.Detail = {recipe:int, name:string, description:string, rarity:0..3, count:int, level:int, ingredients, tools}`. `ingredients` are `eqoa.craft.Ingredient = {name:string, need:int, have:int}` where `have` is how many of that item the bags hold (by name, worn items not counted); `tools` are `eqoa.craft.Tool = {name:string, have:bool}`. |
| `tray` | The tray's items, in the order they were put in, each an `eqoa.craft.TrayItem = {slot:int, id:Id, name:string}`: at most 4. It is the client's own list until a combine. |
| `items` | The components in the bags that the tray can take, each an `eqoa.craft.Item = {slot:int, id:Id, name:string, quantity:int, in_tray:int}`. `slot` is 1-based and is what the tray actions take, with `id`. |
| `question` | The server's "keep this recipe?" after a combine that found one: an `eqoa.craft.Question = {text:string, recipe:int}`, or nil. |
| `notice` | What the server last said about a combine or a recipe, as an `eqoa.craft.Notice = {text:string, seq:int}` (`seq` rises with each, so a repeat of the same words is seen), or nil. Cleared when a combine is sent. |

| Action (capability of the same name) | What it does |
|---|---|
| `eqoa.actions.craft_list(skill:0..5)` | Asks the server for that skill's recipes, as the game's recipe book does on opening. The result arrives as `recipes`. |
| `eqoa.actions.craft_inspect(recipe:int)` | Asks for one recipe's detail (a recipe of the current `recipes`, else `stale_handle`). The result arrives as `detail`. |
| `eqoa.actions.craft_tray_add({slot, id})`, `eqoa.actions.craft_tray_remove({slot, id})` | Puts one of a row of `items` in the tray, or takes one out, by the game's own rules: a component only, no more of an item than the stack holds, four in all (`not_ready` with the game's own words otherwise). `stale_handle` when the row no longer holds that `id`. Nothing is sent: the tray is the client's list. |
| `eqoa.actions.craft_tray_clear()` | Empties the tray. |
| `eqoa.actions.craft_fill(picks)` | Makes the tray exactly the list of up to four `{slot, id}` picks (a row more than once puts more of it in; an empty list empties the tray). All or nothing: a pick the game would refuse leaves the tray as it was. |
| `eqoa.actions.craft_combine()` | Sends one combine of the tray (the game's own combine request) and empties the tray. `not_ready` for an empty tray, `busy` while a request is out or while the "keep this recipe?" question is unanswered (the game's box holds its window the same way). Completes with `detail.sent = true`; the answer arrives as `pending`, `notice`, `question` and `inventory.changed`. |
| `eqoa.actions.craft_keep(yes:bool)` | Answers the question: `true` sends the game's own scribe request (the recipe is kept), `false` sends nothing. `not_ready` when there is no question. |

**One request at a time, never resent.** Each acting call sends once and reports `sent`, never `confirmed`: the client has no other word than the server's reply. A combine whose reply never comes is not replayed by anything; the mod (or the player) closes and reopens the window, which clears `pending` as the game's own Experiment does, and sends again only on a new call. A mod that declares `craft_tray_add`, `craft_fill`, `craft_combine` or `craft_keep` is shown in Mods & Addons as **Crafting: can fill the crafting tray, combine what is in it, answer the keep-this-recipe question** (only the steps it declares are named). Declaring `eqoa.actions.*` grants all of them. Removing, creating and rearranging recipes (the game's own recipe book menu) are not available to a mod in this version: use the game's window.

### Mercenaries, the dye picker and fishing (round A11)

Three windows the server opens (`mercenary_items` and `mercenary_spells`, `dye_picker`, `fishing`; the table in "Reading the game's windows") can be read and worked by a mod: what they hold, and what their buttons do. **A mod can also hold them whole**, as it holds an NPC dialog (round A2): `eqoa.windows.takeover(kind, {scope="all"})` for each of the four kinds (scope `all` only, a `replaces` entry in `mod.json`, the player's **Use the game's windows** switch as for every kind). While a mod holds a kind, the window the server opens does not open as the game's: the holder gets a server-origin session (`windows.session`, `opened_from = "npc"`), whose `data` is the same record its read returns (`mercenary_items`: `inventory` and the rest of `eqoa.merc.get()` without `spells`; `mercenary_spells`: the rest with `spells`; `dye_picker`: `eqoa.dye.get()`; `fishing`: `eqoa.fishing.get()`). The data is told as `changed` when the window's state changes, and the session ends with `reason = "server"` when the server closes the window (a mercenary state with the gone flag, the dye picker's close, the end of the fishing session). Without a holder the game's windows are exactly what they were.

**Ending a held window never loses it.** `eqoa.actions.close_window(ref)` does what the game's own close of that window does, so the server's state ends as it ends there: the inventory window sends its close (0x7E73 7), the spell window sends nothing, the dye picker sends its Cancel, a fishing session sends the window's Escape; the session ends `answered`. **With a catch on the line the Escape lets the catch go (LEAVE ALL) and, as in the game, the window stays**: the call completes with `detail = {sent=true, ended=false}` and the session ends when the server stops the fishing or at a second `close_window` once the line waits (which sends the cancel). `eqoa.windows.pass(ref)`, releasing the kind, disabling or reloading the mod, a mod that fails, the player's switch and the idle limit (120 seconds without `eqoa.windows.session(kind)`, as for dialogs) hand the window to the game, whatever the window's state has been since it opened: its own window opens with the same state, and where another window or a hidden session is up it waits for plain play and opens then (several given back together open one after the other). A mercenary window opens whether or not the last state carried the open flag; the dye picker opens with its token; a fishing window opens with its catch list when something is on the line. The idle limit does not run while a fishing cast is under way or a catch is on the line, and every fishing action counts as the mod showing the window. A zoning or leaving the world ends the session, forgets the picker's token and any window that waited, and sends nothing for the mercenary and dye windows (the server's window ends with the zone); a fishing session gets the game's own cancel at the respawn, held or not; death closes them as the game does and ends them `death`. `eqoa.actions.dialog_answer` takes no option on these (`unsupported`).

**The fishing meter.** A held fishing window is not drawn, so the client runs the window's own cast logic each frame on the window's own clock: `fishing_cast()` starts the meter, `fishing_release()` is the throw, and the power is only what the game's meter shows at that moment (the quality, the splash and the cast request are the window's, `FindWater` included). A mod cannot give a power. The left stick is the mod's: it reads 0 to the window. **A held fishing window ends with the game's own cancel when the player moves more than 25 units from where it opened** (`reason = "walked_away"`), as a held dialog ends; fighting is left alone. A fishing session that starts over a hidden session is the holder's window, not cancelled. The session's `data` has no `power` (it changes every frame): read it with `eqoa.fishing.get()`.

**What a held dye picker loses.** The game's picker previews the colour on the character or pet and keeps the 2.5 second preview after OK, and OK adds the colour to its recent colours. A held picker has none of these (the mod draws its own preview, if it wants one, and keeps its own recent list): it sends the picker's OK, Cancel or No dye bytes and nothing else. A second picker opened while one is held cancels the first, as the game does.

The reads and actions below work with or without a held session. Every action is the game's own request, built by the game's own functions, sent once and reported as `sent`, never `confirmed`; nothing is replayed. The server's answer arrives as the next read. Every rule below is enforced in the client, not in a mod's Lua.

| Read | Returns |
|---|---|
| `eqoa.merc.get()` | `eqoa.merc.State = {revision, available:bool, own_out:bool, waiting:bool, inventory:Inventory?, spells:Spells?}`, or nil until the server has told the client of a mercenary window. `available` is false on servers that do not offer the windows; `own_out` is whether the player's own mercenary is out; `waiting` is true while the server has not answered the last inventory request (the game's window asks one at a time). `Inventory = {open:bool, slot:int, name, class, race, level, hp, max_hp, power, max_power, ac, stats:int[7], most:int, records:Record[]}`, where `open` is whether the game's window is up and `Record = {id:uint, kind:"worn"\|"carried"\|"heirloom"\|"stock", item_id:Id, name, quantity:int, equip_slot:int}` (`id` is the record's key in every request below; `"heirloom"` is shown only; `"stock"` is the owner's potion stock). `Spells = {open:bool, slot:int, name, class, level, set:bool, slots:Id[10], spells:Spell[]}` with `Spell = {id:Id, name, slot:int?}` (`slot` 1 to 10 where the spell is memorized, nil otherwise; `slots` holds the ten spell ids, `"0"` for an empty slot). Needs `eqoa.merc.get`. |
| `eqoa.dye.get()` | `eqoa.dye.State = {revision, token:uint, target:"character"\|"pet"\|"aura", title, slots:int[], current:uint, alpha:bool, open:true, no_dye:bool}` while the game's dye picker is up, nil otherwise. `token` is the server's token for this picker, which every answer echoes. `current` is the colour now as `0xRRGGBBAA`; `alpha` is whether the player may choose the alpha (else it is FF); `no_dye` is whether this picker offers "No dye". Needs `eqoa.dye.get`. |
| `eqoa.fishing.get()` | `eqoa.fishing.State = {revision, open:bool, state:"off"\|"waiting"\|"cast"\|"caught"\|"keeping", casting:bool, power:int, ready:bool, perfect_from, perfect_to, fair_from, fair_to, list:bool, catch:Catch[], message:string}` while a fishing session is going or its window is up, nil otherwise. `power` is the game's own meter, 0 to 120, which falls and rises by the window's own clock after a cast starts; a throw at `perfect_from` to `perfect_to` is a perfect cast and at `fair_from` to `fair_to` a fair one. `list` is whether the window offers its catch list (reel, keep and pull need it). `Catch = {slot:int, name, quantity:int}`, `slot` 1-based as `fishing_keep` takes it. `message` is the window's last line. Needs `eqoa.fishing.get`. |

| Action (capability of the same name) | What it does |
|---|---|
| `eqoa.actions.merc_give(item)` | The game's Give to Merc: `item` is an inventory item handle of an item not worn. `not_ready` while the player's mercenary is not out. |
| `eqoa.actions.merc_equip(id)`, `eqoa.actions.merc_unequip(id)`, `eqoa.actions.merc_take(id)` | Equip a carried record, take a worn one off, take a carried or worn one back, by `Record.id`. `stale_handle` for a record the mercenary no longer holds in that way. `busy` while `waiting` is true. |
| `eqoa.actions.merc_stock(id, count?)` | From the owner's potion stock into the pack: `id` of a `"stock"` record, `count` 0 (a stack, the default) to 127. |
| `eqoa.actions.merc_spell_set(slot, spell)` | Memorizes `spell` (the `Id` of one of `Spells.spells`, as given; `"0"` empties the slot) onto the mercenary's slot 1 to 10 (`out_of_range` outside; `stale_handle` for a spell not in its book). `eqoa.actions.merc_spells_clear()` empties all ten. |
| `eqoa.actions.merc_dismiss()` | **Asks the player first.** Completes with `detail = {asked=true, confirm=<id>}` once the client's own "Are you sure?" (§8) is up; only the player's Yes sends the game's own Dismiss Mercenary. The mod hears the end as `confirm.closed` with `kind = "merc_dismiss"`. Hiring a mercenary is not in the client (the server's own board does it): there is no hire action. |
| `eqoa.actions.dye_pick(token, rgba)` | **Asks the player first** (the dyer may charge): the question names the colour; only the player's Yes sends the picker's OK. `token` must be the picker's own `eqoa.dye.get().token`, echoed exactly (`stale_handle` for any other, before any question; `window_closed` with no picker up). Where the player may not choose the alpha it is sent as FF, as the picker sends it. `confirm.closed` has `kind = "dye_pick"`. |
| `eqoa.actions.dye_no_dye(token)`, `eqoa.actions.dye_cancel(token)` | The picker's "No dye", through the same question (`unsupported` where the picker offers none), and the picker's Cancel, sent at once. |
| `eqoa.actions.fishing_cast()`, `eqoa.actions.fishing_release()` | `fishing_cast` presses the window's own Cast key: the meter starts (`busy` while a cast is under way, `not_ready` unless the line is waiting). `fishing_release` presses it again: the window throws when its own meter comes round, at the power the meter then shows. **A mod cannot give a power**: the meter and its clock are the window's, and neither call takes an argument. Both complete with `detail = {pressed=true}`; the cast request goes to the server when the window throws. |
| `eqoa.actions.fishing_reel()`, `eqoa.actions.fishing_keep(slot)`, `eqoa.actions.fishing_pull()` | The catch list's own rows. `fishing_reel` is the list's top row, the game's TAKE ALL (the whole catch goes to the pack); `fishing_keep(slot)` keeps the catch in `Catch.slot` with its count; `fishing_pull` is the row after the catch, the game's LEAVE ALL (the catch is let go: nothing is taken). `not_ready` unless something is on the line and `list` is true. One at a time: a second is `busy` until the server's answer has changed the state or the catch (or the session has ended). |
| `eqoa.actions.fishing_cancel()` | The window's own Escape: cancels the session, or, with something on the line, sends the same as `fishing_pull` (LEAVE ALL: the catch is let go). |

All of them are `not_ready` while dead or out of the world; the fishing ones are `window_closed` while no fishing window is up. A mod that declares any of these is shown in Mods & Addons as **Mercenary, dye and fishing windows: can ...**, naming only the steps it declares (dismissing and applying a colour are said to ask first). Declaring `eqoa.actions.*` grants all of them.

**A window the server opens over a hidden session** (no mod holding its kind). A hidden corpse, merchant or bank session of a mod leaves the game's mode at plain play (0), so these three windows used to open over it. Now the client does what the game does with a window up: a mercenary's inventory or spells window and the dye picker wait and open when the hidden session ends (the server's open is not lost), and a fishing session that starts is cancelled to the server, as the game cancels one that starts under a window, with one chat line to the player ("Fishing stopped: a mod window was busy, cast again"). Hidden sessions include a mod's brief corpse sessions (the Bag Viewer's auto-loot), so this can happen just after a kill. With no hidden session the three behave as they always did. The client writes a `takeover:` line to the log each time.

### Mail and the auction (round A10)

A mod can show its own inbox, mail compose form and auction. It opens a `"mail_inbox"`, `"mail_compose"` or `"auction_search"` window session (above), reads the game's state with `eqoa.mail.get` and `eqoa.auction.get`, and acts through the actions listed below. The game's own Mail and Auction windows are unchanged, and nothing here exists while no mod holds one of the three sessions. A session cannot be opened while the player has chosen **Use the game's windows**, and the actions are refused `not_ready` while the game's own mail window (modes 23, 24, 28 or 29, or the buddy list's compose) or its own auction or bank window (modes 39 and 11) is up, and `window_closed` without the session they need (`auction_sell`: without the held bank).

**Why the session matters.** The game's gram client reads mail replies in every mode, so mail needs nothing more than the session to be read. The auction does not: the game clears a search page the moment it arrives unless its own search window is up. While a mod holds an `"auction_search"` session the client keeps the page for the mod instead (one line in the client's bank code). The auction is offered only where the bank offers it (`offered`); the actions are refused `unsupported` otherwise. **One request at a time, never resent.** The gram client and the auction each have one pending request, as the game has; a call that finds one out is refused `busy`. Every acting call sends once and reports `sent`, never `confirmed`. An auction request the server never answers is given up on after 15 seconds (`uncertain`: it may or may not have happened) and is never replayed, and a bid on that lot or a sale of that bank item is refused `busy` until a fresh page or list shows its state; mail has no give-up (a mail request the server never answers keeps `status` at `waiting_*` until the connection comes back); a lost connection forgets a mail send or delete the way the game does.

**The question comes first.** These steps do nothing when called: they ask the player "Are you sure?" (the client's own question, starting on No, which a mod cannot answer, skip or lengthen) and the client carries the step out only on the player's Yes, after checking again that the mail, lot or item is still what the question named: `mail_delete`, `mail_send` when the mail carries tunar or an item (a plain mail goes out at once, as in the game's compose window), `auction_bid`, `auction_buyout` and `auction_sell`. They complete with `detail.asked = true`; the answer arrives as `confirm.closed`. Mods & Addons shows them under **Asks you first**; the other steps are shown as **Mail and auction: can send plain mail, take what a mail carries, search the auction, ...**.

`eqoa.mail.get()` returns an `eqoa.mail.State`, or nil when no mod mail window is open (the inbox is asked for as the session opens; read again after a step). Needs `eqoa.mail.get`.

| Field | Meaning |
|---|---|
| `revision` | Changes whenever any other field does. |
| `offered` | Always true: mail is part of the game. |
| `status` | `"idle"`, or the one request the server has not answered (`"waiting_inbox"`, `"waiting_message"`, `"waiting_send"`, `"waiting_delete"`), or how the last one went wrong (`"send_unknown_recipient"`, `"send_mailbox_full"`, `"send_too_many_unread"`, `"send_mail_busy"`, `"send_failed"`, `"send_not_delivered"`, `"inbox_failed"`, `"read_failed"`, `"delete_failed"`). |
| `count`, `truncated` | The mails the server lists; the game keeps 100 of them, so `truncated` is true past that. |
| `new` | The server has said new mail has come. |
| `entries` | The kept mails, each an `eqoa.mail.Entry = {id:int, from:string, subject:string, date:string, read:bool, carries:bool}` (`carries`: the server says it carries tunar or an item). |
| `message` | The mail opened last (`mail_read`), once its text arrives: an `eqoa.mail.Message = {id, from, subject, date, text, to, tunar:int, items:string[], returned:bool}`; or nil. |
| `pack` | The items of the pack a mail can carry, in pack order, each an `eqoa.mail.PackItem = {slot:int, id:Id, name:string, quantity:int}` (`slot` is 1-based and is what `mail_send` takes, with `id`). |
| `carries_offer`, `postage`, `max_items` | Whether the server offers mail that carries tunar and items, the postage for one, and how many items one mail takes. |
| `tunar` | The tunar the character carries. |
| `names`, `delivered`, `refusal` | After a send: the names the server could not deliver to, whether anyone got it, and the server's words when it refused a mail that carries something. |

| Action (capability of the same name) | What it does |
|---|---|
| `eqoa.actions.mail_refresh()` | Asks for the inbox again. Needs a `"mail_inbox"` session. |
| `eqoa.actions.mail_read(id)` | Asks for one mail's text (`stale_handle` when it is not in `entries`). The result arrives as `message`. |
| `eqoa.actions.mail_delete(id)` | Asks the player first, then sends the game's own delete. The question names what the mail still carries. |
| `eqoa.actions.mail_take(id)` | Takes what a mail carries (the game's Take row): `not_ready` until the mail has been read and the server has said it carries something, `busy` while the last Take is unanswered (or 15 seconds). A mail's `id` is the whole u32 the game keeps. |
| `eqoa.actions.mail_send({to, subject, body, tunar?, item?})` | Sends a mail from a `"mail_compose"` session, with the game's own checks (a name, a subject and some text; the live forms' recipient list and limits). `item = {slot, id, count}` is a row of `pack`. Carrying tunar or an item needs a server that offers it (`unsupported` otherwise), a single name in `to` (the client cannot tell whether the server gives it to each name of a list: `invalid_argument`), a subject of one line, no more tunar than carried and no more of the stack than it holds, and asks the player first (the question names the recipient as it will be sent and the postage). |

`eqoa.auction.get()` returns an `eqoa.auction.State`, or nil when no mod auction window is open. Needs `eqoa.auction.get`.

| Field | Meaning |
|---|---|
| `revision`, `offered` | As above; `offered` is whether the bank offers the auction on this server. |
| `mode` | The list shown: `"search"`, `"lots"` (the character's own lots) or `"bids"`. |
| `pending` | `"none"`, or the request out: `"search"` (a page or a list is awaited), `"lots"`, `"bids"`, `"bid"`, `"sell"`, `"bidders"`, `"money"`, `"move"`. |
| `page` | The game's page counter: a reply for another page is dropped, and a late page leaves `pending` as it was. |
| `results` | The page shown, each an `eqoa.auction.Lot = {lot:int, name, quantity, seller, min_bid, buyout, minutes_left:int?, high_bid:int?, leader:int?, bids:int?, next_bid:int?, mine:bool}` (`high_bid` is nil with no bids; `leader` and `bids` are the disc auction's columns, `next_bid` the live one's; `minutes_left` is nil until a page has set the server's clock and below 0 for an ended lot). |
| `minutes` | The server's clock in minutes, or nil. |
| `lots`, `bids` | The character's own: `eqoa.auction.Own = {lot, name, quantity, buyout, minutes_left}` and `eqoa.auction.Bid = {lot, price, count, escrow}`. |
| `bidders_lot`, `bidders` | The lot `auction_bidders` asked about and its bidders, each an `eqoa.auction.Bidder = {name:string, bid:int}`, highest first. |
| `sellable` | The bank's items that can be put up, each an `eqoa.auction.Sellable = {slot:int, id:Id, name:string, quantity:int, value:int}` (`slot` is 1-based and is what `auction_sell` takes, with `id`; `value` is the item's value, the lowest starting price the game's form allows and its default), only while a mod holds the bank. |
| `tunar`, `banked` | Tunar carried and in the bank. |
| `stacks` | The server's auction takes a count: `auction_bid` and `auction_sell` may name a count above 1. |
| `uncertain` | The kind of the last request that got no answer in 15 seconds (`"search"`, `"lots"`, `"bids"`, `"bid"`, `"sell"`, `"bidders"`), or nil. It may have gone through: it is never resent, and the same lot is not bid on, and the same bank item not put up, until a fresh page shows the lots again. While the game's own Auction window is up its own request state is left alone. |

| Action (capability of the same name) | What it does |
|---|---|
| `eqoa.actions.auction_search({name?, seller?, class?, race?, min_level?, max_level?, min_price?, max_price?, wear?, attack?})` | A new search from the first page, built as the game's search form builds it (the same bytes as its Search press). |
| `eqoa.actions.auction_page(forward:bool)` | The next or previous page (`not_ready` with no page shown or in the lots and bids lists). |
| `eqoa.actions.auction_my_lots()`, `eqoa.actions.auction_my_bids()` | The character's own lots or bids (the lists the game asks for from its Start menu). |
| `eqoa.actions.auction_bidders(lot)` | The bidders of a lot of the page shown. |
| `eqoa.actions.auction_bid({lot, price, count?})` | Asks first, then bids `price` for each of `count` items (more than 1 only where the server's auction takes a count). Refused with no question for a lot not in the page, one's own lot, an ended lot, or what the game's own bid form cannot make: a price below the lot's minimum (the live auction's next-bid floor, the disc's high bid plus one) or above the lot's buyout, a count below the one already bid, and, on the live auction, the buyout price for less than the whole stack. The question shows the lot's buyout and, for a count above 1, the total. |
| `eqoa.actions.auction_buyout(lot)` | Asks first, then buys the lot out at its own buyout price (the whole stack where the server's auction takes a count). |
| `eqoa.actions.auction_sell({slot, id, count?, price, buyout, hours})` | Puts a bank item up (a row of `sellable`, from a mod that holds the bank with `eqoa.windows.takeover`: selling is a page of the bank window). Asks first. Refused with the game's own rule for an item that cannot be sold and with no question for what the game's own sell form cannot make: a starting price below the item's value or above 999999999, a buyout below the price or above 999999999, and a time outside 12 to 72 hours. The sale names the banker the bank was opened with. |
### The world map (round A13)

The full map the map key opens (`"map"`) is a window a mod can hold whole. It is a client-side window with no server session, so it is held with scope `"all"` only and the manifest must list it in `replaces` (§2); the claim, the **Use the game's windows** switch, `busy` for a second mod and the release when the mod goes work as for every other kind above.

| Call | What it does |
|---|---|
| `eqoa.windows.takeover("map", {scope="all"})` | The map key (the compass, the minimap's click, and `eqoa.map.open()`) opens the mod's map instead of the game's: the player gets a `map` session (`opened_from = "key"`, `origin = "player"`, `data = {}`, title `"Map"`) and the game's map never draws. The key again closes the mod's map, as it closes the game's. |
| `eqoa.windows.pass(ref)` | On a `map` session: the game's own map opens for that one opening and the mod's session ends (`windows.session` `closed` with reason `"passed"`). `not_ready` while another window of the game is up. The next key press is the mod's again. |
| `eqoa.windows.close_session(ref)` | The mod closes its own map. Nothing of the game's is involved. |
| `eqoa.windows.open_session("map")` | `invalid_argument`: the player's key opens it (a mod that wants it open calls `eqoa.map.open()`, which the holder receives as a session with `opened_from = "mod"`, in the same step, and again without closing it). |

A map already open when the mod claims the kind stays the game's until it closes. With nobody holding the kind, or with the player's switch on, the game's map behaves exactly as before. A claim released, a mod disabled or the player's switch ends the mod's map and gives the game's back. The map's data (`eqoa.map.get`, `markers`, the player's waypoint and guides) is read as always; `eqoa.map.terrain` (below) supplies the ground to draw it over.

### Trade, duels and the blacksmith (round A9)

A mod can show the game's trade request, trade window, duel invitation, duel wager and blacksmith its own way, and answer them. It holds the window with `eqoa.windows.takeover(kind, {scope = "all"})` (§7, "Taking over a window"), the kind being `"trade_ask"` (someone asks to trade), `"trade"` (the trade window), `"duel_invite"` (a duel invitation), `"duel"` (the duel's wager window) or `"blacksmith"` (a smith opened from an NPC). Each needs a matching `replaces` entry in `mod.json`. All five take scope `"all"` only (`invalid_argument` for `"own"`): the server names no speaker, so no request can be proven to answer this mod's own act. While a mod holds a kind, the game's window is put away before it draws and a server-origin window session of that kind opens (`eqoa.windows.session(kind)`); the server's side is unchanged. With no mod holding a kind, the game's windows behave exactly as before.

`eqoa.trade.get()` returns an `eqoa.trade.State`, or nil when no request, trade or wager is up. It covers the game's own window too, held or not. Needs `eqoa.trade.get`.

| Field | Meaning |
|---|---|
| `revision` | Changes whenever any other field does. |
| `kind` | `"trade"` or `"duel"` (a duel's wager reuses the trade window and these actions). |
| `phase` | `"request"` (someone asked to trade; answer with `trade_answer`) or `"open"` (the offers can be changed). |
| `held` | Whether a mod holds the window (only the holder can act). |
| `partner` | An `eqoa.trade.Partner = {id:uint, name:string}`; the name is `"Unknown"` when the other player is out of view. |
| `initiated` | Whether the player asked (a trade; nil for a wager). |
| `seconds_remaining`, `duration` | On a held request only: seconds left before the client refuses it, and 45. |
| `mine`, `theirs` | The two sides, each an `eqoa.trade.Side = {items, money:int, accepted:bool}`; `items` are `eqoa.trade.Item = {slot:int, id:Id, name:string, quantity:int, text:string}` (at most 4; `slot` is 1-based and is what `trade_remove` takes). |
| `bags` | The player's items, each an `eqoa.trade.Bag = {slot:int, id:Id, name:string, quantity:int, worn:bool, no_trade:bool, can_offer:bool, text:string}`; `slot` is 1-based and is what `trade_add` takes. |
| `carried` | The coin the player still carries, not counting what is offered. |
| `max_items` | 4. |
| `can_accept` | The player has not accepted yet. |
| `accept_asked` | A `trade_accept` question is waiting for the player's answer. |

`eqoa.duel.get()` returns an `eqoa.duel.State = {revision, enabled:bool, state, invite, queued:int, opponent_id:uint?}` (`enabled` is whether the server lets the player duel; `state` is `"none"`, `"invited"`, `"wager"`, `"set"` or `"running"`; `invite` an `eqoa.duel.Invite = {id, name, race_id, race_name, class_id, class_name, level, guild, title}` or nil; `queued` the invitations waiting; on a held invitation also `seconds_remaining` and `duration`, about 20 seconds). `eqoa.blacksmith.get()` returns an `eqoa.blacksmith.State = {revision, coin:int, repair_pending:bool, held:bool, total:int, items}`, or nil when no smith is open (the game's own window counts, held or not: `held` says whether a mod holds it, `total` is the sum of every item's `cost`); each item is an `eqoa.blacksmith.Item = {slot:int, id:Id, inventory_key:uint, name:string, quantity:int, condition:number, cost:int, can_repair:bool, worn:bool}` for what the game's window repairs (`inventory_key` is the same key `eqoa.items` gives an inventory item, so a bag can show a cost column; `condition` 0 to 1, `cost` the game's own price, `can_repair` false when the item needs none). `blacksmith.changed` fires when a smith opens or closes (the read goes nil) and when a cost or the coin changes. Needs `eqoa.duel.get` or `eqoa.blacksmith.get`.

| Action (capability of the same name) | What it does |
|---|---|
| `eqoa.actions.trade_answer(yes:bool)` | Yes or no to a held `trade_ask`. Yes is the window's Yes (the trade is set up, the partner told, and the answerer's trade is open at once: the mod's `trade` session when this mod holds `trade`, else the game's own trade window; a trade the player answers Yes to in the game's own window stays the game's, and `busy` while a duel wager is held); no is its No (the partner is told). `window_closed` without a held request (none held, or it is gone). |
| `eqoa.actions.trade_add({slot, id, count?})` | Offers `count` (default 1) of a row of `bags`. Refused in the client: a worn item, a NO TRADE item, more than the row holds, a single item more than once, a fifth item, a stale row, and any change after the player's own accept (`not_ready`, `out_of_range`, `stale_handle`). Sends the game's own add. |
| `eqoa.actions.trade_remove({slot, id})` | Takes back a row of `mine.items`. |
| `eqoa.actions.trade_money(amount:int)` | Sets the coin offered (the whole amount), no more than the player carries (`out_of_range`). |
| `eqoa.actions.trade_accept()` | Asks the player first: the client draws A1's "Are you sure?" naming what each side gives, starting on No. Refused `busy` while one of your own changes (`trade_add`, `trade_remove`, `trade_money`) has not come back from the server, so the question always names the offer the server holds. Only the player's own Yes accepts, and only if both offers are still exactly what the question showed. After the Yes, a change of either offer while the accept stands is answered by the client with the window's own unaccept (`trade.accepted` with `side` `"changed"`) as soon as the client sees the change (a change that reaches the server before the client's accept still completes there: that race exists in the game's own window too). A change of either side's offer ends the question as No, as the game clears the accepts. Completes with `detail.asked = true`; the result of the Yes is the `confirm.closed` event (`kind` `"trade_accept"`, or `"duel_accept"` for a wager). No other call accepts, and a held trade never accepts by itself. |
| `eqoa.actions.trade_unaccept()` | Takes the player's accept back (`not_ready` when there is none). |
| `eqoa.actions.trade_cancel()` | Cancels the trade or the wager. Nothing moves, so it asks nothing. |
| `eqoa.actions.duel_answer(yes:bool)` | Yes or no to a held `duel_invite`: the game's own accept or decline. The accepted invitation opens the wager, which is accepted through `trade_accept` (the question). `window_closed` without one, and after the invitation ran out. |
| `eqoa.actions.blacksmith_repair({inventory_key})` or `({slot, id})` | Repairs one item of the smith's `items`, sending the window's own repair. Works at a smith the caller holds, or at the game's own smith window while it is open and no mod holds `blacksmith` (the window is the player's to see and the game lets any press through; a smith another mod holds is that mod's: `busy`; no smith open: `window_closed`). A mod that repairs without a click is the manifest's **Actions: can act on its own (repair items (spends your coin))**. Refused for an item that needs no repair, when the price is more than the player carries, for a stale row, and `busy` while the last repair has not been answered (the server's answer is that item's `condition` changing; after 3 seconds without one other items may go, but the same item waits for its condition to change, for 10 seconds, or until no smith is open). `inventory_key` may come with the item's `id`, which must still match (`stale_handle`). It does not ask: the game's own window does not either. |
| `eqoa.actions.blacksmith_close()` | Closes the smith, as the window's close does. |

Every acting call sends once and reports `sent`, never `confirmed`; nothing is ever resent. **Events.** `trade.request` `{kind, id, name}` fires when a held request arrives; `trade.changed`, `duel.changed` and `blacksmith.changed` `{before, after, changed}` when the matching read changes; `trade.accepted` `{kind, side}` when the player's accept is sent (`side` `"mine"`), the partner accepts (`"theirs"`, for a wager too), the server refuses an accept (`"refused"`) or the client took the player's accept back because an offer changed (`"changed"`); `trade.closed` `{kind, reason}` when a held trade or wager ends (`reason` `"completed"`, `"cancelled"`, `"closed"`, `"left_world"`, `"death"`, `"released"`, `"game_windows"` or `"passed"`).

**What the client keeps for itself.**
- A request nobody answers is refused after 45 seconds (a duel invitation after its own 20), because the game's timers run only while its window is drawn. The refusal is the window's own No (for a trade, which has no timer in the game, the same 45 seconds the game gives a friend or guild request).
- A held window whose mod is disabled or releases the claim, or when the player turns on **Use the game's windows**, goes back to the game's own window if play is plain (`eqoa.windows.pass` does the same on request), and is refused or cancelled otherwise; the same for death, zoning, a teleport and a lost connection, so the other player never waits. A player's accept is taken back when a held trade is handed to the game's window (its window starts clean).
- A second trade request while a held request or trade is up is answered busy to the newcomer, with the game's own line in the chat, exactly as the game answers one that comes with a window up, and the held one is not touched. A duel invitation that comes behind a hidden wager waits as the game queues it (`queued`) and shows after the wager ends; behind a hidden invitation it is declined (the game would queue it behind the open window). An invitation the game had queued behind another window is taken into the hold when it would show, so it is held once, keeps its own clock, and a refusal is the game's real decline (it cannot come back). A trade request behind a hidden wager or smith is answered busy; an invitation behind a hidden trade or smith waits.
- Mods & Addons shows **Trading: can ...** and **Blacksmith: can close the blacksmith** for what a mod declares, **Actions: can act on its own (repair items (spends your coin))** for `blacksmith_repair`, and **Asks you first: accept a trade or a duel wager** for `trade_accept`.
### The map

Reading the map needs no capability.

| Function | Returns |
|---|---|
| `eqoa.map.get()` | `MapState`, below. |
| `eqoa.map.markers(world_id:int)` | `Marker[]` for that world (0..255): what the game's map can show. |
| `eqoa.map.view({size?, layers?})` | A held minimap picture the mod can draw; see "Minimap views". |
| `eqoa.map.terrain({size?})` | A held picture of the whole world's ground, with the transform between the world and its pixels (round A13); see "The terrain layer". |
| `eqoa.map.set_guide(spec)`, `eqoa.map.clear_guide()` | The mod's own guide line; see "Guides". |
| `eqoa.map.open()` | Opens the full map, as the player's map key does. While a mod holds the `map` kind (§7, "The world map") the holder's map opens or is refreshed in the same step (the session says `opened_from = "mod"`; it is never closed by this call and the game's map never shows). Returns `true`. `not_in_world` outside the world, `unavailable` while the player has Map turned off, `busy` if the request cannot be queued. Needs no capability. |
| `eqoa.map.set_waypoint(point)`, `eqoa.map.clear_waypoint()` | Move or clear the player's own waypoint; see "The player's waypoint". |

`MapState = {enabled:bool, open:bool, minimap:bool, markers_on:bool, world_id:int, world_name:string?, zone:Location?, bounds:Bounds?, revision:uint, waypoint:Waypoint?, search_anchor:Rect?}`

- `enabled`, `markers_on`: the player's Map and Map Markers settings. `open`: the full map is up. `minimap`: the minimap is in view (setting on, in play, full map closed, HUD shown), whether or not a mod hides the game's picture.
- `bounds = {x_min, z_min, x_max, z_max}`: the current world's extent in game units, nil until its map is loaded.
- `waypoint = {world_id:int, x:number, z:number, label:string?, owner:string?}`: the personal waypoint; `owner` is the id of the mod that placed it, nil when it is the player's.
- `search_anchor = {x, y, width, height}`: the free band of the full map's header in window pixels, for a mod that puts a control there; nil before the full map has been drawn once.
- `revision` goes up when the state changes; `map.changed` carries `{before:MapState?, after:MapState?, changed:string[]}`.

`Marker = {kind, name, x, z, level_min, level_max, label, step}`: `kind` is `"camp"`, `"coach"`, `"town"`, `"quest"`, `"community"`, `"waypoint"` or `"group_waypoint"`; `x`, `z` are game units; `level_min` and `level_max` are given for camps and quests; `label` is the game's search label when it has one (a camp's "(XP camp)" label, a coach's town and coachman, a community marker's note); `step` is given for quests: true for a step to do, false for a quest on offer.

#### The player's waypoint (round 7)

`eqoa.map.set_waypoint({world_id:int, x:number, z:number, label:string?, keep:bool?})` and `eqoa.map.clear_waypoint()` return a `RequestId`; the outcome is an `action.result` (§8) of kind `set_waypoint` or `clear_waypoint`. Both need the capability **`eqoa.map.waypoint`**, listed by its own name (`eqoa.actions.*` does not cover it); Mods & Addons then shows "map waypoint" among what the mod can do on its own. No click from the player is needed.

- The call itself fails with `not_in_world`, `unavailable` (Map turned off), the capability errors, or `invalid_argument` (a malformed point, a non-finite number, a label over 128 bytes).
- The request is then `refused` with `invalid_argument` for a world the game does not know or a point outside that world's ground, and with `not_ready` for a known world whose map is not loaded yet. The pin lands only on a loaded map's ground.
- `keep` is optional (`invalid_argument` if it is not true or false). Setting a pin again at the point of a pin the mod does not own takes it over, which is how a mod removes a pin the player placed: set it again with `keep = true`, then `clear_waypoint()` on the `action.result`.
- The pin is the game's own: one per world, saved, drawn and reached as if the player had placed it.
- **Ownership.** A mod owns a pin only while that exact pin is still there. Every game step the client drops ownership, for good, of a pin that is gone or differs (the player moved or cleared it, arrived at it, or cleared and re-placed it at the same spot), and another mod's `set_waypoint` takes it over. `clear_waypoint()` clears the pin this mod placed most recently and still owns; if it owns none the result is `refused` with `busy`. Disabling a mod clears only the pins it still owns, in every world, **except a pin set with `keep = true`** (added 2026-11-24: a pin that is the player's own act made through the mod, such as a click on a map the mod draws; it stays when the mod is turned off, and the mod can still clear it while it runs); the player's pins and another mod's stay.
- The game keeps one pin file per client window, shared by every character played in it: a mod's pin and its ownership carry across a logout and a character change for the rest of the session. Ownership is not saved: after a restart every pin is the player's.
- Known limit: a player who clears and re-places the very same point within one game step (under 34 ms) is not noticed.

#### Guides

`eqoa.map.set_guide({world_id:int, x?, z?, entity_id?, dots?, label?})` sets this mod's guide: a dashed line with optional dots (`dots = {{x, z}, ...}`), drawn on the full map, the minimap and minimap views. It is separate from the waypoint and needs no capability. Give a point (`x`, `z`) or an `entity_id`; an entity guide follows the live entity (`not_found` if the client does not see it in that world). A mod has one guide; setting another replaces it. `eqoa.map.clear_guide()` removes it. Both return `true`. A guide is cleared when the mod is disabled.

`guide.lost` fires when a guide ends by itself: `{owner:uint, world_id:int, entity_id:uint?, label:string?, reason:"entity_left"|"world_left"}` (`owner` is a number identifying the mod's running instance). An entity guide is lost when its entity goes; every guide is lost when the world is left.

#### The terrain layer (round A13)

`eqoa.map.terrain(spec?)` returns a held picture of the player's world, drawn the way the game's own map draws it, so a mod-drawn map (a `map` session, above) needs no terrain drawing of its own. It needs no capability and is `not_in_world` outside the world. A mod may hold two (`quota` for a third). `spec` has two shapes:

- **The whole world**: `{size:number?}`. `size` is the longest side in pixels (1024 when omitted, 64..2048, `out_of_range` otherwise). The picture has the world's shape and is made from the game's overview (15.6 world units a texel at best), never larger than the overview itself.
- **A view** (as the game's map shows a view): `{center = {x, z}, scale:number?, width:number?, height:number?}`. `center` is the world point at the picture's middle; `scale` is world units a pixel (1..4096, `out_of_range` otherwise; when omitted the game's own opening zoom for that picture: 2500 units across its shorter side); `width` and `height` are the picture's pixels (64..2048; `size` stands for both when they are omitted, else 1024). The picture is the overview and, over it, **the game's own 512-texel detail tiles** (3.9 world units a texel) at the zoom where the game's map uses them (`scale` at or below 7.8125), for the zones the game's map has built; where a tile is not there yet the overview shows through. The client asks the game's cache for the tiles of each held view and keeps them, as the game's map keeps its own (at most 12 tiles a view, 40 at once for everything). `t.set_view({center?, scale?})` moves or zooms it (it answers `true`); a whole-world picture has no view to move (`invalid_argument`). While the game's map is browsing another world (Places) a view has the overview only: tiles are the player's own world's.

The game builds its ground behind play for a few seconds after a world is entered, so unbuilt ground is dark and fills in (`progress`). The terrain does not include markers, the waypoint or guides: a mod draws those itself from `eqoa.map.markers`, `eqoa.map.get` and the transform below.

| Member | Meaning |
|---|---|
| `t.texture` | A `TextureInfo` (`uv` is `{0, 0, 1, 1}`: the whole picture), or nil until the game's map has a world to show. **Read `t.texture` each frame** rather than keeping it: a change of world or size replaces the texture. A picture that is only drawn again (the game's map built more ground, a tile arrived, the view moved) keeps the same texture and handle. |
| `t.error`, `t.get_texture()` | As for a minimap view: the `Error` that keeps the picture from being made (`quota` when the mod's texture budget, the same for every mod, is used up), or the `TextureInfo`, nil, or `nil, Error`. |
| `t.world_id`, `t.width`, `t.height` | The world the picture shows and its size in pixels; nil until the game's map has a world to show. |
| `t.bounds` | `{x_min, z_min, x_max, z_max}`: the whole world's rectangle, in game units. |
| `t.progress` | `{done, total}`: zones the game's map has drawn, of zones to draw. |
| `t.project(x, z)` | `px, py, inside`: a world position in the picture's pixels (x grows right, z grows down; a whole-world picture's top left is `x_min, z_min`, a view's middle is `center`), and whether it lies within `bounds`. `unavailable` before the game's map has a world to show (a read whose data has not loaded, §1). |
| `t.unproject(px, py)` | `x, z`: the world position under a pixel. |
| `t.set_view(spec)` | See above. |
| `t.close()` | Releases the picture now. It is also released when the mod is disabled or its Lua collects it; after a close `project` and `unproject` are `stale_handle`. |

The picture follows the player's own world even while the game's map is browsing another (Places). It is drawn again only when its view or the world changes or the game's map has built more ground or loaded more tiles (one pass of a few quads).

**Keys under a mod's map.** While the game's own map is up, every key goes to it (the arrow keys and WASD pan it, Escape closes it). A mod's `map` session does not hold the keyboard: play stays plain underneath, so the character walks on WASD and Escape reaches the game unless the mod claims those keys (`eqoa.input.claim`) or captures text. A mod that draws a full-screen map claims what it uses, and releases them when its session ends. The controller's focus (`eqoa.windows.focus`) works on a `map` session as on any other.

#### Minimap views

`eqoa.map.view({size:number?})` returns a view: the game's minimap, centred on the player, without the game's frame art, as a picture the mod draws itself (for example cut to a circle with `eqoa.hud.image`). `size` is in pixels, 112 when omitted, and must be 16..1024 (`out_of_range` otherwise). A mod may hold four views (`quota` for a fifth). Both numbers are design intent (§9). The game draws the picture only while its own minimap is hidden (`eqoa.hud.hide("minimap")`) and in view.

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
| `ok` | bool? | nil while the outcome is not known: `queued`, `dispatched` (sent, waiting for the server), `unknown`, and `cancelled` by zoning or logout after the request was sent (the server may still act). true for `completed`; false for `refused` and for `cancelled` before anything was sent. Do not use it to tell whether a request is finished: read `state`. |
| `code`, `reason` | string? | Why, when refused, cancelled or unknown: an error code (§1) and a sentence. |
| `queued_tick`, `dispatched_tick`, `completed_tick` | uint, uint?, uint? | 30 Hz game steps at each stage. |
| `detail` | table? | Extra facts for some actions (for example `{typed, submitted}` for `input_text`). The keys that say what `completed` means are listed below the table. |

- `completed` means the client did its part, and the action's row says which part that is. One of:
  - the client did it, a local action such as targeting or filling the crafting tray;
  - the server's answer confirmed it: loot, buy, sell, bank, opening a corpse or a merchant;
  - the request was sent and the server's answer is a separate fact the mod watches for (`cast`, and most actions from round A3 on): `detail.sent = true`. Some of these also say `detail.confirmed = false` (the A3, A7, A9 and A10 actions); where `confirmed` is absent, `sent = true` alone means the same thing. No action sets `confirmed = true`;
  - the action asks the player first (§8): `detail = {asked = true, confirm = <id>}` means the client's own question is on the screen, nothing has been done yet, and the answer arrives as `confirm.closed` with that `confirm` id. `asked` means this and nothing else;
  - the client handed the request to the game itself: `detail.requested = true` (`jump`: the walk takes it when it can) or `detail.pressed = true` (the fishing window's own key).
- `refused` means a check failed or the server said no. `reason` carries the server's words when it gave any.
- `unknown` means the request was sent and no answer the client can tie to it came within 10 seconds. The client does not guess and does not retry. Show it; do not treat it as success or failure.
- `cancelled` means it was cancelled before it was sent (by the mod's `cancel`), or by zoning, logging out, or the mod being disabled.

The `action.result` event carries the `ActionResult` and fires when a request is dispatched to the server (`state = "dispatched"`) and again at its final state; an action with no server step fires once, with its final state. Nothing fires when the request is queued.

| Function | Returns |
|---|---|
| `eqoa.actions.status(id:RequestId\|BatchId)` | The latest `ActionResult` (or `BatchResult`). `not_found` once the record has been dropped: the client keeps the latest 256 action results per mod (§9). |
| `eqoa.actions.cancel(id:RequestId\|BatchId)` | `true` if it cancelled the request: one not yet sent is never sent, and its result is `cancelled`. `false` if it was already sent to the server: that cannot be undone, so nothing is cancelled, the request keeps waiting and its real result (`completed`, `refused` or `unknown`) arrives as usual. `not_found` if the request is no longer pending. With a `BatchId` it stops that batch as `eqoa.actions.stop_batch` does, except that it answers `not_found` (not `false`) when no batch is running. |
| `eqoa.actions.stop()` | Queues the game's stop: ends auto-attack, auto-run, follow and the pending cast, whoever started them (the player's own included). See the note below. |

Limits (§9): 32 requests may wait per mod (one more is refused with `busy`); at most 8 are dispatched per mod per game step, refilling at one per step (30 a second), and 64 per step for all mods together, shared fairly; a request that waits more than 1 second for its turn is refused with `rate_limited`. Only one transaction at a time runs against each of the pack, a corpse, a merchant and the bank, for all mods together: a second one is refused with `busy`.

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
| `eqoa.actions.equip(item:ItemRef)` | The item page's equip command for a pack item, which is the game's toggle: wears it if it is not worn, takes it off if it is. Separate "wear" and "take off" forms may be added later under new names; this one keeps its meaning. |
| `eqoa.actions.swap_items(a:ItemRef, b:ItemRef)` | Swaps two pack stacks. |
| `eqoa.actions.quick_slot(item:ItemRef, slot:uint)` | Puts a pack item in quick slot 1..4. |
| `eqoa.actions.weapon_set(set)` | Switches weapon set: 1..5 (5 is bare hands) or `"bare_hands"`. `unsupported` on a server without weapon sets. |
| `eqoa.actions.prepare_weapon_set(set:uint, name:string)` | Saves the worn weapons as set 1..4 under a name of at most 16 characters. |
| `eqoa.actions.assign_ability(slot:uint, ability)` | Memorizes an ability into hotbar slot 1..20, as the Abilities window does. Like its Memorize, it refuses (`not_ready`) an ability that is already on a slot and one the character has not learned. |
| `eqoa.actions.unmemorize_ability(x)`, `eqoa.actions.unmemorize_all()` | Clears one memorized ability (a slot number 1..20, or an ability) or all of them. The game itself asks nothing before Unmemorize All, and neither does this. |
| `eqoa.actions.arrange_ability(from, to)` | Moves an ability in the Abilities window's order. |
| `eqoa.actions.loot(item:ItemRef, count:uint?)`, `eqoa.actions.loot_all()` | Takes one stack from the open corpse (the whole stack when `count` is omitted, else `count` of it), or everything. `window_closed` if no corpse is open. |
| `eqoa.actions.loot_assign(item:ItemRef, member:uint\|EntityRef)`, `eqoa.actions.loot_abandon()` | Master-looter actions on the open corpse: assigns one stack to an eligible recipient (`MasterLoot.members`), or abandons master loot so the corpse falls back to open looting. **Both ask the player first**, as the game's own master-looter window does: the call completes with `detail = {asked=true, confirm=<id>}` when the client has put the game's own question on the screen ("Assign <item> to <name>?", or the abandon warning), starting on **No**, answered only by the player; on Yes the client re-checks that master loot is still the player's on this corpse, the stack is still there and the member still eligible, then sends; the end is `confirm.closed` with `kind = "loot_assign"` or `"loot_abandon"` and, on `"yes"`, `detail = {sent:bool, reason:string?}`. The refusals below (`window_closed`, `not_ready`, `not_found`, `stale_handle`) come before any question. `window_closed` if no corpse is open, `not_ready` when master loot is not active (the game's own rule), `not_found` for an ineligible recipient, `stale_handle` if the stack is gone. |
| `eqoa.actions.ready_check_answer(id:uint, ready:bool)` | Answers the open raid ready check (`id` from `eqoa.raid.ready_check()`). `not_found` when no matching ready check is open. |
| `eqoa.actions.loot_roll(id:uint, choice:"need"\|"greed"\|"pass")` | Answers an open raid loot roll (`id` from `eqoa.raid.loot_rolls()`). `not_found` when no matching roll is open, `not_ready` if `"need"` is asked on a roll with `may_need = false`. |
| `eqoa.actions.buy(item:ItemRef, count:uint=1)`, `eqoa.actions.sell(item:ItemRef, count:uint=1)` | Buys from the open merchant's wares, sells a pack stack to it. `window_closed` if no merchant is open. `buy` acts at once, as the game does (its how-many box for a ware that stacks is what `count` is). `sell` sells a plain misc item at once, and **asks the player first** in the cases where the game's merchant window asks (below). |
| `eqoa.actions.sell_many(items:ItemRef[])` | Sells 1 to 64 whole pack stacks to the open merchant, in order; one question for the batch where any stack is one the game would ask about (below). `window_closed` if no merchant is open. |
| `eqoa.actions.bank_move(item:ItemRef, count:uint=1)` or `(from:string, item:ItemRef, count:uint=1)` | Moves `count` of a stack between pack and bank. **Omitting `count` moves 1, not the stack**: pass `item.quantity` to move it all. The direction follows from the list the item is in. The optional `from` names that list and must agree with it (`invalid_argument` otherwise): `"bank"` or `"withdraw"` for an item in the bank, `"inventory"`, `"bags"` or `"deposit"` for one in the pack. `window_closed` if no bank is open. |
| `eqoa.actions.bank_money(direction:"deposit"\|"withdraw", amount:uint)` | Moves tunar. |
| `eqoa.actions.open_loot(entity?, options?)` | Asks the server to open a corpse: the given one (`EntityRef` or id) or the nearest. Refuses anything that is not a corpse (`invalid_argument`); `not_found` if there is none. The result is `completed` when the corpse opens, `refused` with the server's reason (for example `out_of_range`), or `unknown`. By default the client targets the corpse for this (which ends an auto-attack) and puts your previous target back when the mod is done, unless you chose another meanwhile. `options = {keep_target = true}` asks for the corpse by its id and changes nothing else of the player's: the target and the auto-attack stay, so a mod can loot in the middle of a fight; see "Opening a corpse without moving the target" below. `options` must be a table or nil, and `keep_target` a boolean or nil (`invalid_argument` otherwise). |
| `eqoa.actions.talk(entity?, {kind="merchant"\|"bank"}?)` | Talks to an NPC: the given one or the nearest of that kind (any friendly NPC when `kind` is omitted). Refuses corpses and NPCs that are not of the kind asked for. `completed` when the merchant or bank opens. The target is put back as for `open_loot`. |
| `eqoa.actions.chat(text:string)` | Sends a line as if typed into chat, in the current channel. The line must be shorter than 128 UTF-16 units and contain no line break (`invalid_argument`). A second argument is refused with `unsupported`: to speak in a particular channel use `command` with that channel's slash command. Grants: rule 4 above. `chat` and `command` check the text before the grant (a bad line is `invalid_argument` even without the grant); every other action checks its capability first. |
| `eqoa.actions.command(text:string)` | The same, for a command: a leading `/` is added if the text has none (and does not start with `#`). The game's own rules for each command still apply. |
| `eqoa.actions.autorun(on:bool?)` | Auto-run on or off; toggles with no argument. |
| `eqoa.actions.follow(entity)` | Follows an entity in view (it is targeted first). `not_ready` if the game will not follow it. `follow(nil)` only stops following: when nothing is being followed it completes and does nothing. |
| `eqoa.actions.camera({heading?, pitch?, distance?, mode?})` | Sets the camera: radians, game units, `mode` 0..3. At least one field. `busy` when the game has the camera locked. |
| `eqoa.actions.mastery_share(fraction:number)` | Sets the share of experience going to Class Mastery: 0..1 in steps of 0.01. |
| `eqoa.actions.mastery_browse(skill_id:int)` | Inspects a mastery skill, as the Class Mastery window's list does. It needs the game's Class Mastery window open, or a `"mastery"` window session of the mod's own (§8, "Window sessions"), with the list loaded (`not_ready` otherwise); `not_found` for a skill not in the list; `busy` while a mastery request waits. The answer is `eqoa.mastery.selected()` (it stays while a mod's session is up). |
| `eqoa.actions.mastery_buy(skill_id:int)`, `eqoa.actions.mastery_refund(skill_id:int)` | Buys an inspected skill, or switches an owned one off (the game's own Refund request), **after asking the player** (below). The same window and inspection are needed as for `mastery_browse` (`not_ready` for a skill not inspected, a skill already owned or not yet known for a buy, a skill not owned for a refund). The game's rules are kept: one mastery request at a time (`busy`), the game's own 10 second give-up of an unanswered one, and a second Master Class is refused (`not_ready`, with the game's own warning) while another is owned. |
| `eqoa.actions.train_stats(points:table)` | Spends training points: `points` names stats from `strength`, `stamina`, `agility`, `dexterity`, `wisdom`, `intelligence`, `charisma`, each a whole number of points (`invalid_argument` for another name, a fraction or no point at all, `out_of_range` for a negative count). Refused like the game's trainer refuses: `out_of_range` when the points added together are more than `eqoa.player.training().points`, or when a stat would go past its cap (`Training.caps`). Otherwise it **asks the player** (below) and, on their Yes, sends the game's own trainer request. |
| `eqoa.actions.request_factions(page:uint=1)` | Asks the server for one page of the faction standings, as the game's Factions window does (page 1 is the first); `out_of_range` for a page past the page count the server last gave. The answer is `eqoa.factions.list()`. Asking again forgets the page held, as the game does. |
| `eqoa.actions.request_combat_record()` | Asks the server for the combat record, on a server that keeps one (`unsupported` otherwise). The server's push is `eqoa.combat.record()`; the game's own window is not opened. |
| `eqoa.actions.open_window(kind:string, arg:int=0)` | Opens one of the game's windows; see the list below. |
| `eqoa.actions.close_window(window:WindowRef\|ref:string)` | Closes that window, as the back button does. With the `ref` of an NPC dialog or offer session (§7, "NPC dialogs and offers (round A2)"): Done for a `text_dialog`, the game's own "no choice" for a `response_dialog`, No for an offer; the result is `sent`. |
| `eqoa.actions.select_row(window, row:uint)`, `eqoa.actions.activate_row(window, row:uint)` | Moves the lit row to `row`; `activate_row` then confirms it. Rows are the 1-based `Window.rows`. `out_of_range` for a row that does not exist, `not_ready` for a row that is not enabled. |
| `eqoa.actions.dialog_answer(window\|ref, option:uint)` | Answers a dialog: the 1-based position in `Dialog.options` for a text dialog (1 for its single Done), or 1 or 2 for a confirm (§7 `Dialog`). With the `ref` string of a session a mod holds (round A2, §7 "NPC dialogs and offers (round A2)") it answers that session's dialog or offer: the 1-based position in the session's full `data.options`, 1 (Done) for a `text_dialog`, 1 = Yes or 2 = No for an offer; `sent`, never `confirmed`, never replayed, `stale_handle` for an ended session. Accepting a game Yes/No box is allowed only for the prompts listed below ("The game's confirmations cannot be skipped"); declining always is. |
| `eqoa.actions.menu_command(window, command:string)` | Runs a command that an enabled row of that window offers (`MenuRow.command`, for example `"loot.take_all"`). Any other string is `unsupported`. With a session `ref` string instead of a `WindowRef`, it runs an entry of the mod's own Start menu (§7). |
| `eqoa.actions.chat_line(channel, text, opts?)`, `eqoa.actions.set_channel(channel)`, `eqoa.actions.open_link(id, name)` | A line on a chosen channel with item links, the game's chat mode, and a link's item card (round A6, §7). |
| `eqoa.actions.input_text(window, text:string, submit:bool=false)` | Types into the game's open text box; see "Text boxes" below. |
| `eqoa.actions.group_invite(name?)` or `(id, name)` or `({id?, name})` | With no arguments, invites the current target to your group, as the start menu's Invite does. With a player name (or entity id and name, e.g. from Friends, Guild, Who or Group Search), invites that player directly through the game's invite builder. |
| `eqoa.actions.friend_add(name:string)`, `eqoa.actions.friend_remove(id_or_name)` | Sends a friend request to `name` or removes a friend by id or name. Refuses self, duplicates, full list (32) or missing entries. |
| `eqoa.actions.friend_accept()`, `eqoa.actions.friend_refuse()` | Answers an active incoming friend request (`window_closed` when none is up). `friend_accept` opens the client-drawn "Are you sure?" question (below) and only sends the accept on the player's own Yes; `friend_refuse` refuses immediately. |
| `eqoa.actions.ignore_add(name:string)`, `eqoa.actions.ignore_remove(id_or_name)` | Adds a player to or removes a player from the ignore list. |
| `eqoa.actions.guild_invite(name:string)`, `eqoa.actions.guild_promote(member)`, `eqoa.actions.guild_demote(member)`, `eqoa.actions.guild_set_motd(text:string)` | Sends guild commands using the game's rank checks (`permission_denied` if your guild rank cannot perform the action). `member` is a roster member id or name. Guild disband and quit keep the game's own prompt and are not offered as mod actions. |
| `eqoa.actions.guild_kick(member)` | Checks rank permission, then opens the client-drawn "Are you sure?" question (below) naming the member. Only the player's own Yes sends the kick after re-checking membership and rank. |
| `eqoa.actions.guild_accept()`, `eqoa.actions.guild_refuse()` | Answers an active incoming guild invitation (`window_closed` when none is up). `guild_accept` opens the client-drawn "Are you sure?" question (below) naming the inviter and guild; `guild_refuse` refuses immediately. |
| `eqoa.actions.pet_attack()`, `eqoa.actions.pet_back_off()` | Send your pet in on your target, or call it off, as the start menu's Pet Attack and Pet Backoff do. `not_ready` without a pet; attacking also needs a target other than yourself or the pet. The game has further pet commands and a dismissal; they are not part of `eqoa/1` yet. |
| `eqoa.actions.emote(name:string)` | One of the game's four emotes, by name: `"wave"`, `"bow"`, `"point"` or `"cheer"` (any letter case); anything else is `invalid_argument` at the call. `completed` once the emote was sent; `detail.emote` names it. |
| `eqoa.actions.jump()` | Asks the walk for a jump, as the jump key does. `unsupported` on a server that does not offer jumping (the reference server's does not). The walk takes the jump at the first game step that allows one within about a third of a second (on the ground, not swimming or flying) and otherwise lets the request go; a mod that wants to know reads `Player.velocity`. |
| `eqoa.actions.remove_effect(buff:BuffRef)` | Takes one of your removable effects off, as a long press on it does in the game's buff window. `buff` comes from `eqoa.effects.buffs()`; one that is not removable is `not_ready` at the call and a reference that went stale is `stale_handle`. |

The A3 actions above report what the client did, never what happened next: `completed` with `detail.sent = true` and `detail.confirmed = false` (for `jump`, `detail.requested = true`: the walk was asked, and no question is put to the player) means the request was sent; the other player's answer to an invitation, the pet's response, the server's resent buff list (`buffs.changed`) and the take-off are separate facts a mod watches for (`group.changed`, `entity.changed`, `buffs.changed`). `remove_effect` re-checks the line when it runs and refuses with `stale_handle` when the effect is gone by then. Like every action they need the character alive and the game in plain play.

`eqoa.actions.destroy_many(items:ItemRef[])` (round 8, added 2026-11-23 for the Bag Viewer's multi-select) asks the same question once for several whole pack stacks: `items` is a list of 1 to 64 pack handles, none twice (`invalid_argument` otherwise; `stale_handle` if one is gone; `not_ready`, "worn items cannot be group-destroyed", if any stack is worn: this holds for every mod, and a stack that has become worn by its turn in the run is skipped like a changed one). It completes with `detail = {asked=true, confirm=<id>}` when the client has put its own question on the screen: **"Destroy these N items? It cannot be undone."**, then one line for each stack (its count, and "(quick slot)" or "(NO TRADE)" where that is so); a list of more than 12 stacks shows the first 12 and "...and N more", so Yes and No stay on the screen. It starts on **No** and is answered only by the player, exactly as `destroy`'s (the next paragraph applies to it unchanged: nothing a mod calls can answer it, one question at a time, 30 seconds, ends as No on the mod's window closing, zoning, death or leaving the world). On Yes the client destroys the stacks one at a time, each waiting for the server's answer to the one before, as the game's own destroy does, and re-checks each stack at its turn: one that is gone, changed or smaller by then is skipped, never destroyed. The mod hears the end as `confirm.closed` with `kind = "destroy_many"` and, on `"yes"`, `detail = {sent:bool, sent_count:uint, total:uint, skipped:string[], reason:string?}`: `sent_count` of `total` destroy requests went to the server (the server may still refuse; watch `inventory.changed`), `skipped` names the stacks not sent, and `reason` says why the run ended early (`"not in the world"`, `"the server did not answer"`, or, as the `answer`, a closed window or zoning). A request is never repeated. Declaring it needs `eqoa.actions.destroy_many` (or `eqoa.actions.*`); the manager lists it under "Asks you first: destroy several items". A run cut short by zoning, death or the mod's window closing is reported as `confirm.closed` with that reason as the `answer` and the same `detail` (`sent_count`, `total`, `skipped`), so a mod can say how far it got.

`eqoa.actions.sell(item, count)` and `eqoa.actions.sell_many(items)` (added 2026-11-25) ask where the game's merchant window asks. The game asks Yes or No, starting on No, before it sells a stack that is **quick-slotted** (its word 931), **no-trade** (932) or **not a misc item** (1842, `Sell item "<name>"?`); a misc item that is none of these sells at once. `sell` does the same: for such a stack it completes with `detail = {asked=true, confirm=<id>}` when the client has put its own question on the screen, naming the item in the game's own words where its string table has them, and what the player gets (`eqoa.merchant.price`'s figure times the count, in tunar). The sale is sent only on the player's Yes, and the stack is checked again when the answer comes: a stack that is gone, smaller, a different item, now worn, or a merchant no longer open means nothing is sold. The mod hears the end as `confirm.closed` with `kind = "sell"` and, on `"yes"`, `detail = {sent:bool, reason:string?}`; whether the merchant took it is `inventory.changed` (and `merchant.*`), as for an unasked sale. A stack that needs no question is sold at once, as before, and completes with the same `detail` as before (`count`, `total_price`), or is refused `window_closed`, `busy`, `stale_handle`, `invalid_argument` as before. **A worn stack is still refused `not_ready`**: the game asks about it, the API is stricter and never offers to sell what the character wears.

`sell_many` takes 1 to 64 pack handles, none twice, and sells each whole stack in order to the open merchant, each waiting for the merchant's answer to the one before and re-checked at its turn (a stack that is gone, changed, smaller or now worn is skipped and counted; the merchant going away ends the run). The argument errors are `destroy_many`'s; `window_closed` when no merchant is open; a worn stack anywhere in the list refuses the whole call `not_ready` ("worn items cannot be sold in a group"). If **no** stack is one the game would ask about, nothing is asked: the call completes with `detail = {asked=false, count, total_price}` and sells at once. If **any** is, ONE question covers the batch: **"Sell these N items for T tunar?"**, then one line for each stack the game would ask about (its count, its price and why: "quick slot", "NO TRADE" or "not a misc item"; more than 12 shows the first 12 and a count of the rest), then "and M other items" for the plain ones; it starts on **No**, Yes sells all, No sells none, and the call completes with `detail = {asked=true, confirm=<id>}`. Either way the run ends with one `confirm.closed` with `kind = "sell_many"`: on a question, `answer` is the player's (`"yes"`, `"no"`, `"expired"`, or the reason the window or world ended it) and, on `"yes"`, `detail = {sent, sent_count, total, skipped:string[], reason?}` as for `destroy_many`; for a call that asked nothing `confirm` is `0` and `answer` is `"not_asked"` with the same `detail`, so a mod learns the end in one place. `sell_many` needs `eqoa.actions.sell_many` (or `eqoa.actions.*`); the manager lists it among the actions that can act on their own, as it lists `sell`.

**Also in this table (round A1):**

- `eqoa.actions.destroy(item, count?)`, `eqoa.actions.drop_quest(quest)`, `eqoa.actions.link_item(item)` and `eqoa.actions.hide_look(item, hide?)` (round A1). `link_item` puts the item's link in the chat line (`unsupported` where the server offers no item links, `unavailable` while the player has Item Links off, `not_ready` when the chat line has no room) and `hide_look` hides or shows a worn item's look (`hide` defaults to true; `not_ready` for an item that is not worn), as the game's item menu does; both complete when the client has sent the request.
- **`destroy`, `drop_quest`, `train_stats`, `mastery_buy`, `mastery_refund`, `guild_kick`, `friend_accept` and `guild_accept` always ask the player first.** They never act at once: the call completes when the client has put its own **"Destroy ...?"** / **"Drop the quest ...?"** / **"Spend N training points ...?"** / **"Buy the Class Mastery skill ...?"** / **"Switch off the Class Mastery skill ...?"** question on the screen (`detail = {asked=true, confirm=<id>}`). The question names the item and count (`count` defaults to the whole stack; `out_of_range` for more than the stack holds) or the quest, starts on **No**, and is answered only by the player's own key press (Left, Right or Tab to choose, Enter to answer, Escape for No) or click on its Yes or No. Nothing a mod calls can answer it, move the choice, extend it or skip it, and there is exactly one at a time (`busy` for a second). It ends as No when the player answers No, after 30 seconds, when the mod's window closes, on zoning, death or leaving the world, and when the mod goes away. On Yes the client re-checks that the item, quest, points or skill is still the one asked about (the points held, the caps, the list and the skill's state, the Master Class rule) and then does what the game's own window does: the destroy, the drop, the trainer's request (`0x001D`) or the Class Mastery request. The mod hears the end as `confirm.closed` (§3); for the points and skills `detail.sent` says the request went out, never that the server accepted it (watch `eqoa.player.training()`, the player's stats and `mastery.changed`). A request is never repeated. Kicking a guild member and accepting a friend request or a guild invitation work the same way: the question names the member, or the inviter and guild, and an incoming request's own 45-second timer running out ends the question as No too. A mod may not yet draw the question itself; a later round may let a mod restyle it, never answer it.
**Not available:**

- `eqoa.actions.move`, `eqoa.actions.look` and `eqoa.actions.buttons` (held movement, camera stick and controller buttons) are `not_yet`: they wait for a change in the upstream client. Calling one returns `nil, Error{code="not_yet"}`.
- `eqoa.actions.toggle_stats_box` belongs to the bundled Stats Box mod; any other mod gets `permission_denied`.

**The game's confirmations cannot be skipped.** Where the game asks the player Yes or No before a step (its message box, a `Dialog` of kind `confirm`, §7), only the player may say yes. A mod may read the box and draw it its own way, and it may always **decline** it: `eqoa.actions.dialog_answer` with No, `close_window`, or Cancel. **Accepting** it (Yes on a Yes/No box, or the OK of a one-button box) through `dialog_answer` is refused with `not_yet` for every prompt except those listed as answerable. Answerable today: `group_invite`. Every other kind, `quit`, `pet_dismiss`, `guild` (disband or leave), `report` and `other` (summons and resurrection read `other` today), cannot be accepted by a mod; the box stays up and nothing is sent. The list may grow as prompts get their own kinds and the confirm contract of the window-replacement work ties a mod-drawn confirmation to the player's own answer. The check is made from the box's own tags when the answer would be applied, so every route to it is covered. NPC conversations (`response_dialog`, `text_dialog`) are not Yes/No boxes: `dialog_answer` and `activate_row` answer them as before. The steps the game confirms in its Class Mastery window (buying a skill, switching one off) are offered with their own question (above), as are destroying an item, dropping a quest and spending training points. Round A2 gives a summons and a resurrection offer their own kinds, `summon_offer` and `revive_offer`, for a mod that holds them (§7, "NPC dialogs and offers (round A2)"): such a mod accepts one only through the client's own "Are you sure?" (as `destroy` does), so the player's own Yes sends it; declining is always sent at once; a `group_invite` held by a mod is accepted at once, as it is today.

**Where the client asks the player.** The API asks wherever the game itself would ask (owner's rule, 2026-11-25). Where the game's own window asks Yes or No before a step, the client asks as well, with the same words where the disc has them, the box on **No**, and only the player's own key press or click answers (the rules of `destroy` above apply unchanged). Where the game acts on one press (buying one ware, a deposit, a repair, a combine), the action acts at once. Steps a mod can take that the game has no single press for, and that spend or lose something (destroying several items, spending training points, mail that carries tunar), ask too. A batch step (`eqoa.actions.batch`) has nobody to ask: a step the client would have to ask about is refused `not_ready` (for `sell`: use `sell` or `sell_many` itself).

### Opening a corpse without moving the target

`eqoa.actions.open_loot(entity, {keep_target = true})` (also the batch step field `keep_target = true`) asks the server for the corpse by its id and leaves the rest of the player alone: the target stays on what it was and the auto-attack goes on. A mod that loots what a fight leaves behind can do so while the fight is still going on. Without the option nothing changes: the client targets the corpse, asks, and gives your previous target back when the mod is done. The corpse still has to be in the zone, a corpse, in reach (the server's rule) and the player alive, not casting and not in another window; otherwise the result is `refused` with the usual code (`not_ready`, `not_found`, `invalid_argument`, `out_of_range`).

The result says what the client did, in `ActionResult.detail.method`: `"keep_target"` (asked by id) or `"retarget"` (the old way: the client targeted the corpse and will give your target back, which ended any auto-attack). A client that does not know the option ignores it and its result has no `method`: read that as `"retarget"`. A mod that must not break an auto-attack therefore waits for the end of a fight until it has seen `method == "keep_target"` once.

How the choice is made: the client sends the by-id request first. If the server answers it with the corpse's list, by id is used for the rest of the session. If the server does not answer within three seconds (or refuses it without giving a reason) while the target was another entity, the client asks again the old way and keeps to the old way for the session; the mod's request stays in flight and ends with the answer to that second request, and `detail.method` says `"retarget"`. A request sent while the target already was the corpse proves nothing either way and never changes the choice. Both outcomes are always written to the game's log (`takeover: loot open method: ...`). An "Out of range." answer, or a refusal that says why (the server's "too far" and "someone else is looting that corpse"), is an answer: the request was understood and the choice stays open.

Everything that belongs to a hidden corpse session follows the corpse the mod asked for, not the player's target: the server's own list of the corpse after each take stays the mod's, a batch of takes asks the corpse again by id, and `loot.changed`'s `source.entity_id` names it. The mod's takeover and the hidden session work as for any `open_loot` (scope `"own"`, §7).

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

`eqoa.actions.input_text(window, text, submit)` needs `eqoa.actions.input_text`, which Mods & Addons shows as "Text boxes: can type into and submit the game's open text boxes". It types `text` (at most 512 characters, design intent; only characters the game's keyboard can type) into the text box that is open, and with `submit = true` confirms it, exactly as the player would. A password box is always refused (`permission_denied`).

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

`eqoa.actions.batch(spec)` returns `BatchId, nil`. `spec` is a list of steps, or `{steps = {...}, require_window = "merchant"|"bank"|"loot"}` (`"corpse"` is accepted for `"loot"`). There may be 1 to 64 steps (design intent, not part of the promise). Each step is a table `{kind = "<action>", ...}` with the fields listed below. It needs `eqoa.actions.batch` (or `eqoa.actions.*`) and each step's own capability (`chat` and `command` steps by rule 4; a `target_target` step counts as `assist`). A mod runs one batch at a time (`busy`).

Steps may be: `target`, `assist`, `target_target`, `interact`, `attack`, `autorun`, `stop`, `cast`, `use_item`, `equip`, `quick_slot`, `weapon_set`, `unmemorize_all`, `open_loot`, `talk`, `loot`, `loot_all`, `buy`, `sell`, `bank_move`, `bank_money`, `chat`, `command`. Any other kind is `invalid_argument`.

A step's fields are not always named as the function's arguments are (`kind` is taken by the step itself):

| Step `kind` | Fields |
|---|---|
| `target` | `target` (an entity; nil clears) |
| `open_loot` | `entity_id?` (nearest corpse when omitted), `keep_target?` (boolean; as the function's option) |
| `talk` | `entity_id?`, `kind_filter?` (`"merchant"` or `"bank"`; any friendly NPC when omitted) |
| `attack`, `autorun` | `on?` (toggles when omitted) |
| `cast` | `ability` |
| `use_item`, `equip` | `item` |
| `quick_slot` | `item`, `slot` |
| `weapon_set` | `set` |
| `loot` | `item`, `count?` (the whole stack when omitted) |
| `buy` | `item` (also accepted as `ware`), `count?` (1 when omitted) |
| `sell` | `item`, `count?` (1 when omitted) |
| `bank_move` | `item`, `count?` (1 when omitted), `from?` |
| `bank_money` | `direction`, `amount` |
| `chat`, `command` | `text` |
| `assist`, `target_target`, `interact`, `stop`, `loot_all`, `unmemorize_all` | none |

`BatchResult = {batch_id, state, ok, completed, total, current_step, code, reason, queued_tick, completed_tick, failed_step}`: `state` is `"running"`, then `"completed"`, `"refused"`, `"stopped"`, `"cancelled"` or `"unknown"`; `ok` is nil while `running` and for `unknown`, true for `completed`, false for `refused`, `stopped` and `cancelled`; `completed` counts the steps done out of `total`; `current_step` is the 1-based step in hand; `failed_step` the step that ended it. (`detail` is declared and never filled; each step's own facts are in that step's `action.result`.)

- `batch.progress` fires as the batch advances and `batch.result` once when it ends, both with the `BatchResult`. Each step also fires its own `action.result`.
- `eqoa.actions.batch_status(id:BatchId)` returns the `BatchResult`; `eqoa.actions.status` does too.
- `eqoa.actions.stop_batch(id:BatchId?)` returns `true` if it stopped the running batch, `false` if none was running. The batch ends `stopped` (code `cancelled`, reason `stopped`). A step not yet sent is never sent. A step already sent finishes: the one `batch.result` waits for the server's answer to it (at most 10 seconds) and counts the step if the server confirmed it.
- A batch ends at once, and its remaining steps are dropped and never replayed, when: a step is refused; the window it needs closes (`window_closed`, reason `merchant closed`, `bank closed` or `corpse closed`); the server does not answer a step within 10 seconds (`unknown`); it waits more than 1 second for its turn (`rate_limited`); the player zones or logs out (`cancelled`); it is stopped; or the player turns on **Use the game's windows** while the batch holds or used a full replacement of a window (§7). Disabling or reloading the mod cancels its requests and its batch without an event: a disabled mod receives nothing.

## 9. The sandbox and its limits

A mod runs in its own Lua 5.1 state, apart from other mods and from the game's memory.

**What Lua has.** The base functions (`pairs`, `ipairs`, `pcall`, `type`, `tostring`, `setmetatable`, `select`, `unpack`, ...), `string`, `table`, `math`, and `require`. From `os` only `clock`, `date`, `remove` and `rename`; from `io` only `io.open` in text modes `"r"` and `"w"` (both work on the mod's private files, like `eqoa.files`). There is no `debug`, `coroutine`, `load`, `loadstring`, `dofile`, `loadfile`, `xpcall`, `collectgarbage`, FFI or native library loading. An instruction or memory limit cannot be swallowed with `pcall`.

**`require`** loads Lua source text from the mod's own folder and its `modules/` folder (and the shared `modules/` folder of the `mods` folder), up to 1 MiB a file (design intent), never bytecode. `require("eqoa")`, `require("eqoa.<module>")` and `require("bit")` return the built-in tables.

**No network, no processes, no other files.** A mod cannot open sockets or URLs, start programs, read the clipboard or reach anything on disk outside the places below. Chat lines and game actions travel over the game's own connection, as the player's do.

**Files a mod can reach.** Paths are relative, use `/`, and may not contain `..`, a drive or a leading slash (`permission_denied`).

| Path | Is | Access |
|---|---|---|
| `package_resources/...` | The mod's own `resources/` folder | Read. A write goes to a private copy that only this mod sees from then on. |
| `resources/...` | The shared `resources/` folder of the `mods` folder (fonts and art shipped with the client) | Read. A write goes to a private copy, as above. |
| anything else, for example `notes.txt` or `data/journal.json` | The mod's private data | Read and write. |
| `@<mod id>/data/...` | Another mod's shared file | Read, with `eqoa.files.load_json` only (§5). |

The files in the `mods` folder itself are never changed by a mod: its private data and private copies live in the player's configuration folder, keyed by the mod's `id`. Removing a file that came from `resources/` or `package_resources/` hides it for this mod only (and it stays hidden after a restart); writing it again brings back a private copy. A bare file name given to `eqoa.textures.load`, `eqoa.fonts.load` or `eqoa.sound.load` is looked for in `package_resources/`, then `resources/`. Links and junctions are refused. Listing, private writes and font and sound loading work on Windows, the published platform, and answer `unsupported` elsewhere.

**Limits.** The table lists every limit a test of the client pins or exercises; these are part of the contract in this sense: a mod within them works. It is the only place on this page that calls a number tested, and a test checks each row against the constant in the code and the test that covers it.

| Limit | Value | |
|---|---|---|
| Lua memory per mod | 8 MiB; 128 MiB with `"large_data"` | tested |
| Lua instructions per mod per frame, all handlers together | 4 million | tested |
| A private file; all private data of a mod, settings included | 1 MiB; 16 MiB | tested |
| Files open at once through `io.open` | 16 | tested |
| HUD draw calls per mod per frame (`eqoa.hud.text` and the like) | 256 | tested |
| ImGui calls in one run of a mod's handlers | 131,072 | tested |
| Drawn ImGui geometry per mod per frame | 524,288 index units (an outlined shape costs up to 24 a point, so about 21,000 points) | tested |
| ImGui scopes open at once (`Begin`, `BeginChild`, pushes) | 64 | tested |
| Actions waiting per mod; dispatched per mod per game step; for all mods per step | 32; 8, refilled one a step; 64 | tested |
| An action waiting for its turn; an action waiting for the server's answer | `rate_limited` after 1 second; `unknown` after 10 seconds | tested |
| Action results kept per mod | 256 | tested |
| A chat or command line | shorter than 128 UTF-16 units | tested |
| A waypoint label | 128 bytes | tested |
| Undelivered events per mod; events delivered per frame | 1,024 (2 MiB); 256 | tested |
| JSON load: file size, depth, values, loads at once | 16 MiB, 32, two million, 2 | tested |
| JSON save: size | 14 MiB | tested |
| Commands waiting per sender (`eqoa.mods.send`); a command's `args` | 4; 4 KiB | tested |
| Fonts and sounds held | 16 each | tested |
| Imported copies kept | 3 | tested |

The ImGui call count starts afresh for each run of a mod's handlers: once for the entry and the frame's events, once for `render`, once for `settings.render`. Two of these rows rest on tests that show the limit is enforced without hitting the number exactly: geometry (a batch of 312,000 units is accepted and a second one refused) and JSON save (a value just under 14 MiB is saved); the numbers are the code's constants. The test of the 256-mod limit needs a graphics adapter.

Design intent, not part of the promise (the client is tuned this way today and no test pins the number): textures up to 2,048 pixels a side, 128 per mod, 64 MiB decoded; media files up to 16 MiB; fonts and sounds up to 64 MiB together; a font size up to 256; `require` nested 32 deep and 1 MiB a source file; settings 32 deep and 1 MiB; 1,024 entries per listing; the last 1,000 chat lines kept and asked for at once; 64 log lines between frames; 128 subscriptions; 32 option buttons; 32 pointer areas per frame; 20 wheel steps a call; four map views of 16 to 1,024 pixels; 32 entries in a manifest list; batches of 1 to 64 steps; 512 characters typed into a text box; a command's `args` 8 levels deep and 256 values; the timing of diagnostics (§2); and for hidden window sessions (§7) the 25-unit walk-away, the 15-second idle time, the 10-second wait and the 3-second re-list hold.

**How many mods run at once is a safety cap, not a number to build on.** The client limits the mods running together to a cap that a build may raise or lower; it is in no table and no part of the promise. Every limit above is per mod, and each mod has budgets of its own (Lua memory, decoded textures, queued events, instructions per frame): there is no memory cap across all mods, so what many mods cost is the sum of what each uses. The instruction budget counts Lua only: time spent inside `eqoa.*` calls is not charged to it. The limits the mods share (64 dispatches a step, one client question at a time) are divided between the mods that run, so a mod must not count on all of one.

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

**The Mods & Addons key.** The client itself has one row of this kind, **Mods & Addons**, default `shift+f10` (F10 is the game's Settings; a saved `f10` for this row moves to the default once, while the game has F10): it opens and closes the Mods & Addons window, follows every rule above, and exists with no mod enabled. It does nothing on the loading screen, with the full map up, or while the world changes.

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
| 8 | The A rounds of the UI plan. A1 (the modern UI's contract): window sessions (`eqoa.windows.open_session`, `session`, `close_session`, `pass`, `focus`, `release_focus`, `confirm`), the events `windows.session`, `windows.key` and `confirm.closed`, the actions `destroy`, `link_item` and `hide_look`, `drop_quest` now supported behind the player's own question, and the manifest's `provides`. A8 (crafting): `eqoa.craft.get` and the actions `craft_list`, `craft_inspect`, `craft_tray_add`, `craft_tray_remove`, `craft_tray_clear`, `craft_fill`, `craft_combine` and `craft_keep`, with the window kinds `recipe_book` and `experiment`. A10 (mail and the auction): `eqoa.mail.get`, `eqoa.auction.get` and the actions `mail_refresh`, `mail_read`, `mail_delete`, `mail_take`, `mail_send`, `auction_search`, `auction_page`, `auction_my_lots`, `auction_my_bids`, `auction_bidders`, `auction_bid`, `auction_buyout` and `auction_sell`, with the window kinds `mail_inbox`, `mail_compose` and `auction_search`; delete, bid, buyout, sell and a mail that carries tunar or an item ask the player first. A3 (frame data): effects with time left (`eqoa.effects.buffs`, `pet`), casts (`eqoa.casts`, `cast.started`, `cast.ended`), quest goals, training points, breath, target title and consider colour, raid roles; the actions `group_invite`, `pet_attack`, `pet_back_off`, `emote`, `jump`, `remove_effect`. A7 (community): `eqoa.social`, `eqoa.guild`, `eqoa.who`, `eqoa.lfg`, `friend_request` and `guild_invite` takeover with 45-second auto-refusal, optional-target `group_invite`, community actions (`friend_add`, `friend_remove`, `friend_accept`, `friend_refuse`, `ignore_add`, `ignore_remove`, `guild_invite`, `guild_promote`, `guild_demote`, `guild_kick`, `guild_set_motd`, `guild_accept`, `guild_refuse`), and client-drawn confirmations for `guild_kick`, `friend_accept` and `guild_accept`. A5 (the character window): `eqoa.actions.train_stats`, `request_factions` and `request_combat_record`; `mastery_buy` and `mastery_refund` supported behind the player's own question; the `"trainer"` and `"mastery"` window sessions; `eqoa.factions.list`, `eqoa.combat.record` and `Training.caps`. A9 (trade, duels, blacksmith): `eqoa.trade.get`, `eqoa.duel.get`, `eqoa.blacksmith.get`, the takeover kinds `trade_ask`, `trade`, `duel_invite`, `duel` and `blacksmith`, the actions `trade_answer`, `trade_add`, `trade_remove`, `trade_money`, `trade_accept` (behind the player's own question), `trade_unaccept`, `trade_cancel`, `duel_answer`, `blacksmith_repair` and `blacksmith_close`, and the events `trade.request`, `trade.changed`, `trade.accepted`, `trade.closed`, `duel.changed` and `blacksmith.changed`. A12 (world overlays, additive): `eqoa.camera.project`, `eqoa.camera.project_entity` and the `eqoa.hud.hide` elements `name_plates`, `damage_numbers` and `target_rings`. A11 (mercenary windows, the dye picker and fishing): `eqoa.merc.get`, `eqoa.dye.get`, `eqoa.fishing.get`, the seventeen actions of the section above, the takeover kinds `mercenary_items`, `mercenary_spells`, `dye_picker` and `fishing` (scope `all`), `confirm.closed` kinds `merc_dismiss` and `dye_pick`. |
| 9 | A2 (NPC dialogs and offers, additive): the takeover kinds `text_dialog`, `response_dialog`, `group_invite`, `revive_offer` and `summon_offer` (scope `all` only), the session's `data`, the event reasons `answered`, `walked_away` and `idle`, `eqoa.actions.dialog_answer` and `close_window` taking a session `ref`, `confirm.closed` kinds `revive_offer` and `summon_offer`. The Stat Window hide (additive): the `eqoa.hud.hide` element `stat_window`. |

## 13. What the client cannot give

The client can only report what the server sends it. There is no list of everything in a zone, only what is in view; no health, disposition or target for entities it has not been told about; no stack count or caster for effects, and a duration only where the server sends the buff window or the pet's effects; no quest objectives or progress except through the quest tracker where the server sends it (the journal holds titles); no word of a cast being interrupted or landing; no exact time remaining on a cooldown, only the lockout and the client's own estimate. Such things are nil, never guessed.

There is no access to the game's memory, no FFI, no packet API. Mods do not run in the browser build. Automation is allowed, but there is no route-finding: a mod that wants to navigate has only positions and, once built, the movement controls of §8.
