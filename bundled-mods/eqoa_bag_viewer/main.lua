-- Bag viewer: Bags, Bank, and Corpse window over the public eqoa items & actions API.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local ui = eqoa.imgui
local B = require("bags")
local HP = require("helpers")

-- Sizes are in HUD frame pixels (640x448), scaled as the HUD is.
local W, PAD, SLOT, GAP, LINE, LOG_LINES = 318, 8, 36, 2, 12, 5
local wmemo, wmemo_k, wmemo_n = {}, nil, 0
local tokens, escape, tab_hits, slot_hits, menu_hits, action_hits, log_rect, grid_rect = {}, nil, {}, {}, {}, {}, nil, nil

local function inside(x, y, r) return r and x >= r.x and x <= r.x + r.w and y >= r.y and y <= r.y + r.h end

-- The window stops listening to everything but loot while it is closed.
local function listen(name, fn)
    local token = eqoa.events.on(name, fn)
    if token then tokens[#tokens + 1] = token end
end

local draw
local function close()
    if not B.open then return end
    for _, token in ipairs(tokens) do eqoa.events.off(token) end
    tokens = {}
    if escape then eqoa.input.release(escape); escape = nil end
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
    tab_hits, slot_hits, menu_hits, action_hits, log_rect, grid_rect = {}, {}, {}, {}, nil, nil
    B.tab_hits, B.slot_hits, B.menu_hits, B.action_hits = tab_hits, slot_hits, menu_hits, action_hits
end

local function open()
    if B.open then return end
    if not B.junk_loaded then B.load_junk() end
    B.open, B.grid, B.menu = true, nil, nil
    if B.char_dirty then B.sync_character() end
    B.scan_reset()
    listen("render", draw)
    listen("inventory.changed", B.dirty)
    listen("bank.changed", function(e) B.on_bank_changed(e.data) end)
    listen("loot.changed", function(e) B.on_loot_changed(e.data) end)
    listen("merchant.changed", function(e) B.on_merchant_changed(e.data) end)
    listen("state.reset", B.dirty)
    listen("action.result", function(e) B.on_action_result(e.data) end)
    listen("batch.progress", function(e) B.on_batch_progress(e.data) end)
    listen("batch.result", function(e) B.on_batch_result(e.data) end)
    listen("input.key", function(e)
        if e.data.down and not e.data["repeat"] and e.data.key:lower() == "escape" then
            if B.menu then B.menu, B.menu_rect = nil, nil else close() end
        end
    end)
    listen("pointer.pressed", function(e)
        local d = e.data
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
        if d.button ~= "left" then return end
        for _, h in ipairs(tab_hits) do if inside(d.x, d.y, h) then B.select(h.tab); return end end
        for _, h in ipairs(slot_hits) do
            if inside(d.x, d.y, h) then B.build_menu(h.item, h.fx, h.fy); return end
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
                B.scroll = math.max(0, math.min(math.max(0, #B.log - LOG_LINES), B.scroll + delta))
            elseif inside(d.x, d.y, grid_rect) then
                B.scroll_grid(delta)
            end
        end
    end)
    -- Escape is the window's, not the game menu's, while it is up (a claim the menu's owner may refuse).
    escape = eqoa.input.claim("Escape", true)
end

eqoa.input.bind({id = "open", label = "Bag viewer", default_key = ""}, function()
    if B.open then close() else open() end
end)
-- The loot log is the one thing kept while the window is closed, from events only.
B.load_junk()
HP.load()
eqoa.events.on("loot.changed", function(e)
    B.log_loot(e.data)
    B.note_loot_closed(e.data)
    if not B.open then B.on_loot_changed(e.data) end
end)
-- Auto-loot listens only while it is on (the default is off), and then only to your hits and to the render event it
-- polls from (bags.lua `auto_poll`): never to every creature update of the zone.
local auto_tokens = {}
local function auto_listeners(on)
    if on and #auto_tokens == 0 then
        local function add(name, fn)
            local token = eqoa.events.on(name, fn)
            if token then auto_tokens[#auto_tokens + 1] = token end
        end
        -- your hits, and one look a poll (AUTO_POLL_SECS) from the render event: never every creature update
        add("combat.hit", function(e) B.on_combat_hit(e.data) end)
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
eqoa.events.on("world.leaving", function(e)
    if e.data and e.data.reason == "disconnect" then B.on_disconnect() end
end)
eqoa.events.on("state.reset", function() B.clear_corpse_hold(); B.scan_reset(); B.reset_icons(); HP.reset(); B.char_dirty = true; B.dirty() end)
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
    local bar_y = log_y + LOG_LINES * LINE + 4
    local expl_y = bar_y + 18
    local bar2_y = expl_y + 14
    local is_corpse = B.tab == 7
    if is_corpse then B.renew_loot_session() end
    local is_bank = B.tab == 6
    local has_ctx_btn = is_corpse
        or eqoa.merchant.get() ~= nil
        or eqoa.loot.get() ~= nil
        or B.nearby_npc("merchant") ~= nil
        or (is_bank and not (eqoa.bank.get() and eqoa.bank.get().open) and B.nearby_npc("bank") ~= nil)
    -- Row 3: Restock / Bank marked (only when a mark exists and the merchant or banker is near or open); Row 4: the three
    -- "do it when I get near" choices (always there: off by default)
    local row3_y = has_ctx_btn and (bar2_y + 18) or bar2_y
    local show_restock = next(HP.keep) ~= nil and (eqoa.merchant.get() ~= nil or B.nearby_npc("merchant") ~= nil)
    local bank_open_now = eqoa.bank.get() and eqoa.bank.get().open
    local show_bank = next(HP.bank) ~= nil and (bank_open_now or B.nearby_npc("bank") ~= nil)
    local has_row3 = show_restock or show_bank
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
        if entry then
            local it = entry.item
            if entry.texture then
                dl:AddImage(entry.texture, at(sx + 2, sy + 2), at(sx + SLOT - 2, sy + SLOT - 2),
                    ImVec2.new(0, 0), ImVec2.new(1, 1), c.white)
            else
                text(sx + 14, sy + 11, it.name:sub(1, 1), c.muted)
            end
            if it.equipped then border = c.worn; fill(sx + 2, sy + 2, 9, 11, c.shadow); text(sx + 4, sy + 2, "E", c.worn) end
            if not it.equipped and B.is_junk(it) then
                fill(sx + SLOT - 11, sy + 2, 9, 11, c.shadow)
                text(sx + SLOT - 9, sy + 2, "J", c.junk)
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
        money_label = "Bank tunar " .. B.tunar(grid.money)
    else
        money_label = "Tunar " .. B.tunar(grid.money)
    end
    text(PAD, info_y, money_label, c.text)
    local used = grid.used .. "/" .. (grid.capacity or "?")
    text(W - PAD - ui.CalcTextSize(used) / k, info_y, used, c.text)

    if B.result then
        local rcol = B.result.ok == true and c.worn or (B.result.ok == false and c.warn or c.text)
        text(PAD, res_y, B.result.text, rcol, W - 2 * PAD)
    else
        text(PAD, res_y, "Click an item for actions", c.muted, W - 2 * PAD)
    end

    fill(PAD, div_y, W - 2 * PAD, 1, c.border)
    text(PAD, log_hdr_y, "Loot log", c.muted)
    log_rect = {x = x0 + PAD * k, y = y0 + log_y * k, w = (W - 2 * PAD) * k, h = LOG_LINES * LINE * k}
    if #B.log == 0 then text(PAD, log_y, "Nothing looted yet", c.muted) end
    for i = 1, LOG_LINES do
        local entry = B.log[B.scroll + i]
        if entry then text(PAD, log_y + (i - 1) * LINE, entry.text, c.text, W - 2 * PAD) end
    end

    action_hits = {}
    B.action_hits = action_hits
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
    if B.batch_id ~= nil or B.pending_open ~= nil then
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

    -- Row 2: Corpse (Open nearest corpse / Take all / Take all but junk), Merchant (Sell junk), or Bank (Open bank).
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
    local near_corpse = B.nearby_corpse()
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
    elseif is_bank and not (eqoa.bank.get() and eqoa.bank.get().open) and B.nearby_npc("bank") ~= nil then
        local far = B.too_far("bank")
        if far then
            ctx_btn(PAD, W - 2 * PAD, "Open bank: " .. far, c.muted, c.border, function()
                B.open_talk_npc("bank", "Open bank", function()
                    B.result = {text = "Bank ready", ok = true}
                    B.dirty()
                end)
            end)
        else
            ctx_btn(PAD, W - 2 * PAD, "Open bank", c.text, c.worn, function()
                B.open_talk_npc("bank", "Open bank", function()
                    B.result = {text = "Bank ready", ok = true}
                    B.dirty()
                end)
            end)
        end
    elseif B.nearby_npc("merchant") ~= nil then
        local _, cnt = B.junk_sell_list()
        local far = B.too_far("merchant")
        if far then
            ctx_btn(PAD, W - 2 * PAD, "Sell junk: " .. far, c.muted, c.border, B.start_sell_junk)
        else
            local lbl = string.format("Sell junk (%d %s)", cnt, cnt == 1 and "item" or "items")
            ctx_btn(PAD, W - 2 * PAD, lbl, cnt > 0 and c.text or c.muted, cnt > 0 and c.junk or c.border, B.start_sell_junk)
        end
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
            local far = eqoa.merchant.get() == nil and B.too_far("merchant") or nil
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
            local far = not bank_open_now and B.too_far("bank") or nil
            local lbl = far and ("Bank marked: " .. far) or string.format("Bank marked (%d)", n)
            local live = n > 0 and not far
            row3_btn(lbl, live and c.text or c.muted, live and c.worn or c.border, HP.start_bank)
        end
    end

    -- Row 4: do it automatically when I get near (each helper's own choice, per character, off by default)
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
            if r.enabled and p and p.inside_window and inside(p.x, p.y, hr) then
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
    ui.SetWindowFontScale(1)
    ui.End()
end
