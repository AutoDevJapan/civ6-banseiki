if poolMajors~=2 then error('fixed pool majors '..poolMajors) end
local poolExpected={[0]={'LEADER_JAC_ODA_NOBUHIDE','JAC_POOL_H040'},[1]={'LEADER_JAC_POOL_O008','JAC_POOL_O008'}}
for pid=0,1 do
  local leader=PlayerConfigurations[pid]:GetLeaderTypeName()
  if leader~=poolExpected[pid][1] then error('fixed pool leader '..pid..' '..tostring(leader)) end
  poolParts[#poolParts+1]=poolPerson(pid,leader)
  if poolByLeader[leader].PersonID~=poolExpected[pid][2] or PlayerConfigurations[pid]:GetCivilizationTypeName()~='CIVILIZATION_JAC_ODA' then error('fixed pool person '..pid) end
end
if PlayerConfigurations[0]:GetLeaderTypeName()==PlayerConfigurations[1]:GetLeaderTypeName() then error('fixed pool leaders are not distinct') end
