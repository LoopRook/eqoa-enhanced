-- EQOA Mana Bars Mod
-- Draws an authentic power bar under the target's health bar. Group members'
-- and the pet's power bars are upstream's own (Extended > UI > HUD: Power
-- Bars, 2026-11-15), and the power this reads is what that option asks the
-- server for: with Power Bars off there is none to draw.

local hud_gauge = eqoa.hud.gauge

eqoa.options.toggle({ id = "mana_bars", label = "Mana Bars", default = true })

eqoa.events.on("render", function()
    if not eqoa.options.get("mana_bars") then return end

    local layout = eqoa.hud.layout()

    -- Target mana bar, under the target's health (vertical = bottom, as the
    -- native target plate sits at the bottom of the frame).
    if eqoa.hud.visible("target") then
        local target = eqoa.target.get()
        if target and target.id and target.power then
            hud_gauge({ x = layout.target.power.frame_x, y = layout.target.power.frame_y, art = 9, fill = target.power, lit = true, vertical = "bottom" })
        end
    end
end)
