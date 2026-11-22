-- Target Frame Demo
-- Off by default. Replaces the game's target frame: hides the native target HUD
-- piece and draws a replacement frame in its Move HUD position.

local hud_panel = eqoa.hud.panel
local hud_text = eqoa.hud.text
local hud_gauge = eqoa.hud.gauge

local WHITE = { 1.0, 1.0, 1.0, 1.0 }
local MUTED = { 0.85, 0.85, 0.85, 1.0 }
local HOSTILE = { 1.0, 0.35, 0.35, 1.0 }
local NEUTRAL = { 1.0, 0.9, 0.35, 1.0 }
local FRIENDLY = { 0.45, 0.95, 0.45, 1.0 }

local hide_token = nil

local function ensure_hidden()
    if hide_token ~= nil then
        return
    end
    local token = eqoa.hud.hide("target")
    if token then
        hide_token = token
    end
end

local function release_hidden()
    if hide_token ~= nil then
        eqoa.hud.show(hide_token)
        hide_token = nil
    end
end

local function disposition_color(name)
    if name == "Hostile" then
        return HOSTILE
    elseif name == "Friendly" then
        return FRIENDLY
    elseif name == "Neutral" then
        return NEUTRAL
    end
    return WHITE
end

eqoa.events.on("enabled", function()
    local client = eqoa.client.get()
    if client and client.in_world then
        ensure_hidden()
    end
end)

eqoa.events.on("world.entered", function()
    hide_token = nil
    ensure_hidden()
end)

eqoa.events.on("state.reset", function(ev)
    if ev and ev.data and ev.data.in_world then
        ensure_hidden()
    else
        hide_token = nil
    end
end)

eqoa.events.on("world.leaving", function()
    hide_token = nil
end)

eqoa.events.on("disabling", function()
    release_hidden()
end)

eqoa.events.on("render", function()
    local client = eqoa.client.get()
    if not client or not client.in_world then
        return
    end
    ensure_hidden()
    if not eqoa.hud.visible("hud") then
        return
    end
    local target = eqoa.target.get()
    if not target then
        return
    end

    local layout = eqoa.hud.layout()
    local panel = layout and layout.target and layout.target.panel
    local x = panel and panel.frame_x or 20
    local y = panel and panel.frame_y or 348
    local w = 186
    local h = target.power and 62 or 50

    hud_panel({ x = x, y = y, w = w, h = h, anchor = "left", vertical = "bottom" })

    local title = target.name and target.name ~= "" and target.name or "Target"
    if target.level then
        title = title .. " (" .. tostring(target.level) .. ")"
    end
    hud_text({
        x = x + 10,
        y = y + 8,
        text = title,
        color = disposition_color(target.disposition_name),
        scale = 0.85,
        shadow = true,
        clip_width = w - 20,
        anchor = "left",
        vertical = "bottom",
    })

    local hp = target.hp_fraction or 0
    hud_gauge({
        x = x + 10,
        y = y + 28,
        art = 7,
        fill = hp,
        lit = true,
        anchor = "left",
        vertical = "bottom",
    })

    if target.hp_fraction ~= nil then
        local pct = math.floor(target.hp_fraction * 100 + 0.5)
        hud_text({
            x = x + w - 10,
            y = y + 26,
            text = tostring(pct) .. "%",
            color = MUTED,
            scale = 0.75,
            align = "right",
            shadow = true,
            anchor = "left",
            vertical = "bottom",
        })
    end

    if target.power ~= nil then
        hud_gauge({
            x = x + 10,
            y = y + 42,
            art = 9,
            fill = target.power,
            lit = true,
            anchor = "left",
            vertical = "bottom",
        })
    end
end)
