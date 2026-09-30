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
    "MOD_DESCRIPTION": ("日本マップの暗殺者募集・任務・保存の試作。未決の国家の行方は保留として記録。", "Japan-map recruitment, missions and persistence. Undecided governance is recorded as pending.", "Набор, задания и сохранение на картах Японии. Неопределённое управление записывается как ожидающее решения."),
    "NINJA_NAME": ("暗殺忍者", "Assassin Ninja", "Ниндзя-убийца"),
    "COMMON_NAME": ("暗殺者", "Assassin", "Убийца"),
    "UNIT_DESCRIPTION": ("地図を歩く暗殺者。都市を指定して通常100ターン・最短20ターンで募集。完成後は全国10ターン募集不可。", "An assassin who travels on the map. Recruit in one city: normally 100 turns, at least 20. After completion, recruitment is locked nationwide for 10 turns.", "Убийца, передвигающийся по карте. Набор в одном городе: обычно 100 ходов, минимум 20. После завершения набор запрещён во всей стране на 10 ходов."),
    "INFILTRATION": ("潜入", "Infiltration", "Проникновение"),
    "INFILTRATION_DESCRIPTION": ("他国の領土に入れる。", "May enter foreign territory.", "Может входить на чужую территорию."),
    "TITLE": ("暗殺者", "Assassins", "Убийцы"),
    "CLOSE": ("閉じる", "Close", "Закрыть"),
    "PRODUCE": ("募集を開始", "Start recruiting", "Начать набор"),
    "CANCEL": ("募集を中止", "Cancel recruitment", "Отменить набор"),
    "ATTEMPT": ("暗殺を実行", "Attempt assassination", "Начать покушение"),
    "WAIT": ("任務中", "On a mission", "На задании"),
    "SELECT": ("募集は都市を指定。暗殺は地図上の暗殺者を選び、対象の近くへ移動して実行。", "Choose a city to recruit. Select an assassin on the map and move near a target to attempt a mission.", "Выберите город для набора. Выберите убийцу на карте и переместите его к цели, чтобы начать задание."),
    "STATUS": ("募集ロック：あと{1}ターン／暗殺コマンド待機：あと{2}ターン", "Recruitment lock: {1} turns / command wait: {2} turns", "Запрет набора: {1} ходов / ожидание команды: {2} ходов"),
    "CITY": ("{1}：募集{2}ターン", "{1}: recruit in {2} turns", "{1}: набор за {2} ходов"),
    "TRAINING": ("都市{1}で募集中：あと{2}ターン", "Recruiting in city {1}: {2} turns remaining", "Набор в городе {1}: осталось {2} ходов"),
    "MISSION": ("{1}への任務：あと{2}ターン", "Mission against {1}: {2} turns remaining", "Задание против {1}: осталось {2} ходов"),
    "TARGET": ("{1}・{2}：基本{3}%、補正後{5}%、{4}ターン", "{1}, {2}: base {3}%, adjusted {5}%, {4} turns", "{1}, {2}: базовый шанс {3}%, итоговый {5}%, {4} ходов"),
    "GOVERNOR": ("Governor", "Governor", "Губернатор"),
    "PRODUCED": ("暗殺者の募集が完了。全国の次の募集まで{1}ターン。", "Recruitment complete. Nationwide recruitment unlocks in {1} turns.", "Набор завершён. Набор во всей стране станет доступен через {1} ходов."),
    "RESULT": ("暗殺任務：{1}", "Assassination mission: {1}", "Задание убийцы: {1}"),
    "RESULT_LEADER_DEAD_PENDING": ("指導者死亡。国家の行方は判断待ち。領土は移管されない。", "Leader dead. Governance pending a decision; territory is retained.", "Правитель погиб. Управление ожидает решения; территория сохранена."),
    "RESULT_GOVERNOR_DEAD_PENDING": ("Governor死亡。既存枠の処理は未確認。", "Governor dead; native-slot handling is unverified.", "Губернатор погиб; обработка штатного слота не проверена."),
    "RESULT_UNIT_KILLED": ("対象Unit死亡。", "Target unit killed.", "Целевой юнит погиб."),
    "RESULT_FAILED": ("失敗。暗殺者死亡。", "Failed. Assassin killed.", "Неудача. Убийца погиб."),
    "RESULT_TARGET_GONE": ("対象が失われたため任務終了。", "Mission ended: target no longer exists.", "Задание завершено: цель больше не существует."),
    "RESULT_ACTOR_GONE": ("暗殺者が失われたため成功効果を中断。", "Success effect cancelled: assassin no longer exists.", "Эффект успеха отменён: убийцы больше не существует."),
    "REASON_PRODUCING": ("別の都市で既に募集中。", "Recruitment is already in progress.", "Набор уже идёт."),
    "REASON_PRODUCTION_LOCK": ("完成後の全国募集ロック中。", "Nationwide recruitment lock is active.", "Действует запрет набора во всей стране."),
    "REASON_MISSION_ACTIVE": ("自国の暗殺任務が進行中。", "A national assassination mission is active.", "Задание убийцы от вашей страны уже идёт."),
    "REASON_COMMAND_LOCK": ("前回の実行からの全国待機中。", "Nationwide command waiting period is active.", "Действует общестрановое ожидание после предыдущей команды."),
    "REASON_TARGET_GONE": ("対象が存在しない。", "Target no longer exists.", "Цель больше не существует."),
    "REASON_TARGET_UNKNOWN": ("対象の所在を確認する。", "Discover the target location.", "Установите местонахождение цели."),
    "REASON_CITY_GONE": ("生産都市が存在しない。", "Recruitment city no longer exists.", "Города набора больше не существует."),
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
    steps=[enable,configure,{"action":"start_game","timeout":300},active(),
        ui("local R=ExposedMembers.JAC_Assassin; assert(R,'Assassin gameplay absent'); local a,b; for _,p in ipairs(PlayerManager.GetAliveMajors()) do local c=PlayerConfigurations[p:GetID()]:GetCivilizationTypeName(); if c=='CIVILIZATION_JAC_ODA' then a=p:GetID() elseif c=='CIVILIZATION_JAC_MATSUDAIRA' then b=p:GetID() end end; assert(a and b,'test houses missing'); assert(not Players[b]:IsHuman(),'target must be AI'); local city=Players[b]:GetCities():GetCapitalCity(); local u=UnitManager.InitUnitValidAdjacentHex(a,R.UNIT,city:GetX(),city:GetY(),1); assert(u,'grant failed'); local cities={}; for _,c in Players[a]:GetCities():Members() do cities[#cities+1]=c:GetID() end; table.sort(cities); assert(#cities>=2,'two producing cities required'); Game:SetProperty(R.TEST,1); Players[a]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0,Branch='FREE_STATE'}); local before={}; for _,id in ipairs({a,b}) do before[tostring(id)]={}; for _,c in Players[id]:GetCities():Members() do table.insert(before[tostring(id)],c:GetID()) end end; Game:SetProperty('JAC_ASN_LIVE_TEST',{Actor=a,Target=b,Unit=u:GetID(),Cities=cities,Before=before,Start=R.Turn()}); assert(R.CHANCES.LEADER==1); assert(R.ProductionTurns({})==100); assert(R.ProductionTurns({Building=1000})==20); assert(R.WaitTurns({})==50); assert(R.WaitTurns({Security=0,Relation=1,School=1,Governor=1,Hero=1,GreatPerson=1})==1); return 'AI target, map unit grant, production/cooldown constants verified'"),
        ui(prefix+"assert(Game.GetLocalPlayer()==t.Actor,'wrong human'); assert(JAC_AssassinProduce(t.Cities[1]),'production not sent'); assert(JAC_AssassinAttempt(t.Unit,t.Target,'LEADER'),'attempt not sent'); return 'production and assassination sent through EXECUTE_SCRIPT'", "JAC_AssassinPanel"),
        {"action":"wait_for","timeout":30,"condition":{"type":"lua","state_name":"GameCore_Tuner","code":prefix+"local s=R.State(t.Actor); return s.Production~=nil and s.Mission~=nil"}},
        ui(prefix+"local s=R.State(t.Actor); assert(s.Production.Turns==100 and s.Production.Due-s.Production.Start==100,'base production'); assert(s.Mission.Success and not s.Mission.Detected and s.Mission.Branch=='FREE_STATE','forced outcome not frozen'); assert(s.CommandUnlock-s.LastCommand==50,'command clock'); local ok,why=R.StartProduction(t.Actor,t.Cities[2]); assert(not ok and why=='PRODUCING','parallel city bypass'); local u=UnitManager.InitUnitValidAdjacentHex(t.Actor,R.UNIT,Players[t.Target]:GetCities():GetCapitalCity():GetX(),Players[t.Target]:GetCities():GetCapitalCity():GetY(),1); assert(u,'second grant failed'); local ok,why=R.StartAttempt(t.Actor,u:GetID(),t.Target,'LEADER'); assert(not ok and why=='MISSION_ACTIVE','second unit bypass'); t.SecondUnit=u:GetID(); t.Frozen=s.Mission; t.ProductionDue=s.Production.Due; Game:SetProperty('JAC_ASN_LIVE_TEST',t); return 'one city and one active mission enforced; three independent clocks'"),
        {"action":"save","name":"JAC-Assassin-Pending","timeout":60},
        {"action":"exit_to_menu","timeout":60},{"action":"load","name":"JAC-Assassin-Pending","timeout":300},active(),
        ui(prefix+"local s=R.State(t.Actor); assert(s.Production.Due==t.ProductionDue,'production changed on load'); assert(s.Mission,'pending mission lost'); for _,k in ipairs({'Start','Due','Success','Detected','SuccessRoll','DetectionRoll','BranchRoll','Branch'}) do assert(s.Mission[k]==t.Frozen[k],'rerolled '..k) end; assert(Players[t.Target]:GetProperty(R.NATION)==nil,'premature leader death'); return 'pending mission, RNG rolls and production survive load'"),
        {"action":"end_turn","turns":10,"timeout":600},active(),
        ui(prefix+"local s=R.State(t.Actor); assert(s.Mission==nil and s.LastMission.Result=='LEADER_DEAD_PENDING','mission did not finish'); local n=Players[t.Target]:GetProperty(R.NATION); assert(n and n.LeaderDead==1 and n.State=='PENDING_FREE_STATE' and n.DecisionRequired==1,'post-death state'); assert(n.DeathTurn==t.Frozen.Due,'wrong death time'); assert(Players[t.Target]:IsAlive(),'native player incorrectly eliminated'); for _,id in ipairs({t.Actor,t.Target}) do local count=0; for _,c in Players[id]:GetCities():Members() do count=count+1; local found=false; for _,old in ipairs(t.Before[tostring(id)]) do if c:GetID()==old then found=true end end; assert(found,'city transferred or created') end; assert(count==#t.Before[tostring(id)],'city lost or acquired') end; local ok,why=R.CanCommand(s,R.Turn()); assert(not ok and why=='COMMAND_LOCK','command wait ignored after success'); return 'logical leader death, pending nation, no territory acquisition; command wait retained'"),
        {"action":"end_turn","turns":90,"timeout":1800},active(),
        ui(prefix+"local s=R.State(t.Actor); assert(s.Production==nil and s.LastCompletion==t.ProductionDue,'base recruitment completion'); assert(s.ProductionUnlock==s.LastCompletion+10,'lock origin'); assert(Players[t.Actor]:GetUnits():FindID(s.LastProducedUnit),'produced unit absent'); for _,city in ipairs(t.Cities) do local ok,why=R.StartProduction(t.Actor,city); assert(not ok and why=='PRODUCTION_LOCK','city lock bypass') end; R.CancelProduction(t.Actor); assert(R.State(t.Actor).ProductionUnlock==s.ProductionUnlock,'cancel cleared lock'); t.Completion=s.LastCompletion; t.Lock=s.ProductionUnlock; Game:SetProperty('JAC_ASN_LIVE_TEST',t); return '100-turn completion; 10-turn nationwide lock in both cities, cancellation cannot clear it'"),
        {"action":"save","name":"JAC-Assassin-Lock","timeout":60},
        {"action":"exit_to_menu","timeout":60},{"action":"load","name":"JAC-Assassin-Lock","timeout":300},active(),
        ui(prefix+"local s=R.State(t.Actor); assert(s.LastCompletion==t.Completion and s.ProductionUnlock==t.Lock,'lock changed on load'); assert(Players[t.Target]:GetProperty(R.NATION).LeaderDead==1,'death state lost'); local ok,why=R.StartProduction(t.Actor,t.Cities[2]); assert(not ok and why=='PRODUCTION_LOCK','load cleared lock'); return 'nation state and nationwide lock survive load'"),
        {"action":"end_turn","turns":9,"timeout":600},active(),
        ui(prefix+"assert(R.Turn()==t.Lock-1,'wrong lock boundary'); local ok,why=R.StartProduction(t.Actor,t.Cities[2]); assert(not ok and why=='PRODUCTION_LOCK','unlocked one turn early'); return 'completion+9 locked'"),
        {"action":"end_turn","turns":1,"timeout":180},active(),
        ui(prefix+"assert(R.Turn()==t.Lock,'wrong unlock boundary'); Players[t.Actor]:GetCities():FindID(t.Cities[2]):SetProperty(R.INPUT,{Building=10000}); local ok,why=R.StartProduction(t.Actor,t.Cities[2]); assert(ok,tostring(why)); local s=R.State(t.Actor); assert(s.Production.City==t.Cities[2] and s.Production.Turns==20,'minimum recruitment in new city'); t.MinStart=R.Turn(); Game:SetProperty('JAC_ASN_LIVE_TEST',t); Game:SetProperty(R.TEST,nil); return 'completion+10 unlocked in another city; minimum recruitment started; forced test mode disabled'"),
        {"action":"end_turn","turns":19,"timeout":900},active(),
        ui(prefix+"local s=R.State(t.Actor); assert(R.Turn()==t.MinStart+19,'wrong minimum boundary'); assert(s.Production~=nil and s.LastCompletion==t.Completion,'completed before 20'); return 'minimum recruitment still active after 19 turns'"),
        {"action":"end_turn","turns":1,"timeout":180},active(),
        ui(prefix+"local s=R.State(t.Actor); assert(s.Production==nil and s.LastCompletion==t.MinStart+20,'did not complete at 20'); assert(s.ProductionUnlock==s.LastCompletion+10,'second nationwide lock'); Players[t.Actor]:GetCities():FindID(t.Cities[2]):SetProperty(R.INPUT,nil); return 'minimum 20-turn completion and new nationwide lock verified'"),
    ]
    data={"name":"Assassin: 100-turn recruitment; forced AI leader death; pending governance; nationwide locks and two save/load cycles", "judge_profile":"japan_map","steps":steps}
    path=ROOT/"tools/civ6-autotest/scenarios/assassin-leader-save-load.json"
    path.write_text(json.dumps(data,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")


if __name__ == "__main__":
    packaging()
    scenarios()
