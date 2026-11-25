-- EQOA Trade: the trade request, the trade window, duel invitations and the duel wager, over the A9 API.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local ui = eqoa.imgui

local T = {}

local PROMPT_W, WIN_W = 330, 430
local KINDS = {"trade_ask", "trade", "duel_invite", "duel"}
-- the claims end with the world and with the player's "Use the game's windows" switch: held again every so often
local CLAIM_EVERY = 120

T.state = {
    claimed = false,
    frames = 0,
    warned = {},
    status = nil, -- {text, ok}: what the player is told in the window
    prompt_status = nil, -- the same, for the request box
    notice = nil, -- {text, finish}
    money_text = nil, -- what is typed in the coin box (nil: nothing typed)
    typing = false,
    enter = nil, -- the claim on Enter, held only while the coin box has the keyboard
    capturing = false,
    last_req = nil,
    last_money_sent = nil, -- the amount the last coin offer carried (what a test reads)
    just_sent = nil, -- the text of the last coin offer: the box writes it back when Enter validates it, and it is not typed
    last_offer = nil, -- {slot, id, at}: the last item offered, against a double click that lands on the next row
}
local S = T.state

local function short(text, max)
    text = tostring(text or "")
    if #text <= max then return text end
    return text:sub(1, max - 2) .. ".."
end

local function say(text, ok) S.status = {text = tostring(text), ok = ok} end
local function say_prompt(text, ok) S.prompt_status = {text = tostring(text), ok = ok} end

local function tunar(n) return string.format("%d tunar", n or 0) end

-- ---------------------------------------------------------------- holding the windows

local function claim()
    for _, kind in ipairs(KINDS) do
        local ok, err = eqoa.windows.takeover(kind, {scope = "all"})
        if not ok and err and err.code ~= "unavailable" and not S.warned[kind] then
            -- once: a kind another mod holds is retried quietly every 120 frames
            S.warned[kind] = true
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

local function run(label, fn, prompt)
    local req, err = fn()
    if err then
        S.last_req = nil
        local text = err.message or err.code or "Refused"
        if prompt then say_prompt(text, false) else say(text, false) end
    elseif req then
        S.last_req = req
        if prompt then say_prompt(label, nil) else say(label, nil) end
    end
end

function T.offer_row(row, whole)
    if not row or not row.can_offer then return end
    -- a second click within a moment on another row is the list moving up under the first (its echo came between the
    -- clicks), not a second wish: ignored. The same row again (a second one of a stack) is allowed.
    local now = os.clock()
    local last = S.last_offer
    if last and now - last.at < 0.4 and (last.slot ~= row.slot or last.id ~= row.id) then return end
    S.last_offer = {slot = row.slot, id = row.id, at = now}
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

-- The coins typed in the box (digits only), offered as the whole amount. Set and Enter both come here.
function T.set_money(text)
    local digits = ((text or ""):gsub("[^%d]", ""):sub(1, 9))
    -- nothing typed offers nothing (an emptied box and Enter is not an offer of 0: Take back / Set 0 are)
    if digits == "" then return end
    local n = tonumber(digits)
    S.just_sent = text
    S.last_money_sent = n
    run("Offering " .. tunar(n), function() return eqoa.actions.trade_money(n) end)
end

function T.answer_trade(yes)
    run(yes and "Request accepted" or "Request declined", function() return eqoa.actions.trade_answer(yes) end, true)
end

function T.answer_duel(yes)
    run(yes and "Request accepted" or "Request declined", function() return eqoa.actions.duel_answer(yes) end, true)
end

-- Accept: the client asks the question (starting on No); nothing changes hands before the Yes
function T.accept()
    run("Waiting for your Yes", function() return eqoa.actions.trade_accept() end)
end

function T.unaccept()
    run("Accept taken back", function() return eqoa.actions.trade_unaccept() end)
end

function T.cancel()
    run("Cancelled", function() return eqoa.actions.trade_cancel() end)
end

-- ---------------------------------------------------------------- the request prompt (trade and duel)

local function seconds(x) return math.max(0, math.ceil(x or 0)) end

local function draw_prompt(t, d)
    local trade_req = t and t.phase == "request" and t.held and eqoa.windows.session("trade_ask")
    local invite = d and d.state == "invited" and d.invite and eqoa.windows.session("duel_invite") and d.invite
    if not trade_req and not invite then
        S.prompt_status = nil
        return
    end
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
        if S.prompt_status then ui.TextWrapped(S.prompt_status.text) end
    end
    ui.End()
end

-- ---------------------------------------------------------------- the trade window (a duel's wager is the same window)

local function item_line(it) return string.format("%s x%d", short(it.name, 24), it.quantity or 1) end

-- the item as the game's own windows describe it (its lines and stat bonuses), else its name and quantity
local function tip(it)
    if ui.IsItemHovered() then
        local text = it.text
        if type(text) ~= "string" or text == "" then text = string.format("%s\nQuantity: %d", it.name, it.quantity or 1) end
        ui.SetTooltip(text)
    end
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
        ui.Text("  Tunar: " .. (t.mine.money or 0))
        ui.SameLine()
        ui.SetNextItemWidth(110)
        -- the text typed stays until Set, Enter or the end of the trade: a click on Set takes the box's focus a frame
        -- before the button fires, so it must not be dropped when the box is left
        local val, changed = ui.InputTextWithHint("##coins", tostring(t.mine.money or 0), S.money_text or "", 0)
        if changed then
            -- the box writes its text back when Enter validates it: what was just offered is not typed again
            if val == S.just_sent then
                S.money_text = nil
            else
                S.just_sent = nil
                S.money_text = val:gsub("[^%d]", ""):sub(1, 9)
            end
        end
        if ui.IsItemActive() then typing_now = true end
        ui.SameLine()
        if ui.Button("Set##coins_set") then
            if S.money_text and S.money_text ~= "" then T.set_money(S.money_text) end
            S.money_text = nil
        end
        ui.TextDisabled(string.format("  You carry %s", tunar(t.carried)))
        ui.Separator()
        -- the other side
        ui.Text(short(t.partner.name, 20) .. " offers" .. (t.theirs.accepted and "  (accepted)" or ""))
        if #t.theirs.items == 0 then ui.TextDisabled("  nothing yet") end
        for _, it in ipairs(t.theirs.items) do
            ui.Text("  " .. item_line(it))
            tip(it)
        end
        ui.Text("  Tunar: " .. (t.theirs.money or 0))
        ui.Separator()
        -- the bags: Offer (left-click) offers one, All the whole stack; a right-click on a row offers one too. All of
        -- these are this window's own, so the window in front takes the click wherever other mod windows overlap
        ui.Text("Your items (right-click a row, or Offer)")
        if ui.BeginChild("##bags", 0, 110, ImGuiChildFlags.Border) then
            local shown_rows = 0
            for _, b in ipairs(t.bags or {}) do
                if b.can_offer then
                    shown_rows = shown_rows + 1
                    -- a width that ends before the buttons: a full-line Selectable would take the hover from them
                    ui.Selectable(item_line(b) .. "##bag_" .. b.slot, false, 0, 230, 0)
                    tip(b)
                    if ui.IsItemHovered() and ui.IsMouseReleased(1) then T.offer_row(b, false) end
                    ui.SameLine()
                    if ui.Button("Offer##offer_" .. b.slot) then T.offer_row(b, false) end
                    if (b.quantity or 1) > 1 then
                        ui.SameLine()
                        if ui.Button("All##all_" .. b.slot) then T.offer_row(b, true) end
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
    S.frames = S.frames + 1
    if not S.claimed or S.frames % CLAIM_EVERY == 0 then claim() end
    local t = eqoa.trade.get()
    local d = eqoa.duel.get()
    draw_prompt(t, d)
    draw_trade(t)
    draw_notice()
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
        if d.ok == false then
            say(d.reason or d.code or "Refused", false)
            say_prompt(d.reason or d.code or "Refused", false)
        end
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
