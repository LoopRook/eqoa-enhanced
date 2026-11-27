-- EQOA Social: Friends, Ignore, Guild, Who and Group Search in one window, plus incoming Friend Request and Guild
-- Invitation prompts (default keys: Shift+F for Friends, Shift+G for Guild).
-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- It sits over play so your character can keep walking; the game's own Start menu pages stay untouched.
-- Guild disband and guild quit keep the game's own prompt and are not offered here.
-- Kicking a guild member and accepting a friend request or guild invitation open the client's own "Are you sure?"
-- prompt, starting on No, answered only by your own click or key press.
local ui = eqoa.imgui

local WIN_W, WIN_H = 460, 360
local PROMPT_W = 340

local state = {
    open = false,
    tab = "friends",
    session = nil,
    escape = nil,
    capturing_text = false,
    status = nil,
    last_req = nil,

    -- Inputs
    friend_input = "",
    ignore_input = "",
    guild_invite_input = "",
    guild_motd_input = "",
    motd_editing = false,
    who_filter = "",
    tell_target = nil,
    tell_input = "",

    -- Row selections
    sel_friend = nil,
    sel_ignore = nil,
    sel_guild = nil,
    sel_who = nil,
    sel_lfg = nil,
    -- Group Search: what to look for (nil = the game's own choice: groups when alone, players when in a group), and
    -- how many the last answer held (nil until an answer comes)
    lfg_kind = nil,
    lfg_count = nil,

    -- Cached lists
    friends = {},
    ignored = {},
    guild = nil,
    who = nil,
    lfg = nil,
    takeover_attempted = false, prompt_warned = {}, claim_frames = 0,
}


-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end

local function say(text, ok)
    state.status = { text = tostring(text), ok = ok }
end

-- Asked every frame a box is being typed in: the client lets the hold lapse about a second after the last ask, and then
-- the keys typed would reach the game and every other mod again.
local function set_text_capture(on)
    on = on == true
    if on then
        state.capturing_text = true
        eqoa.input.capture_text(true)
    elseif state.capturing_text then
        state.capturing_text = false
        eqoa.input.capture_text(false)
    end
end

local function run_action(label, fn)
    local req, err = fn()
    if err then
        state.last_req = nil
        say(err.message or err.code or "Refused", false)
    elseif req then
        state.last_req = req
        say(label .. " sent", nil)
    end
end

local function refresh_all()
    local c = eqoa.client.get()
    if not c or not c.in_world then return end
    local f = eqoa.social.friends()
    if type(f) == "table" then state.friends = f end
    local ig = eqoa.social.ignored()
    if type(ig) == "table" then state.ignored = ig end
    local g = eqoa.guild.get()
    if type(g) == "table" then state.guild = g end
    local w = eqoa.who.get()
    if type(w) == "table" then state.who = w end
    local l = eqoa.lfg.get()
    if type(l) == "table" then state.lfg = l end
end

local function claim_prompts()
    for _, kind in ipairs({ "friend_request", "guild_invite" }) do
        local ok, err = eqoa.windows.takeover(kind, { scope = "all" })
        if not ok and err and err.code ~= "unavailable" and not state.prompt_warned[kind] then
            state.prompt_warned[kind] = true
            eqoa.log.write("warn", "Social did not take over " .. kind .. ": " .. tostring(err.message))
        end
    end
    state.takeover_attempted = true
end

local function release_prompts()
    eqoa.windows.release("friend_request")
    eqoa.windows.release("guild_invite")
    state.takeover_attempted = false
end

local function close_window(from_client)
    if not state.open then return end
    set_text_capture(false)
    if state.escape then
        eqoa.input.release(state.escape)
        state.escape = nil
    end
    if state.session and not from_client then
        eqoa.windows.close_session(state.session.ref)
    end
    state.open = false
    state.session = nil
    state.tell_target = nil
    state.tell_input = ""
    state.motd_editing = false
end

local function open_window(tab, opened_from)
    if state.open then
        if state.tab == tab then
            close_window(false)
        else
            state.tab = tab
            refresh_all()
        end
        return
    end
    local kind = (tab == "guild") and "guild" or "buddy_list"
    local s, err = eqoa.windows.open_session(kind, { opened_from = opened_from or "key" })
    if not s then
        eqoa.log.write("warn", "Social did not open: " .. tostring(err and err.message))
        return
    end
    state.session = s
    state.open = true
    state.tab = tab
    state.status = nil
    state.last_req = nil
    state.tell_target = nil
    state.tell_input = ""
    state.motd_editing = false
    state.escape = eqoa.input.claim("Escape", true)
    refresh_all()
    if tab == "who" then
        run_action("Who", function() return eqoa.who.request() end)
    end
end

local function start_tell(name)
    if not name or name == "" then return end
    state.tell_target = name
    state.tell_input = ""
end

local function send_tell()
    if not state.tell_target or state.tell_target == "" then return end
    local msg = state.tell_input:gsub("^%s+", ""):gsub("%s+$", "")
    if msg == "" then
        say("Type a message to send", false)
        return
    end
    local target = state.tell_target
    run_action("Tell to " .. target, function()
        return eqoa.actions.command("/t " .. target .. " " .. msg)
    end)
    state.tell_input = ""
    state.tell_target = nil
end

local function invite_row(id, name)
    if not name or name == "" then return end
    run_action("Group invite " .. name, function()
        if id and id >= 0 then
            return eqoa.actions.group_invite(id, name)
        end
        return eqoa.actions.group_invite(name)
    end)
end

local function short(text, max_chars)
    text = tostring(text or "")
    if #text <= max_chars then return text end
    if max_chars <= 2 then return text:sub(1, max_chars) end
    return text:sub(1, max_chars - 2) .. ".."
end

local function draw_tell_bar()
    if not state.tell_target then return false end
    local active = false
    ui.Separator()
    ui.Text("Tell " .. short(state.tell_target, 24) .. ":")
    ui.SetNextItemWidth(260)
    local val, changed = ui.InputTextWithHint("##tell_msg", "Message...", state.tell_input, 0)
    if changed then state.tell_input = val:sub(1, 100) end
    if ui.IsItemActive() then active = true end
    ui.SameLine()
    if ui.Button("Send##tell_send") then send_tell() end
    ui.SameLine()
    if ui.Button("Cancel##tell_cancel") then
        state.tell_target = nil
        state.tell_input = ""
    end
    return active
end

local function draw_friends_tab()
    local active = false
    ui.SetNextItemWidth(220)
    local val, changed = ui.InputTextWithHint("##add_friend", "Player name...", state.friend_input, 0)
    if changed then state.friend_input = val:sub(1, 24) end
    if ui.IsItemActive() then active = true end
    ui.SameLine()
    if ui.Button("Add Friend") then
        local name = state.friend_input:gsub("^%s+", ""):gsub("%s+$", "")
        if name == "" then
            say("Enter a name to add", false)
        else
            run_action("Add friend " .. name, function() return eqoa.actions.friend_add(name) end)
            state.friend_input = ""
        end
    end

    local sorted = {}
    for i = 1, #state.friends do sorted[i] = state.friends[i] end
    table.sort(sorted, function(a, b)
        if (a.online == true) ~= (b.online == true) then return a.online == true end
        return tostring(a.name or ""):lower() < tostring(b.name or ""):lower()
    end)

    local sel = nil
    local list_h = state.tell_target and 135 or 195
    if ui.BeginChild("##friends_list", 0, list_h, ImGuiChildFlags.Border) then
        if #sorted == 0 then
            ui.TextDisabled("No friends on your list.")
        else
            for _, f in ipairs(sorted) do
                local is_sel = state.sel_friend == f.id
                if is_sel then sel = f end
                local st = f.online and "On " or "Off"
                local cls = short(f.class_name or ("C" .. tostring(f.class_id or 0)), 10)
                local world = (f.world and f.world ~= "") and (" " .. short(f.world, 12)) or ""
                local line = string.format("[%s] %-16s Lv%-2d %-10s%s", st, short(f.name, 16), f.level or 0, cls, world)
                if ui.Selectable(line .. "##f_" .. tostring(f.id), is_sel) then
                    state.sel_friend = f.id
                    sel = f
                end
            end
        end
    end
    ui.EndChild()

    if sel then
        if ui.Button("Tell##f_tell") then start_tell(sel.name) end
        ui.SameLine()
        if ui.Button("Group Invite##f_inv") then invite_row(sel.id, sel.name) end
        ui.SameLine()
        if ui.Button("Remove##f_rem") then
            local id = sel.id
            local name = sel.name
            run_action("Remove " .. name, function() return eqoa.actions.friend_remove(id) end)
        end
    else
        ui.TextDisabled("Select a friend to tell, invite, or remove.")
    end

    if draw_tell_bar() then active = true end
    return active
end

local function draw_ignore_tab()
    local active = false
    ui.SetNextItemWidth(220)
    local val, changed = ui.InputTextWithHint("##add_ignore", "Player name...", state.ignore_input, 0)
    if changed then state.ignore_input = val:sub(1, 32) end
    if ui.IsItemActive() then active = true end
    ui.SameLine()
    if ui.Button("Ignore Player") then
        local name = state.ignore_input:gsub("^%s+", ""):gsub("%s+$", "")
        if name == "" then
            say("Enter a name to ignore", false)
        else
            run_action("Ignore " .. name, function() return eqoa.actions.ignore_add(name) end)
            state.ignore_input = ""
        end
    end

    local sel = nil
    if ui.BeginChild("##ignore_list", 0, 195, ImGuiChildFlags.Border) then
        if #state.ignored == 0 then
            ui.TextDisabled("Your ignore list is empty.")
        else
            for _, e in ipairs(state.ignored) do
                local is_sel = state.sel_ignore == e.id
                if is_sel then sel = e end
                if ui.Selectable(short(e.name, 32) .. "##ig_" .. tostring(e.id), is_sel) then
                    state.sel_ignore = e.id
                    sel = e
                end
            end
        end
    end
    ui.EndChild()

    if sel then
        if ui.Button("Stop Ignoring##ig_rem") then
            local id = sel.id
            local name = sel.name
            run_action("Unignore " .. name, function() return eqoa.actions.ignore_remove(id) end)
        end
    else
        ui.TextDisabled("Select an ignored player to remove.")
    end
    return active
end

local function draw_guild_tab()
    local active = false
    local g = state.guild
    if not g or not g.in_guild then
        ui.TextWrapped("You are not in a guild.")
        ui.TextDisabled("Guild invitations appear in the prompt box when received.")
        return false
    end

    ui.Text(short(g.name, 38) .. " (" .. tostring(#g.members) .. " members)")
    if state.motd_editing then
        ui.SetNextItemWidth(280)
        local val, changed = ui.InputTextWithHint("##motd_edit", "Message of the day...", state.guild_motd_input, 0)
        if changed then state.guild_motd_input = val:sub(1, 64) end
        if ui.IsItemActive() then active = true end
        ui.SameLine()
        if ui.Button("Save##motd") then
            local text = state.guild_motd_input
            run_action("Set MOTD", function() return eqoa.actions.guild_set_motd(text) end)
            state.motd_editing = false
        end
        ui.SameLine()
        if ui.Button("Cancel##motd") then state.motd_editing = false end
    else
        local motd = (g.motd and g.motd ~= "") and g.motd or "(no message of the day)"
        ui.TextWrapped("MOTD: " .. motd)
        if g.can_set_motd then
            if ui.Button("Edit MOTD") then
                state.guild_motd_input = g.motd or ""
                state.motd_editing = true
            end
        end
    end

    if g.can_invite then
        ui.SetNextItemWidth(220)
        local val, changed = ui.InputTextWithHint("##guild_inv", "Invite player...", state.guild_invite_input, 0)
        if changed then state.guild_invite_input = val:sub(1, 24) end
        if ui.IsItemActive() then active = true end
        ui.SameLine()
        if ui.Button("Guild Invite") then
            local name = state.guild_invite_input:gsub("^%s+", ""):gsub("%s+$", "")
            if name == "" then
                say("Enter a name to invite", false)
            else
                run_action("Guild invite " .. name, function() return eqoa.actions.guild_invite(name) end)
                state.guild_invite_input = ""
            end
        end
    end

    local sorted = {}
    for i = 1, #g.members do sorted[i] = g.members[i] end
    table.sort(sorted, function(a, b)
        if (a.online == true) ~= (b.online == true) then return a.online == true end
        if (a.rank or 0) ~= (b.rank or 0) then return (a.rank or 0) > (b.rank or 0) end
        return tostring(a.name or ""):lower() < tostring(b.name or ""):lower()
    end)

    local sel = nil
    local list_h = state.tell_target and 95 or 140
    if ui.BeginChild("##guild_roster", 0, list_h, ImGuiChildFlags.Border) then
        for _, m in ipairs(sorted) do
            local is_sel = state.sel_guild == m.id
            if is_sel then sel = m end
            local st = m.online and "On " or "Off"
            local cls = short(m.class_name or ("C" .. tostring(m.class_id or 0)), 10)
            local line = string.format("[%s] %-15s %-8s Lv%-2d %s", st, short(m.name, 15), short(m.rank_name, 8), m.level or 0, cls)
            if ui.Selectable(line .. "##g_" .. tostring(m.id), is_sel) then
                state.sel_guild = m.id
                sel = m
            end
        end
    end
    ui.EndChild()

    if sel then
        if ui.Button("Tell##g_tell") then start_tell(sel.name) end
        ui.SameLine()
        if ui.Button("Group Invite##g_ginv") then invite_row(sel.id, sel.name) end
        if sel.can_promote then
            ui.SameLine()
            if ui.Button("Promote##g_pro") then
                local id = sel.id
                run_action("Promote " .. sel.name, function() return eqoa.actions.guild_promote(id) end)
            end
        end
        if sel.can_demote then
            ui.SameLine()
            if ui.Button("Demote##g_dem") then
                local id = sel.id
                run_action("Demote " .. sel.name, function() return eqoa.actions.guild_demote(id) end)
            end
        end
        if sel.can_kick then
            ui.SameLine()
            if ui.Button("Kick##g_kick") then
                local id = sel.id
                run_action("Kick " .. sel.name, function() return eqoa.actions.guild_kick(id) end)
            end
        end
    else
        ui.TextWrapped("Select a member. Disband and Quit stay in the game menu.")
    end

    if draw_tell_bar() then active = true end
    return active
end

local function draw_who_tab()
    local active = false
    if ui.Button("Refresh Who") then
        run_action("Who search", function() return eqoa.who.request() end)
    end
    ui.SameLine()
    ui.SetNextItemWidth(200)
    local val, changed = ui.InputTextWithHint("##who_filter", "Filter name/class...", state.who_filter, 0)
    if changed then state.who_filter = val:sub(1, 32) end
    if ui.IsItemActive() then active = true end

    local w = state.who
    if w and w.pending then
        ui.SameLine()
        ui.TextDisabled("Searching...")
    end

    local needle = state.who_filter:lower()
    local entries = (w and w.entries) or {}
    local shown = {}
    for _, e in ipairs(entries) do
        local hay = (tostring(e.name or "") .. " " .. tostring(e.class_name or "") .. " " .. tostring(e.race_name or "")):lower()
        if needle == "" or hay:find(needle, 1, true) then
            shown[#shown + 1] = e
        end
    end

    local sel = nil
    local list_h = state.tell_target and 130 or 190
    if ui.BeginChild("##who_list", 0, list_h, ImGuiChildFlags.Border) then
        if #shown == 0 then
            ui.TextDisabled((w and w.received and w.received > 0) and "No matching players." or "Click Refresh Who to list online players.")
        else
            for _, e in ipairs(shown) do
                local is_sel = state.sel_who == e.id
                if is_sel then sel = e end
                local line = string.format("%-18s %-10s %-11s Lv%-2d", short(e.name, 18), short(e.race_name or "", 10), short(e.class_name or "", 11), e.level or 0)
                if ui.Selectable(line .. "##w_" .. tostring(e.id), is_sel) then
                    state.sel_who = e.id
                    sel = e
                end
            end
        end
    end
    ui.EndChild()

    if sel then
        if ui.Button("Tell##w_tell") then start_tell(sel.name) end
        ui.SameLine()
        if ui.Button("Group Invite##w_inv") then invite_row(sel.id, sel.name) end
    else
        ui.TextDisabled("Select a player to tell or invite.")
    end

    if draw_tell_bar() then active = true end
    return active
end

-- What the Group Search list says when it has no rows: that a search is out, that it came back empty (so the player
-- knows the answer was heard), or that none was made yet. A global so a test can read it.
function social_lfg_empty_note(busy, count, kind)
    if busy then return "Searching..." end
    if count == 0 then
        if kind == "players" then return "Nobody found. No player is looking for a group right now." end
        if kind == "groups" then return "Nobody found. No group is looking for members right now." end
        return "Nobody found. No one is looking right now (try Find: Players or Find: Groups)."
    end
    return "No group search results yet. Click Search LFG."
end

-- what the tab has heard of the last search, for a test
function social_probe() return state.lfg_count, state.lfg_kind end

local KIND_LABEL = { [false] = "Find: Auto", groups = "Find: Groups", players = "Find: Players" }
local NEXT_KIND = { [false] = "groups", groups = "players", players = false }

local function draw_lfg_tab()
    local active = false
    local l = state.lfg
    if not l or not l.available then
        ui.TextWrapped("Group search is not offered on this server.")
        ui.TextDisabled("Use the Who tab to find players online.")
        return false
    end

    if ui.Button("Search LFG") then
        state.lfg_count = nil
        run_action("Group search", function() return eqoa.lfg.search(state.lfg_kind and { kind = state.lfg_kind } or nil) end)
    end
    ui.SameLine()
    if ui.Button("Register##lfg") then
        run_action("Register LFG", function() return eqoa.lfg.register() end)
    end
    ui.SameLine()
    if ui.Button("Unregister##lfg") then
        run_action("Unregister LFG", function() return eqoa.lfg.unregister() end)
    end
    if l.registered ~= nil then
        ui.SameLine()
        ui.TextDisabled(l.registered and "Registered" or "Not registered")
    end
    if ui.Button(KIND_LABEL[state.lfg_kind or false] .. "##lfg_kind") then
        state.lfg_kind = NEXT_KIND[state.lfg_kind or false] or nil
        state.lfg_count = nil
    end
    if l.busy then
        ui.SameLine()
        ui.TextDisabled("Searching...")
    end

    ui.TextDisabled(string.format("Default level range: %d - %d", l.default_min_level or 1, l.default_max_level or 60))

    local entries = l.entries or {}
    local sel = nil
    local list_h = state.tell_target and 115 or 170
    if ui.BeginChild("##lfg_list", 0, list_h, ImGuiChildFlags.Border) then
        if #entries == 0 then
            ui.TextWrapped(social_lfg_empty_note(l.busy, state.lfg_count, state.lfg_kind))
        else
            for _, e in ipairs(entries) do
                local is_sel = state.sel_lfg == e.id
                if is_sel then sel = e end
                local line = string.format("%-18s %-10s %-11s Lv%-2d", short(e.name, 18), short(e.race_name or "", 10), short(e.class_name or "", 11), e.level or 0)
                if ui.Selectable(line .. "##l_" .. tostring(e.id), is_sel) then
                    state.sel_lfg = e.id
                    sel = e
                end
            end
        end
    end
    ui.EndChild()

    if sel then
        if ui.Button("Tell##l_tell") then start_tell(sel.name) end
        ui.SameLine()
        if ui.Button("Group Invite##l_inv") then invite_row(sel.id, sel.name) end
    else
        ui.TextDisabled("Select a player to tell or invite.")
    end

    if draw_tell_bar() then active = true end
    return active
end

local function draw_prompts()
    local c = eqoa.client.get()
    if not c or not c.in_world then return end
    local dw, dh = ui.GetDisplaySize()
    if dw <= 0 then return end

    local fr = eqoa.social.friend_request()
    local fr_sess = eqoa.windows.session("friend_request")
    if fr and fr_sess then
        ui.SetNextWindowPos((dw - PROMPT_W) * 0.5, 64, ImGuiCond.FirstUseEver)
        ui.SetNextWindowSize(PROMPT_W, 0, ImGuiCond.Always)
        if ui.Begin("Friend Request##eqoa_social_fr", nil, ImGuiWindowFlags.NoCollapse + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.AlwaysAutoResize) then
            ui.TextWrapped(short(fr.name, 24) .. " wants to add you as a friend.")
            ui.TextDisabled(string.format("Auto-refuses in %d s", math.ceil(fr.seconds_remaining or 0)))
            if ui.Button("Accept##fr_acc") then
                run_action("Accept friend", function() return eqoa.actions.friend_accept() end)
            end
            ui.SameLine()
            if ui.Button("Refuse##fr_ref") then
                run_action("Refuse friend", function() return eqoa.actions.friend_refuse() end)
            end
            ui.SameLine()
            if ui.Button("Show in game##fr_pass") then
                eqoa.windows.pass(fr_sess.ref)
            end
        end
        ui.End()
    end

    local gi = eqoa.guild.invite()
    local gi_sess = eqoa.windows.session("guild_invite")
    if gi and gi_sess then
        ui.SetNextWindowPos((dw - PROMPT_W) * 0.5, 176, ImGuiCond.FirstUseEver)
        ui.SetNextWindowSize(PROMPT_W, 0, ImGuiCond.Always)
        if ui.Begin("Guild Invitation##eqoa_social_gi", nil, ImGuiWindowFlags.NoCollapse + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.AlwaysAutoResize) then
            ui.TextWrapped(short(gi.inviter, 24) .. " invited you to join " .. short(gi.guild, 40) .. ".")
            ui.TextDisabled(string.format("Auto-refuses in %d s", math.ceil(gi.seconds_remaining or 0)))
            if ui.Button("Accept##gi_acc") then
                run_action("Accept guild invite", function() return eqoa.actions.guild_accept() end)
            end
            ui.SameLine()
            if ui.Button("Refuse##gi_ref") then
                run_action("Refuse guild invite", function() return eqoa.actions.guild_refuse() end)
            end
            ui.SameLine()
            if ui.Button("Show in game##gi_pass") then
                eqoa.windows.pass(gi_sess.ref)
            end
        end
        ui.End()
    end
end

local function draw()
    local c = eqoa.client.get()
    if not c or not c.in_world then
        set_text_capture(false)
        return
    end
    -- under the game's map: nothing drawn, the keyboard is the game's; the window comes back as it was
    if map_up() then
        set_text_capture(false)
        return
    end
    -- held again every so often: the claims end with the world and with the player's "Use the game's windows" switch
    state.claim_frames = state.claim_frames + 1
    if not state.takeover_attempted or state.claim_frames % 120 == 0 then
        claim_prompts()
    end

    draw_prompts()

    if not state.open then
        set_text_capture(false)
        return
    end

    local dw, dh = ui.GetDisplaySize()
    if dw <= 0 then return end
    ui.SetNextWindowPos(math.max(0, (dw - WIN_W) * 0.5), math.max(0, (dh - WIN_H) * 0.5), ImGuiCond.FirstUseEver)
    ui.SetNextWindowSize(WIN_W, WIN_H, ImGuiCond.FirstUseEver)

    local keep_open, shown = ui.Begin("Social##eqoa_social", true, ImGuiWindowFlags.NoCollapse)
    if not keep_open then
        ui.End()
        close_window(false)
        return
    end

    local text_active = false
    if shown then
        local function tab_btn(id, label)
            local active = state.tab == id
            if ui.Button((active and ("[" .. label .. "]") or label) .. "##tab_" .. id) then
                state.tab = id
                state.tell_target = nil
                refresh_all()
                if id == "who" and (not state.who or (state.who.received or 0) == 0) then
                    run_action("Who search", function() return eqoa.who.request() end)
                end
            end
        end
        tab_btn("friends", "Friends")
        ui.SameLine()
        tab_btn("ignore", "Ignore")
        ui.SameLine()
        tab_btn("guild", "Guild")
        ui.SameLine()
        tab_btn("who", "Who")
        ui.SameLine()
        tab_btn("lfg", "Group Search")
        ui.SameLine()
        ui.TextDisabled("Esc")
        ui.Separator()

        if state.tab == "friends" then
            text_active = draw_friends_tab()
        elseif state.tab == "ignore" then
            text_active = draw_ignore_tab()
        elseif state.tab == "guild" then
            text_active = draw_guild_tab()
        elseif state.tab == "who" then
            text_active = draw_who_tab()
        elseif state.tab == "lfg" then
            text_active = draw_lfg_tab()
        end

        if state.status then
            ui.Separator()
            ui.TextWrapped(state.status.text)
        end
    end
    ui.End()
    set_text_capture(text_active)
end

-- Controls-page bindings
eqoa.input.bind({ id = "friends", label = "Social (Friends)", default_key = "shift+f" }, function()
    open_window("friends", "key")
end)
eqoa.input.bind({ id = "guild", label = "Social (Guild)", default_key = "shift+g" }, function()
    open_window("guild", "key")
end)

-- Another mod (the Start menu) asks for a page: friends, ignore, guild, who or lfg
eqoa.events.on("mod.command", function(e)
    if e.data.command ~= "open" then return end
    local page = type(e.data.args) == "table" and e.data.args.page or "friends"
    if page ~= "friends" and page ~= "ignore" and page ~= "guild" and page ~= "who" and page ~= "lfg" then page = "friends" end
    if state.open and state.tab == page then return end
    open_window(page, "mod")
end)

-- Event wiring
eqoa.events.on("render", draw)
eqoa.events.on("world.entered", function()
    claim_prompts()
    refresh_all()
end)
eqoa.events.on("state.reset", function()
    refresh_all()
end)
eqoa.events.on("friends.changed", function(e)
    if e.data and type(e.data.after) == "table" then state.friends = e.data.after end
end)
eqoa.events.on("ignored.changed", function(e)
    if e.data and type(e.data.after) == "table" then state.ignored = e.data.after end
end)
eqoa.events.on("guild.changed", function(e)
    if e.data and type(e.data.after) == "table" then state.guild = e.data.after end
end)
eqoa.events.on("who.result", function()
    local w = eqoa.who.get()
    if type(w) == "table" then state.who = w end
end)
eqoa.events.on("lfg.result", function(e)
    local l = eqoa.lfg.get()
    if type(l) == "table" then state.lfg = l end
    -- the answer was heard, even an empty one
    state.lfg_count = (e.data and e.data.count) or (l and l.entries and #l.entries) or 0
end)
eqoa.events.on("lfg.changed", function(e)
    if e.data and type(e.data.after) == "table" then state.lfg = e.data.after end
end)
eqoa.events.on("action.result", function(e)
    local d = e.data
    if not d or state.last_req == nil or d.request_id ~= state.last_req then return end
    if d.ok == true then
        if d.detail and d.detail.asked then
            say("Answer the confirmation box on screen", nil)
        else
            say("Sent", true)
        end
    elseif d.ok == false then
        say(d.reason or d.code or "Refused", false)
    end
end)
eqoa.events.on("confirm.closed", function(e)
    local d = e.data
    if not d then return end
    if d.kind == "guild_kick" or d.kind == "friend_accept" or d.kind == "guild_accept" then
        if d.answer == "yes" then
            local sent = d.detail and d.detail.sent
            say(sent and "Done" or ("Not sent: " .. tostring(d.detail and d.detail.reason)), sent == true)
        elseif d.answer == "no" then
            say("Cancelled", true)
        else
            say("Cancelled (" .. tostring(d.answer) .. ")", true)
        end
    end
end)
eqoa.events.on("windows.session", function(e)
    local d = e.data
    if not d then return end
    if d.phase == "closed" and state.session and d.ref == state.session.ref then
        close_window(true)
    elseif d.phase == "closed" and (d.kind == "friend_request" or d.kind == "guild_invite") then
        if d.reason == "timeout" then
            say("Request timed out after 45 seconds and was refused", nil)
        end
    end
end)
eqoa.events.on("input.key", function(e)
    if state.open and not map_up() and e.data.down and not e.data["repeat"] and e.data.key:lower() == "escape" then
        -- Escape while typing (a tell, a name, the guild message) leaves the text box first (ImGui lets go of the field
        -- on the same press); the window and what it was showing stay. The next Escape closes the window.
        if state.capturing_text then
            set_text_capture(false)
            return
        end
        close_window(false)
    end
end)

-- what a test reads (the window's state)
social_keys_probe = { state = state }
eqoa.events.on("world.leaving", function()
    close_window(true)
    state.takeover_attempted = false
end)
eqoa.events.on("disabling", function()
    close_window(false)
    release_prompts()
end)
