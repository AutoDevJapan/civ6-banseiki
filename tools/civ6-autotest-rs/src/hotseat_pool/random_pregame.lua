-- Fixed pregame helper for the registered pool random canonical only.
-- Uses the official PlayerSetupLogic parameters so the host draws RANDOM from the written candidate lists.
if not UI.IsInFrontEnd() or not GameConfiguration.IsHotseat() or Network.IsInSession() then error('hotseat pregame before HostGame required') end
if GameConfiguration.GetValue('RULESET')~='RULESET_JAC_BANSEIKI' then error('Banseiki ruleset required') end
if not JAC_POOL_FILTER_INSTALLED then error('pool frontend hook not loaded') end
local pool,poolRows={},0
for _,row in ipairs(DB.ConfigurationQuery('SELECT LeaderType FROM JAC_BanseikiPool')) do pool[row.LeaderType]=true; poolRows=poolRows+1 end
if poolRows~=300 then error('pool count '..poolRows) end
local setups={}
for pid=0,3 do
  local c=PlayerConfigurations[pid]
  if not c or c:IsHuman()~=(pid<2) then error('pool random slot '..pid) end
  local parameters=CreatePlayerParameters(pid,true)
  parameters:Refresh()
  local leader=parameters.Parameters and parameters.Parameters.PlayerLeader
  if not leader or type(leader.Values)~='table' then error('PlayerLeader parameter missing '..pid) end
  local current=c:GetLeaderTypeName()
  local random,valid,official,currentValid=nil,0,0,false
  for _,v in ipairs(leader.Values) do
    if v.Value=='RANDOM' then random=v
    elseif not v.Invalid then
      valid=valid+1
      if v.Value==current then currentValid=true end
      if not pool[v.Value] then official=official+1 end
      if pid<2 and not pool[v.Value] then error('human candidate outside pool '..pid..' '..tostring(v.Value)) end
    end
  end
  -- AI slots are not pool-filtered: they must keep an official (non-ledger) candidate.
  if not random or valid==0 or not currentValid or (pid>=2 and official==0) then error('pool random candidates '..pid..' '..tostring(current)..' valid='..valid..' official='..official) end
  setups[pid]={parameters,leader,random}
end
for pid=0,3 do setups[pid][1]:SetParameterValue(setups[pid][2],setups[pid][3]) end
for pid=0,3 do setups[pid][1]:Refresh() end
local written={}
for pid=0,3 do
  local value=setups[pid][1]:Config_ReadParameterValues(setups[pid][2])
  if value~='RANDOM' then error('RANDOM leader not written '..pid..' '..tostring(value)) end
  written[#written+1]=pid..'=RANDOM'
end
return 'pool random leaders '..table.concat(written,',')
