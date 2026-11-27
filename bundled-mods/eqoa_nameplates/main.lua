-- EQOA Name Plates: your own plates over heads and floating damage numbers, drawn from what the client tells a mod.
-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- While this mod is on, the game's own name plates and floating damage numbers are hidden (and its target rings,
-- only if you tick "Hide the game's target rings"). Turn it off and the game's come back at once.
-- Per frame it does the least it can: entities are cut by distance before anything is projected, text sizes are
-- remembered, and the tables it needs each frame are reused.
--
-- Text is sized with ImGui's window font scale inside one invisible full-screen window (the way the Modern UI mod
-- scales its text), so it needs no font file; the window takes no clicks, so the game still gets every one.

local ui = eqoa.imgui
-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end
-- Plates belong to the world: they are not drawn over a game window that fills the screen (Options, the Start menu and its
-- pages, the Abilities, Character Summary, quest log and the like). `eqoa.windows.get().native_mode` is the game's own number
-- for what has its controls (the table in the API spec's "Reading the game's windows"); the small windows over the world (loot,
-- a merchant, a dialog) leave the plates alone.
local FULL_SCREEN_MODES = {
    [5] = true, [6] = true, [12] = true, [13] = true, [18] = true, [21] = true, [22] = true, [23] = true, [24] = true,
    [26] = true, [28] = true, [29] = true, [31] = true, [32] = true, [33] = true, [34] = true, [35] = true, [37] = true,
    [39] = true, [40] = true, [41] = true, [42] = true, [60] = true,
}
local function game_page_up()
    local w = eqoa.windows.get()
    if not w then return false end
    return w.start_menu_open == true or FULL_SCREEN_MODES[w.native_mode] == true
end

local DEFAULTS = {
    players = true, npcs = true, creatures = true,
    hide_rings = false,
    distance = 40,   -- game units
    size = 100,      -- percent of the default text size
}
local S = {}
local function reset_settings()
    for key, value in pairs(DEFAULTS) do S[key] = value end
end
reset_settings()
local show_window = false
nameplates_state = S -- (a global so the mod's settings can be read and set from a test)

eqoa.settings.on_save(function() return S end)
eqoa.settings.on_load(function(saved)
    -- a profile with nothing saved, or only some of it, gets the defaults for the rest
    reset_settings()
    if type(saved) ~= "table" then return end
    for _, key in ipairs({ "players", "npcs", "creatures", "hide_rings" }) do
        if type(saved[key]) == "boolean" then S[key] = saved[key] end
    end
    if type(saved.distance) == "number" then S.distance = math.max(10, math.min(120, saved.distance)) end
    if type(saved.size) == "number" then S.size = math.max(60, math.min(200, saved.size)) end
end)

eqoa.options.button({ id = "settings", label = "Name Plate settings..." }, function()
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

-- what the mod remembers of one world: its numbers, its sense of a usual hit, its group
local numbers, number_n = {}, 0
local avg, samples = 0, 0
local group_ids, group_at = {}, -1000

local function forget_world()
    for i = number_n, 1, -1 do numbers[i] = nil end
    number_n, avg, samples = 0, 0, 0
    for id in pairs(group_ids) do group_ids[id] = nil end
    group_at = -1000
end

-- The client drops every hide when the player zones, teleports, respawns or leaves the world, and the mod goes on
-- running: the hides are let go here and taken again by the next frame in the world.
local function lost_world()
    release_all()
    forget_world()
end
eqoa.events.on("world.leaving", lost_world)
eqoa.events.on("world.entered", lost_world)
eqoa.events.on("state.reset", lost_world)
eqoa.events.on("disabling", release_all)

-- ---- colours ---------------------------------------------------------------------------------------------------

local C = {}
local function colors()
    if C.white then return end
    local function u(r, g, b, a) return ui.GetColorU32(r, g, b, a or 1) end
    C.white, C.shadow, C.back = u(1, 1, 1), u(0, 0, 0, 0.9), u(0, 0, 0, 0.5)
    C.target, C.group = u(1, 0.9, 0.35), u(0.4, 1, 0.45)
    C.mark, C.tag = u(1, 0.86, 0.1), u(0.75, 0.75, 0.75)
    C.bar_bg = u(0.05, 0.05, 0.05, 0.85)
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
    if d >= -3 then return C.blue end
    if d >= -6 then return C.lblue end
    if d >= -9 then return C.green end
    return C.grey
end

-- ---- text: sized by the window's font scale, measured once per string and size ------------------------------

local cur_scale = -1
local function scale_to(s)
    if s ~= cur_scale then
        ui.SetWindowFontScale(s)
        cur_scale = s
    end
end

local metrics, metrics_n = {}, 0 -- [size key] = { line = height, [text] = width }
local function metric(s)
    local key = math.floor(s * 100 + 0.5)
    local m = metrics[key]
    if not m then
        scale_to(s)
        local _, h = ui.CalcTextSize("0")
        m = { line = h }
        metrics[key] = m
    end
    return m
end

local function width(text, s)
    local m = metric(s)
    local w = m[text]
    if w == nil then
        if metrics_n > 800 then
            metrics, metrics_n = {}, 0
            m = metric(s)
        end
        scale_to(s)
        w = ui.CalcTextSize(text)
        m[text], metrics_n = w, metrics_n + 1
    end
    return w
end

local function plain(dl, x, y, color, text, s)
    scale_to(s)
    dl:AddText(ui.vec2(x, y), color, text)
end

-- text with a one-pixel shadow (the few damage numbers)
local function shadowed(dl, x, y, color, text, s)
    scale_to(s)
    dl:AddText(ui.vec2(x + 1, y + 1), C.shadow, text)
    dl:AddText(ui.vec2(x, y), color, text)
end

local function backing(dl, x1, y1, x2, y2)
    dl:AddRectFilled(ui.vec2(x1, y1), ui.vec2(x2, y2), C.back)
end

-- the level as text, made once per level
local level_text = {}
local function level_str(level)
    local t = level_text[level]
    if not t then t = tostring(level) level_text[level] = t end
    return t
end

-- a name longer than this is cut (a plate never grows past the screen's edge for one long name)
local MAX_NAME = 24
local short_names, short_n = {}, 0
local function short(name)
    local s = short_names[name]
    if s == nil then
        if short_n > 400 then short_names, short_n = {}, 0 end
        s = name
        if #name > MAX_NAME then
            s = name:sub(1, MAX_NAME - 3)
            -- never end inside a multi-byte letter (names are plain ASCII on the game's servers)
            while #s > 0 and s:byte(-1) >= 128 do s = s:sub(1, -2) end
            s = s .. "..."
        end
        short_names[name], short_n = s, short_n + 1
    end
    return s
end

-- ---- plates -----------------------------------------------------------------------------------------------------

local cands, cand_d2, cand_n = {}, {}, 0
local order = {}
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

local function hp_color(f)
    if f > 0.5 then return C.hp_hi end
    if f > 0.25 then return C.hp_mid end
    return C.hp_lo
end

local function draw_plate(dl, e, x, y, k, is_target, mine, consider)
    local bar_w, bar_h = 44 * k, 5 * k
    local line = metric(k).line
    local name = short(e.name)
    local lvl = e.level and level_str(e.level) or nil
    local tint = is_target and C.target or (group_ids[e.id] and C.group or C.white)
    local lvl_col = (consider and consider.color) and ui.GetColorU32(consider.color.r, consider.color.g, consider.color.b, 1)
        or level_color(e.level, mine)
    local nw = width(name, k)
    local lw = lvl and (width(lvl, k) + width(" ", k)) or 0
    local total = lw + nw
    local left = x - total / 2
    local top = y - line - bar_h - 2 * k
    -- the bar's bottom is the plate point; everything stacks upward from it
    if e.hp_fraction ~= nil then
        local f = math.max(0, math.min(1, e.hp_fraction))
        local bx = x - bar_w / 2
        local by = y - bar_h
        dl:AddRectFilled(ui.vec2(bx, by), ui.vec2(bx + bar_w, by + bar_h), C.bar_bg)
        if f > 0 then dl:AddRectFilled(ui.vec2(bx, by), ui.vec2(bx + bar_w * f, by + bar_h), hp_color(f)) end
    else
        top = y - line
    end
    if is_target then
        dl:AddRect(ui.vec2(left - 3 * k, top - 1), ui.vec2(left + total + 3 * k, top + line + 1), C.target)
    end
    backing(dl, left - 2, top - 1, left + total + 2, top + line + 1)
    if lvl then plain(dl, left, top, lvl_col, lvl, k) end
    plain(dl, left + lw, top, tint, name, k)
    local above = top
    if e.merc_tag then
        local ts = k * 0.85
        local tw = width(e.merc_tag, ts)
        local th = metric(ts).line
        above = above - th
        backing(dl, x - tw / 2 - 2, above - 1, x + tw / 2 + 2, above + th + 1)
        plain(dl, x - tw / 2, above, C.tag, e.merc_tag, ts)
    end
    if e.head_mark then
        local ms = k * 1.6
        local mw = width(e.head_mark, ms)
        above = above - metric(ms).line
        plain(dl, x - mw / 2, above, C.mark, e.head_mark, ms)
    end
end

local function draw_plates(dl, player, k)
    local pos = player.position
    if not pos then return end
    local list = eqoa.entities.list()
    local lim2 = S.distance * S.distance
    local target_id = player.target_id
    cand_n = 0
    local px, py, pz = pos.x, pos.y, pos.z
    for i = 1, #list do
        local e = list[i]
        if e.id ~= player.id and not e.corpse and e.name ~= "" and wanted(e) then
            local p = e.actor_position
            local dx, dy, dz = p.x - px, p.y - py, p.z - pz
            local d2 = dx * dx + dy * dy + dz * dz
            if d2 <= lim2 then
                cand_n = cand_n + 1
                cands[cand_n] = e
                cand_d2[cand_n] = e.id == target_id and -1 or d2 -- the target sorts first
            end
        end
    end
    if frame_no - group_at >= 20 then group_at = frame_no refresh_group() end
    -- more than the cap: the nearest are kept (and the target always)
    local n = cand_n
    for i = 1, n do order[i] = i end
    -- `order` and `cand_d2` are kept from frame to frame and `table.sort` sorts the whole of `order`: the places past this
    -- frame's candidates still hold the last busier frame's numbers, which sorted in among the nearest and named
    -- candidates that are no longer there. Only this frame's candidates are kept in it.
    for i = #order, n + 1, -1 do order[i] = nil end
    if n > MAX_PLATES then
        table.sort(order, function(a, b) return cand_d2[a] < cand_d2[b] end)
        n = MAX_PLATES
    end
    local info = target_id and eqoa.target.info() or nil
    local consider = info and info.id == target_id and info.consider or nil
    for j = 1, n do
        local e = cands[order[j]]
        local pr = eqoa.camera.project_entity(e.id, 0.25)
        if pr and pr.on_screen then
            local is_target = e.id == target_id
            draw_plate(dl, e, pr.x, pr.y, k, is_target, player.level, is_target and consider or nil)
        end
    end
    for i = 1, cand_n do cands[i] = nil end
end

-- ---- damage numbers ---------------------------------------------------------------------------------------------

local now_time = 0 -- ImGui's clock, read in `render` (it is not readable from an event)
local LIFE, MAX_NUMBERS, RISE = 1.2, 24, 55

eqoa.events.on("combat.hit", function(ev)
    local h = ev.data
    if h.kind ~= "damage" or h.amount <= 0 then return end
    local out, inc = h.outgoing, h.incoming
    if not (out or inc) then return end
    local big = false
    -- only a hit the player made counts toward the average (`by_player` true). `by_player` false is a pet's or mercenary's;
    -- nil is a server that sends no tag, where a hit credited to the player may be a pet's or mercenary's too, so it is not
    -- sampled (no bigger numbers there, rather than an average the pet drags down).
    if out and not inc and h.by_player == true then
        -- the game does not mark critical hits; one well above your usual hit is drawn bigger
        if samples >= 4 and h.amount >= avg * 1.6 then big = true end
        avg = samples == 0 and h.amount or (avg * 0.9 + h.amount * 0.1)
        samples = samples + 1
    end
    if number_n >= MAX_NUMBERS then table.remove(numbers, 1) number_n = number_n - 1 end
    local n = { id = h.defender_id, text = tostring(h.amount), t0 = now_time, incoming = inc, big = big }
    if not inc then
        -- where it was hit, so the number can go on rising from there when the creature is gone or out of view
        local e = eqoa.entities.find(h.defender_id)
        local p = e and e.actor_position
        if p then n.wx, n.wy, n.wz = p.x, p.y + 2, p.z end
    end
    number_n = number_n + 1
    numbers[number_n] = n
end)

local function anchor(n, player)
    if n.incoming then
        local p = player.position
        if p then return eqoa.camera.project({ x = p.x, y = p.y + 2.2, z = p.z }) end
        return nil
    end
    local pr = eqoa.camera.project_entity(n.id, 0.6)
    if pr and pr.on_screen then return pr end
    if n.wx then return eqoa.camera.project({ x = n.wx, y = n.wy, z = n.wz }) end
    return pr
end

local function draw_numbers(dl, player, k)
    if number_n == 0 then return end
    local i = 1
    while i <= number_n do
        local n = numbers[i]
        local age = now_time - n.t0
        if age > LIFE then
            table.remove(numbers, i)
            number_n = number_n - 1
        else
            -- drawn only where the point is on the screen: behind you or off to the side it is simply not drawn
            local pr = anchor(n, player)
            if pr and pr.on_screen then
                local scale = k * (n.big and 1.7 or 1.15)
                local w = width(n.text, scale)
                local fade = age > LIFE * 0.6 and (1 - (age - LIFE * 0.6) / (LIFE * 0.4)) or 1
                local color = n.incoming and ui.GetColorU32(1, 0.25, 0.25, fade) or ui.GetColorU32(1, 1, 1, fade)
                local rise = RISE * k * (age / LIFE)
                shadowed(dl, pr.x - w / 2, pr.y - rise - 10 * k, color, n.text, scale)
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

local LAYER_FLAGS
local failed_once = false

local function draw_layer(player)
    -- The window only holds the font scale: it is tiny, empty, takes no input, no focus and no navigation, and
    -- everything is drawn on the background draw list, which is behind every window (other mods' windows, the
    -- game's) however the windows are ordered.
    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(1, 1)
    if not ui.Begin("##nameplates_layer", true, LAYER_FLAGS) then ui.End() return end
    cur_scale = -1
    local k = S.size / 100
    local dl = ui.GetBackgroundDrawList()
    -- a failure here (a bad number from a point near the eye, say) costs this frame's plates, never the mod
    local ok, err = pcall(function()
        draw_plates(dl, player, k)
        draw_numbers(dl, player, k)
    end)
    if not ok and not failed_once then
        failed_once = true
        eqoa.log.write("warn", "name plates: a frame failed: " .. tostring(err))
    end
    scale_to(1)
    ui.End()
end

eqoa.events.on("render", function()
    local player = eqoa.player.get()
    if not player then release_all() return end
    want("name_plates", true)
    want("damage_numbers", true)
    want("target_rings", S.hide_rings)
    colors()
    if not LAYER_FLAGS then
        local f = ImGuiWindowFlags
        LAYER_FLAGS = f.NoDecoration + f.NoInputs + f.NoBackground + f.NoMove + f.NoSavedSettings
            + f.NoFocusOnAppearing + f.NoBringToFrontOnFocus
    end
    frame_no = frame_no + 1
    now_time = ui.GetTime()
    -- under the game's full map: no plates, numbers or settings window (the game's own are still hidden by the claims)
    if map_up() then return end
    -- over a full-screen game page (Options, the Start menu's pages) no plates or numbers either; the settings window is
    -- opened from the Options page, so it stays
    if not game_page_up() then draw_layer(player) end
    settings_window()
end)
