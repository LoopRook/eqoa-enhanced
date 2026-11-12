-- Native search rules and navigation, independent of its presentation.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local M = {open=false,query="",tab=1,selected=1,offset=1,rows={},catalog={},journal={},jobs={},by_name={catalog={},journal={}},dirty=true,refresh=true,world=0,player={x=0,z=0},live={},group={},places={}}
local data=require("spawns")
local valid,spawn=data.valid,data.add
local function distance(p) local x,z=p.x-M.player.x,p.z-M.player.z;return math.sqrt(x*x+z*z) end
local function matches(s,words,lowered) if not lowered then s=s:lower() end;for _,w in ipairs(words) do if not s:find(w,1,true) then return false end end;return true end
local function words() local out={};for w in M.query:lower():gmatch("%S+") do out[#out+1]=w end;return out end
function M.load(value,kind)
    if type(value)~="table" then return end
    M.jobs[#M.jobs+1]={value=value,kind=kind,out={},name=nil,row=nil,index=0,id=nil,by_name={}}
end
local function load_step()
    local j=M.jobs[1];if not j then return end
    local done=false
    for _=1,500 do
        local before=#j.out
        if not j.row then
            j.name,j.row=next(j.value,j.name);j.index=0;j.id=nil
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
            local p=j.out[#j.out];local list=j.by_name[p.key]
            if not list then list={};j.by_name[p.key]=list end
            list[#list+1]=p
        end
    end
    if done then
        M.by_name[j.kind]=j.by_name;M.npc_cache=nil;M.build=nil
        M[j.kind]=j.out
        if j.kind=="journal" then local names={};local n=0;for _,p in ipairs(j.out) do if not names[p.name] then names[p.name]=true;n=n+1 end end;M.journal_names=n end
        table.remove(M.jobs,1);M.dirty=true
    end
end
local function live_valid(e)
    return e.is_creature and not e.corpse and e.hp_fraction and e.hp_fraction>0 and e.search_flags==0 and e.class_code and e.class_code<2 and e.name and not e.name:match("^%s*$") and e.position and valid(e.position.x,e.position.z)
end
local function observe_state()
    local p,map=eqoa.player.get(),eqoa.map.get()
    if not p or not map then M.in_world=false;M.live={};M.group={};M.places={};M.refresh=false;return end
    M.in_world=true
    if M.world~=map.world_id then M.npc_cache=nil;M.dirty=true end
    M.world=map.world_id;M.player=p.position or M.player;M.waypoint=map.waypoint
    M.live={}
    local entities=eqoa.entities.list()
    if entities then for _,e in ipairs(entities) do if live_valid(e) then
        M.live[#M.live+1]={name=e.name,key=e.name:lower(),world=e.world_id,x=e.position.x,z=e.position.z,level=e.level,id=e.id}
    end end end
    local signature={}
    for _,e in ipairs(M.live) do signature[#signature+1]=e.id..":"..e.name..":"..e.world..":"..(e.level or 0) end
    signature=table.concat(signature,"|")
    if signature~=M.live_signature then M.npc_cache=nil;M.live_signature=signature end
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
function M.observe() if M.open then observe_state() end end
function M.clear() M.guide=nil;eqoa.map.clear_guide() end
function M.toggle() M.message=nil;M.message_until=nil;M.open=not M.open;M.build=nil;M.last_query=nil;M.enter_pending=false;if M.open then M.npc_cache=nil;M.refresh=true end;M.selected=1;M.offset=1;M.dirty=true;eqoa.input.capture_text(M.open) end
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
local function build_start()
    if M.tab==1 and M.build and not M.build.candidates and M.build.tab==1 then
        M.dirty=false;return
    end
    local candidates
    if M.tab==1 and M.npc_cache then
        candidates=M.last_tab==1 and M.last_query and M.query:sub(1,#M.last_query)==M.last_query and M.rows or M.npc_cache
    end
    M.build={tab=M.tab,words=words(),names={},source=1,index=1,rows={},sources={M.catalog,M.journal,M.live},candidates=candidates}
    M.dirty=false
end
local function npc_step(b)
    local sources=b.sources
    for _=1,1000 do
        local source=sources[b.source]
        if not source then return true end
        local p=source[b.index]
        if not p then b.source=b.source+1;b.index=1 else
            b.index=b.index+1
            local searchable=p.name
            if b.source==2 then searchable=p.searchable end
            if b.source~=3 or p.world==M.world then
                local r=b.names[p.key]
                if not r then r={name=p.name,kind="npc",key=p.key,count=0};b.names[p.key]=r end
                r.parts=r.parts or {p.name:lower()}
                if b.source==2 then r.parts[#r.parts+1]=searchable end
                if p.world==M.world and (not r.at or (b.source==3 and r.count==0) or ((b.source==3 or r.count==0) and distance(p)<distance(r.at))) then r.at=p end
                if b.source==3 then r.count=r.count+1 end
                if p.level and p.level>0 then r.lo=math.min(r.lo or p.level,p.level);r.hi=math.max(r.hi or p.level,p.level) end
            end
        end
    end
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
    if M.dirty then build_start() end
    local b=M.build;if not b then return end
    if M.tab==1 then
        if not b.candidates then
            if not npc_step(b) then return end
            for _,r in pairs(b.names) do
                if r.count>0 then
                    local level=not r.lo and "level unknown" or (r.lo==r.hi and "level "..r.lo or "levels "..r.lo.."-"..r.hi)
                    r.detail="Spawned nearby: "..r.count.."; "..level
                elseif r.at then r.detail="Not detected (range/spawn unknown)" else r.detail="Other world; not detected" end
                r.searchable=table.concat(r.parts," ");r.parts=nil
                b.rows[#b.rows+1]=r
            end
            sort_rows(b.rows);M.npc_cache=b.rows;M.row_by_key=b.names
            b.candidates=b.rows;b.rows={};b.index=1;b.words=words()
            return
        end
        for _=1,500 do
            local r=b.candidates[b.index];if not r then break end
            b.index=b.index+1
            if matches(r.searchable,b.words,true) then b.rows[#b.rows+1]=r end
        end
        if b.index<=#b.candidates then return end
        M.last_query=M.query
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
function M.step()
    load_step()
    if M.refresh then if M.open then M.observe() elseif M.guide then observe_state() end end
    if M.open then eqoa.input.capture_text(true);build_step() end
end
return M
