--[[
avoids_health_bar -- part of the Modern UI Mods pack by Avoids.

Returns the mod table the pack's dispatcher drives,
{ display_name, Update, Settings, OnDisable, Save?, Load? }. State lives in a global
table so it survives a script reload, the same as when every mod was its own file.
]]

local UI = require("frontiers_forge.ui")                -- Access UI elements
local Player = require("frontiers_forge.player")        -- Access Player attributes and functions
local Util = require("frontiers_forge.util")            -- Access Utility functions
local AvoidsPersist = require("avoids_persist")
local AvoidsCommon = require("avoids_common")

-- Creating a long, specific table name so that I (hopefully) avoid naming collisions
modern_health_bar_state = modern_health_bar_state or {
    window_flags                = ImGuiWindowFlags.AlwaysAutoResize + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoTitleBar,
    initialized                 = false,
    callbacks_registered         = false,
    native_toggle_confirmed      = false, -- v0.1.1: tracks whether the native-bar disable/enable write has actually landed yet

    -- Set once the legacy settings file has been checked for
    settings_file_loaded        = false,

    -- Bar dimensions
    bar_width                   = 200,
    bar_height                  = 24,
    bar_scale                   = 1,

    -- Colors
    background_color            = {0.08, 0.08, 0.08, 0.85},   -- near-black, semi-transparent
    fill_color                  = {0.80, 0.05, 0.05, 1.0},     -- red
    use_low_health_color        = true,
    fill_color_low              = {0.95, 0.55, 0.05, 1.0},     -- amber, shown below the threshold
    low_health_threshold        = 0.25,

    -- Border
    show_border                 = true,
    border_color                = {0, 0, 0, 1},
    border_thickness            = 1.5,
    corner_rounding              = 4,

    -- Text overlay
    show_text                   = true,
    text_color                  = {1, 1, 1, 1},
    text_format                 = "current_max",   -- "current_max" -> "138/158"  |  "percent" -> "87%"
    font_scale                  = 1.0,             -- text size, relative to the default UI font
    bold_text                   = false,           -- simulates bold by drawing the text with a slight offset stroke

    disable_default_health_bar  = true,
}


--[[
Settings persistence

The options below ride in the UiForge profile. Save them with File then Save
Profile, restore them with File then Select Profile. This one list drives both
halves of that, so the payload can never drift from what the settings panel edits.
]]
local PERSISTED_SETTINGS = {
    { key = "bar_width",              type = "number" },
    { key = "bar_height",             type = "number" },
    { key = "bar_scale",              type = "number" },
    { key = "background_color",       type = "color" },
    { key = "fill_color",             type = "color" },
    { key = "use_low_health_color",   type = "bool" },
    { key = "fill_color_low",         type = "color" },
    { key = "low_health_threshold",   type = "number" },
    { key = "show_border",            type = "bool" },
    { key = "border_color",           type = "color" },
    { key = "border_thickness",       type = "number" },
    { key = "corner_rounding",        type = "number" },
    { key = "show_text",              type = "bool" },
    { key = "text_color",             type = "color" },
    { key = "text_format",            type = "string" },
    { key = "font_scale",             type = "number" },
    { key = "bold_text",              type = "bool" },
    { key = "disable_default_health_bar", type = "bool" },
}

-- Settings live in the UiForge profile now. This is the file older builds wrote, read
-- once so an existing setup carries over, then deleted.
local LEGACY_SETTINGS_FILE = tostring(UiForge.resources_path) .. "\\retro_health_hearts\\health_bar_settings.cfg"

-- Save, Load, and the one time legacy read all come from the settings list above.
local Save, Load, LoadSettingsOnce = AvoidsPersist.Bind({
    state = modern_health_bar_state,
    defs = PERSISTED_SETTINGS,
    legacy_path = LEGACY_SETTINGS_FILE,
    -- Applying a profile can change a native hide setting, so clear the confirmed
    -- flags and let the per frame retry reapply them.
    after_load = function()
        modern_health_bar_state.native_toggle_confirmed = false
    end,
})

local function ToggleDefaultHealthBar(owner)
    if modern_health_bar_state.disable_default_health_bar == true then
        return UI.DisableHealthBar(owner)
    end

    return UI.EnableHealthBar(owner)
end

local function Initialize()
    -- Load saved size/colors/format first so everything below reflects your last session.
    LoadSettingsOnce()

    modern_health_bar_state.initialized = true
end

local function Settings()
    local state = modern_health_bar_state

    ImGui.Text("Size")
    state.bar_width  = ImGui.SliderInt("Width", state.bar_width, 50, 400, tostring(state.bar_width))
    state.bar_height = ImGui.SliderInt("Height", state.bar_height, 10, 80, tostring(state.bar_height))
    state.bar_scale  = ImGui.SliderFloat("Scale", state.bar_scale, 0.1, 5.0, tostring(state.bar_scale))

    ImGui.Separator()
    ImGui.Text("Colors")
    state.background_color = ImGui.ColorEdit4("Background Color", state.background_color)
    state.fill_color       = ImGui.ColorEdit4("Fill Color", state.fill_color)

    state.use_low_health_color = ImGui.Checkbox("Use Low Health Color", state.use_low_health_color)
    state.fill_color_low       = ImGui.ColorEdit4("Low Health Fill Color", state.fill_color_low)
    state.low_health_threshold = ImGui.SliderFloat("Low Health Threshold", state.low_health_threshold, 0.05, 0.9,
        string.format("%d%%", math.floor(state.low_health_threshold * 100)))

    ImGui.Separator()
    ImGui.Text("Border")
    state.show_border      = ImGui.Checkbox("Show Border", state.show_border)
    state.border_color     = ImGui.ColorEdit4("Border Color", state.border_color)
    state.border_thickness = ImGui.SliderFloat("Border Thickness", state.border_thickness, 0.5, 5.0, tostring(state.border_thickness))
    state.corner_rounding  = ImGui.SliderFloat("Corner Rounding", state.corner_rounding, 0, 20, tostring(state.corner_rounding))

    ImGui.Separator()
    ImGui.Text("Text")
    state.show_text  = ImGui.Checkbox("Show Text", state.show_text)
    state.text_color = ImGui.ColorEdit4("Text Color", state.text_color)

    if ImGui.RadioButton("Current / Max  (e.g. 138/158)", state.text_format == "current_max") then
        state.text_format = "current_max"
    end
    if ImGui.RadioButton("Percentage  (e.g. 87%)", state.text_format == "percent") then
        state.text_format = "percent"
    end
    state.font_scale = ImGui.SliderFloat("Font Scale", state.font_scale, 0.5, 3.0, tostring(state.font_scale))
    state.bold_text  = ImGui.Checkbox("Bold Text", state.bold_text)

    ImGui.Separator()
    local new_val, pressed = ImGui.Checkbox("Disable Default Health Bar", state.disable_default_health_bar)
    if pressed then
        state.disable_default_health_bar = new_val
        AvoidsCommon.ApplyNativeToggle(state, "native_toggle_confirmed", ToggleDefaultHealthBar)
    end

end

local function OnDisable()
    -- v0.06: fires when this script is unchecked in the UiForge menu. Without this, disabling
    -- the script would leave the default health bar hidden with no way to bring it back short
    -- of restarting the game -- so always restore the native UI here, regardless of the
    -- "Disable Default Health Bar" setting.
    -- Restoring the panel and clearing the flag go together, so re-enabling the mod
    -- hides it again instead of coming back with the setting on but nothing applied.
    AvoidsCommon.ReleaseNativeToggle(modern_health_bar_state, "native_toggle_confirmed", UI.EnableHealthBar)
end

local function Render()
    -- If we are not in game or if the start menu is open, don't display the bar
    if Util.IsInGame() == 0 or Util.IsStartMenuOpen() == 1 then return end

    local state = modern_health_bar_state

    if ImGui.Begin("modern health bar window", true, state.window_flags) then
        -- Scales all text drawn in this window (both ImGui.Text and draw_list:AddText).
        -- Must be set before any text is measured/drawn below.
        ImGui.SetWindowFontScale(state.font_scale)

        local scaled_width  = state.bar_width * state.bar_scale
        local scaled_height = state.bar_height * state.bar_scale

        local current_hp = Player.GetCurrentHp()
        local max_hp = Player.GetMaxHp()
        if max_hp <= 0 then max_hp = 1 end -- guard against a divide-by-zero if max HP ever reads 0

        local health_percent = current_hp / max_hp
        if health_percent < 0 then health_percent = 0 end
        if health_percent > 1 then health_percent = 1 end

        local draw_list = ImGui.GetWindowDrawList()
        local origin_x, origin_y = ImGui.GetCursorScreenPos()
        local p0 = ImVec2.new(origin_x, origin_y)
        local p1 = ImVec2.new(origin_x + scaled_width, origin_y + scaled_height)

        -- Background track
        local background_color_u32 = ImGui.GetColorU32(state.background_color[1], state.background_color[2], state.background_color[3], state.background_color[4])
        draw_list:AddRectFilled(p0, p1, background_color_u32, state.corner_rounding)

        -- Fill, clipped to the current health percentage. Clipping (rather than resizing the
        -- rect itself) keeps the left corners rounded to match the background at any fill amount.
        if health_percent > 0 then
            local active_fill_color = state.fill_color
            if state.use_low_health_color and health_percent <= state.low_health_threshold then
                active_fill_color = state.fill_color_low
            end
            local fill_color_u32 = ImGui.GetColorU32(active_fill_color[1], active_fill_color[2], active_fill_color[3], active_fill_color[4])

            local fill_end_x = origin_x + (scaled_width * health_percent)
            ImGui.PushClipRect(p0.x, p0.y, fill_end_x, p1.y, true)
            draw_list:AddRectFilled(p0, p1, fill_color_u32, state.corner_rounding)
            ImGui.PopClipRect()
        end

        -- Border
        if state.show_border then
            local border_color_u32 = ImGui.GetColorU32(state.border_color[1], state.border_color[2], state.border_color[3], state.border_color[4])
            draw_list:AddRect(p0, p1, border_color_u32, state.corner_rounding, 0, state.border_thickness)
        end

        -- Text overlay, e.g. "138/158" or "87%"
        if state.show_text then
            local display_text
            if state.text_format == "percent" then
                display_text = string.format("%d%%", math.floor(health_percent * 100 + 0.5))
            else
                display_text = string.format("%d/%d", math.floor(current_hp + 0.5), math.floor(max_hp + 0.5))
            end

            -- NOTE: ImGui.CalcTextSize is used here to center the text within the bar.
            -- If your UiForge build doesn't expose it, replace the two lines below with a
            -- fixed offset, e.g.: local text_x, text_y = p0.x + 6, p0.y + (scaled_height - 14) / 2
            local text_width, text_height = ImGui.CalcTextSize(display_text)
            local text_x = p0.x + (scaled_width - text_width) / 2
            local text_y = p0.y + (scaled_height - text_height) / 2

            local text_color_u32 = ImGui.GetColorU32(state.text_color[1], state.text_color[2], state.text_color[3], state.text_color[4])

            if state.bold_text then
                -- No bold font is available to load from Lua, so this fakes it by drawing the
                -- same text a pixel to the right and a pixel down first, thickening the strokes.
                local bold_offset = 1 * state.font_scale
                draw_list:AddText(ImVec2.new(text_x + bold_offset, text_y), text_color_u32, display_text)
                draw_list:AddText(ImVec2.new(text_x, text_y + bold_offset), text_color_u32, display_text)
                draw_list:AddText(ImVec2.new(text_x + bold_offset, text_y + bold_offset), text_color_u32, display_text)
            end
            draw_list:AddText(ImVec2.new(text_x, text_y), text_color_u32, display_text)
        end

        -- Reserve layout space matching the bar so the auto-resize window sizes correctly
        -- (drawing directly to the draw list, like above, doesn't move the ImGui cursor on its own)
        ImGui.Dummy(scaled_width, scaled_height)
    end
    ImGui.End()
end

local function Update()
    if modern_health_bar_state.initialized == false then Initialize() end

    -- v0.1.1: UI.DisableHealthBar/EnableHealthBar now return whether the write actually
    -- landed (false when the game UI isn't ready yet, e.g. right at login). Retrying every
    -- frame ONLY until confirmed is safe and precise -- unlike retrying forever, it stops
    -- the instant it succeeds, so there's no ongoing risk of interfering with anything else.
    AvoidsCommon.EnsureNativeToggle(modern_health_bar_state, "native_toggle_confirmed", ToggleDefaultHealthBar)

    if modern_health_bar_state.initialized then Render() end
end

return {
    display_name = "Health Bar",
    Update = Update,
    Settings = Settings,
    OnDisable = OnDisable,
    Save = Save,
    Load = Load,
}
