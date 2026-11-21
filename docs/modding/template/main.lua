-- Starter mod for EQOA Enhanced (api "eqoa/1", entryMode "once").
-- It shows a small window with your level and health once you are in the world.
-- Copy this folder into your mods folder, rename the folder and change "id"
-- in mod.json to something unique. In the game, open Mods & Addons (F10),
-- click "Rescan folder" and enable the mod.

local ui = eqoa.imgui

-- Retained across disable/enable in the same VM; initialise deliberately.
my_first_mod_state = my_first_mod_state or { shown = 0 }
local state = my_first_mod_state

-- Save and load plain settings (numbers, strings, booleans, plain tables only).
eqoa.settings.on_save(function() return state end)
eqoa.settings.on_load(function(saved)
    if type(saved) == "table" and type(saved.shown) == "number" then
        state.shown = saved.shown
    end
end)

-- Feature-test before using anything beyond the basics.
local can_list_effects = eqoa.capabilities.has("eqoa.effects.list")

-- Draw only inside the "render" event in "once" mode.
eqoa.events.on("render", function()
    -- Queries return value, nil  or  nil, Error. An empty answer is not an error.
    local player = eqoa.player.get()
    if not player then return end          -- not in a world yet

    if ui.Begin("My First Mod") then
        ui.Text("level " .. tostring(player.level or "?"))
        if player.hp then                  -- absent HP is not zero HP
            ui.Text("HP " .. tostring(player.hp.current) .. " / " .. tostring(player.hp.maximum))
        end
        ui.Text(can_list_effects and "effects: available" or "effects: not available here")
    end
    ui.End()                               -- always pair End with Begin
end)

-- Clean up anything you hold when the mod is switched off.
eqoa.events.on("disabling", function()
    eqoa.log.write("info", "my_first_mod disabling")
end)
