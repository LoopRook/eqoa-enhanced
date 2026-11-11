-- Creature journal, ported from EQOA Enhanced map/journal.rs.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local records, known, grid = {}, {}, {}
local count, now, first_dirty, last_change = 0, 0, nil, nil
local ready, protected, scan = false, false, true
local loading, importing, saving, revision, saved_revision = nil, nil, nil, 0, 0
local merge_job
local function finite(n) return type(n) == "number" and n == n and math.abs(n) <= 3.4028234663852886e38 end
local function changed()
    revision = revision + 1
    first_dirty = first_dirty or now
    last_change = now
end
local function index(name, id, info)
    local numeric = tonumber(id)
    if numeric and numeric >= 0 and numeric <= 4294967295 then known[numeric] = true end
    local key = name .. "\0" .. info.level
    local cells = grid[key]
    if not cells then cells = {}; grid[key] = cells end
    local cell = math.floor(info.pos[1] / 30) .. ":" .. math.floor(info.pos[3] / 30)
    local points = cells[cell]
    if not points then points = {}; cells[cell] = points end
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
    local cells = grid[name .. "\0" .. level]
    if not cells then return false end
    local cx, cz = math.floor(x / 30), math.floor(z / 30)
    for dx = -1, 1 do for dz = -1, 1 do
        local points = cells[(cx + dx) .. ":" .. (cz + dz)]
        if points then for _, p in ipairs(points) do
            if (p[1] - x)^2 + (p[3] - z)^2 <= 900 then return true end
        end end
    end end
    return false
end
local function record(e)
    if not ready or protected or not e or not e.id or e.id == 0 or known[e.id] then return end
    local p = eqoa.player.get()
    if p and (e.id == p.id or e.id == p.pet_id) then return end
    if not e.is_creature or e.corpse or not e.class_code or e.class_code >= 2 or e.search_flags ~= 0 then return end
    local name = (e.name or ""):match("^%s*(.-)%s*$")
    local pos = e.position
    if name == "" or not pos or not finite(pos.x) or not finite(pos.y) or not finite(pos.z) then return end
    if near(name, e.level, pos.x, pos.z) then return end
    if put(name, tostring(e.id), {world=e.world_id or 0, level=e.level, pos={pos.x,pos.y,pos.z}}) then changed() end
end
local function save(force)
    if protected or not ready or merge_job or (not force and revision == saved_revision) then return end
    local request, err = eqoa.files.save_json_async("journal.json", records)
    if request then saving = {id=request, revision=revision} else print("Journal save: " .. err.message) end
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
    if type(value) ~= "table" then
        if initial then protected = true end
        print("Journal: expected JSON object at root"); return
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
        elseif j.added > 0 then changed(); save(false) end
    end
end
local function import_journal()
    if not ready or protected or merge_job or importing then return end
    local request, err = eqoa.files.import{title="Import creature journal", extensions={"json"}}
    if request then importing = request else print("Journal import: " .. err.message) end
end
local function export_journal()
    if not ready or protected or merge_job then return end
    -- Flush the current snapshot in the player's callback; export waits on its worker.
    if not save(true) then return end
    local request, err = eqoa.files.export("journal.json", "EQOA-Creature-Journal.json")
    if not request then print("Journal export: " .. err.message) end
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
        if request then importing = {load=request} else print("Journal import: " .. err.message) end
    elseif d.kind == "json" and type(importing) == "table" and d.request == importing.load then
        importing = nil; merge(d.value, false)
    elseif d.kind == "json_saved" and saving and d.request == saving.id then
        saved_revision = saving.revision; saving = nil
        if saved_revision == revision then first_dirty = nil; last_change = nil end
    end
end)
eqoa.events.on("resource.failed", function(e)
    local d = e.data
    if d.kind == "json" and d.request == loading then
        loading = nil; protected = true; print("Journal load failed; preserving existing file")
    elseif saving and d.kind == "json_saved" and d.request == saving.id then saving = nil
    elseif d.kind == "import" or (type(importing) == "table" and d.request == importing.load) then importing = nil end
end)
for _, name in ipairs({"entity.added","entity.changed"}) do
    eqoa.events.on(name, function(e) record(e.data.after) end)
end
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
