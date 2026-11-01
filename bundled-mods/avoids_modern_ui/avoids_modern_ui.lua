--[[
================================================================================
 Modern UI Mods
================================================================================
 Author - Avoids
================================================================================

A pack containing every "modern" UI script: Health Bar, Mana Bar, Experience Bars, Ability
Bar, Chat Log, Group Frames, Pet Frame, Quest Log, Effects (Buffs/Debuffs), and Target
Level. The World Map now lives in its own package at scripts\world_map. Each one has its
own Enable checkbox and full settings panel, all inside this one file's Settings tab.

This file is only the dispatcher. Every mod lives in its own file under modules\.

ARCHITECTURE NOTES (read this before touching the structure below):

1. Each mod is a module under modules\, named avoids_<mod>.lua. They were one file's worth
   of `do...end` blocks before, which was needed because every mod defines identically named
   locals (Settings, Render, Initialize, OnDisable, Update -- literally every mod has all of
   these) and would otherwise shadow each other. Separate files give the same isolation,
   with the benefit that a mod can be read without scrolling past nine others.

   The avoids_ prefix is deliberate. Module names are required bare, so a plain "chat_log"
   could collide with a same named module belonging to another script.

2. Each module returns its mod table -- { display_name, Update, Settings, OnDisable, Save?,
   Load? } -- and this file assembles them into ModPack. That table is how the master
   dispatcher reaches into each mod without needing to know its internals.

3. UiForge only allows ONE callback registration per type (Settings, DisableScript, Save, Load)
   for an entire script file. Previously each mod was its own separate file with its own
   callbacks; now there's exactly one master Settings/DisableScript/Save/Load callback here,
   which fans out to whichever mods need it.

4. Settings live in the UiForge profile, the same as every other script in this project. Each
   mod lists what it persists in a PERSISTED_SETTINGS table, and AvoidsPersist turns that list
   into the profile payload. Nothing here writes settings to disk on its own, so File then Save
   Profile is what commits a change and File then Select Profile restores it. Earlier builds
   kept a .cfg file per mod under resources. Those are read once and deleted so an existing
   setup carries over, and that migration code is marked for removal next release.

   Quest notes are the one exception. They are free-form user data rather than settings, so
   they keep their own file under resources\avoids_modern_ui and save as you type.

5. Per-mod enable/disable is a NEW concept this pack introduces (UiForge itself has no notion of
   "sub-scripts" within one file). It's tracked in modern_ui_mods_state.mod_enabled and rides in
   the profile alongside each mod's settings. Turning a mod off calls its OnDisable()
   immediately -- restoring whatever native UI it was hiding -- the same way UiForge used to do
   automatically when you unchecked an individual script in the old per-file setup.

6. Like every script in this project, THIS file re-executes every frame (UiForge has no
   separate "per-frame" callback type -- the file's own top-level execution IS the per-frame
   hook). The modules under modules\ do not. require caches them, so a module body runs once
   and only the Update functions it returns run per frame.

   Each mod still keeps its state in a global table behind the "or {}" idiom rather than a
   module local. That is what carries settings across a script reload, and it is unchanged
   from when every mod was its own standalone file.

   One consequence of require caching worth knowing while editing: changing a file under
   modules\ needs a full reload to take effect, since the cached copy is what keeps running.
================================================================================
]]

modern_ui_mods_state = modern_ui_mods_state or {
    initialized          = false,
    callbacks_registered = false,

    settings_file_loaded = false,

    mod_enabled = {
        HealthBar    = true,
        ManaBar      = true,
        ExpBars      = true,
        AbilityBar   = true,
        ChatLog      = true,
        GroupFrames  = true,
        PetFrame     = true,
        QuestLog     = true,
        Effects      = true,
        TargetLevel  = true,
    },
}

-- Display/settings-panel order. Also the canonical list of valid mod keys.
local MOD_ORDER = {
    "HealthBar", "ManaBar", "ExpBars", "AbilityBar", "ChatLog",
    "GroupFrames", "PetFrame", "QuestLog", "Effects", "TargetLevel",
}

-- ============================================================================
-- THE MODS
-- ============================================================================
--
-- Each module returns its own { display_name, Update, Settings, OnDisable, Save?, Load? }
-- table. The master dispatcher below is the only thing that calls into them, so a mod can
-- be added or removed here and in MOD_ORDER without touching anything else.

ModPack = {
    HealthBar   = require("avoids_health_bar"),
    ManaBar     = require("avoids_mana_bar"),
    ExpBars     = require("avoids_exp_bars"),
    AbilityBar  = require("avoids_ability_bar"),
    ChatLog     = require("avoids_chat_log"),
    GroupFrames = require("avoids_group_frames"),
    PetFrame    = require("avoids_pet_frame"),
    QuestLog    = require("avoids_quest_log"),
    Effects     = require("avoids_effects"),
    TargetLevel = require("avoids_target_level"),
}

-- ============================================================================
-- MASTER DISPATCHER (ties every mod above together)
-- ============================================================================

-- Which mods are enabled rides in the profile alongside each mod's own settings. This is
-- the file older builds wrote it to, read once so an existing setup carries over, then
-- deleted. Removable next release along with the per mod legacy readers.
local LEGACY_MODPACK_FILE = tostring(UiForge.resources_path) .. "\\modern_ui_mods_settings.cfg"

local function MigrateLegacyModEnabled()
    local file = io.open(LEGACY_MODPACK_FILE, "r")
    if file == nil then return end

    for line in file:lines() do
        local trimmed = (tostring(line):gsub("^%s+", ""):gsub("%s+$", ""))
        if trimmed ~= "" and not trimmed:match("^#") then
            local key, raw_value = trimmed:match("^([%w_]+)=(.*)$")
            if key ~= nil and modern_ui_mods_state.mod_enabled[key] ~= nil then
                modern_ui_mods_state.mod_enabled[key] = (raw_value == "true")
            end
        end
    end

    file:close()
    pcall(os.remove, LEGACY_MODPACK_FILE)
end

-- Each mod module scopes its own requires, so the master lifecycle needs its own handles
-- to the modules it touches.
local Icon = require("frontiers_forge.icon")
local Chat = require("frontiers_forge.chat")
local Util = require("frontiers_forge.util")

-- One claim for the whole pack. The texture caches are shared across every mod that draws
-- icons, so ownership sits here rather than in any single mod's lifecycle.
local ICON_OWNER = "avoids_modern_ui_mods"

local function Initialize()
    if modern_ui_mods_state.settings_file_loaded == false then
        MigrateLegacyModEnabled()
        modern_ui_mods_state.settings_file_loaded = true
    end
    modern_ui_mods_state.initialized = true
end

-- Master Settings: one collapsible section per mod, with its own Enable checkbox followed by
-- its full settings panel (unchanged from when it was a standalone script).
local function Settings()
    local state = modern_ui_mods_state

    ImGui.TextUnformatted("Modern UI Mods -- expand a section below to enable/configure it.")
    ImGui.Separator()

    for _, key in ipairs(MOD_ORDER) do
        local mod = ModPack[key]
        if mod ~= nil then
            if ImGui.CollapsingHeader(mod.display_name) then
                ImGui.Indent()

                local new_val, pressed = ImGui.Checkbox("Enable " .. mod.display_name, state.mod_enabled[key])
                if pressed then
                    state.mod_enabled[key] = new_val
                    -- Restore whatever native UI this mod was hiding immediately on disable,
                    -- the same way UiForge used to do automatically when this was its own
                    -- separate script and you unchecked it in the script list.
                    if new_val == false and mod.OnDisable ~= nil then
                        mod.OnDisable()
                    end
                end

                if mod.Settings ~= nil then
                    ImGui.Separator()
                    mod.Settings()
                else
                    ImGui.TextUnformatted("(no additional settings for this mod)")
                end

                ImGui.Unindent()
            end
        end
    end
end

-- Master DisableScript: fires when the WHOLE combined file gets unchecked in UiForge's script
-- list. Restores every mod's native UI regardless of each mod's own individual enabled state,
-- so nothing is left hidden.
local function OnDisable()
    for _, key in ipairs(MOD_ORDER) do
        local mod = ModPack[key]
        if mod ~= nil and mod.OnDisable ~= nil then
            local ok, err = pcall(mod.OnDisable)
            if not ok then
                -- Never let one mod's cleanup failure stop the others from cleaning up too.
                Util.LogError("OnDisable failed for " .. tostring(key) .. ", " .. tostring(err))
            end
        end
    end

    -- Chat subscriptions and the icon claim belong to the pack, so they go once, after
    -- every mod has had its own cleanup.
    Chat.Off("avoids_chat_log")
    Chat.Off("avoids_quest_xref")
    Icon.Release(ICON_OWNER)
end

-- Master Save/Load: combines every mod that has its own Save/Load (HealthBar, ManaBar, ExpBars,
-- ChatLog) into one profile entry, keyed by mod name.
local function Save()
    local saved = {}
    for _, key in ipairs(MOD_ORDER) do
        local mod = ModPack[key]
        if mod ~= nil and mod.Save ~= nil then
            local ok, result = pcall(mod.Save)
            if ok then
                saved[key] = result
            end
        end
    end
    -- Which mods are on is pack level rather than any one mod's setting. The key is lower
    -- case so it cannot collide with a mod name from MOD_ORDER.
    local enabled = {}
    for _, key in ipairs(MOD_ORDER) do
        enabled[key] = modern_ui_mods_state.mod_enabled[key]
    end
    saved.mod_enabled = enabled

    return saved
end

local function Load(saved)
    if type(saved) ~= "table" then return end

    if type(saved.mod_enabled) == "table" then
        for _, key in ipairs(MOD_ORDER) do
            local value = saved.mod_enabled[key]
            if type(value) == "boolean" then
                modern_ui_mods_state.mod_enabled[key] = value
            end
        end
    end

    for _, key in ipairs(MOD_ORDER) do
        local mod = ModPack[key]
        if mod ~= nil and mod.Load ~= nil and saved[key] ~= nil then
            pcall(mod.Load, saved[key])
        end
    end
end

-- Master Render: calls each enabled mod's own Update() (which itself already handles that mod's
-- lazy Initialize(), any per-frame native-UI maintenance, and its Render()) -- unchanged from
-- what each mod already did as a standalone script, just gated by our own enabled flag now.
local function Render()
    -- Claimed every frame, not once at init, so re-enabling the script after OnDisable
    -- released the claim takes it back. Acquire is idempotent per owner.
    Icon.Acquire(ICON_OWNER)

    for _, key in ipairs(MOD_ORDER) do
        local mod = ModPack[key]
        if mod ~= nil and modern_ui_mods_state.mod_enabled[key] == true then
            local ok, err = pcall(mod.Update)
            if not ok then
                Util.LogError("Update failed for " .. tostring(key) .. ", " .. tostring(err))
            end
        end
    end
end

local function RegisterCallbacks()
    UiForge.RegisterCallback(UiForge.CallbackType.Settings, Settings)
    UiForge.RegisterCallback(UiForge.CallbackType.DisableScript, OnDisable)
    UiForge.RegisterCallback(UiForge.CallbackType.Save, Save)
    UiForge.RegisterCallback(UiForge.CallbackType.Load, Load)
    modern_ui_mods_state.callbacks_registered = true
end

if modern_ui_mods_state.initialized == false then Initialize() end

if modern_ui_mods_state.callbacks_registered == false then RegisterCallbacks() end

Render()
