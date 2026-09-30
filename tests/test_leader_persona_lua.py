"""Run the real gameplay/UI Lua with delayed native operations, distinct Index/Hash and void creation.

These test the mod's transition logic, not Civ6's native implementation. The live scenario tests that.
"""
from pathlib import Path
import unittest

from lupa import LuaRuntime, LuaError  # Required: never silently skip Lua tests.

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_LeaderPersona"
DATE = "LEADER_JAC_DATE_TANEMUNE"
HOJO = "LEADER_JAC_HOJO_UJITSUNA"
ODA = "LEADER_JAC_ODA_NOBUHIDE"

STUBS = r'''
function store()
    local data = {}
    return function(_, k) return data[k] end, function(_, k, v) data[k] = v end
end
Game = { GetLocalPlayer=function() return LOCAL end, GetCurrentGameTurn=function() return TURN end }
Game.GetProperty, Game.SetProperty = store()
LOCAL, TURN, SPAWNS, DESTROYS, FAIL_SPAWN, FAIL_DESTROY = 0, 1, 0, 0, false, false
function collection(rows, key)
    local t = {}
    for _,r in ipairs(rows) do t[r[key]]=r; if r.Index then t[r.Index]=r end end
    return setmetatable(t, {__call=function()
        local i=0; return function() i=i+1; return rows[i] end
    end})
end
local names = {'ODA_NOBUHIDE','DATE_TANEMUNE','HOJO_UJITSUNA'}
local civs = {'ODA','DATE','HOJO'}
ROWS, INDIVIDUALS, GOVDEFS, PROMOS = {}, {}, {}, {}
for i,n in ipairs(names) do
    local leader='LEADER_JAC_'..n
    local row={LeaderType=leader, CivilizationType='CIVILIZATION_JAC_'..civs[i],
        Name='LOC_'..leader..'_NAME', PortraitIcon='ICON_'..leader,
        PersonType='GREAT_PERSON_INDIVIDUAL_JAC_PERSONA_'..n, GovernorType='GOVERNOR_JAC_PERSONA_'..n,
        BasePromotionType='GOVERNOR_PROMOTION_JAC_PERSONA_'..n..'_BASE',
        PromotionType='GOVERNOR_PROMOTION_JAC_PERSONA_'..n..'_PROMOTION_1',
        SecondPromotionType='GOVERNOR_PROMOTION_JAC_PERSONA_'..n..'_PROMOTION_2',
        Personality=i==1 and 'BOLD' or 'CALM', Trait=i==3 and 'DEVOUT' or 'ASTUTE',
        EvolutionCost=i==1 and 1 or 2}
    ROWS[i]=row
    INDIVIDUALS[i]={GreatPersonIndividualType=row.PersonType, Index=i+10, Name=row.Name}
    GOVDEFS[i]={GovernorType=row.GovernorType, Index=i+20, Hash=1000+i, Name=row.Name}
    PROMOS[#PROMOS+1]={GovernorPromotionType=row.BasePromotionType, Index=i+30, Hash=2000+i}
    PROMOS[#PROMOS+1]={GovernorPromotionType=row.PromotionType, Index=i+40, Hash=3000+i}
    PROMOS[#PROMOS+1]={GovernorPromotionType=row.SecondPromotionType, Index=i+50, Hash=4000+i}
end
GameInfo={ JAC_LeaderPersonas=collection(ROWS,'LeaderType'),
    GreatPersonIndividuals=collection(INDIVIDUALS,'GreatPersonIndividualType'),
    Governors=collection(GOVDEFS,'GovernorType'), GovernorPromotions=collection(PROMOS,'GovernorPromotionType') }
PlayerConfigurations, Players = {}, {}
NATIVE_UNITS, NATIVE_GOVS, CITIES, GRANTS = {}, {}, {}, {}
function members(rows)
    local list={}; for _,v in pairs(rows) do list[#list+1]=v end
    table.sort(list,function(a,b) return a:GetID()<b:GetID() end)
    local i=0; return function() i=i+1; if list[i] then return i,list[i] end end
end
for i,row in ipairs(ROWS) do
    local id=i-1
    local config={leader=row.LeaderType, civ=row.CivilizationType}
    config.GetLeaderTypeName=function() return config.leader end
    config.GetCivilizationTypeName=function() return config.civ end
    PlayerConfigurations[id]=config
    NATIVE_UNITS[id], NATIVE_GOVS[id], GRANTS[id], CITIES[id]={},{},{},{}
    local p={points=0,spent=0,alive=true,major=true,active=true}
    p.GetID=function() return id end
    p.IsAlive=function() return p.alive end; p.IsMajor=function() return p.major end
    p.IsTurnActive=function() return p.active end
    p.GetProperty,p.SetProperty=store()
    p.GetUnits=function() return {Members=function() return members(NATIVE_UNITS[id]) end,
        Destroy=function(_,u) DESTROYS=DESTROYS+1; if not FAIL_DESTROY then NATIVE_UNITS[id][u:GetID()]=nil end end} end
    for j=1,2 do
        local cityid=id*10+j
        CITIES[id][cityid]={GetID=function() return cityid end, GetOwner=function() return id end,
            GetX=function() return j end, GetY=function() return id end}
    end
    p.GetCities=function() return {GetCapitalCity=function() return CITIES[id][id*10+1] end} end
    p.GetGovernors=function() return {
        GetGovernor=function(_,hash) return NATIVE_GOVS[id][hash] end,
        HasGovernor=function(_,hash) return NATIVE_GOVS[id][hash]~=nil end,
        CanAppoint=function() return p.points>p.spent end,
        CanAssignGovernor=function(_,hash,city) return NATIVE_GOVS[id][hash]~=nil and city:GetOwner()==id end,
        ChangeGovernorPoints=function(_,delta) GRANTS[id][#GRANTS[id]+1]=delta; p.points=p.points+delta end,
        GetGovernorPoints=function() return p.points end, GetGovernorPointsSpent=function() return p.spent end
    } end
    Players[id]=p
end
CityManager={GetCity=function(owner,id) return CITIES[owner] and CITIES[owner][id] end}
local nextUnit=0
Game.GetGreatPeople=function() return { CreatePerson=function(_,owner,individual,x,y)
    SPAWNS=SPAWNS+1; if FAIL_SPAWN then return end
    assert(GameInfo.GreatPersonIndividuals[individual], 'individual INDEX required')
    assert(x==1 and y==owner)
    nextUnit=nextUnit+1; local id=nextUnit
    local u={ individual=individual, GetID=function() return id end }
    u.GetGreatPerson=function() return {IsGreatPerson=function() return true end, GetIndividual=function() return u.individual end} end
    NATIVE_UNITS[owner][id]=u
    -- Deliberately NO return: neither creation nor removal is used as an object-returning API.
end } end
function NEWGOV(owner,leader)
    local row=GameInfo.JAC_LeaderPersonas[leader]; local def=GameInfo.Governors[row.GovernorType]
    local g={promotions={}}
    g.promotions[GameInfo.GovernorPromotions[row.BasePromotionType].Hash]=true
    g.GetType=function() return def.Index end
    g.GetAssignedCity=function() return g.city end
    g.HasPromotion=function(_,hash) return g.promotions[hash]==true end
    NATIVE_GOVS[owner][def.Hash]=g; return g
end
ExposedMembers={}
function events()
    local handlers={}
    local bus=setmetatable({}, {__index=function(t,k)
        local ev={Add=function(f) handlers[k]=handlers[k] or {}; table.insert(handlers[k],f) end}
        rawset(t,k,ev); return ev
    end})
    return bus,function(name,...) for _,f in ipairs(handlers[name] or {}) do f(...) end end
end
GameEvents,FIRE=events(); Events,FIREUI=events()
function RESET_CONTEXTS() GameEvents,FIRE=events(); Events,FIREUI=events() end
function print() end
function include() end
Mouse={eLClick=1}
Locale={Lookup=function(s) return s end, ToUpper=function(s) return s end}
function control()
    return {SetText=function(_,v) end, SetIcon=function(_,v,size) end,
        SetHide=function(_,v) end, SetDisabled=function(_,v) end,
        RegisterCallback=function(_,event,f) end, CalculateSize=function() end,
        DoAutoSize=function() end, IsHidden=function() return true end, SetToolTipString=function() end}
end
Controls={Rows=control(),PersonaPanel=control(),PersonaToggle=control(),Governors=control()}
VIEWS={}
InstanceManager={new=function() return {
    ResetInstances=function() VIEWS={} end,
    GetInstance=function()
        local v={Portrait=control(),Name=control(),Status=control(),Evolve=control()}
        v.Portrait.SetIcon=function(_,icon) v.icon=icon end
        v.Name.SetText=function(_,name) v.name=name end
        v.Evolve.SetDisabled=function(_,disabled) v.disabled=disabled end
        v.Status.SetText=function(_,status) v.status=status end
        VIEWS[#VIEWS+1]=v; return v
    end
} end}
LuaEvents={GovernorPanel_Open=function() end}
PlayerOperations={EXECUTE_SCRIPT=1,APPOINT_GOVERNOR=2,ASSIGN_GOVERNOR=3,PROMOTE_GOVERNOR=4,
    PARAM_GOVERNOR_TYPE='governor',PARAM_PLAYER_ONE='cityowner',PARAM_CITY_DEST='city',
    PARAM_GOVERNOR_PROMOTION_TYPE='promotion'}
REQUESTS={}
UI={RequestPlayerOperation=function(owner,op,params) REQUESTS[#REQUESTS+1]={owner=owner,op=op,params=params} end}
function STEP()
    local req=table.remove(REQUESTS,1); if not req then return false end
    local owner,p,params=req.owner,Players[req.owner],req.params
    if req.op==1 then FIRE(params.OnStart,owner,params)
    elseif req.op==2 then
        local def=assert(GameInfo.Governors[params.governor], 'operation expects INDEX, not Hash')
        assert(p.points>p.spent)
        local leader
        for _,r in ipairs(ROWS) do if r.GovernorType==def.GovernorType then leader=r.LeaderType end end
        NEWGOV(owner,leader); p.spent=p.spent+1; FIREUI('GovernorChanged',owner,def.Index)
    elseif req.op==3 then
        local def=assert(GameInfo.Governors[params.governor])
        assert(params.cityowner==owner and CITIES[owner][params.city])
        NATIVE_GOVS[owner][def.Hash].city=CITIES[owner][params.city]
    elseif req.op==4 then
        local def=assert(GameInfo.Governors[params.governor]); local promo=assert(GameInfo.GovernorPromotions[params.promotion])
        if p.points>p.spent then NATIVE_GOVS[owner][def.Hash].promotions[promo.Hash]=true; p.spent=p.spent+1 end
    end
    FIREUI('GameCoreEventPublishComplete'); return true
end
function DRAIN() for i=1,20 do if not STEP() then return end end; error('request loop') end
'''


class LeaderPersonaLuaTests(unittest.TestCase):
    def setUp(self):
        self.lua = LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS)
        self.g = self.lua.globals()
        self.load_contexts()

    def load_contexts(self):
        self.lua.execute((MOD / "Scripts/JAC_LeaderPersona_Gameplay.lua").read_text(encoding="utf-8"))
        self.lua.execute((MOD / "UI/JAC_LeaderPersona_View.lua").read_text(encoding="utf-8"))
        self.api = self.g.ExposedMembers.JAC_LeaderPersona

    def prop(self, field, leader=DATE):
        return self.g.Game.GetProperty(None, f"JAC_PERSONA_{leader}_{field}")

    def join(self):
        self.assertEqual(self.api.Join(0, 1, "CALM", "ASTUTE"), "JOINED")

    def evolve(self):
        self.join()
        self.api.GrantTestTitle(0, DATE, "APPOINT")
        self.g.JAC_EvolveLeader(DATE)
        self.g.DRAIN()
        self.assertEqual(self.prop("STATE"), "GOVERNOR")

    def test_void_creation_keeps_named_individual_and_metadata(self):
        self.join()
        self.assertEqual(self.prop("STATE"), "GREAT_PERSON")
        self.assertEqual((self.prop("SOURCE"), self.prop("OWNER"), self.g.SPAWNS), (1, 0, 1))
        self.assertEqual((self.prop("NAME"), self.prop("PORTRAIT")), (f"LOC_{DATE}_NAME", f"ICON_{DATE}"))
        self.assertEqual((self.prop("PERSONALITY"), self.prop("TRAIT")), ("CALM", "ASTUTE"))
        self.assertEqual(self.g.NATIVE_UNITS[0][1].individual, 12)
        self.assertEqual(self.g.Players[1].alive, True)

    def test_designed_personality_trait_and_cost_are_join_defaults(self):
        self.assertEqual(self.api.Join(0, 1), "JOINED")
        self.assertEqual((self.prop("PERSONALITY"), self.prop("TRAIT"), self.prop("EVOLUTION_COST")),
                         ("CALM", "ASTUTE", 2))
        self.assertEqual(self.api.Join(1, 0), "JOINED")
        self.assertEqual((self.prop("PERSONALITY", ODA), self.prop("TRAIT", ODA), self.prop("EVOLUTION_COST", ODA)),
                         ("BOLD", "ASTUTE", 1))

    def test_fixed_trait_cannot_be_replaced_at_join(self):
        self.assertEqual(self.api.Join(0, 1, "CALM", "VALIANT"), "INVALID_METADATA")
        self.assertEqual(self.g.SPAWNS, 0)
        self.assertEqual(self.api.Join(0, 1, "CAREFUL"), "JOINED")
        self.assertEqual((self.prop("PERSONALITY"), self.prop("TRAIT")), ("CAREFUL", "ASTUTE"))

    def test_partial_titles_refuse_costlier_date_but_suffice_for_oda(self):
        self.join()
        self.g.Players[0].points = 1
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "NO_TITLE")
        self.assertEqual((self.prop("STATE"), self.g.Players[0].points, self.g.DESTROYS), ("GREAT_PERSON", 1, 0))
        self.g.JAC_RefreshPersonas()
        self.assertTrue(self.g.VIEWS[1].disabled)
        self.assertEqual(self.api.Join(1, 0), "JOINED")
        self.g.Players[1].points = 1
        self.assertEqual(self.api.PrepareEvolution(1, ODA), "EVOLVING")
        self.assertEqual((self.g.Players[1].points, self.prop("EVOLUTION_SURCHARGE", ODA)), (1, 0))

    def test_pending_surcharge_survives_reload_and_spending_without_double_charge(self):
        self.join()
        self.g.Players[0].points = 2
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "EVOLVING")
        self.assertEqual((self.g.Players[0].points, self.prop("EVOLUTION_SURCHARGE")), (1, 1))
        self.g.Players[0].spent = 1  # another native operation took the remaining title
        self.assertFalse(self.g.JAC_AppointLeader(DATE))
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "APPOINTMENT_PENDING")
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "EVOLVING")
        self.assertEqual((self.g.Players[0].points, self.g.DESTROYS), (1, 0))
        self.g.Players[0].points = 2  # replenished with one new earned title
        self.g.JAC_AppointLeader(DATE); self.g.DRAIN()
        self.assertEqual((self.prop("STATE"), self.g.Players[0].points, self.g.Players[0].spent), ("GOVERNOR", 2, 2))
        self.assertEqual(list(self.g.GRANTS[0].values()), [-1])

    def test_native_rejection_of_negative_delta_never_allows_cheaper_evolution(self):
        self.join()
        self.g.Players[0].points = 2
        self.lua.execute("""
            local original=Players[0].GetGovernors
            Players[0].GetGovernors=function()
                local g=original()
                g.ChangeGovernorPoints=function() end
                return g
            end
        """)
        self.g.JAC_EvolveLeader(DATE); self.g.DRAIN()
        self.assertEqual(self.g.Players[0].GetProperty(None, "JAC_PERSONA_RESULT"), "TITLE_PAYMENT_FAILED")
        self.assertEqual((self.prop("STATE"), self.prop("EVOLUTION_SURCHARGE"), self.g.DESTROYS),
                         ("GREAT_PERSON", None, 0))
        self.assertEqual((self.g.Players[0].points, self.g.Players[0].spent), (2, 0))

    def test_upper_cost_consumes_two_extra_titles_and_one_native_once(self):
        self.g.GameInfo.JAC_LeaderPersonas[DATE].EvolutionCost = 3
        self.join()
        self.api.GrantTestTitle(0, DATE, "APPOINT")
        self.g.JAC_EvolveLeader(DATE); self.g.DRAIN()
        self.assertEqual((self.prop("STATE"), self.prop("EVOLUTION_COST"), self.prop("EVOLUTION_SURCHARGE")),
                         ("GOVERNOR", 3, 2))
        self.assertEqual((self.g.Players[0].points, self.g.Players[0].spent), (1, 1))
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "ALREADY_EVOLVED")
        self.assertEqual(self.api.GrantTestTitle(0, DATE, "APPOINT"), "TITLE_ALREADY_GRANTED")
        self.assertEqual(list(self.g.GRANTS[0].values()), [3, -2])
        self.assertEqual((self.g.DESTROYS, self.g.Players[0].points, self.g.Players[0].spent), (1, 1, 1))

    def test_joined_cost_is_fixed_even_if_balance_data_changes(self):
        self.join()
        self.g.GameInfo.JAC_LeaderPersonas[DATE].EvolutionCost = 3
        self.assertEqual(self.api.EvolutionCost(DATE), 2)

    def test_oda_uses_one_title_and_both_of_its_own_skill_branches(self):
        self.g.LOCAL = 1
        self.assertEqual(self.api.Join(1, 0), "JOINED")
        self.assertEqual(self.api.GrantTestTitle(1, ODA, "APPOINT"), "TITLE_GRANTED")
        self.g.JAC_EvolveLeader(ODA); self.g.DRAIN()
        self.assertEqual((self.prop("STATE", ODA), self.g.Players[1].points, self.g.Players[1].spent),
                         ("GOVERNOR", 1, 1))
        governor = self.g.NATIVE_GOVS[1][1001]
        self.assertTrue(governor.HasPromotion(None, 2001))
        for phase, field, index, native in [("PROMOTE", "PromotionType", 41, 3001),
                                           ("PROMOTE2", "SecondPromotionType", 51, 4001)]:
            self.api.GrantTestTitle(1, ODA, phase)
            self.g.JAC_PromoteLeader(ODA, self.g.GameInfo.JAC_LeaderPersonas[ODA][field])
            self.assertEqual(self.g.REQUESTS[1].params.promotion, index)
            self.g.DRAIN()
            self.assertTrue(governor.HasPromotion(None, native))
        self.assertFalse(governor.HasPromotion(None, 3002))
        self.assertEqual((self.g.Players[1].points, self.g.Players[1].spent), (3, 3))

    def test_native_individual_identity_is_resolved_through_gameinfo(self):
        self.join()
        # The official UI resolves the native token through GameInfo. Do not assume its number
        # must equal the creation parameter; preserve identity whenever the lookup resolves it.
        self.lua.execute('''
            GameInfo.GreatPersonIndividuals[9002]=GameInfo.GreatPersonIndividuals[12]
            NATIVE_UNITS[0][1].individual=9002
        ''')
        self.api.GrantTestTitle(0, DATE, "APPOINT")
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "EVOLVING")
        self.g.NEWGOV(0, DATE)
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "EVOLVED")
        self.assertEqual(self.g.DESTROYS, 1)

    def test_repeated_join_and_other_owner_do_not_spawn_or_replace_metadata(self):
        self.join()
        self.assertEqual(self.api.Join(0, 1, "BRAVE", "VALIANT"), "ALREADY_JOINED")
        self.assertEqual(self.api.Join(2, 1), "OTHER_OWNER")
        self.assertEqual((self.g.SPAWNS, self.prop("PERSONALITY"), self.prop("TRAIT")), (1, "CALM", "ASTUTE"))

    def test_invalid_requests_never_create_a_person(self):
        for args in [(0, 0), (0, 99), (99, 1), (-1, 1), (0.5, 1), (0, 1, "UNKNOWN"), (0, 1, None, "UNKNOWN")]:
            with self.subTest(args=args):
                self.assertIn(self.api.Join(*args), ("INVALID", "INVALID_METADATA"))
        self.g.PlayerConfigurations[1].civ = "CIVILIZATION_EGYPT"
        self.assertEqual(self.api.Join(0, 1), "INVALID")
        self.assertEqual(self.g.SPAWNS, 0)
        self.assertIsNone(self.prop("OWNER"))

    def test_no_city_failed_spawn_and_retry(self):
        city = self.g.CITIES[0][1]
        self.g.CITIES[0][1] = None
        self.assertEqual(self.api.Join(0, 1), "NO_CITY")
        self.g.CITIES[0][1] = city
        self.g.FAIL_SPAWN = True
        self.assertEqual(self.api.Join(0, 1), "SPAWN_FAILED")
        self.assertIsNone(self.prop("STATE"))
        self.g.FAIL_SPAWN = False
        self.join()
        self.assertEqual(self.g.SPAWNS, 2)

    def test_evolution_requires_ownership_person_and_native_title(self):
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "NOT_OWNED")
        self.join()
        self.assertEqual(self.api.PrepareEvolution(2, DATE), "NOT_OWNED")
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "NO_TITLE")
        self.assertEqual(self.prop("STATE"), "GREAT_PERSON")
        self.g.NATIVE_UNITS[0][1] = None
        self.api.GrantTestTitle(0, DATE, "APPOINT")
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "PERSON_MISSING")
        self.assertEqual(self.g.DESTROYS, 0)

    def test_pending_appointment_keeps_unit_until_native_success(self):
        self.join()
        self.api.GrantTestTitle(0, DATE, "APPOINT")
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "EVOLVING")
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "APPOINTMENT_PENDING")
        self.assertEqual(self.g.DESTROYS, 0)
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "EVOLVING")
        self.g.NEWGOV(0, DATE)
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "EVOLVED")
        self.assertIsNone(self.prop("UNIT"))
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "ALREADY_EVOLVED")
        self.assertEqual(self.g.DESTROYS, 1)

    def test_delayed_removal_does_not_mark_evolution_complete(self):
        self.join()
        self.api.GrantTestTitle(0, DATE, "APPOINT")
        self.api.PrepareEvolution(0, DATE)
        self.g.NEWGOV(0, DATE)
        self.g.FAIL_DESTROY = True
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "REMOVAL_PENDING")
        self.assertEqual(self.prop("STATE"), "EVOLVING")
        self.g.FAIL_DESTROY = False
        self.g.FIRE("OnGameTurnStarted")
        self.assertEqual(self.prop("STATE"), "GOVERNOR")

    def test_native_candidate_object_without_appointment_does_not_remove_the_person(self):
        self.join()
        self.api.GrantTestTitle(0, DATE, "APPOINT")
        self.api.PrepareEvolution(0, DATE)
        self.g.NEWGOV(0, DATE)
        self.lua.execute('''
            local original=Players[0].GetGovernors
            Players[0].GetGovernors=function()
                local g=original(); g.HasGovernor=function() return false end; return g
            end
        ''')
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "APPOINTMENT_PENDING")
        self.assertEqual((self.prop("STATE"), self.g.DESTROYS), ("EVOLVING", 0))

    def test_reload_contexts_preserves_unit_and_reconciles_only_pending(self):
        self.join()
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.g.FIRE("OnGameTurnStarted")
        self.assertEqual((self.prop("STATE"), self.g.SPAWNS), ("GREAT_PERSON", 1))
        self.api.GrantTestTitle(0, DATE, "APPOINT")
        self.api.PrepareEvolution(0, DATE)
        self.g.NEWGOV(0, DATE)
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.g.FIRE("OnGameTurnStarted")
        self.assertEqual((self.prop("STATE"), self.g.DESTROYS), ("GOVERNOR", 1))
        self.g.RESET_CONTEXTS(); self.load_contexts(); self.g.FIRE("OnGameTurnStarted")
        self.assertEqual((self.prop("PERSONALITY"), self.prop("TRAIT"), self.g.DESTROYS), ("CALM", "ASTUTE", 1))

    def test_test_titles_are_explicit_once_per_phase_across_reload(self):
        self.join()
        self.assertEqual(self.g.Players[0].points, 0)
        self.assertEqual(self.api.GrantTestTitle(2, DATE, "APPOINT"), "INVALID")
        self.assertEqual(self.api.GrantTestTitle(0, DATE, "OTHER"), "INVALID")
        self.assertEqual(self.api.GrantTestTitle(0, DATE, "APPOINT"), "TITLE_GRANTED")
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.assertEqual(self.api.GrantTestTitle(0, DATE, "APPOINT"), "TITLE_ALREADY_GRANTED")
        self.assertEqual(self.api.GrantTestTitle(0, DATE, "PROMOTE"), "TITLE_GRANTED")
        self.assertEqual(list(self.g.GRANTS[0].values()), [2, 1])
        self.assertEqual(self.g.Players[0].points, 3)

    def test_ui_uses_native_operation_indices_and_engine_requester(self):
        self.join()
        self.api.GrantTestTitle(0, DATE, "APPOINT")
        self.g.JAC_EvolveLeader(DATE)
        self.assertEqual(self.g.REQUESTS[1].params.OnStart, "JAC_LeaderPersonaEvolve")
        self.g.STEP()  # gameplay prepares, UI queues appointment
        self.assertEqual((self.g.REQUESTS[1].op, self.g.REQUESTS[1].params.governor), (2, 22))
        self.assertEqual(self.g.DESTROYS, 0)
        self.g.DRAIN()
        self.assertEqual((self.prop("STATE"), self.g.Players[0].spent), ("GOVERNOR", 1))
        self.assertEqual((self.g.Players[0].points, self.prop("EVOLUTION_SURCHARGE")), (1, 1))
        self.g.JAC_EvolveLeader(DATE); self.g.DRAIN()
        self.assertEqual((self.g.DESTROYS, self.g.Players[0].spent), (1, 1))

    def test_ui_rejects_inactive_turn_and_unowned_governor(self):
        self.join()
        self.g.Players[0].active = False
        with self.assertRaisesRegex(LuaError, "active turn"):
            self.g.JAC_EvolveLeader(DATE)
        self.assertEqual(len(self.g.REQUESTS), 0)
        self.g.Players[0].active = True
        with self.assertRaisesRegex(LuaError, "not owned"):
            self.g.JAC_EvolveLeader(HOJO)
        with self.assertRaisesRegex(LuaError, "governor not owned"):
            self.g.JAC_AssignLeader(DATE, 1)

    def test_ui_native_assignment_reassignment_promotion_and_portrait(self):
        self.evolve()
        governor = self.g.NATIVE_GOVS[0][1002]
        self.g.JAC_AssignLeader(DATE, 1); self.g.DRAIN()
        self.assertEqual(governor.GetAssignedCity().GetID(), 1)
        self.g.JAC_AssignLeader(DATE, 2); self.g.DRAIN()
        self.assertEqual(governor.GetAssignedCity().GetID(), 2)
        with self.assertRaisesRegex(LuaError, "city not assignable"):
            self.g.JAC_AssignLeader(DATE, 11)
        self.api.GrantTestTitle(0, DATE, "PROMOTE")
        self.g.JAC_PromoteLeader(DATE)
        self.assertEqual(self.g.REQUESTS[1].params.promotion, 42)
        self.g.DRAIN()
        self.assertTrue(governor.HasPromotion(None, 3002))
        self.assertEqual(self.g.Players[0].spent, 2)
        self.api.GrantTestTitle(0, DATE, "PROMOTE2")
        self.g.JAC_PromoteLeader(DATE, self.g.GameInfo.JAC_LeaderPersonas[DATE].SecondPromotionType)
        self.assertEqual(self.g.REQUESTS[1].params.promotion, 52)
        self.g.DRAIN()
        self.assertTrue(governor.HasPromotion(None, 4002))
        self.assertEqual((self.g.Players[0].spent, self.g.Players[0].points), (3, 3))
        with self.assertRaisesRegex(LuaError, "not in person's tree"):
            self.g.JAC_PromoteLeader(DATE, self.g.GameInfo.JAC_LeaderPersonas[ODA].PromotionType)
        self.g.JAC_RefreshPersonas()
        self.assertEqual((len(self.g.VIEWS), self.g.VIEWS[1].name, self.g.VIEWS[1].icon), (1, f"LOC_{DATE}_NAME", f"ICON_{DATE}"))

    def test_official_governor_panel_hides_unjoined_candidates_and_keeps_native_buttons(self):
        self.lua.execute('''
            CANDIDATES, APPOINTED = {}, {}
            AddGovernorCandidate=function(def,available) CANDIDATES[#CANDIDATES+1]=def.GovernorType end
            AddGovernorShared=function(def) return {GovernorPortrait={SetIcon=function(_,icon,size) ICON,ICON_SIZE=icon,size end}} end
            OnAppointGovernor=function(id) APPOINTED[#APPOINTED+1]=id end
        ''')
        self.lua.execute((MOD / "UI/JAC_LeaderPersona_GovernorPanel.lua").read_text(encoding="utf-8"))
        gov = self.g.GameInfo.Governors[22]
        self.g.AddGovernorCandidate(gov, True); self.g.OnAppointGovernor(22)
        self.assertEqual((len(self.g.CANDIDATES), len(self.g.APPOINTED)), (0, 0))
        self.join()
        self.g.AddGovernorCandidate(gov, True)
        self.assertEqual(len(self.g.CANDIDATES), 0)
        self.api.GrantTestTitle(0, DATE, "APPOINT"); self.api.PrepareEvolution(0, DATE)
        self.g.AddGovernorCandidate(gov, True); self.g.OnAppointGovernor(22)
        self.g.AddGovernorShared(gov)
        self.assertEqual(list(self.g.CANDIDATES.values()), ["GOVERNOR_JAC_PERSONA_DATE_TANEMUNE"])
        self.assertEqual(list(self.g.APPOINTED.values()), [22])
        self.assertEqual((self.g.ICON, self.g.ICON_SIZE), (f"ICON_{DATE}", 256))

    def test_unit_panel_uses_the_leader_portrait_and_preserves_other_units(self):
        self.join()
        self.lua.execute("GetUnitPortraitIconNames=function(unit) return 'original','prefix','era','fallback' end")
        self.lua.execute((MOD / "UI/JAC_LeaderPersona_UnitPanel.lua").read_text(encoding="utf-8"))
        self.assertEqual(self.g.GetUnitPortraitIconNames(self.g.NATIVE_UNITS[0][1]), (f"ICON_{DATE}",)*4)
        self.assertEqual(self.g.GetUnitPortraitIconNames(None), ("original", "prefix", "era", "fallback"))

    def test_governor_details_preserve_native_refresh_then_set_the_leader_portrait(self):
        self.lua.execute(f'''
            REFRESHES=0
            Refresh=function() REFRESHES=REFRESHES+1 end
            Controls.GovernorNameLabel={{GetText=function() return 'LOC_{DATE}_NAME' end}}
            Controls.GovernorPortrait={{SetIcon=function(_,icon,size) ICON,ICON_SIZE=icon,size end}}
        ''')
        self.lua.execute((MOD / "UI/JAC_LeaderPersona_GovernorDetails.lua").read_text(encoding="utf-8"))
        self.g.Refresh()
        self.assertEqual((self.g.REFRESHES, self.g.ICON, self.g.ICON_SIZE), (1, f"ICON_{DATE}", 256))


if __name__ == "__main__":
    unittest.main()
