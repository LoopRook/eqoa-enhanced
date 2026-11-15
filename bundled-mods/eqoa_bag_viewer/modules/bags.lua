-- Bag viewer model: what the window shows, read from eqoa.inventory / eqoa.bank and the loot events.
-- Nothing here is a copy of game state that can drift: the grid is read again on a change event or an open.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local M = {open = false, tab = 1, grid = nil, log = {}, scroll = 0, hover = nil, builds = 0, draws = 0, rect = nil}

M.COLUMNS, M.MAX_ROWS, M.LOG_MAX = 8, 8, 50
-- Item.wear_slot of a stack nothing wears (the game's -1): `wearable` is wear_slot ~= -1.
local NO_SLOT = 4294967295
M.TABS = {
    {key = "all", label = "All"},
    {key = "equipment", label = "Equipment"},
    {key = "bank", label = "Bank"},
}
local keep = {
    all = function() return true end,
    equipment = function(it) return it.wear_slot ~= NO_SLOT end,
    bank = function() return true end,
}

-- The grid for the tab on show: the items it keeps in the container's own order, with the numbers under it.
function M.rebuild()
    local key = M.TABS[M.tab].key
    M.builds = M.builds + 1
    local c
    if key == "bank" then c = eqoa.bank.get() else c = eqoa.inventory.get() end
    if not c then M.grid = false; return false end
    local keeps, items = keep[key], {}
    for _, it in ipairs(c.items) do
        if keeps(it) then
            local info = it.icon and eqoa.icons.resource(it.icon)
            items[#items + 1] = {item = it, texture = info and info.texture}
        end
    end
    local slots = math.max(c.capacity or 40, #c.items)
    local rows = math.max(5, math.min(M.MAX_ROWS, math.ceil(slots / M.COLUMNS)))
    M.grid = {items = items, used = c.used, capacity = c.capacity, money = c.money, rows = rows, bank = key == "bank"}
    M.hover = nil
    return M.grid
end

function M.dirty() M.grid = nil end

function M.select(tab)
    if tab ~= M.tab and M.TABS[tab] then M.tab = tab; M.scroll = 0; M.grid = nil end
end

-- 1234567 -> "1,234,567"
function M.tunar(n)
    local s, r = tostring(math.floor(n or 0)), nil
    repeat s, r = s:gsub("^(-?%d+)(%d%d%d)", "%1,%2") until r == 0
    return s
end

-- The card for one item: name, level, slot, stats, HP and durability, value, and the red line when the
-- character's class, race or level rule it out. Read once per item hovered, not once per frame.
function M.card(it)
    local h = M.hover
    if h and h.key == it.ref then return h.lines end
    local lines = {{it.name, "title"}}
    if (it.level_required or 0) > 0 then lines[#lines + 1] = {"Level " .. it.level_required, "text"} end
    if it.slot_name then lines[#lines + 1] = {"Slot: " .. it.slot_name, "text"} end
    for _, s in ipairs(it.stats) do
        lines[#lines + 1] = {string.format("%s %+d", s.name or ("Stat " .. s.id), s.value), "text"}
    end
    if (it.maximum_hp or 0) > 0 then lines[#lines + 1] = {"HP " .. it.current_hp .. "/" .. it.maximum_hp, "text"} end
    if (it.durability or 0) > 0 then lines[#lines + 1] = {"Durability " .. it.durability, "text"} end
    if (it.value or 0) > 0 then lines[#lines + 1] = {"Value " .. M.tunar(it.value) .. " tunar", "muted"} end
    local can = eqoa.items.can_use(it.ref)
    if can and not can.allowed then lines[#lines + 1] = {"You can't use this", "red"} end
    M.hover = {key = it.ref, lines = lines}
    return lines
end

-- A pickup is what a corpse loses: loot.changed with the native cause for one item (0x0019) or all (0x001A).
-- Stacks are told apart by their wire key, which the corpse keeps until it is closed.
function M.log_loot(data)
    if (data.cause ~= "loot_one" and data.cause ~= "loot_all") or not data.before or not data.after then return end
    local left = {}
    for _, it in ipairs(data.after.items) do left[it.inventory_key] = it.quantity end
    local stamp = os.date("%H:%M")
    for _, it in ipairs(data.before.items) do
        local taken = it.quantity - (left[it.inventory_key] or 0)
        if taken > 0 then
            table.insert(M.log, 1, {text = string.format("%s %s x%d", stamp, it.name, taken)})
            if #M.log > M.LOG_MAX then table.remove(M.log) end
        end
    end
end

return M
