-- EQOA Mana Bars Mod
-- Draws authentic power bars under group member health bars and target health bar.

eqoa.options.toggle({ id = "mana_bars", label = "Mana Bars", default = true })

eqoa.events.on("render", function()
    if not eqoa.options.get("mana_bars") then return end

    -- Group member mana bars
    if eqoa.hud.visible("group") then
        local grp = eqoa.group.get()
        if grp and grp.members then
            local target = eqoa.target.get()
            local target_id = target and target.id
            for k, m in ipairs(grp.members) do
                if m.power then
                    local y = 93 + 48 * (k - 1) + 35
                    local lit = (target_id and m.id == target_id)
                    eqoa.hud.gauge({ x = 30, y = y, art = 9, fill = m.power, lit = lit })
                end
            end
        end
    end

    -- Target mana bar
    if eqoa.hud.visible("target") then
        local target = eqoa.target.get()
        if target and target.id and target.power then
            eqoa.hud.gauge({ x = 30, y = 408, art = 9, fill = target.power, lit = true })
        end
    end
end)
