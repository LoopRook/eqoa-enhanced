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

-- What the mod may call does not change while it runs: each capability is asked once, not every drawn frame.
local known_caps = {}
local function has(path)
    local yes = known_caps[path]
    if yes == nil then
        yes = eqoa.capabilities.has(path) and true or false
        known_caps[path] = yes
    end
    return yes
end

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

-- The short time the game's own buff strip uses under an icon: "45s", "3m", "3h"; nothing where the server keeps no end.
local function short_clock(seconds)
    if seconds == nil then return "" end
    if seconds < 60 then return string.format("%ds", math.ceil(math.max(seconds, 0))) end
    if seconds < 3600 then return string.format("%dm", math.floor(seconds / 60)) end
    return string.format("%dh", math.floor(seconds / 3600))
end

-- The UI scale, as the other mods read it: every size below is a base pixel times this.
local function ui_scale()
    local f = eqoa.hud and eqoa.hud.frame and eqoa.hud.frame() or nil
    local k = f and math.min(f.scale_x or 1, f.scale_y or 1) or 1
    if not (k > 0) then k = 1 end
    return k
end

-- An effect's icon: asked for by its resource hash and kept; one the game has not loaded yet is asked again a second later
-- (a few times, then every ten seconds), and meanwhile the effect shows a framed square with its first letters.
local ICON_OPTS = { trim = true, trim_color = { r = 0, g = 0, b = 0, a = 1 } }
local ICON_TRIES = 5
local icons = {}
local function icon_texture(hash)
    if not hash or hash == 0 or not has("eqoa.icons.resource") then return nil end
    local e = icons[hash]
    if e == nil then e = { tries = 0, at = 0 }; icons[hash] = e end
    if e.tex then return e.tex end
    local now = ui.GetTime()
    if now < e.at then return nil end
    local info = eqoa.icons.resource(hash, ICON_OPTS)
    if info and info.texture then e.tex = info.texture; return e.tex end
    e.tries = e.tries + 1
    e.at = now + (e.tries >= ICON_TRIES and 10 or 1)
    if e.tries >= ICON_TRIES then e.tries = 0 end
    return nil
end

-- the first two characters of a name (by characters, never bytes)
local function first_letters(name)
    local out, n = "", 0
    for ch in tostring(name or "?"):gmatch("[%z\1-\127\194-\244][\128-\191]*") do
        out, n = out .. ch, n + 1
        if n == 2 then break end
    end
    return out ~= "" and out or "?"
end

-- what the mod last drew, read by its tests
frames_probe = { cells = {} }

-- The player's effects as a strip of icons, wrapping to more rows at the window's width. Under each icon: the time left in
-- short form. Hover: the name, what it does and the exact time. A click takes off an effect the server calls removable (those
-- have a green frame: the server only allows beneficial ones; the record does not say which of the others are harmful).
local function icon_strip(own)
    local k = ui_scale()
    local S, gap = math.floor(32 * k + 0.5), math.max(2, math.floor(4 * k + 0.5))
    local avail = ui.GetContentRegionAvail()
    local x0, y0 = ui.GetCursorScreenPos()
    local lh = ui.GetTextLineHeight()
    local cell_h = S + lh + 2
    local cols = math.max(1, math.floor((avail + gap) / (S + gap)))
    local dl = ui.GetWindowDrawList()
    local col = {
        face = ui.GetColorU32(.12, .12, .14, 1), edge = ui.GetColorU32(.35, .35, .4, 1), good = ui.GetColorU32(.35, .8, .4, 1),
        text = ui.GetColorU32(.91, .91, .91, 1), soon = ui.GetColorU32(1, .55, .25, 1), white = ui.GetColorU32(1, 1, 1, 1),
    }
    frames_probe.cells = {}
    for i, b in ipairs(own) do
        local c, row = (i - 1) % cols, math.floor((i - 1) / cols)
        local cx, cy = x0 + c * (S + gap), y0 + row * (cell_h + gap)
        ui.SetCursorScreenPos(cx, cy)
        local clicked = ui.InvisibleButton("##buff" .. b.index, S, cell_h)
        local hovered = ui.IsItemHovered()
        dl:AddRectFilled(ImVec2.new(cx, cy), ImVec2.new(cx + S, cy + S), col.face)
        local tex = icon_texture(b.icon)
        local letters
        if tex then
            dl:AddImage(tex, ImVec2.new(cx + 1, cy + 1), ImVec2.new(cx + S - 1, cy + S - 1), ImVec2.new(0, 0), ImVec2.new(1, 1), col.white)
        else
            -- no icon: the first letters of the name, in the framed square
            letters = first_letters(b.name)
            local lw = ui.CalcTextSize(letters)
            dl:AddText(ImVec2.new(cx + (S - lw) / 2, cy + (S - lh) / 2), col.text, letters)
        end
        dl:AddRect(ImVec2.new(cx, cy), ImVec2.new(cx + S, cy + S), (b.removable and col.good) or col.edge)
        local t = short_clock(b.time_left)
        if t ~= "" then
            local tw = ui.CalcTextSize(t)
            local soon = b.time_left ~= nil and b.time_left <= 10
            dl:AddText(ImVec2.new(cx + (S - tw) / 2, cy + S + 1), soon and col.soon or col.text, t)
        end
        frames_probe.cells[#frames_probe.cells + 1] = { name = b.name, time = t, x = cx, y = cy, w = S, h = cell_h,
            icon = tex ~= nil, letters = letters, removable = b.removable == true, hovered = hovered }
        if clicked and b.removable then
            local req, err = eqoa.actions.remove_effect(b.ref)
            if not req then eqoa.log.write("info", "frames: " .. tostring(err and err.code)) end
        end
        -- a tooltip only while the pointer is on this effect (SetTooltip draws at once, wherever it is called)
        if hovered then ui.SetTooltip(effect_tip(b)) end
    end
    frames_probe.cols, frames_probe.right = cols, x0 + avail
    local rows = math.ceil(#own / cols)
    ui.SetCursorScreenPos(x0, y0)
    ui.Dummy(avail, math.max(1, rows * (cell_h + gap) - gap))
end

-- Show the player's effects as icons (default) or as the old list of labelled buttons; not kept between sessions.
local show_list = false

local function effects_window()
    local k = ui_scale()
    ui.SetNextWindowPos(60, 60, FIRST_USE)
    ui.SetNextWindowSize(math.floor(230 * k), math.floor(150 * k), FIRST_USE)
    if not ui.Begin("Effects") then ui.End() return end
    ui.SetWindowFontScale(k * 11 / 13)
    local buffs = has("eqoa.effects.buffs") and eqoa.effects.buffs() or nil
    -- the player's own effects are icons (or buttons in the list view); the pet's effects are never buttons: they are the
    -- lines of the dimmer Pet section below, each once, from the buff list when the server has one, else from eqoa.effects.pet
    local pet_lines = {}
    if buffs then
        local mine = {}
        for _, b in ipairs(buffs) do
            if b.whose == "pet" then
                pet_lines[#pet_lines + 1] = { name = b.name, time_left = b.time_left }
            else
                mine[#mine + 1] = b
            end
        end
        if #mine == 0 then
            ui.TextDisabled("no effects")
            frames_probe.cells = {}
        elseif show_list then
            frames_probe.cells = {}
            for _, b in ipairs(mine) do
                local label = string.format("%s  %s##buff%d", b.name, clock(b.time_left), b.index)
                if ui.Button(label) and b.removable then
                    local req, err = eqoa.actions.remove_effect(b.ref)
                    if not req then eqoa.log.write("info", "frames: " .. tostring(err and err.code)) end
                end
                -- a tooltip only while the pointer is on this effect (SetTooltip draws at once, wherever it is called)
                if ui.IsItemHovered() then ui.SetTooltip(effect_tip(b)) end
            end
        else
            icon_strip(mine)
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
    if #pet_lines == 0 then
        local pet = has("eqoa.effects.pet") and eqoa.effects.pet() or nil
        for _, e in ipairs(pet or {}) do pet_lines[#pet_lines + 1] = e end
    end
    if #pet_lines > 0 then
        ui.Separator()
        for _, e in ipairs(pet_lines) do
            ui.TextDisabled(string.format("Pet: %s  %s", e.name, clock(e.time_left)))
        end
    end
    show_list = ui.Checkbox("Show as list", show_list)
    ui.SetWindowFontScale(1)
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

local function invitable(player, target)
    if target.kind ~= "player" or target.id == player.id then return false end
    local group = eqoa.group.get()
    if group then
        for _, m in ipairs(group.members) do
            if m.id == target.id then return false end
        end
    end
    return true
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
    -- each button only where it applies: Invite for another player who is not in your group yet
    local shown = false
    if invitable(player, target) then
        if ui.Button("Invite to group") then eqoa.actions.group_invite() end
        shown = true
    end
    if player and player.has_pet then
        -- the pet cannot attack yourself or itself; back off applies whatever is targeted
        if target.id ~= player.id and target.id ~= player.pet_id then
            if shown then ui.SameLine() end
            if ui.Button("Pet attack") then eqoa.actions.pet_attack() end
            shown = true
        end
        if shown then ui.SameLine() end
        if ui.Button("Pet back off") then eqoa.actions.pet_back_off() end
    end
    ui.End()
end

-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end

eqoa.events.on("render", function()
    if map_up() then return end
    local player = eqoa.player.get()
    if not player then return end            -- not in a world
    local target = eqoa.target.get()
    effects_window()
    cast_window(player, target)
    target_window(player, target)
end)
