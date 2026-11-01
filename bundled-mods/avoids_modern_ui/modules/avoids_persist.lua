--[[
avoids_persist -- settings persistence shared by every mod in the Modern UI Mods pack.
]]

--
-- Settings live in the UiForge profile, the same as every other FrontiersForge script,
-- so File then Save Profile is what commits them and File then Select Profile restores
-- them. Each mod describes its saved options as a PERSISTED_SETTINGS list of
-- { key = "name", type = "number"|"bool"|"string"|"color" } entries, and these helpers
-- turn that one list into both halves of the profile payload.

local AvoidsCommon = require("avoids_common")

local AvoidsPersist = {}

-- Data files this pack still owns, as opposed to settings. They live in the pack's own
-- resources directory rather than the shared one, so everything the pack writes sits
-- beside the code that writes it.
function AvoidsPersist.ResourceDir()
    return tostring(UiForge.scripts_path) .. "\\avoids_modern_ui\\resources"
end

function AvoidsPersist.ResourcePath(file_name)
    return AvoidsPersist.ResourceDir() .. "\\" .. file_name
end

--- Builds the profile payload for one mod from its PERSISTED_SETTINGS list.
function AvoidsPersist.Save(state, defs)
    local saved = {}
    for _, def in ipairs(defs) do
        saved[def.key] = state[def.key]
    end
    return saved
end

--- Applies a saved profile payload back onto a mod's state. A value is only copied when
--- its type matches what is already there, so a hand edited or stale profile cannot
--- corrupt the state table.
function AvoidsPersist.Load(state, defs, saved)
    if type(saved) ~= "table" then return end
    for _, def in ipairs(defs) do
        local value = saved[def.key]
        if value ~= nil and type(value) == type(state[def.key]) then
            state[def.key] = value
        end
    end
end

-- ----------------------------------------------------------------------------
-- Legacy .cfg migration, removable next release
-- ----------------------------------------------------------------------------
--
-- Earlier versions kept each mod's settings in its own auto-generated .cfg file under
-- resources. Those files are no longer written. This reads one in a single time so an
-- existing setup carries over into the profile system, then deletes it. Once shipped
-- long enough for everyone to have started once, this whole section can go.

local function ParseLegacyValue(raw_value, value_type)
    if value_type == "color" then
        local components = {}
        for field in tostring(raw_value):gmatch("[^,]+") do
            components[#components + 1] = tonumber(field)
        end
        if #components == 4 and components[1] and components[2] and components[3] and components[4] then
            return components
        end
        return nil
    elseif value_type == "bool" then
        local text = AvoidsCommon.Trim(raw_value):lower()
        if text == "true" then return true end
        if text == "false" then return false end
        return nil
    elseif value_type == "string" then
        return AvoidsCommon.Trim(raw_value)
    end
    return tonumber(raw_value)
end

--- Reads a legacy settings file into state, then removes it. Does nothing when the file
--- is already gone, which is the normal case after the first launch.
function AvoidsPersist.MigrateLegacyFile(legacy_path, state, defs)
    local file = io.open(legacy_path, "r")
    if file == nil then return false end

    local types_by_key = {}
    for _, def in ipairs(defs) do
        types_by_key[def.key] = def.type
    end

    for line in file:lines() do
        local trimmed = AvoidsCommon.Trim(line)
        if trimmed ~= "" and not trimmed:match("^#") then
            local key, raw_value = trimmed:match("^([%w_]+)=(.*)$")
            if key ~= nil and types_by_key[key] ~= nil then
                local parsed = ParseLegacyValue(raw_value, types_by_key[key])
                if parsed ~= nil and type(parsed) == type(state[key]) then
                    state[key] = parsed
                end
            end
        end
    end

    file:close()
    pcall(os.remove, legacy_path)
    return true
end

-- ----------------------------------------------------------------------------
-- Binding it all to one mod
-- ----------------------------------------------------------------------------

--- Builds a mod's whole persistence surface from its settings list. Every mod wants the
--- same three things, so they are produced together rather than written out nine times.
---
--- @param opts table
---   state       the mod's global state table
---   defs        its PERSISTED_SETTINGS list
---   legacy_path where older builds kept this mod's .cfg, read once then deleted
---   after_load  optional, runs once a profile has been applied
--- @return function Save        returns the profile payload
--- @return function Load        applies a profile payload
--- @return function LoadOnce    reads the legacy file, a no-op after the first call
function AvoidsPersist.Bind(opts)
    local state, defs = opts.state, opts.defs

    local function Save()
        return AvoidsPersist.Save(state, defs)
    end

    local function Load(saved)
        AvoidsPersist.Load(state, defs, saved)
        if opts.after_load ~= nil then opts.after_load() end
    end

    local function LoadOnce()
        if state.settings_file_loaded == true then return end
        AvoidsPersist.MigrateLegacyFile(opts.legacy_path, state, defs)
        state.settings_file_loaded = true
    end

    return Save, Load, LoadOnce
end

return AvoidsPersist
