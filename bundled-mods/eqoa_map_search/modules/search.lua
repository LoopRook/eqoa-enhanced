-- Native search rules and navigation, independent of its presentation.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local M = {open=false,query="",tab=1,selected=1,offset=1,rows={},catalog={},journal={},jobs={},by_name={catalog={},journal={}},dirty=true,refresh=true,world=0,player={x=0,z=0},live={},group={},places={}}
local data=require("spawns")
local valid,spawn,spawn_text=data.valid,data.lean,data.text
local function distance(p) local x,z=p.x-M.player.x,p.z-M.player.z;return math.sqrt(x*x+z*z) end
local function matches(s,words,lowered) if not lowered then s=s:lower() end;for _,w in ipairs(words) do if not s:find(w,1,true) then return false end end;return true end
local function words() local out={};for w in M.query:lower():gmatch("%S+") do out[#out+1]=w end;return out end
function M.load(value,kind)
    if type(value)~="table" then return end
    M.jobs[#M.jobs+1]={value=value,kind=kind,out={},name=nil,row=nil,index=0,id=nil,by_name={},names=0}
end
local function load_step()
    local j=M.jobs[1];if not j then return end
    local done=false
    for _=1,500 do
        local before=#j.out
        if not j.row then
            -- the finished name's raw rows are let go as the walk moves on, so the raw file and the converted spawns are never both whole in memory
            local prev=j.name
            j.name,j.row=next(j.value,prev);j.index=0;j.id=nil
            if prev~=nil then j.value[prev]=nil end
            if not j.name then done=true;break end
            if type(j.row)~="table" then j.row=nil end
        end
        if j.row then
            if j.kind=="catalog" then
                j.index=j.index+1
                local w,p=j.row.world,j.row.pos
                if type(w)~="table" or type(p)~="table" or j.index>math.min(#w,#p) then j.row=nil
                else spawn(j.out,j.name,w[j.index],p[j.index]) end
            else
                local v;j.id,v=next(j.row,j.id)
                if not j.id then j.row=nil
                elseif type(v)=="table" then spawn(j.out,j.name,v.world,v.pos,v.level) end
            end
        end
        if #j.out>before then
            if j.counted~=j.name then j.counted=j.name;j.names=j.names+1;j.lower=j.name:lower() end
            local list=j.by_name[j.lower]
            if not list then list={name=j.name};j.by_name[j.lower]=list end
            list[#list+1]=j.out[#j.out]
        end
    end
    if done then
        M.by_name[j.kind]=j.by_name;M.index_stale=true;M.rows_stale=true;M.build=nil
        M.index_job=nil -- its walk is over the table this load just replaced; the stale flag starts a fresh one
        M[j.kind]=j.out
        if j.kind=="journal" then M.journal_names=j.names end
        table.remove(M.jobs,1);M.dirty=true
    end
end
local function live_valid(e)
    return e.is_creature and not e.corpse and e.hp_fraction and e.hp_fraction>0 and e.search_flags==0 and e.class_code and e.class_code<2 and e.name and not e.name:match("^%s*$") and e.position and valid(e.position.x,e.position.z)
end
-- Keeps the guide on its target: the nearest live creature of its name, the group member, the waypoint.
local function follow_guide()
    local guide=M.guide
    if guide then
        if guide.kind=="npc" then
            local nearest
            for _,e in ipairs(M.live) do if e.key==guide.key and e.world==M.world and (not nearest or distance(e)<distance(nearest)) then nearest=e end end
            if (nearest and nearest.id or nil)~=guide.live_id or (guide.last_player and distance(guide.last_player)>=30) then M.choose(guide,true) end
        elseif guide.kind=="group" then
            for _,g in ipairs(M.group) do if g.id==guide.id then
                if g.active and g.position and g.world_id==M.world then
                    guide.world=M.world;guide.at=g.position;M.choose(guide,true)
                else eqoa.map.clear_guide() end
            end end
        elseif guide.kind=="waypoint" then
            local w=M.waypoint
            if not w then M.clear()
            elseif w.world_id==M.world and (not guide.at or guide.at.x~=w.x or guide.at.z~=w.z) then
                guide.at={x=w.x,z=w.z};M.choose(guide,true)
            end
        end
    end
end
local function observe_state()
    local p,map=eqoa.player.get(),eqoa.map.get()
    if not p or not map then M.in_world=false;M.live={};M.group={};M.places={};M.refresh=false;return end
    M.in_world=true
    if M.world~=map.world_id then M.rows_stale=true;M.dirty=true end
    M.world=map.world_id;M.player=p.position or M.player;M.waypoint=map.waypoint
    M.live={}
    local entities=eqoa.entities.list()
    if entities then for _,e in ipairs(entities) do if live_valid(e) then
        M.live[#M.live+1]={name=e.name,key=e.name:lower(),world=e.world_id,x=e.position.x,z=e.position.z,level=e.level,id=e.id}
    end end end
    local signature={}
    for _,e in ipairs(M.live) do signature[#signature+1]=e.id..":"..e.name..":"..e.world..":"..(e.level or 0) end
    signature=table.concat(signature,"|")
    if signature~=M.live_signature then M.rows_stale=true;M.dirty=true;M.live_signature=signature end
    local group=eqoa.group.get();M.group={}
    if group and group.members then for _,g in ipairs(group.members) do if g.id~=p.id then M.group[#M.group+1]=g end end end
    if M.places_world~=M.world then
    M.places_world=M.world;M.places={}
    local worlds={0,1,2,3};if M.world>3 then worlds[#worlds+1]=M.world end
    for _,world in ipairs(worlds) do
        local marks=eqoa.map.markers(world)
        if marks then for _,m in ipairs(marks) do if m.kind=="camp" or m.kind=="coach" then M.places[#M.places+1]={name=m.label or m.name,x=m.x,z=m.z,world=world} end end end
    end
    end
    follow_guide()
    M.refresh=false
    -- A query owns its snapshot and order until the next query/tab/open.
    -- Movement refreshes positions without invalidating an unfinished build.
    local nearest={}
    for _,e in ipairs(M.live) do if e.world==M.world and (not nearest[e.key] or distance(e)<distance(nearest[e.key])) then nearest[e.key]=e end end
    for key,e in pairs(nearest) do local r=M.row_by_key and M.row_by_key[key];if r then r.at=e end end
    if M.guide and M.guide.kind=="group" then
        local found=false;for _,g in ipairs(M.group) do if g.id==M.guide.id then found=true end end
        if not found then M.clear() end
    end
end
-- With a guide set and the panel closed only the guide's own target is read: the player, the map's
-- waypoint, the group for a member, and the creatures of the guide's name (`entities.named`), never the
-- whole entity list (a record per creature, rebuilt whenever one moves).
local function observe_guide()
    local guide=M.guide
    local p,map=eqoa.player.get(),eqoa.map.get()
    if not p or not map then M.in_world=false;M.live={};M.group={};M.refresh=false;return end
    M.in_world=true
    M.world=map.world_id;M.player=p.position or M.player;M.waypoint=map.waypoint
    if guide.kind=="npc" then
        M.live={}
        local named=eqoa.entities.named(guide.name,true)
        if named then for _,e in ipairs(named) do if live_valid(e) then
            M.live[#M.live+1]={name=e.name,key=e.name:lower(),world=e.world_id,x=e.position.x,z=e.position.z,level=e.level,id=e.id}
        end end end
    elseif guide.kind=="group" then
        local group=eqoa.group.get();M.group={}
        if group and group.members then for _,g in ipairs(group.members) do if g.id~=p.id then M.group[#M.group+1]=g end end end
    end
    follow_guide()
    M.refresh=false
    if M.guide and M.guide.kind=="group" then
        local found=false;for _,g in ipairs(M.group) do if g.id==M.guide.id then found=true end end
        if not found then M.clear() end
    end
end
function M.observe() if M.open then observe_state() end end
function M.clear() M.guide=nil;eqoa.map.clear_guide() end
function M.toggle() M.message=nil;M.message_until=nil;M.open=not M.open;M.build=nil;M.last_query=nil;M.enter_pending=false;if M.open then M.rows_stale=true;M.refresh=true end;M.selected=1;M.offset=1;M.dirty=true;eqoa.input.capture_text(M.open) end
function M.close() M.open=false;M.build=nil;M.enter_pending=false;eqoa.input.capture_text(false) end
local function sort_rows(rows)
    -- Compute distances once. Recomputing square roots inside every comparison
    -- exhausted the VM instruction budget on the unfiltered catalogue.
    for _,r in ipairs(rows) do r.sort_distance=r.at and distance(r.at) or math.huge end
    table.sort(rows,function(a,b)
        if a.sort_distance~=b.sort_distance then return a.sort_distance<b.sort_distance end
        return a.name<b.name
    end)
end
-- The creature search keeps two things apart so typing is never behind a scan of the catalogue:
--  * a STATIC name index (M.npc_cache: one entry per creature name with its lowercase searchable text and journal levels), built
--    once in the background after the catalogue or the journal loads (index_step, a slice a frame) and reused by every query and
--    every opening of the panel; it is not rebuilt when creatures come or go;
--  * the per-query rows, made only for the names a query matches (decorate: the nearest live creature or recorded spawn, the
--    count and the levels), in slices with a point budget, the first rows shown as they are made. A query that only adds letters
--    to the last finished one filters that one's rows (no new scan).
-- Per-frame work is bounded by these slices, so no frame runs long however large the catalogue is.
local INDEX_SLICE,FILTER_SLICE,DECORATE_POINTS=1500,5000,4000
local EMPTY={}
local function index_entry(key)
    local cat,jr=M.by_name.catalog[key],M.by_name.journal[key]
    local first=(cat and cat[1] and cat) or (jr and jr[1] and jr);if not first then return nil end
    local name=first.name
    local parts={name:lower()}
    local lo,hi
    if jr then for _,p in ipairs(jr) do
        parts[#parts+1]=spawn_text(key,p.level,p.world,p.x,p.z)
        if p.level and p.level>0 then lo=math.min(lo or p.level,p.level);hi=math.max(hi or p.level,p.level) end
    end end
    return {name=name,key=key,searchable=table.concat(parts," "),lo=lo,hi=hi}
end
local function index_step()
    -- not while a load is still being read; and the old index goes first, so two never sit in memory together
    if M.index_stale and #M.jobs==0 then M.index_stale=false;M.npc_cache=nil;M.done_rows=nil;M.index_job={phase=1,key=nil,out={},seen={}};M.index_count=(M.index_count or 0)+1 end
    local j=M.index_job;if not j or #M.jobs>0 then return end
    local n=0
    while n<INDEX_SLICE do
        local src=j.phase==1 and M.by_name.catalog or M.by_name.journal
        local k=next(src,j.key)
        if k==nil then
            if j.phase==1 then j.phase=2;j.key=nil
            else M.npc_cache=j.out;M.index_job=nil;M.rows_stale=true;if M.want_index then M.want_index=false;M.dirty=true end;return end
        else
            j.key=k;n=n+1
            if not j.seen[k] then j.seen[k]=true;local e=index_entry(k);if e then j.out[#j.out+1]=e end end
        end
    end
end
-- The live creatures of this world by name, and the names no catalogue or journal knows.
local function live_index()
    local by,only,seen={},{},{}
    for _,e in ipairs(M.live) do if e.world==M.world then
        local l=by[e.key];if not l then l={};by[e.key]=l end;l[#l+1]=e
        if not seen[e.key] and not M.by_name.catalog[e.key] and not M.by_name.journal[e.key] then
            seen[e.key]=true;only[#only+1]={name=e.name,key=e.key,searchable=e.name:lower()}
        end
    end end
    M.live_by_key,M.live_only=by,only
end
-- One row for a matched name; returns it and the number of points looked at (the slice's cost).
local function decorate(s)
    local r={name=s.name,kind="npc",key=s.key,count=0,searchable=s.searchable,lo=s.lo,hi=s.hi}
    local cost=1
    local live=M.live_by_key[s.key]
    if live then
        local nearest
        for _,p in ipairs(live) do
            r.count=r.count+1
            if not nearest or distance(p)<distance(nearest) then nearest=p end
            if p.level and p.level>0 then r.lo=math.min(r.lo or p.level,p.level);r.hi=math.max(r.hi or p.level,p.level) end
        end
        r.at=nearest;cost=cost+#live
    else
        local best,bd
        for _,list in ipairs({M.by_name.catalog[s.key] or EMPTY,M.by_name.journal[s.key] or EMPTY}) do
            cost=cost+#list
            for _,p in ipairs(list) do if p.world==M.world then
                local d=distance(p);if not bd or d<bd then best,bd=p,d end
            end end
        end
        r.at=best
    end
    local level=not r.lo and "level unknown" or (r.lo==r.hi and "level "..r.lo or "levels "..r.lo.."-"..r.hi)
    if r.count>0 then r.detail="Spawned nearby: "..r.count.."; "..level
    elseif r.at then r.detail="Not detected (range/spawn unknown)" else r.detail="Other world; not detected" end
    return r,cost
end
local function build_start()
    local fresh=not (M.last_tab==M.tab and M.last_query==M.query)
    local b={tab=M.tab,words=words(),rows={},matched={},index=1,fresh=fresh}
    if M.tab==1 then
        local reuse=M.last_tab==1 and M.last_query and M.done_rows and not M.rows_stale and M.query:sub(1,#M.last_query)==M.last_query
        if reuse then b.stage="reuse";b.candidates=M.done_rows
        else live_index();b.stage="filter";b.sources={M.npc_cache,M.live_only};b.src=1 end
    end
    M.build=b
    M.dirty=false
end
-- One bounded slice of the creature search; true when its rows are complete.
local function creature_step(b)
    if b.stage=="reuse" then
        local c=b.candidates
        for _=1,FILTER_SLICE do
            local r=c[b.index];if not r then break end
            b.index=b.index+1
            if matches(r.searchable,b.words,true) then b.rows[#b.rows+1]=r end
        end
        return b.index>#c
    elseif b.stage=="filter" then
        local n=0
        while n<FILTER_SLICE do
            local src=b.sources[b.src]
            if not src then b.stage="decorate";b.index=1;return false end
            local s=src[b.index]
            if not s then b.src=b.src+1;b.index=1 else
                b.index=b.index+1;n=n+1
                if matches(s.searchable,b.words,true) then b.matched[#b.matched+1]=s end
            end
        end
        return false
    end
    local budget=DECORATE_POINTS
    while budget>0 do
        local s=b.matched[b.index];if not s then return true end
        b.index=b.index+1
        local r,cost=decorate(s);budget=budget-cost
        b.rows[#b.rows+1]=r
    end
    if b.index>#b.matched then return true end
    -- a typed query shows its first rows at once, the rest follow (sorted by distance when all are made)
    if b.fresh and #b.rows>0 then M.rows=b.rows end
    return false
end
local function other_rows(b)
    if M.tab==2 then
        for _,g in ipairs(M.group) do if matches(g.name,b.words) then
            local p=g.position
            local at=g.active and g.world_id==M.world and p and valid(p.x,p.z) and {x=p.x,z=p.z} or nil
            b.rows[#b.rows+1]={name=g.name,kind="group",id=g.id,at=at,detail=at and "Group position" or (g.world_id and g.world_id~=M.world and "In another world" or "Position unavailable")}
        end end
    else
        local w=M.waypoint
        if w and w.world_id==M.world and matches("Personal waypoint",b.words) then b.rows[#b.rows+1]={name="Personal waypoint",kind="waypoint",at={x=w.x,z=w.z},detail="Your map pin / select to guide"} end
        for _,p in ipairs(M.places) do
            local name=p.name
            if name and matches(name,b.words) then b.rows[#b.rows+1]={name=name,kind="place",at=p.world==M.world and {x=p.x,z=p.z} or nil,detail=p.world==M.world and "Known location" or "In another world"} end
        end
    end
end
local function build_step()
    if M.dirty then
        -- the creature list needs its name index; until it is ready the panel says "Searching..."
        if M.tab==1 and not M.npc_cache then
            -- wait only while an index is really coming; with no catalogue and no journal there is none, and the live creatures are still searched
            if #M.jobs>0 or M.index_job or M.index_stale or (M.loads_pending and M.loads_pending()) then M.want_index=true;return end
            M.npc_cache={}
        end
        build_start()
    end
    local b=M.build;if not b then return end
    if M.tab==1 then
        if not creature_step(b) then return end
        if b.stage=="decorate" then
            sort_rows(b.rows)
            M.row_by_key={};for _,r in ipairs(b.rows) do M.row_by_key[r.key]=r end
            M.rows_stale=false
        end
        M.done_rows=b.rows;M.last_query=M.query
    else other_rows(b);sort_rows(b.rows) end
    M.last_tab=M.tab;M.rows=b.rows;M.build=nil;M.selected=math.min(M.selected,math.max(1,#M.rows))
    M.offset=math.max(1,math.min(M.offset,M.selected,math.max(1,#M.rows-5)))
    if M.enter_pending then M.enter_pending=false;M.choose(M.rows[M.selected]) end
end
function M.choose(r,background)
    if not r or (not r.at and not background) then return end
    local world=M.world
    local spec={world_id=world,label=r.name,dots={}}
    if r.kind=="npc" then
        local nearest
        for _,p in ipairs(M.live) do if p.world==M.world and p.key==r.key and (not nearest or distance(p)<distance(nearest)) then nearest=p end end
        local known
        for _,source in ipairs({M.by_name.catalog[r.key] or {},M.by_name.journal[r.key] or {}}) do for _,p in ipairs(source) do
            if p.world==M.world then
                if #spec.dots<80000 then spec.dots[#spec.dots+1]={x=p.x,z=p.z} end
                if not nearest and (not known or distance(p)<distance(known)) then known=p end
            end
        end end
        if nearest then spec.entity_id=nearest.id;r.live_id=nearest.id
        elseif known then r.live_id=nil;spec.x=known.x;spec.z=known.z;r.at=known
        else r.live_id=nil;eqoa.map.clear_guide();M.guide=r;r.at=nil;return end
    elseif r.kind=="group" then spec.entity_id=r.id else spec.x=r.at.x;spec.z=r.at.z end
    if eqoa.map.set_guide(spec) then r.world=world;r.last_player={x=M.player.x,z=M.player.z};M.guide=r;if not background then M.close() end else M.clear() end
end
function M.key(d)
    if not M.open or not d.down or d.synthetic then return end
    M.message=nil;M.message_until=nil
    local k=d.key
    if k=="escape" then M.close()
    elseif k=="tab" then M.tab=M.tab%3+1;M.selected=1;M.offset=1;M.dirty=true
    elseif k=="up" then M.selected=math.max(1,M.selected-1);M.offset=math.min(M.offset,M.selected)
    elseif k=="down" then M.selected=math.min(math.max(1,#M.rows),M.selected+1);M.offset=math.max(M.offset,M.selected-5)
    elseif k=="enter" then if M.dirty or M.build then M.enter_pending=true else M.choose(M.rows[M.selected]) end
    elseif k=="backspace" then
        M.query=M.query:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or "";M.selected=1;M.offset=1;M.dirty=true
    elseif d.text then
        for c in d.text:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
            local _,n=M.query:gsub("[^\128-\191]","")
            if n>=80 then break end
            if not c:find("[%z\1-\31\127]") then M.query=M.query..c end
        end
        M.selected=1;M.offset=1;M.dirty=true
    end
end
function M.status()
    local r=M.guide;if not r then return end
    if M.in_world==false or (r.world and r.world~=M.world) then return "Unavailable here: "..r.name end
    local at=r.at;local kind=r.kind=="group" and "Group" or r.kind=="waypoint" and "Waypoint" or "Known"
    if r.kind=="npc" then
        local nearest;for _,p in ipairs(M.live) do if p.world==M.world and p.key==r.key and (not nearest or distance(p)<distance(nearest)) then nearest=p end end
        if nearest then at=nearest;kind="Live" end
    elseif r.kind=="group" then
        at=nil;for _,g in ipairs(M.group) do if g.id==r.id and g.world_id==M.world then at=g.position end end
    end
    return at and string.format("%s %.0f: %s",kind,distance(at),r.name) or "Unavailable here: "..r.name
end
-- The open panel re-reads the world when something changed and at least this often (seconds).
local OPEN_REFRESH=0.25
function M.step()
    load_step()
    index_step()
    local now=M.now or 0
    if M.open then
        if M.refresh or now>=(M.next_observe or 0) then M.next_observe=now+OPEN_REFRESH;M.observe() end
    elseif M.guide then observe_guide() end
    if M.open then eqoa.input.capture_text(true);build_step() end
end
return M
