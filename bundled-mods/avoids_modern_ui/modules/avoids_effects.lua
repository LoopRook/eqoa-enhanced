--[[
avoids_effects -- part of the Modern UI Mods pack by Avoids.

Returns the mod table the pack's dispatcher drives,
{ display_name, Update, Settings, OnDisable, Save?, Load? }. State lives in a global
table so it survives a script reload, the same as when every mod was its own file.
]]

local UI = require("frontiers_forge.ui")                -- Access UI elements
local Util = require("frontiers_forge.util")            -- Access Utility functions
local Icon = require("frontiers_forge.icon")             -- Decode game icons into ImGui textures
local Effects = require("frontiers_forge.effects")       -- Access active effects (buffs/debuffs) -- new in v1.0
local AvoidsPersist = require("avoids_persist")
local AvoidsCommon = require("avoids_common")

--[[
Modern Effects (Buffs/Debuffs)

Built on the new effects.lua module (v1.0), which finally exposes the player's active effects
(the icon row next to the health/power/experience bars) -- name and icon per effect, up to 8.

HONEST LIMITATION: there is no buff-vs-debuff distinction anywhere in the exposed data, just a
name and an icon per active effect (matching what the module itself documents -- "couldn't find
any other client side info for them"). This script shows every active effect together in one
row rather than attempting to split them, since there's no reliable way to categorize them (a
name-based guess would be unreliable and is not attempted here).
]]
modern_effects_state = modern_effects_state or {
    initialized                 = false,
    native_toggle_confirmed      = false, -- v0.1.1: tracks whether the native display disable/enable write has actually landed yet

    settings_file_loaded        = false,

    show_effects_panel            = true,
    disable_in_start_menu         = false,

    window_transparent             = true,
    icon_size                     = 40,
    icon_spacing                  = 4,
    background_color               = {0.12, 0.12, 0.12, 0.9},
    show_border                   = true,
    border_color                  = {0, 0, 0, 1},
    border_thickness              = 1.5,
    corner_rounding                 = 6,

    show_names_below               = false, -- names can be long -- off by default, shown as a tooltip on hover instead
    name_text_color                = {1, 1, 1, 0.85},
    font_scale                    = 1.0,

    show_debug_info                = false,

    disable_default_effects_display  = true,
}


local PERSISTED_SETTINGS = {
    { key = "show_effects_panel",              type = "bool" },
    { key = "disable_in_start_menu",            type = "bool" },

    { key = "window_transparent",               type = "bool" },
    { key = "icon_size",                       type = "number" },
    { key = "icon_spacing",                    type = "number" },
    { key = "background_color",                type = "color" },
    { key = "show_border",                     type = "bool" },
    { key = "border_color",                    type = "color" },
    { key = "border_thickness",                type = "number" },
    { key = "corner_rounding",                   type = "number" },

    { key = "show_names_below",                 type = "bool" },
    { key = "name_text_color",                  type = "color" },
    { key = "font_scale",                      type = "number" },

    { key = "show_debug_info",                  type = "bool" },
    { key = "disable_default_effects_display",    type = "bool" },
}

-- Settings live in the UiForge profile now. This is the file older builds wrote, read
-- once so an existing setup carries over, then deleted.
local LEGACY_SETTINGS_FILE = tostring(UiForge.resources_path) .. "\\effects_settings.cfg"

-- Save, Load, and the one time legacy read all come from the settings list above.
local Save, Load, LoadSettingsOnce = AvoidsPersist.Bind({
    state = modern_effects_state,
    defs = PERSISTED_SETTINGS,
    legacy_path = LEGACY_SETTINGS_FILE,
    -- Applying a profile can change a native hide setting, so clear the confirmed
    -- flags and let the per frame retry reapply them.
    after_load = function()
        modern_effects_state.native_toggle_confirmed = false
    end,
})

local function ToggleDefaultEffectsDisplay(owner)
    if modern_effects_state.disable_default_effects_display == true then
        return UI.DisableActiveEffectsDisplay(owner)
    end
    return UI.EnableActiveEffectsDisplay(owner)
end

local function OnDisable()
    modern_effects_state.show_effects_panel = false
    -- Restoring the panel and clearing the flag go together, so re-enabling the mod
    -- hides it again instead of coming back with the setting on but nothing applied.
    AvoidsCommon.ReleaseNativeToggle(modern_effects_state, "native_toggle_confirmed", UI.EnableActiveEffectsDisplay)
end

local function Initialize()
    LoadSettingsOnce()
    modern_effects_state.initialized = true
end

local function Settings()
    local state = modern_effects_state

    state.show_effects_panel = ImGui.Checkbox("Show Effects Panel", state.show_effects_panel)
    state.disable_in_start_menu = ImGui.Checkbox("Hide While Start Menu Is Open", state.disable_in_start_menu)

    ImGui.Separator()
    ImGui.TextUnformatted("No buff/debuff distinction is exposed anywhere in the API -- just")
    ImGui.TextUnformatted("a name and icon per active effect. All shown together below.")

    ImGui.Separator()
    ImGui.Text("Appearance")
    state.window_transparent = ImGui.Checkbox("Window Transparent (only icons have background)", state.window_transparent)
    state.icon_size = ImGui.SliderInt("Icon Size", state.icon_size, 20, 100, tostring(state.icon_size))
    state.icon_spacing = ImGui.SliderInt("Icon Spacing", state.icon_spacing, 0, 20, tostring(state.icon_spacing))
    state.background_color = ImGui.ColorEdit4("Background Color", state.background_color)
    state.show_border = ImGui.Checkbox("Show Border", state.show_border)
    state.border_color = ImGui.ColorEdit4("Border Color", state.border_color)
    state.border_thickness = ImGui.SliderFloat("Border Thickness", state.border_thickness, 0.5, 5.0, tostring(state.border_thickness))
    state.corner_rounding = ImGui.SliderFloat("Corner Rounding", state.corner_rounding, 0, 20, tostring(state.corner_rounding))

    ImGui.Separator()
    ImGui.Text("Names")
    state.show_names_below = ImGui.Checkbox("Show Names Below Icons", state.show_names_below)
    ImGui.TextUnformatted("(Names always show as a tooltip on hover regardless of this.)")
    state.name_text_color = ImGui.ColorEdit4("Name Text Color", state.name_text_color)
    state.font_scale = ImGui.SliderFloat("Font Scale", state.font_scale, 0.5, 3.0, tostring(state.font_scale))

    ImGui.Separator()
    state.show_debug_info = ImGui.Checkbox("Show Debug Info (troubleshooting)", state.show_debug_info)

    local new_val, pressed = ImGui.Checkbox("Disable Default Effects Display", state.disable_default_effects_display)
    if pressed then
        state.disable_default_effects_display = new_val
        AvoidsCommon.ApplyNativeToggle(state, "native_toggle_confirmed", ToggleDefaultEffectsDisplay)
    end

end

local function DrawEffect(state, effect)
    local draw_list = ImGui.GetWindowDrawList()
    local origin_x, origin_y = ImGui.GetCursorScreenPos()
    local p0 = ImVec2.new(origin_x, origin_y)
    local p1 = ImVec2.new(origin_x + state.icon_size, origin_y + state.icon_size)

    local bg_color_u32 = ImGui.GetColorU32(state.background_color[1], state.background_color[2], state.background_color[3], state.background_color[4])
    draw_list:AddRectFilled(p0, p1, bg_color_u32, state.corner_rounding)

    local drew_icon = false
    local debug_text = nil
    local ok_tex, texture = pcall(Icon.GetTexture, effect.icon_ref, { trim_transparent = true, trim_color = true })
    if ok_tex and texture ~= nil then
        local ok_draw = pcall(ImGui.Image, texture, state.icon_size, state.icon_size)
        drew_icon = ok_draw
        if not ok_draw then
            debug_text = "ImGui.Image failed on this effect's texture"
        end
    else
        debug_text = "Icon.GetTexture failed: " .. tostring(texture)
    end
    if not drew_icon then
        ImGui.Dummy(state.icon_size, state.icon_size)
    end

    if state.show_border then
        local border_color_u32 = ImGui.GetColorU32(state.border_color[1], state.border_color[2], state.border_color[3], state.border_color[4])
        draw_list:AddRect(p0, p1, border_color_u32, state.corner_rounding, 0, state.border_thickness)
    end

    if ImGui.IsItemHovered() then
        if state.show_debug_info and debug_text ~= nil then
            ImGui.SetTooltip((effect.name or "Unknown Effect") .. "\n" .. debug_text)
        else
            ImGui.SetTooltip(effect.name or "Unknown Effect")
        end
    end

    if state.show_names_below then
        local name = effect.name or "?"
        local name_width = ImGui.CalcTextSize(name)
        local name_color_u32 = ImGui.GetColorU32(state.name_text_color[1], state.name_text_color[2], state.name_text_color[3], state.name_text_color[4])
        draw_list:AddText(ImVec2.new(origin_x + (state.icon_size - name_width) / 2, origin_y + state.icon_size + 2), name_color_u32, name)
        ImGui.Dummy(state.icon_size, 14)
    end
end

local function Render()
    local state = modern_effects_state

    if Util.IsInGame() == 0 then return end
    if state.disable_in_start_menu and Util.IsStartMenuOpen() == 1 then return end
    if state.show_effects_panel ~= true then return end

    local count = Effects.GetCount()
    if count == 0 then return end -- auto-hide entirely when nothing is active

    local window_flags = ImGuiWindowFlags.AlwaysAutoResize + ImGuiWindowFlags.NoTitleBar
    if state.window_transparent then window_flags = window_flags + ImGuiWindowFlags.NoBackground end

    if ImGui.Begin("Effects", true, window_flags) then
        ImGui.SetWindowFontScale(state.font_scale)

        for index, effect in Effects.All() do
            if index > 0 then
                ImGui.SameLine(0, state.icon_spacing)
            end
            DrawEffect(state, effect)
        end
    end
    ImGui.End()
end

local function Update()
    if modern_effects_state.initialized == false then Initialize() end

    AvoidsCommon.EnsureNativeToggle(modern_effects_state, "native_toggle_confirmed", ToggleDefaultEffectsDisplay)

    if modern_effects_state.initialized then Render() end
end

return {
    display_name = "Effects (Buffs/Debuffs)",
    Update = Update,
    Settings = Settings,
    OnDisable = OnDisable,
    Save = Save,
    Load = Load,
}
