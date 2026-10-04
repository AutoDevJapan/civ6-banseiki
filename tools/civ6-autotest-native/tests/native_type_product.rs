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

const NAMES: &[&str] = &["art-prepare", "artifact-prepare"];

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
        INDEX=KIND=='art' and 3 or 17
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
          city.GetBuildings=function(self)
            local b={HasBuilding=function(_,index) return index==1 end,
              IsPillaged=function() return false end,
              GetNumGreatWorkSlots=function(_,index) return index==1 and 3 or 0 end,
              GetGreatWorkInSlot=function(_,building,slot)
                return building==1 and WORKS[self.x] and WORKS[self.x][slot] or -1
              end}
            if CONTEXT=='UI' then
              b.GetGreatWorkTypeFromIndex=function() ui_only();return 4 end
              b.GetCreatorNameFromIndex=function() ui_only();return 'Original artist' end
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
        local guard=UNIT(2,500,'UNIT_WARRIOR',18)
        guard.damage,guard.moves=7,1
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
            steps.insert(3, json!({"action":"end_turn","id":"age-original","turns":aged_turns,"test_turns":aged_turns,"timeout":1800}));
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
