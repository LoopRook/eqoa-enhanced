--[[
avoids_ability_bar -- part of the Modern UI Mods pack by Avoids.

Returns the mod table the pack's dispatcher drives,
{ display_name, Update, Settings, OnDisable, Save?, Load? }. State lives in a global
table so it survives a script reload, the same as when every mod was its own file.
]]

local UI = require("frontiers_forge.ui")                -- Access UI elements
local Icon = require("frontiers_forge.icon")             -- Decode game icons into ImGui textures
local Util = require("frontiers_forge.util")            -- Access Utility functions
local AbilityBar = require("frontiers_forge.ability_bar") -- Access ability bar slots
local AbilityList = require("frontiers_forge.ability_list") -- Lockout event bus for exact cooldowns
local Player = require("frontiers_forge.player")        -- Own coordinates, current target -- for range checking
local EntityList = require("frontiers_forge.entity_list") -- Resolve target entity for range checking
local AvoidsPersist = require("avoids_persist")
local AvoidsCommon = require("avoids_common")

--[[
Modern Ability Bar v3

Rebuilt on the v1.0 FrontiersForge API. Two major upgrades from v2:

1. Navigation now reads the GAME'S OWN real selection state (AbilityBar.GetSelectedBarIndex()/
   GetSelectedSlotIndex()), instead of us tracking D-Pad presses ourselves in a parallel system.
   This is a genuine mirror of what the game will actually cast, not a guess -- and it also
   means this script no longer reads the D-Pad at all, so it can't conflict with anything else
   that does (e.g. the native quick-chat menu).

2. Cooldown tracking is back on by default and now driven by the AbilityList lockout event bus
   (Acquire + Update + On OnLockoutStart/OnLockoutEnd), so the countdown is the real lockout the
   server reports rather than a poll-based guess. A cooldown that was already running before the
   bar subscribed falls back to the old first-observed estimate. The crash this used to cause is
   fixed at the source in v1.0 -- every pointer-chain read across the whole framework now
   validates each hop against EE_RAM_SIZE before touching it.

Toolbelt/item slots (a different source_type than the ability list) can now show a REAL icon
via AbilityBar.GetSlotUITexId() + Icon.GetUITexture() -- the same lookup the game's own compact
HUD uses for the special-items bar. Still no name available for these slots anywhere in the API.

Icon rendering for ability slots uses the same Icon.GetTexture() pipeline as before.
]]
modern_ability_bar_state = modern_ability_bar_state or {
    initialized                 = false,
    callbacks_registered         = false,
    native_toggle_confirmed      = false, -- v0.1.1: tracks whether the native-bar disable/enable write has actually landed yet

    -- Settings persistence
    settings_file_loaded        = false,

    -- Layout
    bar_scale                    = 1.0,
    slot_size                    = 40,
    slot_spacing                 = 4,
    bar_spacing                  = 10,
    info_panel_top_spacing        = 12,  -- gap between the bars and the highlighted-ability info panel below them

    -- Slot appearance
    background_color              = {0.12, 0.12, 0.12, 0.9},
    empty_slot_color               = {0.05, 0.05, 0.05, 0.6},
    item_slot_color                 = {0.35, 0.28, 0.05, 0.9},  -- distinct color for non-ability (toolbelt/item) slots -- see notes in DrawSlot
    show_icon_background_layer      = true,  -- draws the ability's background icon layer beneath the foreground, matching how the game composites them
    show_border                   = true,
    border_color                  = {0, 0, 0, 1},
    border_thickness              = 1.5,
    corner_rounding                = 6,

    -- Navigation highlight
    show_navigation_highlight      = true,
    enable_cooldown_tracking         = true,  -- fixed at the source in v1.0 -- see header notes
    active_bar_color               = {1.0, 0.85, 0.2, 0.9},   -- outline around the currently active bar
    active_bar_thickness           = 2.5,
    active_bar_padding              = 8,  -- gap between the active-bar outline and the slots themselves (scales with Bar Scale)
    selected_slot_color            = {1.0, 1.0, 1.0, 1.0},    -- highlight around the currently selected slot
    selected_slot_thickness        = 2.5,

    -- Cooldown overlay
    show_cooldown_overlay          = true,
    cooldown_dim_color             = {0, 0, 0, 0.55},
    cooldown_text_color            = {1, 1, 1, 1},

    -- Info overlays
    show_power_cost                = true,
    power_cost_text_color          = {0.55, 0.85, 1.0, 1.0},
    show_slot_number                = true,
    slot_number_color               = {1, 1, 1, 0.6},

    -- Text
    font_scale                     = 1.0,
    bold_text                      = false,

    show_debug_info                = false, -- shows raw errors/status on slots, for troubleshooting icon rendering etc.

    -- Out-of-range indicator: uses the v1.0 Ability:IsInRange() range utility. Only shown
    -- when you actually have a target selected -- with no target there's nothing to check
    -- range against, so abilities just render normally.
    show_range_indicator             = true,
    out_of_range_color                = {0.9, 0.15, 0.15, 0.45},

    -- Removes the window's own default fill, so only the slot backgrounds (drawn explicitly,
    -- always visible regardless of this) show -- empty space around/between bars becomes see-
    -- through instead of a big black rectangle.
    window_transparent              = true,

    -- Name banner: a separate, big-font window showing the currently-highlighted ability's name
    -- and power cost, similar to how the base game displays this. Independent, moveable window
    -- (position it above the ability bar yourself, drag to taste).
    show_name_banner                 = true,
    name_banner_font_scale           = 2.2,
    name_banner_show_power_cost      = true,
    name_banner_text_color            = {1, 1, 1, 1},
    name_banner_power_cost_color      = {0.55, 0.85, 1.0, 1.0},
    name_banner_background_color      = {0.08, 0.08, 0.08, 0.85},
    name_banner_window_transparent    = true,
    name_banner_show_border          = true,
    name_banner_border_color          = {0, 0, 0, 1},
    name_banner_border_thickness      = 1.5,
    name_banner_corner_rounding        = 6,

    disable_default_ability_bar    = true,
}


-- Generic safe method-call helper: TryCall(obj, "MethodName", ...) never throws, even if obj
-- is nil or the method doesn't exist. Returns (true, result) on success, (false, error) otherwise.
local function TryCall(obj, method_name, ...)
    if obj == nil then return false, "object was nil" end
    local method = obj[method_name]
    if type(method) ~= "function" then return false, "no method named " .. tostring(method_name) end
    return pcall(method, obj, ...)
end

--[[
Settings persistence

The options below ride in the UiForge profile. Save them with File then Save
Profile, restore them with File then Select Profile. This one list drives both
halves of that, so the payload can never drift from what the settings panel edits.
]]
local PERSISTED_SETTINGS = {
    { key = "bar_scale",                    type = "number" },
    { key = "slot_size",                    type = "number" },
    { key = "slot_spacing",                 type = "number" },
    { key = "bar_spacing",                  type = "number" },
    { key = "info_panel_top_spacing",        type = "number" },

    { key = "background_color",             type = "color" },
    { key = "empty_slot_color",             type = "color" },
    { key = "item_slot_color",               type = "color" },
    { key = "show_icon_background_layer",    type = "bool" },
    { key = "show_border",                  type = "bool" },
    { key = "border_color",                 type = "color" },
    { key = "border_thickness",             type = "number" },
    { key = "corner_rounding",               type = "number" },

    { key = "show_navigation_highlight",     type = "bool" },
    { key = "enable_cooldown_tracking",        type = "bool" },
    { key = "active_bar_color",              type = "color" },
    { key = "active_bar_thickness",          type = "number" },
    { key = "active_bar_padding",             type = "number" },
    { key = "selected_slot_color",           type = "color" },
    { key = "selected_slot_thickness",       type = "number" },

    { key = "show_cooldown_overlay",          type = "bool" },
    { key = "cooldown_dim_color",             type = "color" },
    { key = "cooldown_text_color",            type = "color" },

    { key = "show_power_cost",                type = "bool" },
    { key = "power_cost_text_color",          type = "color" },
    { key = "show_slot_number",                type = "bool" },
    { key = "slot_number_color",               type = "color" },

    { key = "font_scale",                     type = "number" },
    { key = "bold_text",                      type = "bool" },

    { key = "show_debug_info",                 type = "bool" },
    { key = "show_range_indicator",              type = "bool" },
    { key = "out_of_range_color",                type = "color" },
    { key = "window_transparent",               type = "bool" },

    { key = "show_name_banner",                  type = "bool" },
    { key = "name_banner_font_scale",             type = "number" },
    { key = "name_banner_show_power_cost",        type = "bool" },
    { key = "name_banner_text_color",             type = "color" },
    { key = "name_banner_power_cost_color",       type = "color" },
    { key = "name_banner_background_color",       type = "color" },
    { key = "name_banner_window_transparent",     type = "bool" },
    { key = "name_banner_show_border",            type = "bool" },
    { key = "name_banner_border_color",           type = "color" },
    { key = "name_banner_border_thickness",       type = "number" },
    { key = "name_banner_corner_rounding",          type = "number" },
    { key = "disable_default_ability_bar",     type = "bool" },
}

-- Settings live in the UiForge profile now. This is the file older builds wrote, read
-- once so an existing setup carries over, then deleted.
local LEGACY_SETTINGS_FILE = tostring(UiForge.resources_path) .. "\\ability_bar_settings.cfg"

-- Save, Load, and the one time legacy read all come from the settings list above.
local Save, Load, LoadSettingsOnce = AvoidsPersist.Bind({
    state = modern_ability_bar_state,
    defs = PERSISTED_SETTINGS,
    legacy_path = LEGACY_SETTINGS_FILE,
    -- Applying a profile can change a native hide setting, so clear the confirmed
    -- flags and let the per frame retry reapply them.
    after_load = function()
        modern_ability_bar_state.native_toggle_confirmed = false
    end,
})

local function ToggleDefaultAbilityBar(owner)
    if modern_ability_bar_state.disable_default_ability_bar == true then
        return UI.DisableAbilityBar(owner)
    end
    return UI.EnableAbilityBar(owner)
end

--[[
v0.1.1 UPDATE: the elaborate "enable, wait, then disable" startup sequence that used to be
here is REMOVED. It existed only because UI.DisableAbilityBar/EnableAbilityBar gave no
way to know whether the write actually landed -- so we mimicked the manual "uncheck then
recheck" workaround with a timed delay as a guess. v0.1.1 fixes this properly: these
functions now return true/false for whether the write landed, and also cover all THREE
hotbar windows (compact and expanded) together, which mmvest's release notes say was the
actual source of "ability bar not disabling properly." Retrying every frame ONLY until
confirmed true (below, same pattern as every other mod in this pack) replaces the whole
workaround with something both simpler and more precise.
]]

-- Forward declared, defined in the cooldown tracking section below. OnDisable
-- runs it to drop the lockout hook, and sits above that section in the file.
local ReleaseCooldowns

local function OnDisable()
    -- v0.06+: fires when this script is unchecked in the UiForge menu. Always restore the
    -- native ability bar here, regardless of the "Disable Default Ability Bar" setting.
    -- Restoring the panel and clearing the flag go together, so re-enabling the mod
    -- hides it again instead of coming back with the setting on but nothing applied.
    AvoidsCommon.ReleaseNativeToggle(modern_ability_bar_state, "native_toggle_confirmed", UI.EnableAbilityBar)

    -- The lockout hook and its subscriptions belong to this mod, so they go here.
    -- This fires both on an individual toggle off and on the master pack disable.
    ReleaseCooldowns()

    -- Cached textures are not released here. Several of the mods in this pack draw
    -- icons, so the claim on the shared caches belongs to the pack as a whole and is
    -- dropped in the master OnDisable, not when one mod is toggled off.
end

local function Initialize()
    LoadSettingsOnce()
    modern_ability_bar_state.initialized = true
end

local function Settings()
    local state = modern_ability_bar_state

    ImGui.Text("Layout")
    state.bar_scale = ImGui.SliderFloat("Bar Scale", state.bar_scale, 0.5, 3.0, tostring(state.bar_scale))
    state.slot_size = ImGui.SliderInt("Slot Size", state.slot_size, 20, 100, tostring(state.slot_size))
    state.slot_spacing = ImGui.SliderInt("Slot Spacing", state.slot_spacing, 0, 20, tostring(state.slot_spacing))
    state.bar_spacing = ImGui.SliderInt("Spacing Between Bars", state.bar_spacing, 0, 40, tostring(state.bar_spacing))
    state.info_panel_top_spacing = ImGui.SliderInt("Space Above Info Panel", state.info_panel_top_spacing, 0, 40, tostring(state.info_panel_top_spacing))

    ImGui.Separator()
    ImGui.Text("Slot Appearance")
    state.background_color = ImGui.ColorEdit4("Background Color", state.background_color)
    state.empty_slot_color = ImGui.ColorEdit4("Empty Slot Color", state.empty_slot_color)
    state.item_slot_color = ImGui.ColorEdit4("Item/Toolbelt Slot Color", state.item_slot_color)
    state.show_icon_background_layer = ImGui.Checkbox("Show Icon Background Layer", state.show_icon_background_layer)
    state.show_border = ImGui.Checkbox("Show Border", state.show_border)
    state.border_color = ImGui.ColorEdit4("Border Color", state.border_color)
    state.border_thickness = ImGui.SliderFloat("Border Thickness", state.border_thickness, 0.5, 5.0, tostring(state.border_thickness))
    state.corner_rounding = ImGui.SliderFloat("Corner Rounding", state.corner_rounding, 0, 20, tostring(state.corner_rounding))

    ImGui.Separator()
    ImGui.Text("Navigation")
    ImGui.TextUnformatted("Mirrors the game's own real selection (AbilityBar.GetSelectedBarIndex/")
    ImGui.TextUnformatted("GetSelectedSlotIndex) -- this always matches what will actually cast.")
    state.show_navigation_highlight = ImGui.Checkbox("Show Navigation Highlight", state.show_navigation_highlight)
    state.enable_cooldown_tracking = ImGui.Checkbox("Enable Cooldown Tracking", state.enable_cooldown_tracking)
    ImGui.TextUnformatted("The crash this used to cause is fixed at the source in v1.0 --")
    ImGui.TextUnformatted("every pointer-chain read now validates against EE_RAM_SIZE. Safe")
    ImGui.TextUnformatted("to leave on; the toggle is kept here just in case.")
    state.active_bar_color = ImGui.ColorEdit4("Active Bar Color", state.active_bar_color)
    state.active_bar_thickness = ImGui.SliderFloat("Active Bar Thickness", state.active_bar_thickness, 1.0, 6.0, tostring(state.active_bar_thickness))
    state.active_bar_padding = ImGui.SliderFloat("Active Bar Padding", state.active_bar_padding, 0, 25, tostring(state.active_bar_padding))
    state.selected_slot_color = ImGui.ColorEdit4("Selected Slot Color", state.selected_slot_color)
    state.selected_slot_thickness = ImGui.SliderFloat("Selected Slot Thickness", state.selected_slot_thickness, 1.0, 6.0, tostring(state.selected_slot_thickness))

    ImGui.Separator()
    ImGui.Text("Cooldown")
    state.show_cooldown_overlay = ImGui.Checkbox("Show Cooldown Overlay", state.show_cooldown_overlay)
    state.cooldown_dim_color = ImGui.ColorEdit4("Cooldown Dim Color", state.cooldown_dim_color)
    state.cooldown_text_color = ImGui.ColorEdit4("Cooldown Text Color", state.cooldown_text_color)
    ImGui.TextUnformatted("Countdown uses the ability lockout events, so it is exact once")
    ImGui.TextUnformatted("the cooldown starts. A cooldown already running before the bar")
    ImGui.TextUnformatted("subscribed falls back to an estimate from when it was first seen.")

    ImGui.Separator()
    ImGui.Text("Info Overlays")
    state.show_power_cost = ImGui.Checkbox("Show Power Cost", state.show_power_cost)
    state.power_cost_text_color = ImGui.ColorEdit4("Power Cost Color", state.power_cost_text_color)
    state.show_slot_number = ImGui.Checkbox("Show Slot Number", state.show_slot_number)
    state.slot_number_color = ImGui.ColorEdit4("Slot Number Color", state.slot_number_color)

    ImGui.Separator()
    ImGui.Text("Text")
    state.font_scale = ImGui.SliderFloat("Font Scale", state.font_scale, 0.5, 3.0, tostring(state.font_scale))
    state.bold_text = ImGui.Checkbox("Bold Text", state.bold_text)

    ImGui.Separator()
    state.show_debug_info = ImGui.Checkbox("Show Debug Info (troubleshooting)", state.show_debug_info)

    ImGui.Separator()
    ImGui.Text("Out-of-Range Indicator")
    ImGui.TextUnformatted("Only shown when you have a target selected -- uses Ability:IsInRange()")
    ImGui.TextUnformatted("(new in v1.0). No target means nothing to check range against.")
    state.show_range_indicator = ImGui.Checkbox("Show Out-of-Range Tint", state.show_range_indicator)
    state.out_of_range_color = ImGui.ColorEdit4("Out-of-Range Color", state.out_of_range_color)

    state.window_transparent = ImGui.Checkbox("Window Transparent (only slots have background)", state.window_transparent)

    ImGui.Separator()
    ImGui.Text("Ability Name Banner")
    ImGui.TextUnformatted("A separate, big-font window showing the highlighted ability's name")
    ImGui.TextUnformatted("and power cost. Drag it to position above the ability bar yourself.")
    state.show_name_banner = ImGui.Checkbox("Show Name Banner", state.show_name_banner)
    state.name_banner_font_scale = ImGui.SliderFloat("Banner Font Scale", state.name_banner_font_scale, 1.0, 5.0, tostring(state.name_banner_font_scale))
    state.name_banner_show_power_cost = ImGui.Checkbox("Show Power Cost In Banner", state.name_banner_show_power_cost)
    state.name_banner_text_color = ImGui.ColorEdit4("Banner Text Color", state.name_banner_text_color)
    state.name_banner_power_cost_color = ImGui.ColorEdit4("Banner Power Cost Color", state.name_banner_power_cost_color)
    state.name_banner_background_color = ImGui.ColorEdit4("Banner Background Color", state.name_banner_background_color)
    state.name_banner_window_transparent = ImGui.Checkbox("Banner Window Transparent", state.name_banner_window_transparent)
    state.name_banner_show_border = ImGui.Checkbox("Show Banner Border", state.name_banner_show_border)
    state.name_banner_border_color = ImGui.ColorEdit4("Banner Border Color", state.name_banner_border_color)
    state.name_banner_border_thickness = ImGui.SliderFloat("Banner Border Thickness", state.name_banner_border_thickness, 0.5, 5.0, tostring(state.name_banner_border_thickness))
    state.name_banner_corner_rounding = ImGui.SliderFloat("Banner Corner Rounding", state.name_banner_corner_rounding, 0, 20, tostring(state.name_banner_corner_rounding))

    local new_val, pressed = ImGui.Checkbox("Disable Default Ability Bar", state.disable_default_ability_bar)
    if pressed then
        state.disable_default_ability_bar = new_val
        AvoidsCommon.ApplyNativeToggle(state, "native_toggle_confirmed", ToggleDefaultAbilityBar)
    end

end

-- ============================================================================
-- Navigation (now reads the game's REAL selection state -- see header notes)
-- ============================================================================

-- Returns the game's actual selected bar/slot indices, or nil, nil when the UI isn't loaded.
-- Cache this ONCE per frame at the top of Render() rather than calling it repeatedly --
-- it's cheap, but there's no reason to re-resolve the same pointer chain many times a frame.
local function GetRealSelection()
    local bar_index = AbilityBar.GetSelectedBarIndex()
    if bar_index == nil then
        return nil, nil
    end
    local slot_index = AbilityBar.GetSelectedSlotIndex(bar_index)
    return bar_index, slot_index
end

-- Returns (player_coords, target_coords) for range checking, or (coords, nil) if you have
-- no target selected -- computed once per frame and passed down, rather than re-resolved
-- for every single slot.
local function GetRangeCheckCoordinates()
    local ok_player, player_coords = pcall(Player.GetCoordinates)
    if not ok_player or player_coords == nil then return nil, nil end

    local ok_target_id, target_id = pcall(Player.GetTargetEntityId)
    if not ok_target_id or target_id == nil or target_id == 0 then return player_coords, nil end

    local ok_target, target_entity = pcall(EntityList.GetEntityById, target_id)
    if not ok_target or target_entity == nil then return player_coords, nil end

    return player_coords, { x = target_entity.x, y = target_entity.y, z = target_entity.z }
end

-- ============================================================================
-- Cooldown tracking. The exact lockout comes from the AbilityList lockout event
-- bus (Acquire + Update + On OnLockoutStart/OnLockoutEnd), which reports the true
-- duration the moment the server addresses a slot. The old poll estimate is kept
-- only as a fallback for a cooldown that was already running before we subscribed.
-- ============================================================================

local COOLDOWN_OWNER = "avoids_modern_ui_ability_bar"

-- display index -> { lockout_ms, started_ms }, driven by the lockout events. The
-- game stores a whole lockout and never counts it down, so the countdown runs
-- from the start event's timestamp.
cooldown_events = cooldown_events or {}
cooldowns_subscribed = cooldowns_subscribed or false

-- Fallback estimate, populated by polling. Keyed by ability id.
cooldown_watch_state = cooldown_watch_state or {}

local function NowMs()
    return math.floor(ImGui.GetTime() * 1000)
end

-- Subscribes to the lockout events and claims the hook. Guarded so a hot reload
-- or a re-enable cannot stack duplicates or double-claim.
local function SubscribeCooldowns()
    if cooldowns_subscribed then return end
    AbilityList.On(COOLDOWN_OWNER, AbilityList.Events.OnLockoutStart, function(event)
        cooldown_events[event.display_index] = { lockout_ms = event.lockout_ms, started_ms = NowMs() }
    end)
    AbilityList.On(COOLDOWN_OWNER, AbilityList.Events.OnLockoutEnd, function(event)
        cooldown_events[event.display_index] = nil
    end)
    AbilityList.Acquire(COOLDOWN_OWNER)
    cooldowns_subscribed = true
end

-- Drops the subscriptions and the hook claim. Paired with SubscribeCooldowns and
-- called from OnDisable and whenever cooldown tracking is turned off. Assigns the
-- local forward declared above so OnDisable can see it.
function ReleaseCooldowns()
    if not cooldowns_subscribed then return end
    AbilityList.Off(COOLDOWN_OWNER, AbilityList.Events.OnLockoutStart)
    AbilityList.Off(COOLDOWN_OWNER, AbilityList.Events.OnLockoutEnd)
    AbilityList.Release(COOLDOWN_OWNER)
    cooldown_events = {}
    cooldowns_subscribed = false
end

local function UpdateCooldownTracking()
    if Util.IsInGame() == 0 then return end

    -- Drain the lockout ring and fan the start/end events out to the handlers.
    AbilityList.Update()

    -- The poll fallback records when a cooldown was first observed, for slots the
    -- events never announced a start for.
    local now = os.clock()
    for bar_index = 0, AbilityBar.num_bars - 1 do
        for slot_index = 0, AbilityBar.GetSlotCount(bar_index) - 1 do
            local ok, ability = pcall(AbilityBar.GetAbility, bar_index, slot_index)
            if ok and ability ~= nil then
                local id = ability:GetId()
                if not ability:IsOnCooldown() then
                    cooldown_watch_state[id] = nil
                elseif cooldown_watch_state[id] == nil then
                    cooldown_watch_state[id] = now
                end
            end
        end
    end
end

-- Remaining cooldown seconds for an ability, or nil when it is not on cooldown.
-- Exact when the lockout event was captured, otherwise a poll-based estimate.
local function GetCooldownRemaining(ability)
    if not ability:IsOnCooldown() then return nil end

    local display_index = ability:GetDisplayIndex()
    local running = display_index ~= nil and cooldown_events[display_index] or nil
    if running ~= nil then
        local remaining = (running.lockout_ms - (NowMs() - running.started_ms)) / 1000
        if remaining < 0 then remaining = 0 end
        return remaining
    end

    -- Fallback: estimate from when the cooldown was first observed.
    local started = cooldown_watch_state[ability:GetId()]
    if started == nil then return nil end
    local elapsed = os.clock() - started
    local remaining = (ability:GetCooldownLockoutMs() / 1000) - elapsed
    if remaining < 0 then remaining = 0 end
    return remaining
end

local function BuildAbilityTooltip(ability)
    local ok_name, name = TryCall(ability, "GetName")
    local ok_desc, description = TryCall(ability, "GetDescription")
    local ok_range, range = TryCall(ability, "GetRange")
    local ok_cast, cast_time = TryCall(ability, "GetCastTime")
    local ok_cost, pwr_cost = TryCall(ability, "GetPwrCost")
    local ok_cd, cooldown = TryCall(ability, "GetCooldown")
    local ok_on_cd, on_cooldown = TryCall(ability, "IsOnCooldown")

    local lines = { ok_name and name or "Unknown Ability" }
    if ok_desc and description ~= nil and description ~= "" then
        lines[#lines + 1] = description
    end
    lines[#lines + 1] = "Power Cost: " .. (ok_cost and tostring(pwr_cost) or "?")
    lines[#lines + 1] = "Cast Time: " .. (ok_cast and tostring(cast_time) or "?")
    lines[#lines + 1] = "Range: " .. (ok_range and tostring(range) or "?")
    lines[#lines + 1] = "Base Cooldown: " .. (ok_cd and tostring(cooldown) or "?")

    if ok_on_cd and on_cooldown then
        local remaining = GetCooldownRemaining(ability)
        if remaining ~= nil then
            lines[#lines + 1] = string.format("On Cooldown: %.1fs remaining", remaining)
        else
            lines[#lines + 1] = "On Cooldown (remaining time not yet observed)"
        end
    end

    return table.concat(lines, "\n")
end

-- ============================================================================
-- Rendering
-- ============================================================================

local function DrawSlot(state, bar_index, slot_index, scaled_slot_size, selected_bar_index, selected_slot_index, player_coords, target_coords)
    local is_selected = state.show_navigation_highlight
        and selected_bar_index == bar_index
        and selected_slot_index == slot_index

    local draw_list = ImGui.GetWindowDrawList()
    local slot_origin_x, slot_origin_y = ImGui.GetCursorScreenPos()
    local slot_p0 = ImVec2.new(slot_origin_x, slot_origin_y)
    local slot_p1 = ImVec2.new(slot_origin_x + scaled_slot_size, slot_origin_y + scaled_slot_size)

    local ok_slot, ability_slot = pcall(AbilityBar.GetAbilitySlot, bar_index, slot_index)
    if not ok_slot then ability_slot = nil end

    local ability = nil
    local bg_texture, fg_texture = nil, nil
    local debug_text = nil
    local is_non_ability_item = false -- true for slots holding something OTHER than an ability (e.g. a toolbelt item/weapon) -- see notes below

    -- Bar index 2 (the third bar) is the toolbelt/special-items bar. Its slot icons are a
    -- STATIC PER-POSITION PLACEHOLDER (the game's own "this slot accepts potions" glyph) shown
    -- regardless of whether anything is assigned -- unlike ability slots, which only ever show
    -- an icon once something occupies them. So this lookup deliberately runs BEFORE and
    -- independent of the IsEmpty() check below, only for this one bar.
    --
    -- NOTE: an earlier attempt tried resolving the SPECIFIC assigned item (reading source_index
    -- as an inventory slot number) to override this placeholder with the item's own icon.
    -- CONFIRMED WRONG by testing -- it resolved to a real item, just not the correct one, which
    -- is worse than the honest placeholder. Reverted; this placeholder-only version is the
    -- confirmed-correct state.
    if bar_index == 2 then
        local ok_tex_id, tex_id = pcall(AbilityBar.GetSlotUITexId, slot_index)
        if ok_tex_id and tex_id ~= nil then
            local ok_ui_tex, ui_tex = pcall(Icon.GetUITexture, tex_id)
            if ok_ui_tex and ui_tex ~= nil then
                fg_texture = ui_tex
            elseif debug_text == nil then
                debug_text = "Icon.GetUITexture failed: " .. tostring(ui_tex)
            end
        elseif debug_text == nil then
            debug_text = "AbilityBar.GetSlotUITexId returned nil for this slot position"
        end
    end

    if ability_slot ~= nil then
        local ok_empty, is_empty = TryCall(ability_slot, "IsEmpty")
        if ok_empty and not is_empty then
            local ok_ability, ability_result = TryCall(ability_slot, "GetAbility")
            if ok_ability then
                ability = ability_result
            elseif debug_text == nil then
                debug_text = "GetAbility failed: " .. tostring(ability_result)
            end

            -- A slot can hold something (not IsEmpty) while GetAbility() still returns nil --
            -- this happens for toolbelt/item slots, which use a different source_type than the
            -- ability list. Still no NAME available for these anywhere in the API (the
            -- source_index -> inventory-slot mapping was tested and confirmed wrong).
            if ability == nil then
                is_non_ability_item = true
            end

            -- Icons are decoded straight from game memory via the Icon module and are layered:
            -- a background piece plus a foreground piece drawn on top, matching how the game
            -- itself composites hotbar/spellbook icons. Both refs live on the resolved Ability
            -- object.
            if ability ~= nil then
                local ok_bg_ref, bg_ref = TryCall(ability, "GetIconBackgroundRef")
                local ok_fg_ref, fg_ref = TryCall(ability, "GetIconForegroundRef")

                if ok_bg_ref then
                    local ok_bg_tex, bg_tex = pcall(Icon.GetTexture, bg_ref, { trim_transparent = true, trim_color = true })
                    if ok_bg_tex then
                        bg_texture = bg_tex
                    elseif debug_text == nil then
                        debug_text = "Icon.GetTexture (background) failed: " .. tostring(bg_tex)
                    end
                end

                if ok_fg_ref then
                    local ok_fg_tex, fg_tex = pcall(Icon.GetTexture, fg_ref, { trim_transparent = true, trim_color = true })
                    if ok_fg_tex then
                        fg_texture = fg_tex
                    elseif debug_text == nil then
                        debug_text = "Icon.GetTexture (foreground) failed: " .. tostring(fg_tex)
                    end
                end
            elseif fg_texture == nil then
                -- Toolbelt/item slot and the bar-2 lookup above didn't get a texture (e.g. this
                -- somehow isn't bar 2, or that lookup failed) -- fall back to the per-slot ref.
                local ok_slot_ref, slot_ref = TryCall(ability_slot, "GetIconRef")
                if ok_slot_ref then
                    local ok_tex, tex = pcall(Icon.GetTexture, slot_ref, { trim_transparent = true, trim_color = true })
                    if ok_tex then
                        fg_texture = tex
                    elseif debug_text == nil then
                        debug_text = "Icon.GetTexture (slot) failed: " .. tostring(tex)
                    end
                end
            end
        end
    else
        debug_text = "GetAbilitySlot returned nil or errored"
    end

    -- Background
    local bg_color = state.empty_slot_color
    if ability ~= nil then
        bg_color = state.background_color
    elseif is_non_ability_item then
        bg_color = state.item_slot_color
    end
    local bg_color_u32 = ImGui.GetColorU32(bg_color[1], bg_color[2], bg_color[3], bg_color[4])
    draw_list:AddRectFilled(slot_p0, slot_p1, bg_color_u32, state.corner_rounding)

    -- Icon layers, both stretched to fill the slot uniformly (falls back to an invisible
    -- placeholder of the same size if neither layer is available).
    local drew_icon = false
    if state.show_icon_background_layer and bg_texture ~= nil then
        local cursor_x, cursor_y = ImGui.GetCursorPos()
        local ok_draw = pcall(ImGui.Image, bg_texture, scaled_slot_size, scaled_slot_size)
        if ok_draw then
            drew_icon = true
            ImGui.SetCursorPos(cursor_x, cursor_y)
        elseif debug_text == nil then
            debug_text = "ImGui.Image failed on the background texture"
        end
    end
    if fg_texture ~= nil then
        local ok_draw = pcall(ImGui.Image, fg_texture, scaled_slot_size, scaled_slot_size)
        if ok_draw then
            drew_icon = true
        elseif debug_text == nil then
            debug_text = "ImGui.Image failed on the foreground texture"
        end
    end
    if not drew_icon then
        ImGui.Dummy(scaled_slot_size, scaled_slot_size)
    end

    -- Cooldown overlay: dims the icon and shows the remaining countdown
    if state.show_cooldown_overlay and ability ~= nil then
        local ok_on_cd, on_cooldown = TryCall(ability, "IsOnCooldown")
        if ok_on_cd and on_cooldown then
            local dim_color_u32 = ImGui.GetColorU32(state.cooldown_dim_color[1], state.cooldown_dim_color[2], state.cooldown_dim_color[3], state.cooldown_dim_color[4])
            draw_list:AddRectFilled(slot_p0, slot_p1, dim_color_u32, state.corner_rounding)

            local remaining = GetCooldownRemaining(ability)
            if remaining ~= nil then
                local cd_text = remaining >= 10 and string.format("%.0f", remaining) or string.format("%.1f", remaining)
                local cd_text_width, cd_text_height = ImGui.CalcTextSize(cd_text)
                local cd_color_u32 = ImGui.GetColorU32(state.cooldown_text_color[1], state.cooldown_text_color[2], state.cooldown_text_color[3], state.cooldown_text_color[4])
                draw_list:AddText(ImVec2.new(slot_origin_x + (scaled_slot_size - cd_text_width) / 2, slot_origin_y + (scaled_slot_size - cd_text_height) / 2), cd_color_u32, cd_text)
            end
        end
    end

    -- Out-of-range overlay: only shown when you actually have a target selected -- with no
    -- target there's nothing to check range against, so abilities render normally.
    local is_out_of_range = false
    if state.show_range_indicator and ability ~= nil and player_coords ~= nil and target_coords ~= nil then
        local ok_range, in_range = pcall(function() return ability:IsInRange(player_coords, target_coords) end)
        if ok_range and in_range == false then
            is_out_of_range = true
        end
    end
    if is_out_of_range then
        local range_color_u32 = ImGui.GetColorU32(state.out_of_range_color[1], state.out_of_range_color[2], state.out_of_range_color[3], state.out_of_range_color[4])
        draw_list:AddRectFilled(slot_p0, slot_p1, range_color_u32, state.corner_rounding)
    end

    if ImGui.IsItemHovered() then
        if ability ~= nil then
            local tooltip_text = BuildAbilityTooltip(ability)
            if is_out_of_range then
                tooltip_text = tooltip_text .. "\nOut of range"
            end
            ImGui.SetTooltip(tooltip_text)
        elseif state.show_debug_info and debug_text ~= nil then
            ImGui.SetTooltip(debug_text)
        elseif state.show_debug_info and is_non_ability_item then
            -- Debug info IS on, there just wasn't an error to report -- the icon lookup
            -- succeeded (that's why fg_texture is set and drew_icon is true). Previously this
            -- fell through to a message telling you to "enable Show Debug Info" even though it
            -- was already on, which was confusing/wrong. Report the actual status instead.
            ImGui.SetTooltip(drew_icon and "Item/Toolbelt Slot -- icon resolved fine, no error" or "Item/Toolbelt Slot -- no icon and no error recorded (unexpected)")
        elseif is_non_ability_item then
            ImGui.SetTooltip("Item/Toolbelt Slot (not empty -- enable Show Debug Info for details)")
        else
            ImGui.SetTooltip("Empty Slot")
        end
    end

    if state.show_border then
        local border_color_u32 = ImGui.GetColorU32(state.border_color[1], state.border_color[2], state.border_color[3], state.border_color[4])
        draw_list:AddRect(slot_p0, slot_p1, border_color_u32, state.corner_rounding, 0, state.border_thickness)
    end

    -- Navigation cursor highlight, drawn on top of the normal border
    if is_selected then
        local sel_color_u32 = ImGui.GetColorU32(state.selected_slot_color[1], state.selected_slot_color[2], state.selected_slot_color[3], state.selected_slot_color[4])
        draw_list:AddRect(slot_p0, slot_p1, sel_color_u32, state.corner_rounding, 0, state.selected_slot_thickness)
    end

    if state.show_slot_number then
        local label = tostring(slot_index + 1)
        local label_color_u32 = ImGui.GetColorU32(state.slot_number_color[1], state.slot_number_color[2], state.slot_number_color[3], state.slot_number_color[4])
        draw_list:AddText(ImVec2.new(slot_p0.x + 2, slot_p0.y + 2), label_color_u32, label)
    end

    if is_non_ability_item and not drew_icon then
        -- Only shown as a last-resort fallback now -- v1.0's AbilityBar.GetSlotUITexId() +
        -- Icon.GetUITexture() usually gets a real icon for these slots (see the icon-fetching
        -- section above), so this text label only appears if that somehow didn't resolve.
        local item_label = "ITEM"
        local item_text_width, item_text_height = ImGui.CalcTextSize(item_label)
        local item_color_u32 = ImGui.GetColorU32(1, 1, 1, 0.85)
        draw_list:AddText(ImVec2.new(slot_origin_x + (scaled_slot_size - item_text_width) / 2, slot_origin_y + (scaled_slot_size - item_text_height) / 2), item_color_u32, item_label)
    end

    if state.show_power_cost and ability ~= nil then
        local ok_cost, pwr_cost = TryCall(ability, "GetPwrCost")
        if ok_cost and pwr_cost ~= nil then
            local cost_text = tostring(pwr_cost)
            local text_width, text_height = ImGui.CalcTextSize(cost_text)
            local cost_color_u32 = ImGui.GetColorU32(state.power_cost_text_color[1], state.power_cost_text_color[2], state.power_cost_text_color[3], state.power_cost_text_color[4])
            draw_list:AddText(ImVec2.new(slot_p1.x - text_width - 2, slot_p1.y - text_height - 2), cost_color_u32, cost_text)
        end
    end

    if state.show_debug_info and debug_text ~= nil then
        local warning_color_u32 = ImGui.GetColorU32(1, 0.3, 0.3, 1)
        draw_list:AddText(ImVec2.new(slot_p0.x + 2, slot_p1.y - 14), warning_color_u32, "!")
    end
end

local function DrawBar(state, bar_index, scaled_slot_size, selected_bar_index, selected_slot_index, player_coords, target_coords)
    local slot_count = AbilityBar.GetSlotCount(bar_index)
    local is_active_bar = state.show_navigation_highlight and selected_bar_index == bar_index

    local bar_origin_x, bar_origin_y = ImGui.GetCursorScreenPos()

    ImGui.BeginGroup()
    for slot_index = 0, slot_count - 1 do
        if slot_index > 0 and state.slot_spacing > 0 then
            ImGui.Dummy(1, state.slot_spacing)
        end
        DrawSlot(state, bar_index, slot_index, scaled_slot_size, selected_bar_index, selected_slot_index, player_coords, target_coords)
    end
    ImGui.EndGroup()
    local _, bar_end_y = ImGui.GetCursorScreenPos()

    if is_active_bar then
        local draw_list = ImGui.GetWindowDrawList()
        local active_color_u32 = ImGui.GetColorU32(state.active_bar_color[1], state.active_bar_color[2], state.active_bar_color[3], state.active_bar_color[4])
        local bar_width = scaled_slot_size
        -- Measured directly from the cursor's actual position after drawing, rather than
        -- calculated from slot_count * scaled_slot_size + gaps -- that formula didn't account
        -- for ImGui's own automatic spacing between widgets (added on top of our explicit
        -- gaps), which made the border fall short of the last slot.
        local bar_height = bar_end_y - bar_origin_y
        local vertical_pad = state.active_bar_padding * state.bar_scale
        -- Horizontal padding is capped separately from vertical: active_bar_padding scales with
        -- bar_scale, but the gap between bars (bar_spacing) does NOT -- at larger icon sizes the
        -- padding could exceed that gap entirely, making the border spill into the neighboring
        -- bar's icons. Capping to half the gap (minus a small margin) guarantees it never does,
        -- while top/bottom keep the full requested padding since there's no neighbor there.
        local horizontal_pad = math.min(vertical_pad, math.max(state.bar_spacing / 2 - 1, 0))
        draw_list:AddRect(
            ImVec2.new(bar_origin_x - horizontal_pad, bar_origin_y - vertical_pad),
            ImVec2.new(bar_origin_x + bar_width + horizontal_pad, bar_origin_y + bar_height + vertical_pad),
            active_color_u32, state.corner_rounding + 2, 0, state.active_bar_thickness)
    end
end

-- Builds a plain-text (not tooltip) description of whatever the D-pad cursor is currently on,
-- since hover tooltips are useless for controller-only navigation -- there's no mouse to hover
-- with. This is shown as a permanent panel in the window instead.
local function BuildHighlightedSlotInfo(state, bar_index, slot_index)
    if bar_index == nil or slot_index == nil then
        return "(game UI not loaded)"
    end

    local ok_slot, ability_slot = pcall(AbilityBar.GetAbilitySlot, bar_index, slot_index)
    if not ok_slot or ability_slot == nil then
        return "Bar " .. (bar_index + 1) .. ", Slot " .. (slot_index + 1) .. ": (no data)"
    end

    local ok_empty, is_empty = TryCall(ability_slot, "IsEmpty")
    if ok_empty and is_empty then
        return "Bar " .. (bar_index + 1) .. ", Slot " .. (slot_index + 1) .. ": Empty"
    end

    local ok_ability, ability = TryCall(ability_slot, "GetAbility")
    if ok_ability and ability ~= nil then
        return BuildAbilityTooltip(ability)
    end

    -- Known limitation: toolbelt/item slots (a different source_type than the ability list)
    -- don't currently expose a name anywhere in the API. (An earlier attempt tried reading
    -- source_index as an inventory slot number -- confirmed wrong by testing, removed.)
    local base_text = "Bar " .. (bar_index + 1) .. ", Slot " .. (slot_index + 1) .. ": Item/Toolbelt slot (name not yet available)"

    if not state.show_debug_info then
        return base_text
    end

    -- Same diagnostic DrawSlot runs for the mouse-hover tooltip, duplicated here so it's
    -- readable by following the D-Pad cursor alone -- this whole bar is controller-navigated,
    -- so a mouse-only diagnostic is a poor fit.
    local ok_tex_id, tex_id = pcall(AbilityBar.GetSlotUITexId, slot_index)
    if not ok_tex_id or tex_id == nil then
        return base_text .. "\nDEBUG: AbilityBar.GetSlotUITexId returned nil for this slot position"
    end
    local ok_ui_tex, ui_tex = pcall(Icon.GetUITexture, tex_id)
    if not ok_ui_tex or ui_tex == nil then
        return base_text .. string.format("\nDEBUG: GetSlotUITexId=%s, but Icon.GetUITexture failed: %s", tostring(tex_id), tostring(ui_tex))
    end
    return base_text .. string.format("\nDEBUG: GetSlotUITexId=%s resolved fine -- if no icon is showing, the issue is in ImGui.Image itself", tostring(tex_id))
end

-- Just the name + power cost, for the big banner (as opposed to BuildHighlightedSlotInfo's
-- full multi-line tooltip-style text used in the bottom info panel).
local function GetHighlightedNameAndCost(bar_index, slot_index)
    if bar_index == nil or slot_index == nil then
        return "(game UI not loaded)", nil
    end

    local ok_slot, ability_slot = pcall(AbilityBar.GetAbilitySlot, bar_index, slot_index)
    if not ok_slot or ability_slot == nil then
        return "(no data)", nil
    end

    local ok_empty, is_empty = TryCall(ability_slot, "IsEmpty")
    if ok_empty and is_empty then
        return "Empty", nil
    end

    local ok_ability, ability = TryCall(ability_slot, "GetAbility")
    if ok_ability and ability ~= nil then
        local ok_name, name = TryCall(ability, "GetName")
        local ok_cost, cost = TryCall(ability, "GetPwrCost")
        return (ok_name and name or "Unknown Ability"), (ok_cost and cost or nil)
    end

    -- Known limitation: same as BuildHighlightedSlotInfo -- toolbelt/item slots have no name
    -- exposed anywhere in the API yet.
    return "Item/Toolbelt (name not yet available)", nil
end

local function DrawNameBanner(state)
    if Util.IsInGame() == 0 or Util.IsStartMenuOpen() == 1 then return end
    if state.show_name_banner ~= true then return end

    local window_flags = ImGuiWindowFlags.AlwaysAutoResize + ImGuiWindowFlags.NoTitleBar
    if state.name_banner_window_transparent then window_flags = window_flags + ImGuiWindowFlags.NoBackground end

    if ImGui.Begin("Ability Name Banner", true, window_flags) then
        local selected_bar_index, selected_slot_index = GetRealSelection()
        local name, cost = GetHighlightedNameAndCost(selected_bar_index, selected_slot_index)

        local origin_x, origin_y = ImGui.GetCursorScreenPos()
        local draw_list = ImGui.GetWindowDrawList()

        ImGui.SetWindowFontScale(state.name_banner_font_scale)
        local name_width, name_height = ImGui.CalcTextSize(name)

        local cost_text = nil
        local cost_width, cost_height = 0, 0
        if state.name_banner_show_power_cost and cost ~= nil then
            cost_text = "Power Cost: " .. tostring(cost)
            cost_width, cost_height = ImGui.CalcTextSize(cost_text)
        end

        local content_width = math.max(name_width, cost_width)
        local content_height = name_height + (cost_text ~= nil and cost_height or 0)
        local padding = 10

        if state.name_banner_background_color[4] > 0 or state.name_banner_show_border then
            local bg_color_u32 = ImGui.GetColorU32(state.name_banner_background_color[1], state.name_banner_background_color[2], state.name_banner_background_color[3], state.name_banner_background_color[4])
            draw_list:AddRectFilled(
                ImVec2.new(origin_x - padding, origin_y - padding),
                ImVec2.new(origin_x + content_width + padding, origin_y + content_height + padding),
                bg_color_u32, state.name_banner_corner_rounding)

            if state.name_banner_show_border then
                local border_color_u32 = ImGui.GetColorU32(state.name_banner_border_color[1], state.name_banner_border_color[2], state.name_banner_border_color[3], state.name_banner_border_color[4])
                draw_list:AddRect(
                    ImVec2.new(origin_x - padding, origin_y - padding),
                    ImVec2.new(origin_x + content_width + padding, origin_y + content_height + padding),
                    border_color_u32, state.name_banner_corner_rounding, 0, state.name_banner_border_thickness)
            end
        end

        local name_color_u32 = ImGui.GetColorU32(state.name_banner_text_color[1], state.name_banner_text_color[2], state.name_banner_text_color[3], state.name_banner_text_color[4])
        draw_list:AddText(ImVec2.new(origin_x, origin_y), name_color_u32, name)

        if cost_text ~= nil then
            local cost_color_u32 = ImGui.GetColorU32(state.name_banner_power_cost_color[1], state.name_banner_power_cost_color[2], state.name_banner_power_cost_color[3], state.name_banner_power_cost_color[4])
            draw_list:AddText(ImVec2.new(origin_x, origin_y + name_height), cost_color_u32, cost_text)
        end

        -- Reserve the actual space so the window sizes itself correctly around the manually-
        -- drawn text above (draw_list calls don't advance the layout cursor on their own).
        ImGui.Dummy(content_width, content_height)
    end
    ImGui.End()
end

local function Render()
    local state = modern_ability_bar_state

    if Util.IsInGame() == 0 or Util.IsStartMenuOpen() == 1 then return end

    -- Removes the window's own default fill when "Window Transparent" is on -- our own slot
    -- backgrounds are drawn explicitly regardless and stay visible either way.
    local window_flags = ImGuiWindowFlags.AlwaysAutoResize + ImGuiWindowFlags.NoTitleBar
    if state.window_transparent then window_flags = window_flags + ImGuiWindowFlags.NoBackground end

    if ImGui.Begin("modern ability bar window", true, window_flags) then
        ImGui.SetWindowFontScale(state.font_scale)

        local scaled_slot_size = state.slot_size * state.bar_scale
        local selected_bar_index, selected_slot_index = GetRealSelection()
        local player_coords, target_coords = GetRangeCheckCoordinates()

        -- The active-bar border (drawn in DrawBar) extends active_bar_padding pixels above
        -- the topmost icon and below the bottom icon of whichever bar is currently active.
        -- Since this window uses AlwaysAutoResize (sizes tightly to the normal widget
        -- layout), that overhang isn't accounted for on its own -- the top portion gets
        -- clipped by the window's own edge, and the bottom edge looks disconnected from the
        -- content. Reserving this space explicitly, once, fixes both.
        local border_overhang = 0
        if state.show_navigation_highlight then
            border_overhang = state.active_bar_padding * state.bar_scale
        end
        if border_overhang > 0 then
            ImGui.Dummy(1, border_overhang)
        end

        for bar_index = 0, AbilityBar.num_bars - 1 do
            if bar_index > 0 then
                ImGui.SameLine(0, state.bar_spacing)
            end
            DrawBar(state, bar_index, scaled_slot_size, selected_bar_index, selected_slot_index, player_coords, target_coords)
        end

        if border_overhang > 0 then
            ImGui.Dummy(1, border_overhang)
        end

        if state.show_navigation_highlight then
            if state.info_panel_top_spacing > 0 then
                ImGui.Dummy(1, state.info_panel_top_spacing)
            end
            ImGui.Separator()
            ImGui.PushTextWrapPos(0.0)
            ImGui.TextUnformatted(BuildHighlightedSlotInfo(state, selected_bar_index, selected_slot_index))
            ImGui.PopTextWrapPos()
        end
    end
    ImGui.End()
end


-- Creating a local variable to interact with the global table so that the name is shorter/easier to use

local function Update()
    if modern_ability_bar_state.initialized == false then Initialize() end

    -- Cooldown tracking runs regardless of window visibility, so the lockout events are drained
    -- and the countdowns stay correct even when the bar is hidden. Subscribing claims the hook,
    -- turning the setting off drops it.
    if modern_ability_bar_state.enable_cooldown_tracking then
        SubscribeCooldowns()
        UpdateCooldownTracking()
    else
        ReleaseCooldowns()
    end

    -- v0.1.1: retry only until confirmed (see the note on ToggleDefaultAbilityBar above) --
    -- replaces the old startup-sequence workaround now that we have a real success signal.
    AvoidsCommon.EnsureNativeToggle(modern_ability_bar_state, "native_toggle_confirmed", ToggleDefaultAbilityBar)

    if modern_ability_bar_state.initialized then Render() end
    if modern_ability_bar_state.initialized then DrawNameBanner(modern_ability_bar_state) end
end

return {
    display_name = "Ability Bar",
    Update = Update,
    Settings = Settings,
    OnDisable = OnDisable,
    Save = Save,
    Load = Load,
}
