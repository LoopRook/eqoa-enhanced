-- Native eqoa/1 example. All coordinates are physical output pixels.
local ui = eqoa.imgui
hearts_state = hearts_state or { x = 40, y = 100, size = 18 }
local settings = hearts_state
local claim

local function release()
    if claim then eqoa.hud.show(claim); claim = nil end
end

eqoa.settings.on_save(function() return settings end)
eqoa.settings.on_load(function(saved)
    if type(saved) == "table" then
        for _, key in ipairs({ "x", "y", "size" }) do
            if type(saved[key]) == "number" then settings[key] = saved[key] end
        end
        settings.size = math.max(8, math.min(64, settings.size))
    end
end)

eqoa.events.on("world.leaving", release)
eqoa.events.on("disabling", release)
eqoa.events.on("state.reset", function() release() end)
eqoa.events.on("render", function()
    local player = eqoa.player.get()
    if not player or not player.hp or player.hp.maximum <= 0 then release(); return end
    if not claim then claim = eqoa.hud.hide("health") end
    local fraction = math.max(0, math.min(1, player.hp.current / player.hp.maximum))
    local filled = math.ceil(fraction * 10)
    local draw = ui.GetBackgroundDrawList()
    for i = 1, 10 do
        local size = settings.size
        local x = settings.x + (i - 1) * (size + 4)
        local y = settings.y
        local color = i <= filled and 0xff5050ec or 0xff404048
        draw:AddCircleFilled(ui.vec2(x + size * 0.28, y + size * 0.28), size * 0.28, color)
        draw:AddCircleFilled(ui.vec2(x + size * 0.72, y + size * 0.28), size * 0.28, color)
        draw:AddTriangleFilled(ui.vec2(x, y + size * 0.3), ui.vec2(x + size, y + size * 0.3), ui.vec2(x + size * 0.5, y + size), color)
    end
    draw:AddText(ui.vec2(settings.x, settings.y + settings.size + 5), 0xffffffff,
        tostring(player.hp.current) .. " / " .. tostring(player.hp.maximum))
end)
