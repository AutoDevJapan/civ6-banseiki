local poolRows,poolByLeader=0,{}
for row in GameInfo.JAC_BanseikiPool() do poolRows=poolRows+1; poolByLeader[row.LeaderType]=row end
local ledgerRows,ledgerVersion,ledgerCount=0,nil,nil
for row in GameInfo.JAC_BanseikiPoolLedger() do ledgerRows=ledgerRows+1; ledgerVersion=row.Version; ledgerCount=row.Count end
if poolRows~=300 or ledgerRows~=1 or ledgerCount~=300 or type(ledgerVersion)~='string' or not ledgerVersion:match('^300:%x+$') then error('pool ledger '..poolRows..' '..ledgerRows..' '..tostring(ledgerCount)..' '..tostring(ledgerVersion)) end
local savedLedger=Game:GetProperty('JAC_BANSEIKI_POOL_LEDGER')
if savedLedger~=ledgerVersion then error('saved pool ledger '..tostring(savedLedger)) end
local poolParts={savedLedger}
local function poolPerson(pid,leader)
  local row=poolByLeader[leader]; local p=Players[pid]; local c=PlayerConfigurations[pid]
  if not row or not p or not c or not p:IsHuman() then error('pool human '..pid..' '..tostring(leader)) end
  if c:GetCivilizationTypeName()~=row.CivilizationType then error('pool civilization '..pid..' '..tostring(c:GetCivilizationTypeName())) end
  local person=p:GetProperty('JAC_POOL_PERSON_ID')
  local trait=p:GetProperty('JAC_POOL_TRAIT')
  local role=p:GetProperty('JAC_POOL_ROLE')
  if person~=row.PersonID or trait~=row.TraitType or role~='LEADER' then error('pool person '..pid..' '..tostring(person)..' '..tostring(trait)..' '..tostring(role)) end
  return table.concat({pid,leader,row.CivilizationType,person,trait,role},'~')
end
local poolMajors=0
for _ in ipairs(PlayerManager.GetAliveMajors()) do poolMajors=poolMajors+1 end
