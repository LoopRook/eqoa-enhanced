-- EQOA Stats Box Mod
-- Authentic 2-column x 8-row stat display matching the Summary order, with
-- click-to-open Summary and the rebindable B key (as the Controls page binds).

local gold = { 0.95, 0.82, 0.5, 1.0 }
local off_white = { 0.96, 0.95, 0.9, 1.0 }
local raised_blue = { 0.45, 0.78, 1.0, 1.0 }
local lowered_red = { 1.0, 0.45, 0.4, 1.0 }

eqoa.options.toggle({ id = "stats_box", label = "Stats Box", default = false })

eqoa.input.bind({ id = "toggle", label = "Toggle Stats Box", default_key = "b" }, function()
    eqoa.actions.toggle_stats_box()
end)

eqoa.events.on("render", function()
    if not eqoa.options.get("stats_box") then return end
    if not eqoa.hud.visible("stats_box") then return end

    local s = eqoa.player.stats()
    if not s or not s.rows or #s.rows == 0 then return end

    -- Panel at [200, 54, 348, 186] (width 148, height 132), drawn in the
    -- HUD's frame coords; the layer lays out the rows inside its borders.
    eqoa.hud.panel({ x = 200, y = 54, w = 148, h = 132 })

    for i = 1, math.min(16, #s.rows) do
        local r = s.rows[i]
        local col = (i > 8) and 2 or 1
        local row = ((i - 1) % 8) + 1
        local color = off_white
        if r.shift == "raised" then color = raised_blue
        elseif r.shift == "lowered" then color = lowered_red end

        local label = eqoa.text.localized(r.word) or r.short or r.name
        eqoa.hud.text({ col = col, row = row, text = label, color = gold, shadow = true })
        eqoa.hud.text({ col = col, row = row, text = tostring(r.current), color = color, align = "right", shadow = true })
    end

    -- Clickable area to open the Summary window, only while the HUD is in
    -- plain play (no map, menu, search or loading screen over it).
    if eqoa.imgui then
        local frame = eqoa.hud.frame()
        local sx = frame.offset_x + 200 * frame.scale_x
        local sy = frame.offset_y + 54 * frame.scale_y
        local sw = 148 * frame.scale_x
        local sh = 132 * frame.scale_y
        eqoa.imgui.SetNextWindowPos(sx, sy)
        eqoa.imgui.SetNextWindowSize(sw, sh)
        if eqoa.imgui.Begin("eqoa_stats_box_click", nil,
            ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove
                + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings) then
            if eqoa.imgui.InvisibleButton("stats_box_button", sw, sh) then
                eqoa.windows.open("summary")
            end
            eqoa.imgui.End()
        end
    end
end)
