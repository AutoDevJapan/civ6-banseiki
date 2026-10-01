"""Execute diagnostic Lua against independent engine inputs; never a Civ6 success claim."""
from pathlib import Path
import unittest

from lupa import LuaError, LuaRuntime
from tests import test_leader_persona_lua as production_support

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_Test_PersonaBoundary"
LEADER = "LEADER_JAC_ODA_NOBUHIDE"

ENGINE = r'''
LEADER='LEADER_JAC_ODA_NOBUHIDE'
SKILL=LEADER..'_SIGNATURE_1'
SETTLED='JAC_PERSONA_SETTLED_'..LEADER
BASE='JAC_PERSONA_ODA_NOBUHIDE_BASE'
NODE='JAC_PERSONA_TREE_'..SKILL
TURN=5
LOGS, REQUESTS, UNIT_REQUESTS={}, {}, {}
function print(...) local a={...}; LOGS[#LOGS+1]=table.concat(a,' ') end
function include() end
function event() local e={}; e.Add=function(f) e[#e+1]=f end; return e end
Events,GameEvents={},{}
for _,k in ipairs({'GameCoreEventPublishComplete','LocalPlayerTurnBegin','LoadScreenClose','SpyMissionCompleted'}) do Events[k]=event() end
for _,k in ipairs({'JAC_BoundaryPrepare','JAC_BoundaryLoseCity','JAC_BoundarySpawnSpy'}) do GameEvents[k]=event() end
function fire(e,...) for _,f in ipairs(e) do f(...) end end
function collection(rows,key)
 local r={}; for i,row in ipairs(rows) do r[i-1]=row; r[row[key]]=row; if row.Hash then r[row.Hash]=row end end
 return setmetatable(r,{__call=function() local i=0; return function() i=i+1; return rows[i] end end})
end
local props={JAC_BOUNDARY_CONFIG={owner=0,leader=LEADER,cityID=11,controlID=10,x=2,y=0,armedTurn=0,skill=SKILL,total=1,spent=0}}
props['JAC_PERSONA_'..LEADER..'_OWNER']=0
props['JAC_PERSONA_'..LEADER..'_STATE']='GOVERNOR'
props['JAC_PERSONA_'..LEADER..'_MERIT']=12
props['JAC_PERSONA_'..LEADER..'_NODE_'..SKILL]=1
WRITES={}
Game={GetProperty=function(_,k) return props[k] end,
 SetProperty=function(_,k,v) WRITES[#WRITES+1]=k; props[k]=v end,
 GetCurrentGameTurn=function() return TURN end,GetLocalPlayer=function() return 0 end}
GameInfo={}
GameInfo.JAC_LeaderPersonas=collection({{LeaderType=LEADER,GovernorType='GOV_TEST',PersonType='GP_TEST',BasePromotionType='BASE_TEST',EvolutionCost=1}},'LeaderType')
GameInfo.Governors=collection({{GovernorType='GOV_TEST',Hash=9000,Index=0}},'GovernorType')
GameInfo.GovernorPromotions={BASE_TEST={Hash=7000}}
GameInfo.JAC_PersonaSkills=collection({{LeaderType=LEADER,ModifierId=BASE}},'ModifierId')
GameInfo.JAC_PersonaTreeNodes=collection({{LeaderType=LEADER,NodeType=SKILL,ModifierId=NODE,CategoryType='ADMIN',RouteType='SIGNATURE'}},'NodeType')
GameInfo.GreatPersonClasses={GREAT_PERSON_CLASS_JAC_PERSONA={UnitType='UNIT_PERSONA'}}
GameInfo.Units={UNIT_PERSONA={Index=99},UNIT_SPY={Index=55}}
GameInfo.GreatPersonIndividuals={[3]={GreatPersonIndividualType='GP_TEST'}}
GameInfo.Yields=collection({{YieldType='YIELD_GOLD',Index=0},{YieldType='YIELD_FOOD',Index=1}},'YieldType')
GameInfo.UnitOperations=collection({{OperationType='UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR',Hash=990,Index=0}},'OperationType')
UnitOperationTypes={SPY_TRAVEL_NEW_CITY=991,PARAM_X='x',PARAM_Y='y'}
EspionageResultTypes={SUCCESS_UNDETECTED=1,SUCCESS_MUST_ESCAPE=2,FAIL_UNDETECTED=3,CAPTURED=4,NO_RESULT=0}
PlayerOperations={EXECUTE_SCRIPT=1,ASSIGN_GOVERNOR=2,PARAM_GOVERNOR_TYPE='governor',PARAM_PLAYER_ONE='owner',PARAM_CITY_DEST='city'}
local function members(list)
 local i=0; return function() i=i+1; if list[i] then return i,list[i] end end
end
CITIES={}
function city(id,owner,x,y,name,marker)
 local c={id=id,owner=owner,x=x,y=y,name=name,marker=marker}
 c.GetID=function() return c.id end; c.GetOwner=function() return c.owner end
 c.GetX=function() return c.x end; c.GetY=function() return c.y end
 c.GetName=function() return c.name end
 c.GetProperty=function(_,k) if k==SETTLED then return c.marker end end
 c.GetYield=function(_,i) return i==0 and 13.5 or 8.25 end
 CITIES[#CITIES+1]=c; return c
end
CONTROL=city(10,0,1,0,'Control',0)
TARGET=city(11,0,2,0,'Target',1)
FOREIGN=city(20,1,2,1,'Foreign',0)
PLOTS={}
for _,c in ipairs(CITIES) do
 local index=c.x+c.y*10
 PLOTS[c.x..'/'..c.y]={GetIndex=function() return index end,GetProperty=function(_,k) if k==NODE then return 1 end end}
end
Map={GetPlot=function(x,y) return PLOTS[x..'/'..y] end}
Cities={GetPlotPurchaseCity=function(plot) for _,c in ipairs(CITIES) do if Map.GetPlot(c.x,c.y)==plot then return c end end end}
GOV={assigned=TARGET,established=true,neutralized=0,base=true,type=0}
GOV.GetType=function() return GOV.type end
GOV.GetAssignedCity=function() return GOV.assigned end
GOV.IsEstablished=function() return GOV.established end
GOV.GetNeutralizedTurns=function() return GOV.neutralized end
GOV.HasPromotion=function(_,h) assert(h==7000); return GOV.base end
APPOINTED=true
UNITS={[0]={},[1]={},[62]={}}
HISTORY={[0]={},[1]={}}
ACTIVE_TURN=true
Players={}
for _,id in ipairs({0,1,62}) do
 local p={}
 p.IsHuman=function() return id==0 end; p.IsAlive=function() return true end
 p.IsTurnActive=function() return ACTIVE_TURN end
 p.GetGovernors=function() return {
  HasGovernor=function(_,hash) assert(hash==9000); return id==0 and APPOINTED end,
  GetGovernor=function(_,hash) assert(hash==9000); if id==0 then return GOV end end,
  GetGovernorPoints=function() return 1 end,GetGovernorPointsSpent=function() return 1 end,
  CanAssignGovernor=function(_,hash,c) return GOV.neutralized==0 and c.owner==id end} end
 p.GetCities=function() return {
  Members=function() local owned={}; for _,c in ipairs(CITIES) do if c.owner==id then owned[#owned+1]=c end end; return members(owned) end,
  GetCapitalCity=function() if id==0 then return CONTROL elseif id==1 then return FOREIGN end end} end
 p.GetUnits=function() return {Members=function() return members(UNITS[id]) end,FindID=function(_,uid) for _,u in ipairs(UNITS[id]) do if u:GetID()==uid then return u end end end} end
 p.GetDiplomacy=function() return {GetMission=function(_,attacker,mid) assert(attacker==id); return HISTORY[id][mid] or 0 end} end
 Players[id]=p
end
PlayerManager={GetAliveMajorIDs=function() return {0,1} end,GetFreeCitiesPlayerID=function() return 62 end}
CityManager={GetCity=function(owner,id) for _,c in ipairs(CITIES) do if c.owner==owner and c.id==id then return c end end end,
 TransferCityToFreeCities=function(c) assert(c==TARGET); TRANSFERRED=c; c.owner=62; c.marker=0; GOV.assigned=nil; GOV.established=false; EFFECT_TARGET=false end}
EFFECT_TARGET=true
GameEffects={GetModifiers=function() return {100,101,102} end,
 GetModifierDefinition=function(id) return {Id=({BASE,SETTLED,NODE})[id-99]} end,
 GetModifierOwner=function() return {owner=0,name='Native governor'} end,
 GetModifierActive=function() return EFFECT_TARGET end,
 GetModifierSubjects=function() return {TARGET} end,
 GetObjectsPlayerId=function(obj) return obj.owner end,GetObjectName=function(obj) return obj.name end}
Locale={Lookup=function(name) return name end}
ExposedMembers={JAC_LeaderPersonaNativeRead=function() error('persona UI bridge is not native evidence') end}
ExposedMembers.JAC_PersonaBoundaryGameplayRead=function()
 local result={merit=props['JAC_PERSONA_'..LEADER..'_MERIT'],cities={}}
 for _,c in ipairs(CITIES) do if c.owner==0 or c==TARGET then
  result.cities[c.x..'/'..c.y]={id=c.id,owner=c.owner,settled=c.marker,selectors={[NODE]=1}}
 end end
 return result
end
ContextPtr={SetHide=function() end,SetUpdate=function(_,fn) FRAME=fn end}
UI={IsGameCoreBusy=function() return BUSY or false end,
 RequestPlayerOperation=function(owner,op,params) REQUESTS[#REQUESTS+1]={owner=owner,op=op,params=params} end}
CAN_START=true
UnitManager={
 CanStartOperation=function(spy,op,plot,a,b) QUERY={spy=spy,op=op,plot=plot,a=a,b=b}; return CAN_START,{failure={'native reason'}} end,
 RequestOperation=function(spy,op,params) UNIT_REQUESTS[#UNIT_REQUESTS+1]={spy=spy,op=op,params=params} end,
 GetTimeToComplete=function(index) assert(index==0); return 8 end,
 GetResultProbability=function() return {ESPIONAGE_SUCCESS_UNDETECTED=.25,ESPIONAGE_SUCCESS_MUST_ESCAPE=.20} end,
 GetTravelTime=function() return 3 end,GetEstablishInCityTime=function() return 4 end,
 InitUnitValidAdjacentHex=function(owner,typ,x,y,radius)
  assert(typ=='UNIT_SPY' and owner==0 and x==1 and y==0 and radius==1); makeSpy(owner,15)
 end}
function makeSpy(owner,id)
 local spy={x=2,y=1,op=-1,finish=TURN,owner=owner,id=id}
 spy.GetID=function() return spy.id end; spy.GetOwner=function() return spy.owner end
 spy.GetType=function() return 55 end; spy.GetX=function() return spy.x end; spy.GetY=function() return spy.y end
 spy.GetSpyOperation=function() return spy.op end; spy.GetSpyOperationEndTurn=function() return spy.finish end
 UNITS[owner][#UNITS[owner]+1]=spy; return spy
end
function stop(remaining)
 GOV.assigned=nil; GOV.established=false; GOV.neutralized=remaining; TARGET.marker=0; EFFECT_TARGET=false
end
function lose()
 stop(0); TARGET.owner=62
 props.JAC_BOUNDARY_CONFIG.transfer={fromOwner=0,fromCityID=11,toOwner=62,toCityID=11,x=2,y=0}
end
'''


class PersonaBoundaryLuaTests(unittest.TestCase):
    def setUp(self):
        self.lua = LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(ENGINE)
        self.lua.execute((MOD / "Shared/JAC_PersonaBoundaryRules.lua").read_text(encoding="utf-8"))
        self.lua.execute((MOD / "UI/JAC_PersonaBoundary_View.lua").read_text(encoding="utf-8"))
        self.g = self.lua.globals()
        self.b = self.g.ExposedMembers.JAC_PersonaBoundary

    def test_native_resolution_rejects_property_only_or_wrong_native_instance(self):
        self.b.Check("SETTLED")
        self.g.APPOINTED = False
        with self.assertRaisesRegex(LuaError, "native governor not appointed"):
            self.b.Read()
        self.g.APPOINTED = True
        self.g.GOV.type = 77
        with self.assertRaisesRegex(LuaError, "native governor type mismatch"):
            self.b.Read()
        self.assertEqual(len(self.g.WRITES), 0)

    def test_native_yields_effects_and_two_context_markers_are_in_same_record(self):
        s = self.b.Read()
        self.assertEqual(s.cities["2/0"].yields.YIELD_GOLD, 13.5)
        self.assertEqual(s.cities["2/0"].effects[self.g.NODE], 1)
        self.assertEqual(s.cities["2/0"].selectors[self.g.NODE], 1)
        self.assertEqual(s.gameplay.cities["2/0"].settled, 1)
        self.assertEqual((s.native.type, s.native.neutralized, s.merit), (0, 0, 12))
        self.lua.execute("ExposedMembers.JAC_PersonaBoundaryGameplayRead=function() return {merit=13,cities={}} end")
        with self.assertRaisesRegex(LuaError, "merit disagreement"):
            self.b.Check("SETTLED")

    def test_native_success_with_captured_escape_is_still_a_neutralization(self):
        self.lua.execute("stop(3); HISTORY[1][0]={Operation=0,PlotIndex=2,CompletionTurn=5,InitialResult=2,EscapeResult=4}")
        s = self.b.Read()
        self.assertTrue(s.neutralizationMission)
        self.assertEqual((s.missions["1/0"].attacker, s.missions["1/0"].escape), (1, 4))
        self.b.Check("NEUTRALIZED", 12)
        self.lua.execute("fire(Events.SpyMissionCompleted,1,0)")
        self.assertTrue(any("MISSION_EVENT" in x and "attacker=1.000000" in x for x in self.g.LOGS.values()))

    def test_failed_wrong_city_or_pre_fixture_mission_cannot_pass_neutralization(self):
        self.lua.execute("stop(3)")
        for fields in ((3, 2, 5), (1, 12, 5), (1, 2, -1)):
            initial, plot, turn = fields
            self.lua.execute(f"HISTORY[1][0]={{Operation=0,PlotIndex={plot},CompletionTurn={turn},InitialResult={initial},EscapeResult=0}}")
            with self.subTest(fields=fields), self.assertRaisesRegex(LuaError, "mission history was not observed"):
                self.b.Check("NEUTRALIZED")
        self.assertTrue(any("CHECK_NEUTRALIZED" in x for x in self.g.LOGS.values()))

    def test_missing_enemy_history_is_reported_and_cannot_pass_spy_check(self):
        self.lua.execute("stop(3); Players[1].GetDiplomacy=function() return {GetMission=function() error('enemy history unavailable') end} end")
        self.assertIn("unavailable", self.b.Read().missions["1/unavailable"])
        with self.assertRaisesRegex(LuaError, "mission history was not observed"):
            self.b.Check("NEUTRALIZED")
        self.lua.execute("lose()")
        self.b.Check("LOST", 12)  # City loss does not depend on enemy history.

    def test_recovery_is_unassigned_and_never_an_automatic_return(self):
        self.lua.execute("stop(0)")
        self.b.Check("RECOVERED", 12)
        self.g.GOV.assigned = self.g.TARGET
        with self.assertRaisesRegex(LuaError, "automatic return"):
            self.b.Check("RECOVERED", 12)
        self.b.Check("ESTABLISHING", 12)
        self.g.TARGET.marker = 1
        with self.assertRaisesRegex(LuaError, "settlement marker"):
            self.b.Check("ESTABLISHING")

    def test_stale_settlement_active_skill_or_merit_is_rejected_after_loss(self):
        self.lua.execute("lose()")
        self.b.Check("LOST", 12)
        # Acquisition selectors are logged separately; an old selector is not an active effect.
        self.g.TARGET.marker = 1
        with self.assertRaisesRegex(LuaError, "settlement marker"):
            self.b.Check("LOST")
        self.g.TARGET.marker = 0
        self.g.EFFECT_TARGET = True
        with self.assertRaisesRegex(LuaError, "native city effect"):
            self.b.Check("LOST")
        self.g.EFFECT_TARGET = False
        self.g.Game.SetProperty(None, f"JAC_PERSONA_{LEADER}_MERIT", 13)
        with self.assertRaisesRegex(LuaError, "merit changed"):
            self.b.Check("LOST", 12)

    def test_unknown_native_effect_inventory_is_not_treated_as_zero_effects(self):
        self.lua.execute("stop(0); GameEffects.GetModifiers=function() return nil end")
        with self.assertRaisesRegex(LuaError, "inventory unavailable"):
            self.b.Check("RECOVERED")
        self.lua.execute("GameEffects.GetModifiers=function() return {100} end; GameEffects.GetModifierActive=function() return nil end")
        with self.assertRaisesRegex(LuaError, "activity unavailable"):
            self.b.Check("RECOVERED")

    def test_orphaned_native_subject_after_transfer_is_recorded_and_rejected(self):
        self.lua.execute("""
            lose(); EFFECT_TARGET=true
            GameEffects.GetModifierSubjects=function() return {{owner=0,name='Old cached city name'}} end
        """)
        self.assertEqual(len(self.b.Read().unmatchedEffects),3)
        with self.assertRaisesRegex(LuaError,"unmatched city subject"):
            self.b.Check("LOST")
        self.assertTrue(any("Old cached city name" in x for x in self.g.LOGS.values()))

    def test_other_players_same_type_modifier_is_not_attributed_to_the_fixture(self):
        self.lua.execute("""
            GameEffects.GetModifiers=function() return {100,101,102,200} end
            local definition,owner,subjects=GameEffects.GetModifierDefinition,GameEffects.GetModifierOwner,GameEffects.GetModifierSubjects
            GameEffects.GetModifierDefinition=function(id) return id==200 and {Id=NODE} or definition(id) end
            GameEffects.GetModifierOwner=function(id) return id==200 and {owner=1,name='Other governor'} or owner(id) end
            GameEffects.GetModifierSubjects=function(id) return id==200 and {FOREIGN} or subjects(id) end
        """)
        self.b.Check("SETTLED")
        self.lua.execute("lose(); GameEffects.GetModifierActive=function(id) return id==200 end")
        self.b.Check("LOST",12)

    def test_native_state_is_read_again_after_context_reload(self):
        self.lua.execute("stop(3); HISTORY[1][0]={Operation=0,PlotIndex=2,CompletionTurn=5,InitialResult=1,EscapeResult=0}")
        before = self.b.Check("NEUTRALIZED", 12)
        self.lua.execute((MOD / "UI/JAC_PersonaBoundary_View.lua").read_text(encoding="utf-8"))
        b = self.g.ExposedMembers.JAC_PersonaBoundary
        self.assertEqual(b.Check("NEUTRALIZED", 12), before)
        self.g.GOV.neutralized = 2
        self.assertNotEqual(b.Check("NEUTRALIZED", 12), before)
        self.assertEqual(len(self.g.WRITES), 0)

    def test_native_spy_travel_is_sent_once_from_an_ordinary_frame(self):
        self.lua.execute("makeSpy(0,15); Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=15})")
        self.b.Queue("SPY_TRAVEL", self.lua.table_from({"owner": 1, "city": 20}))
        self.assertEqual(len(self.g.UNIT_REQUESTS), 0)
        self.g.BUSY = True
        self.g.FRAME()
        self.assertEqual(len(self.g.UNIT_REQUESTS), 0)
        self.g.BUSY = False
        self.g.FRAME()
        request = self.g.UNIT_REQUESTS[1]
        self.assertEqual((request.op, request.params.x, request.params.y), (991, 2, 1))
        self.g.FRAME()
        self.assertEqual(len(self.g.UNIT_REQUESTS), 1)

    def test_foreign_destination_is_selected_by_native_travel_eligibility(self):
        self.lua.execute("makeSpy(0,15); Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=15})")
        destination=self.b.FindSpyDestination()
        self.assertEqual((destination.owner,destination.city),(1,20))
        self.g.CAN_START=False
        with self.assertRaisesRegex(LuaError,"no native-eligible foreign spy destination"):
            self.b.FindSpyDestination()
        self.assertEqual(len(self.g.UNIT_REQUESTS),0)

    def test_native_mission_refusal_and_enemy_control_are_not_replayed(self):
        self.lua.execute("makeSpy(0,15); Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=15})")
        probe = self.b.Probe(1, 20)
        self.assertEqual((probe.turns, probe.travel, probe.establish), (8, 3, 4))
        self.assertEqual((self.g.QUERY.a, self.g.QUERY.b), (False, True))
        self.b.Queue("SPY_MISSION", self.lua.table_from({"owner": 1, "city": 20}))
        with self.assertRaisesRegex(LuaError, "target governor absent"):
            self.g.FRAME()
        self.g.FRAME()
        self.assertEqual(len(self.g.UNIT_REQUESTS), 0)
        self.lua.execute("makeSpy(1,16); Game:SetProperty('JAC_BOUNDARY_SPY',{owner=1,id=16})")
        with self.assertRaisesRegex(LuaError, "no proven control path"):
            self.b.Probe(1, 20)

    def test_native_mission_request_is_not_itself_evidence_of_neutralization(self):
        self.lua.execute("""
            makeSpy(0,15); Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=15})
            Players[1].GetGovernors=function() return {
                HasGovernor=function() return true end,
                GetGovernor=function() return {
                    GetType=function() return 0 end,GetAssignedCity=function() return FOREIGN end,
                    IsEstablished=function() return true end,GetNeutralizedTurns=function() return 0 end} end
            } end
        """)
        self.b.Queue("SPY_MISSION", self.lua.table_from({"owner": 1, "city": 20}))
        self.g.FRAME()
        self.assertEqual(self.g.UNIT_REQUESTS[1].op, 990)
        self.assertIsNone(self.g.UNIT_REQUESTS[1].params)
        self.g.FRAME()
        self.assertEqual(len(self.g.UNIT_REQUESTS), 1)
        self.assertEqual(self.b.Read().native.neutralized, 0)
        with self.assertRaisesRegex(LuaError, "neutralization was not observed"):
            self.b.Check("NEUTRALIZED")

    def test_missing_or_unidentified_modifier_definition_cannot_report_zero_effects(self):
        for missing in ("nil", "{}", "{Id=false}", "{Id=''}"):
            with self.subTest(definition=missing):
                self.setUp()
                self.lua.execute("lose(); GameEffects.GetModifierDefinition=function() return " + missing + " end")
                with self.assertRaisesRegex(LuaError, "definition unavailable"):
                    self.b.Check("LOST", 12)
                self.assertEqual(len(self.g.WRITES), 0)

    def test_loss_rejects_replacement_city_and_missing_or_wrong_transfer_identity(self):
        for replacement, reason in (("TARGET.id=77", "identity"), ("TARGET.x=3", "original plot missing"),
                                   ("Game:GetProperty('JAC_BOUNDARY_CONFIG').transfer=nil", "identity"),
                                   ("Game:GetProperty('JAC_BOUNDARY_CONFIG').transfer.fromCityID=77", "identity")):
            with self.subTest(replacement=replacement):
                self.setUp()
                self.lua.execute("lose(); " + replacement)
                with self.assertRaisesRegex(LuaError, reason):
                    self.b.Check("LOST", 12)

    def test_transfer_result_id_is_read_from_native_operation_not_inferred_later(self):
        self.lua.execute((MOD / "Scripts/JAC_PersonaBoundary_Fixture.lua").read_text(encoding="utf-8"))
        self.lua.execute("""
            local transfer=CityManager.TransferCityToFreeCities
            CityManager.TransferCityToFreeCities=function(c) transfer(c); c.id=77 end
            fire(GameEvents.JAC_BoundaryLoseCity,0)
        """)
        record=self.g.Game.GetProperty(None,"JAC_BOUNDARY_CONFIG").transfer
        self.assertEqual((record.fromOwner,record.fromCityID,record.toOwner,record.toCityID),(0,11,62,77))
        self.b.Check("LOST",12)
        self.g.TARGET.id=78
        with self.assertRaisesRegex(LuaError,"identity"):
            self.b.Check("LOST",12)

    def prepare_inputs(self, title_delta):
        self.lua.execute((MOD / "Scripts/JAC_PersonaBoundary_Fixture.lua").read_text(encoding="utf-8"))
        self.lua.execute("""
            Game:SetProperty('JAC_BOUNDARY_CONFIG',nil); WRITES={}; APPOINTED=false
            PlayerConfigurations={
                [0]={GetLeaderTypeName=function() return 'LEADER_JAC_HOJO_UJITSUNA' end},
                [1]={GetLeaderTypeName=function() return LEADER end}}
            GameInfo.JAC_PersonaTitleCivics=collection({{CivicType='CIVIC_FIXTURE'}},'CivicType')
            GameInfo.Civics={CIVIC_FIXTURE={Index=7}}
            POINTS,COMPLETED,JOIN_CALLS=0,false,0
            Players[0].GetGovernors=function() return {
                GetGovernorPoints=function() return POINTS end,GetGovernorPointsSpent=function() return 0 end} end
            Players[0].GetCulture=function() return {
                HasCivic=function(_,index) assert(index==7); return COMPLETED end,
                GetCultureCost=function(_,index) assert(index==7); return 100 end,
                SetCulturalProgress=function(_,index,amount)
                    assert(index==7 and amount==100); COMPLETED=true; POINTS=POINTS+TITLE_DELTA
                end} end
            ExposedMembers.JAC_LeaderPersona={Join=function(owner,source)
                assert(owner==0 and source==1 and POINTS==1); JOIN_CALLS=JOIN_CALLS+1; return 'JOINED'
            end}
        """)
        self.g.TITLE_DELTA = title_delta

    def test_fixture_prepare_uses_civic_completion_and_regular_join_with_noncapital_city(self):
        self.prepare_inputs(1)
        self.lua.execute("fire(GameEvents.JAC_BoundaryPrepare,0)")
        config = self.g.Game.GetProperty(None, "JAC_BOUNDARY_CONFIG")
        self.assertEqual((config.cityID, config.controlID, config.source, config.total, config.spent), (11, 10, 1, 1, 0))
        self.assertEqual(self.g.JOIN_CALLS, 1)
        self.assertEqual(list(self.g.WRITES.values()), ["JAC_BOUNDARY_CONFIG"])

    def test_fixture_prepare_rejects_incorrect_native_title_grant_before_join(self):
        self.prepare_inputs(2)
        with self.assertRaisesRegex(LuaError, "exactly one title"):
            self.lua.execute("fire(GameEvents.JAC_BoundaryPrepare,0)")
        self.assertEqual(self.g.JOIN_CALLS, 0)
        self.assertEqual(len(self.g.WRITES), 0)

    def test_native_city_transfer_and_spy_creation_do_not_write_persona_state(self):
        self.lua.execute((MOD / "Scripts/JAC_PersonaBoundary_Fixture.lua").read_text(encoding="utf-8"))
        self.lua.execute("fire(GameEvents.JAC_BoundaryLoseCity,0)")
        self.assertEqual((self.g.TRANSFERRED.id, self.g.TARGET.owner), (11, 62))
        self.b.Check("LOST", 12)
        self.assertEqual(list(self.g.WRITES.values()), ["JAC_BOUNDARY_CONFIG"])
        self.lua.execute("fire(GameEvents.JAC_BoundarySpawnSpy,0)")
        self.assertEqual(self.g.Game.GetProperty(None, "JAC_BOUNDARY_SPY").id, 15)
        self.assertEqual(list(self.g.WRITES.values()), ["JAC_BOUNDARY_CONFIG", "JAC_BOUNDARY_SPY"])

    def test_real_persona_gameplay_stops_all_four_merit_sources_at_each_boundary(self):
        production_support.LeaderPersonaLuaTests.setUpClass()
        for phase in ("NEUTRALIZED", "RECOVERED", "ESTABLISHING", "LOST"):
            with self.subTest(phase=phase):
                p = production_support.LeaderPersonaLuaTests("runTest")
                p.setUp()
                governor = p.settled()
                if phase == "NEUTRALIZED":
                    governor.city, governor.established, governor.neutralized = None, False, 3
                elif phase == "RECOVERED":
                    governor.city, governor.established = None, False
                elif phase == "ESTABLISHING":
                    governor.established = False
                else:
                    # Native transfer inputs, including a stale marker on the foreign old city.
                    old = p.g.CITIES[0][1]
                    old.GetOwner = lambda _=None: 2
                    old.markerOverrides["JAC_PERSONA_SETTLED_" + production_support.DATE] = 1
                    p.g.CITIES[0][1] = None
                    p.g.CITIES[2][99] = old
                    governor.city, governor.established = None, False
                p.g.FIRE("PlayerTurnStarted", 0)
                p.g.FIRE("BuildingConstructed", 0, 1, 0, 100, True)
                p.combat(loser_id=999, x=3)
                p.g.Players[0].allies[1] = True
                p.g.FIREUI("DiplomacyRelationshipChanged", 0, 1)
                self.assertEqual(p.prop("MERIT"), 0)
                p.g.RESET_CONTEXTS()
                p.load_contexts()
                p.g.TURN = 2
                p.g.FIRE("PlayerTurnStarted", 0)
                p.g.FIRE("BuildingConstructed", 0, 1, 0, 100, True)
                p.combat(loser_id=999, x=3)
                p.g.FIREUI("DiplomacyRelationshipChanged", 0, 1)
                self.assertEqual(p.prop("MERIT"), 0)
                # Positive control: a new, properly established city can still earn.
                governor.city = p.g.CITIES[0][2]
                governor.established, governor.neutralized = True, 0
                p.g.TURN = 3
                p.g.FIRE("PlayerTurnStarted", 0)
                p.g.FIRE("BuildingConstructed", 0, 2, 1, 300, True)
                self.assertEqual(p.prop("MERIT"), 5)


if __name__ == "__main__":
    unittest.main()
