-- Map search entry: lifecycle, player input and the dark/red Space Mono panel.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local s=require("search")
local hits={}
local requests={}
local reload=false
local retry_at=0
local function load_journal()
    if requests.journal then reload=true;return end
    requests.journal=eqoa.files.load_json("@eqoa_creature_journal/data/journal.json")
    if not requests.journal then reload=true;retry_at=(s.now or 0)+1 end
end
requests.catalog=eqoa.files.load_json("package_resources/map-entities.json")
load_journal()
local font_info=eqoa.fonts.load("package_resources/SpaceMono-Regular.ttf",13)
local font=font_info and font_info.font
local function command(name)
    s.message=nil;s.message_until=nil
    local ok,err=eqoa.mods.send("eqoa_creature_journal",name)
    s.message=ok and nil or (err and err.message or "Journal unavailable");s.message_until=(s.now or 0)+6
end
eqoa.input.bind({id="open",label="Map search",default_key="f2"},function()
    local map=eqoa.map.get()
    if not map or map.enabled==false then return end
    if not map.open and not eqoa.hud.visible("minimap") then
        local ok=eqoa.map.open()
        if not ok then return end
        s.requested_open_until=(s.now or 0)+1
    end
    s.toggle()
end)
eqoa.input.bind({id="clear",label="Clear map guide",default_key="f4"},function() s.clear() end)
eqoa.events.on("input.key",function(e) s.key(e.data) end)
eqoa.events.on("mod.command",function(e)
    if e.data.from=="eqoa_creature_journal" and e.data.command=="journal_status" and e.data.args then
        s.message=e.data.args.message;s.message_until=e.time+6
    end
end)
for _,name in ipairs({"entity.added","entity.changed","entity.removed","group.changed","map.changed","player.changed","state.reset","events.dropped"}) do
    eqoa.events.on(name,function() s.refresh=true end)
end
eqoa.events.on("shared.changed",function(e)
    if e.data.mod=="eqoa_creature_journal" and e.data.path=="data/journal.json" then if s.open then load_journal() else reload=true end end
end)
eqoa.events.on("resource.ready",function(e)
    local d=e.data
    if d.kind=="json" then
        for _,kind in ipairs({"catalog","journal"}) do if d.request==requests[kind] then requests[kind]=nil;s.load(d.value,kind) end end
        if reload and s.open and not requests.journal then reload=false;load_journal() end
    end
end)
eqoa.events.on("resource.failed",function(e)
    local d=e.data
    if d.kind=="json" then for _,kind in ipairs({"catalog","journal"}) do if d.request==requests[kind] then requests[kind]=nil;if kind=="journal" then reload=true;retry_at=(s.now or 0)+1 end end end end
end)
eqoa.events.on("guide.lost",function(e)
    if s.guide then s.refresh=true end
end)
eqoa.events.on("disabling",function() s.close();s.clear() end)
local function inside(d,r) return d.x>=r.x and d.x<=r.x+r.w and d.y>=r.y and d.y<=r.y+r.h end
eqoa.events.on("pointer.pressed",function(e)
    local d=e.data;if not d.inside_window then return end
    if d.button=="right" and s.open then s.close();return end
    for i=#hits,1,-1 do local h=hits[i];if inside(d,h) then h.click();return end end
    if s.open then s.close() end
end)
eqoa.events.on("pointer.wheel",function(e)
    if s.open then
        s.selected=math.max(1,math.min(#s.rows,s.selected-(e.data.steps or 0)));s.offset=math.max(1,s.selected-5)
    end
end)
local function draw()
    hits={}
    local map=eqoa.map.get();if not map or map.enabled==false then s.close();return end
    local mini=not map.open and eqoa.hud.visible("minimap")
    if not mini and not map.open then
        if s.open and (not s.requested_open_until or (s.now or 0)>s.requested_open_until) then s.close() end
        return
    end
    s.requested_open_until=nil
    if not eqoa.imgui then return end
    local ui=eqoa.imgui
    local w,h=ui.GetDisplaySize()
    local layout=eqoa.hud.layout();local m=layout.minimap
    local x,y=map.open and 16 or m.x+m.width-20,map.open and 44 or m.y+m.height+2
    local pw,ph=math.min(336,w-24),math.min(400,h-72)
    local px=map.open and 16 or math.max(12,m.x-pw-8)
    local py=math.max(12,math.min(map.open and 70 or m.y,h-ph-12))
    ui.SetNextWindowPos(0,0);ui.SetNextWindowSize(w,h)
    local flags=ImGuiWindowFlags.NoTitleBar+ImGuiWindowFlags.NoBackground+ImGuiWindowFlags.NoMove+ImGuiWindowFlags.NoResize+ImGuiWindowFlags.NoScrollbar+ImGuiWindowFlags.NoSavedSettings+ImGuiWindowFlags.NoInputs
    ui.Begin("eqoa_map_search_layer",nil,flags)
    if font then ui.PushFont(font) end
    local dl=ui.GetWindowDrawList()
    local v=ImVec2.new
    local c={text=ui.GetColorU32(.91,.91,.91,1),muted=ui.GetColorU32(.6,.6,.6,1),surface=ui.GetColorU32(.067,.067,.067,1),raised=ui.GetColorU32(.102,.102,.102,1),border=ui.GetColorU32(.2,.2,.2,1),red=ui.GetColorU32(.843,.098,.129,1),selected=ui.GetColorU32(.17,.067,.08,1)}
    local function fill(x,y,w,h,color) dl:AddRectFilled(v(x,y),v(x+w,y+h),color) end
    local function text(x,y,value,color,maxw)
        value=tostring(value)
        while #value>0 and maxw do local tw=ui.CalcTextSize(value);if tw<=maxw then break end;value=value:match("^(.*)[%z\1-\127\194-\244][\128-\191]*$") or "" end
        dl:AddText(v(x,y),color or c.text,value)
    end
    local function hit(x,y,w,h,click)
        local r={x=x,y=y,w=w,h=h,click=click};hits[#hits+1]=r
        eqoa.input.claim_pointer({x=x,y=y,width=w,height=h})
    end
    local function button(x,y,w,h,label,click,active)
        fill(x,y,w,h,active and c.selected or c.raised);dl:AddRect(v(x,y),v(x+w,y+h),active and c.red or c.border)
        text(x+6,y+4,label,c.text,w-10);hit(x,y,w,h,click)
    end
    button(x,y,mini and 20 or 100,20,mini and "" or ("Search  "..((eqoa.binds.get("open") or ""):upper())),s.toggle,s.open)
    if mini then dl:AddCircle(v(x+8,y+8),5,c.text,24,1);dl:AddLine(v(x+12,y+12),v(x+17,y+17),c.text,1) end
    if s.guide then button(x-24,y,20,20,"X",s.clear) end
    local status=s.status()
    if status then text(math.max(12,x-300),y+25,status,c.text,320) end
    if s.open then
        fill(px,py,pw,ph,c.surface);dl:AddRect(v(px,py),v(px+pw,py+ph),c.border)
        hit(px,py,pw,ph,function() end)
        text(px+12,py+12,"FIND ON MAP",c.muted)
        button(px+pw-28,py+5,22,22,"X",s.close)
        fill(px+10,py+36,pw-20,28,ui.GetColorU32(0,0,0,1));dl:AddRect(v(px+10,py+36),v(px+pw-10,py+64),c.red)
        text(px+17,py+42,s.query=="" and ({"NPC name...","Group member...","Camp or coach..."})[s.tab] or s.query.."_",s.query=="" and c.muted or c.text,pw-34)
        for i,label in ipairs({"Creatures","Group","Places"}) do local tab=i;local tx=px+10+(i-1)*(pw-20)/3
            button(tx,py+73,(pw-20)/3-3,25,label,function() s.tab=tab;s.selected=1;s.offset=1;s.dirty=true end,s.tab==i)
        end
        local n=math.min(6,math.max(1,math.floor((ph-175)/34)))
        for i=s.offset,math.min(#s.rows,s.offset+n-1) do
            local r=s.rows[i];local yy=py+108+(i-s.offset)*34
            if i==s.selected then fill(px+7,yy-2,pw-14,32,c.selected);fill(px+7,yy-2,2,32,c.red) end
            text(px+16,yy,r.name,r.at and c.text or c.muted,pw-32);text(px+16,yy+15,r.detail,c.muted,pw-32)
            if r.at then hit(px+7,yy-2,pw-14,32,function() s.choose(r) end) end
        end
        if #s.rows==0 then text(px+16,py+116,s.build and "Searching..." or s.tab==2 and "No group members" or (#s.catalog==0 and s.tab==1 and "NPC list unavailable: reinstall from the ZIP" or "No matching locations"),c.muted,pw-32) end
        local foot=py+ph-58
        fill(px+10,foot-6,pw-20,1,c.border)
        text(px+12,foot,(s.message and (not s.message_until or s.message_until>s.now) and s.message) or (#s.rows.." matches / Enter guides"),c.muted,pw-24)
        text(px+12,foot+16,"Journal: "..(s.journal_names or 0).." creatures, "..#s.journal.." spawns",c.muted,pw-24)
        button(px+10,py+ph-25,45,21,"Prev",function() s.offset=math.max(1,s.offset-6);s.selected=s.offset end)
        button(px+59,py+ph-25,82,21,"Export",function() command("export_journal") end)
        button(px+145,py+ph-25,82,21,"Import",function() command("import_journal") end)
        button(px+pw-55,py+ph-25,45,21,"Next",function() s.offset=math.min(math.max(1,#s.rows),s.offset+6);s.selected=s.offset end)
    end
    if font then ui.PopFont() end
    ui.End()
end
eqoa.events.on("render",function(e)
    s.now=e.time
    if s.message_until and e.time>=s.message_until then s.message=nil;s.message_until=nil end
    if s.open and reload and e.time>=retry_at and not requests.journal then reload=false;load_journal() end
    s.step()
    draw()
end)
