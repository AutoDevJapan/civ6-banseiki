"""Build a short, controlled gameplay test; its clock jump is never used by the 200-turn run."""
import argparse
import copy
import json
from pathlib import Path

FOLDER = Path(__file__).resolve().parent / "scenarios"
PREAMBLE = """local R=ExposedMembers.JAC_CourtArbitration
if not R or not R.AiCandidate then error('AI arbitration gameplay absent') end
local t=Game:GetProperty('JAC_ARB_AI_TEST')
if not t then error('AI fixture absent') end
"""
PICK_PAIR = """local ids={}
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
"""
SNAPSHOT = PREAMBLE + """local function encode(v)
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
  -- Civ6 credits some other houses' current-turn income again while entering a loaded game
  -- (live02: Honganji +9.19921875, the human Ouchi +2.5). Compare money for the actual parties/shogun;
  -- legitimacy and AI intervals still compare every house. No game accounting is changed here.
  local gold=(id==t.a or id==t.b or id==t.shogun) and Players[id]:GetTreasury():GetGoldBalance() or 'outside-arbitration'
  out[#out+1]=id..':'..encode(Players[id]:GetProperty(R.AI_LAST_KEY))..':'..R.Legitimacy(id)
    ..':'..encode(gold)
  for _,other in ipairs(ids) do if id<other then
    out[#out+1]=id..'_'..other..':'..encode(Game:GetProperty(R.AiPairKey(id,other)))
      ..':'..encode(Game:GetProperty(R.TruceKey(id,other)))..':'..encode(Game:GetProperty(R.AwaitKey(id,other)))
      ..':'..encode(Players[id]:GetDiplomacy():IsAtWarWith(other))
  end end
end
return table.concat(out,'|')
"""


def scenario():
    base = json.loads((FOLDER / "japan-15km-200turn-full.json").read_text(encoding="utf-8"))
    steps = copy.deepcopy(base["steps"][:3])
    steps[1]["seed"] = 620981

    def lua(code, **kwargs):
        steps.append(dict(action="ui_lua", state_name="GameCore_Tuner", code=code, **kwargs))

    def turn():
        steps.append(dict(action="autoplay", turns=1, timeout=600))

    def round_trip(label):
        capture = label.upper().replace("-", "_")
        lua(SNAPSHOT, capture_as=capture)
        steps.append(dict(action="save", name="JAC-AI-" + label, timeout=90))
        lua(PREAMBLE + """Game:SetProperty(R.PENDING_KEY,nil)
Game:SetProperty(R.AI_WARS_KEY,nil)
Game:SetProperty(R.LAST_REQUEST_KEY,nil)
Players[t.shogun]:SetProperty(R.AI_LAST_KEY,nil)
Players[t.a]:SetProperty(R.AI_LAST_KEY,nil)
Game:SetProperty(R.AiPairKey(t.a,t.b),nil)
Players[t.a]:GetTreasury():ChangeGoldBalance(7)
Players[t.a]:SetProperty(R.LEGITIMACY_KEY,90)
return 'changed saved state to prove restoration'
""")
        steps.append(dict(action="exit_to_menu", timeout=600))
        steps.append(dict(action="load", name="JAC-AI-" + label, timeout=600))
        steps.append(dict(action="wait_for", timeout=600, condition=dict(type="lua", state_name="InGame",
                         code="local p=Players[Game.GetLocalPlayer()]; return p~=nil and p:IsTurnActive()")))
        lua(SNAPSHOT, equals_capture=capture)

    lua("""local R=ExposedMembers.JAC_CourtArbitration
if not R or not R.AiCandidate or R.GameTurn()~=1 then error('AI gameplay/turn missing') end
local t={used={}}
for _,p in ipairs(PlayerManager.GetAliveMajors()) do if R.IsAshikaga(p:GetID()) then t.shogun=p:GetID() end end
if not t.shogun or Players[t.shogun]:IsHuman() then error('AI shogun required') end
Players[t.shogun]:SetProperty(R.LEGITIMACY_KEY,0) -- isolate the party path
Game:SetProperty('JAC_ARB_AI_TEST',t)
""" + PICK_PAIR + """
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
""")
    turn()
    lua(PREAMBLE + """local p=Game:GetProperty(R.PENDING_KEY)
if not p or p.Source~='ai_party' or p.Requester~=t.a or p.A~=t.a or p.B~=t.b
  or p.Turn~=2 or p.Power~=1 then error('exhausted AI did not request on turn 2') end
if Players[t.a]:GetProperty(R.AI_LAST_KEY)~=2 or Game:GetProperty(R.AiPairKey(t.a,t.b))~=2
  then error('AI interval not recorded') end
if not Players[t.a]:GetDiplomacy():IsAtWarWith(t.b) then error('same-turn ruling') end
return 'exhausted AI party requested; paid normal cost; awaiting the next turn'
""")
    round_trip("party-pending")
    turn()
    lua(PREAMBLE + """if R.GameTurn()~=3 or Game:GetProperty(R.PENDING_KEY)~=nil
  or Players[t.a]:GetDiplomacy():IsAtWarWith(t.b) then error('AI party ruling did not make peace') end
if Game:GetProperty(R.TruceKey(t.a,t.b))~=13 then error('party truce not ten turns') end
return 'party request accepted on the following turn; ten-turn truce'
""")
    round_trip("party-resolved")
    lua(PREAMBLE + PICK_PAIR + """
Players[t.shogun]:SetProperty(R.LEGITIMACY_KEY,70)
Players[t.shogun]:GetTreasury():ChangeGoldBalance(1000)
Players[a]:SetProperty(R.LEGITIMACY_KEY,70); Players[b]:SetProperty(R.LEGITIMACY_KEY,70)
local wars=Game:GetProperty(R.AI_WARS_KEY); wars[R.AiWarKey(a,b)].Since=R.GameTurn()-12
Game:SetProperty(R.AI_WARS_KEY,wars)
return 'fresh AI war prepared for the shogun long-war proposal'
""")
    turn()
    lua(PREAMBLE + """local p=Game:GetProperty(R.PENDING_KEY)
if not p or p.Source~='ai_shogun' or p.Requester~=t.shogun or p.A~=t.a or p.B~=t.b
  or p.Turn~=4 or p.Power~=3 then error('shogun proposal absent before 1573') end
if Players[t.shogun]:GetProperty(R.AI_LAST_KEY)~=4 then error('shogun interval missing') end
return 'AI shogun proposed before 1573; threefold power and one-third cost'
""")
    round_trip("shogun-pending")
    turn()
    lua(PREAMBLE + """if R.GameTurn()~=5 or Game:GetProperty(R.PENDING_KEY)~=nil
  or Players[t.a]:GetDiplomacy():IsAtWarWith(t.b) then error('shogun ruling did not make peace') end
if Game:GetProperty(R.TruceKey(t.a,t.b))~=35 then error('shogun truce not thirty turns') end
return 'both AI parties accepted the shogun proposal; thirty-turn truce'
""")
    round_trip("shogun-resolved")
    lua(PREAMBLE + """-- Controlled boundary test only: 113 turns are skipped, never a performance/balance measurement.
if R.GameTurn()~=5 then error('clock jump must start at turn 5') end
Game.SetCurrentGameTurn(GameConfiguration.GetStartTurn()+117)
if R.GameTurn()~=118 then error('clock jump failed') end
for _,p in ipairs(PlayerManager.GetAliveMajors()) do p:SetProperty(R.LEGITIMACY_KEY,0) end
Players[t.shogun]:SetProperty(R.LEGITIMACY_KEY,70)
Players[t.shogun]:GetTreasury():ChangeGoldBalance(1000)
""" + PICK_PAIR + """
local wars=Game:GetProperty(R.AI_WARS_KEY); wars[R.AiWarKey(a,b)].Since=R.GameTurn()-12
Game:SetProperty(R.AI_WARS_KEY,wars)
return 'prepared exact 1573 boundary: new war, with other AI parties below petition legitimacy'
""")
    turn()
    lua(PREAMBLE + """local p=Game:GetProperty(R.PENDING_KEY)
if R.GameTurn()~=119 or not p or p.Source~='ai_shogun' or p.Requester~=t.shogun or p.Power~=3
  then error('shogun did not propose on the last empowered turn') end
return 'shogun proposal on turn 119, the last turn before the cutoff'
""")
    turn()
    lua(PREAMBLE + """if R.GameTurn()~=120 or R.IsEmpoweredShogun(t.shogun,120)
  or Game:GetProperty(R.PENDING_KEY)~=nil then error('1573 cutoff failed') end
if Players[t.shogun]:GetProperty(R.AI_LAST_KEY)~=119 then error('post-cutoff shogun proposed') end
return 'at 1573 cutoff: previous proposal resolved; no new third-party proposal'
""")
    round_trip("after-cutoff")
    turn()
    lua(PREAMBLE + """if R.GameTurn()~=121 or Game:GetProperty(R.PENDING_KEY)~=nil
  or Players[t.shogun]:GetProperty(R.AI_LAST_KEY)~=119 then error('shogun proposed after load/cutoff') end
return 'after save/load and another real turn: no shogun third-party proposal'
""")
    return dict(name="AI arbitration: exhausted AI vs AI party, shogun third-party proposal, exact 1573 cutoff, five save/load round trips. Army losses are real; prior fatigue and boundary clock are controlled fixtures.",
                judge_profile="japan_map", test_mode=dict(skipped_turns=113, continuous_performance_valid=False), steps=steps)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=FOLDER / "court-arbitration-ai.json")
    args = parser.parse_args()
    args.output.write_text(json.dumps(scenario(), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
