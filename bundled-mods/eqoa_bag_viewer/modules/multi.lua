-- Bag viewer multi-select: Ctrl+click and Shift+click pick several slots, one menu acts on all of them.
-- Marks (junk, bank) are instant. Sell, Put in bank and Take out of bank go item by item through the same batch the helpers
-- use (the game's own pace); each item is found again by its server key before its step, so a slot that moved or emptied
-- while the run goes is skipped and counted, never acted on by position. Destroy asks the player once for the whole list
-- (`eqoa.actions.destroy_many`: the client's own question, answered only by the player's own Yes).
-- SPDX-License-Identifier: AGPL-3.0-or-later
local B = require("bags")

local X = {
    sel = {},       -- key -> {key, list, id, name}
    n = 0,          -- how many are selected
    anchor = nil,   -- key of the slot clicked last (where Shift+click starts)
    run = nil,      -- the Sell / Put in bank / Take out of bank in progress
    destroy_note = nil,
}
B.multi = X

local MAX_STEPS = 60
-- a batch that ends with one of these is not an item's fault: the rest is not tried
local STOP_CODES = {window_closed = true, cancelled = true, rate_limited = true, permission_denied = true,
    busy = true, unsupported = true}
local KINDS = {
    sell = {npc = "merchant", label = "Sell", verb = "Sold", doing = "Selling"},
    deposit = {npc = "bank", label = "Put in bank", verb = "Banked", doing = "Banking"},
    withdraw = {npc = "bank", label = "Take out of bank", verb = "Took out", doing = "Taking out"},
}

local function key_of(it) return tostring(it.list) .. ":" .. tostring(it.inventory_key) end

local function container(list)
    if list == "bank" then return eqoa.bank.get() end
    return eqoa.inventory.get()
end

-- The item as it is now (by the server's key for the stack), or nil when it is gone.
local function find(list, key)
    local c = container(list)
    for _, it in ipairs(c and c.items or {}) do
        if key_of(it) == key then return it end
    end
    return nil
end

local function plural(n, one, many) return n == 1 and one or many end

local function say_log(text)
    if eqoa.log and eqoa.log.write then pcall(eqoa.log.write, "info", "bag multi-select: " .. text) end
end

-- ---------------------------------------------------------------- the selection

function X.is_selected(it) return it ~= nil and X.sel[key_of(it)] ~= nil end

function X.drop_selection() X.sel, X.n = {}, 0 end

-- Closing the window, zoning, changing tab, the end of an action: nothing stays selected.
function X.clear()
    X.sel, X.n, X.anchor, X.destroy_note = {}, 0, nil, nil
end

local function add(it)
    local k = key_of(it)
    if not X.sel[k] then
        X.sel[k] = {key = k, list = it.list, id = it.id, name = it.name, qty = it.quantity or 1}
        X.n = X.n + 1
    end
end

local function remove(k)
    if X.sel[k] then X.sel[k] = nil; X.n = X.n - 1 end
end

local function index_in(grid, key)
    for i, e in ipairs(grid and grid.items or {}) do
        if key_of(e.item) == key then return i end
    end
    return nil
end

-- A left click on a slot. Returns true when the click was a selection click (no menu then); false for a plain click, which
-- clears the selection and goes on to the item's own menu.
function X.click(h, d)
    local it = h.item
    if it.list ~= "inventory" and it.list ~= "bank" then X.clear(); return false end
    local k = key_of(it)
    if d.shift then
        local grid = B.grid
        local a, b = index_in(grid, X.anchor), index_in(grid, k)
        if a and b then
            if not d.ctrl then X.drop_selection() end
            for i = math.min(a, b), math.max(a, b) do add(grid.items[i].item) end
        else
            add(it)
            X.anchor = k
        end
        return true
    elseif d.ctrl then
        if X.sel[k] then remove(k) else add(it) end
        X.anchor = k
        return true
    end
    X.drop_selection()
    X.anchor = k
    return false
end

-- The selected items as they are now, in the container's own order, and how many of the selection are gone.
function X.current()
    local out = {}
    local list = B.grid and B.grid.bank and "bank" or "inventory"
    local c = container(list)
    for _, it in ipairs(c and c.items or {}) do
        if X.sel[key_of(it)] then out[#out + 1] = it end
    end
    return out, X.n - #out
end

-- Rebuilding the grid (any change): a selected stack that is no longer in the view leaves the selection.
function X.prune(items)
    if X.n == 0 then return end
    local here = {}
    for _, e in ipairs(items) do here[key_of(e.item)] = true end
    for k in pairs(X.sel) do
        if not here[k] then remove(k) end
    end
    if X.anchor and not here[X.anchor] then X.anchor = nil end
end

function X.hint()
    if X.n == 0 then return nil end
    return string.format("%d %s selected: right-click one for actions", X.n, plural(X.n, "item", "items"))
end

-- ---------------------------------------------------------------- the menu

local function names_list(items)
    local t = {}
    for _, it in ipairs(items) do t[#t + 1] = it.name end
    return t
end

local function worn_line(worn)
    if #worn == 0 then return "" end
    return string.format("; %d worn %s kept", #worn, plural(#worn, "item", "items"))
end

-- A Yes / No page in place of the menu: only the player's click on Yes goes on.
local function ask_page(title, yes_text, no_text, run)
    local menu = B.menu
    local yes = {label = "Yes", text = yes_text, enabled = true, group = true, confirmed = true, run = run}
    local no = {label = "No", text = no_text, enabled = true, group = true, confirmed = true, run = function() end}
    B.menu = {title = title, rows = {yes, no}, fx = menu and menu.fx or 8, fy = menu and menu.fy or 46, asking = true}
end

local function mark_junk(flag)
    local items = X.current()
    local n, worn = 0, {}
    for _, it in ipairs(items) do
        if flag and it.equipped then
            worn[#worn + 1] = it
        elseif flag and not B.is_junk(it) then
            B.junk_ids[tostring(it.id)] = true
            n = n + 1
        elseif not flag and B.is_junk(it) then
            B.junk_ids[tostring(it.id)] = nil
            n = n + 1
        end
    end
    B.save_junk()
    B.result = {text = string.format("%s %d %s%s%s", flag and "Marked" or "Unmarked", n, plural(n, "item", "items"),
        flag and " as junk" or "", worn_line(worn)), ok = true}
    X.clear()
    B.dirty()
end

local function mark_bank(flag)
    local H = B.helpers
    if not H then return end
    local n, worn = 0, {}
    for _, it in ipairs((X.current())) do
        -- a worn item cannot be marked, but a mark it already has (made before it was put on) can always be taken off
        if flag and it.equipped then
            worn[#worn + 1] = it
        elseif flag and not H.is_banked(it) then
            H.bank[tostring(it.id)] = true
            n = n + 1
        elseif not flag and H.is_banked(it) then
            H.bank[tostring(it.id)] = nil
            n = n + 1
        end
    end
    H.save()
    B.result = {text = flag and string.format("Marked %d %s to bank%s", n, plural(n, "item", "items"), worn_line(worn))
        or string.format("Unmarked %d %s for the bank", n, plural(n, "item", "items")), ok = true}
    X.clear()
    B.dirty()
end

local function split_worn(items)
    local ok, worn = {}, {}
    for _, it in ipairs(items) do
        if it.equipped then worn[#worn + 1] = it else ok[#ok + 1] = it end
    end
    return ok, worn
end

-- Destroy: one question for the whole list. The game's own "are you sure?" is the client's (it names every item and starts
-- on No); nothing is destroyed without the player's Yes, and a worn item is not on the list at all.
local function destroy(items, worn)
    if X.busy() then
        B.result = {text = "Destroy: another action is still running", ok = false}
        B.dirty()
        return
    end
    local refs, keys = {}, {}
    for _, it in ipairs(items) do refs[#refs + 1] = it.ref; keys[#keys + 1] = key_of(it) end
    local req, err = eqoa.actions.destroy_many(refs)
    if not req then
        B.result = {text = "Destroy: " .. tostring(err and (err.message or err.code) or "refused"), ok = false}
        B.dirty()
        return
    end
    X.destroy_note = {total = #items, worn = worn, keys = keys}
    B.ask_req = req
    B.result = {text = "Destroy: answer the question on the screen", ok = nil}
    B.dirty()
end

-- How many of the stacks that were asked about are gone from the pack now (the server's answers, not the requests sent);
-- nil when the pack cannot be read.
local function gone_count(keys)
    local inv = eqoa.inventory.get()
    if not inv then return nil end
    local have = {}
    for _, it in ipairs(inv.items) do have[key_of(it)] = true end
    local n = 0
    for _, k in ipairs(keys or {}) do
        if not have[k] then n = n + 1 end
    end
    return n
end

-- The answers that end the world the stacks were in: the pack cannot say what the server did with them.
local WORLD_ENDS = {zoning = true, death = true, left_world = true}

function X.on_destroy_closed(d)
    local note = X.destroy_note or {total = 0, worn = {}, keys = {}}
    X.destroy_note = nil
    local det = d.detail or {}
    local sent = det.sent_count or 0
    local total = det.total or note.total
    local text, ok
    if d.answer == "no" then
        text, ok = "Destroy: kept", true
    elseif d.answer ~= "yes" and sent == 0 then
        text, ok = "Destroy: kept (" .. tostring(d.answer) .. ")", true
    else
        -- the Yes was given and the run went (all the way, or until `stopped`)
        local stopped = d.answer ~= "yes" and tostring(d.answer) or det.reason
        local done
        if not (WORLD_ENDS[d.answer] or det.reason == "not in the world") then done = gone_count(note.keys) end
        local head, parts = nil, {}
        if done == nil then
            head = string.format("Asked to destroy %d of %d", sent, total)
        else
            head = string.format("Destroyed %d of %d", done, total)
            if done < sent then parts[#parts + 1] = string.format("%d not confirmed by the server yet", sent - done) end
        end
        local skipped = det.skipped or {}
        if #skipped > 0 then
            parts[#parts + 1] = tostring(skipped[1]) .. " had moved" .. (#skipped > 1 and (" and " .. (#skipped - 1) .. " more") or "")
        end
        local untried = total - sent - #skipped
        if stopped and untried > 0 then parts[#parts + 1] = string.format("%d not tried", untried) end
        if stopped then parts[#parts + 1] = "stopped: " .. stopped end
        text = head .. (#parts > 0 and (": " .. table.concat(parts, "; ")) or "") .. worn_line(note.worn)
        ok = #parts == 0 and (done or sent) == total
    end
    B.result = {text = text, ok = ok}
    say_log(text)
    X.clear()
    B.dirty()
end

local function ask_text_sell(items)
    local risky = 0
    for _, it in ipairs(items) do if B.sell_risk(it) then risky = risky + 1 end end
    return risky
end

-- The group menu for the selected items (right-click on one of them). `fx`, `fy` is where it opens.
function X.build_menu(fx, fy)
    local items = X.current()
    local n = #items
    if n == 0 then X.clear(); return nil end
    local rows = {}
    local function row(label, enabled, reason, run, opts)
        opts = opts or {}
        rows[#rows + 1] = {label = label, enabled = enabled, reason = reason, run = run, group = true,
            far_kind = opts.far_kind, ask = opts.ask, ask_title = opts.ask_title, ask_yes = opts.ask_yes, ask_q = opts.ask_q}
    end
    local on_bank = items[1].list == "bank"
    local usable, worn = split_worn(items)
    local bank = eqoa.bank.get()
    local bank_open = bank and bank.open
    if on_bank then
        local near = B.nearby_npc("bank") ~= nil
        local far = not bank_open and near and B.too_far("bank") or nil
        local deny = (not bank_open and not near) and "no banker nearby" or far
        row(string.format("Take out of bank (%d)", n), deny == nil, deny,
            function() X.start("withdraw", items, {}) end, {far_kind = far and "bank" or nil})
        if B.auction_list then
            -- the bank is opened as Take out of bank opens it; one form for each item in turn
            row(string.format("List on auction (%d)", n), deny == nil, deny,
                function() B.auction_list.start_group(items) end, {far_kind = far and "bank" or nil})
        end
    else
        local H = B.helpers
        local to_mark, marked, risky_mark = 0, 0, 0
        for _, it in ipairs(usable) do
            if B.is_junk(it) then marked = marked + 1 else
                to_mark = to_mark + 1
                if B.sell_risk(it) then risky_mark = risky_mark + 1 end
            end
        end
        local mark_ask
        if risky_mark > 0 then
            mark_ask = string.format("%d %s gear, a potion, quick-slotted or no-trade", risky_mark, risky_mark == 1 and "is" or "are")
        end
        row("Mark as junk", to_mark > 0, #usable == 0 and "all selected items are worn" or "all are already junk",
            function() mark_junk(true) end, {ask = mark_ask, ask_title = "Mark these items as junk?",
                ask_yes = "Yes, mark them as junk", ask_q = "mark them all as junk?"})
        row("Unmark", marked > 0, "none is marked as junk", function() mark_junk(false) end)
        if H then
            local to_bank = 0
            for _, it in ipairs(usable) do if not H.is_banked(it) then to_bank = to_bank + 1 end end
            row("Mark to bank", to_bank > 0, #usable == 0 and "all selected items are worn" or "all are already marked",
                function() mark_bank(true) end)
            -- the mark can be taken off again: offered when any selected item (worn or not) carries it
            local banked = 0
            for _, it in ipairs(items) do if H.is_banked(it) then banked = banked + 1 end end
            if banked > 0 then
                row(string.format("Unmark bank (%d)", banked), true, nil, function() mark_bank(false) end)
            end
        end
        -- Sell: through the open merchant, or by talking to the nearest one first (as Sell junk does)
        local merch_open = eqoa.merchant.get() ~= nil
        local near_merch = B.nearby_npc("merchant") ~= nil
        local sell_far = not merch_open and near_merch and B.too_far("merchant") or nil
        local sell_deny = (#usable == 0 and "all selected items are worn")
            or ((not merch_open and not near_merch) and "no merchant nearby") or sell_far or nil
        local risky = ask_text_sell(usable)
        local sell_ask
        if risky > 0 then
            sell_ask = string.format("%d %s gear, a potion, quick-slotted or no-trade", risky, risky == 1 and "is" or "are")
        end
        row(string.format("Sell (%d)", #usable), sell_deny == nil, sell_deny,
            function() X.start("sell", usable, worn) end,
            {far_kind = (#usable > 0 and sell_far) and "merchant" or nil, ask = sell_ask,
                ask_title = "Sell these items?", ask_yes = "Yes, sell them", ask_q = "sell them all?"})
        local near_bank = B.nearby_npc("bank") ~= nil
        local dep_far = not bank_open and near_bank and B.too_far("bank") or nil
        local dep_deny = (#usable == 0 and "all selected items are worn")
            or ((not bank_open and not near_bank) and "no banker nearby") or dep_far or nil
        row(string.format("Put in bank (%d)", #usable), dep_deny == nil, dep_deny,
            function() X.start("deposit", usable, worn) end,
            {far_kind = (#usable > 0 and dep_far) and "bank" or nil})
        -- not while a merchant window is open (what the server answers there is unproven) or another group action is going
        local destroy_deny = (#usable == 0 and "all selected items are worn")
            or (merch_open and "close the merchant first")
            or (X.busy() and "another action is still running") or nil
        row("Destroy...", destroy_deny == nil, destroy_deny, function() destroy(usable, worn) end)
    end
    B.menu = {title = string.format("%d %s selected", n, plural(n, "item", "items")), rows = rows, fx = fx or 8, fy = fy or 46,
        group = true}
    return B.menu
end

function X.open_menu(h)
    if X.is_selected(h.item) then X.build_menu(h.fx, h.fy) end
    return B.menu ~= nil and B.menu.group == true
end

-- A group menu row was chosen (bags.lua `trigger_menu` hands every `group` row here).
function X.trigger(row)
    if not row.confirmed and row.ask and (row.enabled or row.far_kind) then
        if not row.enabled and row.far_kind and B.far_gate(row.far_kind, row.label) then return end
        ask_page(row.ask_title or (row.label .. "?"), row.ask_yes or "Yes", "No, leave them", row.run)
        -- why it asks, in the result line (a menu row is one short line)
        B.result = {text = row.ask .. ": " .. (row.ask_q or "go on?"), ok = nil}
        return
    end
    if not row.enabled then
        if row.far_kind and row.run then
            if B.far_gate(row.far_kind, row.label) then return end
            row.run()
            return
        end
        B.result = {text = row.label .. ": " .. (row.reason or "unavailable"), ok = false}
        return
    end
    if row.run then row.run() end
end

-- ---------------------------------------------------------------- the run (Sell, Put in bank, Take out of bank)

local function step_for(kind, it, entry)
    local qty = math.min(entry and entry.qty or math.huge, it.quantity or 1)
    if kind == "sell" then return {kind = "sell", item = it.ref, count = qty} end
    return {kind = "bank_move", item = it.ref, count = qty, from = kind == "deposit" and "inventory" or "bank"}
end

local function release_for(run)
    local npc = KINDS[run.kind].npc
    if npc == "bank" and B.tab == 6 then return end
    B.release_takeover(npc)
end

-- The run is over (finished, or stopped for `why`): one line says what happened.
function X.finish(why)
    local run = X.run
    if not run then return end
    X.run = nil
    local k = KINDS[run.kind]
    local text = string.format("%s %d of %d", k.verb, run.done, run.total)
    local parts = {}
    if #run.failed > 0 then
        local first = run.failed[1]
        parts[#parts + 1] = first.name .. " was refused" .. (#run.failed > 1 and (" and " .. (#run.failed - 1) .. " more") or "")
        -- the server's or the client's words for each refusal go to the log
        for _, f in ipairs(run.failed) do say_log(k.label .. ": " .. f.name .. " was refused (" .. tostring(f.reason or "no reason given") .. ")") end
    end
    if #run.skipped > 0 then
        parts[#parts + 1] = string.format("%d moved or gone", #run.skipped)
    end
    local untried = run.total - run.done - #run.failed - #run.skipped
    if why and untried > 0 then parts[#parts + 1] = tostring(why) end
    if #parts > 0 then text = text .. ": " .. table.concat(parts, "; ") end
    text = text .. worn_line(run.worn or {})
    B.result = {text = text, ok = run.done == run.total and #run.failed == 0 and #run.skipped == 0}
    say_log(text)
    release_for(run)
    X.clear()
    B.dirty()
end

-- Starts the next batch from what is left, finding each item again first.
function X.launch()
    local run = X.run
    if not run then return end
    local k = KINDS[run.kind]
    local steps, entries = {}, {}
    while run.next <= #run.items and #steps < MAX_STEPS do
        local entry = run.items[run.next]
        run.next = run.next + 1
        local cur = find(entry.list, entry.key)
        if not cur or cur.equipped then
            run.skipped[#run.skipped + 1] = entry
        else
            steps[#steps + 1] = step_for(run.kind, cur, entry)
            entries[#entries + 1] = entry
        end
    end
    if #steps == 0 then X.finish(nil); return end
    local bid, err = eqoa.actions.batch({steps = steps, require_window = k.npc})
    if not bid then
        -- nothing of this batch was tried: those items are not counted as done, and not as failed either
        X.finish(tostring(err and (err.message or err.code) or "refused"))
        return
    end
    run.cur = entries
    B.batch_id, B.batch_kind, B.batch_verb, B.batch_total = bid, "multi", k.verb, run.total
    B.result = {text = string.format("%s %d of %d...", k.doing, run.done, run.total), ok = nil}
    B.dirty()
end

-- The merchant or banker window first (the game's own if open, else the mod's hidden one by talking), then the batch.
function X.go()
    local run = X.run
    if not run then return end
    local k = KINDS[run.kind]
    local open
    if k.npc == "merchant" then open = eqoa.merchant.get() ~= nil else
        local b = eqoa.bank.get()
        open = b ~= nil and b.open == true
    end
    if open then X.launch(); return end
    local req, err = B.open_talk_npc(k.npc, k.label, function() X.go() end)
    if not req then
        X.finish(tostring(err and (err.message or err.code) or "could not reach the " .. k.npc))
    end
end

-- items: the selected items now (worn ones already left out, named in `worn`).
-- Is a group action, a batch, a talk to a merchant or banker, or a destroy question still going?
function X.busy()
    -- a run whose batch and talk are both over (the merchant never answered) is not running
    if X.run and not B.batch_id and not B.pending_open then X.run = nil end
    return X.run ~= nil or B.batch_id ~= nil or B.pending_open ~= nil or X.destroy_note ~= nil
end

function X.start(kind, items, worn)
    if X.busy() then
        B.result = {text = KINDS[kind].label .. ": another action is still running", ok = false}
        B.dirty()
        return false
    end
    local run = {kind = kind, items = {}, total = #items, done = 0, failed = {}, skipped = {}, worn = worn, next = 1}
    for _, it in ipairs(items) do
        -- how many the player selected: a stack that grew since is not sold or moved whole
        local picked = X.sel[key_of(it)]
        run.items[#run.items + 1] = {key = key_of(it), list = it.list, name = it.name, idx = #run.items + 1,
            qty = picked and picked.qty or it.quantity}
    end
    X.run = run
    X.go()
    return true
end

function X.on_progress(d)
    local run = X.run
    if not run then return true end
    local k = KINDS[run.kind]
    B.result = {text = string.format("%s %d of %d...", k.doing, run.done + (d.completed or 0), run.total), ok = nil}
    return true
end

-- A batch of the run ended. Counts what it did and goes on with the rest, unless it was stopped or the window went away.
function X.on_batch_result(d)
    local run = X.run
    B.batch_id, B.batch_kind = nil, nil
    if not run then
        B.release_takeover("merchant")
        if B.tab ~= 6 then B.release_takeover("bank") end
        return
    end
    local cur = run.cur or {}
    local completed = d.completed or 0
    run.done = run.done + math.min(completed, #cur)
    if d.ok == true then
        if run.next > #run.items then X.finish(nil) else X.launch() end
        return
    end
    local code = d.code
    local failed_at = d.failed_step or (completed + 1)
    local entry = cur[failed_at]
    local reason = d.reason
    if entry and not STOP_CODES[code] and d.state == "refused" then
        if code == "stale_handle" or code == "not_found" then
            run.skipped[#run.skipped + 1] = entry
        else
            run.failed[#run.failed + 1] = {name = entry.name, reason = reason}
        end
        -- the items after the one that was refused were not tried: they go back in front of the rest (and whatever was
        -- found gone among them is looked at again with them)
        run.next = entry.idx + 1
        for i = #run.skipped, 1, -1 do
            if run.skipped[i].idx >= run.next then table.remove(run.skipped, i) end
        end
        if run.next > #run.items then X.finish(nil) else X.launch() end
        return
    end
    X.finish(tostring(reason or code or d.state or "stopped"))
end

return X
