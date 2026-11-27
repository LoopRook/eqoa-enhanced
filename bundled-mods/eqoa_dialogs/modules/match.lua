-- Quest turn-in matching: pure Lua, no eqoa calls (so it can be tested alone).
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- An option matches a quest only when the quest's title appears in the option's own text (whole words, any case,
-- punctuation ignored). The option's position or number is never used.
local M = {}

-- lower case, every run of anything but letters and digits becomes one space, trimmed
function M.norm(s)
    s = tostring(s or ""):lower()
    s = s:gsub("[^%w\128-\255]+", " ")
    return (s:gsub("^%s+", ""):gsub("%s+$", ""))
end

-- true when the normalised title is a whole-word part of the normalised text
function M.contains(text, title)
    local t, q = M.norm(text), M.norm(title)
    if q == "" then return false end
    return (" " .. t .. " "):find(" " .. q .. " ", 1, true) ~= nil
end

-- The active quest an option's text names: the one with the longest matching title, or nil.
function M.quest_for(option_text, quests)
    local best, best_len
    for _, q in ipairs(quests or {}) do
        local title = q and q.title
        if title and M.contains(option_text, title) then
            local n = #M.norm(title)
            if not best or n > best_len then best, best_len = q, n end
        end
    end
    return best
end

-- Every option of a dialog that names an active quest: { {index = <1-based position>, quest = q, text = option}, ... }
function M.matches(options, quests)
    local out = {}
    for i, text in ipairs(options or {}) do
        local q = M.quest_for(text, quests)
        if q then out[#out + 1] = { index = i, quest = q, text = text } end
    end
    return out
end

-- The client's quest marks carry the guide's tag on a name ("Commander Nothard (guide)"): the plain name is what is compared.
local GUIDE = " (guide)"

-- true when a quest "step" mark (a "?": a step to do or a hand-in, not a quest on offer) is on this name.
-- Names are compared whole, any case, punctuation ignored; the mark's position is never used.
function M.has_step_mark(markers, name)
    local want = M.norm(name)
    if want == "" then return false end
    for _, m in ipairs(markers or {}) do
        if m.kind == "quest" and m.step == true and type(m.name) == "string" then
            local plain = m.name
            if plain:sub(-#GUIDE) == GUIDE then plain = plain:sub(1, #plain - #GUIDE) end
            if M.norm(plain) == want then return true end
        end
    end
    return false
end

return M
