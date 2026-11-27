-- World Map: the game's map in a movable, resizable window of this mod's own, in the dark/red look of the minimap mods,
-- with pins and a place search.
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The map key opens it (the mod holds the "map" window); "Use the game's map" (or turning the mod off) gives the game's own
-- map back. The ground is the game's own map picture for the view (eqoa.map.terrain), so it is as sharp as the game's.
local ui = eqoa.imgui
local ID = "eqoa_world_map"

-- One size rule: every length below is a base value (the numbers at the game's 640 x 448 frame) times one factor F =
-- (the client's UI scale, as the other mods read it: eqoa.hud.frame) x (the player's "Text size"). Markers use the
-- same scale times the player's "Marker size". `set_factor` turns the bases into pixels, once a frame.
local BASE_MIN_W, BASE_MIN_H = 360, 260
local BASE_TITLE_H, BASE_TOOL_H, BASE_FOOT_H, BASE_GRIP = 24, 28, 18, 14
local TEXT_BASE = 11 -- the text's size in base pixels: what the Auction and Bag Viewer windows use
local SIZE_NAMES = { "Small", "Normal", "Large", "Very large" }
local SIZE_MULT = { 0.85, 1, 1.3, 1.7 }
local MIN_W, MIN_H = BASE_MIN_W, BASE_MIN_H
local MAX_AREA = 2048 -- the picture the client makes is at most this many pixels across
local TITLE_H, TOOL_H, FOOT_H, GRIP = BASE_TITLE_H, BASE_TOOL_H, BASE_FOOT_H, BASE_GRIP
local F, FM, UI_K = 1, 1, 1 -- text/layout factor, marker factor, the client's UI scale
local NOTE_MAX, PINS_MAX = 40, 60
local ZOOM_STEP = 1.25
-- The closest zoom, in world units a pixel. The client makes the ground picture at 1 unit a pixel at the finest (the game's
-- 512-texel detail tiles are 3.9 units a texel, so 3.9 pixels a texel there); beyond that this mod asks for the picture at
-- 1 and draws it larger, up to MIN_SCALE = 0.25: 4 times larger, 15.6 pixels a texel, about 200 units across a 760-pixel
-- window (a street in a city). Past that the tiles are only blocks.
local MIN_SCALE = 0.25
local TILE_SCALE = 7.8 -- the game's map draws its detail tiles at or below this many world units a pixel
local CELL = 2000
local FONT_PATH = "package_resources/SpaceMono-Regular.ttf"
-- The font is baked at the size drawn (a font baked small and scaled up is blurry). `cur` is the one in use, `pending` the
-- one being baked for a new size: the old one is scaled to the new size until the new one is ready.
local fonts = { cur = nil, pending = nil }
do
    local info = eqoa.fonts.load(FONT_PATH, 13)
    if info and info.font then fonts.cur = { font = info.font, px = 13 } end
end
local KEYS = { "Escape", "Enter", "w", "a", "s", "d", "W", "A", "S", "D" }

local C = nil -- packed colours, made on the first frame
local state = {
    session = nil, tokens = {}, claims = {},
    win = { x = nil, y = nil, w = nil, h = nil }, -- w, h: nil until the player drags them (then a default times F)
    opts = { text = 2, marker = 2 }, show_sizes = false,
    pins = {}, -- world id (as text) -> { { x, z, note } }
    view = nil, -- { x, z, scale }: the world point at the middle of the map area and the world units a pixel
    last_scale = nil, following = true,
    terrain = nil, terrain_size = nil,
    hits = {}, rect = nil, drag = nil, held = {},
    menu = nil, note = nil, search = nil, results = {}, focus = nil, capturing = false,
    markers = {}, markers_world = nil, markers_stale = true, markers_on = true, marker_calls = 0,
    player = nil, dots = { ids = {}, xs = {}, zs = {} }, dot_cache = {}, dot_cache_at = 0, drawn_dots = {},
    now = 0, message = nil, message_until = 0, overflow = -1e9, expect = {},
}
world_map = state -- read by the mod's tests

-- The live dots in the game's minimap colours (`Play::mini_view`): a friendly NPC with a role (Minimap Colors, `minimapcolor::role`)
-- in the role's default colour, else its name plate's colour (`Play::name_color`, `play.rs` 5548): a creature with code 0 or 1
-- tan; a player, a pet or any other code from 2 up blue (a mercenary a reddish blue). The game paints a player flagged for PvP
-- green; the flag is not readable by a mod, so that player is blue here. The API gives no Minimap Colors choice and no
-- spell-merchant list from the server, so the role colours are the defaults and a spell merchant is one with "Spell" in its
-- name. Bit tests by division: no bit operators needed.
local function rgb8(r, g, b) return { r / 255, g / 255, b / 255 } end
local TAN, BLUE, MERC = rgb8(212, 192, 111), rgb8(100, 175, 220), rgb8(155, 165, 210)
local ROLE_RGB = {
    spells = rgb8(176, 96, 255), smiths = rgb8(255, 140, 0), merchants = rgb8(255, 236, 40), bankers = rgb8(0, 240, 240),
    trainers = rgb8(255, 110, 199), coachmen = rgb8(130, 190, 255), quest = rgb8(255, 255, 255),
}
local SMITHS = { Smithy = 1, Smith = 1, Blacksmith = 1, Armorer = 1, Armourer = 1, Weaponsmith = 1, Shieldsmith = 1, Bowyer = 1, Fletcher = 1 }
local TRAINERS = { Guildmaster = 1, Trainer = 1, Training = 1, Master = 1, Rehabilitator = 1, Guild = 1 }
local function has_bit(n, bit) return math.floor((n or 0) / bit) % 2 == 1 end

local function role_of(e)
    local flags = e.npc_flags or 0
    if not has_bit(flags, 128) then return nil end
    local name = e.name or ""
    local first = name:match("%S+") or ""
    local merchant = has_bit(flags, 1)
    if name:sub(1, 9) == "Coachman " or name:sub(1, 11) == "Coachwoman " then return "coachmen" end
    if has_bit(flags, 2) or first == "Banker" then return "bankers" end
    if merchant and name:find("Spell", 1, true) then return "spells" end
    if has_bit(flags, 8) or (merchant and SMITHS[first]) then return "smiths" end
    if TRAINERS[first] or name:sub(1, 12) == "Guild Master" then return "trainers" end
    if e.head_mark then return "quest" end
    return merchant and "merchants" or nil
end

-- the minimap's colour for an entity, as {r, g, b}
function state.dot_rgb(e)
    -- a creature with code 0 or 1 (kind "npc") is an NPC; a player, a pet or another code from 2 up is blue
    if e.kind == "player" or e.kind == "pet" or (e.kind == "other" and (e.class_code or 0) >= 2) then
        return e.merc_tag and MERC or BLUE
    end
    return ROLE_RGB[role_of(e) or ""] or TAN
end

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

-- everything the mod keeps, always whole (a partial table would drop the rest)
local function snapshot()
    return { version = 1, win = state.win, pins = state.pins, scale = state.last_scale,
             text_size = state.opts.text, marker_size = state.opts.marker }
end
local function persist()
    local ok, err = eqoa.settings.save(snapshot())
    state.save_error = not ok and err and (err.code .. ": " .. tostring(err.message)) or nil
    if state.save_error then say("The map could not save your pins: " .. tostring(err and err.message)) end
end
state.persist = persist -- read by the mod's tests
eqoa.settings.on_save(snapshot)
eqoa.settings.on_load(function(saved)
    if type(saved) ~= "table" then return end
    for key, field in pairs({ text = "text_size", marker = "marker_size" }) do
        local n = saved[field]
        state.opts[key] = (type(n) == "number" and n >= 1 and n <= #SIZE_NAMES and n == math.floor(n)) and n or 2
    end
    local w = saved.win
    if type(w) == "table" then
        for _, k in ipairs({ "x", "y", "w", "h" }) do
            if type(w[k]) == "number" and w[k] == w[k] and math.abs(w[k]) < 1e5 then state.win[k] = w[k] end
        end
        if state.win.w then state.win.w = clamp(state.win.w, BASE_MIN_W, MAX_AREA + 2) end
        if state.win.h then state.win.h = clamp(state.win.h, BASE_MIN_H, MAX_AREA + 80) end
    end
    if type(saved.scale) == "number" and saved.scale == saved.scale and saved.scale >= MIN_SCALE and saved.scale < 1e5 then state.last_scale = saved.scale end
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
    -- the minimap (the round one steps aside for any hide claim) comes back with the window's end
    if state.minimap_claim then eqoa.hud.show(state.minimap_claim); state.minimap_claim = nil end
    for _, token in ipairs(state.tokens) do eqoa.events.off(token) end
    state.tokens = {}
    release_keys()
    release_terrain()
    set_capture(false)
    state.session, state.hits, state.rect, state.drag = nil, {}, nil, nil
    state.menu, state.note, state.search, state.focus, state.held = nil, nil, nil, nil, {}
    state.results, state.found, state.view, state.expect = {}, nil, nil, {}
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

-- Creatures and group members for the box, from the same reads Map Search (F2) takes its live rows from: the creatures the
-- client sees now (the filter Map Search's `live_valid` uses: alive, a plain creature or NPC, not searchable loot or a corpse),
-- then the group's members, nearest first. Recorded spawn locations come from Map Search's catalogue, a file of that mod that
-- no other mod can read, so they are not offered here. At least two letters, so one letter does not list the zone.
local CREATURE_MOST = 20
local function creature_results(q, map)
    local out = {}
    if #q < 2 then return out end
    local me = eqoa.player.get()
    local here = me and me.position
    local function dist(x, z) return here and math.sqrt((x - here.x) ^ 2 + (z - here.z) ^ 2) or 0 end
    local seen = {}
    local named = eqoa.entities.named(q, false)
    for _, e in ipairs(named or {}) do
        local p = e.position
        if e.is_creature and not e.corpse and e.hp_fraction and e.hp_fraction > 0 and e.search_flags == 0 and e.class_code
            and e.class_code < 2 and e.name and p and (e.world_id == nil or e.world_id == map.world_id) then
            out[#out + 1] = { name = e.name, kind = (e.level and e.level > 0) and ("creature, level " .. e.level) or "creature",
                x = p.x, z = p.z, d = dist(p.x, p.z) }
            seen[e.id] = true
        end
    end
    table.sort(out, function(a, b) if a.d ~= b.d then return a.d < b.d end return a.name < b.name end)
    while #out > CREATURE_MOST do out[#out] = nil end
    local group = eqoa.group.get()
    for _, g in ipairs(group and group.members or {}) do
        local p = g.position
        if g.name and g.name:lower():find(q, 1, true) and g.active and p and g.world_id == map.world_id and not seen[g.id] then
            out[#out + 1] = { name = g.name, kind = "group", x = p.x, z = p.z, d = dist(p.x, p.z) }
        end
    end
    return out
end

local function search_results()
    local out = {}
    local q = (state.search or ""):lower()
    if q == "" then return out end
    local map = eqoa.map.get()
    if not map then return out end
    out = creature_results(q, map)
    local creatures = #out
    refresh_markers(map.world_id)
    for _, m in ipairs(state.markers) do
        local name = m.label or m.name
        if name and name ~= "" and (m.kind == "camp" or m.kind == "coach" or m.kind == "town" or m.kind == "quest" or m.kind == "community")
            and name:lower():find(q, 1, true) then
            out[#out + 1] = { name = name, kind = m.kind, x = m.x, z = m.z }
            if #out - creatures >= 40 then break end
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
    -- the minimap goes while this window is up (its corner would draw over the map)
    if not state.minimap_claim then state.minimap_claim = eqoa.hud.hide("minimap") end
    state.held, state.menu, state.note, state.search, state.focus = {}, nil, nil, nil, nil
    state.markers_stale = true
    state.following = true
    claim_keys()
    listen("render", draw)
    listen("input.key", function(e) on_key(e.data) end)
    listen("pointer.pressed", function(e) state.on_press(e.data) end)
    listen("pointer.wheel", function(e) state.on_wheel(e.data) end)
    -- what the mod asked of the waypoint is answered later: say what was refused
    listen("action.result", function(e) state.on_result(e.data) end)
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
    local sw = clamp(w.w or 760 * F, MIN_W, math.max(MIN_W, math.min(dw - 8, MAX_AREA + 2)))
    local sh = clamp(w.h or 540 * F, MIN_H, math.max(MIN_H, math.min(dh - 8, MAX_AREA + TITLE_H + TOOL_H + FOOT_H)))
    local sx = clamp(w.x or (dw - sw) / 2, 0, math.max(0, dw - sw))
    local sy = clamp(w.y or (dh - sh) / 2, 0, math.max(0, dh - sh))
    local area = { x = sx + 1, y = sy + TITLE_H + TOOL_H, w = sw - 2, h = math.max(1, sh - TITLE_H - TOOL_H - FOOT_H) }
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
    local t, err = eqoa.map.terrain({ center = { x = v.x, z = v.z }, scale = math.max(v.scale or 8, 1), width = pw, height = ph })
    state.terrain, state.terrain_size = t, t and key or nil
    state.terrain_error = (not t and err) and err.message or nil
    return t, pw, ph
end

local function fit_limits(t, area)
    local b = t and t.bounds
    if not b then return MIN_SCALE, 400 end
    local fit = math.max((b.x_max - b.x_min) / area.w, (b.z_max - b.z_min) / area.h) * 1.1
    return MIN_SCALE, math.max(fit, 2)
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
        local r2 = (5 * FM * v.scale) ^ 2
        local self_id = state.self_id
        for i = 1, #dots.ids do
            local dx, dz = dots.xs[i] - px, dots.zs[i] - pz
            if dx * dx + dz * dz <= r2 and dots.ids[i] ~= self_id then
                local id = dots.ids[i]
                try(dots.xs[i], dots.zs[i], 5 * FM, function()
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
        if name and name ~= "" and m.kind ~= "waypoint" and (state.markers_on or (m.kind ~= "camp" and m.kind ~= "coach" and m.kind ~= "community")) then
            local sx, sy = to_screen(area, m.x, m.z)
            if (sx - mx) ^ 2 + (sy - my) ^ 2 <= (7 * FM) ^ 2 then
                try(m.x, m.z, 7 * FM, function()
                    local lines = { { m.kind == "camp" and (m.name or name) or name, "title" } }
                    if m.kind == "quest" then lines[#lines + 1] = { m.step and "Quest step" or "Quest to take", "text" } end
                    local lv = levels(m)
                    if lv then lines[#lines + 1] = { lv, "text" } end
                    return lines
                end)
            end
        end
    end
    for i, p in ipairs(pins_here()) do
        try(p.x, p.z, 8 * FM, function() return { { p.note ~= "" and p.note or "Pin", "title" }, { "Right-click to remove", "muted" } } end,
            function() table.remove(pins_here(), i); persist() end)
    end
    local wp = map and map.waypoint
    if wp and wp.world_id == world then
        try(wp.x, wp.z, 9 * FM, function() return { { wp.label and wp.label ~= "" and wp.label or "Waypoint", "title" }, { "Right-click to remove", "muted" } } end,
            function() state.remove_waypoint(wp) end)
    end
    if not best then return nil end
    local lines = best.make and best.make() or {}
    if #lines == 0 then lines = { { "?", "title" } } end
    lines[#lines + 1] = { away_text(best.x, best.z), "text" }
    best.lines = lines
    return best
end

-- A request id is an object: an answer is matched with ==, never by a table key.
local function expect(req, what) state.expect[#state.expect + 1] = { id = req, what = what } end
function state.on_result(d)
    if not d then return end
    for i, want in ipairs(state.expect) do
        if d.request_id == want.id then
            table.remove(state.expect, i)
            if d.ok == false then
                say("Waypoint: " .. tostring(d.reason or d.code))
            elseif want.what == "clear_after" then
                local req, err = eqoa.map.clear_waypoint()
                if req then expect(req, "clear") elseif err then say("Waypoint: " .. tostring(err.message)) end
            end
            return
        end
    end
end

-- Removing the waypoint: this mod can clear only a pin it placed and still owns. One it did not place (the player's
-- own, another mod's, one from before a restart) is first set again at the same point, which takes it over, and cleared
-- when that is answered.
function state.remove_waypoint(wp)
    -- always set again first, even for a pin this mod placed: `clear_waypoint` clears the pin placed most recently in
    -- any world, so this one must be the most recent
    local req, err = eqoa.map.set_waypoint({ world_id = wp.world_id, x = wp.x, z = wp.z, label = wp.label, keep = true })
    if req then expect(req, "clear_after") else say("Waypoint: " .. tostring(err and err.message)) end
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
        -- the game's own (`member_color`), by place in the group with the player 0: 1 green, 2 blue, 3 cyan, the rest yellow
        members = { ui.GetColorU32(0, 1, 0, 1), ui.GetColorU32(.2, .4, 1, 1), ui.GetColorU32(0, 1, 1, 1), ui.GetColorU32(1, 1, 0, 1) },
    }
end

-- A dot's colour, worked out about once a second for each creature, each at its own moment (a record read for every dot every
-- frame would cost more than it shows, and all of them together would be one slow frame). A creature that is gone is dropped
-- (`prune_dots`). TAN while the record is not there, as upstream's name plate colour for a creature it cannot place.
local function dot_colour(id)
    local k = state.dot_cache[id]
    if k and state.now - k.at < 1 + (id % 8) / 8 then return k.col end
    local e = eqoa.entities.find(id)
    local rgb = e and state.dot_rgb(e) or TAN
    local col = ui.GetColorU32(rgb[1], rgb[2], rgb[3], 1)
    state.dot_cache[id] = { col = col, at = state.now }
    return col
end

-- every ten seconds the cache keeps only the creatures that are still dots
local function prune_dots()
    if state.now - state.dot_cache_at <= 10 then return end
    local keep = {}
    for _, id in ipairs(state.dots.ids) do keep[id] = state.dot_cache[id] end
    state.dot_cache, state.dot_cache_at = keep, state.now
end

-- the factors, from the client's UI scale and the two settings; the layout's pixels follow
local function set_factor()
    local f = eqoa.hud.frame()
    local k = f and math.min(f.scale_x or 1, f.scale_y or 1) or 1
    if not (k > 0) then k = 1 end
    UI_K = k
    F = k * SIZE_MULT[state.opts.text]
    FM = k * SIZE_MULT[state.opts.marker]
    MIN_W, MIN_H = BASE_MIN_W * F, BASE_MIN_H * F
    TITLE_H, TOOL_H, FOOT_H, GRIP = BASE_TITLE_H * F, BASE_TOOL_H * F, BASE_FOOT_H * F, BASE_GRIP * F
    state.F, state.FM, state.ui_k = F, FM, k -- read by the mod's tests
end

-- bake the font at the size drawn; one request at a time, the old size scaled meanwhile
eqoa.events.on("resource.ready", function(e)
    local d = e.data
    if d and d.kind == "font" and fonts.pending then
        if fonts.cur and fonts.cur.font then eqoa.fonts.release(fonts.cur.font) end
        fonts.cur, fonts.pending = fonts.pending, nil
    end
end)
local function ensure_font(want)
    if fonts.cur and fonts.cur.px == want then return end
    if fonts.pending then return end
    local info = eqoa.fonts.load(FONT_PATH, want)
    if not (info and info.font) then return end
    if info.status == "fallback" then eqoa.fonts.release(info.font); return end
    local made = { font = info.font, px = want }
    if info.status == "ready" then
        if fonts.cur and fonts.cur.font then eqoa.fonts.release(fonts.cur.font) end
        fonts.cur = made
    else
        fonts.pending = made
    end
end

-- the small window for the two sizes (a row in Mods & Addons opens it)
eqoa.options.button({ id = "sizes", label = "World Map: text and marker size..." }, function() state.show_sizes = not state.show_sizes end)
local function sizes_window()
    if not state.show_sizes then return end
    ui.SetNextWindowPos(80, 80, 4)
    if not ui.Begin("World Map sizes") then ui.End() return end
    local o = state.opts
    local t = ui.SliderInt("Text size", o.text, 1, #SIZE_NAMES)
    ui.Text(SIZE_NAMES[t] or SIZE_NAMES[2])
    local m = ui.SliderInt("Marker size", o.marker, 1, #SIZE_NAMES)
    ui.Text(SIZE_NAMES[m] or SIZE_NAMES[2])
    t, m = clamp(math.floor(t or o.text), 1, #SIZE_NAMES), clamp(math.floor(m or o.marker), 1, #SIZE_NAMES)
    if t ~= o.text or m ~= o.marker then o.text, o.marker = t, m; persist() end
    if ui.Button("Close") then state.show_sizes = false end
    ui.End()
end
eqoa.events.on("render", sizes_window)

function draw(e)
    if not state.session then return end
    local dw, dh = ui.GetDisplaySize()
    if dw <= 0 or dh <= 0 then return end
    state.now = e and e.time or state.now
    local dt = clamp(e and e.data and e.data.dt or 1 / 60, 0, 0.1)
    set_factor()
    local rect, area = geometry(dw, dh)
    state.rect, state.area = rect, area
    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_world_map_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove
        + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    -- the window is the map's whatever else happens this frame: clicks on it never reach the game. Claimed inside the
    -- ImGui window being built, which is what puts it in the window order (the front one is the one that is clicked).
    eqoa.input.claim_pointer({ x = rect.x, y = rect.y, width = rect.w, height = rect.h })
    local map = eqoa.map.get()
    if not map then ui.End(); return end
    C = C or colours()
    local c = C
    local pointer = eqoa.input.pointer()

    -- one read of the player a frame; keys let go when the window lost the keyboard; claims the map could not take asked again
    local pl = eqoa.player.get()
    state.player = pl and pl.position or nil
    state.heading = pl and pl.heading
    state.markers_on = map.markers_on ~= false
    state.self_id = pl and pl.id
    local inp = eqoa.input.get()
    -- a key-up the game never delivers (the window lost focus, a game window or a text field took the keyboard) must not
    -- leave W held
    local wins = eqoa.windows.get()
    if (inp and (inp.focused == false or inp.text_capture)) or (wins and wins.busy) then state.held = {} end
    if math.floor(state.now) ~= state.claim_tick then state.claim_tick = math.floor(state.now); claim_keys() end

    -- the view: the player's area at the game's opening zoom (or where it was left), following the player until moved
    local v = state.view
    if not v then
        local p = state.player
        if not p then ui.End(); return end
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
        if v.scale >= 1 then
            t.set_view({ center = { x = v.x + (pw - area.w) / 2 * v.scale, z = v.z + (ph - area.h) / 2 * v.scale }, scale = v.scale })
            state.mag = 1
        else
            -- closer than the client's finest picture: the picture at 1 a pixel, centred on the view, drawn 1/scale larger
            t.set_view({ center = { x = v.x, z = v.z }, scale = 1 })
            state.mag = 1 / v.scale
        end
    end
    refresh_markers(map.world_id)
    if state.focus or state.note then set_capture(true) else set_capture(false) end
    if v.scale <= 12 then state.dots = eqoa.entities.dots() or state.dots end
    prune_dots()

    -- the font: baked at the size drawn (or the old size scaled until the new one is ready)
    local want_px = clamp(math.floor(TEXT_BASE * F + 0.5), 6, 96)
    ensure_font(want_px)
    local cur = fonts.cur
    if cur and cur.font then ui.PushFont(cur.font) end
    ui.SetWindowFontScale(want_px / ((cur and cur.px) or 13))
    state.text_px = want_px
    local lh = select(2, ui.CalcTextSize("Mg"))
    state.line_h = lh
    local function tw(value) return (ui.CalcTextSize(value)) end
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
    -- text cut to `maxw` ends in "..." (the tests read how far it reaches past the window)
    local function text(x, y, value, col, maxw)
        value = tostring(value)
        if maxw and tw(value) > maxw then
            while #value > 0 and tw(value .. "...") > maxw do
                value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
            end
            value = value .. "..."
            if tw(value) > maxw then value = "" end
        end
        dl:AddText(V(x, y), col or c.text, value)
        local over = x + tw(value) - (rect.x + rect.w)
        if over > state.overflow then state.overflow = over end
    end
    -- the same, centred up and down in a box of height h
    local function ctext(x, y, h, value, col, maxw) text(x, y + (h - lh) / 2, value, col, maxw) end
    state.hits = {}
    state.overflow = -1e9
    state.marker_calls = 0
    state.bar = {} -- the top bar's and the footer's boxes, by name (the tests read them)
    local function button(x, y, w, h, label, run, active)
        fill(x, y, w, h, active and c.selected or c.raised)
        frame(x, y, w, h, active and c.red or c.border)
        ctext(x + 6 * F, y, h, label, c.text, w - 10 * F)
        state.hits[#state.hits + 1] = { x = x, y = y, w = w, h = h, run = run, label = label }
        state.bar[label] = { x = x, y = y, w = w, h = h }
    end

    -- the window
    fill(rect.x, rect.y, rect.w, rect.h, c.surface)
    frame(rect.x, rect.y, rect.w, rect.h, c.border)
    local title = "WORLD MAP  " .. (map.world_name or "World")
    if map.zone and map.zone.name and map.zone.name ~= "" then title = title .. ": " .. map.zone.name end
    local xs = 20 * F
    ctext(rect.x + 10 * F, rect.y, TITLE_H, title, c.muted, rect.w - xs - 24 * F)
    state.bar.title = { x = rect.x + 10 * F, y = rect.y, w = math.min(tw(title), rect.w - xs - 24 * F), h = TITLE_H }
    button(rect.x + rect.w - xs - 4 * F, rect.y + 2 * F, xs, xs, "X", close)

    -- the toolbar: the search box, the way back to the player, the game's map. Widths follow the words; when they do not
    -- all fit, the search box gives way first, then the game's-map button.
    local th = 22 * F
    local ty = rect.y + TITLE_H + 2 * F
    local gap, margin = 6 * F, 8 * F
    local cb_w = tw("Center") + 14 * F
    local gm_w = tw("Use the game's map") + 14 * F
    local avail = rect.w - 2 * margin
    local sw = clamp(avail - cb_w - gm_w - 2 * gap, 70 * F, 220 * F)
    if sw + cb_w + gm_w + 2 * gap > avail then
        local excess = sw + cb_w + gm_w + 2 * gap - avail
        gm_w = gm_w - math.min(excess, math.max(0, gm_w - 44 * F))
        sw = math.max(12 * F, math.min(sw, avail - cb_w - gm_w - 2 * gap))
    end
    local sx = rect.x + margin
    fill(sx, ty, sw, th, c.black)
    frame(sx, ty, sw, th, state.focus and c.red or c.border)
    local qtext = state.search or ""
    if qtext == "" and not state.focus then ctext(sx + 6 * F, ty, th, "Find a place or creature...", c.muted, sw - 12 * F)
    else ctext(sx + 6 * F, ty, th, qtext .. (state.focus and "_" or ""), c.text, sw - 12 * F) end
    state.hits[#state.hits + 1] = { label = "search", x = sx, y = ty, w = sw, h = th, run = function()
        state.focus = true
        state.note = nil
        state.search = state.search or ""
        state.results = search_results()
    end }
    state.bar.search = { x = sx, y = ty, w = sw, h = th }
    button(sx + sw + gap, ty, cb_w, th, "Center", function() state.following = true end, state.following)
    button(rect.x + rect.w - margin - gm_w, ty, gm_w, th, "Use the game's map", game_map)

    -- the ground
    local info = t and t.texture
    fill(area.x, area.y, area.w, area.h, c.black)
    dl:PushClipRect(V(area.x, area.y), V(area.x + area.w, area.y + area.h), true)
    if info then
        local uv = info.uv
        if (state.mag or 1) > 1 then
            -- the middle of the picture, as much of it as the area shows at this magnification
            local fu, fv = area.w / (state.mag * pw), area.h / (state.mag * ph)
            local cu, cv = (uv[1] + uv[3]) / 2, (uv[2] + uv[4]) / 2
            local hu, hv = (uv[3] - uv[1]) * fu / 2, (uv[4] - uv[2]) * fv / 2
            dl:AddImage(info.texture, V(area.x, area.y), V(area.x + area.w, area.y + area.h),
                V(cu - hu, cv - hv), V(cu + hu, cv + hv), c.white)
        else
        dl:AddImage(info.texture, V(area.x, area.y), V(area.x + area.w, area.y + area.h),
            V(uv[1], uv[2]), V(uv[1] + (uv[3] - uv[1]) * area.w / pw, uv[2] + (uv[4] - uv[2]) * area.h / ph), c.white)
        end
    elseif state.terrain_error then
        text(area.x + 10 * F, area.y + 10 * F, "The map picture is not available: " .. state.terrain_error, c.muted, area.w - 20 * F)
    elseif t and t.error then
        text(area.x + 10 * F, area.y + 10 * F, "The map picture is not available: " .. tostring(t.error.message), c.muted, area.w - 20 * F)
    end
    local world = map.world_id
    local function on_screen(sx2, sy2, r) r = r * FM; return sx2 >= area.x - r and sx2 <= area.x + area.w + r and sy2 >= area.y - r and sy2 <= area.y + area.h + r end
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
        -- the group's members are drawn below with their own colours, as the minimap leaves them out of its dots
        local grouped = {}
        local grp0 = eqoa.group.get()
        for _, m in ipairs(grp0 and grp0.members or {}) do grouped[m.id] = true end
        state.drawn_dots = {}
        for i = 1, #dots.ids do
            local id = dots.ids[i]
            local sx2, sy2 = to_screen(area, dots.xs[i], dots.zs[i])
            if on_screen(sx2, sy2, 3) and id ~= state.self_id and not grouped[id] then
                local col = dot_colour(id)
                state.drawn_dots[id] = col
                dl:AddCircleFilled(V(sx2, sy2), 2 * FM, col, 8)
            end
        end
    end
    local show_marks = state.markers_on -- the game's Map Markers setting: camps, coaches and community marks
    local town_boxes = {}
    local lw = math.max(1, math.floor(2 * FM + 0.5))
    for _, m in ipairs(state.markers) do
        local kind = m.kind
        if kind ~= "waypoint" and ((kind ~= "camp" and kind ~= "coach" and kind ~= "community") or show_marks) then
            local sx2, sy2 = to_screen(area, m.x, m.z)
            if on_screen(sx2, sy2, 8) then
                local name = m.label or m.name
                if kind == "quest" then
                    -- "!" a quest to take, "?" a step to do, drawn in the text's size
                    local mark = m.step and "?" or "!"
                    local ox, oy = sx2 - tw(mark) / 2, sy2 - lh / 2
                    text(ox + 1, oy + 1, mark, c.black); text(ox, oy, mark, c.quest)
                    state.marker_calls = state.marker_calls + 2
                elseif kind == "coach" then
                    dl:AddCircleFilled(V(sx2, sy2), 5 * FM, c.black, 12); dl:AddCircleFilled(V(sx2, sy2), 4 * FM, c.coach, 12)
                    state.marker_calls = state.marker_calls + 2
                elseif kind == "camp" then
                    -- one diamond a layer: the world in view holds about 280 camps
                    dl:AddNgonFilled(V(sx2, sy2), 4.8 * FM, c.black, 4)
                    dl:AddNgonFilled(V(sx2, sy2), 3.6 * FM, c.camp, 4)
                    state.marker_calls = state.marker_calls + 2
                else
                    local col = kind == "group_waypoint" and c.group or c[kind] or c.group
                    dl:AddRectFilled(V(sx2 - 3.5 * FM, sy2 - 3.5 * FM), V(sx2 + 3.5 * FM, sy2 + 3.5 * FM), c.black)
                    dl:AddRectFilled(V(sx2 - 2.5 * FM, sy2 - 2.5 * FM), V(sx2 + 2.5 * FM, sy2 + 2.5 * FM), col)
                    state.marker_calls = state.marker_calls + 2
                    if kind == "group_waypoint" then
                        local a = away(m.x, m.z)
                        text(sx2 + 7 * FM, sy2 - lh / 2, (name or "Group") .. (a and string.format("  %.0f away", a) or ""), c.text, 160 * F)
                    end
                end
                if kind == "town" and name then
                    -- a name that would cover a name already placed is left out
                    local box = { x = sx2 + 6 * FM, y = sy2 - lh / 2, w = math.min(tw(name), 150 * F), h = lh }
                    local clear = true
                    for _, o in ipairs(town_boxes) do
                        if box.x < o.x + o.w and o.x < box.x + box.w and box.y < o.y + o.h and o.y < box.y + box.h then clear = false; break end
                    end
                    if clear then town_boxes[#town_boxes + 1] = box; text(box.x, box.y, name, c.text, 150 * F) end
                end
            end
        end
    end
    for _, p in ipairs(pins_here()) do
        local sx2, sy2 = to_screen(area, p.x, p.z)
        if on_screen(sx2, sy2, 10) then
            dl:AddTriangleFilled(V(sx2, sy2), V(sx2 - 5 * FM, sy2 - 11 * FM), V(sx2 + 5 * FM, sy2 - 11 * FM), c.black)
            dl:AddTriangleFilled(V(sx2, sy2 - 1.5 * FM), V(sx2 - 3.5 * FM, sy2 - 10 * FM), V(sx2 + 3.5 * FM, sy2 - 10 * FM), c.red)
            if p.note ~= "" then text(sx2 + 8 * FM, sy2 - 11 * FM - lh / 2, p.note, c.text, 160 * F) end
        end
    end
    local wp = map.waypoint
    if wp and wp.world_id == world then
        local sx2, sy2 = to_screen(area, wp.x, wp.z)
        if on_screen(sx2, sy2, 12) then
            dl:AddCircleFilled(V(sx2, sy2), 7 * FM, c.black, 16)
            dl:AddCircleFilled(V(sx2, sy2), 5 * FM, c.waypoint, 16)
            local a = away(wp.x, wp.z)
            if a then text(sx2 + 9 * FM, sy2 - lh / 2, string.format("%.0f away", a), c.text, 120 * F) end
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
                    dl:AddCircleFilled(V(sx2, sy2), 5 * FM, c.black, 12)
                    dl:AddCircleFilled(V(sx2, sy2), 3.5 * FM, c.members[math.min(i, #c.members)], 12)
                end
            end
        end
    end
    -- the target's ring
    local target = eqoa.target.get()
    local tp = target and target.position
    if tp and (target.world_id == nil or target.world_id == world) then
        local sx2, sy2 = to_screen(area, tp.x, tp.z)
        if on_screen(sx2, sy2, 10) then dl:AddCircle(V(sx2, sy2), 8 * FM, c.target, 20, lw) end
    end
    -- the player, a dart facing the heading
    local pp = state.player
    if pp then
        local sx2, sy2 = to_screen(area, pp.x, pp.z)
        if on_screen(sx2, sy2, 10) then
            local h = state.heading or 0
            local fx, fy = math.sin(h), math.cos(h)
            local r = 8 * FM
            local tipx, tipy = sx2 + fx * r, sy2 + fy * r
            local lx, ly = sx2 - fx * r * .6 + fy * r * .6, sy2 - fy * r * .6 - fx * r * .6
            local rx, ry = sx2 - fx * r * .6 - fy * r * .6, sy2 - fy * r * .6 + fx * r * .6
            local o = 1.5 * FM
            dl:AddTriangleFilled(V(tipx + fx, tipy + fy), V(lx - fy * o, ly + fx * o), V(rx + fy * o, ry - fx * o), c.black)
            dl:AddTriangleFilled(V(tipx, tipy), V(lx, ly), V(rx, ry), c.red)
        end
    end
    if state.found and state.found.until_ > state.now then
        local sx2, sy2 = to_screen(area, state.found.x, state.found.z)
        dl:AddCircle(V(sx2, sy2), 12 * FM, c.title, 24, lw)
    end
    dl:PopClipRect()
    frame(area.x, area.y, area.w, area.h, c.border)

    -- the footer: the help line is the longest of its versions that fits the window
    local foot
    local footw = rect.w - GRIP - 20 * F
    if state.message and state.message_until > state.now then foot = state.message
    elseif t and t.progress and t.progress.done < t.progress.total then foot = string.format("Drawing the map: %d of %d zones", t.progress.done, t.progress.total)
    else
        for _, line in ipairs({ "Drag: pan  Wheel: zoom  Right-click: waypoint or pin  Esc: close",
                                "Drag: pan  Wheel: zoom  Right-click: pin  Esc: close",
                                "Drag: pan  Wheel: zoom  Right-click: pin", "Right-click: waypoint or pin", "Right-click: pin" }) do
            foot = line
            if tw(line) <= footw then break end
        end
    end
    local fy = rect.y + rect.h - FOOT_H
    ctext(rect.x + 8 * F, fy, FOOT_H, foot, c.muted, footw)
    state.bar.footer = { x = rect.x + 8 * F, y = fy, w = math.min(tw(foot), footw), h = FOOT_H }
    state.foot_text = foot

    -- the grip
    state.grip = { x = rect.x + rect.w - GRIP, y = rect.y + rect.h - GRIP, w = GRIP, h = GRIP }
    dl:AddTriangleFilled(V(rect.x + rect.w - 2, rect.y + rect.h - GRIP), V(rect.x + rect.w - 2, rect.y + rect.h - 2), V(rect.x + rect.w - GRIP, rect.y + rect.h - 2), c.border)

    -- search results
    local rowh = math.max(18 * F, lh + 4 * F)
    if state.focus and #state.results > 0 then
        local rows = math.min(6, #state.results)
        local more = #state.results > rows
        local ry = ty + th + 2 * F
        local boxh = rows * rowh + 4 * F + (more and rowh or 0)
        fill(sx, ry, sw, boxh, c.surface)
        frame(sx, ry, sw, boxh, c.border)
        for i = 1, rows do
            local r = state.results[i]
            local yy = ry + 2 * F + (i - 1) * rowh
            if i == 1 then fill(sx + 2, yy, sw - 4, rowh - 1, c.selected) end
            ctext(sx + 6 * F, yy, rowh, r.name .. "  (" .. r.kind .. ")", c.text, sw - 12 * F)
            state.hits[#state.hits + 1] = { label = r.name, x = sx, y = yy, w = sw, h = rowh - 1, run = function() go_to(r) end }
        end
        if more then ctext(sx + 6 * F, ry + rows * rowh + 4 * F, rowh, "+" .. (#state.results - rows) .. " more: type more letters", c.muted, sw - 12 * F) end
    elseif state.focus and (state.search or "") ~= "" then
        local ry = ty + th + 2 * F
        fill(sx, ry, sw, th, c.surface); frame(sx, ry, sw, th, c.border)
        ctext(sx + 6 * F, ry, th, "No matching place or creature", c.muted, sw - 12 * F)
    end

    -- the pin's note box
    if state.note then
        local n = state.note
        local bw = math.min(250 * F, math.max(40, area.w))
        local fh = lh + 8 * F
        local bh = 10 * F + lh + 4 * F + fh + 6 * F
        local bx, by = clamp(n.sx, area.x, math.max(area.x, area.x + area.w - bw)), clamp(n.sy, area.y, math.max(area.y, area.y + area.h - bh))
        fill(bx, by, bw, bh, c.surface); frame(bx, by, bw, bh, c.red)
        text(bx + 8 * F, by + 5 * F, "Note for this pin (Enter saves, Esc cancels)", c.muted, bw - 16 * F)
        local fy2 = by + 10 * F + lh + 4 * F
        fill(bx + 8 * F, fy2, bw - 16 * F, fh, c.black); frame(bx + 8 * F, fy2, bw - 16 * F, fh, c.border)
        ctext(bx + 12 * F, fy2, fh, n.text .. "_", c.text, bw - 24 * F)
        state.hits[#state.hits + 1] = { x = bx, y = by, w = bw, h = bh, run = function() end }
    end

    -- the right-click menu
    local mn = state.menu
    if mn and not state.note then
        local rows = {
            { "Set waypoint here", function()
                local req, err = eqoa.map.set_waypoint({ world_id = mn.world, x = mn.wx, z = mn.wz, label = "World Map", keep = true })
                if req then expect(req, "set") elseif err then say("Waypoint: " .. tostring(err.message or err.code)) end
            end },
            { "Drop a pin here...", function()
                local sx2, sy2 = to_screen(area, mn.wx, mn.wz)
                state.focus = nil
                state.note = { x = mn.wx, z = mn.wz, text = "", sx = sx2, sy = sy2, max = NOTE_MAX }
            end },
        }
        if map.waypoint then rows[#rows + 1] = { "Clear waypoint", function() state.remove_waypoint(map.waypoint) end } end
        local mrow = math.max(20 * F, lh + 6 * F)
        local mw = 170 * F
        for _, r in ipairs(rows) do mw = math.max(mw, tw(r[1]) + 16 * F) end
        mw = math.min(mw, math.max(40, area.w))
        local mx2, my2 = clamp(mn.x, area.x, math.max(area.x, area.x + area.w - mw)), clamp(mn.y, area.y, math.max(area.y, area.y + area.h - #rows * mrow - 4 * F))
        fill(mx2, my2, mw, #rows * mrow + 4 * F, c.surface); frame(mx2, my2, mw, #rows * mrow + 4 * F, c.border)
        for i, r in ipairs(rows) do
            local yy = my2 + 2 * F + (i - 1) * mrow
            ctext(mx2 + 8 * F, yy, mrow, r[1], c.text, mw - 16 * F)
            local run = r[2]
            state.hits[#state.hits + 1] = { label = r[1], x = mx2, y = yy, w = mw, h = mrow, run = function() state.menu = nil; run() end }
        end
    end
    -- hover card
    state.card_box = nil
    if pointer and pointer.inside_window and inside(pointer.x, pointer.y, area) and not state.menu and not state.note and not state.drag
        and not (state.focus and #state.results > 0) then
        local hit = under(area, pointer.x, pointer.y, world, map)
        state.card = hit
        if hit then
            local lines = {}
            for _, l in ipairs(hit.lines) do if l[1] and l[1] ~= "" then lines[#lines + 1] = l end end
            local pad, lgap = 8 * F, 2 * F
            local w, h = 0, 6 * F
            for _, l in ipairs(lines) do local lw2, lh2 = ui.CalcTextSize(l[1]); w = math.max(w, lw2); h = h + lh2 + lgap end
            w = math.min(w + 2 * pad, 280 * F, math.max(40, area.w - 4))
            h = math.min(h, math.max(lh, area.h - 4))
            local cx = clamp(hit.sx + 14 * F, area.x + 2, math.max(area.x + 2, area.x + area.w - w - 2))
            local cy = clamp(hit.sy - h / 2, area.y + 2, math.max(area.y + 2, area.y + area.h - h - 2))
            fill(cx, cy, w, h, c.surface); frame(cx, cy, w, h, c.border)
            state.card_box = { x = cx, y = cy, w = w, h = h }
            local yy = cy + 4 * F
            for _, l in ipairs(lines) do
                text(cx + pad, yy, l[1], l[2] == "title" and c.title or l[2] == "muted" and c.muted or c.card, w - 2 * pad)
                local _, lh2 = ui.CalcTextSize(l[1])
                yy = yy + lh2 + lgap
            end
        end
    else
        state.card = nil
    end

    ui.SetWindowFontScale(1)
    if cur and cur.font then ui.PopFont() end
    ui.End()
end
