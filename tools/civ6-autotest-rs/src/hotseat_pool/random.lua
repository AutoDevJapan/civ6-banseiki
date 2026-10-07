if poolMajors~=4 then error('random pool majors '..poolMajors) end
local poolLeaders={}
for pid=0,3 do
  local c=PlayerConfigurations[pid]; local p=Players[pid]
  local leader=c and c:GetLeaderTypeName()
  if not p or type(leader)~='string' or leader=='RANDOM' or not GameInfo.Leaders[leader] or p:IsHuman()~=(pid<2) then error('random pool player '..pid..' '..tostring(leader)) end
  if poolLeaders[leader] then error('random pool duplicate leader '..leader) end
  poolLeaders[leader]=true
  if pid<2 then poolParts[#poolParts+1]=poolPerson(pid,leader)
  else poolParts[#poolParts+1]=table.concat({pid,leader,tostring(c:GetCivilizationTypeName())},'~') end
end
