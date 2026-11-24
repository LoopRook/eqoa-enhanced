-- EQOA Trade: the trade request, the trade window, duel invitations and the duel wager, over the A9 API.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local ui = eqoa.imgui

local T = {}

local PROMPT_W, WIN_W = 330, 430
local ROWS = 12
local KINDS = {"trade_ask", "trade", "duel_invite", "duel"}

T.state = {
    claimed = false,
    status = nil, -- {text, ok}
    notice = nil, -- {text, until}
    money_text = nil, -- what is typed in the coin box (nil: not being edited)
    typing = false,
    enter = nil, -- the claim on Enter, held only while the coin box has the keyboard
    capturing = false,
    rclick = nil, -- a right-click not yet used by a bag row: {shift = bool}
    last_req = nil,
}
local S = T.state

local function short(text, max)
    text = tostring(text or "")
    if #text <= max then return text end
    return text:sub(1, max - 2) .. ".."
end

local function say(text, ok) S.status = {text = tostring(text), ok = ok} end

local function coins(n) return string.format("%d coin%s", n or 0, n == 1 and "" or "s") end

-- ---------------------------------------------------------------- holding the windows

local function claim()
    for _, kind in ipairs(KINDS) do
        local ok, err = eqoa.windows.takeover(kind, {scope = "all"})
        if not ok and err and err.code ~= "unavailable" then
            eqoa.log.write("warn", "Trade did not take over " .. kind .. ": " .. tostring(err.message))
        end
    end
    S.claimed = true
end

local function release()
    for _, kind in ipairs(KINDS) do eqoa.windows.release(kind) end
    S.claimed = false
end

-- ---------------------------------------------------------------- the keyboard (the coin box)

-- The text box keeps the keyboard only while it is the item in use, and the game's text capture lapses after about a
-- second unless it is asked for again: asked every frame the box is wanted, let go the first frame it is not.
local function set_typing(on)
    on = on == true
    if on then
        eqoa.input.capture_text(true)
        S.capturing = true
        if not S.enter then S.enter = eqoa.input.claim("Enter", true) end
    else
        if S.capturing then
            eqoa.input.capture_text(false)
            S.capturing = false
        end
        if S.enter then
            eqoa.input.release(S.enter)
            S.enter = nil
        end
    end
    S.typing = on
end

T.set_typing = set_typing

-- ---------------------------------------------------------------- the actions

local function run(label, fn)
    local req, err = fn()
    if err then
        S.last_req = nil
        say(err.message or err.code or "Refused", false)
    elseif req then
        S.last_req = req
        say(label, nil)
    end
end

function T.offer_row(t, row, whole)
    if not row or not row.can_offer then return end
    local count = whole and row.quantity or 1
    run("Offering " .. short(row.name, 20), function()
        return eqoa.actions.trade_add({slot = row.slot, id = row.id, count = count})
    end)
end

function T.take_back(row)
    run("Taking back " .. short(row.name, 20), function()
        return eqoa.actions.trade_remove({slot = row.slot, id = row.id})
    end)
end

function T.set_money(text)
    local n = tonumber(((text or ""):gsub("[^%d]", "")))
    if not n then n = 0 end
    run("Offering " .. coins(n), function() return eqoa.actions.trade_money(n) end)
end

-- ---------------------------------------------------------------- the request prompt (trade and duel)

function T.answer_trade(yes)
    run(yes and "Trade accepted" or "Trade declined", function() return eqoa.actions.trade_answer(yes) end)
end

function T.answer_duel(yes)
    run(yes and "Duel accepted" or "Duel declined", function() return eqoa.actions.duel_answer(yes) end)
end

-- Accept: the client asks "You give X, you get Y. Trade?" (starting on No); nothing changes hands before the Yes
function T.accept()
    run("Waiting for your Yes", function() return eqoa.actions.trade_accept() end)
end

function T.unaccept()
    run("Accept taken back", function() return eqoa.actions.trade_unaccept() end)
end

function T.cancel()
    run("Cancelled", function() return eqoa.actions.trade_cancel() end)
end

local function seconds(x) return math.max(0, math.ceil(x or 0)) end

local function draw_prompt(t, d)
    local trade_req = t and t.phase == "request" and t.held and eqoa.windows.session("trade_ask")
    local invite = d and d.state == "invited" and d.invite and eqoa.windows.session("duel_invite") and d.invite
    if not trade_req and not invite then return end
    local dw = ui.GetDisplaySize()
    if dw <= 0 then return end
    ui.SetNextWindowPos((dw - PROMPT_W) * 0.5, 64, ImGuiCond.FirstUseEver)
    ui.SetNextWindowSize(PROMPT_W, 0, ImGuiCond.Always)
    if ui.Begin("Request##eqoa_trade_prompt", nil,
        ImGuiWindowFlags.NoCollapse + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.AlwaysAutoResize) then
        if trade_req then
            ui.TextWrapped(short(t.partner.name, 24) .. " wants to trade with you.")
            ui.TextDisabled(string.format("Refused automatically in %d s", seconds(t.seconds_remaining)))
            if ui.Button("Accept##trade_yes") then T.answer_trade(true) end
            ui.SameLine()
            if ui.Button("Decline##trade_no") then T.answer_trade(false) end
            ui.SameLine()
            if ui.Button("Show in game##trade_pass") then eqoa.windows.pass(eqoa.windows.session("trade_ask").ref) end
        end
        if trade_req and invite then ui.Separator() end
        if invite then
            ui.TextWrapped(string.format("%s (level %d %s) challenges you to a duel.", short(invite.name, 24),
                invite.level or 0, invite.class_name or ""))
            ui.TextDisabled(string.format("Declined automatically in %d s", seconds(d.seconds_remaining)))
            if ui.Button("Accept##duel_yes") then T.answer_duel(true) end
            ui.SameLine()
            if ui.Button("Decline##duel_no") then T.answer_duel(false) end
            ui.SameLine()
            if ui.Button("Show in game##duel_pass") then eqoa.windows.pass(eqoa.windows.session("duel_invite").ref) end
        end
    end
    ui.End()
end

-- ---------------------------------------------------------------- the trade window (a duel's wager is the same window)

local function item_line(it) return string.format("%s x%d", short(it.name, 24), it.quantity or 1) end

local function tip(it)
    if ui.IsItemHovered() then ui.SetTooltip(string.format("%s\nQuantity: %d", it.name, it.quantity or 1)) end
end

local function draw_trade(t)
    if not (t and t.phase == "open" and t.held) then
        set_typing(false)
        S.money_text = nil
        return
    end
    local kind_id = t.kind == "duel" and "duel" or "trade"
    local sess = eqoa.windows.session(kind_id)
    if not sess then
        set_typing(false)
        return
    end
    local dw, dh = ui.GetDisplaySize()
    if dw <= 0 then return end
    ui.SetNextWindowPos(math.max(0, (dw - WIN_W) * 0.5), math.max(0, dh * 0.2), ImGuiCond.FirstUseEver)
    ui.SetNextWindowSize(WIN_W, 0, ImGuiCond.Always)
    local title = (t.kind == "duel" and "Duel wager with " or "Trade with ") .. short(t.partner.name, 20)
    local typing_now = T.force_typing == true
    if ui.Begin(title .. "##eqoa_trade_window", nil,
        ImGuiWindowFlags.NoCollapse + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.AlwaysAutoResize) then
        -- the player's side
        ui.Text("You offer" .. (t.mine.accepted and "  (accepted)" or ""))
        if #t.mine.items == 0 then ui.TextDisabled("  nothing yet") end
        for _, it in ipairs(t.mine.items) do
            ui.Text("  " .. item_line(it))
            tip(it)
            ui.SameLine()
            if ui.Button("Take back##mine_" .. it.slot) then T.take_back(it) end
        end
        ui.Text("  Coins: " .. (t.mine.money or 0))
        ui.SameLine()
        ui.SetNextItemWidth(110)
        local shown = S.money_text or tostring(t.mine.money or 0)
        local val, changed = ui.InputTextWithHint("##coins", "coins", shown, 0)
        if changed then S.money_text = val:gsub("[^%d]", ""):sub(1, 9) end
        if ui.IsItemActive() then typing_now = true end
        ui.SameLine()
        if ui.Button("Set##coins_set") then
            T.set_money(S.money_text or shown)
            S.money_text = nil
        end
        ui.TextDisabled(string.format("  You carry %s", coins(t.carried)))
        ui.Separator()
        -- the other side
        ui.Text(short(t.partner.name, 20) .. " offers" .. (t.theirs.accepted and "  (accepted)" or ""))
        if #t.theirs.items == 0 then ui.TextDisabled("  nothing yet") end
        for _, it in ipairs(t.theirs.items) do
            ui.Text("  " .. item_line(it))
            tip(it)
        end
        ui.Text("  Coins: " .. (t.theirs.money or 0))
        ui.Separator()
        -- the bags: a right-click offers one (Shift+right-click the whole stack)
        ui.Text("Your items (right-click to offer; Shift+right-click for all)")
        if ui.BeginChild("##bags", 0, 110, ImGuiChildFlags.Border) then
            local shown_rows = 0
            for _, b in ipairs(t.bags or {}) do
                if b.can_offer then
                    shown_rows = shown_rows + 1
                    ui.Selectable(item_line(b) .. "##bag_" .. b.slot, false)
                    tip(b)
                    if S.rclick and ui.IsItemHovered() then
                        T.offer_row(t, b, S.rclick.shift)
                        S.rclick = nil
                    end
                end
            end
            if shown_rows == 0 then ui.TextDisabled("nothing to offer") end
        end
        ui.EndChild()
        ui.Separator()
        if t.mine.accepted then
            if ui.Button("Take back accept##unaccept") then T.unaccept() end
        else
            if ui.Button("Accept##accept") then T.accept() end
        end
        ui.SameLine()
        if ui.Button("Cancel##cancel") then T.cancel() end
        ui.SameLine()
        if ui.Button("Show in game##show") then eqoa.windows.pass(sess.ref) end
        if t.accept_asked then ui.TextWrapped("Answer the question on the screen.") end
        if S.status then ui.TextWrapped(S.status.text) end
    end
    ui.End()
    -- the coin box was left: what was typed is not kept
    if not typing_now and S.typing then S.money_text = nil end
    set_typing(typing_now)
end

local function draw_notice()
    if not S.notice then return end
    if os.clock() > S.notice.finish then
        S.notice = nil
        return
    end
    local dw = ui.GetDisplaySize()
    if dw <= 0 then return end
    ui.SetNextWindowPos((dw - PROMPT_W) * 0.5, 24, ImGuiCond.FirstUseEver)
    ui.SetNextWindowSize(PROMPT_W, 0, ImGuiCond.Always)
    if ui.Begin("Trade##eqoa_trade_notice", nil,
        ImGuiWindowFlags.NoCollapse + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.AlwaysAutoResize) then
        ui.TextWrapped(S.notice.text)
    end
    ui.End()
end

local function on_render()
    local c = eqoa.client.get()
    if not c or not c.in_world then
        set_typing(false)
        return
    end
    if not S.claimed then claim() end
    local t = eqoa.trade.get()
    local d = eqoa.duel.get()
    draw_prompt(t, d)
    draw_trade(t)
    draw_notice()
    S.rclick = nil
end

-- ---------------------------------------------------------------- events

local CLOSED = {
    completed = "Trade completed",
    cancelled = "Trade cancelled",
    closed = "The wager window closed",
}

function T.start()
    eqoa.events.on("render", on_render)
    eqoa.events.on("world.entered", function() claim() end)
    eqoa.events.on("state.reset", function() S.claimed = false end)
    eqoa.events.on("world.leaving", function()
        set_typing(false)
        S.claimed = false
        S.money_text = nil
    end)
    eqoa.events.on("pointer.pressed", function(e)
        local d = e.data
        if d.button == "right" then S.rclick = {shift = d.shift == true} end
    end)
    -- Enter in the coin box sets the coins (it is claimed only while the box has the keyboard)
    eqoa.events.on("input.key", function(e)
        local d = e.data
        if S.typing and d.down and not d["repeat"] and tostring(d.key):lower() == "enter" and S.money_text then
            T.set_money(S.money_text)
            S.money_text = nil
        end
    end)
    eqoa.events.on("trade.closed", function(e)
        local d = e.data or {}
        local text = CLOSED[d.reason] or ("Trade ended (" .. tostring(d.reason) .. ")")
        if d.kind == "duel" and d.reason == "completed" then text = "Duel wager accepted by both" end
        S.notice = {text = text, finish = os.clock() + 5}
        S.status = nil
        S.money_text = nil
    end)
    eqoa.events.on("action.result", function(e)
        local d = e.data
        if not d or S.last_req == nil or d.request_id ~= S.last_req then return end
        if d.ok == false then say(d.reason or d.code or "Refused", false) end
    end)
    eqoa.events.on("confirm.closed", function(e)
        local d = e.data
        if not d or (d.kind ~= "trade_accept" and d.kind ~= "duel_accept") then return end
        if d.answer == "yes" then
            local sent = d.detail and d.detail.sent
            say(sent and "Accepted: waiting for the other side" or ("Not sent: " .. tostring(d.detail and d.detail.reason)), sent == true)
        elseif d.answer == "changed" then
            say("The offer changed: nothing was accepted", nil)
        else
            say("Not accepted", nil)
        end
    end)
    eqoa.events.on("trade.accepted", function(e)
        local d = e.data or {}
        if d.side == "changed" then say("The offer changed: your accept was taken back", nil) end
    end)
    eqoa.events.on("disabling", function()
        set_typing(false)
        release()
    end)
end

return T
