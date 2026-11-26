-- EQOA Target of Target Mod
-- Displays the Target of Target bar above the Target panel with health and
-- mana gauges, and Assist Target on click.

local hud_text, hud_gauge, hud_image = eqoa.hud.text, eqoa.hud.gauge, eqoa.hud.image

local black = { 0.0, 0.0, 0.0, 1.0 }

eqoa.options.toggle({ id = "target_of_target", label = "Target of Target", default = false })

-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end

eqoa.events.on("render", function()
    if map_up() then return end
    if not eqoa.options.get("target_of_target") then return end
    if not eqoa.hud.visible("target") then return end

    local target = eqoa.target.get()
    if not target or not target.id or not target.target_id then return end
    if target.target_id == 0 or target.target_id == target.id then return end

    local tot = eqoa.entities.find(target.target_id)
    if not tot or not tot.name or tot.name == "" or tot.name:match("^%s*$") then return end

    -- Placed from the target panel as it is drawn now: moved (Move HUD) and grown about its corner with HUD Scale or
    -- the PC interface's piece size (174 wide at 100%). Each point is its place on the panel at 100%, scaled.
    local panel = eqoa.hud.layout().target.panel
    local k = (panel.frame_width and panel.frame_width > 0) and panel.frame_width / 174 or 1
    local sc = (k ~= 1) and k or nil
    local function fx(x) return math.floor(panel.frame_x + (x - 20) * k + 0.5) end
    local function fy(y) return math.floor(panel.frame_y + (y - 348) * k + 0.5) end

    -- 3-piece bar at [20, 300], width 174
    hud_image({ ui_id = 25, x = fx(20), y = fy(300), w = 174, scale = sc, target_fade = true, vertical = "bottom" })

    -- Centered black name text
    hud_text({ x = fx(20 + 87), y = fy(300 + 12), text = tot.name, color = black, scale = sc, clip_width = math.floor(160 * k + 0.5), target_fade = true, vertical = "bottom" })

    -- HP gauge (art 7)
    if tot.hp_fraction then
        hud_gauge({ x = fx(30), y = fy(325), art = 7, fill = tot.hp_fraction, lit = true, scale = sc, vertical = "bottom" })
    end

    -- Power gauge (art 9), gated by the Mana Bars option
    if tot.power and eqoa.options.get("mana_bars", "eqoa_mana_bars") then
        hud_gauge({ x = fx(30), y = fy(335), art = 9, fill = tot.power, lit = true, scale = sc, vertical = "bottom" })
    end

    -- Clickable name bar to Assist, only while the HUD is in plain play.
    if eqoa.imgui and eqoa.hud.visible("plain_play") then
        local frame = eqoa.hud.frame()
        local sx = frame.offset_x + (fx(20) - frame.side) * frame.scale_x
        local sy = frame.offset_y + (fy(300) + frame.top) * frame.scale_y
        local sw = 174 * k * frame.scale_x
        local sh = 26 * k * frame.scale_y
        eqoa.imgui.SetNextWindowPos(sx, sy)
        eqoa.imgui.SetNextWindowSize(sw, sh)
        if eqoa.imgui.Begin("eqoa_target_of_target_click", nil,
            ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove
                + ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings) then
            eqoa.imgui.SetCursorScreenPos(sx, sy)
            if eqoa.imgui.InvisibleButton("tot_button", sw, sh) then
                eqoa.actions.assist()
            end
        end
        eqoa.imgui.End()
    end
end)
