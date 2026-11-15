-- Bag viewer: a read-only Bags window over the public eqoa items API.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local ui = eqoa.imgui
local B = require("bags")

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, PAD, SLOT, GAP, LINE, LOG_LINES = 286, 8, 32, 2, 12, 6
local tokens, escape, tab_hits, log_rect, last = {}, nil, {}, nil, nil

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

-- The window stops listening to everything but loot while it is closed.
local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then tokens[#tokens + 1] = token end
end

local draw
local function close()
    if not B.open then return end
    for _, token in ipairs(tokens) do eqoa.events.off(token) end
    tokens = {}
    if escape then eqoa.input.release(escape); escape = nil end
    B.open, B.hover, tab_hits, log_rect = false, nil, {}, nil
end

local function open()
    if B.open then return end
    B.open, B.grid = true, nil
    listen("render", draw)
    for _, name in ipairs({"inventory.changed", "bank.changed", "state.reset"}) do listen(name, B.dirty) end
    listen("input.key", function(e)
        if e.data.down and not e.data["repeat"] and e.data.key:lower() == "escape" then close() end
    end)
    listen("pointer.pressed", function(e)
        local d = e.data
        if d.button ~= "left" then return end
        for _, h in ipairs(tab_hits) do if inside(d.x, d.y, h) then B.select(h.tab); return end end
    end)
    listen("pointer.wheel", function(e)
        local d, steps = e.data, e.data.steps or 0
        if steps ~= 0 and inside(d.x, d.y, log_rect) then
            B.scroll = math.max(0, math.min(math.max(0, #B.log - LOG_LINES), B.scroll + (steps > 0 and -1 or 1)))
        end
    end)
    -- Escape is the window's, not the game menu's, while it is up (a claim the menu's owner may refuse).
    escape = eqoa.input.claim("Escape", true)
end

eqoa.input.bind({id = "open", label = "Bag viewer", default_key = ""}, function()
    if B.open then close() else open() end
end)
-- The loot log is the one thing kept while the window is closed, from events only.
eqoa.events.on("loot.changed", function(e) B.log_loot(e.data) end)
eqoa.events.on("disabling", close)

function draw()
    local grid = B.grid
    if grid == nil then grid = B.rebuild() end
    if not grid then return end
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end

    local gy = 46
    local gh = grid.rows * SLOT + (grid.rows - 1) * GAP
    local info_y = gy + gh + 5
    local log_y = info_y + 38
    local H = log_y + LOG_LINES * LINE + PAD
    -- centred in the HUD frame, and on screen wherever the window is sized
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    B.draws, B.rect = B.draws + 1, {x = x0, y = y0, w = W * k, h = H * k, k = k}

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    local flags = ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove +
        ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings +
        ImGuiWindowFlags.NoInputs
    ui.Begin("eqoa_bag_viewer_layer", nil, flags)
    ui.SetWindowFontScale(k * 11 / 13)
    local dl = ui.GetWindowDrawList()
    local c = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), red = ui.GetColorU32(.843, .098, .129, 1),
        worn = ui.GetColorU32(.35, .8, .4, 1), warn = ui.GetColorU32(1, .3, .3, 1),
        white = ui.GetColorU32(1, 1, 1, 1), shadow = ui.GetColorU32(0, 0, 0, 1),
    }
    local kinds = {title = c.white, text = c.text, muted = c.muted, red = c.warn}
    local function fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    local function frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    local function text(x, y, value, color, maxw)
        value = tostring(value)
        while maxw and #value > 0 and ui.CalcTextSize(value) > maxw * k do
            value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
        end
        dl:AddText(at(x, y), color or c.text, value)
    end

    fill(0, 0, W, H, c.surface)
    frame(0, 0, W, H, c.border)
    text(PAD, 7, "Bags", c.white)

    tab_hits = {}
    B.tab_hits = tab_hits
    local tx = PAD
    for i, t in ipairs(B.TABS) do
        local tw = ui.CalcTextSize(t.label) / k + 14
        text(tx + 7, 25, t.label, B.tab == i and c.text or c.muted)
        if B.tab == i then fill(tx, 39, tw, 2, c.red) end
        tab_hits[#tab_hits + 1] = {x = x0 + tx * k, y = y0 + 22 * k, w = tw * k, h = 19 * k, tab = i}
        tx = tx + tw + 2
    end

    local p = eqoa.input.pointer()
    local over
    for slot = 1, grid.rows * B.COLUMNS do
        local col, row = (slot - 1) % B.COLUMNS, math.floor((slot - 1) / B.COLUMNS)
        local sx, sy = PAD + col * (SLOT + GAP), gy + row * (SLOT + GAP)
        fill(sx, sy, SLOT, SLOT, c.raised)
        local entry = grid.items[slot]
        local border = c.border
        if entry then
            local it = entry.item
            if entry.texture then
                dl:AddImage(entry.texture, at(sx + 2, sy + 2), at(sx + SLOT - 2, sy + SLOT - 2),
                    ImVec2.new(0, 0), ImVec2.new(1, 1), c.white)
            else
                text(sx + 11, sy + 9, it.name:sub(1, 1), c.muted)
            end
            if it.equipped then border = c.worn; fill(sx + 2, sy + 2, 9, 11, c.shadow); text(sx + 4, sy + 2, "E", c.worn) end
            if it.quantity > 1 then
                local label = tostring(it.quantity)
                local lw = ui.CalcTextSize(label) / k
                text(sx + SLOT - lw - 2, sy + SLOT - 13, label, c.shadow)
                text(sx + SLOT - lw - 3, sy + SLOT - 14, label, c.white)
            end
            if p and p.inside_window and inside(p.x, p.y, {x = x0 + sx * k, y = y0 + sy * k, w = SLOT * k, h = SLOT * k}) then
                over, border = entry, c.white
            end
        end
        frame(sx, sy, SLOT, SLOT, border)
    end

    text(PAD, info_y, (grid.bank and "Bank tunar " or "Tunar ") .. B.tunar(grid.money), c.text)
    local used = grid.used .. "/" .. (grid.capacity or "?")
    text(W - PAD - ui.CalcTextSize(used) / k, info_y, used, c.text)
    fill(PAD, info_y + 16, W - 2 * PAD, 1, c.border)
    text(PAD, info_y + 21, "Loot log", c.muted)
    log_rect = {x = x0 + PAD * k, y = y0 + log_y * k, w = (W - 2 * PAD) * k, h = LOG_LINES * LINE * k}
    if #B.log == 0 then text(PAD, log_y, "Nothing looted yet", c.muted) end
    for i = 1, LOG_LINES do
        local entry = B.log[B.scroll + i]
        if entry then text(PAD, log_y + (i - 1) * LINE, entry.text, c.text, W - 2 * PAD) end
    end
    eqoa.input.claim_pointer({x = x0, y = y0, width = W * k, height = H * k})

    if over then
        local lines = B.card(over.item)
        local cw = 0
        for _, l in ipairs(lines) do cw = math.max(cw, ui.CalcTextSize(l[1])) end
        local cwk, chk = math.min(cw + 12 * k, 220 * k), (#lines * LINE + 8) * k
        local cx = math.max(0, math.min(dw - cwk, p.x + 14 * k > dw - cwk and p.x - 14 * k - cwk or p.x + 14 * k))
        local cy = math.max(0, math.min(dh - chk, p.y + 14 * k))
        dl:AddRectFilled(ImVec2.new(cx, cy), ImVec2.new(cx + cwk, cy + chk), c.surface)
        dl:AddRect(ImVec2.new(cx, cy), ImVec2.new(cx + cwk, cy + chk), c.red)
        for i, l in ipairs(lines) do
            local value = l[1]
            while #value > 0 and ui.CalcTextSize(value) > cwk - 8 * k do
                value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
            end
            dl:AddText(ImVec2.new(cx + 6 * k, cy + 4 * k + (i - 1) * LINE * k), kinds[l[2]], value)
        end
    end
    ui.SetWindowFontScale(1)
    ui.End()
end
