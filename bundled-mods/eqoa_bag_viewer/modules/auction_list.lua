-- Listing a bank item at the auction, from the Bag Viewer's item menu ("List on auction...") and from the multi-select
-- menu. SPDX-License-Identifier: AGPL-3.0-or-later
-- The menu page lets the player set the price, the buyout and the time (the game's own limits: a starting price from
-- the item's value, a buyout from the price, 12 to 72 hours). "List it" only asks the client to put its own "Are you
-- sure?" to the player, which shows the price; nothing is listed until their Yes. A group is listed one item at a
-- time, each with its own question showing its own price.
local B = require("bags")

local L = { session = nil, form = nil, queue = nil, waiting = false, total = 0, done = 0 }

local HOURS = { 12, 24, 48, 72 }
local MAX_PRICE = 999999999

-- the auction window the client reads `eqoa.auction.get` for (what can be put up is read from it)
local function ensure()
    if L.session then return true end
    local s, err = eqoa.windows.open_session("auction_search", { opened_from = "mod" })
    if not s then return false, err and err.message or "the auction is not available" end
    L.session = s.ref
    return true
end

function L.close()
    if L.session then eqoa.windows.close_session(L.session) end
    L.session, L.form, L.queue, L.waiting = nil, nil, nil, false
end

-- the client ended the session (zoning, death, the player's switch)
function L.on_session(d)
    if d and d.phase == "closed" and d.ref == L.session then
        L.session, L.form, L.queue, L.waiting = nil, nil, nil, false
    end
end

-- Whether the menu may offer listing: the bank is held (selling is a page of the bank window).
function L.available()
    local bank = eqoa.bank.get()
    if not (bank and bank.open) then return false, "the bank window is not open" end
    return true
end

-- The bank row of `it` that the client says can be put up.
function L.lookup(it)
    local ok, why = ensure()
    if not ok then return nil, why end
    local a = eqoa.auction.get()
    if not a then return nil, "reading the auction: try again" end
    if not a.offered then return nil, "this server has no auction" end
    for _, s in ipairs(a.sellable) do
        if s.id == it.id then return s end
    end
    return nil, "this item cannot be put up at the auction"
end

local function clamp(v, lo, hi) return math.max(lo, math.min(hi, v)) end

local function defaults(s)
    local price = clamp(math.max(1, s.value or 1), 1, MAX_PRICE)
    return { s = s, price = price, buyout = clamp(price * 2, price, MAX_PRICE), hours = 12 }
end

local function sell(f)
    local function go(count)
        return eqoa.actions.auction_sell({ slot = f.s.slot, id = f.s.id, count = count, price = f.price, buyout = f.buyout, hours = f.hours })
    end
    local count = math.max(1, f.s.quantity or 1)
    local r, e = go(count)
    -- a server that takes one item at a time: the one
    if not r and e and e.code == "invalid_argument" and count > 1 then r, e = go(1) end
    return r, e
end

local show

local function step(f, field, factor)
    if field == "price" then
        local d = math.max(1, math.floor(f.price * 0.1))
        f.price = clamp(f.price + factor * d, math.max(1, f.s.value or 1), MAX_PRICE)
        f.buyout = clamp(f.buyout, f.price, MAX_PRICE)
    else
        local d = math.max(1, math.floor(f.buyout * 0.1))
        f.buyout = clamp(f.buyout + factor * d, f.price, MAX_PRICE)
    end
end

-- The menu page for the form (a fresh table each time, so the menu stays up after a click).
function show()
    local f, menu = L.form, B.menu
    if not f then return end
    local rows = {}
    local function add(label, run)
        rows[#rows + 1] = { label = label, enabled = true, local_only = true, run = run }
    end
    add("Price " .. f.price .. " tunar: click for 10% less", function() step(f, "price", -1); show() end)
    add("Price " .. f.price .. " tunar: click for 10% more", function() step(f, "price", 1); show() end)
    add("Buy now " .. f.buyout .. " tunar: 10% less", function() step(f, "buyout", -1); show() end)
    add("Buy now " .. f.buyout .. " tunar: 10% more", function() step(f, "buyout", 1); show() end)
    add("Time " .. f.hours .. " hours: click for the next", function()
        local at = 1
        for i, h in ipairs(HOURS) do if h == f.hours then at = i end end
        f.hours = HOURS[at % #HOURS + 1]
        show()
    end)
    add("List it for " .. f.price .. ", buy now " .. f.buyout, function()
        local form = f
        L.form = nil
        B.run_asking("List on auction", function() return sell(form) end)
    end)
    add("Cancel", function() L.form = nil end)
    B.menu = { title = "List " .. f.s.name .. (f.s.quantity > 1 and (" x" .. f.s.quantity) or "") .. " on auction?",
        ref = menu and menu.ref, rows = rows, fx = menu and menu.fx or 8, fy = menu and menu.fy or 46, asking = true }
end

-- The "List on auction..." row of a bank item's menu was chosen.
function L.begin(it)
    local s, why = L.lookup(it)
    if not s then
        B.result = { text = "List on auction: " .. why, ok = false }
        return
    end
    L.form = defaults(s)
    show()
end

-- A group of bank items, one at a time with each item's own question.
function L.start_group(items)
    local queue = {}
    for _, it in ipairs(items) do queue[#queue + 1] = it end
    L.queue, L.total, L.done, L.waiting = queue, #queue, 0, false
    B.result = { text = "Listing " .. #queue .. " items: one question for each", ok = nil }
end

-- Each step: the next item of a group goes when the client has nothing waiting.
function L.tick()
    local q = L.queue
    if not q or L.waiting then return end
    local a = eqoa.auction.get()
    if not a or a.pending ~= "none" then return end
    local it = table.remove(q, 1)
    if not it then L.queue = nil; return end
    local s, why = L.lookup(it)
    if not s then
        B.result = { text = it.name .. ": " .. tostring(why), ok = false }
        if #q == 0 then L.queue = nil end
        return
    end
    local r, e = sell(defaults(s))
    if not r then
        B.result = { text = "List on auction: " .. (e and (e.message or e.code) or "refused"), ok = false }
        L.queue = nil
        return
    end
    L.waiting = true
    B.result = { text = "Listing " .. s.name .. ": answer the question on the screen", ok = nil }
end

-- What the player answered to the client's question.
function L.on_confirm_closed(d)
    if not d or d.kind ~= "auction_sell" then return false end
    if d.answer == "yes" then
        local sent = d.detail and d.detail.sent
        B.result = { text = sent and "Put up at the auction: sent" or ("Not listed (" .. tostring(d.detail and d.detail.reason) .. ")"), ok = sent == true }
        L.done = L.done + 1
        L.waiting = false
        if not sent then L.queue = nil end
    else
        B.result = { text = "Not listed", ok = true }
        L.waiting, L.queue = false, nil
    end
    B.dirty()
    return true
end

return L
