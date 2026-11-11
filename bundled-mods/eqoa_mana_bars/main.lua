-- EQOA Mana Bars Mod
-- Draws authentic power bars under group member health bars and the target
-- health bar.

local hud_gauge = eqoa.hud.gauge

eqoa.options.toggle({ id = "mana_bars", label = "Mana Bars", default = true })

eqoa.events.on("render", function()
    if not eqoa.options.get("mana_bars") then return end

    local layout = eqoa.hud.layout()

    -- Group member mana bars (the player's own member is not in the list;
    -- the raid panel has its own power bars and never gets these).
    if eqoa.hud.visible("group") then
        local grp = eqoa.group.get()
        if grp and grp.members then
            local target = eqoa.target.get()
            local target_id = target and target.id
            for k, m in ipairs(grp.members) do
                if m.power then
                    local rect = layout.group_members[k].power
                    local lit = (target_id and m.id == target_id)
                    hud_gauge({ x = rect.frame_x, y = rect.frame_y, art = 9, fill = m.power, lit = lit })
                end
            end
        end
    end

    -- Target mana bar, under the target's health (vertical = bottom, as the
    -- native target plate sits at the bottom of the frame).
    if eqoa.hud.visible("target") then
        local target = eqoa.target.get()
        if target and target.id and target.power then
            hud_gauge({ x = layout.target.power.frame_x, y = layout.target.power.frame_y, art = 9, fill = target.power, lit = true, vertical = "bottom" })
        end
    end
end)
