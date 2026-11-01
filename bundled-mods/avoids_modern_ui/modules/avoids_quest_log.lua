--[[
avoids_quest_log -- part of the Modern UI Mods pack by Avoids.

Returns the mod table the pack's dispatcher drives,
{ display_name, Update, Settings, OnDisable, Save?, Load? }. State lives in a global
table so it survives a script reload, the same as when every mod was its own file.
]]

local Util = require("frontiers_forge.util")            -- Access Utility functions
local Chat = require("frontiers_forge.chat")            -- Cross-reference chat messages against quest names
local QuestLog = require("frontiers_forge.quest_log")   -- Access the active quest list
local AvoidsPersist = require("avoids_persist")
local AvoidsCommon = require("avoids_common")

--[[
Modern Quest Log

HONEST LIMITATION (please read before expecting more from this): the game only ever exposes
quest TITLES to the client -- QuestLog.GetCount()/GetQuestByIndex()/Quests(), and Quest:GetName()
is the only field Quest objects have. IDs, descriptions, and objective/completion state are all
server-side and only ever arrive as transient dialogue text -- they are never stored anywhere
the client (and therefore this script) can read afterward. This is not a gap in what's been
reverse-engineered yet; the data genuinely isn't kept anywhere on the client at all.

So instead of a real objective tracker (which isn't possible from what's exposed), this does
three honest things with what IS available:
  1. A clean, modern-styled list of your active quest titles (capped at 8, with a "log full"
     warning at 8/8, matching the real in-game cap).
  2. Chat cross-referencing: since we DO have full chat access, this watches incoming messages
     for any that mention a quest title currently in your log, and flags that quest with a
     "recently mentioned" badge + timestamp for a while afterward. This can't tell you WHAT
     changed (no objective data exists to read), only THAT something related showed up in chat --
     a reasonable stand-in given the constraints, not a full replacement for real objective text.
  3. Personal notes: since the game keeps none, you can type your own reminder next to each
     quest title. These are saved keyed by quest NAME (the only stable identifier available --
     there's no numeric quest ID exposed), so two simultaneously active quests that happen to
     share the exact same name would share a note. Rare, but worth knowing.
]]
modern_quest_log_state = modern_quest_log_state or {
    initialized                 = false,
    callbacks_registered         = false,

    -- Settings persistence
    settings_file_loaded        = false,

    show_quest_log               = true,
    disable_in_start_menu        = false,

    -- Appearance
    background_color             = {0.08, 0.08, 0.08, 0.85},
    show_border                  = true,
    border_color                 = {0, 0, 0, 1},
    border_thickness             = 1.5,
    corner_rounding                = 6,
    font_scale                   = 1.0,
    bold_text                    = false,

    quest_name_color              = {1, 1, 1, 1},
    log_full_warning_color        = {1, 0.4, 0.3, 1},

    -- Chat cross-referencing
    show_recent_activity_badge     = true,
    recent_activity_color          = {1.0, 0.85, 0.2, 1.0},
    recent_activity_duration_seconds = 300, -- how long the "recently mentioned" badge stays lit

    -- Notes
    show_notes                   = true,
}


--[[
Settings persistence

The options below ride in the UiForge profile. Save them with File then Save
Profile, restore them with File then Select Profile. This one list drives both
halves of that, so the payload can never drift from what the settings panel edits.
]]
local PERSISTED_SETTINGS = {
    { key = "show_quest_log",                type = "bool" },
    { key = "disable_in_start_menu",          type = "bool" },

    { key = "background_color",              type = "color" },
    { key = "show_border",                   type = "bool" },
    { key = "border_color",                  type = "color" },
    { key = "border_thickness",              type = "number" },
    { key = "corner_rounding",                 type = "number" },
    { key = "font_scale",                    type = "number" },
    { key = "bold_text",                     type = "bool" },

    { key = "quest_name_color",               type = "color" },
    { key = "log_full_warning_color",          type = "color" },

    { key = "show_recent_activity_badge",      type = "bool" },
    { key = "recent_activity_color",           type = "color" },
    { key = "recent_activity_duration_seconds", type = "number" },

    { key = "show_notes",                    type = "bool" },
}

-- Settings live in the UiForge profile now. This is the file older builds wrote, read
-- once so an existing setup carries over, then deleted.
local LEGACY_SETTINGS_FILE = tostring(UiForge.resources_path) .. "\\quest_log_settings.cfg"

-- Notes are free-form user data rather than settings, so they keep their own file
-- instead of riding in the profile. It lives in this pack's own resources directory.
local LEGACY_NOTES_FILE = tostring(UiForge.resources_path) .. "\\quest_log_notes.cfg"

local function GetQuestNotesFilePath()
    return AvoidsPersist.ResourcePath("quest_log_notes.cfg")
end

-- Save, Load, and the one time legacy read all come from the settings list above.
local Save, Load, LoadSettingsOnce = AvoidsPersist.Bind({
    state = modern_quest_log_state,
    defs = PERSISTED_SETTINGS,
    legacy_path = LEGACY_SETTINGS_FILE,
})

--[[
Notes storage: a separate file, since arbitrary quest-name keys with free-form text don't fit
the fixed-schema key=value format above. One line per note: "<quest name>|<note text>".
Pipe characters and newlines are stripped from both the name and note text when saving, since
they'd break this simple line format -- good enough for short personal reminders.
]]
quest_notes = quest_notes or {}
local quest_notes_loaded = false
local quest_notes_snapshot = ""

local function SanitizeForNotesFile(str)
    return (tostring(str):gsub("[|\r\n]", " "))
end

local function BuildNotesSnapshot()
    local lines = {}
    for name, note in pairs(quest_notes) do
        if note ~= nil and note ~= "" then
            lines[#lines + 1] = SanitizeForNotesFile(name) .. "|" .. SanitizeForNotesFile(note)
        end
    end
    table.sort(lines) -- stable ordering, so the file doesn't churn every save just from table iteration order
    return table.concat(lines, "\n")
end

local function SaveQuestNotesIfChanged()
    local snapshot = BuildNotesSnapshot()
    if snapshot ~= quest_notes_snapshot then
        local file = io.open(GetQuestNotesFilePath(), "w")
        if file ~= nil then
            file:write("# quest_log_notes.cfg -- auto-generated. One line per note: name|note text\n")
            file:write(snapshot)
            file:write("\n")
            file:close()
            quest_notes_snapshot = snapshot
        end
    end
end

local function ReadNotesFile(path)
    local file = io.open(path, "r")
    if file == nil then return false end

    for line in file:lines() do
        local trimmed = AvoidsCommon.Trim(line)
        if trimmed ~= "" and not trimmed:match("^#") then
            local name, note = trimmed:match("^(.-)|(.*)$")
            if name ~= nil then
                quest_notes[name] = note
            end
        end
    end
    file:close()
    return true
end

-- Notes moved out of the resources root into this pack's own directory. When only the
-- old file is there, it is read and rewritten at the new path, then removed, so the
-- notes move rather than being lost or duplicated.
local function LoadQuestNotesOnce()
    if quest_notes_loaded then return end
    quest_notes_loaded = true

    if ReadNotesFile(GetQuestNotesFilePath()) then
        quest_notes_snapshot = BuildNotesSnapshot()
        return
    end

    if ReadNotesFile(LEGACY_NOTES_FILE) then
        SaveQuestNotesIfChanged()
        if quest_notes_snapshot == BuildNotesSnapshot() then
            pcall(os.remove, LEGACY_NOTES_FILE)
        end
    end
end

--[[
Chat cross-referencing: tracks the last time (os.clock()) each active quest's title was seen
mentioned in a chat message, so we can show a "recently mentioned" badge for a while afterward.
]]
quest_last_mentioned = quest_last_mentioned or {}

-- Its own owner name, separate from the chat log's. Both features read chat, and a
-- shared name would mean one subscription replacing the other.
local QUEST_XREF_OWNER = "avoids_quest_xref"

local function ScanMessageForQuestNames(text)
    if text == nil or text == "" then return end

    local now = os.clock()
    local count = QuestLog.GetCount()
    for i = 0, count - 1 do
        local ok, quest = pcall(QuestLog.GetQuestByIndex, i)
        if ok and quest ~= nil then
            local ok_name, name = pcall(function() return quest:GetName() end)
            if ok_name and name ~= nil and name ~= "" then
                if text:find(name, 1, true) then -- plain substring search, no pattern-matching surprises
                    quest_last_mentioned[name] = now
                end
            end
        end
    end
end

local function HandleChatMessage(message)
    ScanMessageForQuestNames(message.text)
end

-- Re-asserted every frame, for the same reason as the chat log's subscription.
local function SubscribeToChat()
    Chat.On(QUEST_XREF_OWNER, Chat.Events.OnChatMessage, HandleChatMessage)
end

local function GetRecentActivitySecondsAgo(state, quest_name)
    local last_seen = quest_last_mentioned[quest_name]
    if last_seen == nil then return nil end
    local elapsed = os.clock() - last_seen
    if elapsed > state.recent_activity_duration_seconds then return nil end
    return elapsed
end

local function OnDisable()
    -- No native quest log toggle exists to restore (there's nothing in ui.lua for this), so
    -- just hide our own window.
    modern_quest_log_state.show_quest_log = false
end

local function Initialize()
    LoadSettingsOnce()
    LoadQuestNotesOnce()
    SubscribeToChat()
    modern_quest_log_state.initialized = true
end

local function Settings()
    local state = modern_quest_log_state

    state.show_quest_log = ImGui.Checkbox("Show Quest Log", state.show_quest_log)
    state.disable_in_start_menu = ImGui.Checkbox("Hide While Start Menu Is Open", state.disable_in_start_menu)

    ImGui.Separator()
    ImGui.Text("Appearance")
    state.background_color = ImGui.ColorEdit4("Background Color", state.background_color)
    state.show_border = ImGui.Checkbox("Show Border", state.show_border)
    state.border_color = ImGui.ColorEdit4("Border Color", state.border_color)
    state.border_thickness = ImGui.SliderFloat("Border Thickness", state.border_thickness, 0.5, 5.0, tostring(state.border_thickness))
    state.corner_rounding = ImGui.SliderFloat("Corner Rounding", state.corner_rounding, 0, 20, tostring(state.corner_rounding))
    state.font_scale = ImGui.SliderFloat("Font Scale", state.font_scale, 0.5, 3.0, tostring(state.font_scale))
    state.bold_text = ImGui.Checkbox("Bold Text", state.bold_text)
    state.quest_name_color = ImGui.ColorEdit4("Quest Name Color", state.quest_name_color)
    state.log_full_warning_color = ImGui.ColorEdit4("Log Full Warning Color", state.log_full_warning_color)

    ImGui.Separator()
    ImGui.Text("Chat Cross-Referencing")
    ImGui.TextUnformatted("Flags a quest when its title is mentioned in chat. Can't show WHAT")
    ImGui.TextUnformatted("changed (no objective data exists to read) -- just THAT something did.")
    state.show_recent_activity_badge = ImGui.Checkbox("Show Recent Activity Badge", state.show_recent_activity_badge)
    state.recent_activity_color = ImGui.ColorEdit4("Recent Activity Color", state.recent_activity_color)
    state.recent_activity_duration_seconds = ImGui.SliderInt("Badge Duration (seconds)", state.recent_activity_duration_seconds, 30, 1800, tostring(state.recent_activity_duration_seconds))

    ImGui.Separator()
    ImGui.Text("Notes")
    ImGui.TextUnformatted("The game keeps no notes of its own -- these are saved by this script")
    ImGui.TextUnformatted("only, keyed by quest name.")
    state.show_notes = ImGui.Checkbox("Show Personal Notes", state.show_notes)

end

local function Render()
    local state = modern_quest_log_state

    if Util.IsInGame() == 0 then return end
    if state.disable_in_start_menu and Util.IsStartMenuOpen() == 1 then return end

    -- Cross-reference chat every frame regardless of window visibility, so activity flags stay
    -- accurate even while the log is hidden. The subscription set up in Initialize does the
    -- matching, and a second Update in the same frame is a no-op.
    Chat.Update()

    if state.show_quest_log ~= true then return end

    if ImGui.Begin("Quest Log") then
        ImGui.SetWindowFontScale(state.font_scale)

        local count = QuestLog.GetCount()
        local header_color = (count >= 8) and state.log_full_warning_color or state.quest_name_color
        local header_color_u32 = ImGui.GetColorU32(header_color[1], header_color[2], header_color[3], header_color[4])
        local header_text = "Quests (" .. count .. "/8)" .. (count >= 8 and "  -- LOG FULL" or "")

        ImGui.PushStyleColor(ImGuiCol.Text, header_color[1], header_color[2], header_color[3], header_color[4])
        ImGui.TextUnformatted(header_text)
        ImGui.PopStyleColor()
        ImGui.Separator()

        if count == 0 then
            ImGui.TextUnformatted("(no active quests)")
        end

        for i = 0, count - 1 do
            local ok, quest = pcall(QuestLog.GetQuestByIndex, i)
            if ok and quest ~= nil then
                local ok_name, name = pcall(function() return quest:GetName() end)
                name = (ok_name and name ~= nil and name ~= "") and name or "(unnamed quest)"

                ImGui.PushStyleColor(ImGuiCol.Text, state.quest_name_color[1], state.quest_name_color[2], state.quest_name_color[3], state.quest_name_color[4])
                if state.bold_text then
                    local cursor_x, cursor_y = ImGui.GetCursorPos()
                    ImGui.SetCursorPos(cursor_x + state.font_scale, cursor_y)
                    ImGui.TextUnformatted(name)
                    ImGui.SetCursorPos(cursor_x, cursor_y)
                end
                ImGui.TextUnformatted(name)
                ImGui.PopStyleColor()

                if state.show_recent_activity_badge then
                    local seconds_ago = GetRecentActivitySecondsAgo(state, name)
                    if seconds_ago ~= nil then
                        ImGui.SameLine()
                        local badge_color = state.recent_activity_color
                        ImGui.PushStyleColor(ImGuiCol.Text, badge_color[1], badge_color[2], badge_color[3], badge_color[4])
                        ImGui.TextUnformatted(string.format("[mentioned %ds ago]", math.floor(seconds_ago)))
                        ImGui.PopStyleColor()
                    end
                end

                if state.show_notes then
                    local existing_note = quest_notes[name] or ""
                    local new_note, changed = ImGui.InputText("##note_" .. i, existing_note)
                    if changed then
                        quest_notes[name] = new_note
                        SaveQuestNotesIfChanged()
                    end
                end

                if i < count - 1 then
                    ImGui.Separator()
                end
            end
        end
    end
    ImGui.End()
end


-- Creating a local variable to interact with the global table so that the name is shorter/easier to use

local function Update()
    if modern_quest_log_state.initialized == false then Initialize() end
    SubscribeToChat()
    if modern_quest_log_state.initialized then Render() end
end

return {
    display_name = "Quest Log",
    Update = Update,
    Settings = Settings,
    OnDisable = OnDisable,
    Save = Save,
    Load = Load,
}
