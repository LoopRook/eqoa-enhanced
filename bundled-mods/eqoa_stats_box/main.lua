-- EQOA Stats Box Mod
-- Authentic 2-column x 8-row stat display matching Summary order, with click-to-open Summary window.

local is_open = true
local gold = { 0.85, 0.72, 0.38, 1.0 }
local off_white = { 0.96, 0.95, 0.9, 1.0 }
local raised_blue = { 0.45, 0.78, 1.0, 1.0 }
local lowered_red = { 1.0, 0.45, 0.4, 1.0 }

eqoa.options.toggle({ id = "stats_box", label = "Stats Box", default = true })

eqoa.binds.register("toggle", "b", "Toggle Stats Box")

eqoa.events.on("input.key", function(e)
    if e.down and not e.repeat_ then
        local bound = (eqoa.binds.get and eqoa.binds.get("toggle")) or "b"
        if e.key == bound then
            is_open = not is_open
        end
    end
end)

eqoa.events.on("render", function()
    if not is_open then return end
    if not eqoa.options.get("stats_box") then return end
    if not eqoa.hud.visible("hud") then return end

    local s = eqoa.player.stats()
    if not s or not s.rows or #s.rows == 0 then return end

    -- Panel at [200, 54, 348, 186] (width 148, height 132)
    eqoa.hud.panel({ x = 200, y = 54, w = 148, h = 132 })

    local top = 58
    local step = 15.5
    local scale = 0.55

    -- Column 1: rows 1..8
    for i = 1, math.min(8, #s.rows) do
        local r = s.rows[i]
        local y = top + (i - 1) * step
        local color = off_white
        if r.shift == "raised" then color = raised_blue
        elseif r.shift == "lowered" then color = lowered_red end

        local label = r.short or r.name
        eqoa.hud.text({ x = 206, y = y, text = label, color = gold, scale = scale, shadow = true })
        eqoa.hud.text({ x = 270, y = y, text = tostring(r.current), color = color, scale = scale, align = "right", shadow = true })
    end

    -- Column 2: rows 9..16
    for i = 9, math.min(16, #s.rows) do
        local r = s.rows[i]
        local y = top + (i - 9) * step
        local color = off_white
        if r.shift == "raised" then color = raised_blue
        elseif r.shift == "lowered" then color = lowered_red end

        local label = r.short or r.name
        eqoa.hud.text({ x = 278, y = y, text = label, color = gold, scale = scale, shadow = true })
        eqoa.hud.text({ x = 342, y = y, text = tostring(r.current), color = color, scale = scale, align = "right", shadow = true })
    end

    -- Clickable area to open Summary window
    local frame = eqoa.hud.frame()
    if frame and eqoa.imgui then
        local sx = frame.offset_x + 200 * frame.scale_x
        local sy = frame.offset_y + 54 * frame.scale_y
        local sw = 148 * frame.scale_x
        local sh = 132 * frame.scale_y
        eqoa.imgui.SetCursorScreenPos(sx, sy)
        if eqoa.imgui.InvisibleButton("stats_box_button", sw, sh) then
            eqoa.windows.open("summary")
        end
    end
end)
