-- World Map: the game's map in a movable, resizable window of this mod's own, in the dark/red look of the minimap mods,
-- with pins and a place search.
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The map key opens it (the mod holds the "map" window); "Use the game's map" (or turning the mod off) gives the game's own
-- map back. The ground is the game's own map picture for the view (eqoa.map.terrain), so it is as sharp as the game's.
local ui = eqoa.imgui

local MIN_W, MIN_H = 360, 260
local TITLE_H, TOOL_H, FOOT_H, GRIP = 24, 28, 18, 14
local NOTE_MAX, PINS_MAX = 40, 60
local ZOOM_STEP = 1.25
local font_info = eqoa.fonts.load("package_resources/SpaceMono-Regular.ttf", 13)
local font = font_info and font_info.font

local C = nil -- packed colours, made on the first frame
local state = {
    session = nil, tokens = {}, claims = {},
    win = { x = nil, y = nil, w = 760, h = 540 },
    pins = {}, -- world id (as text) -> { { x, z, note } }
    view = nil, -- { x, z, scale }: the world point at the middle of the map area and the world units a pixel
    terrain = nil, terrain_size = nil,
    hits = {}, rect = nil, drag = nil, held = {},
    menu = nil, note = nil, search = nil, results = {}, focus = nil,
    markers = {}, markers_world = nil, markers_stale = true,
    dots = {}, dots_at = -1, now = 0, message = nil, message_until = 0, overflow = -1e9,
}
world_map = state -- read by the mod's tests

local function clamp(v, lo, hi) return math.max(lo, math.min(hi, v)) end
local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

-- ---------------------------------------------------------------- settings: the window's place and size, the pins

local function persist()
    local ok, err = eqoa.settings.save({ version = 1, win = state.win, pins = state.pins })
    state.save_error = not ok and err and (err.code .. ": " .. tostring(err.message)) or nil
end
eqoa.settings.on_save(function() return { version = 1, win = state.win, pins = state.pins } end)
eqoa.settings.on_load(function(saved)
    if type(saved) ~= "table" then return end
    local w = saved.win
    if type(w) == "table" then
        for _, k in ipairs({ "x", "y", "w", "h" }) do
            if type(w[k]) == "number" and w[k] == w[k] and math.abs(w[k]) < 1e5 then state.win[k] = w[k] end
        end
        state.win.w = clamp(state.win.w, MIN_W, 4096)
        state.win.h = clamp(state.win.h, MIN_H, 4096)
    end
    if type(saved.pins) == "table" then
        state.pins = {}
        for world, list in pairs(saved.pins) do
            if type(list) == "table" then
                local out = {}
                for _, p in ipairs(list) do
                    if type(p) == "table" and type(p.x) == "number" and type(p.z) == "number" and #out < PINS_MAX then
                        out[#out + 1] = { x = p.x, z = p.z, note = type(p.note) == "string" and p.note:sub(1, NOTE_MAX * 4) or "" }
                    end
                end
                state.pins[tostring(world)] = out
            end
        end
    end
end)

local function pins_here()
    local map = eqoa.map.get()
    local key = tostring(map and map.world_id or 0)
    state.pins[key] = state.pins[key] or {}
    return state.pins[key]
end

-- ---------------------------------------------------------------- the world

local function player_position()
    local p = eqoa.player.get()
    return p and p.position
end

local function refresh_markers(world)
    if state.markers_world == world and not state.markers_stale then return end
    state.markers_world, state.markers_stale = world, false
    state.markers = eqoa.map.markers(world) or {}
end
for _, name in ipairs({ "map.changed", "state.reset" }) do
    eqoa.events.on(name, function() state.markers_stale = true end)
end

local function away(x, z)
    local p = player_position()
    if not p then return nil end
    return math.sqrt((x - p.x) ^ 2 + (z - p.z) ^ 2)
end

-- ---------------------------------------------------------------- opening and closing

local draw

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then state.tokens[#state.tokens + 1] = token end
end

local function release_terrain()
    if state.terrain then state.terrain.close() end
    state.terrain, state.terrain_size = nil, nil
end

local function finish()
    for _, token in ipairs(state.tokens) do eqoa.events.off(token) end
    state.tokens = {}
    for _, token in ipairs(state.claims) do eqoa.input.release(token) end
    state.claims = {}
    release_terrain()
    state.session, state.hits, state.rect, state.drag = nil, {}, nil, nil
    state.menu, state.note, state.search, state.focus, state.held = nil, nil, nil, nil, {}
    state.results = {}
end

local function close()
    if not state.session then return end
    eqoa.windows.close_session(state.session.ref)
    finish()
end

local function game_map()
    if not state.session then return end
    local ok, err = eqoa.windows.pass(state.session.ref)
    if not ok then
        state.message, state.message_until = (err and err.message) or "The game's map cannot open now", state.now + 4
    end
end

-- the keys a map holds: the game's own pan keys and Escape, taken only while this map is up (the keys of the game's own
-- map that a mod can claim; the arrow keys stay the game's), and Enter for the search and note boxes
local function claim_keys()
    for _, k in ipairs({ "Escape", "Enter", "w", "a", "s", "d" }) do
        local token = eqoa.input.claim(k, true)
        if token then state.claims[#state.claims + 1] = token end
    end
end

local function edit_text(box, d)
    local key = d.key:lower()
    if key == "backspace" then
        box.text = box.text:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
    elseif d.text and d.text ~= "" and d.text:match("^[^%c]+$") and #box.text + #d.text <= (box.max or 40) then
        box.text = box.text .. d.text
    end
end

local function commit_note()
    local n = state.note
    if not n then return end
    state.note = nil
    local pins = pins_here()
    if #pins >= PINS_MAX then
        state.message, state.message_until = "Too many pins here: remove one first", state.now + 4
        return
    end
    pins[#pins + 1] = { x = n.x, z = n.z, note = n.text }
    persist()
end

local function search_results()
    local out = {}
    local q = (state.search or ""):lower()
    if q == "" then return out end
    local map = eqoa.map.get()
    if not map then return out end
    refresh_markers(map.world_id)
    for _, m in ipairs(state.markers) do
        local name = m.label or m.name
        if name and name ~= "" and (m.kind == "camp" or m.kind == "coach" or m.kind == "town") and name:lower():find(q, 1, true) then
            out[#out + 1] = { name = name, kind = m.kind, x = m.x, z = m.z }
            if #out >= 40 then break end
        end
    end
    for _, p in ipairs(pins_here()) do
        if p.note ~= "" and p.note:lower():find(q, 1, true) then out[#out + 1] = { name = p.note, kind = "pin", x = p.x, z = p.z } end
    end
    return out
end

local function go_to(r)
    state.view = state.view or { x = r.x, z = r.z, scale = 8 }
    state.view.x, state.view.z = r.x, r.z
    state.view.scale = math.min(state.view.scale, 8)
    state.found = { x = r.x, z = r.z, until_ = state.now + 5 }
    state.search, state.focus, state.results = nil, nil, {}
end

local function on_key(d)
    local key = d.key:lower()
    if not d.down then
        state.held[key] = nil
        return
    end
    if state.note then
        if d["repeat"] and key ~= "backspace" then return end
        if key == "enter" then commit_note()
        elseif key == "escape" then state.note = nil
        else edit_text(state.note, d) end
        return
    elseif state.focus then
        if key == "enter" then
            if not d["repeat"] and state.results[1] then go_to(state.results[1]) end
        elseif key == "escape" then
            if not d["repeat"] then state.search, state.focus, state.results = nil, nil, {} end
        else
            local b = { text = state.search or "", max = 30 }
            edit_text(b, d)
            if b.text ~= state.search then state.search = b.text; state.results = search_results() end
        end
        return
    end
    if d["repeat"] then return end
    if key == "escape" then
        if state.menu then state.menu = nil else close() end
    elseif key == "w" or key == "a" or key == "s" or key == "d" then
        state.held[key] = true
    elseif key == "+" or key == "=" or key == "-" or key == "_" then
        local v = state.view
        if v then v.scale = v.scale / ((key == "+" or key == "=") and ZOOM_STEP or 1 / ZOOM_STEP) end
    end
end

local function show(session)
    local fresh = state.session == nil
    state.session = session
    if not fresh then return end
    local p = player_position()
    state.view = nil
    state.held, state.menu, state.note, state.search, state.focus = {}, nil, nil, nil, nil
    state.markers_stale = true
    claim_keys()
    listen("render", draw)
    listen("input.key", function(e) on_key(e.data) end)
    listen("pointer.pressed", function(e) state.on_press(e.data) end)
    listen("pointer.wheel", function(e) state.on_wheel(e.data) end)
    if p then state.view = { x = p.x, z = p.z, scale = nil } end
end

local ok, err = eqoa.windows.takeover("map", { scope = "all" })
if not ok then
    eqoa.log.write("warn", "World Map could not take the map key: " .. tostring(err and err.message))
end
eqoa.events.on("windows.session", function(e)
    local d = e.data
    if d.kind ~= "map" then return end
    if d.phase == "closed" then
        if state.session and d.ref == state.session.ref then finish() end
    elseif d.phase == "opened" or d.phase == "changed" then
        local s = eqoa.windows.session("map")
        if s then show(s) end
    end
end)
eqoa.events.on("disabling", function() finish(); eqoa.windows.release("map") end)
eqoa.events.on("world.leaving", function() finish() end)

-- ---------------------------------------------------------------- the map area: view, picture, marks

local function geometry(dw, dh)
    local w = state.win
    w.w, w.h = clamp(w.w, MIN_W, math.max(MIN_W, dw - 8)), clamp(w.h, MIN_H, math.max(MIN_H, dh - 8))
    w.x = clamp(w.x or (dw - w.w) / 2, 0, math.max(0, dw - w.w))
    w.y = clamp(w.y or (dh - w.h) / 2, 0, math.max(0, dh - w.h))
    local area = { x = w.x + 1, y = w.y + TITLE_H + TOOL_H, w = w.w - 2, h = w.h - TITLE_H - TOOL_H - FOOT_H }
    return { x = w.x, y = w.y, w = w.w, h = w.h }, area
end

-- world point <-> window pixel, for the area
local function to_screen(area, x, z)
    local v = state.view
    return area.x + area.w / 2 + (x - v.x) / v.scale, area.y + area.h / 2 + (z - v.z) / v.scale
end
local function to_world(area, sx, sy)
    local v = state.view
    return v.x + (sx - area.x - area.w / 2) * v.scale, v.z + (sy - area.y - area.h / 2) * v.scale
end

-- the picture's size: the area rounded up to 64 pixels, so a resize does not remake it at every pixel
local function ensure_terrain(area)
    local pw = clamp(math.ceil(area.w / 64) * 64, 64, 2048)
    local ph = clamp(math.ceil(area.h / 64) * 64, 64, 2048)
    local key = pw .. "x" .. ph
    if state.terrain and state.terrain_size == key then return state.terrain, pw, ph end
    release_terrain()
    local v = state.view
    local t = eqoa.map.terrain({ center = { x = v.x, z = v.z }, scale = v.scale or 8, width = pw, height = ph })
    state.terrain, state.terrain_size = t, t and key or nil
    return t, pw, ph
end

local function fit_limits(t, area)
    local b = t and t.bounds
    if not b then return 1, 400 end
    local fit = math.max((b.x_max - b.x_min) / area.w, (b.z_max - b.z_min) / area.h) * 1.1
    return 1, math.max(fit, 2)
end

local function pan_by_keys(dt, area)
    local v = state.view
    local dx = (state.held.d and 1 or 0) - (state.held.a and 1 or 0)
    local dz = (state.held.s and 1 or 0) - (state.held.w and 1 or 0)
    if dx ~= 0 or dz ~= 0 then
        local step = 0.6 * area.h * dt * v.scale
        v.x, v.z = v.x + dx * step, v.z + dz * step
        state.following = false
    end
end

local function refresh_dots()
    if state.now - state.dots_at < 0.5 then return end
    state.dots_at = state.now
    local list = eqoa.entities.list() or {}
    local out = {}
    for _, e in ipairs(list) do
        local p = e.position
        if p and e.name and e.name ~= "" then out[#out + 1] = { x = p.x, z = p.z, name = e.name, level = e.level, kind = e.kind } end
        if #out >= 400 then break end
    end
    state.dots = out
end

-- what is under the pointer (the nearest within a few pixels): { x, y, lines, remove = fn? }
local function under(area, mx, my, world, map)
    local best, bd = nil, 1e9
    local function try(x, z, lines, remove, reach)
        local sx, sy = to_screen(area, x, z)
        local d = (sx - mx) ^ 2 + (sy - my) ^ 2
        if d <= reach * reach and d < bd then best, bd = { x = sx, y = sy, lines = lines, remove = remove, wx = x, wz = z }, d end
    end
    local function dist_line(x, z)
        local a = away(x, z)
        return a and string.format("%.0f away", a) or ""
    end
    if state.view.scale <= 12 then
        for _, e in ipairs(state.dots) do
            local level = (e.level and e.level > 0) and ("Level " .. e.level) or "Level unknown"
            local kind = e.kind == "player" and "Player" or e.kind == "pet" and "Pet" or "NPC"
            try(e.x, e.z, { { e.name, "title" }, { level .. " / " .. kind, "text" }, { dist_line(e.x, e.z), "text" } }, nil, 5)
        end
    end
    for _, m in ipairs(state.markers) do
        local name = m.label or m.name
        if name and name ~= "" and m.kind ~= "waypoint" then
            try(m.x, m.z, { { name, "title" }, { dist_line(m.x, m.z), "text" } }, nil, 7)
        end
    end
    for i, p in ipairs(pins_here()) do
        try(p.x, p.z, { { p.note ~= "" and p.note or "Pin", "title" }, { dist_line(p.x, p.z), "text" }, { "Right-click to remove", "muted" } },
            function() table.remove(pins_here(), i); persist() end, 8)
    end
    local wp = map and map.waypoint
    if wp and wp.world_id == world then
        try(wp.x, wp.z, { { wp.label and wp.label ~= "" and wp.label or "Waypoint", "title" }, { dist_line(wp.x, wp.z), "text" }, { "Right-click to remove", "muted" } },
            function() eqoa.map.clear_waypoint() end, 9)
    end
    return best
end

-- ---------------------------------------------------------------- pointer

function state.on_press(d)
    if not state.session or not state.rect or not inside(d.x, d.y, state.rect) then return end
    if d.button == "left" then
        for i = #state.hits, 1, -1 do
            local h = state.hits[i]
            if inside(d.x, d.y, h) then h.run(); return end
        end
        state.menu = nil
        state.focus = nil
        local r = state.rect
        if inside(d.x, d.y, state.grip) then
            state.drag = { kind = "resize", px = d.x, py = d.y, w = state.win.w, h = state.win.h, age = 0 }
        elseif d.y < r.y + TITLE_H then
            state.drag = { kind = "move", px = d.x, py = d.y, wx = state.win.x, wy = state.win.y, age = 0 }
        elseif state.area and inside(d.x, d.y, state.area) and state.view then
            state.drag = { kind = "pan", px = d.x, py = d.y, vx = state.view.x, vz = state.view.z, age = 0 }
            state.following = false
        end
    elseif d.button == "right" then
        state.menu = nil
        if not (state.area and inside(d.x, d.y, state.area) and state.view) then return end
        local map = eqoa.map.get()
        if not map then return end
        local hit = under(state.area, d.x, d.y, map.world_id, map)
        if hit and hit.remove then hit.remove(); return end
        local wx, wz = to_world(state.area, d.x, d.y)
        state.menu = { x = d.x, y = d.y, wx = wx, wz = wz, world = map.world_id }
    end
end

function state.on_wheel(d)
    if not state.session or not state.area or not state.view or not d.steps or d.steps == 0 then return end
    if not inside(d.x, d.y, state.area) then return end
    local v = state.view
    local bx, bz = to_world(state.area, d.x, d.y)
    local lo, hi = fit_limits(state.terrain, state.area)
    v.scale = clamp(v.scale / ZOOM_STEP ^ d.steps, lo, hi)
    -- the point under the pointer stays under it
    local ax, az = to_world(state.area, d.x, d.y)
    v.x, v.z = v.x + bx - ax, v.z + bz - az
    state.following = false
end

-- ---------------------------------------------------------------- drawing

local function colours()
    return {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .97), raised = ui.GetColorU32(.102, .102, .102, 1),
        border = ui.GetColorU32(.2, .2, .2, 1), red = ui.GetColorU32(.843, .098, .129, 1),
        selected = ui.GetColorU32(.17, .067, .08, 1), black = ui.GetColorU32(0, 0, 0, 1),
        title = ui.GetColorU32(1, .86, .45, 1), card = ui.GetColorU32(.95, .93, .86, 1), white = ui.GetColorU32(1, 1, 1, 1),
        camp = ui.GetColorU32(.95, .75, .2, 1), coach = ui.GetColorU32(.4, .8, 1, 1), town = ui.GetColorU32(.95, .95, .95, 1),
        quest = ui.GetColorU32(1, .9, .3, 1), community = ui.GetColorU32(.6, .9, .6, 1),
        waypoint = ui.GetColorU32(1, .3, .85, 1), group = ui.GetColorU32(1, .5, .05, 1),
        npc = ui.GetColorU32(.75, .7, .45, 1), player = ui.GetColorU32(.45, .7, 1, 1), pet = ui.GetColorU32(.5, .9, .5, 1),
    }
end

function draw(e)
    if not state.session then return end
    local dw, dh = ui.GetDisplaySize()
    if dw <= 0 or dh <= 0 then return end
    local map = eqoa.map.get()
    if not map then return end
    C = C or colours()
    local c = C
    local rect, area = geometry(dw, dh)
    state.rect, state.area = rect, area
    local pointer = eqoa.input.pointer()
    state.now = e and e.time or state.now
    local dt = clamp(e and e.data and e.data.dt or 1 / 60, 0, 0.1)

    -- the view: first the player's area at the game's own opening zoom, then following the player until moved
    local v = state.view
    if not v then
        local p = player_position()
        if not p then return end
        v = { x = p.x, z = p.z, scale = nil }
        state.view = v
    end
    v.scale = v.scale or math.max(2500 / math.min(area.w, area.h), 1)
    local t, pw, ph = ensure_terrain(area)
    local lo, hi = fit_limits(t, area)
    v.scale = clamp(v.scale, lo, hi)
    pan_by_keys(dt, area)
    if t then
        -- the picture's middle is the view's middle shifted by the rounding of its size
        t.set_view({ center = { x = v.x + (pw - area.w) / 2 * v.scale, z = v.z + (ph - area.h) / 2 * v.scale }, scale = v.scale })
    end
    refresh_markers(map.world_id)
    if state.focus or state.note then eqoa.input.capture_text(true) end
    if v.scale <= 12 then refresh_dots() end

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_world_map_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove
        + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    if font then ui.PushFont(font) end
    ui.SetWindowFontScale(12 / 13)
    local dl = ui.GetWindowDrawList()
    local V = ImVec2.new
    -- a press that took the title strip or the grip, followed to its release
    local d = state.drag
    if d then
        d.age = d.age + 1
        if ui.IsMouseDown(0) and pointer then
            d.seen = true
            local dx, dy = pointer.x - d.px, pointer.y - d.py
            if d.kind == "move" then state.win.x, state.win.y = d.wx + dx, d.wy + dy
            elseif d.kind == "resize" then state.win.w, state.win.h = d.w + dx, d.h + dy
            elseif d.kind == "pan" and state.view then state.view.x, state.view.z = d.vx - dx * state.view.scale, d.vz - dy * state.view.scale end
            rect, area = geometry(dw, dh)
            state.rect, state.area = rect, area
        elseif d.seen or d.age > 5 then
            state.drag = nil
            if d.kind ~= "pan" then persist() end
        end
    end

    local function fill(x, y, w, h, col) dl:AddRectFilled(V(x, y), V(x + w, y + h), col) end
    local function frame(x, y, w, h, col) dl:AddRect(V(x, y), V(x + w, y + h), col) end
    local function text(x, y, value, col, maxw)
        value = tostring(value)
        while maxw and #value > 0 and ui.CalcTextSize(value) > maxw do
            value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
        end
        dl:AddText(V(x, y), col or c.text, value)
        -- how far the text reaches past the window (the tests read it)
        local over = x + ui.CalcTextSize(value) - (rect.x + rect.w)
        if over > state.overflow then state.overflow = over end
    end
    state.hits = {}
    state.overflow = -1e9
    local function button(x, y, w, h, label, run, active)
        fill(x, y, w, h, active and c.selected or c.raised)
        frame(x, y, w, h, active and c.red or c.border)
        text(x + 6, y + 4, label, c.text, w - 10)
        state.hits[#state.hits + 1] = { x = x, y = y, w = w, h = h, run = run, label = label }
    end

    -- the window
    fill(rect.x, rect.y, rect.w, rect.h, c.surface)
    frame(rect.x, rect.y, rect.w, rect.h, c.border)
    local zone = map.world_name or "World"
    text(rect.x + 10, rect.y + 5, "WORLD MAP  " .. zone, c.muted, rect.w - 50)
    button(rect.x + rect.w - 24, rect.y + 2, 20, 20, "X", close)

    -- the toolbar: the search box, the game's map
    local ty = rect.y + TITLE_H + 2
    local gm_w = 150
    local sw = clamp(rect.w - gm_w - 40, 80, 220)
    local sx = rect.x + 8
    fill(sx, ty, sw, 22, c.black)
    frame(sx, ty, sw, 22, state.focus and c.red or c.border)
    local qtext = state.search or ""
    if qtext == "" and not state.focus then text(sx + 6, ty + 4, "Find a place...", c.muted, sw - 12)
    else text(sx + 6, ty + 4, qtext .. (state.focus and "_" or ""), c.text, sw - 12) end
    state.hits[#state.hits + 1] = { label = "search", x = sx, y = ty, w = sw, h = 22, run = function()
        state.focus = true
        state.search = state.search or ""
        state.results = search_results()
    end }
    button(rect.x + rect.w - gm_w - 8, ty, gm_w, 22, "Use the game's map", game_map)

    -- the ground
    local info = t and t.texture
    fill(area.x, area.y, area.w, area.h, c.black)
    dl:PushClipRect(V(area.x, area.y), V(area.x + area.w, area.y + area.h), true)
    if info then
        local uv = info.uv
        dl:AddImage(info.texture, V(area.x, area.y), V(area.x + area.w, area.y + area.h),
            V(uv[1], uv[2]), V(uv[1] + (uv[3] - uv[1]) * area.w / pw, uv[2] + (uv[4] - uv[2]) * area.h / ph), c.white)
    end
    local world = map.world_id
    local function on_screen(sx2, sy2, r) return sx2 >= area.x - r and sx2 <= area.x + area.w + r and sy2 >= area.y - r and sy2 <= area.y + area.h + r end
    if v.scale <= 12 then
        for _, e in ipairs(state.dots) do
            local sx2, sy2 = to_screen(area, e.x, e.z)
            if on_screen(sx2, sy2, 3) then
                dl:AddCircleFilled(V(sx2, sy2), 2, e.kind == "player" and c.player or e.kind == "pet" and c.pet or c.npc, 8)
            end
        end
    end
    for _, m in ipairs(state.markers) do
        if m.kind ~= "waypoint" then
            local sx2, sy2 = to_screen(area, m.x, m.z)
            if on_screen(sx2, sy2, 8) then
                local col = c[m.kind] or c.group
                if m.kind == "group_waypoint" then col = c.group end
                dl:AddRectFilled(V(sx2 - 3.5, sy2 - 3.5), V(sx2 + 3.5, sy2 + 3.5), c.black)
                dl:AddRectFilled(V(sx2 - 2.5, sy2 - 2.5), V(sx2 + 2.5, sy2 + 2.5), col)
                if m.kind == "town" and (m.label or m.name) then text(sx2 + 6, sy2 - 6, m.label or m.name, c.text, 150) end
            end
        end
    end
    for _, p in ipairs(pins_here()) do
        local sx2, sy2 = to_screen(area, p.x, p.z)
        if on_screen(sx2, sy2, 10) then
            dl:AddTriangleFilled(V(sx2, sy2), V(sx2 - 5, sy2 - 11), V(sx2 + 5, sy2 - 11), c.black)
            dl:AddTriangleFilled(V(sx2, sy2 - 1.5), V(sx2 - 3.5, sy2 - 10), V(sx2 + 3.5, sy2 - 10), c.red)
            if p.note ~= "" then text(sx2 + 8, sy2 - 14, p.note, c.text, 160) end
        end
    end
    local wp = map.waypoint
    if wp and wp.world_id == world then
        local sx2, sy2 = to_screen(area, wp.x, wp.z)
        if on_screen(sx2, sy2, 12) then
            dl:AddCircleFilled(V(sx2, sy2), 7, c.black, 16)
            dl:AddCircleFilled(V(sx2, sy2), 5, c.waypoint, 16)
            local a = away(wp.x, wp.z)
            if a then text(sx2 + 9, sy2 - 6, string.format("%.0f away", a), c.text, 120) end
        end
    end
    local pp = player_position()
    if pp then
        local sx2, sy2 = to_screen(area, pp.x, pp.z)
        if on_screen(sx2, sy2, 8) then
            dl:AddCircleFilled(V(sx2, sy2), 6, c.white, 16)
            dl:AddCircleFilled(V(sx2, sy2), 4, c.red, 16)
        end
    end
    if state.found and state.found.until_ > state.now then
        local sx2, sy2 = to_screen(area, state.found.x, state.found.z)
        dl:AddCircle(V(sx2, sy2), 12, c.title, 24, 2)
    end
    dl:PopClipRect()
    frame(area.x, area.y, area.w, area.h, c.border)

    -- the footer
    local fy = rect.y + rect.h - FOOT_H + 2
    local foot
    if state.message and state.message_until > state.now then foot = state.message
    elseif t and t.progress and t.progress.done < t.progress.total then foot = string.format("Drawing the map: %d of %d zones", t.progress.done, t.progress.total)
    else foot = "Drag: pan   Wheel: zoom   Right-click: waypoint or pin   Esc: close" end
    text(rect.x + 8, fy, foot, c.muted, rect.w - GRIP - 20)

    -- the grip
    state.grip = { x = rect.x + rect.w - GRIP, y = rect.y + rect.h - GRIP, w = GRIP, h = GRIP }
    dl:AddTriangleFilled(V(rect.x + rect.w - 2, rect.y + rect.h - GRIP), V(rect.x + rect.w - 2, rect.y + rect.h - 2), V(rect.x + rect.w - GRIP, rect.y + rect.h - 2), c.border)

    -- search results
    if state.focus and #state.results > 0 then
        local rows = math.min(6, #state.results)
        local ry = ty + 24
        fill(sx, ry, sw, rows * 18 + 4, c.surface)
        frame(sx, ry, sw, rows * 18 + 4, c.border)
        for i = 1, rows do
            local r = state.results[i]
            local yy = ry + 2 + (i - 1) * 18
            if i == 1 then fill(sx + 2, yy, sw - 4, 17, c.selected) end
            text(sx + 6, yy + 2, r.name .. "  (" .. r.kind .. ")", c.text, sw - 12)
            state.hits[#state.hits + 1] = { label = r.name, x = sx, y = yy, w = sw, h = 17, run = function() go_to(r) end }
        end
    elseif state.focus and (state.search or "") ~= "" then
        local ry = ty + 24
        fill(sx, ry, sw, 22, c.surface); frame(sx, ry, sw, 22, c.border)
        text(sx + 6, ry + 4, "No matching place", c.muted, sw - 12)
    end

    -- the pin's note box
    if state.note then
        local n = state.note
        local bw, bh = 250, 54
        local bx, by = clamp(n.sx, area.x, area.x + area.w - bw), clamp(n.sy, area.y, area.y + area.h - bh)
        fill(bx, by, bw, bh, c.surface); frame(bx, by, bw, bh, c.red)
        text(bx + 8, by + 5, "Note for this pin (Enter saves, Esc cancels)", c.muted, bw - 16)
        fill(bx + 8, by + 24, bw - 16, 22, c.black); frame(bx + 8, by + 24, bw - 16, 22, c.border)
        text(bx + 12, by + 28, n.text .. "_", c.text, bw - 24)
        state.hits[#state.hits + 1] = { x = bx, y = by, w = bw, h = bh, run = function() end }
    end

    -- the right-click menu
    local mn = state.menu
    if mn and not state.note then
        local rows = {
            { "Set waypoint here", function()
                local _, e = eqoa.map.set_waypoint({ world_id = mn.world, x = mn.wx, z = mn.wz, label = "World Map" })
                if e then state.message, state.message_until = e.message or e.code, state.now + 4 end
            end },
            { "Drop a pin here...", function()
                local sx2, sy2 = to_screen(area, mn.wx, mn.wz)
                state.note = { x = mn.wx, z = mn.wz, text = "", sx = sx2, sy = sy2, max = NOTE_MAX }
            end },
        }
        if map.waypoint then rows[#rows + 1] = { "Clear waypoint", function() eqoa.map.clear_waypoint() end } end
        local mw = 170
        local mx2, my2 = clamp(mn.x, area.x, area.x + area.w - mw), clamp(mn.y, area.y, area.y + area.h - #rows * 20 - 4)
        fill(mx2, my2, mw, #rows * 20 + 4, c.surface); frame(mx2, my2, mw, #rows * 20 + 4, c.border)
        for i, r in ipairs(rows) do
            local yy = my2 + 2 + (i - 1) * 20
            text(mx2 + 8, yy + 3, r[1], c.text, mw - 16)
            local run = r[2]
            state.hits[#state.hits + 1] = { label = r[1], x = mx2, y = yy, w = mw, h = 20, run = function() state.menu = nil; run() end }
        end
    end
    state.note_ready = state.note ~= nil
    -- hover card
    if pointer and pointer.inside_window and inside(pointer.x, pointer.y, area) and not state.menu and not state.note and not state.drag
        and not (state.focus and #state.results > 0) then
        local hit = under(area, pointer.x, pointer.y, world, map)
        world_map.card = hit
        if hit then
            local lines = {}
            for _, l in ipairs(hit.lines) do if l[1] and l[1] ~= "" then lines[#lines + 1] = l end end
            local w, h = 0, 6
            for _, l in ipairs(lines) do local tw, th = ui.CalcTextSize(l[1]); w = math.max(w, tw); h = h + th + 2 end
            w = math.min(w + 16, 280)
            local cx = clamp(hit.x + 14, area.x + 2, math.max(area.x + 2, area.x + area.w - w - 2))
            local cy = clamp(hit.y - h / 2, area.y + 2, math.max(area.y + 2, area.y + area.h - h - 2))
            fill(cx, cy, w, h, c.surface); frame(cx, cy, w, h, c.border)
            local yy = cy + 4
            for _, l in ipairs(lines) do
                text(cx + 8, yy, l[1], l[2] == "title" and c.title or l[2] == "muted" and c.muted or c.card, w - 16)
                local _, th = ui.CalcTextSize(l[1])
                yy = yy + th + 2
            end
        end
    else
        world_map.card = nil
    end

    eqoa.input.claim_pointer({ x = rect.x, y = rect.y, width = rect.w, height = rect.h })
    ui.SetWindowFontScale(1)
    if font then ui.PopFont() end
    ui.End()
end
