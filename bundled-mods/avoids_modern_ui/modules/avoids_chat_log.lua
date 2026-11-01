--[[
avoids_chat_log -- part of the Modern UI Mods pack by Avoids.

Returns the mod table the pack's dispatcher drives,
{ display_name, Update, Settings, OnDisable, Save?, Load? }. State lives in a global
table so it survives a script reload, the same as when every mod was its own file.
]]

local UI = require("frontiers_forge.ui")                -- Access UI elements
local Util = require("frontiers_forge.util")            -- Access Utility functions
local Chat = require("frontiers_forge.chat")            -- Access chat messages
local Input = require("frontiers_forge.input")           -- Intercept Enter + suppress it -- for typing directly into this window (v0.1.1 pattern)
local AvoidsPersist = require("avoids_persist")
local AvoidsCommon = require("avoids_common")

--[[
Modern Chat Log

Chat publishes every message to its subscribers, so this window subscribes once in
Initialize and builds its own scrollback history in memory, capped at max_messages
entries. Chat.Update() drives the drain and is safe to call from more than one place,
which matters here because the quest log subscribes to chat as well.

This window has a normal title bar (movable/resizable) and, since it doesn't have a close (X)
button, "Show Chat Log" in the settings tab is the on/off toggle -- defaulting to visible,
since a chat log is normally something you want up at all times.

Channel visibility/colors are controlled per-channel (Say/Shout/Party/Tell/Guild/Unknown).
Quick toggles live directly in the chat window itself; colors and other appearance options
live in the settings tab.
]]
modern_chat_log_state = modern_chat_log_state or {
    initialized                 = false,
    callbacks_registered         = false,
    native_toggle_confirmed      = false, -- v0.1.1: tracks whether the native chat window disable/enable write has actually landed yet

    -- Custom chat input: mirrors mmvest's own ff_example.lua demo pattern exactly (Enter
    -- suppressed via Input's key hook, typed text sent through Chat.SendChatText so it goes
    -- through the game's own send path, slash commands and all). Off by default -- this
    -- takes over your Enter key, so it's an explicit opt-in, not an automatic behavior change.
    enable_custom_chat_input       = false,
    chat_input_mode_index           = 0, -- index into CHAT_INPUT_MODES below

    -- Settings persistence (message HISTORY itself is intentionally not persisted -- a fresh
    -- log each session is expected, same as the game's own chat window)
    settings_file_loaded        = false,

    messages                    = {},   -- accumulated {text, type_string, timestamp} entries
    max_messages                = 200,

    show_chat_log                = true,
    disable_in_start_menu        = false,
    -- Default true: hides the native chat window (avoiding duplicate chat displays), since the
    -- underlying rendering glitch this used to cause is fixed as of the corrected ui.lua that
    -- properly skips BeginDraw/EndDraw together via a branch instruction, instead of NOP'ing
    -- BeginDraw alone. If you're on an older/unpatched ui.lua, set this to false instead -- see
    -- the note in the settings panel below.
    disable_default_chat_window  = true,

    -- Display options
    show_timestamps              = true,
    word_wrap                    = true,
    auto_scroll                  = true,   -- step 1: always snaps to newest message (not yet "smart" about manual scroll-up -- see Render())
    font_scale                   = 1.0,
    bold_text                    = false,

    -- Panel appearance
    background_color             = {0.05, 0.05, 0.05, 0.75},
    show_border                  = true,
    window_transparent           = false,   -- removes the window's own default opaque fill (title bar stays visible/movable)
    border_color                 = {0, 0, 0, 1},
    border_thickness             = 1.5,
    corner_rounding               = 4,

    -- Per-channel visibility
    show_say                    = true,
    show_shout                  = true,
    show_party                  = true,
    show_tell                   = true,
    show_guild                  = true,
    show_unknown                 = true,

    -- Per-channel colors
    say_color                   = {1.0, 1.0, 1.0, 1.0},     -- white
    shout_color                  = {1.0, 0.35, 0.2, 1.0},   -- orange-red
    party_color                  = {0.4, 0.7, 1.0, 1.0},    -- blue
    tell_color                   = {0.9, 0.4, 0.9, 1.0},    -- pink/purple
    guild_color                  = {0.4, 1.0, 0.5, 1.0},    -- green
    unknown_color                 = {0.7, 0.7, 0.7, 1.0},   -- gray
}


--[[
Settings persistence

The options below ride in the UiForge profile. Save them with File then Save
Profile, restore them with File then Select Profile. This one list drives both
halves of that, so the payload can never drift from what the settings panel edits.
]]
local PERSISTED_SETTINGS = {
    { key = "max_messages",                 type = "number" },
    { key = "show_chat_log",                type = "bool" },
    { key = "disable_in_start_menu",         type = "bool" },
    { key = "disable_default_chat_window",  type = "bool" },
    { key = "enable_custom_chat_input",       type = "bool" },
    { key = "chat_input_mode_index",           type = "number" },

    { key = "show_timestamps",              type = "bool" },
    { key = "word_wrap",                    type = "bool" },
    { key = "auto_scroll",                  type = "bool" },
    { key = "font_scale",                   type = "number" },
    { key = "bold_text",                    type = "bool" },

    { key = "background_color",             type = "color" },
    { key = "show_border",                  type = "bool" },
    { key = "window_transparent",           type = "bool" },
    { key = "border_color",                 type = "color" },
    { key = "border_thickness",             type = "number" },
    { key = "corner_rounding",               type = "number" },

    { key = "show_say",                     type = "bool" },
    { key = "show_shout",                   type = "bool" },
    { key = "show_party",                   type = "bool" },
    { key = "show_tell",                    type = "bool" },
    { key = "show_guild",                   type = "bool" },
    { key = "show_unknown",                  type = "bool" },

    { key = "say_color",                    type = "color" },
    { key = "shout_color",                   type = "color" },
    { key = "party_color",                   type = "color" },
    { key = "tell_color",                    type = "color" },
    { key = "guild_color",                   type = "color" },
    { key = "unknown_color",                  type = "color" },
}

-- Settings live in the UiForge profile now. This is the file older builds wrote, read
-- once so an existing setup carries over, then deleted.
local LEGACY_SETTINGS_FILE = tostring(UiForge.resources_path) .. "\\chat_log_settings.cfg"

-- Save, Load, and the one time legacy read all come from the settings list above.
local Save, Load, LoadSettingsOnce = AvoidsPersist.Bind({
    state = modern_chat_log_state,
    defs = PERSISTED_SETTINGS,
    legacy_path = LEGACY_SETTINGS_FILE,
    -- Applying a profile can change a native hide setting, so clear the confirmed
    -- flags and let the per frame retry reapply them.
    after_load = function()
        modern_chat_log_state.native_toggle_confirmed = false
    end,
})

-- Real wall-clock time if the OS clock is reachable from this Lua environment; otherwise falls
-- back to a session-relative "minutes:seconds since script start" using ImGui's own clock.
local function GetTimestampString()
    local ok_date, ts = pcall(os.date, "%H:%M:%S")
    if ok_date and ts ~= nil then return ts end

    local ok_time, elapsed = pcall(ImGui.GetTime)
    if ok_time and type(elapsed) == "number" then
        local total_seconds = math.floor(elapsed)
        local minutes = math.floor(total_seconds / 60)
        local seconds = total_seconds % 60
        return string.format("%d:%02d", minutes, seconds)
    end

    return "??:??"
end

local function ChannelVisible(state, type_string)
    if type_string == "Say" then return state.show_say end
    if type_string == "Shout" then return state.show_shout end
    if type_string == "Party" then return state.show_party end
    if type_string == "Tell" then return state.show_tell end
    if type_string == "Guild" then return state.show_guild end
    return state.show_unknown
end

local function GetColorForType(state, type_string)
    if type_string == "Say" then return state.say_color end
    if type_string == "Shout" then return state.shout_color end
    if type_string == "Party" then return state.party_color end
    if type_string == "Tell" then return state.tell_color end
    if type_string == "Guild" then return state.guild_color end
    return state.unknown_color
end

local function ToggleDefaultChatWindow(owner)
    if modern_chat_log_state.disable_default_chat_window == true then
        return UI.DisableChatWindow(owner)
    end
    return UI.EnableChatWindow(owner)
end

--[[
Custom chat input -- mirrors mmvest's own ff_example.lua demo pattern exactly (his own
stated motivation for building the input-hook system in the first place: typing into a
mod's own window instead of the game's native chat popup). This is copied as closely as
possible to his tested version rather than reimplemented independently, since it involves
keyboard hooks and a send hook -- not the place to improvise.

Two hooks make this work:
  * Input.InstallKeyHook() lets us observe keys and suppress Enter specifically, so the
    game's own typing window never opens.
  * Chat.InstallSendHook() lets Chat.SendChatText hand the message to the game's own chat
    sender on the next frame, so the outbound packet is exactly what typing in the native
    chat window would produce (slash commands and all).
]]
custom_chat_input_state = custom_chat_input_state or {
    capturing = false,          -- key hook installed and Enter owned by us
    box_active = false,         -- our ImGui chat box is open
    box_focused = false,        -- the box holds keyboard focus, so the game gets no keys
    focus_pending = false,      -- give the box keyboard focus on the next drawn frame
    text = "",                  -- message being composed
    last_sent = nil,
    last_error = nil,
    next_retry_time = 0,        -- os.clock() value; don't retry a failed install before this
}

-- The name other mods see if they ask for Enter while this feature holds it, so it has to
-- read as the mod the user would go and turn off rather than as this one file.
local KEY_OWNER = "avoids_modern_ui"

-- Selectable chat modes, same as the demo. Typing an explicit /command in the message
-- overrides this, exactly like typing into the native chat box would.
local CHAT_INPUT_MODES = {
    { name = "Default", prefix = Chat.ChatMode.Default },
    { name = "Say",     prefix = Chat.ChatMode.Say },
    { name = "Group",   prefix = Chat.ChatMode.Group },
    { name = "Guild",   prefix = Chat.ChatMode.Guild },
    { name = "Shout",   prefix = Chat.ChatMode.Shout },
}

local function SendCustomChatText()
    local text = custom_chat_input_state.text
    if text == "" then return end

    local mode = CHAT_INPUT_MODES[modern_chat_log_state.chat_input_mode_index + 1].prefix
    local ok, err = Chat.SendChatText(text, mode)
    if ok then
        custom_chat_input_state.last_sent = text
        custom_chat_input_state.last_error = nil
        custom_chat_input_state.text = ""
    else
        custom_chat_input_state.last_error = err
    end
end

-- Forward-declared: UpdateCustomChatCapture (below) needs to call StartCustomChatCapture
-- (defined further below) to retry a failed hook install -- this keeps it a proper local
-- instead of accidentally leaking a global.
local StartCustomChatCapture

-- Runs every frame while capturing so the key ring buffer never overruns, even when the
-- chat log window itself is hidden. The only key acted on is Enter: it opens the box and
-- moves keyboard focus to it -- everything else is ImGui's own text-editing job.
local function UpdateCustomChatCapture()
    if not custom_chat_input_state.capturing then
        -- Found by comparing against another mod's implementation of this same pattern:
        -- our first attempt at installing the key hook could fail silently (e.g. the game
        -- isn't fully ready yet) with no retry at all, leaving the setting showing "enabled"
        -- while nothing was actually capturing. Retry on a slow clock (not every frame,
        -- since a failing install shouldn't be hammered constantly) whenever the setting
        -- wants capturing on but it hasn't actually started yet.
        if modern_chat_log_state.enable_custom_chat_input and os.clock() >= custom_chat_input_state.next_retry_time then
            StartCustomChatCapture()
        end
        return
    end
    if not Input.IsKeyHookInstalled() then
        return
    end

    -- Polling under our own name, so draining here cannot swallow the events another
    -- mod is waiting on.
    for _, event in ipairs(Input.PollKeyEvents(KEY_OWNER)) do
        if event.is_down and event.key == Input.Key.Enter and not custom_chat_input_state.box_active then
            custom_chat_input_state.box_active = true
            custom_chat_input_state.focus_pending = true
        end
    end

    -- While the box holds focus the whole keyboard is taken from the game, so typing a
    -- message cannot fire abilities or open the game's own windows behind this one.
    Input.SetTextCaptureActive(KEY_OWNER, custom_chat_input_state.box_focused)
    -- Consumed rather than left standing, so hiding the window mid message cannot hold the
    -- keyboard away from the game. The next draw sets it again.
    custom_chat_input_state.box_focused = false
end

function StartCustomChatCapture()
    -- Enter is claimed, not merely suppressed, so two mods can never both answer it. A
    -- refusal names whoever holds it, which is what the settings panel shows the user.
    local ok, err = Input.AcquireKey(KEY_OWNER, Input.Key.Enter)
    if not ok then
        custom_chat_input_state.last_error = err
        custom_chat_input_state.next_retry_time = os.clock() + 2
        return
    end
    ok, err = Chat.InstallSendHook()
    if not ok then
        Input.ReleaseKeyHook(KEY_OWNER)
        custom_chat_input_state.last_error = err
        custom_chat_input_state.next_retry_time = os.clock() + 2
        return
    end

    Input.SetKeySuppressed(KEY_OWNER, Input.Key.Enter, true)
    custom_chat_input_state.capturing = true
    custom_chat_input_state.last_error = nil
end

local function StopCustomChatCapture()
    -- Give the keyboard back, dropping only our own claim. The hook itself comes down
    -- once the last mod using it lets go, so this cannot disarm another mod's chat input.
    Input.SetTextCaptureActive(KEY_OWNER, false)
    Input.ReleaseKeyHook(KEY_OWNER)
    Chat.UninstallSendHook()
    custom_chat_input_state.capturing = false
    custom_chat_input_state.box_active = false
    custom_chat_input_state.box_focused = false
    custom_chat_input_state.focus_pending = false
end

-- Draws the input box (and mode selector) at the bottom of the chat window, only while a
-- message is actively being composed.
local function DrawCustomChatInput()
    if not modern_chat_log_state.enable_custom_chat_input then return end

    ImGui.Separator()

    for i, m in ipairs(CHAT_INPUT_MODES) do
        if i > 1 then ImGui.SameLine() end
        modern_chat_log_state.chat_input_mode_index = ImGui.RadioButton(m.name, modern_chat_log_state.chat_input_mode_index, i - 1)
    end

    if not custom_chat_input_state.box_active then
        custom_chat_input_state.box_focused = false
        ImGui.TextUnformatted("Press Enter to type a message.")
    else
        if custom_chat_input_state.focus_pending then
            ImGui.SetKeyboardFocusHere()
            custom_chat_input_state.focus_pending = false
        end
        local entered
        custom_chat_input_state.text, entered = ImGui.InputText("##custom_chat_message",
            custom_chat_input_state.text, ImGuiInputTextFlags.EnterReturnsTrue)
        custom_chat_input_state.box_focused = ImGui.IsItemActive()
        if entered then
            SendCustomChatText()
            custom_chat_input_state.box_active = false
        end
        ImGui.SameLine()
        if ImGui.Button("Cancel") then
            custom_chat_input_state.text = ""
            custom_chat_input_state.box_active = false
        end
    end

    if custom_chat_input_state.last_error ~= nil then
        ImGui.TextUnformatted("error: " .. tostring(custom_chat_input_state.last_error))
    end
end

-- Chat publishes to subscribers now, so history accumulates on its own rather than
-- being pulled in from Render. The owner name is per feature, not per mod, since two
-- subscriptions sharing one name would replace each other.
local CHAT_LOG_OWNER = "avoids_chat_log"

local function HandleChatMessage(message)
    local state = modern_chat_log_state
    table.insert(state.messages, {
        text = message.text,
        type_string = message.type_name,
        timestamp = GetTimestampString(),
    })
    while #state.messages > state.max_messages do
        table.remove(state.messages, 1)
    end
end

-- Re-asserted every frame rather than once at init. Disabling the script drops the
-- subscription, but the state table holding "initialized" outlives that, so a one time
-- subscribe would never come back on re-enable. Chat.On replaces by owner, so repeating
-- it costs a table assignment and cannot stack duplicates.
local function SubscribeToChat()
    Chat.On(CHAT_LOG_OWNER, Chat.Events.OnChatMessage, HandleChatMessage)
end

local function Initialize()
    LoadSettingsOnce()
    SubscribeToChat()
    modern_chat_log_state.initialized = true
end

local function Settings()
    local state = modern_chat_log_state

    state.show_chat_log = ImGui.Checkbox("Show Chat Log", state.show_chat_log)
    state.disable_in_start_menu = ImGui.Checkbox("Hide While Start Menu Is Open", state.disable_in_start_menu)

    ImGui.Separator()
    ImGui.Text("History")
    state.max_messages = ImGui.SliderInt("Max Stored Messages", state.max_messages, 20, 500, tostring(state.max_messages))

    ImGui.Separator()
    ImGui.Text("Display")
    state.show_timestamps = ImGui.Checkbox("Show Timestamps", state.show_timestamps)
    state.word_wrap = ImGui.Checkbox("Word Wrap", state.word_wrap)
    state.auto_scroll = ImGui.Checkbox("Auto-Scroll To Newest", state.auto_scroll)
    state.font_scale = ImGui.SliderFloat("Font Scale", state.font_scale, 0.5, 3.0, tostring(state.font_scale))
    state.bold_text = ImGui.Checkbox("Bold Text", state.bold_text)

    ImGui.Separator()
    ImGui.Text("Panel Appearance")
    state.background_color = ImGui.ColorEdit4("Background Color", state.background_color)
    state.show_border = ImGui.Checkbox("Show Border", state.show_border)
    state.window_transparent = ImGui.Checkbox("Window Transparent", state.window_transparent)
    state.border_color = ImGui.ColorEdit4("Border Color", state.border_color)
    state.border_thickness = ImGui.SliderFloat("Border Thickness", state.border_thickness, 0.5, 6.0, tostring(state.border_thickness))
    state.corner_rounding = ImGui.SliderFloat("Corner Rounding", state.corner_rounding, 0, 20, tostring(state.corner_rounding))

    ImGui.Separator()
    ImGui.Text("Channels")
    state.show_say = ImGui.Checkbox("Show Say", state.show_say)
    state.say_color = ImGui.ColorEdit4("Say Color", state.say_color)
    state.show_shout = ImGui.Checkbox("Show Shout", state.show_shout)
    state.shout_color = ImGui.ColorEdit4("Shout Color", state.shout_color)
    state.show_party = ImGui.Checkbox("Show Party", state.show_party)
    state.party_color = ImGui.ColorEdit4("Party Color", state.party_color)
    state.show_tell = ImGui.Checkbox("Show Tell", state.show_tell)
    state.tell_color = ImGui.ColorEdit4("Tell Color", state.tell_color)
    state.show_guild = ImGui.Checkbox("Show Guild", state.show_guild)
    state.guild_color = ImGui.ColorEdit4("Guild Color", state.guild_color)
    state.show_unknown = ImGui.Checkbox("Show Unknown", state.show_unknown)
    state.unknown_color = ImGui.ColorEdit4("Unknown Color", state.unknown_color)

    ImGui.Separator()
    ImGui.TextUnformatted("NOTE: This requires the corrected ui.lua (fixed BeginDraw/EndDraw")
    ImGui.TextUnformatted("handling for the chat window). If shout messages render as a")
    ImGui.TextUnformatted("floating panel near your target, uncheck this and update ui.lua.")
    local new_val, pressed = ImGui.Checkbox("Disable Default Chat Window", state.disable_default_chat_window)
    if pressed then
        state.disable_default_chat_window = new_val
        AvoidsCommon.ApplyNativeToggle(state, "native_toggle_confirmed", ToggleDefaultChatWindow)
    end

    ImGui.Separator()
    ImGui.Text("Custom Chat Input")
    ImGui.TextUnformatted("Type directly into this window instead of opening the game's own")
    ImGui.TextUnformatted("chat popup -- press Enter in-game to compose, Enter again to send.")
    ImGui.TextUnformatted("This takes over your Enter key while enabled.")
    local input_new_val, input_pressed = ImGui.Checkbox("Enable Custom Chat Input", state.enable_custom_chat_input)
    if input_pressed then
        state.enable_custom_chat_input = input_new_val
        if input_new_val then
            StartCustomChatCapture()
        else
            StopCustomChatCapture()
        end
    end
    if state.enable_custom_chat_input then
        ImGui.TextUnformatted("Capturing: " .. tostring(custom_chat_input_state.capturing))
        ImGui.TextUnformatted("Enter key owner: " ..
            (Input.GetKeyOwner(Input.Key.Enter) or "nobody"))
        if custom_chat_input_state.last_error ~= nil then
            ImGui.TextUnformatted("error: " .. tostring(custom_chat_input_state.last_error))
        end
    end

end

local function OnDisable()
    -- v0.06: fires when this script is unchecked in the UiForge menu. Without this, disabling
    -- the script would leave the default chat window hidden with no way to bring it back short
    -- of restarting the game -- so always restore it here, regardless of the
    -- "Disable Default Chat Window" setting.
    -- Restoring the panel and clearing the flag go together, so re-enabling the mod
    -- hides it again instead of coming back with the setting on but nothing applied.
    AvoidsCommon.ReleaseNativeToggle(modern_chat_log_state, "native_toggle_confirmed", UI.EnableChatWindow)

    -- Critical: if custom chat input is active, this releases the keyboard hook and gives
    -- Enter back to the game. Without this, disabling the mod would leave Enter permanently
    -- suppressed for the rest of the session.
    if custom_chat_input_state.capturing then
        StopCustomChatCapture()
    end
end

local function Render()
    local state = modern_chat_log_state

    if Util.IsInGame() == 0 then return end
    if state.disable_in_start_menu and Util.IsStartMenuOpen() == 1 then return end

    -- Drains chat every frame regardless of window visibility, so history keeps
    -- accumulating even while the log is hidden. The subscription set up in Initialize
    -- is what actually appends, and a second Update in the same frame is a no-op.
    Chat.Update()

    if state.show_chat_log ~= true then return end

    -- Removes the window's own default opaque fill when "Window Transparent" is on. Title bar
    -- stays visible/movable either way since NoTitleBar isn't part of this flag.
    local chat_window_flags = 0
    if state.window_transparent then chat_window_flags = ImGuiWindowFlags.NoBackground end

    if ImGui.Begin("Chat Log", true, chat_window_flags) then
        ImGui.SetWindowFontScale(state.font_scale)

        -- Quick per-channel toggles, right in the window for fast access
        state.show_say = ImGui.Checkbox("Say", state.show_say)
        ImGui.SameLine()
        state.show_shout = ImGui.Checkbox("Shout", state.show_shout)
        ImGui.SameLine()
        state.show_party = ImGui.Checkbox("Party", state.show_party)
        ImGui.SameLine()
        state.show_tell = ImGui.Checkbox("Tell", state.show_tell)
        ImGui.SameLine()
        state.show_guild = ImGui.Checkbox("Guild", state.show_guild)

        ImGui.Separator()

        -- Messages are drawn directly in the main window (no BeginChild/EndChild -- an earlier
        -- version used a scrollable child region here, but it triggered a native ImGui
        -- assertion crash on first use, so this sticks to the same plain Begin/End pattern
        -- already proven stable in every other script). The window scrolls on its own once
        -- content overflows it -- resize the window if you want to see more lines at once.
        --
        -- Background is drawn per-line (a band behind each message, sized to that line's own
        -- height) rather than as one panel measured upfront. An earlier version measured the
        -- background once at the top of the window, before any messages were laid out -- once
        -- the chat log grew taller than that one-time measurement, everything past it had no
        -- background at all, causing a visible "solid background fades to fully transparent"
        -- seam partway down the window. Drawing it fresh behind each line avoids that entirely,
        -- since it always matches however much content actually exists.
        local avail_width = ImGui.GetContentRegionAvail()
        local draw_list = ImGui.GetWindowDrawList()

        for _, msg in ipairs(state.messages) do
            if ChannelVisible(state, msg.type_string) then
                local color = GetColorForType(state, msg.type_string)

                local line = ""
                if state.show_timestamps then
                    line = "[" .. msg.timestamp .. "] "
                end
                line = line .. "[" .. msg.type_string .. "] " .. msg.text

                if state.show_border or state.background_color[4] > 0 then
                    local line_origin_x, line_origin_y = ImGui.GetCursorScreenPos()
                    -- NOTE: deliberately NOT passing a wrap_width here. An earlier attempt at
                    -- that used ImGui.CalcTextSize(line, nil, false, wrap_width) to also account
                    -- for wrapped multi-line height, but passing nil for the "ending" parameter
                    -- triggered a hard crash (a Begin/End window-stack assertion, meaning the
                    -- call errored out mid-frame before reaching ImGui.End()) -- the same failure
                    -- class as the earlier BeginChild/Begin(nil, ...) issues elsewhere in this
                    -- project. This single-argument form is the same safe pattern used
                    -- everywhere else. Tradeoff: wrapped (multi-line) messages may get a
                    -- background band that's a bit short -- cosmetic only, not a crash risk.
                    local _, line_text_height = ImGui.CalcTextSize(line)
                    local bg_color_u32 = ImGui.GetColorU32(state.background_color[1], state.background_color[2], state.background_color[3], state.background_color[4])
                    draw_list:AddRectFilled(ImVec2.new(line_origin_x, line_origin_y), ImVec2.new(line_origin_x + avail_width, line_origin_y + line_text_height), bg_color_u32, state.corner_rounding)
                    if state.show_border then
                        local border_color_u32 = ImGui.GetColorU32(state.border_color[1], state.border_color[2], state.border_color[3], state.border_color[4])
                        draw_list:AddRect(ImVec2.new(line_origin_x, line_origin_y), ImVec2.new(line_origin_x + avail_width, line_origin_y + line_text_height), border_color_u32, state.corner_rounding, 0, state.border_thickness)
                    end
                end

                ImGui.PushStyleColor(ImGuiCol.Text, color[1], color[2], color[3], color[4])
                if state.word_wrap then ImGui.PushTextWrapPos(0.0) end

                if state.bold_text then
                    -- No bold font is available to load from Lua, so this fakes it by
                    -- drawing the line a second time, one pixel over, to thicken it.
                    local cursor_x, cursor_y = ImGui.GetCursorPos()
                    ImGui.SetCursorPos(cursor_x + state.font_scale, cursor_y)
                    ImGui.TextUnformatted(line)
                    ImGui.SetCursorPos(cursor_x, cursor_y)
                end
                ImGui.TextUnformatted(line)

                if state.word_wrap then ImGui.PopTextWrapPos() end
                ImGui.PopStyleColor()
            end
        end

        -- Step 1 of auto-scroll: always snap to the newest message every frame, using only
        -- SetScrollHereY (deliberately not yet combined with GetScrollY/GetScrollMaxY, so this
        -- introduces exactly one new, previously-untested function at a time after the earlier
        -- BeginChild-related crash). This always follows the bottom -- it doesn't yet respect
        -- you having manually scrolled up to read history. Once confirmed stable, step 2 will
        -- add that "only follow if you were already at the bottom" behavior back in.
        if state.auto_scroll then
            ImGui.SetScrollHereY(1.0)
        end

        DrawCustomChatInput()
    end
    ImGui.End()
end

local function Update()
    if modern_chat_log_state.initialized == false then Initialize() end
    SubscribeToChat()

    AvoidsCommon.EnsureNativeToggle(modern_chat_log_state, "native_toggle_confirmed", ToggleDefaultChatWindow)

    -- Runs every frame regardless of window visibility, same as the official demo, so the
    -- key ring buffer never overruns even while the chat log window itself is hidden.
    UpdateCustomChatCapture()

    if modern_chat_log_state.initialized then Render() end
end

return {
    display_name = "Chat Log",
    Update = Update,
    Settings = Settings,
    OnDisable = OnDisable,
    Save = Save,
    Load = Load,
}
