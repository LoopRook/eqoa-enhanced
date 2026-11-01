--[[
avoids_pet_frame -- part of the Modern UI Mods pack by Avoids.

Returns the mod table the pack's dispatcher drives,
{ display_name, Update, Settings, OnDisable, Save?, Load? }. State lives in a global
table so it survives a script reload, the same as when every mod was its own file.
]]

local Util = require("frontiers_forge.util")
local UI = require("frontiers_forge.ui")
local Player = require("frontiers_forge.player")
local EntityList = require("frontiers_forge.entity_list")
local AbilityList = require("frontiers_forge.ability_list")
local AvoidsPersist = require("avoids_persist")

--[[
Modern Pet Frame

There's no dedicated pet module or "is this entity my pet" flag anywhere in the API. Instead,
this detects your pet by cross-referencing nearby entity names against your own known ability
names -- confirmed working because a summoned pet's entity name exactly matches the name of the
ability that summoned it (e.g. casting "Walking Bones" creates an entity ALSO named "Walking
Bones"). This is a real observed pattern, not a guess, but it does rest on an assumption: that
EVERY summon-type ability in this game follows the same naming convention. If you ever summon a
pet and this frame doesn't detect it, that assumption may not hold for that specific summon --
let me know and we can look into it further.

A distance cutoff is also applied (default 30 units) so a coincidental name match on some faraway
NPC doesn't get mistaken for your pet.
]]
modern_pet_frame_state = modern_pet_frame_state or {
    initialized                 = false,
    callbacks_registered         = false,

    settings_file_loaded        = false,

    show_pet_frame                = true,
    disable_in_start_menu         = false,
    max_pet_distance               = 75,  -- increased from 30 -- pets pathing around terrain can realistically fall this far behind during normal play
    pet_grace_period_seconds        = 5,   -- if the pet isn't confirmed for a moment (lag, desync, briefly out of range), keep showing its last known state for this long instead of vanishing

    window_transparent              = true,  -- removes the window's own background; only the health bar and name chip keep their own dark backing
    disable_default_pet_panel       = true,

    background_color              = {0.08, 0.08, 0.08, 0.85},
    show_border                   = true,
    border_color                  = {0, 0, 0, 1},
    border_thickness              = 1.5,
    corner_rounding                 = 6,
    font_scale                    = 1.0,
    bold_text                     = false,

    bar_width                     = 180,
    bar_height                    = 22,
    health_fill_color              = {0.2, 0.85, 0.2, 1.0},
    low_health_color                = {0.9, 0.2, 0.2, 1.0},
    low_health_threshold_percent     = 30,
    health_bar_bg_color             = {0.15, 0.15, 0.15, 1.0},
    show_health_text                = true,

    show_debug_info                 = false, -- shows every candidate name match considered, for troubleshooting
}


local PERSISTED_SETTINGS = {
    { key = "show_pet_frame",                type = "bool" },
    { key = "disable_in_start_menu",          type = "bool" },
    { key = "max_pet_distance",               type = "number" },
    { key = "pet_grace_period_seconds",         type = "number" },
    { key = "window_transparent",              type = "bool" },
    { key = "disable_default_pet_panel",        type = "bool" },

    { key = "background_color",              type = "color" },
    { key = "show_border",                   type = "bool" },
    { key = "border_color",                  type = "color" },
    { key = "border_thickness",              type = "number" },
    { key = "corner_rounding",                 type = "number" },
    { key = "font_scale",                    type = "number" },
    { key = "bold_text",                     type = "bool" },

    { key = "bar_width",                     type = "number" },
    { key = "bar_height",                    type = "number" },
    { key = "health_fill_color",              type = "color" },
    { key = "low_health_color",                type = "color" },
    { key = "low_health_threshold_percent",      type = "number" },
    { key = "health_bar_bg_color",              type = "color" },
    { key = "show_health_text",                 type = "bool" },

    { key = "show_debug_info",                type = "bool" },
}

-- Settings live in the UiForge profile now. This is the file older builds wrote, read
-- once so an existing setup carries over, then deleted.
local LEGACY_SETTINGS_FILE = tostring(UiForge.resources_path) .. "\\pet_frame_settings.cfg"

-- Save, Load, and the one time legacy read all come from the settings list above.
local Save, Load, LoadSettingsOnce = AvoidsPersist.Bind({
    state = modern_pet_frame_state,
    defs = PERSISTED_SETTINGS,
    legacy_path = LEGACY_SETTINGS_FILE,
})

--[[
Known ability names, built once at Initialize() and refreshable via a Settings button (in case
you learn new abilities later). Used as the cross-reference set for pet detection.
]]
known_ability_names = known_ability_names or {}
local known_ability_names_populated = false

local function RefreshKnownAbilityNames()
    known_ability_names = {}
    local ok, count = pcall(AbilityList.GetCount)
    if not ok then return end

    for index = 1, count do
        local ok_ability, ability = pcall(AbilityList.GetAbilityByIndex, index)
        if ok_ability and ability ~= nil then
            local ok_name, name = pcall(function() return ability:GetName() end)
            if ok_name and name ~= nil and name ~= "" then
                known_ability_names[name] = true
                known_ability_names_populated = true
            end
        end
    end
end

local function ToggleDefaultPetPanel(owner)
    if modern_pet_frame_state.disable_default_pet_panel == true then
        UI.DisablePetPanel(owner)
        return
    end
    UI.EnablePetPanel(owner)
end

local function OnDisable()
    modern_pet_frame_state.show_pet_frame = false
    -- Always restore the native pet panel here, regardless of the "Disable Default Pet Panel"
    -- setting.
    UI.EnablePetPanel(owner)
end

local function Initialize()
    LoadSettingsOnce()
    RefreshKnownAbilityNames()
    modern_pet_frame_state.initialized = true
end

local function Settings()
    local state = modern_pet_frame_state

    state.show_pet_frame = ImGui.Checkbox("Show Pet Frame", state.show_pet_frame)
    ImGui.TextUnformatted("(Only actually shows when a pet is detected nearby, regardless of this.)")
    state.disable_in_start_menu = ImGui.Checkbox("Hide While Start Menu Is Open", state.disable_in_start_menu)
    state.max_pet_distance = ImGui.SliderInt("Max Pet Detection Distance", state.max_pet_distance, 5, 200, tostring(state.max_pet_distance))
    state.pet_grace_period_seconds = ImGui.SliderInt("Grace Period If Briefly Lost (seconds)", state.pet_grace_period_seconds, 0, 30, tostring(state.pet_grace_period_seconds))
    ImGui.TextUnformatted("(Keeps showing the pet's last known state briefly if it falls out")
    ImGui.TextUnformatted("of range/lags, instead of the frame vanishing immediately.)")
    state.window_transparent = ImGui.Checkbox("Window Transparent (only text/bar have background)", state.window_transparent)

    local new_val, pressed = ImGui.Checkbox("Disable Default Pet Panel", state.disable_default_pet_panel)
    if pressed then
        state.disable_default_pet_panel = new_val
        ToggleDefaultPetPanel()
    end

    if ImGui.Button("Refresh Known Ability Names") then
        RefreshKnownAbilityNames()
    end
    ImGui.TextUnformatted("(Run this if you've learned new abilities since loading this script.)")

    ImGui.Separator()
    ImGui.Text("Appearance")
    state.background_color = ImGui.ColorEdit4("Background Color", state.background_color)
    state.show_border = ImGui.Checkbox("Show Border", state.show_border)
    state.border_color = ImGui.ColorEdit4("Border Color", state.border_color)
    state.border_thickness = ImGui.SliderFloat("Border Thickness", state.border_thickness, 0.5, 5.0, tostring(state.border_thickness))
    state.corner_rounding = ImGui.SliderFloat("Corner Rounding", state.corner_rounding, 0, 20, tostring(state.corner_rounding))
    state.font_scale = ImGui.SliderFloat("Font Scale", state.font_scale, 0.5, 3.0, tostring(state.font_scale))
    state.bold_text = ImGui.Checkbox("Bold Text", state.bold_text)

    ImGui.Separator()
    ImGui.Text("Health Bar")
    state.bar_width = ImGui.SliderInt("Bar Width", state.bar_width, 80, 400, tostring(state.bar_width))
    state.bar_height = ImGui.SliderInt("Bar Height", state.bar_height, 10, 50, tostring(state.bar_height))
    state.health_fill_color = ImGui.ColorEdit4("Health Fill Color", state.health_fill_color)
    state.low_health_color = ImGui.ColorEdit4("Low Health Color", state.low_health_color)
    state.low_health_threshold_percent = ImGui.SliderInt("Low Health Threshold (%)", state.low_health_threshold_percent, 0, 100, tostring(state.low_health_threshold_percent))
    state.health_bar_bg_color = ImGui.ColorEdit4("Health Bar Background", state.health_bar_bg_color)
    state.show_health_text = ImGui.Checkbox("Show Health Text", state.show_health_text)

    ImGui.Separator()
    state.show_debug_info = ImGui.Checkbox("Show Debug Info (troubleshooting)", state.show_debug_info)

end

-- Finds the nearby entity most likely to be your pet: a name matching a known ability, within
-- max_pet_distance, picking the CLOSEST match if somehow more than one qualifies.
local function FindPetEntity(state, player_coords)
    local best_entity = nil
    local best_distance = nil
    local candidates_checked = {}

    local entities = EntityList.GetAllEntities()
    for _, entity in ipairs(entities) do
        if entity.name ~= nil and entity.name ~= "" and known_ability_names[entity.name] then
            local dx = entity.x - player_coords.x
            local dz = entity.z - player_coords.z
            local distance = math.sqrt(dx * dx + dz * dz)

            table.insert(candidates_checked, entity.name .. " (dist=" .. string.format("%.0f", distance) .. ")")

            if distance <= state.max_pet_distance then
                if best_distance == nil or distance < best_distance then
                    best_entity = entity
                    best_distance = distance
                end
            end
        end
    end

    return best_entity, best_distance, candidates_checked
end

local function DrawHealthBar(state, health_percent)
    local draw_list = ImGui.GetWindowDrawList()
    local origin_x, origin_y = ImGui.GetCursorScreenPos()
    local p0 = ImVec2.new(origin_x, origin_y)
    local p1 = ImVec2.new(origin_x + state.bar_width, origin_y + state.bar_height)

    local bg_color_u32 = ImGui.GetColorU32(state.health_bar_bg_color[1], state.health_bar_bg_color[2], state.health_bar_bg_color[3], state.health_bar_bg_color[4])
    draw_list:AddRectFilled(p0, p1, bg_color_u32, state.corner_rounding)

    local fill_color = (health_percent <= state.low_health_threshold_percent) and state.low_health_color or state.health_fill_color
    local fill_color_u32 = ImGui.GetColorU32(fill_color[1], fill_color[2], fill_color[3], fill_color[4])
    local fill_width = state.bar_width * math.max(math.min(health_percent, 100), 0) / 100
    if fill_width > 0 then
        draw_list:AddRectFilled(p0, ImVec2.new(origin_x + fill_width, origin_y + state.bar_height), fill_color_u32, state.corner_rounding)
    end

    if state.show_border then
        local border_color_u32 = ImGui.GetColorU32(state.border_color[1], state.border_color[2], state.border_color[3], state.border_color[4])
        draw_list:AddRect(p0, p1, border_color_u32, state.corner_rounding, 0, state.border_thickness)
    end

    if state.show_health_text then
        local text = string.format("%.0f/100", health_percent)
        local text_width, text_height = ImGui.CalcTextSize(text)
        local text_color_u32 = ImGui.GetColorU32(1, 1, 1, 1)
        draw_list:AddText(ImVec2.new(origin_x + (state.bar_width - text_width) / 2, origin_y + (state.bar_height - text_height) / 2), text_color_u32, text)
    end

    ImGui.Dummy(state.bar_width, state.bar_height)
end

local function Render()
    local state = modern_pet_frame_state

    if Util.IsInGame() == 0 then return end
    if state.disable_in_start_menu and Util.IsStartMenuOpen() == 1 then return end

    local ok_player_coords, player_coords = pcall(Player.GetCoordinates)
    if not ok_player_coords or player_coords == nil then return end

    local pet_entity, pet_distance, candidates = FindPetEntity(state, player_coords)

    -- Sticky cache: a pet naturally lags behind while pathing around terrain, and can
    -- temporarily fall outside max_pet_distance or briefly desync from the entity list for a
    -- frame or two. Without this, the frame would flicker or vanish every time that happens,
    -- even though the pet is still alive and following you. Instead, keep showing its last
    -- known name/level/HP for a grace period before actually treating it as gone (dismissed,
    -- died, or genuinely out of range for a sustained time).
    pet_sticky_cache = pet_sticky_cache or { snapshot = nil, last_seen_time = nil }

    local display_entity = nil
    if pet_entity ~= nil then
        pet_sticky_cache.snapshot = { name = pet_entity.name, level = pet_entity.level, percent_hp = pet_entity.percent_hp }
        pet_sticky_cache.last_seen_time = os.clock()
        display_entity = pet_entity
    elseif pet_sticky_cache.snapshot ~= nil and pet_sticky_cache.last_seen_time ~= nil then
        if os.clock() - pet_sticky_cache.last_seen_time <= state.pet_grace_period_seconds then
            display_entity = pet_sticky_cache.snapshot
        end
    end

    if state.show_debug_info then
        if ImGui.Begin("Pet Frame Debug") then
            ImGui.TextUnformatted("Known ability names loaded: " .. (function()
                local n = 0
                for _ in pairs(known_ability_names) do n = n + 1 end
                return n
            end)())
            ImGui.TextUnformatted("Candidate name matches this frame:")
            if #candidates == 0 then
                ImGui.TextUnformatted("  (none)")
            end
            for _, c in ipairs(candidates) do
                ImGui.TextUnformatted("  " .. c)
            end
            if pet_entity == nil and display_entity ~= nil then
                local age = os.clock() - pet_sticky_cache.last_seen_time
                ImGui.TextUnformatted(string.format("Showing cached data (last confirmed %.1fs ago)", age))
            end
        end
        ImGui.End()
    end

    if state.show_pet_frame ~= true then return end
    if display_entity == nil then return end -- auto-hide entirely when no pet detected AND the grace period has elapsed

    local window_flags = ImGuiWindowFlags.AlwaysAutoResize + ImGuiWindowFlags.NoTitleBar
    if state.window_transparent then window_flags = window_flags + ImGuiWindowFlags.NoBackground end

    if ImGui.Begin("Pet Frame", true, window_flags) then
        ImGui.SetWindowFontScale(state.font_scale)

        -- Drawn manually (not as plain ImGui.Text widgets) so we can give the text its own dark
        -- background chip -- needed since the window itself has no background when
        -- window_transparent is on, and floating text with nothing behind it can be hard to read
        -- over bright/busy game scenery.
        local name_line = display_entity.name .. "  (Lvl " .. display_entity.level .. ")"
        local origin_x, origin_y = ImGui.GetCursorScreenPos()
        local draw_list = ImGui.GetWindowDrawList()
        local text_width, text_height = ImGui.CalcTextSize(name_line)

        local chip_padding = 3
        local chip_color_u32 = ImGui.GetColorU32(0, 0, 0, 0.7)
        draw_list:AddRectFilled(
            ImVec2.new(origin_x - chip_padding, origin_y - chip_padding),
            ImVec2.new(origin_x + text_width + chip_padding, origin_y + text_height + chip_padding),
            chip_color_u32, state.corner_rounding)

        local name_color_u32 = ImGui.GetColorU32(1, 1, 1, 1)
        draw_list:AddText(ImVec2.new(origin_x, origin_y), name_color_u32, name_line)
        ImGui.Dummy(text_width, text_height)

        DrawHealthBar(state, display_entity.percent_hp * 100)
    end
    ImGui.End()
end

local function Update()
    if modern_pet_frame_state.initialized == false then Initialize() end

    -- Both retried every frame while not yet successful/while in-game -- the ability bar taught us
    -- the first attempt at reading game data or applying a native UI toggle can silently fail if it
    -- runs before the game is fully ready, with nothing to retry it afterward otherwise.
    if Util.IsInGame() ~= 0 then
        if not known_ability_names_populated then
            RefreshKnownAbilityNames()
        end
        ToggleDefaultPetPanel()
    end

    if modern_pet_frame_state.initialized then Render() end
end

return {
    display_name = "Pet Frame",
    Update = Update,
    Settings = Settings,
    OnDisable = OnDisable,
    Save = Save,
    Load = Load,
}
