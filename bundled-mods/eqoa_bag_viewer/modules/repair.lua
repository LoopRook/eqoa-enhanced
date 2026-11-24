-- Bag Viewer: repair at a blacksmith. Lists each damaged item with its repair cost (click to repair), "Repair all (N
-- coins)" after one Yes that names the total, and an option to repair everything when a smith opens.
-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- The game's own blacksmith window is left as it is: this panel sits beside it. One repair is sent at a time, and the
-- next only after the server has answered the last (the smith's `repair_pending`). Nothing is ever sent twice.
local ui = eqoa.imgui
local HP = require("helpers")

local R = {}
local FILE = "repair_options.txt"
local W = 300
local MAX_ROWS = 10

R.auto = false -- "Repair automatically at a blacksmith": off by default
R.loaded = false
R.opened = false -- a smith has been open since the last time none was
R.run = nil -- {queue = {...}, total = N, spent = N, done = N, auto = bool, wait = request or nil}
R.ask = nil -- {n = items, total = coins}: the one question of "Repair all"
R.status = nil

local function coins(n) return string.format("%d coin%s", n, n == 1 and "" or "s") end

local function short(text, max)
    text = tostring(text or "")
    if #text <= max then return text end
    return text:sub(1, max - 2) .. ".."
end

function R.load()
    R.auto = false
    if eqoa.files and eqoa.files.read then
        local data = eqoa.files.read(FILE)
        if type(data) == "string" and data:match("auto=1") then R.auto = true end
    end
    R.loaded = true
end

function R.save()
    if not (eqoa.files and eqoa.files.write) then return end
    eqoa.files.write(FILE, "auto=" .. (R.auto and "1" or "0") .. "\n")
end

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

local function total_of(list)
    local t = 0
    for _, it in ipairs(list) do t = t + it.cost end
    return t
end

local function finish(text, ok)
    local run = R.run
    R.run = nil
    R.status = {text = text, ok = ok}
    -- one chat line (a line in the Bag Viewer's result row where the client cannot say one)
    if run and (run.auto or run.done > 0) then HP.say(text, ok) end
end

local function summary(run)
    return string.format("Repaired %d item%s for %s", run.done, run.done == 1 and "" or "s", coins(run.spent))
end

-- Starts the one-at-a-time run over what is damaged now (`auto`: no question was asked: the option was on)
function R.start(s, auto)
    local list = R.damaged(s)
    if #list == 0 then
        if not auto then R.status = {text = "Nothing needs repair", ok = nil} end
        return
    end
    if R.run then return end
    local queue = {}
    for _, it in ipairs(list) do queue[#queue + 1] = {key = it.inventory_key, id = it.id, name = it.name, cost = it.cost} end
    R.run = {queue = queue, total = total_of(list), spent = 0, done = 0, auto = auto == true, wait = nil}
end

-- One repair now (a click on a row)
function R.repair_one(it)
    if R.run then return end
    local req, err = eqoa.actions.blacksmith_repair({inventory_key = it.inventory_key, id = it.id})
    if not req then
        R.status = {text = (err and (err.message or err.code)) or "Refused", ok = false}
        return
    end
    R.run = {queue = {}, total = it.cost, spent = 0, done = 0, auto = false, wait = req,
        current = {name = it.name, cost = it.cost}, single = true}
    R.status = {text = "Repairing " .. short(it.name, 24) .. "...", ok = nil}
end

local function step(s)
    local run = R.run
    if not run or run.wait then return end
    if s.repair_pending then return end
    local nxt = table.remove(run.queue, 1)
    if not nxt then
        finish(summary(run), true)
        return
    end
    -- still damaged, and still the same item
    local now
    for _, it in ipairs(s.items or {}) do
        if it.inventory_key == nxt.key and it.id == nxt.id then now = it end
    end
    if not now or not now.can_repair or (now.cost or 0) <= 0 then return end
    local req, err = eqoa.actions.blacksmith_repair({inventory_key = nxt.key, id = nxt.id})
    if not req then
        finish(summary(run) .. " (stopped: " .. ((err and (err.message or err.code)) or "refused") .. ")", false)
        return
    end
    run.wait, run.current = req, nxt
end

function R.on_action_result(d)
    local run = R.run
    if not run or not run.wait or d.request_id ~= run.wait then return end
    if d.ok == nil or d.phase == "dispatch" then return end
    run.wait = nil
    if d.ok == true then
        run.done = run.done + 1
        run.spent = run.spent + (run.current and run.current.cost or 0)
        if run.single then finish(summary(run), true) end
    else
        local why = d.reason or d.code or "refused"
        finish(summary(run) .. " (stopped: " .. tostring(why) .. ")", run.done > 0)
    end
end

-- "Repair all": one question that names the total; the Yes starts the run (a changed total asks again)
function R.begin_ask(s)
    local list = R.damaged(s)
    if #list > 0 and not R.run then R.ask = {n = #list, total = total_of(list)} end
end

function R.answer(s, yes)
    local asked = R.ask
    R.ask = nil
    if not yes or not asked then return end
    if total_of(R.damaged(s)) == asked.total then
        R.start(s, false)
    else
        R.status = {text = "The prices changed: ask again", ok = false}
    end
end

local function draw_panel(s)
    local dw = ui.GetDisplaySize()
    if dw <= 0 then return end
    ui.SetNextWindowPos(24, 96, ImGuiCond.FirstUseEver)
    ui.SetNextWindowSize(W, 0, ImGuiCond.Always)
    if ui.Begin("Repair##eqoa_bag_viewer_repair", nil,
        ImGuiWindowFlags.NoCollapse + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.AlwaysAutoResize) then
        local list = R.damaged(s)
        local total = total_of(list)
        if #list == 0 then
            ui.TextWrapped("Nothing needs repair.")
        else
            local shown = 0
            for _, it in ipairs(list) do
                shown = shown + 1
                if shown > MAX_ROWS then break end
                ui.Text(string.format("%s  %d", short(it.name, 20), it.cost))
                ui.SameLine()
                if ui.Button("Repair##rep_" .. tostring(it.inventory_key)) then R.repair_one(it) end
            end
            if #list > MAX_ROWS then ui.TextDisabled(string.format("and %d more", #list - MAX_ROWS)) end
        end
        ui.Separator()
        ui.TextWrapped(string.format("Repairing everything: %s. You carry %s.", coins(total), coins(s.coin or 0)))
        if R.ask then
            ui.TextWrapped(string.format("Repair %d item%s for %s?", R.ask.n, R.ask.n == 1 and "" or "s", coins(R.ask.total)))
            if ui.Button("Yes##rep_yes") then R.answer(s, true) end
            ui.SameLine()
            if ui.Button("No##rep_no") then R.answer(s, false) end
        elseif #list > 0 and not R.run then
            if ui.Button(string.format("Repair all (%s)##rep_all", coins(total))) then R.begin_ask(s) end
        end
        local want = ui.Checkbox("Repair automatically at a blacksmith", R.auto)
        if want ~= R.auto then
            R.auto = want == true
            R.save()
        end
        if R.status then ui.TextWrapped(R.status.text) end
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
        return
    end
    if not R.opened then
        R.opened = true
        -- the option: everything is repaired as the smith opens, with no question; one chat line gives the total
        if R.auto then R.start(s, true) end
    end
    step(s)
    draw_panel(s)
end

eqoa.events.on("render", on_render)
eqoa.events.on("action.result", function(e) R.on_action_result(e.data or {}) end)
eqoa.events.on("state.reset", function() R.opened, R.run, R.ask, R.status = false, nil, nil, nil end)
eqoa.events.on("disabling", function() R.run, R.ask = nil, nil end)

return R
