-- EQOA Target of Target Mod
-- Displays the Target of Target bar above the Target panel with health and
-- mana gauges, and Assist Target on click.

local black = { 0.0, 0.0, 0.0, 1.0 }

eqoa.options.toggle({ id = "target_of_target", label = "Target of Target", default = false })

eqoa.events.on("render", function()
    if not eqoa.options.get("target_of_target") then return end
    if not eqoa.hud.visible("target") then return end

    local target = eqoa.target.get()
    if not target or not target.id or not target.target_id then return end
    if target.target_id == 0 or target.target_id == target.id then return end

    local tot = eqoa.entities.find(target.target_id)
    if not tot or not tot.name or tot.name == "" or tot.name:match("^%s*$") then return end

    -- 3-piece bar at [20, 300], width 174
    eqoa.hud.image({ ui_id = 25, x = 20, y = 300, w = 174, vertical = "bottom" })

    -- Centered black name text
    eqoa.hud.text({ x = 20 + 87, y = 300 + 12, text = tot.name, color = black, clip_width = 160, vertical = "bottom" })

    -- HP gauge (art 7)
    if tot.hp_fraction then
        eqoa.hud.gauge({ x = 30, y = 325, art = 7, fill = tot.hp_fraction, lit = true, vertical = "bottom" })
    end

    -- Power gauge (art 9), gated by the Mana Bars option
    if tot.power and eqoa.options.get("mana_bars") then
        eqoa.hud.gauge({ x = 30, y = 335, art = 9, fill = tot.power, lit = true, vertical = "bottom" })
    end

    -- Clickable name bar to Assist, only while the HUD is in plain play.
    if eqoa.imgui and eqoa.hud.visible("stats_box") then
        local frame = eqoa.hud.frame()
        local sx = frame.offset_x + (20 - frame.side) * frame.scale_x
        local sy = frame.offset_y + (300 + frame.top) * frame.scale_y
        local sw = 174 * frame.scale_x
        local sh = 26 * frame.scale_y
        eqoa.imgui.SetNextWindowPos(sx, sy)
        eqoa.imgui.SetNextWindowSize(sw, sh)
        if eqoa.imgui.Begin("eqoa_target_of_target_click", nil,
            ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove
                + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings) then
            eqoa.imgui.SetCursorScreenPos(sx, sy)
            if eqoa.imgui.InvisibleButton("tot_button", sw, sh) then
                eqoa.actions.assist()
            end
        end
        eqoa.imgui.End()
    end
end)
