-- EQOA HUD Numbers Mod
-- Authentic numeric readouts for player HP, Power, XP, and Target HP percentage.

local white = { 1.0, 1.0, 1.0, 1.0 }
local blue = { 0.4, 0.7, 1.0, 1.0 }
local gold = { 1.0, 0.85, 0.35, 1.0 }
local green = { 0.4, 1.0, 0.4, 1.0 }
local yellow = { 1.0, 0.9, 0.3, 1.0 }
local red = { 1.0, 0.3, 0.3, 1.0 }

eqoa.options.toggle({ id = "hud_numbers", label = "HUD Numbers", default = true })
eqoa.options.toggle({ id = "target_hp_percent", label = "Target HP %", default = true })
eqoa.options.toggle({ id = "xp_details", label = "XP Details", default = true })

eqoa.events.on("render", function()
    local hud_on = eqoa.options.get("hud_numbers")
    local xp_on = eqoa.options.get("xp_details")
    local target_on = eqoa.options.get("target_hp_percent")

    local player = eqoa.player.get()
    if player then
        -- Player HP %
        if hud_on and eqoa.hud.visible("health") and player.hp and player.hp.maximum and player.hp.maximum > 0 then
            local pct = math.floor((player.hp.current / player.hp.maximum) * 100 + 0.5)
            eqoa.hud.text({
                x = 99,
                y = 35,
                text = tostring(pct) .. "%",
                color = white,
                scale = 0.6,
                align = "center",
                outline = true,
            })
        end

        -- Player Power
        if hud_on and eqoa.hud.visible("power") and player.power and player.power.maximum and player.power.maximum > 0 then
            local txt = tostring(player.power.current) .. "/" .. tostring(player.power.maximum)
            eqoa.hud.text({
                x = 99,
                y = 45,
                text = txt,
                color = blue,
                scale = 0.6,
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
                eqoa.hud.text({
                    x = 99,
                    y = 55,
                    text = txt,
                    color = gold,
                    scale = 0.6,
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

            eqoa.hud.text({
                x = 60,
                y = 397,
                text = tostring(pct) .. "%",
                color = col,
                scale = 0.65,
                align = "center",
                outline = true,
                vertical = "bottom",
            })
        end
    end
end)
