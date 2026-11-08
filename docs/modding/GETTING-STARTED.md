# Writing an EQOA Enhanced mod

The native API is `eqoa`. Round 2 is under implementation and review; `eqoa/1`
becomes a stability promise after the Round 5 audit and approved release.
See [the specification](EQOA-API-SPEC.md) for the exact contracts.

Create a folder under the active `mods` directory with `mod.json` and `main.lua`:

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

```lua
eqoa.events.on("render", function()
    local player, err = eqoa.player.get()
    if not player or not player.hp then return end
    local ui = eqoa.imgui
    if ui.Begin("My Health") then
        ui.Text(tostring(player.hp.current) .. " / " .. tostring(player.hp.maximum))
    end
    ui.End()
end)
```

Rescan in the Addon Manager, then enable your mod. Errors and unsupported calls
appear there and in `mods/addons_error.log`, including errors caught by `pcall`.
Reload creates a fresh VM. Disable releases the mod's HUD and input claims and
stops its sound. Ordinary re-enable retains globals and executes the entry again.

Use `entryMode: "once"` for event-based mods. Without it the entry is the render
callback and runs every presentation frame; subscribing to `render` in that mode
fails visibly. State events work in both modes. Register handlers once per enable
and initialize retained globals deliberately. Draw only in the entry in frame
mode, or in `render`/`settings.render` callbacks.

Queries normally return `value, nil`, or `nil, Error`. A missing optional field
is nil. Zero HP is real data; absent HP is not zero HP. References are opaque and
expire across worlds or record replacement. Reacquire them after `state.reset`.
Native arrays and positional indexes start at 1, times are seconds, headings are
radians, and positions are native game units. Forge compatibility keeps its own
older naming, units and index conventions.

Probe features without side effects:

```lua
if eqoa.capabilities.has("eqoa.effects.list") then
    local effects, err = eqoa.effects.list()
    -- nil/unavailable means the server has not sent effect state yet.
end
```

`not_yet` means a scheduled feature has not landed; `unsupported` means an
excluded feature or unavailable platform backend. Capability support does not
mean a player, asset or window is currently available. Items and transactions
arrive in Round 3; map, mastery, camera, dialogs and general actions in Round 4.
Use the running client's capability list as the implementation inventory.

Save plain settings through `eqoa.settings.on_save`/`on_load`, or explicit
`save`/`load`. They use the manager's active addon/profile slot. Settings contain
only finite numbers, strings, booleans, nil and plain acyclic tables with string
keys or dense array indexes. Handles and functions cannot be saved.

`eqoa.files` paths are relative to private data. `resources/` selects this mod's
overlay over read-only shared bundled resources; `package_resources/` selects
its own package resources and overlay. Writes never alter either backing tree.
Removing a fallback creates a private tombstone that survives restart. Other
mods retain their own fallback. Bare media names search package then shared
resources through the same overlays. Absolute host paths, traversal, other mods,
links/junctions, networking, raw memory and process launching are excluded.

Start with [the bundled hearts example](../../bundled-mods/eqoa_example_hearts/main.lua).
Its ten hearts use ImGui draw lists, real player health, lifecycle cleanup and
profile settings, with no Forge dependency. Live-game testing is still required.
