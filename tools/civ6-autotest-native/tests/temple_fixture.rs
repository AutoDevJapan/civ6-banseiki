//! Scenario-only fixture checks; these do not establish engine API availability.
use mlua::{Lua, Table};
use serde_json::Value;

const SCENARIO: &str = include_str!("../../civ6-autotest/scenarios/absorption-native-custody.json");

fn scenario() -> Value {
    serde_json::from_str(SCENARIO).expect("custody scenario JSON")
}

fn script(marker: &str) -> String {
    scenario()["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .filter_map(|step| step["code"].as_str())
        .find(|code| code.contains(marker))
        .unwrap_or_else(|| panic!("missing script: {marker}"))
        .to_owned()
}

fn world() -> mlua::Result<Lua> {
    let lua = Lua::new();
    lua.load(
        r#"
        ExposedMembers={}
        local props={}
        Game={GetLocalPlayer=function() return 1 end,
          GetProperty=function(_,key) return props[key] end,
          SetProperty=function(_,key,value) props[key]=value end}
        GameInfo={Districts={DISTRICT_HOLY_SITE={Index=5}},
          Buildings={BUILDING_SHRINE={Index=2},BUILDING_TEMPLE={Index=3},
            BUILDING_JAC_ABSORPTION_CACHE={Index=4},[2]={Index=2},[3]={Index=3}},
          GreatWorkSlotTypes={[1]={GreatWorkSlotType='GREATWORKSLOT_RELIC'},
            [2]={GreatWorkSlotType='GREATWORKSLOT_ARTIFACT'}}}
        CITIES={};PLOTS={};CALLS={};PENDING={}
        local function city(id,x)
          local c={id=id,x=x,owner=1,districts={},purchased={},built={},at={},
            pillaged={},slotCount=1,slotType=1,work=-1}
          function c:GetID() return self.id end
          function c:GetOwner() return self.owner end
          function c:GetX() return self.x end
          function c:GetY() return 0 end
          function c:GetDistricts() return {Members=function() return ipairs(c.districts) end} end
          function c:GetBuildings()
            return {
              HasBuilding=function(_,b) return c.built[b]==true end,
              GetBuildingsAtLocation=function(_,plot) return c.at[plot] or {} end,
              IsPillaged=function(_,b) return c.pillaged[b]==true end,
              GetNumGreatWorkSlots=function(_,b) assert(b==3);return c.slotCount end,
              GetGreatWorkSlotType=function(_,b,s) assert(b==3 and s==0);return c.slotType end,
              GetGreatWorkInSlot=function(_,b,s) assert(b==3 and s==0);return c.work end}
          end
          function c:GetBuildQueue()
            local function enqueue(kind,index,plot,construction)
              assert(construction==100 and PLOTS[plot].purchase==c,
                'mutation targeted another city purchase plot')
              if kind=='building' then
                assert(c.districts[1].complete,'engine prerequisite: district incomplete')
                if index==3 then assert(c.built[2],'engine prerequisite: shrine absent') end
              end
              local request={city=c,kind=kind,index=index,plot=plot}
              CALLS[#CALLS+1]=request;PENDING[#PENDING+1]=request
              return nil -- A dispatch return is never a publication.
            end
            return {
              CreateIncompleteDistrict=function(_,b,p,n) return enqueue('district',b,p,n) end,
              CreateIncompleteBuilding=function(_,b,p,n) return enqueue('building',b,p,n) end}
          end
          CITIES[id]=c;return c
        end
        local function plot(index,x,purchase,district,center)
          local p={index=index,x=x,purchase=purchase,district=district or -1,center=center}
          function p:GetIndex() return self.index end
          function p:GetX() return self.x end
          function p:GetY() return 0 end
          function p:GetOwner() return self.purchase.owner end
          function p:GetDistrictType() return self.district end
          function p:GetImprovementType() return -1 end
          function p:IsCity() return self.center==true end
          function p:IsWater() return false end
          function p:IsMountain() return false end
          function p:IsImpassable() return false end
          function p:IsNaturalWonder() return false end
          PLOTS[index]=p;return p
        end
        function DISTRICT(city,plot,complete)
          local d={city=city,plot=plot,complete=complete,pillaged=false}
          function d:GetType() return 5 end
          function d:GetCity() return self.city end
          function d:GetX() return PLOTS[self.plot].x end
          function d:GetY() return 0 end
          function d:IsComplete() return self.complete end
          function d:IsPillaged() return self.pillaged end
          return d
        end
        local cap=city(10,0);local control=city(20,10);local neighbor=city(30,20)
        plot(0,0,cap,0,true);plot(1,2,cap,5);plot(2,1,neighbor,5)
        plot(3,3,cap);plot(10,10,control,0,true);plot(11,12,control,5)
        cap.purchased={0,1,2,3};control.purchased={10,11}
        cap.districts={DISTRICT(cap,1,false)}
        control.districts={DISTRICT(control,11,true)}
        neighbor.districts={DISTRICT(neighbor,2,true)}
        control.built={[2]=true,[3]=true};control.at[11]={2,3}
        local function at(x,y)
          for _,c in pairs(CITIES) do if c.x==x and y==0 then return c end end
        end
        CityManager={GetCityAt=at}
        CORE_CITY_MANAGER={GetCityAt=function(x,y)
          local c=at(x,y)
          if not c then return nil end
          return {GetID=function() return c.id end,GetOwner=function() return c.owner end,
            GetBuildQueue=function() return c:GetBuildQueue() end,
            GetDistricts=function() error('Gameplay district members must not be read') end,
            GetBuildings=function() error('Gameplay slot getters must not be read') end}
        end}
        Cities={GetPlotPurchaseCity=function(q) return q.purchase end}
        Map={GetPlotByIndex=function(i) return PLOTS[i] end,
          GetPlot=function(x,y) for _,p in pairs(PLOTS) do if p.x==x and y==0 then return p end end end,
          GetPlotDistance=function(x,y,a,b) return math.abs(x-a) end,
          GetCityPlots=function() return {GetPurchasedPlots=function(_,c) return c.purchased end} end}
        Players={[1]={GetID=function() return 1 end,GetCities=function()
          return {GetCapitalCity=function() return cap end,
            Members=function() return ipairs({cap,control}) end}
        end}}
        function PUBLISH()
          assert(#PENDING==1,'one stage must be requested at a time')
          local r=table.remove(PENDING,1)
          if r.kind=='district' then
            if #r.city.districts==0 then r.city.districts={DISTRICT(r.city,r.plot,true)} end
            r.city.districts[1].complete=true;PLOTS[r.plot].district=5
          else
            r.city.built[r.index]=true
            r.city.at[r.plot]=r.city.at[r.plot] or {}
            table.insert(r.city.at[r.plot],r.index)
          end
        end
        "#,
    )
    .exec()?;
    lua.load("SAVED_GAME_SETTER=Game.SetProperty;Game.SetProperty=nil")
        .exec()?;
    lua.load(script("function F.Plan(")).exec()?;
    lua.load(
        r#"
        Game.SetProperty=SAVED_GAME_SETTER
        local function copy(v)
          if type(v)~='table' then return v end
          local result={};for k,value in pairs(v) do result[k]=copy(value) end;return result
        end
        JAC_AbsorptionGameplay={Rules={Copy=copy}}
    "#,
    )
    .exec()?;
    lua.load(script("persist detached native temple plan in Gameplay"))
        .exec()?;
    let core: Table = lua.create_table()?;
    let metatable = lua.create_table()?;
    metatable.set("__index", lua.globals())?;
    core.set_metatable(Some(metatable))?;
    core.set(
        "CityManager",
        lua.globals().get::<Table>("CORE_CITY_MANAGER")?,
    )?;
    lua.load(script("local function requestTemple("))
        .set_environment(core)
        .exec()?;
    Ok(lua)
}

#[test]
fn temple_plan_finds_nonadjacent_own_district_and_reuses_completed_control() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
        local t=Game:GetProperty('JAC_NATIVE_CUSTODY_PROBE')
        assert(t.Fixtures.source.Plot==1 and t.Fixtures.control.Plot==11)
        assert(t.Source.ID==10 and t.Control.ID==20 and t.Cache==nil)
        local F=ExposedMembers.JAC_NativeTempleFixture
        for _,stage in ipairs({'Holy','Shrine','Temple'}) do
          ExposedMembers.JAC_NativeTempleRequest('control',stage)
          assert(F.Ready('control',stage))
        end
        assert(#CALLS==0,'already complete stock Temple was recreated')
        local r=F.Read('control','exact')
        assert(r.Owner==1 and r.ID==20 and r.Plot==11 and r.Temple
          and r.SlotCount==1 and r.SlotType=='GREATWORKSLOT_RELIC' and r.SlotValue==-1)
        "#,
    )
    .exec()
}

#[test]
fn temple_plan_rejects_same_player_other_city_district_and_purchase_plot() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
        local F=ExposedMembers.JAC_NativeTempleFixture
        local cap=CITIES[10]
        cap.districts={CITIES[30].districts[1]}
        cap.purchased={2,3}
        assert(F.Plan(cap,'carrier').Plot==3,'used adjacent Holy Site belonging to neighbor')
        cap.purchased={2}
        local ok,err=pcall(F.Plan,cap,'carrier')
        assert(not ok and tostring(err):find('no Holy Site or empty plot purchased by this exact city',1,true))
        assert(#CALLS==0,'failed plan mutated a neighboring city')
        "#,
    )
    .exec()
}

#[test]
fn temple_requests_wait_for_each_published_stage_without_core_ui_getters() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
        local R=ExposedMembers.JAC_NativeTempleRequest
        local F=ExposedMembers.JAC_NativeTempleFixture
        R('source','Holy')
        assert(#CALLS==1 and CALLS[1].kind=='district' and CALLS[1].plot==1)
        assert(not F.Ready('source','Holy'),'nil request return proved completion')
        local ok,err=pcall(R,'source','Shrine')
        assert(not ok and tostring(err):find('Holy Site not published',1,true) and #CALLS==1)
        PUBLISH();assert(F.Ready('source','Holy'))
        R('source','Shrine')
        assert(#CALLS==2 and CALLS[2].index==2 and not F.Ready('source','Shrine'))
        ok,err=pcall(R,'source','Temple')
        assert(not ok and tostring(err):find('shrine not published',1,true) and #CALLS==2)
        PUBLISH();assert(F.Ready('source','Shrine'))
        R('source','Temple')
        assert(#CALLS==3 and CALLS[3].index==3 and not F.Ready('source','Temple'))
        PUBLISH();assert(F.Ready('source','Temple'))
        ok,err=pcall(R,'source','Temple')
        assert(not ok and tostring(err):find('duplicate temple request',1,true) and #CALLS==3)
        "#,
    )
    .exec()
}

#[test]
fn temple_stale_or_changed_identity_observations_do_not_advance() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
        local F=ExposedMembers.JAC_NativeTempleFixture
        local R=ExposedMembers.JAC_NativeTempleRequest
        local read=F.Read
        R('control','Holy')
        local stale=F.Read('control','control:Holy')
        stale.Read=Game:GetProperty('JAC_NATIVE_CUSTODY_PROBE').Fixtures.control.Request.Read
        F.Read=function() return stale end
        local ok,err=pcall(F.Ready,'control','Holy')
        assert(not ok and tostring(err):find('stale temple observation',1,true))
        F.Read=read
        CITIES[20].id=99
        ok,err=pcall(R,'control','Shrine')
        assert(not ok and tostring(err):find('fixture city identity changed',1,true) and #CALLS==0)
        CITIES[20].id=20;PLOTS[11].purchase=CITIES[10]
        ok,err=pcall(R,'control','Shrine')
        assert(not ok and tostring(err):find('purchase city changed',1,true) and #CALLS==0)
        "#,
    )
    .exec()
}

#[test]
fn temple_completion_requires_actual_plot_complete_district_and_exact_relic_slot()
-> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
        local F=ExposedMembers.JAC_NativeTempleFixture
        local c=CITIES[20]
        c.at[11]={2};c.at[10]={3}
        assert(not F.Read('control','wrong-plot').Temple,'HasBuilding alone accepted wrong plot')
        c.at[11]={2,3};c.pillaged[2]=true
        assert(not F.Read('control','pillaged').Temple)
        c.pillaged[2]=false;c.districts[1].complete=false
        assert(not F.Read('control','incomplete').Temple)
        c.districts[1].complete=true;c.slotCount=2
        local ok,err=pcall(F.Read,'control','wrong-count')
        assert(not ok and tostring(err):find('exact stock relic slot 0',1,true))
        c.slotCount=1;c.slotType=2
        ok,err=pcall(F.Read,'control','wrong-type')
        assert(not ok and tostring(err):find('exact stock relic slot 0',1,true))
        c.slotType=1
        assert(F.Read('control','complete').Temple)
        "#,
    )
    .exec()
}

#[test]
fn official_control_precedes_separate_carrier_and_all_temple_waits_are_bounded() -> mlua::Result<()>
{
    let scenario = scenario();
    let steps = scenario["steps"].as_array().expect("steps");
    let lua = Lua::new();
    let mut checked = 0;
    for step in steps {
        if step["state_name"].as_str() == Some("InGame") {
            assert!(
                !step["code"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("Game:SetProperty("),
                "UI must not persist Gameplay state"
            );
        }
        for code in [step["code"].as_str(), step["condition"]["code"].as_str()]
            .into_iter()
            .flatten()
        {
            lua.load(code).into_function()?;
            checked += 1;
        }
    }
    assert_eq!(checked, 48, "all scenario Lua scripts must compile");
    let baseline = steps
        .iter()
        .position(|s| {
            s["code"]
                .as_str()
                .is_some_and(|c| c.contains("M.Request('local-official'"))
        })
        .expect("first control");
    let carrier = steps
        .iter()
        .position(|s| {
            s["code"]
                .as_str()
                .is_some_and(|c| c.contains("p:GetCities():Create("))
        })
        .expect("separate carrier creation");
    let grant = steps
        .iter()
        .position(|s| {
            s["code"]
                .as_str()
                .is_some_and(|c| c.contains("p:AttachModifierByID("))
        })
        .expect("relic grant");
    let control = steps
        .iter()
        .position(|s| {
            s["code"]
                .as_str()
                .is_some_and(|c| c.contains("JAC_NativeTempleRequest)('control','Holy')"))
        })
        .expect("control preparation");
    assert!(grant < control && control < baseline);
    assert_eq!(steps[grant + 1]["action"], "wait_for");
    assert_eq!(steps[grant + 1]["timeout"], 20);
    assert!(baseline < carrier);
    assert_eq!(steps[baseline + 1]["action"], "wait_for");
    assert!(
        steps[baseline + 1]["condition"]["code"]
            .as_str()
            .expect("physical oracle")
            .contains(".Verify()")
    );
    let mut waits = 0;
    for (i, step) in steps.iter().enumerate() {
        let Some(code) = step["code"].as_str() else {
            continue;
        };
        if code.starts_with("return assert(ExposedMembers.JAC_NativeTempleRequest)") {
            let next = &steps[i + 1];
            assert_eq!(step["state_name"], "JAC_Absorption_Entry");
            assert_eq!(next["action"], "wait_for");
            assert_eq!(next["timeout"], 20);
            assert_eq!(next["condition"]["state_name"], "InGame");
            assert!(
                next["condition"]["code"]
                    .as_str()
                    .expect("fresh read")
                    .contains(".Ready(")
            );
            waits += 1;
        }
    }
    assert_eq!(
        waits, 9,
        "source/control/carrier need all three publication stages"
    );
    Ok(())
}
