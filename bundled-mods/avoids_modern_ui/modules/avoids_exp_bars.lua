--[[
avoids_exp_bars -- part of the Modern UI Mods pack by Avoids.

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
modern_exp_bars_state = modern_exp_bars_state or {
    window_flags                = ImGuiWindowFlags.AlwaysAutoResize + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoTitleBar,
    initialized                 = false,
    callbacks_registered         = false,
    main_toggle_confirmed        = false, -- v0.1.1: tracks whether each native-bar write has actually landed yet
    secondary_toggle_confirmed   = false,

    -- Set once the legacy settings file has been checked for
    settings_file_loaded        = false,

    -- Shared dimensions (both bars use the same width/scale so they line up)
    bar_width                   = 200,
    bar_height                  = 20,
    bar_scale                   = 1,
    bar_spacing                 = 4,     -- vertical gap between the two bars
    font_scale                  = 1.0,   -- text size for both bars, relative to the default UI font
    bold_text                   = false, -- simulates bold by drawing the text with a slight offset stroke

    -- Main (gold) exp bar -- fills toward your next level
    main_background_color       = {0.08, 0.08, 0.08, 0.85},
    main_fill_color              = {0.85, 0.65, 0.05, 1.0},   -- gold
    main_show_border            = true,
    main_border_color           = {0, 0, 0, 1},
    main_border_thickness       = 1.5,
    main_corner_rounding         = 4,
    main_show_text               = true,
    main_text_color              = {1, 1, 1, 1},
    main_text_format             = "percent",   -- "percent" -> "42%"  |  "current_max" -> "1234/5000"

    -- Secondary (pink/CM) bar -- fills a bonus pool that feeds into the main bar
    secondary_background_color  = {0.08, 0.08, 0.08, 0.85},
    secondary_fill_color        = {0.90, 0.20, 0.55, 1.0},    -- pink
    secondary_show_border       = true,
    secondary_border_color      = {0, 0, 0, 1},
    secondary_border_thickness  = 1.5,
    secondary_corner_rounding    = 4,
    secondary_show_text          = true,
    secondary_text_color         = {1, 1, 1, 1},
    pink_fills_per_level         = 4,   -- how many full pink-bar fills make up one gold-bar level

    disable_default_main_exp_bar      = true,
    disable_default_secondary_exp_bar = true,
}


--[[
Settings persistence

The options below ride in the UiForge profile. Save them with File then Save
Profile, restore them with File then Select Profile. This one list drives both
halves of that, so the payload can never drift from what the settings panel edits.
]]
local PERSISTED_SETTINGS = {
    { key = "bar_width",                        type = "number" },
    { key = "bar_height",                       type = "number" },
    { key = "bar_scale",                        type = "number" },
    { key = "bar_spacing",                      type = "number" },
    { key = "pink_fills_per_level",              type = "number" },
    { key = "font_scale",                       type = "number" },
    { key = "bold_text",                        type = "bool" },

    { key = "main_background_color",            type = "color" },
    { key = "main_fill_color",                  type = "color" },
    { key = "main_show_border",                 type = "bool" },
    { key = "main_border_color",                type = "color" },
    { key = "main_border_thickness",            type = "number" },
    { key = "main_corner_rounding",              type = "number" },
    { key = "main_show_text",                    type = "bool" },
    { key = "main_text_color",                   type = "color" },
    { key = "main_text_format",                  type = "string" },

    { key = "secondary_background_color",       type = "color" },
    { key = "secondary_fill_color",              type = "color" },
    { key = "secondary_show_border",             type = "bool" },
    { key = "secondary_border_color",            type = "color" },
    { key = "secondary_border_thickness",        type = "number" },
    { key = "secondary_corner_rounding",          type = "number" },
    { key = "secondary_show_text",                type = "bool" },
    { key = "secondary_text_color",               type = "color" },

    { key = "disable_default_main_exp_bar",             type = "bool" },
    { key = "disable_default_secondary_exp_bar",        type = "bool" },
}

-- Settings live in the UiForge profile now. This is the file older builds wrote, read
-- once so an existing setup carries over, then deleted.
local LEGACY_SETTINGS_FILE = tostring(UiForge.resources_path) .. "\\exp_bars\\exp_bars_settings.cfg"

-- Save, Load, and the one time legacy read all come from the settings list above.
local Save, Load, LoadSettingsOnce = AvoidsPersist.Bind({
    state = modern_exp_bars_state,
    defs = PERSISTED_SETTINGS,
    legacy_path = LEGACY_SETTINGS_FILE,
    -- Applying a profile can change a native hide setting, so clear the confirmed
    -- flags and let the per frame retry reapply them.
    after_load = function()
        modern_exp_bars_state.main_toggle_confirmed = false
        modern_exp_bars_state.secondary_toggle_confirmed = false
    end,
})

local function ToggleDefaultMainExpBar(owner)
    if modern_exp_bars_state.disable_default_main_exp_bar == true then
        return UI.DisableMainExpBar(owner)
    end
    return UI.EnableMainExpBar(owner)
end

local function ToggleDefaultSecondaryExpBar(owner)
    if modern_exp_bars_state.disable_default_secondary_exp_bar == true then
        return UI.DisableSecondaryExpBar(owner)
    end
    return UI.EnableSecondaryExpBar(owner)
end

local function Initialize()
    -- Load saved size/colors/format first so everything below reflects your last session.
    LoadSettingsOnce()

    modern_exp_bars_state.initialized = true
end

local function Settings()
    local state = modern_exp_bars_state

    ImGui.Text("Shared Size")
    state.bar_width   = ImGui.SliderInt("Width", state.bar_width, 50, 400, tostring(state.bar_width))
    state.bar_height  = ImGui.SliderInt("Height", state.bar_height, 10, 60, tostring(state.bar_height))
    state.bar_scale   = ImGui.SliderFloat("Scale", state.bar_scale, 0.1, 5.0, tostring(state.bar_scale))
    state.bar_spacing = ImGui.SliderInt("Spacing Between Bars", state.bar_spacing, 0, 30, tostring(state.bar_spacing))
    state.pink_fills_per_level = ImGui.SliderInt("Pink Fills Per Level", state.pink_fills_per_level, 1, 20, tostring(state.pink_fills_per_level))
    state.font_scale = ImGui.SliderFloat("Font Scale", state.font_scale, 0.5, 3.0, tostring(state.font_scale))
    state.bold_text  = ImGui.Checkbox("Bold Text", state.bold_text)

    ImGui.Separator()
    ImGui.Text("Main Exp Bar (Gold)")
    state.main_background_color = ImGui.ColorEdit4("Background##main", state.main_background_color)
    state.main_fill_color       = ImGui.ColorEdit4("Fill Color##main", state.main_fill_color)
    state.main_show_border      = ImGui.Checkbox("Show Border##main", state.main_show_border)
    state.main_border_color     = ImGui.ColorEdit4("Border Color##main", state.main_border_color)
    state.main_border_thickness = ImGui.SliderFloat("Border Thickness##main", state.main_border_thickness, 0.5, 5.0, tostring(state.main_border_thickness))
    state.main_corner_rounding  = ImGui.SliderFloat("Corner Rounding##main", state.main_corner_rounding, 0, 20, tostring(state.main_corner_rounding))
    state.main_show_text        = ImGui.Checkbox("Show Text##main", state.main_show_text)
    state.main_text_color       = ImGui.ColorEdit4("Text Color##main", state.main_text_color)
    if ImGui.RadioButton("Percentage  (e.g. 42%)##main", state.main_text_format == "percent") then
        state.main_text_format = "percent"
    end
    if ImGui.RadioButton("Current / Max  (e.g. 1234/5000)##main", state.main_text_format == "current_max") then
        state.main_text_format = "current_max"
    end

    local new_val_main, pressed_main = ImGui.Checkbox("Disable Default Main Exp Bar", state.disable_default_main_exp_bar)
    if pressed_main then
        state.disable_default_main_exp_bar = new_val_main
        AvoidsCommon.ApplyNativeToggle(state, "main_toggle_confirmed", ToggleDefaultMainExpBar)
    end

    ImGui.Separator()
    ImGui.Text("Secondary Exp Bar (Pink / CM)")
    state.secondary_background_color = ImGui.ColorEdit4("Background##secondary", state.secondary_background_color)
    state.secondary_fill_color       = ImGui.ColorEdit4("Fill Color##secondary", state.secondary_fill_color)
    state.secondary_show_border      = ImGui.Checkbox("Show Border##secondary", state.secondary_show_border)
    state.secondary_border_color     = ImGui.ColorEdit4("Border Color##secondary", state.secondary_border_color)
    state.secondary_border_thickness = ImGui.SliderFloat("Border Thickness##secondary", state.secondary_border_thickness, 0.5, 5.0, tostring(state.secondary_border_thickness))
    state.secondary_corner_rounding  = ImGui.SliderFloat("Corner Rounding##secondary", state.secondary_corner_rounding, 0, 20, tostring(state.secondary_corner_rounding))
    state.secondary_show_text        = ImGui.Checkbox("Show Text##secondary", state.secondary_show_text)
    state.secondary_text_color       = ImGui.ColorEdit4("Text Color##secondary", state.secondary_text_color)

    local new_val_secondary, pressed_secondary = ImGui.Checkbox("Disable Default Secondary Exp Bar", state.disable_default_secondary_exp_bar)
    if pressed_secondary then
        state.disable_default_secondary_exp_bar = new_val_secondary
        AvoidsCommon.ApplyNativeToggle(state, "secondary_toggle_confirmed", ToggleDefaultSecondaryExpBar)
    end

end

local function OnDisable()
    -- v0.06: fires when this script is unchecked in the UiForge menu. Without this, disabling
    -- the script would leave both default exp bars hidden with no way to bring them back short
    -- of restarting the game -- so always restore the native UI here, regardless of the
    -- "Disable Default Main/Secondary Exp Bar" settings.
    -- Restoring a panel and clearing its flag go together, so re-enabling the mod hides
    -- them again instead of coming back with the settings on but nothing applied.
    AvoidsCommon.ReleaseNativeToggle(modern_exp_bars_state, "main_toggle_confirmed", UI.EnableMainExpBar)
    AvoidsCommon.ReleaseNativeToggle(modern_exp_bars_state, "secondary_toggle_confirmed", UI.EnableSecondaryExpBar)
end

-- Draws a single bar at the current cursor position and reserves layout space for it.
-- percent must already be clamped to [0, 1]. Returns nothing; advances the ImGui cursor.
local function DrawBar(scaled_width, scaled_height, percent, background_color, fill_color, show_border, border_color, border_thickness, corner_rounding, display_text, show_text, text_color, bold_text, font_scale)
    local draw_list = ImGui.GetWindowDrawList()
    local origin_x, origin_y = ImGui.GetCursorScreenPos()
    local p0 = ImVec2.new(origin_x, origin_y)
    local p1 = ImVec2.new(origin_x + scaled_width, origin_y + scaled_height)

    local background_color_u32 = ImGui.GetColorU32(background_color[1], background_color[2], background_color[3], background_color[4])
    draw_list:AddRectFilled(p0, p1, background_color_u32, corner_rounding)

    if percent > 0 then
        local fill_color_u32 = ImGui.GetColorU32(fill_color[1], fill_color[2], fill_color[3], fill_color[4])
        local fill_end_x = origin_x + (scaled_width * percent)
        ImGui.PushClipRect(p0.x, p0.y, fill_end_x, p1.y, true)
        draw_list:AddRectFilled(p0, p1, fill_color_u32, corner_rounding)
        ImGui.PopClipRect()
    end

    if show_border then
        local border_color_u32 = ImGui.GetColorU32(border_color[1], border_color[2], border_color[3], border_color[4])
        draw_list:AddRect(p0, p1, border_color_u32, corner_rounding, 0, border_thickness)
    end

    if show_text and display_text ~= nil then
        -- NOTE: ImGui.CalcTextSize is used here to center the text within the bar.
        -- If your UiForge build doesn't expose it, replace the two lines below with a
        -- fixed offset, e.g.: local text_x, text_y = p0.x + 6, p0.y + (scaled_height - 14) / 2
        local text_width, text_height = ImGui.CalcTextSize(display_text)
        local text_x = p0.x + (scaled_width - text_width) / 2
        local text_y = p0.y + (scaled_height - text_height) / 2
        local text_color_u32 = ImGui.GetColorU32(text_color[1], text_color[2], text_color[3], text_color[4])

        if bold_text then
            -- No bold font is available to load from Lua, so this fakes it by drawing the
            -- same text a pixel to the right and a pixel down first, thickening the strokes.
            local bold_offset = 1 * font_scale
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

local function Render()
    -- If we are not in game or if the start menu is open, don't display the bars
    if Util.IsInGame() == 0 or Util.IsStartMenuOpen() == 1 then return end

    local state = modern_exp_bars_state

    if ImGui.Begin("modern exp bars window", true, state.window_flags) then
        -- Scales all text drawn in this window (both ImGui.Text and draw_list:AddText).
        -- Must be set before any text is measured/drawn below.
        ImGui.SetWindowFontScale(state.font_scale)

        local scaled_width  = state.bar_width * state.bar_scale
        local scaled_height = state.bar_height * state.bar_scale

        -- Main (gold) bar: current exp progress toward the next level
        local current_exp = Player.GetExp()
        local required_exp = Util.GetExpRequiredForLevel(Player.GetLevel())
        if required_exp <= 0 then required_exp = 1 end -- guard against a divide-by-zero

        local main_percent = current_exp / required_exp
        if main_percent < 0 then main_percent = 0 end
        if main_percent > 1 then main_percent = 1 end

        local main_text
        if state.main_text_format == "current_max" then
            main_text = string.format("%d/%d", math.floor(current_exp + 0.5), math.floor(required_exp + 0.5))
        else
            main_text = string.format("%d%%", math.floor(main_percent * 100 + 0.5))
        end

        DrawBar(scaled_width, scaled_height, main_percent,
            state.main_background_color, state.main_fill_color,
            state.main_show_border, state.main_border_color, state.main_border_thickness, state.main_corner_rounding,
            main_text, state.main_show_text, state.main_text_color, state.bold_text, state.font_scale)

        ImGui.Dummy(scaled_width, state.bar_spacing)

        -- Secondary (pink) bar: per the dev notes, filling this bar once = 25% of the gold bar
        -- (i.e. it takes 4 full pink fills to level up). Rather than pull this from a separate
        -- API value, we derive it directly from the same exp progress driving the gold bar:
        -- take the gold percent, multiply by how many pink fills make up a full level, and
        -- keep only the fractional part -- that's how far through the *current* pink fill you are.
        local pink_raw = main_percent * state.pink_fills_per_level
        local secondary_percent = pink_raw - math.floor(pink_raw)
        if main_percent >= 1 then secondary_percent = 1 end -- edge case: fully leveled, show the pink bar full too

        local secondary_text = string.format("%d%%", math.floor(secondary_percent * 100 + 0.5))

        DrawBar(scaled_width, scaled_height, secondary_percent,
            state.secondary_background_color, state.secondary_fill_color,
            state.secondary_show_border, state.secondary_border_color, state.secondary_border_thickness, state.secondary_corner_rounding,
            secondary_text, state.secondary_show_text, state.secondary_text_color, state.bold_text, state.font_scale)
    end
    ImGui.End()
end

local function Update()
    if modern_exp_bars_state.initialized == false then Initialize() end

    AvoidsCommon.EnsureNativeToggle(modern_exp_bars_state, "main_toggle_confirmed", ToggleDefaultMainExpBar)
    AvoidsCommon.EnsureNativeToggle(modern_exp_bars_state, "secondary_toggle_confirmed", ToggleDefaultSecondaryExpBar)

    if modern_exp_bars_state.initialized then Render() end
end

return {
    display_name = "Experience Bars",
    Update = Update,
    Settings = Settings,
    OnDisable = OnDisable,
    Save = Save,
    Load = Load,
}
