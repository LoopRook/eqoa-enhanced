-- The Character window: summary and training, Class Masteries, factions and the combat record.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local ui = eqoa.imgui

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, PAD, LINE, ROWS = 340, 8, 13, 15
local TAB_Y, TOP = 22, 44
local STATS = { "strength", "stamina", "agility", "dexterity", "wisdom", "intelligence", "charisma" }
local STAT_LABELS = { "Strength", "Stamina", "Agility", "Dexterity", "Wisdom", "Intelligence", "Charisma" }
local RESISTS = { { "armor", "Armor" }, { "fire", "Fire" }, { "cold", "Cold" }, { "lightning", "Lightning" },
    { "arcane", "Arcane" }, { "poison", "Poison" }, { "disease", "Disease" } }
local TABS = { { "summary", "Summary" }, { "masteries", "Masteries" }, { "factions", "Factions" }, { "combat", "Combat" } }
local MASTER_CLASS = 4

-- `drawn` and `cuts` are kept only while `record` is set (the layout test reads them)
local M = { drawn = {}, cuts = 0, record = false, rect = nil }
local S = {
    open = false, tab = "summary", main = nil, mastery = nil, tokens = {}, escape = nil,
    hits = {}, row_hits = {}, rect = nil, list_rect = nil, sb_rect = nil,
    add = {}, scroll = 0, sel = nil, result = nil, last_req = nil,
    skills = nil, factions_asked = false, record_asked = false,
}
for _, name in ipairs(STATS) do S.add[name] = 0 end

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then S.tokens[#S.tokens + 1] = token end
end

local function say(text, ok) S.result = { text = text, ok = ok } end

-- A call that asks something of the client: what came back is shown at the foot of the window.
local function run(label, fn)
    local request, err = fn()
    if err then
        S.last_req = nil
        say(err.message or err.code or "Refused", false)
    elseif request then
        S.last_req = request
        say(label, nil)
    end
end

local function on_result(r)
    if not r or not S.open or S.last_req == nil or r.request_id ~= S.last_req then return end
    if r.ok == true then
        if r.detail and r.detail.asked then say("Waiting for your answer to the question", nil) else say("Sent", true) end
    elseif r.ok == false then
        say(r.reason or r.code or "Refused", false)
    end
end

local function on_confirm_closed(d)
    if not d then return end
    local kinds = { train_stats = "Training", mastery_buy = "Purchase", mastery_refund = "Switch off" }
    local what = kinds[d.kind]
    if not what then return end
    if d.answer == "yes" and d.detail and d.detail.sent then
        say(what .. " sent", true)
        if d.kind == "train_stats" then
            for _, name in ipairs(STATS) do S.add[name] = 0 end
        end
    elseif d.answer == "yes" then
        say(what .. " not sent: " .. tostring(d.detail and d.detail.reason or "things changed"), false)
    elseif d.answer == "no" or d.answer == "expired" then
        say(what .. " not confirmed", nil)
    end
end

-- ---- what the window shows -----------------------------------------------------------------------------------------

local function sorted_skills()
    if S.skills == nil then
        local list = eqoa.mastery.skills() or {}
        local copy = {}
        for i, s in ipairs(list) do copy[i] = s end
        table.sort(copy, function(a, b)
            if a.category ~= b.category then return a.category < b.category end
            return (a.name or "") < (b.name or "")
        end)
        S.skills = copy
    end
    return S.skills
end

local function skill_by_id(id)
    for _, s in ipairs(sorted_skills()) do if s.id == id then return s end end
end

local function skill_state(s)
    if s.owned then return "owned" end
    if s.known then return "buy" end
    return "locked"
end

local function counts(tab)
    if tab == "masteries" then return #sorted_skills() end
    if tab == "factions" then
        local f = eqoa.factions.list()
        return f and #f.entries or 0
    end
    if tab == "combat" then
        local r = eqoa.combat.record()
        return r and #r.rows or 0
    end
    return 0
end

-- How many rows of the list each tab shows (the rest scrolls).
local function visible_rows(tab)
    if tab == "masteries" then return ROWS - 5 end
    return ROWS - 1
end

local function clamp_scroll()
    local most = math.max(0, counts(S.tab) - visible_rows(S.tab))
    if S.scroll > most then S.scroll = most end
    if S.scroll < 0 then S.scroll = 0 end
end

local close

local function set_tab(tab)
    if S.tab == tab then return end
    if S.tab == "masteries" and S.mastery then
        eqoa.windows.close_session(S.mastery.ref)
        S.mastery = nil
    end
    S.tab, S.scroll, S.sel = tab, 0, nil
    if tab == "masteries" then
        local s, err = eqoa.windows.open_session("mastery", { opened_from = "mod" })
        if not s then
            say("Class Mastery could not open: " .. tostring(err and err.message), false)
            S.tab = "summary"
            return
        end
        S.mastery, S.skills = s, nil
    elseif tab == "factions" then
        run("Asking for the factions", function() return eqoa.actions.request_factions(1) end)
    elseif tab == "combat" then
        run("Asking for the combat record", function() return eqoa.actions.request_combat_record() end)
    end
end

local function scroll_to(y)
    local r = S.sb_rect
    local most = math.max(0, counts(S.tab) - visible_rows(S.tab))
    if not r or most == 0 then return end
    local frac = math.max(0, math.min(1, (y - r.y) / r.h))
    S.scroll = math.floor(frac * most + 0.5)
end

local function training()
    local t = eqoa.player.training()
    if not t then return { points = 0, stat_cap = 0, caps = {} } end
    return t
end

local function pending_total()
    local n = 0
    for _, name in ipairs(STATS) do n = n + S.add[name] end
    return n
end

local function adjust(stat, delta)
    local t = training()
    local att = eqoa.player.get() and eqoa.player.get().attributes
    local base = att and att.base[stat] or 0
    local cap = (t.caps and t.caps[stat]) or t.stat_cap or 0
    local add = S.add[stat]
    if delta > 0 then
        if pending_total() >= t.points then say("No training points left", false); return end
        if base + add + 1 > cap then say(stat:sub(1, 1):upper() .. stat:sub(2) .. " is at its cap", false); return end
    elseif add + delta < 0 then
        return
    end
    S.add[stat] = add + delta
end

local function spend()
    local asks = {}
    for _, name in ipairs(STATS) do
        if S.add[name] > 0 then asks[name] = S.add[name] end
    end
    if next(asks) == nil then say("Choose where to put your points first", false); return end
    run("Asking you to confirm", function() return eqoa.actions.train_stats(asks) end)
end

local function inspect(skill)
    S.sel = skill.id
    local m = eqoa.mastery.get()
    if m and m.pending then say("Waiting for the server's last answer", false); return end
    run("Opening " .. skill.name, function() return eqoa.actions.mastery_browse(skill.id) end)
end

local function pointer_pressed(d)
    if d.button ~= "left" then return end
    if inside(d.x, d.y, S.sb_rect) then scroll_to(d.y); return end
    for _, h in ipairs(S.hits) do
        if inside(d.x, d.y, h) and h.run then h.run(); return end
    end
    for _, h in ipairs(S.row_hits) do
        if inside(d.x, d.y, h) and h.skill then inspect(h.skill); return end
    end
end

local draw

function M.is_open() return S.open end

function M.open(opened_from)
    if S.open then return end
    local s, err = eqoa.windows.open_session("trainer", { opened_from = opened_from or "key" })
    if not s then
        -- a refusal says why (the player is using the game's windows, dead, out of the world...)
        eqoa.log.write("warn", "Character did not open: " .. tostring(err and err.message))
        return
    end
    S.main, S.open, S.tab, S.scroll, S.sel, S.result, S.skills, S.last_req = s, true, "summary", 0, nil, nil, nil, nil
    for _, name in ipairs(STATS) do S.add[name] = 0 end
    listen("render", draw)
    listen("mastery.changed", function() S.skills = nil end)
    listen("state.reset", function() S.skills = nil end)
    listen("action.result", function(e) on_result(e.data) end)
    listen("confirm.closed", function(e) on_confirm_closed(e.data) end)
    -- the client ends a session when the player zones, dies, uses the game's windows or turns the mod off
    listen("windows.session", function(e)
        local d = e.data
        if d.phase ~= "closed" then return end
        if S.main and d.ref == S.main.ref then close(true)
        elseif S.mastery and d.ref == S.mastery.ref then
            S.mastery, S.tab, S.sel, S.scroll = nil, "summary", nil, 0
        end
    end)
    listen("input.key", function(e)
        -- the client's own question takes Escape (as No) while it is up
        if e.data.down and not e.data["repeat"] and e.data.key:lower() == "escape" and not eqoa.windows.confirm() then
            close()
        end
    end)
    listen("pointer.pressed", function(e) pointer_pressed(e.data) end)
    listen("pointer.wheel", function(e)
        local d = e.data
        if (d.steps or 0) ~= 0 and inside(d.x, d.y, S.list_rect) then
            local most = math.max(0, counts(S.tab) - visible_rows(S.tab))
            S.scroll = math.max(0, math.min(most, S.scroll + (d.steps > 0 and -1 or 1)))
        end
    end)
    -- Escape is the window's, not the game menu's, while it is up
    S.escape = eqoa.input.claim("Escape", true)
end

function close(from_client)
    if not S.open then return end
    for _, token in ipairs(S.tokens) do eqoa.events.off(token) end
    S.tokens = {}
    if S.escape then eqoa.input.release(S.escape); S.escape = nil end
    if S.mastery then eqoa.windows.close_session(S.mastery.ref) end
    if S.main and not from_client then eqoa.windows.close_session(S.main.ref) end
    S.open, S.main, S.mastery, S.hits, S.row_hits, S.rect = false, nil, nil, {}, {}, nil
    M.rect = nil
end
M.close = close

-- ---- drawing -------------------------------------------------------------------------------------------------------

function draw()
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    local list_y = TOP
    local list_h = ROWS * LINE
    local foot_y = list_y + list_h + 6
    local bar_y = foot_y + LINE + 4
    local H = bar_y + 16 + PAD
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    S.rect = { x = x0, y = y0, w = W * k, h = H * k }
    M.rect = S.rect
    if M.record then M.drawn = {} end

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_character_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 11 / 13)
    local dl = ui.GetWindowDrawList()
    local c = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), good = ui.GetColorU32(.35, .8, .4, 1),
        warn = ui.GetColorU32(1, .3, .3, 1), white = ui.GetColorU32(1, 1, 1, 1),
        hl = ui.GetColorU32(.20, .20, .24, 1), blue = ui.GetColorU32(.3, .5, 1, 1),
        red = ui.GetColorU32(.85, .2, .2, 1), tab = ui.GetColorU32(.16, .16, .2, 1),
    }
    local function fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    local function frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    -- one line of text, cut to `maxw` frame pixels (the window never draws past its edge)
    local function text(x, y, value, color, maxw)
        value = tostring(value)
        maxw = math.min(maxw or (W - PAD - x), W - PAD - x)
        while #value > 0 and ui.CalcTextSize(value) > maxw * k do
            value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
            if M.record then M.cuts = M.cuts + 1 end
        end
        if M.record then
            M.drawn[#M.drawn + 1] = { x = x0 + x * k, y = y0 + y * k, w = ui.CalcTextSize(value), h = LINE * k }
        end
        dl:AddText(at(x, y), color or c.text, value)
    end
    -- words wrapped to `maxw`, at most `lines` lines (the last one cut)
    local function wrapped(x, y, value, color, maxw, lines)
        local line, n = "", 0
        local function flush() text(x, y + n * LINE, line, color, maxw); n = n + 1; line = "" end
        for word in tostring(value):gmatch("%S+") do
            local try = line == "" and word or (line .. " " .. word)
            if ui.CalcTextSize(try) > maxw * k and line ~= "" then
                if n + 1 >= lines then line = try; break end
                flush()
                line = word
            else
                line = try
            end
        end
        if line ~= "" and n < lines then flush() end
    end
    local function hit(x, y, w, h, extra)
        local r = { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k }
        for key, v in pairs(extra or {}) do r[key] = v end
        return r
    end
    local function button(bx, label, enabled, fn, by, bh)
        by, bh = by or bar_y, bh or 16
        local bw = ui.CalcTextSize(label) / k + 12
        fill(bx, by, bw, bh, c.raised)
        frame(bx, by, bw, bh, enabled and c.border or c.surface)
        text(bx + 6, by + 2, label, enabled and c.text or c.muted, bw - 6)
        if enabled then S.hits[#S.hits + 1] = hit(bx, by, bw, bh, { run = fn, label = label }) end
        return bx + bw + 4
    end

    fill(0, 0, W, H, c.surface)
    frame(0, 0, W, H, c.border)
    text(PAD, 6, "Character", c.white)
    text(W - 118, 6, "Shift+C or Esc", c.muted, 92)
    S.hits, S.row_hits, S.sb_rect = {}, {}, nil
    fill(W - 18, 4, 12, 12, c.raised)
    frame(W - 18, 4, 12, 12, c.border)
    text(W - 15, 4, "x", c.text, 10)
    S.hits[1] = hit(W - 18, 4, 12, 12, { run = function() close() end, label = "close" })

    -- the tabs
    local tx = PAD
    for _, t in ipairs(TABS) do
        local tw = ui.CalcTextSize(t[2]) / k + 14
        fill(tx, TAB_Y, tw, 16, S.tab == t[1] and c.hl or c.tab)
        frame(tx, TAB_Y, tw, 16, S.tab == t[1] and c.white or c.border)
        text(tx + 7, TAB_Y + 2, t[2], S.tab == t[1] and c.white or c.muted, tw - 7)
        S.hits[#S.hits + 1] = hit(tx, TAB_Y, tw, 16, { run = function() set_tab(t[1]) end, label = "tab:" .. t[1] })
        tx = tx + tw + 3
    end

    S.list_rect = { x = x0 + PAD * k, y = y0 + list_y * k, w = (W - 2 * PAD) * k, h = list_h * k }
    -- the scroll track beside a list of `total` lines, `rows` of them showing (the rows start a line below the top)
    local function scrollbar(total, rows)
        if total <= rows then return end
        local sb_x, th = W - PAD + 1, math.max(10, rows * LINE * rows / total)
        fill(sb_x, list_y + LINE, 6, rows * LINE, c.border)
        fill(sb_x, list_y + LINE + (rows * LINE - th) * (S.scroll / (total - rows)), 6, th, c.muted)
        S.sb_rect = { x = x0 + (sb_x - 1) * k, y = y0 + (list_y + LINE) * k, w = 7 * k, h = rows * LINE * k }
    end
    clamp_scroll()

    if S.tab == "summary" then
        local p = eqoa.player.get()
        local st = eqoa.player.stats()
        local t = training()
        if not p then
            text(PAD, list_y, "The character is not in the world", c.muted)
        else
            text(PAD, list_y, string.format("%s   Level %d   %s %s", p.name or "?", p.level or 0,
                p.race_name or "", p.class_name or ""), c.white, W - 2 * PAD)
            local hp = p.hp and string.format("HP %d/%d", p.hp.current, p.hp.maximum) or "HP -"
            local pw = p.power and string.format("Power %d/%d", p.power.current, p.power.maximum) or "Power -"
            local m = eqoa.mastery.get()
            local mp = (m and m.available) and ("Mastery points " .. m.available) or "Mastery points -"
            text(PAD, list_y + LINE, hp .. "   " .. pw .. "   " .. mp, c.text, W - 2 * PAD)
            local att = p.attributes
            local left = t.points - pending_total()
            text(PAD, list_y + 3 * LINE, "Stat  now / cap  (with gear)", c.muted, 190)
            for i, name in ipairs(STATS) do
                local y = list_y + (3 + i) * LINE + 2
                local base = att and att.base[name] or 0
                local total = att and att.total[name] or 0
                local cap = (t.caps and t.caps[name]) or t.stat_cap or 0
                local add = S.add[name]
                text(PAD, y, STAT_LABELS[i], c.text, 68)
                text(PAD + 70, y, string.format("%d / %d (%d)", base + add, cap, total + add), add > 0 and c.good or c.text, 96)
                local bx = PAD + 168
                fill(bx, y, 12, 12, c.raised)
                frame(bx, y, 12, 12, add > 0 and c.border or c.surface)
                text(bx + 3, y, "-", add > 0 and c.text or c.muted, 8)
                S.hits[#S.hits + 1] = hit(bx, y, 12, 12, { run = function() adjust(name, -1) end, label = "-" .. name })
                fill(bx + 14, y, 12, 12, c.raised)
                frame(bx + 14, y, 12, 12, left > 0 and c.border or c.surface)
                text(bx + 17, y, "+", left > 0 and c.text or c.muted, 8)
                S.hits[#S.hits + 1] = hit(bx + 14, y, 12, 12, { run = function() adjust(name, 1) end, label = "+" .. name })
                if add > 0 then text(bx + 28, y, "+" .. add, c.good, 26) end
            end
            text(PAD, list_y + 12 * LINE, string.format("Training points: %d   left to place: %d", t.points, left), c.text, 200)
            -- the resists and armor
            text(236, list_y + 3 * LINE, "Resists", c.muted, W - PAD - 236)
            for i, r in ipairs(RESISTS) do
                local y = list_y + (3 + i) * LINE + 2
                local item = st and st[r[1]]
                text(236, y, r[2], c.text, 56)
                text(236 + 58, y, item and tostring(item.current) or "-", c.text, W - PAD - 294)
            end
        end
    elseif S.tab == "masteries" then
        local list = sorted_skills()
        local m = eqoa.mastery.get()
        text(PAD, list_y - 1, string.format("Points available %s   spent %s   %s",
            (m and m.available) and m.available or "-", (m and m.spent) and m.spent or "-",
            (m and m.pending) and "waiting for the server" or ""), c.text, W - 2 * PAD)
        if #list == 0 then
            text(PAD, list_y + LINE, "Waiting for the skill list from the server", c.muted)
        end
        local rows = ROWS - 5
        for i = 1, rows do
            local s = list[S.scroll + i]
            if s then
                local y = list_y + i * LINE
                if S.sel == s.id then fill(PAD, y, W - 2 * PAD - 8, LINE, c.hl) end
                local state = skill_state(s)
                text(PAD + 2, y, (s.category == MASTER_CLASS and "[M] " or "") .. s.name,
                    state == "locked" and c.muted or c.text, 196)
                text(PAD + 204, y, tostring(s.cost) .. " pt", c.muted, 40)
                text(PAD + 250, y, state, state == "owned" and c.good or (state == "buy" and c.white or c.muted), 50)
                S.row_hits[#S.row_hits + 1] = hit(PAD, y, W - 2 * PAD - 8, LINE, { skill = s, label = "skill:" .. s.id })
            end
        end
        scrollbar(#list, rows)
        -- the chosen skill
        local dy = list_y + (rows + 1) * LINE + 2
        local sel = S.sel and skill_by_id(S.sel)
        local node = eqoa.mastery.selected()
        if node and sel and node.skill_id ~= sel.id and node.id ~= sel.id then node = nil end
        if sel then
            text(PAD, dy, (node and node.title ~= "" and node.title) or sel.name, c.white, W - 2 * PAD)
            if node then
                local req = string.format("Level %d   %d points spent first", node.level_required or 0, node.points_spent_required or 0)
                text(PAD, dy + LINE, req, c.muted, W - 2 * PAD)
                wrapped(PAD, dy + 2 * LINE, node.text or "", c.text, W - 2 * PAD, 1)
            else
                text(PAD, dy + LINE, "Opening it...", c.muted)
            end
        else
            text(PAD, dy, "Click a skill to read it", c.muted)
        end
        local bx = PAD
        local ready = sel and node
        bx = button(bx, "Buy", ready and (not sel.owned) and sel.known and true or false, function()
            run("Asking you to confirm", function() return eqoa.actions.mastery_buy(sel.id) end)
        end)
        bx = button(bx, "Refund", ready and sel.owned and true or false, function()
            run("Asking you to confirm", function() return eqoa.actions.mastery_refund(sel.id) end)
        end)
    elseif S.tab == "factions" then
        local fl = eqoa.factions.list()
        if not fl then
            text(PAD, list_y, "Waiting for the factions from the server", c.muted)
        else
            text(PAD, list_y - 1, string.format("Page %d of %d", fl.page, math.max(fl.pages, 1)), c.muted, 120)
            local rows = ROWS - 1
            for i = 1, rows do
                local e = fl.entries[S.scroll + i]
                if e then
                    local y = list_y + i * LINE
                    text(PAD + 2, y, e.name, c.text, 150)
                    -- a bar from the middle: red to the left, blue to the right (the game's own, clamped to 2000)
                    local mid, half = PAD + 160 + 60, 60
                    local v = math.max(-2000, math.min(2000, e.standing))
                    fill(mid - half, y + 3, half * 2, 6, c.raised)
                    if v >= 0 then fill(mid, y + 3, half * v / 2000, 6, c.blue) else fill(mid + half * v / 2000, y + 3, -half * v / 2000, 6, c.red) end
                    text(mid + half + 4, y, tostring(e.standing), c.muted, W - PAD - (mid + half + 4) - 8)
                end
            end
            scrollbar(#fl.entries, rows)
            local bx = PAD
            bx = button(bx, "Previous page", fl.page > 1, function()
                S.scroll = 0
                run("Asking for the page", function() return eqoa.actions.request_factions(fl.page - 1) end)
            end)
            bx = button(bx, "Next page", fl.page < fl.pages, function()
                S.scroll = 0
                run("Asking for the page", function() return eqoa.actions.request_factions(fl.page + 1) end)
            end)
        end
    else
        local r, err = eqoa.combat.record()
        if not r then
            text(PAD, list_y, err and err.code == "unavailable" and tostring(err.message) or "No record yet", c.muted, W - 2 * PAD)
            wrapped(PAD, list_y + LINE, "Press Refresh to ask the server for your record.", c.muted, W - 2 * PAD, 2)
        else
            text(PAD, list_y - 1, r.name, c.white, W - 2 * PAD)
            local rows = ROWS - 1
            for i = 1, rows do
                local row = r.rows[S.scroll + i]
                if row then
                    local y = list_y + i * LINE
                    if row.heading then
                        text(PAD, y, row.label, c.white, W - 2 * PAD - 8)
                    else
                        text(PAD + 8, y, row.label, c.text, 150)
                        text(PAD + 164, y, row.value, c.muted, W - 2 * PAD - 8 - 164)
                    end
                end
            end
            scrollbar(#r.rows, rows)
        end
        button(PAD, "Refresh", true, function()
            run("Asking for the combat record", function() return eqoa.actions.request_combat_record() end)
        end)
    end
    if S.tab == "summary" then
        local bx = PAD
        bx = button(bx, "Train", pending_total() > 0, spend)
        button(bx, "Clear", pending_total() > 0, function()
            for _, name in ipairs(STATS) do S.add[name] = 0 end
        end)
    end
    if S.result then
        text(PAD, foot_y, S.result.text, S.result.ok == false and c.warn or c.muted, W - 2 * PAD)
    elseif S.tab == "summary" then
        text(PAD, foot_y, "Train asks you to confirm before any point is spent", c.muted, W - 2 * PAD)
    end

    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    ui.SetWindowFontScale(1)
    ui.End()
end

M.state = S
M.rows = ROWS
return M
