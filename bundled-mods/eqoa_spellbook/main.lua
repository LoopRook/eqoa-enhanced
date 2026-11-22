-- Spellbook: the character's abilities in a window of their own, on its own key (default Shift+K).
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- It sits over play: the character keeps walking, and the game's Start menu > Abilities page is untouched.
-- Press the key again, press Escape or click the X to close it.
local ui = eqoa.imgui

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, PAD, LINE, ROWS, SLOT_W = 332, 8, 13, 13, 30
local state = {
    open = false, session = nil, tokens = {}, escape = nil,
    list = nil, scroll = 0, selected = nil, result = nil,
    hits = {}, row_hits = {}, slot_hits = {}, rect = nil,
}

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then state.tokens[#state.tokens + 1] = token end
end

local function abilities()
    if state.list == nil then
        local list = eqoa.abilities.list()
        table.sort(list, function(a, b)
            if (a.level or 0) ~= (b.level or 0) then return (a.level or 0) < (b.level or 0) end
            return (a.name or "") < (b.name or "")
        end)
        state.list = list
    end
    return state.list
end

local function by_id(id)
    for _, a in ipairs(abilities()) do if a.id == id then return a end end
end

local function by_slot(n)
    for _, a in ipairs(abilities()) do if a.spellbook_slot == n then return a end end
end

local function say(text, ok) state.result = { text = text, ok = ok } end

-- What the game said about an action this window asked for.
local function on_result(r)
    if not r or not state.open or state.last_req == nil or r.request_id ~= state.last_req then return end
    if r.ok == true then
        say("Done", true)
    elseif r.ok == false then
        say((r.reason or r.code or "Refused"), false)
    end
end

local function run(label, fn)
    local request, err = fn()
    if err then
        state.last_req = nil
        say(err.message or err.code or "Refused", false)
    elseif request then
        state.last_req = request
        say(label .. " sent", nil)
    end
end

local close

local function pointer_pressed(d)
    if d.button ~= "left" and d.button ~= "right" then return end
    for _, h in ipairs(state.hits) do
        if d.button == "left" and inside(d.x, d.y, h) and h.run then h.run(); return end
    end
    for _, h in ipairs(state.slot_hits) do
        if inside(d.x, d.y, h) then
            if d.button == "right" then
                if h.ability then
                    run("Unmemorize", function() return eqoa.actions.unmemorize_ability(h.slot) end)
                end
            elseif state.selected and by_id(state.selected) then
                local a = by_id(state.selected)
                run("Memorize", function() return eqoa.actions.assign_ability(h.slot, a.ref) end)
            elseif h.ability then
                state.selected = h.ability.id
            end
            return
        end
    end
    for _, h in ipairs(state.row_hits) do
        if d.button == "left" and inside(d.x, d.y, h) then state.selected = h.ability.id; return end
    end
end

local draw

local function open(opened_from)
    if state.open then return end
    local s, err = eqoa.windows.open_session("spellbook", { opened_from = opened_from or "key" })
    if not s then
        -- a refusal says why (the player is using the game's windows, dead, out of the world...)
        eqoa.log.write("Spellbook did not open: " .. tostring(err and err.message))
        return
    end
    state.session, state.open, state.list, state.scroll, state.result = s, true, nil, 0, nil
    state.last_req = nil
    listen("render", draw)
    listen("abilities.changed", function() state.list = nil end)
    listen("hotbar.changed", function() state.list = nil end)
    listen("state.reset", function() state.list = nil end)
    listen("action.result", function(e) on_result(e.data) end)
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
        if (d.steps or 0) ~= 0 and inside(d.x, d.y, state.list_rect) then
            local most = math.max(0, #abilities() - ROWS)
            state.scroll = math.max(0, math.min(most, state.scroll + (d.steps > 0 and -1 or 1)))
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
    if state.session and not from_client then eqoa.windows.close_session(state.session.ref) end
    state.open, state.session, state.hits, state.row_hits, state.slot_hits, state.rect = false, nil, {}, {}, {}, nil
end

eqoa.input.bind({ id = "open", label = "Spellbook", default_key = "shift+k" }, function()
    if state.open then close() else open("key") end
end)
eqoa.events.on("disabling", function() close() end)
eqoa.events.on("world.leaving", function() close() end)

function draw()
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    local list = abilities()
    local sel = state.selected and by_id(state.selected)
    local list_y = 24
    local list_h = ROWS * LINE
    local slots_y = list_y + list_h + 8
    local detail_y = slots_y + 2 * (SLOT_W + 2) + 8
    local detail_h = 5 * LINE
    local bar_y = detail_y + detail_h + 4
    local H = bar_y + 20 + PAD
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    state.rect = { x = x0, y = y0, w = W * k, h = H * k }

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_spellbook_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
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
    text(PAD, 6, "Spellbook", c.white)
    text(W - PAD - 70, 6, "Shift+K or Esc", c.muted)
    -- the X
    state.hits = {}
    fill(W - 18, 4, 12, 12, c.raised)
    frame(W - 18, 4, 12, 12, c.border)
    text(W - 15, 4, "x", c.text)
    state.hits[1] = hit(W - 18, 4, 12, 12, { run = function() close() end })

    -- the abilities
    state.row_hits = {}
    state.list_rect = { x = x0 + PAD * k, y = y0 + list_y * k, w = (W - 2 * PAD) * k, h = list_h * k }
    local p = eqoa.input.pointer()
    if #list == 0 then text(PAD, list_y, "No abilities known yet", c.muted) end
    for i = 1, ROWS do
        local a = list[state.scroll + i]
        if a then
            local y = list_y + (i - 1) * LINE
            local on = state.selected == a.id
            if on then fill(PAD, y, W - 2 * PAD, LINE, c.hl) end
            local cd = eqoa.cooldowns.get(a.ref)
            local color = (not a.usable) and c.muted or ((cd and cd.active) and c.warn or c.text)
            text(PAD + 2, y, a.name, color, 190)
            text(PAD + 196, y, "Lv " .. tostring(a.level or 0), c.muted)
            text(PAD + 236, y, tostring(a.power_cost or 0) .. " pw", c.muted)
            if a.spellbook_slot then text(W - PAD - 24, y, "[" .. a.spellbook_slot .. "]", c.good) end
            state.row_hits[#state.row_hits + 1] = hit(PAD, y, W - 2 * PAD, LINE, { ability = a })
        end
    end
    if #list > ROWS then
        local sb_x, thumb_h = W - PAD + 1, math.max(10, list_h * ROWS / #list)
        fill(sb_x, list_y, 2, list_h, c.border)
        fill(sb_x, list_y + (list_h - thumb_h) * (state.scroll / (#list - ROWS)), 2, thumb_h, c.muted)
    end

    -- the 20 memorized slots: click one with an ability chosen to put it there; right-click to clear it
    state.slot_hits = {}
    text(PAD, slots_y - 11, "Memorized (click with an ability chosen to memorize; right-click to clear)", c.muted, W - 2 * PAD)
    local per_row = 10
    for slot = 1, 20 do
        local col, row = (slot - 1) % per_row, math.floor((slot - 1) / per_row)
        local sx, sy = PAD + col * (SLOT_W + 2), slots_y + row * (SLOT_W + 2)
        local a = by_slot(slot)
        fill(sx, sy, SLOT_W, SLOT_W, c.raised)
        frame(sx, sy, SLOT_W, SLOT_W, (a and state.selected == a.id) and c.white or c.border)
        text(sx + 2, sy + 1, tostring(slot), c.muted)
        if a then text(sx + 2, sy + 14, a.name:sub(1, 5), c.text, SLOT_W - 3) end
        state.slot_hits[#state.slot_hits + 1] = hit(sx, sy, SLOT_W, SLOT_W, { slot = slot, ability = a })
    end

    -- the chosen ability
    if sel then
        text(PAD, detail_y, sel.name, c.white, W - 2 * PAD)
        text(PAD, detail_y + LINE, string.format("Level %d   Power %d   Cast %.1fs   Range %d", sel.level or 0,
            sel.power_cost or 0, sel.cast_time or 0, math.floor(sel.range or 0)), c.text, W - 2 * PAD)
        local desc = tostring(sel.description or "")
        for n = 0, 2 do
            local part = desc:sub(n * 58 + 1, (n + 1) * 58)
            if #part > 0 then text(PAD, detail_y + (2 + n) * LINE, part, c.muted, W - 2 * PAD) end
        end
    else
        text(PAD, detail_y, "Choose an ability to see it here", c.muted)
    end
    if state.result then
        text(W - PAD - 150, detail_y, state.result.text, state.result.ok == false and c.warn or c.muted, 150)
    end

    -- the buttons
    local bx = PAD
    local function button(label, enabled, run)
        local bw = ui.CalcTextSize(label) / k + 12
        fill(bx, bar_y, bw, 16, c.raised)
        frame(bx, bar_y, bw, 16, enabled and c.border or c.surface)
        text(bx + 6, bar_y + 2, label, enabled and c.text or c.muted)
        if enabled then state.hits[#state.hits + 1] = hit(bx, bar_y, bw, 16, { run = run }) end
        bx = bx + bw + 4
    end
    button("Cast", sel ~= nil and sel.spellbook_slot ~= nil, function()
        run("Cast", function() return eqoa.actions.cast(sel.ref) end)
    end)
    button("Unmemorize", sel ~= nil and sel.spellbook_slot ~= nil, function()
        run("Unmemorize", function() return eqoa.actions.unmemorize_ability(sel.ref) end)
    end)
    button("Unmemorize all", true, function()
        run("Unmemorize all", function() return eqoa.actions.unmemorize_all() end)
    end)

    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    ui.SetWindowFontScale(1)
    ui.End()
end
