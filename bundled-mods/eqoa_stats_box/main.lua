-- EQOA Stats Box Mod
-- Authentic 2-column x 8-row stat display matching the Summary order, with
-- click-to-open Summary and the rebindable B key (as the Controls page binds).

local gold = { 0.95, 0.82, 0.5, 1.0 }
local off_white = { 0.96, 0.95, 0.9, 1.0 }
local raised_blue = { 0.45, 0.78, 1.0, 1.0 }
local lowered_red = { 1.0, 0.45, 0.4, 1.0 }

eqoa.options.toggle({ id = "stats_box", label = "Stats Box", default = false })

eqoa.input.bind({ id = "toggle", label = "Toggle Stats Box", default_key = "y" }, function()
    eqoa.actions.toggle_stats_box()
end)

-- Stats change on server channels, not on every display frame. Copy only the
-- scalar drawing data; retain no mutable public snapshot arrays.
local hud_text, hud_panel = eqoa.hud.text, eqoa.hud.panel
local panel_spec = { x = 200, y = 54, w = 148, h = 132 }
local rows
local dirty = true
local function invalidate() dirty = true end
for _, event in ipairs({ "player.stats_changed", "state.reset", "enabled", "events.dropped" }) do
    eqoa.events.on(event, invalidate)
end

-- The game's own Stat Window (F4, /stats) shows the same figures: while this mod's box is on, it is held back. It
-- comes back the moment the box is turned off, the mod is disabled, fails or the world is left (a claim ends then).
local claim
local function hold_stat_window(on)
    if on and not claim then
        claim = eqoa.hud.hide("stat_window")
    elseif not on and claim then
        eqoa.hud.show(claim)
        claim = nil
    end
end
eqoa.events.on("state.reset", function() hold_stat_window(false) end)

-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end

eqoa.events.on("render", function()
    hold_stat_window(eqoa.options.get("stats_box") == true)
    if map_up() then return end
    if not eqoa.options.get("stats_box") then return end
    if not eqoa.hud.visible("stats_box") then return end
    if dirty then
        local s = eqoa.player.stats()
        if not s or not s.rows or #s.rows == 0 then return end
        rows = {}
        for i = 1, math.min(16, #s.rows) do
            local r = s.rows[i]
            local color = off_white
            if r.shift == "raised" then color = raised_blue
            elseif r.shift == "lowered" then color = lowered_red end
            local col, row = (i > 8) and 2 or 1, ((i - 1) % 8) + 1
            local label = eqoa.text.localized(r.word) or r.short or r.name
            -- Spec tables are built once per change and reused every frame.
            rows[i] = {
                { col = col, row = row, text = label, color = gold, shadow = true },
                { col = col, row = row, text = tostring(r.current), color = color, align = "right", shadow = true },
            }
        end
        dirty = false
    end
    if not rows then return end
    hud_panel(panel_spec)
    for i = 1, #rows do
        local r = rows[i]
        hud_text(r[1])
        hud_text(r[2])
    end

    -- Clickable area to open the Summary window, only while the HUD is in
    -- plain play (no map, menu, search or loading screen over it).
    if eqoa.imgui then
        local frame = eqoa.hud.frame()
        local sx = frame.offset_x + (200 - frame.side) * frame.scale_x
        local sy = frame.offset_y + (54 - frame.top) * frame.scale_y
        local sw = 148 * frame.scale_x
        local sh = 132 * frame.scale_y
        eqoa.imgui.SetNextWindowPos(sx, sy)
        eqoa.imgui.SetNextWindowSize(sw, sh)
        if eqoa.imgui.Begin("eqoa_stats_box_click", nil,
            ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove
                + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings) then
            eqoa.imgui.SetCursorScreenPos(sx, sy)
            if eqoa.imgui.InvisibleButton("stats_box_button", sw, sh) then
                eqoa.windows.open("summary")
            end
        end
        eqoa.imgui.End()
    end
end)
