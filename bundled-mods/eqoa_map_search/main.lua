-- Map search entry: lifecycle, player input and the dark/red Space Mono panel.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local s=require("search")
local hits={}
local requests={}
-- The Journal may still be starting when Map Search first renders.
local reload=true
local owner_pending=true
local retry_at=0
local function load_journal()
    local request,err=eqoa.files.load_json("@eqoa_creature_journal/data/journal.json")
    requests.journal=request
    owner_pending=err and err.code=="not_yet"
    if not request then reload=true;retry_at=(s.now or 0)+1 end
end
requests.catalog=eqoa.files.load_json("package_resources/map-entities.json")
-- the name index waits while a file is still being read; a read that failed leaves nothing to wait for
s.loads_pending=function() return next(requests)~=nil end
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
    if not map.open and not (map.minimap or eqoa.hud.visible("minimap")) then
        local ok=eqoa.map.open()
        if not ok then return end
        s.requested_open_until=(s.now or 0)+1
    end
    s.toggle()
end)
-- no default key: F4 is the game's Stat Window; a player can give this a key on Controls (the button clears too)
eqoa.input.bind({id="clear",label="Clear map guide",default_key=""},function() s.clear() end)
eqoa.events.on("input.key",function(e) s.key(e.data) end)
eqoa.events.on("mod.command",function(e)
    if e.data.from=="eqoa_creature_journal" and e.data.command=="journal_status" and e.data.args then
        s.message=e.data.args.message;s.message_until=e.time+6
    end
end)
-- Not the entity or player streams: a record per creature per event; the panel reads on a timer instead.
for _,name in ipairs({"group.changed","map.changed","state.reset","events.dropped"}) do
    eqoa.events.on(name,function() s.refresh=true end)
end
eqoa.events.on("shared.changed",function(e)
    if e.data.mod=="eqoa_creature_journal" and e.data.path=="data/journal.json" then reload=true end
end)
eqoa.events.on("resource.ready",function(e)
    local d=e.data
    if d.kind=="json" then
        for _,kind in ipairs({"catalog","journal"}) do if d.request==requests[kind] then requests[kind]=nil;s.load(d.value,kind) end end
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
    if s.open and s.list_rect and inside(e.data,s.list_rect) then
        local steps=e.data.steps or 0
        if steps~=0 then s.selected=math.max(1,math.min(#s.rows,s.selected+(steps>0 and -1 or 1)));s.offset=math.max(1,s.selected-5) end
    end
end)
local function draw()
    hits={}
    s.panel_rect=nil;s.list_rect=nil
    local map=eqoa.map.get();if not map or map.enabled==false then s.close();return end
    local mini=not map.open and (map.minimap or eqoa.hud.visible("minimap"))
    if not mini and not map.open then
        if s.open and (not s.requested_open_until or (s.now or 0)>s.requested_open_until) then s.close() end
        return
    end
    s.requested_open_until=nil
    if not eqoa.imgui then return end
    local ui=eqoa.imgui
    local w,h=ui.GetDisplaySize()
    local layout=eqoa.hud.layout();local m=layout.minimap
    -- sizes are the native search's, in HUD frame pixels, scaled as the HUD is: k window pixels a frame pixel
    local k=math.max(m.height/112,0.5)
    local anchor=map.search_anchor
    if map.open and not anchor then return end
    local bw=mini and 20*k or 100*k
    local x,y=map.open and anchor.x or m.x+m.width-20*k,map.open and anchor.y or m.y+m.height+2*k
    local pw=math.min(252*k,w-16*k)
    local ph=math.min(300*k,map.open and anchor.height-26*k or h-70*k)
    local px=map.open and 8*k+8 or math.max(8*k,m.x-pw-6*k)
    local py=math.max(8*k,math.min(map.open and y+24*k or m.y,h-ph-8*k))
    local rows=math.max(1,math.min(6,math.floor((ph/k-146)/28)))
    s.panel_rect={x=px,y=py,w=pw,h=ph};s.button_rect={x=x,y=y,w=bw,h=20*k};s.scale=k
    s.list_rect={x=px+6*k,y=py+92*k,w=pw-12*k,h=rows*28*k}
    ui.SetNextWindowPos(0,0);ui.SetNextWindowSize(w,h)
    local flags=ImGuiWindowFlags.NoTitleBar+ImGuiWindowFlags.NoBackground+ImGuiWindowFlags.NoMove+ImGuiWindowFlags.NoResize+ImGuiWindowFlags.NoScrollbar+ImGuiWindowFlags.NoSavedSettings+ImGuiWindowFlags.NoInputs
    ui.Begin("eqoa_map_search_layer",nil,flags)
    if font then ui.PushFont(font) end
    ui.SetWindowFontScale(k*12/13) -- the native font's 12 frame pixels, from the 13 pixel face
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
        local r={x=x,y=y,w=w,h=h,click=function() s.message=nil;s.message_until=nil;click() end};hits[#hits+1]=r
        eqoa.input.claim_pointer({x=x,y=y,width=w,height=h})
    end
    local function button(x,y,w,h,label,click,active)
        fill(x,y,w,h,active and c.selected or c.raised);dl:AddRect(v(x,y),v(x+w,y+h),active and c.red or c.border)
        text(x+6*k,y+3*k,label,c.text,w-12*k);hit(x,y,w,h,click)
    end
    button(x,y,bw,20*k,mini and "" or ("Search  "..((eqoa.binds.get("open") or ""):upper())),s.toggle,s.open)
    if mini then dl:AddCircle(v(x+8*k,y+8*k),5*k,c.text,24,math.max(1,k));dl:AddLine(v(x+12*k,y+12*k),v(x+17*k,y+17*k),c.text,math.max(1,k)) end
    if s.guide then
        if mini then button(x-23*k,y,20*k,20*k,"X",s.clear)
        else local label="Stop  "..((eqoa.binds.get("clear") or ""):upper());button(x+bw+3*k,y,ui.CalcTextSize(label)+12*k,20*k,label,s.clear) end
    end
    local status=s.status()
    if status then text(math.max(12,x-300*k),y+23*k,status,c.text,320*k) end
    if map.open and map.waypoint then
        button(x+bw+(s.guide and 90*k or 4*k),y,110*k,20*k,"Track waypoint",function()
            s.choose({name="Personal waypoint",kind="waypoint",at={x=map.waypoint.x,z=map.waypoint.z}})
        end)
    end
    if s.open then
        eqoa.input.claim_pointer({x=0,y=0,width=w,height=h})
        fill(px,py,pw,ph,c.surface);dl:AddRect(v(px,py),v(px+pw,py+ph),c.border)
        hit(px,py,pw,ph,function() end)
        text(px+10*k,py+10*k,"FIND ON MAP",c.muted,pw-40*k)
        button(px+pw-25*k,py+5*k,20*k,20*k,"X",s.close)
        fill(px+8*k,py+32*k,pw-16*k,24*k,ui.GetColorU32(0,0,0,1));dl:AddRect(v(px+8*k,py+32*k),v(px+pw-8*k,py+56*k),c.red)
        text(px+15*k,py+37*k,s.query=="" and ({"NPC name...","Group member...","Camp or coach..."})[s.tab] or s.query.."_",s.query=="" and c.muted or c.text,pw-30*k)
        for i,label in ipairs({"Creatures","Group","Places"}) do local tab=i;local tx=px+8*k+(i-1)*(pw-16*k)/3
            local tw=(pw-16*k)/3-2*k
            text(tx+5*k,py+67*k,label,s.tab==i and c.text or c.muted,tw-10*k)
            if s.tab==i then fill(tx,py+83*k,tw,2*k,c.red) end
            hit(tx,py+64*k,tw,21*k,function() s.tab=tab;s.selected=1;s.offset=1;s.dirty=true end)
        end
        for i=s.offset,math.min(#s.rows,s.offset+rows-1) do
            local r=s.rows[i];local yy=py+94*k+(i-s.offset)*28*k
            if i==s.selected then fill(px+6*k,yy-2*k,pw-12*k,27*k,c.selected);fill(px+6*k,yy-2*k,2*k,27*k,c.red) end
            text(px+13*k,yy,r.name,r.at and c.text or c.muted,pw-26*k);text(px+13*k,yy+13*k,r.detail,c.muted,pw-26*k)
            if r.at then hit(px+6*k,yy-2*k,pw-12*k,27*k,function() s.choose(r) end) end
        end
        if #s.rows==0 then text(px+13*k,py+108*k,(s.build or s.dirty) and "Searching..." or s.tab==2 and "No group members" or (#s.catalog==0 and s.tab==1 and "NPC list unavailable: reinstall from the ZIP" or "No matching locations"),c.muted,pw-26*k) end
        local foot=py+ph-48*k
        fill(px+8*k,foot-7*k,pw-16*k,1,c.border)
        text(px+10*k,foot-2*k,(s.message and (not s.message_until or s.message_until>s.now) and s.message) or (#s.rows.." matches / Enter guides"),c.muted,pw-20*k)
        text(px+10*k,foot+11*k,"Journal: "..(s.journal_names or 0).." creatures, "..#s.journal.." spawns",c.muted,pw-20*k)
        local by=foot+25*k
        local bh=math.max(8*k,py+ph-3*k-by)
        local avail=pw/k-16
        local wp,we,wi,wn=44,58,58,44
        if avail<220 then local q=math.max(28,(avail-12)/4);wp,we,wi,wn=q,q,q,q end
        local bx=px+8*k
        button(bx,by,wp*k,bh,"Prev",function() s.offset=math.max(1,s.offset-rows);s.selected=s.offset end)
        button(bx+(wp+4)*k,by,we*k,bh,"Export",function() command("export_journal") end)
        button(bx+(wp+we+8)*k,by,wi*k,bh,"Import",function() command("import_journal") end)
        button(px+pw-8*k-wn*k,by,wn*k,bh,"Next",function() s.offset=math.min(math.max(1,#s.rows),s.offset+rows);s.selected=s.offset end)
    end
    ui.SetWindowFontScale(1)
    if font then ui.PopFont() end
    ui.End()
end
eqoa.events.on("render",function(e)
    s.now=e.time
    if s.message_until and e.time>=s.message_until then s.message=nil;s.message_until=nil end
    if (s.open or owner_pending) and reload and e.time>=retry_at and not requests.journal then reload=false;load_journal() end
    s.step()
    draw()
end)
