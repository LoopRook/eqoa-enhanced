-- Quest Journal: your quests in a window of their own, on its own key (default Shift+J).
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- It sits over play: the character keeps walking, and the game's Start menu > Quests page is untouched.
-- Press the key again, press Escape or click the X to close it.
--
-- Dropping a quest never happens at once. The Drop button asks the game to drop it; the client then puts its own
-- "Drop the quest ...?" question to you, starting on No, and only your own Yes drops it. This window cannot answer
-- that question for you.
local ui = eqoa.imgui

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, PAD, LINE = 332, 8, 13
local state = {
    open = false, session = nil, tokens = {}, escape = nil, selected = nil, result = nil,
    hits = {}, row_hits = {}, last_req = nil,
    first = 1,          -- the first quest row shown (rows are as tall as their wrapped text, so the list scrolls by rows)
    list_rect = nil,    -- the list's box in screen pixels, for the wheel
}
local LIST_H = 8 * LINE -- the list's height; the goals pane below holds as many lines

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end
local GATED = { render = true, ["input.key"] = true, ["pointer.pressed"] = true, ["pointer.wheel"] = true, ["windows.key"] = true, ["input.pad"] = true }
local function listen(name, fn)
    local inner = fn
    if GATED[name] then inner = function(e) if map_up() then return end return fn(e) end end
    local token = eqoa.events.on(name, inner)
    if token then state.tokens[#state.tokens + 1] = token end
end

-- `text` wrapped to `maxw` HUD pixels (modules/wrap.lua: the Bag Viewer tooltip's rule). `k` is the HUD scale (ImGui measures
-- in screen pixels).
local wrapper = require("wrap")
local function wrap(text, maxw, k)
    return wrapper.wrap(text, maxw, function(t) return ui.CalcTextSize(t) / k end)
end

local function say(text, ok) state.result = { text = text, ok = ok } end

local function goals_for(title)
    local ok, all = pcall(eqoa.quests.goals)
    if not ok or type(all) ~= "table" then return nil end
    for _, g in ipairs(all) do
        if g.title == title then return g.goals end
    end
end

local function run(label, fn)
    local request, err = fn()
    if err then
        state.last_req = nil
        say(err.message or err.code or "Refused", false)
    elseif request then
        state.last_req = request
        say(label, nil)
    end
end

local close
local draw

local function pointer_pressed(d)
    if d.button ~= "left" then return end
    for _, h in ipairs(state.hits) do
        if inside(d.x, d.y, h) and h.run then h.run(); return end
    end
    for _, h in ipairs(state.row_hits) do
        if inside(d.x, d.y, h) then
            state.selected = h.quest.title
            eqoa.log.write("debug", "Quest Journal: selected " .. tostring(h.quest.title))
            return
        end
    end
end

local function open(opened_from)
    if state.open then return end
    local s, err = eqoa.windows.open_session("quest_log", { opened_from = opened_from or "key" })
    if not s then
        eqoa.log.write("info", "Quest Journal did not open: " .. tostring(err and err.message))
        return
    end
    state.session, state.open, state.result, state.last_req = s, true, nil, nil
    listen("render", draw)
    listen("action.result", function(e)
        local d = e.data
        if state.last_req ~= nil and d.request_id == state.last_req then
            if d.ok == true then
                -- the question is the client's: it is now up, and nothing is dropped until you answer Yes
                say("Answer the question on screen", nil)
            elseif d.ok == false then
                say(d.reason or d.code or "Refused", false)
            end
        end
    end)
    -- what you answered: yes (and whether it was sent), no, or why it was cancelled
    listen("confirm.closed", function(e)
        local d = e.data
        if d.kind ~= "drop_quest" then return end
        if d.answer == "yes" then
            local sent = d.detail and d.detail.sent
            say(sent and "Quest dropped" or ("Not dropped: " .. tostring(d.detail and d.detail.reason)), sent == true)
        elseif d.answer == "no" then
            say("Kept the quest", true)
        else
            say("Kept the quest (" .. tostring(d.answer) .. ")", true)
        end
    end)
    -- the client ends the window's session when the player zones, dies, uses the game's windows or turns the mod off
    listen("windows.session", function(e)
        local d = e.data
        if d.phase == "closed" and state.session and d.ref == state.session.ref then close(true) end
    end)
    listen("input.key", function(e)
        if e.data.down and not e.data["repeat"] and e.data.key:lower() == "escape" then close() end
    end)
    listen("pointer.pressed", function(e) pointer_pressed(e.data) end)
    listen("pointer.wheel", function(e)
        local d = e.data
        if not inside(d.x, d.y, state.list_rect) then return end
        state.first = math.max(1, state.first + (d.steps > 0 and -1 or 1))
    end)
    -- Escape is the window's, not the game menu's, while it is up
    state.escape = eqoa.input.claim("Escape", true)
end

function close(from_client)
    if not state.open then return end
    for _, token in ipairs(state.tokens) do eqoa.events.off(token) end
    state.tokens = {}
    if state.escape then eqoa.input.release(state.escape); state.escape = nil end
    if state.session and not from_client then eqoa.windows.close_session(state.session.ref) end
    state.open, state.session, state.hits, state.row_hits = false, nil, {}, {}
end

eqoa.input.bind({ id = "open", label = "Quest Journal", default_key = "shift+j" }, function()
    if state.open then close() else open("key") end
end)
eqoa.events.on("mod.command", function(e)
    if e.data.command == "open" then open("mod") end
end)
eqoa.events.on("disabling", function() close() end)
eqoa.events.on("world.leaving", function() close() end)

function draw()
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    local quests = eqoa.quests.list() or {}
    local H = 24 + 8 * LINE + 12 + 8 * LINE + 26 + PAD
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_quest_journal_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 11 / 13)
    local dl = ui.GetWindowDrawList()
    local c = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), red = ui.GetColorU32(.843, .098, .129, 1),
        good = ui.GetColorU32(.35, .8, .4, 1), warn = ui.GetColorU32(1, .3, .3, 1),
        white = ui.GetColorU32(1, 1, 1, 1), hl = ui.GetColorU32(.20, .20, .24, 1),
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
    local function hit(x, y, w, h, extra)
        local r = { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k }
        for key, v in pairs(extra or {}) do r[key] = v end
        return r
    end

    fill(0, 0, W, H, c.surface)
    frame(0, 0, W, H, c.border)
    text(PAD, 6, "Quest Journal", c.white)
    text(W - PAD - 100, 6, "Shift+J or Esc", c.muted)
    state.hits, state.row_hits = {}, {}
    fill(W - 18, 4, 12, 12, c.raised)
    frame(W - 18, 4, 12, 12, c.border)
    text(W - 15, 4, "x", c.text)
    state.hits[1] = hit(W - 18, 4, 12, 12, { run = function() close() end })

    local list_y = 24
    local textw = W - 2 * PAD - 4
    if #quests == 0 then text(PAD, list_y, "No quests in your journal", c.muted) end
    state.first = math.max(1, math.min(state.first, math.max(1, #quests)))
    local chosen
    for _, q in ipairs(quests) do
        if state.selected == q.title then chosen = q end
    end
    -- each quest takes as many lines as its words need; the rows that fit the list's height are drawn, from `first`
    local y = 0
    local last_shown = state.first - 1
    for i = state.first, #quests do
        local q = quests[i]
        local lines = wrap(q.title, textw, k)
        local h = #lines * LINE
        if y + h > LIST_H and i > state.first then break end
        local shown = math.min(#lines, math.floor((LIST_H - y) / LINE))
        h = shown * LINE
        if state.selected == q.title then fill(PAD, list_y + y, W - 2 * PAD, h, c.hl) end
        for n = 1, shown do text(PAD + 2, list_y + y + (n - 1) * LINE, lines[n], c.text) end
        state.row_hits[#state.row_hits + 1] = hit(PAD, list_y + y, W - 2 * PAD, h, { quest = q })
        y = y + h
        last_shown = i
    end
    state.list_rect = hit(PAD, list_y, W - 2 * PAD, LIST_H)
    if state.first > 1 then text(W - PAD - 8, list_y, "^", c.muted) end
    if last_shown < #quests then text(W - PAD - 8, list_y + LIST_H - LINE, "v", c.muted) end

    -- the chosen quest's goals, where the server sends the tracker; every line wrapped, as many as the pane holds
    local detail_y = list_y + LIST_H + 12
    fill(PAD, detail_y - 6, W - 2 * PAD, 1, c.border)
    if chosen then
        local pane = {}
        for _, l in ipairs(wrap(chosen.title, W - 2 * PAD, k)) do pane[#pane + 1] = { l, c.white, 0 } end
        local goals = goals_for(chosen.title)
        if goals and #goals > 0 then
            for i = 1, #goals do
                local g = goals[i]
                local line = g.text
                if (g.need or 0) > 0 then line = string.format("%s  %d/%d", g.text, g.have or 0, g.need) end
                for _, l in ipairs(wrap(line, W - 2 * PAD - 8, k)) do pane[#pane + 1] = { l, g.done and c.good or c.text, 4 } end
            end
        else
            pane[#pane + 1] = { "No goals sent for this quest", c.muted, 4 }
        end
        if #pane > 8 then
            for n = #pane, 8, -1 do pane[n] = nil end
            pane[8] = { "...", c.muted, 4 }
        end
        for n, l in ipairs(pane) do text(PAD + l[3], detail_y + (n - 1) * LINE, l[1], l[2]) end
    else
        text(PAD, detail_y, "Click a quest to see its goals", c.muted)
    end

    local bar_y = detail_y + 8 * LINE + 4
    if state.result then
        text(PAD + 70, bar_y + 2, state.result.text, state.result.ok == false and c.warn or c.muted, W - 2 * PAD - 70)
    end
    local label = "Drop quest"
    local bw = ui.CalcTextSize(label) / k + 12
    fill(PAD, bar_y, bw, 16, c.raised)
    frame(PAD, bar_y, bw, 16, chosen and c.red or c.surface)
    text(PAD + 6, bar_y + 2, label, chosen and c.text or c.muted)
    if chosen then
        state.hits[#state.hits + 1] = hit(PAD, bar_y, bw, 16, { run = function()
            run("Asking", function() return eqoa.actions.drop_quest(chosen.ref) end)
        end })
    end

    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    ui.SetWindowFontScale(1)
    ui.End()
end
