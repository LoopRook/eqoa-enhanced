-- Native search rules and navigation, independent of its presentation.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local M = {open=false,query="",tab=1,selected=1,offset=1,rows={},catalog={},journal={},jobs={},dirty=true,refresh=true,world=0,player={x=0,z=0},live={},group={},places={}}
local function valid(x,z) return type(x)=="number" and type(z)=="number" and x==x and z==z and math.abs(x)<10000000 and math.abs(z)<10000000 end
local function distance(p) local x,z=p.x-M.player.x,p.z-M.player.z;return math.sqrt(x*x+z*z) end
local function matches(s,words) s=s:lower();for _,w in ipairs(words) do if not s:find(w,1,true) then return false end end;return true end
local function words() local out={};for w in M.query:lower():gmatch("%S+") do out[#out+1]=w end;return out end
local function spawn(out,name,world,p,level)
    if type(name)~="string" or name:match("^%s*$") or type(world)~="number" or world%1~=0 or world<0 or world>2147483647 or type(p)~="table" or not valid(p[1],p[3]) then return end
    out[#out+1]={name=name,key=name:lower(),world=world,x=p[1],z=p[3],level=level}
end
function M.load(value,kind)
    if type(value)~="table" then return end
    M.jobs[#M.jobs+1]={value=value,kind=kind,out={},name=nil,row=nil,index=0,id=nil}
end
local function load_step()
    local j=M.jobs[1];if not j then return end
    local done=false
    for _=1,500 do
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
    end
    if done then
        M[j.kind]=j.out
        if j.kind=="journal" then local names={};local n=0;for _,p in ipairs(j.out) do if not names[p.name] then names[p.name]=true;n=n+1 end end;M.journal_names=n end
        table.remove(M.jobs,1);M.dirty=true
    end
end
local function live_valid(e)
    return e.is_creature and not e.corpse and e.hp_fraction and e.hp_fraction>0 and e.search_flags==0 and e.class_code and e.class_code<2 and e.name and not e.name:match("^%s*$") and e.position and valid(e.position.x,e.position.z)
end
function M.observe()
    local p,map=eqoa.player.get(),eqoa.map.get()
    if not p or not map then M.live={};M.group={};M.places={};return end
    M.world=map.world_id;M.player=p.position or M.player;M.waypoint=map.waypoint
    M.live={}
    local entities=eqoa.entities.list()
    if entities then for _,e in ipairs(entities) do if live_valid(e) then
        M.live[#M.live+1]={name=e.name,key=e.name:lower(),world=e.world_id,x=e.position.x,z=e.position.z,level=e.level,id=e.id}
    end end end
    local group=eqoa.group.get();M.group={}
    if group and group.members then for _,g in ipairs(group.members) do if g.id~=p.id then M.group[#M.group+1]=g end end end
    M.places={}
    local worlds={0,1,2,3};if M.world>3 then worlds[#worlds+1]=M.world end
    for _,world in ipairs(worlds) do
        local marks=eqoa.map.markers(world)
        if marks then for _,m in ipairs(marks) do if m.kind=="camp" or m.kind=="coach" then M.places[#M.places+1]={name=m.label or m.name,x=m.x,z=m.z,world=world} end end end
    end
    M.refresh=false;M.dirty=true
    if M.guide and M.guide.kind=="group" then
        local found=false;for _,g in ipairs(M.group) do if g.id==M.guide.id then found=true end end
        if not found then M.clear() end
    end
end
function M.clear() M.guide=nil;eqoa.map.clear_guide() end
function M.toggle() M.open=not M.open;M.selected=1;M.offset=1;M.dirty=true;eqoa.input.capture_text(M.open) end
function M.close() M.open=false;eqoa.input.capture_text(false) end
local function sort_rows(rows)
    -- Compute distances once. Recomputing square roots inside every comparison
    -- exhausted the VM instruction budget on the unfiltered catalogue.
    for _,r in ipairs(rows) do r.sort_distance=r.at and distance(r.at) or math.huge end
    table.sort(rows,function(a,b)
        if a.sort_distance~=b.sort_distance then return a.sort_distance<b.sort_distance end
        return a.name<b.name
    end)
end
local world_names={[0]="tunaria",[1]="odus",[2]="plane of sky",[3]="lavastorm"}
local function build_start()
    M.build={words=words(),names={},source=1,index=1,rows={}}
    M.dirty=false
end
local function npc_step(b)
    local sources={M.catalog,M.journal,M.live}
    for _=1,1000 do
        local source=sources[b.source]
        if not source then return true end
        local p=source[b.index]
        if not p then b.source=b.source+1;b.index=1 else
            b.index=b.index+1
            local searchable=p.name
            if b.source==2 then searchable=searchable.." level "..tostring(p.level or 0).." "..(world_names[p.world] or "").." world "..p.world.." "..string.format("%.0f %.0f",p.x,p.z) end
            if (b.source~=3 or p.world==M.world) and (b.source==2 or matches(searchable,b.words)) then
                local r=b.names[p.key]
                if not r then r={name=p.name,kind="npc",key=p.key,count=0};b.names[p.key]=r end
                if b.source~=2 or matches(p.name,b.words) then r.keep=true
                elseif not r.keep then
                    r.parts=r.parts or {p.name};r.parts[#r.parts+1]=searchable
                end
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
            local at=g.world_id==M.world and p and valid(p.x,p.z) and {x=p.x,z=p.z} or nil
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
        if not npc_step(b) then return end
        for _,r in pairs(b.names) do
            if r.keep or (r.parts and matches(table.concat(r.parts," "),b.words)) then
            if r.count>0 then
                local level=not r.lo and "level unknown" or (r.lo==r.hi and "level "..r.lo or "levels "..r.lo.."-"..r.hi)
                r.detail="Spawned nearby: "..r.count.."; "..level
            elseif r.at then r.detail="Not detected (range/spawn unknown)" else r.detail="Other world; not detected" end
            b.rows[#b.rows+1]=r
            end
        end
    else other_rows(b) end
    sort_rows(b.rows);M.rows=b.rows;M.build=nil;M.selected=math.min(M.selected,math.max(1,#M.rows))
    if M.enter_pending then M.enter_pending=false;M.choose(M.rows[M.selected]) end
end
function M.choose(r)
    if not r or not r.at then return end
    local spec={world_id=M.world,label=r.name,dots={}}
    if r.kind=="npc" then
        local nearest
        for _,p in ipairs(M.live) do if p.world==M.world and p.key==r.key and (not nearest or distance(p)<distance(nearest)) then nearest=p end end
        if nearest then spec.entity_id=nearest.id else
            local known
            for _,source in ipairs({M.catalog,M.journal}) do for _,p in ipairs(source) do if p.world==M.world and p.key==r.key and (not known or distance(p)<distance(known)) then known=p end end end
            if not known then eqoa.map.clear_guide();M.guide=r;M.close();return end
            spec.x=known.x;spec.z=known.z;r.at=known
        end
        for _,source in ipairs({M.catalog,M.journal}) do for _,p in ipairs(source) do if p.world==M.world and p.key==r.key and #spec.dots<80000 then spec.dots[#spec.dots+1]={x=p.x,z=p.z} end end end
    elseif r.kind=="group" then spec.entity_id=r.id else spec.x=r.at.x;spec.z=r.at.z end
    if eqoa.map.set_guide(spec) then M.guide=r;M.close() end
end
function M.key(d)
    if not M.open or not d.down or d.synthetic then return end
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
    if M.refresh then M.observe() end
    if M.open then eqoa.input.capture_text(true);build_step() end
end
return M
