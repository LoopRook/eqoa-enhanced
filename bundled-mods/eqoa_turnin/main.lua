-- Quest turn-in helper: holds NPC dialogs, and when one of a dialog's answers names a quest in your journal it
-- shows a small "Hand in" box. Every dialog it does not recognise goes straight back to the game's own window.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local ui = eqoa.imgui
local match = require("match")

local KINDS = { "response_dialog", "text_dialog" }
local W, PAD, LINE = 300, 8, 13
local RECLAIM_FRAMES, RETRY_FRAMES = 120, 30

eqoa.options.toggle({ id = "auto_answer", label = "Answer quest hand-ins automatically", default = false })

local state = {
    held = {},        -- kind -> true while the claim is ours
    tell = {},        -- kind -> last claim outcome told in the log
    frame = 0,
    seen = nil,       -- ref of the dialog already judged
    current = nil,    -- { ref, text, matches } while the box is up
    retry_at = 0, hits = {}, result = nil, escape = nil,
}

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function note(level, text) eqoa.log.write(level, "Quest turn-in: " .. text) end

-- Holds the dialog kinds. Asking again while we hold one just keeps it; a claim lost (zoning, the player's
-- "Use the game's windows" switch) or refused (another mod holds it: `busy`) is tried again every so often.
local function claim()
    for _, kind in ipairs(KINDS) do
        local ok, err = eqoa.windows.takeover(kind, { scope = "all" })
        local told = ok and "held" or (err and err.code or "refused")
        if state.tell[kind] ~= told then
            state.tell[kind] = told
            if ok then
                note("info", "holding " .. kind)
            else
                note("warn", "cannot hold " .. kind .. ": " .. tostring(err and err.message or "refused"))
            end
        end
        state.held[kind] = ok and true or false
    end
end

local function say(text, ok) state.result = { text = text, ok = ok, at = state.frame } end

local function answer(c, m)
    local request, err = eqoa.actions.dialog_answer(c.ref, m.index)
    if err then
        say(err.message or err.code or "Refused", false)
        note("warn", "the answer was refused: " .. tostring(err.code))
        return false
    end
    say("Handed in: " .. tostring(m.quest.title) .. " (sent)", nil)
    return request ~= nil
end

local function pass(ref)
    local ok, err = eqoa.windows.pass(ref)
    if not ok then
        note("debug", "could not give the dialog back yet: " .. tostring(err and err.code))
    end
    return ok
end

-- The dialog now up, judged once: no quest named -> the game's window at once.
local function judge(s)
    if state.seen == s.ref then
        -- a pass that was refused (another window was up) is tried again
        if not state.current and state.frame >= state.retry_at then
            state.retry_at = state.frame + RETRY_FRAMES
            pass(s.ref)
        end
        return
    end
    state.seen, state.current, state.hits, state.retry_at = s.ref, nil, {}, state.frame + RETRY_FRAMES
    local data = s.data or {}
    if s.kind ~= "response_dialog" then
        pass(s.ref)
        return
    end
    local found = match.matches(data.options, eqoa.quests.list() or {})
    if #found == 0 then
        pass(s.ref)
        return
    end
    state.current = { ref = s.ref, text = tostring(data.text or ""), matches = found }
    note("info", "a dialog names " .. tostring(found[1].quest.title))
    -- only when exactly one answer names a quest: two matches are never guessed between
    if #found == 1 and eqoa.options.get("auto_answer") then
        answer(state.current, found[1])
    end
end

local function give_up_box(pass_it)
    local c = state.current
    state.current, state.hits = nil, {}
    if c and pass_it then pass(c.ref) end
end

eqoa.events.on("windows.session", function(e)
    local d = e.data
    if d.phase == "closed" then
        if state.current and d.ref == state.current.ref then
            if d.reason == "answered" then say("Sent", true) end
            state.current, state.hits = nil, {}
        end
        if state.seen == d.ref then state.seen = nil end
    end
end)
eqoa.events.on("input.key", function(e)
    if state.current and e.data.down and not e.data["repeat"] and e.data.key:lower() == "escape" then
        give_up_box(true)
    end
end)
eqoa.events.on("pointer.pressed", function(e)
    local d = e.data
    if d.button ~= "left" then return end
    for _, h in ipairs(state.hits) do
        if inside(d.x, d.y, h) and h.run then h.run(); return end
    end
end)
eqoa.events.on("state.reset", function() state.seen, state.current, state.hits = nil, nil, {}; claim() end)
-- the host lets go of every claim when the world is left: held again as soon as the new one is up
eqoa.events.on("world.entered", function() claim() end)
eqoa.events.on("disabling", function()
    -- the host gives any held dialog back to the game's window; nothing to do but let go
    if state.escape then eqoa.input.release(state.escape); state.escape = nil end
    eqoa.windows.release()
end)

local function wrap(text, maxw, k)
    local lines = {}
    for para in (tostring(text) .. "\n"):gmatch("(.-)\n") do
        local line = ""
        for word in para:gmatch("%S+") do
            local try = line == "" and word or (line .. " " .. word)
            if line ~= "" and ui.CalcTextSize(try) / k > maxw then
                lines[#lines + 1] = line
                line = word
            else
                line = try
            end
        end
        lines[#lines + 1] = line
    end
    return lines
end

local function draw_box(c)
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_turnin_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 11 / 13)
    local lines = wrap(c.text, W - 2 * PAD, k)
    local shown = math.min(#lines, 4)
    local H = 22 + shown * LINE + 6 + #c.matches * 20 + 20 + PAD
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + 60 * f.scale_y))
    local dl = ui.GetWindowDrawList()
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    local col = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), good = ui.GetColorU32(.35, .8, .4, 1),
        warn = ui.GetColorU32(1, .3, .3, 1), white = ui.GetColorU32(1, 1, 1, 1),
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
    local function hit(x, y, w, h, run)
        return { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k, run = run }
    end
    fill(0, 0, W, H, col.surface)
    frame(0, 0, W, H, col.border)
    text(PAD, 5, "Quest hand-in", col.white)
    text(W - PAD - 100, 5, "Esc: game window", col.muted, 100)
    for i = 1, shown do text(PAD, 20 + (i - 1) * LINE, lines[i], col.muted, W - 2 * PAD) end
    local y = 20 + shown * LINE + 6
    state.hits = {}
    for _, m in ipairs(c.matches) do
        fill(PAD, y, W - 2 * PAD, 17, col.raised)
        frame(PAD, y, W - 2 * PAD, 17, col.good)
        text(PAD + 6, y + 2, "Hand in: " .. tostring(m.quest.title), col.text, W - 2 * PAD - 12)
        state.hits[#state.hits + 1] = hit(PAD, y, W - 2 * PAD, 17, function() answer(c, m) end)
        y = y + 20
    end
    fill(PAD, y, 120, 17, col.raised)
    frame(PAD, y, 120, 17, col.border)
    text(PAD + 6, y + 2, "Not now (game window)", col.text, 110)
    state.hits[#state.hits + 1] = hit(PAD, y, 120, 17, function() give_up_box(true) end)
    if state.result and state.frame - state.result.at < 300 then
        text(PAD + 128, y + 2, state.result.text, state.result.ok == false and col.warn or col.muted, W - 2 * PAD - 128)
    end
    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    ui.SetWindowFontScale(1)
    ui.End()
end

eqoa.events.on("render", function()
    state.frame = state.frame + 1
    if state.frame == 1 or state.frame % RECLAIM_FRAMES == 0 then claim() end
    -- reading the session each frame is what tells the host this mod is still looking at it
    for _, kind in ipairs(KINDS) do
        local s = eqoa.windows.session(kind)
        if s then judge(s) end
    end
    local c = state.current
    if c then
        if not state.escape then state.escape = eqoa.input.claim("Escape", true) end
        draw_box(c)
    elseif state.escape then
        eqoa.input.release(state.escape)
        state.escape = nil
    end
end)
