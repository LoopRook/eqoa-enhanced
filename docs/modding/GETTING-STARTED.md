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
mean a player, asset or window is currently available. Items (reading the pack,
bank, corpse and merchant) are in; buying, selling, looting and moving items are
actions, which come with the rest of them. Use the running client's capability
list as the implementation inventory.

## Items

`eqoa.inventory.get()`, `eqoa.bank.get()`, `eqoa.loot.get()` and
`eqoa.merchant.get()` each return a container: `{kind, revision, open, items,
capacity, used, remaining, full, money, merchant}`. The pack and the bank are
always there once you are in a world (the bank's contents arrive with the world,
not at the banker; `open` says whether its window is up). `loot.get()` and
`merchant.get()` return `nil, nil` while their window is closed, never a corpse
or a shop from before. Positions (`item.index`) start at 1 and change as the
list compacts; keep an `item.ref` to follow a stack: it names the same stack
after others are taken, and `eqoa.items.get(ref)` answers `stale_handle` once it
is gone. References are not saved and do not survive zoning.

```lua
local function show(c)
    if not c then return end
    for _, item in ipairs(c.items) do
        local ok = eqoa.items.can_use(item.ref)  -- class, race, level
        local label = item.name .. (item.quantity > 1 and (" x" .. item.quantity) or "")
        if ok and not ok.allowed then label = label .. "  (" .. table.concat(ok.reasons, ", ") .. ")" end
        eqoa.imgui.Text(label)
    end
    eqoa.imgui.Text(tostring(c.used) .. " used, " .. tostring(c.money or 0) .. " tunar")
end

eqoa.events.on("loot.changed", function(e)
    -- e.data.cause is "open", "close", "reset" (zoning) or the game's own event
    -- ("loot_one", "loot_all"); before/after are containers, either may be nil
    if e.data.cause == "open" then eqoa.log.write("info", "a corpse with " .. e.data.after.used .. " items") end
end)
```

`inventory.changed`, `bank.changed`, `loot.changed` and `merchant.changed` fire
once per change with `{before, after, changed, cause}`. Prices come from the
game's own rounding: `eqoa.merchant.price(ref, "buy" or "sell", quantity)`.
`eqoa.items.slots`, `.stats` and `.attack_types` are the item enums by name, and
`eqoa.gems.get/is_gem/format` look up the crafting gems.

Not given yet: the item page's tab and selection (`Container.view`) and the
merchant window's mode and selection (`MerchantState.mode`, `options`); asking
for them says so in the Addon Manager. FrontiersForge's `inventory`, `item`,
`bank`, `loot`, `merchant` and `gems` modules read the same data with their own
shapes (tab enum 0..3, `bank.GetItems` as `{idx, slot, level, name, amount}`).

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

`eqoa.player.experience_for_level(level)` returns the native experience threshold for the requested level (1?59), including the native formula change at level 51. It does not add one to the argument. To query the next level's threshold, pass `player.level + 1`; arguments outside 1?59 return `nil, nil`. Forge keeps its pinned experience formula.
