"""Run the real gameplay/UI Lua with delayed native operations, distinct Index/Hash and void creation.

These test the mod's transition logic, not Civ6's native implementation. The live scenario tests that.
"""
from pathlib import Path
import unittest

from lupa import LuaRuntime, LuaError  # Required: never silently skip Lua tests.
import tests.test_leader_persona as persona_test_support

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
        Personality=i==1 and 'BOLD' or 'CALM', Trait=i==3 and 'DEVOUT' or 'ASTUTE',
        EvolutionCost=i==1 and 1 or 2}
    ROWS[i]=row
    INDIVIDUALS[i]={GreatPersonIndividualType=row.PersonType, GreatPersonClassType="GREAT_PERSON_CLASS_JAC_PERSONA", Index=i+10, Name=row.Name}
    GOVDEFS[i]={GovernorType=row.GovernorType, Index=i+20, Hash=1000+i, Name=row.Name}
    PROMOS[#PROMOS+1]={GovernorPromotionType=row.BasePromotionType, Index=i+30, Hash=2000+i}
end
GameInfo={ GreatPersonClasses=collection({{GreatPersonClassType="GREAT_PERSON_CLASS_JAC_PERSONA",UnitType="UNIT_JAC_PERSONA"}},"GreatPersonClassType"),
    Units=collection({{UnitType="UNIT_JAC_PERSONA",Index=99}},"UnitType"), JAC_LeaderPersonas=collection(ROWS,'LeaderType'),
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
    local p={points=0,spent=0,alive=true,major=true,active=true,human=true,allies={}}
    p.GetID=function() return id end
    p.IsAlive=function() return p.alive end; p.IsMajor=function() return p.major end
    p.IsTurnActive=function() return p.active end
    p.IsHuman=function() return p.human end
    p.GetDiplomacy=function() return {HasAllied=function(_,other) return p.allies[other]==true end} end
    p.GetProperty,p.SetProperty=store()
    p.GetUnits=function() return {Members=function() return members(NATIVE_UNITS[id]) end,
        FindID=function(_,unit) return NATIVE_UNITS[id][unit] end,
        Destroy=function(_,u) DESTROYS=DESTROYS+1; if not FAIL_DESTROY then NATIVE_UNITS[id][u:GetID()]=nil end end} end
    for j=1,2 do
        local cityid=id*10+j
        CITIES[id][cityid]={GetID=function() return cityid end, GetOwner=function() return id end,
            GetX=function() return j end, GetY=function() return id end}
    end
    p.GetCities=function() return {GetCapitalCity=function() return CITIES[id][id*10+1] end,
        Members=function() return members(CITIES[id]) end} end
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
PlayerManager={GetAliveMajorIDs=function() return {0,1,2} end}
PLOTS={}
Map={GetPlot=function(x,y)
    local key=x..'_'..y
    if not PLOTS[key] then local p={};p.GetProperty,p.SetProperty=store();PLOTS[key]=p end
    return PLOTS[key]
end,GetPlotDistance=function(x,y,ox,oy) return math.max(math.abs(x-ox),math.abs(y-oy)) end}
CityManager={GetCity=function(owner,id) return CITIES[owner] and CITIES[owner][id] end}
local nextUnit=0
Game.GetGreatPeople=function() return { CreatePerson=function(_,owner,individual,x,y)
    SPAWNS=SPAWNS+1; if FAIL_SPAWN then return end
    assert(GameInfo.GreatPersonIndividuals[individual], 'individual INDEX required')
    assert(x==1 and y==owner)
    nextUnit=nextUnit+1; local id=nextUnit
    local u={ individual=individual, GetID=function() return id end,
        GetOwner=function() return owner end, GetType=function() return 99 end }
    u.GetGreatPerson=function()
        if GAMECORE_UNIT_READS and not UI_NATIVE_READ then return {GetIndividual=function() return u.individual end} end
        return {IsGreatPerson=function() return true end, GetIndividual=function() return u.individual end}
    end
    NATIVE_UNITS[owner][id]=u
    -- Deliberately NO return: neither creation nor removal is used as an object-returning API.
end } end
function NEWGOV(owner,leader)
    local row=GameInfo.JAC_LeaderPersonas[leader]; local def=GameInfo.Governors[row.GovernorType]
    local g={promotions={},established=true,neutralized=0}
    g.promotions[GameInfo.GovernorPromotions[row.BasePromotionType].Hash]=true
    g.GetType=function() return def.Index end
    g.IsEstablished=function() return g.established end
    g.GetNeutralizedTurns=function() return g.neutralized end
    g.GetAssignedCity=function() return g.city end
    g.HasPromotion=function(_,hash) return g.promotions[hash]==true end
    NATIVE_GOVS[owner][def.Hash]=g; return g
end
ExposedMembers={}
function events()
    local handlers={}
    local bus=setmetatable({}, {__index=function(t,k)
        -- GameplaySetActivePlayer is not a UI event in the installed game.
        if k=='GameplaySetActivePlayer' then return nil end
        local ev=setmetatable({Add=function(f) handlers[k]=handlers[k] or {}; table.insert(handlers[k],f) end},
            {__call=function(_,...) for _,f in ipairs(handlers[k] or {}) do f(...) end end})
        rawset(t,k,ev); return ev
    end})
    return bus,function(name,...) for _,f in ipairs(handlers[name] or {}) do f(...) end end
end
GameEvents,FIRE=events(); Events,FIREUI=events(); LuaEvents=events()
function RESET_CONTEXTS() GameEvents,FIRE=events(); Events,FIREUI=events(); LuaEvents=events() end
function print() end
function include() end
Mouse={eLClick=1}
Locale={Lookup=function(s) return s end, ToUpper=function(s) return s end}

function control()
    local c={hidden=true,disabled=false,text='',callbacks={}}
    c.SetText=function(_,v) c.text=v end; c.GetText=function() return c.text end
    c.SetIcon=function(_,v,size) c.icon=v;c.size=size end
    c.SetHide=function(_,v) c.hidden=v end; c.SetDisabled=function(_,v) c.disabled=v end
    c.RegisterCallback=function(_,event,f) c.callbacks[event]=f end
    c.CalculateSize=function() end; c.CalculateInternalSize=function() end
    c.DoAutoSize=function() end; c.IsHidden=function() return c.hidden end
    c.SetToolTipString=function(_,v) c.tooltip=v end
    c.SetScrollValue=function(_,v) c.scroll=v end
    return c
end
Controls={}
for _,name in ipairs({'Rows','PersonaPanel','PersonaToggle','Governors','Categories','Nodes','NodesScroll',
    'Name','Portrait','Merit','Commitment','Selection','Effect','Result','Confirmation','ConfirmText','Locks',
    'Confirm','Cancel','Close','NextPerson'}) do Controls[name]=control() end
ContextPtr=control(); ContextPtr.SetInputHandler=function(_,f) INPUT=f end
Keys={VK_F8=119,VK_TAB=9,VK_UP=38,VK_DOWN=40,VK_LEFT=37,VK_RIGHT=39,VK_RETURN=13,VK_ESCAPE=27}
KeyEvents={KeyUp=1}
function INPUT_KEY(key) return {GetKey=function() return key end,GetMessageType=function() return 1 end} end
VIEWS,CATEGORY_VIEWS,TREE_VIEWS={},{},{}
InstanceManager={new=function(_,kind)
    local function list()
        if kind=='PersonaRow' then return VIEWS elseif kind=='PersonaCategory' then return CATEGORY_VIEWS else return TREE_VIEWS end
    end
    return {ResetInstances=function()
        if kind=='PersonaRow' then VIEWS={} elseif kind=='PersonaCategory' then CATEGORY_VIEWS={} else TREE_VIEWS={} end
    end,GetInstance=function()
        local v={}
        for _,n in ipairs({'Root','Portrait','Name','Status','Evolve','Merit','Tree','Route','Node','Effect'}) do v[n]=control() end
        v.Portrait.SetIcon=function(_,icon) v.icon=icon end
        v.Name.SetText=function(_,name) v.name=name end
        v.Evolve.SetDisabled=function(_,disabled) v.disabled=disabled end
        v.Status.SetText=function(_,status) v.status=status end
        local items=list();items[#items+1]=v;return v
    end}
end}
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
    @classmethod
    def setUpClass(cls):
        persona_test_support.LeaderPersonaDatabaseTests.setUpClass()
        cls.db = persona_test_support.LeaderPersonaDatabaseTests.db

    def setUp(self):
        self.lua = LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS)
        self.g = self.lua.globals()
        self.inject_database()
        self.load_contexts()

    def inject_database(self):
        for table,key in [("JAC_PersonaTreeNodes","NodeType"),("JAC_PersonaTreePeople","LeaderType"),
                          ("JAC_PersonaDisabledGovernors","GovernorType"),
                          ("JAC_PersonaCategories","CategoryType"),("JAC_PersonaCategoryAccess","AccessType"),
                          ("JAC_PersonaRouteTemplates","RouteType"),("JAC_PersonaTreeParameters","Name"),
                          ("JAC_PersonaMeritRules","Profile"),("Buildings","BuildingType")]:
            cursor = self.db.execute(f"SELECT rowid, * FROM {table}")
            columns = [d[0] for d in cursor.description]
            rows = []
            for values in cursor:
                row = dict(zip(columns, values)); row["Index"] = row.pop("rowid") - 1
                rows.append(self.lua.table_from({k:v for k,v in row.items() if v is not None}))
            self.g.GameInfo[table] = self.g.collection(self.lua.table_from(rows),key)

    def load_contexts(self):
        self.lua.execute((MOD / "Shared/JAC_PersonaTreeRules.lua").read_text(encoding="utf-8"))
        self.lua.execute((MOD / "Scripts/JAC_LeaderPersona_Gameplay.lua").read_text(encoding="utf-8"))
        self.lua.execute((MOD / "UI/JAC_LeaderPersona_View.lua").read_text(encoding="utf-8"))
        self.lua.execute((MOD / "UI/JAC_LeaderPersona_Tree.lua").read_text(encoding="utf-8"))
        self.api = self.g.ExposedMembers.JAC_LeaderPersona

    def prop(self, field, leader=DATE):
        return self.g.Game.GetProperty(None, f"JAC_PERSONA_{leader}_{field}")

    def join(self):
        self.assertEqual(self.api.Join(0, 1, "CALM", "ASTUTE"), "JOINED")

    def evolve(self):
        self.join()
        self.grant_titles(0, DATE)
        self.g.JAC_EvolveLeader(DATE)
        self.g.DRAIN()
        self.assertEqual(self.prop("STATE"), "GOVERNOR")

    def settled(self, leader=DATE, city=1):
        if leader == DATE:
            self.evolve()
        else:
            source = {ODA:0, HOJO:2}[leader]
            self.assertEqual(self.api.Join(0, source), "JOINED")
            self.grant_titles(0, leader)
            self.g.JAC_EvolveLeader(leader); self.g.DRAIN()
        governor = self.g.NATIVE_GOVS[0][self.g.GameInfo.Governors[self.g.GameInfo.JAC_LeaderPersonas[leader].GovernorType].Hash]
        governor.city = self.g.CITIES[0][city]
        return governor

    def grant_titles(self, owner, leader):
        # Native balance fixture belongs to this test harness, never to the shipped MOD.
        self.g.Players[owner].GetGovernors().ChangeGovernorPoints(None,self.api.EvolutionCost(leader))

    def set_merit(self, amount, leader=DATE):
        self.g.Game.SetProperty(None, f"JAC_PERSONA_{leader}_MERIT", amount)

    def purchase(self, route, depth=1, leader=DATE):
        node = f"{leader}_{route}_{depth}"
        definition = self.g.GameInfo.JAC_PersonaTreeNodes[node]
        return self.request_purchase(0, leader, node, definition.CategoryType, definition.RouteType)

    def request_purchase(self, owner, leader, node, category=None, route=None):
        params = self.lua.table_from({k: v for k, v in {
            "Leader": leader, "Node": node, "ConfirmCategory": category, "ConfirmRoute": route
        }.items() if v is not None})
        self.g.FIRE("JAC_LeaderPersonaPurchase", owner, params)
        return self.g.Players[owner].GetProperty(None, "JAC_PERSONA_PURCHASE_RESULT")

    def test_gameplay_without_ui_governor_methods_uses_fresh_native_reads(self):
        # leaderperson-04: GameCore has only the title APIs. The native reader
        # executes in UI; it must not cache the initial unappointed state.
        self.lua.execute("""
            local original=Players[0].GetGovernors
            local read=ExposedMembers.JAC_LeaderPersonaNativeRead
            UI_NATIVE_READ=false
            GAMECORE_UNIT_READS=true
            Players[0].GetGovernors=function()
                local all=original()
                if UI_NATIVE_READ then return all end
                return {GetGovernorPoints=all.GetGovernorPoints,
                    GetGovernorPointsSpent=all.GetGovernorPointsSpent,
                    ChangeGovernorPoints=all.ChangeGovernorPoints}
            end
            ExposedMembers.JAC_LeaderPersonaNativeRead=function(owner,leader)
                UI_NATIVE_READ=true
                local ok,result=pcall(read,owner,leader)
                UI_NATIVE_READ=false
                if not ok then error(result) end
                return result
            end
        """)
        self.join()
        self.grant_titles(0, DATE)
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "EVOLVING")
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "APPOINTMENT_PENDING")
        governor = self.g.NEWGOV(0, DATE)
        governor.city = self.g.CITIES[0][1]
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "EVOLVED")
        self.api.OnPlayerTurnStarted(0)
        self.assertEqual(self.prop("MERIT"), 2)
        self.set_merit(8)
        self.assertEqual(self.purchase("ENVOYS"), "PURCHASED")
        governor.established = False
        self.g.TURN = 2
        self.api.OnPlayerTurnStarted(0)
        self.assertEqual(self.prop("MERIT"), 0)
        governor.established = True
        governor.city = self.g.CITIES[0][2]
        self.g.TURN = 3
        self.api.OnPlayerTurnStarted(0)
        self.assertEqual(self.prop("MERIT"), 2)

    def test_missing_native_reader_never_creates_or_removes_a_person(self):
        read = self.g.ExposedMembers.JAC_LeaderPersonaNativeRead
        self.g.ExposedMembers.JAC_LeaderPersonaNativeRead = None
        self.assertEqual(self.api.Join(0, 1), "NATIVE_UNAVAILABLE")
        self.assertEqual(self.g.SPAWNS, 0)
        self.g.ExposedMembers.JAC_LeaderPersonaNativeRead = read
        self.join()
        self.grant_titles(0, DATE)
        self.api.PrepareEvolution(0, DATE)
        self.g.NEWGOV(0, DATE)
        self.g.ExposedMembers.JAC_LeaderPersonaNativeRead = None
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "APPOINTMENT_PENDING")
        self.assertEqual((self.prop("STATE"), self.g.DESTROYS), ("EVOLVING", 0))
        self.g.ExposedMembers.JAC_LeaderPersonaNativeRead = read
        self.assertEqual(self.api.CompleteEvolution(0, DATE), "EVOLVED")

    def test_non_great_person_unit_type_never_reads_an_individual_identity(self):
        self.lua.execute("""
            NATIVE_UNITS[0][77]={GetID=function() return 77 end,
                GetOwner=function() return 0 end,GetType=function() return 0 end,
                GetGreatPerson=function() error('ordinary unit has no person identity') end}
        """)
        self.join()
        self.assertEqual(self.g.SPAWNS, 1)
        self.assertEqual(self.prop("UNIT"), 1)

    def test_purchase_is_only_available_through_the_gameplay_event(self):
        self.assertIsNone(self.api.Purchase)

    def test_governor_panel_preserves_foreign_governors_but_blocks_official_and_unjoined_personas(self):
        self.lua.execute("""
            GOVDEFS[#GOVDEFS+1]={GovernorType='GOVERNOR_OTHER_MOD',Index=90,Hash=9090}
            GOVDEFS[#GOVDEFS+1]={GovernorType='GOVERNOR_THE_DEFENDER',Index=91,Hash=9191}
            GameInfo.Governors=collection(GOVDEFS,'GovernorType')
            GameInfo.JAC_PersonaDisabledGovernors=collection({{GovernorType='GOVERNOR_THE_DEFENDER'}},'GovernorType')
            PANEL_CALLS={}
            AddGovernorCandidate=function(def,can) PANEL_CALLS[#PANEL_CALLS+1]='candidate:'..def.GovernorType..':'..tostring(can) end
            AddGovernorShared=function() return {GovernorPortrait=control(),GovernorTitle=control()} end
            OnAppointGovernor=function(id) PANEL_CALLS[#PANEL_CALLS+1]='appoint:'..id end
            OnNameButton=function(id) PANEL_CALLS[#PANEL_CALLS+1]='name:'..id end
            OnPromoteGovernor=function(id,promotion) PANEL_CALLS[#PANEL_CALLS+1]='promote:'..id..':'..promotion end
        """)
        self.lua.execute((MOD / "UI/JAC_LeaderPersona_GovernorPanel.lua").read_text(encoding="utf-8"))
        self.lua.execute("""
            AddGovernorCandidate(GameInfo.Governors[90],true)
            OnAppointGovernor(90); OnPromoteGovernor(90,77); OnNameButton(90)
            AddGovernorCandidate(GameInfo.Governors[91],true)
            OnAppointGovernor(91); OnPromoteGovernor(91,77)
            AddGovernorCandidate(GameInfo.Governors[22],true)
            OnAppointGovernor(22)
        """)
        self.assertEqual(list(self.g.PANEL_CALLS.values()),
                         ["candidate:GOVERNOR_OTHER_MOD:true", "appoint:90", "promote:90:77", "name:90"])

    def test_only_established_unneutralized_city_governance_earns_once_across_reload(self):
        governor = self.settled()
        governor.established = False
        self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual(self.prop("MERIT"), 0)
        governor.established = True; governor.neutralized = 2
        self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual(self.prop("MERIT"), 0)
        governor.neutralized = 0
        self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual((self.prop("MERIT"), self.prop("GOVERN_LAST_TURN")), (2, 1))
        self.g.RESET_CONTEXTS(); self.load_contexts(); self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual(self.prop("MERIT"), 2)
        self.g.TURN = 2; self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual(self.prop("MERIT"), 4)
        governor.city = None
        self.g.TURN = 3; self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual(self.prop("MERIT"), 4)

    def test_personal_profiles_pay_different_amounts_and_never_share_balances(self):
        self.settled(DATE, 1); self.settled(HOJO, 2)
        self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual((self.prop("MERIT"), self.prop("MERIT", HOJO)), (2, 3))
        self.g.FIRE("BuildingConstructed", 0, 2, 0, 100, True)
        self.assertEqual((self.prop("MERIT"), self.prop("MERIT", HOJO)), (2, 9))
        self.set_merit(10, HOJO)
        self.assertEqual(self.purchase("TAX", leader=HOJO), "PURCHASED")
        self.assertEqual((self.prop("MERIT"), self.prop("MERIT", HOJO)), (2, 2))
        self.assertIsNone(self.prop("CATEGORY"))

    def test_building_deeds_reject_repairs_wrong_city_duplicates_and_enforce_cap(self):
        self.settled()
        self.g.FIRE("BuildingConstructed", 0, 1, 0, 100, False)
        self.g.FIRE("BuildingConstructed", 0, 2, 0, 101, True)
        self.g.FIRE("BuildingConstructed", 0, 1, 99999, 100, True)
        self.assertEqual(self.prop("MERIT"), 0)
        for building in range(3): self.g.FIRE("BuildingConstructed", 0, 1, building, 100, True)
        self.assertEqual(self.prop("MERIT"), 6)  # envoy profile: 2 deeds * 3
        self.g.RESET_CONTEXTS(); self.load_contexts(); self.g.TURN = 2
        for building in range(3): self.g.FIRE("BuildingConstructed", 0, 1, building, 100, True)
        self.assertEqual(self.prop("MERIT"), 6)  # capped third event was remembered too
        self.g.FIRE("BuildingConstructed", 0, 1, 3, 100, True)
        self.assertEqual(self.prop("MERIT"), 9)

    def combat(self, loser_id=900, x=3, dead=True, owner=1, attacker=0):
        self.lua.execute(f"""
            function warunit(owner,id,x,dead)
                return {{GetID=function() return id end,GetOwner=function() return owner end,
                    GetX=function() return x end,GetY=function() return 0 end,
                    IsDead=function() return dead end,IsDelayedDeath=function() return false end}}
            end
            NATIVE_UNITS[{attacker}][800]=warunit({attacker},800,1,false)
            NATIVE_UNITS[{owner}][{loser_id}]=warunit({owner},{loser_id},{x},{str(dead).lower()})
        """)
        self.g.FIRE("OnCombatOccurred", attacker, 800, owner, loser_id)

    def test_nearby_victories_pay_personal_merit_and_ignore_nonlethal_or_distant_battles(self):
        self.settled()
        self.combat(dead=False)
        self.combat(x=10)
        self.assertEqual(self.prop("MERIT"), 0)
        self.combat(x=3)
        self.assertEqual(self.prop("MERIT"), 2)
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.combat(x=3)
        self.assertEqual(self.prop("MERIT"), 2)
        self.combat(loser_id=901)
        self.combat(loser_id=902)
        self.assertEqual(self.prop("MERIT"), 4)

    def test_deferred_building_and_victory_notifications_survive_reload_and_pay_once(self):
        self.settled()
        self.g.ExposedMembers.JAC_LeaderPersonaNativeRead = None
        self.g.FIRE("BuildingConstructed", 0, 1, 0, 100, True)
        self.combat(x=3)
        self.assertEqual(self.prop("MERIT"), 0)
        self.assertIsNone(self.prop("DEED_BUILDING_100_0"))
        pending = self.g.Game.GetProperty(None, "JAC_PERSONA_PENDING_DEEDS")
        self.assertEqual(len(list(pending.keys())), 2)
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.g.FIREUI("LoadScreenClose")
        self.assertEqual(len(self.g.REQUESTS), 1)
        self.g.DRAIN()
        self.assertEqual(self.prop("MERIT"), 5)  # original building 3 + nearby victory 2
        self.assertIsNone(self.g.Game.GetProperty(None, "JAC_PERSONA_PENDING_DEEDS"))
        self.g.FIRE("JAC_LeaderPersonaNativeReady", 0)
        self.g.FIRE("BuildingConstructed", 0, 1, 0, 100, True)
        self.combat(x=3)
        self.assertEqual(self.prop("MERIT"), 5)

    def test_unready_native_reads_do_not_finalize_alliance_or_ai_turn(self):
        self.settled(); self.set_merit(100)
        self.g.Players[0].human = False
        self.g.Players[0].allies[2] = True
        self.g.ExposedMembers.JAC_LeaderPersonaNativeRead = None
        self.api.OnPlayerTurnStarted(0)
        self.assertEqual(self.prop("MERIT"), 100)
        for field in ("ALLY_SEEN_2", "AI_LAST_TURN", "GOVERN_LAST_TURN", "CATEGORY"):
            self.assertIsNone(self.prop(field))
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.g.FIRE("JAC_LeaderPersonaNativeReady", 0)
        self.assertEqual(self.prop("MERIT"), 102)  # govern 2 + alliance 8 - first skill 8
        self.assertEqual(self.prop("NODE_"+DATE+"_ENVOYS_1"), 1)
        self.assertEqual(self.prop("ALLY_SEEN_2"), 1)
        self.assertEqual(self.prop("AI_LAST_TURN"), 1)
        self.g.FIRE("JAC_LeaderPersonaNativeReady", 0)
        self.api.OnPlayerTurnStarted(0)
        self.assertEqual(self.prop("MERIT"), 102)

    def test_deferred_deeds_keep_original_turn_caps_and_duplicate_records(self):
        self.settled()
        self.g.ExposedMembers.JAC_LeaderPersonaNativeRead = None
        for building in range(3): self.g.FIRE("BuildingConstructed", 0, 1, building, 100, True)
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.g.FIRE("JAC_LeaderPersonaNativeReady", 0)
        self.assertEqual(self.prop("MERIT"), 6)
        self.g.TURN = 2
        for building in range(3): self.g.FIRE("BuildingConstructed", 0, 1, building, 100, True)
        self.assertEqual(self.prop("MERIT"), 6)

    def test_deferred_events_preserve_ai_turn_before_later_construction(self):
        self.settled(); self.set_merit(5)
        self.g.Players[0].human = False
        self.g.ExposedMembers.JAC_LeaderPersonaNativeRead = None
        self.api.OnPlayerTurnStarted(0)
        self.g.FIRE("BuildingConstructed", 0, 1, 0, 100, True)
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.g.FIRE("JAC_LeaderPersonaNativeReady", 0)
        self.assertEqual(self.prop("MERIT"), 10)  # 5 + govern 2, then building 3
        self.assertIsNone(self.prop("CATEGORY"))  # only 7 at AI's original decision point
        self.assertEqual(self.prop("AI_LAST_TURN"), 1)

    def test_historical_pending_deed_is_kept_without_inventing_eligibility(self):
        self.settled()
        self.g.ExposedMembers.JAC_LeaderPersonaNativeRead = None
        self.g.FIRE("BuildingConstructed", 0, 1, 0, 100, True)
        self.g.TURN = 2
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.g.FIRE("JAC_LeaderPersonaNativeReady", 0)
        self.assertEqual(self.prop("MERIT"), 0)
        pending = self.g.Game.GetProperty(None, "JAC_PERSONA_PENDING_DEEDS")
        self.assertEqual(len(list(pending.keys())), 1)
        event = pending[DATE+"_BUILDING_100_0"]
        self.assertEqual((event.turn, event.cityID, event.identity), (1, 1, "100_0"))
        self.assertEqual(self.g.Game.GetProperty(None, "JAC_PERSONA_PENDING_NATIVE_HISTORY"), 1)

    def test_initial_alliances_and_repeated_renewals_do_not_pay_merit(self):
        self.g.Players[0].allies[1] = True
        self.settled()
        self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual(self.prop("MERIT"), 2)
        self.g.Players[0].allies[2] = True
        self.g.TURN = 2; self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual(self.prop("MERIT"), 12)  # governance 2 + new alliance 8
        self.g.Players[0].allies[2] = False; self.g.TURN = 3; self.g.FIRE("PlayerTurnStarted", 0)
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.g.Players[0].allies[2] = True; self.g.TURN = 4; self.g.FIRE("PlayerTurnStarted", 0)
        self.assertEqual(self.prop("MERIT"), 16)  # two governance turns only

    def test_skill_purchase_requires_own_person_prerequisite_merit_and_confirmation(self):
        self.settled()
        first, second = DATE+"_ENVOYS_1", DATE+"_ENVOYS_2"
        self.assertEqual(self.request_purchase(2, DATE, first, "CONTACT", "ENVOYS"), "NOT_OWNED")
        self.assertEqual(self.request_purchase(0, DATE, ODA+"_EXCHANGE_1", "CONTACT", "EXCHANGE"), "INVALID_NODE")
        self.assertEqual(self.purchase("ENVOYS", 2), "PREREQ_LOCKED")
        self.assertEqual(self.purchase("ENVOYS"), "NO_MERIT")
        self.set_merit(8)
        self.assertEqual(self.request_purchase(0, DATE, first), "CONFIRM_REQUIRED")
        self.assertEqual(self.request_purchase(0, DATE, first, "ADMIN", "ENVOYS"), "CONFIRM_REQUIRED")
        self.assertIsNone(self.prop("CATEGORY"))
        self.assertEqual(self.prop("MERIT"), 8)
        titles = (self.g.Players[0].points, self.g.Players[0].spent, len(self.g.GRANTS[0]))
        self.assertEqual(self.purchase("ENVOYS"), "PURCHASED")
        self.assertEqual(self.prop("MERIT"), 0)

        self.assertEqual((self.g.Players[0].points, self.g.Players[0].spent, len(self.g.GRANTS[0])), titles)
        self.assertEqual(self.prop("CATEGORY"), "CONTACT")
        self.assertEqual(self.prop("BRANCH_CONTACT_RELATIONS"), "ENVOYS")
        self.assertEqual(self.prop("NODE_"+first), 1)
        self.assertEqual(self.purchase("ENVOYS"), "OWNED")
        self.assertEqual(self.prop("MERIT"), 0)

    def test_malformed_gameplay_requests_do_not_reach_native_lookups_or_change_state(self):
        self.settled(); self.set_merit(8)
        for leader in (None, 0, self.lua.table()):
            self.assertEqual(self.request_purchase(0,leader,DATE+"_ENVOYS_1","CONTACT","ENVOYS"),"NOT_OWNED")
            self.assertFalse(self.g.JAC_TreeOpen(leader))
        for node in (None,0,self.lua.table(),"unknown"):
            self.assertEqual(self.request_purchase(0,DATE,node,"CONTACT","ENVOYS"),"INVALID_NODE")
        self.assertEqual(self.prop("MERIT"),8)
        self.assertIsNone(self.prop("CATEGORY"))
    def test_irreversible_category_and_exclusive_sibling_locks_survive_load(self):
        self.settled(); self.set_merit(100)
        self.assertEqual(self.purchase("ENVOYS"), "PURCHASED")
        self.assertEqual(self.purchase("WORKS"), "CATEGORY_LOCKED")
        self.assertEqual(self.purchase("EXCHANGE"), "BRANCH_LOCKED")
        self.assertEqual(self.purchase("ENVOYS", 3), "PREREQ_LOCKED")
        self.assertEqual(self.purchase("ENVOYS", 2), "PURCHASED")
        self.assertEqual(self.purchase("ENVOYS", 3), "PURCHASED")
        self.assertEqual(self.purchase("HOSTING"), "PURCHASED")
        self.assertEqual(self.purchase("MARKET"), "BRANCH_LOCKED")
        self.assertEqual(self.purchase("SIGNATURE"), "PURCHASED")
        balance = self.prop("MERIT")
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.assertEqual(self.prop("MERIT"), balance)
        self.assertEqual(self.purchase("ENVOYS"), "OWNED")
        self.assertEqual(self.purchase("EXCHANGE"), "BRANCH_LOCKED")
        self.assertEqual(self.purchase("WORKS"), "CATEGORY_LOCKED")
        self.assertEqual(self.purchase("MARKET"), "BRANCH_LOCKED")
        self.assertEqual(self.prop("MERIT"), balance)

    def test_all_six_shared_promotions_and_three_signature_nodes_can_be_acquired(self):
        self.settled(); self.set_merit(200)
        for route in ("ENVOYS", "HOSTING", "SIGNATURE"):
            for depth in (1,2,3): self.assertEqual(self.purchase(route, depth), "PURCHASED")
        self.assertEqual(self.prop("MERIT"), 200-38-47-38)
        self.assertIsNone(self.api.Refund)
        self.assertIsNone(self.api.Respec)

    def test_owned_node_flags_follow_native_assignment_and_new_city_centres(self):
        governor = self.settled(); self.set_merit(8); self.purchase("ENVOYS")
        modifier = self.g.GameInfo.JAC_PersonaTreeNodes[DATE+"_ENVOYS_1"].ModifierId
        for x in (1,2): self.assertEqual(self.g.Map.GetPlot(x,0).GetProperty(None,modifier),1)
        self.g.JAC_AssignLeader(DATE,2); self.g.DRAIN()
        self.assertEqual(governor.city.GetID(),2)
        self.assertEqual(list(governor.promotions.values()),[True])  # no native promotion or title added
        self.lua.execute("CITIES[0][3]={GetID=function() return 3 end,GetOwner=function() return 0 end,GetX=function() return 3 end,GetY=function() return 0 end}")
        self.g.FIRE("CityBuilt",0,3,3,0)
        self.assertEqual(self.g.Map.GetPlot(3,0).GetProperty(None,modifier),1)

    def test_ai_uses_personality_role_preference_and_only_one_purchase_each_turn(self):
        self.settled(); self.set_merit(100)
        self.g.Players[0].human = False
        self.g.FIRE("PlayerTurnStarted",0)
        self.assertEqual((self.prop("CATEGORY"),self.prop("BRANCH_CONTACT_RELATIONS")),("CONTACT","ENVOYS"))
        self.assertEqual(self.prop("MERIT"),94)  # +2 governing -8 strong route
        self.g.RESET_CONTEXTS(); self.load_contexts(); self.g.FIRE("PlayerTurnStarted",0)
        self.assertEqual(self.prop("MERIT"),94)
        self.assertIsNone(self.prop("NODE_"+DATE+"_ENVOYS_2"))
        self.g.TURN=2; self.g.FIRE("PlayerTurnStarted",0)
        self.assertEqual(self.prop("NODE_"+DATE+"_ENVOYS_2"),1)
        self.assertEqual(self.prop("MERIT"),84)

    def test_tree_confirmation_lists_losses_and_cancel_changes_nothing(self):
        self.settled(); self.set_merit(8)
        self.assertTrue(self.g.JAC_TreeOpen(DATE))
        self.assertTrue(self.g.JAC_TreeReview(DATE+"_ENVOYS_1"))
        self.assertFalse(self.g.Controls.Confirmation.hidden)
        self.assertIsNone(self.prop("CATEGORY"))
        self.assertEqual(self.prop("MERIT"),8)
        # Locale stub returns the key; capture values separately to prove loss lists.
        categories,routes=self.g.JAC_PersonaTreeRules.Locks(DATE,self.g.GameInfo.JAC_PersonaTreeNodes[DATE+"_ENVOYS_1"])
        self.assertEqual({r.CategoryType for r in categories.values()},{"MILITARY","ADMIN"})
        self.assertEqual([r.RouteType for r in routes.values()],["EXCHANGE"])
        self.g.JAC_TreeCancel()
        self.assertTrue(self.g.Controls.Confirmation.hidden)
        self.assertFalse(self.g.JAC_TreeConfirm())
        self.assertEqual((self.prop("MERIT"),len(self.g.REQUESTS)),(8,0))

    def test_tree_spends_only_via_gameplay_request_and_rechecks_queued_duplicates(self):
        self.settled(); self.set_merit(30); self.g.JAC_TreeOpen(DATE)
        node=DATE+"_ENVOYS_1"
        for _ in range(2):
            self.assertTrue(self.g.JAC_TreeReview(node)); self.assertTrue(self.g.JAC_TreeConfirm())
        self.assertEqual(len(self.g.REQUESTS),2)
        self.assertTrue(all(r.op==1 and r.params.OnStart=="JAC_LeaderPersonaPurchase" for r in self.g.REQUESTS.values()))
        self.assertEqual(self.prop("MERIT"),30)
        self.g.DRAIN()
        self.assertEqual(self.prop("MERIT"),22)
        self.assertEqual(self.prop("NODE_"+node),1)
        self.assertEqual((self.g.Players[0].points,self.g.Players[0].spent),(1,1))

    def test_queued_conflicting_confirmations_cannot_buy_sibling_or_other_category(self):
        self.settled(); self.set_merit(40)
        # All three screens are reviewed before the first gameplay request is processed.
        for route in ("ENVOYS", "EXCHANGE", "WORKS"):
            node=DATE+"_"+route+"_1"
            self.assertTrue(self.g.JAC_TreeOpen(DATE,node))
            self.assertTrue(self.g.JAC_TreeConfirm())
        self.g.DRAIN()
        self.assertEqual(self.prop("MERIT"),32)
        self.assertEqual(self.prop("NODE_"+DATE+"_ENVOYS_1"),1)
        self.assertIsNone(self.prop("NODE_"+DATE+"_EXCHANGE_1"))
        self.assertIsNone(self.prop("NODE_"+DATE+"_WORKS_1"))
        self.assertEqual((self.prop("CATEGORY"),self.prop("BRANCH_CONTACT_RELATIONS")),("CONTACT","ENVOYS"))

    def test_tree_rechecks_stale_balance_at_confirmation_and_respects_inactive_turn(self):
        self.settled(); self.set_merit(8); self.g.JAC_TreeOpen(DATE)
        self.g.JAC_TreeReview(DATE+"_ENVOYS_1"); self.set_merit(0)
        self.assertFalse(self.g.JAC_TreeConfirm())
        self.assertEqual(len(self.g.REQUESTS),0)
        self.set_merit(8); self.g.Players[0].active=False
        self.assertFalse(self.g.JAC_TreeReview(DATE+"_ENVOYS_1"))
        self.assertFalse(self.g.JAC_TreeConfirm())
        self.assertIsNone(self.prop("CATEGORY"))

    def test_tree_only_shows_matching_new_purchase_receipts_and_never_replays_old_results(self):
        self.settled(); self.set_merit(30)
        p=self.g.Players[0]
        p.SetProperty(None,"JAC_PERSONA_RESULT","PURCHASED")
        p.SetProperty(None,"JAC_PERSONA_PURCHASE_SERIAL",5)
        p.SetProperty(None,"JAC_PERSONA_PURCHASE_LEADER",DATE)
        p.SetProperty(None,"JAC_PERSONA_PURCHASE_NODE",DATE+"_ENVOYS_1")
        p.SetProperty(None,"JAC_PERSONA_PURCHASE_RESULT","PURCHASED")
        self.g.JAC_TreeOpen(DATE); self.g.FIREUI("GameCoreEventPublishComplete")
        self.assertEqual(self.g.Controls.Result.text,"")
        self.g.JAC_TreeReview(DATE+"_ENVOYS_1"); self.g.JAC_TreeConfirm()
        self.assertEqual(self.g.REQUESTS[1].params.RequestSerial,6)
        self.g.FIREUI("GameCoreEventPublishComplete")
        self.assertEqual(self.g.Controls.Result.text,"")
        self.g.DRAIN()
        self.assertEqual(self.g.Controls.Result.text,"LOC_JAC_PERSONA_NODE_STATUS_PURCHASED")
        self.g.JAC_TreeOpen(DATE); self.g.FIREUI("GameCoreEventPublishComplete")
        self.assertEqual(self.g.Controls.Result.text,"")
        self.g.RESET_CONTEXTS(); self.load_contexts(); self.g.JAC_TreeOpen(DATE)
        self.g.JAC_TreeReview(DATE+"_ENVOYS_2"); self.g.JAC_TreeConfirm()
        self.assertEqual(self.g.REQUESTS[1].params.RequestSerial,7)
        p.SetProperty(None,"JAC_PERSONA_PURCHASE_SERIAL",7)
        p.SetProperty(None,"JAC_PERSONA_PURCHASE_NODE",DATE+"_HOSTING_1")
        self.g.FIREUI("GameCoreEventPublishComplete")
        self.assertEqual(self.g.Controls.Result.text,"")
        self.g.DRAIN()
        self.assertEqual(self.g.Controls.Result.text,"LOC_JAC_PERSONA_NODE_STATUS_PURCHASED")

    def test_keyboard_can_review_cancel_commit_switch_categories_and_cycle_governors(self):
        self.settled(); self.set_merit(20); self.g.JAC_TreeOpen(DATE)
        self.assertTrue(self.g.JAC_TreeInput(self.g.INPUT_KEY(13)))  # Enter reviews first preferred route
        self.assertFalse(self.g.Controls.Confirmation.hidden)
        self.g.JAC_TreeInput(self.g.INPUT_KEY(27))  # Esc cancels
        self.assertTrue(self.g.Controls.Confirmation.hidden)
        self.g.JAC_TreeInput(self.g.INPUT_KEY(13)); self.g.JAC_TreeInput(self.g.INPUT_KEY(13)); self.g.DRAIN()
        self.assertEqual(self.prop("NODE_"+DATE+"_ENVOYS_1"),1)
        self.g.JAC_TreeInput(self.g.INPUT_KEY(40)); self.g.JAC_TreeInput(self.g.INPUT_KEY(39))
        self.assertTrue(0 <= self.g.Controls.NodesScroll.scroll <= 1)
        self.g.JAC_TreeInput(self.g.INPUT_KEY(9))
        self.g.JAC_TreeInput(self.g.INPUT_KEY(27))
        self.assertTrue(self.g.ContextPtr.hidden)

    def test_promotion_titles_fixture_is_removed_and_official_candidates_are_hidden(self):
        self.join()
        self.assertIsNone(self.api.GrantTestTitle)
        self.assertEqual(self.g.Players[0].points,0)
        self.lua.execute("""
            CANDIDATES,APPOINTED={},{}
            AddGovernorCandidate=function(def) CANDIDATES[#CANDIDATES+1]=def.GovernorType end
            AddGovernorShared=function(def) return {GovernorPortrait=control(),GovernorTitle=control()} end
            OnAppointGovernor=function(id) APPOINTED[#APPOINTED+1]=id end
            GameInfo.Governors[99]={GovernorType='GOVERNOR_THE_EDUCATOR',Index=99,Hash=999}
        """)
        self.lua.execute((MOD/"UI/JAC_LeaderPersona_GovernorPanel.lua").read_text(encoding="utf-8"))
        self.g.AddGovernorCandidate(self.g.GameInfo.Governors[99],True); self.g.OnAppointGovernor(99)
        self.assertEqual((len(self.g.CANDIDATES),len(self.g.APPOINTED)),(0,0))

    def test_void_creation_keeps_named_individual_and_metadata(self):
        self.join()
        self.assertEqual(self.prop("STATE"), "GREAT_PERSON")
        self.assertEqual((self.prop("SOURCE"), self.prop("OWNER"), self.g.SPAWNS), (1, 0, 1))
        self.assertEqual((self.prop("NAME"), self.prop("PORTRAIT")), (f"LOC_{DATE}_NAME", f"ICON_{DATE}"))
        self.assertEqual((self.prop("PERSONALITY"), self.prop("TRAIT")), ("CALM", "ASTUTE"))
        self.assertEqual(self.g.NATIVE_UNITS[0][1].individual, 12)
        self.assertEqual(self.g.Players[1].alive, True)

    def test_synchronized_join_validates_source_and_replays_only_once(self):
        self.g.JAC_RequestLeaderJoin(1)
        self.assertEqual(self.g.SPAWNS, 0)
        self.assertIsNone(self.prop("OWNER"))
        self.g.DRAIN()
        self.assertEqual((self.prop("OWNER"), self.prop("SOURCE"), self.g.SPAWNS), (0, 1, 1))
        self.g.FIRE("JAC_LeaderPersonaJoin", 0, self.lua.table_from({"Source": 1}))
        self.assertEqual(self.g.SPAWNS, 1)
        self.assertEqual(self.g.Players[0].GetProperty(None, "JAC_PERSONA_RESULT"), "ALREADY_JOINED")
        for params in [None, self.lua.table_from({}), self.lua.table_from({"Source": "1"}),
                       self.lua.table_from({"Source": 0})]:
            self.g.FIRE("JAC_LeaderPersonaJoin", 0, params)
            self.assertEqual(self.g.SPAWNS, 1)
            self.assertEqual(self.g.Players[0].GetProperty(None, "JAC_PERSONA_RESULT"), "INVALID")

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

    def test_two_pending_evolutions_cannot_share_the_native_appointment_title(self):
        self.join()
        self.assertEqual(self.api.Join(0, 2), "JOINED")
        self.g.Players[0].points = 3
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "EVOLVING")
        self.assertEqual(self.api.PrepareEvolution(0, HOJO), "NO_TITLE")
        self.assertEqual((self.prop("STATE", HOJO), self.g.Players[0].points), ("GREAT_PERSON", 2))
        self.assertIsNone(self.prop("EVOLUTION_SURCHARGE", HOJO))
        self.g.JAC_RefreshPersonas()
        self.assertTrue(self.g.VIEWS[1].disabled)
        self.assertTrue(self.g.VIEWS[2].disabled)
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.assertEqual(self.api.PrepareEvolution(0, HOJO), "NO_TITLE")
        self.g.JAC_AppointLeader(DATE); self.g.DRAIN()
        self.assertEqual(self.prop("STATE"), "GOVERNOR")
        self.assertEqual(self.api.PrepareEvolution(0, HOJO), "NO_TITLE")
        self.g.Players[0].points += 1
        self.assertEqual(self.api.PrepareEvolution(0, HOJO), "EVOLVING")
        self.g.JAC_AppointLeader(HOJO); self.g.DRAIN()
        self.assertEqual(self.prop("STATE", HOJO), "GOVERNOR")
        self.assertEqual((self.g.Players[0].points, self.g.Players[0].spent), (2, 2))
        self.assertEqual(list(self.g.GRANTS[0].values()), [-1, -1])

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

    def test_partial_native_payment_restores_balance_without_appointing_or_recharging(self):
        self.g.GameInfo.JAC_LeaderPersonas[DATE].EvolutionCost = 3
        self.join()
        self.g.Players[0].points = 3
        self.lua.execute("""
            local original=Players[0].GetGovernors
            Players[0].GetGovernors=function()
                local g=original()
                local change=g.ChangeGovernorPoints
                g.ChangeGovernorPoints=function(_,delta)
                    change(g, delta < 0 and -1 or delta)
                end
                return g
            end
        """)
        for _ in range(2):
            self.g.JAC_EvolveLeader(DATE); self.g.DRAIN()
            self.assertEqual(self.g.Players[0].GetProperty(None, "JAC_PERSONA_RESULT"), "TITLE_PAYMENT_FAILED")
            self.assertEqual((self.prop("STATE"), self.prop("EVOLUTION_SURCHARGE"), self.g.DESTROYS),
                             ("GREAT_PERSON", None, 0))
            self.assertEqual((self.g.Players[0].points, self.g.Players[0].spent), (3, 0))
            self.assertIsNone(self.g.NATIVE_GOVS[0][1002])
        self.assertEqual(list(self.g.GRANTS[0].values()), [-1, 1, -1, 1])

    def test_upper_cost_consumes_two_extra_titles_and_one_native_once(self):
        self.g.GameInfo.JAC_LeaderPersonas[DATE].EvolutionCost = 3
        self.join()
        self.grant_titles(0, DATE)
        self.g.JAC_EvolveLeader(DATE); self.g.DRAIN()
        self.assertEqual((self.prop("STATE"), self.prop("EVOLUTION_COST"), self.prop("EVOLUTION_SURCHARGE")),
                         ("GOVERNOR", 3, 2))
        self.assertEqual((self.g.Players[0].points, self.g.Players[0].spent), (1, 1))
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "ALREADY_EVOLVED")
        self.assertIsNone(self.api.GrantTestTitle)
        self.assertEqual(list(self.g.GRANTS[0].values()), [3, -2])
        self.assertEqual((self.g.DESTROYS, self.g.Players[0].points, self.g.Players[0].spent), (1, 1, 1))

    def test_joined_cost_is_fixed_even_if_balance_data_changes(self):
        self.join()
        self.g.GameInfo.JAC_LeaderPersonas[DATE].EvolutionCost = 3
        self.assertEqual(self.api.EvolutionCost(DATE), 2)

    def test_native_individual_identity_is_resolved_through_gameinfo(self):
        self.join()
        # The official UI resolves the native token through GameInfo. Do not assume its number
        # must equal the creation parameter; preserve identity whenever the lookup resolves it.
        self.lua.execute('''
            GameInfo.GreatPersonIndividuals[9002]=GameInfo.GreatPersonIndividuals[12]
            NATIVE_UNITS[0][1].individual=9002
        ''')
        self.grant_titles(0, DATE)
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
        self.grant_titles(0, DATE)
        self.assertEqual(self.api.PrepareEvolution(0, DATE), "PERSON_MISSING")
        self.assertEqual(self.g.DESTROYS, 0)

    def test_pending_appointment_keeps_unit_until_native_success(self):
        self.join()
        self.grant_titles(0, DATE)
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
        self.grant_titles(0, DATE)
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
        self.grant_titles(0, DATE)
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
        self.grant_titles(0, DATE)
        self.api.PrepareEvolution(0, DATE)
        self.g.NEWGOV(0, DATE)
        self.g.RESET_CONTEXTS(); self.load_contexts()
        self.g.FIRE("OnGameTurnStarted")
        self.assertEqual((self.prop("STATE"), self.g.DESTROYS), ("GOVERNOR", 1))
        self.g.RESET_CONTEXTS(); self.load_contexts(); self.g.FIRE("OnGameTurnStarted")
        self.assertEqual((self.prop("PERSONALITY"), self.prop("TRAIT"), self.g.DESTROYS), ("CALM", "ASTUTE", 1))

    def test_ui_uses_native_operation_indices_and_engine_requester(self):
        self.join()
        self.grant_titles(0, DATE)
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

    def test_official_governor_panel_hides_unjoined_candidates_and_keeps_native_buttons(self):
        self.lua.execute('''
            CANDIDATES, APPOINTED = {}, {}
            AddGovernorCandidate=function(def,available) CANDIDATES[#CANDIDATES+1]=def.GovernorType end
            AddGovernorShared=function(def) return {GovernorPortrait={SetIcon=function(_,icon,size) ICON,ICON_SIZE=icon,size end}, GovernorTitle=control()} end
            OnAppointGovernor=function(id) APPOINTED[#APPOINTED+1]=id end
        ''')
        self.lua.execute((MOD / "UI/JAC_LeaderPersona_GovernorPanel.lua").read_text(encoding="utf-8"))
        gov = self.g.GameInfo.Governors[22]
        self.g.AddGovernorCandidate(gov, True); self.g.OnAppointGovernor(22)
        self.assertEqual((len(self.g.CANDIDATES), len(self.g.APPOINTED)), (0, 0))
        self.join()
        self.g.AddGovernorCandidate(gov, True)
        self.assertEqual(len(self.g.CANDIDATES), 0)
        self.grant_titles(0, DATE); self.api.PrepareEvolution(0, DATE)
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
            Controls.GovernorNameLabel=control(); Controls.GovernorNameLabel.GetText=function() return 'LOC_{DATE}_NAME' end
            Controls.GovernorPortrait={{SetIcon=function(_,icon,size) ICON,ICON_SIZE=icon,size end}}
        ''')
        self.lua.execute((MOD / "UI/JAC_LeaderPersona_GovernorDetails.lua").read_text(encoding="utf-8"))
        self.g.Refresh()
        self.assertEqual((self.g.REFRESHES, self.g.ICON, self.g.ICON_SIZE), (1, f"ICON_{DATE}", 256))


if __name__ == "__main__":
    unittest.main()
