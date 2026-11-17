-- Bag viewer model: what the window shows, read from eqoa.inventory / eqoa.bank / eqoa.loot and game events.
-- Nothing here is a copy of game state that can drift: the grid is read again on a change event or an open.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local M = {
    open = false, tab = 1, grid = nil, log = {}, scroll = 0, hover = nil,
    builds = 0, draws = 0, rect = nil, menu = nil, result = nil,
    pending_label = nil, last_req = nil,
    junk_ids = {}, sell_grey = false, junk_loaded = false,
    batch_id = nil, batch_kind = nil, batch_verb = nil, batch_total = 0,
    action_hits = {},
    icon_req = {}, has_pending_icons = false,
}

local JUNK_FILE = "junk.cfg"
local TRIM_OPTS = {trim = true, trim_color = {r = 0, g = 0, b = 0, a = 1}}
local ICON_RETRY_LIMIT = 5
local _frame_tried = {}
local _frame_tex = {}

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

function M.reset_icons()
    for k in pairs(M.icon_req) do M.icon_req[k] = nil end
    M.has_pending_icons = false
end

local function clear_pass_cache()
    for k in pairs(_frame_tried) do _frame_tried[k] = nil end
    for k in pairs(_frame_tex) do _frame_tex[k] = nil end
end

local function fetch_entry_icon(entry)
    local icon = entry.item and entry.item.icon
    if not icon then return end
    local st = M.icon_req[icon]
    if st == "failed" then return end
    local cached = _frame_tex[icon]
    if cached then
        entry.texture = cached
        return
    end
    if _frame_tried[icon] then
        M.has_pending_icons = true
        return
    end
    local info = eqoa.icons.resource(icon, TRIM_OPTS)
    if info and info.texture then
        entry.texture = info.texture
        M.icon_req[icon] = "ok"
        _frame_tex[icon] = info.texture
    else
        _frame_tried[icon] = true
        local tries = (type(st) == "number" and st or 0) + 1
        if tries >= ICON_RETRY_LIMIT then
            M.icon_req[icon] = "failed"
        else
            M.icon_req[icon] = tries
            M.has_pending_icons = true
        end
    end
end

function M.sync_visible_icons(first_slot, last_slot)
    local grid = M.grid
    if not grid then return end
    M.has_pending_icons = false
    clear_pass_cache()
    local items = grid.items
    for i = 1, #items do
        local entry = items[i]
        if i < first_slot or i > last_slot then
            entry.texture = nil
        elseif entry.texture == nil then
            fetch_entry_icon(entry)
        end
    end
    clear_pass_cache()
end

function M.resolve_pending_icons(first_slot, last_slot)
    local grid = M.grid
    if not grid or not M.has_pending_icons then return end
    M.has_pending_icons = false
    clear_pass_cache()
    local items = grid.items
    local hi = math.min(#items, last_slot)
    for i = math.max(1, first_slot), hi do
        local entry = items[i]
        if entry.texture == nil and entry.item and entry.item.icon then
            fetch_entry_icon(entry)
        end
    end
    clear_pass_cache()
end

local function active_key_and_container()
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
    return key, c
end

function M.is_grid_stale()
    local grid = M.grid
    if grid == nil then return true end
    local key, c = active_key_and_container()
    if not c then return grid ~= false end
    if grid == false then return true end
    return grid.tab_key ~= key or grid.rev ~= c.revision
end

-- The grid for the tab on show: the items it keeps in the container's own order, with the numbers under it.
function M.rebuild()
    M.builds = M.builds + 1
    local key, c = active_key_and_container()
    if not c then M.grid = false; M.has_pending_icons = false; return false end
    local items = {}
    for _, it in ipairs(c.items) do
        if keeps(key, it) then
            items[#items + 1] = {item = it, texture = nil}
        end
    end
    local slots = math.max(c.capacity or 40, #c.items)
    local rows = math.max(5, math.min(M.MAX_ROWS, math.ceil(slots / M.COLUMNS)))
    M.grid = {
        items = items, used = c.used, capacity = c.capacity, money = c.money,
        rows = rows, bank = key == "bank", corpse = key == "corpse",
        tab_key = key, rev = c.revision,
    }
    M.sync_visible_icons(1, rows * M.COLUMNS)
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

function M.load_junk()
    M.junk_ids = {}
    M.sell_grey = false
    if eqoa and eqoa.files and eqoa.files.read then
        local data = eqoa.files.read(JUNK_FILE)
        if type(data) == "string" then
            for line in data:gmatch("[^\r\n]+") do
                if line == "grey=1" then
                    M.sell_grey = true
                elseif line == "grey=0" then
                    M.sell_grey = false
                elseif line:match("^%d+$") then
                    M.junk_ids[line] = true
                end
            end
        end
    end
    M.junk_loaded = true
end

function M.save_junk()
    if not (eqoa and eqoa.files and eqoa.files.write) then return end
    local ids = {}
    for k, v in pairs(M.junk_ids) do
        if v then ids[#ids + 1] = k end
    end
    table.sort(ids)
    local lines = {M.sell_grey and "grey=1" or "grey=0"}
    for _, k in ipairs(ids) do lines[#lines + 1] = k end
    eqoa.files.write(JUNK_FILE, table.concat(lines, "\n") .. "\n")
end

-- Grey vendor trash: unequipped, non-equippable, non-quickslot misc item without usable/charged role or stats.
function M.is_grey_item(it)
    if not it or it.equipped or it.equippable or it.quick_slot ~= nil then return false end
    local wearable = it.wear_slot ~= nil and it.wear_slot ~= 0 and it.wear_slot ~= 4294967295
    if wearable then return false end
    if has_tab(it, "usable") or has_tab(it, "charged") or has_tab(it, "equipment") then return false end
    if not has_tab(it, "misc") then return false end
    if (it.charges or 0) > 0 then return false end
    if it.stats and #it.stats > 0 then return false end
    return true
end

function M.is_junk(it)
    if not it then return false end
    if M.junk_ids[tostring(it.id)] then return true end
    if M.sell_grey and M.is_grey_item(it) then return true end
    return false
end

function M.set_junk(it, flag)
    if not it then return end
    if flag then
        M.junk_ids[tostring(it.id)] = true
        M.result = {text = "Marked " .. it.name .. " as junk", ok = true}
    else
        M.junk_ids[tostring(it.id)] = nil
        M.result = {text = "Unmarked " .. it.name, ok = true}
    end
    M.save_junk()
end

function M.toggle_grey()
    M.sell_grey = not M.sell_grey
    M.save_junk()
    M.result = {
        text = M.sell_grey and "Also sell grey items: ON" or "Also sell grey items: OFF",
        ok = nil,
    }
end

function M.junk_sell_list()
    local inv = eqoa.inventory.get()
    if not inv then return {}, 0, 0 end
    local steps, total_tunar = {}, 0
    for _, it in ipairs(inv.items) do
        if not it.equipped and M.is_junk(it) then
            local qty = it.quantity or 1
            local q = eqoa.merchant.price(it.ref, "sell", qty)
            steps[#steps + 1] = {kind = "sell", item = it.ref, count = qty}
            total_tunar = total_tunar + (q or 0)
        end
    end
    return steps, #steps, total_tunar
end

function M.loot_non_junk_list()
    local loot = eqoa.loot.get()
    if not loot then return {}, 0 end
    local steps = {}
    for _, it in ipairs(loot.items) do
        if not M.is_junk(it) then
            steps[#steps + 1] = {kind = "loot", item = it.ref, count = it.quantity or 1}
        end
    end
    return steps, #loot.items
end

function M.start_sell_junk()
    local steps, count = M.junk_sell_list()
    if count == 0 then
        M.result = {text = "No junk items to sell", ok = false}
        return
    end
    local bid, err = eqoa.actions.batch({steps = steps, require_window = "merchant"})
    if not bid then
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = "Sell junk: " .. msg, ok = false}
        return
    end
    M.batch_id = bid
    M.batch_kind = "sell"
    M.batch_verb = "Sold"
    M.batch_total = count
    M.result = {text = string.format("Selling junk 0 of %d...", count), ok = nil}
end

function M.start_loot_non_junk()
    local steps, total = M.loot_non_junk_list()
    if total == 0 then
        M.dirty()
        M.result = {text = "Corpse has no items", ok = false}
        return
    end
    if #steps == 0 then
        M.result = {text = "Corpse has only junk items", ok = false}
        return
    end
    local bid, err = eqoa.actions.batch({steps = steps, require_window = "loot"})
    if not bid then
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = "Take all but junk: " .. msg, ok = false}
        return
    end
    M.batch_id = bid
    M.batch_kind = "loot"
    M.batch_verb = "Looted"
    M.batch_total = #steps
    M.result = {text = string.format("Looting %d of %d...", 0, #steps), ok = nil}
end

function M.stop_batch()
    if not M.batch_id then return end
    local stopped = eqoa.actions.stop_batch(M.batch_id)
    if stopped then
        M.result = {text = "Stopping batch...", ok = nil}
    end
end

function M.on_batch_progress(d)
    if not d or not M.batch_id or d.batch_id ~= M.batch_id then return end
    local verb = M.batch_kind == "loot" and "Looting" or "Selling junk"
    M.result = {
        text = string.format("%s %d of %d...", verb, d.completed or 0, d.total or M.batch_total or 0),
        ok = nil,
    }
end

function M.on_batch_result(d)
    if not d or not M.batch_id or d.batch_id ~= M.batch_id then return end
    local verb = M.batch_verb or (M.batch_kind == "loot" and "Looted" or "Sold")
    local done, total = d.completed or 0, d.total or M.batch_total or 0
    if d.ok == true then
        M.result = {text = string.format("%s %d of %d", verb, done, total), ok = true}
    else
        local reason = d.reason or d.code or d.state or "stopped"
        M.result = {text = string.format("%s %d of %d: %s", verb, done, total, reason), ok = false}
    end
    M.batch_id = nil
end

-- Build the click menu for one stack (never includes Destroy; refused entries carry the game's reason).
function M.build_menu(it, fx, fy)
    local rows = {}
    local function add(label, enabled, reason, run, local_only)
        rows[#rows + 1] = {label = label, enabled = enabled, reason = reason, run = run, local_only = local_only}
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
    if M.junk_ids[tostring(it.id)] then
        add("Unmark", true, nil, function() M.set_junk(it, false) end, true)
    else
        add("Mark as junk", not it.equipped,
            it.equipped and "cannot mark an equipped item as junk" or nil,
            function() M.set_junk(it, true) end, true)
    end
    M.menu = {title = it.name, ref = it.ref, rows = rows, fx = fx or 8, fy = fy or 46}
    return M.menu
end

function M.trigger_menu(row)
    if not row then return end
    if row.local_only and row.enabled and row.run then
        row.run()
        return
    end
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
