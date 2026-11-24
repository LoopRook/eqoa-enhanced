-- Character: the character summary (stats, resists, training points), Class Masteries, factions and the combat
-- record in a window of their own, on its own key (default Shift+C).
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- It sits over play: the character keeps walking, and the game's Start menu pages are untouched. Spending a training
-- point and buying or switching off a Class Mastery skill always ask "Are you sure?" first (the client draws that
-- question; only your own Yes sends the request).
local C = require("character")

eqoa.input.bind({ id = "open", label = "Character", default_key = "shift+c" }, function()
    if C.is_open() then C.close() else C.open("key") end
end)
eqoa.events.on("mod.command", function(e)
    if e.data.command == "open" then C.open("mod") end
end)
eqoa.events.on("disabling", function() C.close() end)
eqoa.events.on("world.leaving", function() C.close() end)
