-- Auction: search, bid and buy now in a window of this mod's own (default Shift+A, or the Start menu's Auction).
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- It sits over play: the character keeps walking, and the game's own Auction pages are exactly as they were with the
-- mod off. A bid and a buy now are asked of the player by the client ("Are you sure?", showing the price): nothing
-- here answers it. Putting an item up for sale is done from the Bag Viewer's bank item menu, not here.
local ui = eqoa.imgui

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, H, PAD, LINE = 380, 336, 8, 13
local ROWS = 8
-- the game's wear slots a search can name (its own picker's list); the words come from the game
local WEARS = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 22, 23, 24, 25, 27 }
local LIMITS = { name = 40, min_level = 3, max_level = 3, bid = 9 }

local state = {
    open = false, tab = "search", session = nil, tokens = {}, escape = nil, enter = nil,
    name = "", min_level = "", max_level = "", wear = nil, bid = "", count = 1, sel = nil, scroll = 0,
    field = nil, result = nil, last_req = nil, picking = false, pick_scroll = 0, searched = false,
    hits = {}, row_hits = {}, list_rect = nil, pick_rect = nil, field_rect = nil, a = nil,
}

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then state.tokens[#state.tokens + 1] = token end
end

local function say(text, ok) state.result = { text = text, ok = ok } end

local function chop(s) return s:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or "" end

local function run(fn)
    local request, err = fn()
    if err then
        state.last_req = nil
        say(err.message or err.code or "Refused", false)
        return nil
    end
    state.last_req = request
    state.result = nil
    return request
end

local function wear_name(w)
    local info = eqoa.items.slot_info(w)
    return info and info.name or ("Slot " .. w)
end

local close, draw

local function stop_typing()
    state.field = nil
    if state.enter then eqoa.input.release(state.enter); state.enter = nil end
    eqoa.input.capture_text(false)
end

local function start_typing(field)
    state.field = field
    -- Enter is this window's while a box is typed in (the exact name: another spelling is refused and leaks to the game)
    if not state.enter then state.enter = eqoa.input.claim("Enter", true) end
end

local function search()
    stop_typing()
    local args = { name = state.name ~= "" and state.name or nil, wear = state.wear }
    if state.min_level ~= "" then args.min_level = tonumber(state.min_level) end
    if state.max_level ~= "" then args.max_level = tonumber(state.max_level) end
    state.sel, state.scroll, state.searched = nil, 0, true
    run(function() return eqoa.actions.auction_search(args) end)
end

local function turn(forward)
    state.sel, state.scroll = nil, 0
    run(function() return eqoa.actions.auction_page(forward) end)
end

-- the cheapest bid the game's form takes on a lot
local function floor_of(l)
    if l.next_bid then return math.max(l.min_bid, l.next_bid) end
    return math.max(l.min_bid, (l.high_bid or -1) + 1)
end

local function select_lot(l)
    state.sel = l.lot
    state.bid = tostring(floor_of(l))
    state.count = 1
end

local function switch_tab(tab)
    if tab == state.tab then return end
    stop_typing()
    state.tab, state.result, state.sel, state.scroll, state.picking = tab, nil, nil, 0, false
    if tab == "lots" then run(function() return eqoa.actions.auction_my_lots() end)
    elseif tab == "bids" then run(function() return eqoa.actions.auction_my_bids() end) end
end

local function on_confirm(d)
    if not d or (d.kind ~= "auction_bid" and d.kind ~= "auction_buyout") then return end
    local what = d.kind == "auction_bid" and "Bid" or "Buy now"
    if d.answer == "yes" then
        local sent = d.detail and d.detail.sent
        say(sent and (what .. " sent") or (what .. " not made (" .. tostring(d.detail and d.detail.reason) .. ")"), sent == true)
    else
        say(what .. " cancelled", true)
    end
end

local function open(page)
    if state.open then
        if page == "lots" or page == "bids" or page == "search" then switch_tab(page) end
        return
    end
    local s, err = eqoa.windows.open_session("auction_search", { opened_from = "key" })
    if not s then
        eqoa.log.write("warn", "Auction did not open: " .. tostring(err and err.message))
        return
    end
    state.session = s.ref
    state.open, state.result, state.last_req, state.sel, state.field, state.tab, state.scroll = true, nil, nil, nil, nil, "search", 0
    state.searched = false
    -- My lots / My bids from the Start menu: the list is asked for once the window is up (a first draw), not here
    state.want = nil
    if page == "lots" or page == "bids" then state.tab, state.want = page, page end
    listen("render", draw)
    listen("action.result", function(e)
        local r = e.data
        if r and state.last_req ~= nil and r.request_id == state.last_req and r.ok == false then
            say(r.reason or r.code or "Refused", false)
        end
    end)
    listen("confirm.closed", function(e) on_confirm(e.data) end)
    listen("windows.session", function(e)
        local d = e.data
        if d.phase == "closed" and d.ref == state.session then close(true) end
    end)
    listen("input.key", function(e)
        local d = e.data
        if not d.down then return end
        local key = d.key:lower()
        if state.field then
            local f = state.field
            if key == "escape" then stop_typing()
            elseif key == "enter" or key == "return" then
                if f == "name" or f == "min_level" or f == "max_level" then search() else stop_typing() end
            elseif key == "tab" then
                state.field = f == "name" and "min_level" or (f == "min_level" and "max_level" or "name")
            elseif key == "backspace" then state[f] = chop(state[f])
            elseif d.text and #d.text > 0 then
                local t = d.text
                if f ~= "name" then t = t:gsub("%D", "") else t = t:gsub("[\r\n]", " ") end
                if #state[f] + #t <= LIMITS[f] then state[f] = state[f] .. t end
            end
        elseif key == "escape" and not d["repeat"] then
            if state.picking then state.picking = false else close() end
        end
    end)
    listen("pointer.pressed", function(e)
        local d = e.data
        if d.button ~= "left" then return end
        for _, h in ipairs(state.hits) do
            if inside(d.x, d.y, h) and h.run then h.run(); return end
        end
        if state.field and not inside(d.x, d.y, state.field_rect) then stop_typing() end
        for _, h in ipairs(state.row_hits) do
            if inside(d.x, d.y, h) then h.run(); return end
        end
    end)
    listen("pointer.wheel", function(e)
        local d = e.data
        if (d.steps or 0) == 0 then return end
        local delta = d.steps > 0 and -1 or 1
        if state.picking and inside(d.x, d.y, state.pick_rect) then
            state.pick_scroll = math.max(0, math.min(math.max(0, #WEARS + 1 - 6), state.pick_scroll + delta))
        elseif inside(d.x, d.y, state.list_rect) and state.a then
            local n = state.tab == "search" and #state.a.results or (state.tab == "lots" and #state.a.lots or #state.a.bids)
            state.scroll = math.max(0, math.min(math.max(0, n - ROWS), state.scroll + delta))
        end
    end)
    -- Escape is the window's, not the game menu's, while it is up
    state.escape = eqoa.input.claim("Escape", true)
end

function close(from_client)
    if not state.open then return end
    for _, token in ipairs(state.tokens) do eqoa.events.off(token) end
    state.tokens = {}
    if state.escape then eqoa.input.release(state.escape); state.escape = nil end
    if not from_client and state.session then eqoa.windows.close_session(state.session) end
    stop_typing()
    state.session, state.open = nil, false
    state.hits, state.row_hits, state.a, state.picking = {}, {}, nil, false
end

eqoa.input.bind({ id = "open", label = "Auction", default_key = "shift+a" }, function()
    if state.open then close() else open("search") end
end)
eqoa.events.on("mod.command", function(e)
    if e.data.command ~= "open" then return end
    local page = type(e.data.args) == "table" and e.data.args.page or "search"
    open(page)
end)
eqoa.events.on("disabling", function() close() end)
eqoa.events.on("world.leaving", function() close() end)
-- the client lets go of every session and claim when the world is left or a new one starts: nothing is kept from before
eqoa.events.on("state.reset", function() close() end)
eqoa.events.on("world.entered", function() close() end)

local function time_left(minutes)
    if minutes == nil then return "" end
    if minutes <= 0 then return "ended" end
    if minutes >= 60 then return math.floor(minutes / 60) .. "h " .. (minutes % 60) .. "m" end
    return minutes .. "m"
end

function draw()
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    local a = eqoa.auction.get()
    state.a = a
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    -- the text capture lapses after about a second: renewed every frame a box is typed in
    if state.field then eqoa.input.capture_text(true) end

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_auction_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 11 / 13)
    local dl = ui.GetWindowDrawList()
    local col = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), good = ui.GetColorU32(.35, .8, .4, 1),
        warn = ui.GetColorU32(1, .3, .3, 1), white = ui.GetColorU32(1, 1, 1, 1), hl = ui.GetColorU32(.20, .20, .24, 1),
        mark = ui.GetColorU32(.95, .72, .20, 1),
    }
    local function fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    local function frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    local function text(x, y, value, color, maxw)
        value = tostring(value)
        while maxw and #value > 0 and ui.CalcTextSize(value) > maxw * k do value = chop(value) end
        dl:AddText(at(x, y), color or col.text, value)
    end
    local function hit(x, y, w, h, run_) return { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k, run = run_ } end
    local function button(x, y, w, label, enabled, run_)
        fill(x, y, w, 16, col.raised)
        frame(x, y, w, 16, enabled and col.border or col.surface)
        text(x + 6, y + 2, label, enabled and col.text or col.muted, w - 8)
        if enabled then state.hits[#state.hits + 1] = hit(x, y, w, 16, run_) end
    end
    local function box(x, y, w, label, field)
        text(x, y, label, col.muted)
        local bx = x + (label ~= "" and (ui.CalcTextSize(label) / k + 4) or 0)
        local bw = w - (bx - x)
        fill(bx, y - 2, bw, 14, col.raised)
        frame(bx, y - 2, bw, 14, state.field == field and col.white or col.border)
        local shown = state[field] .. (state.field == field and "_" or "")
        while #shown > 0 and ui.CalcTextSize(shown) > (bw - 8) * k do shown = shown:sub(2) end
        text(bx + 4, y, shown, col.text)
        state.hits[#state.hits + 1] = hit(bx, y - 2, bw, 14, function() start_typing(field); state.picking = false end)
        if state.field == field then state.field_rect = { x = x0 + bx * k, y = y0 + (y - 2) * k, w = bw * k, h = 14 * k } end
    end

    fill(0, 0, W, H, col.surface)
    frame(0, 0, W, H, col.border)
    text(PAD, 6, "Auction", col.white)
    text(W - PAD - 110, 6, "Shift+A or Esc", col.muted)
    state.hits, state.row_hits = {}, {}
    fill(W - 18, 4, 12, 12, col.raised)
    frame(W - 18, 4, 12, 12, col.border)
    text(W - 15, 4, "x", col.text)
    state.hits[1] = hit(W - 18, 4, 12, 12, function() close() end)

    if not a then
        text(PAD, 40, "Opening...", col.muted)
        ui.SetWindowFontScale(1)
        ui.End()
        return
    end
    for i, t in ipairs({ { "search", "Search" }, { "lots", "My lots" }, { "bids", "My bids" } }) do
        local tx = PAD + (i - 1) * 84
        local on = state.tab == t[1]
        fill(tx, 20, 80, 16, on and col.hl or col.raised)
        frame(tx, 20, 80, 16, on and col.white or col.border)
        text(tx + 8, 22, t[2], on and col.white or col.muted)
        state.hits[#state.hits + 1] = hit(tx, 20, 80, 16, function() switch_tab(t[1]) end)
    end
    if state.want and a.offered then
        local w = state.want
        state.want = nil
        run(function() return w == "lots" and eqoa.actions.auction_my_lots() or eqoa.actions.auction_my_bids() end)
    end
    if not a.offered then
        text(PAD, 44, "This server has no auction.", col.warn)
        state.hits = { state.hits[1] }
        eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
        ui.SetWindowFontScale(1)
        ui.End()
        return
    end
    local busy = a.pending ~= "none"
    state.list_rect, state.field_rect, state.pick_rect = nil, nil, nil
    local list_y

    if state.tab == "search" then
        box(PAD, 44, 170, "Name", "name")
        local wlabel = state.wear and wear_name(state.wear) or "Any type"
        button(PAD + 180, 41, 130, "Type: " .. wlabel, true, function()
            stop_typing(); state.picking = not state.picking; state.pick_scroll = 0
        end)
        box(PAD, 62, 100, "Level", "min_level")
        box(PAD + 104, 62, 60, "to", "max_level")
        button(PAD + 180, 59, 60, "Search", not busy, search)
        button(PAD + 244, 59, 50, "< Page", not busy and #a.results > 0, function() turn(false) end)
        button(PAD + 298, 59, 56, "Page >", not busy and #a.results > 0, function() turn(true) end)
        list_y = 84
        state.list_rect = { x = x0 + PAD * k, y = y0 + list_y * k, w = (W - 2 * PAD) * k, h = ROWS * LINE * k }
        if #a.results == 0 then
            text(PAD + 2, list_y, busy and "Searching..." or (state.searched and "Nothing found" or "Type what to look for and press Search"), col.muted)
        end
        local chosen
        for i = 1, ROWS do
            local l = a.results[state.scroll + i]
            if l then
                local y = list_y + (i - 1) * LINE
                if state.sel == l.lot then fill(PAD, y, W - 2 * PAD, LINE, col.hl); chosen = l end
                text(PAD + 2, y, l.name .. (l.quantity > 1 and (" x" .. l.quantity) or ""), l.mine and col.mark or col.text, 168)
                text(PAD + 176, y, l.high_bid and tostring(l.high_bid) or "no bids", col.muted, 56)
                text(PAD + 236, y, l.buyout > 0 and tostring(l.buyout) or "", col.good, 56)
                text(PAD + 296, y, time_left(l.minutes_left), (l.minutes_left or 1) <= 0 and col.warn or col.muted, 60)
                state.row_hits[#state.row_hits + 1] = hit(PAD, y, W - 2 * PAD, LINE, function() select_lot(l) end)
            end
        end
        text(PAD + 176, list_y - 11, "High bid", col.muted)
        text(PAD + 236, list_y - 11, "Buy now", col.muted)
        text(PAD + 296, list_y - 11, "Time left", col.muted)
        -- the lot chosen
        local dy = list_y + ROWS * LINE + 6
        if chosen then
            text(PAD, dy, chosen.name, col.white, W - 2 * PAD)
            text(PAD, dy + LINE, "Seller " .. chosen.seller .. "   Lowest bid " .. floor_of(chosen)
                .. (chosen.buyout > 0 and ("   Buy now " .. chosen.buyout) or "") .. (chosen.quantity > 1 and ("   " .. chosen.quantity .. " in the lot") or ""),
                col.muted, W - 2 * PAD)
            if chosen.mine then text(PAD, dy + 2 * LINE, "This is your own lot", col.mark) end
            local by = dy + 3 * LINE + 2
            box(PAD, by + 2, 120, "Bid", "bid")
            if a.results[1] and a.results[1].next_bid ~= nil and chosen.quantity > 1 then
                text(PAD + 128, by + 2, "Count", col.muted)
                button(PAD + 160, by - 1, 20, "-", state.count > 1, function() state.count = state.count - 1 end)
                text(PAD + 184, by + 2, state.count, col.text)
                button(PAD + 204, by - 1, 20, "+", state.count < chosen.quantity, function() state.count = state.count + 1 end)
            end
            local can = not busy and not chosen.mine and (chosen.minutes_left or 1) > 0
            button(PAD + 232, by - 1, 52, "Bid", can and tonumber(state.bid) ~= nil, function()
                stop_typing()
                run(function() return eqoa.actions.auction_bid({ lot = chosen.lot, price = tonumber(state.bid), count = state.count }) end)
            end)
            button(PAD + 290, by - 1, 78, "Buy now", can and chosen.buyout > 0, function()
                stop_typing()
                run(function() return eqoa.actions.auction_buyout(chosen.lot) end)
            end)
        else
            text(PAD, dy, "Click a lot to bid on it", col.muted)
        end
    elseif state.tab == "lots" then
        list_y = 44
        state.list_rect = { x = x0 + PAD * k, y = y0 + list_y * k, w = (W - 2 * PAD) * k, h = ROWS * LINE * k }
        text(PAD, list_y - 1 + 0, "", col.muted)
        if #a.lots == 0 then text(PAD + 2, list_y, busy and "Asking..." or "You have nothing up at the auction", col.muted) end
        for i = 1, ROWS + 4 do
            local l = a.lots[state.scroll + i]
            if l then
                local y = list_y + (i - 1) * LINE
                text(PAD + 2, y, l.name .. (l.quantity > 1 and (" x" .. l.quantity) or ""), col.text, 230)
                text(PAD + 240, y, l.buyout > 0 and ("buy now " .. l.buyout) or "", col.good, 80)
                text(PAD + 316, y, time_left(l.minutes_left), col.muted, 50)
            end
        end
        button(PAD, H - 40, 64, "Refresh", not busy, function() run(function() return eqoa.actions.auction_my_lots() end) end)
    else
        list_y = 44
        state.list_rect = { x = x0 + PAD * k, y = y0 + list_y * k, w = (W - 2 * PAD) * k, h = ROWS * LINE * k }
        if #a.bids == 0 then text(PAD + 2, list_y, busy and "Asking..." or "You have no bids", col.muted) end
        local names = {}
        for _, l in ipairs(a.results) do names[l.lot] = l.name end
        for i = 1, ROWS + 4 do
            local b = a.bids[state.scroll + i]
            if b then
                local y = list_y + (i - 1) * LINE
                text(PAD + 2, y, names[b.lot] or ("Lot " .. b.lot), col.text, 200)
                text(PAD + 210, y, b.count .. " at " .. b.price, col.muted, 90)
                text(PAD + 306, y, "held " .. b.escrow, col.muted, 60)
            end
        end
        button(PAD, H - 40, 64, "Refresh", not busy, function() run(function() return eqoa.actions.auction_my_bids() end) end)
    end

    -- the type picker
    if state.picking and state.tab == "search" then
        local pw, ph = 150, 6 * LINE + 8
        local px, py = PAD + 180, 80
        fill(px, py, pw, ph, col.raised)
        frame(px, py, pw, ph, col.white)
        state.pick_rect = { x = x0 + px * k, y = y0 + py * k, w = pw * k, h = ph * k }
        for i = 1, 6 do
            local idx = state.pick_scroll + i
            local y = py + 4 + (i - 1) * LINE
            local w = idx > 1 and WEARS[idx - 1] or nil
            if idx <= #WEARS + 1 then
                text(px + 4, y, idx == 1 and "Any type" or wear_name(w), col.text, pw - 8)
                state.hits[#state.hits + 1] = hit(px, y, pw, LINE, function() state.wear = w; state.picking = false end)
            end
        end
    end

    local line = H - 20
    if state.result then
        text(PAD, line, state.result.text, state.result.ok == false and col.warn or col.muted, W - 2 * PAD)
    elseif a.uncertain then
        text(PAD, line, "The server did not answer the last " .. a.uncertain .. ": look again before trying it again", col.warn, W - 2 * PAD)
    elseif busy then
        text(PAD, line, "Waiting for the server...", col.muted)
    end

    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    ui.SetWindowFontScale(1)
    ui.End()
end
