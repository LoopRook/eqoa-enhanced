-- Mail: the inbox and a write form in a window of this mod's own (default Shift+M, or the Start menu's Mail).
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- It sits over play: the character keeps walking, and the game's own Mail windows are exactly as they were with the
-- mod off. Deleting a mail, and sending a mail that carries tunar or an item, are asked of the player by the client
-- ("Are you sure?"): nothing here answers it.
local ui = eqoa.imgui

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, H, PAD, LINE = 360, 336, 8, 13
local LIST_ROWS, BODY_LINES, PICK_ROWS = 8, 6, 6
local KINDS = { inbox = "mail_inbox", write = "mail_compose" }
local LIMITS = { to = 60, subject = 32, body = 500, tunar = 9 }

local state = {
    open = false, tab = "inbox", sessions = {}, tokens = {}, escape = nil, enter = nil,
    sel = nil, scroll = 0, field = nil, result = nil, last_req = nil, await = nil,
    to = "", subject = "", body = "", tunar = "", attach = nil, picking = false, pick_scroll = 0,
    hits = {}, row_hits = {}, top_hits = {}, list_rect = nil, pick_rect = nil, msg_rect = nil, rect = nil, m = nil,
    msg_scroll = 0, was_down = false, fresh = false, deleting = nil, wrapped = nil,
}

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then state.tokens[#state.tokens + 1] = token end
end

local function say(text, ok) state.result = { text = text, ok = ok } end

local function chop(s)
    return s:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
end
-- text is cut by characters, never by bytes (a letter with an accent is two bytes)
local function clen(s) local _, n = s:gsub("[^\128-\191]", ""); return n end
local function limit_chars(s, n) while clen(s) > n do s = chop(s) end return s end
local function drop_first(s) return s:match("^[%z\1-\127\194-\244][\128-\191]*(.*)$") or "" end

-- what the gram client's state says in words
local STATUS = {
    waiting_inbox = "Asking for the inbox...", waiting_message = "Opening the mail...",
    waiting_send = "Sending...", waiting_delete = "Deleting...",
    send_unknown_recipient = "Nobody is called that", send_mailbox_full = "Their mailbox is full",
    send_too_many_unread = "They have too many unread mails", send_mail_busy = "The mail system is busy",
    send_failed = "The mail could not be sent", send_not_delivered = "The mail was not delivered to everyone",
    inbox_failed = "The inbox could not be read", read_failed = "The mail could not be read",
    delete_failed = "The mail could not be deleted",
}

local function run(label, fn)
    local request, err = fn()
    if err then
        state.last_req = nil
        say(err.message or err.code or "Refused", false)
        return nil, err
    end
    state.last_req = request
    state.result = nil
    return request
end

local function carries()
    return (tonumber(state.tunar) or 0) > 0 or state.attach ~= nil
end

local function several_names()
    return state.to:find("[,;]") ~= nil
end

local close, draw, begin_await

local function stop_typing()
    state.field = nil
    if state.enter then eqoa.input.release(state.enter); state.enter = nil end
    eqoa.input.capture_text(false)
end

local function start_typing(field)
    state.field = field
    state.fresh = true
    -- Enter is this window's while a box is typed in (the exact name: another spelling is refused and leaks to the game)
    if not state.enter then state.enter = eqoa.input.claim("Enter", true) end
end

local function open_session(tab)
    local s, err = eqoa.windows.open_session(KINDS[tab], { opened_from = "key" })
    if not s then
        eqoa.log.write("warn", "Mail did not open: " .. tostring(err and err.message))
        return false
    end
    state.sessions[tab] = s.ref
    return true
end

local function switch_tab(tab)
    if tab == state.tab then return end
    local old = state.sessions[state.tab]
    if not open_session(tab) then return end
    if old then eqoa.windows.close_session(old) end
    state.sessions[state.tab] = nil
    stop_typing()
    state.tab, state.result, state.picking = tab, nil, false
end

local function on_confirm(d)
    if not d then return end
    if d.kind == "mail_delete" then
        if d.answer == "yes" then
            say(d.detail and d.detail.sent and "Mail deleted" or ("Not deleted (" .. tostring(d.detail and d.detail.reason) .. ")"),
                d.detail and d.detail.sent == true)
            if state.sel == state.deleting then state.sel = nil end
        else
            say("Kept", true)
        end
    elseif d.kind == "mail_send" then
        if d.answer == "yes" and d.detail and d.detail.sent then
            begin_await()
        elseif d.answer == "yes" then
            say("Not sent (" .. tostring(d.detail and d.detail.reason) .. ")", false)
            state.await = nil
        else
            say("Not sent", true)
            state.await = nil
        end
    end
end

-- A send's answer, read off the gram client's state. The state of an earlier failure stays until the next request, so
-- nothing is judged until the client has shown "sending" or its state is no longer what it was when the mail went;
-- the form is emptied only when the mail really went, and Send stays off until then (never a second send on a guess).
local function watch_send(m)
    local a = state.await
    if not a then return end
    a.frames = a.frames + 1
    if m.status == "waiting_send" then a.seen = true; return end
    local failed = STATUS[m.status] and m.status:sub(1, 7) ~= "waiting"
    if a.seen or m.status ~= a.status0 then
        if failed then
            say(STATUS[m.status], false)
            state.await = nil
            return
        elseif m.status == "idle" then
            say("Mail sent", true)
            state.to, state.subject, state.body, state.tunar, state.attach = "", "", "", "", nil
            state.await = nil
            return
        end
    end
    if a.frames > 600 then
        say("The server has not said whether the mail went: look before sending it again", false)
        state.await = nil
    end
end

begin_await = function()
    local m = state.m
    state.await = { seen = false, frames = 0, status0 = m and m.status or "idle" }
end

local function open(page)
    if state.open then
        if page == "compose" then switch_tab("write") elseif page == "inbox" then switch_tab("inbox") end
        return
    end
    state.tab = page == "compose" and "write" or "inbox"
    if not open_session(state.tab) then return end
    state.open, state.result, state.last_req, state.scroll, state.sel, state.field = true, nil, nil, 0, nil, nil
    listen("render", draw)
    listen("action.result", function(e)
        local r = e.data
        if r and state.last_req ~= nil and r.request_id == state.last_req and r.ok == false then
            say(r.reason or r.code or "Refused", false)
            -- a send the client refused is not waiting for any answer: Send is there again
            state.await = nil
        end
    end)
    listen("confirm.closed", function(e) on_confirm(e.data) end)
    listen("windows.session", function(e)
        local d = e.data
        if d.phase ~= "closed" then return end
        for _, ref in pairs(state.sessions) do
            if ref == d.ref then close(true); return end
        end
    end)
    listen("input.key", function(e)
        local d = e.data
        if not d.down then return end
        local key = d.key:lower()
        if state.field then
            local f = state.field
            if key == "escape" then stop_typing()
            elseif key == "enter" or key == "return" then
                if f == "body" then
                    if clen(state.body) < LIMITS.body then state.body = state.body .. "\n" end
                elseif f == "to" then state.field = "subject"
                elseif f == "subject" then state.field = "body"
                else stop_typing() end
            elseif key == "tab" then
                local has_tunar = state.m and state.m.carries_offer
                state.field = f == "to" and "subject" or (f == "subject" and "body" or (f == "body" and has_tunar and "tunar" or "to"))
            elseif key == "backspace" then state[f] = chop(state[f])
            elseif d.text and #d.text > 0 then
                local t = d.text
                if f == "tunar" then t = t:gsub("%D", "") end
                if f ~= "body" then t = t:gsub("[\r\n]", " ") end
                if clen(state[f]) + clen(t) <= LIMITS[f] then state[f] = state[f] .. t end
            end
        elseif key == "escape" and not d["repeat"] then
            if state.picking then state.picking = false else close() end
        end
    end)
    listen("pointer.pressed", function(e)
        local d = e.data
        if state.picking then
            -- the picker is in front of the Text box: its rows first, and a click inside it is not the box's
            for _, h in ipairs(state.top_hits) do
                if inside(d.x, d.y, h) and h.run then h.run(); return end
            end
            if inside(d.x, d.y, state.pick_rect) then return end
        end
        if d.button ~= "left" then
            if state.field then stop_typing() end
            return
        end
        for _, h in ipairs(state.hits) do
            if inside(d.x, d.y, h) and h.run then h.run(); return end
        end
        if state.field and not inside(d.x, d.y, state.field_rect) then stop_typing() end
        for _, h in ipairs(state.row_hits) do
            if inside(d.x, d.y, h) then h.run(); return end
        end
    end)
    listen("pointer.wheel", function(e)
        local d = e.data
        if (d.steps or 0) == 0 then return end
        local delta = d.steps > 0 and -1 or 1
        local m = state.m
        if not m then return end
        if state.tab == "inbox" and inside(d.x, d.y, state.msg_rect) then
            state.msg_scroll = math.max(0, state.msg_scroll + delta)
        elseif state.picking and inside(d.x, d.y, state.pick_rect) then
            state.pick_scroll = math.max(0, math.min(math.max(0, #m.pack - PICK_ROWS), state.pick_scroll + delta))
        elseif state.tab == "inbox" and inside(d.x, d.y, state.list_rect) then
            state.scroll = math.max(0, math.min(math.max(0, #m.entries - LIST_ROWS), state.scroll + delta))
        end
    end)
    -- Escape is the window's, not the game menu's, while it is up
    state.escape = eqoa.input.claim("Escape", true)
end

function close(from_client)
    if not state.open then return end
    for _, token in ipairs(state.tokens) do eqoa.events.off(token) end
    state.tokens = {}
    if state.escape then eqoa.input.release(state.escape); state.escape = nil end
    if not from_client then
        for _, ref in pairs(state.sessions) do eqoa.windows.close_session(ref) end
    end
    stop_typing()
    state.sessions, state.open, state.await = {}, false, nil
    state.hits, state.row_hits, state.rect, state.m, state.picking = {}, {}, nil, nil, false
end

eqoa.input.bind({ id = "open", label = "Mail", default_key = "shift+m" }, function()
    if state.open then close() else open("inbox") end
end)
eqoa.events.on("mod.command", function(e)
    if e.data.command ~= "open" then return end
    local page = type(e.data.args) == "table" and e.data.args.page or "inbox"
    open(page)
end)
eqoa.events.on("disabling", function() close() end)
eqoa.events.on("world.leaving", function() close() end)
-- the client lets go of every session and claim when the world is left or a new one starts: nothing is kept from before
eqoa.events.on("state.reset", function() close() end)
eqoa.events.on("world.entered", function() close() end)

function draw()
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    local m = eqoa.mail.get()
    state.m = m
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    state.rect = { x = x0, y = y0, w = W * k, h = H * k }
    -- a press anywhere else (another mod's window, the other button) ends the typing: the capture is the player's
    local down = ui.IsMouseDown(0) or ui.IsMouseDown(1)
    if state.field and down and not state.was_down then
        if state.fresh then state.fresh = false else
            local p = eqoa.input.pointer()
            if not (p and inside(p.x, p.y, state.field_rect)) then stop_typing() end
        end
    end
    state.was_down = down
    state.fresh = false
    -- the text capture lapses after about a second: renewed every frame a box is typed in
    if state.field then eqoa.input.capture_text(true) end
    state.top_hits = {}

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_mail_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 11 / 13)
    local dl = ui.GetWindowDrawList()
    local col = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), good = ui.GetColorU32(.35, .8, .4, 1),
        warn = ui.GetColorU32(1, .3, .3, 1), white = ui.GetColorU32(1, 1, 1, 1), hl = ui.GetColorU32(.20, .20, .24, 1),
        mark = ui.GetColorU32(.95, .72, .20, 1),
    }
    local function fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    local function frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    local function text(x, y, value, color, maxw)
        value = tostring(value)
        while maxw and #value > 0 and ui.CalcTextSize(value) > maxw * k do value = chop(value) end
        dl:AddText(at(x, y), color or col.text, value)
    end
    -- the words broken into lines that fit `maxw`, a new line where the text has one; a word longer than a line is
    -- broken by characters
    local function wrap(value, maxw)
        local lines = {}
        local function fits(t) return ui.CalcTextSize(t) <= maxw * k end
        for para in (tostring(value) .. "\n"):gmatch("(.-)\n") do
            local line = ""
            for word in para:gmatch("%S+") do
                while not fits(word) and clen(word) > 1 do
                    -- the longest start of the word that fits (by characters)
                    local head, rest = "", word
                    for ch in word:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
                        if not fits(head .. ch) then break end
                        head = head .. ch
                    end
                    if head == "" then break end
                    if line ~= "" then lines[#lines + 1] = line; line = "" end
                    lines[#lines + 1] = head
                    word = word:sub(#head + 1)
                end
                local try = line == "" and word or (line .. " " .. word)
                if line ~= "" and not fits(try) then lines[#lines + 1] = line; line = word else line = try end
            end
            lines[#lines + 1] = line
        end
        return lines
    end
    local function hit(x, y, w, h, run) return { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k, run = run } end
    local function button(x, y, w, label, enabled, run)
        fill(x, y, w, 16, col.raised)
        frame(x, y, w, 16, enabled and col.border or col.surface)
        text(x + 6, y + 2, label, enabled and col.text or col.muted, w - 8)
        if enabled then state.hits[#state.hits + 1] = hit(x, y, w, 16, run) end
    end

    fill(0, 0, W, H, col.surface)
    frame(0, 0, W, H, col.border)
    text(PAD, 6, "Mail", col.white)
    text(W - PAD - 110, 6, "Shift+M or Esc", col.muted)
    state.hits, state.row_hits = {}, {}
    fill(W - 18, 4, 12, 12, col.raised)
    frame(W - 18, 4, 12, 12, col.border)
    text(W - 15, 4, "x", col.text)
    state.hits[1] = hit(W - 18, 4, 12, 12, function() close() end)

    if not m then
        text(PAD, 40, "Opening...", col.muted)
        eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
        ui.SetWindowFontScale(1)
        ui.End()
        return
    end
    local waiting = m.status:sub(1, 7) == "waiting"
    watch_send(m)

    for i, tab in ipairs({ "inbox", "write" }) do
        local tx = PAD + (i - 1) * 84
        local on = state.tab == tab
        fill(tx, 20, 80, 16, on and col.hl or col.raised)
        frame(tx, 20, 80, 16, on and col.white or col.border)
        text(tx + 8, 22, tab == "inbox" and "Inbox" or "Write", on and col.white or col.muted)
        state.hits[#state.hits + 1] = hit(tx, 20, 80, 16, function() switch_tab(tab) end)
    end
    if m.new then text(W - PAD - 100, 22, "New mail", col.mark) end

    state.list_rect, state.field_rect, state.pick_rect = nil, nil, nil
    local sel_entry
    if state.tab == "inbox" then
        state.list_rect = { x = x0 + PAD * k, y = y0 + 40 * k, w = (W - 2 * PAD) * k, h = LIST_ROWS * LINE * k }
        if #m.entries == 0 then
            text(PAD + 2, 40, waiting and "Asking for the inbox..." or "No mail", col.muted)
        end
        for i = 1, LIST_ROWS do
            local e = m.entries[state.scroll + i]
            if e then
                local y = 40 + (i - 1) * LINE
                if state.sel == e.id then fill(PAD, y, W - 2 * PAD, LINE, col.hl) end
                text(PAD + 2, y, e.carries and "+" or "", col.mark)
                text(PAD + 10, y, e.from, e.read and col.muted or col.white, 70)
                text(PAD + 84, y, e.subject, e.read and col.muted or col.text, 170)
                text(PAD + 262, y, e.date:sub(1, 10), col.muted)
                state.row_hits[#state.row_hits + 1] = hit(PAD, y, W - 2 * PAD, LINE, function()
                    if state.sel ~= e.id then state.msg_scroll = 0 end
                    state.sel = e.id
                    if not waiting then run("Read", function() return eqoa.actions.mail_read(e.id) end) end
                end)
            end
            if e and e.id == state.sel then sel_entry = e end
        end
        if m.truncated then text(PAD, 40 + LIST_ROWS * LINE, "The server lists more than the game keeps (100)", col.muted, W - 2 * PAD) end
        -- the mail opened
        local py = 40 + LIST_ROWS * LINE + 14
        local msg = m.message
        if state.sel and msg and msg.id == state.sel then
            text(PAD, py, "From " .. msg.from .. "   " .. msg.date, col.muted, W - 2 * PAD)
            text(PAD, py + LINE, msg.subject, col.white, W - 2 * PAD)
            -- wrapped once for a mail (its text and the window's scale), not every frame
            local key = msg.id .. ":" .. #msg.text .. ":" .. k
            if not state.wrapped or state.wrapped.key ~= key then
                state.wrapped = { key = key, lines = wrap(msg.text, W - 2 * PAD) }
            end
            local lines = state.wrapped.lines
            state.msg_scroll = math.min(state.msg_scroll, math.max(0, #lines - BODY_LINES))
            state.msg_rect = { x = x0 + PAD * k, y = y0 + (py + 2 * LINE) * k, w = (W - 2 * PAD) * k, h = BODY_LINES * LINE * k }
            for i = 1, BODY_LINES do
                local l = lines[state.msg_scroll + i]
                if l then text(PAD, py + (i + 1) * LINE, l, col.text, W - 2 * PAD) end
            end
            if #lines > BODY_LINES then
                text(W - PAD - 150, py + (BODY_LINES + 2) * LINE, string.format("line %d-%d of %d (wheel)", state.msg_scroll + 1,
                    math.min(#lines, state.msg_scroll + BODY_LINES), #lines), col.muted, 150)
            end
            local what = {}
            if msg.tunar > 0 then what[#what + 1] = msg.tunar .. " tunar" end
            for _, it in ipairs(msg.items) do what[#what + 1] = it end
            if #what > 0 then
                text(PAD, py + (BODY_LINES + 3) * LINE, "Carries: " .. table.concat(what, ", "), col.mark, W - 2 * PAD)
            end
        elseif state.sel then
            text(PAD, py, waiting and "Opening the mail..." or "Click the mail again to read it", col.muted)
        else
            text(PAD, py, "Click a mail to read it. A + marks a mail that carries something.", col.muted, W - 2 * PAD)
        end
        local by = H - 40
        local msg_ok = state.sel and msg and msg.id == state.sel
        button(PAD, by, 64, "Refresh", not waiting, function()
            run("Refresh", function() return eqoa.actions.mail_refresh() end)
        end)
        button(PAD + 68, by, 52, "Reply", msg_ok, function()
            state.to = msg.from
            state.subject = (msg.subject:sub(1, 3):lower() == "re:" and "" or "Re: ") .. msg.subject
            state.subject = limit_chars(state.subject, LIMITS.subject)
            state.body, state.tunar, state.attach = "", "", nil
            switch_tab("write")
        end)
        local has_load = msg_ok and (msg.tunar > 0 or #msg.items > 0)
        button(PAD + 124, by, 96, "Take contents", has_load and not waiting, function()
            run("Take", function() return eqoa.actions.mail_take(msg.id) end)
        end)
        button(PAD + 224, by, 56, "Delete", state.sel ~= nil and not waiting, function()
            state.deleting = state.sel
            run("Delete", function() return eqoa.actions.mail_delete(state.sel) end)
        end)
    else
        -- the write form
        local function box(y, label, field, h)
            text(PAD, y, label, col.muted)
            local by = y + 11
            fill(PAD, by, W - 2 * PAD, h, col.raised)
            frame(PAD, by, W - 2 * PAD, h, state.field == field and col.white or col.border)
            local value = state[field]
            if field == "body" then
                local lines = wrap(value .. (state.field == field and "_" or ""), W - 2 * PAD - 8)
                local first = math.max(1, #lines - math.floor(h / LINE) + 1)
                for i = first, #lines do
                    text(PAD + 4, by + 1 + (i - first) * LINE, lines[i], col.text, W - 2 * PAD - 8)
                end
            else
                local shown = value .. (state.field == field and "_" or "")
                -- the end of a long line is the part typed
                while #shown > 0 and ui.CalcTextSize(shown) > (W - 2 * PAD - 8) * k do shown = drop_first(shown) end
                text(PAD + 4, by + 1, shown, col.text)
            end
            state.hits[#state.hits + 1] = hit(PAD, by, W - 2 * PAD, h, function()
                start_typing(field); state.picking = false
            end)
            if state.field == field then
                state.field_rect = { x = x0 + PAD * k, y = y0 + by * k, w = (W - 2 * PAD) * k, h = h * k }
            end
        end
        box(40, "To (one name)", "to", 14)
        box(68, "Subject", "subject", 14)
        box(96, "Text", "body", 6 * LINE + 2)
        local ay = 96 + 11 + 6 * LINE + 8
        if m.carries_offer then
            text(PAD, ay, "Tunar", col.muted)
            fill(PAD + 40, ay - 2, 80, 14, col.raised)
            frame(PAD + 40, ay - 2, 80, 14, state.field == "tunar" and col.white or col.border)
            text(PAD + 44, ay, state.tunar .. (state.field == "tunar" and "_" or ""), col.text, 72)
            state.hits[#state.hits + 1] = hit(PAD + 40, ay - 2, 80, 14, function() start_typing("tunar"); state.picking = false end)
            if state.field == "tunar" then
                state.field_rect = { x = x0 + (PAD + 40) * k, y = y0 + (ay - 2) * k, w = 80 * k, h = 14 * k }
            end
            text(PAD + 128, ay, "You carry " .. m.tunar, col.muted, 90)
            -- one item may go along
            local label = state.attach and (state.attach.name .. " x" .. state.attach.count) or "Attach an item"
            button(PAD + 220, ay - 3, W - 2 * PAD - 220, label, m.max_items > 0, function()
                stop_typing(); state.picking = not state.picking; state.pick_scroll = 0
            end)
            if state.attach then
                button(PAD + 220, ay + 15, 30, "-", state.attach.count > 1, function() state.attach.count = state.attach.count - 1 end)
                button(PAD + 254, ay + 15, 30, "+", state.attach.count < state.attach.max, function() state.attach.count = state.attach.count + 1 end)
                button(PAD + 288, ay + 15, 56, "Remove", true, function() state.attach = nil end)
            end
            if m.postage > 0 and carries() then text(PAD, ay + 17, "Postage " .. m.postage .. " tunar", col.muted, 200) end
        else
            text(PAD, ay, "This server's mail carries only text.", col.muted, W - 2 * PAD)
        end
        local by = H - 40
        local ok_to_send = not waiting and state.to ~= "" and state.subject ~= "" and state.body ~= ""
            and not (carries() and several_names()) and state.await == nil
        button(PAD, by, 64, "Send", ok_to_send, function()
            stop_typing()
            local item
            if state.attach then item = { slot = state.attach.slot, id = state.attach.id, count = state.attach.count } end
            local tunar = tonumber(state.tunar) or 0
            local req = run("Send", function()
                return eqoa.actions.mail_send({ to = state.to, subject = state.subject, body = state.body,
                    tunar = tunar > 0 and tunar or nil, item = item })
            end)
            if req and not carries() then begin_await()
            elseif req then say("Answer the question on the screen", nil) end
        end)
        button(PAD + 68, by, 52, "Clear", true, function()
            state.to, state.subject, state.body, state.tunar, state.attach = "", "", "", "", nil
        end)
        if carries() and several_names() then
            text(PAD + 128, by + 2, "A mail that carries something goes to one name", col.warn, W - PAD - 134)
        end
        -- the item picker
        if state.picking then
            local pw, ph = W - 2 * PAD - 100, PICK_ROWS * LINE + 8
            local px, py = PAD + 100, ay - ph - 4
            fill(px, py, pw, ph, col.raised)
            frame(px, py, pw, ph, col.white)
            state.pick_rect = { x = x0 + px * k, y = y0 + py * k, w = pw * k, h = ph * k }
            if #m.pack == 0 then text(px + 4, py + 4, "Nothing in your pack can be mailed", col.muted, pw - 8) end
            for i = 1, PICK_ROWS do
                local it = m.pack[state.pick_scroll + i]
                if it then
                    local y = py + 4 + (i - 1) * LINE
                    text(px + 4, y, it.name .. (it.quantity > 1 and (" x" .. it.quantity) or ""), col.text, pw - 8)
                    state.top_hits[#state.top_hits + 1] = hit(px, y, pw, LINE, function()
                        state.attach = { slot = it.slot, id = it.id, name = it.name, count = 1, max = it.quantity }
                        state.picking = false
                    end)
                end
            end
        end
    end

    -- what the client says about the mail, or why a click was refused
    local line = H - 20
    if state.result then
        text(PAD, line, state.result.text, state.result.ok == false and col.warn or col.muted, W - 2 * PAD)
    elseif state.await then
        text(PAD, line, "Sending...", col.muted)
    elseif STATUS[m.status] then
        text(PAD, line, STATUS[m.status], waiting and col.muted or col.warn, W - 2 * PAD)
    end

    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    ui.SetWindowFontScale(1)
    ui.End()
end
