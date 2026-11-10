-- EQOA Target of Target Mod
-- Displays the Target of Target bar above the Target panel with health and mana gauges, and assist on click.

local black = { 0.0, 0.0, 0.0, 1.0 }

eqoa.options.toggle({ id = "target_of_target", label = "Target of Target", default = true })

eqoa.events.on("render", function()
    if not eqoa.options.get("target_of_target") then return end
    if not eqoa.hud.visible("target") then return end

    local target = eqoa.target.get()
    if not target or not target.id or not target.target_id or target.target_id == 0 then return end

    local tot
    local entities = eqoa.entities.list()
    if entities then
        for _, e in ipairs(entities) do
            if e.id == target.target_id then
                tot = e
                break
            end
        end
    end
    if not tot or not tot.name then return end

    -- Draw 3-piece bar at [20, 300], width 174
    eqoa.hud.image({ ui_id = 25, x = 20, y = 300, w = 174 })

    -- Centered black name text
    eqoa.hud.text({ x = 20 + 87, y = 300 + 4, text = tot.name, color = black, scale = 0.55, align = "center" })

    -- HP gauge (art 7)
    if tot.hp_fraction then
        eqoa.hud.gauge({ x = 30, y = 325, art = 7, fill = tot.hp_fraction, lit = true })
    end

    -- Power gauge (art 9)
    if tot.power then
        eqoa.hud.gauge({ x = 30, y = 335, art = 9, fill = tot.power, lit = true })
    end

    -- Clickable area to assist
    local frame = eqoa.hud.frame()
    if frame and eqoa.imgui then
        local sx = frame.offset_x + 20 * frame.scale_x
        local sy = frame.offset_y + 300 * frame.scale_y
        local sw = 174 * frame.scale_x
        local sh = 46 * frame.scale_y
        eqoa.imgui.SetCursorScreenPos(sx, sy)
        if eqoa.imgui.InvisibleButton("tot_button", sw, sh) then
            eqoa.actions.assist()
        end
    end
end)
