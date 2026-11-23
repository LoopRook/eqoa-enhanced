-- Crafting: a recipe book and an experiment tray in a window of their own, on their own key (default Shift+R).
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- It sits over play: the character keeps walking, and the game's Start menu > Tradeskills pages are untouched.
-- Nothing here combines by itself: Fill tray puts the recipe's ingredients in the tray, and each click on Combine
-- does one combine. The server's "keep this recipe?" question is answered only by a click on its two buttons.
local ui = eqoa.imgui

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, PAD, LINE = 360, 8, 13
local REC_ROWS, ITEM_ROWS = 8, 16
local TAB_Y, TOP_Y = 20, 40
local TRAY_Y, BTN_Y, NOTE_Y, H = 266, 296, 316, 350
local KINDS = { recipes = "recipe_book", experiment = "experiment" }

local state = {
    open = false, tab = "recipes", sessions = {}, tokens = {}, escape = nil,
    c = nil, scroll = 0, sel = nil, filter = "", typing = false, result = nil, last_req = nil,
    hits = {}, row_hits = {}, list_rect = nil, rect = nil,
}

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then state.tokens[#state.tokens + 1] = token end
end

local function say(text, ok) state.result = { text = text, ok = ok } end

-- Ask the game for something; what it says back arrives as an action.result event.
local function run(label, fn)
    local request, err = fn()
    if err then
        state.last_req = nil
        say(err.message or err.code or "Refused", false)
    elseif request then
        state.last_req = request
        state.result = nil
    end
end

local function on_result(r)
    if not r or not state.open or state.last_req == nil or r.request_id ~= state.last_req then return end
    if r.ok == false then say(r.reason or r.code or "Refused", false) end
end

-- The recipes that match the Find box, in the book's order.
local function visible_recipes()
    local out, f = {}, state.filter:lower()
    for _, r in ipairs(state.c and state.c.recipes or {}) do
        if f == "" or r.name:lower():find(f, 1, true) then out[#out + 1] = r end
    end
    return out
end

-- The tray for a recipe: each ingredient taken from the bag rows that hold it (the game's own tray takes one of an
-- item at a time, never more than the stack, four in all). Nothing is guessed: a missing ingredient says which.
local function picks_for(detail, items)
    local picks = {}
    for _, ing in ipairs(detail.ingredients or {}) do
        local left = math.max(1, ing.need or 1)
        for _, row in ipairs(items or {}) do
            if left == 0 then break end
            if row.name:lower() == ing.name:lower() then
                local take = math.min(left, row.quantity)
                for _ = 1, take do picks[#picks + 1] = { slot = row.slot, id = row.id } end
                left = left - take
            end
        end
        if left > 0 then return nil, "You do not have enough " .. ing.name end
    end
    if #picks == 0 then return nil, "This recipe lists no ingredients" end
    if #picks > 4 then return nil, "This needs more than four items: use the game's own window" end
    return picks
end

local close, draw

local function stop_typing()
    if state.typing then eqoa.input.capture_text(false) end
    state.typing = false
end

local function open_session(kind)
    local s, err = eqoa.windows.open_session(KINDS[kind], { opened_from = "key" })
    if not s then
        -- a refusal says why (the player is using the game's windows, dead, out of the world...)
        eqoa.log.write("warn", "Crafting did not open: " .. tostring(err and err.message))
        return false
    end
    state.sessions[kind] = s.ref
    return true
end

local function switch_tab(tab)
    if tab == state.tab then return end
    -- the new window first, so a crafting window of the mod is open the whole time
    local old = state.sessions[state.tab]
    if not open_session(tab) then return end
    if old then eqoa.windows.close_session(old) end
    state.sessions[state.tab] = nil
    stop_typing()
    state.tab, state.scroll, state.result = tab, 0, nil
end

local function open()
    if state.open then return end
    if not open_session(state.tab) then return end
    state.open, state.scroll, state.result, state.last_req, state.typing = true, 0, nil, nil, false
    listen("render", draw)
    listen("action.result", function(e) on_result(e.data) end)
    -- the client ends the windows when the player zones, dies, uses the game's windows or turns the mod off
    listen("windows.session", function(e)
        local d = e.data
        if d.phase ~= "closed" then return end
        for _, ref in pairs(state.sessions) do
            if ref == d.ref then close(true); return end
        end
    end)
    listen("input.key", function(e)
        local d = e.data
        if not d.down or d["repeat"] and not state.typing then return end
        local key = d.key:lower()
        if state.typing then
            if key == "escape" or key == "enter" or key == "return" then stop_typing()
            elseif key == "backspace" then state.filter = state.filter:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
            elseif d.text and #d.text > 0 and #state.filter < 40 then state.filter = state.filter .. d.text; state.scroll = 0 end
        elseif key == "escape" then
            close()
        end
    end)
    listen("pointer.pressed", function(e)
        local d = e.data
        if d.button ~= "left" then return end
        for _, h in ipairs(state.hits) do
            if inside(d.x, d.y, h) and h.run then h.run(); return end
        end
        if state.typing and not inside(d.x, d.y, state.find_rect) then stop_typing() end
        for _, h in ipairs(state.row_hits) do
            if inside(d.x, d.y, h) then h.run(); return end
        end
    end)
    listen("pointer.wheel", function(e)
        local d = e.data
        if (d.steps or 0) ~= 0 and inside(d.x, d.y, state.list_rect) then
            local rows = state.tab == "recipes" and #visible_recipes() or #(state.c and state.c.items or {})
            local shown = state.tab == "recipes" and REC_ROWS or ITEM_ROWS
            state.scroll = math.max(0, math.min(math.max(0, rows - shown), state.scroll + (d.steps > 0 and -1 or 1)))
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
    if not from_client then
        for _, ref in pairs(state.sessions) do eqoa.windows.close_session(ref) end
    end
    stop_typing()
    state.sessions, state.open = {}, false
    state.hits, state.row_hits, state.rect, state.c = {}, {}, nil, nil
end

eqoa.input.bind({ id = "open", label = "Crafting", default_key = "shift+r" }, function()
    if state.open then close() else open() end
end)
eqoa.events.on("disabling", function() close() end)
eqoa.events.on("world.leaving", function() close() end)

function draw()
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    state.c = eqoa.craft.get()
    local c = state.c
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    state.rect = { x = x0, y = y0, w = W * k, h = H * k }
    if state.typing then eqoa.input.capture_text(true) end

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_crafting_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 11 / 13)
    local dl = ui.GetWindowDrawList()
    local col = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), good = ui.GetColorU32(.35, .8, .4, 1),
        warn = ui.GetColorU32(1, .3, .3, 1), white = ui.GetColorU32(1, 1, 1, 1), hl = ui.GetColorU32(.20, .20, .24, 1),
    }
    local function fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    local function frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    local function text(x, y, value, color, maxw)
        value = tostring(value)
        while maxw and #value > 0 and ui.CalcTextSize(value) > maxw * k do
            value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
        end
        dl:AddText(at(x, y), color or col.text, value)
    end
    -- the words broken into lines that fit `maxw`
    local function wrap(value, maxw)
        local lines, line = {}, ""
        for word in tostring(value):gmatch("%S+") do
            local try = line == "" and word or (line .. " " .. word)
            if line ~= "" and ui.CalcTextSize(try) > maxw * k then lines[#lines + 1] = line; line = word else line = try end
        end
        if line ~= "" then lines[#lines + 1] = line end
        return lines
    end
    local function hit(x, y, w, h, run) return { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k, run = run } end
    local function button(x, y, w, label, enabled, run)
        fill(x, y, w, 16, col.raised)
        frame(x, y, w, 16, enabled and col.border or col.surface)
        text(x + 6, y + 2, label, enabled and col.text or col.muted, w - 8)
        if enabled then state.hits[#state.hits + 1] = hit(x, y, w, 16, run) end
    end

    fill(0, 0, W, H, col.surface)
    frame(0, 0, W, H, col.border)
    text(PAD, 6, "Crafting", col.white)
    text(W - PAD - 100, 6, "Shift+R or Esc", col.muted)
    state.hits, state.row_hits = {}, {}
    fill(W - 18, 4, 12, 12, col.raised)
    frame(W - 18, 4, 12, 12, col.border)
    text(W - 15, 4, "x", col.text)
    state.hits[1] = hit(W - 18, 4, 12, 12, function() close() end)

    if not c then
        text(PAD, TOP_Y, "Opening...", col.muted)
        ui.SetWindowFontScale(1)
        ui.End()
        return
    end
    local waiting = c.pending ~= "none"
    local asking = c.question ~= nil

    -- the two tabs
    for i, tab in ipairs({ "recipes", "experiment" }) do
        local label = tab == "recipes" and "Recipes" or "Experiment"
        local tx = PAD + (i - 1) * 84
        fill(tx, TAB_Y, 80, 16, state.tab == tab and col.hl or col.raised)
        frame(tx, TAB_Y, 80, 16, state.tab == tab and col.white or col.border)
        text(tx + 8, TAB_Y + 2, label, state.tab == tab and col.white or col.muted)
        if not asking then state.hits[#state.hits + 1] = hit(tx, TAB_Y, 80, 16, function() switch_tab(tab) end) end
    end

    state.list_rect = nil
    if state.tab == "recipes" then
        -- the six skills: pick one to read its recipes
        local sw = (W - 2 * PAD) / 6
        for i, s in ipairs(c.skills) do
            local sx = PAD + (i - 1) * sw
            local on = c.listed_skill == s.id
            fill(sx, TOP_Y, sw - 2, 26, on and col.hl or col.raised)
            frame(sx, TOP_Y, sw - 2, 26, on and col.white or col.border)
            text(sx + 2, TOP_Y + 1, s.name, col.text, sw - 6)
            text(sx + 2, TOP_Y + 13, "Lv " .. s.level .. " Rk " .. s.rank, col.muted, sw - 6)
            if not asking and not waiting then
                state.hits[#state.hits + 1] = hit(sx, TOP_Y, sw - 2, 26, function()
                    state.sel, state.scroll = nil, 0
                    run("List", function() return eqoa.actions.craft_list(s.id) end)
                end)
            end
        end
        -- Find
        local fy = TOP_Y + 30
        fill(PAD, fy, W - 2 * PAD, 14, col.raised)
        frame(PAD, fy, W - 2 * PAD, 14, state.typing and col.white or col.border)
        text(PAD + 4, fy + 1, state.filter == "" and not state.typing and "Find a recipe (click, then type)"
            or (state.filter .. (state.typing and "_" or "")), state.filter == "" and col.muted or col.text, W - 2 * PAD - 8)
        state.find_rect = { x = x0 + PAD * k, y = y0 + fy * k, w = (W - 2 * PAD) * k, h = 14 * k }
        if not asking then
            state.hits[#state.hits + 1] = hit(PAD, fy, W - 2 * PAD, 14, function() state.typing = true end)
        end
        -- the recipes
        local list_y = fy + 18
        local list = visible_recipes()
        state.list_rect = { x = x0 + PAD * k, y = y0 + list_y * k, w = (W - 2 * PAD) * k, h = REC_ROWS * LINE * k }
        if #list == 0 then
            text(PAD + 2, list_y, c.listed_skill == nil and "Choose a skill to read its recipes"
                or (waiting and "Waiting for the server..." or "No recipes"), col.muted)
        end
        for i = 1, REC_ROWS do
            local r = list[state.scroll + i]
            if r then
                local y = list_y + (i - 1) * LINE
                if state.sel == r.id then fill(PAD, y, W - 2 * PAD, LINE, col.hl) end
                text(PAD + 2, y, r.name, col.text, 190)
                text(PAD + 196, y, r.difficulty_name, col.muted)
                text(PAD + 226, y, "Lv " .. r.level, col.muted)
                text(PAD + 262, y, r.component_name, col.muted, 80)
                if not asking and not waiting then
                    state.row_hits[#state.row_hits + 1] = hit(PAD, y, W - 2 * PAD, LINE, function()
                        state.sel = r.id
                        run("Inspect", function() return eqoa.actions.craft_inspect(r.id) end)
                    end)
                end
            end
        end
        -- the chosen recipe
        local dy = list_y + REC_ROWS * LINE + 4
        local d = c.detail
        if state.sel and d and d.recipe == state.sel then
            text(PAD, dy, d.name, col.white, W - 2 * PAD)
            text(PAD, dy + LINE, "Makes " .. d.count .. "   Item level " .. d.level, col.muted, W - 2 * PAD)
            local parts = {}
            for _, ing in ipairs(d.ingredients) do
                parts[#parts + 1] = { text = ing.name .. (ing.need > 1 and (" x" .. ing.need) or "") .. " (have " .. ing.have .. ")",
                    ok = ing.have >= math.max(1, ing.need) }
            end
            local px = PAD
            for _, p in ipairs(parts) do
                local w = ui.CalcTextSize(p.text) / k + 10
                if px + w > W - PAD then px = PAD; dy = dy + LINE end
                text(px, dy + 2 * LINE, p.text, p.ok and col.good or col.warn)
                px = px + w
            end
            local tools = {}
            for _, t in ipairs(d.tools) do tools[#tools + 1] = t.name .. (t.have and "" or " (missing)") end
            if #tools > 0 then text(PAD, dy + 3 * LINE, "Tools: " .. table.concat(tools, ", "), col.muted, W - 2 * PAD) end
        elseif state.sel then
            text(PAD, dy, waiting and "Waiting for the server..." or "Choose the recipe again to read it", col.muted)
        else
            text(PAD, dy, "Choose a recipe to see what it needs", col.muted)
        end
    else
        -- the experiment: components in the bags, put into the tray with a click
        local items = c.items
        state.list_rect = { x = x0 + PAD * k, y = y0 + TOP_Y * k, w = (W - 2 * PAD) * k, h = ITEM_ROWS * LINE * k }
        if #items == 0 then text(PAD + 2, TOP_Y, "No crafting components in your bags", col.muted) end
        for i = 1, ITEM_ROWS do
            local it = items[state.scroll + i]
            if it then
                local y = TOP_Y + (i - 1) * LINE
                if it.in_tray > 0 then fill(PAD, y, W - 2 * PAD, LINE, col.hl) end
                text(PAD + 2, y, it.name, col.text, 220)
                text(PAD + 230, y, "x" .. it.quantity, col.muted)
                if it.in_tray > 0 then text(PAD + 270, y, it.in_tray .. " in tray", col.good) end
                if not asking then
                    state.row_hits[#state.row_hits + 1] = hit(PAD, y, W - 2 * PAD, LINE, function()
                        run("Add", function() return eqoa.actions.craft_tray_add({ slot = it.slot, id = it.id }) end)
                    end)
                end
            end
        end
        text(PAD, TOP_Y + ITEM_ROWS * LINE + 2, "Click an item to put it in the tray (up to four); click a tray slot to take it out.", col.muted, W - 2 * PAD)
    end

    -- the tray, the same in both tabs: four slots, a click on one takes it out again
    text(PAD, TRAY_Y - 11, "Tray", col.muted)
    local tw = (W - 2 * PAD - 6) / 4
    for i = 1, 4 do
        local t = c.tray[i]
        local tx = PAD + (i - 1) * (tw + 2)
        fill(tx, TRAY_Y, tw, 26, col.raised)
        frame(tx, TRAY_Y, tw, 26, t and col.white or col.border)
        if t then
            text(tx + 3, TRAY_Y + 7, t.name, col.text, tw - 6)
            if not asking and not waiting then
                state.hits[#state.hits + 1] = hit(tx, TRAY_Y, tw, 26, function()
                    run("Remove", function() return eqoa.actions.craft_tray_remove({ slot = t.slot, id = t.id }) end)
                end)
            end
        end
    end
    local bx = PAD
    if state.tab == "recipes" then
        local d = c.detail
        local can_fill = not asking and not waiting and d ~= nil and state.sel ~= nil and d.recipe == state.sel
        button(bx, BTN_Y, 70, "Fill tray", can_fill, function()
            local picks, why = picks_for(d, c.items)
            if not picks then say(why, false); return end
            run("Fill", function() return eqoa.actions.craft_fill(picks) end)
        end)
        bx = bx + 74
    end
    button(bx, BTN_Y, 64, "Combine", not asking and not waiting and #c.tray > 0, function()
        run("Combine", function() return eqoa.actions.craft_combine() end)
    end)
    button(bx + 68, BTN_Y, 80, "Empty tray", not asking and #c.tray > 0, function()
        run("Empty", function() return eqoa.actions.craft_tray_clear() end)
    end)
    if waiting then text(W - PAD - 120, BTN_Y + 2, "Waiting for the server...", col.muted, 120) end

    -- what the server last said, or why a click was refused
    local line = NOTE_Y
    if state.result then
        text(PAD, line, state.result.text, state.result.ok == false and col.warn or col.muted, W - 2 * PAD)
        line = line + LINE
    end
    if c.notice then
        for _, l in ipairs(wrap(c.notice.text, W - 2 * PAD)) do
            if line < H - LINE then text(PAD, line, l, col.text); line = line + LINE end
        end
    end

    -- the server's question after a combine found a recipe: only a click on a button answers it
    if asking then
        state.hits, state.row_hits = { state.hits[1] }, {}
        local qy = 110
        fill(PAD, qy, W - 2 * PAD, 96, col.raised)
        frame(PAD, qy, W - 2 * PAD, 96, col.white)
        local y = qy + 8
        for _, l in ipairs(wrap(c.question.text, W - 2 * PAD - 16)) do
            if y < qy + 64 then text(PAD + 8, y, l, col.text); y = y + LINE end
        end
        button(PAD + 8, qy + 72, 80, "Keep recipe", not waiting, function()
            run("Keep", function() return eqoa.actions.craft_keep(true) end)
        end)
        button(PAD + 96, qy + 72, 80, "Leave it", true, function()
            run("Leave", function() return eqoa.actions.craft_keep(false) end)
        end)
    end

    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    ui.SetWindowFontScale(1)
    ui.End()
end
