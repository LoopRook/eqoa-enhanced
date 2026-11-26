-- Bag viewer: Bags, Bank, and Corpse window over the public eqoa items & actions API.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local ui = eqoa.imgui
local B = require("bags")
local HP = require("helpers")
local X = require("multi") -- multi-select (Ctrl+click, Shift+click): one menu for several items
local AL = require("auction_list") -- listing a bank item at the auction (the bank item menu, the group menu)
B.auction_list = AL
AL.sell_action = function(args) return eqoa.actions.auction_sell(args) end

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, PAD, SLOT, GAP, LINE = 318, 8, 36, 2, 12
-- The loot log shows LOG_LINES lines (the wheel scrolls the rest); LOG_LINES_HELPERS while the Restock / Bank marked row is
-- up, so the window still fits the 448-pixel HUD frame with the helper rows (the game's frame is the limit, not the screen).
local LOG_LINES, LOG_LINES_HELPERS = 3, 2
local log_shown = LOG_LINES
local wmemo, wmemo_k, wmemo_n = {}, nil, 0
-- Round A1: the window is a session ("inventory") the client ends for us (zoning, death, "Use the game's windows", the
-- mod being turned off), and it may hold the controller's d-pad, confirm and back while it is up.
local session
local pad = {on = false, idx = 1, msel = 1}
local tokens, escape, tab_hits, slot_hits, menu_hits, action_hits, log_rect, grid_rect = {}, nil, {}, {}, {}, {}, nil, nil

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

-- Bank tunar (the Bank tab, a banker near or the bank open): an amount typed in a box (digits only), Deposit and Withdraw, and an All
-- for each. The amounts are what the client says the pack and the bank hold (`eqoa.inventory.get().money`,
-- `eqoa.bank.get().money`, which follow the game's own money events); a button that cannot work is greyed and a press
-- on it says why. The keyboard is taken only while the box is typed in, as the auction price box takes it.
local BM = {text = "", field = false, enter = nil, rect = nil, was_down = false}
local BM_DIGITS = 9
package.loaded["bank_money_state"] = BM -- for the mod's tests: nothing else reads it
local function bm_stop()
    local was = BM.field
    BM.field = false
    if BM.enter then eqoa.input.release(BM.enter); BM.enter = nil end
    if was then eqoa.input.capture_text(false) end
end
local function bm_start()
    BM.field = true
    B.result = nil
    if not BM.enter then BM.enter = eqoa.input.claim("Enter", true) end
end
-- the amount the box holds, and what the pack and the bank hold
local function bm_numbers()
    local bank, inv = eqoa.bank.get(), eqoa.inventory.get()
    return bank and bank.open == true, (inv and inv.money) or 0, (bank and bank.money) or 0, tonumber(BM.text) or 0
end
-- why `amount` cannot be moved in `dir` now, or nil
local function bm_why(dir, amount)
    local open, carried, banked = bm_numbers()
    if not open then
        -- no window needed: a banker near is enough, as for Put in bank / Take out of bank (the mod opens its own hidden one)
        if B.nearby_npc("bank", B.DRAW_TTL) == nil then return "no banker is near" end
        local far = B.too_far("bank", B.DRAW_TTL)
        if far then return far end
    end
    if dir == "deposit" then
        if carried <= 0 then return "you carry no tunar" end
        if amount <= 0 then return "type an amount" end
        if amount > carried then return "you carry only " .. B.tunar(carried) end
    else
        if banked <= 0 then return "the bank holds no tunar" end
        if amount <= 0 then return "type an amount" end
        if amount > banked then return "the bank holds only " .. B.tunar(banked) end
    end
    return nil
end
local function bm_send(dir, amount)
    bm_stop()
    local why = bm_why(dir, amount)
    local open = bm_numbers()
    local label = (dir == "deposit" and "Deposit " or "Withdraw ") .. B.tunar(amount) .. " tunar"
    if why then
        B.result = {text = label .. ": " .. why, ok = false}
        return
    end
    local function go() B.run_single(label, function() return eqoa.actions.bank_money(dir, amount) end) end
    if open then return go() end
    B.open_talk_npc("bank", label, go) -- the mod's own hidden bank session, released as the other helpers release it
end
function BM.on_key(d)
    if not BM.field or not d.down then return false end
    local key = d.key:lower()
    if key == "escape" or key == "enter" or key == "return" then bm_stop()
    elseif key == "backspace" then BM.text = BM.text:sub(1, -2)
    elseif d.text and #d.text > 0 then
        local t = (d.text:gsub("%D", ""))
        if #BM.text + #t <= BM_DIGITS then BM.text = BM.text .. t end
    end
    return true
end

-- The window stops listening to everything but loot while it is closed.
-- While the game's own full map is up this mod draws nothing and takes no click or key; it comes back as it was.
local function map_up() local m = eqoa.map.get(); return m ~= nil and m.open == true end
local GATED = { render = true, ["input.key"] = true, ["pointer.pressed"] = true, ["pointer.wheel"] = true, ["windows.key"] = true, ["input.pad"] = true }
local function listen(name, fn)
    local inner = fn
    if GATED[name] then
        inner = function(e)
            if map_up() then
                if name == "render" and (AL.enter or (AL.form and AL.form.field)) then AL.stop_typing() end -- a price being typed lets go of the keyboard at once
                if name == "render" and BM.field then bm_stop() end -- so does an amount
                return
            end
            return fn(e)
        end
    end
    local token = eqoa.events.on(name, inner)
    if token then tokens[#tokens + 1] = token end
end

local draw
local function close(from_client)
    if not B.open then return end
    for _, token in ipairs(tokens) do eqoa.events.off(token) end
    tokens = {}
    if session and not from_client then eqoa.windows.close_session(session.ref) end
    session, pad.on = nil, false
    AL.close()
    bm_stop()
    if escape then eqoa.input.release(escape); escape = nil end
    B.drop_pending_npc_open()
    if eqoa.windows and eqoa.windows.release then
        eqoa.windows.release("merchant")
        eqoa.windows.release("bank")
        if not (B.pending_open and B.pending_open.kind == "loot")
            and not (B.batch_id and B.batch_kind == "loot")
        then
            eqoa.windows.release("loot")
        end
    end
    B.clear_corpse_hold()
    B.open, B.hover, B.menu, B.menu_rect, B.grid_scroll = false, nil, nil, nil, 0
    X.clear()
    tab_hits, slot_hits, menu_hits, action_hits, log_rect, grid_rect = {}, {}, {}, {}, nil, nil
    B.tab_hits, B.slot_hits, B.menu_hits, B.action_hits = tab_hits, slot_hits, menu_hits, action_hits
end

local function pad_key(key)
    -- the controller: d-pad moves, confirm presses, back backs out (the menu first, then the window)
    local grid = B.grid
    if B.menu then
        local n = #B.menu.rows
        if key == "up" then pad.msel = (pad.msel - 2) % n + 1
        elseif key == "down" then pad.msel = pad.msel % n + 1
        elseif key == "confirm" then
            local shown = B.menu
            B.trigger_menu(shown.rows[pad.msel])
            if B.menu == shown then B.menu, B.menu_rect = nil, nil end
            pad.msel = 1
        elseif key == "back" or key == "left" then B.menu, B.menu_rect = nil, nil end
        return
    end
    if key == "back" then close(); return end
    if not grid then return end
    local cols, rows = B.COLUMNS, grid.rows
    local col, row = (pad.idx - 1) % cols, math.floor((pad.idx - 1) / cols)
    if key == "left" then col = math.max(0, col - 1)
    elseif key == "right" then col = math.min(cols - 1, col + 1)
    elseif key == "up" then
        if row == 0 then B.scroll_grid(-1) else row = row - 1 end
    elseif key == "down" then
        if row >= rows - 1 then B.scroll_grid(1) else row = row + 1 end
    elseif key == "confirm" then
        local entry = grid.items[B.grid_scroll * cols + pad.idx]
        if entry then
            B.build_menu(entry.item, PAD + col * (SLOT + GAP), 46 + row * (SLOT + GAP))
            pad.msel = 1
        end
        return
    end
    pad.idx = row * cols + col + 1
end

local function open()
    if B.open then return end
    local s, err = eqoa.windows.open_session("inventory", {opened_from = "key"})
    if not s then
        -- the client says why (the player is using the game's windows, dead, out of the world)
        eqoa.log.write("info", "Bag viewer did not open: " .. tostring(err and err.message))
        return
    end
    session = s
    if not B.junk_loaded then B.load_junk() end
    B.open, B.grid, B.menu = true, nil, nil
    pad.idx, pad.msel = 1, 1
    pad.on = eqoa.windows.focus(s.ref) == true
    if B.char_dirty then B.sync_character() end
    B.scan_reset()
    listen("render", draw)
    listen("inventory.changed", function() B.on_pack_changed() end)
    listen("bank.changed", function(e) B.on_bank_changed(e.data) end)
    listen("loot.changed", function(e) B.on_loot_changed(e.data) end)
    listen("merchant.changed", function(e) B.on_merchant_changed(e.data) end)
    listen("state.reset", B.dirty)
    listen("action.result", function(e) if not AL.on_action_result(e.data) then B.on_action_result(e.data) end end)
    listen("batch.progress", function(e) B.on_batch_progress(e.data) end)
    listen("batch.result", function(e) B.on_batch_result(e.data) end)
    -- what the player answered to the destroy question the client put to them
    listen("confirm.closed", function(e) B.on_confirm_closed(e.data) end)
    listen("windows.session", function(e)
        local d = e.data
        AL.on_session(d)
        if session and d.ref == session.ref then
            if d.phase == "closed" then close(true)
            elseif d.phase == "changed" then pad.on = false end -- the system took the focus: the pad is the game's
        end
    end)
    listen("windows.key", function(e)
        local d = e.data
        if session and d.ref == session.ref and d.down then pad_key(d.key) end
    end)
    listen("input.key", function(e)
        if AL.on_key(e.data) or BM.on_key(e.data) then return end
        if e.data.down and not e.data["repeat"] and e.data.key:lower() == "escape" then
            if B.menu then B.menu, B.menu_rect = nil, nil else close() end
        end
    end)
    listen("pointer.pressed", function(e)
        local d = e.data
        if AL.on_pointer(d) then return end
        -- a press anywhere but the amount box (or the other button) ends the typing
        if BM.field and not (d.button == "left" and inside(d.x, d.y, BM.rect)) then bm_stop() end
        if B.menu then
            if d.button == "right" then B.menu, B.menu_rect = nil, nil; return end
            if d.button == "left" then
                for _, h in ipairs(menu_hits) do
                    if inside(d.x, d.y, h) then
                        local shown = B.menu
                        B.trigger_menu(h.row)
                        -- a question ("sell it anyway?") is the menu's next page: it stays up for the answer
                        if B.menu == shown then B.menu, B.menu_rect = nil, nil end
                        return
                    end
                end
                if inside(d.x, d.y, B.menu_rect) then return end
                B.menu, B.menu_rect = nil, nil
            end
        end
        if d.button == "right" then
            -- right-click on a selected item: one menu for every selected item
            for _, h in ipairs(slot_hits) do
                if inside(d.x, d.y, h) then
                    if X.open_menu(h) then pad.msel = 1 end
                    return
                end
            end
            return
        end
        if d.button ~= "left" then return end
        for _, h in ipairs(tab_hits) do if inside(d.x, d.y, h) then B.select(h.tab); return end end
        for _, h in ipairs(slot_hits) do
            if inside(d.x, d.y, h) then
                -- Ctrl+click and Shift+click select; a plain click clears the selection and opens the item's own menu
                if not X.click(h, d) then B.build_menu(h.item, h.fx, h.fy) end
                return
            end
        end
        for _, h in ipairs(action_hits) do
            if inside(d.x, d.y, h) and h.run then h.run(); return end
        end
    end)
    listen("pointer.wheel", function(e)
        local d, steps = e.data, e.data.steps or 0
        if steps ~= 0 then
            local delta = steps > 0 and -1 or 1
            if inside(d.x, d.y, log_rect) then
                B.scroll = math.max(0, math.min(math.max(0, #B.log - log_shown), B.scroll + delta))
            elseif inside(d.x, d.y, grid_rect) then
                B.scroll_grid(delta)
            end
        end
    end)
    -- Escape is the window's, not the game menu's, while it is up (a claim the menu's owner may refuse).
    escape = eqoa.input.claim("Escape", true)
end

eqoa.input.bind({id = "open", label = "Bag viewer", default_key = "shift+b"}, function()
    if B.open then close() else open() end
end)
-- the Start menu (or another mod) asks for the window
eqoa.events.on("mod.command", function(e)
    if e.data.command == "open" then open() end
end)
-- The loot log is the one thing kept while the window is closed, from events only.
B.load_junk()
HP.load()
require("repair") -- the blacksmith's repair panel (modules/repair.lua)
eqoa.events.on("loot.changed", function(e)
    B.log_loot(e.data)
    B.note_loot_closed(e.data)
    if not B.open then B.on_loot_changed(e.data) end
end)
-- Your hits are remembered whether or not Auto-loot is on (one cheap handler per hit; owner 2026-11-25: a creature killed
-- before Auto-loot was switched on is still your kill, within the same ENGAGED_SECS memory). Auto-loot itself listens only
-- while it is on (the default is off), and then only to the render event it polls from (bags.lua `auto_poll`): never to
-- every creature update of the zone.
eqoa.events.on("combat.hit", function(e) B.on_combat_hit(e.data) end)
local auto_tokens = {}
local function auto_listeners(on)
    if on and #auto_tokens == 0 then
        local function add(name, fn)
            local token = eqoa.events.on(name, fn)
            if token then auto_tokens[#auto_tokens + 1] = token end
        end
        -- one look a poll (AUTO_POLL_SECS) from the render event: never every creature update
        add("render", function() B.auto_poll() end)
    elseif not on then
        for _, token in ipairs(auto_tokens) do eqoa.events.off(token) end
        auto_tokens = {}
    end
end
B.on_auto_mode = function(mode) auto_listeners(mode ~= "off") end
auto_listeners(B.auto_mode ~= "off")
-- The automatic helpers (Sell junk, Restock, Bank marked near a merchant or a banker; each off by default) look from the
-- render event, every HP.AUTO_POLL_SECS, and only while one is on for some character.
local helper_token
local function helper_listeners()
    if HP.any_auto() and not helper_token then
        helper_token = eqoa.events.on("render", function() HP.auto_poll() end)
    elseif not HP.any_auto() and helper_token then
        eqoa.events.off(helper_token)
        helper_token = nil
    end
end
HP.on_auto_change = helper_listeners
helper_listeners()
-- a merchant or bank the player opened by hand is a real answer from where they stand: the mod learns the reach
eqoa.events.on("merchant.changed", function(e)
    if e.data and e.data.cause == "open" and eqoa.merchant.get() ~= nil then B.learn_native_open("merchant") end
end)
eqoa.events.on("bank.changed", function(e)
    local b = eqoa.bank.get()
    if e.data and e.data.cause == "open" and b and b.open then B.learn_native_open("bank") end
end)
eqoa.events.on("action.result", function(e)
    if not B.open then B.on_action_result(e.data) end
end)
eqoa.events.on("batch.progress", function(e)
    if not B.open then B.on_batch_progress(e.data) end
end)
eqoa.events.on("batch.result", function(e)
    if not B.open then B.on_batch_result(e.data) end
end)
-- a helper's sale (Sell junk) ends with the client's `confirm.closed`, whether or not the window is open
eqoa.events.on("confirm.closed", function(e)
    if not B.open then B.on_confirm_closed(e.data) end
end)
eqoa.events.on("world.leaving", function(e)
    if e.data and e.data.reason == "disconnect" then B.on_disconnect() end
end)
eqoa.events.on("state.reset", function() AL.on_session({phase = "closed", ref = AL.session}); X.clear(); B.clear_corpse_hold(); B.scan_reset(); B.reset_icons(); HP.reset(); B.char_dirty = true; B.dirty() end)
-- the character is read once the world is up (and again after a new world or login): the Auto-loot choice is per character
eqoa.events.on("player.changed", function(e)
    -- asked on a new world or login, when the update names another character, and once a second besides
    local after = e.data and e.data.after
    local now = os.clock()
    if B.char_dirty or now - B.char_checked >= 1.0 or (after and after.name and after.name ~= B.character) then
        B.char_checked = now
        B.sync_character()
    end
end)
eqoa.events.on("disabling", function()
    auto_listeners(false)
    if helper_token then eqoa.events.off(helper_token); helper_token = nil end
    close()
    if eqoa.windows and eqoa.windows.release then eqoa.windows.release() end
end)

function draw()
    AL.tick()
    local grid = B.grid
    if grid == nil or B.is_grid_stale() then
        grid = B.rebuild()
    elseif B.has_pending_icons then
        local first_slot, last_slot = B.visible_slot_range()
        B.resolve_pending_icons(first_slot, last_slot)
    end
    if not grid then return end
    local f = eqoa.hud.frame()
    local k = math.min(f.scale_x, f.scale_y)
    local dw, dh = ui.GetDisplaySize()
    if k <= 0 or dw <= 0 then return end

    local gy = 46
    local gh = grid.rows * SLOT + (grid.rows - 1) * GAP
    local info_y = gy + gh + 5
    local res_y = info_y + 14
    local div_y = res_y + 13
    local log_hdr_y = div_y + 4
    local log_y = log_hdr_y + 15
    -- Row 3 (Restock / Bank marked: only when a mark exists and the merchant or banker is near or open) makes the loot log
    -- one line shorter; Row 4 is the three "do it when I get near" choices (always there: off by default)
    local bank_open_now = eqoa.bank.get() and eqoa.bank.get().open
    local show_money = B.tab == 6 -- the Bank tab carries the tunar row (one log line less keeps the window's height)
    local show_restock = next(HP.keep) ~= nil and (eqoa.merchant.get() ~= nil or B.nearby_npc("merchant", B.DRAW_TTL) ~= nil)
    local show_bank = next(HP.bank) ~= nil and (bank_open_now or B.nearby_npc("bank", B.DRAW_TTL) ~= nil)
    local has_row3 = show_restock or show_bank
    local log_n = has_row3 and LOG_LINES_HELPERS or LOG_LINES
    if show_money then log_n = math.max(1, log_n - 1) end
    log_shown = log_n
    local bar_y = log_y + log_n * LINE + 4
    local expl_y = bar_y + 18
    local bar2_y = expl_y + 14
    local is_corpse = B.tab == 7
    if is_corpse then B.renew_loot_session() end
    local is_bank = B.tab == 6
    HP.bank_tab_tick()
    -- the Bank tab's status line (replaces the old "Open bank" button): only the reason a banker cannot be reached
    local bank_status
    if is_bank and not bank_open_now and B.nearby_npc("bank", B.DRAW_TTL) ~= nil then
        bank_status = B.too_far("bank", B.DRAW_TTL)
    end
    local has_ctx_btn = is_corpse
        or eqoa.merchant.get() ~= nil
        or eqoa.loot.get() ~= nil
        or B.nearby_npc("merchant", B.DRAW_TTL) ~= nil
        or bank_status ~= nil
    local money_y = has_ctx_btn and (bar2_y + 18) or bar2_y
    local row3_y = show_money and (money_y + 18) or money_y
    local row4_y = has_row3 and (row3_y + 18) or row3_y
    local H = row4_y + 17 + PAD
    -- centred in the HUD frame, and on screen wherever the window is sized
    local x0 = math.max(0, math.min(dw - W * k, f.offset_x + (640 - W) / 2 * f.scale_x))
    local y0 = math.max(0, math.min(dh - H * k, f.offset_y + (448 - H) / 2 * f.scale_y))
    local function at(x, y) return ImVec2.new(x0 + x * k, y0 + y * k) end
    B.draws, B.rect = B.draws + 1, {x = x0, y = y0, w = W * k, h = H * k, k = k}

    ui.SetNextWindowPos(0, 0)
    ui.SetNextWindowSize(dw, dh)
    local flags = ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground + ImGuiWindowFlags.NoMove +
        ImGuiWindowFlags.NoResize + ImGuiWindowFlags.NoScrollbar + ImGuiWindowFlags.NoSavedSettings +
        ImGuiWindowFlags.NoInputs
    ui.Begin("eqoa_bag_viewer_layer", nil, flags)
    ui.SetWindowFontScale(k * 11 / 13)
    local dl = ui.GetWindowDrawList()
    if wmemo_k ~= k or wmemo_n > 400 then wmemo, wmemo_n, wmemo_k = {}, 0, k end
    local function tw(value)
        local w = wmemo[value]
        if w == nil then
            w = ui.CalcTextSize(value)
            wmemo[value], wmemo_n = w, wmemo_n + 1
        end
        return w
    end
    local c = {
        text = ui.GetColorU32(.91, .91, .91, 1), muted = ui.GetColorU32(.6, .6, .6, 1),
        surface = ui.GetColorU32(.067, .067, .067, .96), raised = ui.GetColorU32(.11, .11, .11, 1),
        border = ui.GetColorU32(.25, .25, .25, 1), red = ui.GetColorU32(.843, .098, .129, 1),
        worn = ui.GetColorU32(.35, .8, .4, 1), warn = ui.GetColorU32(1, .3, .3, 1),
        junk = ui.GetColorU32(.95, .72, .20, 1),
        white = ui.GetColorU32(1, 1, 1, 1), shadow = ui.GetColorU32(0, 0, 0, 1),
        mark = ui.GetColorU32(.45, .7, 1, 1),
        dim_icon = ui.GetColorU32(.35, .35, .35, 1),
        menu_bg = ui.GetColorU32(.09, .09, .10, .98), menu_hl = ui.GetColorU32(.20, .20, .24, 1),
    }
    local kinds = {title = c.white, text = c.text, muted = c.muted, red = c.warn}
    local function fill(x, y, w, h, color) dl:AddRectFilled(at(x, y), at(x + w, y + h), color) end
    local function frame(x, y, w, h, color) dl:AddRect(at(x, y), at(x + w, y + h), color) end
    local function text(x, y, value, color, maxw)
        value = tostring(value)
        while maxw and #value > 0 and tw(value) > maxw * k do
            value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
        end
        dl:AddText(at(x, y), color or c.text, value)
    end

    fill(0, 0, W, H, c.surface)
    frame(0, 0, W, H, c.border)
    text(PAD, 7, "Bags", c.white)
    -- the Auction window (the EQOA Auction mod, when it is on): its own button, drawn with the other actions below
    local auction_btn = {x = W - PAD - 56, y = 3, w = 56, h = 15}
    fill(auction_btn.x, auction_btn.y, auction_btn.w, auction_btn.h, c.raised)
    frame(auction_btn.x, auction_btn.y, auction_btn.w, auction_btn.h, c.border)
    text(auction_btn.x + 5, auction_btn.y + 2, "Auction", c.text, auction_btn.w - 6)

    tab_hits = {}
    B.tab_hits = tab_hits
    local tx = PAD
    for _, t in ipairs(B.visible_tabs()) do
        local tabw = tw(t.label) / k + 8
        text(tx + 4, 25, t.label, B.tab == t.idx and c.text or c.muted)
        if B.tab == t.idx then fill(tx, 39, tabw, 2, c.red) end
        tab_hits[#tab_hits + 1] = {x = x0 + tx * k, y = y0 + 22 * k, w = tabw * k, h = 19 * k, tab = t.idx}
        tx = tx + tabw + 2
    end

    local pl = eqoa.player.get()
    local use_stamp = tostring(pl and pl.level) .. ":" .. tostring(pl and pl.class_id) .. ":" .. tostring(pl and pl.race_id)
    local p = eqoa.input.pointer()
    local over
    slot_hits = {}
    B.slot_hits = slot_hits
    local gw = B.COLUMNS * SLOT + (B.COLUMNS - 1) * GAP
    grid_rect = {x = x0 + PAD * k, y = y0 + gy * k, w = gw * k, h = gh * k}
    local row_offset = B.grid_scroll * B.COLUMNS
    for idx = 1, grid.rows * B.COLUMNS do
        local slot = row_offset + idx
        local col, row = (idx - 1) % B.COLUMNS, math.floor((idx - 1) / B.COLUMNS)
        local sx, sy = PAD + col * (SLOT + GAP), gy + row * (SLOT + GAP)
        local p0, p1 = at(sx, sy), at(sx + SLOT, sy + SLOT)
        dl:AddRectFilled(p0, p1, c.raised)
        local entry = grid.items[slot]
        local border = c.border
        if pad.on and not B.menu and idx == pad.idx then border = c.white end
        if entry then
            local it = entry.item
            if entry.texture then
                dl:AddImage(entry.texture, at(sx + 2, sy + 2), at(sx + SLOT - 2, sy + SLOT - 2),
                    ImVec2.new(0, 0), ImVec2.new(1, 1), B.icon_dimmed(entry, use_stamp) and c.dim_icon or c.white)
            else
                text(sx + 14, sy + 11, it.name:sub(1, 1), c.muted)
            end
            if it.equipped then border = c.worn; fill(sx + 2, sy + 2, 9, 11, c.shadow); text(sx + 4, sy + 2, "E", c.worn) end
            if not it.equipped and B.is_junk(it) then
                fill(sx + SLOT - 11, sy + 2, 9, 11, c.shadow)
                text(sx + SLOT - 9, sy + 2, "J", c.junk)
            end
            local mark = B.mark_letter(it)
            if mark then
                fill(sx + 2, sy + SLOT - 13, 9, 11, c.shadow)
                text(sx + 4, sy + SLOT - 13, mark, c.mark)
            end
            if it.quantity > 1 then
                local label = tostring(it.quantity)
                local lw = tw(label) / k
                text(sx + SLOT - lw - 2, sy + SLOT - 13, label, c.shadow)
                text(sx + SLOT - lw - 3, sy + SLOT - 14, label, c.white)
            end
            slot_hits[#slot_hits + 1] = {
                x = x0 + sx * k, y = y0 + sy * k, w = SLOT * k, h = SLOT * k,
                item = it, fx = sx, fy = sy,
            }
            if not B.menu and p and p.inside_window and inside(p.x, p.y, {x = x0 + sx * k, y = y0 + sy * k, w = SLOT * k, h = SLOT * k}) then
                over, border = entry, c.white
            end
            -- a selected slot (Ctrl+click, Shift+click) has a blue outline, kept over the hover and worn colours
            if X.is_selected(it) then
                border = c.mark
                dl:AddRect(at(sx + 1, sy + 1), at(sx + SLOT - 1, sy + SLOT - 1), c.mark)
            end
        end
        dl:AddRect(p0, p1, border)
    end
    local total_rows = grid.total_rows or grid.rows
    if total_rows > grid.rows then
        local sb_x = PAD + gw + 3
        fill(sb_x, gy, 2, gh, c.border)
        local thumb_h = math.max(12, gh * grid.rows / total_rows)
        local max_scroll = total_rows - grid.rows
        local thumb_y = gy + (gh - thumb_h) * (B.grid_scroll / max_scroll)
        fill(sb_x, thumb_y, 2, thumb_h, c.muted)
    end

    local money_label
    if grid.corpse then
        money_label = "Corpse items"
    elseif grid.bank then
        local inv = eqoa.inventory.get()
        money_label = "Bank tunar " .. B.tunar(grid.money) .. "   Carried " .. B.tunar(inv and inv.money or 0)
    else
        money_label = "Tunar " .. B.tunar(grid.money)
    end
    text(PAD, info_y, money_label, c.text, W - 2 * PAD - 40)
    local used = grid.used .. "/" .. (grid.capacity or "?")
    text(W - PAD - ui.CalcTextSize(used) / k, info_y, used, c.text)

    if B.result then
        local rcol = B.result.ok == true and c.worn or (B.result.ok == false and c.warn or c.text)
        text(PAD, res_y, B.result.text, rcol, W - 2 * PAD)
    else
        text(PAD, res_y, X.hint() or "Click an item for actions", X.n > 0 and c.mark or c.muted, W - 2 * PAD)
    end

    fill(PAD, div_y, W - 2 * PAD, 1, c.border)
    text(PAD, log_hdr_y, "Loot log", c.muted)
    log_rect = {x = x0 + PAD * k, y = y0 + log_y * k, w = (W - 2 * PAD) * k, h = log_n * LINE * k}
    if #B.log == 0 then text(PAD, log_y, "Nothing looted yet", c.muted) end
    for i = 1, log_n do
        local entry = B.log[B.scroll + i]
        if entry then text(PAD, log_y + (i - 1) * LINE, entry.text, c.text, W - 2 * PAD) end
    end

    action_hits = {}
    B.action_hits = action_hits
    action_hits[1] = {
        label = "Auction", x = x0 + auction_btn.x * k, y = y0 + auction_btn.y * k, w = auction_btn.w * k, h = auction_btn.h * k,
        run = function()
            if not eqoa.mods.send("eqoa_auction", "open") then
                B.result = {text = "The Auction window is a mod: turn on EQOA Auction in Mods & Addons", ok = false}
            end
        end,
    }
    -- Row 1: the Auto-loot choice (Off / Everything / Everything but junk), and Stop when a batch is running
    -- (right); the line under it says what the chosen one does.
    local seg_x = PAD
    for _, m in ipairs(B.AUTO_MODES) do
        local on = B.auto_mode == m.key
        local bw = tw(m.label) / k + 12
        fill(seg_x, bar_y, bw, 16, on and c.menu_hl or c.raised)
        frame(seg_x, bar_y, bw, 16, on and c.worn or c.border)
        text(seg_x + 6, bar_y + 2, m.label, on and c.text or c.muted)
        local mode = m.key
        action_hits[#action_hits + 1] = {
            label = "Auto-loot: " .. m.label, x = x0 + seg_x * k, y = y0 + bar_y * k, w = bw * k, h = 16 * k,
            run = function() B.set_auto_mode(mode) end,
        }
        seg_x = seg_x + bw + 3
    end
    text(PAD, expl_y, B.auto_explain(), B.auto_mode == "off" and c.muted or c.text, W - 2 * PAD)
    -- (a sale in the client's hands, `sell_many`, ends with the player's own answer: there is nothing for Stop to stop)
    if (B.batch_id ~= nil and B.batch_id ~= B.SELL_MANY) or B.pending_open ~= nil then
        local sw = math.max(44, ui.CalcTextSize("Stop") / k + 14)
        local sx = W - PAD - sw
        fill(sx, bar_y, sw, 16, c.raised)
        frame(sx, bar_y, sw, 16, c.red)
        text(sx + 7, bar_y + 2, "Stop", c.white)
        action_hits[#action_hits + 1] = {
            label = "Stop", x = x0 + sx * k, y = y0 + bar_y * k, w = sw * k, h = 16 * k,
            run = B.stop_batch,
        }
    end

    -- Row 2: Corpse (Open nearest corpse / Take all / Take all but junk), Merchant (Sell junk), or the Bank tab's status line.
    local function ctx_btn(bx, bw, lbl, col, bcol, run)
        fill(bx, bar2_y, bw, 16, c.raised)
        frame(bx, bar2_y, bw, 16, bcol)
        text(bx + 4, bar2_y + 2, lbl, col, bw - 8)
        action_hits[#action_hits + 1] = {
            label = lbl, x = x0 + bx * k, y = y0 + bar2_y * k, w = bw * k, h = 16 * k,
            run = run,
        }
    end
    local loot = eqoa.loot.get()
    local merch = eqoa.merchant.get()
    -- only the corpse buttons read it: a look at the zone is not made for it otherwise
    local near_corpse = (is_corpse or loot ~= nil) and B.nearby_corpse() or nil
    if is_corpse or loot ~= nil then
        -- "Look at corpse" only shows what is in it; the "Take" buttons take (the green ones)
        local bw, bgap = W - 2 * PAD, 6
        local w_loot, w_all = 94, 56
        local w_non = bw - w_loot - w_all - 2 * bgap
        local can_open = loot ~= nil or near_corpse ~= nil
        local has_items = (loot ~= nil and #(loot.items or {}) > 0) or (loot == nil and near_corpse ~= nil)
        local steps, _ = B.loot_non_junk_list()
        local can_non_junk = (loot ~= nil and #steps > 0) or (loot == nil and near_corpse ~= nil)
        ctx_btn(PAD, w_loot, "Look at corpse", can_open and c.text or c.muted, c.border, B.start_loot_button)
        ctx_btn(PAD + w_loot + bgap, w_all, "Take all", has_items and c.text or c.muted, has_items and c.worn or c.border, B.start_loot_all)
        ctx_btn(PAD + w_loot + bgap + w_all + bgap, w_non, "Take all but junk", can_non_junk and c.text or c.muted, can_non_junk and c.worn or c.border, B.start_loot_non_junk)
    elseif merch ~= nil then
        -- the merchant window is open (the player's own or ours): never blocked by distance
        local _, cnt, tunar = B.junk_sell_list()
        local lbl = string.format("Sell junk (%d %s, %s tunar)", cnt, cnt == 1 and "item" or "items", B.tunar(tunar))
        ctx_btn(PAD, W - 2 * PAD, lbl, cnt > 0 and c.text or c.muted, cnt > 0 and c.junk or c.border, B.start_sell_junk)
    elseif bank_status ~= nil then
        -- not a button: the tab refreshes itself near a banker (`HP.bank_tab_tick`); this line says why it cannot
        text(PAD, bar2_y + 2, bank_status, c.muted, W - 2 * PAD)
    elseif B.nearby_npc("merchant", B.DRAW_TTL) ~= nil then
        local _, cnt = B.junk_sell_list()
        local far = B.too_far("merchant", B.DRAW_TTL)
        if far then
            ctx_btn(PAD, W - 2 * PAD, "Sell junk: " .. far, c.muted, c.border, B.start_sell_junk)
        else
            local lbl = string.format("Sell junk (%d %s)", cnt, cnt == 1 and "item" or "items")
            ctx_btn(PAD, W - 2 * PAD, lbl, cnt > 0 and c.text or c.muted, cnt > 0 and c.junk or c.border, B.start_sell_junk)
        end
    end

    -- Bank tunar: the amount box, Deposit and Withdraw, and an All for each
    if show_money then
        local down = ui.IsMouseDown(0) or ui.IsMouseDown(1)
        if BM.field and down and not BM.was_down then
            -- a press on another window lying over this one ends the typing too: the capture is the player's
            local pt = eqoa.input.pointer()
            if not (pt and inside(pt.x, pt.y, BM.rect)) then bm_stop() end
        end
        BM.was_down = down
        if BM.field then
            -- renewed every frame: the capture lapses after about a second
            eqoa.input.capture_text(true)
        end
        local bx, bw = PAD, 76
        fill(bx, money_y, bw, 16, c.raised)
        frame(bx, money_y, bw, 16, BM.field and c.white or c.border)
        if BM.field then text(bx + 4, money_y + 2, BM.text .. "_", c.text, bw - 8)
        elseif BM.text ~= "" then text(bx + 4, money_y + 2, BM.text, c.text, bw - 8)
        else text(bx + 4, money_y + 2, "tunar", c.muted, bw - 8) end
        BM.rect = {x = x0 + bx * k, y = y0 + money_y * k, w = bw * k, h = 16 * k}
        action_hits[#action_hits + 1] = {
            label = "Tunar amount", x = BM.rect.x, y = BM.rect.y, w = BM.rect.w, h = BM.rect.h, run = bm_start,
        }
        bx = bx + bw + 6
        local _, carried, banked, amount = bm_numbers()
        local function mbtn(label, hit_label, dir, amt)
            local why = bm_why(dir, amt)
            local w = tw(label) / k + 12
            fill(bx, money_y, w, 16, c.raised)
            frame(bx, money_y, w, 16, why == nil and c.worn or c.border)
            text(bx + 6, money_y + 2, label, why == nil and c.text or c.muted, w - 8)
            action_hits[#action_hits + 1] = {
                label = hit_label, x = x0 + bx * k, y = y0 + money_y * k, w = w * k, h = 16 * k,
                run = function() bm_send(dir, amt) end,
            }
            bx = bx + w + 3
        end
        mbtn("Deposit", "Deposit tunar", "deposit", amount)
        mbtn("All", "Deposit all tunar", "deposit", carried)
        bx = bx + 3
        mbtn("Withdraw", "Withdraw tunar", "withdraw", amount)
        mbtn("All", "Withdraw all tunar", "withdraw", banked)
    else
        BM.rect = nil
        if BM.field then bm_stop() end
    end

    -- Row 3: the marked-item helpers
    if has_row3 then
        local bw = show_restock and show_bank and (W - 2 * PAD - 6) / 2 or (W - 2 * PAD)
        local bx = PAD
        local function row3_btn(lbl, col, bcol, run)
            fill(bx, row3_y, bw, 16, c.raised)
            frame(bx, row3_y, bw, 16, bcol)
            text(bx + 4, row3_y + 2, lbl, col, bw - 8)
            action_hits[#action_hits + 1] = {
                label = lbl, x = x0 + bx * k, y = y0 + row3_y * k, w = bw * k, h = 16 * k, run = run,
            }
            bx = bx + bw + 6
        end
        if show_restock then
            local lbl
            local plan = eqoa.merchant.get() ~= nil and HP.restock_plan_cached() or nil
            local n = plan and #plan.buys or HP.short_count()
            local far = eqoa.merchant.get() == nil and B.too_far("merchant", B.DRAW_TTL) or nil
            if far then
                lbl = "Restock: " .. far
            elseif plan then
                lbl = string.format("Restock (%d %s, %s tunar)", plan.total, plan.total == 1 and "item" or "items", B.tunar(plan.cost))
            else
                lbl = string.format("Restock (%d)", n)
            end
            local live = (plan and plan.total or n) > 0 and not far
            row3_btn(lbl, live and c.text or c.muted, live and c.worn or c.border, HP.start_restock)
        end
        if show_bank then
            local n = HP.bank_count()
            local far = not bank_open_now and B.too_far("bank", B.DRAW_TTL) or nil
            local lbl = far and ("Bank marked: " .. far) or string.format("Bank marked (%d)", n)
            local live = n > 0 and not far
            row3_btn(lbl, live and c.text or c.muted, live and c.worn or c.border, HP.start_bank)
        end
    end

    -- Row 4: do it automatically when I get near (each helper's own choice, per character; Bank starts on, the other two off)
    do
        local lead = "Do it when I get near:"
        text(PAD, row4_y + 2, lead, c.muted)
        local cx = PAD + tw(lead) / k + 6
        for _, h in ipairs(HP.HELPERS) do
            local on = HP.auto_on(h.key)
            local cw = tw(h.label) / k + 12
            fill(cx, row4_y, cw, 16, on and c.menu_hl or c.raised)
            frame(cx, row4_y, cw, 16, on and c.worn or c.border)
            text(cx + 6, row4_y + 2, h.label, on and c.text or c.muted)
            local key, now_on = h.key, on
            action_hits[#action_hits + 1] = {
                label = "Auto " .. string.lower(h.label), x = x0 + cx * k, y = y0 + row4_y * k, w = cw * k, h = 16 * k,
                run = function() HP.set_auto(key, not now_on) end,
            }
            cx = cx + cw + 3
        end
    end

    eqoa.input.claim_pointer({x = x0, y = y0, width = W * k, height = H * k})

    menu_hits = {}
    B.menu_hits = menu_hits
    if B.menu then
        local m = B.menu
        local mw, rh = 232, 15
        local mh = 18 + #m.rows * rh + 4
        local mx = math.max(PAD, math.min(W - mw - PAD, m.fx + SLOT + 2))
        local my = math.max(PAD, math.min(H - mh - PAD, m.fy))
        B.menu_rect = {x = x0 + mx * k, y = y0 + my * k, w = mw * k, h = mh * k}
        fill(mx, my, mw, mh, c.menu_bg)
        frame(mx, my, mw, mh, c.red)
        text(mx + 6, my + 3, m.title, c.white, mw - 12)
        fill(mx + 4, my + 16, mw - 8, 1, c.border)
        for i, r in ipairs(m.rows) do
            local ry = my + 18 + (i - 1) * rh
            local hr = {x = x0 + (mx + 2) * k, y = y0 + ry * k, w = (mw - 4) * k, h = rh * k, row = r}
            if (r.enabled and p and p.inside_window and inside(p.x, p.y, hr)) or (pad.on and i == pad.msel) then
                fill(mx + 2, ry, mw - 4, rh, c.menu_hl)
            end
            local label = r.enabled and (r.text or r.label) or (r.label .. " (" .. (r.reason or "disabled") .. ")")
            text(mx + 6, ry + 2, label, r.enabled and c.text or c.muted, mw - 12)
            menu_hits[#menu_hits + 1] = hr
        end
    else
        B.menu_rect = nil
    end

    if over then
        local lines = B.card(over.item)
        local cw = 0
        for _, l in ipairs(lines) do cw = math.max(cw, ui.CalcTextSize(l[1])) end
        local cwk, chk = math.min(cw + 12 * k, 220 * k), (#lines * LINE + 8) * k
        local cx = math.max(0, math.min(dw - cwk, p.x + 14 * k > dw - cwk and p.x - 14 * k - cwk or p.x + 14 * k))
        local cy = math.max(0, math.min(dh - chk, p.y + 14 * k))
        dl:AddRectFilled(ImVec2.new(cx, cy), ImVec2.new(cx + cwk, cy + chk), c.surface)
        dl:AddRect(ImVec2.new(cx, cy), ImVec2.new(cx + cwk, cy + chk), c.red)
        for i, l in ipairs(lines) do
            local value = l[1]
            while #value > 0 and ui.CalcTextSize(value) > cwk - 8 * k do
                value = value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or ""
            end
            dl:AddText(ImVec2.new(cx + 6 * k, cy + 4 * k + (i - 1) * LINE * k), kinds[l[2]], value)
        end
    end
    -- the listing form, over the window, when one is up
    AL.draw({ui = ui, k = k, c = c, x0 = x0, y0 = y0, W = W, fill = fill, frame = frame, text = text})
    ui.SetWindowFontScale(1)
    ui.End()
end
