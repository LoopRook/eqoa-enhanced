-- Start Menu: the game's Start menu in a window of this mod's own, on the Start key and the controller's Start.
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The entries are the game's own for this server; clicking one does what the game's menu does. Items opens the
-- game's own Start menu (its item page has no copy here), so the bag windows stay the game's or Bag Viewer's.
local ui = eqoa.imgui

local W, PAD, ROW, ROWS = 220, 8, 15, 12

-- Entries that an EQOA mod does the job of. Choosing one asks that mod to open its window (`eqoa.mods.send`,
-- the command "open"); with the mod off, missing or refusing, the game's own entry runs as before. An entry is
-- known by the game's command (its message, the part of "0x3020000:0" before the colon), not by its label: labels are
-- the server's to word, and the same word is on different entries ("Guild" is a sub-menu of Community and a chat
-- mode). Only entries that run a command are routed, never a sub-menu, so every sub-menu stays open to browse. The
-- game's labels on the disc's tables, for the record: Community (Who, Guild, Group, Buddy List, Chat Modes, Message,
-- Mail, Trade, Ignore, Show Shouts, Hide Shouts), Character (Summary, Tradeskills, Class Mastery and Factions with
-- the expansion), Inventory, Ability, Pet, Quest, Support, Options, Quit. Ability is the game's own page: no mod.
local ROUTES = {
    ["0x3020000"] = { mod = "eqoa_character" },                   -- Character > Summary
    ["0x2080000"] = { mod = "eqoa_character" },                   -- Character > Class Mastery
    ["0xaa000001"] = { mod = "eqoa_character" },                  -- Character > Factions
    ["0x7e20001"] = { mod = "eqoa_character" },                   -- Character > Combat Record (servers that offer it)
    ["0xb0a0000"] = { mod = "eqoa_quest_journal" },               -- Quest
    ["0xa7010000"] = { mod = "eqoa_crafting" },                   -- Tradeskills > Recipe Book > a skill's book
    ["0x2000000"] = { mod = "eqoa_bag_viewer" },                  -- Inventory
    ["0xb030004"] = { mod = "eqoa_social", page = "who" },        -- Community > Who
    ["0xb050000"] = { mod = "eqoa_social", page = "friends" },    -- Community > Buddy List > View List
    ["0xb053000"] = { mod = "eqoa_social", page = "ignore" },     -- Community > Ignore > View List
    ["0xb051001"] = { mod = "eqoa_social", page = "guild" },      -- Community > Guild > Guild List
    ["0x2090000"] = { mod = "eqoa_social", page = "lfg" },        -- Community > Group > Group Search (live servers)
}
local state = {
    session = nil, path = {}, selected = 1, scroll = 0, tokens = {}, escape = nil, enter = nil,
    hits = {}, rect = nil, note = nil,
}

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then state.tokens[#state.tokens + 1] = token end
end

-- The entries shown now: the session's list, down the sub-menus chosen so far.
local function entries()
    local list = state.session and state.session.data and state.session.data.entries or {}
    for _, index in ipairs(state.path) do
        local e = list[index]
        list = e and e.entries or {}
    end
    return list
end

local function title()
    local list = state.session and state.session.data and state.session.data.entries or {}
    local name = "Start Menu"
    for _, index in ipairs(state.path) do
        local e = list[index]
        if not e then break end
        name, list = e.text, e.entries or {}
    end
    return name
end

local finish

local function close()
    if not state.session then return end
    eqoa.windows.close_session(state.session.ref)
    finish()
end

-- Asks the EQOA mod that does this entry's job to open its window. True when it took the request (the menu is
-- then closed so the mod's window can open); false leaves the game's entry to run.
local function open_in_mod(e)
    local msg = e.command and e.command:match("^(0x%x+):")
    local r = msg and ROUTES[msg]
    if not r or not state.session then return false end
    local ok = eqoa.mods.send(r.mod, "open", r.page and { page = r.page } or nil)
    if not ok then return false end
    close()
    return true
end

-- The game's own Support entries (Request Help, Help Status, Send Report) are the player's to press in the game's own
-- menu: the client never runs a report route for a mod, so this menu would do nothing. They hand the player over to the
-- game's own Start menu the way Items does (the one entry the client marks `native`), and the session ends.
local function is_report_route(e)
    return e.command ~= nil and e.command:match("^0xb0b1%x%x%x:") ~= nil
end

local function native_command(list)
    for _, e in ipairs(list or {}) do
        if e.native and e.command then return e.command end
        local found = e.entries and native_command(e.entries)
        if found then return found end
    end
    return nil
end

local function choose(index)
    local e = entries()[index]
    if not e or not e.enabled then return end
    if open_in_mod(e) then return end
    if e.entries then
        state.path[#state.path + 1] = index
        state.selected, state.scroll = 1, 0
    elseif is_report_route(e) and state.session then
        local command = native_command(state.session.data and state.session.data.entries)
        if not command then
            state.note = "Support is in the game's own Start menu"
            return
        end
        local _, err = eqoa.actions.menu_command(state.session.ref, command)
        if err then state.note = err.message or err.code end
    elseif e.command and state.session then
        -- the client ends this session when the game's entry runs
        local _, err = eqoa.actions.menu_command(state.session.ref, e.command)
        if err then state.note = err.message or err.code end
    end
end

local function back()
    if #state.path > 0 then
        table.remove(state.path)
        state.selected, state.scroll = 1, 0
    else
        close()
    end
end

local function step(d)
    local n = #entries()
    if n == 0 then return end
    state.selected = ((state.selected - 1 + d) % n) + 1
    if state.selected <= state.scroll then state.scroll = state.selected - 1 end
    if state.selected > state.scroll + ROWS then state.scroll = state.selected - ROWS end
end

local function on_pad(d)
    if not d.down then return end
    if d.key == "up" then step(-1)
    elseif d.key == "down" then step(1)
    elseif d.key == "confirm" or d.key == "right" then choose(state.selected)
    elseif d.key == "back" or d.key == "left" then back() end
end

local draw

local function show(session)
    local fresh = state.session == nil
    state.session = session
    if not fresh then return end
    state.path, state.selected, state.scroll, state.note = {}, 1, 0, nil
    listen("render", draw)
    -- the pad: up, down, confirm and back are this menu's while it is up (the Start button itself stays the game's)
    eqoa.windows.focus(session.ref, { movement = false })
    listen("windows.key", function(e)
        if state.session and e.data.ref == state.session.ref then on_pad(e.data) end
    end)
    listen("input.key", function(e)
        local d = e.data
        if not d.down or d["repeat"] then return end
        local key = d.key:lower()
        if key == "escape" then back()
        elseif key == "up" then step(-1)
        elseif key == "down" then step(1)
        elseif key == "return" or key == "enter" then choose(state.selected) end
    end)
    listen("pointer.pressed", function(e)
        local d = e.data
        if d.button ~= "left" then return end
        for _, h in ipairs(state.hits) do
            if inside(d.x, d.y, h) then h.run(); return end
        end
    end)
    listen("pointer.wheel", function(e)
        local d = e.data
        if (d.steps or 0) ~= 0 and inside(d.x, d.y, state.rect) then
            local most = math.max(0, #entries() - ROWS)
            state.scroll = math.max(0, math.min(most, state.scroll + (d.steps > 0 and -1 or 1)))
        end
    end)
    state.escape = eqoa.input.claim("Escape", true)
    -- Enter chooses here. Left to the game it would also open the game's chat line in the same press, and the chat
    -- mod takes that line: its box then held the keyboard under the page the entry opened (owner test 2026-11-24:
    -- Ability, then nothing answered).
    state.enter = eqoa.input.claim("Enter", true)
end

function finish()
    for _, token in ipairs(state.tokens) do eqoa.events.off(token) end
    state.tokens = {}
    if state.escape then eqoa.input.release(state.escape); state.escape = nil end
    if state.enter then eqoa.input.release(state.enter); state.enter = nil end
    state.session, state.hits, state.rect, state.path = nil, {}, nil, {}
end

local ok, err = eqoa.windows.takeover("start_menu", { scope = "all" })
if not ok then
    eqoa.log.write("warn", "Start Menu could not take the Start key: " .. tostring(err and err.message))
end

eqoa.events.on("windows.session", function(e)
    local d = e.data
    if d.kind ~= "start_menu" then return end
    if d.phase == "closed" then
        if state.session and d.ref == state.session.ref then finish() end
    elseif d.phase == "opened" or d.phase == "changed" then
        local s = eqoa.windows.session("start_menu")
        if s then show(s) end
    end
end)
eqoa.events.on("disabling", function() finish(); eqoa.windows.release("start_menu") end)
eqoa.events.on("world.leaving", function() finish() end)

function draw()
    if not state.session then return end
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    local list = entries()
    local shown = math.min(ROWS, math.max(1, #list))
    local H = 24 + shown * ROW + 22
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    state.rect = { x = x0, y = y0, w = W * k, h = H * k }

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_start_menu_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 11 / 13)
    local dl = ui.GetWindowDrawList()
    local c = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.55, .55, .55, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), white = ui.GetColorU32(1, 1, 1, 1),
        hl = ui.GetColorU32(.20, .20, .26, 1), warn = ui.GetColorU32(1, .3, .3, 1),
    }
    local function fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    local function frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    local function text(x, y, value, color, maxw)
        value = tostring(value)
        while maxw and #value > 0 and ui.CalcTextSize(value) > maxw * k do
            value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
        end
        dl:AddText(at(x, y), color or c.text, value)
    end
    local function hit(x, y, w, h, run) return { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k, run = run } end

    fill(0, 0, W, H, c.surface)
    frame(0, 0, W, H, c.border)
    text(PAD, 6, title(), c.white, W - 2 * PAD - 18)
    state.hits = {}
    fill(W - 18, 4, 12, 12, c.raised)
    frame(W - 18, 4, 12, 12, c.border)
    text(W - 15, 4, "x", c.text)
    state.hits[1] = hit(W - 18, 4, 12, 12, close)
    if #list == 0 then text(PAD, 24, "Nothing here", c.muted) end
    for i = 1, shown do
        local index = state.scroll + i
        local e = list[index]
        if e then
            local y = 24 + (i - 1) * ROW
            if index == state.selected then fill(PAD - 2, y, W - 2 * PAD + 4, ROW - 1, c.hl) end
            local label = e.text ~= "" and e.text or "(no name)"
            if e.entries then label = label .. "  >" end
            text(PAD, y + 1, label, e.enabled and c.text or c.muted, W - 2 * PAD)
            state.hits[#state.hits + 1] = hit(PAD - 2, y, W - 2 * PAD + 4, ROW - 1, function()
                state.selected = index
                choose(index)
            end)
        end
    end
    local foot = 24 + shown * ROW + 4
    if state.note then
        text(PAD, foot, state.note, c.warn, W - 2 * PAD)
    else
        text(PAD, foot, "Esc: " .. (#state.path > 0 and "up one menu" or "close"), c.muted, W - 2 * PAD)
    end

    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    ui.SetWindowFontScale(1)
    ui.End()
end
