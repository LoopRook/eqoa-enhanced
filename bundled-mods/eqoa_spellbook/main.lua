-- Spellbook: the character's abilities in a window of their own, on its own key (default Shift+K).
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- It sits over play: the character keeps walking, and the game's Start menu > Abilities page is untouched.
-- Press the key again, press Escape or click the X to close it.
-- Type in the search box to filter by name; the Self/Target/Group/Pet buttons filter by who the ability affects; the
-- list is grouped by level. Drag an ability onto a slot to memorize it there, drag a slot onto another to move or swap
-- them, right-click a slot to clear it.
local ui = eqoa.imgui

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, PAD, LINE, ROWS, SLOT_W = 332, 8, 13, 12, 30
local HEAD_LINES, DESC_LINES, STATUS_LINES = 2, 3, 2
local ICON = 11            -- the icon in a list row
local QUERY_MAX = 24       -- characters in the search box
local DRAG_PX = 4          -- how far (HUD pixels) a press must travel to count as a drag, not a click
-- `Ability.scope` (docs/modding/EQOA-API-SPEC.md, eqoa.abilities.scope): self 0, target 1, group 2, pet 3, corpse 4,
-- unknown 5. The buttons name the first four; All shows every ability, corpse and unknown ones included.
local FILTERS = {
    { key = "self", label = "Self", scope = 0 }, { key = "target", label = "Target", scope = 1 },
    { key = "group", label = "Group", scope = 2 }, { key = "pet", label = "Pet", scope = 3 },
    { key = "all", label = "All" },
}
local state = {
    open = false, session = nil, tokens = {}, escape = nil,
    list = nil, rows = nil, scroll = 0, selected = nil, result = nil,
    query = "", filter = "all", focus = false, drag = nil, plan = nil,
    hits = {}, row_hits = {}, slot_hits = {}, rect = nil, sb_rect = nil, search_rect = nil, clear_rect = nil,
}
local icons = { tex = {}, due = {}, frame = 0 }

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then state.tokens[#state.tokens + 1] = token end
end

local function abilities()
    if state.list == nil then
        -- as the game's Abilities page: only what the character has learned, and not the bar-only records (252 up)
        local list = {}
        for _, a in ipairs(eqoa.abilities.list() or {}) do
            if a.learned ~= false and (a.display_key or 0) < 252 then list[#list + 1] = a end
        end
        table.sort(list, function(a, b)
            if (a.level or 0) ~= (b.level or 0) then return (a.level or 0) < (b.level or 0) end
            return (a.name or "") < (b.name or "")
        end)
        state.list = list
    end
    return state.list
end

local function invalidate() state.list, state.rows = nil, nil end

-- The rows as drawn, top to bottom: a header for each block of ten levels and the abilities of that block that pass the
-- search box and the scope button. (The list is sorted by level, then name.)
local function visible_rows()
    if state.rows then return state.rows end
    local rows, last = {}, nil
    local q = state.query:lower()
    local want
    for _, f in ipairs(FILTERS) do if f.key == state.filter then want = f.scope end end
    for _, a in ipairs(abilities()) do
        if (want == nil or a.scope == want) and (q == "" or (a.name or ""):lower():find(q, 1, true)) then
            local block = math.max(0, math.floor(((a.level or 1) - 1) / 10))
            if block ~= last then
                rows[#rows + 1] = { header = string.format("Levels %d-%d", block * 10 + 1, block * 10 + 10) }
                last = block
            end
            rows[#rows + 1] = { ability = a }
        end
    end
    state.rows = rows
    return rows
end

local function by_id(id)
    for _, a in ipairs(abilities()) do if a.id == id then return a end end
end

local function by_slot(n)
    for _, a in ipairs(abilities()) do if a.spellbook_slot == n then return a end end
end

local function say(text, ok) state.result = { text = text, ok = ok } end

local advance

-- What the game said about an action this window asked for. A move that takes several actions goes on to the next one
-- only when the game has confirmed the one before; any refusal ends it.
local function on_result(r)
    if not r or not state.open or state.last_req == nil or r.request_id ~= state.last_req then return end
    if r.state == "completed" or r.ok == true then
        if state.plan then advance() else say("Done", true) end
    elseif r.state == "refused" or r.state == "cancelled" or r.ok == false then
        state.plan = nil
        say((r.reason or r.code or "Refused"), false)
    elseif r.state == "unknown" then
        state.plan = nil
        say("The game did not answer", false)
    end
end

local function run(label, fn)
    local request, err = fn()
    if err then
        state.last_req, state.plan = nil, nil
        say(err.message or err.code or "Refused", false)
    elseif request then
        state.last_req = request
        say(label .. " sent", nil)
    end
end

-- Steps are { label, function } pairs, sent one at a time.
function advance()
    local plan = state.plan
    if not plan then return end
    plan.i = plan.i + 1
    local step = plan.steps[plan.i]
    if not step then
        state.plan = nil
        say("Done", true)
        return
    end
    run(step[1], step[2])
end

local function start_plan(steps)
    if state.plan then say("Wait for the last move to finish", false); return end
    state.plan = { steps = steps, i = 0 }
    advance()
end

local close

-- Memorize, as the game's Abilities page does it: an ability already memorized is refused (the game says so and
-- offers Unmemorize), one the character has not learned is refused, else it goes on the slot chosen (or the first
-- free one). `usable` is not the test: it is the game's "on a slot" flag, false for every ability not yet memorized.
local function memorize(a, slot)
    if not a then say("Choose an ability first", false); return end
    if a.spellbook_slot then
        say(a.name .. " is already memorized (slot " .. a.spellbook_slot .. "): Unmemorize it first", false)
        return
    end
    if a.learned == false then say(a.name .. " is not learned yet", false); return end
    if not slot then
        for n = 1, 20 do
            if not by_slot(n) then slot = n; break end
        end
    end
    if not slot then say("Every slot is taken: Unmemorize one first", false); return end
    if by_slot(slot) then say("Slot " .. slot .. " is taken: Unmemorize it first", false); return end
    run("Memorize", function() return eqoa.actions.assign_ability(slot, a.ref) end)
end

-- The scroll track: a click on it puts the list there (the API has no button-release event, so no drag).
local function scroll_to(y)
    local r = state.sb_rect
    local most = math.max(0, #visible_rows() - ROWS)
    if not r or most == 0 then return end
    local frac = math.max(0, math.min(1, (y - r.y) / r.h))
    state.scroll = math.floor(frac * most + 0.5)
end

local function focus(on)
    on = on and true or false
    if state.focus == on then return end
    state.focus = on
    if not on then eqoa.input.capture_text(false) end
end

local function set_query(q)
    state.query, state.rows, state.scroll = q, nil, 0
end

-- A typed key while the search box has the keyboard.
local function typed(d)
    if not d.down or d.synthetic then return end
    local key = (d.key or ""):lower()
    if key == "escape" or key == "enter" then
        if not d["repeat"] then focus(false) end
    elseif key == "backspace" then
        set_query(state.query:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or "")
    elseif d.text and d.text ~= "" then
        local q = state.query
        for c in d.text:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
            local _, n = q:gsub("[^\128-\191]", "")
            if n >= QUERY_MAX then break end
            if not c:find("[%z\1-\31\127]") then q = q .. c end
        end
        if q ~= state.query then set_query(q) end
    end
end

-- Put `a` on `slot` as the game's Abilities page does it (Memorize, then the slot), moving whatever is in the way:
-- the API has no move or swap action, so a move is what the player would do by hand, one action at a time and each
-- only after the game confirmed the one before: Unmemorize the slot the ability is leaving, Unmemorize what is on the
-- target slot, Memorize the ability onto the target, and (a swap) Memorize the other ability onto the vacated slot.
local function drop_on(a, from, slot)
    if state.plan then say("Wait for the last move to finish", false); return end
    from = from or a.spellbook_slot
    if from == slot then return end
    if not from and a.learned == false then say(a.name .. " is not learned yet", false); return end
    local other = by_slot(slot)
    local steps = {}
    if from then steps[#steps + 1] = { "Unmemorize", function() return eqoa.actions.unmemorize_ability(from) end } end
    if other then steps[#steps + 1] = { "Unmemorize", function() return eqoa.actions.unmemorize_ability(slot) end } end
    steps[#steps + 1] = { "Memorize", function() return eqoa.actions.assign_ability(slot, a.ref) end }
    if other and from then
        steps[#steps + 1] = { "Memorize", function() return eqoa.actions.assign_ability(from, other.ref) end }
    end
    start_plan(steps)
end

local function pointer_pressed(d)
    if d.button ~= "left" and d.button ~= "right" then return end
    if d.button == "left" then
        if inside(d.x, d.y, state.search_rect) then
            if state.query ~= "" and inside(d.x, d.y, state.clear_rect) then set_query("") else focus(true) end
            return
        end
        focus(false)
    end
    if d.button == "left" and inside(d.x, d.y, state.sb_rect) then scroll_to(d.y); return end
    for _, h in ipairs(state.hits) do
        if d.button == "left" and inside(d.x, d.y, h) and h.run then h.run(); return end
    end
    for _, h in ipairs(state.slot_hits) do
        if inside(d.x, d.y, h) then
            if d.button == "right" then
                if h.ability then
                    run("Unmemorize", function() return eqoa.actions.unmemorize_ability(h.slot) end)
                end
            elseif h.ability then
                -- a press on a memorized slot chooses it and may start a drag
                state.selected = h.ability.id
                state.drag = { ability = h.ability, from = h.slot, age = 0 }
            elseif state.selected and by_id(state.selected) then
                memorize(by_id(state.selected), h.slot)
            end
            return
        end
    end
    for _, h in ipairs(state.row_hits) do
        if d.button == "left" and inside(d.x, d.y, h) then
            state.selected = h.ability.id
            state.drag = { ability = h.ability, age = 0 }
            return
        end
    end
end

-- Follows a press to its release (ImGui's own mouse state: the pointer events only report presses). A press that
-- travels a few pixels is a drag, and where it ends decides: on a slot, the ability goes there; anywhere else, nothing.
-- A press that does not travel is only a click (the choosing was done when it was pressed).
local function update_drag(k)
    local d = state.drag
    if not d then return end
    d.age = d.age + 1
    local down, released = ui.IsMouseDown(0), ui.IsMouseReleased(0)
    if down then d.seen = true end
    if down or released then
        local dx, dy = ui.GetMouseDragDelta(0, 0)
        if dx * dx + dy * dy >= (DRAG_PX * k) * (DRAG_PX * k) then d.moved = true end
    end
    if down then return end
    if not (d.seen or released) then
        -- ImGui never saw this press (it was taken somewhere else): not a drag
        if d.age > 5 then state.drag = nil end
        return
    end
    state.drag = nil
    if not d.moved then return end
    local p = eqoa.input.pointer()
    for _, h in ipairs(state.slot_hits) do
        if inside(p.x, p.y, h) then drop_on(d.ability, d.from, h.slot); return end
    end
end

local draw

local function open(opened_from)
    if state.open then return end
    local s, err = eqoa.windows.open_session("spellbook", { opened_from = opened_from or "key" })
    if not s then
        -- a refusal says why (the player is using the game's windows, dead, out of the world...)
        -- "info": the game's log keeps it, and it stays off the screen (a player's own setting is not a fault)
        eqoa.log.write("info", "Spellbook did not open: " .. tostring(err and err.message))
        return
    end
    state.session, state.open, state.scroll, state.result = s, true, 0, nil
    invalidate()
    state.last_req, state.plan, state.drag, state.focus = nil, nil, nil, false
    listen("render", draw)
    listen("abilities.changed", invalidate)
    listen("hotbar.changed", invalidate)
    listen("state.reset", function() invalidate(); icons.tex, icons.due = {}, {} end)
    listen("action.result", function(e) on_result(e.data) end)
    -- the client ends the window's session when the player zones, dies, uses the game's windows or turns the mod off
    listen("windows.session", function(e)
        local d = e.data
        if d.phase == "closed" and state.session and d.ref == state.session.ref then close(true) end
    end)
    listen("input.key", function(e)
        if state.focus then typed(e.data); return end
        if e.data.down and not e.data["repeat"] and e.data.key:lower() == "escape" then close() end
    end)
    listen("pointer.pressed", function(e) pointer_pressed(e.data) end)
    listen("pointer.wheel", function(e)
        local d = e.data
        if (d.steps or 0) ~= 0 and inside(d.x, d.y, state.list_rect) then
            local most = math.max(0, #visible_rows() - ROWS)
            state.scroll = math.max(0, math.min(most, state.scroll + (d.steps > 0 and -1 or 1)))
        end
    end)
    -- Escape is the window's, not the game menu's, while it is up
    state.escape = eqoa.input.claim("Escape", true)
end

function close(from_client)
    if not state.open then return end
    for _, token in ipairs(state.tokens) do eqoa.events.off(token) end
    state.tokens = {}
    focus(false)
    state.plan, state.drag = nil, nil
    if state.escape then eqoa.input.release(state.escape); state.escape = nil end
    if state.session and not from_client then eqoa.windows.close_session(state.session.ref) end
    state.open, state.session, state.hits, state.row_hits, state.slot_hits, state.rect = false, nil, {}, {}, {}, nil
end

eqoa.input.bind({ id = "open", label = "Spellbook", default_key = "shift+k" }, function()
    if state.open then close() else open("key") end
end)
eqoa.events.on("mod.command", function(e)
    if e.data.command == "open" then open("mod") end
end)
eqoa.events.on("disabling", function() close() end)
eqoa.events.on("world.leaving", function() close() end)

function draw()
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end
    icons.frame = icons.frame + 1
    local rows = visible_rows()
    local most = math.max(0, #rows - ROWS)
    if state.scroll > most then state.scroll = most end
    local sel = state.selected and by_id(state.selected)
    -- The rows, top to bottom. Each block of text has a fixed number of lines (the text is wrapped into them), so
    -- the window is the same height whatever it says and nothing runs off it.
    local filter_y = 22
    local list_y = 40
    local list_h = ROWS * LINE
    local head_y = list_y + list_h + 4
    local slots_y = head_y + HEAD_LINES * LINE + 2
    local detail_y = slots_y + 2 * (SLOT_W + 2) + 8
    local status_y = detail_y + (2 + DESC_LINES) * LINE + 2
    local bar_y = status_y + STATUS_LINES * LINE + 2
    local H = bar_y + 16 + PAD
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    state.rect = { x = x0, y = y0, w = W * k, h = H * k }

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    ui.Begin("eqoa_spellbook_layer", nil, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoInputs)
    ui.SetWindowFontScale(k * 11 / 13)
    local dl = ui.GetWindowDrawList()
    local c = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), red = ui.GetColorU32(.843, .098, .129, 1),
        good = ui.GetColorU32(.35, .8, .4, 1), warn = ui.GetColorU32(1, .3, .3, 1),
        white = ui.GetColorU32(1, 1, 1, 1), hl = ui.GetColorU32(.20, .20, .24, 1),
    }
    update_drag(k)
    local function fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    local function frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    local function fits(value, maxw) return ui.CalcTextSize(value) <= maxw * k end
    local function drop_last(value) return value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or "" end
    -- `clip`: a label that is meant to be cut to its box (a name in the list, the short name in a slot). Any other
    -- text that does not fit is a layout fault: it is said in the game's log (debug: not on screen), once per text.
    local function text(x, y, value, color, maxw, clip)
        value = tostring(value)
        if maxw and not fits(value, maxw) then
            if not clip and not state.cut_noted then state.cut_noted = {} end
            if not clip and not state.cut_noted[value] then
                state.cut_noted[value] = true
                eqoa.log.write("debug", "Spellbook cut a text that does not fit: " .. value)
            end
            while #value > 0 and not fits(value, maxw) do value = drop_last(value) end
        end
        dl:AddText(at(x, y), color or c.text, value)
    end
    -- The text broken at spaces into at most `max_lines` lines that each fit `maxw` frame pixels (a word wider than
    -- a line is broken where it must be); what does not fit in the last line ends in "...".
    local function wrap(value, maxw, max_lines)
        local lines, line, more = {}, "", false
        local function push(piece)
            if #lines < max_lines then lines[#lines + 1] = piece else more = true end
        end
        for word in tostring(value):gmatch("%S+") do
            local try = line == "" and word or (line .. " " .. word)
            if fits(try, maxw) then
                line = try
            else
                if line ~= "" then push(line); line = "" end
                while not fits(word, maxw) do
                    local cut = word
                    while #cut > 1 and not fits(cut, maxw) do cut = drop_last(cut) end
                    push(cut)
                    word = word:sub(#cut + 1)
                end
                line = word
            end
        end
        if line ~= "" then push(line) end
        if more and #lines > 0 then
            local last = lines[#lines]
            while #last > 0 and not fits(last .. "...", maxw) do last = drop_last(last) end
            lines[#lines] = last .. "..."
        end
        return lines
    end
    local function paragraph(x, y, value, color, maxw, max_lines)
        for i, line in ipairs(wrap(value, maxw, max_lines)) do text(x, y + (i - 1) * LINE, line, color) end
    end
    local function hit(x, y, w, h, extra)
        local r = { x = x0 + x * k, y = y0 + y * k, w = w * k, h = h * k }
        for key, v in pairs(extra or {}) do r[key] = v end
        return r
    end

    fill(0, 0, W, H, c.surface)
    frame(0, 0, W, H, c.border)
    text(PAD, 6, "Spellbook", c.white)
    -- the hint ends where the X begins (the X is at W - 18)
    local hint = "Shift+K or Esc"
    text(W - 22 - ui.CalcTextSize(hint) / k, 6, hint, c.muted)
    -- the X
    state.hits = {}
    fill(W - 18, 4, 12, 12, c.raised)
    frame(W - 18, 4, 12, 12, c.border)
    text(W - 15, 4, "x", c.text)
    state.hits[1] = hit(W - 18, 4, 12, 12, { run = function() close() end })

    -- the search box and the scope buttons
    local function texture(hash)
        if not hash or hash == 0 then return nil end
        local t = icons.tex[hash]
        if t then return t end
        if (icons.due[hash] or 0) > icons.frame then return nil end
        local info = eqoa.icons.resource(hash)
        if info and info.texture then icons.tex[hash] = info.texture; return info.texture end
        icons.due[hash] = icons.frame + 90  -- the game may not have the art yet: ask again in a moment or two
        return nil
    end
    -- an ability's icon: its background, then its picture over it, into the box; false if the art is not there
    local function picture(a, x, y, size)
        local bg, fg = texture(a.icon_background), texture(a.icon_foreground)
        if not (bg or fg) then return false end
        local u0, u1 = ImVec2.new(0, 0), ImVec2.new(1, 1)
        if bg then dl:AddImage(bg, at(x, y), at(x + size, y + size), u0, u1, c.white) end
        if fg then dl:AddImage(fg, at(x, y), at(x + size, y + size), u0, u1, c.white) end
        return true
    end
    local box_w = 104
    state.search_rect = hit(PAD, filter_y, box_w, 14)
    fill(PAD, filter_y, box_w, 14, c.raised)
    frame(PAD, filter_y, box_w, 14, state.focus and c.white or c.border)
    local inner = box_w - 6 - (state.query ~= "" and 12 or 0)
    if state.query == "" and not state.focus then
        text(PAD + 4, filter_y + 1, "Search", c.muted)
    else
        local shown = state.query
        while #shown > 0 and not fits(shown, inner - 4) do shown = shown:match("^[%z\1-\127\194-\244][\128-\191]*(.*)$") or "" end
        text(PAD + 4, filter_y + 1, shown, c.text)
        if state.focus and math.floor(icons.frame / 20) % 2 == 0 then
            text(PAD + 4 + ui.CalcTextSize(shown) / k, filter_y + 1, "|", c.white)
        end
    end
    state.clear_rect = nil
    if state.query ~= "" then
        text(PAD + box_w - 11, filter_y + 1, "x", c.muted)
        state.clear_rect = hit(PAD + box_w - 13, filter_y, 13, 14)
    end
    if state.focus then eqoa.input.capture_text(true) end
    -- five buttons of one width, from x 118 on, 41 apart (so each is where the player expects it whatever the font)
    local fx, bw = PAD + box_w + 6, 38
    for _, f in ipairs(FILTERS) do
        fill(fx, filter_y, bw, 14, c.raised)
        frame(fx, filter_y, bw, 14, state.filter == f.key and c.white or c.border)
        text(fx + (bw - ui.CalcTextSize(f.label) / k) / 2, filter_y + 1, f.label, state.filter == f.key and c.white or c.text, bw)
        state.hits[#state.hits + 1] = hit(fx, filter_y, bw, 14, {
            run = function() state.filter, state.rows, state.scroll = f.key, nil, 0 end })
        fx = fx + bw + 3
    end

    -- the abilities, grouped by level
    state.row_hits = {}
    state.list_rect = { x = x0 + PAD * k, y = y0 + list_y * k, w = (W - 2 * PAD) * k, h = list_h * k }
    if #rows == 0 then
        local filtered = state.query ~= "" or state.filter ~= "all"
        text(PAD, list_y, (filtered and #abilities() > 0) and "No abilities match" or "No abilities known yet", c.muted)
    end
    for i = 1, ROWS do
        local row = rows[state.scroll + i]
        if row and row.header then
            local y = list_y + (i - 1) * LINE
            fill(PAD, y, W - 2 * PAD, LINE - 1, c.raised)
            text(PAD + 2, y, row.header, c.muted)
        elseif row then
            local a = row.ability
            local y = list_y + (i - 1) * LINE
            local on = state.selected == a.id
            if on then fill(PAD, y, W - 2 * PAD, LINE, c.hl) end
            if not picture(a, PAD + 2, y + 1, ICON) then
                fill(PAD + 2, y + 1, ICON, ICON, c.raised)
                frame(PAD + 2, y + 1, ICON, ICON, c.border)
            end
            local cd = eqoa.cooldowns.get(a.ref)
            local color = (cd and cd.active) and c.warn or c.text
            text(PAD + 16, y, a.name, color, 176, true)
            text(PAD + 196, y, "Lv " .. tostring(a.level or 0), c.muted)
            text(PAD + 236, y, tostring(a.power_cost or 0) .. " pw", c.muted)
            if a.spellbook_slot then text(W - PAD - 24, y, "[" .. a.spellbook_slot .. "]", c.good) end
            state.row_hits[#state.row_hits + 1] = hit(PAD, y, W - 2 * PAD, LINE, { ability = a })
        end
    end
    state.sb_rect = nil
    if #rows > ROWS then
        local sb_x, thumb_h = W - PAD + 1, math.max(10, list_h * ROWS / #rows)
        fill(sb_x, list_y, 6, list_h, c.border)
        fill(sb_x, list_y + (list_h - thumb_h) * (state.scroll / (#rows - ROWS)), 6, thumb_h, c.muted)
        state.sb_rect = { x = x0 + (sb_x - 1) * k, y = y0 + list_y * k, w = 7 * k, h = list_h * k }
    end

    -- the 20 memorized slots: drag an ability onto one, drag a slot onto another to move or swap them, right-click to
    -- clear one; with an ability chosen, a click on an empty slot memorizes it there too
    state.slot_hits = {}
    paragraph(PAD, head_y, "Memorized: drag an ability onto a slot, or choose one and press Memorize. Right-click a slot to clear it.",
        c.muted, W - 2 * PAD, HEAD_LINES)
    local per_row = 10
    local p = eqoa.input.pointer()
    local dragging = state.drag and state.drag.moved
    local over
    for slot = 1, 20 do
        local col, row = (slot - 1) % per_row, math.floor((slot - 1) / per_row)
        local sx, sy = PAD + col * (SLOT_W + 2), slots_y + row * (SLOT_W + 2)
        local a = by_slot(slot)
        fill(sx, sy, SLOT_W, SLOT_W, c.raised)
        local h = hit(sx, sy, SLOT_W, SLOT_W, { slot = slot, ability = a })
        state.slot_hits[#state.slot_hits + 1] = h
        if dragging and inside(p.x, p.y, h) then over = slot end
        frame(sx, sy, SLOT_W, SLOT_W, (dragging and over == slot) and c.good or ((a and state.selected == a.id) and c.white or c.border))
        if a then
            if not picture(a, sx + 2, sy + 2, SLOT_W - 4) then text(sx + 2, sy + 14, a.name:sub(1, 5), c.text, SLOT_W - 3, true) end
        end
        text(sx + 2, sy + 1, tostring(slot), c.muted)
    end

    -- the chosen ability
    if sel then
        text(PAD, detail_y, sel.name, c.white, W - 2 * PAD)
        text(PAD, detail_y + LINE, string.format("Level %d   Power %d   Cast %.1fs   Range %d", sel.level or 0,
            sel.power_cost or 0, sel.cast_time or 0, math.floor(sel.range or 0)), c.text, W - 2 * PAD)
        paragraph(PAD, detail_y + 2 * LINE, sel.description or "", c.muted, W - 2 * PAD, DESC_LINES)
    else
        text(PAD, detail_y, "Choose an ability to see it here", c.muted)
    end
    -- what the game said about the last action, on lines of its own under the description
    if state.result then
        paragraph(PAD, status_y, state.result.text, state.result.ok == false and c.warn or c.muted, W - 2 * PAD,
            STATUS_LINES)
    end

    -- the buttons
    local bx = PAD
    local function button(label, enabled, run)
        local bw = ui.CalcTextSize(label) / k + 12
        fill(bx, bar_y, bw, 16, c.raised)
        frame(bx, bar_y, bw, 16, enabled and c.border or c.surface)
        text(bx + 6, bar_y + 2, label, enabled and c.text or c.muted)
        if enabled then state.hits[#state.hits + 1] = hit(bx, bar_y, bw, 16, { run = run }) end
        bx = bx + bw + 4
    end
    button("Memorize", sel ~= nil and sel.spellbook_slot == nil, function() memorize(sel) end)
    button("Cast", sel ~= nil and sel.spellbook_slot ~= nil, function()
        run("Cast", function() return eqoa.actions.cast(sel.ref) end)
    end)
    button("Unmemorize", sel ~= nil and sel.spellbook_slot ~= nil, function()
        run("Unmemorize", function() return eqoa.actions.unmemorize_ability(sel.ref) end)
    end)
    button("Unmemorize all", true, function()
        run("Unmemorize all", function() return eqoa.actions.unmemorize_all() end)
    end)

    -- the ability being dragged follows the pointer
    if state.drag and state.drag.moved then
        local gx, gy = (p.x - x0) / k - 9, (p.y - y0) / k - 9
        fill(gx - 1, gy - 1, 20, 20, c.raised)
        frame(gx - 1, gy - 1, 20, 20, c.white)
        if not picture(state.drag.ability, gx, gy, 18) then text(gx + 2, gy + 3, state.drag.ability.name:sub(1, 3), c.text) end
    end

    eqoa.input.claim_pointer({ x = x0, y = y0, width = W * k, height = H * k })
    ui.SetWindowFontScale(1)
    ui.End()
end
