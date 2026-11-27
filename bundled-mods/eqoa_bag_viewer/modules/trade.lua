-- Bag viewer: merchants and bankers in the Bags window (an option, off by default).
-- With "Open merchants and bankers in Bag Viewer" on, the mod holds the game's merchant and bank windows whole
-- (`eqoa.windows.takeover(kind, {scope = "all"})`): a merchant or banker the player talks to opens this window instead,
-- on the Merchant tab (wares and prices on the left, the pack on the right) or the Bank tab (the bank on the left, the
-- pack on the right). A click buys, sells or moves; a stack asks how many first. Leaving the tab, closing the window,
-- Esc, walking away and zoning end the visit, and the game's session with it. Option off: nothing here runs.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local B = require("bags")
local X = require("multi")

local T = {
    TAB = 8, TAB_LABEL = "Merchant",
    PANEL_W = 270, -- the left panel's width, in HUD frame pixels (the window is this much wider while it is up)
    on = false, held = {}, told = {}, polled = 0,
    prompt = nil, enter = nil, scroll = 0, hits = {}, prompt_rect = nil, rect = nil,
    pad_list = false, pad_row = 1,
    auto_opened = false, was_bank = false, cache = nil,
}
B.trade = T

local OPTION = "open_npc"
local KINDS = {"merchant", "bank"}
local HEAD, ROW, FOOT = 16, 17, 17 -- the panel's header, a row, and the foot (Sell (N))

eqoa.options.toggle({id = OPTION, label = "Open merchants and bankers in Bag Viewer", default = false})

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

function T.refresh() T.on = eqoa.options.get(OPTION) == true end
T.refresh()

local function note(text) eqoa.log.write("info", "Bag Viewer merchants and bankers: " .. text) end

-- ---------------------------------------------------------------- the claims

local function claim(kind)
    local ok, err = eqoa.windows.takeover(kind, {scope = "all"})
    T.held[kind] = ok == true
    local told = ok and "held" or tostring(err and err.code or "refused")
    if T.told[kind] ~= told then
        T.told[kind] = told
        note(ok and ("holding the " .. kind .. " window") or ("cannot hold the " .. kind .. " window: " .. tostring(err and err.message or "refused")))
    end
end

-- Held while the option is on, and held again every so often: a claim ends on zoning and with the player's "Use the
-- game's windows" switch. Let go when the option is turned off.
function T.poll(force)
    local now = os.clock()
    if not force and now - T.polled < 1.0 then return end
    T.polled = now
    T.refresh()
    for _, kind in ipairs(KINDS) do
        if T.on then
            claim(kind)
        elseif T.held[kind] then
            eqoa.windows.release(kind)
            T.held[kind], T.told[kind] = nil, nil
            note("no longer holding the " .. kind .. " window")
        end
    end
end

-- A helper (or the window closing) let the kind go: the claim is taken again at once while the option is on.
function T.after_release(kind)
    T.held[kind] = nil
    if T.on then claim(kind) end
end

-- The merchant the player is shopping at, which no helper may let go.
function T.keeps(kind) return kind == "merchant" and T.visit_merchant() end

local function end_visit(kind)
    eqoa.windows.release(kind)
    T.after_release(kind)
end

-- The game's own window for the session this mod holds (the option went off, or Bag Viewer could not open).
local function try_pass(kind)
    local s = eqoa.windows.session(kind)
    if not s then return false end
    local ok, err = eqoa.windows.pass(s.ref)
    note(ok and ("the game's own " .. kind .. " window is shown") or ("the game's " .. kind .. " window could not be shown (" .. tostring(err and err.code) .. ")"))
    if not T.on then T.poll(true) end
    return true
end

function T.hand_over(kind)
    try_pass(kind)
end

-- ---------------------------------------------------------------- what is on show

-- The merchant the player talked to (a helper's own talk is not a visit).
function T.visit_merchant()
    return T.on and T.held.merchant == true and not B.helper_hold.merchant and eqoa.merchant.get() ~= nil
end

-- "merchant" on the Merchant tab, "bank" on the Bank tab while the bank is open here, else nil.
function T.mode()
    if not T.on then return nil end
    if B.tab == T.TAB then return T.visit_merchant() and "merchant" or nil end
    if B.tab == 6 and T.held.bank == true then
        local bank = eqoa.bank.get()
        if bank and bank.open then return "bank" end
    end
    return nil
end

local function begin_visit(kind)
    local was_open = B.open
    if not was_open then T.open_viewer() end
    if not B.open then T.hand_over(kind); return end -- the window would not open: the game's own is shown
    B.select(kind == "merchant" and T.TAB or 6)
    T.auto_opened = not was_open -- (after the select: leaving a tab clears it)
end

-- An NPC the player talked to. Only a window this mod holds is ours to open over: another mod's claim, or none, is left
-- alone (its window, or the game's, is what the player sees). The option may have gone off since the last poll: the game's
-- own window is shown then, and the claims go back with it.
local function opened(kind)
    T.refresh()
    if not T.held[kind] then return end
    if not T.on then T.hand_over(kind); return end
    begin_visit(kind)
end

-- merchant.changed / bank.changed (the mod's always-on listeners): an NPC the player talked to opens the window.
function T.on_merchant(d)
    if not d then return end
    if d.after == nil then B.helper_hold.merchant = nil; return end
    if d.before == nil and not B.helper_hold.merchant then opened("merchant") end
end

function T.on_bank(d)
    if not d then return end
    local after, before = d.after, d.before
    if after == nil or not after.open then B.helper_hold.bank = nil; return end
    if not (before and before.open) and not B.helper_hold.bank then opened("bank") end
end

-- The Merchant tab is left: the visit ends (the game's session with it).
function T.leaving(old)
    T.cancel_prompt()
    T.auto_opened = false
    T.pad_list = false
    if old == T.TAB and eqoa.merchant.get() ~= nil then end_visit("merchant") end
end

function T.after_close()
    T.cancel_prompt()
    T.auto_opened, T.was_bank, T.pad_list, T.hits, T.cache = false, false, false, {}, nil
    for _, kind in ipairs(KINDS) do T.after_release(kind) end
end

-- A new world: the claims ended with the old one.
function T.reset()
    T.cancel_prompt()
    T.auto_opened, T.was_bank, T.held, T.told, T.cache = false, false, {}, {}, nil
    T.poll(true)
end

-- Each frame the window is up: has the visit ended (walked away, the server closed it, the option went off)?
function T.tick()
    T.refresh()
    -- the option went off while a visit (or just the claim) was up: the windows go back to the game now
    if not T.on and (T.held.merchant or T.held.bank) then T.poll(true) end
    local mode = T.mode()
    local ended_merchant = B.tab == T.TAB and mode == nil
    local ended_bank = B.tab == 6 and T.was_bank and mode == nil
    T.was_bank = mode == "bank"
    if T.prompt and mode == nil then T.cancel_prompt() end
    if ended_merchant or ended_bank then
        T.pad_list = false
        if T.auto_opened then B.step_aside = true -- the visit opened the window: it goes with it
        elseif ended_merchant then B.select(1) end
    end
end

-- ---------------------------------------------------------------- the question "how many"

local function find(list, inventory_key)
    local c
    if list == "merchant" then c = eqoa.merchant.get() elseif list == "bank" then c = eqoa.bank.get() else c = eqoa.inventory.get() end
    for _, it in ipairs(c and c.items or {}) do
        if it.inventory_key == inventory_key then return it end
    end
    return nil
end

local function carried()
    local inv = eqoa.inventory.get()
    return inv and inv.money or 0
end

local function price(it, direction, n) return (eqoa.merchant.price(it.ref, direction, n)) end

-- Whether the tunar the player carries pays a price (a price the client could not give is not held against the ware).
local function affords(unit) return unit == nil or unit <= carried() end
function T.affordable(entry) return affords(entry.price) end

local function times(n) return n > 1 and (" x" .. n) or "" end

local function stop_typing()
    if T.enter then
        eqoa.input.release(T.enter)
        T.enter = nil
        eqoa.input.capture_text(false)
    end
end

function T.typing() return T.enter ~= nil end

function T.cancel_prompt()
    T.prompt = nil
    stop_typing()
end

local VERBS = {buy = "Buy", sell = "Sell", withdraw = "Take out", deposit = "Put in bank"}

local function open_prompt(kind, it, max, default)
    T.prompt = {kind = kind, list = it.list, key = it.inventory_key, name = it.name, max = max, qty = default, text = nil}
    B.result = nil
    if not T.enter then T.enter = eqoa.input.claim("Enter", true) end
    B.dirty()
end

-- ---------------------------------------------------------------- the actions (each count is exactly what was asked)

function T.buy(it, n)
    local total = price(it, "buy", n)
    local label = "Buy " .. it.name .. times(n) .. (total and (" (" .. B.tunar(total) .. "t)") or "")
    local have = carried()
    if total and total > have then
        B.result = {text = label .. ": you carry only " .. B.tunar(have) .. " tunar", ok = false}
        B.dirty()
        return
    end
    B.run_single(label, function() return eqoa.actions.buy(it.ref, n) end)
end

function T.sell(it, n)
    local total = price(it, "sell", n)
    local label = "Sell " .. it.name .. times(n) .. (total and (" (" .. B.tunar(total) .. "t)") or "")
    B.run_single(label, function() return eqoa.actions.sell(it.ref, n) end)
end

function T.move(it, n)
    local from = it.list == "bank" and "bank" or "inventory"
    local label = (from == "bank" and "Take out " or "Put in bank ") .. it.name .. times(n)
    B.run_single(label, function() return eqoa.actions.bank_move(from, it.ref, n) end)
end

-- A ware was clicked: one is bought at once, a stackable one asks how many first. What cannot be paid is not tried.
function T.click_ware(it)
    local unit = price(it, "buy", 1)
    local have = carried()
    if not affords(unit) then
        B.result = {text = it.name .. ": costs " .. B.tunar(unit) .. " tunar, you carry " .. B.tunar(have), ok = false}
        B.dirty()
        return
    end
    if (it.max_stack or 0) > 1 then
        local afford = unit and unit > 0 and math.floor(have / unit) or it.max_stack
        open_prompt("buy", it, math.min(it.max_stack, afford), 1)
    else
        T.buy(it, 1)
    end
end

-- A stack in the bank (left) or the pack (right, on the Bank tab) was clicked: one moves at once, a stack asks how many.
function T.click_move(it)
    if it.equipped then
        B.result = {text = it.name .. ": cannot move an equipped item", ok = false}
        B.dirty()
        return
    end
    local q = it.quantity or 1
    if q > 1 then
        open_prompt(it.list == "bank" and "withdraw" or "deposit", it, q, q)
    else
        T.move(it, 1)
    end
end

-- Right-click on a pack item at a merchant: a single item is offered at once, a stack asks how many (the whole stack
-- is what it starts on).
function T.sell_item(it)
    if it.equipped then
        B.result = {text = it.name .. ": cannot sell an equipped item", ok = false}
        B.dirty()
        return
    end
    local q = it.quantity or 1
    if q > 1 then open_prompt("sell", it, q, q) else T.sell(it, 1) end
end

local function prompt_qty(p, n)
    p.qty = math.max(0, math.min(p.max, n))
    p.text = nil
end

function T.confirm_prompt()
    local p = T.prompt
    if not p then return end
    local it = find(p.list, p.key)
    local n = math.floor(p.qty)
    T.cancel_prompt()
    if n < 1 then return end
    if not it then
        B.result = {text = VERBS[p.kind] .. " " .. p.name .. ": it is gone", ok = false}
        B.dirty()
        return
    end
    if p.kind ~= "buy" then n = math.min(n, it.quantity or 1) end
    if p.kind == "buy" then T.buy(it, n)
    elseif p.kind == "sell" then T.sell(it, n)
    else T.move(it, n) end
end

function T.on_key(d)
    local p = T.prompt
    if not p or not d.down then return false end
    local key = d.key:lower()
    if key == "escape" then T.cancel_prompt()
    elseif key == "enter" or key == "return" then T.confirm_prompt()
    elseif key == "backspace" then
        local text = (p.text or tostring(math.floor(p.qty))):sub(1, -2)
        p.text = text
        p.qty = math.min(p.max, tonumber(text) or 0)
    elseif d.text and #d.text > 0 then
        local digits = d.text:gsub("%D", "")
        if digits ~= "" then
            local text = ((p.text or "") .. digits):sub(1, 4)
            p.text = text
            p.qty = math.min(p.max, tonumber(text) or 0)
        end
    end
    return true
end

-- ---------------------------------------------------------------- the controller

-- Left at the pack grid's first column goes to the left panel; up and down move over its rows, confirm presses one, right
-- goes back to the grid. While the question is up: up and down change the count, confirm answers, back cancels.
function T.pad_key(key, col0)
    local mode = T.mode()
    if not mode then return false end
    local p = T.prompt
    if p then
        if key == "up" then prompt_qty(p, p.qty + 1)
        elseif key == "down" then prompt_qty(p, p.qty - 1)
        elseif key == "right" then prompt_qty(p, p.qty + 10)
        elseif key == "left" then prompt_qty(p, p.qty - 10)
        elseif key == "confirm" then T.confirm_prompt()
        elseif key == "back" then T.cancel_prompt() end
        return true
    end
    if T.pad_list then
        local entries = T.entries(mode)
        if key == "up" then T.pad_row = math.max(1, T.pad_row - 1)
        elseif key == "down" then T.pad_row = math.min(math.max(1, #entries), T.pad_row + 1)
        elseif key == "right" then T.pad_list = false
        elseif key == "confirm" then
            local e = entries[T.pad_row]
            if e then
                if mode == "merchant" then T.click_ware(e.item) else T.click_move(e.item) end
            end
        elseif key == "back" then
            return false
        end
        return true
    end
    if key == "left" and col0 then
        T.pad_list, T.pad_row = true, math.max(1, math.min(T.pad_row, #T.entries(mode)))
        return true
    end
    return false
end

-- ---------------------------------------------------------------- the left panel

-- The NPC's stacks as rows, read again when the container changes.
function T.entries(mode)
    local c
    if mode == "merchant" then c = eqoa.merchant.get() else c = eqoa.bank.get() end
    if not c then return {} end
    local cache = T.cache
    if cache and cache.mode == mode and cache.rev == c.revision then return cache.entries end
    local entries = {}
    for _, it in ipairs(c.items) do
        entries[#entries + 1] = {item = it, price = mode == "merchant" and price(it, "buy", 1) or nil}
    end
    T.cache = {mode = mode, rev = c.revision, entries = entries}
    return entries
end

-- The merchant's own words for its menu choices (buy, sell, exit), else the key capitalised.
local function option_label(key)
    local m = eqoa.merchant.get()
    local state = m and m.merchant
    for _, o in ipairs(state and state.options or {}) do
        if o.key == key and o.label and o.label ~= "" then return o.label end
    end
    return key:sub(1, 1):upper() .. key:sub(2)
end

-- The label of the panel's top-right button: the merchant's own word for leaving, else Done.
function T.leave_label(mode) return mode == "merchant" and option_label("exit") or "Done" end

-- What the open question's count comes to (tunar, for a buy or a sale), or nil when it is not a price.
function T.prompt_total(p)
    local qty = math.floor(p.qty)
    if (p.kind ~= "buy" and p.kind ~= "sell") or qty < 1 then return nil end
    local it = find(p.list, p.key)
    return it and price(it, p.kind, qty) or nil
end

-- What the line under the panels says before anything has happened.
function T.hint(mode)
    if mode == "merchant" then
        return string.format("%s: click a ware.  %s: right-click a bag item.", option_label("buy"), option_label("sell"))
    end
    return "Click an item to move it across"
end

-- The left panel's caption (under it, where the pack's own caption is on the right).
function T.caption(mode)
    if mode == "merchant" then
        local m = eqoa.merchant.get()
        local state = m and m.merchant
        return (state and state.pending) and "Waiting for the merchant..." or "Wares"
    end
    local bank = eqoa.bank.get()
    return bank and ("Bank " .. bank.used .. "/" .. tostring(bank.capacity or "?") .. "   tunar " .. B.tunar(bank.money)) or ""
end

-- The hover card of a pack item at a merchant gains what it would sell for.
function T.card(lines, it, mode)
    if mode ~= "merchant" or it.list ~= "inventory" or it.equipped then return lines end
    local total = price(it, "sell", it.quantity or 1)
    if not total then return lines end
    local out = {}
    for i, l in ipairs(lines) do out[i] = l end
    out[#out + 1] = {"Sells for " .. B.tunar(total) .. " tunar" .. ((it.quantity or 1) > 1 and " (the stack)" or ""), "muted"}
    return out
end

function T.on_wheel(d)
    local steps = d.steps or 0
    if steps == 0 or not T.mode() then return false end
    local p = T.prompt
    if p and inside(d.x, d.y, T.prompt_rect) then
        prompt_qty(p, p.qty + (steps > 0 and 1 or -1))
        return true
    end
    if inside(d.x, d.y, T.rect) then
        T.scroll = math.max(0, T.scroll + (steps > 0 and -1 or 1))
        return true
    end
    return false
end

function T.on_pointer(d)
    local mode = T.mode()
    if not mode or B.menu then return false end
    if T.prompt then
        if d.button == "left" then
            for _, h in ipairs(T.hits) do
                if h.prompt and inside(d.x, d.y, h) then h.run(); return true end
            end
            if inside(d.x, d.y, T.prompt_rect) then return true end
        end
        T.cancel_prompt() -- a press anywhere else is No
        return true
    end
    if d.button == "left" then
        for _, h in ipairs(T.hits) do
            if inside(d.x, d.y, h) then h.run(); return true end
        end
        if mode == "bank" and not d.ctrl and not d.shift then
            for _, h in ipairs(B.slot_hits) do
                if inside(d.x, d.y, h) then X.drop_selection(); T.click_move(h.item); return true end
            end
        end
    elseif d.button == "right" and mode == "merchant" then
        for _, h in ipairs(B.slot_hits) do
            -- a selected item's right-click is the group menu's (Sell (N))
            if inside(d.x, d.y, h) and not X.is_selected(h.item) then T.sell_item(h.item); return true end
        end
    end
    return false
end

-- Leave (the panel's top right button): the same as closing the Merchant tab or the window.
function T.leave()
    if T.auto_opened then B.step_aside = true else B.select(1) end
end

-- Drawn each frame the panel is up. `g` carries the window's drawing helpers and the panel's place (x, y, h in frame pixels).
function T.draw(g)
    local c, k, mode = g.c, g.k, g.mode
    local fill, frame, text = g.fill, g.frame, g.text
    local x, y, w, h = g.x, g.y, T.PANEL_W, g.h
    T.rect = {x = g.x0 + x * k, y = g.y0 + y * k, w = w * k, h = h * k}
    T.hits = {}
    fill(x, y, w, h, c.raised)
    frame(x, y, w, h, c.border)

    local function button(bx, by, bw, label, col, bcol, run, in_prompt)
        fill(bx, by, bw, 14, c.surface)
        frame(bx, by, bw, 14, bcol)
        text(bx + 4, by + 1, label, col, bw - 6)
        T.hits[#T.hits + 1] = {x = g.x0 + bx * k, y = g.y0 + by * k, w = bw * k, h = 14 * k, run = run, prompt = in_prompt}
    end

    -- header: what this side is, and Leave
    local leave = T.leave_label(mode)
    local lw = g.tw(leave) / k + 10
    text(x + 6, y + 3, mode == "merchant" and option_label("buy") or "Bank", c.white)
    button(x + w - lw - 3, y + 1, lw, leave, c.text, c.border, T.leave)

    local entries = T.entries(mode)
    local foot = mode == "merchant" and X.n > 0
    local rows = math.floor((h - HEAD - (foot and FOOT or 0)) / ROW)
    T.scroll = math.max(0, math.min(math.max(0, #entries - rows), T.scroll))
    T.pad_row = math.max(1, math.min(T.pad_row, math.max(1, #entries)))
    local visible = {}
    for i = 1, rows do
        local e = entries[T.scroll + i]
        if e then visible[#visible + 1] = e end
    end
    g.icons(visible)
    for i, e in ipairs(visible) do
        local it, ry = e.item, y + HEAD + (i - 1) * ROW
        local rect = {x = g.x0 + (x + 2) * k, y = g.y0 + ry * k, w = (w - 4) * k, h = ROW * k}
        local cannot = mode == "merchant" and not T.affordable(e)
        local hot = (g.pointer and g.pointer.inside_window and inside(g.pointer.x, g.pointer.y, rect) and not T.prompt)
            or (g.pad_on and T.pad_list and T.scroll + i == T.pad_row)
        if hot then fill(x + 2, ry, w - 4, ROW, c.menu_hl) end
        if e.texture then
            g.dl:AddImage(e.texture, g.at(x + 4, ry + 1), g.at(x + 19, ry + 16), ImVec2.new(0, 0), ImVec2.new(1, 1),
                cannot and c.dim_icon or c.white)
        end
        local right = mode == "merchant" and (e.price and B.tunar(e.price) or "?") or ((it.quantity or 1) > 1 and ("x" .. it.quantity) or "")
        local rw = g.tw(right) / k
        text(x + 23, ry + 3, it.name, cannot and c.muted or c.text, w - 23 - rw - 14)
        text(x + w - rw - 8, ry + 3, right, cannot and c.muted or (mode == "merchant" and c.junk or c.text))
        T.hits[#T.hits + 1] = {x = rect.x, y = rect.y, w = rect.w, h = rect.h, run = function()
            if mode == "merchant" then T.click_ware(it) else T.click_move(it) end
        end}
    end
    if #entries == 0 then text(x + 6, y + HEAD + 4, mode == "merchant" and "Nothing for sale" or "The bank is empty", c.muted) end
    if #entries > rows and rows > 0 then
        local gh = rows * ROW
        local thumb = math.max(10, gh * rows / #entries)
        fill(x + w - 4, y + HEAD, 2, gh, c.border)
        fill(x + w - 4, y + HEAD + (gh - thumb) * (T.scroll / (#entries - rows)), 2, thumb, c.muted)
    end

    if foot then
        local usable = X.sellable()
        local total, known = 0, #usable > 0
        for _, it in ipairs(usable) do
            local p = price(it, "sell", it.quantity or 1)
            if p then total = total + p else known = false end
        end
        local label = string.format("Sell (%d)%s", #usable, known and (" for " .. B.tunar(total) .. " tunar") or "")
        button(x + 3, y + h - FOOT + 1, w - 6, label, #usable > 0 and c.text or c.muted, #usable > 0 and c.worn or c.border,
            X.sell_selected)
    end

    T.prompt_rect = nil
    local p = T.prompt
    if p then
        local bx, by, bw, bh = x + 6, y + HEAD + 6, w - 12, 76
        T.prompt_rect = {x = g.x0 + bx * k, y = g.y0 + by * k, w = bw * k, h = bh * k}
        fill(bx, by, bw, bh, c.menu_bg)
        frame(bx, by, bw, bh, c.red)
        text(bx + 6, by + 3, VERBS[p.kind] .. " how many?", c.white)
        text(bx + 6, by + 16, p.name, c.muted, bw - 12)
        local sx = bx + 6
        local function step(label, delta)
            local sw = g.tw(label) / k + 10
            button(sx, by + 30, sw, label, c.text, c.border, function() prompt_qty(p, p.qty + delta) end, true)
            sx = sx + sw + 3
        end
        step("-10", -10)
        step("-", -1)
        local shown = (p.text ~= nil and (p.text .. "_")) or tostring(math.floor(p.qty))
        fill(sx, by + 30, 40, 14, c.surface)
        frame(sx, by + 30, 40, 14, c.white)
        text(sx + 4, by + 31, shown, c.text, 34)
        sx = sx + 43
        step("+", 1)
        step("+10", 10)
        button(sx, by + 30, g.tw("Max") / k + 10, "Max", c.text, c.border, function() prompt_qty(p, p.max) end, true)
        local qty = math.floor(p.qty)
        local line
        if p.kind == "buy" or p.kind == "sell" then
            local total = T.prompt_total(p)
            line = total and ("Total " .. B.tunar(total) .. " tunar") or "Total: -"
        else
            line = qty .. " of " .. p.max
        end
        text(bx + 6, by + 48, line, c.text, bw - 12)
        local ok = qty >= 1
        button(bx + 6, by + 60, 66, VERBS[p.kind], ok and c.text or c.muted, ok and c.worn or c.border, T.confirm_prompt, true)
        button(bx + 78, by + 60, 50, "Cancel", c.text, c.border, T.cancel_prompt, true)
        -- the amount typed in is taken from the keyboard only while the question is up (renewed each frame)
        eqoa.input.capture_text(true)
    end
end

return T
