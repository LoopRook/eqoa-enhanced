-- EQOA Name Plates: your own plates over heads and floating damage numbers, drawn from what the client tells a mod.
-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- While this mod is on, the game's own name plates and floating damage numbers are hidden (and its target rings,
-- only if you tick "Hide the game's target rings"). Turn it off and the game's come back at once.
-- Per frame it does the least it can: entities are cut by distance before anything is projected, text widths are
-- remembered, and the tables it needs each frame are reused.

local ui = eqoa.imgui

-- What the player can change (the settings window; kept in the mod's saved settings).
local S = {
    players = true, npcs = true, creatures = true,
    hide_rings = false,
    distance = 40,   -- game units
    size = 100,      -- percent of the default text size
}
local show_window = false
nameplates_state = S -- (a global so the mod's settings can be read and set from a test)

eqoa.settings.on_save(function() return S end)
eqoa.settings.on_load(function(saved)
    if type(saved) ~= "table" then return end
    for _, key in ipairs({ "players", "npcs", "creatures", "hide_rings" }) do
        if type(saved[key]) == "boolean" then S[key] = saved[key] end
    end
    if type(saved.distance) == "number" then S.distance = math.max(10, math.min(120, saved.distance)) end
    if type(saved.size) == "number" then S.size = math.max(60, math.min(200, saved.size)) end
end)

eqoa.options.button({ id = "settings", label = "Name Plates: settings..." }, function()
    show_window = not show_window
end)

-- ---- holding back the game's own ------------------------------------------------------------------------------

local claims = {}

local function want(name, on)
    if on and not claims[name] then
        local c = eqoa.hud.hide(name)
        if c then claims[name] = c end
    elseif not on and claims[name] then
        eqoa.hud.show(claims[name])
        claims[name] = nil
    end
end

local function release_all()
    for name in pairs(claims) do want(name, false) end
end

eqoa.events.on("world.leaving", release_all)
eqoa.events.on("disabling", release_all)
eqoa.events.on("state.reset", function() release_all() end)

-- ---- colours and text ------------------------------------------------------------------------------------------

local C = {}
local function colors()
    if C.white then return end
    local function u(r, g, b, a) return ui.GetColorU32(r, g, b, a or 1) end
    C.white, C.shadow = u(1, 1, 1), u(0, 0, 0, 0.9)
    C.target, C.group = u(1, 0.9, 0.35), u(0.4, 1, 0.45)
    C.mark, C.tag, C.dim = u(1, 0.86, 0.1), u(0.75, 0.75, 0.75), u(0.6, 0.6, 0.6)
    C.bar_bg, C.bar_edge = u(0.05, 0.05, 0.05, 0.85), u(0, 0, 0, 0.9)
    C.hp_hi, C.hp_mid, C.hp_lo = u(0.3, 0.85, 0.3), u(0.95, 0.8, 0.2), u(0.9, 0.25, 0.2)
    C.red, C.yellow, C.lblue, C.blue, C.grey = u(1, 0.3, 0.3), u(1, 0.9, 0.3), u(0.5, 0.8, 1), u(0.35, 0.5, 1), u(0.65, 0.65, 0.65)
    C.green = u(0.4, 1, 0.4)
end

-- The consider colour of a level: the server's own colour for your target, otherwise from how far the level is
-- from yours (the server decides the true bands; this is the usual shape of them).
local function level_color(level, mine)
    if not level or not mine then return C.white end
    local d = level - mine
    if d >= 3 then return C.red end
    if d >= 1 then return C.yellow end
    if d == 0 then return C.white end
    if d >= -3 then return C.lblue end
    if d >= -9 then return C.blue end
    return C.grey
end

local font, base_size
local function setup_font()
    if font ~= nil then return end
    -- the default font, where this client has a font service; without one the plates keep the default text size
    local f = eqoa.capabilities.has("eqoa.fonts.default") and eqoa.fonts.default() or nil
    font = f and f.font or false
    local ok, h = pcall(ui.GetTextLineHeight)
    base_size = ok and h or 14
end

-- text widths, remembered per string (and per size): the number of distinct names is small
local widths, widths_n, widths_k = {}, 0, 0
local function width(text, k)
    if not font then k = 1 end
    if widths_k ~= k or widths_n > 600 then widths, widths_n, widths_k = {}, 0, k end
    local w = widths[text]
    if w == nil then
        w = ui.CalcTextSize(text) * k
        widths[text], widths_n = w, widths_n + 1
    end
    return w
end

local function put(dl, x, y, color, text, k)
    if font and k ~= 1 then
        local size = base_size * k
        dl:AddText(font, size, ui.vec2(x + 1, y + 1), C.shadow, text)
        dl:AddText(font, size, ui.vec2(x, y), color, text)
    else
        dl:AddText(ui.vec2(x + 1, y + 1), C.shadow, text)
        dl:AddText(ui.vec2(x, y), color, text)
    end
end

-- ---- plates -----------------------------------------------------------------------------------------------------

local cands, cand_n = {}, 0
local group_ids, group_at = {}, -1000
local frame_no = 0
local MAX_PLATES = 60

local function refresh_group()
    local g = eqoa.group.get()
    for id in pairs(group_ids) do group_ids[id] = nil end
    if g and g.in_group then
        for _, m in ipairs(g.members) do group_ids[m.id] = true end
    end
end

local function wanted(e)
    local kind = e.kind
    if kind == "player" then return S.players end
    if kind == "npc" then
        local f = e.npc_flags
        if f and f > 0 then return S.npcs end
        return S.creatures
    end
    return S.creatures -- pets and the rest
end

-- a name longer than this is cut (a plate never grows past the screen's edge for one long name)
local MAX_NAME = 24
local short_names = {}
local function short(name)
    local s = short_names[name]
    if s == nil then
        s = name
        if #name > MAX_NAME then
            s = name:sub(1, MAX_NAME - 3)
            -- never end inside a multi-byte letter (names are plain ASCII on the game's servers)
            while #s > 0 and s:byte(-1) >= 128 do s = s:sub(1, -2) end
            s = s .. "..."
        end
        short_names[name] = s
    end
    return s
end

local function hp_color(f)
    if f > 0.5 then return C.hp_hi end
    if f > 0.25 then return C.hp_mid end
    return C.hp_lo
end

local function draw_plate(dl, e, x, y, k, is_target, mine, consider)
    local bar_w, bar_h = 44 * k, 5 * k
    -- text only grows where the client has a font to size (else it stays the default size)
    local tk = font and k or 1
    local line = (font and base_size or 14) * tk
    local name = short(e.name)
    local lvl = e.level and tostring(e.level) or nil
    local tint = is_target and C.target or (group_ids[e.id] and C.group or C.white)
    local lvl_col = (consider and consider.color) and ui.GetColorU32(consider.color.r, consider.color.g, consider.color.b, 1)
        or level_color(e.level, mine)
    local nw = width(name, tk)
    local lw = lvl and (width(lvl, tk) + width(" ", tk)) or 0
    local total = lw + nw
    local left = x - total / 2
    local top = y - line - bar_h - 2 * k
    -- the bar's top is the plate point; everything stacks upward from the name
    if e.hp_fraction ~= nil then
        local f = math.max(0, math.min(1, e.hp_fraction))
        local bx = x - bar_w / 2
        local by = y - bar_h
        dl:AddRectFilled(ui.vec2(bx, by), ui.vec2(bx + bar_w, by + bar_h), C.bar_bg)
        if f > 0 then dl:AddRectFilled(ui.vec2(bx, by), ui.vec2(bx + bar_w * f, by + bar_h), hp_color(f)) end
        dl:AddRect(ui.vec2(bx, by), ui.vec2(bx + bar_w, by + bar_h), C.bar_edge)
    else
        top = y - line
    end
    if is_target then
        dl:AddRect(ui.vec2(left - 3 * k, top - 1), ui.vec2(left + total + 3 * k, top + line + 1), C.target)
    end
    if lvl then put(dl, left, top, lvl_col, lvl, tk) end
    put(dl, left + lw, top, tint, name, tk)
    local above = top
    if e.merc_tag then
        local tw = width(e.merc_tag, tk * 0.85)
        above = above - line * 0.85
        put(dl, x - tw / 2, above, C.tag, e.merc_tag, tk * 0.85)
    end
    if e.head_mark then
        local mw = width(e.head_mark, tk * 1.6)
        above = above - line * 1.6
        put(dl, x - mw / 2, above, C.mark, e.head_mark, tk * 1.6)
    end
end

local function draw_plates(dl, player, k)
    local pos = player.position
    if not pos then return end
    local list = eqoa.entities.list()
    local lim2 = S.distance * S.distance
    cand_n = 0
    local px, py, pz = pos.x, pos.y, pos.z
    for i = 1, #list do
        local e = list[i]
        if e.id ~= player.id and not e.corpse and e.name ~= "" and wanted(e) then
            local p = e.actor_position
            local dx, dy, dz = p.x - px, p.y - py, p.z - pz
            if dx * dx + dy * dy + dz * dz <= lim2 then
                cand_n = cand_n + 1
                cands[cand_n] = e
            end
        end
    end
    if frame_no - group_at >= 20 then group_at = frame_no refresh_group() end
    local target_id = player.target_id
    local info = target_id and eqoa.target.info() or nil
    local consider = info and info.id == target_id and info.consider or nil
    local drawn = 0
    for i = 1, cand_n do
        if drawn >= MAX_PLATES then break end
        local e = cands[i]
        cands[i] = nil
        local pr = eqoa.camera.project_entity(e.id, 0.25)
        if pr and pr.on_screen then
            local is_target = e.id == target_id
            draw_plate(dl, e, pr.x, pr.y, k, is_target, player.level, is_target and consider or nil)
            drawn = drawn + 1
        end
    end
end

-- ---- damage numbers ---------------------------------------------------------------------------------------------

local numbers, number_n = {}, 0
local now_time = 0 -- ImGui's clock, read in `render` (it is not readable from an event)
local LIFE, MAX_NUMBERS, RISE = 1.2, 24, 55
local avg, samples = 0, 0

eqoa.events.on("combat.hit", function(ev)
    local h = ev.data
    if h.kind ~= "damage" or h.amount <= 0 then return end
    local out, inc = h.outgoing, h.incoming
    if not (out or inc) then return end
    local big = false
    if out then
        -- the game does not mark critical hits; one well above your usual hit is drawn bigger
        if samples >= 4 and h.amount >= avg * 1.6 then big = true end
        avg = samples == 0 and h.amount or (avg * 0.9 + h.amount * 0.1)
        samples = samples + 1
    end
    if number_n >= MAX_NUMBERS then table.remove(numbers, 1) number_n = number_n - 1 end
    number_n = number_n + 1
    numbers[number_n] = { id = h.defender_id, text = tostring(h.amount), t0 = now_time, incoming = inc and not out, big = big }
end)

local function draw_numbers(dl, player, k)
    if number_n == 0 then return end
    local now = now_time
    local i = 1
    while i <= number_n do
        local n = numbers[i]
        local age = now - n.t0
        if age > LIFE then
            table.remove(numbers, i)
            number_n = number_n - 1
        else
            local pr
            if n.incoming then
                local p = player.position
                if p then pr = eqoa.camera.project({ x = p.x, y = p.y + 2.2, z = p.z }) end
            else
                pr = eqoa.camera.project_entity(n.id, 0.6)
            end
            if pr and pr.x then
                n.x, n.y = pr.x, pr.y
            end
            if n.x then
                local scale = (font and k or 1) * (n.big and 1.7 or 1.15)
                local w = width(n.text, scale)
                local fade = age > LIFE * 0.6 and (1 - (age - LIFE * 0.6) / (LIFE * 0.4)) or 1
                local color = n.incoming and ui.GetColorU32(1, 0.25, 0.25, fade) or ui.GetColorU32(1, 1, 1, fade)
                local rise = RISE * k * (age / LIFE)
                put(dl, n.x - w / 2, n.y - rise - 10 * k, color, n.text, scale)
            end
            i = i + 1
        end
    end
end

-- ---- the settings window ---------------------------------------------------------------------------------------

local function settings_window()
    if not show_window then return end
    ui.SetNextWindowPos(80, 80, 4)
    if not ui.Begin("Name Plates") then ui.End() return end
    S.players = ui.Checkbox("Players", S.players)
    S.npcs = ui.Checkbox("NPCs (merchants, bankers, townsfolk)", S.npcs)
    S.creatures = ui.Checkbox("Creatures (and pets)", S.creatures)
    S.hide_rings = ui.Checkbox("Hide the game's target rings", S.hide_rings)
    S.distance = ui.SliderInt("Distance", S.distance, 10, 120)
    S.size = ui.SliderInt("Size %", S.size, 60, 200)
    if ui.Button("Close") then show_window = false eqoa.settings.save(S) end
    ui.End()
end

-- ---- the frame --------------------------------------------------------------------------------------------------

eqoa.events.on("render", function()
    local player = eqoa.player.get()
    if not player then release_all() return end
    want("name_plates", true)
    want("damage_numbers", true)
    want("target_rings", S.hide_rings)
    colors()
    setup_font()
    frame_no = frame_no + 1
    now_time = ui.GetTime()
    local k = S.size / 100
    local dl = ui.GetBackgroundDrawList()
    draw_plates(dl, player, k)
    draw_numbers(dl, player, k)
    settings_window()
end)
