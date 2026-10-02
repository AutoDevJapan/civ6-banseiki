// Original embedded Lua from build_arbitration_ai_scenario.py, kept verbatim.
pub const PREAMBLE: &str = r###"local R=ExposedMembers.JAC_CourtArbitration
if not R or not R.AiCandidate then error('AI arbitration gameplay absent') end
local t=Game:GetProperty('JAC_ARB_AI_TEST')
if not t then error('AI fixture absent') end
"###;

pub const PICK_PAIR: &str = r###"local ids={}
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
  local id=p:GetID()
  if not p:IsHuman() and id~=t.shogun and p:GetCities():GetCount()>=2 then ids[#ids+1]=id end
end
table.sort(ids)
local a,b
for i=1,#ids do for j=i+1,#ids do
  local x,y=ids[i],ids[j]
  if not a and not t.used[R.AiWarKey(x,y)] and not Players[x]:GetDiplomacy():IsAtWarWith(y)
    and not Players[x]:GetDiplomacy():HasAllied(y)
    and Players[x]:GetCities():GetCount()==Players[y]:GetCities():GetCount() then a,b=x,y end
end end
if not a then error('no fresh equal-city AI pair') end
t.a,t.b=a,b; t.used[R.AiWarKey(a,b)]=true
Players[a]:GetDiplomacy():DeclareWarOn(b,WarTypes.FORMAL_WAR,true)
if not Players[a]:GetDiplomacy():IsAtWarWith(b) then error('fixture declaration failed') end
local all={}; for _,p in ipairs(PlayerManager.GetAliveMajors()) do all[#all+1]=p:GetID() end; table.sort(all)
R.ObserveAiWars(R.GameTurn(),all)
Game:SetProperty('JAC_ARB_AI_TEST',t)
"###;

pub const SNAPSHOT_TAIL: &str = r###"local function encode(v)
  if type(v)~='table' then return tostring(v) end
  local keys={}; for k in pairs(v) do keys[#keys+1]=k end
  table.sort(keys,function(a,b) return tostring(a)<tostring(b) end)
  local parts={}; for _,k in ipairs(keys) do parts[#parts+1]=tostring(k)..'='..encode(v[k]) end
  return '{'..table.concat(parts,',')..'}'
end
local out={encode(R.GameTurn()),encode(Game:GetProperty(R.PENDING_KEY)),
  encode(Game:GetProperty(R.AI_WARS_KEY)),encode(Game:GetProperty(R.LAST_REQUEST_KEY))}
local ids={}; for _,p in ipairs(PlayerManager.GetAliveMajors()) do ids[#ids+1]=p:GetID() end; table.sort(ids)
for _,id in ipairs(ids) do
  -- Native AI can spend/earn money while the local turn becomes active after loading (live02/03).
  -- Compare the saved exact payment receipt; current treasury is checked only at payment time.
  out[#out+1]=id..':'..encode(Players[id]:GetProperty(R.AI_LAST_KEY))..':'..R.Legitimacy(id)
    ..':'..encode(Players[id]:GetProperty(R.COST_RECORD_KEY))
  for _,other in ipairs(ids) do if id<other then
    out[#out+1]=id..'_'..other..':'..encode(Game:GetProperty(R.AiPairKey(id,other)))
      ..':'..encode(Game:GetProperty(R.TruceKey(id,other)))..':'..encode(Game:GetProperty(R.AwaitKey(id,other)))
      ..':'..encode(Players[id]:GetDiplomacy():IsAtWarWith(other))
  end end
end
return table.concat(out,'|')
"###;

pub const MUTATE_STATE: &str = r###"Game:SetProperty(R.PENDING_KEY,nil)
Game:SetProperty(R.AI_WARS_KEY,nil)
Game:SetProperty(R.LAST_REQUEST_KEY,nil)
Players[t.shogun]:SetProperty(R.AI_LAST_KEY,nil)
Players[t.a]:SetProperty(R.AI_LAST_KEY,nil)
Players[t.shogun]:SetProperty(R.COST_RECORD_KEY,nil)
Players[t.a]:SetProperty(R.COST_RECORD_KEY,nil)
Game:SetProperty(R.AiPairKey(t.a,t.b),nil)
Players[t.a]:GetTreasury():ChangeGoldBalance(7)
Players[t.a]:SetProperty(R.LEGITIMACY_KEY,90)
return 'changed saved state to prove restoration'
"###;

pub const PREPARE_PARTY_PREFIX: &str = r###"local R=ExposedMembers.JAC_CourtArbitration
if not R or not R.AiCandidate or R.GameTurn()~=1 then error('AI gameplay/turn missing') end
local t={used={}}
for _,p in ipairs(PlayerManager.GetAliveMajors()) do if R.IsAshikaga(p:GetID()) then t.shogun=p:GetID() end end
if not t.shogun or Players[t.shogun]:IsHuman() then error('AI shogun required') end
Players[t.shogun]:SetProperty(R.LEGITIMACY_KEY,0) -- isolate the party path
Game:SetProperty('JAC_ARB_AI_TEST',t)
"###;

pub const PREPARE_PARTY_SUFFIX: &str = r###"
local p=Players[a]; local cap=p:GetCities():GetCapitalCity()
-- InitUnitValidAdjacentHex has no returned Unit: collect the military list after creation.
for i=1,3 do UnitManager.InitUnitValidAdjacentHex(a,'UNIT_WARRIOR',cap:GetX(),cap:GetY(),1) end
local units={}
for _,u in p:GetUnits():Members() do local row=GameInfo.Units[u:GetType()]
  if row and ((row.Combat or 0)>0 or (row.RangedCombat or 0)>0) then units[#units+1]=u end
end
if #units<3 then error('not enough military for the loss fixture') end
local wars=Game:GetProperty(R.AI_WARS_KEY); local w=wars[R.AiWarKey(a,b)]
w.Since=R.GameTurn()-4; w.ArmyA=R.AiArmyCount(a); w.ArmyB=R.AiArmyCount(b)
Game:SetProperty(R.AI_WARS_KEY,wars)
for i=1,3 do UnitManager.Kill(units[i]) end
p:SetProperty(R.LEGITIMACY_KEY,35); p:GetTreasury():ChangeGoldBalance(1000)
Players[b]:SetProperty(R.LEGITIMACY_KEY,70)
local score=R.AiPartyScore(a,b,R.GameTurn(),w)
if score<6 then error('party fixture is not exhausted: '..score) end
return 'AI war prepared: real net army loss 3; recorded four-turn fatigue; legitimacy 35'
"###;

pub const PARTY_PENDING: &str = r###"local p=Game:GetProperty(R.PENDING_KEY)
if not p or p.Source~='ai_party' or p.Requester~=t.a or p.A~=t.a or p.B~=t.b
  or p.Turn~=2 or p.Power~=1 then error('exhausted AI did not request on turn 2') end
if Players[t.a]:GetProperty(R.AI_LAST_KEY)~=2 or Game:GetProperty(R.AiPairKey(t.a,t.b))~=2
  then error('AI interval not recorded') end
local cost=Players[t.a]:GetProperty(R.COST_RECORD_KEY)
if not cost or cost.Turn~=2 or cost.Source~='ai_party' or cost.Gold~=100 or cost.Legitimacy~=5
  or cost.GoldBefore-cost.GoldAfter~=100 or cost.LegitimacyBefore-cost.LegitimacyAfter~=5
  then error('party did not pay exactly 100 gold and 5 legitimacy') end
if not Players[t.a]:GetDiplomacy():IsAtWarWith(t.b) then error('same-turn ruling') end
return 'exhausted AI party requested; paid normal cost; awaiting the next turn'
"###;

pub const PARTY_RESOLVED: &str = r###"if R.GameTurn()~=3 or Game:GetProperty(R.PENDING_KEY)~=nil
  or Players[t.a]:GetDiplomacy():IsAtWarWith(t.b) then error('AI party ruling did not make peace') end
if Game:GetProperty(R.TruceKey(t.a,t.b))~=13 then error('party truce not ten turns') end
return 'party request accepted on the following turn; ten-turn truce'
"###;

pub const PREPARE_SHOGUN: &str = r###"
Players[t.shogun]:SetProperty(R.LEGITIMACY_KEY,70)
Players[t.shogun]:GetTreasury():ChangeGoldBalance(1000)
Players[a]:SetProperty(R.LEGITIMACY_KEY,70); Players[b]:SetProperty(R.LEGITIMACY_KEY,70)
local wars=Game:GetProperty(R.AI_WARS_KEY); wars[R.AiWarKey(a,b)].Since=R.GameTurn()-12
Game:SetProperty(R.AI_WARS_KEY,wars)
return 'fresh AI war prepared for the shogun long-war proposal'
"###;

pub const SHOGUN_PENDING: &str = r###"local p=Game:GetProperty(R.PENDING_KEY)
if not p or p.Source~='ai_shogun' or p.Requester~=t.shogun or p.A~=t.a or p.B~=t.b
  or p.Turn~=4 or p.Power~=3 then error('shogun proposal absent before 1573') end
if Players[t.shogun]:GetProperty(R.AI_LAST_KEY)~=4 then error('shogun interval missing') end
local cost=Players[t.shogun]:GetProperty(R.COST_RECORD_KEY)
if not cost or cost.Turn~=4 or cost.Source~='ai_shogun' or cost.Gold~=34 or cost.Legitimacy~=2
  or cost.GoldBefore-cost.GoldAfter~=34 or cost.LegitimacyBefore-cost.LegitimacyAfter~=2
  then error('shogun did not pay exactly 34 gold and 2 legitimacy') end
return 'AI shogun proposed before 1573; threefold power and one-third cost'
"###;

pub const SHOGUN_RESOLVED: &str = r###"if R.GameTurn()~=5 or Game:GetProperty(R.PENDING_KEY)~=nil
  or Players[t.a]:GetDiplomacy():IsAtWarWith(t.b) then error('shogun ruling did not make peace') end
if Game:GetProperty(R.TruceKey(t.a,t.b))~=35 then error('shogun truce not thirty turns') end
return 'both AI parties accepted the shogun proposal; thirty-turn truce'
"###;

pub const PREPARE_CUTOFF_PREFIX: &str = r###"-- Controlled boundary test only: 113 turns are skipped, never a performance/balance measurement.
if R.GameTurn()~=5 then error('clock jump must start at turn 5') end
Game.SetCurrentGameTurn(GameConfiguration.GetStartTurn()+117)
if R.GameTurn()~=118 then error('clock jump failed') end
for _,p in ipairs(PlayerManager.GetAliveMajors()) do p:SetProperty(R.LEGITIMACY_KEY,0) end
Players[t.shogun]:SetProperty(R.LEGITIMACY_KEY,70)
Players[t.shogun]:GetTreasury():ChangeGoldBalance(1000)
"###;

pub const PREPARE_CUTOFF_SUFFIX: &str = r###"
local wars=Game:GetProperty(R.AI_WARS_KEY); wars[R.AiWarKey(a,b)].Since=R.GameTurn()-12
Game:SetProperty(R.AI_WARS_KEY,wars)
return 'prepared exact 1573 boundary: new war, with other AI parties below petition legitimacy'
"###;

pub const LAST_EMPOWERED: &str = r###"local p=Game:GetProperty(R.PENDING_KEY)
if R.GameTurn()~=119 or not p or p.Source~='ai_shogun' or p.Requester~=t.shogun or p.Power~=3
  then error('shogun did not propose on the last empowered turn') end
return 'shogun proposal on turn 119, the last turn before the cutoff'
"###;

pub const AT_CUTOFF: &str = r###"if R.GameTurn()~=120 or R.IsEmpoweredShogun(t.shogun,120)
  or Game:GetProperty(R.PENDING_KEY)~=nil then error('1573 cutoff failed') end
if Players[t.shogun]:GetProperty(R.AI_LAST_KEY)~=119 then error('post-cutoff shogun proposed') end
return 'at 1573 cutoff: previous proposal resolved; no new third-party proposal'
"###;

pub const AFTER_CUTOFF: &str = r###"if R.GameTurn()~=121 or Game:GetProperty(R.PENDING_KEY)~=nil
  or Players[t.shogun]:GetProperty(R.AI_LAST_KEY)~=119 then error('shogun proposed after load/cutoff') end
return 'after save/load and another real turn: no shogun third-party proposal'
"###;
