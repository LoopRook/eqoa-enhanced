--[[
avoids_target_level -- part of the Modern UI Mods pack by Avoids.

Returns the mod table the pack's dispatcher drives,
{ display_name, Update, Settings, OnDisable, Save?, Load? }. State lives in a global
table so it survives a script reload, the same as when every mod was its own file.
]]

local Util      = require("frontiers_forge.util")
local Player    = require("frontiers_forge.player")
local EntityList = require("frontiers_forge.entity_list")

local function DisplayTargetLevel()
    -- Util.IsInGame() returns a number (0 or 1), not a real Lua boolean -- "not Util.IsInGame()"
    -- would never be true even when not in-game, since 0 is truthy in Lua.
    if Util.IsInGame() == 0 then return end

    -- AlwaysAutoResize (used elsewhere in this pack) forces the window back to fit its
    -- content every frame, which overrides any manual resize -- deliberately NOT used here
    -- so you can drag-resize this box. SetNextWindowSize only sets the STARTING size
    -- (FirstUseEver), so resizing it yourself sticks after that.
    ImGui.SetNextWindowSize(220, 80, ImGuiCond.FirstUseEver)
    local window_flags = ImGuiWindowFlags.NoTitleBar
    if ImGui.Begin("Target Level", true, window_flags) then
        local entity = EntityList.GetEntityById(Player.GetTargetEntityId())
        if entity == nil then
            ImGui.Text("Target: (none)")
            ImGui.Text("Level: --")
        else
            ImGui.Text("Target: " .. entity.name)
            ImGui.Separator()
            ImGui.Text("Level: " .. tostring(entity.level))
        end
    end
    ImGui.End()
end

return {
    display_name = "Target Level",
    Update = DisplayTargetLevel,
    Settings = nil,   -- no settings panel for this simple mod
    OnDisable = nil,  -- no native UI to restore
}
