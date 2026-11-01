--[[
    arpg_ui.lua

    An ARPG style heads up display built on the itsmars art kit. A framed health
    panel holds a liquid health orb and the ten ability slots, a matching power
    panel holds the power orb, a square item pad holds the four item slots, a
    segmented experience bar sits below, a buff row above, and a target
    nameplate doubles as the target's health bar.

    Live values come from the game. Health, power, and experience read the player
    record, the tray slots read the game's own hotbars, the buff row reads the
    active effects table, and the target frame reads the current target entity.
    Ability, item, and buff art is decoded straight out of emulated memory, the
    frame and orb art is loaded from this package's resources folder.

    The mod hides the game's own bars, ability bar, effects, and target nameplate
    by default since it replaces them. Each element is its own window, unlock the
    HUD from the settings panel to drag them, then lock it again.
]]

local Player     = require("frontiers_forge.player")
local EntityList = require("frontiers_forge.entity_list")
local Entity     = require("frontiers_forge.entity")
local AbilityBar = require("frontiers_forge.ability_bar")
local AbilityList = require("frontiers_forge.ability_list")
local Effects    = require("frontiers_forge.effects")
local Icon       = require("frontiers_forge.icon")
local UI         = require("frontiers_forge.ui")
local Util       = require("frontiers_forge.util")
local Input      = require("frontiers_forge.input")

local OWNER = "arpg_ui"

--------------------------------------------------------------------------------
-- Placement constants (fractions of the panel art they sit in). Tweak these to
-- nudge the orb or the ability grid inside their frame.
--------------------------------------------------------------------------------

-- The ability grid rectangle inside the health panel's right frame.
local ABILITY_RECT = { x0 = 0.425, y0 = 0.10, x1 = 0.975, y1 = 0.88 }

-- The orbs are their own draggable windows, independent of the panel art they
-- sit over. Both orbs render from this one base diameter times their own scale,
-- so scale 1.0 gives both the same size and equal scales stay equal.
local BASE_ORB_DIAM = 200

-- Native aspect ratios of the two backing panels.
local HEALTH_PANEL_ASPECT = 129 / 530
local POWER_PANEL_ASPECT  = 108 / 230

--------------------------------------------------------------------------------
-- Fonts (from the shared resources folder)
--------------------------------------------------------------------------------

local FONTS = {
    { name = "Default" },
    { name = "Cinzel", path = "fonts/Cinzel/static/Cinzel-Regular.ttf" },
    { name = "Cinzel Bold", path = "fonts/Cinzel/static/Cinzel-Bold.ttf" },
    { name = "Libre Baskerville", path = "fonts/Libre_Baskerville/static/LibreBaskerville-Regular.ttf" },
    { name = "Merriweather Sans", path = "fonts/Merriweather_Sans/static/MerriweatherSans-Regular.ttf" },
}

--------------------------------------------------------------------------------
-- State (globals survive the per frame rerun and hot reloads)
--------------------------------------------------------------------------------

local RESOURCE_FILES = {
    orb_back        = "itsmars_orb_back2.png",
    orb_fill        = "itsmars_orb_fill.png",
    orb_highlight   = "itsmars_orb_highlight.png",
    orb_shadow      = "itsmars_orb_shadow.png",
    orb_ring        = "DarkOrbBorder.png",
    health_panel    = "HealthPanel.png",
    power_panel     = "itsmars_orb_back1.png",
    exp_back        = "itsmars_exp_back.png",
    exp_fill        = "itsmars_exp_fill.png",
    exp_highlight   = "itsmars_exp_highlight.png",
    exp_shadow      = "itsmars_exp_shadow.png",
    slot_blank      = "BlankSlot.png",
    slot_spell      = "52x52 SpellSlotBorder.png",
    slot_bag        = "64x64 BagSlotBorder.png",
    name_panel      = "NamePanel.png",
    divider         = "Divider.png",
    scroll_fill     = "itsmars_scroll_fill.png",
    scroll_lip      = "itsmars_scroll_lip.png",
}

textures = textures or {}
textures_tried = textures_tried or false

config = config or {
    locked = false,

    -- Snap each element to a grid while dragging it (only when unlocked).
    grid_enabled = false,
    grid_size = 10,

    font = "Default",
    font_size = 16,

    -- Per-component font sizes. Only apply when a custom font is chosen, since
    -- the built in font cannot be resized.
    font_sizes = {
        orbs = 18,
        abilities = 14,
        exp = 12,
        target = 15,
        pet = 14,
        selected = 15,
    },

    show_health = true,
    show_power = true,
    show_items = true,
    show_exp = true,
    show_buffs = true,
    show_target = true,

    health_panel_width = 470,
    power_panel_width = 210,
    item_slot_size = 54,
    exp_width = 560,
    exp_height = 14,
    buff_size = 34,
    target_width = 320,

    -- Orb scale multiplies BASE_ORB_DIAM. Position is the orb window itself,
    -- dragged like any other element, independent of the panel behind it.
    health_orb_scale = 1.0,
    power_orb_scale = 1.0,
    show_selected_name = true,
    show_pet = true,

    -- Scrolling liquid noise inside the orbs.
    orb_scroll = true,
    orb_scroll_speed = 30,
    orb_scroll_strength = 0.25,

    pet_width = 260,

    -- Ability slot indicators (mirrors wereoxxs).
    dim_out_of_range = true,
    dim_low_power = true,
    show_cooldown_numbers = true,
    gcd_enabled = true,
    gcd_duration_ms = 500,
    selected_thickness = 2,
    selected_color = { 1.0, 1.0, 1.0, 1.0 },

    -- Rotates the dpad so the horizontal tray moves with left and right.
    rotate_dpad = false,

    health_color = { 0.82, 0.12, 0.12 },
    power_color  = { 0.18, 0.42, 0.95 },
    exp_color    = { 0.85, 0.70, 0.20 },
    target_hostile_color  = { 0.85, 0.20, 0.20 },
    target_neutral_color  = { 0.90, 0.80, 0.25 },
    target_friendly_color = { 0.35, 0.80, 0.35 },

    -- The game's own panels this mod replaces, hidden by default.
    hide = {
        health_bar = true,
        power_bar = true,
        exp_bars = true,
        ability_bar = true,
        active_effects = true,
        target_nameplate = true,
        pet_panel = true,
    },

    pos = {
        health = { 24, 560 },
        health_orb = { 8, 500 },
        power  = { 1080, 560 },
        power_orb = { 1070, 500 },
        items  = { 900, 560 },
        exp    = { 360, 700 },
        buffs  = { 360, 24 },
        target = { 700, 40 },
        pet    = { 700, 120 },
    },
}

relayout_pending = relayout_pending == nil and true or relayout_pending
game_ui_hidden = game_ui_hidden or {}
initialized = initialized or false

--------------------------------------------------------------------------------
-- Textures and fonts
--------------------------------------------------------------------------------

local function LoadTextures()
    if textures_tried then return end
    textures_tried = true
    for key, file in pairs(RESOURCE_FILES) do
        local tex = UiForge.LoadTexture(file)
        textures[key] = tex ~= nil and { tex = tex } or false
    end
end

local function ReleaseTextures()
    for key, entry in pairs(textures) do
        if entry and entry.tex then
            UiForge.ReleaseTexture(entry.tex)
        end
        textures[key] = nil
    end
    textures_tried = false
end

-- The chosen font handle at the given size, or nil for ImGui's built in font
-- (which cannot be resized). The host caches fonts per file and size, so asking
-- every frame is cheap.
local function GetModFont(size)
    if config.font == "Default" then return nil end
    size = size or config.font_size
    for _, entry in ipairs(FONTS) do
        if entry.name == config.font and entry.path ~= nil then
            return UiForge.LoadFont(entry.path, size)
        end
    end
    return nil
end

--------------------------------------------------------------------------------
-- Draw helpers
--------------------------------------------------------------------------------

local WHITE = { 1, 1, 1, 1 }

local function U32(c, a)
    return ImGui.GetColorU32(c[1], c[2], c[3], a or c[4] or 1)
end

local function DrawTex(dl, key, x, y, w, h, col_u32)
    local entry = textures[key]
    if not entry then return false end
    dl:AddImage(entry.tex, ImVec2.new(x, y), ImVec2.new(x + w, y + h),
        ImVec2.new(0, 0), ImVec2.new(1, 1), col_u32 or U32(WHITE))
    return true
end

local function DrawCenteredText(dl, cx, top, text, col_u32)
    local tw = ImGui.CalcTextSize(text)
    local x = cx - tw * 0.5
    dl:AddText(ImVec2.new(x + 1, top + 1), U32({ 0, 0, 0 }, 0.85), text)
    dl:AddText(ImVec2.new(x, top), col_u32, text)
end

-- A horizontal bar filled left to right to frac. The fill PNG is dark, so a
-- solid color base guarantees a visible color and the exp art is layered on top
-- for texture and gloss. The shadow frame is drawn last, over the whole bar, so
-- it matches the bar's width and height and rims the fill.
local function DrawTexturedFill(dl, x, y, w, h, frac, color, with_shadow)
    DrawTex(dl, "exp_back", x, y, w, h)
    frac = math.max(0, math.min(1, frac))
    if frac > 0 then
        dl:PushClipRect(ImVec2.new(x, y), ImVec2.new(x + w * frac, y + h), true)
        dl:AddRectFilled(ImVec2.new(x, y), ImVec2.new(x + w, y + h), U32(color, 1))
        DrawTex(dl, "exp_fill", x, y, w, h, U32(WHITE, 0.5))
        DrawTex(dl, "exp_highlight", x, y, w, h, U32(WHITE, 0.6))
        dl:PopClipRect()
    end
    if with_shadow ~= false then
        DrawTex(dl, "exp_shadow", x, y, w, h, U32(WHITE, 0.6))
    end
end

local HUD_FLAGS = nil
local WINDOW_PAD = 8

-- Opens one HUD element window and pushes the mod font. Returns the draw list
-- and screen origin, or nil when the window is not visible. Paired with EndElement.
local function BeginElement(id, pos_key, width, height, font_size)
    if HUD_FLAGS == nil then
        HUD_FLAGS = ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoResize
            + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoScrollWithMouse
            + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoSavedSettings
            + ImGuiWindowFlags.NoFocusOnAppearing + ImGuiWindowFlags.NoCollapse
    end
    local flags = HUD_FLAGS
    if config.locked then
        flags = flags + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoInputs
    end

    -- A profile saved before this element existed has no position for it, so
    -- fall back to a default rather than indexing nil.
    local p = config.pos[pos_key]
    if p == nil then
        p = { 100, 100 }
        config.pos[pos_key] = p
    end
    local cond = relayout_pending and ImGuiCond.Always or ImGuiCond.FirstUseEver
    ImGui.SetNextWindowPos(p[1], p[2], cond)
    ImGui.SetNextWindowSize(width + WINDOW_PAD * 2, height + WINDOW_PAD * 2, ImGuiCond.Always)

    local visible = ImGui.Begin("##arpg_" .. id, true, flags)
    if not visible then
        ImGui.End()
        return nil
    end
    if not config.locked and not relayout_pending then
        local wx, wy = ImGui.GetWindowPos()
        -- Snap the dragged position to the grid so pieces line up.
        if config.grid_enabled then
            local step = math.max(1, config.grid_size)
            local sx = math.floor(wx / step + 0.5) * step
            local sy = math.floor(wy / step + 0.5) * step
            if sx ~= wx or sy ~= wy then
                ImGui.SetWindowPos(sx, sy)
                wx, wy = sx, sy
            end
        end
        p[1], p[2] = wx, wy
    end
    local font = GetModFont(font_size)
    if font ~= nil then ImGui.PushFont(font) end
    element_font_pushed = font ~= nil
    local ox, oy = ImGui.GetCursorScreenPos()
    return ImGui.GetWindowDrawList(), ox, oy
end

local function EndElement()
    if element_font_pushed then ImGui.PopFont() end
    element_font_pushed = false
    ImGui.End()
end

--------------------------------------------------------------------------------
-- Orb
--------------------------------------------------------------------------------

-- Draws a liquid orb centered on (cx, cy) with the given diameter. fraction
-- Draws a scrolling noise texture masked to a circle of radius nr centered on
-- (cx, cy), clipped to the liquid surface at fill_top. Motion is a continuous
-- one-direction vertical scroll (offset o, 0..1). The circle is a real
-- anti-aliased AddImageRounded (rounding = radius), so the edge is smooth with
-- no stair steps. The wrap is handled by splitting the draw at the single seam
-- line rather than tiling, so every visible UV stays in [0,1] (the backend
-- samples clamped) and both halves share the same circle geometry.
local function DrawScrollLayer(dl, tex, cx, cy, nr, fill_top, o, col)
    local dtop, dbot = cy - nr, cy + nr
    local left, right = cx - nr, cx + nr
    local size = nr * 2
    local pmin, pmax = ImVec2.new(left, dtop), ImVec2.new(right, dbot)
    local seam = dbot - o * size

    -- Above the seam the noise samples uv.y in [o, 1].
    local top1 = math.max(fill_top, dtop)
    if math.min(seam, dbot) > top1 then
        dl:PushClipRect(ImVec2.new(left, top1), ImVec2.new(right, math.min(seam, dbot)), true)
        dl:AddImageRounded(tex, pmin, pmax, ImVec2.new(0, o), ImVec2.new(1, 1 + o), col, nr)
        dl:PopClipRect()
    end
    -- Below the seam it wraps to uv.y in [0, o].
    local top2 = math.max(fill_top, seam)
    if dbot > top2 then
        dl:PushClipRect(ImVec2.new(left, top2), ImVec2.new(right, dbot), true)
        dl:AddImageRounded(tex, pmin, pmax, ImVec2.new(0, o - 1), ImVec2.new(1, o), col, nr)
        dl:PopClipRect()
    end
end

-- 0..1 fills from the bottom. The sphere art is drawn at the full ring diameter
-- so it fills the dark border rather than leaving a gap inside it.
local function DrawOrb(dl, cx, cy, diameter, fraction, color)
    local r = diameter * 0.5
    local x0, y0 = cx - r, cy - r

    fraction = math.max(0, math.min(1, fraction))

    DrawTex(dl, "orb_back", x0, y0, diameter, diameter)

    if fraction > 0 then
        local fill_top = y0 + diameter * (1 - fraction)
        dl:PushClipRect(ImVec2.new(x0, fill_top), ImVec2.new(x0 + diameter, y0 + diameter), true)
        DrawTex(dl, "orb_fill", x0, y0, diameter, diameter, U32(color, 1))

        -- Scrolling noise gives the liquid a continuous one-direction drift.
        -- The mask radius sits just inside the border's inner opening (~0.874x)
        -- so the noise never pokes past the dark ring. The fill and lip layers
        -- scroll at different speeds for a bit of parallax.
        local scroll = config.orb_scroll and textures.scroll_fill
        if scroll then
            local speed = config.orb_scroll_speed
            local a = config.orb_scroll_strength
            local tint = { math.min(1, color[1] + 0.25), math.min(1, color[2] + 0.25), math.min(1, color[3] + 0.25) }
            local nr = r * 0.86
            local o1 = (ImGui.GetTime() * speed / diameter) % 1
            DrawScrollLayer(dl, scroll.tex, cx, cy, nr, fill_top, o1, U32(tint, a))
            if textures.scroll_lip then
                local o2 = (ImGui.GetTime() * speed * 0.6 / diameter) % 1
                DrawScrollLayer(dl, textures.scroll_lip.tex, cx, cy, nr, fill_top, o2, U32(tint, a * 0.7))
            end
        end

        -- A meniscus line matching the circle's chord at the liquid surface.
        if fraction < 1 then
            local dy = fill_top - cy
            local half = math.sqrt(math.max(0, r * r - dy * dy))
            local bright = { math.min(1, color[1] + 0.3), math.min(1, color[2] + 0.3), math.min(1, color[3] + 0.3) }
            dl:AddLine(ImVec2.new(cx - half, fill_top), ImVec2.new(cx + half, fill_top), U32(bright, 0.6), 2)
        end
        dl:PopClipRect()
    end

    DrawTex(dl, "orb_shadow", x0, y0, diameter, diameter, U32(WHITE, 0.35))
    DrawTex(dl, "orb_highlight", x0, y0, diameter, diameter, U32(WHITE, 0.30))
    DrawTex(dl, "orb_ring", x0, y0, diameter, diameter)
end

--------------------------------------------------------------------------------
-- Slots (icons, selection, cooldown / range / power indicators like wereoxxs)
--------------------------------------------------------------------------------

-- Live cooldown countdowns keyed by ability display index, driven by the
-- AbilityList lockout events. The game stores a whole lockout and never counts
-- it down, so the countdown runs from the start event.
cooldown_running = cooldown_running or {}

-- The server holds every cast for roughly half a second past the cast bar, a
-- global gate the client never stores, so slots sweep it from the cast event.
gcd_state = gcd_state or { until_ms = 0, total_ms = 0 }

local function NowMs()
    return math.floor(ImGui.GetTime() * 1000)
end

local function SubscribeAbilities()
    AbilityList.On(OWNER, AbilityList.Events.OnLockoutStart, function(event)
        local now_ms = NowMs()
        cooldown_running[event.display_index] = { lockout = event.lockout_ms, started_ms = now_ms }
        if config.gcd_enabled then
            local ability = AbilityList.GetAbilityByDisplayIndex(event.display_index)
            local cast_time = ability ~= nil and (ability:GetCastTime() or 0) or 0
            local total = cast_time * 1000 + config.gcd_duration_ms
            gcd_state.until_ms = now_ms + total
            gcd_state.total_ms = total
        end
    end)
    AbilityList.On(OWNER, AbilityList.Events.OnLockoutEnd, function(event)
        cooldown_running[event.display_index] = nil
    end)
end

-- The remaining part of the global cooldown, 0 when the gate is not running.
local function GcdFraction(now_ms)
    if not config.gcd_enabled or gcd_state.total_ms <= 0 or now_ms >= gcd_state.until_ms then
        return 0
    end
    return (gcd_state.until_ms - now_ms) / gcd_state.total_ms
end

local function CooldownRemaining(ability, now_ms)
    local di = ability:GetDisplayIndex()
    if di ~= nil then
        local running = cooldown_running[di]
        if running ~= nil then
            return math.max(0, running.lockout - (now_ms - running.started_ms))
        end
    end
    if ability:IsOnCooldown() then
        return ability:GetCooldownLockoutMs() or 0
    end
    return 0
end

-- Player and target points for the range dim, resolved once per frame. nil when
-- nothing is targeted, which leaves every slot in range.
local function TargetRangePoints()
    local tid = Player.GetTargetEntityId()
    if tid == nil or tid == 0 or tid == 0xFFFFFFFF then return nil end
    local entity = EntityList.GetEntityById(tid)
    if entity == nil then return nil end
    local tx, ty, tz = entity:GetPosition()
    return { from = Player.GetCoordinates(), to = { x = tx, y = ty, z = tz } }
end

-- Builds an ability slot row (icons, cooldown, range, power, selection).
local function BuildAbilitySlot(bar, index, selected, now_ms, range_points, current_pwr)
    local slot = { selected = selected }
    if index >= AbilityBar.GetSlotCount(bar) then return slot end
    local ability = AbilityBar.GetAbility(bar, index)
    if ability == nil then return slot end

    slot.name = ability:GetName()
    slot.fg, slot.fg_w, slot.fg_h = Icon.GetTexture(ability:GetIconForegroundRef(),
        { trim_transparent = true, trim_color = true })
    slot.bg, slot.bg_w, slot.bg_h = Icon.GetTexture(ability:GetIconBackgroundRef(),
        { trim_transparent = true, trim_color = true })
    slot.remaining_ms = CooldownRemaining(ability, now_ms)
    if slot.remaining_ms <= 0 then
        slot.gcd_fraction = GcdFraction(now_ms)
    end
    if range_points ~= nil and config.dim_out_of_range then
        slot.out_of_range = not ability:IsInRange(range_points.from, range_points.to)
    end
    local cost = ability:GetPwrCost() or 0
    if current_pwr ~= nil and config.dim_low_power then
        slot.low_power = cost > current_pwr
    end
    slot.tooltip = string.format("%s\n%s\nPower  %d", slot.name or "?",
        ability:GetDescription() or "", cost)
    return slot
end

-- Builds an item slot row. The game's compact hotbar draws a fixed glyph per
-- slot position, so that is the default, stretched to fill the square.
local function BuildItemSlot(bar, index, selected)
    local slot = { selected = selected, stretch = true }
    if index >= AbilityBar.GetSlotCount(bar) then return slot end
    local item = AbilityBar.GetItem(bar, index)
    if item == nil then return slot end
    slot.name = item:GetName()
    local glyph = AbilityBar.GetSlotUITexId(index)
    if glyph ~= nil then
        slot.fg, slot.fg_w, slot.fg_h = Icon.GetUITexture(glyph)
    end
    if slot.fg == nil then
        slot.fg, slot.fg_w, slot.fg_h = Icon.GetTexture(item:GetIconRef(), { trim_transparent = true })
    end
    slot.tooltip = string.format("%s\n%s", slot.name or "?", item:GetDescription() or "")
    return slot
end

-- Where a clockwise-from-twelve angle meets the edge of a size square centered
-- on (cx, cy). Scaling by the dominant axis keeps every point on the square.
local function GcdEdgePoint(cx, cy, half, angle)
    local dx, dy = math.sin(angle), -math.cos(angle)
    local tx = dx ~= 0 and half / math.abs(dx) or math.huge
    local ty = dy ~= 0 and half / math.abs(dy) or math.huge
    local t = math.min(tx, ty)
    return ImVec2.new(cx + dx * t, cy + dy * t)
end

-- A pie slice dim covering the tail of a clockwise sweep, so a slot held by the
-- global cooldown undims in a clockwise motion. fraction is the remaining part.
local function DrawGcdSweep(dl, x, y, size, fraction, packed)
    local half = size / 2
    local cx, cy = x + half, y + half
    local center = ImVec2.new(cx, cy)
    local full = 2 * math.pi
    local start = (1 - math.min(fraction, 1)) * full

    -- A fan of 15 degree segments. The step divides the corner angles evenly,
    -- so every triangle edge lies exactly on the slot's boundary.
    local step = full / 24
    local prev = GcdEdgePoint(cx, cy, half, start)
    local angle = (math.floor(start / step) + 1) * step
    while angle < full - 1e-6 do
        local point = GcdEdgePoint(cx, cy, half, angle)
        dl:AddTriangleFilled(center, prev, point, packed)
        prev = point
        angle = angle + step
    end
    dl:AddTriangleFilled(center, prev, GcdEdgePoint(cx, cy, half, full), packed)
end

-- Draws a prepared slot: blank fill, plate, art, cooldown / power / range dim,
-- frame, and the selected highlight. A single dim shows at a time.
local function DrawRichSlot(dl, x, y, size, frame_key, slot)
    local uv0, uv1 = ImVec2.new(0, 0), ImVec2.new(1, 1)
    local white = U32(WHITE)
    DrawTex(dl, "slot_blank", x, y, size, size)

    if slot.bg ~= nil then
        dl:AddImage(slot.bg, ImVec2.new(x, y), ImVec2.new(x + size, y + size), uv0, uv1, white)
    end
    if slot.fg ~= nil then
        local w, h = size, size
        if not slot.stretch and slot.fg_w and slot.fg_h and slot.fg_w > 0 and slot.fg_h > 0 then
            if slot.bg ~= nil and slot.bg_w and slot.bg_h and slot.bg_w > 0 and slot.bg_h > 0 then
                w, h = size * (slot.fg_w / slot.bg_w), size * (slot.fg_h / slot.bg_h)
            else
                local sc = size / math.max(slot.fg_w, slot.fg_h)
                w, h = slot.fg_w * sc, slot.fg_h * sc
            end
        else
            -- Stretched art still leaves a hair of the frame showing.
            local inset = size * 0.08
            w, h = size - inset * 2, size - inset * 2
        end
        local ix, iy = x + (size - w) * 0.5, y + (size - h) * 0.5
        dl:AddImage(slot.fg, ImVec2.new(ix, iy), ImVec2.new(ix + w, iy + h), uv0, uv1, white)
    end

    local remaining = slot.remaining_ms or 0
    if remaining > 0 then
        dl:AddRectFilled(ImVec2.new(x, y), ImVec2.new(x + size, y + size), U32({ 0, 0, 0 }, 0.62))
        if config.show_cooldown_numbers then
            local label = tostring(math.ceil(remaining / 1000))
            local tw, th = ImGui.CalcTextSize(label)
            dl:AddText(ImVec2.new(x + (size - tw) * 0.5 + 1, y + (size - th) * 0.5 + 1), U32({ 0, 0, 0 }, 0.9), label)
            dl:AddText(ImVec2.new(x + (size - tw) * 0.5, y + (size - th) * 0.5), U32(WHITE), label)
        end
    elseif (slot.gcd_fraction or 0) > 0 then
        DrawGcdSweep(dl, x, y, size, slot.gcd_fraction, U32({ 0, 0, 0 }, 0.62))
    elseif slot.low_power then
        dl:AddRectFilled(ImVec2.new(x, y), ImVec2.new(x + size, y + size), U32({ 0.30, 0.05, 0.08 }, 0.55))
    elseif slot.out_of_range then
        dl:AddRectFilled(ImVec2.new(x, y), ImVec2.new(x + size, y + size), U32({ 0, 0, 0 }, 0.55))
    end

    DrawTex(dl, frame_key, x, y, size, size)

    if slot.selected then
        dl:AddRect(ImVec2.new(x - 1, y - 1), ImVec2.new(x + size + 1, y + size + 1),
            U32(config.selected_color), 0, 0, config.selected_thickness)
    end
end

--------------------------------------------------------------------------------
-- Orbs (own windows, independent of the panel art behind them)
--------------------------------------------------------------------------------

selected_name_box = selected_name_box or nil

local function DrawFloatingOrb(pos_key, scale, color, cur, max)
    local diam = BASE_ORB_DIAM * scale
    local dl, ox, oy = BeginElement(pos_key, pos_key, diam, diam, config.font_sizes.orbs)
    if dl == nil then return end
    local cx, cy = ox + diam * 0.5, oy + diam * 0.5
    local frac = (max and max > 0) and (cur / max) or 0
    DrawOrb(dl, cx, cy, diam, frac, color)
    local _, th = ImGui.CalcTextSize("0")
    DrawCenteredText(dl, cx, cy - th * 0.5, string.format("%d / %d", cur or 0, max or 0), U32(WHITE))
    EndElement()
end

local function DrawHealthOrb()
    DrawFloatingOrb("health_orb", config.health_orb_scale, config.health_color,
        Player.GetCurrentHp(), Player.GetMaxHp())
end

local function DrawPowerOrb()
    DrawFloatingOrb("power_orb", config.power_orb_scale, config.power_color,
        Player.GetCurrentPwr(), Player.GetMaxPwr())
end

--------------------------------------------------------------------------------
-- Health panel (frame + ten ability slots)
--------------------------------------------------------------------------------

local function DrawHealthPanel()
    local pw = config.health_panel_width
    local ph = pw * HEALTH_PANEL_ASPECT

    local dl, ox, oy = BeginElement("health", "health", pw, ph, config.font_sizes.abilities)
    if dl == nil then return end

    DrawTex(dl, "health_panel", ox, oy, pw, ph)

    -- Ten ability slots, two rows of five, centered in the right frame rectangle.
    local rx0 = ox + pw * ABILITY_RECT.x0
    local ry0 = oy + ph * ABILITY_RECT.y0
    local rw = pw * (ABILITY_RECT.x1 - ABILITY_RECT.x0)
    local rh = ph * (ABILITY_RECT.y1 - ABILITY_RECT.y0)
    local gap = 3
    local slot = math.min((rw - gap * 4) / 5, (rh - gap) / 2)
    local grid_w = slot * 5 + gap * 4
    local grid_h = slot * 2 + gap
    local gx = rx0 + (rw - grid_w) * 0.5
    local gy = ry0 + (rh - grid_h) * 0.5

    local now_ms = NowMs()
    local range_points = TargetRangePoints()
    local current_pwr = Player.GetCurrentPwr()
    local sel_bar = AbilityBar.GetSelectedBarIndex()
    local sel_slot = sel_bar ~= nil and AbilityBar.GetSelectedSlotIndex(sel_bar) or nil

    for i = 0, 9 do
        local row = i < 5 and 0 or 1
        local col = i % 5
        local x = gx + col * (slot + gap)
        local y = gy + row * (slot + gap)
        local selected = (row == sel_bar) and (col == sel_slot)
        local data = BuildAbilitySlot(row, col, selected, now_ms, range_points, current_pwr)
        DrawRichSlot(dl, x, y, slot, "slot_spell", data)
        if selected and data.name then
            -- Both rows show their name at the top of the panel, above the top row.
            -- min_x is the far left of the first slot so the plate never slides
            -- left under the health orb.
            selected_name_box = { cx = x + slot * 0.5, bottom = gy - 4, name = data.name, min_x = gx }
        end
        if data.tooltip and ImGui.IsMouseHoveringRect(x, y, x + slot, y + slot) then
            ImGui.SetTooltip(data.tooltip)
        end
    end
    EndElement()
end

--------------------------------------------------------------------------------
-- Power panel (backing art only, the orb floats over it)
--------------------------------------------------------------------------------

local function DrawPowerPanel()
    local pw = config.power_panel_width
    local ph = pw * POWER_PANEL_ASPECT

    local dl, ox, oy = BeginElement("power", "power", pw, ph)
    if dl == nil then return end

    DrawTex(dl, "power_panel", ox, oy, pw, ph)
    EndElement()
end

--------------------------------------------------------------------------------
-- Item pad (four slots in a touching square)
--------------------------------------------------------------------------------

local function DrawItemPad()
    local s = config.item_slot_size
    local w = s * 2
    local dl, ox, oy = BeginElement("items", "items", w, w)
    if dl == nil then return end

    local sel_bar = AbilityBar.GetSelectedBarIndex()
    local sel_slot = sel_bar == 2 and AbilityBar.GetSelectedSlotIndex(2) or nil

    for i = 0, 3 do
        local col = i % 2
        local row = math.floor(i / 2)
        local x = ox + col * s
        local y = oy + row * s
        local selected = (i == sel_slot)
        local data = BuildItemSlot(2, i, selected)
        DrawRichSlot(dl, x, y, s, "slot_bag", data)
        if selected and data.name then
            selected_name_box = { cx = x + s * 0.5, bottom = y - 4, name = data.name, min_x = ox }
        end
        if data.tooltip and ImGui.IsMouseHoveringRect(x, y, x + s, y + s) then
            ImGui.SetTooltip(data.tooltip)
        end
    end
    EndElement()
end

--------------------------------------------------------------------------------
-- Experience bar
--------------------------------------------------------------------------------

local function DrawExpBar()
    local w, h = config.exp_width, config.exp_height

    local dl, ox, oy = BeginElement("exp", "exp", w, h, config.font_sizes.exp)
    if dl == nil then return end

    local level = Player.GetLevel() or 0
    local exp = Player.GetExp() or 0
    local required = Util.GetExpRequiredForLevel(level)

    local frac, label
    if required ~= nil and required > 0 then
        frac = math.max(0, math.min(1, exp / required))
        label = string.format("Level %d      %d / %d  (%.1f%%)", level, exp, required, frac * 100)
    else
        -- No threshold for this level (outside the table), show the raw count.
        frac = 0
        label = string.format("Level %d      %d exp", level, exp)
    end

    DrawTexturedFill(dl, ox, oy, w, h, frac, config.exp_color)
    -- Text sits on the bar itself, vertically centered.
    local _, th = ImGui.CalcTextSize(label)
    DrawCenteredText(dl, ox + w * 0.5, oy + (h - th) * 0.5, label, U32(WHITE))
    EndElement()
end

--------------------------------------------------------------------------------
-- Buffs and debuffs
--------------------------------------------------------------------------------

local function DrawBuffs()
    local size = config.buff_size
    local gap = 4
    local slots = math.max(Effects.GetCount(), 1)
    local w = slots * size + (slots - 1) * gap

    local dl, ox, oy = BeginElement("buffs", "buffs", w, size)
    if dl == nil then return end

    for index, effect in Effects.All() do
        local x = ox + index * (size + gap)
        DrawTex(dl, "slot_blank", x, oy, size, size)
        local tex = Icon.GetTexture(effect.icon_ref, { trim_transparent = true })
        if tex ~= nil then
            local inset = size * 0.08
            dl:AddImage(tex, ImVec2.new(x + inset, oy + inset),
                ImVec2.new(x + size - inset, oy + size - inset),
                ImVec2.new(0, 0), ImVec2.new(1, 1), U32(WHITE))
        end
        DrawTex(dl, "slot_spell", x, oy, size, size)
        if effect.name and ImGui.IsMouseHoveringRect(x, oy, x + size, oy + size) then
            ImGui.SetTooltip(effect.name)
        end
    end
    EndElement()
end

--------------------------------------------------------------------------------
-- Target nameplate (name + health bar, level and disposition below a divider)
--------------------------------------------------------------------------------

local function DispositionColor(disp_name)
    if disp_name == "Hostile" then return config.target_hostile_color end
    if disp_name == "Neutral" then return config.target_neutral_color end
    if disp_name == "Friendly" then return config.target_friendly_color end
    return { 0.70, 0.70, 0.70 }
end

local function GetTargetEntity()
    local target_id = Player.GetTargetEntityId()
    if target_id == nil or target_id == 0 or target_id == 0xFFFFFFFF then
        return nil
    end
    return EntityList.GetEntityById(target_id)
end

local function DrawTargetFrame()
    local target = GetTargetEntity()
    if target == nil then return end

    local w = config.target_width
    local name_h = w * (25 / 400)   -- NamePanel native aspect
    local div_h = 3
    local info_h = config.font_sizes.target + 6
    local total_h = name_h + div_h + info_h + 4

    local dl, ox, oy = BeginElement("target", "target", w, total_h, config.font_sizes.target)
    if dl == nil then return end

    local disp_name = target.disposition_name or "Unknown"
    local disp_color = DispositionColor(disp_name)
    local hp = target.percent_hp or 0

    -- NamePanel is fully opaque, so it is the frame and the health fill goes on
    -- top of it (inset within its border), then the name on top of the fill.
    DrawTex(dl, "name_panel", ox, oy, w, name_h)
    local inset = math.max(2, name_h * 0.16)
    local fx, fy = ox + inset, oy + inset
    local fw, fh = w - inset * 2, name_h - inset * 2
    DrawTexturedFill(dl, fx, fy, fw, fh, hp, disp_color, false)

    local _, th = ImGui.CalcTextSize(target.name or "?")
    DrawCenteredText(dl, ox + w * 0.5, oy + (name_h - th) * 0.5, target.name or "?", U32(WHITE))

    -- Divider, then level and disposition beneath it.
    DrawTex(dl, "divider", ox, oy + name_h, w, div_h)
    DrawCenteredText(dl, ox + w * 0.5, oy + name_h + div_h + 2,
        string.format("Level %d   %s", target.level or 0, disp_name), U32(disp_color, 1))
    EndElement()
end

--------------------------------------------------------------------------------
-- Pet nameplate (name + health bar, level below a divider, like the target)
--------------------------------------------------------------------------------

local function GetPetEntity()
    local pet_id = Player.GetPetEntityId()
    if pet_id == nil or pet_id == 0 or pet_id == 0xFFFFFFFF then
        return nil
    end
    return EntityList.GetEntityById(pet_id)
end

local function DrawPetFrame()
    local pet = GetPetEntity()
    if pet == nil then return end

    local w = config.pet_width
    local name_h = w * (25 / 400)   -- NamePanel native aspect
    local div_h = 3
    local info_h = config.font_sizes.pet + 6
    local total_h = name_h + div_h + info_h + 4

    local dl, ox, oy = BeginElement("pet", "pet", w, total_h, config.font_sizes.pet)
    if dl == nil then return end

    local color = config.target_friendly_color
    local hp = pet.percent_hp or 0

    DrawTex(dl, "name_panel", ox, oy, w, name_h)
    local inset = math.max(2, name_h * 0.16)
    DrawTexturedFill(dl, ox + inset, oy + inset, w - inset * 2, name_h - inset * 2, hp, color, false)

    local name = pet.name or "Pet"
    local _, th = ImGui.CalcTextSize(name)
    DrawCenteredText(dl, ox + w * 0.5, oy + (name_h - th) * 0.5, name, U32(WHITE))

    DrawTex(dl, "divider", ox, oy + name_h, w, div_h)
    DrawCenteredText(dl, ox + w * 0.5, oy + name_h + div_h + 2,
        string.format("Level %d   %d%%", pet.level or 0, math.floor(hp * 100 + 0.5)), U32(color, 1))
    EndElement()
end

--------------------------------------------------------------------------------
-- Selected ability name (a floating box above the selected slot, its own window
-- so it never clips against the panel, the same idea as wereoxxs)
--------------------------------------------------------------------------------

local function DrawNameBox(box)
    if box == nil or box.name == nil or box.name == "" then return end

    -- Measure with the selected font first, so the left-edge clamp below uses the
    -- box's real width.
    local font = GetModFont(config.font_sizes.selected)
    if font ~= nil then ImGui.PushFont(font) end
    local tw, th = ImGui.CalcTextSize(box.name)
    if font ~= nil then ImGui.PopFont() end
    local padx, pady = 8, 4
    local w, h = tw + padx * 2, th + pady * 2

    -- Center over the slot, but never let the left edge cross min_x (the far left
    -- of the first ability slot) so the plate cannot slide under the health orb.
    local left = box.cx - w * 0.5
    if box.min_x ~= nil and left < box.min_x then left = box.min_x end

    local flags = ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoResize
        + ImGuiWindowFlags.AlwaysAutoResize + ImGuiWindowFlags.NoBackground
        + ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoFocusOnAppearing
        + ImGuiWindowFlags.NoInputs + ImGuiWindowFlags.NoMove
    -- Pivot bottom-left, so left is the actual left edge after the clamp.
    ImGui.SetNextWindowPos(left, box.bottom, ImGuiCond.Always, 0.0, 1.0)
    if ImGui.Begin("##arpg_ability_name", true, flags) then
        if font ~= nil then ImGui.PushFont(font) end
        local dl = ImGui.GetWindowDrawList()
        local bx, by = ImGui.GetCursorScreenPos()
        dl:AddRectFilled(ImVec2.new(bx, by), ImVec2.new(bx + w, by + h), U32({ 0.07, 0.07, 0.10 }, 0.92))
        dl:AddRect(ImVec2.new(bx, by), ImVec2.new(bx + w, by + h), U32({ 0.45, 0.42, 0.30 }, 0.9), 0, 0, 1)
        dl:AddText(ImVec2.new(bx + padx, by + pady), U32({ 1.0, 0.95, 0.75 }), box.name)
        ImGui.Dummy(w, h)
        if font ~= nil then ImGui.PopFont() end
    end
    ImGui.End()
end

--------------------------------------------------------------------------------
-- Hiding the game's own UI
--------------------------------------------------------------------------------

local function DisablePlayerBars()
    local a = UI.DisableHealthBar(OWNER)
    local b = UI.DisablePowerBar(OWNER)
    local c = UI.DisableExperienceBars(OWNER)
    return a or b or c
end
local function EnablePlayerBars()
    UI.EnableHealthBar(OWNER)
    UI.EnablePowerBar(OWNER)
    UI.EnableExperienceBars(OWNER)
    return true
end

local GAME_PANELS = {
    { key = "health_bar",       disable = DisablePlayerBars,             enable = EnablePlayerBars },
    { key = "ability_bar",      disable = UI.DisableAbilityBar,          enable = UI.EnableAbilityBar },
    { key = "active_effects",   disable = UI.DisableActiveEffectsDisplay, enable = UI.EnableActiveEffectsDisplay },
    { key = "target_nameplate", disable = UI.DisableTargetNameplate,     enable = UI.EnableTargetNameplate },
    { key = "pet_panel",        disable = UI.DisablePetPanel,            enable = UI.EnablePetPanel },
}
-- health_bar drives all three player bars, so power_bar and exp_bars share it.
local function PanelDesired(key)
    if key == "health_bar" then
        return config.hide.health_bar or config.hide.power_bar or config.hide.exp_bars
    end
    return config.hide[key]
end

local function ApplyGameUiState()
    for _, panel in ipairs(GAME_PANELS) do
        local desired = PanelDesired(panel.key) == true
        if desired ~= (game_ui_hidden[panel.key] == true) then
            local applied = desired and panel.disable(OWNER) or panel.enable(OWNER)
            if applied then game_ui_hidden[panel.key] = desired end
        end
    end
end

-- The tray is horizontal, so the dpad is rotated a quarter turn to match it, left
-- and right walking the tray the way up and down normally would. It is dropped
-- whenever a dialog, menu or the pause screen owns input so those keep their usual
-- directions. The hook only has to be installed once, after that Input.Update
-- detaches and restores it around loading screens on its own.
-- The claim and the backoff have to outlive the frame, so they are globals like
-- the rest of this script's state.
dpad_retry_at = dpad_retry_at or 0
dpad_claimed = dpad_claimed or false

local function ApplyDpadRotation()
    if not config.rotate_dpad then
        -- Only ever drops our own claim, the hook stays up for any other mod holding one.
        if dpad_claimed then
            Input.ReleaseDpadRotation(OWNER)
            dpad_claimed = false
        end
        return
    end

    if not dpad_claimed then
        -- Backed off because a failed install falls through to a full memory scan.
        local now = os.clock()
        if Util.IsInGame() == 0 or now < dpad_retry_at then return end
        if not Input.AcquireDpadRotation(OWNER) then
            dpad_retry_at = now + 2
            return
        end
        dpad_claimed = true
    end
    Input.Update()
    Input.SetDpadRotated(OWNER, not Util.IsUiBusy())
end

local function RestoreGameUi()
    for _, panel in ipairs(GAME_PANELS) do
        if game_ui_hidden[panel.key] then panel.enable(OWNER) end
    end
    game_ui_hidden = {}
end

--------------------------------------------------------------------------------
-- Settings, save, load, cleanup
--------------------------------------------------------------------------------

local function ColorRow(label, key)
    local new_color, used = ImGui.ColorEdit3(label, config[key])
    if used then config[key] = new_color end
end

local function IntRow(label, key, lo, hi)
    config[key] = (ImGui.SliderInt(label, config[key], lo, hi))
end

local function FloatRow(label, key, lo, hi)
    config[key] = (ImGui.SliderFloat(label, config[key], lo, hi))
end

local function HideRow(label, key)
    config.hide[key] = ImGui.Checkbox(label, config.hide[key])
end

local function FontSizeRow(label, key)
    config.font_sizes[key] = (ImGui.SliderInt(label, config.font_sizes[key], 8, 40))
end

local function Settings()
    ImGui.TextWrapped("An ARPG heads up display. Unlock to drag each piece, then lock it back down.")
    ImGui.Separator()

    config.locked = ImGui.Checkbox("Lock HUD position", config.locked)
    config.grid_enabled = ImGui.Checkbox("Snap to grid while dragging", config.grid_enabled)
    IntRow("Grid size", "grid_size", 1, 64)
    if ImGui.Button("Reset layout to defaults") then
        config.pos = {
            health = { 24, 560 }, health_orb = { 8, 500 },
            power = { 1080, 560 }, power_orb = { 1070, 500 },
            items = { 900, 560 }, exp = { 360, 700 },
            buffs = { 360, 24 }, target = { 700, 40 },
            pet = { 700, 120 },
        }
        relayout_pending = true
    end

    if ImGui.BeginCombo("Font", config.font) then
        for _, entry in ipairs(FONTS) do
            local selected = entry.name == config.font
            local now = ImGui.Selectable(entry.name, selected)
            if now and not selected then config.font = entry.name end
        end
        ImGui.EndCombo()
    end
    IntRow("Base font size", "font_size", 10, 40)
    if ImGui.TreeNode("Per component font size") then
        ImGui.TextWrapped("Only applies with a custom font, the built in font cannot resize.")
        FontSizeRow("Orbs", "orbs")
        FontSizeRow("Ability cooldowns", "abilities")
        FontSizeRow("Experience bar", "exp")
        FontSizeRow("Target nameplate", "target")
        FontSizeRow("Pet nameplate", "pet")
        FontSizeRow("Selected ability name", "selected")
        ImGui.TreePop()
    end

    ImGui.Separator()
    ImGui.Text("Elements")
    config.show_health = ImGui.Checkbox("Health panel", config.show_health)
    config.show_power  = ImGui.Checkbox("Power panel", config.show_power)
    config.show_items  = ImGui.Checkbox("Item pad", config.show_items)
    config.show_exp    = ImGui.Checkbox("Experience bar", config.show_exp)
    config.show_buffs  = ImGui.Checkbox("Buffs and debuffs", config.show_buffs)
    config.show_target = ImGui.Checkbox("Target nameplate", config.show_target)
    config.show_pet    = ImGui.Checkbox("Pet nameplate", config.show_pet)

    ImGui.Separator()
    ImGui.Text("Sizing")
    IntRow("Health panel width", "health_panel_width", 300, 800)
    IntRow("Power panel width", "power_panel_width", 140, 400)
    IntRow("Item slot size", "item_slot_size", 36, 90)
    IntRow("Experience width", "exp_width", 300, 1200)
    IntRow("Experience height", "exp_height", 10, 60)
    IntRow("Buff size", "buff_size", 24, 64)
    IntRow("Target width", "target_width", 200, 500)
    IntRow("Pet width", "pet_width", 160, 500)

    ImGui.Separator()
    ImGui.Text("Orbs (drag the orb itself to move it over its panel)")
    FloatRow("Health orb scale", "health_orb_scale", 0.3, 3.0)
    FloatRow("Power orb scale", "power_orb_scale", 0.3, 3.0)
    config.orb_scroll = ImGui.Checkbox("Scrolling liquid noise", config.orb_scroll)
    IntRow("Scroll speed", "orb_scroll_speed", 0, 120)
    FloatRow("Scroll strength", "orb_scroll_strength", 0.0, 1.0)

    ImGui.Separator()
    ImGui.Text("Ability slots")
    config.dim_out_of_range = ImGui.Checkbox("Dim when target out of range", config.dim_out_of_range)
    config.dim_low_power = ImGui.Checkbox("Dim when not enough power", config.dim_low_power)
    config.show_cooldown_numbers = ImGui.Checkbox("Cooldown seconds", config.show_cooldown_numbers)
    config.show_selected_name = ImGui.Checkbox("Selected ability name box", config.show_selected_name)
    IntRow("Selected highlight thickness", "selected_thickness", 1, 5)
    config.gcd_enabled = ImGui.Checkbox("Radial sweep on ready slots after a cast", config.gcd_enabled)
    if config.gcd_enabled then
        IntRow("Global cooldown (ms)", "gcd_duration_ms", 0, 1500)
    end

    ImGui.Separator()
    ImGui.Text("Controls")
    config.rotate_dpad = ImGui.Checkbox("Rotate dpad for the horizontal tray", config.rotate_dpad)
    if config.rotate_dpad then
        if Input.IsDpadHookSuspended() then
            ImGui.Text("Waiting to reattach.")
        end
        local dpad_err = Input.GetDpadHookError()
        if dpad_err ~= nil then
            ImGui.TextWrapped(dpad_err)
        end
    end

    ImGui.Separator()
    ImGui.Text("Colors")
    ColorRow("Health", "health_color")
    ColorRow("Power", "power_color")
    ColorRow("Experience", "exp_color")
    ColorRow("Target hostile", "target_hostile_color")
    ColorRow("Target neutral", "target_neutral_color")
    ColorRow("Target friendly", "target_friendly_color")

    ImGui.Separator()
    if ImGui.TreeNode("Hide the game's own UI") then
        HideRow("Hide health bar", "health_bar")
        HideRow("Hide power bar", "power_bar")
        HideRow("Hide experience bars", "exp_bars")
        HideRow("Hide ability bar", "ability_bar")
        HideRow("Hide active effects", "active_effects")
        HideRow("Hide target nameplate", "target_nameplate")
        HideRow("Hide pet panel", "pet_panel")
        ImGui.TreePop()
    end
end

local function Save()
    return { config = config }
end

local function Load(saved)
    if type(saved) ~= "table" or type(saved.config) ~= "table" then return end
    for k, v in pairs(saved.config) do
        -- Merge the nested tables key by key so a profile saved before a key
        -- existed keeps this version's default for it instead of dropping it.
        if (k == "pos" or k == "hide" or k == "font_sizes") and type(v) == "table" and type(config[k]) == "table" then
            for sub_k, sub_v in pairs(v) do
                config[k][sub_k] = sub_v
            end
        else
            config[k] = v
        end
    end
    relayout_pending = true
end

local function Cleanup()
    Input.ReleaseDpadRotation(OWNER)
    dpad_claimed = false
    RestoreGameUi()
    ReleaseTextures()
    AbilityList.Off(OWNER, AbilityList.Events.OnLockoutStart)
    AbilityList.Off(OWNER, AbilityList.Events.OnLockoutEnd)
    AbilityList.Release(OWNER)
    Icon.Release(OWNER)
    cooldown_running = {}
    initialized = false
end

--------------------------------------------------------------------------------
-- Lifecycle
--------------------------------------------------------------------------------

if not initialized then
    SubscribeAbilities()
    cooldown_running = {}
    UiForge.RegisterCallback(UiForge.CallbackType.Settings, Settings)
    UiForge.RegisterCallback(UiForge.CallbackType.Save, Save)
    UiForge.RegisterCallback(UiForge.CallbackType.Load, Load)
    UiForge.RegisterCallback(UiForge.CallbackType.DisableScript, Cleanup)
    UiForge.RegisterCallback(UiForge.CallbackType.OnEject, Cleanup)
    initialized = true
end

--------------------------------------------------------------------------------
-- Per frame
--------------------------------------------------------------------------------

-- Outside the in game guard so the hook still detaches on the way out.
ApplyDpadRotation()

if Util.IsInGame() ~= 0 then
    Icon.Acquire(OWNER)
    AbilityList.Acquire(OWNER)
    AbilityList.Update()
    LoadTextures()
    ApplyGameUiState()

    if Util.IsStartMenuOpen() == 0 then
        selected_name_box = nil
        -- Panels first, then orbs so the orbs sit on top of the panel art.
        if config.show_health then DrawHealthPanel() end
        if config.show_power then DrawPowerPanel() end
        if config.show_items then DrawItemPad() end
        if config.show_health then DrawHealthOrb() end
        if config.show_power then DrawPowerOrb() end
        if config.show_exp then DrawExpBar() end
        if config.show_buffs then DrawBuffs() end
        if config.show_target then DrawTargetFrame() end
        if config.show_pet then DrawPetFrame() end
        if config.show_selected_name then DrawNameBox(selected_name_box) end
    end

    relayout_pending = false
end
