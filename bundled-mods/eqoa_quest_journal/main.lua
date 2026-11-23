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
}

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then state.tokens[#state.tokens + 1] = token end
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
        if inside(d.x, d.y, h) then state.selected = h.quest.title; return end
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
    if #quests == 0 then text(PAD, list_y, "No quests in your journal", c.muted) end
    local chosen
    for i, q in ipairs(quests) do
        local y = list_y + (i - 1) * LINE
        local on = state.selected == q.title
        if on then fill(PAD, y, W - 2 * PAD, LINE, c.hl); chosen = q end
        text(PAD + 2, y, q.title, c.text, W - 2 * PAD - 4)
        state.row_hits[#state.row_hits + 1] = hit(PAD, y, W - 2 * PAD, LINE, { quest = q })
    end

    -- the chosen quest's goals, where the server sends the tracker
    local detail_y = list_y + 8 * LINE + 12
    fill(PAD, detail_y - 6, W - 2 * PAD, 1, c.border)
    if chosen then
        text(PAD, detail_y, chosen.title, c.white, W - 2 * PAD)
        local goals = goals_for(chosen.title)
        if goals and #goals > 0 then
            for i = 1, math.min(6, #goals) do
                local g = goals[i]
                local line = g.text
                if (g.need or 0) > 0 then line = string.format("%s  %d/%d", g.text, g.have or 0, g.need) end
                text(PAD + 4, detail_y + i * LINE, line, g.done and c.good or c.text, W - 2 * PAD - 8)
            end
        else
            text(PAD + 4, detail_y + LINE, "No goals sent for this quest", c.muted)
        end
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
