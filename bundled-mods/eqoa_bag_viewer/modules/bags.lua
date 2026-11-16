-- Bag viewer model: what the window shows, read from eqoa.inventory / eqoa.bank / eqoa.loot and game events.
-- Nothing here is a copy of game state that can drift: the grid is read again on a change event or an open.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local M = {
    open = false, tab = 1, grid = nil, log = {}, scroll = 0, hover = nil,
    builds = 0, draws = 0, rect = nil, menu = nil, result = nil,
    pending_label = nil, last_req = nil,
}

M.COLUMNS, M.MAX_ROWS, M.LOG_MAX = 8, 8, 50
-- The game's item-page tabs, in its order, with All first and the bank last. A stack is under a tab when the
-- game's own rule for that tab says so: `Item.tabs` lists the keys, the rule is not copied here.
M.TABS = {
    {key = "all", label = "All"},
    {key = "usable", label = "Usable"},
    {key = "equipment", label = "Equipment"},
    {key = "charged", label = "Charged"},
    {key = "misc", label = "Misc"},
    {key = "bank", label = "Bank"},
}
M.CORPSE_TAB = {key = "corpse", label = "Corpse"}

local function has_tab(it, key)
    if not it or not it.tabs then return false end
    for _, k in ipairs(it.tabs) do if k == key then return true end end
    return false
end

local function keeps(key, it)
    if key == "all" or key == "bank" or key == "corpse" then return true end
    return has_tab(it, key)
end

function M.visible_tabs()
    local tabs = {}
    for _, t in ipairs(M.TABS) do tabs[#tabs + 1] = t end
    if eqoa.loot.get() then
        local t = M.CORPSE_TAB
        tabs[#tabs + 1] = {key = t.key, label = t.label, idx = 7}
    end
    for i, t in ipairs(tabs) do
        tabs[i] = {key = t.key, label = t.label, idx = t.idx or i}
    end
    return tabs
end

-- The grid for the tab on show: the items it keeps in the container's own order, with the numbers under it.
function M.rebuild()
    M.builds = M.builds + 1
    local key
    if M.tab == 7 then
        key = "corpse"
    elseif M.TABS[M.tab] then
        key = M.TABS[M.tab].key
    else
        M.tab = 1
        key = "all"
    end
    local c
    if key == "bank" then
        c = eqoa.bank.get()
    elseif key == "corpse" then
        c = eqoa.loot.get()
        if not c then
            M.tab = 1
            key = "all"
            c = eqoa.inventory.get()
        end
    else
        c = eqoa.inventory.get()
    end
    if not c then M.grid = false; return false end
    local items = {}
    for _, it in ipairs(c.items) do
        if keeps(key, it) then
            local info = it.icon and eqoa.icons.resource(it.icon, {trim = true, trim_color = {r = 0, g = 0, b = 0, a = 1}})
            items[#items + 1] = {item = it, texture = info and info.texture}
        end
    end
    local slots = math.max(c.capacity or 40, #c.items)
    local rows = math.max(5, math.min(M.MAX_ROWS, math.ceil(slots / M.COLUMNS)))
    M.grid = {
        items = items, used = c.used, capacity = c.capacity, money = c.money,
        rows = rows, bank = key == "bank", corpse = key == "corpse",
    }
    M.hover = nil
    M.menu = nil
    return M.grid
end

function M.dirty() M.grid = nil end

function M.select(tab)
    if tab == 7 and eqoa.loot.get() and M.tab ~= 7 then
        M.tab = 7; M.scroll = 0; M.grid = nil; M.menu = nil
    elseif tab ~= M.tab and M.TABS[tab] then
        M.tab = tab; M.scroll = 0; M.grid = nil; M.menu = nil
    end
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

local function cannot_reason(it, can)
    if not can or can.allowed then return nil end
    local r = can.reasons and can.reasons[1]
    if r == "class" then return "wrong class"
    elseif r == "race" then return "wrong race"
    elseif r == "level" then return "requires level " .. tostring(it.level_required or 0)
    end
    return "cannot use item"
end

-- Build the click menu for one stack (never includes Destroy; refused entries carry the game's reason).
function M.build_menu(it, fx, fy)
    local rows = {}
    local function add(label, enabled, reason, run)
        rows[#rows + 1] = {label = label, enabled = enabled, reason = reason, run = run}
    end
    local loot = eqoa.loot.get()
    local bank = eqoa.bank.get()
    local bank_open = bank and bank.open
    local merch = eqoa.merchant.get()
    local merch_open = merch ~= nil

    if it.list == "loot" then
        local take_qty = it.quantity or 1
        add(take_qty > 1 and ("Take stack (" .. take_qty .. ")") or "Take", loot ~= nil,
            loot == nil and "corpse window is not open" or nil,
            function() return eqoa.actions.loot(it.ref, take_qty) end)
        if take_qty > 1 then
            add("Take 1", loot ~= nil,
                loot == nil and "corpse window is not open" or nil,
                function() return eqoa.actions.loot(it.ref, 1) end)
        end
        add("Take all", loot ~= nil,
            loot == nil and "corpse window is not open" or nil,
            function() return eqoa.actions.loot_all() end)
    elseif it.list == "bank" then
        if it.quantity > 1 then
            add("Withdraw 1", bank_open, not bank_open and "bank window is not open" or nil,
                function() return eqoa.actions.bank_move("bank", it.ref, 1) end)
            add("Withdraw stack (" .. it.quantity .. ")", bank_open, not bank_open and "bank window is not open" or nil,
                function() return eqoa.actions.bank_move("bank", it.ref, it.quantity) end)
        else
            add("Withdraw", bank_open, not bank_open and "bank window is not open" or nil,
                function() return eqoa.actions.bank_move("bank", it.ref, 1) end)
        end
    else
        local can = eqoa.items.can_use(it.ref)
        local deny = cannot_reason(it, can)
        local block_msg = merch_open and "close merchant first"
            or (bank_open and "close bank first"
            or (loot ~= nil and "close corpse first" or nil))
        local wearable = it.wear_slot ~= nil and it.wear_slot ~= 4294967295
        local usable = has_tab(it, "usable") or has_tab(it, "charged")

        if usable then
            add("Use", deny == nil and block_msg == nil, deny or block_msg,
                function() return eqoa.actions.use_item(it.ref) end)
        elseif not wearable then
            add("Use", false, "item is not usable", nil)
        end

        if wearable then
            local elabel = it.equipped and "Unequip" or "Equip"
            add(elabel, deny == nil and block_msg == nil, deny or block_msg,
                function() return eqoa.actions.equip(it.ref) end)
        elseif not usable then
            add("Equip", false, "item is not equippable", nil)
        end

        if usable then
            for s = 1, 4 do
                add("Quick slot " .. s, block_msg == nil, block_msg,
                    function() return eqoa.actions.quick_slot(it.ref, s) end)
            end
        end

        if merch_open then
            local sell_deny = it.equipped and "cannot sell an equipped item" or nil
            local p1 = eqoa.merchant.price(it.ref, "sell", 1)
            local lbl1 = "Sell 1" .. (p1 and (" (" .. M.tunar(p1) .. "t)") or "")
            add(lbl1, sell_deny == nil, sell_deny,
                function() return eqoa.actions.sell(it.ref, 1) end)
            if (it.quantity or 1) > 1 then
                local ps = eqoa.merchant.price(it.ref, "sell", it.quantity)
                local lbls = "Sell stack (" .. it.quantity .. (ps and (", " .. M.tunar(ps) .. "t)") or ")")
                add(lbls, sell_deny == nil, sell_deny,
                    function() return eqoa.actions.sell(it.ref, it.quantity) end)
            end
        end

        if bank_open then
            local dep_deny = it.equipped and "cannot deposit an equipped item" or nil
            if (it.quantity or 1) > 1 then
                add("Deposit 1", dep_deny == nil, dep_deny,
                    function() return eqoa.actions.bank_move("inventory", it.ref, 1) end)
                add("Deposit stack (" .. it.quantity .. ")", dep_deny == nil, dep_deny,
                    function() return eqoa.actions.bank_move("inventory", it.ref, it.quantity) end)
            else
                add("Deposit", dep_deny == nil, dep_deny,
                    function() return eqoa.actions.bank_move("inventory", it.ref, 1) end)
            end
        end
    end
    M.menu = {title = it.name, ref = it.ref, rows = rows, fx = fx or 8, fy = fy or 46}
    return M.menu
end

function M.trigger_menu(row)
    if not row then return end
    if not row.enabled or not row.run then
        M.result = {text = row.label .. ": " .. (row.reason or "unavailable"), ok = false}
        return
    end
    M.pending_label = row.label
    local req, err = row.run()
    if not req then
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = row.label .. ": " .. msg, ok = false}
        M.last_req = nil
    else
        M.last_req = req
        M.result = {text = row.label .. ": queued...", ok = nil}
    end
end

function M.on_action_result(d)
    if not d then return end
    if M.last_req == nil or d.request_id ~= M.last_req then return end
    local label = M.pending_label or d.kind
    if d.state == "dispatched" and d.phase == "dispatch" then
        M.result = {text = label .. ": waiting for server...", ok = nil}
    elseif d.ok == true then
        M.result = {text = label .. ": ok", ok = true}
    else
        M.result = {text = label .. ": " .. (d.reason or d.code or d.state), ok = false}
    end
end

-- A pickup is what a corpse loses: loot.changed with the native cause for one item (0x0019) or all (0x001A).
-- Stacks are told apart by their wire key, which the corpse keeps until it is closed.
function M.log_loot(data)
    if (data.cause ~= "loot_one" and data.cause ~= "loot_all") or not data.before or not data.after then return end
    local left = {}
    for _, it in ipairs(data.after.items) do left[it.inventory_key] = it.quantity end
    local stamp = os.date("%H:%M")
    -- the corpse the window belongs to, when the client knows which it asked for
    local source = data.before.source
    for _, it in ipairs(data.before.items) do
        local taken = it.quantity - (left[it.inventory_key] or 0)
        if taken > 0 then
            local text = string.format("%s %s x%d", stamp, it.name, taken)
            if source then text = text .. ", from " .. source.name end
            table.insert(M.log, 1, {text = text})
            if #M.log > M.LOG_MAX then table.remove(M.log) end
        end
    end
end

return M
