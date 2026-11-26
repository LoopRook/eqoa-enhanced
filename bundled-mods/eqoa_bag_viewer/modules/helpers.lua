-- Bag viewer helpers: sell junk, restock and bank marked items near a merchant or a banker, by a button or (a setting
-- per helper, default off) once when the player walks up. Built on the hidden sessions of bags.lua (scope "own": a
-- merchant or banker the player talks to still opens the game's window exactly as before).
-- SPDX-License-Identifier: AGPL-3.0-or-later
local B = require("bags")

local H = {
    -- "keep N" marks (item id -> N) and "bank this" marks (item id -> true): per item type, shared by characters like the
    -- junk marks. `auto_by_char[name][helper]` is the "do it automatically when I get near" choice of one character.
    keep = {}, bank = {}, auto_by_char = {}, loaded = false,
    armed = {},       -- helper -> false once it has acted for this approach; nil/true when it may act
    log = {}, said = {}, -- lines said (for the tests); one line per reason
    chat = {},        -- what was said to the player (for the tests)
    polled = nil, polls = 0,
}
B.helpers = H

-- The actions its batches run, one step each (`buy` for Restock, `bank_move` for Bank marked): each is declared in mod.json.
H.STEP_ACTIONS = {"eqoa.actions.batch", "eqoa.actions.buy", "eqoa.actions.bank_move"}

local FILE = "helpers.cfg"
-- the three helpers: the key, the label the player sees, the kind of NPC it works with
H.HELPERS = {
    {key = "sell", label = "Sell junk", npc = "merchant"},
    {key = "restock", label = "Restock", npc = "merchant"},
    {key = "bank", label = "Bank", npc = "bank"},
}
local NPC_OF = {sell = "merchant", restock = "merchant", bank = "bank"}
local MAX_STEPS = 64

H.AUTO_POLL_SECS = 0.5
-- a helper acts once for each approach: it arms again when the NPC is out of reach by this much more (so standing at the
-- edge of the reach does not act again and again)
H.REARM_EXTRA = 5.0
H.KEEP_STEP = 5

-- ---------------------------------------------------------------- what the player is told

-- Said in chat when the client can say a line there (`eqoa.chat.local`, which `eqoa/1` does not have yet); otherwise it
-- is the Bag Viewer's result line and the game's log. Never a line sent to the server: that would be said to the world.
function H.say(text, ok, keep_result)
    H.chat[#H.chat + 1] = text
    if #H.chat > 40 then table.remove(H.chat, 1) end
    local sent = false
    -- feature-tested, never probed: reading a name the client does not have is a diagnostic in Mods & Addons
    if eqoa.capabilities and eqoa.capabilities.has("eqoa.chat.local") then
        sent = pcall(eqoa.chat["local"], text, {channel = "system"}) == true
    end
    if not keep_result then B.result = {text = text, ok = ok == nil and true or ok} end
    if eqoa.log and eqoa.log.write then pcall(eqoa.log.write, "info", "bag helpers: " .. text) end
    B.dirty()
    return sent
end

-- A line for the game's log only (an automatic helper that found nothing to do, or waited), once per reason.
function H.note(key, text)
    if H.said[key] then return end
    H.said[key] = true
    H.log[#H.log + 1] = text
    if #H.log > 80 then table.remove(H.log, 1) end
    if eqoa.log and eqoa.log.write then pcall(eqoa.log.write, "info", "bag helpers: " .. text) end
end
function H.note_forget(key) H.said[key] = nil end
function H.log_line(text)
    H.log[#H.log + 1] = text
    if #H.log > 80 then table.remove(H.log, 1) end
    if eqoa.log and eqoa.log.write then pcall(eqoa.log.write, "info", "bag helpers: " .. text) end
end

local function plural(n, one, many) return n == 1 and one or many end

-- ---------------------------------------------------------------- marks and settings

local function mark_key(it) return tostring(it.id) end

local function helper_valid(key)
    for _, h in ipairs(H.HELPERS) do if h.key == key then return true end end
    return false
end

function H.load()
    H.keep, H.bank, H.auto_by_char = {}, {}, {}
    if eqoa and eqoa.files and eqoa.files.read then
        local data = eqoa.files.read(FILE)
        if type(data) == "string" then
            for line in data:gmatch("[^\r\n]+") do
                local id, n = line:match("^keep:(%d+)=(%d+)$")
                local bid = line:match("^bank:(%d+)$")
                local who, helper, flag = line:match("^auto:(.+):(%a+)=([01])$")
                if id then
                    n = tonumber(n)
                    if n and n > 0 then H.keep[id] = n end
                elseif bid then
                    H.bank[bid] = true
                elseif who and helper_valid(helper) then
                    H.auto_by_char[who] = H.auto_by_char[who] or {}
                    H.auto_by_char[who][helper] = flag == "1"
                end
                -- anything else is ignored (a mark written by a newer version)
            end
        end
    end
    -- an item saved with both marks keeps the bank mark and loses junk (nothing gets sold)
    local dropped = false
    for id in pairs(H.bank) do
        if B.junk_ids[id] then B.junk_ids[id] = nil; dropped = true end
    end
    if dropped then B.save_junk() end
    H.loaded = true
end

function H.save()
    if not (eqoa and eqoa.files and eqoa.files.write) then return end
    local lines = {}
    local ids = {}
    for id in pairs(H.keep) do ids[#ids + 1] = id end
    table.sort(ids)
    for _, id in ipairs(ids) do lines[#lines + 1] = "keep:" .. id .. "=" .. H.keep[id] end
    ids = {}
    for id, v in pairs(H.bank) do if v then ids[#ids + 1] = id end end
    table.sort(ids)
    for _, id in ipairs(ids) do lines[#lines + 1] = "bank:" .. id end
    local who = {}
    for name in pairs(H.auto_by_char) do who[#who + 1] = name end
    table.sort(who)
    for _, name in ipairs(who) do
        for _, h in ipairs(H.HELPERS) do
            local v = H.auto_by_char[name][h.key]
            -- an explicit off is kept ("=0"): a character with a saved choice never falls back to the default
            if v ~= nil then lines[#lines + 1] = "auto:" .. name .. ":" .. h.key .. (v and "=1" or "=0") end
        end
    end
    eqoa.files.write(FILE, table.concat(lines, "\n") .. (#lines > 0 and "\n" or ""))
end

-- How many of the item type are in the pack now (worn ones count: they are carried too, and the restock never buys more
-- than the mark says).
function H.owned(id)
    local inv = eqoa.inventory.get()
    local n = 0
    for _, it in ipairs(inv and inv.items or {}) do
        if tostring(it.id) == tostring(id) then n = n + (it.quantity or 1) end
    end
    return n
end

function H.keep_of(it) return it and H.keep[mark_key(it)] or nil end
function H.is_banked(it) return it ~= nil and H.bank[mark_key(it)] == true end

-- "Keep up to N": the first mark is what the pack holds now (at least 1); the menu steps it by KEEP_STEP.
function H.set_keep(it, n)
    if not it then return end
    if n == nil or n <= 0 then
        H.keep[mark_key(it)] = nil
        B.result = {text = "Stopped keeping " .. it.name, ok = true}
    else
        n = math.min(math.floor(n), 9999)
        H.keep[mark_key(it)] = n
        B.result = {text = "Keep " .. n .. " " .. it.name .. " (Restock buys up to that many)", ok = true}
    end
    H.save()
    B.dirty()
end

function H.start_keep(it)
    H.set_keep(it, math.max(H.owned(it.id), 1))
end
function H.change_keep(it, delta)
    local n = H.keep_of(it)
    if not n then return end
    H.set_keep(it, math.max(1, n + delta))
end

function H.set_bank(it, flag)
    if not it then return end
    if flag then
        H.bank[mark_key(it)] = true
        -- junk (sell) and bank are exclusive: marking one takes the other off
        local was_junk = B.junk_ids[mark_key(it)] == true
        if was_junk then B.junk_ids[mark_key(it)] = nil; B.save_junk() end
        B.result = {text = "Marked " .. it.name .. " to bank" .. (was_junk and " (no longer marked as junk)" or ""), ok = true}
    else
        H.bank[mark_key(it)] = nil
        B.result = {text = "Stopped banking " .. it.name, ok = true}
    end
    H.save()
    B.dirty()
end

-- An item the player takes out of the bank loses its bank mark. The server's own answer to a take (`bank.changed`, cause
-- "bank_took") is what is heard, so every way of taking one out counts: Withdraw 1, Withdraw stack, the multi-select's
-- Take out of bank, the game's own bank window. The mark is the item type's (the item id, as every mark here is), so
-- it goes for every stack of that item, in the bank or the pack: a mark taken off is one a click puts back, and a mark
-- left behind would bank the item again at the next visit. A deposit (`bank_added`) never marks anything.
function H.on_bank_changed(d)
    if not (d and d.cause == "bank_took" and d.before and d.after) then return end
    if not H.loaded then H.load() end
    local function held(container)
        local n = {}
        for _, it in ipairs(container.items or {}) do
            local k = mark_key(it)
            n[k] = (n[k] or 0) + (it.quantity or 1)
        end
        return n
    end
    local before, after, cleared = held(d.before), held(d.after), 0
    for k, n in pairs(before) do
        if (after[k] or 0) < n and H.bank[k] then H.bank[k] = nil; cleared = cleared + 1 end
    end
    if cleared > 0 then H.save(); B.dirty() end
end

local function character()
    if not B.character then B.sync_character() end
    return B.character
end

-- What a character with no saved choice gets: Bank on (an item marked to bank is a clear instruction); Sell junk and
-- Restock sell items and spend tunar, so they stay off until the player turns them on.
H.DEFAULT_AUTO = {sell = false, restock = false, bank = true}

-- Is the helper on for this character? A saved choice (even "off") wins; a character with none gets the default.
function H.is_on(who, helper)
    if who == nil then return false end
    local flags = H.auto_by_char[who]
    if flags == nil then return H.DEFAULT_AUTO[helper] == true end
    return flags[helper] == true
end

function H.auto_on(helper)
    return H.is_on(B.character, helper)
end

-- Does anything need the render poll? A saved "on" for any character, or the default for the character in the world (or
-- one not known yet).
function H.any_auto()
    for _, flags in pairs(H.auto_by_char) do
        for _, v in pairs(flags) do if v == true then return true end end
    end
    local who = B.character
    if who == nil or H.auto_by_char[who] == nil then return H.DEFAULT_AUTO.bank == true end
    return false
end

function H.set_auto(helper, on)
    if not helper_valid(helper) then return false end
    local who = character()
    if not who then
        B.result = {text = "Automatic helpers: waiting for your character", ok = false}
        B.dirty()
        return false
    end
    if H.auto_by_char[who] == nil then
        -- the first saved choice of a character keeps the defaults of the others, now as saved choices
        H.auto_by_char[who] = {}
        for k, v in pairs(H.DEFAULT_AUTO) do H.auto_by_char[who][k] = v end
    end
    H.auto_by_char[who][helper] = on and true or false
    H.armed[helper] = nil -- standing there now counts as an approach
    H.save()
    local label
    for _, h in ipairs(H.HELPERS) do if h.key == helper then label = h.label end end
    B.result = {text = (on and "Automatic " or "No longer automatic: ") .. string.lower(label)
        .. (on and " when you get near" or ""), ok = true}
    if H.on_auto_change then H.on_auto_change() end
    B.dirty()
    return true
end

-- The rows an item's menu gets for the marks (`add` is build_menu's row adder: label, enabled, reason, run, local_only).
function H.menu_rows(it, add)
    local n = H.keep_of(it)
    if n then
        add("Keep up to " .. n .. ": more (+" .. H.KEEP_STEP .. ")", true, nil, function() H.change_keep(it, H.KEEP_STEP) end, true)
        add("Keep up to " .. n .. ": fewer (-" .. H.KEEP_STEP .. ")", n > 1, n <= 1 and "at the least" or nil,
            function() H.change_keep(it, -H.KEEP_STEP) end, true)
        add("Stop keeping", true, nil, function() H.set_keep(it, nil) end, true)
    else
        add("Keep up to " .. math.max(H.owned(it.id), 1) .. " (Restock)", true, nil, function() H.start_keep(it) end, true)
    end
    if H.is_banked(it) then
        add("Unmark bank", true, nil, function() H.set_bank(it, false) end, true)
    else
        add("Bank this", not it.equipped, it.equipped and "cannot bank an equipped item" or nil,
            function() H.set_bank(it, true) end, true)
    end
end

function H.card_lines(it, lines)
    local n = H.keep_of(it)
    if n then lines[#lines + 1] = {"Restock: keep up to " .. n, "muted"} end
    if H.is_banked(it) then lines[#lines + 1] = {"Marked to bank", "muted"} end
end

-- ---------------------------------------------------------------- restock

-- The merchant's wares a "keep N" mark names, against what the pack holds and the money and room there are. Returns
-- {steps = batch steps, buys = {{name, count, cost}} (one per step), short = the kept items still short, limit = why it
-- stopped early ("out of money", "bags are full") or nil, merchant_has = how many of the marks this merchant sells}.
function H.restock_plan(merch, inv)
    merch = merch or eqoa.merchant.get()
    inv = inv or eqoa.inventory.get()
    local plan = {steps = {}, buys = {}, short = {}, limit = nil, sells = 0, total = 0, cost = 0}
    if not merch or not inv then return plan end
    local owned, room = {}, {}
    for _, it in ipairs(inv.items or {}) do
        local k = mark_key(it)
        owned[k] = (owned[k] or 0) + (it.quantity or 1)
        local free = (it.max_stack or 1) - (it.quantity or 1)
        if free > 0 then room[k] = (room[k] or 0) + free end
    end
    local money = inv.money or 0
    local slots = B.bag_free()
    if slots == nil then slots = 0 end
    for _, ware in ipairs(merch.items or {}) do
        local k = mark_key(ware)
        local keep = H.keep[k]
        if keep then
            plan.sells = plan.sells + 1
            local need = keep - (owned[k] or 0)
            if need > 0 then
                local per = math.max(ware.max_stack or 1, 1)
                local unit = eqoa.merchant.price(ware.ref, "buy", 1)
                local n, why = need, nil
                if unit and unit > 0 and math.floor(money / unit) < n then
                    n, why = math.floor(money / unit), "out of money"
                end
                local fit = (room[k] or 0) + slots * per
                if fit < n then n, why = fit, "bags are full" end
                if n > 0 then
                    local cost = eqoa.merchant.price(ware.ref, "buy", n) or ((unit or 0) * n)
                    -- more than a stack's worth is bought stack by stack
                    local left = n
                    while left > 0 and #plan.steps < MAX_STEPS do
                        local chunk = math.min(left, per)
                        plan.steps[#plan.steps + 1] = {kind = "buy", item = ware.ref, count = chunk}
                        plan.buys[#plan.buys + 1] = {name = ware.name, count = chunk,
                            cost = (eqoa.merchant.price(ware.ref, "buy", chunk) or ((unit or 0) * chunk))}
                        left = left - chunk
                    end
                    if left > 0 then
                        n = n - left
                        cost = eqoa.merchant.price(ware.ref, "buy", n) or ((unit or 0) * n)
                        why = why or "that is a lot to buy at once; ask again"
                    end
                    money = money - cost
                    owned[k] = (owned[k] or 0) + n
                    local used = math.min(n, room[k] or 0)
                    room[k] = (room[k] or 0) - used
                    slots = slots - math.ceil((n - used) / per)
                    plan.total, plan.cost = plan.total + n, plan.cost + cost
                end
                if n < need then
                    plan.short[#plan.short + 1] = ware.name
                    plan.limit = plan.limit or why or "could not buy it"
                end
            end
        end
    end
    return plan
end

-- How many kept items are below their mark (a merchant need not be open).
function H.short_count()
    local n = 0
    for id, keep in pairs(H.keep) do
        if H.owned(id) < keep then n = n + 1 end
    end
    return n
end

local plan_cache = {key = nil, plan = nil}
-- the plan for the open merchant, kept while neither the wares nor the pack changed
function H.restock_plan_cached()
    local merch, inv = eqoa.merchant.get(), eqoa.inventory.get()
    if not merch or not inv then return nil end
    local key = tostring(merch.revision) .. ":" .. tostring(inv.revision)
    if plan_cache.key ~= key or not plan_cache.plan then
        plan_cache.key, plan_cache.plan = key, H.restock_plan(merch, inv)
    end
    return plan_cache.plan
end

function H.start_restock()
    if B.batch_id then return false end -- before any talk: a hidden session nothing would close
    local merch = eqoa.merchant.get()
    if not merch then
        if B.nearby_npc("merchant") ~= nil then
            return B.open_talk_npc("merchant", "Restock", H.start_restock) ~= nil
        end
        H.say("Restock: no merchant is near", false)
        return false
    end
    if B.batch_id then return false end
    local plan = H.restock_plan(merch, eqoa.inventory.get())
    if #plan.steps == 0 then
        B.release_takeover("merchant")
        if plan.limit then
            H.say("Restock: " .. plan.limit .. "; nothing bought", false)
        elseif plan.sells == 0 then
            H.say("Restock: this merchant sells none of the items you keep", false)
        else
            H.say("Restock: you already hold what you keep", true)
        end
        return false
    end
    local bid, err = eqoa.actions.batch({steps = plan.steps, require_window = "merchant"})
    if not bid then
        B.release_takeover("merchant")
        H.say("Restock: " .. (err and (err.message or err.code) or "refused"), false)
        return false
    end
    B.batch_id, B.batch_kind, B.batch_verb, B.batch_total = bid, "restock", "Bought", #plan.steps
    H.batch_plan = plan
    B.result = {text = string.format("Restocking 0 of %d...", #plan.steps), ok = nil}
    B.dirty()
    return true
end

-- ---------------------------------------------------------------- bank

-- The pack's stacks of items marked "bank this", with the bank's room taken into account. Worn items never go.
-- Returns {steps, names, count, full = how many did not fit, limit}.
function H.bank_plan(inv, bank)
    inv = inv or eqoa.inventory.get()
    bank = bank or eqoa.bank.get()
    local plan = {steps = {}, names = {}, count = 0, full = 0}
    if not inv then return plan end
    local room = {}
    for _, it in ipairs(bank and bank.items or {}) do
        local free = (it.max_stack or 1) - (it.quantity or 1)
        if free > 0 then room[mark_key(it)] = (room[mark_key(it)] or 0) + free end
    end
    local slots = bank and bank.remaining
    if slots == nil then slots = bank and bank.capacity and math.max(bank.capacity - (bank.used or 0), 0) or 40 end
    for _, it in ipairs(inv.items or {}) do
        if H.is_banked(it) and not it.equipped and #plan.steps < MAX_STEPS then
            local k, qty = mark_key(it), it.quantity or 1
            local ok = false
            if (room[k] or 0) >= qty then
                room[k] = room[k] - qty -- joins a stack the bank has room in
                ok = true
            elseif slots > 0 then
                slots = slots - 1 -- takes a free bank slot
                ok = true
            end
            if ok then
                plan.steps[#plan.steps + 1] = {kind = "bank_move", item = it.ref, count = qty, from = "inventory"}
                plan.names[#plan.names + 1] = it.name
                plan.count = plan.count + 1
            else
                plan.full = plan.full + 1
            end
        end
    end
    return plan
end

-- How many pack stacks are marked "bank this" (the bank's room is not looked at).
function H.bank_count()
    local inv = eqoa.inventory.get()
    local n = 0
    for _, it in ipairs(inv and inv.items or {}) do
        if H.is_banked(it) and not it.equipped then n = n + 1 end
    end
    return n
end

function H.start_bank()
    if B.batch_id then return false end -- before any talk: a hidden session nothing would close
    local bank = eqoa.bank.get()
    if not (bank and bank.open) then
        if B.nearby_npc("bank") ~= nil then
            return B.open_talk_npc("bank", "Bank marked", H.start_bank) ~= nil
        end
        H.say("Bank marked: no banker is near", false)
        return false
    end
    if B.batch_id then return false end
    local plan = H.bank_plan(eqoa.inventory.get(), bank)
    if #plan.steps == 0 then
        if not (B.tab == 6) then B.release_takeover("bank") end
        if plan.full > 0 then
            H.say("Bank marked: the bank is full; nothing banked", false)
        else
            H.say("Bank marked: nothing marked to bank is in your bags", false)
        end
        return false
    end
    local bid, err = eqoa.actions.batch({steps = plan.steps, require_window = "bank"})
    if not bid then
        if not (B.tab == 6) then B.release_takeover("bank") end
        H.say("Bank marked: " .. (err and (err.message or err.code) or "refused"), false)
        return false
    end
    B.batch_id, B.batch_kind, B.batch_verb, B.batch_total = bid, "bank", "Banked", #plan.steps
    H.batch_plan = plan
    B.result = {text = string.format("Banking 0 of %d...", #plan.steps), ok = nil}
    B.dirty()
    return true
end

-- ---------------------------------------------------------------- the end of a batch: the line the player reads

-- The lines a batch ends with go to the log (and chat when the client has it); the window's result line gets them all,
-- joined into one short line, in the not-ok colour if any of them is a problem.
local final_lines
local function batch_say(text, ok)
    if final_lines then final_lines[#final_lines + 1] = {text = text, ok = ok ~= false} end
    return H.say(text, ok, true)
end
local function result_from(lines)
    if #lines == 0 then return end
    local parts, ok = {}, true
    for _, l in ipairs(lines) do
        parts[#parts + 1] = l.text
        if not l.ok then ok = false end
    end
    B.result = {text = table.concat(parts, "; "), ok = ok}
    B.dirty()
end

local function junk_line(done, d, quotes, skipped)
    local gained = 0
    for i = 1, done do gained = gained + (quotes[i] or 0) end
    local what = plural(done, "item", "items")
    if done > 0 then
        batch_say(string.format("Sold %d junk %s for %s tunar", done, what, B.tunar(gained)), d.ok == true)
    elseif d.ok ~= true then
        batch_say("Sell junk: nothing sold (" .. tostring(d.reason or d.code or d.state or "stopped") .. ")", false)
    end
    if done > 0 and d.ok ~= true then
        batch_say(string.format("Sell junk stopped after %d of %d: %s", done, d.total or done,
            tostring(d.reason or d.code or d.state or "stopped")), false)
    end
end

-- Called by bags.lua when a helper's batch ends (`kind` is "sell", "restock" or "bank"; `d` the batch result). The Bag
-- Viewer's own result line is set here too, so it says the same as chat.
function H.batch_done(kind, d)
    final_lines = {}
    local ok_, err_ = pcall(H.batch_done_lines, kind, d)
    local lines = final_lines
    final_lines = nil
    result_from(lines)
    if not ok_ then error(err_, 0) end
end
function H.batch_done_lines(kind, d)
    local done = d.completed or 0
    if kind == "sell" then
        junk_line(done, d, B.batch_quotes or {}, B.batch_skipped)
        B.batch_quotes = nil
        local skipped = B.batch_skipped
        B.batch_skipped = nil
        if skipped and #skipped > 0 then batch_say(H.skipped_line(skipped), true) end
    elseif kind == "restock" then
        local plan = H.batch_plan or {buys = {}}
        local items, cost = 0, 0
        for i = 1, done do
            local b = plan.buys[i]
            if b then items, cost = items + b.count, cost + b.cost end
        end
        if done > 0 then
            batch_say(string.format("Restocked: bought %d %s for %s tunar", items, plural(items, "item", "items"), B.tunar(cost)),
                d.ok == true)
        end
        if d.ok ~= true then
            batch_say(string.format("Restock stopped after %d of %d: %s", done, d.total or #plan.buys,
                tostring(d.reason or d.code or d.state or "stopped")), false)
        elseif plan.limit then
            batch_say(string.format("Restock stopped: %s (still short: %s)", plan.limit, table.concat(plan.short, ", ")), false)
        end
        H.batch_plan = nil
    elseif kind == "bank" then
        local plan = H.batch_plan or {names = {}, full = 0}
        if done > 0 then
            batch_say(string.format("Banked %d %s", done, plural(done, "item", "items")), d.ok == true)
        end
        if d.ok ~= true then
            batch_say(string.format("Banking stopped after %d of %d: %s", done, d.total or #plan.names,
                tostring(d.reason or d.code or d.state or "stopped")), false)
        elseif plan.full > 0 then
            batch_say(string.format("The bank is full: %d %s left in your bags", plan.full, plural(plan.full, "item", "items")), false)
        end
        H.batch_plan = nil
    end
end
B.on_helper_batch_done = H.batch_done

-- "Kept 1 junk item: you are wearing it": marked junk the one click does not sell (only what is worn right now)
function H.skipped_line(skipped)
    return string.format("Kept %d junk %s: you are wearing %s", #skipped, plural(#skipped, "item", "items"),
        plural(#skipped, "it", "them"))
end

-- ---------------------------------------------------------------- automatic helpers

-- May a helper start now? Never while a game window or any session (the player's or the mod's own) is up, a batch or a
-- single action is waiting, or the player is in a fight.
function H.can_start()
    if B.batch_id or B.pending_open or B.last_req then return false, "busy" end
    if eqoa.merchant.get() ~= nil or eqoa.loot.get() ~= nil then return false, "a merchant or corpse window is up" end
    local bank = eqoa.bank.get()
    if bank and bank.open then return false, "the bank is up" end
    local w = eqoa.windows and eqoa.windows.get and eqoa.windows.get()
    if w and (w.any_window_open == true or w.start_menu_open == true) then return false, "a game window is up" end
    if B.auto_take or B.corpse_tab_kept then return false, "Auto-loot is taking from a corpse" end
    local pl = eqoa.player and eqoa.player.get and eqoa.player.get()
    if not pl then return false, "not in the world" end
    local fighting, why = B.in_combat(pl)
    if fighting then return false, "in combat (" .. tostring(why) .. ")" end
    return true
end

-- What each helper has to do now (the count), without a merchant or banker open.
local function pending_count(helper)
    if helper == "sell" then
        local _, n = B.junk_sell_list()
        return n
    elseif helper == "restock" then
        return H.short_count()
    end
    return H.bank_count()
end

local RUN = {sell = function() return B.start_sell_junk() end, restock = function() return H.start_restock() end,
    bank = function() return H.start_bank() end}

-- The poll (from the render event while any automatic helper is on, every AUTO_POLL_SECS): a helper acts once when the
-- player is within reach of the NPC, and again only after the player has been out of reach (and back).
function H.auto_poll(force)
    local now = os.clock()
    if not force and H.polled and now - H.polled < H.AUTO_POLL_SECS and now >= H.polled then return end
    H.polled = now
    H.polls = H.polls + 1
    if not B.character then B.sync_character() end
    local who = B.character
    if not who then return end
    for _, h in ipairs(H.HELPERS) do
        local key, kind = h.key, NPC_OF[h.key]
        if H.is_on(who, key) then
            local dist = B.npc_distance(kind)
            local reach = nil
            if dist ~= nil then reach = B.within_reach(kind, dist) end
            if dist == nil then
                H.armed[key] = nil
                H.note_forget(key .. ":nothing")
            elseif not reach then
                -- out of reach: armed again once clearly away
                local r = B.range[kind] or {}
                local limit = math.max(r.ok or 0, B.DEFAULT_REACH) + H.REARM_EXTRA
                if dist > limit then
                    H.armed[key] = nil
                    H.note_forget(key .. ":nothing")
                end
            elseif H.armed[key] == nil then
                local ok, why = H.can_start()
                if ok then
                    if pending_count(key) > 0 then
                        H.armed[key] = false
                        H.note_forget(key .. ":wait")
                        H.note_forget(key .. ":nothing")
                        H.log_line("automatic " .. string.lower(h.label) .. ": you are near, starting")
                        RUN[key]()
                    else
                        H.armed[key] = false
                        H.note(key .. ":nothing", "automatic " .. string.lower(h.label) .. ": nothing to do here")
                    end
                else
                    H.note(key .. ":wait", "automatic " .. string.lower(h.label) .. ": waiting (" .. tostring(why) .. ")")
                end
            end
        else
            H.armed[key] = nil
        end
    end
end

-- The Bank tab keeps itself current: with a banker in reach and the tab shown, the mod talks to the banker through its
-- hidden session (the one the tab then holds), so the contents are what the server has now. It asks once on showing the
-- tab, once when a banker comes into reach, and again when the session it held has ended (after the mod's own bank
-- actions that let go of it); never every frame, and never into a busy moment. Out of reach it asks nothing (the tab's
-- status line says "too far from the banker"). Called each time the window is drawn.
function H.bank_tab_tick()
    if B.tab ~= 6 then
        H.bank_refresh_wanted, H.bank_near_seen, H.bank_was_open = true, nil, nil
        return
    end
    local bank = eqoa.bank.get()
    local open = bank ~= nil and bank.open == true
    if open then
        H.bank_was_open = true
        return
    end
    if H.bank_was_open then H.bank_was_open, H.bank_refresh_wanted = nil, true end -- the session ended: ask again
    if B.nearby_npc("bank", B.DRAW_TTL) == nil then
        H.bank_near_seen = false
        return
    end
    if H.bank_near_seen ~= true then H.bank_near_seen, H.bank_refresh_wanted = true, true end
    if not H.bank_refresh_wanted then return end
    if B.pending_open or B.batch_id or B.last_req or (B.multi and B.multi.busy and B.multi.busy()) then return end
    if eqoa.merchant.get() ~= nil or eqoa.loot.get() ~= nil then return end
    if B.auction_list and B.auction_list.busy and B.auction_list.busy() then return end
    if B.too_far("bank", B.DRAW_TTL) then return end
    H.bank_refresh_wanted = false
    H.bank_refreshes = (H.bank_refreshes or 0) + 1
    B.open_talk_npc("bank", "Bank", function()
        B.result = {text = "Bank refreshed", ok = true}
        B.dirty()
    end)
end

-- A new world or login: nothing has been approached yet
function H.reset()
    H.armed = {}
    H.said = {}
    H.batch_plan = nil
    plan_cache.key, plan_cache.plan = nil, nil
end

return H
