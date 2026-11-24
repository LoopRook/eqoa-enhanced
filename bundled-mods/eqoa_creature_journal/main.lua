-- Creature journal, ported from EQOA Enhanced map/journal.rs.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local records, known, grid = {}, {}, {}
local count, now, first_dirty, last_change = 0, 0, nil, nil
local ready, protected, scan = false, false, true
local loading, importing, saving, revision, saved_revision = nil, nil, nil, 0, 0
local merge_job

local function notify(message)
    print(message)
    eqoa.mods.send("eqoa_map_search", "journal_status", {message=message})
end
-- Rust str::trim uses Unicode White_Space, beyond Lua's ASCII %s class.
local spaces = {}
for c in ("\194\133\194\160\225\154\128\226\128\128\226\128\129\226\128\130\226\128\131\226\128\132\226\128\133\226\128\134\226\128\135\226\128\136\226\128\137\226\128\138\226\128\168\226\128\169\226\128\175\226\129\159\227\128\128"):gmatch("[%z\1-\127\194-\244][\128-\191]*") do spaces[c] = true end
-- Creatures repeat their names thousands of times: each name is trimmed once.
local trimmed, trimmed_count = {}, 0
local function trim_name_uncached(name)
    local chars = {}
    for c in name:gmatch("[%z\1-\127\194-\244][\128-\191]*") do chars[#chars+1] = c end
    local first,last = 1,#chars
    while first <= last and (spaces[chars[first]] or chars[first]:match("^%s$")) do first=first+1 end
    while last >= first and (spaces[chars[last]] or chars[last]:match("^%s$")) do last=last-1 end
    return first > last and "" or table.concat(chars,"",first,last)
end
local function trim_name(name)
    local t = trimmed[name]
    if t then return t end
    t = trim_name_uncached(name)
    if trimmed_count >= 4096 then trimmed, trimmed_count = {}, 0 end
    trimmed[name] = t; trimmed_count = trimmed_count + 1
    return t
end
local function finite(n) return type(n) == "number" and n == n and math.abs(n) <= 3.4028234663852886e38 end
local function changed()
    revision = revision + 1
    first_dirty = first_dirty or now
    last_change = now
end
-- The grid is name, level, then 30-unit cell column and row: numbers, not strings built per lookup.
local keys = {}
local function grid_key(name, level)
    local by_level = keys[name]
    local key = by_level and by_level[level]
    if key then return key end
    key = name .. "\0" .. level
    if not by_level then by_level = {}; keys[name] = by_level end
    by_level[level] = key
    return key
end
local function index(name, id, info)
    local numeric = tonumber(id)
    if numeric and numeric >= 0 and numeric <= 4294967295 then known[numeric] = true end
    local key = grid_key(name, info.level)
    local cells = grid[key]
    if not cells then cells = {}; grid[key] = cells end
    local column = math.floor(info.pos[1] / 30)
    local rows = cells[column]
    if not rows then rows = {}; cells[column] = rows end
    local row = math.floor(info.pos[3] / 30)
    local points = rows[row]
    if not points then points = {}; rows[row] = points end
    points[#points + 1] = info.pos
end
local function put(name, id, info)
    if count >= 80000 then return false end
    local ids = records[name]
    if not ids then ids = {}; records[name] = ids end
    if ids[id] then return false end
    ids[id] = info; count = count + 1; index(name, id, info)
    return true
end
local function near(name, level, x, z)
    local cells = grid[grid_key(name, level)]
    if not cells then return false end
    local cx, cz = math.floor(x / 30), math.floor(z / 30)
    for dx = -1, 1 do
        local rows = cells[cx + dx]
        if rows then for dz = -1, 1 do
            local points = rows[cz + dz]
            if points then for _, p in ipairs(points) do
                if (p[1] - x)^2 + (p[3] - z)^2 <= 900 then return true end
            end end
        end end
    end
    return false
end
-- The player is read at most once a frame: an event per moving creature each asked for it.
local player, player_at
local function the_player(frame)
    if frame == nil or frame ~= player_at then player = eqoa.player.get(); player_at = frame end
    return player
end
local function record(e, frame)
    if not ready or protected or not e or not e.id or e.id == 0 or known[e.id] then return end
    if not e.is_creature or e.corpse or not e.class_code or e.class_code >= 2 or e.search_flags ~= 0 then return end
    local name = trim_name(e.name or "")
    local pos = e.actor_position or e.position
    if name == "" or not pos or not finite(pos.x) or not finite(pos.y) or not finite(pos.z) then return end
    if near(name, e.level, pos.x, pos.z) then return end
    local p = the_player(frame)
    if p and (e.id == p.id or e.id == p.pet_id) then return end
    if put(name, tostring(e.id), {world=e.world_id or 0, level=e.level, pos={pos.x,pos.y,pos.z}}) then changed() end
end
local function save(force)
    if protected or not ready or merge_job or (not force and revision == saved_revision) then return end
    local request, err = eqoa.files.save_json_async("journal.json", records)
    if request then saving = {id=request, revision=revision};first_dirty=nil else notify((force and "Export: " or "Journal save: ") .. (err and err.message or "snapshot unavailable")) end
    return request
end
local function valid(name, id, v)
    if type(name) ~= "string" or #name > 64 or name:find("[%z\1-\31\127]") or name:find("\194[\128-\159]") then return end
    if type(id) ~= "string" or #id > 20 or id:find("[^0-9]") or type(v) ~= "table" then return end
    if type(v.level) ~= "number" or v.level % 1 ~= 0 or v.level < 0 or v.level > 255 then return end
    local p = v.pos
    if type(p) ~= "table" or not finite(p[1]) or not finite(p[2]) or not finite(p[3]) then return end
    local w = v.world
    if type(w) ~= "number" or w % 1 ~= 0 then w = 0 end
    w = (w + 2147483648) % 4294967296 - 2147483648
    return {world=w,level=v.level,pos={p[1],p[2],p[3]}}
end
-- Walk large imports in bounded batches; neither validation nor indexing takes
-- an entire frame. Existing local name/id entries win, as in the native journal.
local function merge(value, initial)
    if type(value) ~= "table" or getmetatable(value) == "JSON array" then
        if initial then protected = true end
        notify("Import: expected JSON object at root"); return
    end
    merge_job = {value=value, name=nil, id=nil, ids=nil, n=0, initial=initial, added=0}
end
local function merge_step()
    local j = merge_job
    if not j then return end
    for _ = 1, 200 do
        if not j.ids then
            j.name, j.ids = next(j.value, j.name)
            j.id = nil
            if j.name == nil then break end
            if type(j.ids) ~= "table" then j.ids = nil end
        end
        if j.ids then
            local v
            j.id, v = next(j.ids, j.id)
            if j.id == nil then j.ids = nil else
                local info = valid(j.name, j.id, v)
                if info then
                    j.n = j.n + 1
                    if put(j.name, j.id, info) then j.added = j.added + 1 end
                end
            end
        end
        if count >= 80000 or (not j.initial and j.n >= 50000) then break end
    end
    if j.name == nil or count >= 80000 or (not j.initial and j.n >= 50000) then
        merge_job = nil
        if j.initial then ready = true; scan = true
        else
            if j.added > 0 then changed(); save(false) end
            notify("Imported " .. j.added .. " spawns")
        end
    end
end
local function import_journal()
    if not ready or merge_job or importing then notify("Journal is loading"); return end
    if protected then notify("Journal is disabled to protect existing file"); return end
    local request, err = eqoa.files.import{title="Import creature journal", extensions={"json"}}
    if request then importing = request else notify("Import: " .. err.message) end
end
local function export_journal()
    if protected then notify("Export: Journal is disabled to protect existing file");return end
    if not ready or merge_job then notify("Export: Journal is loading");return end
    -- Flush the current snapshot in the player's callback; export waits on its worker.
    if not save(true) then return end
    local request, err = eqoa.files.export("journal.json", "EQOA-Creature-Journal.json")
    if not request then notify("Export: " .. err.message) end
end
eqoa.options.button({id="import",label="Import journal..."}, import_journal)
eqoa.options.button({id="export",label="Export journal"}, export_journal)
eqoa.events.on("mod.command", function(e)
    if e.data.command == "import_journal" then import_journal()
    elseif e.data.command == "export_journal" then export_journal() end
end)
eqoa.events.on("resource.ready", function(e)
    local d = e.data
    if d.kind == "json" and d.request == loading then loading = nil; merge(d.value, true)
    elseif d.kind == "import" and d.request == importing then
        importing = nil
        local request, err = eqoa.files.load_json(d.path)
        if request then importing = {load=request} else notify("Import: " .. err.message) end
    elseif d.kind == "json" and type(importing) == "table" and d.request == importing.load then
        importing = nil; merge(d.value, false)
    elseif d.kind == "export" then notify("Saved to " .. d.path)
    elseif d.kind == "json_saved" and saving and d.request == saving.id then
        saved_revision = saving.revision; saving = nil
        if saved_revision == revision then first_dirty = nil; last_change = nil end
    end
end)
eqoa.events.on("resource.failed", function(e)
    local d = e.data
    if d.kind == "json" and d.request == loading then
        loading = nil; protected = true; print("Journal load failed; preserving existing file")
    elseif saving and d.kind == "json_saved" and d.request == saving.id then saving = nil;if revision~=saved_revision then first_dirty=first_dirty or now;last_change=last_change or now end;notify("Export: " .. (d.error and d.error.message or "save failed"))
    elseif d.kind == "import" or (type(importing) == "table" and d.request == importing.load) then
        importing = nil; notify("Import: " .. (d.error and d.error.message or "failed"))
    elseif d.kind == "export" then notify("Export: " .. (d.error and d.error.message or "failed")) end
end)
-- An update that changed only fields the journal does not read (hit points, who it targets, an animation) cannot make a
-- creature newly worth recording: the last look at it, an added or a scan, had everything the answer depends on. Only
-- the fields `record` reads are worth the work.
local READ = {id=true, name=true, level=true, is_creature=true, corpse=true, class_code=true, search_flags=true,
    position=true, actor_position=true, world_id=true}
local function reads_changed(list)
    if type(list) ~= "table" or #list == 0 then return true end
    for _, field in ipairs(list) do if READ[field] then return true end end
    return false
end
eqoa.events.on("entity.added", function(e) record(e.data.after, e.frame) end)
eqoa.events.on("entity.changed", function(e)
    if reads_changed(e.data.changed) then record(e.data.after, e.frame) end
end)
for _, name in ipairs({"state.reset","events.dropped"}) do eqoa.events.on(name,function() scan = true end) end
eqoa.events.on("render", function(e)
    now = e.time
    merge_step()
    if ready and scan then
        scan = false
        local entities = eqoa.entities.list()
        if entities then for _, entity in ipairs(entities) do record(entity) end end
    end
    if first_dirty and not saving and (now - last_change >= 30 or now - first_dirty >= 120) then save(false) end
end)
eqoa.events.on("disabling", function() save(false) end)
local err
loading, err = eqoa.files.load_json("journal.json")
if not loading then
    if err and err.message:find("file not found", 1, true) then ready = true
    else protected = true; print("Journal load: " .. (err and err.message or "unavailable")) end
end
