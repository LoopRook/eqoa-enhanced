-- EQOA Mana Bars Mod
-- Draws an authentic power bar under the target's health bar. Group members'
-- and the pet's power bars are upstream's own (Extended > UI > HUD: Power
-- Bars, 2026-11-15), and the power this reads is what that option asks the
-- server for: with Power Bars off there is none to draw.

local hud_gauge = eqoa.hud.gauge

eqoa.options.toggle({ id = "mana_bars", label = "Mana Bars", default = true })

-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end

eqoa.events.on("render", function()
    if map_up() then return end
    if not eqoa.options.get("mana_bars") then return end

    local layout = eqoa.hud.layout()

    -- Target mana bar, under the target's health (vertical = bottom, as the
    -- native target plate sits at the bottom of the frame).
    if eqoa.hud.visible("target") then
        local target = eqoa.target.get()
        if target and target.id and target.power then
            -- The bar sits on the game's own power bar: moved (Move HUD) and grown with HUD Scale or the PC
            -- interface's piece size (the layout rectangle is 136 wide at 100%). Unscaled, the draw is unchanged.
            local pw = layout.target.power
            local k = (pw.frame_width and pw.frame_width > 0) and pw.frame_width / 136 or 1
            hud_gauge({ x = math.floor(pw.frame_x + 0.5), y = math.floor(pw.frame_y + 0.5), art = 9, fill = target.power, lit = true, vertical = "bottom", scale = (k ~= 1) and k or nil })
        end
    end
end)
