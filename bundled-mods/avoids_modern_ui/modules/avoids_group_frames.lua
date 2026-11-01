--[[
avoids_group_frames -- part of the Modern UI Mods pack by Avoids.

Returns the mod table the pack's dispatcher drives,
{ display_name, Update, Settings, OnDisable, Save?, Load? }. State lives in a global
table so it survives a script reload, the same as when every mod was its own file.
]]

local UI = require("frontiers_forge.ui")                -- Access UI elements
local Util = require("frontiers_forge.util")            -- Access Utility functions
local Player = require("frontiers_forge.player")        -- Identify "self" among group members, own coordinates
local Group = require("frontiers_forge.group")          -- Access group member data
local AvoidsPersist = require("avoids_persist")

--[[
Modern Group Frames

HONEST LIMITATION: group members only expose a HEALTH PERCENTAGE (0-255 raw, converted to
0-100 here), not real current/max HP numbers -- there's no equivalent to Player.GetCurrentHp()/
GetMaxHp() for other group members anywhere in the API. So "80/100" for a member here is really
their percentage scaled to 100, not a true max-HP value, unlike your own health bar script which
has real numbers. Styled to match visually (bar + fraction text), just built on different data.

Also per group.lua's own comment: the member array includes the local player as one of its
entries. This script filters "yourself" out of the list by default (identified by name match
against Player.GetName(), since there's no direct entity-id cross-reference exposed) since you
already have a dedicated health bar -- toggle "Include Myself In List" to show yourself too.

Pets: there's no separate pet module anywhere in the API -- but per your own observation, a
summoned pet occupies a regular group member slot, so it's already covered here with no extra
work needed.
]]
modern_group_frames_state = modern_group_frames_state or {
    initialized                 = false,
    callbacks_registered         = false,

    -- Settings persistence
    settings_file_loaded        = false,

    show_group_frames             = true,  -- master toggle -- the panel only actually shows when this AND Group.IsInGroup() are both true
    disable_in_start_menu         = false,
    include_self_in_list          = false,

    -- Appearance
    background_color              = {0.08, 0.08, 0.08, 0.85},
    show_border                   = true,
    border_color                  = {0, 0, 0, 1},
    border_thickness              = 1.5,
    corner_rounding                 = 6,
    font_scale                    = 1.0,
    bold_text                     = false,

    -- Health bar
    bar_width                     = 180,
    bar_height                    = 22,
    health_fill_color              = {0.2, 0.85, 0.2, 1.0},
    low_health_color                = {0.9, 0.2, 0.2, 1.0},
    low_health_threshold_percent     = 30,
    health_bar_bg_color             = {0.15, 0.15, 0.15, 1.0},
    show_health_text                = true,
    health_text_format               = "percent_100", -- "percent_100" shows "80/100", "percent" shows "80%"

    -- Inactive (out of range / not currently tracked) members
    show_inactive_members            = true,
    inactive_alpha                  = 0.4,

    -- Distance
    show_distance                  = true,
    far_away_threshold              = 500, -- beyond this, just show "Far Away" instead of a specific number
    distance_text_color              = {0.7, 0.7, 0.7, 1.0},

    -- Leader
    show_leader_indicator            = true,
    leader_indicator_color            = {1.0, 0.85, 0.2, 1.0},

    -- Header
    show_member_count_header          = true,

    show_debug_info                 = false, -- shows raw Group.IsInGroup()/GetMemberCount() even when the panel would otherwise auto-hide, for troubleshooting

    disable_default_group_display     = true,
    window_transparent                = true,  -- removes the window's own background; name/health bar keep their own dark backing
}


--[[
Settings persistence

The options below ride in the UiForge profile. Save them with File then Save
Profile, restore them with File then Select Profile. This one list drives both
halves of that, so the payload can never drift from what the settings panel edits.
]]
local PERSISTED_SETTINGS = {
    { key = "show_group_frames",              type = "bool" },
    { key = "disable_in_start_menu",           type = "bool" },
    { key = "include_self_in_list",            type = "bool" },

    { key = "background_color",               type = "color" },
    { key = "show_border",                    type = "bool" },
    { key = "border_color",                   type = "color" },
    { key = "border_thickness",               type = "number" },
    { key = "corner_rounding",                  type = "number" },
    { key = "font_scale",                     type = "number" },
    { key = "bold_text",                      type = "bool" },

    { key = "bar_width",                      type = "number" },
    { key = "bar_height",                     type = "number" },
    { key = "health_fill_color",               type = "color" },
    { key = "low_health_color",                 type = "color" },
    { key = "low_health_threshold_percent",       type = "number" },
    { key = "health_bar_bg_color",               type = "color" },
    { key = "show_health_text",                  type = "bool" },
    { key = "health_text_format",                type = "string" },

    { key = "show_inactive_members",             type = "bool" },
    { key = "inactive_alpha",                   type = "number" },

    { key = "show_distance",                   type = "bool" },
    { key = "far_away_threshold",                type = "number" },
    { key = "distance_text_color",               type = "color" },

    { key = "show_leader_indicator",             type = "bool" },
    { key = "leader_indicator_color",             type = "color" },

    { key = "show_member_count_header",           type = "bool" },
    { key = "show_debug_info",                   type = "bool" },

    { key = "disable_default_group_display",      type = "bool" },
    { key = "window_transparent",                  type = "bool" },
}

-- Settings live in the UiForge profile now. This is the file older builds wrote, read
-- once so an existing setup carries over, then deleted.
local LEGACY_SETTINGS_FILE = tostring(UiForge.resources_path) .. "\\group_frames_settings.cfg"

-- Save, Load, and the one time legacy read all come from the settings list above.
local Save, Load, LoadSettingsOnce = AvoidsPersist.Bind({
    state = modern_group_frames_state,
    defs = PERSISTED_SETTINGS,
    legacy_path = LEGACY_SETTINGS_FILE,
})

local function ToggleDefaultGroupDisplay(owner)
    if modern_group_frames_state.disable_default_group_display == true then
        UI.DisableGroupDisplay(owner)
        return
    end
    UI.EnableGroupDisplay(owner)
end

local function OnDisable()
    -- v0.06+: fires when this script is unchecked in the UiForge menu. Always restore the
    -- native group display here, regardless of the "Disable Default Group Display" setting.
    UI.EnableGroupDisplay(owner)
end

local function Initialize()
    LoadSettingsOnce()
    modern_group_frames_state.initialized = true
end

local function Settings()
    local state = modern_group_frames_state

    state.show_group_frames = ImGui.Checkbox("Show Group Frames", state.show_group_frames)
    ImGui.TextUnformatted("(Only actually shows while you're in a group, regardless of this.)")
    state.disable_in_start_menu = ImGui.Checkbox("Hide While Start Menu Is Open", state.disable_in_start_menu)
    state.include_self_in_list = ImGui.Checkbox("Include Myself In List", state.include_self_in_list)

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
    ImGui.TextUnformatted("NOTE: group members only expose a health PERCENTAGE, not real HP")
    ImGui.TextUnformatted("numbers -- \"X/100\" below means X percent, scaled to 100, not a")
    ImGui.TextUnformatted("true max HP value like your own health bar has.")
    state.bar_width = ImGui.SliderInt("Bar Width", state.bar_width, 80, 400, tostring(state.bar_width))
    state.bar_height = ImGui.SliderInt("Bar Height", state.bar_height, 10, 50, tostring(state.bar_height))
    state.health_fill_color = ImGui.ColorEdit4("Health Fill Color", state.health_fill_color)
    state.low_health_color = ImGui.ColorEdit4("Low Health Color", state.low_health_color)
    state.low_health_threshold_percent = ImGui.SliderInt("Low Health Threshold (%)", state.low_health_threshold_percent, 0, 100, tostring(state.low_health_threshold_percent))
    state.health_bar_bg_color = ImGui.ColorEdit4("Health Bar Background", state.health_bar_bg_color)
    state.show_health_text = ImGui.Checkbox("Show Health Text", state.show_health_text)

    if ImGui.RadioButton("Format: 80/100", state.health_text_format == "percent_100") then
        state.health_text_format = "percent_100"
    end
    ImGui.SameLine()
    if ImGui.RadioButton("Format: 80%", state.health_text_format == "percent") then
        state.health_text_format = "percent"
    end

    ImGui.Separator()
    ImGui.Text("Inactive Members")
    ImGui.TextUnformatted("A member is \"inactive\" when the game isn't currently tracking them")
    ImGui.TextUnformatted("(e.g. out of range) -- their health/position may be stale.")
    state.show_inactive_members = ImGui.Checkbox("Show Inactive Members", state.show_inactive_members)
    state.inactive_alpha = ImGui.SliderFloat("Inactive Opacity", state.inactive_alpha, 0.1, 1.0, tostring(state.inactive_alpha))

    ImGui.Separator()
    ImGui.Text("Distance")
    state.show_distance = ImGui.Checkbox("Show Distance", state.show_distance)
    state.far_away_threshold = ImGui.SliderInt("Far Away Threshold", state.far_away_threshold, 50, 2000, tostring(state.far_away_threshold))
    state.distance_text_color = ImGui.ColorEdit4("Distance Text Color", state.distance_text_color)

    ImGui.Separator()
    ImGui.Text("Leader")
    state.show_leader_indicator = ImGui.Checkbox("Show Leader Indicator (only detects if YOU are leader)", state.show_leader_indicator)
    state.leader_indicator_color = ImGui.ColorEdit4("Leader Indicator Color", state.leader_indicator_color)

    ImGui.Separator()
    state.show_member_count_header = ImGui.Checkbox("Show Member Count Header", state.show_member_count_header)
    state.show_debug_info = ImGui.Checkbox("Show Debug Info (troubleshooting)", state.show_debug_info)

    ImGui.Separator()
    state.window_transparent = ImGui.Checkbox("Window Transparent (only text/bars have background)", state.window_transparent)
    local new_val, pressed = ImGui.Checkbox("Disable Default Group Display", state.disable_default_group_display)
    if pressed then
        state.disable_default_group_display = new_val
        ToggleDefaultGroupDisplay()
    end

end

local function GetHealthColor(state, health_percent)
    if health_percent ~= nil and health_percent <= state.low_health_threshold_percent then
        return state.low_health_color
    end
    return state.health_fill_color
end

local function DrawHealthBar(state, health_percent, alpha_multiplier)
    local draw_list = ImGui.GetWindowDrawList()
    local origin_x, origin_y = ImGui.GetCursorScreenPos()
    local p0 = ImVec2.new(origin_x, origin_y)
    local p1 = ImVec2.new(origin_x + state.bar_width, origin_y + state.bar_height)

    local bg_color = state.health_bar_bg_color
    local bg_color_u32 = ImGui.GetColorU32(bg_color[1], bg_color[2], bg_color[3], bg_color[4] * alpha_multiplier)
    draw_list:AddRectFilled(p0, p1, bg_color_u32, state.corner_rounding)

    if health_percent ~= nil then
        local fill_color = GetHealthColor(state, health_percent)
        local fill_color_u32 = ImGui.GetColorU32(fill_color[1], fill_color[2], fill_color[3], fill_color[4] * alpha_multiplier)
        local fill_width = state.bar_width * math.max(math.min(health_percent, 100), 0) / 100
        if fill_width > 0 then
            draw_list:AddRectFilled(p0, ImVec2.new(origin_x + fill_width, origin_y + state.bar_height), fill_color_u32, state.corner_rounding)
        end
    end

    if state.show_border then
        local border_color_u32 = ImGui.GetColorU32(state.border_color[1], state.border_color[2], state.border_color[3], state.border_color[4] * alpha_multiplier)
        draw_list:AddRect(p0, p1, border_color_u32, state.corner_rounding, 0, state.border_thickness)
    end

    if state.show_health_text then
        local text
        if health_percent == nil then
            text = "?"
        elseif state.health_text_format == "percent" then
            text = string.format("%.0f%%", health_percent)
        else
            text = string.format("%.0f/100", health_percent)
        end
        local text_width, text_height = ImGui.CalcTextSize(text)
        local text_color_u32 = ImGui.GetColorU32(1, 1, 1, alpha_multiplier)
        draw_list:AddText(ImVec2.new(origin_x + (state.bar_width - text_width) / 2, origin_y + (state.bar_height - text_height) / 2), text_color_u32, text)
    end

    ImGui.Dummy(state.bar_width, state.bar_height)
end

local function GetDistanceText(state, member)
    local ok_coords, member_coords = pcall(function() return member:GetCoordinates() end)
    if not ok_coords or member_coords == nil then return nil end

    local ok_player_coords, player_coords = pcall(Player.GetCoordinates)
    if not ok_player_coords or player_coords == nil then return nil end

    local dx = member_coords.x - player_coords.x
    local dz = member_coords.z - player_coords.z
    local distance = math.sqrt(dx * dx + dz * dz)

    if distance > state.far_away_threshold then
        return "Far Away"
    end
    return string.format("%.0fm", distance)
end

local function Render()
    local state = modern_group_frames_state

    if Util.IsInGame() == 0 then return end
    if state.disable_in_start_menu and Util.IsStartMenuOpen() == 1 then return end

    -- Shown regardless of the auto-hide logic below, specifically to troubleshoot cases where
    -- the main panel isn't appearing when you expect it to (e.g. with a pet summoned but no
    -- other real party member) -- this tells us exactly what the game itself is reporting.
    if state.show_debug_info then
        if ImGui.Begin("Group Frames Debug") then
            local ok_in_group, in_group = pcall(Group.IsInGroup)
            local ok_count, count = pcall(Group.GetMemberCount)
            local ok_leader, is_leader = pcall(Group.IsSelfLeader)
            ImGui.TextUnformatted("IsInGroup(): " .. (ok_in_group and tostring(in_group) or "ERROR: " .. tostring(in_group)))
            ImGui.TextUnformatted("GetMemberCount(): " .. (ok_count and tostring(count) or "ERROR: " .. tostring(count)))
            ImGui.TextUnformatted("IsSelfLeader(): " .. (ok_leader and tostring(is_leader) or "ERROR: " .. tostring(is_leader)))
        end
        ImGui.End()
    end

    if state.show_group_frames ~= true then return end
    if not Group.IsInGroup() then return end -- auto-hide entirely when not grouped

    local self_name = nil
    if not state.include_self_in_list then
        local ok_name, name = pcall(Player.GetName)
        if ok_name then self_name = name end
    end

    local window_flags = 0
    if state.window_transparent then window_flags = ImGuiWindowFlags.NoBackground end

    if ImGui.Begin("Group Frames", true, window_flags) then
        ImGui.SetWindowFontScale(state.font_scale)

        local draw_list = ImGui.GetWindowDrawList()

        if state.show_member_count_header then
            local count = Group.GetMemberCount()
            local header = "Group (" .. count .. ")"
            if state.show_leader_indicator and Group.IsSelfLeader() then
                header = header .. "  [You are the Leader]"
            end

            -- Drawn manually (not ImGui.Text) so it gets its own dark background chip -- needed
            -- since the window itself has no background when window_transparent is on.
            local header_origin_x, header_origin_y = ImGui.GetCursorScreenPos()
            local header_width, header_height = ImGui.CalcTextSize(header)
            local chip_padding = 3
            local chip_color_u32 = ImGui.GetColorU32(0, 0, 0, 0.7)
            draw_list:AddRectFilled(
                ImVec2.new(header_origin_x - chip_padding, header_origin_y - chip_padding),
                ImVec2.new(header_origin_x + header_width + chip_padding, header_origin_y + header_height + chip_padding),
                chip_color_u32, state.corner_rounding)
            draw_list:AddText(ImVec2.new(header_origin_x, header_origin_y), ImGui.GetColorU32(1, 1, 1, 1), header)
            ImGui.Dummy(header_width, header_height)
            ImGui.Separator()
        end

        local drew_any = false
        for _, member in Group.Members() do
            local ok_name, name = pcall(function() return member:GetName() end)
            name = (ok_name and name ~= nil and name ~= "") and name or "(unknown)"

            local is_self = (self_name ~= nil and name == self_name)
            if not is_self then
                local ok_active, is_active = pcall(function() return member:IsActive() end)
                is_active = ok_active and is_active

                if is_active or state.show_inactive_members then
                    drew_any = true
                    local alpha_multiplier = is_active and 1.0 or state.inactive_alpha

                    local distance_text = nil
                    if state.show_distance then
                        distance_text = GetDistanceText(state, member)
                    end

                    -- Name + distance drawn manually as one combined chip, same reasoning as
                    -- the header above -- gives the whole line one dark background regardless
                    -- of window transparency, while keeping distance in its own color.
                    local line_origin_x, line_origin_y = ImGui.GetCursorScreenPos()
                    local name_width, name_height = ImGui.CalcTextSize(name)
                    local gap = 6
                    local distance_width = 0
                    local distance_display = nil
                    if distance_text ~= nil then
                        distance_display = "(" .. distance_text .. ")"
                        distance_width = (ImGui.CalcTextSize(distance_display))
                    end
                    local combined_width = name_width + (distance_text ~= nil and (gap + distance_width) or 0)

                    local chip_padding = 3
                    local chip_color_u32 = ImGui.GetColorU32(0, 0, 0, 0.7 * alpha_multiplier)
                    draw_list:AddRectFilled(
                        ImVec2.new(line_origin_x - chip_padding, line_origin_y - chip_padding),
                        ImVec2.new(line_origin_x + combined_width + chip_padding, line_origin_y + name_height + chip_padding),
                        chip_color_u32, state.corner_rounding)

                    draw_list:AddText(ImVec2.new(line_origin_x, line_origin_y), ImGui.GetColorU32(1, 1, 1, alpha_multiplier), name)
                    if distance_display ~= nil then
                        local dc = state.distance_text_color
                        draw_list:AddText(ImVec2.new(line_origin_x + name_width + gap, line_origin_y), ImGui.GetColorU32(dc[1], dc[2], dc[3], dc[4] * alpha_multiplier), distance_display)
                    end
                    ImGui.Dummy(combined_width, name_height)

                    local ok_hp, health_percent = pcall(function() return member:GetHealthPercent() end)
                    DrawHealthBar(state, ok_hp and health_percent or nil, alpha_multiplier)

                    ImGui.Separator()
                end
            end
        end

        if not drew_any then
            ImGui.TextUnformatted("(no other group members to show)")
        end
    end
    ImGui.End()
end

local function Update()
    if modern_group_frames_state.initialized == false then Initialize() end

    -- Re-applied every frame (not just once at Initialize) since the ability bar script taught us
    -- the first attempt can silently fail if the script loads before the relevant game UI object is
    -- ready -- retrying every frame is cheap and guarantees it sticks once the game catches up.
    if Util.IsInGame() ~= 0 then
        ToggleDefaultGroupDisplay()
    end

    if modern_group_frames_state.initialized then Render() end
end

return {
    display_name = "Group Frames",
    Update = Update,
    Settings = Settings,
    OnDisable = OnDisable,
    Save = Save,
    Load = Load,
}
