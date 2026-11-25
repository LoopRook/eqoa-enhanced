-- Bag viewer model: what the window shows, read from eqoa.inventory / eqoa.bank / eqoa.loot and game events.
-- Nothing here is a copy of game state that can drift: the grid is read again on a change event or an open.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local M = {
    open = false, tab = 1, grid = nil, log = {}, scroll = 0, grid_scroll = 0, hover = nil,
    builds = 0, draws = 0, rect = nil, menu = nil, result = nil,
    pending_label = nil, last_req = nil,
    junk_ids = {}, junk_loaded = false,
    -- Auto-loot: "off", "all" (everything) or "non_junk" (everything but the items marked as junk); saved with the junk marks
    auto_mode = "off",
    -- the Auto-loot choice of each character (the junk marks stay shared: they are per item type); `character` is whose
    -- choice `auto_mode` is, `char_dirty` that the character may have changed (a new world, a login)
    auto_by_char = {}, character = nil, char_dirty = true, char_checked = 0,
    scan_count = 0, -- looks at the zone taken (for the tests)
    -- how far the server answered a merchant or a banker (kept per session; see `note_range`)
    range = {merchant = {}, bank = {}},
    -- a greyed button the player clicked once: the second click (within 10 s, about the same distance) is let through
    try_anyway = {},
    batch_id = nil, batch_kind = nil, batch_verb = nil, batch_total = 0,
    corpse_tab_kept = false,
    action_hits = {},
    icon_req = {}, has_pending_icons = false,
}

local JUNK_FILE = "junk.cfg"
local TRIM_OPTS = {trim = true, trim_color = {r = 0, g = 0, b = 0, a = 1}}
local ICON_RETRY_LIMIT = 5
local ICON_RETRY_COOLDOWN_S = 10
local _frame_tried = {}
local _frame_tex = {}
local EMPTY_CORPSE = {items = {}, used = 0, capacity = 40, money = 0, revision = -1}

local function loot_batch_active()
    return M.batch_id ~= nil and M.batch_kind == "loot"
end

local function keeping_corpse_tab()
    return M.tab == 7 and (loot_batch_active() or M.corpse_tab_kept)
end

local function dist_sq(a, b)
    if not a or not b then return nil end
    local dx = (a.x or 0) - (b.x or 0)
    local dy = (a.y or 0) - (b.y or 0)
    local dz = (a.z or 0) - (b.z or 0)
    return dx * dx + dy * dy + dz * dz
end

-- One look at the zone's creatures answers every per-frame question (the nearest corpse, merchant and banker, a live
-- hostile with the player targeted). `eqoa.entities.list()` hands over a record for every creature in the zone, so
-- asking it per question, per frame, cost the Bags window 7-10 ms a frame in a busy zone (owner log 2026-11-18). The
-- look is kept SCAN_TTL seconds (distances only drift by a step in that time), so a crowded zone never makes it one
-- look per frame; Auto-loot's poll (`auto_poll`) shares it.
local SCAN_TTL, SCAN_MIN = 0.5, 0.25
-- What the open window asks (is a merchant, a banker or a corpse near: a button's place and colour) may be this old
-- before the zone is looked at again: they drift by steps, and a look is ~3.4 ms. Everything that acts on the answer
-- (Auto-loot's poll, the fight check, the helpers) keeps SCAN_TTL.
M.DRAW_TTL = 1.0
-- A merchant or banker farther than this is not "nearby" at all: no Sell junk or bank row for one at the other end of a
-- city (the "too far" line is for one within sight that the server may not answer yet).
M.NPC_SHOWN_WITHIN = 40.0
local scan_at, scan_data, scan_dirty, scan_pid = nil, nil, false, nil

local function has_bit(val, b)
    return val ~= nil and math.floor(val / b) % 2 == 1
end

-- A live NPC with the player targeted is a fight only when it can be one: not the player's pet or a group member (a
-- mercenary heals or follows you with you targeted), not a merchant, banker or other service NPC (they face whoever
-- spoke to them), and not farther than a fight reaches (a creature that once chased you keeps you as its target at the
-- other end of the zone). Owner log 2026-11-18: Auto-loot stopped for good while such an NPC had the player targeted.
-- Returns why it is not a fight, nil when it is one.
M.FIGHT_RANGE = 30.0
function M.not_a_fight(e, ppos, pl)
    if pl and pl.pet_id ~= nil and e.id == pl.pet_id then return "your pet" end
    local flags = e.npc_flags
    if flags and (has_bit(flags, 1) or has_bit(flags, 2) or has_bit(flags, 8) or has_bit(flags, 128)) then
        return "a merchant, banker or trainer"
    end
    local d2 = dist_sq(ppos, e.actor_position or e.position)
    if d2 and d2 > M.FIGHT_RANGE * M.FIGHT_RANGE then return "too far away to fight" end
    local g = eqoa.group and eqoa.group.get and eqoa.group.get()
    if g and g.members then
        for _, m in ipairs(g.members) do
            if m.id == e.id then return "in your group" end
        end
    end
    return nil
end

local function scan_zone(ttl)
    local now = os.clock()
    if scan_data and scan_at and now >= scan_at then
        local age = now - scan_at
        if age < (ttl or SCAN_TTL) and not (scan_dirty and age >= SCAN_MIN) then return scan_data end
    end
    local data = {corpses = {}, nearest = {}}
    M.scan_count = M.scan_count + 1
    scan_data, scan_at, scan_dirty = data, now, false
    local list = eqoa.entities and eqoa.entities.list and eqoa.entities.list()
    if not list then
        data.none = true
        return data
    end
    local pl = eqoa.player and eqoa.player.get and eqoa.player.get()
    local ppos = pl and pl.position
    local pid = pl and pl.id
    scan_pid = pid
    local nearest = data.nearest
    local function consider(kind, e, d2)
        local cur = nearest[kind]
        if not cur or d2 < cur.d2 then nearest[kind] = {e = e, d2 = d2} end
    end
    for _, e in ipairs(list) do
        if e.corpse then
            local d2 = dist_sq(ppos, e.actor_position or e.position) or 0
            data.corpses[#data.corpses + 1] = e
            consider("corpse", e, d2)
        else
            if pid and not data.hostile and e.kind == "npc" and e.target_id == pid and not M.not_a_fight(e, ppos, pl) then
                data.hostile = (e.name and e.name ~= "") and e.name or ("creature " .. tostring(e.id))
            end
            local flags = e.npc_flags
            if flags then
                local merchant, bank = has_bit(flags, 1), has_bit(flags, 2)
                if merchant or bank or has_bit(flags, 128) or has_bit(flags, 8) then
                    local d2 = dist_sq(ppos, e.actor_position or e.position) or 0
                    if d2 <= M.NPC_SHOWN_WITHIN * M.NPC_SHOWN_WITHIN then
                        consider("any", e, d2)
                        if merchant then consider("merchant", e, d2) end
                        if bank then consider("bank", e, d2) end
                    end
                end
            end
        end
    end
    return data
end

function M.scan_reset()
    scan_at, scan_data, scan_dirty = nil, nil, false
end

function M.nearby_corpse(ttl)
    local data = scan_zone(ttl)
    local c = data.nearest.corpse
    if not c then return nil end
    return c.e, c.d2
end

function M.nearby_npc(kind, ttl)
    local data = scan_zone(ttl)
    local c = data.nearest[kind == "merchant" and "merchant" or (kind == "bank" and "bank" or "any")]
    if not c then return nil end
    return c.e, c.d2
end

-- How near a merchant or a banker must be. The game has no range check of its own: the server decides, and a
-- request out of its reach is simply not answered. The mod learns the reach from real answers, per session: the
-- largest distance that got an answer (`ok`) and the smallest that got none (`fail`). Until a distance is known to
-- work or not to, the conservative DEFAULT_REACH decides. It only greys out the mod's own buttons: a window the
-- player opened (or that is open) is never blocked.
M.DEFAULT_REACH = 10.0
local TOO_FAR = {merchant = "too far from the merchant", bank = "too far from the banker"}

-- An answer at `dist` always clears the doubt (`fail`, the misses). Silence is weaker evidence than an answer: one
-- unanswered request (lag, a busy server) greys nothing; two misses grey the smallest distance missed. A miss closer
-- than the farthest answer is different: the reach has changed, the old answers no longer count and it greys at once.
function M.note_range(kind, dist, answered)
    local r = M.range[kind]
    if not r or type(dist) ~= "number" then return end
    if answered then
        if not r.ok or dist > r.ok then r.ok = dist end
        r.fail, r.misses = nil, {}
        M.try_anyway[kind] = nil
        return
    end
    M.try_anyway[kind] = nil
    if r.ok and dist <= r.ok then
        r.ok, r.fail, r.misses = nil, dist, {dist}
        return
    end
    r.misses = r.misses or {}
    r.misses[#r.misses + 1] = dist
    if #r.misses >= 2 then
        local low = r.misses[1]
        for _, d in ipairs(r.misses) do
            if d < low then low = d end
        end
        r.fail = low
    end
end

-- Is `dist` within the reach of a `kind` NPC, as far as the mod knows?
function M.within_reach(kind, dist)
    local r = M.range[kind] or {}
    if r.ok and dist <= r.ok then return true end
    if r.fail and dist >= r.fail then return false end
    if r.ok and r.fail then return true end -- between a distance that worked and one that did not: worth trying
    return dist <= M.DEFAULT_REACH
end

-- The nearest `kind` NPC's distance (nil when none is in the zone). `fresh` asks the zone again first.
function M.npc_distance(kind, fresh, ttl)
    if fresh then M.scan_reset() end
    local npc, d2 = M.nearby_npc(kind, ttl)
    if not npc then return nil end
    return math.sqrt(d2 or 0), npc
end

-- Why a button for `kind` is out of reach ("too far from the banker"), or nil when it is in reach or there is none.
function M.too_far(kind, ttl)
    local dist = M.npc_distance(kind, nil, ttl)
    if dist == nil or M.within_reach(kind, dist) then return nil end
    return TOO_FAR[kind]
end

-- Called when a button that is greyed for being too far is clicked: the first click says why and asks to click again, the
-- second (within 10 s, standing about where they stood) goes through, so the mod's guess never keeps a try from the
-- player. Returns true when the click is held back.
function M.far_gate(kind, verb)
    local far = M.too_far(kind)
    if not far then return false end
    local dist = M.npc_distance(kind)
    local now = os.clock()
    local t = M.try_anyway[kind]
    if t and now <= t.expires and dist and math.abs(t.dist - dist) < 2 then
        t.expires = now + 10
        return false
    end
    M.try_anyway[kind] = {dist = dist or 0, expires = now + 10}
    M.result = {text = verb .. ": " .. far .. " (click again to try anyway)", ok = false}
    M.dirty()
    return true
end

-- A window of `kind` that the player opened by hand answers from where they stand: that is a real answer too.
function M.learn_native_open(kind)
    if M.pending_open and M.pending_open.kind == kind then return end
    local dist = M.npc_distance(kind, true)
    if dist then M.note_range(kind, dist, true) end
end

local function release_takeover(kind)
    if kind and eqoa.windows and eqoa.windows.release then
        eqoa.windows.release(kind)
    end
end
M.release_takeover = release_takeover

function M.ensure_takeover(kind)
    if not (eqoa.windows and eqoa.windows.takeover and eqoa.windows.taken) then
        return true
    end
    if eqoa.windows.taken(kind) then
        return true
    end
    local ok, err = eqoa.windows.takeover(kind)
    if not ok then
        M.result = {text = (err and err.message) or ("cannot take over " .. kind), ok = false}
        M.dirty()
        return false
    end
    return true
end

function M.clear_corpse_hold()
    M.corpse_tab_kept = false
    M.pending_open = nil
    M.held_for_req = nil
    M.eligible_corpses = {}
    M.auto_looted = {}
    M.tracked_target_id = nil
    M.tracked_target_pos = nil
    M.recently_dead = nil
    M.engaged = {}
    M.auto_retry = {}
    -- corpses waiting for bag space or for another try, the close marks and the guard are the old world's: a new creature
    -- may be given the same entity id
    M.auto_bags, M.auto_more = {}, {}
    M.last_close, M.auto_player_open, M.keep_req = nil, nil, nil
    M.auto_take = nil
    M.auto_done = {}
    M.auto_left = {}
    M.auto_said = {}
    M.engaged_pos = {}
    M.corpses_seen, M.corpses_seen_in = nil, nil
end

M.COLUMNS, M.MAX_ROWS, M.LOG_MAX = 8, 6, 50
-- The game's item-page tabs, in its order, with All first and the bank last. A stack is under a tab when the
-- game's own rule for that tab says so: `Item.tabs` lists the keys, the rule is not copied here.
M.TABS = {
    {key = "all", label = "All"},
    {key = "usable", label = "Usable"},
    {key = "equipment", label = "Equipment"},
    {key = "charged", label = "Charged"},
    {key = "misc", label = "Misc"},
    {key = "bank", label = "Bank"},
}
M.CORPSE_TAB = {key = "corpse", label = "Corpse"}

local function has_tab(it, key)
    if not it or not it.tabs then return false end
    for _, k in ipairs(it.tabs) do if k == key then return true end end
    return false
end

local function keeps(key, it)
    if key == "all" or key == "bank" or key == "corpse" then return true end
    return has_tab(it, key)
end

function M.visible_tabs()
    local tabs = {}
    for _, t in ipairs(M.TABS) do tabs[#tabs + 1] = t end
    if eqoa.loot.get() or keeping_corpse_tab() or M.nearby_corpse(M.DRAW_TTL) ~= nil then
        local t = M.CORPSE_TAB
        tabs[#tabs + 1] = {key = t.key, label = t.label, idx = 7}
    end
    for i, t in ipairs(tabs) do
        tabs[i] = {key = t.key, label = t.label, idx = t.idx or i}
    end
    return tabs
end

function M.reset_icons()
    for k in pairs(M.icon_req) do M.icon_req[k] = nil end
    M.has_pending_icons = false
end

local function clear_pass_cache()
    for k in pairs(_frame_tried) do _frame_tried[k] = nil end
    for k in pairs(_frame_tex) do _frame_tex[k] = nil end
end

local function fetch_entry_icon(entry)
    local icon = entry.item and entry.item.icon
    if not icon then return end
    local st = M.icon_req[icon]
    if type(st) == "number" and st < 0 then return end
    local cached = _frame_tex[icon]
    if cached then
        entry.texture = cached
        return
    end
    if _frame_tried[icon] then
        M.has_pending_icons = true
        return
    end
    local info = eqoa.icons.resource(icon, TRIM_OPTS)
    if info and info.texture then
        entry.texture = info.texture
        M.icon_req[icon] = "ok"
        _frame_tex[icon] = info.texture
    else
        _frame_tried[icon] = true
        local tries = (type(st) == "number" and st > 0 and st or 0) + 1
        if tries >= ICON_RETRY_LIMIT then
            M.icon_req[icon] = -(os.clock() + ICON_RETRY_COOLDOWN_S)
        else
            M.icon_req[icon] = tries
            M.has_pending_icons = true
        end
    end
end

function M.sync_visible_icons(first_slot, last_slot)
    local grid = M.grid
    if not grid then return end
    M.has_pending_icons = false
    clear_pass_cache()
    local now = os.clock()
    local items = grid.items
    for i = 1, #items do
        local entry = items[i]
        if i < first_slot or i > last_slot then
            entry.texture = nil
        elseif entry.texture == nil then
            local icon = entry.item and entry.item.icon
            local st = icon and M.icon_req[icon]
            if type(st) == "number" and st < 0 and now >= -st then
                M.icon_req[icon] = nil
            end
            fetch_entry_icon(entry)
        end
    end
    clear_pass_cache()
end

function M.resolve_pending_icons(first_slot, last_slot)
    local grid = M.grid
    if not grid or not M.has_pending_icons then return end
    M.has_pending_icons = false
    clear_pass_cache()
    local items = grid.items
    local hi = math.min(#items, last_slot)
    for i = math.max(1, first_slot), hi do
        local entry = items[i]
        if entry.texture == nil and entry.item and entry.item.icon then
            fetch_entry_icon(entry)
        end
    end
    clear_pass_cache()
end

local function active_key_and_container()
    local key
    if M.tab == 7 then
        key = "corpse"
    elseif M.TABS[M.tab] then
        key = M.TABS[M.tab].key
    else
        M.tab = 1
        key = "all"
    end
    local c
    if key == "bank" then
        c = eqoa.bank.get()
    elseif key == "corpse" then
        c = eqoa.loot.get()
        if c then
            if not loot_batch_active() then
                M.corpse_tab_kept = false
            end
        elseif keeping_corpse_tab() or M.nearby_corpse() ~= nil then
            c = EMPTY_CORPSE
        else
            M.tab = 1
            key = "all"
            c = eqoa.inventory.get()
        end
    else
        c = eqoa.inventory.get()
    end
    return key, c
end

function M.is_grid_stale()
    local grid = M.grid
    if grid == nil then return true end
    local key, c = active_key_and_container()
    if not c then return grid ~= false end
    if grid == false then return true end
    return grid.tab_key ~= key or grid.rev ~= c.revision
end

function M.visible_slot_range()
    local grid = M.grid
    if not grid then return 1, 0 end
    local first = M.grid_scroll * M.COLUMNS + 1
    local last = (M.grid_scroll + grid.rows) * M.COLUMNS
    return first, last
end

function M.scroll_grid(delta)
    local grid = M.grid
    if not grid then return end
    local max_scroll = math.max(0, (grid.total_rows or grid.rows) - grid.rows)
    local next_scroll = math.max(0, math.min(max_scroll, M.grid_scroll + delta))
    if next_scroll == M.grid_scroll then return end
    M.grid_scroll = next_scroll
    local first, last = M.visible_slot_range()
    M.sync_visible_icons(first, last)
end

-- The grid for the tab on show: the items it keeps in the container's own order, with the numbers under it.
function M.rebuild()
    M.builds = M.builds + 1
    local key, c = active_key_and_container()
    if not c then M.grid = false; M.has_pending_icons = false; M.grid_scroll = 0; return false end
    local items = {}
    for _, it in ipairs(c.items) do
        if keeps(key, it) then
            items[#items + 1] = {item = it, texture = nil}
        end
    end
    if M.multi then M.multi.prune(items) end -- a selected stack that left the view leaves the selection
    local slots = math.max(c.capacity or 40, #c.items)
    local total_rows = math.max(5, math.ceil(slots / M.COLUMNS))
    local rows = math.min(M.MAX_ROWS, total_rows)
    M.grid_scroll = math.max(0, math.min(math.max(0, total_rows - rows), M.grid_scroll))
    M.grid = {
        items = items, used = c.used, capacity = c.capacity, money = c.money,
        rows = rows, total_rows = total_rows,
        bank = key == "bank", corpse = key == "corpse",
        tab_key = key, rev = c.revision,
    }
    local first, last = M.visible_slot_range()
    M.sync_visible_icons(first, last)
    M.hover = nil
    M.menu = nil
    return M.grid
end

function M.dirty()
    for k, v in pairs(M.icon_req) do
        if type(v) == "number" and v < 0 then M.icon_req[k] = nil end
    end
    M.grid = nil
end

function M.select(tab)
    if M.multi and tab ~= M.tab then M.multi.clear() end -- multi-select: a selection belongs to the tab it was made on
    if tab == 7 and (eqoa.loot.get() or keeping_corpse_tab() or M.nearby_corpse() ~= nil) and M.tab ~= 7 then
        if M.tab == 6 then release_takeover("bank") end
        M.tab = 7; M.scroll = 0; M.grid_scroll = 0; M.grid = nil; M.menu = nil
    elseif tab ~= M.tab and M.TABS[tab] then
        if M.tab == 7 then
            M.note_player_closed()
            release_takeover("loot")
        end
        if M.tab == 6 then release_takeover("bank") end
        M.corpse_tab_kept = false
        M.tab = tab; M.scroll = 0; M.grid_scroll = 0; M.grid = nil; M.menu = nil
    end
end

-- 1234567 -> "1,234,567"
function M.tunar(n)
    local s, r = tostring(math.floor(n or 0)), nil
    repeat s, r = s:gsub("^(-?%d+)(%d%d%d)", "%1,%2") until r == 0
    return s
end

-- The card for one item: name, level, slot, stats, HP and durability, value, and the red line when the
-- character's class, race or level rule it out. Read once per item hovered, not once per frame.
-- The game's class and race names by ID (the order of the class and race lists the items' masks use).
M.CLASS_NAMES = {[0] = "Warrior", "Ranger", "Paladin", "Shadowknight", "Monk", "Bard", "Rogue", "Druid", "Shaman",
    "Cleric", "Magician", "Necromancer", "Enchanter", "Wizard", "Alchemist"}
M.RACE_NAMES = {[0] = "Human", "Elf", "Dark Elf", "Gnome", "Dwarf", "Troll", "Barbarian", "Halfling", "Erudite", "Ogre"}
local CLASS_COUNT, RACE_COUNT = 15, 10

-- "Warrior, Paladin", or "" when the list is empty or names every one (an item nobody is shut out of).
local function names_of(ids, names, count)
    if type(ids) ~= "table" or #ids == 0 or #ids >= count then return "" end
    local out = {}
    for _, id in ipairs(ids) do out[#out + 1] = names[id] or ("#" .. tostring(id)) end
    return table.concat(out, ", ")
end

-- What rules the item out for this character: a set like {class = true}, or nil when it can be used.
function M.blockers(it)
    local can = eqoa.items.can_use(it.ref)
    if not can or can.allowed then return nil end
    local set = {}
    for _, r in ipairs(can.reasons or {}) do set[r] = true end
    return set
end

-- Gear: an item with a wear slot (the game's own test, `wear_slot != -1`; the server's 0xFFFFFFFF is "none").
function M.is_gear(it)
    return it.wear_slot ~= nil and it.wear_slot ~= 4294967295
end

-- The grid draws an item's icon dim when this is true: gear the character cannot use, and only gear. The game
-- itself consults the class, race and level rules (`VIPlayer::CanUse`) only for an item with a wear slot (its
-- bank, duel and pack pages badge only those); crafting, misc, junk and sellable items have no class or race
-- rule to meet (their masks do not name the character), so `can_use` says no for some of them: they are never dimmed. Kept on the entry, and read again when the
-- character's level changes (the stamp), not once per frame.
function M.icon_dimmed(entry, stamp)
    if entry.dim_stamp ~= stamp then
        entry.dim_stamp = stamp
        entry.dim = entry.item ~= nil and M.is_gear(entry.item) and M.blockers(entry.item) ~= nil
    end
    return entry.dim
end

-- The corner letter for a pack item set aside for a helper: "B" marked to bank, "R" kept up to a number when selling
-- or restocking. (Junk has its own "J".) nil for anything else. Bottom-left, clear of the stack count.
function M.mark_letter(it)
    local H = M.helpers
    if not H or it.list ~= "inventory" then return nil end
    if H.is_banked(it) then return "B" end
    if H.keep_of(it) then return "R" end
    return nil
end

local DESCRIPTION_WIDTH, DESCRIPTION_LINES = 34, 3

-- The description cut into lines of about DESCRIPTION_WIDTH characters at spaces, at most DESCRIPTION_LINES, with
-- "..." when it goes on.
function M.wrap_description(text)
    local lines, line = {}, ""
    local cut = false
    for word in tostring(text or ""):gmatch("%S+") do
        if line ~= "" and #line + 1 + #word > DESCRIPTION_WIDTH then
            lines[#lines + 1] = line
            line = word
            if #lines == DESCRIPTION_LINES then cut = true; line = ""; break end
        else
            line = line == "" and word or (line .. " " .. word)
        end
    end
    if line ~= "" then lines[#lines + 1] = line end
    if cut then lines[#lines] = lines[#lines] .. "..." end
    return lines
end

function M.card(it)
    local h = M.hover
    if h and h.key == it.ref then return h.lines end
    local why = M.blockers(it) or {}
    local lines = {{it.name, "title"}}
    if (it.level_required or 0) > 0 then
        lines[#lines + 1] = {"Level " .. it.level_required, why.level and "red" or "text"}
    end
    -- the class and race lines are for gear only, as the dimming is: a crafting or misc item has no class or race
    -- rule, and "All classes" on it was a statement the game never makes
    if M.is_gear(it) then
        local classes = names_of(it.allowed_classes, M.CLASS_NAMES, CLASS_COUNT)
        lines[#lines + 1] = {classes == "" and "All classes" or ("Class: " .. classes), why.class and "red" or "text"}
        local races = names_of(it.allowed_races, M.RACE_NAMES, RACE_COUNT)
        if races ~= "" then lines[#lines + 1] = {"Race: " .. races, why.race and "red" or "text"} end
    end
    if it.slot_name then lines[#lines + 1] = {"Slot: " .. it.slot_name, "text"} end
    for _, s in ipairs(it.stats) do
        lines[#lines + 1] = {string.format("%s %+d", s.name or ("Stat " .. s.id), s.value), "text"}
    end
    if (it.maximum_hp or 0) > 0 then lines[#lines + 1] = {"HP " .. it.current_hp .. "/" .. it.maximum_hp, "text"} end
    if (it.durability or 0) > 0 then lines[#lines + 1] = {"Durability " .. it.durability, "text"} end
    if (it.value or 0) > 0 then lines[#lines + 1] = {"Value " .. M.tunar(it.value) .. " tunar", "muted"} end
    if next(why) ~= nil then lines[#lines + 1] = {"You can't use this", "red"} end
    if M.helpers and it.list == "inventory" then M.helpers.card_lines(it, lines) end
    for _, d in ipairs(M.wrap_description(it.description)) do lines[#lines + 1] = {d, "muted"} end
    M.hover = {key = it.ref, lines = lines}
    return lines
end

local function cannot_reason(it, can)
    if not can or can.allowed then return nil end
    local r = can.reasons and can.reasons[1]
    if r == "class" then return "wrong class"
    elseif r == "race" then return "wrong race"
    elseif r == "level" then return "requires level " .. tostring(it.level_required or 0)
    end
    return "cannot use item"
end

M.AUTO_MODES = {
    {key = "off", label = "Off", explain = "Auto-loot: off. Loot with the buttons below."},
    {key = "all", label = "Everything", explain = "Auto-loot: takes every item from creatures you kill."},
    {key = "non_junk", label = "Everything but junk", explain = "Auto-loot: takes all but items marked as junk."},
}

local function auto_mode_valid(key)
    for _, m in ipairs(M.AUTO_MODES) do
        if m.key == key then return true end
    end
    return false
end

function M.load_junk()
    M.junk_ids = {}
    M.auto_by_char = {}
    if eqoa and eqoa.files and eqoa.files.read then
        local data = eqoa.files.read(JUNK_FILE)
        if type(data) == "string" then
            for line in data:gmatch("[^\r\n]+") do
                local who, mode = line:match("^auto:([^=]+)=(%a[%w_]*)$")
                if who then
                    if auto_mode_valid(mode) then M.auto_by_char[who] = mode end
                elseif line:match("^%d+$") then
                    M.junk_ids[line] = true
                end
                -- anything else (a retired "grey=" line, an un-keyed "auto=" line) is ignored
            end
        end
    end
    M.junk_loaded = true
    M.character = nil
    M.char_dirty = true
    M.sync_character()
end

function M.save_junk()
    if not (eqoa and eqoa.files and eqoa.files.write) then return end
    local ids = {}
    for k, v in pairs(M.junk_ids) do
        if v then ids[#ids + 1] = k end
    end
    table.sort(ids)
    local who = {}
    for name, mode in pairs(M.auto_by_char) do
        if mode ~= "off" then who[#who + 1] = name end
    end
    table.sort(who)
    local lines = {}
    for _, name in ipairs(who) do lines[#lines + 1] = "auto:" .. name .. "=" .. M.auto_by_char[name] end
    for _, k in ipairs(ids) do lines[#lines + 1] = k end
    eqoa.files.write(JUNK_FILE, table.concat(lines, "\n") .. "\n")
end

-- The Auto-loot choice belongs to the character in the world: when it changes (a login, another world) the choice of
-- the new one is read back. Nil until the game says who the character is.
function M.sync_character()
    local pl = eqoa and eqoa.player and eqoa.player.get and eqoa.player.get()
    local name = pl and pl.name
    if type(name) ~= "string" or name == "" then return false end
    M.char_dirty = false
    if name == M.character then return false end
    M.character = name
    local mode = M.auto_by_char[name] or "off"
    M.alog(nil, "mode:" .. name .. ":" .. mode, M.auto_mode_line(mode, name))
    if mode ~= M.auto_mode then
        M.auto_mode = mode
        M.eligible_corpses = {}
        M.auto_looted = {}
        M.scan_reset()
        if M.on_auto_mode then M.on_auto_mode(mode) end
        M.dirty()
    end
    return true
end

function M.auto_mode_line(mode, name)
    local what = mode == "all" and "Everything" or (mode == "non_junk" and "Everything but junk" or "Off")
    local line = "mode " .. what .. " for " .. tostring(name)
    if mode == "off" then line = line .. ": kills are left alone" end
    return line
end

function M.auto_explain()
    for _, m in ipairs(M.AUTO_MODES) do
        if m.key == M.auto_mode then return m.explain end
    end
    return ""
end

function M.set_auto_mode(mode)
    if not auto_mode_valid(mode) then return false end
    if not M.character then M.sync_character() end
    if not M.character then
        M.result = {text = "Auto-loot: waiting for your character", ok = false}
        M.dirty()
        return false
    end
    M.auto_mode = mode
    M.auto_by_char[M.character] = mode
    M.alog_forget(nil, "mode:" .. M.character .. ":" .. mode)
    M.alog(nil, "mode:" .. M.character .. ":" .. mode, M.auto_mode_line(mode, M.character))
    -- corpses held for the old choice are not looted under the new one
    M.eligible_corpses = {}
    M.auto_looted = {}
    M.save_junk()
    M.scan_reset()
    if M.on_auto_mode then M.on_auto_mode(mode) end
    M.dirty()
    return true
end

function M.is_junk(it)
    if not it then return false end
    return M.junk_ids[tostring(it.id)] == true
end

function M.set_junk(it, flag)
    if not it then return end
    if flag then
        M.junk_ids[tostring(it.id)] = true
        M.result = {text = "Marked " .. it.name .. " as junk", ok = true}
    else
        M.junk_ids[tostring(it.id)] = nil
        M.result = {text = "Unmarked " .. it.name, ok = true}
    end
    M.save_junk()
end

-- Why an item is not sold without a question: the game's own merchant window asks Yes or No in these cases (an item worn,
-- quick-slotted, no-trade, or not a misc one). Returns the reasons as a list (worn items are never sold at all), nil when
-- the item is an ordinary one.
function M.sell_risk(it)
    local why = {}
    if it.quick_slot ~= nil then why[#why + 1] = "quick-slotted" end
    if it.no_trade then why[#why + 1] = "no-trade" end
    if not has_tab(it, "misc") then why[#why + 1] = "not an ordinary item" end
    if #why == 0 then return nil end
    return why
end

-- "quick-slotted and no-trade"
function M.risk_text(why)
    if #why <= 1 then return why[1] or "" end
    return table.concat(why, ", ", 1, #why - 1) .. " and " .. why[#why]
end

-- The junk the one-click Sell junk sells: every stack marked as junk, gear and potions included (marking is the player's
-- decision: the menu asked once when the item was not an ordinary Misc one, see `ask_to_mark`). The one exception is an item
-- worn right now, which is kept. Returns the batch steps, their count, the quoted tunar and, last, the marked junk left
-- alone ({item, reasons}) and the quote of each step (for the line said when the batch ends).
function M.junk_sell_list()
    local inv = eqoa.inventory.get()
    if not inv then return {}, 0, 0, {}, {} end
    local has_merch = eqoa.merchant.get() ~= nil
    local steps, total_tunar, skipped, quotes = {}, 0, {}, {}
    for _, it in ipairs(inv.items) do
        if M.is_junk(it) then
            if it.equipped then
                skipped[#skipped + 1] = {item = it, reasons = {"you are wearing it"}}
            else
                local qty = it.quantity or 1
                local q = has_merch and eqoa.merchant.price(it.ref, "sell", qty) or 0
                steps[#steps + 1] = {kind = "sell", item = it.ref, count = qty}
                quotes[#quotes + 1] = q or 0
                total_tunar = total_tunar + (q or 0)
            end
        end
    end
    return steps, #steps, total_tunar, skipped, quotes
end

function M.loot_non_junk_list()
    local loot = eqoa.loot.get()
    if not loot then return {}, 0 end
    local steps = {}
    for _, it in ipairs(loot.items) do
        if not M.is_junk(it) then
            steps[#steps + 1] = {kind = "loot", item = it.ref, count = it.quantity or 1}
        end
    end
    return steps, #loot.items
end

-- `keep_target` (Auto-loot): the corpse is asked for by its id and your target and auto-attack are left alone
-- (eqoa.actions.open_loot's option of that name). The client says in the result's `detail.method` what it did:
-- "keep_target", or "retarget" when the server did not accept a request that names a corpse you have not targeted and
-- the client asked the old way (target it, loot it, give the target back). A client that does not know the option
-- ignores it and says nothing: that is the old way too.
function M.open_loot_corpse(after_kind, corpse_id, keep_target)
    -- one open at a time: a second request while the first is unanswered would orphan the first (its answer, or
    -- its silence, would no longer reach the button or Auto-loot that asked)
    if M.pending_open then
        return nil, {code = "busy", message = "still waiting for the server to open a corpse"}
    end
    if not M.ensure_takeover("loot") then return nil, {message = "window busy"} end
    local req, err = eqoa.actions.open_loot(corpse_id, keep_target and {keep_target = true} or nil)
    if not req then
        release_takeover("loot")
        local msg = (err and (err.message or err.code)) or "refused"
        M.result = {text = "Open corpse: " .. msg, ok = false}
        M.dirty()
        return nil, err
    end
    local verb = after_kind == "loot_all" and "Take all"
        or (after_kind == "loot_non_junk" and "Take all but junk" or "Open corpse")
    M.pending_open = {kind = "loot", req = req, after = after_kind, verb = verb}
    M.keep_req = keep_target and req or nil -- its result says which method the client used (`note_loot_method`)
    M.result = {text = "Opening corpse...", ok = nil}
    M.dirty()
    return req, nil
end

function M.open_talk_npc(talk_kind, verb, after_fn)
    -- one open at a time, as for corpses: a second talk while the first is unanswered would take over the
    -- first's answer (its item unsold, the second sold in its place)
    if M.pending_open then
        local what = M.pending_open.kind == "loot" and "a corpse" or ("the " .. M.pending_open.kind)
        return nil, {code = "busy", message = "still waiting for the server to open " .. what}
    end
    local dist = M.npc_distance(talk_kind, true)
    if M.far_gate(talk_kind, verb) then return nil, {message = TOO_FAR[talk_kind]} end
    if not M.ensure_takeover(talk_kind) then return nil, {message = "window busy"} end
    local req, err = eqoa.actions.talk(nil, {kind = talk_kind})
    if not req then
        release_takeover(talk_kind)
        local msg = (err and (err.message or err.code)) or "refused"
        M.result = {text = verb .. ": " .. msg, ok = false}
        M.dirty()
        return nil, err
    end
    M.pending_open = {kind = talk_kind, req = req, fn = after_fn, verb = verb, dist = dist}
    M.result = {text = talk_kind == "merchant" and "Opening merchant..." or "Opening bank...", ok = nil}
    M.dirty()
    return req, nil
end

-- The Bag Viewer closed: an unanswered talk to a merchant or banker is forgotten (its late reply neither opens the game's
-- window nor runs the step); a corpse open is left to the caller's own rule.
function M.drop_pending_npc_open()
    local p = M.pending_open
    if not p or p.kind == "loot" then return end
    M.pending_open = nil
    if p.req then pcall(eqoa.actions.cancel, p.req) end
end

function M.start_loot_button()
    local l = eqoa.loot.get()
    if l and l.open then
        M.select(#M.TABS + 1)
        M.result = {text = "Corpse ready", ok = true}
        M.dirty()
        return true
    end
    return M.open_loot_corpse(nil) ~= nil
end

function M.start_sell_junk()
    -- another batch owns the merchant/bank session: refuse before anything here can release it
    if M.batch_id then return false end
    local merch = eqoa.merchant.get()
    if not merch then
        if M.nearby_npc("merchant") ~= nil then
            return M.open_talk_npc("merchant", "Sell junk", M.start_sell_junk) ~= nil
        end
        M.result = {text = "Sell junk: merchant is not open", ok = false}
        return false
    end
    local steps, count, _, skipped, quotes = M.junk_sell_list()
    if count == 0 then
        release_takeover("merchant")
        M.result = {text = "No junk items to sell", ok = false}
        if #skipped > 0 and M.helpers then
            M.helpers.say(M.helpers.skipped_line(skipped), false)
        end
        return false
    end
    local bid, err = eqoa.actions.batch({steps = steps, require_window = "merchant"})
    if not bid then
        release_takeover("merchant")
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = "Sell junk: " .. msg, ok = false}
        return false
    end
    M.batch_id = bid
    M.batch_kind = "sell"
    M.batch_verb = "Sold"
    M.batch_total = count
    M.batch_quotes, M.batch_skipped = quotes, skipped
    M.result = {text = string.format("Selling junk 0 of %d...", count), ok = nil}
    return true
end

function M.start_loot_all()
    local loot = eqoa.loot.get()
    if not loot then
        if M.nearby_corpse() ~= nil then
            return M.open_loot_corpse("loot_all") ~= nil
        end
        M.dirty()
        M.result = {text = "Corpse has no items", ok = false}
        return false
    end
    if #(loot.items or {}) == 0 then
        release_takeover("loot")
        M.dirty()
        M.result = {text = "Corpse has no items", ok = false}
        M.auto_done_line("done: no items on it (coin only)")
        return false
    end
    M.pending_label = "Take all"
    M.auto_take_n = #loot.items
    local req, err = eqoa.actions.loot_all()
    if not req then
        release_takeover("loot")
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = "Take all: " .. msg, ok = false}
        local passing = M.not_final(err)
        M.auto_done_line("Take all refused: " .. msg, passing and msg or nil, passing and M.bags_block_rest(false))
        M.last_req = nil
        return false
    else
        M.last_req = req
        M.held_for_req = "loot"
        M.result = {text = "Take all: queued...", ok = nil}
        return true
    end
end

function M.start_loot_non_junk()
    local loot = eqoa.loot.get()
    if not loot then
        if M.nearby_corpse() ~= nil then
            return M.open_loot_corpse("loot_non_junk") ~= nil
        end
        M.dirty()
        M.result = {text = "Corpse has no items", ok = false}
        return false
    end
    local steps, total = M.loot_non_junk_list()
    if total == 0 then
        release_takeover("loot")
        M.dirty()
        M.result = {text = "Corpse has no items", ok = false}
        M.auto_done_line("done: no items on it (coin only)")
        return false
    end
    if #steps == 0 then
        release_takeover("loot")
        M.result = {text = "Corpse has only junk items", ok = false}
        M.auto_done_line("done: only junk on it, left there")
        return false
    end
    -- Auto-loot with no junk on the corpse: one Take all (one round trip) instead of one take and one reopen per item
    if M.auto_take and #steps == total then return M.start_loot_all() end
    local bid, err = eqoa.actions.batch({steps = steps, require_window = "loot"})
    if not bid then
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = "Take all but junk: " .. msg, ok = false}
        local passing = M.not_final(err)
        M.auto_done_line("Take all but junk refused: " .. msg, passing and msg or nil, passing and M.bags_block_rest(true))
        return false
    end
    M.batch_id = bid
    M.batch_kind = "loot"
    M.batch_verb = "Looted"
    M.batch_total = #steps
    M.corpse_tab_kept = (M.tab == 7)
    M.result = {text = string.format("Looting %d of %d...", 0, #steps), ok = nil}
    return true
end

function M.stop_batch()
    M.note_player_closed() -- Stop is the player's: a corpse they stopped is not asked for again
    if M.pending_open then
        local req = M.pending_open.req
        local kind = M.pending_open.kind
        M.pending_open = nil
        if req then pcall(eqoa.actions.cancel, req) end
        release_takeover(kind)
        M.result = {text = "Stopped", ok = false}
        M.dirty()
    end
    if not M.batch_id then return end
    local stopped = eqoa.actions.stop_batch(M.batch_id)
    if stopped then
        M.result = {text = "Stopping batch...", ok = nil}
    end
end

function M.on_batch_progress(d)
    if not d or not M.batch_id or d.batch_id ~= M.batch_id then return end
    if M.batch_kind == "multi" and M.multi and M.multi.on_progress(d) then return end
    local verb = ({loot = "Looting", restock = "Restocking", bank = "Banking"})[M.batch_kind] or "Selling junk"
    M.result = {
        text = string.format("%s %d of %d...", verb, d.completed or 0, d.total or M.batch_total or 0),
        ok = nil,
    }
end

function M.on_batch_result(d)
    if not d or not M.batch_id or d.batch_id ~= M.batch_id then return end
    -- a multi-select Sell / Put in bank / Take out of bank: the group's own run counts it and goes on with the rest
    if M.batch_kind == "multi" and M.multi then M.multi.on_batch_result(d); return end
    local kind = M.batch_kind
    local verb = M.batch_verb or (kind == "loot" and "Looted" or "Sold")
    local done, total = d.completed or 0, d.total or M.batch_total or 0
    if d.ok == true then
        M.result = {text = string.format("%s %d of %d", verb, done, total), ok = true}
        if kind == "loot" then M.auto_done_line(string.format("done: took %d of %d items (junk left)", done, total)) end
    else
        local reason = d.reason or d.code or d.state or "stopped"
        M.result = {text = string.format("%s %d of %d: %s", verb, done, total, reason), ok = false}
        if kind == "loot" then
            M.auto_done_line(string.format("stopped after %d of %d items: %s", done, total, reason),
                d.code ~= "not_found" and d.code ~= "permission_denied" and tostring(reason) or nil,
                tostring(reason) == "inventory is full" or (d.code ~= "permission_denied" and M.bags_block_rest(true)))
        end
    end
    if kind == "loot" and M.tab == 7 then
        M.corpse_tab_kept = true
    end
    M.batch_id = nil
    -- a helper's batch (Sell junk, Restock, Bank marked) tells the player what it did
    if M.on_helper_batch_done and (kind == "sell" or kind == "restock" or kind == "bank") then
        M.on_helper_batch_done(kind, d)
    end
    local session = kind == "loot" and "loot" or (kind == "bank" and "bank" or "merchant")
    if not (session == "bank" and M.tab == 6) then release_takeover(session) end
end

-- Build the click menu for one stack (Destroy only asks: the client puts its own "are you sure?" to the player, starting
-- on No, and the Yes is theirs; refused entries carry the game's reason).
function M.build_menu(it, fx, fy)
    local rows = {}
    local function add(label, enabled, reason, run, local_only, far_kind, confirm)
        rows[#rows + 1] = {label = label, enabled = enabled, reason = reason, run = run, local_only = local_only,
            far_kind = far_kind, confirm = confirm}
    end
    local loot = eqoa.loot.get()
    local bank = eqoa.bank.get()
    local bank_open = bank and bank.open
    local merch = eqoa.merchant.get()
    local merch_open = merch ~= nil

    if it.list == "loot" then
        local take_qty = it.quantity or 1
        add(take_qty > 1 and ("Take stack (" .. take_qty .. ")") or "Take", loot ~= nil,
            loot == nil and "corpse window is not open" or nil,
            function() return eqoa.actions.loot(it.ref, take_qty) end)
        if take_qty > 1 then
            add("Take 1", loot ~= nil,
                loot == nil and "corpse window is not open" or nil,
                function() return eqoa.actions.loot(it.ref, 1) end)
        end
        add("Take all", loot ~= nil,
            loot == nil and "corpse window is not open" or nil,
            function() return eqoa.actions.loot_all() end)
    elseif it.list == "bank" then
        local near_bank = M.nearby_npc("bank") ~= nil
        local bank_far = not bank_open and near_bank and M.too_far("bank") or nil
        local can_bank = bank_open or (near_bank and not bank_far)
        local no_bank = bank_far or "bank window is not open"
        local function bank_move_or_open(qty)
            if bank and bank.open then
                return eqoa.actions.bank_move("bank", it.ref, qty)
            end
            return M.open_talk_npc("bank", "Withdraw", function()
                M.run_single("Withdraw", function() return eqoa.actions.bank_move("bank", it.ref, qty) end)
            end)
        end
        if it.quantity > 1 then
            add("Withdraw 1", can_bank, not can_bank and no_bank or nil,
                function() return bank_move_or_open(1) end, nil, bank_far and "bank" or nil)
            add("Withdraw stack (" .. it.quantity .. ")", can_bank, not can_bank and no_bank or nil,
                function() return bank_move_or_open(it.quantity) end, nil, bank_far and "bank" or nil)
        else
            add("Withdraw", can_bank, not can_bank and no_bank or nil,
                function() return bank_move_or_open(1) end, nil, bank_far and "bank" or nil)
        end
    else
        local can = eqoa.items.can_use(it.ref)
        local deny = cannot_reason(it, can)
        local block_msg = merch_open and "close merchant first"
            or (bank_open and "close bank first"
            or (loot ~= nil and "close corpse first" or nil))
        local wearable = it.wear_slot ~= nil and it.wear_slot ~= 4294967295
        local usable = has_tab(it, "usable") or has_tab(it, "charged")

        if usable then
            add("Use", deny == nil and block_msg == nil, deny or block_msg,
                function() return eqoa.actions.use_item(it.ref) end)
        elseif not wearable then
            add("Use", false, "item is not usable", nil)
        end

        if wearable then
            local elabel = it.equipped and "Unequip" or "Equip"
            add(elabel, deny == nil and block_msg == nil, deny or block_msg,
                function() return eqoa.actions.equip(it.ref) end)
        elseif not usable then
            add("Equip", false, "item is not equippable", nil)
        end

        if usable then
            for s = 1, 4 do
                add("Quick slot " .. s, block_msg == nil, block_msg,
                    function() return eqoa.actions.quick_slot(it.ref, s) end)
            end
        end

        -- Sell 1 / Sell stack: through the merchant window when one is open (the game's, or the mod's hidden
        -- one); otherwise by talking to the nearest merchant first, as Sell junk does; greyed with the reason
        -- when no merchant is near.
        local near_merch = M.nearby_npc("merchant") ~= nil
        local sell_far = not merch_open and near_merch and M.too_far("merchant") or nil
        local sell_deny = (it.equipped and "cannot sell an equipped item")
            or ((not merch_open and not near_merch) and "no merchant nearby")
            or sell_far or nil
        local sell_far_kind = (not it.equipped and sell_far) and "merchant" or nil
        local function sell_or_talk(label, qty)
            if eqoa.merchant.get() then
                return eqoa.actions.sell(it.ref, qty)
            end
            return M.open_talk_npc("merchant", label, function()
                M.run_single(label, function() return eqoa.actions.sell(it.ref, qty) end)
            end)
        end
        -- an item the game's own merchant window would ask about is asked about here too (`ask_to_sell`)
        local risk = M.sell_risk(it)
        local risk_why = risk and M.risk_text(risk) or nil
        local p1 = merch_open and eqoa.merchant.price(it.ref, "sell", 1) or nil
        add("Sell 1" .. (p1 and (" (" .. M.tunar(p1) .. "t)") or ""), sell_deny == nil, sell_deny,
            function() return sell_or_talk("Sell 1", 1) end, nil, sell_far_kind, risk_why)
        if (it.quantity or 1) > 1 then
            local ps = merch_open and eqoa.merchant.price(it.ref, "sell", it.quantity) or nil
            add("Sell stack (" .. it.quantity .. (ps and (", " .. M.tunar(ps) .. "t)") or ")"), sell_deny == nil,
                sell_deny, function() return sell_or_talk("Sell stack", it.quantity) end, nil, sell_far_kind, risk_why)
        end

        local near_bank = M.nearby_npc("bank") ~= nil
        if bank_open or near_bank then
            local dep_far = not bank_open and M.too_far("bank") or nil
            local dep_deny = it.equipped and "cannot deposit an equipped item" or dep_far or nil
            local dep_far_kind = (not it.equipped and dep_far) and "bank" or nil
            local function dep_move_or_open(qty)
                if bank_open then
                    return eqoa.actions.bank_move("inventory", it.ref, qty)
                end
                return M.open_talk_npc("bank", "Deposit", function()
                    M.run_single("Deposit", function() return eqoa.actions.bank_move("inventory", it.ref, qty) end)
                end)
            end
            if (it.quantity or 1) > 1 then
                add("Deposit 1", dep_deny == nil, dep_deny,
                    function() return dep_move_or_open(1) end, nil, dep_far_kind)
                add("Deposit stack (" .. it.quantity .. ")", dep_deny == nil, dep_deny,
                    function() return dep_move_or_open(it.quantity) end, nil, dep_far_kind)
            else
                add("Deposit", dep_deny == nil, dep_deny,
                    function() return dep_move_or_open(1) end, nil, dep_far_kind)
            end
        end
    end
    if it.list == "inventory" then
        add("Link in chat", true, nil, function() return eqoa.actions.link_item(it.ref) end)
        if it.equipped then
            add("Hide look", true, nil, function() return eqoa.actions.hide_look(it.ref, true) end)
            add("Show look", true, nil, function() return eqoa.actions.hide_look(it.ref, false) end)
        end
        -- local_only: it runs at once, and what it runs is only a question (`run_asking`)
        add("Destroy...", true, nil, function()
            M.run_asking("Destroy", function() return eqoa.actions.destroy(it.ref) end)
        end, true)
    end
    if M.junk_ids[tostring(it.id)] then
        add("Unmark", true, nil, function() M.set_junk(it, false) end, true)
    else
        add("Mark as junk", not it.equipped,
            it.equipped and "cannot mark an equipped item as junk" or nil,
            function() M.set_junk(it, true) end, true)
        -- not an ordinary Misc item, quick-slotted or no-trade: asked once (`ask_to_mark`)
        rows[#rows].mark_ask = M.sell_risk(it) ~= nil or nil
    end
    -- the helpers' marks ("keep N" for Restock, "bank this" for Bank marked) are made on the pack's items
    local H = M.helpers
    if H and it.list == "inventory" then H.menu_rows(it, add) end
    M.menu = {title = it.name, ref = it.ref, rows = rows, fx = fx or 8, fy = fy or 46}
    return M.menu
end

-- Destroy: the call only puts the client's own question to the player (nothing is destroyed until their Yes). Its
-- result says the question is up; the answer arrives as `confirm.closed` (`on_confirm_closed`).
function M.run_asking(label, fn)
    local req, err = fn()
    if not req then
        M.result = {text = label .. ": " .. (err and (err.message or err.code) or "refused"), ok = false}
        M.ask_req = nil
    else
        M.ask_req = req
        M.result = {text = label .. ": answer the question on the screen", ok = nil}
    end
    M.dirty()
    return req, err
end

function M.on_confirm_closed(d)
    if d and d.kind == "destroy_many" and M.multi then M.multi.on_destroy_closed(d); return end
    if not d or d.kind ~= "destroy" then return end
    if d.answer == "yes" then
        local sent = d.detail and d.detail.sent
        M.result = {text = sent and "Destroy: sent" or ("Destroy: not done (" .. tostring(d.detail and d.detail.reason) .. ")"),
            ok = sent == true}
    elseif d.answer == "no" then
        M.result = {text = "Destroy: kept", ok = true}
    else
        M.result = {text = "Destroy: kept (" .. tostring(d.answer) .. ")", ok = true}
    end
    M.dirty()
end

function M.run_single(label, fn)
    M.pending_label = label
    local req, err = fn()
    if not req then
        local msg = err and (err.message or err.code) or "refused"
        M.result = {text = label .. ": " .. msg, ok = false}
        M.last_req = nil
    else
        M.last_req = req
        if not M.pending_open then
            M.result = {text = label .. ": queued...", ok = nil}
        end
    end
    M.dirty()
    return req, err
end

-- Marking an item as junk when it is not an ordinary Misc one (equipment, a potion or other usable, a quick-slotted or
-- no-trade item): the menu asks once, short. Yes marks it, No leaves it unmarked (Escape or a click elsewhere is No too).
function M.ask_to_mark(row)
    local menu = M.menu
    local yes = {label = row.label, text = "Yes, mark it as junk", enabled = true, local_only = true, run = row.run,
        confirmed = true}
    local no = {label = "No", text = "No, leave it", enabled = true, local_only = true, run = function() end}
    M.menu = {title = "Mark " .. (menu and menu.title or "this item") .. " as junk?", ref = menu and menu.ref,
        rows = {yes, no}, fx = menu and menu.fx or 8, fy = menu and menu.fy or 46, asking = true}
end

-- Selling an item the game's merchant window would ask about: the menu asks first (Yes sells it, No keeps it). The
-- question names the item and why; only the player's click on Yes goes on.
function M.ask_to_sell(row)
    local menu = M.menu
    local yes = {label = row.label, text = "Yes, sell it: it is " .. row.confirm, enabled = true, run = row.run,
        confirmed = true}
    local no = {label = "No", text = "No, keep it", enabled = true, local_only = true, run = function() end}
    M.menu = {title = "Sell " .. (menu and menu.title or "this item") .. "?", ref = menu and menu.ref,
        rows = {yes, no}, fx = menu and menu.fx or 8, fy = menu and menu.fy or 46, asking = true}
end

function M.trigger_menu(row)
    if not row then return end
    if row.group and M.multi then M.multi.trigger(row); return end -- a row of the multi-select menu
    if row.mark_ask and not row.confirmed and row.enabled and row.run then
        M.ask_to_mark(row)
        return
    end
    if row.confirm and not row.confirmed and row.run and (row.enabled or row.far_kind) then
        -- the greyed-for-distance gate speaks first (its second click is the player's "try anyway")
        if not row.enabled and row.far_kind and M.far_gate(row.far_kind, row.label) then return end
        M.ask_to_sell(row)
        return
    end
    if row.local_only and row.enabled and row.run then
        row.run()
        return
    end
    if not row.enabled and row.far_kind and row.run then
        -- greyed for being too far: the second click tries anyway
        if M.far_gate(row.far_kind, row.label) then return end
        M.run_single(row.label, row.run)
        return
    end
    if not row.enabled or not row.run then
        M.result = {text = row.label .. ": " .. (row.reason or "unavailable"), ok = false}
        return
    end
    M.run_single(row.label, row.run)
end

local function finish_pending_open(expected_kind)
    local p = M.pending_open
    if not p or p.kind ~= expected_kind then return false end
    M.pending_open = nil
    if p.dist then M.note_range(p.kind, p.dist, true) end
    if p.kind == "loot" then
        if p.auto then
            M.auto_take = p.auto
            M.auto_take_at = os.clock()
            if M.auto_retry then M.auto_retry[p.auto] = nil end
            M.count_more_attempt(p.auto) -- the corpse really opened: that is one of the extra tries
            if p.auto_d2 then M.note_loot_reach(math.sqrt(p.auto_d2), true) end
        end
        if p.after == "loot_all" then
            M.start_loot_all()
        elseif p.after == "loot_non_junk" then
            M.start_loot_non_junk()
        else
            M.result = {text = "Corpse ready", ok = true}
            M.dirty()
        end
    elseif p.fn then
        p.fn()
        if M.last_req ~= nil then
            M.held_for_req = p.kind
        elseif not M.batch_id and not (p.kind == "bank" and M.tab == 6) then
            release_takeover(p.kind)
        end
    end
    return true
end

function M.on_loot_changed(d)
    if d and d.before and not d.after then
        if not keeping_corpse_tab() and M.tab == 7 then
            M.tab = 1
        end
    elseif eqoa.loot.get() ~= nil then
        finish_pending_open("loot")
    end
    M.dirty()
end

function M.on_merchant_changed(d)
    if eqoa.merchant.get() ~= nil then
        finish_pending_open("merchant")
    end
    M.dirty()
end

function M.on_bank_changed(d)
    local b = eqoa.bank.get()
    if b and b.open then
        finish_pending_open("bank")
    end
    M.dirty()
end

-- Auto-loot's "my kill" signal. The wire does not say who killed a creature or who may loot it, so the best the
-- client can know is that YOU hit it: an outgoing damage hit (combat.hit, you as the attacker) or your auto-attack
-- on it while it was your target. A creature nobody on your side touched, another player's kill, a player's corpse
-- and a pet's are never auto-looted. What stays uncertain: a creature you hit that another player then killed
-- still counts (the server decides who may loot and its refusal is shown); manual Loot works on any corpse.
M.ENGAGED_SECS = 120.0

function M.mark_engaged(id)
    if not id then return end
    M.engaged = M.engaged or {}
    M.engaged[id] = os.clock() + M.ENGAGED_SECS
end

function M.engaged_recently(id)
    local until_ = M.engaged and M.engaged[id]
    return until_ ~= nil and os.clock() <= until_
end

function M.on_combat_hit(d)
    if d and d.outgoing and d.kind == "damage" and not d.from_pet and d.defender_id then
        M.mark_engaged(d.defender_id)
    end
end

-- The player is in combat: the game's own combat flag (`fighting`, the one that asks for battle music), auto-attack
-- on a live hostile target, or a live hostile that has the player targeted. Auto-loot must not run then when the client
-- opens corpses the old way (`M.loot_method == "retarget"`): that retargets, which ends the auto-attack. Opened by id it
-- does not, and Auto-loot ignores combat. The game leaves auto-attack on after its target dies (owner log
-- 2026-11-19: corpses waited "in combat (auto-attack is on)" long after the kill), so auto-attack counts only while the
-- target is a live creature that can be fought. Returns true and why (for the log). `pl` is the player when the
-- caller has it already.
function M.in_combat(pl)
    pl = pl or (eqoa.player and eqoa.player.get and eqoa.player.get())
    if not pl then return false end
    if pl.fighting == true then return true, "the game says you are fighting" end
    if pl.auto_attacking == true and pl.target_id then
        local t = eqoa.entities and eqoa.entities.find and eqoa.entities.find(pl.target_id)
        if t and not t.corpse and t.kind == "npc" and t.disposition_name ~= "Friendly"
            and not M.not_a_fight(t, pl.position, pl)
        then
            return true, "auto-attack is on against " .. ((t.name and t.name ~= "") and t.name or "your target")
        end
    end
    local h = pl.id ~= nil and scan_zone().hostile
    if h then return true, h .. " has you targeted" end
    return false
end

-- Auto-loot's decisions go to the game's log (eqoa.log, through eqoa.log.write), one line per corpse and reason, so
-- a play session explains itself: queued, waiting (in combat, walk closer), skipped (not your kill, already looted),
-- the server's answers and the retries, done. `auto_log` keeps the last lines for the tests.
M.auto_log, M.auto_said = {}, {}
function M.alog(id, key, text)
    local k = tostring(id) .. ":" .. key
    if M.auto_said[k] then return end
    M.auto_said[k] = true
    local line = id and string.format("auto-loot: corpse 0x%x: %s", id, text) or ("auto-loot: " .. text)
    M.auto_log[#M.auto_log + 1] = line
    if #M.auto_log > 80 then table.remove(M.auto_log, 1) end
    if eqoa.log and eqoa.log.write then pcall(eqoa.log.write, "info", line) end
end
-- a reason said for a corpse may be said again (its state moved on: a new try, a fight over)
function M.alog_forget(id, key)
    M.auto_said[tostring(id) .. ":" .. key] = nil
end

local function corpse_name(e)
    return (e and e.name and e.name ~= "") and e.name or "a corpse"
end

local function make_eligible(e)
    M.eligible_corpses = M.eligible_corpses or {}
    if not M.eligible_corpses[e.id] then
        M.alog(e.id, "queued", corpse_name(e) .. ", your kill: queued for Auto-loot (" .. M.auto_mode .. ")")
    end
    M.eligible_corpses[e.id] = e
end

-- How near a corpse must be. The server answers a loot request from too far with "Out of range." (owner log
-- 2026-11-19: it opened corpses at 3 and 6 units and refused them at 11, 12, 13 and 15). The mod learns the reach from
-- real answers: the farthest distance that opened (`ok`) and the nearest that was refused (`fail`); until it knows, it
-- tries within LOOT_DEFAULT_REACH. A refusal is a firm answer (no second miss needed, unlike a merchant's silence).
M.LOOT_DEFAULT_REACH = 7.0
M.loot_reach = {}

function M.note_loot_reach(dist, opened)
    if type(dist) ~= "number" then return end
    local r = M.loot_reach
    if opened then
        if not r.ok or dist > r.ok then r.ok = dist end
        if r.fail and r.fail <= dist then r.fail = nil end -- the reach changed: the old refusal no longer holds
    else
        if not r.fail or dist < r.fail then r.fail = dist end
        if r.ok and r.ok >= dist then r.ok = nil end
    end
end

-- The distance Auto-loot opens corpses within, as far as the mod knows.
function M.loot_reach_now()
    local r = M.loot_reach
    if r.fail then return math.max(r.ok or 0, r.fail - 0.5) end
    return math.max(r.ok or 0, M.LOOT_DEFAULT_REACH)
end

-- The answer to Auto-loot's open of corpse `id` (asked from `d2`, squared distance) was not the corpse (`code`, `why`).
--  * "Out of range." (or 0x0085's "too far"): the reach is learned and the corpse waits until the player is closer;
--    never a try, never a give-up.
--  * The game could not send it ("not_ready": a window of its own is up, the player is casting or dead; "busy": the
--    game turned the list away): waits AUTO_RETRY_SECS, not a try.
--  * Anything else (no answer in 10 s, another refusal) is a try: asked again 2 units closer or AUTO_RETRY_SECS later,
--    AUTO_TRIES in all.
function M.auto_open_failed(id, d2, why, code)
    M.auto_retry = M.auto_retry or {}
    if M.auto_looted then M.auto_looted[id] = nil end
    M.alog_forget(id, "far")
    M.alog_forget(id, "combat")
    local r = M.auto_retry[id] or {n = 0}
    local dist = d2 and math.sqrt(d2)
    local text = tostring(why or "")
    if code == "out_of_range" or text:find("Out of range", 1, true) or text:lower():find("too far", 1, true) then
        M.note_loot_reach(dist, false)
        r.wait_closer = dist
        M.auto_retry[id] = r
        M.alog_forget(id, "closer")
        M.alog(id, "oor" .. tostring(dist and math.floor(dist + 0.5)), string.format(
            "the server said \"%s\" at %.0f units: waiting until you are closer (server reach ~%.0f units)",
            text, dist or 0, M.loot_reach_now()))
        return
    end
    if code == "not_ready" or code == "busy" then
        r.d2, r.at, r.soon = d2, os.clock(), true
        M.auto_retry[id] = r
        M.alog(id, "notready" .. r.n, string.format(
            "the game could not open it now (%s): trying again in %.1f s (not counted as a try)", text, M.AUTO_NOTREADY_SECS))
        return
    end
    r.soon = nil
    r.n, r.d2, r.at = r.n + 1, d2, os.clock()
    M.auto_retry[id] = r
    if r.n >= M.AUTO_TRIES then
        if M.auto_looted then M.auto_looted[id] = true end
        M.alog(id, "gaveup", string.format("gave up after %d tries (%s); use the Loot button", r.n, text))
        return
    end
    M.alog(id, "retry" .. r.n, string.format(
        "the server did not open it (%s): will retry when you are closer or in %d s (try %d of %d)",
        text, M.AUTO_RETRY_SECS, r.n + 1, M.AUTO_TRIES))
end

-- Who closed the corpse window last (`loot.changed`'s `closed_by`: "player" for a click on the game's window, "mod",
-- "server", "other"), and the player's own closes of the mod's Loot tab and Stop button. A corpse the player closed
-- or stopped is theirs: Auto-loot does not ask for it again.
M.PLAYER_CLOSE_SECS = 5.0
function M.note_loot_closed(d)
    if not (d and d.before and not d.after) then return end
    local id = d.before.source and d.before.source.entity_id
    if d.closed_by ~= "player" and M.player_closed_recently(id) then return end -- keep the player's mark
    M.last_close = {by = d.closed_by or "other", at = os.clock(), id = id}
end

-- The player closed or stopped the corpse in hand: the one being taken, else the one the window shows (nil when neither
-- is known: then it counts for any corpse).
function M.note_player_closed()
    local l = eqoa.loot and eqoa.loot.get and eqoa.loot.get()
    M.last_close = {by = "player", at = os.clock(), id = M.auto_take or (l and l.source and l.source.entity_id) or nil}
end

-- Did the player close or stop corpse `id` (nil: any) in the last PLAYER_CLOSE_SECS? `since` (a clock time): only a
-- close made after that counts (a take is judged by what happened during it, not by an older mark)
function M.player_closed_recently(id, since)
    local c = M.last_close
    if not (c ~= nil and c.by == "player" and os.clock() - c.at <= M.PLAYER_CLOSE_SECS) then return false end
    if since ~= nil and c.at < since then return false end
    return c.id == nil or id == nil or c.id == id
end

-- An unfinished corpse is asked again AUTO_MORE_TRIES times at most; only an attempt that really reached the corpse's list
-- counts (an open the game could not send, the server's refusal and a silence are not a try of this kind: they are the open's
-- own retries, `auto_open_failed`).
function M.count_more_attempt(id)
    local m = M.auto_more and M.auto_more[id]
    if m and m.left > 0 then m.left = m.left - 1 end
    -- the corpse really opened: it no longer waits for bag space (a take that ends full again waits again)
    if M.auto_bags then M.auto_bags[id] = nil end
end

-- Free stack slots in the pack: `remaining` of the inventory container (`eqoa.inventory.get()`, capacity minus used), nil
-- when the client does not say.
local function bag_free()
    local inv = eqoa.inventory and eqoa.inventory.get and eqoa.inventory.get()
    if not inv then return nil end
    if inv.remaining ~= nil then return inv.remaining end
    if inv.capacity ~= nil and inv.used ~= nil then return math.max(inv.capacity - inv.used, 0) end
    return nil
end
M.bag_free = bag_free

-- A take that ended without getting everything it should: asked again (see AUTO_MORE_TRIES), unless the player closed
-- or stopped it (theirs), or it has been asked enough times. `bags_full`: the rest did not fit the bags: not on the timer
-- (and not counted): it goes back when there is more free space than there was when it failed (see `tick_auto_loot`).
function M.auto_unfinished(id, text, why, bags_full)
    if bags_full then
        M.auto_bags = M.auto_bags or {}
        M.auto_bags[id] = {free = bag_free() or 0}
        if M.auto_looted then M.auto_looted[id] = nil end
        M.alog_forget(id, "closer")
        M.alog_forget(id, "bagswait")
        M.alog(id, "bagsfull", string.format(
            "%s; unfinished (%s): will go back for the rest when you have free bag space, if you are still within reach",
            text, tostring(why)))
        return
    end
    M.auto_more = M.auto_more or {}
    local m = M.auto_more[id] or {left = M.AUTO_MORE_TRIES}
    m.at = os.clock()
    M.auto_more[id] = m
    if m.left <= 0 then
        M.auto_done = M.auto_done or {}
        M.auto_done[id] = true
        M.alog(id, "moregaveup", string.format("%s; still unfinished after %d more tries (%s): left alone, use the Loot button",
            text, M.AUTO_MORE_TRIES, tostring(why)))
        return
    end
    if M.auto_looted then M.auto_looted[id] = nil end
    M.alog_forget(id, "closer")
    M.alog(id, "unfinished" .. m.left, string.format(
        "%s; unfinished (%s): will try again when you are within reach (%d more %s)",
        text, tostring(why), m.left, m.left == 1 and "try" or "tries"))
end

-- A refusal that may pass (the game was busy or not ready) is a take that did not finish; the others are final.
function M.not_final(err)
    return err ~= nil and (err.code == "busy" or err.code == "not_ready")
end

-- What Auto-loot took from its corpse (or why nothing), said once the take is over. `unfinished` (a reason) marks a take
-- that left something it should have taken.
function M.auto_done_line(text, unfinished, bags_full)
    local id = M.auto_take
    if not id then return end
    M.auto_take = nil
    -- the game's own window was the player's for this corpse only until the take ended
    if M.auto_player_open == id then M.auto_player_open = nil end
    if unfinished and not M.player_closed_recently(id, M.auto_take_at) then
        M.auto_unfinished(id, text, unfinished, bags_full)
        return
    end
    if unfinished then
        -- the player's close: theirs, not asked for again, but its items were NOT taken: never "already looted"
        M.auto_left = M.auto_left or {}
        M.auto_left[id] = true
        M.alog(id, "done", text .. " (you closed it: left alone)")
        return
    end
    M.auto_done = M.auto_done or {}
    M.auto_done[id] = true
    M.alog(id, "done", text)
end

-- The corpse's items still listed after a Take all: the bags took what fit.
local function corpse_items_left()
    local l = eqoa.loot and eqoa.loot.get and eqoa.loot.get()
    return l and #(l.items or {}) or 0
end

-- The pack has no free slot and the corpse still lists items: whatever the take said, nothing more fits. `non_junk`:
-- only the items not marked as junk count (a batch leaves the junk on purpose).
function M.bags_block_rest(non_junk)
    if bag_free() ~= 0 then return false end
    local l = eqoa.loot and eqoa.loot.get and eqoa.loot.get()
    for _, it in ipairs(l and l.items or {}) do
        if not (non_junk and M.is_junk(it)) then return true end
    end
    return false
end

-- Looting stays hidden: the game's own loot window must never show during an Auto-loot. It never has in the owner's
-- play; this is a guard, not a fix: if the game's window (native mode 1) is up while Auto-loot is opening or taking
-- from a corpse, one line says so, once per corpse.
function M.check_window_shown()
    local id = M.auto_take or (M.pending_open and M.pending_open.auto)
    if not id or id == M.auto_player_open then return end -- a corpse the player opened is theirs to see
    local w = eqoa.windows and eqoa.windows.get and eqoa.windows.get()
    if w and w.native_mode == 1 then
        M.alog(id, "shown", "the game's loot window was SHOWN during Auto-loot (it should stay hidden); the mod took over too late or not at all")
    end
end

-- A corpse seen by the poll: a creature you hit (or one that vanished where a corpse now lies) is queued; any other
-- creature's corpse within sight is said once to be left alone.
local function consider_corpse(e, new)
    if e.kind == "player" or e.kind == "pet" then return end
    local yours = M.engaged_recently(e.id)
    if not yours and M.recently_dead and os.clock() <= M.recently_dead.expires then
        local d2 = dist_sq(M.recently_dead.pos, e.actor_position or e.position)
        yours = d2 ~= nil and d2 <= 36.0
    end
    if yours then
        if M.auto_left and M.auto_left[e.id] then
            M.alog(e.id, "left", "skipped, left alone (you closed it before its items were taken)")
        elseif M.auto_done and M.auto_done[e.id] then
            M.alog(e.id, "already", "skipped, already looted")
        else
            make_eligible(e)
        end
    elseif new then
        local pl = M.poll_pl
        local d2 = dist_sq(pl and pl.position, e.actor_position or e.position)
        if not (d2 and d2 > M.NPC_SHOWN_WITHIN * M.NPC_SHOWN_WITHIN) then
            M.alog(e.id, "notyours", corpse_name(e) .. ": skipped, not your kill (you did not hit it)")
        end
    end
end

-- Auto-loot looks at the world AUTO_POLL_SECS apart, from the render event, instead of listening to every creature
-- update: in a crowd those were 50-70 deliveries a frame, each asking for the player again (owner log 2026-11-19:
-- 2.2-4.3 ms a frame with Auto-loot on, 0.1 ms off). A poll reads the player once, asks after the few creatures you
-- hit (`eqoa.entities.find`), looks for new corpses in the shared look at the zone (at most every SCAN_TTL), then
-- decides. Your hits come from `combat.hit`, the one other event it hears.
M.AUTO_POLL_SECS = 0.25
-- while a creature you hit is alive and followed (a fight), or has just vanished: a death is seen this soon (a lookup of
-- the few engaged ids, no look at the zone)
M.AUTO_POLL_ENGAGED_SECS = 0.1
M.AUTO_IDLE_SCAN_SECS = 2.0
M.poll_count = 0 -- polls made (for the tests)

function M.auto_poll(force)
    if M.auto_mode == "off" then return end
    local now = os.clock()
    local gap = M.AUTO_POLL_SECS
    if M.engaged_alive or M.recently_dead then gap = math.min(gap, M.AUTO_POLL_ENGAGED_SECS) end
    if not force and M.auto_polled and now - M.auto_polled < gap and now >= M.auto_polled then return end
    M.auto_polled = now
    M.poll_count = M.poll_count + 1
    local pl = eqoa.player and eqoa.player.get and eqoa.player.get()
    M.poll_pl = pl
    if not pl then return end
    M.check_window_shown()
    if M.recently_dead and now > M.recently_dead.expires then M.recently_dead = nil end
    if pl.auto_attacking == true and pl.target_id then M.mark_engaged(pl.target_id) end
    local find = eqoa.entities and eqoa.entities.find
    -- the creatures you hit: a corpse now, still alive (where), or gone (its corpse may come as a new entity there)
    M.engaged_pos = M.engaged_pos or {}
    M.engaged_alive = false
    if M.engaged and find then
        for id, until_ in pairs(M.engaged) do
            if until_ < now then
                M.engaged[id] = nil
                M.engaged_pos[id] = nil
            else
                local e = find(id)
                if e and e.corpse then
                    consider_corpse(e, false)
                elseif e then
                    M.engaged_pos[id] = e.actor_position or e.position
                    M.engaged_alive = true
                elseif M.engaged_pos[id] then
                    M.recently_dead = {id = id, pos = M.engaged_pos[id], expires = now + 3.0}
                    M.engaged_pos[id] = nil
                end
            end
        end
    end
    -- a corpse waiting that is gone (decayed, looted by someone): forget it
    if M.eligible_corpses and find then
        for id in pairs(M.eligible_corpses) do
            local e = find(id)
            if not (e and e.corpse) then
                M.eligible_corpses[id] = nil
                if M.auto_retry then M.auto_retry[id] = nil end
                if M.auto_bags then M.auto_bags[id] = nil end
            else
                M.eligible_corpses[id] = e
            end
        end
    end
    -- new corpses near you, from the shared look at the zone (one look at a crowded zone costs ~3.75 ms in the owner's
    -- log): every SCAN_TTL while a creature you hit has just vanished (its corpse may be a new entity) or a corpse waits
    -- (the fight check reads the look too), every AUTO_IDLE_SCAN_SECS otherwise; the creatures you hit are followed by
    -- `entities.find` above, without a look at the zone
    local busy = M.recently_dead ~= nil
    if not busy and M.eligible_corpses then
        for id in pairs(M.eligible_corpses) do
            if not (M.auto_looted and M.auto_looted[id]) then busy = true break end
        end
    end
    if not busy and M.idle_scan_at and now - M.idle_scan_at < M.AUTO_IDLE_SCAN_SECS and now >= M.idle_scan_at then
        M.tick_auto_loot(pl)
        return
    end
    M.idle_scan_at = now
    local data = scan_zone()
    if data.corpses and data ~= M.corpses_seen_in then
        M.corpses_seen_in = data
        local seen, before = {}, M.corpses_seen or {}
        for _, e in ipairs(data.corpses) do
            seen[e.id] = true
            if not before[e.id] then consider_corpse(e, M.corpses_seen ~= nil) end
        end
        M.corpses_seen = seen
    end
    M.tick_auto_loot(pl)
end

-- Auto-loot opens a corpse within the learned reach. A corpse whose open failed is asked again 2 units closer or
-- AUTO_RETRY_SECS later (AUTO_TRIES real failures in all; see `auto_open_failed`).
M.AUTO_TRIES, M.AUTO_CLOSER, M.AUTO_RETRY_SECS = 3, 2.0, 5.0
-- "not ready"/"busy" (casting, a game window up) is asked again this soon, never counted as a try
M.AUTO_NOTREADY_SECS = 0.5
-- A take that did not finish (the window went away, the server was not ready) is asked again, when you are within reach
-- and AUTO_MORE_SECS after it ended, at most AUTO_MORE_TRIES more times for each corpse. A take that left items behind
-- because the bags were full is not on that timer: it waits until you have free bag space (`bag_free`), then goes back
-- if you are still within reach and the corpse is still there, with no limit on how often; walking out of reach drops it.
M.AUTO_MORE_TRIES, M.AUTO_MORE_SECS = 2, 5.0
-- how the client opens a corpse for Auto-loot, learned from the first open's result: nil (not known yet: by id is
-- tried), "keep_target" (your target and auto-attack are left alone) or "retarget" (the old way: it retargets, which
-- ends the auto-attack, so Auto-loot waits for the end of a fight)
M.loot_method = nil

-- The connection ended (`world.leaving` with reason "disconnect"): another server may not take a corpse open that names the
-- corpse, so how the client opens corpses is learned again from the next open's result.
function M.on_disconnect()
    M.loot_method = nil
    M.keep_req = nil
end

function M.note_loot_method(d)
    local m = d and d.detail and d.detail.method or "retarget"
    if m ~= "keep_target" then m = "retarget" end
    if M.loot_method == m then return end
    M.loot_method = m
    M.alog_forget(nil, "method:" .. m)
    if m == "keep_target" then
        M.alog(nil, "method:" .. m, "corpses are opened by their id: your target and auto-attack are left alone, so Auto-loot works in a fight")
    else
        M.alog(nil, "method:" .. m, "corpses are opened the old way (the client targets the corpse and gives your target back): that ends an auto-attack, so Auto-loot waits for the end of a fight")
    end
end

function M.tick_auto_loot(pl)
    if M.auto_mode == "off" then return end
    if not (M.eligible_corpses and next(M.eligible_corpses)) then return end
    local waiting = false
    for id in pairs(M.eligible_corpses) do
        if not (M.auto_looted and M.auto_looted[id]) then waiting = true break end
    end
    if not waiting then return end
    if M.batch_id or M.pending_open or M.last_req or loot_batch_active() then return end
    pl = pl or (eqoa.player and eqoa.player.get and eqoa.player.get())
    -- the old way of opening a corpse retargets, which ends the auto-attack: then it waits for the end of a fight (the
    -- corpse stays eligible). Opened by id (`M.loot_method` nil until the first open says otherwise, or "keep_target")
    -- nothing is retargeted, so a fight is no reason to wait.
    local fighting, why = false, nil
    if M.loot_method == "retarget" then fighting, why = M.in_combat(pl) end
    if fighting then
        for id in pairs(M.eligible_corpses) do
            if not (M.auto_looted and M.auto_looted[id]) then
                M.alog(id, "combat", "waiting: in combat (" .. tostring(why) .. ")")
            end
        end
        return
    end
    -- a corpse already open (the player's attack key on the dead target loots it, or a click, or the Loot button): the
    -- server will not open another while it is up, and the client refuses to ask. If it is the corpse Auto-loot wants,
    -- Auto-loot takes from that session; any other corpse waits until it closes (owner log 2026-11-19: three tries
    -- burned on "cannot open corpse right now" while the player's own attack key had the corpse open).
    local open = eqoa.loot and eqoa.loot.get and eqoa.loot.get()
    if open then
        local src = open.source and open.source.entity_id
        local bw = src and M.auto_bags and M.auto_bags[src]
        if bw and not (bag_free() ~= nil and bag_free() > bw.free) then
            -- the player opened a corpse that waits for bag space: not taken from while the bags are still full
            M.alog(src, "bagswait", "waiting: the bags are still full; going back for the rest once you have free bag space")
        elseif src and M.eligible_corpses[src] and not (M.auto_looted and M.auto_looted[src]) then
            M.auto_looted = M.auto_looted or {}
            M.auto_looted[src] = true
            M.auto_take = src
            M.auto_take_at = os.clock()
            M.auto_player_open = src
            M.count_more_attempt(src)
            local take = M.auto_mode == "all" and "Take all" or "Take all but junk"
            M.alog(src, "takeopen", "it is open already (in the game's window or the mod's): taking from it (" .. take .. ")")
            if M.auto_mode == "all" then M.start_loot_all() else M.start_loot_non_junk() end
        else
            for id in pairs(M.eligible_corpses) do
                if not (M.auto_looted and M.auto_looted[id]) then
                    M.alog(id, "busy:" .. tostring(src), src
                        and string.format("waiting: another corpse (0x%x) is open; looting this one once it closes", src)
                        or "waiting: a corpse window is open; looting this one once it closes")
                end
            end
        end
        return
    end
    local ppos = pl and pl.position
    local now = os.clock()
    local reach = M.loot_reach_now()
    local best, best_d2 = nil, nil
    for id, e in pairs(M.eligible_corpses) do
        if not (M.auto_looted and M.auto_looted[id]) then
            M.alog_forget(id, "combat")
            M.alog_forget(id, "busy:nil")
            local d2 = dist_sq(ppos, e.actor_position or e.position)
            local dist = d2 and math.sqrt(d2)
            local r = M.auto_retry and M.auto_retry[id]
            local ready = true
            if r and r.at then
                local closer = dist ~= nil and r.d2 ~= nil and math.sqrt(r.d2) - dist >= M.AUTO_CLOSER
                local wait = r.soon and M.AUTO_NOTREADY_SECS or M.AUTO_RETRY_SECS
                ready = r.n < M.AUTO_TRIES and (closer or now - r.at >= wait)
            end
            -- a take that did not finish is asked again once it has been a moment and you are within reach
            local more = M.auto_more and M.auto_more[id]
            if more and now - more.at < M.AUTO_MORE_SECS then ready = false end
            local bw = M.auto_bags and M.auto_bags[id]
            if bw and dist ~= nil and dist > reach then
                -- the bags were full and you walked away: not chased
                M.auto_bags[id] = nil
                M.eligible_corpses[id] = nil
                M.auto_done = M.auto_done or {}
                M.auto_done[id] = true
                M.alog(id, "bagsdrop", string.format("you left reach (%.0f units) with items still on it: left alone", dist))
            elseif bw then
                -- waiting for free bag space (strictly more than there was when the take failed)
                local free = bag_free()
                if free ~= nil and free > bw.free then
                    -- room again: asked like any other open (a not-ready or refused ask waits its own time)
                    if ready and (not best or (d2 or 0) < (best_d2 or 0)) then best, best_d2 = e, d2 or 0 end
                else
                    M.alog(id, "bagswait", "waiting: the bags are still full; going back for the rest once you have free bag space")
                end
            elseif dist ~= nil and dist > reach then
                M.alog(id, "closer", string.format("waiting: walk closer (%.0f units; server reach ~%.0f units)", dist,
                    reach))
            elseif ready and (not best or (d2 or 0) < (best_d2 or 0)) then
                best, best_d2 = e, d2 or 0
            end
        end
    end
    if best then
        M.auto_looted = M.auto_looted or {}
        M.auto_looted[best.id] = true
        M.alog_forget(best.id, "closer")
        local after = M.auto_mode == "all" and "loot_all" or "loot_non_junk"
        local r = M.auto_retry and M.auto_retry[best.id]
        local n = r and r.n or 0
        local m = M.auto_more and M.auto_more[best.id]
        local again = m and string.format(", asking again after an unfinished take (%d more %s after this)", m.left - 1,
            m.left - 1 == 1 and "try" or "tries") or ""
        M.alog(best.id, "open" .. n .. ":" .. math.floor(math.sqrt(best_d2) + 0.5) .. ":" .. tostring(m and m.left or ""), string.format(
            "opening it (%.0f units away, %s, try %d of %d%s)", math.sqrt(best_d2),
            after == "loot_all" and "Take all" or "Take all but junk", n + 1, M.AUTO_TRIES, again))
        local req, err = M.open_loot_corpse(after, best.id, true)
        if req and M.pending_open and M.pending_open.req == req then
            M.pending_open.auto, M.pending_open.auto_d2 = best.id, best_d2
        elseif not req then
            M.auto_open_failed(best.id, best_d2, (err and (err.message or err.code)) or "refused", err and err.code)
        end
    end
end

-- A mod's hidden corpse session ends after 15 s without use; showing the Loot tab keeps it alive: asking for the
-- takeover again while holding it renews the timer (walking away, zoning or releasing still end it).
function M.renew_loot_session()
    if eqoa.windows and eqoa.windows.takeover and eqoa.windows.taken and eqoa.windows.taken("loot") and eqoa.loot.get() then
        eqoa.windows.takeover("loot")
    end
end

-- What the result line says when the server gave no answer in time (the game has no range check of its own: the
-- server decides, and a request out of its reach is simply not answered).
function M.no_answer_text(kind)
    if kind == "merchant" then return "the merchant did not answer (too far away?)" end
    if kind == "bank" then return "the banker did not answer (too far from the banker?)" end
    if kind == "loot" then return "the corpse did not open (too far away?)" end
    if kind == "bank_move" or kind == "bank_money" then return "no answer from the server (too far from the banker?)" end
    if kind == "sell" or kind == "buy" then return "no answer from the server (too far from the merchant?)" end
    return "no answer from the server"
end

-- The broker's own timeout (nothing came back in 10 s). Other "unknown" outcomes keep their own reason
-- (for instance "corpse closed" for a take the server never confirmed).
function M.timed_out(d)
    if d.state ~= "unknown" and d.code ~= "unknown" then return false end
    return d.reason == nil or d.reason:find("outcome unknown after", 1, true) ~= nil
end

function M.on_action_result(d)
    if not d then return end
    -- the destroy call's own result: the question is up, or the call was refused (the answer is `confirm.closed`)
    if M.ask_req and d.request_id == M.ask_req and d.ok ~= nil then
        M.ask_req = nil
        if d.ok == false then M.result = {text = "Destroy: " .. (d.reason or d.code or "refused"), ok = false} end
        M.dirty()
        return
    end
    -- the open's own result says how the client opened the corpse; the corpse's list (loot.changed) may have ended the
    -- pending open before it arrives
    if M.keep_req and d.request_id == M.keep_req and d.ok ~= nil and d.phase ~= "dispatch" then
        M.keep_req = nil
        if d.ok == true then M.note_loot_method(d) end
    end
    if M.pending_open and d.request_id == M.pending_open.req then
        local p = M.pending_open
        if d.ok == false then
            M.pending_open = nil
            release_takeover(p.kind)
            local msg = d.reason or d.code or d.state or "refused"
            M.result = {text = (p.verb or d.kind) .. ": " .. msg, ok = false}
            if p.auto then M.auto_open_failed(p.auto, p.auto_d2, msg, d.code) end
            M.dirty()
            return
        elseif d.ok == true and d.phase ~= "dispatch" then
            finish_pending_open(p.kind)
            return
        elseif d.ok == nil and M.timed_out(d) then
            -- the server never answered the open: say so, never hang on "Opening..."
            if p.dist then M.note_range(p.kind, p.dist, false) end
            M.pending_open = nil
            release_takeover(p.kind)
            M.result = {text = (p.verb or d.kind) .. ": " .. M.no_answer_text(p.kind), ok = false}
            if p.auto then M.auto_open_failed(p.auto, p.auto_d2, "no answer from the server in 10 s", "timeout") end
            M.dirty()
            return
        end
    end
    if M.last_req == nil or d.request_id ~= M.last_req then return end
    local label = M.pending_label or d.kind
    if d.state == "dispatched" and d.phase == "dispatch" then
        M.result = {text = label .. ": waiting for server...", ok = nil}
    elseif d.ok == true then
        M.result = {text = label .. ": ok", ok = true}
        local left = M.auto_take and corpse_items_left() or 0
        -- items left with no free stack slot: the bags were full; with free slots left something else kept them
        local full = left > 0 and bag_free() == 0
        M.auto_done_line(string.format("done: %s ok (%d items)", label, M.auto_take_n or 0),
            left > 0 and string.format("%d items still on the corpse (%s)", left,
                full and "the bags are full" or "the bags took what fit") or nil, full)
        M.last_req = nil
        if M.held_for_req then
            local k = M.held_for_req
            M.held_for_req = nil
            release_takeover(k)
        end
    else
        local why = d.reason or d.code or d.state
        if d.ok == nil and M.timed_out(d) then
            why = M.no_answer_text(d.kind)
        end
        M.result = {text = label .. ": " .. why, ok = false}
        -- the pack was already full: the server could move nothing ("did not change corpse state") or refused
        local full = M.auto_take ~= nil and d.code ~= "permission_denied" and M.bags_block_rest(false)
        M.auto_done_line("the take did not finish: " .. tostring(why),
            d.code ~= "permission_denied" and tostring(why) or nil, full)
        M.last_req = nil
        if M.held_for_req then
            local k = M.held_for_req
            M.held_for_req = nil
            release_takeover(k)
        end
    end
end

-- A pickup is what a corpse loses: loot.changed with the native cause for one item (0x0019) or all (0x001A).
-- Stacks are told apart by their wire key, which the corpse keeps until it is closed.
function M.log_loot(data)
    if (data.cause ~= "loot_one" and data.cause ~= "loot_all") or not data.before or not data.after then return end
    local left = {}
    for _, it in ipairs(data.after.items) do left[it.inventory_key] = it.quantity end
    local stamp = os.date("%H:%M")
    -- the corpse the window belongs to, when the client knows which it asked for
    local source = data.before.source
    for _, it in ipairs(data.before.items) do
        local taken = it.quantity - (left[it.inventory_key] or 0)
        if taken > 0 then
            local text = string.format("%s %s x%d", stamp, it.name, taken)
            if source then text = text .. ", from " .. source.name end
            table.insert(M.log, 1, {text = text})
            if #M.log > M.LOG_MAX then table.remove(M.log) end
        end
    end
end

return M
