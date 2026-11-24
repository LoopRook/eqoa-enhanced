-- Listing bank items at the auction, from the Bag Viewer's item menu ("List on auction...") and from the multi-select
-- menu. SPDX-License-Identifier: AGPL-3.0-or-later
-- A small form opens over the Bags window for each item: the price and the buy-now price are numbers typed in boxes
-- (the lowest the game's own form allows is filled in), the time is 12, 24, 48 or 72 hours, and a stack's count is
-- set where the server takes one. "List it" only asks the client to put its own "Are you sure?" to the player, which
-- shows the item and the price; nothing is listed until their Yes. A group is listed one item at a time: the form
-- comes again for each, with Skip and Stop. The bank is opened the way Withdraw and Deposit open it, and is kept
-- while a form or a question is up.
local B = require("bags")

-- `sell_action` is set by main.lua (the one place the mod's eqoa.actions calls are made)
local L = {
    sell_action = nil, session = nil, form = nil, queue = nil, run = nil,
    hits = {}, rect = nil, typing_edge = false,
}

local HOURS = { 12, 24, 48, 72 }
local MAX_PRICE = 999999999
local FORM_W, FORM_H = 244, 124
local SETTLE_FRAMES = 150

local function chop(s) return s:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or "" end

local function say(text, ok)
    B.result = { text = text, ok = ok }
    B.dirty()
end

-- the auction window the client reads `eqoa.auction.get` for (what can be put up is read from it)
local function ensure()
    if L.session then return true end
    local s, err = eqoa.windows.open_session("auction_search", { opened_from = "mod" })
    if not s then return false, err and err.message or "the auction is not available" end
    L.session = s.ref
    return true
end

local function stop_typing()
    if L.form and L.form.field then L.form.field = nil end
    if L.enter then eqoa.input.release(L.enter); L.enter = nil end
    eqoa.input.capture_text(false)
end

function L.close()
    stop_typing()
    if L.session then eqoa.windows.close_session(L.session) end
    L.session, L.form, L.queue, L.run, L.hits, L.rect = nil, nil, nil, nil, {}, nil
end

-- the client ended the session (zoning, death, the player's switch) or the world was reset: nothing is kept
function L.on_session(d)
    if d and d.phase == "closed" and d.ref == L.session then
        stop_typing()
        L.session, L.form, L.queue, L.run = nil, nil, nil, nil
        L.hits, L.rect = {}, nil
    end
end

-- The bank is held for a listing (a form, a question or a run is up): it is not let go meanwhile.
function L.busy() return L.form ~= nil or L.run ~= nil end

-- The bank row `it` was drawn from, as the client holds it now.
local function current(it)
    if it.inventory_key == nil then return it end
    local b = eqoa.bank.get()
    for _, x in ipairs(b and b.items or {}) do
        if x.inventory_key == it.inventory_key then return x end
    end
    return nil
end

-- the sellable entry of the exact bank row (slot and id, as the rest of the client names an item)
local function sellable_for(a, it)
    for _, s in ipairs(a.sellable) do
        if s.id == it.id and (it.index == nil or s.slot == it.index) then return s end
    end
    return nil
end

local function digits(text)
    return tonumber(text) or 0
end

local function lowest(s) return math.max(1, s.value or 1) end

local function new_form(s, it, group)
    local price = math.min(lowest(s), MAX_PRICE)
    local count = math.max(1, s.quantity or 1)
    return {
        s = s, it = it, price = tostring(price), buyout = tostring(math.min(price * 2, MAX_PRICE)), hours = 12,
        count = count, field = nil, msg = nil, group = group, mode = "form",
    }
end

-- ---------------------------------------------------------------- opening the form

-- The bank row of `it` is held up for its form once the client has said what can be put up.
local function start_form(it, group)
    local ok, why = ensure()
    if not ok then say("List on auction: " .. tostring(why), false); L.run = nil; return end
    L.form = { waiting = true, it = it, group = group, frames = 0, mode = "wait" }
end

-- Each frame: a form waiting for the client's reading of the auction takes it when it comes.
local function resolve_wait()
    local f = L.form
    if not (f and f.mode == "wait") then return end
    f.frames = f.frames + 1
    local a = eqoa.auction.get()
    local now = current(f.it)
    if not now then L.skip(f, "the item is no longer in the bank"); return end
    if a then
        if not a.offered then say("List on auction: this server has no auction", false); L.finish(); return end
        local s = sellable_for(a, now)
        if s then
            L.form = new_form(s, now, f.group)
            L.form.stacks = a.stacks == true
            if not L.form.stacks then L.form.count = 1 end
            return
        end
        if f.frames > 3 then
            L.skip(f, now.name .. " cannot be put up at the auction (no trade, no rent, or a stack this server will not take)")
        end
        return
    end
    if f.frames > 180 then
        L.skip(f, "the auction could not be read (if the game's own bank window is open, close it)")
    end
end

-- One item, then the next of the group.
function L.skip(f, why)
    say("List on auction: " .. why, false)
    if f and f.group then f.group.failed = f.group.failed + 1 end
    L.next_item()
end

function L.finish()
    stop_typing()
    local run = L.run
    L.form, L.run, L.queue, L.hits, L.rect = nil, nil, nil, {}, nil
    if run and run.total > 1 then
        say(string.format("Listing done: %d of %d put up%s%s", run.sent, run.total,
            run.skipped > 0 and (", " .. run.skipped .. " skipped") or "",
            run.failed > 0 and (", " .. run.failed .. " not possible") or ""), run.sent > 0)
    end
end

function L.next_item()
    stop_typing()
    local q = L.queue
    local it = q and table.remove(q, 1)
    if not it then L.finish(); return end
    L.run.at = L.run.at + 1
    start_form(it, L.run)
end

-- "List on auction..." of one bank item: the bank is opened as Withdraw opens it, then the form.
function L.begin(it)
    if L.busy() then say("List on auction: finish the one in front of you first", false); return end
    L.run = { total = 1, at = 1, sent = 0, skipped = 0, failed = 0 }
    L.queue = nil
    local function go() start_form(it, nil) end
    local bank = eqoa.bank.get()
    if bank and bank.open then go(); return end
    local req, err = B.open_talk_npc("bank", "List on auction", go)
    if not req then L.run = nil end
    return req, err
end

-- A group of bank items, one form after the other.
function L.start_group(items)
    if L.busy() then say("List on auction: finish the one in front of you first", false); return end
    local queue = {}
    for _, it in ipairs(items) do queue[#queue + 1] = it end
    L.run = { total = #queue, at = 0, sent = 0, skipped = 0, failed = 0 }
    L.queue = queue
    local function go() L.next_item() end
    local bank = eqoa.bank.get()
    if bank and bank.open then go(); return end
    local req = B.open_talk_npc("bank", "List on auction", go)
    if not req then L.run, L.queue = nil, nil end
end

-- ---------------------------------------------------------------- sending

local function limits_say(f)
    local price, buyout = digits(f.price), digits(f.buyout)
    local lo = lowest(f.s)
    if price < lo then return "The lowest starting price for this item is " .. lo end
    if price > MAX_PRICE or buyout > MAX_PRICE then return "A price is at most " .. MAX_PRICE end
    if buyout < price then return "The buy-now price cannot be below the starting price" end
    return nil
end

local function send(f)
    local why = limits_say(f)
    if why then f.msg = why; return end
    stop_typing()
    local req, err = L.sell_action({
        slot = f.s.slot, id = f.s.id, count = f.stacks and f.count or 1,
        price = digits(f.price), buyout = digits(f.buyout), hours = f.hours,
    })
    if not req then
        f.msg = err and (err.message or err.code) or "refused"
        return
    end
    f.mode, f.req, f.msg = "queued", req, nil
    say("List on auction: answer the question on the screen", nil)
end

-- the call's own result: the question is up, or the call was refused
function L.on_action_result(d)
    local f = L.form
    if not (f and f.mode == "queued" and d and d.request_id == f.req and d.ok ~= nil) then return false end
    if d.ok == true then
        f.mode = "question"
        return true
    end
    local reason = d.reason or d.code or "refused"
    if d.code == "busy" then
        -- the last one has not been answered yet, or another question is up: the same form again, to try again
        f.mode, f.req = "form", nil
        f.msg = "Still waiting for the server (or a question is up): try again in a moment"
        if reason:find("game's own", 1, true) then f.msg = "Close the game's own bank window first: listing goes through this window" end
        return true
    end
    f.mode, f.req = "form", nil
    f.msg = reason
    if reason:find("game's own", 1, true) then f.msg = "Close the game's own bank window first: listing goes through this window" end
    return true
end

-- What the player answered to the client's question.
function L.on_confirm_closed(d)
    if not d or d.kind ~= "auction_sell" then return false end
    local f = L.form
    local run = L.run
    if d.answer == "yes" and d.detail and d.detail.sent then
        say("Put up at the auction: sent", true)
        if run then run.sent = run.sent + 1 end
        -- the sale is the bank's request: the next waits until the client says it is answered
        if f then f.mode, f.settle, f.seen = "settle", 0, false end
    elseif d.answer == "yes" then
        say("Not listed (" .. tostring(d.detail and d.detail.reason) .. ")", false)
        if run then run.failed = run.failed + 1 end
        L.next_item()
    else
        say("Not listed", true)
        if run then run.skipped = run.skipped + 1 end
        L.next_item()
    end
    B.dirty()
    return true
end

local function settle_tick()
    local f = L.form
    if not (f and f.mode == "settle") then return end
    f.settle = f.settle + 1
    local a = eqoa.auction.get()
    if a and a.pending == "sell" then f.seen = true end
    -- answered: the client has shown the sale pending and now idle (or a while has passed with no sign of it)
    if a and a.pending == "none" and (f.seen or f.settle > 20) or f.settle > SETTLE_FRAMES then
        L.next_item()
    end
end

function L.tick()
    if not L.form then return end
    resolve_wait()
    settle_tick()
end

-- ---------------------------------------------------------------- the form's input

local function type_into(f, field, text)
    local cap = 9
    if #f[field] + #text <= cap then f[field] = f[field] .. text end
end

-- A key while the form is up. True when the form took it.
function L.on_key(d)
    local f = L.form
    if not (f and f.mode == "form") or not d.down then return false end
    local key = d.key:lower()
    if f.field then
        if key == "escape" then stop_typing()
        elseif key == "enter" or key == "return" then stop_typing()
        elseif key == "tab" then f.field = f.field == "price" and "buyout" or "price"
        elseif key == "backspace" then f[f.field] = chop(f[f.field])
        elseif d.text and #d.text > 0 then type_into(f, f.field, (d.text:gsub("%D", ""))) end
        return true
    end
    if key == "escape" and not d["repeat"] then
        L.skip_current(true)
        return true
    end
    return false
end

-- Cancel (a single listing) or Stop (a group).
function L.skip_current(stop)
    local run = L.run
    if run and stop then run.skipped = run.skipped + 1 + #(L.queue or {}) end
    L.queue = nil
    L.finish()
end

-- A press while the form is up: the form is modal, every press is the form's. True when taken.
function L.on_pointer(d)
    local f = L.form
    if not (f and f.mode == "form") then return false end
    if d.button == "left" then
        for _, h in ipairs(L.hits) do
            if d.x >= h.x and d.x <= h.x + h.w and d.y >= h.y and d.y <= h.y + h.h and h.run then h.run(); return true end
        end
    end
    -- a click anywhere else (or the other button) ends the typing
    stop_typing()
    return true
end

-- Each frame the Bags window is drawn: the form, over it. `h` has the window's own drawing helpers.
function L.draw(h)
    local f = L.form
    if not (f and f.mode == "form") then
        L.hits, L.rect = {}, nil
        if L.enter then stop_typing() end
        return
    end
    local ui, k = h.ui, h.k
    local c = h.c
    local x, y = (h.W - FORM_W) / 2, 52
    L.rect = { x = h.x0 + x * k, y = h.y0 + y * k, w = FORM_W * k, h = FORM_H * k }
    -- the pointer pressed anywhere else (another mod's window in front too) ends the typing: the capture is the player's
    local down = ui.IsMouseDown(0) or ui.IsMouseDown(1)
    if f.field and down and not L.was_down then
        local p = eqoa.input.pointer()
        local r = L.rect
        if not (p and p.x >= r.x and p.x <= r.x + r.w and p.y >= r.y and p.y <= r.y + r.h) then stop_typing() end
    end
    L.was_down = down
    if f.field then
        if not L.enter then L.enter = eqoa.input.claim("Enter", true) end
        -- renewed every frame: the capture lapses after about a second
        eqoa.input.capture_text(true)
    elseif L.enter then
        eqoa.input.release(L.enter); L.enter = nil
    end
    L.hits = {}
    local function hit(hx, hy, hw, hh, run, label) L.hits[#L.hits + 1] = { x = h.x0 + hx * k, y = h.y0 + hy * k, w = hw * k, h = hh * k, run = run, label = label } end
    h.fill(x, y, FORM_W, FORM_H, c.menu_bg)
    h.frame(x, y, FORM_W, FORM_H, c.red)
    local title = "List on auction"
    if f.group and f.group.total > 1 then title = title .. string.format("  (%d of %d)", f.group.at, f.group.total) end
    h.text(x + 6, y + 4, title, c.white, FORM_W - 12)
    local name = f.it.name .. (f.s.quantity > 1 and (" x" .. f.s.quantity) or "")
    h.text(x + 6, y + 17, name, c.text, FORM_W - 12)
    local function box(by, label, field, hint)
        h.text(x + 6, by + 2, label, c.muted)
        local bx, bw = x + 62, 84
        h.fill(bx, by, bw, 14, c.raised)
        h.frame(bx, by, bw, 14, f.field == field and c.white or c.border)
        h.text(bx + 4, by + 2, f[field] .. (f.field == field and "_" or ""), c.text, bw - 8)
        h.text(bx + bw + 6, by + 2, hint, c.muted, FORM_W - (bx - x) - bw - 12)
        hit(bx, by, bw, 14, function() f.field = field; f.msg = nil; if not L.enter then L.enter = eqoa.input.claim("Enter", true) end end, field)
    end
    box(y + 32, "Price", "price", "lowest " .. lowest(f.s) .. (f.s.quantity > 1 and f.stacks and " each" or ""))
    box(y + 50, "Buy now", "buyout", "from the price")
    -- the time
    h.text(x + 6, y + 70, "Time", c.muted)
    for i, hr in ipairs(HOURS) do
        local bx = x + 62 + (i - 1) * 40
        local on = f.hours == hr
        h.fill(bx, y + 68, 36, 14, on and c.menu_hl or c.raised)
        h.frame(bx, y + 68, 36, 14, on and c.white or c.border)
        h.text(bx + 4, y + 70, hr .. " h", on and c.white or c.muted, 30)
        hit(bx, y + 68, 36, 14, function() f.hours = hr end, "hours" .. hr)
    end
    -- a stack's count, where the server takes one
    if f.stacks and f.s.quantity > 1 then
        h.text(x + 6, y + 88, "Count " .. f.count, c.muted)
        local function step(bx, label, delta, enabled)
            h.fill(bx, y + 86, 22, 14, c.raised)
            h.frame(bx, y + 86, 22, 14, enabled and c.border or c.surface)
            h.text(bx + 8, y + 88, label, enabled and c.text or c.muted)
            if enabled then hit(bx, y + 86, 22, 14, function() f.count = f.count + delta end, label) end
        end
        step(x + 62, "-", -1, f.count > 1)
        step(x + 88, "+", 1, f.count < f.s.quantity)
    end
    -- the buttons
    local by = y + FORM_H - 20
    local function button(bx, bw, label, run)
        h.fill(bx, by, bw, 16, c.raised)
        h.frame(bx, by, bw, 16, c.border)
        h.text(bx + 6, by + 2, label, c.text, bw - 8)
        hit(bx, by, bw, 16, run, label)
    end
    button(x + 6, 62, "List it", function() send(f) end)
    if f.group and f.group.total > 1 then
        button(x + 72, 46, "Skip", function()
            f.group.skipped = f.group.skipped + 1
            L.next_item()
        end)
        button(x + 122, 46, "Stop", function() L.skip_current(true) end)
    else
        button(x + 72, 56, "Cancel", function() L.skip_current(false) end)
    end
    if f.msg then h.text(x + 6, y + FORM_H - 34, f.msg, c.warn, FORM_W - 12) end
    eqoa.input.claim_pointer({ x = L.rect.x, y = L.rect.y, width = L.rect.w, height = L.rect.h })
end

return L
