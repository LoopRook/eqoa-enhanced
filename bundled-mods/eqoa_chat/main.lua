-- Chat: the chat box with an All tab and one tab per chat style, a channel picker and clickable item links.
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The game's own chat box is hidden while this is on. Enter (or the pad's R3) opens this mod's entry line; the
-- game's passwords, report text and the other text boxes are never taken. Item links in a line open the game's
-- own item card; the Bag Viewer's Link button puts an item into the line.
local ui = eqoa.imgui

local W, H, PAD, LINE, ROWS = 330, 150, 6, 12, 8
local MAX_LINES = 400
local CHANNELS = {
    { id = "say", label = "Say" }, { id = "group", label = "Group" }, { id = "guild", label = "Guild" },
    { id = "shout", label = "Shout" }, { id = "tell", label = "Tell" }, { id = "reply", label = "Reply" },
}
local STYLE_LABELS = {
    say = "Say", tells = "Tell", group = "Group", guild = "Guild", shouts = "Shout", combat = "Combat",
    loot = "Loot", rolls = "Rolls", experience = "XP", system = "System",
}

local state = {
    lines = {}, notices = {}, tab = "all", own_tabs = false, scroll = 0,
    session = nil, text = "", channel = "say", links = {}, last_offer = 0, note = nil,
    hits = {}, rect = nil, tokens = {}, claim = nil,
}

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function notice(text)
    state.notices[#state.notices + 1] = { text = text, kind = "notice", links = {} }
    while #state.notices > 50 do table.remove(state.notices, 1) end
end

local function add_line(m)
    state.lines[#state.lines + 1] = { text = m.text, kind = m.kind or "system", links = m.links or {}, color = m.color }
    while #state.lines > MAX_LINES do table.remove(state.lines, 1) end
end

-- The tabs: All, then a tab per chat style, or the player's own tabs (the game's Chat Tabs setting).
local function tabs()
    local out = { { id = "all", label = "All", kinds = nil } }
    if state.own_tabs then
        local setup = eqoa.chat.tabs()
        for _, t in ipairs(setup and setup.tabs or {}) do
            local set = {}
            for _, k in ipairs(t.kinds) do set[k] = true end
            out[#out + 1] = { id = "own" .. t.index, label = t.name, kinds = set, main = t.index == 0 }
        end
    else
        local setup = eqoa.chat.tabs()
        for _, k in ipairs(setup and setup.kinds or {}) do
            out[#out + 1] = { id = k, label = STYLE_LABELS[k] or k, kinds = { [k] = true } }
        end
    end
    out[#out + 1] = { id = "notices", label = "Notices", notices = true }
    return out
end

local function active_tab()
    for _, t in ipairs(tabs()) do if t.id == state.tab then return t end end
    state.tab = "all"
    return tabs()[1]
end

local function visible_lines(tab)
    if tab.notices then return state.notices end
    if not tab.kinds then return state.lines end
    local out = {}
    for _, l in ipairs(state.lines) do
        if tab.kinds[l.kind] then out[#out + 1] = l end
    end
    -- the player's Main tab also takes every kind no other tab was given
    return out
end

-- Sending: Tell takes the first word as the player's name; a line that starts with / is a command.
local function send()
    local text = state.text:gsub("^%s+", ""):gsub("%s+$", "")
    if text == "" then
        if state.session then eqoa.windows.close_session(state.session.ref) end
        return
    end
    local request, err
    if text:sub(1, 1) == "/" then
        request, err = eqoa.actions.command(text)
    else
        local opts = { links = {} }
        local route = state.channel
        if route == "tell" then
            local who, rest = text:match("^(%S+)%s+(.+)$")
            if not who then state.note = "Tell: type the player's name, a space, then the message"; return end
            opts.to, text = who, rest
        end
        -- each link is found in the text by its name, in order
        local from = 1
        for _, link in ipairs(state.links) do
            local a, b = text:find(link.name, from, true)
            if a then
                opts.links[#opts.links + 1] = { start = utf8.len(text, 1, a - 1) + 1, len = utf8.len(link.name), list = link.list, key = link.key }
                from = b + 1
            end
        end
        request, err = eqoa.actions.chat_line(route, text, opts)
    end
    if err then
        state.note = err.message or err.code
        notice("Not sent: " .. tostring(state.note))
        return
    end
    state.text, state.links, state.note = "", {}, nil
    if state.session then eqoa.windows.close_session(state.session.ref) end
end

local function insert_text(s)
    for c in s:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
        if utf8.len(state.text) >= 110 then break end
        if not c:find("[%z\1-\31\127]") then state.text = state.text .. c end
    end
end

-- Take what the game would have put in its own box: its text, its channel, the links it kept, a link offered.
local function load_session(s)
    local d = s.data or {}
    local first = state.session == nil
    state.session = s
    if first then
        state.text, state.links, state.note, state.last_offer = "", {}, nil, 0
        state.channel = d.channel or "say"
        local t = d.text or ""
        -- R and T open the game's line with "/reply " and "/tell "
        if t:match("^/[rR]e?p?l?y? ?$") or t:lower() == "/r " or t:lower() == "/reply " then state.channel, t = "reply", ""
        elseif t:lower() == "/t " or t:lower() == "/tell " then state.channel, t = "tell", ""
        end
        state.text = t
        for _, l in ipairs(d.links or {}) do state.links[#state.links + 1] = { name = l.name, list = l.list, key = l.key } end
    end
    local offer = d.offer
    if offer and offer.seq ~= state.last_offer then
        state.last_offer = offer.seq
        if #state.links < 4 then
            if state.text ~= "" and state.text:sub(-1) ~= " " then state.text = state.text .. " " end
            state.text = state.text .. offer.name
            state.links[#state.links + 1] = { name = offer.name, list = offer.list, key = offer.key }
        else
            state.note = "A line carries at most four item links"
        end
    end
end

local function on_key(d)
    if not state.session or not d.down then return end
    local k = d.key:lower()
    if k == "escape" then
        eqoa.windows.close_session(state.session.ref)
    elseif k == "enter" or k == "return" then
        send()
    elseif k == "backspace" then
        state.text = state.text:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
    elseif k == "tab" then
        for i, c in ipairs(CHANNELS) do
            if c.id == state.channel then state.channel = CHANNELS[i % #CHANNELS + 1].id; break end
        end
    elseif d.text then
        insert_text(d.text)
    end
end

local function open_link(link)
    local _, err = eqoa.actions.open_link(link.id, link.name)
    if err then notice("Could not open " .. link.name .. ": " .. tostring(err.message)) end
end

local function pointer_pressed(d)
    if d.button ~= "left" then return end
    for _, h in ipairs(state.hits) do
        if inside(d.x, d.y, h) and h.run then h.run(); return end
    end
end

local function on_wheel(d)
    if (d.steps or 0) ~= 0 and inside(d.x, d.y, state.rect) then
        state.scroll = math.max(0, state.scroll + (d.steps > 0 and 1 or -1))
    end
end

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then state.tokens[#state.tokens + 1] = token end
end

-- the history first, then every line as it comes
do
    local old = eqoa.chat.history(200)
    for _, m in ipairs(old or {}) do add_line(m) end
end
listen("chat.message", function(e) add_line(e.data) end)
listen("pointer.pressed", function(e) pointer_pressed(e.data) end)
listen("pointer.wheel", function(e) on_wheel(e.data) end)
listen("input.key", function(e) on_key(e.data) end)
listen("windows.session", function(e)
    local d = e.data
    if d.kind ~= "chat_entry" then return end
    if d.phase == "closed" then
        if state.session and d.ref == state.session.ref then
            state.session = nil
            eqoa.input.capture_text(false)
            if d.reason == "released" then notice("The chat line was taken back by the game") end
        end
    else
        local s = eqoa.windows.session("chat_entry")
        if s then load_session(s) end
    end
end)

local ok, err = eqoa.windows.takeover("chat_entry", { scope = "all" })
if not ok then
    eqoa.log.write("warn", "Chat could not take the Enter key: " .. tostring(err and err.message))
end
state.hide = eqoa.hud.hide("chat")

local function release()
    if state.session then eqoa.windows.close_session(state.session.ref); state.session = nil end
    eqoa.input.capture_text(false)
    eqoa.windows.release("chat_entry")
    if state.hide then eqoa.hud.show(state.hide); state.hide = nil end
end
eqoa.events.on("disabling", release)
eqoa.events.on("render", function()
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    if state.session then eqoa.input.capture_text(true) end
    local tab = active_tab()
    local list = visible_lines(tab)
    local entry_h = state.session and 40 or 0
    local Ht = H + entry_h
    local x0 = f.offset_x + 4 * f.scale_x
    local y0 = math.min(dh - Ht * k, f.offset_y + (448 - Ht - 24) * f.scale_y)
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    state.rect = { x = x0, y = y0, w = W * k, h = Ht * k }

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_chat_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 10 / 13)
    local dl = ui.GetWindowDrawList()
    local c = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.55, .55, .55, 1),
        surface = ui.GetColorU32(.05, .05, .05, .8), raised = ui.GetColorU32(.12, .12, .12, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), white = ui.GetColorU32(1, 1, 1, 1),
        link = ui.GetColorU32(.45, .7, 1, 1), warn = ui.GetColorU32(1, .35, .35, 1),
        on = ui.GetColorU32(.2, .2, .3, 1),
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

    fill(0, 0, W, Ht, c.surface)
    frame(0, 0, W, Ht, c.border)
    state.hits = {}
    -- the tab strip
    local tx = 2
    for _, t in ipairs(tabs()) do
        local tw = ui.CalcTextSize(t.label) / k + 8
        if tx + tw > W - 40 then break end
        fill(tx, 2, tw, 12, t.id == state.tab and c.on or c.raised)
        frame(tx, 2, tw, 12, c.border)
        text(tx + 4, 2, t.label, t.id == state.tab and c.white or c.muted)
        state.hits[#state.hits + 1] = hit(tx, 2, tw, 12, function() state.tab, state.scroll = t.id, 0 end)
        tx = tx + tw + 2
    end
    local mode = state.own_tabs and "Mine" or "Styles"
    fill(W - 38, 2, 36, 12, c.raised)
    frame(W - 38, 2, 36, 12, c.border)
    text(W - 34, 2, mode, c.muted)
    state.hits[#state.hits + 1] = hit(W - 38, 2, 36, 12, function() state.own_tabs, state.tab, state.scroll = not state.own_tabs, "all", 0 end)

    -- the lines, newest at the bottom; the wheel scrolls back
    local most = math.max(0, #list - ROWS)
    state.scroll = math.min(state.scroll, most)
    for row = 1, ROWS do
        local index = #list - state.scroll - (ROWS - row)
        local l = list[index]
        if l then
            local y = 18 + (row - 1) * LINE
            local body = l.text
            local color = l.kind == "notice" and c.warn or c.text
            if l.color and l.color.r then color = ui.GetColorU32(l.color.r, l.color.g, l.color.b, 1) end
            text(PAD, y, body, color, W - 2 * PAD)
            for _, link in ipairs(l.links or {}) do
                local from = utf8.offset(body, link.start) or link.start
                local upto = utf8.offset(body, link.start + link.len) or (#body + 1)
                local before = body:sub(1, from - 1)
                local name = body:sub(from, upto - 1)
                local lx = PAD + ui.CalcTextSize(before) / k
                local lw = ui.CalcTextSize(name) / k
                if lx + lw < W - PAD then
                    text(lx, y, name, c.link)
                    state.hits[#state.hits + 1] = hit(lx, y, lw, LINE, function() open_link(link) end)
                end
            end
        end
    end
    if tab.notices and #list == 0 then text(PAD, 18, "Nothing to report", c.muted) end

    -- the entry line while the player is typing
    if state.session then
        local y = H
        fill(0, y - 2, W, entry_h + 2, c.raised)
        local x = PAD
        for _, ch in ipairs(CHANNELS) do
            local label = ch.label
            if ch.id == "reply" then
                local to = eqoa.chat.reply_to()
                if to then label = "Reply " .. to:sub(1, 8) end
            end
            local bw = ui.CalcTextSize(label) / k + 8
            fill(x, y, bw, 12, ch.id == state.channel and c.on or c.surface)
            frame(x, y, bw, 12, c.border)
            text(x + 4, y, label, ch.id == state.channel and c.white or c.muted)
            local id = ch.id
            state.hits[#state.hits + 1] = hit(x, y, bw, 12, function()
                state.channel = id
                if id == "say" or id == "group" or id == "guild" or id == "shout" then
                    eqoa.actions.set_channel(id)
                end
            end)
            x = x + bw + 2
        end
        local prompt = state.channel == "tell" and "To (name message): " or (state.channel == "reply" and "Reply: " or "")
        text(PAD, y + 15, prompt .. state.text .. "_", c.white, W - 2 * PAD)
        local foot = state.note or "Enter sends, Esc cancels, Tab changes channel"
        text(PAD, y + 28, foot, state.note and c.warn or c.muted, W - 2 * PAD)
    end

    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = Ht * k })
    ui.SetWindowFontScale(1)
    ui.End()
end)
