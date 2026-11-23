-- Chat: the chat box with an All tab and one tab per chat style, a channel picker and clickable item links.
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The game's own chat box is hidden while this is on. Enter (or the pad's R3) opens this mod's entry line; the
-- game's passwords, report text and the other text boxes are never taken. Item links in a line open the game's
-- own item card; the Bag Viewer's Link button puts an item into the line.
local ui = eqoa.imgui

local PAD, LINE = 6, 12
-- The window's size and place are the player's: drag the strip beside the tabs to move it, the corner to size it. They
-- are kept in this mod's settings, in HUD units (the 640 by 448 picture).
local HEAD = 10 -- the strip at the top that moves the window
local DEF_W, DEF_H, MIN_W, MAX_W, MIN_H, MAX_H = 330, HEAD + 22 + 8 * 12, 200, 640, HEAD + 22 + 2 * 12, 300
local MAX_LINES = 400
local CHANNELS = {
    { id = "say", label = "Say" }, { id = "group", label = "Group" }, { id = "guild", label = "Guild" },
    { id = "shout", label = "Shout" }, { id = "tell", label = "Tell" }, { id = "reply", label = "Reply" },
}
local STYLE_LABELS = {
    say = "Say", tells = "Tell", group = "Group", guild = "Guild", shouts = "Shout", combat = "Combat",
    loot = "Loot", rolls = "Rolls", experience = "XP", system = "System",
}

-- the sandbox has no utf8 library: characters are counted by their lead bytes
local function chars(s) local _, n = s:gsub("[^\128-\191]", ""); return n end
-- the byte where character `n` (1-based) of `s` starts; #s + 1 past the end
local function byte_at(s, n)
    local seen = 0
    for i = 1, #s do
        local b = s:byte(i)
        if b < 128 or b >= 192 then
            seen = seen + 1
            if seen == n then return i end
        end
    end
    return #s + 1
end

local state = {
    lines = {}, notices = {}, tab = "all", own_tabs = false, scroll = 0,
    session = nil, text = "", channel = "say", links = {}, last_offer = 0, note = nil,
    hits = {}, rect = nil, tokens = {}, claim = nil,
    w = DEF_W, h = DEF_H, lx = 4, ly = nil, drag = nil,
    -- the tabs the player edits (nil until the styles are known, or after Reset tabs), the next id, the open menu
    -- (a tab's, or the strip's), its rows' hit areas, a tab being renamed, and what the text was cut by
    custom = nil, next_tab = 1, menu = nil, menu_hits = {}, rename = nil, cuts = 0, menu_cuts = 0, capturing = false,
}

local function clamp(v, lo, hi) return math.max(lo, math.min(hi, v)) end

-- The chat styles a tab can show, in the order its menu lists them.
local STYLE_ORDER = { "say", "group", "guild", "shouts", "tells", "combat", "loot", "rolls", "experience", "system" }
local NAME_MOST = 12

-- What is kept: the window's place and size, and the tabs the player made (nil = the default set).
local function snapshot()
    local out = { x = state.lx, y = state.ly, w = state.w, h = state.h, next_tab = state.next_tab }
    if state.custom then
        out.tabs = {}
        for i, t in ipairs(state.custom) do
            local kinds = {}
            for _, k in ipairs(STYLE_ORDER) do if t.kinds and t.kinds[k] then kinds[#kinds + 1] = k end end
            out.tabs[i] = { id = t.id, name = t.name, all = t.all == true, kinds = kinds }
        end
    end
    return out
end

local function persist() eqoa.settings.save(snapshot()) end

eqoa.settings.on_save(snapshot)
eqoa.settings.on_load(function(saved)
    if type(saved) ~= "table" then return end
    if type(saved.x) == "number" then state.lx = saved.x end
    if type(saved.y) == "number" then state.ly = saved.y end
    if type(saved.w) == "number" then state.w = clamp(saved.w, MIN_W, MAX_W) end
    if type(saved.h) == "number" then state.h = clamp(saved.h, MIN_H, MAX_H) end
    if type(saved.next_tab) == "number" then state.next_tab = saved.next_tab end
    if type(saved.tabs) == "table" then
        local list, has_all = {}, false
        for _, t in ipairs(saved.tabs) do
            if type(t) == "table" and type(t.id) == "string" and type(t.name) == "string" then
                local entry = { id = t.id, name = t.name:sub(1, NAME_MOST), all = t.all == true }
                if entry.all then has_all = true else
                    entry.kinds = {}
                    for _, k in ipairs(type(t.kinds) == "table" and t.kinds or {}) do entry.kinds[k] = true end
                end
                list[#list + 1] = entry
            end
        end
        -- the All tab is always there
        if has_all then state.custom = list end
    end
end)

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function notice(text)
    state.notices[#state.notices + 1] = { text = text, kind = "notice", links = {} }
    while #state.notices > 50 do table.remove(state.notices, 1) end
end

local function add_line(m)
    state.lines[#state.lines + 1] = { text = m.text, kind = m.kind or "system", links = m.links or {}, color = m.color }
    while #state.lines > MAX_LINES do table.remove(state.lines, 1) end
end

-- The default set: All, then a tab per chat style. Empty of styles while the client has not said which exist.
local function default_tabs()
    local out = { { id = "all", name = "All", all = true } }
    local setup = eqoa.chat.tabs()
    for _, k in ipairs(setup and setup.kinds or {}) do
        out[#out + 1] = { id = k, name = STYLE_LABELS[k] or k, kinds = { [k] = true } }
    end
    return out
end

-- The tabs as edited: kept in `state.custom` once the styles are known.
local function edited_tabs()
    if not state.custom then
        local d = default_tabs()
        if #d > 1 then state.custom = d end
        return d
    end
    return state.custom
end

-- The tabs: All, then a tab per chat style (as the player edited them), or the player's own tabs (the game's Chat
-- Tabs setting, not editable here).
local function tabs()
    local out = {}
    if state.own_tabs then
        out[1] = { id = "all", label = "All", kinds = nil }
        local setup = eqoa.chat.tabs()
        for _, t in ipairs(setup and setup.tabs or {}) do
            local set = {}
            for _, k in ipairs(t.kinds) do set[k] = true end
            out[#out + 1] = { id = "own" .. t.index, label = t.name, kinds = set, main = t.index == 0 }
        end
    else
        for _, t in ipairs(edited_tabs()) do
            out[#out + 1] = { id = t.id, label = t.name, kinds = (not t.all) and t.kinds or nil, entry = t }
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

-- ---- editing the tabs -------------------------------------------------------------------------------------------

local function index_of(t)
    for i, e in ipairs(edited_tabs()) do if e == t then return i end end
end

local function close_menu() state.menu = nil; state.menu_hits = {} end

local function move_tab(t, delta)
    local list = edited_tabs()
    local i = index_of(t)
    local j = i and i + delta
    if not i or j < 1 or j > #list then return end
    list[i], list[j] = list[j], list[i]
    persist()
end

local function delete_tab(t)
    if t.all then return end
    local list = edited_tabs()
    local i = index_of(t)
    if not i then return end
    table.remove(list, i)
    if state.tab == t.id then state.tab, state.scroll = "all", 0 end
    persist()
end

local function new_tab()
    local list = edited_tabs()
    local id
    repeat
        id = "t" .. state.next_tab
        state.next_tab = state.next_tab + 1
        local taken = false
        for _, e in ipairs(list) do if e.id == id then taken = true end end
    until not taken
    -- starts with nothing ticked
    list[#list + 1] = { id = id, name = "New", kinds = {} }
    state.tab, state.scroll = id, 0
    state.custom = list
    persist()
end

local function reset_tabs()
    state.custom = nil
    state.tab, state.scroll = "all", 0
    persist()
end

local function toggle_kind(t, kind)
    if t.all then return end
    t.kinds = t.kinds or {}
    t.kinds[kind] = (not t.kinds[kind]) or nil
    persist()
end

local function begin_rename(t)
    state.rename = { t = t, text = t.name }
end

local function finish_rename(apply)
    local r = state.rename
    state.rename = nil
    if apply and r and r.text:match("%S") then
        r.t.name = r.text:gsub("^%s+", ""):gsub("%s+$", "")
        persist()
    end
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
                opts.links[#opts.links + 1] = { start = chars(text:sub(1, a - 1)) + 1, len = chars(link.name), list = link.list, key = link.key }
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
        if chars(state.text) >= 110 then break end
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
    if not d.down then return end
    local k = d.key:lower()
    -- a tab being renamed has the keys: Enter keeps the name, Escape drops it
    if state.rename then
        if k == "escape" then finish_rename(false)
        elseif k == "enter" or k == "return" then finish_rename(true)
        elseif k == "backspace" then
            state.rename.text = state.rename.text:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
        elseif d.text then
            for c in d.text:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
                if chars(state.rename.text) >= NAME_MOST then break end
                if not c:find("[%z\1-\31\127]") then state.rename.text = state.rename.text .. c end
            end
        end
        return
    end
    if state.menu and k == "escape" then close_menu(); return end
    if not state.session then return end
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
    -- an open menu has the press: a row runs, anything else closes it
    if state.menu then
        local row
        for _, h in ipairs(state.menu_hits) do if inside(d.x, d.y, h) then row = h; break end end
        if row and d.button == "left" then
            if row.run then row.run() end
            return
        end
        close_menu()
        if d.button == "left" or row then return end
    end
    if state.rename then return end
    if d.button == "right" then
        for _, h in ipairs(state.hits) do
            if inside(d.x, d.y, h) and h.ctx then h.ctx(d); return end
        end
        return
    end
    if d.button ~= "left" then return end
    for _, h in ipairs(state.hits) do
        if inside(d.x, d.y, h) then
            if h.drag then
                state.drag = { kind = h.drag, px = d.x, py = d.y, lx = state.lx, ly = state.eff_ly or state.ly or 0,
                    w = state.w, h = state.h, age = 0 }
                return
            end
            if h.run then h.run(); return end
        end
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
    close_menu()
    state.rename = nil
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
    local want = state.session ~= nil or state.rename ~= nil
    if want ~= state.capturing then
        state.capturing = want
        eqoa.input.capture_text(want)
    end
    local tab = active_tab()
    local list = visible_lines(tab)
    local entry_h = state.session and 40 or 0
    local W, H, ROWS, Ht, x0, y0
    local function layout()
        W, H = state.w, state.h
        ROWS = math.max(2, math.floor((H - HEAD - 22) / LINE))
        Ht = H + entry_h
        local ly = state.ly or (448 - Ht - 24)
        x0 = clamp(f.offset_x + state.lx * f.scale_x, 0, math.max(0, dw - W * k))
        y0 = clamp(f.offset_y + ly * f.scale_y, 0, math.max(0, dh - Ht * k))
        state.eff_ly = ly
        state.rect = { x = x0, y = y0, w = W * k, h = Ht * k }
    end
    layout()
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_chat_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 10 / 13)
    -- follow a press that took the move strip or the corner to its release (ImGui's own mouse state)
    local d = state.drag
    if d then
        d.age = d.age + 1
        if ui.IsMouseDown(0) then
            d.seen = true
            local p = eqoa.input.pointer()
            local dx, dy = p.x - d.px, p.y - d.py
            if d.kind == "move" then
                state.lx = d.lx + dx / f.scale_x
                state.ly = d.ly + dy / f.scale_y
            else
                state.w = clamp(d.w + dx / k, MIN_W, MAX_W)
                state.h = clamp(d.h + dy / k, MIN_H, MAX_H)
            end
            layout()
        elseif d.seen or d.age > 5 then
            -- let go (or ImGui never saw the press): the place is kept
            state.drag = nil
            eqoa.settings.save({ x = state.lx, y = state.ly, w = state.w, h = state.h })
        end
    end
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
            state.cuts = state.cuts + 1
        end
        dl:AddText(at(x, y), color or c.text, value)
    end
    local function hit(x, y, w, h, run) return { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k, run = run } end

    fill(0, 0, W, Ht, c.surface)
    frame(0, 0, W, Ht, c.border)
    state.hits = {}
    -- the tab strip
    local function tag(h, id, ctx) h.id, h.ctx = id, ctx; return h end
    local tx = 2
    local limit = state.own_tabs and W - 40 or W - 54
    if state.rename then
        -- a tab being renamed: its name is typed here (Enter keeps it, Esc drops it)
        fill(2, HEAD + 2, W - 44, 12, c.raised)
        frame(2, HEAD + 2, W - 44, 12, c.white)
        text(6, HEAD + 2, "Rename: " .. state.rename.text .. "_   (Enter, Esc)", c.white, W - 52)
    else
        for _, t in ipairs(tabs()) do
            local tw = ui.CalcTextSize(t.label) / k + 8
            if tx + tw > limit then break end
            fill(tx, HEAD + 2, tw, 12, t.id == state.tab and c.on or c.raised)
            frame(tx, HEAD + 2, tw, 12, c.border)
            text(tx + 4, HEAD + 2, t.label, t.id == state.tab and c.white or c.muted)
            -- a click chooses the tab; a right click (the styles' tabs) opens its menu
            local entry = t.entry
            state.hits[#state.hits + 1] = tag(hit(tx, HEAD + 2, tw, 12, function() state.tab, state.scroll = t.id, 0 end),
                "tab:" .. t.id, entry and function(d) state.menu = { x = d.x, y = d.y, entry = entry } end or nil)
            tx = tx + tw + 2
        end
        if not state.own_tabs then
            -- the small + (always there, beside Styles) makes a new tab
            fill(W - 52, HEAD + 2, 12, 12, c.raised)
            frame(W - 52, HEAD + 2, 12, 12, c.border)
            text(W - 49, HEAD + 2, "+", c.muted)
            state.hits[#state.hits + 1] = tag(hit(W - 52, HEAD + 2, 12, 12, new_tab), "tab:new")
        end
        if not state.own_tabs and tx < limit then
            -- the empty strip beside the tabs: a right click there offers New tab and Reset tabs
            state.hits[#state.hits + 1] = tag(hit(tx, HEAD + 2, limit - tx, 12, function() end), "strip",
                function(d) state.menu = { x = d.x, y = d.y } end)
        end
    end
    local mode = state.own_tabs and "Mine" or "Styles"
    fill(W - 38, HEAD + 2, 36, 12, c.raised)
    frame(W - 38, HEAD + 2, 36, 12, c.border)
    text(W - 34, HEAD + 2, mode, c.muted)
    state.hits[#state.hits + 1] = tag(hit(W - 38, HEAD + 2, 36, 12, function()
        close_menu()
        state.own_tabs, state.tab, state.scroll = not state.own_tabs, "all", 0
    end), "mode")

    -- the strip beside the tabs moves the window; the corner sizes it
    state.hits[#state.hits + 1] = { x = x0, y = y0, w = W * k, h = HEAD * k, drag = "move" }
    text(W / 2 - 20, 0, "drag to move", c.muted, 60)
    state.hits[#state.hits + 1] = { x = x0 + (W - 12) * k, y = y0 + (H - 12) * k, w = 12 * k, h = 12 * k, drag = "size" }
    fill(W - 8, H - 3, 6, 1, c.muted)
    fill(W - 6, H - 6, 4, 1, c.muted)
    fill(W - 4, H - 9, 2, 1, c.muted)

    -- the lines, newest at the bottom; the wheel scrolls back
    local most = math.max(0, #list - ROWS)
    state.scroll = math.min(state.scroll, most)
    for row = 1, ROWS do
        local index = #list - state.scroll - (ROWS - row)
        local l = list[index]
        if l then
            local y = HEAD + 18 + (row - 1) * LINE
            local body = l.text
            local color = l.kind == "notice" and c.warn or c.text
            if l.color and l.color.r then color = ui.GetColorU32(l.color.r, l.color.g, l.color.b, 1) end
            text(PAD, y, body, color, W - 2 * PAD)
            for _, link in ipairs(l.links or {}) do
                local from = byte_at(body, link.start)
                local upto = byte_at(body, link.start + link.len)
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
    if tab.notices and #list == 0 then text(PAD, HEAD + 18, "Nothing to report", c.muted) end

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

    -- the open tab menu, over everything (it may reach past the window, so it is claimed too)
    state.menu_hits = {}
    local m = state.menu
    if m and m.entry and not index_of(m.entry) then close_menu(); m = nil end
    if m then
        local rows = {}
        local function add(label, run, enabled, check, header)
            rows[#rows + 1] = { label = label, run = run, enabled = enabled ~= false, check = check, header = header }
        end
        local e = m.entry
        if e then
            local at_i, count = index_of(e), #edited_tabs()
            add("Rename", function() close_menu(); begin_rename(e) end)
            add("Move left", function() close_menu(); move_tab(e, -1) end, at_i > 1)
            add("Move right", function() close_menu(); move_tab(e, 1) end, at_i < count)
            if not e.all then add("Delete", function() close_menu(); delete_tab(e) end) end
            if not e.all then
                add("Show in this tab", nil, false, nil, true)
                for _, kind in ipairs(STYLE_ORDER) do
                    add(STYLE_LABELS[kind], function() toggle_kind(e, kind) end, true, (e.kinds and e.kinds[kind]) and true or false)
                end
            end
        end
        add("New tab", function() close_menu(); new_tab() end)
        add("Reset tabs", function() close_menu(); reset_tabs() end)
        local mw, mh = 104 * k, (#rows * LINE + 4) * k
        local mx = clamp(m.x, 0, math.max(0, dw - mw))
        local my = clamp(m.y, 0, math.max(0, dh - mh))
        dl:AddRectFilled(ImVec2.new(mx, my), ImVec2.new(mx + mw, my + mh), c.raised)
        dl:AddRect(ImVec2.new(mx, my), ImVec2.new(mx + mw, my + mh), c.white)
        for i, r in ipairs(rows) do
            local ry = my + (2 + (i - 1) * LINE) * k
            local label = (r.check ~= nil and (r.check and "[x] " or "[ ] ") or "") .. r.label
            while #label > 0 and ui.CalcTextSize(label) > mw - 10 * k do
                label = label:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
                state.menu_cuts = state.menu_cuts + 1
            end
            local color = r.header and c.muted or (r.enabled and c.white or c.muted)
            dl:AddText(ImVec2.new(mx + 5 * k, ry), color, label)
            state.menu_hits[#state.menu_hits + 1] = { x = mx, y = ry, w = mw, h = LINE * k, run = r.enabled and r.run or nil,
                id = "menu:" .. r.label, enabled = r.enabled, check = r.check }
        end
        eqoa.input.claim_pointer({ x = mx, y = my, width = mw, height = mh })
    end
    ui.SetWindowFontScale(1)
    ui.End()
end)

-- what a test reads: the tabs, what the open one shows, and the hit areas by name
chat_probe = {
    state = state,
    visible = function() return #visible_lines(active_tab()) end,
    names = function()
        local out = {}
        for _, t in ipairs(tabs()) do out[#out + 1] = t.label end
        return out
    end,
    hit = function(id)
        for _, h in ipairs(state.menu and state.menu_hits or state.hits) do
            if h.id == id then return h end
        end
    end,
}
