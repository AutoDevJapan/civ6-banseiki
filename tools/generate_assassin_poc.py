"""Rebuild Assassin PoC packaging/localisation and self-checking live scenarios.

The specification remains SSOT 9.5; coefficients live in JAC_Assassin_Rules.lua.
No game installation, save or user configuration is changed by this generator.
"""
from pathlib import Path
import copy
import json
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_Assassin"
MOD_ID = "fbb5219d-d7b9-4b31-884f-057f623476a0"
MAP_ID = "403be889-1455-4de7-967b-ad54c0ff3431"
TEXT = {
    "MOD_NAME": ("JAC PoC：暗殺者", "JAC PoC: Assassin", "JAC PoC: убийца"),
    "MOD_DESCRIPTION": ("日本マップの暗殺者育成・任務・保存の試作。死亡後の四分岐は明示した推奨案で処理。", "Japan-map training, missions and persistence. Post-death branches use explicit proposed policies.", "Подготовка, задания и сохранение на картах Японии. После гибели правителя применяются явно предложенные правила."),
    "NINJA_NAME": ("暗殺忍者", "Assassin Ninja", "Ниндзя-убийца"),
    "COMMON_NAME": ("暗殺者", "Assassin", "Убийца"),
    "UNIT_DESCRIPTION": ("地図を歩く暗殺者。都市を指定して通常100ターン・最短20ターンで育成。完成後は全国10ターン育成不可。", "An assassin who travels on the map. Train in one city: normally 100 turns, at least 20. After completion, training is locked nationwide for 10 turns.", "Убийца, передвигающийся по карте. Подготовка в одном городе: обычно 100 ходов, минимум 20. После завершения подготовка запрещена во всей стране на 10 ходов."),
    "INFILTRATION": ("潜入", "Infiltration", "Проникновение"),
    "INFILTRATION_DESCRIPTION": ("他国の領土に入れる。", "May enter foreign territory.", "Может входить на чужую территорию."),
    "TITLE": ("暗殺者", "Assassins", "Убийцы"),
    "CLOSE": ("閉じる", "Close", "Закрыть"),
    "PRODUCE": ("育成を開始", "Start training", "Начать подготовку"),
    "CANCEL": ("育成を中止", "Cancel training", "Отменить подготовку"),
    "ATTEMPT": ("暗殺を実行", "Attempt assassination", "Начать покушение"),
    "WAIT": ("任務中", "On a mission", "На задании"),
    "SELECT": ("育成は都市を指定。暗殺は地図上の暗殺者を選び、対象の近くへ移動して実行。", "Choose a city for training. Select an assassin on the map and move near a target to attempt a mission.", "Выберите город для подготовки. Выберите убийцу на карте и переместите его к цели, чтобы начать задание."),
    "STATUS": ("育成ロック：あと{1}ターン／暗殺コマンド待機：あと{2}ターン", "Training lock: {1} turns / command wait: {2} turns", "Запрет подготовки: {1} ходов / ожидание команды: {2} ходов"),
    "CITY": ("{1}：育成{2}ターン", "{1}: train in {2} turns", "{1}: подготовка за {2} ходов"),
    "TRAINING": ("都市{1}で育成中：あと{2}ターン", "Training in city {1}: {2} turns remaining", "Подготовка в городе {1}: осталось {2} ходов"),
    "MISSION": ("{1}への任務：あと{2}ターン", "Mission against {1}: {2} turns remaining", "Задание против {1}: осталось {2} ходов"),
    "TARGET": ("{1}・{2}：基本{3}%、補正後{5}%、{4}ターン", "{1}, {2}: base {3}%, adjusted {5}%, {4} turns", "{1}, {2}: базовый шанс {3}%, итоговый {5}%, {4} ходов"),
    "GOVERNOR": ("Governor", "Governor", "Губернатор"),
    "SUCCESSOR": ("後継者", "Successor", "Преемник"),
    "PRODUCED": ("暗殺者の育成が完了。全国の次の育成まで{1}ターン。", "Training complete. Nationwide training unlocks in {1} turns.", "Подготовка завершена. Подготовка во всей стране станет доступна через {1} ходов."),
    "RESULT": ("暗殺任務：{1}", "Assassination mission: {1}", "Задание убийцы: {1}"),
    "RESULT_LEADER_FREE_STATE": ("指導者死亡。都市とUnitは同じ国の評議会が引き継ぐ。", "Leader dead. A council retains the cities and units in the same nation.", "Правитель погиб. Города и юниты остаются у совета той же страны."),
    "RESULT_LEADER_ABSORPTION": ("指導者死亡。隣国へ都市を移管し、旧軍を解散。存命人物は受入国へ加入。", "Leader dead. Cities transferred to a neighbor; old units disbanded and surviving people enrolled.", "Правитель погиб. Города переданы соседу; войска распущены, выжившие личности приняты."),
    "RESULT_LEADER_SUCCESSION": ("指導者死亡。登録済みの後継者が同じ国と資産を継承。", "Leader dead. A registered successor inherits the same nation and assets.", "Правитель погиб. Зарегистрированный преемник наследует страну и имущество."),
    "RESULT_GOVERNOR_DEAD_PENDING": ("Governor死亡。既存枠の処理は未確認。", "Governor dead; native-slot handling is unverified.", "Губернатор погиб; обработка штатного слота не проверена."),
    "RESULT_UNIT_KILLED": ("対象Unit死亡。", "Target unit killed.", "Целевой юнит погиб."),
    "RESULT_FAILED": ("失敗。暗殺者死亡。", "Failed. Assassin killed.", "Неудача. Убийца погиб."),
    "RESULT_TARGET_GONE": ("対象が失われたため任務終了。", "Mission ended: target no longer exists.", "Задание завершено: цель больше не существует."),
    "RESULT_ACTOR_GONE": ("暗殺者消失。すべての結果を取消し、固定の友好度・不平の罰則を適用。", "Assassin lost. All results cancelled; fixed friendship and grievance penalties applied.", "Убийца потерян. Все результаты отменены; применены фиксированные штрафы к дружбе и недовольству."),
    "REASON_PRODUCING": ("別の都市で既に育成中。", "Training is already in progress.", "Подготовка уже идёт."),
    "REASON_PRODUCTION_LOCK": ("完成後の全国育成ロック中。", "Nationwide training lock is active.", "Действует запрет подготовки во всей стране."),
    "REASON_MISSION_ACTIVE": ("自国の暗殺任務が進行中。", "A national assassination mission is active.", "Задание убийцы от вашей страны уже идёт."),
    "REASON_COMMAND_LOCK": ("前回の実行からの全国待機中。", "Nationwide command waiting period is active.", "Действует общестрановое ожидание после предыдущей команды."),
    "REASON_TARGET_GONE": ("対象が存在しない。", "Target no longer exists.", "Цель больше не существует."),
    "REASON_TARGET_UNKNOWN": ("対象の所在を確認する。", "Discover the target location.", "Установите местонахождение цели."),
    "REASON_CITY_GONE": ("育成する都市が存在しない。", "Training city no longer exists.", "Города для подготовки больше не существует."),
    "REASON_TARGET_KIND": ("この対象には実行できない。", "Invalid target.", "Недопустимая цель."),
    "REASON_ASSASSIN_REQUIRED": ("暗殺者Unitを選択する。", "Select an assassin unit.", "Выберите юнит убийцы."),
    "REASON_TOO_FAR": ("対象の近くまで移動する。", "Move closer to the target.", "Подойдите ближе к цели."),
    "REASON_DURATION_UNSET": ("任務期間が未設定。", "Mission duration is unset.", "Длительность задания не задана."),
}


def write_xml(path, tree):
    ET.indent(tree, space="  ")
    path.parent.mkdir(parents=True, exist_ok=True)
    ET.ElementTree(tree).write(path, encoding="utf-8", xml_declaration=True)
    with path.open("ab") as f:
        f.write(b"\n")


def packaging():
    root = ET.Element("Mod", id=MOD_ID, version="1")
    props = ET.SubElement(root, "Properties")
    for key, value in {"Name":"LOC_JAC_ASN_MOD_NAME", "Teaser":"LOC_JAC_ASN_MOD_DESCRIPTION",
                       "Description":"LOC_JAC_ASN_MOD_DESCRIPTION", "Authors":"civ6-banseiki",
                       "CompatibleVersions":"2.0"}.items():
        ET.SubElement(props, key).text = value
    dependencies = ET.SubElement(root, "Dependencies")
    for mod_id, title in [("4873eb62-8ccc-4574-b784-dda455e74e68","LOC_EXPANSION2_MOD_TITLE"),
                          ("bd7a82c5-6b38-4b69-a7f5-8dc9af534249","LOC_JAC_BANSEIKI_MOD_NAME"),
                          ("ba3a1c58-9287-4654-a22f-85f9e6841e9b","LOC_JAC_POC_CUSTOM_CIVS_MOD_NAME"),
                          ("26c967d0-c10d-4eeb-8aaf-c83dc7e621e7","LOC_JAC_SENGOKU_TREES_MOD_NAME"),
                          (MAP_ID,"LOC_JAC_JAPAN_SCALE_MOD_NAME")]:
        ET.SubElement(dependencies, "Mod", id=mod_id, title=title)
    loc = ET.SubElement(root, "LocalizedText")
    for tag in ("MOD_NAME", "MOD_DESCRIPTION"):
        el = ET.SubElement(loc, "Text", id="LOC_JAC_ASN_"+tag)
        for lang, value in zip(("ja_JP","en_US","ru_RU"), TEXT[tag]):
            ET.SubElement(el, lang).text=value
    criteria=ET.SubElement(root,"ActionCriteria")
    actions=ET.SubElement(root,"InGameActions")
    files={"Data/JAC_Assassin.sql", "Data/JAC_Assassin_Icons.xml", "Text/JAC_Assassin_Text.xml",
           "Scripts/JAC_Assassin_Gameplay.lua", "Scripts/JAC_Assassin_Rules.lua",
           "UI/JAC_AssassinPanel.xml", "UI/JAC_AssassinPanel.lua"}
    for km in (15,20):
        for suffix in ("", "_Settlers"):
            cid=f"JAC_Assassin_{km}{suffix}"
            c=ET.SubElement(criteria,"Criteria",id=cid)
            ET.SubElement(c,"RuleSetInUse").text="RULESET_JAC_BANSEIKI"
            match=ET.SubElement(c,"ConfigurationValueMatches")
            for tag,value in (("Group","Map"),("ConfigurationId","MAP_SCRIPT"),
                              ("Value",f"{{{MAP_ID}}}Maps/JAC_Japan_{km}km{suffix}.Civ6Map")):
                ET.SubElement(match,tag).text=value
            for kind,path in (("UpdateDatabase","Data/JAC_Assassin.sql"),
                              ("UpdateText","Text/JAC_Assassin_Text.xml"),
                              ("UpdateIcons","Data/JAC_Assassin_Icons.xml"),
                              ("AddGameplayScripts","Scripts/JAC_Assassin_Gameplay.lua"),
                              ("AddUserInterfaces","UI/JAC_AssassinPanel.xml")):
                a=ET.SubElement(actions,kind,id=cid+"_"+kind,criteria=cid)
                p=ET.SubElement(a,"Properties")
                ET.SubElement(p,"LoadOrder").text="350"
                if kind=="AddUserInterfaces": ET.SubElement(p,"Context").text="InGame"
                ET.SubElement(a,"File").text=path
    imports=ET.SubElement(actions,"ImportFiles",id="JAC_Assassin_SharedFiles")
    for path in ("Scripts/JAC_Assassin_Rules.lua","UI/JAC_AssassinPanel.lua"):
        ET.SubElement(imports,"File").text=path
    f=ET.SubElement(root,"Files")
    for path in sorted(files): ET.SubElement(f,"File").text=path
    write_xml(MOD/"JAC_PoC_Assassin.modinfo",root)
    game=ET.Element("GameData"); loc=ET.SubElement(game,"LocalizedText")
    for tag,values in TEXT.items():
        for lang,value in zip(("ja_JP","en_US","ru_RU"),values):
            ET.SubElement(ET.SubElement(loc,"Row",Tag="LOC_JAC_ASN_"+tag,Language=lang),"Text").text=value
    write_xml(MOD/"Text/JAC_Assassin_Text.xml",game)
    icons=ET.Element("GameData"); defs=ET.SubElement(icons,"IconDefinitions")
    # Official texture references only; no copyrighted texture data is copied.
    for unit in ("UNIT_JAC_JAPAN_SENGOKU_ASSASSIN","UNIT_JAC_ASSASSIN"):
        for suffix,atlas,index in (("","ICON_ATLAS_UNITS",19),("_FOW","ICON_ATLAS_UNITS_FOW",19),
                                   ("_PORTRAIT","ICON_ATLAS_UNIT_PORTRAITS",17),
                                   ("_WHITE","ICON_ATLAS_UNIT_FLAG_SYMBOLS_WHITE",19)):
            ET.SubElement(defs,"Row",Name="ICON_"+unit+suffix,Atlas=atlas,Index=str(index))
    write_xml(MOD/"Data/JAC_Assassin_Icons.xml",icons)


def ui(code, state="GameCore_Tuner"):
    return {"action":"ui_lua","state_name":state,"code":code}


def active():
    return {"action":"wait_for","timeout":600,"condition":{"type":"lua","state_name":"InGame",
        "code":"local p=Players[Game.GetLocalPlayer()]; return p~=nil and p:IsTurnActive()"}}


def scenarios():
    source=json.loads((ROOT/"tools/civ6-autotest/scenarios/court-arbitration.json").read_text(encoding="utf-8"))
    enable=copy.deepcopy(source["steps"][0])
    enable["ids"]=[i for i in enable["ids"] if i!="4785cfe4-8d9a-4deb-89ea-d2d566551fec"]+[MOD_ID]
    enable["disable_ids"] += ["4785cfe4-8d9a-4deb-89ea-d2d566551fec", "57a3af09-fb38-41da-b367-818de11a995c"]
    configure=copy.deepcopy(source["steps"][1]); configure["seed"]=620995
    for p in configure["players"]: p["human"]=p["civ"]=="CIVILIZATION_JAC_ODA"
    prefix="local R=ExposedMembers.JAC_Assassin; assert(R,'Assassin gameplay absent'); local t=Game:GetProperty('JAC_ASN_LIVE_TEST'); "
    grant="""local function Grant(owner,kind,city)
      local before={}; for _,u in Players[owner]:GetUnits():Members() do before[u:GetID()]=true end
      UnitManager.InitUnitValidAdjacentHex(owner,kind,city:GetX(),city:GetY(),1)
      local found; for _,u in Players[owner]:GetUnits():Members() do
        if not before[u:GetID()] and u:GetType()==GameInfo.Units[kind].Index then found=u end
      end
      assert(found,'unit grant failed'); return found
    end; """
    steps=[enable,configure,{"action":"start_game","timeout":300},active()]

    def check(code, state="GameCore_Tuner"):
        steps.append(ui(prefix+grant+" ".join(line.strip() for line in code.splitlines()),state))

    def advance(n):
        steps.extend([{"action":"end_turn","turns":n,"timeout":max(180,n*60)},active()])

    def round_trip(name):
        steps.extend([{"action":"save","name":name,"timeout":60},{"action":"exit_to_menu","timeout":60},
                      {"action":"load","name":name,"timeout":300},active()])

    check("""
      local houses={}; for _,p in ipairs(PlayerManager.GetAliveMajors()) do
        houses[PlayerConfigurations[p:GetID()]:GetCivilizationTypeName()]=p:GetID()
      end
      local a,b=houses.CIVILIZATION_JAC_ODA,houses.CIVILIZATION_JAC_MATSUDAIRA
      assert(a and b,'test houses missing'); assert(not Players[b]:IsHuman(),'target must be AI')
      local city=Players[b]:GetCities():GetCapitalCity(); local u=Grant(a,R.UNIT,city)
      assert(PlayersVisibility[a]:IsRevealed(city:GetX(),city:GetY()),'grant did not reveal capital')
      local cities={}; for _,c in Players[a]:GetCities():Members() do cities[#cities+1]=c:GetID() end
      table.sort(cities); assert(#cities>=2,'two training cities required')
      Game:SetProperty(R.TEST,1); Players[a]:SetProperty(R.INPUT,{Security=50})
      Players[a]:GetCities():FindID(cities[1]):SetProperty(R.INPUT,nil)
      Players[a]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0,Branch='FREE_STATE'})
      Game:SetProperty('JAC_ASN_LIVE_TEST',{Actor=a,Target=b,Unit=u:GetID(),Cities=cities,Houses=houses,Start=R.Turn()})
      assert(R.CHANCES.LEADER==1 and R.ProductionTurns({})==100 and R.ProductionTurns({Building=1000})==20)
      assert(R.WaitTurns({})==50 and R.WaitTurns({Security=0,Relation=1,School=1,Governor=1,Hero=1,GreatPerson=1})==1)
      return 'base 100/minimum 20; no-return unit grant found by ID delta'
    """)
    check("""assert(Game.GetLocalPlayer()==t.Actor,'wrong human')
      assert(JAC_AssassinProduce(t.Cities[1]),'training not sent')
      assert(JAC_AssassinAttempt(t.Unit,t.Target,'LEADER'),'attempt not sent')
      return 'training and mission sent via EXECUTE_SCRIPT during the active turn'""", "JAC_AssassinPanel")
    steps.append({"action":"wait_for","timeout":30,"condition":{"type":"lua","state_name":"GameCore_Tuner",
        "code":prefix+"local s=R.State(t.Actor); return s.Production~=nil and s.Mission~=nil"}})
    check("""
      local s=R.State(t.Actor); assert(s.Production.Turns==100 and s.Production.Speed==1,'base training')
      assert(s.Mission.Success and not s.Mission.Detected and s.Mission.Branch=='FREE_STATE','frozen outcome')
      assert(s.CommandUnlock-s.LastCommand==50,'command clock')
      local ok,why=R.StartProduction(t.Actor,t.Cities[2]); assert(not ok and why=='PRODUCING','parallel city')
      local u=Grant(t.Actor,R.UNIT,Players[t.Target]:GetCities():GetCapitalCity())
      local ok,why=R.StartAttempt(t.Actor,u:GetID(),t.Target,'LEADER'); assert(not ok and why=='MISSION_ACTIVE','second unit')
      t.SecondUnit=u:GetID(); t.Frozen=s.Mission; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      Players[t.Actor]:SetProperty(R.INPUT,{Security=100}); return 'separate clocks and saved proposal plan'
    """)
    advance(1)
    check("""local s=R.State(t.Actor); assert(R.Turn()==t.Start+1,'end_turn must honor turns')
      assert(s.Production.Due==t.Start+120 and s.Production.Turns==120,'current security did not extend training')
      return 'training extended from 100 to 120 on the next turn'""")
    round_trip("JAC-Assassin-Dynamic")
    check("""
      local s=R.State(t.Actor); assert(s.Production.Start==t.Start and s.Production.Due==t.Start+120,'training lost on load')
      assert(s.Mission,'mission lost'); for _,k in ipairs({'Start','Due','Success','Detected','SuccessRoll','DetectionRoll','BranchRoll','Branch'}) do
        assert(s.Mission[k]==t.Frozen[k],'rerolled '..k)
      end
      assert(s.Mission.PostDeathPlan.Policy.FreeState=='SAME_SLOT_COUNCIL','plan lost')
      Players[t.Actor]:SetProperty(R.INPUT,{Security=0}); return 'load retains start, speed, inputs and frozen result'
    """)
    advance(9)
    check("""
      local s=R.State(t.Actor); assert(R.Turn()==t.Start+10,'turn count')
      assert(s.Production.Due==t.Start+80 and s.Production.Turns==80,'current security did not shorten training')
      local n=R.Nation(t.Target); assert(s.LastMission.Result=='LEADER_FREE_STATE' and n.State=='FREE_STATE','council not installed')
      assert(n.LeaderDead==1 and n.DeathTurn==t.Frozen.Due and n.Government=='COUNCIL' and n.DecisionRequired==nil)
      assert(n.Slot=='RETAINED' and Players[t.Target]:IsAlive(),'council slot')
      local ok,why=R.CanCommand(s,R.Turn()); assert(not ok and why=='COMMAND_LOCK','command wait')
      return 'council governs same nation; completion moved to 80'
    """)
    advance(69)
    check("""assert(R.Turn()==t.Start+79,'wrong training boundary'); assert(R.State(t.Actor).Production,'completed before 80')
      return 'dynamic training remains active at 79'""")
    advance(1)
    check("""
      local s=R.State(t.Actor); assert(s.Production==nil and s.LastCompletion==t.Start+80,'80-turn completion')
      assert(s.ProductionUnlock==s.LastCompletion+10,'lock origin')
      assert(Players[t.Actor]:GetUnits():FindID(s.LastProducedUnit),'trained unit absent')
      for _,city in ipairs(t.Cities) do local ok,why=R.StartProduction(t.Actor,city); assert(not ok and why=='PRODUCTION_LOCK') end
      R.CancelProduction(t.Actor); assert(R.State(t.Actor).ProductionUnlock==s.ProductionUnlock,'cancel cleared lock')
      t.Completion=s.LastCompletion; t.Lock=s.ProductionUnlock; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      return '80-turn dynamic completion; nationwide 10-turn lock'
    """)
    round_trip("JAC-Assassin-Lock")
    check("""local s=R.State(t.Actor); assert(s.LastCompletion==t.Completion and s.ProductionUnlock==t.Lock,'lock lost')
      assert(R.Nation(t.Target).State=='FREE_STATE','council lost'); return 'nation and lock survive load'""")
    advance(9)
    check("""assert(R.Turn()==t.Lock-1,'wrong lock boundary'); local ok,why=R.StartProduction(t.Actor,t.Cities[2])
      assert(not ok and why=='PRODUCTION_LOCK','unlocked early'); return 'completion+9 locked'""")
    advance(1)
    check("""assert(R.Turn()==t.Lock,'wrong unlock boundary'); Players[t.Actor]:GetCities():FindID(t.Cities[2]):SetProperty(R.INPUT,{Building=10000})
      assert(R.StartProduction(t.Actor,t.Cities[2])); t.MinStart=R.Turn(); Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      return 'completion+10 unlocked; 20-turn training begins'""")
    advance(19)
    check("""assert(R.Turn()==t.MinStart+19 and R.State(t.Actor).Production,'minimum violated'); return 'minimum+19 active'""")
    advance(1)
    check("""local s=R.State(t.Actor); assert(s.LastCompletion==t.MinStart+20 and s.Production==nil,'minimum completion')
      Players[t.Actor]:GetCities():FindID(t.Cities[2]):SetProperty(R.INPUT,nil)
      return 'minimum 20 and second nationwide lock verified'""")

    # Each subsequent fixture clears only the actor wait, then uses real StartAttempt/Tick.
    # They use separate AI nations so a prior death is never undone for a new test.
    for branch,house in (("SUCCESSION","IMAGAWA"),("ABSORPTION","ASAKURA"),("CONGRESS","ASHIKAGA")):
        check("""
          local b=t.Houses.CIVILIZATION_JAC_"""+house+"""; assert(b and Players[b]:IsAlive() and not Players[b]:IsHuman(),'branch target')
          local city=Players[b]:GetCities():GetCapitalCity(); assert(city,'capital absent')
          local u=Grant(t.Actor,R.UNIT,city); local input=R.Input(Players[b])
        """+("""
          local gp=Grant(b,'UNIT_GREAT_GENERAL',city)
          input.Successors={{Identity='JAC_LIVE_HEIR',Kind='GREAT_PERSON',Unit=gp:GetID(),Kin=0}}
          t.HeirUnit=gp:GetID()
        """ if branch=="SUCCESSION" else "")+("""
          local votes={}; for _,p in ipairs(PlayerManager.GetAliveMajors()) do votes[tostring(p:GetID())]='FREE_STATE' end
          input.Congress={Enabled=true,Votes=votes}
        """ if branch=="CONGRESS" else "")+"""
          Players[b]:SetProperty(R.INPUT,input)
          local s=R.State(t.Actor); assert(s.Mission==nil); s.CommandUnlock=R.Turn(); Players[t.Actor]:SetProperty(R.STATE,s)
          Players[t.Actor]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0,Branch='"""+branch+"""'})
          assert(R.StartAttempt(t.Actor,u:GetID(),b,'LEADER')); local m=R.State(t.Actor).Mission
          assert(m.Due==R.Turn()+5,'grant is outside the near-distance band')
          t.BranchTarget=b; t.BranchDue=m.Due; t.BranchPlan=m.PostDeathPlan
          t.BranchAssets={}; for _,c in Players[b]:GetCities():Members() do t.BranchAssets[#t.BranchAssets+1]={X=c:GetX(),Y=c:GetY()} end
          if m.Branch=='CONGRESS' then input.Congress.Votes={}; Players[b]:SetProperty(R.INPUT,input) end
          Game:SetProperty('JAC_ASN_LIVE_TEST',t); return '"""+branch+""" proposal frozen at command'
        """)
        # Grant is at most one tile from the capital, so the base leader duration is five.
        advance(5)
        check("""
          local s=R.State(t.Actor); local n=R.Nation(t.BranchTarget)
          assert(s.Mission==nil and n and n.Branch=='"""+branch+"""' and n.DecisionRequired==nil,'branch unresolved')
          assert(n.DeathTurn==t.BranchDue,'wrong resolution turn')
        """+("""
          assert(n.State=='SUCCESSION' and n.CurrentLeader=='JAC_LIVE_HEIR' and n.Slot=='RETAINED')
          assert(Players[t.BranchTarget]:GetUnits():FindID(t.HeirUnit)==nil,'GP occupies two roles')
          assert(R.ResolveTarget(t.BranchTarget,'LEADER').Identity=='JAC_LIVE_HEIR','successor not targetable')
        """ if branch=="SUCCESSION" else "")+("""
          assert(n.State=='ABSORPTION' and n.Recipient==t.BranchPlan.Recipient,'wrong recipient')
          assert(Players[t.BranchTarget]:GetCities():GetCount()==0 and Players[t.BranchTarget]:GetUnits():GetCount()==0,'old assets remain')
          assert(not Players[t.BranchTarget]:IsAlive(),'empty native slot still active: requires engine adapter')
          for _,c in ipairs(t.BranchAssets) do local actual=CityManager.GetCityAt(c.X,c.Y)
            assert(actual and actual:GetOwner()==n.Recipient,'city transfer failed') end
          assert(n.Slot=='RETIRED' and n.AI=='RECIPIENT_SLOT','old AI retained')
        """ if branch=="ABSORPTION" else "")+("""
          assert(n.State=='FREE_STATE' and n.Plan.Resolution=='FREE_STATE' and n.Plan.Tally.FREE_STATE>0,'ballot changed')
          assert(Players[t.BranchTarget]:IsAlive(),'Congress council eliminated')
        """ if branch=="CONGRESS" else "")+"""return '"""+branch+""" city/unit/leader/slot consequences verified'""")

    for success,detected in ((1,0),(1,1),(0,0),(0,1)):
        check("""
          local b=t.Target; local city=Players[b]:GetCities():GetCapitalCity(); assert(city,'loss-test capital')
          local victim=Grant(b,'UNIT_WARRIOR',city); local u=Grant(t.Actor,R.UNIT,city)
          local s=R.State(t.Actor); assert(s.Mission==nil); s.CommandUnlock=R.Turn(); Players[t.Actor]:SetProperty(R.STATE,s)
          Players[t.Actor]:SetProperty('JAC_ASN_FORCE_NEXT',{Success="""+str(success)+",Detected="+str(detected)+"""})
          assert(R.StartAttempt(t.Actor,u:GetID(),b,'UNIT',victim:GetID()))
          local m=R.State(t.Actor).Mission; local ledger=Players[b]:GetProperty('JAC_ASN_DIPLOMACY_V1') or {}
          assert(m.Due==R.Turn()+2,'unit targets are too far apart')
          t.LossMission=m; t.LossVictim=victim:GetID(); t.LossGrievances=(ledger.Grievances or {})[tostring(t.Actor)] or 0
          t.LossCommandUnlock=R.State(t.Actor).CommandUnlock
          t.LossFriendship=(ledger.Friendship or {})[tostring(t.Actor)] or 0
          UnitManager.Kill(u); Game:SetProperty('JAC_ASN_LIVE_TEST',t)
          return 'lost-actor fixture with success="""+str(success)+", detected="+str(detected)+"""'
        """)
        advance(2)
        check("""
          local s=R.State(t.Actor); local ledger=Players[t.Target]:GetProperty('JAC_ASN_DIPLOMACY_V1')
          assert(s.LastMission.ID==t.LossMission.ID and s.LastMission.Result=='ACTOR_GONE' and s.LastMission.Cancelled,'ghost result')
          assert(Players[t.Target]:GetUnits():FindID(t.LossVictim),'ghost success killed target')
          assert(ledger.Grievances[tostring(t.Actor)]==t.LossGrievances+25 and ledger.Friendship[tostring(t.Actor)]==t.LossFriendship-10,'fixed penalty')
          assert((ledger.WarReason or {})[tostring(t.Actor)]==nil,'ghost detection')
          for _,p in ipairs(PlayerManager.GetAliveMajors()) do assert(p:GetProperty('JAC_ASN_DENOUNCE_'..t.Actor)==nil,'ghost denunciation') end
          local before=ledger.Grievances[tostring(t.Actor)]; R.Tick()
          assert(Players[t.Target]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances[tostring(t.Actor)]==before,'duplicate penalty')
          assert(s.CommandUnlock==t.LossCommandUnlock,'wait changed')
          return 'all outcomes cancelled; fixed penalty once; nationwide wait retained'
        """)
    check("""
      local ledger=Players[t.Target]:GetProperty('JAC_ASN_DIPLOMACY_V1')
      local s=R.State(t.Actor); assert(s.LastMission.Result=='ACTOR_GONE')
      t.FinalGrievances=ledger.Grievances[tostring(t.Actor)]; t.FinalFriendship=ledger.Friendship[tostring(t.Actor)]
      t.FinalApplied=ledger.Applied; t.FinalMissionID=s.LastMission.ID; t.FinalWait=s.CommandUnlock
      Game:SetProperty('JAC_ASN_LIVE_TEST',t); return 'pre-save penalty and applied mission expectations recorded'
    """)
    round_trip("JAC-Assassin-Outcomes")
    check("""
      local h=t.Houses
      assert(R.Nation(t.Target).State=='FREE_STATE','council lost')
      assert(R.Nation(h.CIVILIZATION_JAC_IMAGAWA).CurrentLeader=='JAC_LIVE_HEIR','successor lost')
      assert(R.Nation(h.CIVILIZATION_JAC_ASAKURA).State=='ABSORPTION','absorption history lost')
      assert(R.Nation(h.CIVILIZATION_JAC_ASHIKAGA).Branch=='CONGRESS','ballot history lost')
      local ledger=Players[t.Target]:GetProperty('JAC_ASN_DIPLOMACY_V1')
      assert(ledger.Grievances[tostring(t.Actor)]==t.FinalGrievances and ledger.Friendship[tostring(t.Actor)]==t.FinalFriendship,'load changed penalties')
      for id,applied in pairs(t.FinalApplied) do assert(ledger.Applied[id]==applied,'load lost applied mission') end
      for id,applied in pairs(ledger.Applied) do assert(t.FinalApplied[id]==applied,'load added applied mission') end
      local s=R.State(t.Actor); assert(s.LastMission.ID==t.FinalMissionID and s.LastMission.Result=='ACTOR_GONE','cancelled result lost')
      assert(s.CommandUnlock==t.FinalWait,'load changed wait'); R.Tick()
      return 'four branches, cancelled result and pre-save penalty expectations survive load'
    """)
    advance(1)
    check("""
      local ledger=Players[t.Target]:GetProperty('JAC_ASN_DIPLOMACY_V1')
      assert(ledger.Grievances[tostring(t.Actor)]==t.FinalGrievances and ledger.Friendship[tostring(t.Actor)]==t.FinalFriendship,'next turn repeated penalties')
      for id,applied in pairs(t.FinalApplied) do assert(ledger.Applied[id]==applied,'next turn lost applied mission') end
      for id,applied in pairs(ledger.Applied) do assert(t.FinalApplied[id]==applied,'next turn added applied mission') end
      local s=R.State(t.Actor); assert(s.LastMission.ID==t.FinalMissionID and s.CommandUnlock==t.FinalWait,'next turn changed result or wait')
      Game:SetProperty(R.TEST,nil); Players[t.Actor]:SetProperty(R.INPUT,nil)
      return 'next turn leaves penalties and applied missions unchanged; test mode disabled'
    """)
    data={"name":"Assassin: dynamic 100/120/80/20 training, four proposed death outcomes, lost-actor penalties, three save/load cycles",
          "judge_profile":"japan_map","steps":steps}
    path=ROOT/"tools/civ6-autotest/scenarios/assassin-leader-save-load.json"
    path.write_text(json.dumps(data,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")


if __name__ == "__main__":
    packaging()
    scenarios()
