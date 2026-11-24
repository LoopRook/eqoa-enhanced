-- Bag Viewer: repair at a blacksmith. Lists each damaged item with its repair cost (click to repair), "Repair all (N
-- coins)" after one Yes that names the total, and an option to repair everything when a smith opens.
-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- The game's own blacksmith window is left as it is: this panel sits beside it. One repair is sent at a time, and the
-- next only after the smith's own state shows the last one done. What is told as repaired is what the smith's list
-- shows repaired, never what was only sent. Nothing is ever sent twice.
local ui = eqoa.imgui
local HP = require("helpers")

local R = {}
local FILE = "repair_options.txt"
local W = 300
local LAST_SECS = 10

R.auto_by_char = {} -- "Repair automatically at a blacksmith": per character, off by default
R.loaded = false
R.opened = false -- a smith has been open since the last time none was
R.run = nil -- {queue, total, spent, done, skipped, auto, wait, current}
R.ask = nil -- {n = items, total = coins, skipped = items}: the one question of "Repair all"
R.status = nil
R.last = nil -- the last result, kept up for a few seconds after the smith closes

local function coins(n) return string.format("%d coin%s", n, n == 1 and "" or "s") end

local function short(text, max)
    text = tostring(text or "")
    if #text <= max then return text end
    return text:sub(1, max - 2) .. ".."
end

-- ---------------------------------------------------------------- the option, per character

function R.character()
    local p = eqoa.player and eqoa.player.get and eqoa.player.get()
    return p and p.name or nil
end

function R.auto_on()
    local who = R.character()
    return who ~= nil and R.auto_by_char[who] == true
end

function R.load()
    R.auto_by_char = {}
    if eqoa.files and eqoa.files.read then
        local data = eqoa.files.read(FILE)
        if type(data) == "string" then
            for line in data:gmatch("[^\r\n]+") do
                local who = line:match("^auto:(.+)=1$")
                if who then R.auto_by_char[who] = true end
            end
        end
    end
    R.loaded = true
end

function R.save()
    if not (eqoa.files and eqoa.files.write) then return end
    local names = {}
    for who, on in pairs(R.auto_by_char) do if on then names[#names + 1] = who end end
    table.sort(names)
    local lines = {}
    for _, who in ipairs(names) do lines[#lines + 1] = "auto:" .. who .. "=1" end
    eqoa.files.write(FILE, table.concat(lines, "\n") .. (#lines > 0 and "\n" or ""))
end

function R.set_auto(on)
    local who = R.character()
    if not who then return end
    R.auto_by_char[who] = on and true or nil
    R.save()
end

-- ---------------------------------------------------------------- reading the smith

local function smith()
    if not (eqoa.blacksmith and eqoa.blacksmith.get) then return nil end
    return (eqoa.blacksmith.get())
end

-- what the smith would repair now, in the smith's order
function R.damaged(s)
    local out = {}
    for _, it in ipairs(s and s.items or {}) do
        if it.can_repair and (it.cost or 0) > 0 then out[#out + 1] = it end
    end
    return out
end

-- What "Repair all" does with the coin carried: each item in the smith's order that can still be paid for, the rest
-- skipped (counted, and told at the end)
function R.plan(s)
    local items, total, skipped = {}, 0, 0
    local left = s and s.coin or 0
    for _, it in ipairs(R.damaged(s)) do
        if it.cost <= left then
            items[#items + 1] = it
            left = left - it.cost
            total = total + it.cost
        else
            skipped = skipped + 1
        end
    end
    return {items = items, total = total, skipped = skipped}
end

local function find(s, key, id)
    for _, it in ipairs(s and s.items or {}) do
        if it.inventory_key == key and it.id == id then return it end
    end
    return nil
end

-- ---------------------------------------------------------------- a run

local function summary(run)
    local text = string.format("Repaired %d item%s for %s", run.done, run.done == 1 and "" or "s", coins(run.spent))
    if (run.skipped or 0) > 0 then
        text = text .. string.format("; %d skipped (not enough coin)", run.skipped)
    end
    return text
end

local function finish(text, ok)
    local run = R.run
    R.run = nil
    R.status = {text = text, ok = ok}
    R.last = {text = text, finish = os.clock() + LAST_SECS}
    -- the line in the Bags window's result row, and the game's log (the client has no way yet to say a line in chat)
    if run and (run.auto or run.done > 0) then HP.say(text, ok) end
end

-- Starts the one-at-a-time run over what can be paid for (`auto`: no question was asked: the option was on)
function R.start(s, auto)
    if R.run then return end
    local plan = R.plan(s)
    if #plan.items == 0 then
        if not auto or plan.skipped > 0 then
            R.status = {text = plan.skipped > 0 and string.format("%d skipped (not enough coin)", plan.skipped) or "Nothing needs repair", ok = nil}
        end
        return
    end
    local queue = {}
    for _, it in ipairs(plan.items) do queue[#queue + 1] = {key = it.inventory_key, id = it.id, name = it.name, cost = it.cost} end
    R.run = {queue = queue, total = plan.total, spent = 0, done = 0, skipped = plan.skipped, auto = auto == true}
end

-- One repair now (a click on a row)
function R.repair_one(it)
    if R.run then return end
    local req, err = eqoa.actions.blacksmith_repair({inventory_key = it.inventory_key, id = it.id})
    if not req then
        R.status = {text = (err and (err.message or err.code)) or "Refused", ok = false}
        return
    end
    R.run = {queue = {}, total = it.cost, spent = 0, done = 0, skipped = 0, auto = false, wait = req,
        current = {key = it.inventory_key, id = it.id, name = it.name, cost = it.cost}}
    R.status = {text = "Repairing " .. short(it.name, 24) .. "...", ok = nil}
end

local function step(s)
    local run = R.run
    if not run or run.wait then return end
    -- the item last sent: told as repaired only when the smith's list shows it repaired
    if run.current then
        local cur = run.current
        local now = find(s, cur.key, cur.id)
        if not now or not now.can_repair or (now.cost or 0) <= 0 then
            run.done = run.done + 1
            run.spent = run.spent + cur.cost
            run.current = nil
        elseif s.repair_pending then
            return
        else
            run.current = nil
            finish(summary(run) .. " (stopped: no answer from the smith for " .. short(cur.name, 24) .. ")", false)
            return
        end
    end
    local nxt = table.remove(run.queue, 1)
    if not nxt then
        finish(summary(run), true)
        return
    end
    if s.repair_pending then
        table.insert(run.queue, 1, nxt)
        return
    end
    local now = find(s, nxt.key, nxt.id)
    if not now or not now.can_repair or (now.cost or 0) <= 0 then return end -- already whole
    if now.cost > nxt.cost and not run.auto then
        finish(summary(run) .. " (stopped: the price of " .. short(nxt.name, 24) .. " changed)", false)
        return
    end
    local req, err = eqoa.actions.blacksmith_repair({inventory_key = nxt.key, id = nxt.id})
    if not req then
        finish(summary(run) .. " (stopped: " .. ((err and (err.message or err.code)) or "refused") .. ")", false)
        return
    end
    nxt.cost = now.cost
    run.wait, run.current = req, nxt
end

-- the request was sent (ok) or refused: a refusal ends the run, a sent repair is counted only when the list shows it
function R.on_action_result(d)
    local run = R.run
    if not run or not run.wait or d.request_id ~= run.wait then return end
    if d.ok == nil or d.phase == "dispatch" then return end
    run.wait = nil
    if d.ok ~= true then
        local why = d.reason or d.code or "refused"
        run.current = nil
        finish(summary(run) .. " (stopped: " .. tostring(why) .. ")", run.done > 0)
    end
end

-- "Repair all": one question that names the total; the Yes starts the run (a changed total asks again)
function R.begin_ask(s)
    if R.run then return end
    local plan = R.plan(s)
    if #plan.items > 0 then R.ask = {n = #plan.items, total = plan.total, skipped = plan.skipped} end
end

function R.answer(s, yes)
    local asked = R.ask
    R.ask = nil
    if not yes or not asked then return end
    local plan = R.plan(s)
    if plan.total == asked.total and #plan.items == asked.n then
        R.start(s, false)
    else
        R.status = {text = "The prices changed: ask again", ok = false}
    end
end

-- ---------------------------------------------------------------- drawing

local function draw_panel(s)
    local dw = ui.GetDisplaySize()
    if dw <= 0 then return end
    ui.SetNextWindowPos(24, 96, ImGuiCond.FirstUseEver)
    ui.SetNextWindowSize(W, 0, ImGuiCond.Always)
    if ui.Begin("Repair##eqoa_bag_viewer_repair", nil,
        ImGuiWindowFlags.NoCollapse + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.AlwaysAutoResize) then
        local list = R.damaged(s)
        local plan = R.plan(s)
        if #list == 0 then
            ui.TextWrapped("Nothing needs repair.")
        else
            -- every damaged item, scrolling past eight rows
            if ui.BeginChild("##rep_rows", 0, math.min(#list, 8) * 24 + 8, ImGuiChildFlags.Border) then
                for _, it in ipairs(list) do
                    ui.Text(string.format("%s  %d", short(it.name, 20), it.cost))
                    ui.SameLine()
                    if ui.Button("Repair##rep_" .. tostring(it.inventory_key)) then R.repair_one(it) end
                end
            end
            ui.EndChild()
        end
        ui.Separator()
        ui.TextWrapped(string.format("Repairing everything: %s. You carry %s.", coins(plan.total), coins(s.coin or 0)))
        if R.ask then
            local q = string.format("Repair %d item%s for %s?", R.ask.n, R.ask.n == 1 and "" or "s", coins(R.ask.total))
            if R.ask.skipped > 0 then q = q .. string.format(" %d cost more than you carry and are skipped.", R.ask.skipped) end
            ui.TextWrapped(q)
            if ui.Button("Yes##rep_yes") then R.answer(s, true) end
            ui.SameLine()
            if ui.Button("No##rep_no") then R.answer(s, false) end
        elseif #plan.items > 0 and not R.run then
            if ui.Button(string.format("Repair all (%s)##rep_all", coins(plan.total))) then R.begin_ask(s) end
        end
        local want = ui.Checkbox("Repair automatically at a blacksmith", R.auto_on())
        if want ~= R.auto_on() then R.set_auto(want == true) end
        if R.status then ui.TextWrapped(R.status.text) end
    end
    ui.End()
end

-- the last result stays up for a few seconds after the smith closes (the player may leave at once)
local function draw_last()
    if not R.last then return end
    if os.clock() > R.last.finish then
        R.last = nil
        return
    end
    local dw = ui.GetDisplaySize()
    if dw <= 0 then return end
    ui.SetNextWindowPos(24, 96, ImGuiCond.FirstUseEver)
    ui.SetNextWindowSize(W, 0, ImGuiCond.Always)
    if ui.Begin("Repair##eqoa_bag_viewer_repair_done", nil,
        ImGuiWindowFlags.NoCollapse + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.AlwaysAutoResize) then
        ui.TextWrapped(R.last.text)
    end
    ui.End()
end

local function on_render()
    if not R.loaded then R.load() end
    local s = smith()
    if not s then
        if R.opened then
            R.opened = false
            R.ask = nil
            R.run = nil
            R.status = nil
        end
        draw_last()
        return
    end
    if not R.opened then
        R.opened = true
        R.last = nil
        -- the option: everything that can be paid for is repaired as the smith opens, with no question
        if R.auto_on() then R.start(s, true) end
    end
    step(s)
    draw_panel(s)
end

eqoa.events.on("render", on_render)
eqoa.events.on("action.result", function(e) R.on_action_result(e.data or {}) end)
eqoa.events.on("state.reset", function() R.opened, R.run, R.ask, R.status = false, nil, nil, nil end)
eqoa.events.on("disabling", function() R.run, R.ask = nil, nil end)

return R
