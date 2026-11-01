--[[
avoids_common -- helpers shared by more than one mod in the Modern UI Mods pack.

Only genuinely repeated code belongs here. Two mods having a same named function is not
enough on its own, for example group_frames and pet_frame both draw a DrawHealthBar but
draw different things, so those stay where they are.
]]

local AvoidsCommon = {}

--- Strips leading and trailing whitespace. Nil safe, so it can be pointed straight at
--- whatever came out of a file or a text box.
--- @param str any
--- @return string
function AvoidsCommon.Trim(str)
    if str == nil then return "" end
    return (tostring(str):gsub("^%s+", ""):gsub("%s+$", ""))
end

-- ----------------------------------------------------------------------------
-- Hiding the game's own UI panels
-- ----------------------------------------------------------------------------
--
-- Every mod that replaces a native panel does the same three things. It hides or restores
-- the panel to match a setting, it retries because the first attempt silently fails when
-- the script loads before the game UI object exists, and it clears the tracking flag when
-- it hands the panel back so re-enabling the mod hides it again.
--
-- The retry is what the flag is for. UI.Disable*/Enable* return whether the write actually
-- landed, so retrying only until it does is precise, it stops the instant it succeeds
-- rather than writing forever.

-- Each mod keeps its own small toggle function, since which setting to read and which
-- UI.Disable*/Enable* pair to call is the part that genuinely differs. What is shared is
-- the bookkeeping around it, which is what these three own.

--- The name this pack hides native panels under. A panel stays hidden while any mod still
--- wants it gone, so hiding the same panel from two mods composes instead of one of them
--- winning, and this is the name the other side sees.
AvoidsCommon.UI_OWNER = "avoids_modern_ui"

--- Runs a mod's toggle now and records whether the write landed.
--- @param state table The mod's state table, which holds the tracking flag.
--- @param flag_key string Field on state tracking the confirmed write.
--- @param toggle_fn function The mod's own ToggleDefault* function.
function AvoidsCommon.ApplyNativeToggle(state, flag_key, toggle_fn)
    state[flag_key] = toggle_fn(AvoidsCommon.UI_OWNER) == true
end

--- Applies a mod's toggle until the write lands. Safe to call every frame, it does nothing
--- once confirmed, so it stops the instant it succeeds instead of writing forever.
--- @param state table The mod's state table.
--- @param flag_key string Field on state tracking the confirmed write.
--- @param toggle_fn function The mod's own ToggleDefault* function.
function AvoidsCommon.EnsureNativeToggle(state, flag_key, toggle_fn)
    if state[flag_key] == true then return end
    AvoidsCommon.ApplyNativeToggle(state, flag_key, toggle_fn)
end

--- Restores a native panel and clears its tracking flag together. Call this from OnDisable.
--- Clearing the flag is what lets the panel be hidden again if the mod comes back, and
--- keeping the two in one call is what stops them drifting apart, which is a bug this pack
--- has already had once.
--- @param state table The mod's state table.
--- @param flag_key string Field on state tracking the confirmed write.
--- @param enable_fn function The UI.Enable* call that gives the panel back.
function AvoidsCommon.ReleaseNativeToggle(state, flag_key, enable_fn)
    enable_fn(AvoidsCommon.UI_OWNER)
    state[flag_key] = false
end

return AvoidsCommon
