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
local ENTRY_ROWS_MAX = 4 -- the line being typed wraps to this many rows, then keeps its end (the caret) in view
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
    lines = {}, combat = {}, seq = 0, tab = "all", scroll = 0,
    session = nil, text = "", channel = "say", links = {}, last_offer = 0, note = nil,
    hits = {}, rect = nil, tokens = {}, claim = nil,
    w = DEF_W, h = DEF_H, lx = 4, ly = nil, drag = nil,
    -- the tabs the player edits (nil until the styles are known, or after Reset tabs), the next id, the open menu
    -- (a tab's, or the strip's), its rows' hit areas, a tab being renamed, and what the text was cut by
    custom = nil, next_tab = 1, menu = nil, menu_hits = {}, rename = nil, cuts = 0, menu_cuts = 0, capturing = false,
    -- `rev` rises whenever what a tab shows can have changed (a new line, a style ticked, a tab made or removed): the
    -- list a tab shows is worked out again only then (`visible_lines`)
    rev = 0,
}

local function clamp(v, lo, hi) return math.max(lo, math.min(hi, v)) end

-- What the bridge is asked for is remembered while it cannot have changed: a text's width, how far a text is cut to
-- fit, a colour's packed value. They hold for one scale of the window and one font (`measure_key`, checked each frame
-- with a reference text), so a resize or a font change measures everything again. Bounded: it starts over past a limit.
local MEASURE_MOST = 1500
local measure_key, widths, width_n, fits, fit_n, packed = nil, {}, 0, {}, 0, nil
local function width(s)
    local w = widths[s]
    if w == nil then
        if width_n >= MEASURE_MOST then widths, width_n = {}, 0 end
        w = ui.CalcTextSize(s)
        widths[s], width_n = w, width_n + 1
    end
    return w
end

-- How a message wraps in `limit` window pixels: its rows as {first byte, last byte}, words kept whole, a word longer than
-- a row broken inside it, the spaces a row ends at dropped. A message that fits is one row.
local function wrap_rows(body, limit)
    local n = #body
    if n == 0 or width(body) <= limit then return { { 1, n } } end
    local rows = {}
    local start = body:find("%S") or 1
    while start and start <= n do
        local last, i = nil, start
        while true do
            local ws, we = body:find("%S+", i)
            if not ws then last = last or n; break end
            if width(body:sub(start, we)) <= limit then
                last, i = we, we + 1
            elseif last then
                break
            else
                -- one word wider than a row: as many characters as fit (at least one)
                local seg = body:sub(start, we)
                local lo, hi = 1, chars(seg)
                while lo < hi do
                    local mid = math.ceil((lo + hi) / 2)
                    if width(seg:sub(1, byte_at(seg, mid + 1) - 1)) <= limit then lo = mid else hi = mid - 1 end
                end
                last = start + byte_at(seg, lo + 1) - 2
                break
            end
        end
        rows[#rows + 1] = { start, last }
        start = body:find("%S", last + 1)
    end
    if #rows == 0 then rows[1] = { 1, n } end
    return rows
end

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
    state.rev = state.rev + 1
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

local function add_line(m)
    state.seq = state.seq + 1
    state.lines[#state.lines + 1] = { text = m.text, kind = m.kind or "system", links = m.links or {}, color = m.color, seq = state.seq }
    while #state.lines > MAX_LINES do table.remove(state.lines, 1) end
    state.rev = state.rev + 1
end

-- What this mod has to say (a line it could not send, and why) comes as a System line, in red.
local function notice(text)
    add_line({ text = "[Chat] " .. text, kind = "system", color = { r = 1, g = .45, b = .45 } })
end

-- The combat log: the game tells the client of every hit as a number over the head, not as a line of chat, so the
-- Combat tab is made from those hits (yours, your pet's and what hits you or your pet), in time order with the lines.
local MAX_COMBAT = 300
local function add_combat(text, color)
    state.seq = state.seq + 1
    state.combat[#state.combat + 1] = { text = text, kind = "combat", links = {}, color = color, seq = state.seq }
    while #state.combat > MAX_COMBAT do table.remove(state.combat, 1) end
    state.rev = state.rev + 1
end

local function entity_name(id)
    local e = eqoa.entities.find(id)
    return e and e.name or "Something"
end

-- `by_player` and `from_merc` are the core's answer to who made the figure: a server that tags its numbers tells a pet's or
-- a mercenary's hit with the player as the attacker, so `outgoing` alone would make them yours. Three states: `by_player`
-- true (yours: "You"), false (a pet's, a mercenary's or another's: named; a pet-tagged hit is "Your pet" only while you have one), nil (`attribution == "unknown"`: the server sends
-- no tag, so a hit credited to the player may be the player's, its pet's or its mercenary's; "Your side", never "You").
local function on_hit(d)
    if not d or not (d.outgoing or d.incoming or d.from_pet or d.to_pet or d.from_merc) then return end
    local heal = d.kind == "heal"
    local mine = d.outgoing and d.by_player == true
    local unknown = d.outgoing and d.by_player == nil and d.from_merc == nil
    local who
    if mine then
        who = "You"
    elseif d.from_merc then
        local e = not d.outgoing and eqoa.entities.find(d.attacker_id)
        who = e and e.name or "Your mercenary"
    elseif unknown then
        who = "Your side"
    elseif d.outgoing then
        -- the tag's pet bit (a pet of the player's side made it): the player's own pet only while the player has one. A
        -- mercenary has pets of its own, tagged the same way, so with none of yours out the line says "Your side's pet".
        local p = eqoa.player.get()
        who = (p and p.pet_id ~= nil) and "Your pet" or "Your side's pet"
    else
        who = entity_name(d.attacker_id)
    end
    local whom = (mine and d.incoming) and "yourself" or (d.incoming and "you" or entity_name(d.defender_id))
    local verb = heal and (mine and "heal" or "heals") or (mine and "hit" or "hits")
    local color = heal and { r = .5, g = 1, b = .6 } or (d.incoming and { r = 1, g = .6, b = .6 } or { r = .85, g = .95, b = .85 })
    add_combat(string.format("%s %s %s for %d.", who, verb, whom, d.amount or 0), color)
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

-- The tabs: All, then a tab per chat style, as the player edited them.
local function tabs()
    local out = {}
    for _, t in ipairs(edited_tabs()) do
        out[#out + 1] = { id = t.id, label = t.name, kinds = (not t.all) and t.kinds or nil, entry = t }
    end
    return out
end

local function active_tab()
    for _, t in ipairs(tabs()) do if t.id == state.tab then return t end end
    state.tab = "all"
    return tabs()[1]
end

-- The list the open tab shows, kept while nothing it is made of has changed (`state.rev`) and the tab is the same one:
-- the lines of a style's tab were filtered, and the combat log merged in, again every drawn frame.
local shown = { id = nil, kinds = nil, rev = -1, list = nil }
local function visible_lines(tab)
    -- All is every line of chat (the combat log is for the tabs that show Combat)
    if not tab.kinds then return state.lines end
    if shown.list and shown.id == tab.id and shown.kinds == tab.kinds and shown.rev == state.rev then return shown.list end
    local out = {}
    for _, l in ipairs(state.lines) do
        if tab.kinds[l.kind] then out[#out + 1] = l end
    end
    if tab.kinds.combat and #state.combat > 0 then
        -- the combat log lines in time order with the rest
        local merged, i, j = {}, 1, 1
        while i <= #out or j <= #state.combat do
            local a, b = out[i], state.combat[j]
            if b == nil or (a and a.seq < b.seq) then
                merged[#merged + 1] = a
                i = i + 1
            else
                merged[#merged + 1] = b
                j = j + 1
            end
        end
        out = merged
    end
    shown.id, shown.kinds, shown.rev, shown.list = tab.id, tab.kinds, state.rev, out
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
    state.rev = state.rev + 1
    persist()
end

local function delete_tab(t)
    if t.all then return end
    local list = edited_tabs()
    local i = index_of(t)
    if not i then return end
    table.remove(list, i)
    state.rev = state.rev + 1
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
    state.rev = state.rev + 1
    persist()
end

local function reset_tabs()
    state.custom = nil
    state.rev = state.rev + 1
    state.tab, state.scroll = "all", 0
    persist()
end

local function toggle_kind(t, kind)
    if t.all then return end
    t.kinds = t.kinds or {}
    t.kinds[kind] = (not t.kinds[kind]) or nil
    state.rev = state.rev + 1
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

-- The channel a line goes to. The game's own four also become the game's chat mode (what Community > Chat Modes sets).
local function choose_channel(id)
    state.channel = id
    if id == "say" or id == "group" or id == "guild" or id == "shout" then
        local _, err = eqoa.actions.set_channel(id)
        if err then state.note = err.message or err.code end
    end
end

-- A slash command that only names a channel changes the channel and keeps the line: "/g", "/s", "/gu", "/say" (and
-- their long names), "/t" and "/tell" (then the name), "/r" and "/reply". `final` is Enter; without it (the space typed
-- after the word) only the word and its space count. Returns whether the text was one.
local SLASH_CHANNELS = {
    s = "shout", shout = "shout", g = "group", group = "group", gu = "guild", guild = "guild", sa = "say", say = "say",
    t = "tell", tell = "tell", r = "reply", reply = "reply",
}
local function channel_command(final)
    local word, rest = state.text:match("^/(%a+)%s*(.*)$")
    local id = word and SLASH_CHANNELS[word:lower()]
    if not id then return false end
    local spaced = state.text:match("^/%a+%s") ~= nil
    if not final and not (spaced and rest == "") then return false end
    if id == "tell" then
        -- "/t Wil" keeps the name for the message; a message after it is a line to send, not a channel change
        if rest:find("%s") then return false end
        choose_channel("tell")
        state.text = rest ~= "" and (rest .. " ") or ""
        return true
    end
    if rest ~= "" then return false end
    choose_channel(id)
    state.text = ""
    return true
end

-- Sending: Tell takes the first word as the player's name; a line that starts with / is a command.
local function send()
    -- "/g" and the like only change the channel: the line stays open to type in
    if channel_command(true) then return end
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
    -- the space after "/g" (or "/t", "/r") is the channel change
    channel_command(false)
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

-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end
local GATED = { ["input.key"] = true, ["pointer.pressed"] = true, ["pointer.wheel"] = true }
local function listen(name, fn)
    local inner = fn
    if GATED[name] then inner = function(e) if map_up() then return end return fn(e) end end
    local token = eqoa.events.on(name, inner)
    if token then state.tokens[#state.tokens + 1] = token end
end

-- the history first, then every line as it comes
do
    local old = eqoa.chat.history(200)
    for _, m in ipairs(old or {}) do add_line(m) end
end
listen("chat.message", function(e) add_line(e.data) end)
listen("combat.hit", function(e) on_hit(e.data) end)
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

-- The client lets go of every claim when the world is left (zoning, a respawn, a teleport, a new login) and with the
-- player's "Use the game's windows" switch: the chat line and the hiding of the game's chat are taken again on the new
-- world, and now and then besides. Asking for a kind already held is no change. The hide is asked for again on each new
-- world (`e.generation` is the world's number), after letting go of the old token, so the mod never holds two claims
-- and an old world's token never outlives the mod.
local told
local in_world = false
local hide_world, hide_told
local function claim(world)
    local ok, err = eqoa.windows.takeover("chat_entry", { scope = "all" })
    local now = ok and "held" or (err and err.code or "refused")
    if now ~= told then
        told = now
        if not ok then
            eqoa.log.write("warn", "Chat could not take the Enter key: " .. tostring(err and err.message))
        end
    end
    if not in_world then return end
    if state.hide and world ~= nil and world ~= hide_world then
        eqoa.hud.show(state.hide)
        state.hide = nil
    end
    if not state.hide then
        local token, herr = eqoa.hud.hide("chat")
        state.hide, hide_world = token, world
        local code = token and "held" or (herr and herr.code or "refused")
        if code ~= hide_told then
            hide_told = code
            if not token then
                eqoa.log.write("warn", "Chat could not hide the game's chat: " .. tostring(herr and herr.message))
            end
        end
    end
end
claim()
eqoa.events.on("world.entered", function(e) in_world = true; claim(e.generation) end)
eqoa.events.on("state.reset", function(e)
    in_world = e.data and e.data.in_world == true
    claim(e.generation)
end)
eqoa.events.on("world.leaving", function() in_world = false end)

local function release()
    close_menu()
    state.rename = nil
    if state.session then eqoa.windows.close_session(state.session.ref); state.session = nil end
    eqoa.input.capture_text(false)
    eqoa.windows.release("chat_entry")
    if state.hide then eqoa.hud.show(state.hide); state.hide = nil end
end
eqoa.events.on("disabling", release)
local RECLAIM_FRAMES, frames = 120, 0
eqoa.events.on("render", function()
    frames = frames + 1
    if frames % RECLAIM_FRAMES == 0 then claim() end
    if map_up() then
        -- the game's map has the screen and the keyboard: a line being typed ends as the game's would, a menu or a
        -- rename is dropped, nothing is drawn or claimed; the window, its tabs and lines are as they were when it closes
        local s = state.session
        if s then state.session = nil; eqoa.windows.close_session(s.ref) end
        if state.rename then finish_rename(false) end
        close_menu()
        state.drag = nil
        if state.capturing or s then eqoa.input.capture_text(false); state.capturing = false end
        state.hits, state.rect = {}, nil
        return
    end
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    -- the client lets a text capture lapse after a second unless it is asked for again: every frame it is wanted
    local want = state.session ~= nil or state.rename ~= nil
    if want then
        eqoa.input.capture_text(true)
        state.capturing = true
    elseif state.capturing then
        eqoa.input.capture_text(false)
        state.capturing = false
    end
    local tab = active_tab()
    local list = visible_lines(tab)
    local entry_h = state.session and 40 or 0
    local W, H, ROWS, Ht, x0, y0
    local function layout()
        W, H = state.w, state.h
        ROWS = math.max(2, math.floor((H - HEAD - 22) / LINE))
        Ht = H + entry_h
        -- a line wrapped to more rows grows the window upward: its bottom edge stays where it was
        local extra = state.session and (entry_h - 40) or 0
        local ly = state.ly and (state.ly - extra) or (448 - Ht - 24)
        x0 = clamp(f.offset_x + state.lx * f.scale_x, 0, math.max(0, dw - W * k))
        y0 = clamp(f.offset_y + ly * f.scale_y, 0, math.max(0, dh - Ht * k))
        state.eff_ly = ly + (state.ly and extra or 0)
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
    -- one reference measure a frame says whether the scale or the font moved since the remembered measures were made
    local mkey = k .. "|" .. ui.CalcTextSize("Mgj WW")
    if mkey ~= measure_key then
        measure_key = mkey
        widths, width_n, fits, fit_n, packed = {}, 0, {}, 0, nil
    end
    -- the line being typed, wrapped at the window's inner width (words kept whole, a long word broken inside); the last
    -- ENTRY_ROWS_MAX rows show, so the caret at the end is always in view. A new height is laid out again at once.
    local entry_rows, prompt
    if state.session then
        prompt = state.channel == "tell" and "To (name message): " or (state.channel == "reply" and "Reply: " or "")
        local body = prompt .. state.text .. "_"
        local rows = wrap_rows(body, (W - 2 * PAD) * k)
        entry_rows = {}
        for i = math.max(1, #rows - ENTRY_ROWS_MAX + 1), #rows do entry_rows[#entry_rows + 1] = body:sub(rows[i][1], rows[i][2]) end
        local want_h = 40 + (#entry_rows - 1) * LINE
        if want_h ~= entry_h then entry_h = want_h; layout() end
        -- readable by the mod's tests: the widest shown row, the room a row has, and what the whole line would need
        local widest = 0
        for _, r in ipairs(entry_rows) do widest = math.max(widest, width(r)) end
        state.entry_rows, state.entry_fit = entry_rows, { widest = widest, limit = (W - 2 * PAD) * k, full = width(body) }
    end
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
            persist()
        end
    end
    local dl = ui.GetWindowDrawList()
    if not packed then
        packed = {
            text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.55, .55, .55, 1),
            surface = ui.GetColorU32(.05, .05, .05, .8), raised = ui.GetColorU32(.12, .12, .12, 1),
            border = ui.GetColorU32(.25, .25, .25, 1), white = ui.GetColorU32(1, 1, 1, 1),
            link = ui.GetColorU32(.45, .7, 1, 1), warn = ui.GetColorU32(1, .35, .35, 1),
            on = ui.GetColorU32(.2, .2, .3, 1),
        }
    end
    local c = packed
    local function fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    local function frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    local function text(x, y, value, color, maxw)
        value = tostring(value)
        if maxw then
            -- cut to fit once for a text and a width; the cut is the same until the scale or font changes
            local by_width = fits[maxw]
            if not by_width then by_width = {}; fits[maxw] = by_width end
            local kept = by_width[value]
            if not kept then
                if fit_n >= MEASURE_MOST then fits, fit_n = {}, 0; by_width = {}; fits[maxw] = by_width end
                local cut, shown_text = 0, value
                while #shown_text > 0 and width(shown_text) > maxw * k do
                    shown_text = shown_text:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
                    cut = cut + 1
                end
                kept = { text = shown_text, cut = cut }
                by_width[value], fit_n = kept, fit_n + 1
            end
            state.cuts = state.cuts + kept.cut
            value = kept.text
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
    local limit = W - 20
    if state.rename then
        -- a tab being renamed: its name is typed here (Enter keeps it, Esc drops it)
        fill(2, HEAD + 2, W - 44, 12, c.raised)
        frame(2, HEAD + 2, W - 44, 12, c.white)
        text(6, HEAD + 2, "Rename: " .. state.rename.text .. "_   (Enter, Esc)", c.white, W - 52)
    else
        for _, t in ipairs(tabs()) do
            local tw = width(t.label) / k + 8
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
        do
            -- the small + (always there, at the right) makes a new tab
            fill(W - 16, HEAD + 2, 12, 12, c.raised)
            frame(W - 16, HEAD + 2, 12, 12, c.border)
            text(W - 13, HEAD + 2, "+", c.muted)
            state.hits[#state.hits + 1] = tag(hit(W - 16, HEAD + 2, 12, 12, new_tab), "tab:new")
        end
        if tx < limit then
            -- the empty strip beside the tabs: a right click there offers New tab and Reset tabs
            state.hits[#state.hits + 1] = tag(hit(tx, HEAD + 2, limit - tx, 12, function() end), "strip",
                function(d) state.menu = { x = d.x, y = d.y } end)
        end
    end

    -- the strip beside the tabs moves the window; the corner sizes it
    state.hits[#state.hits + 1] = { x = x0, y = y0, w = W * k, h = HEAD * k, drag = "move", id = "move" }
    -- a faint grip line, no words
    fill(W / 2 - 14, 4, 28, 1, c.border)
    fill(W / 2 - 14, 6, 28, 1, c.border)
    state.hits[#state.hits + 1] = { x = x0 + (W - 12) * k, y = y0 + (H - 12) * k, w = 12 * k, h = 12 * k, drag = "size" }
    fill(W - 8, H - 3, 6, 1, c.muted)
    fill(W - 6, H - 6, 4, 1, c.muted)
    fill(W - 4, H - 9, 2, 1, c.muted)

    -- the lines, newest at the bottom; the wheel scrolls back, a row at a time. A message longer than the window wraps at
    -- the window's inner width (words kept whole; a word longer than the line breaks inside it). Its rows are worked out
    -- once for a width and kept on the message (`wrap_rows`), and only the messages the visible rows (and the scrolled-back
    -- ones) come from are looked at: never the whole history in a frame.
    local limit_px = (W - 2 * PAD) * k
    local wkey = measure_key .. "|" .. limit_px
    local need = state.scroll + ROWS
    local picked, found = {}, 0
    local index = #list
    while index >= 1 and found < need do
        local l = list[index]
        if l.wkey ~= wkey then l.wrows, l.wkey = wrap_rows(l.text, limit_px), wkey end
        for r = #l.wrows, 1, -1 do
            found = found + 1
            picked[found] = { l = l, b0 = l.wrows[r][1], b1 = l.wrows[r][2] }
            if found >= need then break end
        end
        index = index - 1
    end
    -- the oldest row is at the top at the most: that many rows back, and no more
    if found < need then state.scroll = math.max(0, found - ROWS) end
    state.drawn = {}
    for row = 1, ROWS do
        local pk = picked[state.scroll + (ROWS - row) + 1]
        if pk then
            local l = pk.l
            local y = HEAD + 18 + (row - 1) * LINE
            local color = l.kind == "notice" and c.warn or c.text
            if l.color and l.color.r then
                if l.packed_for ~= packed then
                    l.packed, l.packed_for = ui.GetColorU32(l.color.r, l.color.g, l.color.b, 1), packed
                end
                color = l.packed
            end
            local body = l.text
            local piece = body:sub(pk.b0, pk.b1)
            text(PAD, y, piece, color, W - 2 * PAD)
            state.drawn[#state.drawn + 1] = { text = piece, width = width(piece) / k, y = y, color = l.color, msg = l.seq }
            for _, link in ipairs(l.links or {}) do
                local from = byte_at(body, link.start)
                local upto = byte_at(body, link.start + link.len) - 1
                -- the part of the link on this row (a link split across rows is a link on each)
                local s0, e0 = math.max(from, pk.b0), math.min(upto, pk.b1)
                if s0 <= e0 then
                    local lx = PAD + width(body:sub(pk.b0, s0 - 1)) / k
                    local name = body:sub(s0, e0)
                    local lw = width(name) / k
                    if lx + lw < W - PAD + 0.5 then
                        text(lx, y, name, c.link)
                        state.hits[#state.hits + 1] = tag(hit(lx, y, lw, LINE, function() open_link(link) end), "link")
                    end
                end
            end
        end
    end

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
            local bw = width(label) / k + 8
            fill(x, y, bw, 12, ch.id == state.channel and c.on or c.surface)
            frame(x, y, bw, 12, c.border)
            text(x + 4, y, label, ch.id == state.channel and c.white or c.muted)
            local id = ch.id
            state.hits[#state.hits + 1] = tag(hit(x, y, bw, 12, function()
                choose_channel(id)
            end), "chan:" .. id)
            x = x + bw + 2
        end
        for i, row in ipairs(entry_rows) do text(PAD, y + 15 + (i - 1) * LINE, row, c.white, W - 2 * PAD) end
        local foot = state.note or "Enter sends, Esc cancels, Tab changes channel"
        text(PAD, y + 28 + (#entry_rows - 1) * LINE, foot, state.note and c.warn or c.muted, W - 2 * PAD)
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
    rows = function() return state.drawn end,
    state = state,
    visible = function() return #visible_lines(active_tab()) end,
    lines = function()
        local out = {}
        for _, l in ipairs(visible_lines(active_tab())) do out[#out + 1] = l.text end
        return out
    end,
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
