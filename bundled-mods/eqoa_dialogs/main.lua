-- Dialog window: NPC dialogs (and, if you like, group invitations, summons and resurrection offers) in a window of
-- the mod's own, in place of the game's. Pressing Escape or the x gives the dialog back to the game's own window.
-- An answer that names a quest in your journal is marked and has a Hand in button; with the option on, a dialog whose
-- only real answer is such a hand-in is answered by itself.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local ui = eqoa.imgui
local match = require("match")

local DIALOG_KINDS = { "response_dialog", "text_dialog" }
local OFFER_KINDS = { "group_invite", "summon_offer", "revive_offer" }
local OFFER_TITLES = { group_invite = "Group invitation", summon_offer = "Summons", revive_offer = "Resurrection offer" }
local W, PAD, LINE, PAGE = 360, 8, 13, 6
local OFFER_W = 300
local RECLAIM_FRAMES, RETRY_FRAMES = 120, 30
local DIGITS = { "1", "2", "3", "4", "5", "6", "7", "8", "9" }

eqoa.options.toggle({ id = "offers", label = "Show invitations, summons and resurrection offers", default = true })
eqoa.options.toggle({ id = "highlight", label = "Highlight quest hand-ins", default = true })
eqoa.options.toggle({ id = "auto_answer", label = "Answer quest hand-ins automatically", default = false })

-- Never a real answer: the game's own paging entries, and a plain farewell (leaving is not a choice to make for the
-- player, so a hand-in offered beside Goodbye is still the dialog's only real answer).
local NOT_ANSWERS = { more = true, back = true, close = true, goodbye = true, ["good bye"] = true, farewell = true, bye = true }
local HAND_W = 56

local state = {
    frame = 0,
    tell = {},          -- kind -> last claim outcome told in the log
    claimed = {},       -- key name -> input claim token
    warned = {},        -- key name -> a refused claim was already told in the log
    page = 0, page_ref = nil,
    hits = {}, keys = {}, result = nil, retry = {},
    offers_on = true,
    auto_ref = nil,     -- ref of the dialog the automatic hand-in already judged
}

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function note(level, text) eqoa.log.write(level, "Dialog window: " .. text) end

local function say(text, ok) state.result = { text = text, ok = ok, at = state.frame } end

local function claim_kind(kind)
    local ok, err = eqoa.windows.takeover(kind, { scope = "all" })
    local told = ok and "held" or (err and err.code or "refused")
    if state.tell[kind] ~= told then
        state.tell[kind] = told
        if ok then
            note("info", "holding " .. kind)
        else
            note("warn", "cannot hold " .. kind .. ": " .. tostring(err and err.message or "refused"))
        end
    end
end

-- Held again every so often: a claim ends on zoning and with the player's "Use the game's windows" switch.
local function claim()
    for _, kind in ipairs(DIALOG_KINDS) do claim_kind(kind) end
    local want = eqoa.options.get("offers") ~= false
    for _, kind in ipairs(OFFER_KINDS) do
        if want then
            claim_kind(kind)
        elseif state.tell[kind] == "held" then
            eqoa.windows.release(kind)
            state.tell[kind] = "released"
            note("info", "no longer holding " .. kind)
        end
    end
end

-- One answer, as the game's own keys would give it. The result is "sent", never "confirmed".
local function answer(ref, option, label)
    local request, err = eqoa.actions.dialog_answer(ref, option)
    if err then
        say(err.message or err.code or "Refused", false)
        note("warn", "the answer was refused: " .. tostring(err.code))
        return
    end
    say(label .. " sent", nil)
    return request
end

local function pass(ref)
    local ok, err = eqoa.windows.pass(ref)
    if ok then
        say("Back in the game's window", nil)
    else
        say(err and err.message or "Could not hand the dialog back yet", false)
        note("debug", "pass refused: " .. tostring(err and err.code))
    end
end

-- The answers that name a quest in the journal: option index -> quest (the option's words only, never its position).
local function hand_ins(options)
    local found = {}
    for _, m in ipairs(match.matches(options, eqoa.quests.list() or {})) do found[m.index] = m.quest end
    return found
end

-- The one real answer of a dialog: the only option that is not the game's own More / Back / Close or a farewell.
-- nil when the dialog offers a choice (a path to pick, say) or nothing.
local function only_answer(options)
    local index
    for i, text in ipairs(options) do
        if not NOT_ANSWERS[match.norm(text)] then
            if index then return nil end
            index = i
        end
    end
    return index
end

-- Answers a hand-in by itself, once per dialog, only when it is the dialog's single real answer.
local function auto_hand_in(s)
    if state.auto_ref == s.ref then return end
    state.auto_ref = s.ref
    if s.kind ~= "response_dialog" or not eqoa.options.get("auto_answer") then return end
    local options = (s.data or {}).options or {}
    local index = only_answer(options)
    if not index then return end
    local quest = hand_ins(options)[index]
    if not quest then return end
    note("info", "handing in " .. tostring(quest.title) .. " automatically")
    answer(s.ref, index, "Hand in " .. tostring(quest.title))
end

local function wrap(text, maxw, k)
    local lines = {}
    for para in (tostring(text) .. "\n"):gmatch("(.-)\n") do
        local line = ""
        for word in para:gmatch("%S+") do
            local try = line == "" and word or (line .. " " .. word)
            if line ~= "" and ui.CalcTextSize(try) / k > maxw then
                lines[#lines + 1] = line
                line = word
            else
                line = try
            end
        end
        lines[#lines + 1] = line
    end
    return lines
end

-- What the player's keys do while a window is up: Escape gives the dialog back to the game; 1 to 9 pick an answer.
local function press(key)
    local run = state.keys[key]
    if run then run() end
end

eqoa.events.on("input.key", function(e)
    local d = e.data
    if d.down and not d["repeat"] then press(d.key:lower()) end
end)
eqoa.events.on("pointer.pressed", function(e)
    local d = e.data
    if d.button ~= "left" then return end
    for _, h in ipairs(state.hits) do
        if inside(d.x, d.y, h) and h.run then h.run(); return end
    end
end)
eqoa.events.on("confirm.closed", function(e)
    local d = e.data
    if d.kind ~= "revive_offer" and d.kind ~= "summon_offer" then return end
    if d.answer == "yes" then
        say((d.detail and d.detail.sent) and "Accepted (sent)" or "Not sent", d.detail and d.detail.sent == true)
    else
        say("Not accepted", true)
    end
end)
eqoa.events.on("windows.session", function(e)
    local d = e.data
    if d.phase == "opened" then
        note("info", "a " .. tostring(d.kind) .. " opened")
    elseif d.phase == "closed" and d.reason and d.reason ~= "answered" and d.reason ~= "passed" then
        note("info", tostring(d.kind) .. " ended: " .. tostring(d.reason))
    end
end)
-- the host lets go of every claim when the world is left: held again as soon as the new one is up
eqoa.events.on("world.entered", function() claim() end)
eqoa.events.on("state.reset", function() claim() end)
eqoa.events.on("disabling", function()
    for key, token in pairs(state.claimed) do eqoa.input.release(token); state.claimed[key] = nil end
    eqoa.windows.release()
end)

-- Keys held for the window while it is up (and let go the moment it is not). `eqoa.input.claim` knows a key only by its
-- app name ("Enter", "Escape", a digit): any other spelling is refused, and a refused claim leaves the key to the game.
local CLAIM_NAMES = { escape = "Escape", enter = "Enter" }
local function set_claims(want)
    for key, token in pairs(state.claimed) do
        if not want[key] then eqoa.input.release(token); state.claimed[key] = nil end
    end
    for key in pairs(want) do
        if not state.claimed[key] then
            local token = eqoa.input.claim(CLAIM_NAMES[key] or key, true)
            if token then
                state.claimed[key] = token
            else
                if not state.warned[key] then
                    state.warned[key] = true
                    note("warn", "could not hold the " .. key .. " key")
                end
            end
        end
    end
end

-- The drawing kit: a layer over the whole display, drawn in HUD frame pixels.
local function kit(f, k, dw, dh, x0, y0)
    local dl = ui.GetWindowDrawList()
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    local col = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), good = ui.GetColorU32(.35, .8, .4, 1),
        warn = ui.GetColorU32(1, .3, .3, 1), white = ui.GetColorU32(1, 1, 1, 1),
    }
    local t = { col = col }
    function t.fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    function t.frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    function t.text(x, y, value, color, maxw)
        value = tostring(value)
        while maxw and #value > 0 and ui.CalcTextSize(value) > maxw * k do
            value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
        end
        dl:AddText(at(x, y), color or col.text, value)
    end
    function t.hit(x, y, w, h, run)
        state.hits[#state.hits + 1] = { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k, run = run }
    end
    function t.button(x, y, w, label, run, hot)
        t.fill(x, y, w, 17, col.raised)
        t.frame(x, y, w, 17, hot and col.good or col.border)
        t.text(x + 6, y + 2, label, col.text, w - 12)
        t.hit(x, y, w, 17, run)
    end
    return t
end

-- The NPC's dialog: its words wrapped to fit, the answers as buttons (page of PAGE), More and Back.
local function draw_dialog(s, f, k, dw, dh)
    local data = s.data or {}
    local options = data.options or {}
    local response = s.kind == "response_dialog"
    if state.page_ref ~= s.ref then state.page_ref, state.page = s.ref, 0 end
    local pages = math.max(1, math.ceil(#options / PAGE))
    state.page = math.max(0, math.min(state.page, pages - 1))
    local first = state.page * PAGE
    local last = math.min(#options, first + PAGE)
    local lines = wrap(data.text or "", W - 2 * PAD, k)
    local hints = (response and eqoa.options.get("highlight") ~= false) and hand_ins(options) or {}
    local rows = response and (last - first) or 1
    local more, back = state.page < pages - 1, state.page > 0
    rows = rows + (more and 1 or 0) + (back and 1 or 0)
    local H = 24 + #lines * LINE + 8 + rows * 20 + PAD + 14
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + 40 * f.scale_y))
    local t = kit(f, k, dw, dh, x0, y0)
    local col = t.col
    t.fill(0, 0, W, H, col.surface)
    t.frame(0, 0, W, H, col.border)
    local title = (data.talked_to and data.talked_to.name and data.talked_to.name ~= "") and data.talked_to.name or "Dialog"
    t.text(PAD, 6, title, col.white, W - 60)
    -- the x and Escape both hand the dialog back to the game's own window
    t.fill(W - 18, 4, 12, 12, col.raised)
    t.frame(W - 18, 4, 12, 12, col.border)
    t.text(W - 15, 4, "x", col.text)
    t.hit(W - 18, 4, 12, 12, function() pass(s.ref) end)
    for i, line in ipairs(lines) do t.text(PAD, 22 + (i - 1) * LINE, line, col.text, W - 2 * PAD) end
    local y = 22 + #lines * LINE + 8
    local keys = { escape = function() pass(s.ref) end }
    if response then
        -- Enter is this window's while a dialog is up, so it never reaches the game (it would open the chat line);
        -- on answers it does nothing: the number keys and clicks answer
        keys["enter"] = function() end
        for i = first + 1, last do
            local n = i - first
            local option = i
            local run = function() answer(s.ref, option, "Answer") end
            local quest = hints[i]
            if quest then
                -- a quest hand-in: the answer is marked and has its own Hand in button beside it
                t.button(PAD, y, W - 2 * PAD - HAND_W - 4, n .. ". " .. tostring(options[i]), run, true)
                t.button(W - PAD - HAND_W, y, HAND_W, "Hand in",
                    function() answer(s.ref, option, "Hand in " .. tostring(quest.title)) end, true)
            else
                t.button(PAD, y, W - 2 * PAD, n .. ". " .. tostring(options[i]), run)
            end
            keys[DIGITS[n]] = run
            y = y + 20
        end
        if more then
            local run = function() state.page = state.page + 1 end
            t.button(PAD, y, W - 2 * PAD, "7. More", run)
            keys["7"] = run
            y = y + 20
        end
        if back then
            local run = function() state.page = state.page - 1 end
            t.button(PAD, y, W - 2 * PAD, "8. Back", run)
            keys["8"] = run
            y = y + 20
        end
    else
        local run = function() answer(s.ref, 1, "Done") end
        t.button(PAD, y, 80, "Done", run, true)
        keys["enter"], keys["1"] = run, run
    end
    t.text(PAD, H - 14, "Esc: the game's own window", col.muted, W - 2 * PAD)
    if state.result and state.frame - state.result.at < 300 then
        t.text(W - PAD - 150, H - 14, state.result.text, state.result.ok == false and col.warn or col.muted, 150)
    end
    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    return keys, y0 + H * k
end

-- An invitation, summons or resurrection offer. Accepting a summons or resurrection makes the game's own
-- "Are you sure?" appear: only your own Yes there sends it.
local function draw_offer(s, f, k, dw, dh, below)
    local data = s.data or {}
    local lines = wrap(data.text or "", OFFER_W - 2 * PAD, k)
    local H = 24 + #lines * LINE + 8 + 17 + PAD
    local x0 = math.max(0, math.min(dw - OFFER_W * k, f.offset_x + (640 - OFFER_W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, math.max(below + 6 * k, f.offset_y + 60 * f.scale_y)))
    local t = kit(f, k, dw, dh, x0, y0)
    local col = t.col
    t.fill(0, 0, OFFER_W, H, col.surface)
    t.frame(0, 0, OFFER_W, H, col.border)
    t.text(PAD, 6, OFFER_TITLES[s.kind] or "Offer", col.white, OFFER_W - 2 * PAD)
    for i, line in ipairs(lines) do t.text(PAD, 22 + (i - 1) * LINE, line, col.text, OFFER_W - 2 * PAD) end
    local y = 22 + #lines * LINE + 8
    t.button(PAD, y, 80, "Accept", function()
        local request = answer(s.ref, 1, "Accept")
        if request and s.kind ~= "group_invite" then say("The game will ask you to confirm", nil) end
    end, true)
    t.button(PAD + 88, y, 80, "Decline", function() answer(s.ref, 2, "Decline") end)
    t.button(PAD + 176, y, 110, "Game window", function() pass(s.ref) end)
    if state.result and state.frame - state.result.at < 300 then
        t.text(PAD, y + 19, state.result.text, state.result.ok == false and col.warn or col.muted, OFFER_W - 2 * PAD)
    end
    eqoa.input.claim_pointer({ x = x0, y = y0, width = OFFER_W * k, height = (H + 12) * k })
    return {}
end

eqoa.events.on("render", function()
    state.frame = state.frame + 1
    if state.frame == 1 or state.frame % RECLAIM_FRAMES == 0 then claim() end
    -- reading the sessions every frame is what tells the host this mod is still looking at them
    local dialog
    for _, kind in ipairs(DIALOG_KINDS) do
        local s = eqoa.windows.session(kind)
        if s and not dialog then dialog = s end
    end
    if dialog then auto_hand_in(dialog) end
    local offer
    for _, kind in ipairs(OFFER_KINDS) do
        local s = eqoa.windows.session(kind)
        if s and not offer then offer = s end
    end
    state.hits, state.keys = {}, {}
    if not dialog and not offer then
        set_claims({})
        return
    end
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_dialogs_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 11 / 13)
    local below = 0
    if dialog then
        state.keys, below = draw_dialog(dialog, f, k, dw, dh)
    end
    if offer then
        local keys = draw_offer(offer, f, k, dw, dh, below)
        if not dialog then
            state.keys = keys
            state.keys.escape = function() pass(offer.ref) end
        end
    end
    local want = {}
    for key in pairs(state.keys) do want[key] = true end
    set_claims(want)
    ui.SetWindowFontScale(1)
    ui.End()
end)
