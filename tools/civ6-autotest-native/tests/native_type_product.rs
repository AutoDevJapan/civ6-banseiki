//! Executes the type scenarios' actual Lua against the production absorption modules.
//! These are Lua 5.1 checks with a native-world model, not Civ6 engine evidence.
//!
//! Render a template with the actual input save (including its output hash):
//! JAC_TYPE_TEMPLATE=artifact-prepare | city-owner-transfer | scatter | return
//! JAC_TYPE_SOURCE_SAVE=<successful save name without .Civ6Save>
//! JAC_TYPE_AGE=fresh | aged | artifact (branches only)
//! JAC_TYPE_OUTPUT=<absolute absorption-native-type-run-*.json in this scenario directory>
//! cargo test --locked --manifest-path tools/civ6-autotest-native/Cargo.toml
//!   --test native_type_product render_type_scenario -- --ignored --exact
//! For art-prepare the checked-in input is already the successful 134 save.
//! Aged branches advance real turns from the SAME preparation save; optional
//! JAC_TYPE_AGED_TURNS defaults to 11. The Lua checks the actual engine lock and age.

use mlua::Lua;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};

const NAMES: &[&str] = &[
    "art-prepare",
    "artifact-prepare",
    "city-owner-transfer",
    "scatter",
    "return",
];

fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn scenario_dir() -> PathBuf {
    repo("tools/civ6-autotest/scenarios")
}

fn scenario(name: &str) -> Value {
    let bytes = fs::read(scenario_dir().join(format!("absorption-native-type-{name}.json")))
        .expect("type scenario exists");
    serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
        .expect("valid scenario JSON")
}

fn step(name: &str, id: &str) -> Value {
    scenario(name)["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .unwrap_or_else(|| panic!("missing {name}/{id}"))
        .clone()
}

fn execute(lua: &Lua, name: &str, id: &str) -> mlua::Result<()> {
    let step = step(name, id);
    let context = if step["state_name"] == "JAC_Absorption_Entry" {
        "Gameplay"
    } else {
        "UI"
    };
    lua.globals().set("CONTEXT", context)?;
    lua.load(step["code"].as_str().unwrap())
        .set_name(format!("{name}/{id}"))
        .exec()
}

fn condition(lua: &Lua, name: &str, id: &str) -> mlua::Result<bool> {
    let step = step(name, id);
    let condition = &step["condition"];
    lua.globals().set(
        "CONTEXT",
        if condition["state_name"] == "JAC_Absorption_Entry" {
            "Gameplay"
        } else {
            "UI"
        },
    )?;
    let value: mlua::Value = lua
        .load(condition["code"].as_str().unwrap())
        .set_name(format!("wait {name}/{id}"))
        .eval()?;
    match value {
        mlua::Value::Boolean(result) => Ok(result),
        other => Err(mlua::Error::external(format!(
            "native wait {name}/{id} returned a nonboolean: {other:?}"
        ))),
    }
}

fn drive(lua: &Lua, name: &str, id: &str) -> mlua::Result<()> {
    for _ in 0..40 {
        if condition(lua, name, id)? {
            return Ok(());
        }
        lua.load("publish()").exec()?;
    }
    let detail: String = lua.load("local k=Game:GetProperty('JAC_TYPE_CASE');local tx=JAC_AbsorptionGameplay.GetState().Transactions[k.ID] or {};local f=JAC_AbsorptionFlowGameplay.Get(k.ID) or {};local s=(tx.NativeWorks or {})[1] or {};return JAC_AbsorptionGameplay.Canonical({Status=tx.Status,Phase=f.Phase,Error=f.Error,Final=tx.AwaitingFinalUI,Carrier=s.Carrier and s.Carrier.Phase})").eval()?;
    Err(mlua::Error::external(format!(
        "{name}/{id} did not become ready: {detail}"
    )))
}

fn load(lua: &Lua, path: &str) -> mlua::Result<()> {
    lua.load(fs::read_to_string(repo(path)).map_err(mlua::Error::external)?)
        .set_name(path)
        .exec()
}

fn model(kind: &str, age: &str) -> mlua::Result<Lua> {
    let lua = Lua::new();
    let outcomes = lua
        .load(
            r#"return {
            {Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='UNIT',Outcome='INCORPORATE',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='TRANSFER',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'},
            {Kind='ITEM',Outcome='SCATTER',Destination='ARCHIVE'}}"#,
        )
        .eval::<mlua::Table>()?;
    lua.load(include_str!("fixtures/absorption_world.lua"))
        .call::<()>(outcomes)?;
    lua.globals().set("KIND", kind)?;
    lua.globals().set("AGE_CASE", age)?;
    lua.load(
        r#"
        function include(_) end
        CONTEXT='Gameplay'
        function in_context(context,fn,...)
          local before=CONTEXT;CONTEXT=context
          local ok,value=pcall(fn,...);CONTEXT=before
          if not ok then error(value,0) end
          return value
        end
        function ui_only() assert(CONTEXT=='UI','UI-only native getter called in Gameplay') end
        function event()
          local handlers={}
          return {Add=function(fn) handlers[#handlers+1]=fn end,
            Emit=function(...) for _,fn in ipairs(handlers) do fn(...) end end}
        end
        PlayerManager.GetFreeCitiesPlayerID=function() return 3 end
        for owner,configuration in pairs(PlayerConfigurations) do
          configuration.GetLeaderName=function() return 'Leader '..owner end
        end
        Players[1].units={}
        Players[1]:SetProperty('JAC_ABSORPTION_ASSETS_V1',{})
        Players[2]:SetProperty('JAC_ABSORPTION_ASSETS_V1',{})
        CITY(0,2);CITY(2,10)
        for owner,position in pairs({[0]=0,[1]=8,[2]=12,[3]=16}) do
          local original=Players[owner].GetCities
          Players[owner].GetCities=function(self)
            local collection=original(self)
            collection.GetCapitalCity=function() return CITIES[position] end
            return collection
          end
        end
        INDEX=KIND=='art' and 3 or 0
        OBJECT=KIND=='art' and 'GREATWORKOBJECT_PORTRAIT' or 'GREATWORKOBJECT_ARTIFACT'
        BUILDING=KIND=='art' and 'BUILDING_MUSEUM_ART' or 'BUILDING_MUSEUM_ARTIFACT'
        SLOT_TYPE=KIND=='art' and 'GREATWORKSLOT_ART' or 'GREATWORKSLOT_ARTIFACT'
        ORIGINAL_TURN=5
        CURRENT_TURN=AGE_CASE=='aged' and 16 or 5
        GlobalParameters={GREATWORK_ART_LOCK_TIME=10}
        GameInfo.Buildings=setmetatable({[0]={Index=0,BuildingType='BUILDING_PALACE'},
          [1]={Index=1,BuildingType=BUILDING}},
          {__call=function(t) local i=-1;return function() i=i+1;return t[i] end end})
        GameInfo.Buildings.BUILDING_PALACE=GameInfo.Buildings[0]
        GameInfo.Buildings[BUILDING]=GameInfo.Buildings[1]
        GameInfo.GreatWorks={[4]={Index=4,GreatWorkType='GREATWORK_TYPE_ORIGINAL',
          GreatWorkObjectType=OBJECT,Name='Original',Tourism=8}}
        GameInfo.GreatWorks.GREATWORK_TYPE_ORIGINAL=GameInfo.GreatWorks[4]
        GameInfo.GreatWorkSlotTypes={[1]={GreatWorkSlotType=SLOT_TYPE}}
        GameInfo.GreatWork_YieldChanges=function()
          local read=false;return function() if not read then read=true;
            return {GreatWorkType='GREATWORK_TYPE_ORIGINAL',YieldType='YIELD_CULTURE',YieldChange=3} end end
        end
        GameInfo.GreatWork_ValidSubTypes=function()
          local read=false;return function() if not read then read=true;
            return {GreatWorkObjectType=OBJECT,GreatWorkSlotType=SLOT_TYPE} end end
        end
        setmetatable(Game,{__index=function(_,key)
          if key=='GetGreatWorkPlayer' and CONTEXT=='UI' then
            return function() ui_only();return 2 end
          end
        end})
        WORKS={[4]={[0]=INDEX},[12]={[0]=44}}
        for _,city in pairs(CITIES) do
          local cultural=city.GetCulturalIdentity
          city.GetCulturalIdentity=function(self)
            local result=cultural(self)
            if CONTEXT~='UI' then result.GetLoyalty=nil end
            return result
          end
          local religion=city.GetReligion
          city.GetReligion=function(self)
            local result=religion(self)
            if CONTEXT~='UI' then result.GetReligionsInCity=nil end
            return result
          end
          local districts=city.GetDistricts
          city.GetDistricts=function(self)
            local result=districts(self)
            if CONTEXT~='UI' then result.Members=nil end
            return result
          end
          city.GetBuildings=function(self)
            local b={HasBuilding=function(_,index) return index==1 and self.x~=0 and self.x~=8 end,
              IsPillaged=function() return false end,
              GetNumGreatWorkSlots=function(_,index) return index==1 and 3 or 0 end,
              GetGreatWorkInSlot=function(_,building,slot)
                return building==1 and WORKS[self.x] and WORKS[self.x][slot] or -1
              end}
            if CONTEXT=='UI' then
              b.GetGreatWorkTypeFromIndex=function() ui_only();return 4 end
              b.GetCreatorNameFromIndex=function() ui_only();return KIND=='art' and 'Original artist' or '' end
              b.GetTurnFromIndex=function() ui_only();return ORIGINAL_TURN end
              b.GetGreatWorkSlotType=function() ui_only();return 1 end
            end
            return b
          end
        end
        Map.GetCityPlots=function() ui_only();return {
          GetPurchasedPlots=function(_,city)
            ui_only();local plots={}
            for index,q in pairs(PLOTS) do if q.city==city.x then plots[#plots+1]=index end end
            return plots
          end} end
        Cities={GetPlotPurchaseCity=function(q) ui_only();return q.city and CITIES[q.city] end}
        for _,q in pairs(PLOTS) do q.IsImpassable=function() return false end end
        UnitManager.GetUnit=function(owner,id) return Players[owner].units[id] end
        local improve=ImprovementBuilder.SetImprovementType
        ImprovementBuilder.SetImprovementType=function(plot,kind,...)
          if kind==-1 then plot.improvement=-1 else return improve(plot,kind,...) end
        end
        local guard=UNIT(2,500,'UNIT_WARRIOR',18)
        guard.damage,guard.moves=7,1
        if AGE_CASE=='aged' then
          local donorGuard=UNIT(1,600,'UNIT_WARRIOR',4)
          donorGuard.damage,donorGuard.moves,donorGuard.xp=13,1,20
          donorGuard.promotions[7]=true
        end
        PLOTS[18].owner=2
        MOVE_CALLS,DEAL_CALLS,MOVE_EFFECT=0,0,true
        DealDirection={OUTGOING=1};DealItemTypes={GREATWORK=2}
        local deal={items={}}
        function deal:GetItemCount() return #self.items end
        function deal:AddItemOfType(kind,owner)
          assert(kind==2 and owner==1)
          local item={SetSubType=function(_,value) assert(value==4) end,
            SetValueType=function(_,value) assert(value==INDEX) end}
          self.items[#self.items+1]=item;return item
        end
        function deal:Validate() return 1 end
        function deal:IsValid() return #self.items==1 end
        DealManager={GetWorkingDeal=function() return deal end,
          GetPossibleDealItems=function() return {{ForType=INDEX,IsValid=true}} end,
          EnactWorkingDeal=function(from,to)
            assert(from==1 and to==0);DEAL_CALLS=DEAL_CALLS+1
            if MOVE_EFFECT then WORKS[4][0]=nil;WORKS[2]={[0]=INDEX} end
            return true
          end}
        "#,
    )
    .exec()?;
    boot(&lua)?;
    lua.globals().set("CONTEXT", "Gameplay")?;
    lua.load(
        r#"
        local A=JAC_AbsorptionGameplay
        local original=assert(A.NativeWorkLocations(INDEX)[1])
        local function slot(owner,x)
          return {Owner=owner,City=CITIES[x]:GetID(),X=x,Y=0,Building=1,Slot=0,SlotType=SLOT_TYPE}
        end
        local k={Kind=KIND,Index=INDEX,From=1,To=2,Third=0,Free=3,
          Source=A.Rules.Copy(original),Original=A.Rules.Copy(original),CityOwnerProof=true,
          Guards={},Museums={},SourceWorks={original},
          SourceAssets=A.CityAssets(CITIES[4]),Fixtures={
            {Role='Human',X=10,Y=0,Museum=1,Destination=2},
            {Role='Free',X=16,Y=0,Museum=1,Destination=3},
            {Role='Third',X=2,Y=0,Museum=1,Destination=0}},
          HumanSlot=slot(2,10),FreeSlot=slot(3,16),ThirdSlot=slot(0,2)}
        Game:SetProperty('JAC_TYPE_CASE',k)
        "#,
    )
    .exec()?;
    execute(&lua, "art-prepare", "checks")?;
    lua.load("local k=Game:GetProperty('JAC_TYPE_CASE');k.Guards={ExposedMembers.JAC_TypeChecks.Unit(Players[2].units[500])};Game:SetProperty('JAC_TYPE_CASE',k)")
        .exec()?;
    Ok(lua)
}

fn boot(lua: &Lua) -> mlua::Result<()> {
    lua.load("CONTEXT='Gameplay';GameEvents={OnGameTurnStarted=event(),JAC_AbsorptionFlow_Operation=event()}")
        .exec()?;
    for name in ["Rules", "Gameplay", "Flow"] {
        load(
            lua,
            &format!("poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_{name}.lua"),
        )?;
    }
    lua.load(include_str!("fixtures/absorption_panel.lua"))
        .exec()?;
    lua.load(
        r#"
        CONTEXT='UI'
        PlayerOperations.MOVE_GREAT_WORK='native-move'
        for _,key in ipairs({'PLAYER_ONE','CITY_SRC','CITY_DEST','BUILDING_SRC','BUILDING_DEST','GREAT_WORK_INDEX','SLOT'}) do
          PlayerOperations['PARAM_'..key]=key
        end
        UI.RequestPlayerOperation=function(actor,operation,params)
          assert(CONTEXT=='UI' and actor==2,'operation must be sent by the UI local human')
          if operation==PlayerOperations.EXECUTE_SCRIPT then
            return in_context('Gameplay',function()
              GameEvents.JAC_AbsorptionFlow_Operation.Emit(actor,params)
              return true
            end)
          end
          assert(operation=='native-move')
          local from=assert(Players[params.PLAYER_ONE]:GetCities():FindID(params.CITY_SRC))
          local to=assert(Players[params.PLAYER_ONE]:GetCities():FindID(params.CITY_DEST))
          assert(from.owner==to.owner and params.GREAT_WORK_INDEX==INDEX and params.BUILDING_SRC==1
            and params.BUILDING_DEST==1 and not (WORKS[to.x] and WORKS[to.x][params.SLOT]))
          MOVE_CALLS=MOVE_CALLS+1
          if MOVE_EFFECT then
            local sourceSlot
            for slot,index in pairs(WORKS[from.x]) do if index==INDEX then sourceSlot=slot end end
            assert(sourceSlot~=nil,'MOVE original not at its saved source')
            WORKS[from.x][sourceSlot]=nil;WORKS[to.x]=WORKS[to.x] or {};WORKS[to.x][params.SLOT]=INDEX
          end
          return true
        end
        "#,
    )
    .exec()?;
    load(lua, "poc/JAC_PoC_Absorption/UI/JAC_AbsorptionFlow.lua")?;
    load(lua, "poc/JAC_PoC_Absorption/UI/JAC_Absorption_HeroKeys.lua")?;
    lua.load(
        r#"
        local reads=assert(ExposedMembers.JAC_HeroReadAPI)
        for _,name in ipairs({'ReadNativeWork','FindCompatibleNativeSlots','ReadCarrierPlots','ReadCity','ReadUnit'}) do
          local method=reads[name]
          reads[name]=function(...) return in_context('UI',method,...) end
        end
        function publish()
          in_context('UI',Events.GameCoreEventPublishComplete.Emit)
          in_context('Gameplay',JAC_AbsorptionFlowGameplay.Tick)
        end
        CONTEXT='Gameplay'
        "#,
    )
    .exec()
}

#[test]
fn type_preparation_proves_one_real_original_and_rejects_changed_metadata() -> mlua::Result<()> {
    for kind in ["art", "artifact"] {
        let lua = model(kind, if kind == "art" { "fresh" } else { "artifact" })?;
        execute(&lua, "art-prepare", "prepare-proof")?;
        execute(&lua, "art-prepare", "prepared-reload-proof")?;
        lua.load("WORKS[10]={[0]=INDEX}").exec()?;
        assert!(execute(&lua, "art-prepare", "prepare-proof").is_err());
        lua.load("WORKS[10]={};GameInfo.GreatWorks[4].Tourism=99")
            .exec()?;
        assert!(execute(&lua, "art-prepare", "prepare-proof").is_err());
    }
    Ok(())
}

#[test]
fn prepared_ready_wait_returns_a_boolean_true_to_the_native_runner() -> mlua::Result<()> {
    let lua = model("art", "fresh")?;
    lua.globals().set("CONTEXT", "UI")?;
    for id in ["ready", "reload-ready"] {
        let condition = step("art-prepare", id);
        let value: mlua::Value = lua
            .load(condition["condition"]["code"].as_str().unwrap())
            .eval()?;
        assert!(
            matches!(value, mlua::Value::Boolean(true)),
            "{id} returned {value:?}"
        );
    }
    Ok(())
}

#[test]
fn successful_saved_target_is_required_and_artifact_zero_index_is_valid() -> mlua::Result<()> {
    for kind in ["art", "artifact"] {
        let name = if kind == "art" {
            "art-prepare"
        } else {
            "artifact-prepare"
        };
        let lua = model(kind, if kind == "art" { "fresh" } else { "artifact" })?;
        lua.load(
            r#"
            CityManager.TransferCity(CITIES[4],2)
            local A=JAC_AbsorptionGameplay
            local original=A.NativeWorkLocations(INDEX)[1]
            local direct={Index=INDEX,Moved=true,RequestOnly=false,GlobalCount=1,
              SourceAfter=-1,TargetAfter=INDEX,Target={Owner=2,City=original.CityID,
                X=4,Y=0,Building=1,Slot=0}}
            local originalKey=KIND=='art' and 'JAC_ARTWORK_FRESH_ORIGINAL' or 'JAC_ARTIFACT_FULL_ORIGINAL'
            local evidenceKey=KIND=='art' and 'JAC_ARTWORK_CASE_EVIDENCE' or 'JAC_ARTIFACT_FULL_EVIDENCE'
            local label=KIND=='art' and 'art_fresh' or 'artifact_full_empty'
            Game:SetProperty(originalKey,original)
            Game:SetProperty(evidenceKey,{[label]=direct})
            "#,
        ).exec()?;
        execute(&lua, name, "capture-original")?;
        lua.load("local k=Game:GetProperty('JAC_TYPE_CASE');assert(k.Index==INDEX and k.To==2 and k.Source.Owner==2);if KIND=='artifact' then assert(k.Index==0 and k.Original.CreatorName==nil) end").exec()?;
        let before: String = lua
            .load("return JAC_AbsorptionGameplay.Canonical(Game:GetProperty('JAC_TYPE_CASE'))")
            .eval()?;
        lua.load("local key=KIND=='art' and 'JAC_ARTWORK_CASE_EVIDENCE' or 'JAC_ARTIFACT_FULL_EVIDENCE';local label=KIND=='art' and 'art_fresh' or 'artifact_full_empty';Game:GetProperty(key)[label].Target.Slot=1").exec()?;
        assert!(execute(&lua, name, "capture-original").is_err());
        assert_eq!(
            lua.load("return JAC_AbsorptionGameplay.Canonical(Game:GetProperty('JAC_TYPE_CASE'))")
                .eval::<String>()?,
            before
        );
        lua.load("assert(CITIES[4]:GetOwner()==2,'failed original proof transferred the source')")
            .exec()?;
    }
    Ok(())
}

#[test]
fn fixture_center_guard_moves_without_changing_owner_or_native_state() -> mlua::Result<()> {
    let lua = model("art", "fresh")?;
    lua.load(
        r#"
        local unit=Players[2].units[500];unit.x=4
        local H=ExposedMembers.JAC_TypeChecks
        local original=H.Unit(unit)
        local moved=H.ClearCenters({{X=4,Y=0},{X=16,Y=0},{X=2,Y=0},{X=10,Y=0}})
        assert(#moved==1 and moved[1].Owner==2 and moved[1].ID==500)
        assert(moved[1].X~=4 and moved[1].X~=16 and moved[1].X~=2 and moved[1].X~=10)
        original.X,original.Y=moved[1].X,moved[1].Y
        assert(JAC_AbsorptionGameplay.Rules.Equal(moved[1],original),'guard identity/abilities changed')
        H.Guards(moved)
        local k=Game:GetProperty('JAC_TYPE_CASE')
        local id=CITIES[16].id;CITIES[16].id=id+1
        assert(not pcall(H.Empty,k.FreeSlot),'reserved empty slot accepted another city ID')
        CITIES[16].id=id
        "#,
    ).exec()
}

#[test]
fn kept_human_recovery_land_is_a_safe_guard_destination_while_transfer_land_stays_excluded()
-> mlua::Result<()> {
    let lua = model("artifact", "artifact")?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[4],2)
        CityManager.TransferCity(CITIES[16],2)
        for index,q in pairs(PLOTS) do
          if index~=4 and index~=5 and index~=10 and index~=11 and index~=16 and index~=17 then q.owner=0 end
        end
        local H=ExposedMembers.JAC_TypeChecks
        local unit=Players[2].units[500];unit.x=4
        local original=H.Unit(unit)
        local centers={{X=4,Y=0,Role='Source'},{X=16,Y=0,Role='Free'},
          {X=2,Y=0,Role='Third'},{X=10,Y=0,Role='Human'}}
        local moved=H.ClearCenters(centers)
        assert(#moved==1 and moved[1].Owner==2 and moved[1].ID==500 and moved[1].X==11,
          'human recovery purchased land was incorrectly forbidden')
        original.X=11
        assert(JAC_AbsorptionGameplay.Rules.Equal(moved[1],original))
        H.Guards(moved)
        unit.x=4;PLOTS[11].owner=0
        local accepted,why=pcall(H.ClearCenters,centers)
        assert(not accepted and tostring(why):find('candidates=0',1,true),
          'changing source/Free/Third land was admitted without a safe kept-human plot')
        assert(unit:GetOwner()==2 and unit:GetID()==500 and unit:GetX()==4 and unit:GetDamage()==7 and unit:GetMovesRemaining()==1)
        "#,
    ).exec()
}

#[test]
fn all_type_templates_pass_native_validation_and_lua51_compilation() {
    for name in NAMES {
        let output = Command::new(env!("CARGO_BIN_EXE_civ6-autotest-native"))
            .arg(scenario_dir().join(format!("absorption-native-type-{name}.json")))
            .arg("--check")
            .output()
            .expect("native check process");
        assert!(
            output.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn render(
    mut data: Value,
    save: &str,
    age: Option<&str>,
    aged_turns: u32,
) -> Result<Value, String> {
    if save.is_empty()
        || save.len() > 80
        || !save
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        || !save.as_bytes()[0].is_ascii_alphanumeric()
        || ["TYPE_PREPARED_SAVE", "ARTIFACT_FULL_SUCCESS_SAVE"].contains(&save)
    {
        return Err("actual successful save name without .Civ6Save required".into());
    }
    let steps = data["steps"].as_array_mut().ok_or("steps required")?;
    let input = steps
        .iter_mut()
        .find(|s| s["action"] == "load")
        .ok_or("input load required")?;
    input["name"] = save.into();
    input["exact_name"] = true.into();
    if let Some(age) = age {
        if !["fresh", "aged", "artifact"].contains(&age) || aged_turns == 0 {
            return Err("fresh/aged/artifact and positive aged turns required".into());
        }
        for step in steps.iter_mut() {
            if let Some(source) = step["code"].as_str() {
                step["code"] = source.replace("__TYPE_AGE__", age).into();
            }
        }
        if age == "aged" {
            let begin = steps
                .iter()
                .position(|s| s["id"] == "begin")
                .ok_or("branch begin required")?;
            steps.insert(begin, json!({"action":"end_turn","id":"age-original","turns":aged_turns,"test_turns":aged_turns,"timeout":1800}));
        }
        data["parameters"]["age"] = age.into();
    } else if steps.iter().any(|s| {
        s["code"]
            .as_str()
            .is_some_and(|c| c.contains("__TYPE_AGE__"))
    }) {
        return Err("branch age argument required".into());
    }
    data["parameters"]["source_save"] = save.into();
    Ok(data)
}

#[test]
fn renderer_uses_exact_external_save_and_ages_only_the_independent_branch() {
    let input = "JAC-artifact-full-success-chain-150-0123456789ab";
    let prepared = render(scenario("artifact-prepare"), input, None, 11).unwrap();
    assert_eq!(prepared["steps"][1]["name"], input);
    let art = "JAC-type-art-prepared-0123456789ab";
    for branch in ["city-owner-transfer", "scatter", "return"] {
        let fresh = render(scenario(branch), art, Some("fresh"), 11).unwrap();
        assert!(
            !fresh["steps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["action"] == "end_turn")
        );
        let aged = render(scenario(branch), art, Some("aged"), 11).unwrap();
        assert_eq!(aged["steps"][1]["name"], fresh["steps"][1]["name"]);
        let steps = aged["steps"].as_array().unwrap();
        let age = steps
            .iter()
            .position(|s| s["id"] == "age-original")
            .unwrap();
        let handoff = steps
            .iter()
            .position(|s| s["id"] == "old-results-history-proof")
            .unwrap();
        assert!(age > handoff);
        assert_eq!(steps[age]["turns"], 11);
        assert_eq!(steps[age + 1]["id"], "begin");
        assert!(render(scenario(branch), art, None, 11).is_err());
    }
    assert!(
        render(
            scenario("artifact-prepare"),
            "ARTIFACT_FULL_SUCCESS_SAVE",
            None,
            11
        )
        .is_err()
    );
    assert!(render(scenario("artifact-prepare"), "../another-save", None, 11).is_err());
}

fn normalize_rendered_inputs(mut case: Value, template: &Value, age: &str) -> Value {
    let steps = case["steps"].as_array_mut().unwrap();
    let input = steps.iter_mut().find(|s| s["action"] == "load").unwrap();
    input["name"] = template["parameters"]["source_save"].clone();
    for step in steps {
        if let Some(source) = step["code"].as_str() {
            step["code"] = source
                .replace(
                    &format!("local ageCase='{age}'"),
                    "local ageCase='__TYPE_AGE__'",
                )
                .into();
        }
    }
    case["parameters"]["source_save"] = template["parameters"]["source_save"].clone();
    case["parameters"]["age"] = template["parameters"]["age"].clone();
    case
}

#[test]
fn all_six_art_and_artifact_renderings_preserve_every_template_step_except_save_and_age() {
    let mut count = 0;
    for name in ["city-owner-transfer", "return", "scatter"] {
        let template = scenario(name);
        for (save, age) in [
            (
                "JAC-type-art-prepared-absorb-flow-155-777473861537",
                "fresh",
            ),
            (
                "JAC-type-artifact-prepared-absorb-flow-160-4ec876518338",
                "artifact",
            ),
        ] {
            let rendered = render(template.clone(), save, Some(age), 11).unwrap();
            assert_eq!(
                normalize_rendered_inputs(rendered.clone(), &template, age),
                template,
                "{name}/{age}: rendering failed to propagate a template step"
            );
            let steps = rendered["steps"].as_array().unwrap();
            for required in [
                "old-results-snapshot",
                "old-results-screen-ack",
                "old-results-history-proof",
                "native-intent-source-published",
            ] {
                assert!(
                    steps.iter().any(|s| s["id"] == required),
                    "{name}/{age}: missing {required}"
                );
            }
            let checks = steps.iter().find(|s| s["id"] == "checks").unwrap()["code"]
                .as_str()
                .unwrap();
            assert!(
                checks.contains("if t.Role~='Human' then"),
                "kept human recovery land fix missing"
            );
            let begin = steps.iter().find(|s| s["id"] == "begin").unwrap()["code"]
                .as_str()
                .unwrap();
            assert!(
                begin.contains("center.Role=role"),
                "prepared save roles were not bound to guard relocation"
            );
            let mut stale = rendered.clone();
            stale["steps"]
                .as_array_mut()
                .unwrap()
                .retain(|s| s["id"] != "old-results-screen-ack");
            assert_ne!(
                normalize_rendered_inputs(stale, &template, age),
                template,
                "parity accepted a rendered case missing the actual old-result UI handoff"
            );
            let mut stale_guard = rendered;
            for s in stale_guard["steps"].as_array_mut().unwrap() {
                if s["id"] == "checks" {
                    s["code"] = s["code"]
                        .as_str()
                        .unwrap()
                        .replace("if t.Role~='Human' then", "if true then")
                        .into();
                }
            }
            assert_ne!(
                normalize_rendered_inputs(stale_guard, &template, age),
                template,
                "parity accepted a rendered case using the old guard prohibition"
            );
            count += 1;
        }
    }
    assert_eq!(count, 6);
}

fn product_to_intent(name: &str, kind: &str, age: &str) -> mlua::Result<Lua> {
    let lua = model(kind, age)?;
    execute(&lua, "art-prepare", "prepare-proof")?;
    result_handoff(&lua, name)?;
    lua.globals().set("CONTEXT", "Gameplay")?;
    lua.load(
        step(name, "begin")["code"]
            .as_str()
            .unwrap()
            .replace("__TYPE_AGE__", age),
    )
    .set_name(format!("{name}/begin {kind}/{age}"))
    .exec()?;
    let calls_before: usize = lua.load("return #RANDOM_CALLS").eval()?;
    boot(&lua)?;
    execute(&lua, name, "checks-choice-reloaded")?;
    execute(&lua, name, "choice-reload-proof")?;
    assert_eq!(
        lua.load("return #RANDOM_CALLS").eval::<usize>()?,
        calls_before
    );
    execute(&lua, name, "screen-choices")?;
    drive(&lua, name, "screen-selected")?;
    drive(&lua, name, "screen-confirm-ready")?;
    execute(&lua, name, "hold-native-publication")?;
    execute(&lua, name, "screen-confirm")?;
    drive(&lua, name, "native-intent-ready")?;
    drive(&lua, name, "native-intent-source-published")?;
    execute(&lua, name, "native-intent-proof")?;
    Ok(lua)
}

fn result_handoff(lua: &Lua, name: &str) -> mlua::Result<()> {
    execute(lua, name, "old-results-snapshot")?;
    execute(lua, name, "old-results-screen-ack")?;
    drive(lua, name, "old-results-acknowledged")?;
    execute(lua, name, "old-results-history-proof")
}

fn old_flow(lua: &Lua, phase: &str, status: &str) -> mlua::Result<()> {
    lua.globals().set("OLD_PHASE", phase)?;
    lua.globals().set("OLD_STATUS", status)?;
    lua.load(r#"
    local A=JAC_AbsorptionGameplay
    local state=A.GetState()
    state.Transactions['old-result']={ID='old-result',From=0,To=2,Status=OLD_STATUS,Units={},NativeWorks={},HistoricalMarker='keep'}
    Game:SetProperty(A.Rules.STATE_KEY,state)
    Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows={
      ['old-result']={ID='old-result',From=0,To=2,Phase=OLD_PHASE,Phase1Complete=true,
       Request={ID='old-result',From=0,To=2,Mode='MERGER'},CompletedTurn=4,
       Entries={{ID='old-item',Kind='ITEM',Name='Old item',FinalOutcome='TRANSFER',
        Rolls={P1_ESCAPE={Raw=13,Used=13},P3_SUCCESS={Raw=23,Used=23}}}}},
      ['old-done']={ID='old-done',From=3,To=0,Phase='DONE',Phase1Complete=true,
       AcknowledgedTurn=1,History={Original='retained'}}},
      World={['old-world']={ID='old-world',FlowID='old-result',Status='COMPLETE',
       Recipient=2,AssetID='old-item',HistoricalMarker='world retained'}}})
    "#).exec()
}

#[test]
fn old_completed_results_are_acknowledged_via_real_ui_without_erasing_history() -> mlua::Result<()>
{
    for name in ["city-owner-transfer", "return", "scatter"] {
        let lua = model("art", "fresh")?;
        old_flow(&lua, "RESULT", "COMPLETE")?;
        lua.load("local R=JAC_AbsorptionGameplay.Rules;local state=JAC_AbsorptionGameplay.GetState();state.Transactions['old-result-2']=R.Copy(state.Transactions['old-result']);state.Transactions['old-result-2'].ID='old-result-2';state.Transactions['old-result-2'].From=3;Game:SetProperty(R.STATE_KEY,state);local f=Game:GetProperty('JAC_ABSORPTION_FLOW_V1');f.Flows['old-result-2']=R.Copy(f.Flows['old-result']);f.Flows['old-result-2'].ID='old-result-2';f.Flows['old-result-2'].From=3;Game:SetProperty('JAC_ABSORPTION_FLOW_V1',f)").exec()?;
        let calls = lua.load("return #RANDOM_CALLS").eval::<usize>()?;
        result_handoff(&lua, name)?;
        lua.load("assert(JAC_AbsorptionFlowGameplay.Get('old-result-2').Phase=='DONE','second old result was not acknowledged')").exec()?;
        assert_eq!(lua.load("return #RANDOM_CALLS").eval::<usize>()?, calls);
        lua.load("assert(JAC_AbsorptionFlowGameplay.Get('old-result').Phase=='DONE');assert(Game:GetProperty('JAC_ABSORPTION_FLOW_V1').World['old-world'].HistoricalMarker=='world retained')").exec()?;
        result_handoff(&lua, name)?;
        lua.globals().set("CONTEXT", "Gameplay")?;
        lua.load(
            step(name, "begin")["code"]
                .as_str()
                .unwrap()
                .replace("__TYPE_AGE__", "fresh"),
        )
        .exec()?;
        lua.load("assert(JAC_AbsorptionFlowGameplay.Get(Game:GetProperty('JAC_TYPE_CASE').ID).Phase=='CHOICE')").exec()?;
    }
    Ok(())
}

#[test]
fn old_choice_applying_and_incomplete_results_cannot_be_acknowledged_or_reset() -> mlua::Result<()>
{
    for (phase, status) in [
        ("CHOICE", "COMPLETE"),
        ("APPLYING", "STAGED"),
        ("RESULT", "STAGED"),
    ] {
        let lua = model("art", "fresh")?;
        old_flow(&lua, phase, status)?;
        lua.load("local R=JAC_AbsorptionGameplay.Rules;local state=JAC_AbsorptionGameplay.GetState();state.Transactions['a-completed']=R.Copy(state.Transactions['old-result']);state.Transactions['a-completed'].ID='a-completed';state.Transactions['a-completed'].From=3;state.Transactions['a-completed'].Status='COMPLETE';Game:SetProperty(R.STATE_KEY,state);local f=Game:GetProperty('JAC_ABSORPTION_FLOW_V1');f.Flows['a-completed']=R.Copy(f.Flows['old-result']);f.Flows['a-completed'].ID='a-completed';f.Flows['a-completed'].From=3;f.Flows['a-completed'].Phase='RESULT';Game:SetProperty('JAC_ABSORPTION_FLOW_V1',f);ACK_CALLS=0;local send=UI.RequestPlayerOperation;UI.RequestPlayerOperation=function(actor,operation,params) if params.Command=='ACK' then ACK_CALLS=ACK_CALLS+1 end return send(actor,operation,params) end").exec()?;
        let before: String=lua.load("return JAC_AbsorptionGameplay.Canonical({Flow=Game:GetProperty('JAC_ABSORPTION_FLOW_V1'),State=JAC_AbsorptionGameplay.GetState()})").eval()?;
        assert!(execute(&lua, "city-owner-transfer", "old-results-snapshot").is_err());
        assert_eq!(lua.load("return JAC_AbsorptionGameplay.Canonical({Flow=Game:GetProperty('JAC_ABSORPTION_FLOW_V1'),State=JAC_AbsorptionGameplay.GetState()})").eval::<String>()?,before);
        lua.load("assert(Game:GetProperty('JAC_TYPE_RESULT_HANDOFF')==nil,'failed preflight changed saved state')").exec()?;
        lua.load("assert(ACK_CALLS==0 and JAC_AbsorptionFlowGameplay.Get('a-completed').Phase=='RESULT','mixed preflight acknowledged a completed result before rejecting the unfinished flow')").exec()?;
    }
    Ok(())
}

fn run_product_branch(name: &str, kind: &str, age: &str) -> mlua::Result<Lua> {
    let lua = product_to_intent(name, kind, age)?;
    boot(&lua)?;
    execute(&lua, name, "checks-intent-reloaded")?;
    execute(&lua, name, "slot-type-resume-native-intent")?;
    execute(&lua, name, "resume-native-intent")?;
    drive(&lua, name, "result-ready")?;
    execute(&lua, name, "slot-type-result-proof")?;
    execute(&lua, name, "result-proof")?;
    execute(&lua, name, "request-exact-verification")?;
    drive(&lua, name, "verification-published")?;
    execute(&lua, name, "verify-exact")?;
    boot(&lua)?;
    execute(&lua, name, "checks-result-reloaded")?;
    execute(&lua, name, "slot-type-result-reload-proof")?;
    execute(&lua, name, "result-reload-proof")?;
    execute(&lua, name, "request-reloaded-verification")?;
    drive(&lua, name, "reloaded-verification-published")?;
    execute(&lua, name, "verify-exact-reloaded")?;
    execute(&lua, name, "screen-result-acknowledge")?;
    drive(&lua, name, "done")?;
    if name == "scatter" {
        execute(&lua, name, "schedule-original-world-reward")?;
        drive(&lua, name, "world-site-ready")?;
        execute(&lua, name, "prepare-recovery")?;
        execute(&lua, name, "hold-recovery-publication")?;
        execute(&lua, name, "claim-original-site")?;
        drive(&lua, name, "recovery-source-published")?;
        execute(&lua, name, "recovery-source-proof")?;
        boot(&lua)?;
        execute(&lua, name, "checks-recovery-reloaded")?;
        execute(&lua, name, "resume-world-recovery")?;
        drive(&lua, name, "recovered-ready")?;
        execute(&lua, name, "recovered-proof")?;
        boot(&lua)?;
        execute(&lua, name, "checks-recovered-reloaded")?;
        execute(&lua, name, "recovered-reload-proof")?;
    }
    Ok(lua)
}

#[test]
fn city_owner_product_transfer_never_sends_an_individual_work_move() -> mlua::Result<()> {
    for (kind, age) in [("art", "fresh"), ("art", "aged"), ("artifact", "artifact")] {
        let lua = run_product_branch("city-owner-transfer", kind, age)?;
        lua.load("assert(MOVE_CALLS==0 and DEAL_CALLS==0,'ordinary museum inheritance issued an individual MOVE or deal')").exec()?;
    }
    Ok(())
}

#[test]
fn type_return_uses_actual_panel_and_preserves_reserved_original_after_reload() -> mlua::Result<()>
{
    for (kind, age) in [("art", "fresh"), ("art", "aged"), ("artifact", "artifact")] {
        run_product_branch("return", kind, age)?;
    }
    Ok(())
}

#[test]
fn type_scatter_recovers_the_same_original_once_after_both_intent_reloads() -> mlua::Result<()> {
    for (kind, age) in [("art", "fresh"), ("art", "aged"), ("artifact", "artifact")] {
        run_product_branch("scatter", kind, age)?;
    }
    Ok(())
}

#[test]
fn accepted_move_without_a_physical_effect_cannot_pass_the_scatter_oracle() -> mlua::Result<()> {
    let lua = product_to_intent("scatter", "art", "fresh")?;
    lua.globals().set("MOVE_EFFECT", false)?;
    boot(&lua)?;
    execute(&lua, "scatter", "checks-intent-reloaded")?;
    execute(&lua, "scatter", "slot-type-resume-native-intent")?;
    execute(&lua, "scatter", "resume-native-intent")?;
    assert!(drive(&lua, "scatter", "result-ready").is_err());
    assert!(execute(&lua, "scatter", "result-proof").is_err());
    lua.load("local k=Game:GetProperty('JAC_TYPE_CASE');local H=ExposedMembers.JAC_TypeChecks;H.At(k.Source,k.Original);H.Restored(k.Museums.Free,k.Index);assert(not JAC_AbsorptionGameplay.GetRoster(k.To)[k.Asset]);assert(MOVE_CALLS==2,'unchanged MOVE did not stop after its two saved attempts: '..MOVE_CALLS)").exec()?;
    Ok(())
}

#[test]
#[ignore = "explicit renderer: requires actual successful source save and output arguments"]
fn render_type_scenario() {
    let name = std::env::var("JAC_TYPE_TEMPLATE").expect("JAC_TYPE_TEMPLATE required");
    assert!(NAMES.contains(&name.as_str()), "unknown type template");
    let save = std::env::var("JAC_TYPE_SOURCE_SAVE").expect("JAC_TYPE_SOURCE_SAVE required");
    let age = std::env::var("JAC_TYPE_AGE").ok();
    let turns = std::env::var("JAC_TYPE_AGED_TURNS")
        .map_or(11, |v| v.parse().expect("positive turn count"));
    let output =
        PathBuf::from(std::env::var_os("JAC_TYPE_OUTPUT").expect("JAC_TYPE_OUTPUT required"));
    assert_eq!(
        output.parent().unwrap().canonicalize().unwrap(),
        scenario_dir().canonicalize().unwrap()
    );
    let file = output.file_name().unwrap().to_str().unwrap();
    assert!(
        file.starts_with("absorption-native-type-run-") && file.ends_with(".json"),
        "output must be a dedicated rendered type scenario"
    );
    assert!(
        !output.exists(),
        "refusing to overwrite an existing rendered case"
    );
    let data =
        render(scenario(&name), &save, age.as_deref(), turns).expect("valid source save and age");
    fs::write(&output, serde_json::to_vec_pretty(&data).unwrap()).unwrap();
    println!("rendered {}", output.display());
}

#[test]
fn committed_six_type_cases_equal_their_current_template_and_saved_input() {
    for (template, file, age) in [
        (
            "city-owner-transfer",
            "absorption-native-type-run-art-city-fresh.json",
            "fresh",
        ),
        (
            "return",
            "absorption-native-type-run-art-return-fresh.json",
            "fresh",
        ),
        (
            "scatter",
            "absorption-native-type-run-art-scatter-fresh.json",
            "fresh",
        ),
        (
            "city-owner-transfer",
            "absorption-native-type-run-artifact-city.json",
            "artifact",
        ),
        (
            "return",
            "absorption-native-type-run-artifact-return.json",
            "artifact",
        ),
        (
            "scatter",
            "absorption-native-type-run-artifact-scatter.json",
            "artifact",
        ),
    ] {
        let actual: Value =
            serde_json::from_slice(&fs::read(scenario_dir().join(file)).unwrap()).unwrap();
        let save = actual["parameters"]["source_save"].as_str().unwrap();
        assert_eq!(actual["parameters"]["age"], age);
        let expected = render(scenario(template), save, Some(age), 11).unwrap();
        assert_eq!(
            actual, expected,
            "committed case diverged from its source template: {file}"
        );
    }
}

#[test]
fn kept_recovery_city_allows_same_owner_plot_membership_only_and_borrowed_city_stays_exact()
-> mlua::Result<()> {
    let lua = model("artifact", "artifact")?;
    let data = scenario("return");
    let helper = data["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|s| {
            s["code"]
                .as_str()
                .filter(|code| code.contains("function H.Kept"))
        })
        .unwrap();
    lua.load(helper).exec()?;
    lua.load(r#"
        local A=JAC_AbsorptionGameplay
        local H=ExposedMembers.JAC_TypeChecks
        local before={Owner=2,X=4,Y=5,Population=3,Assets={museum={Works={[0]=-1}}},
          PurchasedPlots={8,9},UnitsOnCenter={},Governance={ID='original'}}
        CURRENT=A.Rules.Copy(before);CURRENT.PurchasedPlots={8};PLOT_OWNER=2
        H.City=function() return A.Rules.Copy(CURRENT) end
        CityManager.GetCityAt=function() return {} end
        Map.GetPlotByIndex=function() return {GetOwner=function() return PLOT_OWNER end} end
        H.Kept(before,900)
        assert(not pcall(H.Restored,before,900),'borrowed city accepted a changed purchased-plot set')
        PLOT_OWNER=3
        assert(not pcall(H.Kept,before,900),'recovery city lost real territory ownership')
        PLOT_OWNER=2;CURRENT.Population=4
        assert(not pcall(H.Kept,before,900),'same-owner membership exception hid population change')
        CURRENT.Population=3
        Map.GetPlotByIndex=function(plot) return {GetOwner=function() return plot==9 and 3 or 2 end} end
        assert(not pcall(H.Kept,before,900),'only the removed plot changed owner without detection')
        Map.GetPlotByIndex=function() return {GetOwner=function() return PLOT_OWNER end} end
        CURRENT.Population=3;CURRENT.Assets.museum.Works[0]=901
        assert(not pcall(H.Kept,before,900),'same-owner membership exception hid another original work')
    "#).exec()
}

#[test]
fn first_recovery_intent_is_immutable_across_publication() -> mlua::Result<()> {
    let lua = model("artifact", "artifact")?;
    let data = scenario("scatter");
    let helper = data["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|s| {
            s["code"]
                .as_str()
                .filter(|code| code.contains("function H.RecoveryIntent"))
        })
        .unwrap();
    lua.load(helper).exec()?;
    lua.load(r#"
        local A=JAC_AbsorptionGameplay
        local H=ExposedMembers.JAC_TypeChecks
        local first={Source={Index=900,Owner=3,CityID=40},Target={Owner=2,City=10,Slot=0},Token='first'}
        local k={Asset='native-work:900',To=2,RecoveryIntent=A.Rules.Copy(first)}
        local intent={ID=k.Asset,Owner=k.To,NativeSource=A.Rules.Copy(first.Source),NativeTarget=A.Rules.Copy(first.Target),ObservationToken=first.Token,NativeCarrier={Phase='BORROW',BorrowToken=first.Token}}
        H.RecoveryIntent(k,intent)
        for _,change in ipairs({'source','target','token'}) do
          local current=A.Rules.Copy(intent)
          if change=='source' then current.NativeSource.Owner=9 end
          if change=='target' then current.NativeTarget.Slot=1 end
          if change=='token' then current.ObservationToken='another' end
          assert(not pcall(H.RecoveryIntent,k,current),'changed recovery intent accepted: '..change)
          assert(A.Rules.Equal(k.RecoveryIntent,first),'first intent was overwritten by its observation')
        end
    "#).exec()
}

#[test]
fn native_intent_waits_only_for_actual_city_cache_readiness_and_one_original() -> mlua::Result<()> {
    for name in ["city-owner-transfer", "return", "scatter"] {
        let lua = model("artifact", "artifact")?;
        lua.load("ORIGINAL_NATIVE_READ=ExposedMembers.JAC_HeroReadAPI.ReadNativeWork;ExposedMembers.JAC_HeroReadAPI.ReadNativeWork=function() error('UI city cache is not ready') end").exec()?;
        assert!(!condition(&lua, name, "native-intent-source-published")?);
        lua.load("ExposedMembers.JAC_HeroReadAPI.ReadNativeWork=function() error('native provenance corruption') end").exec()?;
        assert!(condition(&lua, name, "native-intent-source-published").is_err());
        lua.load("ExposedMembers.JAC_HeroReadAPI.ReadNativeWork=ORIGINAL_NATIVE_READ")
            .exec()?;
        assert!(condition(&lua, name, "native-intent-source-published")?);
        lua.load("WORKS[10]={[0]=INDEX}").exec()?;
        assert!(!condition(&lua, name, "native-intent-source-published")?);
        lua.load("WORKS[10]={};WORKS[4]={}").exec()?;
        assert!(!condition(&lua, name, "native-intent-source-published")?);
    }
    Ok(())
}
