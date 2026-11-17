-- Bag viewer model: what the window shows, read from eqoa.inventory / eqoa.bank / eqoa.loot and game events.
-- Nothing here is a copy of game state that can drift: the grid is read again on a change event or an open.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local M = {
    open = false, tab = 1, grid = nil, log = {}, scroll = 0, grid_scroll = 0, hover = nil,
    builds = 0, draws = 0, rect = nil, menu = nil, result = nil,
    pending_label = nil, last_req = nil,
    junk_ids = {}, sell_grey = false, junk_loaded = false,
    batch_id = nil, batch_kind = nil, batch_verb = nil, batch_total = 0,
    corpse_tab_kept = false,
    action_hits = {},
    icon_req = {}, has_pending_icons = false,
}

local JUNK_FILE = "junk.cfg"
local TRIM_OPTS = {trim = true, trim_color = {r = 0, g = 0, b = 0, a = 1}}
local ICON_RETRY_LIMIT = 5
local ICON_RETRY_COOLDOWN_S = 10
local _frame_tried = {}
local _frame_tex = {}
local EMPTY_CORPSE = {items = {}, used = 0, capacity = 40, money = 0, revision = -1}

local function loot_batch_active()
    return M.batch_id ~= nil and M.batch_kind == "loot"
end

local function keeping_corpse_tab()
    return M.tab == 7 and (loot_batch_active() or M.corpse_tab_kept)
end

local function dist_sq(a, b)
    if not a or not b then return nil end
    local dx = (a.x or 0) - (b.x or 0)
    local dy = (a.y or 0) - (b.y or 0)
    local dz = (a.z or 0) - (b.z or 0)
    return dx * dx + dy * dy + dz * dz
end

function M.nearby_corpse()
    local list = eqoa.entities and eqoa.entities.list and eqoa.entities.list()
    if not list then return nil end
    local pl = eqoa.player and eqoa.player.get and eqoa.player.get()
    local ppos = pl and pl.position
    local best, best_d2 = nil, nil
    for _, e in ipairs(list) do
        if e.corpse then
            local d2 = dist_sq(ppos, e.actor_position or e.position) or 0
            if not best or d2 < best_d2 then
                best, best_d2 = e, d2
            end
        end
    end
    return best, best_d2
end

local function has_bit(val, b)
    return val ~= nil and math.floor(val / b) % 2 == 1
end

function M.nearby_npc(kind)
    local list = eqoa.entities and eqoa.entities.list and eqoa.entities.list()
    if not list then return nil end
    local pl = eqoa.player and eqoa.player.get and eqoa.player.get()
    local ppos = pl and pl.position
    local want_bit = kind == "merchant" and 8 or (kind == "bank" and 2 or 0)
    local best, best_d2 = nil, nil
    for _, e in ipairs(list) do
        if not e.corpse and e.npc_flags then
            local match = want_bit ~= 0 and has_bit(e.npc_flags, want_bit)
                or (want_bit == 0 and (has_bit(e.npc_flags, 128) or has_bit(e.npc_flags, 8) or has_bit(e.npc_flags, 2) or has_bit(e.npc_flags, 1)))
            if match then
                local d2 = dist_sq(ppos, e.actor_position or e.position) or 0
                if not best or d2 < best_d2 then
                    best, best_d2 = e, d2
                end
            end
        end
    end
    return best, best_d2
end

function M.ensure_takeover(kind)
    if not (eqoa.windows and eqoa.windows.takeover and eqoa.windows.taken) then
        return true
    end
    if eqoa.windows.taken(kind) then
        return true
    end
    local ok, err = eqoa.windows.takeover(kind)
    if not ok then
        M.result = {text = (err and err.message) or ("cannot take over " .. kind), ok = false}
        M.dirty()
        return false
    end
    return true
end

function M.clear_corpse_hold()
    M.corpse_tab_kept = false
    M.pending_open = nil
    M.eligible_corpses = {}
    M.auto_looted = {}
    M.tracked_target_id = nil
    M.tracked_target_pos = nil
    M.recently_dead = nil
end

M.COLUMNS, M.MAX_ROWS, M.LOG_MAX = 8, 6, 50
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
    if eqoa.loot.get() or keeping_corpse_tab() or M.nearby_corpse() ~= nil then
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
    if type(st) == "number" and st < 0 then return end
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
        local tries = (type(st) == "number" and st > 0 and st or 0) + 1
        if tries >= ICON_RETRY_LIMIT then
            M.icon_req[icon] = -(os.clock() + ICON_RETRY_COOLDOWN_S)
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
    local now = os.clock()
    local items = grid.items
    for i = 1, #items do
        local entry = items[i]
        if i < first_slot or i > last_slot then
            entry.texture = nil
        elseif entry.texture == nil then
            local icon = entry.item and entry.item.icon
            local st = icon and M.icon_req[icon]
            if type(st) == "number" and st < 0 and now >= -st then
                M.icon_req[icon] = nil
            end
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
        if c then
            if not loot_batch_active() then
                M.corpse_tab_kept = false
            end
        elseif keeping_corpse_tab() or M.nearby_corpse() ~= nil then
            c = EMPTY_CORPSE
        else
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

function M.visible_slot_range()
    local grid = M.grid
    if not grid then return 1, 0 end
    local first = M.grid_scroll * M.COLUMNS + 1
    local last = (M.grid_scroll + grid.rows) * M.COLUMNS
    return first, last
end

function M.scroll_grid(delta)
    local grid = M.grid
    if not grid then return end
    local max_scroll = math.max(0, (grid.total_rows or grid.rows) - grid.rows)
    local next_scroll = math.max(0, math.min(max_scroll, M.grid_scroll + delta))
    if next_scroll == M.grid_scroll then return end
    M.grid_scroll = next_scroll
    local first, last = M.visible_slot_range()
    M.sync_visible_icons(first, last)
end

-- The grid for the tab on show: the items it keeps in the container's own order, with the numbers under it.
function M.rebuild()
    M.builds = M.builds + 1
    local key, c = active_key_and_container()
    if not c then M.grid = false; M.has_pending_icons = false; M.grid_scroll = 0; return false end
    local items = {}
    for _, it in ipairs(c.items) do
        if keeps(key, it) then
            items[#items + 1] = {item = it, texture = nil}
        end
    end
    local slots = math.max(c.capacity or 40, #c.items)
    local total_rows = math.max(5, math.ceil(slots / M.COLUMNS))
    local rows = math.min(M.MAX_ROWS, total_rows)
    M.grid_scroll = math.max(0, math.min(math.max(0, total_rows - rows), M.grid_scroll))
    M.grid = {
        items = items, used = c.used, capacity = c.capacity, money = c.money,
        rows = rows, total_rows = total_rows,
        bank = key == "bank", corpse = key == "corpse",
        tab_key = key, rev = c.revision,
    }
    local first, last = M.visible_slot_range()
    M.sync_visible_icons(first, last)
    M.hover = nil
    M.menu = nil
    return M.grid
end

function M.dirty()
    for k, v in pairs(M.icon_req) do
        if type(v) == "number" and v < 0 then M.icon_req[k] = nil end
    end
    M.grid = nil
end

function M.select(tab)
    if tab == 7 and (eqoa.loot.get() or keeping_corpse_tab() or M.nearby_corpse() ~= nil) and M.tab ~= 7 then
        M.tab = 7; M.scroll = 0; M.grid_scroll = 0; M.grid = nil; M.menu = nil
    elseif tab ~= M.tab and M.TABS[tab] then
        M.corpse_tab_kept = false
        M.tab = tab; M.scroll = 0; M.grid_scroll = 0; M.grid = nil; M.menu = nil
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
    local has_merch = eqoa.merchant.get() ~= nil
    local steps, total_tunar = {}, 0
    for _, it in ipairs(inv.items) do
        if not it.equipped and M.is_junk(it) then
            local qty = it.quantity or 1
            local q = has_merch and eqoa.merchant.price(it.ref, "sell", qty) or 0
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

function M.open_loot_corpse(after_kind, corpse_id)
    if not M.ensure_takeover("loot") then return nil, {message = "window busy"} end
    local req, err = eqoa.actions.open_loot(corpse_id)
    if not req then
        local msg = (err and (err.message or err.code)) or "refused"
        M.result = {text = "Loot: " .. msg, ok = false}
        M.dirty()
        return nil, err
    end
    local verb = after_kind == "loot_all" and "Take all"
        or (after_kind == "loot_non_junk" and "Take all but junk" or "Loot")
    M.pending_open = {kind = "loot", req = req, after = after_kind, verb = verb}
    M.result = {text = "Opening corpse...", ok = nil}
    M.dirty()
    return req, nil
end

function M.open_talk_npc(talk_kind, verb, after_fn)
    if not M.ensure_takeover(talk_kind) then return nil, {message = "window busy"} end
    local req, err = eqoa.actions.talk(nil, {kind = talk_kind})
    if not req then
        local msg = (err and (err.message or err.code)) or "refused"
        M.result = {text = verb .. ": " .. msg, ok = false}
        M.dirty()
        return nil, err
    end
    M.pending_open = {kind = talk_kind, req = req, fn = after_fn, verb = verb}
    M.result = {text = talk_kind == "merchant" and "Opening merchant..." or "Opening bank...", ok = nil}
    M.dirty()
    return req, nil
end

function M.start_loot_button()
    local l = eqoa.loot.get()
    if l and l.open then
        M.select(#M.TABS + 1)
        M.result = {text = "Corpse ready", ok = true}
        M.dirty()
        return true
    end
    return M.open_loot_corpse(nil) ~= nil
end

function M.start_sell_junk()
    local merch = eqoa.merchant.get()
    if not merch then
        if M.nearby_npc("merchant") ~= nil then
            return M.open_talk_npc("merchant", "Sell junk", M.start_sell_junk) ~= nil
        end
        M.result = {text = "Sell junk: merchant is not open", ok = false}
        return false
    end
    local steps, count = M.junk_sell_list()
    if count == 0 then
        M.result = {text = "No junk items to sell", ok = false}
        return false
    end
    local bid, err = eqoa.actions.batch({steps = steps, require_window = "merchant"})
    if not bid then
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = "Sell junk: " .. msg, ok = false}
        return false
    end
    M.batch_id = bid
    M.batch_kind = "sell"
    M.batch_verb = "Sold"
    M.batch_total = count
    M.result = {text = string.format("Selling junk 0 of %d...", count), ok = nil}
    return true
end

function M.start_loot_all()
    local loot = eqoa.loot.get()
    if not loot then
        if M.nearby_corpse() ~= nil then
            return M.open_loot_corpse("loot_all") ~= nil
        end
        M.dirty()
        M.result = {text = "Corpse has no items", ok = false}
        return false
    end
    if #(loot.items or {}) == 0 then
        M.dirty()
        M.result = {text = "Corpse has no items", ok = false}
        return false
    end
    M.pending_label = "Take all"
    local req, err = eqoa.actions.loot_all()
    if not req then
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = "Take all: " .. msg, ok = false}
        M.last_req = nil
        return false
    else
        M.last_req = req
        M.result = {text = "Take all: queued...", ok = nil}
        return true
    end
end

function M.start_loot_non_junk()
    local loot = eqoa.loot.get()
    if not loot then
        if M.nearby_corpse() ~= nil then
            return M.open_loot_corpse("loot_non_junk") ~= nil
        end
        M.dirty()
        M.result = {text = "Corpse has no items", ok = false}
        return false
    end
    local steps, total = M.loot_non_junk_list()
    if total == 0 then
        M.dirty()
        M.result = {text = "Corpse has no items", ok = false}
        return false
    end
    if #steps == 0 then
        M.result = {text = "Corpse has only junk items", ok = false}
        return false
    end
    local bid, err = eqoa.actions.batch({steps = steps, require_window = "loot"})
    if not bid then
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = "Take all but junk: " .. msg, ok = false}
        return false
    end
    M.batch_id = bid
    M.batch_kind = "loot"
    M.batch_verb = "Looted"
    M.batch_total = #steps
    M.corpse_tab_kept = (M.tab == 7)
    M.result = {text = string.format("Looting %d of %d...", 0, #steps), ok = nil}
    return true
end

function M.stop_batch()
    if M.pending_open then
        local req = M.pending_open.req
        M.pending_open = nil
        if req then pcall(eqoa.actions.cancel, req) end
        M.result = {text = "Stopped", ok = false}
        M.dirty()
    end
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
    if M.batch_kind == "loot" and M.tab == 7 then
        M.corpse_tab_kept = true
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
        local near_bank = M.nearby_npc("bank") ~= nil
        local can_bank = bank_open or near_bank
        local function bank_move_or_open(qty)
            if bank and bank.open then
                return eqoa.actions.bank_move("bank", it.ref, qty)
            end
            return M.open_talk_npc("bank", "Withdraw", function()
                M.run_single("Withdraw", function() return eqoa.actions.bank_move("bank", it.ref, qty) end)
            end)
        end
        if it.quantity > 1 then
            add("Withdraw 1", can_bank, not can_bank and "bank window is not open" or nil,
                function() return bank_move_or_open(1) end)
            add("Withdraw stack (" .. it.quantity .. ")", can_bank, not can_bank and "bank window is not open" or nil,
                function() return bank_move_or_open(it.quantity) end)
        else
            add("Withdraw", can_bank, not can_bank and "bank window is not open" or nil,
                function() return bank_move_or_open(1) end)
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

        local near_bank = M.nearby_npc("bank") ~= nil
        if bank_open or near_bank then
            local dep_deny = it.equipped and "cannot deposit an equipped item" or nil
            local function dep_move_or_open(qty)
                if bank_open then
                    return eqoa.actions.bank_move("inventory", it.ref, qty)
                end
                return M.open_talk_npc("bank", "Deposit", function()
                    M.run_single("Deposit", function() return eqoa.actions.bank_move("inventory", it.ref, qty) end)
                end)
            end
            if (it.quantity or 1) > 1 then
                add("Deposit 1", dep_deny == nil, dep_deny,
                    function() return dep_move_or_open(1) end)
                add("Deposit stack (" .. it.quantity .. ")", dep_deny == nil, dep_deny,
                    function() return dep_move_or_open(it.quantity) end)
            else
                add("Deposit", dep_deny == nil, dep_deny,
                    function() return dep_move_or_open(1) end)
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

function M.run_single(label, fn)
    M.pending_label = label
    local req, err = fn()
    if not req then
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = label .. ": " .. msg, ok = false}
        M.last_req = nil
    else
        M.last_req = req
        if not M.pending_open then
            M.result = {text = label .. ": queued...", ok = nil}
        end
    end
    M.dirty()
    return req, err
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
    M.run_single(row.label, row.run)
end

local function finish_pending_open(expected_kind)
    local p = M.pending_open
    if not p or p.kind ~= expected_kind then return false end
    M.pending_open = nil
    if p.kind == "loot" then
        if p.after == "loot_all" then
            M.start_loot_all()
        elseif p.after == "loot_non_junk" then
            M.start_loot_non_junk()
        else
            M.result = {text = "Corpse ready", ok = true}
            M.dirty()
        end
    elseif p.fn then
        p.fn()
    end
    return true
end

function M.on_loot_changed(d)
    if d and d.before and not d.after then
        if not keeping_corpse_tab() and M.tab == 7 then
            M.tab = 1
        end
    elseif eqoa.loot.get() ~= nil then
        finish_pending_open("loot")
    end
    M.dirty()
end

function M.on_merchant_changed(d)
    if eqoa.merchant.get() ~= nil then
        finish_pending_open("merchant")
    end
    M.dirty()
end

function M.on_bank_changed(d)
    local b = eqoa.bank.get()
    if b and b.open then
        finish_pending_open("bank")
    end
    M.dirty()
end

function M.on_target_update()
    local e = eqoa.target and eqoa.target.get and eqoa.target.get()
    if not e and eqoa.player and eqoa.player.get and eqoa.entities and eqoa.entities.find then
        local pl = eqoa.player.get()
        if pl and pl.target_id then
            e = eqoa.entities.find(pl.target_id)
        end
    end
    if e and not e.corpse then
        M.tracked_target_id = e.id
        M.tracked_target_pos = e.actor_position or e.position
    end
end

function M.on_entity_changed(d)
    if not (d and d.after) then return end
    local after = d.after
    if after.corpse and ((d.before and not d.before.corpse) or after.id == M.tracked_target_id) then
        if after.id == M.tracked_target_id then
            M.eligible_corpses = M.eligible_corpses or {}
            M.eligible_corpses[after.id] = true
        end
    elseif not after.corpse and after.id == M.tracked_target_id then
        M.tracked_target_pos = after.actor_position or after.position
    end
end

function M.on_entity_removed(d)
    if d and d.before and not d.before.corpse and d.before.id == M.tracked_target_id then
        M.recently_dead = {pos = d.before.actor_position or d.before.position, expires = os.clock() + 3.0}
    end
end

function M.on_entity_added(d)
    local after = d and d.after
    if not after or not after.corpse then return end
    if M.recently_dead and os.clock() <= M.recently_dead.expires then
        local d2 = dist_sq(M.recently_dead.pos, after.actor_position or after.position)
        if d2 ~= nil and d2 <= 36.0 then
            M.eligible_corpses = M.eligible_corpses or {}
            M.eligible_corpses[after.id] = true
        end
    end
end

function M.tick_auto_loot()
    local enabled = eqoa.options and eqoa.options.get and eqoa.options.get("auto_loot") == true
    if not enabled then return end
    if not (M.eligible_corpses and next(M.eligible_corpses)) then return end
    if M.batch_id or M.pending_open or M.last_req or loot_batch_active() then return end
    local list = eqoa.entities and eqoa.entities.list and eqoa.entities.list()
    if not list then return end
    local pl = eqoa.player and eqoa.player.get and eqoa.player.get()
    local ppos = pl and pl.position
    local best, best_d2 = nil, nil
    for _, e in ipairs(list) do
        if e.corpse and M.eligible_corpses[e.id] and not (M.auto_looted and M.auto_looted[e.id]) then
            local d2 = dist_sq(ppos, e.actor_position or e.position)
            if (d2 == nil or d2 <= 225.0) and (not best or (d2 or 0) < (best_d2 or 0)) then
                best, best_d2 = e, d2 or 0
            end
        end
    end
    if best then
        M.auto_looted = M.auto_looted or {}
        M.auto_looted[best.id] = true
        M.open_loot_corpse("loot_non_junk", best.id)
    end
end

function M.on_action_result(d)
    if not d then return end
    if M.pending_open and d.request_id == M.pending_open.req then
        local p = M.pending_open
        if d.ok == false then
            M.pending_open = nil
            local msg = d.reason or d.code or d.state or "refused"
            M.result = {text = (p.verb or d.kind) .. ": " .. msg, ok = false}
            M.dirty()
            return
        elseif d.ok == true and d.phase ~= "dispatch" then
            finish_pending_open(p.kind)
            return
        end
    end
    if M.last_req == nil or d.request_id ~= M.last_req then return end
    local label = M.pending_label or d.kind
    if d.state == "dispatched" and d.phase == "dispatch" then
        M.result = {text = label .. ": waiting for server...", ok = nil}
    elseif d.ok == true then
        M.result = {text = label .. ": ok", ok = true}
        M.last_req = nil
    else
        M.result = {text = label .. ": " .. (d.reason or d.code or d.state), ok = false}
        M.last_req = nil
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
