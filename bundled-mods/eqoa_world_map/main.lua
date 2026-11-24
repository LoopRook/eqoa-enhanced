-- World Map: the game's map in a movable, resizable window of this mod's own, in the dark/red look of the minimap mods,
-- with pins and a place search.
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The map key opens it (the mod holds the "map" window); "Use the game's map" (or turning the mod off) gives the game's own
-- map back. The ground is the game's own map picture for the view (eqoa.map.terrain), so it is as sharp as the game's.
local ui = eqoa.imgui
local ID = "eqoa_world_map"

local MIN_W, MIN_H = 360, 260
local MAX_AREA = 2048 -- the picture the client makes is at most this many pixels across
local TITLE_H, TOOL_H, FOOT_H, GRIP = 24, 28, 18, 14
local NOTE_MAX, PINS_MAX = 40, 60
local ZOOM_STEP = 1.25
local TILE_SCALE = 7.8 -- the game's map draws its detail tiles at or below this many world units a pixel
local CELL = 2000
local font_info = eqoa.fonts.load("package_resources/SpaceMono-Regular.ttf", 13)
local font = font_info and font_info.font
local KEYS = { "Escape", "Enter", "w", "a", "s", "d", "W", "A", "S", "D" }

local C = nil -- packed colours, made on the first frame
local state = {
    session = nil, tokens = {}, claims = {},
    win = { x = nil, y = nil, w = 760, h = 540 },
    pins = {}, -- world id (as text) -> { { x, z, note } }
    view = nil, -- { x, z, scale }: the world point at the middle of the map area and the world units a pixel
    last_scale = nil, following = true,
    terrain = nil, terrain_size = nil,
    hits = {}, rect = nil, drag = nil, held = {},
    menu = nil, note = nil, search = nil, results = {}, focus = nil, capturing = false,
    markers = {}, markers_world = nil, markers_stale = true,
    player = nil, dots = { ids = {}, xs = {}, zs = {} },
    now = 0, message = nil, message_until = 0, overflow = -1e9, expect = {},
}
world_map = state -- read by the mod's tests

local function clamp(v, lo, hi) return math.max(lo, math.min(hi, v)) end
local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end
local function say(text) state.message, state.message_until = text, state.now + 5 end

-- ---------------------------------------------------------------- the map key: asked for again after every zoning

state.claim_at = 0
local function claim_map()
    if eqoa.windows.taken("map") == true then return true end
    local ok, err = eqoa.windows.takeover("map", { scope = "all" })
    if not ok and err and err.code ~= "busy" and err.code ~= "unavailable" then
        eqoa.log.write("warn", "World Map could not take the map key: " .. tostring(err.message))
    end
    return ok == true
end
claim_map()
-- the client lets go of every claim at a zoning, a respawn and when "Use the game's windows" goes on: ask again, and
-- slowly otherwise (the switch coming off, another mod letting go)
for _, name in ipairs({ "world.entered", "state.reset" }) do
    eqoa.events.on(name, function() claim_map() end)
end
eqoa.events.on("render", function(e)
    state.now = e.time or state.now
    if state.now >= state.claim_at then
        state.claim_at = state.now + 3
        claim_map()
    end
end)

-- ---------------------------------------------------------------- settings: the window's place and size, the pins

local function persist()
    local ok, err = eqoa.settings.save({ version = 1, win = state.win, pins = state.pins, scale = state.last_scale })
    state.save_error = not ok and err and (err.code .. ": " .. tostring(err.message)) or nil
    if state.save_error then say("The map could not save your pins: " .. tostring(err and err.message)) end
end
eqoa.settings.on_save(function() return { version = 1, win = state.win, pins = state.pins, scale = state.last_scale } end)
eqoa.settings.on_load(function(saved)
    if type(saved) ~= "table" then return end
    local w = saved.win
    if type(w) == "table" then
        for _, k in ipairs({ "x", "y", "w", "h" }) do
            if type(w[k]) == "number" and w[k] == w[k] and math.abs(w[k]) < 1e5 then state.win[k] = w[k] end
        end
        state.win.w = clamp(state.win.w, MIN_W, MAX_AREA + 2)
        state.win.h = clamp(state.win.h, MIN_H, MAX_AREA + TITLE_H + TOOL_H + FOOT_H)
    end
    if type(saved.scale) == "number" and saved.scale == saved.scale and saved.scale >= 1 and saved.scale < 1e5 then state.last_scale = saved.scale end
    if type(saved.pins) == "table" then
        state.pins = {}
        local worlds = 0
        for world, list in pairs(saved.pins) do
            if type(list) == "table" and worlds < 256 then
                worlds = worlds + 1
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

local function refresh_markers(world)
    if state.markers_world == world and not state.markers_stale then return end
    state.markers_world, state.markers_stale = world, false
    state.markers = eqoa.map.markers(world) or {}
end
for _, name in ipairs({ "map.changed", "state.reset" }) do
    eqoa.events.on(name, function() state.markers_stale = true end)
end

-- how far (x, z) is from the player, from this frame's one read of the player
local function away(x, z)
    local p = state.player
    if not p then return nil end
    return math.sqrt((x - p.x) ^ 2 + (z - p.z) ^ 2)
end
local function away_text(x, z)
    local a = away(x, z)
    return a and string.format("%.0f away", a) or ""
end

local function levels(m)
    if m.level_min and m.level_min > 0 then
        if m.level_max and m.level_max > m.level_min then return string.format("Levels %d to %d", m.level_min, m.level_max) end
        return "Level " .. m.level_min
    end
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

local function set_capture(on)
    if on then
        eqoa.input.capture_text(true)
        state.capturing = true
    elseif state.capturing then
        eqoa.input.capture_text(false)
        state.capturing = false
    end
end

-- typing ends: the keyboard is the game's again at once
local function end_typing()
    state.focus, state.note = nil, nil
    set_capture(false)
end

local function release_keys()
    for name, token in pairs(state.claims) do eqoa.input.release(token); state.claims[name] = nil end
end

local function finish()
    for _, token in ipairs(state.tokens) do eqoa.events.off(token) end
    state.tokens = {}
    release_keys()
    release_terrain()
    set_capture(false)
    state.session, state.hits, state.rect, state.drag = nil, {}, nil, nil
    state.menu, state.note, state.search, state.focus, state.held = nil, nil, nil, nil, {}
    state.results, state.found, state.view = {}, nil, nil
end

local function close()
    if not state.session then return end
    eqoa.windows.close_session(state.session.ref)
    finish()
end

local function game_map()
    if not state.session then return end
    local ok, err = eqoa.windows.pass(state.session.ref)
    if not ok then say((err and err.message) or "The game's map cannot open now") end
end

-- the keys a map holds: Escape, Enter and the game's pan keys, taken only while this map is up. A key that another
-- window holds is asked for again every second until it is free. The arrow keys cannot be claimed (they stay the game's).
local function claim_keys()
    for _, k in ipairs(KEYS) do
        if not state.claims[k] then
            local token = eqoa.input.claim(k, true)
            if token then state.claims[k] = token end
        end
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
    local pins = pins_here()
    if #pins >= PINS_MAX then
        say("Too many pins in this world: remove one first")
        return
    end
    pins[#pins + 1] = { x = n.x, z = n.z, note = n.text }
    persist()
    end_typing()
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
        if name and name ~= "" and (m.kind == "camp" or m.kind == "coach" or m.kind == "town" or m.kind == "quest" or m.kind == "community")
            and name:lower():find(q, 1, true) then
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
    state.following = false
    state.found = { x = r.x, z = r.z, until_ = state.now + 5 }
    state.search, state.results = nil, {}
    end_typing()
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
        elseif key == "escape" then end_typing()
        else edit_text(state.note, d) end
        return
    elseif state.focus then
        if key == "enter" then
            if not d["repeat"] and state.results[1] then go_to(state.results[1]) end
        elseif key == "escape" then
            if not d["repeat"] then state.search, state.results = nil, {}; end_typing() end
        else
            local b = { text = state.search or "", max = 30 }
            edit_text(b, d)
            if b.text ~= state.search then state.search = b.text; state.results = search_results() end
        end
        return
    end
    -- another mod's text editor has the keyboard (a chat line, a sell box): its keys are not this map's
    local input = eqoa.input.get()
    if input and input.text_capture then return end
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
    state.view = nil
    state.held, state.menu, state.note, state.search, state.focus = {}, nil, nil, nil, nil
    state.markers_stale = true
    state.following = true
    claim_keys()
    listen("render", draw)
    listen("input.key", function(e) on_key(e.data) end)
    listen("pointer.pressed", function(e) state.on_press(e.data) end)
    listen("pointer.wheel", function(e) state.on_wheel(e.data) end)
    -- what the mod asked of the waypoint is answered later: say what was refused
    listen("action.result", function(e)
        local d = e.data
        local want = d and state.expect[d.request_id]
        if not want then return end
        state.expect[d.request_id] = nil
        if d.ok == false then
            say("Waypoint: " .. tostring(d.reason or d.code))
        elseif want == "clear_after" then
            local req, err = eqoa.map.clear_waypoint()
            if req then state.expect[req] = "clear" elseif err then say("Waypoint: " .. tostring(err.message)) end
        end
    end)
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
-- turning the mod off gives the map key back; a waypoint the player set through it stays (it is set with keep)
eqoa.events.on("disabling", function() finish(); eqoa.windows.release("map") end)
eqoa.events.on("world.leaving", function() finish() end)

-- ---------------------------------------------------------------- the map area: view, picture, marks

local function geometry(dw, dh)
    local w = state.win
    -- the window as shown: the saved size is only changed by the player's own drag, never by a small display
    local sw = clamp(w.w, MIN_W, math.max(MIN_W, math.min(dw - 8, MAX_AREA + 2)))
    local sh = clamp(w.h, MIN_H, math.max(MIN_H, math.min(dh - 8, MAX_AREA + TITLE_H + TOOL_H + FOOT_H)))
    local sx = clamp(w.x or (dw - sw) / 2, 0, math.max(0, dw - sw))
    local sy = clamp(w.y or (dh - sh) / 2, 0, math.max(0, dh - sh))
    local area = { x = sx + 1, y = sy + TITLE_H + TOOL_H, w = sw - 2, h = sh - TITLE_H - TOOL_H - FOOT_H }
    return { x = sx, y = sy, w = sw, h = sh }, area
end

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
    local t, err = eqoa.map.terrain({ center = { x = v.x, z = v.z }, scale = v.scale or 8, width = pw, height = ph })
    state.terrain, state.terrain_size = t, t and key or nil
    state.terrain_error = (not t and err) and err.message or nil
    return t, pw, ph
end

local function fit_limits(t, area)
    local b = t and t.bounds
    if not b then return 1, 400 end
    local fit = math.max((b.x_max - b.x_min) / area.w, (b.z_max - b.z_min) / area.h) * 1.1
    return 1, math.max(fit, 2)
end

-- the middle kept inside the world, as the game's map keeps it
local function keep_in(t)
    local b = t and t.bounds
    local v = state.view
    if b and v then
        v.x, v.z = clamp(v.x, b.x_min, b.x_max), clamp(v.z, b.z_min, b.z_max)
    end
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

-- what is under the pointer: the nearest thing within a few pixels, found by distance alone; the card's lines are
-- made for that one thing only. { sx, sy, x, z, lines, remove? }
local function under(area, mx, my, world, map)
    local best, bd = nil, 1e18
    local function try(x, z, reach, make, remove)
        local sx, sy = to_screen(area, x, z)
        local d = (sx - mx) ^ 2 + (sy - my) ^ 2
        if d <= reach * reach and d < bd then best, bd = { sx = sx, sy = sy, x = x, z = z, make = make, remove = remove }, d end
    end
    local v = state.view
    if v.scale <= 12 then
        local dots = state.dots
        local px, pz = to_world(area, mx, my)
        local r2 = (5 * v.scale) ^ 2
        local self_id = state.self_id
        for i = 1, #dots.ids do
            local dx, dz = dots.xs[i] - px, dots.zs[i] - pz
            if dx * dx + dz * dz <= r2 and dots.ids[i] ~= self_id then
                local id = dots.ids[i]
                try(dots.xs[i], dots.zs[i], 5, function()
                    local e = eqoa.entities.find(id)
                    if not e then return nil end
                    local level = (e.level and e.level > 0) and ("Level " .. e.level) or "Level unknown"
                    local kind = e.kind == "player" and "Player" or e.kind == "pet" and "Pet" or "NPC"
                    return { { e.name, "title" }, { level .. " / " .. kind, "text" } }
                end)
            end
        end
    end
    for _, m in ipairs(state.markers) do
        local name = m.label or m.name
        if name and name ~= "" and m.kind ~= "waypoint" then
            local sx, sy = to_screen(area, m.x, m.z)
            if (sx - mx) ^ 2 + (sy - my) ^ 2 <= 49 then
                try(m.x, m.z, 7, function()
                    local lines = { { name, "title" } }
                    if m.kind == "quest" then lines[#lines + 1] = { m.step and "Quest step" or "Quest to take", "text" } end
                    local lv = levels(m)
                    if lv then lines[#lines + 1] = { lv, "text" } end
                    return lines
                end)
            end
        end
    end
    for i, p in ipairs(pins_here()) do
        try(p.x, p.z, 8, function() return { { p.note ~= "" and p.note or "Pin", "title" }, { "Right-click to remove", "muted" } } end,
            function() table.remove(pins_here(), i); persist() end)
    end
    local wp = map and map.waypoint
    if wp and wp.world_id == world then
        try(wp.x, wp.z, 9, function() return { { wp.label and wp.label ~= "" and wp.label or "Waypoint", "title" }, { "Right-click to remove", "muted" } } end,
            function() state.remove_waypoint(wp) end)
    end
    if not best then return nil end
    local lines = best.make and best.make() or {}
    if #lines == 0 then lines = { { "?", "title" } } end
    lines[#lines + 1] = { away_text(best.x, best.z), "text" }
    best.lines = lines
    return best
end

-- Removing the waypoint: this mod can clear only a pin it placed and still owns. One it did not place (the player's
-- own, another mod's, one from before a restart) is first set again at the same point, which takes it over, and cleared
-- when that is answered.
function state.remove_waypoint(wp)
    if wp.owner == ID then
        local req, err = eqoa.map.clear_waypoint()
        if req then state.expect[req] = "clear" else say("Waypoint: " .. tostring(err and err.message)) end
        return
    end
    local req, err = eqoa.map.set_waypoint({ world_id = wp.world_id, x = wp.x, z = wp.z, label = wp.label, keep = true })
    if req then state.expect[req] = "clear_after" else say("Waypoint: " .. tostring(err and err.message)) end
end

-- ---------------------------------------------------------------- pointer

function state.on_press(d)
    if not state.session then return end
    state.last_press = d.button .. " " .. d.x .. "," .. d.y
    if not state.rect or not inside(d.x, d.y, state.rect) then
        -- a click back in the game: typing ends, the keyboard is the game's again
        if state.focus or state.note or state.menu then end_typing(); state.menu = nil end
        return
    end
    if d.button == "left" then
        for i = #state.hits, 1, -1 do
            local h = state.hits[i]
            if inside(d.x, d.y, h) then h.run(); return end
        end
        state.menu = nil
        if state.focus then end_typing() end
        local r = state.rect
        if inside(d.x, d.y, state.grip) then
            state.drag = { kind = "resize", px = d.x, py = d.y, w = r.w, h = r.h, age = 0 }
        elseif d.y < r.y + TITLE_H then
            state.drag = { kind = "move", px = d.x, py = d.y, wx = r.x, wy = r.y, age = 0 }
        elseif state.area and inside(d.x, d.y, state.area) and state.view then
            state.drag = { kind = "pan", px = d.x, py = d.y, vx = state.view.x, vz = state.view.z, age = 0 }
            state.following = false
        end
    elseif d.button == "right" then
        if state.note then return end
        state.menu = nil
        if state.focus then end_typing() end
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
        waypoint = ui.GetColorU32(1, .3, .85, 1), group = ui.GetColorU32(1, .5, .05, 1), npc = ui.GetColorU32(.75, .7, .45, 1),
        grid = ui.GetColorU32(1, 1, 1, .08), target = ui.GetColorU32(1, .3, .3, 1),
        members = {
            ui.GetColorU32(.35, .75, 1, 1), ui.GetColorU32(.5, .95, .5, 1), ui.GetColorU32(1, .8, .3, 1),
            ui.GetColorU32(.95, .5, 1, 1), ui.GetColorU32(1, .55, .45, 1), ui.GetColorU32(.6, .6, 1, 1),
        },
    }
end

function draw(e)
    if not state.session then return end
    local dw, dh = ui.GetDisplaySize()
    if dw <= 0 or dh <= 0 then return end
    state.now = e and e.time or state.now
    local dt = clamp(e and e.data and e.data.dt or 1 / 60, 0, 0.1)
    local rect, area = geometry(dw, dh)
    state.rect, state.area = rect, area
    -- the window is the map's whatever else happens this frame: clicks on it never reach the game
    eqoa.input.claim_pointer({ x = rect.x, y = rect.y, width = rect.w, height = rect.h })
    local map = eqoa.map.get()
    if not map then return end
    C = C or colours()
    local c = C
    local pointer = eqoa.input.pointer()

    -- one read of the player a frame; keys let go when the window lost the keyboard; claims the map could not take asked again
    local pl = eqoa.player.get()
    state.player = pl and pl.position or nil
    state.heading = pl and pl.heading
    state.self_id = pl and pl.id
    local inp = eqoa.input.get()
    if inp and inp.focused == false then state.held = {} end
    if math.floor(state.now) ~= state.claim_tick then state.claim_tick = math.floor(state.now); claim_keys() end

    -- the view: the player's area at the game's opening zoom (or where it was left), following the player until moved
    local v = state.view
    if not v then
        local p = state.player
        if not p then return end
        v = { x = p.x, z = p.z, scale = state.last_scale }
        state.view = v
        state.following = true
    end
    v.scale = v.scale or math.min(math.max(2500 / math.min(area.w, area.h), 1), TILE_SCALE)
    if state.following and state.player then v.x, v.z = state.player.x, state.player.z end
    local t, pw, ph = ensure_terrain(area)
    local lo, hi = fit_limits(t, area)
    v.scale = clamp(v.scale, lo, hi)
    state.last_scale = v.scale
    pan_by_keys(dt, area)
    keep_in(t)
    if t then
        -- the picture's middle is the view's middle shifted by the rounding of its size
        t.set_view({ center = { x = v.x + (pw - area.w) / 2 * v.scale, z = v.z + (ph - area.h) / 2 * v.scale }, scale = v.scale })
    end
    refresh_markers(map.world_id)
    if state.focus or state.note then set_capture(true) else set_capture(false) end
    if v.scale <= 12 then state.dots = eqoa.entities.dots() or state.dots end

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_world_map_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove
        + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    if font then ui.PushFont(font) end
    ui.SetWindowFontScale(12 / 13)
    local dl = ui.GetWindowDrawList()
    local V = ImVec2.new
    -- a press that took the title strip, the grip or the map, followed to its release
    local d = state.drag
    if d then
        d.age = d.age + 1
        if ui.IsMouseDown(0) and pointer then
            d.seen = true
            local dx, dy = pointer.x - d.px, pointer.y - d.py
            if d.kind == "move" then state.win.x, state.win.y = d.wx + dx, d.wy + dy
            elseif d.kind == "resize" then
                state.win.w = clamp(d.w + dx, MIN_W, MAX_AREA + 2)
                state.win.h = clamp(d.h + dy, MIN_H, MAX_AREA + TITLE_H + TOOL_H + FOOT_H)
            elseif d.kind == "pan" and state.view then state.view.x, state.view.z = d.vx - dx * state.view.scale, d.vz - dy * state.view.scale end
            rect, area = geometry(dw, dh)
            state.rect, state.area = rect, area
        elseif d.seen or d.age > 5 then
            state.drag = nil
            if d.kind ~= "pan" then
                -- what is kept is what is shown
                state.win.x, state.win.y, state.win.w, state.win.h = rect.x, rect.y, rect.w, rect.h
                persist()
            end
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
    local title = "WORLD MAP  " .. (map.world_name or "World")
    if map.zone and map.zone.name and map.zone.name ~= "" then title = title .. ": " .. map.zone.name end
    text(rect.x + 10, rect.y + 5, title, c.muted, rect.w - 50)
    button(rect.x + rect.w - 24, rect.y + 2, 20, 20, "X", close)

    -- the toolbar: the search box, the way back to the player, the game's map
    local ty = rect.y + TITLE_H + 2
    local gm_w, cb_w = 150, 56
    local sw = clamp(rect.w - gm_w - cb_w - 30, 70, 220)
    local sx = rect.x + 8
    fill(sx, ty, sw, 22, c.black)
    frame(sx, ty, sw, 22, state.focus and c.red or c.border)
    local qtext = state.search or ""
    if qtext == "" and not state.focus then text(sx + 6, ty + 4, "Find a place...", c.muted, sw - 12)
    else text(sx + 6, ty + 4, qtext .. (state.focus and "_" or ""), c.text, sw - 12) end
    state.hits[#state.hits + 1] = { label = "search", x = sx, y = ty, w = sw, h = 22, run = function()
        state.focus = true
        state.note = nil
        state.search = state.search or ""
        state.results = search_results()
    end }
    button(sx + sw + 6, ty, cb_w, 22, "Center", function() state.following = true end, state.following)
    button(rect.x + rect.w - gm_w - 8, ty, gm_w, 22, "Use the game's map", game_map)

    -- the ground
    local info = t and t.texture
    fill(area.x, area.y, area.w, area.h, c.black)
    dl:PushClipRect(V(area.x, area.y), V(area.x + area.w, area.y + area.h), true)
    if info then
        local uv = info.uv
        dl:AddImage(info.texture, V(area.x, area.y), V(area.x + area.w, area.y + area.h),
            V(uv[1], uv[2]), V(uv[1] + (uv[3] - uv[1]) * area.w / pw, uv[2] + (uv[4] - uv[2]) * area.h / ph), c.white)
    elseif state.terrain_error then
        text(area.x + 10, area.y + 10, "The map picture is not available: " .. state.terrain_error, c.muted, area.w - 20)
    elseif t and t.error then
        text(area.x + 10, area.y + 10, "The map picture is not available: " .. tostring(t.error.message), c.muted, area.w - 20)
    end
    local world = map.world_id
    local function on_screen(sx2, sy2, r) return sx2 >= area.x - r and sx2 <= area.x + area.w + r and sy2 >= area.y - r and sy2 <= area.y + area.h + r end
    -- the game's grid of cells, where a cell is big enough to see
    local b = t and t.bounds
    if b and CELL / v.scale >= 48 then
        local x0 = math.floor(b.x_min / CELL)
        for col = 0, math.floor((b.x_max - b.x_min) / CELL) do
            local sx2 = to_screen(area, (x0 + col) * CELL, 0)
            if sx2 >= area.x and sx2 <= area.x + area.w then dl:AddLine(V(sx2, area.y), V(sx2, area.y + area.h), c.grid, 1) end
        end
        local z0 = math.floor(b.z_min / CELL)
        for row = 0, math.floor((b.z_max - b.z_min) / CELL) do
            local _, sy2 = to_screen(area, 0, (z0 + row) * CELL)
            if sy2 >= area.y and sy2 <= area.y + area.h then dl:AddLine(V(area.x, sy2), V(area.x + area.w, sy2), c.grid, 1) end
        end
    end
    if v.scale <= 12 then
        local dots = state.dots
        for i = 1, #dots.ids do
            local sx2, sy2 = to_screen(area, dots.xs[i], dots.zs[i])
            if on_screen(sx2, sy2, 3) and dots.ids[i] ~= state.self_id then dl:AddCircleFilled(V(sx2, sy2), 2, c.npc, 8) end
        end
    end
    local show_marks = map.markers_on ~= false -- the game's Map Markers setting: camps and coaches
    local town_boxes = {}
    for _, m in ipairs(state.markers) do
        local kind = m.kind
        if kind ~= "waypoint" and ((kind ~= "camp" and kind ~= "coach") or show_marks) then
            local sx2, sy2 = to_screen(area, m.x, m.z)
            if on_screen(sx2, sy2, 8) then
                local name = m.label or m.name
                if kind == "quest" then
                    -- "!" a quest to take, "?" a step to do
                    local mark = m.step and "?" or "!"
                    text(sx2 - 3, sy2 - 7, mark, c.black); text(sx2 - 4, sy2 - 8, mark, c.quest)
                elseif kind == "coach" then
                    dl:AddCircleFilled(V(sx2, sy2), 5, c.black, 12); dl:AddCircleFilled(V(sx2, sy2), 4, c.coach, 12)
                elseif kind == "camp" then
                    local big = name and name:find("(XP camp)", 1, true) and 5 or 3.5
                    dl:AddTriangleFilled(V(sx2, sy2 - big - 1), V(sx2 + big + 1, sy2), V(sx2, sy2 + big + 1), c.black)
                    dl:AddTriangleFilled(V(sx2, sy2 - big - 1), V(sx2, sy2 + big + 1), V(sx2 - big - 1, sy2), c.black)
                    dl:AddTriangleFilled(V(sx2, sy2 - big), V(sx2 + big, sy2), V(sx2, sy2 + big), c.camp)
                    dl:AddTriangleFilled(V(sx2, sy2 - big), V(sx2, sy2 + big), V(sx2 - big, sy2), c.camp)
                else
                    local col = kind == "group_waypoint" and c.group or c[kind] or c.group
                    dl:AddRectFilled(V(sx2 - 3.5, sy2 - 3.5), V(sx2 + 3.5, sy2 + 3.5), c.black)
                    dl:AddRectFilled(V(sx2 - 2.5, sy2 - 2.5), V(sx2 + 2.5, sy2 + 2.5), col)
                    if kind == "group_waypoint" then
                        local a = away(m.x, m.z)
                        text(sx2 + 7, sy2 - 6, (name or "Group") .. (a and string.format("  %.0f away", a) or ""), c.text, 160)
                    end
                end
                if kind == "town" and name then
                    -- a name that would cover a name already placed is left out
                    local box = { x = sx2 + 6, y = sy2 - 6, w = math.min(ui.CalcTextSize(name), 150), h = 13 }
                    local clear = true
                    for _, o in ipairs(town_boxes) do
                        if box.x < o.x + o.w and o.x < box.x + box.w and box.y < o.y + o.h and o.y < box.y + box.h then clear = false; break end
                    end
                    if clear then town_boxes[#town_boxes + 1] = box; text(box.x, box.y, name, c.text, 150) end
                end
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
    -- the group, in member colours by place in the group
    local grp = eqoa.group.get()
    if grp and grp.members then
        for i, m in ipairs(grp.members) do
            local mp = m.position
            if mp and m.world_id == world then
                local sx2, sy2 = to_screen(area, mp.x, mp.z)
                if on_screen(sx2, sy2, 8) then
                    dl:AddCircleFilled(V(sx2, sy2), 5, c.black, 12)
                    dl:AddCircleFilled(V(sx2, sy2), 3.5, c.members[(i - 1) % #c.members + 1], 12)
                end
            end
        end
    end
    -- the target's ring
    local target = eqoa.target.get()
    local tp = target and target.position
    if tp and (target.world_id == nil or target.world_id == world) then
        local sx2, sy2 = to_screen(area, tp.x, tp.z)
        if on_screen(sx2, sy2, 10) then dl:AddCircle(V(sx2, sy2), 8, c.target, 20, 2) end
    end
    -- the player, a dart facing the heading
    local pp = state.player
    if pp then
        local sx2, sy2 = to_screen(area, pp.x, pp.z)
        if on_screen(sx2, sy2, 10) then
            local h = state.heading or 0
            local fx, fy = math.sin(h), math.cos(h)
            local r = 8
            local tipx, tipy = sx2 + fx * r, sy2 + fy * r
            local lx, ly = sx2 - fx * r * .6 + fy * r * .6, sy2 - fy * r * .6 - fx * r * .6
            local rx, ry = sx2 - fx * r * .6 - fy * r * .6, sy2 - fy * r * .6 + fx * r * .6
            dl:AddTriangleFilled(V(tipx + fx, tipy + fy), V(lx - fy * 1.5, ly + fx * 1.5), V(rx + fy * 1.5, ry - fx * 1.5), c.black)
            dl:AddTriangleFilled(V(tipx, tipy), V(lx, ly), V(rx, ry), c.red)
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
    else foot = "Drag: pan  Wheel: zoom  Right-click: waypoint or pin  Esc: close" end
    text(rect.x + 8, fy, foot, c.muted, rect.w - GRIP - 20)

    -- the grip
    state.grip = { x = rect.x + rect.w - GRIP, y = rect.y + rect.h - GRIP, w = GRIP, h = GRIP }
    dl:AddTriangleFilled(V(rect.x + rect.w - 2, rect.y + rect.h - GRIP), V(rect.x + rect.w - 2, rect.y + rect.h - 2), V(rect.x + rect.w - GRIP, rect.y + rect.h - 2), c.border)

    -- search results
    if state.focus and #state.results > 0 then
        local rows = math.min(6, #state.results)
        local more = #state.results > rows
        local ry = ty + 24
        fill(sx, ry, sw, rows * 18 + 4 + (more and 14 or 0), c.surface)
        frame(sx, ry, sw, rows * 18 + 4 + (more and 14 or 0), c.border)
        for i = 1, rows do
            local r = state.results[i]
            local yy = ry + 2 + (i - 1) * 18
            if i == 1 then fill(sx + 2, yy, sw - 4, 17, c.selected) end
            text(sx + 6, yy + 2, r.name .. "  (" .. r.kind .. ")", c.text, sw - 12)
            state.hits[#state.hits + 1] = { label = r.name, x = sx, y = yy, w = sw, h = 17, run = function() go_to(r) end }
        end
        if more then text(sx + 6, ry + rows * 18 + 4, "+" .. (#state.results - rows) .. " more: type more letters", c.muted, sw - 12) end
    elseif state.focus and (state.search or "") ~= "" then
        local ry = ty + 24
        fill(sx, ry, sw, 22, c.surface); frame(sx, ry, sw, 22, c.border)
        text(sx + 6, ry + 4, "No matching place", c.muted, sw - 12)
    end

    -- the pin's note box
    if state.note then
        local n = state.note
        local bw, bh = 250, 54
        local bx, by = clamp(n.sx, area.x, math.max(area.x, area.x + area.w - bw)), clamp(n.sy, area.y, math.max(area.y, area.y + area.h - bh))
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
                local req, err = eqoa.map.set_waypoint({ world_id = mn.world, x = mn.wx, z = mn.wz, label = "World Map", keep = true })
                if req then state.expect[req] = "set" elseif err then say("Waypoint: " .. tostring(err.message or err.code)) end
            end },
            { "Drop a pin here...", function()
                local sx2, sy2 = to_screen(area, mn.wx, mn.wz)
                state.focus = nil
                state.note = { x = mn.wx, z = mn.wz, text = "", sx = sx2, sy = sy2, max = NOTE_MAX }
            end },
        }
        if map.waypoint then rows[#rows + 1] = { "Clear waypoint", function() state.remove_waypoint(map.waypoint) end } end
        local mw = 170
        local mx2, my2 = clamp(mn.x, area.x, math.max(area.x, area.x + area.w - mw)), clamp(mn.y, area.y, math.max(area.y, area.y + area.h - #rows * 20 - 4))
        fill(mx2, my2, mw, #rows * 20 + 4, c.surface); frame(mx2, my2, mw, #rows * 20 + 4, c.border)
        for i, r in ipairs(rows) do
            local yy = my2 + 2 + (i - 1) * 20
            text(mx2 + 8, yy + 3, r[1], c.text, mw - 16)
            local run = r[2]
            state.hits[#state.hits + 1] = { label = r[1], x = mx2, y = yy, w = mw, h = 20, run = function() state.menu = nil; run() end }
        end
    end
    -- hover card
    if pointer and pointer.inside_window and inside(pointer.x, pointer.y, area) and not state.menu and not state.note and not state.drag
        and not (state.focus and #state.results > 0) then
        local hit = under(area, pointer.x, pointer.y, world, map)
        state.card = hit
        if hit then
            local lines = {}
            for _, l in ipairs(hit.lines) do if l[1] and l[1] ~= "" then lines[#lines + 1] = l end end
            local w, h = 0, 6
            for _, l in ipairs(lines) do local tw, th = ui.CalcTextSize(l[1]); w = math.max(w, tw); h = h + th + 2 end
            w = math.min(w + 16, 280)
            local cx = clamp(hit.sx + 14, area.x + 2, math.max(area.x + 2, area.x + area.w - w - 2))
            local cy = clamp(hit.sy - h / 2, area.y + 2, math.max(area.y + 2, area.y + area.h - h - 2))
            fill(cx, cy, w, h, c.surface); frame(cx, cy, w, h, c.border)
            local yy = cy + 4
            for _, l in ipairs(lines) do
                text(cx + 8, yy, l[1], l[2] == "title" and c.title or l[2] == "muted" and c.muted or c.card, w - 16)
                local _, th = ui.CalcTextSize(l[1])
                yy = yy + th + 2
            end
        end
    else
        state.card = nil
    end

    ui.SetWindowFontScale(1)
    if font then ui.PopFont() end
    ui.End()
end
