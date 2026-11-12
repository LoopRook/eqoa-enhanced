-- Round minimap: upstream's own minimap picture cut to a circle, with a ring, a hover card and
-- rim arrows for waypoints the square picture would crop at its corners.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local FRAME, INSET, SIZE = 112, 6, 100 -- upstream's minimap frame, border and picture, in HUD frame pixels
local STRIPS = 14
local hide_token, view, shown = nil, nil, false
local view_x, view_y = 0, 0 -- the stock rectangle's window position this frame
local markers, markers_world, markers_stale = {}, nil, true
local card
round_minimap = { arrows = {} } -- what the mod last worked out; read by its tests
local font_info = eqoa.fonts.load("package_resources/SpaceMono-Regular.ttf", 13)
local font = font_info and font_info.font

local function reset() hide_token, view, shown, markers_world, markers_stale = nil, nil, false, nil, true end
for _, name in ipairs({ "state.reset", "map.changed" }) do
    eqoa.events.on(name, function(e)
        markers_stale = true
        if name == "state.reset" then reset() end
    end)
end
eqoa.events.on("disabling", function()
    if view then view:close() end
    reset()
end)

-- The stock minimap is hidden and one held view carries its picture; both need a world.
local function ensure()
    if not hide_token then hide_token = eqoa.hud.hide("minimap") end
    if hide_token and not view then view = eqoa.map.view({ size = FRAME }); round_minimap.view = view end
    return hide_token and view
end

-- The circle in window pixels: centre, radius, and the picture's square.
local function geometry(lay)
    local k = lay.width / FRAME
    local inner = SIZE * k
    local w, h = lay.width * SIZE / FRAME, lay.height * SIZE / FRAME
    local x, y = lay.x + INSET * k, lay.y + INSET * lay.height / FRAME
    local r = math.min(w, h) / 2
    return { k = k, cx = x + w / 2, cy = y + h / 2, r = r, inner = inner }
end

local function claim(g)
    -- the pointer, the wheel and clicks over the circle only: strips that follow its edge
    for i = 0, STRIPS - 1 do
        local y0 = g.cy - g.r + 2 * g.r * i / STRIPS
        local y1 = y0 + 2 * g.r / STRIPS
        local ym = math.min(math.abs(y0 - g.cy), math.abs(y1 - g.cy))
        if y0 < g.cy and y1 > g.cy then ym = 0 end
        local half = math.sqrt(math.max(0, g.r * g.r - ym * ym))
        eqoa.input.claim_pointer({ x = g.cx - half, y = y0, width = 2 * half, height = y1 - y0 })
    end
end

local function inside_circle(g, x, y) return (x - g.cx) ^ 2 + (y - g.cy) ^ 2 <= g.r * g.r end

local function refresh_markers(world)
    if markers_world == world and not markers_stale then return end
    markers_world, markers_stale = world, false
    markers = eqoa.map.markers(world) or {}
end

-- Dots as the stock minimap draws them: the creatures in view but the player and the group.
local function dot_under(g, p, mx, my)
    local player = eqoa.player.get()
    local group = eqoa.group.get()
    local skip = {}
    if player and player.id then skip[player.id] = true end
    if group and group.members then for _, m in ipairs(group.members) do skip[m.id] = true end end
    local reach, best, bd = 4 * g.k, nil, nil
    for _, e in ipairs(eqoa.entities.list()) do
        local pos = e.position
        local name = e.name and e.name:match("^%s*(.-)%s*$") or ""
        if pos and name ~= "" and e.is_creature and not e.corpse and not skip[e.id] and e.hp_fraction then
            local px, py = view.project(pos.x, pos.z)
            if px then
                local wx, wy = view_x + px * g.k, view_y + py * g.k
                local d = math.sqrt((wx - mx) ^ 2 + (wy - my) ^ 2)
                if inside_circle(g, wx, wy) and d <= reach and (not bd or d < bd or (d == bd and e.id < best.e.id)) then
                    best, bd = { e = e, x = wx, y = wy }, d
                end
            end
        end
    end
    if not best then return nil end
    local e = best.e
    local kind = e.kind == "player" and "Player" or e.kind == "pet" and "Pet" or "NPC"
    local level = (e.level and e.level > 0) and ("Level " .. e.level) or "Level unknown"
    local away = player and player.position and math.sqrt((e.position.x - player.position.x) ^ 2 + (e.position.z - player.position.z) ^ 2) or 0
    return { x = best.x, y = best.y, lines = { { e.name, "title" }, { level .. " / " .. kind, "text" },
        { string.format("%.0f away / observed nearby", away), "text" } } }
end

local function mark_under(g, mx, my, world)
    refresh_markers(world)
    local reach = 6 * g.k
    for _, m in ipairs(markers) do
        if m.world_id == nil or m.world_id == world then
            local px, py = view.project(m.x, m.z)
            if px then
                local wx, wy = view_x + px * g.k, view_y + py * g.k
                if inside_circle(g, wx, wy) and (wx - mx) ^ 2 + (wy - my) ^ 2 <= reach * reach then
                    local title = m.label or m.name
                    if title and title ~= "" then return { x = wx, y = wy, lines = { { title, "title" } } } end
                end
            end
        end
    end
end

local function waypoint_arrows(g, ui, dl, v, world)
    refresh_markers(world)
    round_minimap.arrows = {}
    for _, m in ipairs(markers) do
        if (m.kind == "waypoint" or m.kind == "group_waypoint") and (m.world_id == nil or m.world_id == world) then
            local px, py, inside = view.project(m.x, m.z)
            if px and not inside then
                -- the square picture puts its own rim mark at the nearest edge; draw ours only where
                -- the circle would crop it
                local cx, cy = view_x + px * g.k - g.cx, view_y + py * g.k - g.cy
                local n = math.sqrt(cx * cx + cy * cy)
                local square = math.max(math.abs(cx), math.abs(cy))
                local cropped = n * (g.r * 0.92) / math.max(square, 1e-6) > g.r * 0.97
                if n > 0 and cropped then
                    local rr = g.r + 6 * g.k
                    local ax, ay = g.cx + cx / n * rr, g.cy + cy / n * rr
                    local s = 4.5 * g.k
                    round_minimap.arrows[#round_minimap.arrows + 1] = { x = ax, y = ay, kind = m.kind }
                    local color = m.kind == "waypoint" and ui.GetColorU32(1, 0.3, 0.85, 1) or ui.GetColorU32(0.3, 0.75, 1, 1)
                    local dark = ui.GetColorU32(0, 0, 0, 1)
                    local function diamond(r, c)
                        dl:AddTriangleFilled(v(ax, ay - r), v(ax + r, ay), v(ax, ay + r), c)
                        dl:AddTriangleFilled(v(ax, ay - r), v(ax, ay + r), v(ax - r, ay), c)
                    end
                    diamond(s + 1, dark)
                    diamond(s, color)
                end
            end
        end
    end
end

local function draw_card(ui, dl, v, g, c)
    if font then ui.PushFont(font) end
    local w, h = 0, 0
    for _, l in ipairs(c.lines) do local tw, th = ui.CalcTextSize(l[1]); w = math.max(w, tw); h = h + th + 2 end
    w, h = w + 16, h + 10
    local x = g.cx - g.r - w - 14
    local y = math.max(6, c.y - h / 2)
    local surface, border = ui.GetColorU32(0.067, 0.067, 0.067, 1), ui.GetColorU32(0.2, 0.2, 0.2, 1)
    dl:AddRectFilled(v(x, y), v(x + w, y + h), surface)
    dl:AddRect(v(x, y), v(x + w, y + h), border)
    local yy = y + 5
    for _, l in ipairs(c.lines) do
        local col = l[2] == "title" and ui.GetColorU32(1, 0.86, 0.45, 1) or ui.GetColorU32(0.95, 0.93, 0.86, 1)
        dl:AddText(v(x + 8, yy), col, l[1])
        local _, th = ui.CalcTextSize(l[1])
        yy = yy + th + 2
    end
    if font then ui.PopFont() end
end

local function draw()
    if not ensure() then return end
    local info = view.texture
    shown = info ~= nil
    round_minimap.shown, round_minimap.card, round_minimap.arrows = shown, nil, {}
    if not info then return end
    local lay = eqoa.hud.layout().minimap
    local g = geometry(lay)
    view_x, view_y = lay.x, lay.y
    -- upstream's picture without its square frame, cut to the circle
    local uv = info.uv
    local du, dv = (uv[3] - uv[1]) * INSET / FRAME, (uv[4] - uv[2]) * INSET / FRAME
    local ok, err = eqoa.hud.image({
        texture = { texture = info.texture, width = info.width, height = info.height,
            uv = { uv[1] + du, uv[2] + dv, uv[3] - du, uv[4] - dv } },
        x = lay.frame_x + INSET, y = lay.frame_y + INSET, w = SIZE, h = SIZE,
        anchor = "right", vertical = "top", shape = "circle",
    })
    round_minimap.image_error = not ok and err and err.message or nil
    if round_minimap_debug then eqoa.hud.image({texture=info,x=0,y=0,w=320,h=224}) end
    claim(g)
    round_minimap.circle = g
    if not eqoa.imgui then return end
    local ui = eqoa.imgui
    local map = eqoa.map.get()
    local world = map and map.world_id
    local pointer = eqoa.input.pointer()
    card = nil
    if pointer and pointer.inside_window and inside_circle(g, pointer.x, pointer.y) then
        card = dot_under(g, pointer, pointer.x, pointer.y) or (world and mark_under(g, pointer.x, pointer.y, world))
    end
    local w, h = ui.GetDisplaySize()
    ui.SetNextWindowPos(0, 0); ui.SetNextWindowSize(w, h)
    local flags = ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize
        + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs
    ui.Begin("eqoa_round_minimap_layer", nil, flags)
    local dl = ui.GetWindowDrawList()
    local v = ImVec2.new
    -- the ring just outside the picture (the picture is drawn over the window layer)
    dl:AddCircle(v(g.cx, g.cy), g.r + 1.5 * g.k, ui.GetColorU32(0.2, 0.2, 0.2, 1), 96, 3 * g.k)
    if world then waypoint_arrows(g, ui, dl, v, world) end
    round_minimap.card = card
    if card then draw_card(ui, dl, v, g, card) end
    ui.End()
end

eqoa.events.on("pointer.wheel", function(e)
    local d = e.data
    if not (shown and d.inside_window and d.steps) then return end
    local g = geometry(eqoa.hud.layout().minimap)
    if inside_circle(g, d.x, d.y) then eqoa.input.minimap_wheel(d.steps) end
end)
eqoa.events.on("render", draw)
