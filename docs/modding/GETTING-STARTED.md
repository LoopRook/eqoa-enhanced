# Writing an EQOA Enhanced mod

A mod is a folder with a small manifest and Lua code. It runs inside the game through the `eqoa` API
(`eqoa/1`). This page takes you from nothing to a working mod. The exact contracts are in
[the reference](EQOA-API-SPEC.md). Every example on this page is loaded and run for a few frames by the
client's own tests, which catch a misspelt name or an error at load; code inside a handler that waits
for a key or a result is checked for its names only.

## 1. Where mods live

The client has a `mods` folder beside its program (in the game: **Options → Extended → UI → Enhanced →
Open Mods Folder**). Each mod is one folder inside it:

```
mods/
  my_health/
    mod.json
    main.lua
```

The quickest start is to copy [`template/`](template/) into `mods`, rename the folder and change the `id`.

## 2. The manifest

`mod.json` tells the client what the mod is:

```json
{
  "schemaVersion": 1,
  "runtime": "lua",
  "api": "eqoa/1",
  "id": "my_health",
  "name": "My Health",
  "entry": "main.lua",
  "entryMode": "once"
}
```

- `id` is the mod's identity (letters, digits, `_`, `.`, `-`). Make it unique and never change it: saved
  settings and private files are kept under it.
- `entryMode: "once"` means `main.lua` runs once when the mod is enabled and registers handlers. Use it.
  (Without it the file runs every frame, a mode kept for older Forge-style code.)
- Other fields (`capabilities`, `replaces`, `shares`, `commands`, `permissions`, `enabled`) are in
  [the reference, section 2](EQOA-API-SPEC.md#2-packages-versions-capabilities-and-diagnostics).

## 3. The first mod

`main.lua`:

```lua
eqoa.events.on("render", function()
    local player = eqoa.player.get()
    if not player or not player.hp then return end   -- not in the world yet, or no data yet
    local ui = eqoa.imgui
    if ui.Begin("My Health") then
        ui.Text(tostring(player.hp.current) .. " / " .. tostring(player.hp.maximum))
    end
    ui.End()                                         -- always pair End with Begin
end)
```

Draw only inside a `render` handler. Drawing is done with ImGui (`eqoa.imgui` is the same object as the
global `ImGui`).

## 4. Turning it on, reloading, finding errors

1. Start the game and enter the world. Press **F10** to open **Mods & Addons** (the key is the
   **Mods & Addons** row on the Controls page and can be changed; the window is also reached through
   Options → Extended → UI → Enhanced → Addon Manager).
2. Click **Rescan folder** if the mod was added while the game was running. New mods start disabled.
3. The entry for your mod shows what it declares. Click **Enable**.
4. After changing the code, click **Reload** on the mod: it starts again in a fresh Lua state.

When something is wrong:

- The mod's entry shows `[Warnings: N]` and the messages are listed at the bottom of the window. A mod
  whose code raised an error shows **Failed — reload to retry**.
- The same messages are written to `addons_error.log` in the `mods` folder.
- A manifest the client cannot accept (a missing field, a bad `id`) is reported there too, and the mod
  does not appear in the list.
- Misspelt API names are reported, not ignored: reading a name the API does not have (say `gett`
  where `get` was meant) gives nil and a diagnostic with the code `unknown_symbol`. A clean mod has no
  warnings.
- Your own messages go to the game's log, `eqoa.log`:

```lua
eqoa.log.write("info", "my_health started")      -- "warn" and "error" also show in Mods & Addons
```

## 5. The rules that prevent most bugs

- **Two results.** A query returns `value, nil` or `nil, Error`. "Nothing there" (no target, a closed
  window) is `nil, nil`, which is not an error. An `Error` is `{code, message, capability}`.
- **Absent is not zero.** A value the client does not know is nil. Zero health is real; unknown health
  is nil.
- **Counting starts at 1.** Times are seconds, headings radians, positions game units.
- **Handles expire.** An `item.ref` or `entity.ref` names one thing in one world. After zoning, fetch
  again; an expired handle answers `stale_handle`.
- **Use events for changes**, not a comparison every frame.
- **Globals survive a disable.** When the player disables and enables the mod, `main.lua` runs again
  with its old globals and no handlers. Initialise on purpose.

```lua
my_health_state = my_health_state or { shown = 0 }   -- kept across disable/enable, not across Reload
local state = my_health_state

eqoa.events.on("world.entered", function(e)
    eqoa.log.write("info", "entered world " .. tostring(e.data.world_id))
end)

eqoa.events.on("state.reset", function()
    state.target = nil            -- everything read earlier is out of date: drop handles, read again
end)

eqoa.events.on("target.changed", function(e)
    state.target = e.data.after   -- an EntityRef, or nil
end)

eqoa.events.on("disabling", function()
    eqoa.log.write("info", "my_health disabling")
end)
```

## 6. Asking what the client supports

The running client answers for itself, with no side effects:

```lua
if eqoa.capabilities.has("eqoa.effects.list") then
    local effects, err = eqoa.effects.list()
    -- nil with err.code == "unavailable" means the server has not sent the effects yet
end

local c = eqoa.capabilities.get("eqoa.actions.move")
-- c.status is "supported", "partial", "not_yet", "unsupported" or "unknown"; c.reason says why
```

`supported` means the function works, not that there is data right now. `eqoa.revision` (1 today) rises
when something is added. The full list of names with their status is the generated
[support table](SUPPORT-TABLE.md).

## 7. Settings

Plain values only: numbers, strings, booleans and tables of them.

```lua
local settings = { x = 100, y = 100 }

eqoa.settings.on_save(function() return settings end)
eqoa.settings.on_load(function(saved)
    if type(saved) == "table" then
        settings.x = tonumber(saved.x) or settings.x
        settings.y = tonumber(saved.y) or settings.y
    end
end)
```

The client calls `on_load` when the mod is first enabled and `on_save` when it is disabled (and for the
**Load profile** and **Save profile** buttons in Mods & Addons). Settings are kept per player profile. For your own files use
`eqoa.files`: a small private area that belongs to the mod.

```lua
local ok, err = eqoa.files.write("notes.txt", "hello")
local text = eqoa.files.read("notes.txt")            -- "hello"
```

## 8. Reading the game

```lua
local function describe(container)
    if not container then return end
    for _, item in ipairs(container.items) do
        local line = item.name
        if item.quantity > 1 then line = line .. " x" .. item.quantity end
        local can = eqoa.items.can_use(item.ref)      -- class, race, level
        if can and not can.allowed then
            line = line .. " (" .. table.concat(can.reasons, ", ") .. ")"
        end
        eqoa.log.write("debug", line)
    end
end

describe(eqoa.inventory.get())      -- the pack: always there in the world
describe(eqoa.loot.get())           -- nil, nil unless a corpse is open

eqoa.events.on("loot.changed", function(e)
    -- e.data.cause: "open", "close", "reset" (zoning) or the game's own event ("loot_one", ...)
    if e.data.cause == "open" and e.data.after then
        eqoa.log.write("info", "a corpse with " .. e.data.after.used .. " items")
    end
end)

local windows = eqoa.windows.get()  -- always answers; mode is "none" outside the world
if windows.busy then
    eqoa.log.write("debug", "a game window has the controls: " .. windows.mode)
end
```

The reference lists everything that can be read: the player, entities, the target, the group,
abilities and hotbars, cooldowns, effects, chat, combat, items, quests, Class Mastery, the camera, the
game's windows and the map.

## 9. Keys

Give the mod a key the player can change on the game's Controls page:

```lua
local shown = true
eqoa.input.bind({ id = "toggle", label = "Show My Health", default_key = "u" }, function()
    shown = not shown
end)
```

A key is its full name: `u`, `ctrl+u` and `shift+u` are three keys. If the default is already used by a
game control the row starts empty and says so, and the player picks another. A mod key works in plain
play, not while the player types or a game window is up. The rules are in
[the reference, section 10](EQOA-API-SPEC.md#10-keys).

## 10. Doing things: actions

A mod may act in the game (target, cast, loot, sell, chat and more) on its own, without a click. Two
things are needed:

**Declare it.** List each action in `mod.json`. The player sees this list before enabling the mod.

```json
{
  "schemaVersion": 1,
  "runtime": "lua",
  "api": "eqoa/1",
  "id": "my_helper",
  "name": "My Helper",
  "entry": "main.lua",
  "entryMode": "once",
  "capabilities": ["eqoa.actions.use_item", "eqoa.actions.chat"]
}
```

`eqoa.actions.*` declares them all. Without a `capabilities` list an action returns the error
`not_declared`; with a list that lacks it, `permission_denied`. A chat line that starts with `/` needs
`eqoa.actions.command`; any other line needs `eqoa.actions.chat`.

**Wait for the result.** An action returns a request, not an outcome. The outcome arrives as an
`action.result` event.

```lua
local last

eqoa.events.on("action.result", function(e)
    local r = e.data
    if r.request_id ~= last then return end
    -- r.state: "dispatched" (sent, waiting for the server), then "completed", "refused",
    -- "cancelled" or "unknown" (the server never answered: do not assume either way)
    if r.state == "refused" then
        eqoa.log.write("warn", r.kind .. ": " .. tostring(r.reason or r.code))
    end
end)

eqoa.input.bind({ id = "use_first", label = "Use first item", default_key = "y" }, function()
    local pack = eqoa.inventory.get()
    local item = pack and pack.items[1]
    if not item then return end
    local request, err = eqoa.actions.use_item(item.ref)
    if request then last = request else eqoa.log.write("warn", err.code .. ": " .. err.message) end
end)
```

Things to know:

- Actions work only in the game world, and most need the character alive with no game window up.
- `eqoa.actions.destroy` does not exist for mods (`unsupported`); held movement and camera controls
  (`move`, `look`, `buttons`) are not built yet (`not_yet`).
- A mod can never send a bug report or type a password.
- To work at a merchant, banker or corpse without the game's window appearing, see "Taking over a
  window" in [the reference, section 7](EQOA-API-SPEC.md#7-quests-class-mastery-camera-windows-and-the-map-rounds-4-and-6);
  [the bag viewer](../../bundled-mods/eqoa_bag_viewer/main.lua) does it.

## 11. Where to go next

- [EXAMPLES.md](EXAMPLES.md): what each bundled mod shows. Start with
  [the hearts example](../../bundled-mods/eqoa_example_hearts/main.lua).
- [EQOA-API-SPEC.md](EQOA-API-SPEC.md): the reference.
- [VERSIONING.md](VERSIONING.md): what will never change under `eqoa/1`.
- Test in the live game. A mod that loads without warnings is a good start, not proof that it works.
