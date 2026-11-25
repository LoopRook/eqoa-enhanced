-- EQOA HUD Numbers Mod
-- Authentic numeric readouts for player HP, Power, XP, and Target HP percentage.

local hud_text = eqoa.hud.text

local white = { 1.0, 1.0, 1.0, 1.0 }
local blue = { 0.4, 0.7, 1.0, 1.0 }
local gold = { 1.0, 0.85, 0.35, 1.0 }
local green = { 0.4, 1.0, 0.4, 1.0 }
local yellow = { 1.0, 0.9, 0.3, 1.0 }
local red = { 1.0, 0.3, 0.3, 1.0 }

eqoa.options.toggle({ id = "hud_numbers", label = "HUD Numbers", default = true })
eqoa.options.toggle({ id = "target_hp_percent", label = "Target HP %", default = true })
eqoa.options.toggle({ id = "xp_details", label = "XP Details", default = true })

-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end

eqoa.events.on("render", function()
    if map_up() then return end
    local hud_on = eqoa.options.get("hud_numbers")
    local xp_on = eqoa.options.get("xp_details")
    local target_on = eqoa.options.get("target_hp_percent")

    local layout = eqoa.hud.layout()
    -- The panels as they are drawn now: moved (Move HUD) and, with HUD Scale or the PC interface's piece size
    -- off 100%, grown about their corner. The numbers sit at the same place on the panel (in the panel's own
    -- units, 190 and 174 wide at 100%) and grow with it.
    local pp, tp = layout.player.panel, layout.target.panel
    local pk = (pp.frame_width and pp.frame_width > 0) and pp.frame_width / 190 or 1
    local tk = (tp.frame_width and tp.frame_width > 0) and tp.frame_width / 174 or 1
    local player = eqoa.player.get()
    if player then
        -- Player HP %
        if hud_on and eqoa.hud.visible("health") and player.hp_fraction ~= nil then
            local pct = math.floor(player.hp_fraction * 100 + 0.5)
            hud_text({
                x = pp.frame_x + 89 * pk,
                y = pp.frame_y + 20 * pk,
                text = tostring(pct) .. "%",
                color = white,
                scale = 0.6 * pk,
                align = "center",
                outline = true,
            })
        end

        -- Player Power
        if hud_on and eqoa.hud.visible("power") and player.power and player.power.maximum and player.power.maximum > 0 then
            local txt = tostring(player.power.current) .. "/" .. tostring(player.power.maximum)
            hud_text({
                x = pp.frame_x + 89 * pk,
                y = pp.frame_y + 30 * pk,
                text = txt,
                color = blue,
                scale = 0.6 * pk,
                align = "center",
                outline = true,
            })
        end

        -- XP details
        if xp_on and eqoa.hud.visible("exp") and player.experience and player.level then
            local need = eqoa.player.experience_for_level(player.level + 1)
            if need and need > 0 then
                local cur = player.experience
                local pct = math.max(0, math.min(100, (cur / need) * 100))
                local txt = string.format("%d/%d (%.1f%%)", cur, need, pct)
                hud_text({
                    x = pp.frame_x + 89 * pk,
                    y = pp.frame_y + 40 * pk,
                    text = txt,
                    color = gold,
                    scale = 0.6 * pk,
                    align = "center",
                    outline = true,
                })
            end
        end
    end

    -- Target HP % (bottom-anchored with the target plate)
    if target_on and eqoa.hud.visible("target") then
        local target = eqoa.target.get()
        if target and target.id and target.hp_fraction then
            local pct = math.floor(target.hp_fraction * 100 + 0.5)
            local col = red
            if pct > 50 then col = green
            elseif pct > 20 then col = yellow end

            hud_text({
                x = tp.frame_x + 40 * tk,
                y = tp.frame_y + 49 * tk,
                text = tostring(pct) .. "%",
                color = col,
                scale = 0.65 * tk,
                align = "center",
                outline = true,
                vertical = "bottom",
            })
        end
    end
end)
