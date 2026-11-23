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
    local k, ky = lay.width / FRAME, lay.height / FRAME -- window pixels per frame pixel, across and down
    local inner = SIZE * k
    local w, h = lay.width * SIZE / FRAME, lay.height * SIZE / FRAME
    local x, y = lay.x + INSET * k, lay.y + INSET * lay.height / FRAME
    local r = math.min(w, h) / 2
    return { k = k, ky = ky, cx = x + w / 2, cy = y + h / 2, r = r, inner = inner }
end

local function claim(g)
    if not (g.r > 1 and g.r < 1e5) then return end -- layout not ready: no empty rectangle
    -- the pointer, the wheel and clicks over the circle only: strips that follow its edge, each as
    -- wide as the circle is at the strip's end nearer the rim (never past the circle)
    for i = 0, STRIPS - 1 do
        local y0 = g.cy - g.r + 2 * g.r * i / STRIPS
        local y1 = y0 + 2 * g.r / STRIPS
        local ym = math.max(math.abs(y0 - g.cy), math.abs(y1 - g.cy))
        local half = math.sqrt(math.max(0, g.r * g.r - ym * ym))
        -- the strips at the very top and bottom are zero-wide at their rim end: claim nothing there
        if half > 0.5 and y1 > y0 then
            eqoa.input.claim_pointer({ x = g.cx - half, y = y0, width = 2 * half, height = y1 - y0 })
        end
    end
end

local function inside_circle(g, x, y) return (x - g.cx) ^ 2 + (y - g.cy) ^ 2 <= g.r * g.r end

local function refresh_markers(world)
    if markers_world == world and not markers_stale then return end
    markers_world, markers_stale = world, false
    markers = eqoa.map.markers(world) or {}
end

-- Pick from this frame's native dot positions and projection in one pass. Only the
-- selected creature needs a full entity record; no per-creature Lua tables or calls.
local function dot_under(g, mx, my)
    local player = eqoa.player.get()
    local group = eqoa.group.get()
    local skip = {}
    if player and player.id then skip[#skip + 1] = player.id end
    if group and group.members then for _, m in ipairs(group.members) do skip[#skip + 1] = m.id end end
    local best = eqoa.entities.pick_dot({ x = mx - view_x, y = my - view_y,
        scale_x = g.k, scale_y = g.ky, center_x = g.cx - view_x, center_y = g.cy - view_y,
        radius = g.r, reach = 4 * g.k, exclude = skip })
    if not best then return nil end
    local e = eqoa.entities.find(best.id)
    if not e then return nil end
    local kind = e.kind == "player" and "Player" or e.kind == "pet" and "Pet" or "NPC"
    local level = (e.level and e.level > 0) and ("Level " .. e.level) or "Level unknown"
    local away = player and player.position and math.sqrt((best.world_x - player.position.x) ^ 2 + (best.world_z - player.position.z) ^ 2) or 0
    return { x = view_x + best.x, y = view_y + best.y, lines = { { e.name, "title" }, { level .. " / " .. kind, "text" },
        { string.format("%.0f away / observed nearby", away), "text" } } }
end

-- The marks the stock minimap draws, where it draws them (its own place_marks): camps and coaches
-- with Map Markers on, quests to take where they lie, quest steps where they lie or on the
-- picture's rim; none of the towns, community notes or waypoints. The last placed is the topmost.
local function placed_marks(g, world, map)
    refresh_markers(world)
    local k = g.k
    local ky = g.ky
    local x0, y0 = view_x + INSET * k, view_y + INSET * ky
    local x1, y1 = x0 + SIZE * k, y0 + SIZE * ky
    local r, half = 6 * ky, 8 * ky
    local ex0, ey0, ex1, ey1 = x0 + half * 0.6, y0 + half, x1 - half * 0.6, y1 - half
    local mx, my = (x0 + x1) / 2, (y0 + y1) / 2
    local function lies(q) return q.x >= x0 - r and q.x <= x1 + r and q.y >= y0 - r and q.y <= y1 + r end
    local groups = { {}, {}, {} } -- camps and coaches, quests to take, steps
    for _, m in ipairs(markers) do
        do -- eqoa.map.markers(world) already holds this world's marks only
            local px, py = view.project(m.x, m.z)
            if px then
                local q = { x = view_x + px * k, y = view_y + py * ky, m = m }
                if m.kind == "camp" or m.kind == "coach" then
                    if map.markers_on and lies(q) then table.insert(groups[1], q) end
                elseif m.kind == "quest" and not m.step then
                    if lies(q) then table.insert(groups[2], q) end
                elseif m.kind == "quest" then
                    local dx, dy = q.x - mx, q.y - my
                    local f = math.max(math.abs(dx) / math.max((ex1 - ex0) / 2, 1e-6), math.abs(dy) / math.max((ey1 - ey0) / 2, 1e-6))
                    if f > 1 then q.x, q.y = mx + dx / f, my + dy / f end
                    table.insert(groups[3], q)
                end
            end
        end
    end
    local out = {}
    for _, grp in ipairs(groups) do for _, q in ipairs(grp) do out[#out + 1] = q end end
    return out
end

local function mark_under(g, mx, my, world, map)
    local placed = placed_marks(g, world, map)
    local reach = 6 * g.ky
    for i = #placed, 1, -1 do
        local q = placed[i]
        -- only what the circle leaves visible
        if inside_circle(g, q.x, q.y) and (q.x - mx) ^ 2 + (q.y - my) ^ 2 <= reach * reach then
            local title = q.m.label or q.m.name
            if title and title ~= "" then return { x = q.x, y = q.y, lines = { { title, "title" } } } end
        end
    end
end

local function waypoint_arrows(g, ui, dl, v, world)
    refresh_markers(world)
    round_minimap.arrows = {}
    for _, m in ipairs(markers) do
        if (m.kind == "waypoint" or m.kind == "group_waypoint") then
            local px, py, inside = view.project(m.x, m.z)
            if px and not inside then
                -- the square picture puts its own rim mark at the nearest edge; draw ours only where
                -- the circle would crop it
                local cx, cy = view_x + px * g.k - g.cx, view_y + py * g.ky - g.cy
                local n = math.sqrt(cx * cx + cy * cy)
                local square = math.max(math.abs(cx), math.abs(cy))
                local cropped = n * (g.r * 0.92) / math.max(square, 1e-6) > g.r * 0.97
                if n > 0 and cropped then
                    local rr = g.r + 6 * g.k
                    local ax, ay = g.cx + cx / n * rr, g.cy + cy / n * rr
                    local s = 4.5 * g.k
                    round_minimap.arrows[#round_minimap.arrows + 1] = { x = ax, y = ay, kind = m.kind }
                    -- the waypoint's magenta, the group waypoint's orange (as the stock minimap's)
                    local color = m.kind == "waypoint" and ui.GetColorU32(1, 0.3, 0.85, 1) or ui.GetColorU32(1, 0.5, 0.05, 1)
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

-- `text` cut at its spaces into lines no wider than `most`; a word wider than that stays whole
local function wrap(ui, text, most)
    local lines, cur = {}, ""
    for word in tostring(text):gmatch("%S+") do
        local try = cur == "" and word or (cur .. " " .. word)
        if cur ~= "" and ui.CalcTextSize(try) > most then lines[#lines + 1] = cur; cur = word else cur = try end
    end
    if cur ~= "" then lines[#lines + 1] = cur end
    return lines
end

-- the last UTF-8 character, to cut
local utf8_last = "^(.*)[%z" .. string.char(1) .. "-" .. string.char(127, 194) .. "-" .. string.char(244) .. "][" .. string.char(128) .. "-" .. string.char(191) .. "]*$"

local function draw_card(ui, dl, v, g, c, win_w, win_h)
    if font then ui.PushFont(font) end
    local most = 260 * g.k
    local lines = {}
    for _, l in ipairs(c.lines) do
        for _, piece in ipairs(wrap(ui, l[1], most)) do
            -- a single word wider than the card is cut, with an ellipsis
            if ui.CalcTextSize(piece) > most then
                while #piece > 0 and ui.CalcTextSize(piece .. "...") > most do
                    piece = piece:match(utf8_last) or ""
                end
                piece = piece .. "..."
            end
            lines[#lines + 1] = { piece, l[2] }
        end
    end
    -- never taller than the window: the lines that do not fit are dropped
    local _, line_h = ui.CalcTextSize("Ag")
    local room = math.max(1, math.floor((win_h - 22) / (line_h + 2)))
    while #lines > room do table.remove(lines) end
    round_minimap.card_text = {} -- what is drawn, line by line; read by its tests
    for i, l in ipairs(lines) do round_minimap.card_text[i] = l[1] end
    local w, h = 0, 0
    for _, l in ipairs(lines) do local tw, th = ui.CalcTextSize(l[1]); w = math.max(w, tw); h = h + th + 2 end
    w, h = w + 16, h + 10
    -- beside the circle (left, else right), inside the window wherever Move HUD put it
    local x = g.cx - g.r - w - 14
    if x < 6 then x = g.cx + g.r + 14 end
    x = math.max(6, math.min(x, win_w - w - 6))
    local y = math.max(6, math.min(c.y - h / 2, win_h - h - 6))
    round_minimap.card_rect = { x = x, y = y, w = w, h = h }
    local surface, border = ui.GetColorU32(0.067, 0.067, 0.067, 1), ui.GetColorU32(0.2, 0.2, 0.2, 1)
    dl:AddRectFilled(v(x, y), v(x + w, y + h), surface)
    dl:AddRect(v(x, y), v(x + w, y + h), border)
    local yy = y + 5
    for _, l in ipairs(lines) do
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
    round_minimap.shown, round_minimap.card, round_minimap.card_rect, round_minimap.arrows = shown, nil, nil, {}
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
        card = dot_under(g, pointer.x, pointer.y) or (world and map and mark_under(g, pointer.x, pointer.y, world, map))
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
    if card then
        draw_card(ui, dl, v, g, card, w, h)
    end
    ui.End()

end

eqoa.events.on("pointer.wheel", function(e)
    local d = e.data
    if not (shown and d.inside_window and d.steps) then return end
    local g = geometry(eqoa.hud.layout().minimap)
    if inside_circle(g, d.x, d.y) then eqoa.input.minimap_wheel(d.steps) end
end)
eqoa.events.on("render", draw)
