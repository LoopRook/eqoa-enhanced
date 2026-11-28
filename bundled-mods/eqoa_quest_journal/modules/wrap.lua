-- Text wrapping for the Quest Journal: pure Lua, no eqoa calls (so it can be tested alone).
-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The same rule as the Bag Viewer's tooltip (`wrap_card`): a line goes on over several at the spaces, and a word wider than the
-- width is cut. `width(text)` measures a text in the same unit as `maxw`.
local M = {}

function M.wrap(text, maxw, width)
    local out, cur = {}, ""
    for word in tostring(text):gmatch("%S+") do
        while width(word) > maxw do
            local cut = #word
            while cut > 1 and width(word:sub(1, cut)) > maxw do cut = cut - 1 end
            if cur ~= "" then out[#out + 1] = cur; cur = "" end
            out[#out + 1] = word:sub(1, cut)
            word = word:sub(cut + 1)
        end
        local try = cur == "" and word or (cur .. " " .. word)
        if cur ~= "" and width(try) > maxw then out[#out + 1] = cur; cur = word else cur = try end
    end
    if cur ~= "" then out[#out + 1] = cur end
    if #out == 0 then out[1] = "" end
    return out
end

return M
