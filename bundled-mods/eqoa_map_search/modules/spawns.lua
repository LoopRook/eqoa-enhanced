-- Validated spawn records and cached journal search text.
-- SPDX-License-Identifier: AGPL-3.0-or-later
local M={}
local world_names={[0]="tunaria",[1]="odus",[2]="plane of sky",[3]="lavastorm"}
function M.valid(x,z) return type(x)=="number" and type(z)=="number" and x==x and z==z and math.abs(x)<10000000 and math.abs(z)<10000000 end
function M.add(out,name,world,p,level)
    if type(name)~="string" or name:match("^%s*$") or type(world)~="number" or world%1~=0 or world<0 or world>2147483647 or type(p)~="table" or not M.valid(p[1],p[3]) then return end
    local row={name=name,key=name:lower(),world=world,x=p[1],z=p[3],level=level}
    local wn=world_names[world] or ""
    row.searchable=(name.." level "..tostring(level or 0).." "..wn.." world "..world.." "..string.format("%.0f %.0f",row.x,row.z)):lower()
    out[#out+1]=row
end
return M
