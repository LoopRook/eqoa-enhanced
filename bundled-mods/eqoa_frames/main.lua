-- EQOA Frames (demo): the data a unit-frame mod gets from the client, drawn plainly.
-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- Three small windows you can drag: your effects with the time left (click one to take it off where the game
-- allows it), a cast bar for you and for your target, and the target's title and consider colour with buttons to
-- invite it to your group and to send your pet in or call it off.
-- Nothing is drawn or sent until this mod is switched on in Mods & Addons.

local ui = eqoa.imgui

-- ImGuiCond_FirstUseEver: the windows start side by side, and keep where the player drags them
local FIRST_USE = 4

local function has(path) return eqoa.capabilities.has(path) end

-- "1:05", "12s" or "--" where the server keeps no end for an effect.
local function clock(seconds)
    if seconds == nil then return "--" end
    if seconds >= 60 then return string.format("%d:%02d", math.floor(seconds / 60), math.floor(seconds % 60)) end
    return string.format("%ds", math.ceil(seconds))
end

-- What hovering an effect says: its name, what it does where the client knows (the description of an ability the
-- player has with that name), the time left, and how to take it off where that is allowed.
local function effect_tip(b)
    local lines = { b.name }
    if b.description and b.description ~= "" then
        lines[#lines + 1] = b.description
    else
        lines[#lines + 1] = "(the game does not say what this one does)"
    end
    if b.time_left ~= nil then lines[#lines + 1] = "time left: " .. clock(b.time_left) end
    if b.removable then lines[#lines + 1] = "click to take it off" end
    return table.concat(lines, "\n")
end

local function effects_window()
    ui.SetNextWindowPos(60, 60, FIRST_USE)
    if not ui.Begin("Effects") then ui.End() return end
    local buffs = has("eqoa.effects.buffs") and eqoa.effects.buffs() or nil
    if buffs then
        if #buffs == 0 then ui.TextDisabled("no effects") end
        for _, b in ipairs(buffs) do
            local label = string.format("%s  %s##buff%d", b.name, clock(b.time_left), b.index)
            if b.whose == "pet" then label = "Pet: " .. label end
            if ui.Button(label) and b.removable then
                local req, err = eqoa.actions.remove_effect(b.ref)
                if not req then eqoa.log.write("info", "frames: " .. tostring(err and err.code)) end
            end
            -- a tooltip only while the pointer is on this effect (SetTooltip draws at once, wherever it is called)
            if ui.IsItemHovered() then ui.SetTooltip(effect_tip(b)) end
        end
    else
        -- no buff window from this server: the game's own strip has names and no times
        local list = has("eqoa.effects.list") and eqoa.effects.list() or nil
        if list and #list > 0 then
            for _, e in ipairs(list) do ui.Text(e.name) end
        else
            ui.TextDisabled("no effects")
        end
    end
    local pet = has("eqoa.effects.pet") and eqoa.effects.pet() or nil
    if pet and #pet > 0 then
        ui.Separator()
        for _, e in ipairs(pet) do ui.Text(string.format("Pet: %s  %s", e.name, clock(e.time_left))) end
    end
    ui.End()
end

local function cast_bar(who, cast)
    if not cast then return end
    ui.Text(who)
    ui.ProgressBar(cast.fraction, 200, 14, string.format("%.1fs", cast.remaining))
end

local function cast_window(player, target)
    ui.SetNextWindowPos(60, 240, FIRST_USE)
    if not ui.Begin("Casting") then ui.End() return end
    local mine = player and eqoa.casts.get(player.id) or nil
    local theirs = target and eqoa.casts.get(target.id) or nil
    if not mine and not theirs then ui.TextDisabled("nobody you watch is casting") end
    cast_bar("You", mine)
    if theirs then cast_bar(target.name, theirs) end
    ui.End()
end

local function target_window(player, target)
    ui.SetNextWindowPos(320, 60, FIRST_USE)
    if not ui.Begin("Target") then ui.End() return end
    if not target then ui.TextDisabled("no target") ui.End() return end
    local info = eqoa.target.info()
    local name = target.name
    -- a consider code the client does not know has no colour: draw the name plainly
    if info and info.consider and info.consider.color then
        local c = info.consider.color
        ui.TextColored(c.r, c.g, c.b, c.a, name)
    else
        ui.Text(name)
    end
    if target.level then ui.SameLine() ui.Text("level " .. tostring(target.level)) end
    if info and info.title then ui.Text(info.title) end
    if info and info.guild then ui.TextDisabled("<" .. info.guild .. ">") end
    if ui.Button("Invite to group") then eqoa.actions.group_invite() end
    if player and player.has_pet then
        ui.SameLine()
        if ui.Button("Pet attack") then eqoa.actions.pet_attack() end
        ui.SameLine()
        if ui.Button("Pet back off") then eqoa.actions.pet_back_off() end
    end
    ui.End()
end

eqoa.events.on("render", function()
    local player = eqoa.player.get()
    if not player then return end            -- not in a world
    local target = eqoa.target.get()
    effects_window()
    cast_window(player, target)
    target_window(player, target)
end)
