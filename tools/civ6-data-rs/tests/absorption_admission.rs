use civ6_data_rs::{absorption_admission, Repository};
use mlua::Lua;

#[test]
fn normal_fixture_proves_gameplay_roles_and_visible_controller_in_separate_contexts(
) -> anyhow::Result<()> {
    let scene: serde_json::Value = serde_json::from_str(&absorption_admission::live_scenario(
        &Repository::checked_in(),
    )?)?;
    let steps = scene["steps"].as_array().unwrap();
    let core = steps[6]["code"].as_str().unwrap();
    let ui = steps[7]["code"].as_str().unwrap();
    assert_eq!(steps[6]["state_name"], "JAC_Absorption_Entry");
    assert_eq!(steps[7]["state_name"], "InGame");
    let lua = Lua::new();
    lua.load(r#"
      PlayerManager={GetAliveMajors=function() return {{GetID=function()return 3 end},{GetID=function()return 1 end}} end}
      Players={[3]={IsHuman=function()return false end},[1]={IsHuman=function()return true end}}
      PlayerConfigurations={[3]={GetCivilizationTypeName=function()return 'CIVILIZATION_JAC_TAKEDA' end},[1]={GetCivilizationTypeName=function()return 'CIVILIZATION_JAC_ODA' end}}
      Game={GetLocalPlayer=function() error('UI controller lookup in Gameplay') end,SetProperty=function(_,k,v) assert(k=='JAC_ABSORPTION_FLOW_LIVE_TEST' and v.From==3 and v.To==1);SAVED=true end}
      GameInfo={HeroClasses=function()local n=0;return function()n=n+1;if n<=2 then return {HeroClassType=n==1 and 'HEROCLASS_JAC_WARLORD_A' or 'HEROCLASS_JAC_WARLORD_B'} end end end}
    "#).exec()?;
    lua.load(core).exec()?;
    assert!(lua.globals().get::<bool>("SAVED")?);
    lua.load("Game.GetLocalPlayer=function()return 1 end;Game.GetProperty=function()error('UI property unavailable')end").exec()?;
    lua.load(ui).exec()?;
    lua.load("Game.GetLocalPlayer=function()return 0 end")
        .exec()?;
    assert!(lua.load(ui).exec().is_err());
    Ok(())
}

fn world() -> mlua::Result<Lua> {
    let lua = Lua::new();
    lua.load(
        r#"
        local function copy(v)
          if type(v)~='table' then return v end
          local out={}; for k,x in pairs(v) do out[k]=copy(x) end; return out
        end
        local function equal(a,b)
          if type(a)~=type(b) then return false end
          if type(a)~='table' then return a==b end
          for k,v in pairs(a) do if not equal(v,b[k]) then return false end end
          for k in pairs(b) do if a[k]==nil then return false end end; return true
        end
        local function keys(t) local out={}; for k in pairs(t) do out[#out+1]=k end
          table.sort(out); return out end
        JAC_AbsorptionGameplay={Rules={Copy=copy, Equal=equal, Keys=keys},PlayerFaithBalance=function() return FAITH end}
        TURN, GOLD, FAITH, ENVOYS, TILES, BEGIN_COUNT, FAIL_BEGIN = 7, 99, 10, 2, 2, 0, false
        CAN_RECEIVE=true
        local props={}
        Game={GetProperty=function(_,k) return copy(props[k]) end,
          SetProperty=function(_,k,v) props[k]=copy(v) end,
          GetCurrentGameTurn=function() return TURN end}
        Players={}
        for id=1,3 do
          Players[id]={IsAlive=function() return true end,IsHuman=function() return id==2 end,IsMajor=function() return true end,
            GetTreasury=function() return {GetGoldBalance=function() return GOLD end} end,
            GetReligion=function() return {GetFaithBalance=function() return FAITH end} end,
            GetInfluence=function() return {CanReceiveInfluence=function() return CAN_RECEIVE end,
              GetTokensReceived=function(_,sender)
              assert(sender==2);return ENVOYS end} end}
        end
        Map={GetPlotCount=function() return TILES end,
          GetPlotByIndex=function() return {GetOwner=function() return 1 end} end}
        local function event()
          local handlers={}; return {Add=function(fn) handlers[#handlers+1]=fn end,
            Call=function(...) for _,fn in ipairs(handlers) do fn(...) end end}
        end
        GameEvents={JAC_Absorption_AdmissionRequested=event(),JAC_Absorption_LeaderOffer=event(),
          OnGameTurnStarted=event()}
        FLOWS={}
        JAC_AbsorptionFlowGameplay={Begin=function(plan)
            BEGIN_COUNT=BEGIN_COUNT+1
            FLOWS[plan.ID]=FLOWS[plan.ID] or {ID=plan.ID, From=plan.From, To=plan.To, Phase='CHOICE', Request=copy(plan)}
            Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
            if FAIL_BEGIN then error('interrupted first-stage journal') end
            FLOWS[plan.ID].Phase1Complete=true; return copy(FLOWS[plan.ID])
          end}
        OFFER={ID='leader-1:terms-1',From=1,To=2,AreaKm2=9999,
          Requirements={{ID='food-reserve',Kind='GOLD_MINIMUM',Minimum=100},
            {ID='faith-reserve',Kind='FAITH_MINIMUM',Minimum=10}},
          RelationRequirements={{ID='federation-release',Kind='RELATION_RELEASE'}}, RelationsReviewed=true,
          Plan={Leader='RECRUIT'}}
        RELEASE=false
        "#,
    )
    .exec()?;
    load(&lua)?;
    Ok(lua)
}

fn load(lua: &Lua) -> mlua::Result<()> {
    lua.load(absorption_admission::lua()).exec()?;
    lua.load(
        "N=JAC_AbsorptionAdmissionGameplay; N.RegisterRequirement('RELATION_RELEASE',function() return RELEASE end)",
    )
    .exec()
}

#[test]
fn major_recipient_is_required_for_offers_requests_and_saved_prestart_plans() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      Players[2].IsMajor=function() return false end
      assert(not pcall(N.DefineOffer,OFFER))
      assert(not pcall(N.Request,'leader-offer',OFFER.ID,1,2))
      assert(Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1')==nil and BEGIN_COUNT==0)
      Players[2].IsMajor=function() return true end
      N.DefineOffer(OFFER);N.Request('leader-offer',OFFER.ID,1,2)
      Players[2].IsMajor=function() return false end
      GOLD=100;RELEASE=true;N.Tick();assert(BEGIN_COUNT==0)
      assert(N.Get('leader-offer',OFFER.ID).LastError:find('major civilization',1,true))
      assert(N.CanCancelOffer(OFFER.ID));N.CancelOffer(OFFER.ID)
      assert(N.Get('leader-offer',OFFER.ID).Status=='CANCELLED')
    "#,
    )
    .exec()
}

#[test]
fn every_admission_scene_checks_the_actual_start_year_and_rejects_wrong_calendars(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let mut scenes = vec![
        absorption_admission::live_scenario(&repo)?,
        absorption_admission::envoy_probe_scenario(&repo)?,
    ];
    for small in [true, false] {
        for human in [true, false] {
            scenes.push(civ6_data_rs::absorption_product::acceptance_scenario(
                &repo, small, human,
            )?);
        }
    }
    let check = absorption_admission::start_calendar_step();
    for (scene, path) in scenes.into_iter().zip([
        "tools/civ6-autotest/scenarios/absorption-normal-admission.json",
        "tools/civ6-autotest/scenarios/absorption-envoy-api.json",
        "tools/civ6-autotest/scenarios/absorption-product-small-human.json",
        "tools/civ6-autotest/scenarios/absorption-product-small-ai.json",
        "tools/civ6-autotest/scenarios/absorption-product-merger-human.json",
        "tools/civ6-autotest/scenarios/absorption-product-merger-ai.json",
    ]) {
        let scene: serde_json::Value = serde_json::from_str(&scene)?;
        let shipped: serde_json::Value = serde_json::from_str(&repo.read(path)?)?;
        assert_eq!(scene, shipped, "stale calendar acceptance scene: {path}");
        assert_eq!(
            scene["steps"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|step| **step == check)
                .count(),
            1
        );
    }
    for (era_year, calendar, turn, date, accepted) in [
        (
            1534,
            "CALENDAR_STANDARD_FROM_START_ERA",
            16603,
            "1534 AD",
            true,
        ),
        (
            1534,
            "CALENDAR_STANDARD_FROM_START_ERA",
            16603,
            "1534年",
            true,
        ),
        (
            1534,
            "CALENDAR_STANDARD_FROM_START_ERA",
            16603,
            "1534 г.",
            true,
        ),
        (
            -4000,
            "CALENDAR_STANDARD_FROM_START_ERA",
            16603,
            "4000 BC",
            false,
        ),
        (1534, "CALENDAR_DEFAULT", 16603, "1534 AD", false),
        (
            1534,
            "CALENDAR_STANDARD_FROM_START_ERA",
            16604,
            "1534 AD",
            false,
        ),
        (
            1534,
            "CALENDAR_STANDARD_FROM_START_ERA",
            16603,
            "15340 AD",
            false,
        ),
        (1534, "CALENDAR_STANDARD_FROM_START_ERA", 16603, "", false),
    ] {
        let lua = Lua::new();
        lua.load(format!("GameConfiguration={{GetRuleSet=function() return 'RULESET_JAC_BANSEIKI' end,GetCalendarType=function() return '{}' end,GetStartTurn=function() return 16603 end}};GameInfo={{StartEras={{ERA_ANCIENT={{Year={era_year}}}}}}};Game={{GetCurrentGameTurn=function() return {turn} end}};DB={{MakeHash=function() return 12345 end}};Calendar={{MakeYearStr=function() return {} end}}", calendar, serde_json::to_string(date)?)).exec()?;
        assert_eq!(
            lua.load(check["code"].as_str().unwrap())
                .eval::<String>()
                .is_ok(),
            accepted,
            "{date}"
        );
    }
    Ok(())
}

#[test]
fn faith_requirements_use_the_existing_context_aware_absorption_reader() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      Players[2].GetReligion=function() return {} end
      GOLD=100;RELEASE=true;FAITH=9
      N.DefineOffer(OFFER);N.Tick();assert(BEGIN_COUNT==0)
      FAITH=10;N.Tick();assert(BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn stored_and_dynamic_demand_order_is_canonical_by_id() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(r#"
      local original=JAC_AbsorptionGameplay.Rules.Copy(OFFER)
      OFFER.Requirements={original.Requirements[2],original.Requirements[1]}
      N.DefineOffer(OFFER)
      assert(N.GetOffer(OFFER.ID).Requirements[1].ID=='faith-reserve')
      N.DefineOffer(original)
      local reverse=false
      N.Register('dynamic',function()
        local a={ID='a',Met=true};local z={ID='z',Met=true}
        return {From=1,To=2,Turn=TURN,Mode='MERGER',AreaKm2=10000,RelationsResolved=true,Requirements=reverse and {z,a} or {a,z}}
      end)
      N.Request('dynamic','first',1,2)
      for _,flow in pairs(FLOWS) do flow.Phase='DONE' end
      Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
      reverse=true;N.Request('dynamic','second',1,2)
      local a,b=N.Get('dynamic','first'),N.Get('dynamic','second')
      assert(a.Decision.Requirements[1].ID=='a' and b.Decision.Requirements[1].ID=='a')
      assert(JAC_AbsorptionGameplay.Rules.Equal(a.Decision,b.Decision) and BEGIN_COUNT==2)
    "#).exec()
}

#[test]
fn generated_connection_is_packaged_without_a_test_or_ui_start_command() -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    assert_eq!(
        repo.read(absorption_admission::OUTPUT)?,
        absorption_admission::lua()
    );
    let manifest_text = repo.read("poc/JAC_PoC_Absorption/JAC_PoC_Absorption.modinfo")?;
    let manifest = roxmltree::Document::parse(&manifest_text)?;
    assert!(manifest.descendants().any(
        |n| n.has_tag_name("File") && n.text() == Some("Scripts/JAC_Absorption_Admission.lua")
    ));
    Ok(())
}

#[test]
fn live_recipe_uses_natural_turns_and_real_saves_without_direct_or_test_absorption_start(
) -> anyhow::Result<()> {
    let value: serde_json::Value = serde_json::from_str(&absorption_admission::live_scenario(
        &Repository::checked_in(),
    )?)?;
    let steps = value["steps"].as_array().unwrap();
    assert_eq!(
        steps.iter().filter(|s| s["action"] == "end_turn").count(),
        2
    );
    assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 2);
    assert_eq!(steps.iter().filter(|s| s["action"] == "load").count(), 2);
    for step in steps {
        if let Some(code) = step["code"].as_str() {
            assert!(!code.contains("F.Begin(") && !code.contains("JAC_Absorption_TestCall"));
            assert!(!code.contains("FLOW_TEST_MODE',1"));
        }
    }
    Ok(())
}

#[test]
fn ordinary_offer_waits_for_all_demands_relations_and_actual_envoys() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      GameEvents.JAC_Absorption_LeaderOffer.Call(OFFER)
      GameEvents.OnGameTurnStarted.Call()
      assert(BEGIN_COUNT==0)
      GOLD=100; GameEvents.OnGameTurnStarted.Call(); assert(BEGIN_COUNT==0)
      RELEASE=true; ENVOYS=1; GameEvents.OnGameTurnStarted.Call(); assert(BEGIN_COUNT==0)
      ENVOYS=2; GameEvents.OnGameTurnStarted.Call(); assert(BEGIN_COUNT==1)
      local receipt=N.Get('leader-offer',OFFER.ID)
      assert(receipt.Status=='STARTED' and receipt.Plan.Mode=='SMALL_STATE')
      assert(receipt.Plan.EntryConditionsMet and receipt.Plan.RelationsResolved)
      assert(receipt.Decision.OwnedTiles==2 and receipt.Decision.Envoys==2)
      GameEvents.OnGameTurnStarted.Call(); assert(BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn country_area_boundary_uses_merger_and_does_not_require_city_state_api() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      OFFER.AreaKm2=10000; GOLD=100; RELEASE=true
      Players[1].GetInfluence=nil; Map.GetPlotCount=nil
      N.DefineOffer(OFFER); N.Tick()
      local receipt=N.Get('leader-offer',OFFER.ID)
      assert(receipt.Status=='STARTED' and receipt.Plan.Mode=='MERGER' and BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn below_one_tile_still_needs_one_envoy_and_requirements_are_re_read() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      GOLD=100; RELEASE=true; TILES=0; ENVOYS=0
      N.DefineOffer(OFFER); N.Tick(); assert(BEGIN_COUNT==0)
      ENVOYS=1; GOLD=99; N.Tick(); assert(BEGIN_COUNT==0)
      GOLD=100; N.Tick(); assert(BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn unreviewed_relationships_wait_even_when_no_known_requirement_exists() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      OFFER.RelationRequirements={}; OFFER.RelationsReviewed=false
      GOLD=100; RELEASE=true; N.DefineOffer(OFFER); N.Tick()
      assert(BEGIN_COUNT==0 and N.Get('leader-offer',OFFER.ID).Status=='WAITING')
      OFFER.RelationsReviewed=true
      N.DefineOffer(OFFER); N.Tick(); assert(BEGIN_COUNT==1)
      OFFER.RelationsReviewed=false; assert(not pcall(N.DefineOffer,OFFER))
      assert(BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn reload_of_waiting_offer_registers_reader_again_and_reload_after_start_does_not_reenter(
) -> mlua::Result<()> {
    let lua = world()?;
    lua.load("N.DefineOffer(OFFER); N.Tick(); assert(BEGIN_COUNT==0)")
        .exec()?;
    load(&lua)?;
    lua.load("GOLD=100; RELEASE=true; N.Tick(); assert(BEGIN_COUNT==1)")
        .exec()?;
    load(&lua)?;
    lua.load("N.Tick(); N.Request('leader-offer',OFFER.ID,1,2); assert(BEGIN_COUNT==1)")
        .exec()
}

#[test]
fn interrupted_first_stage_resumes_frozen_plan_without_rechecking_or_replacing_terms(
) -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      GOLD=100; RELEASE=true; FAIL_BEGIN=true
      N.DefineOffer(OFFER); N.Tick()
      local receipt=N.Get('leader-offer',OFFER.ID)
      assert(receipt.Status=='STARTING' and receipt.LastError and BEGIN_COUNT==1)
      ORIGINAL_PLAN=JAC_AbsorptionGameplay.Rules.Copy(receipt.Plan)
      Players[1].IsAlive=function() return false end
      GOLD=0; RELEASE=false; FAIL_BEGIN=false
    "#,
    )
    .exec()?;
    load(&lua)?;
    lua.load(
        r#"
      N.Tick(); local receipt=N.Get('leader-offer',OFFER.ID)
      assert(receipt.Status=='STARTED' and BEGIN_COUNT==2)
      assert(JAC_AbsorptionGameplay.Rules.Equal(receipt.Plan,ORIGINAL_PLAN))
      N.Tick(); assert(BEGIN_COUNT==2)
    "#,
    )
    .exec()
}

#[test]
fn malformed_requirements_stale_decision_unknown_mode_and_reused_identity_do_not_start(
) -> mlua::Result<()> {
    let lua = world()?;
    lua.load(r#"
      for index,bad in ipairs({
        {Turn=6,Mode='MERGER',AreaKm2=10000},
        {Turn=7,Mode='FEDERATION',AreaKm2=10000},
        {Turn=7,Mode='SMALL_STATE',AreaKm2=10000},
        {Turn=7,Mode='MERGER',AreaKm2=0/0},
        {Turn=7,Mode='SMALL_STATE',AreaKm2=0}}) do
        bad.From=1;bad.To=2;bad.RelationsResolved=true
        bad.Requirements={{ID='promise',Met=true}}
        local source='invalid'..index
        N.Register(source,function() return bad end)
        assert(N.Request(source,'decision',1,2).LastError and BEGIN_COUNT==0)
      end
      N.Register('bad-demands',function() return {From=1,To=2,Turn=7,Mode='MERGER',
        AreaKm2=10000,RelationsResolved=true,Requirements={{ID='a',Met=1}}} end)
      N.Request('bad-demands','id',1,2); assert(BEGIN_COUNT==0)
      assert(not pcall(N.Request,'bad-demands','id',1,3))
      local zero=JAC_AbsorptionGameplay.Rules.Copy(OFFER);zero.AreaKm2=0
      assert(not pcall(N.DefineOffer,zero))
      N.DefineOffer(OFFER); OFFER.Requirements[1].Minimum=0
      assert(not pcall(N.DefineOffer,OFFER))
      assert(not pcall(N.Request,'unregistered','x',1,2))
      assert(not pcall(N.DefineOffer,{ID='x',From=1,To=2,AreaKm2=10,
        Requirements={{ID='a',Kind='GOLD_MINIMUM'},[3]={ID='b',Kind='GOLD_MINIMUM'}},RelationRequirements={}}))
    "#).exec()
}

#[test]
fn cancelled_offer_before_its_first_turn_stays_cancelled_after_reload_and_replay(
) -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      N.DefineOffer(OFFER)
      local cancelled=N.CancelOffer(OFFER.ID)
      assert(cancelled.Status=='CANCELLED' and cancelled.CancelledTurn==7)
      GOLD=100;RELEASE=true
      N.Tick();N.DefineOffer(OFFER)
      assert(N.Request('leader-offer',OFFER.ID,1,2).Status=='CANCELLED')
      assert(BEGIN_COUNT==0)
    "#,
    )
    .exec()?;
    load(&lua)?;
    lua.load(
        "N.Tick();assert(BEGIN_COUNT==0 and N.Get('leader-offer',OFFER.ID).Status=='CANCELLED')",
    )
    .exec()
}

#[test]
fn replacement_retires_old_terms_and_preserves_new_terms_across_reload_and_replay(
) -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      RELEASE=true;N.DefineOffer(OFFER);N.Tick()
      NEW=JAC_AbsorptionGameplay.Rules.Copy(OFFER)
      NEW.ID='leader-1:terms-2';NEW.Requirements[1].Minimum=200
      N.ReplaceOffer(OFFER.ID,NEW)
      assert(N.Get('leader-offer',OFFER.ID).Status=='CANCELLED')
      GOLD=100;N.Tick();assert(BEGIN_COUNT==0)
      assert(N.Get('leader-offer',NEW.ID).Status=='WAITING')
    "#,
    )
    .exec()?;
    load(&lua)?;
    lua.load(
        r#"
      N.Tick();assert(BEGIN_COUNT==0)
      N.ReplaceOffer(OFFER.ID,NEW)
      GOLD=200;N.Tick();assert(BEGIN_COUNT==1)
      local current=N.Get('leader-offer',NEW.ID)
      assert(current.Status=='STARTED' and current.Decision.Requirements[1].Met)
      NEW.RelationsReviewed=false
      local save=Game.SetProperty
      Game.SetProperty=function() error('a replacement replay must not write') end
      N.ReplaceOffer(OFFER.ID,NEW)
      Game.SetProperty=save
      assert(N.Get('leader-offer',OFFER.ID).ReplacedBy==NEW.ID)
      N.Request('leader-offer',OFFER.ID,1,2);N.Tick();assert(BEGIN_COUNT==1)
      NEW.Requirements[1].Minimum=201
      assert(not pcall(N.ReplaceOffer,OFFER.ID,NEW))
      NEW.ID='leader-1:terms-3'
      assert(not pcall(N.ReplaceOffer,OFFER.ID,NEW))
      assert(BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn frozen_first_stage_cannot_be_cancelled_or_replaced_and_resumes_its_original_plan(
) -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      GOLD=100;RELEASE=true;FAIL_BEGIN=true
      N.DefineOffer(OFFER);N.Tick()
      ORIGINAL=JAC_AbsorptionGameplay.Rules.Copy(N.Get('leader-offer',OFFER.ID).Plan)
      local newer=JAC_AbsorptionGameplay.Rules.Copy(OFFER);newer.ID='new-terms'
      assert(not pcall(N.Cancel,'leader-offer',OFFER.ID))
      assert(not pcall(N.CancelOffer,OFFER.ID))
      assert(not pcall(N.ReplaceOffer,OFFER.ID,newer))
      assert(not Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1').Offers[newer.ID])
      FAIL_BEGIN=false;GOLD=0;RELEASE=false
    "#,
    )
    .exec()?;
    load(&lua)?;
    lua.load(
        r#"
      N.Tick()
      local current=N.Get('leader-offer',OFFER.ID)
      assert(current.Status=='STARTED' and BEGIN_COUNT==2)
      assert(JAC_AbsorptionGameplay.Rules.Equal(current.Plan,ORIGINAL))
      assert(not pcall(N.CancelOffer,OFFER.ID))
      N.Tick();assert(BEGIN_COUNT==2)
    "#,
    )
    .exec()
}

#[test]
fn failed_replacement_save_does_not_retire_old_terms_or_publish_new_terms() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(r#"
      N.DefineOffer(OFFER);N.Tick()
      local before=Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1')
      local newer=JAC_AbsorptionGameplay.Rules.Copy(OFFER);newer.ID='new-terms'
      local save=Game.SetProperty
      Game.SetProperty=function() error('refused replacement save') end
      assert(not pcall(N.ReplaceOffer,OFFER.ID,newer))
      Game.SetProperty=save
      assert(JAC_AbsorptionGameplay.Rules.Equal(before,Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1')))
      assert(N.Get('leader-offer',OFFER.ID).Status=='WAITING' and BEGIN_COUNT==0)
      N.ReplaceOffer(OFFER.ID,newer)
      assert(N.Get('leader-offer',OFFER.ID).Status=='CANCELLED')
    "#).exec()
}

#[test]
fn registered_provider_cancellation_prevents_later_reader_changes_from_starting() -> mlua::Result<()>
{
    let lua = world()?;
    lua.load(
        r#"
      MET=false
      N.Register('another-provider',function() return {From=1,To=2,Turn=7,Mode='MERGER',
        AreaKm2=10000,RelationsResolved=true,Requirements={{ID='a',Met=MET}}} end)
      N.Request('another-provider','version-1',1,2)
      N.Cancel('another-provider','version-1');MET=true
      assert(N.Request('another-provider','version-1',1,2).Status=='CANCELLED')
      N.Tick();assert(BEGIN_COUNT==0)
    "#,
    )
    .exec()
}

#[test]
fn ineligible_influence_recipient_waits_even_with_a_nonzero_existing_envoy_count(
) -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      GOLD=100;RELEASE=true;CAN_RECEIVE=false;ENVOYS=100
      N.DefineOffer(OFFER);N.Tick()
      assert(N.Get('leader-offer',OFFER.ID).Status=='WAITING' and BEGIN_COUNT==0)
      CAN_RECEIVE=true;N.Tick();assert(BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn reader_cancellation_is_not_overwritten_by_the_outer_ready_decision() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      N.Register('reentrant-cancel',function()
        N.Request('reentrant-cancel','offer',1,2)
        N.Cancel('reentrant-cancel','offer')
        return {From=1,To=2,Turn=7,Mode='MERGER',AreaKm2=10000,RelationsResolved=true,
          Requirements={{ID='ready',Met=true}}}
      end)
      local receipt=N.Request('reentrant-cancel','offer',1,2)
      assert(receipt.Status=='CANCELLED' and not receipt.Plan and BEGIN_COUNT==0)
      N.Tick();assert(BEGIN_COUNT==0)
    "#,
    )
    .exec()
}

#[test]
fn relationship_review_changed_inside_a_requirement_reader_is_rechecked_next_turn(
) -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      GOLD=100;RELEASE=true;REVOKE=true
      OFFER.Requirements[#OFFER.Requirements+1]={ID='review-change',Kind='REVOKE_REVIEW'}
      N.RegisterRequirement('REVOKE_REVIEW',function()
        if REVOKE then OFFER.RelationsReviewed=false;N.DefineOffer(OFFER);REVOKE=false end
        return true
      end)
      N.DefineOffer(OFFER);N.Tick();assert(BEGIN_COUNT==0)
      assert(N.Get('leader-offer',OFFER.ID).Status=='WAITING')
      N.Tick();assert(BEGIN_COUNT==0)
      OFFER.RelationsReviewed=true;N.DefineOffer(OFFER);N.Tick();assert(BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn permanent_preflight_failure_can_be_cancelled_but_an_inflight_retry_cannot() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      GOLD=100;RELEASE=true
      local begin=JAC_AbsorptionFlowGameplay.Begin
      JAC_AbsorptionFlowGameplay.Begin=function(plan) error('unsupported inventory permanently') end
      N.DefineOffer(OFFER);N.Tick()
      assert(N.Get('leader-offer',OFFER.ID).Status=='STARTING')
      assert(not Game:GetProperty('JAC_ABSORPTION_FLOW_V1'))
      JAC_AbsorptionFlowGameplay.Begin=function(plan)
        assert(not pcall(N.CancelOffer,OFFER.ID),'inflight intent was cancelled')
        error('unsupported inventory permanently')
      end
      N.Tick()
      N.CancelOffer(OFFER.ID)
      assert(N.Get('leader-offer',OFFER.ID).Status=='CANCELLED')
      JAC_AbsorptionFlowGameplay.Begin=begin
      N.Tick();assert(BEGIN_COUNT==0)
      local replacement=JAC_AbsorptionGameplay.Rules.Copy(OFFER)
      replacement.ID='corrected-terms'
      N.ReplaceOffer(OFFER.ID,replacement);N.Tick();assert(BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn failure_before_flow_journal_rechecks_live_conditions_before_starting() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      GOLD=100;RELEASE=true;FAIL_BEFORE_JOURNAL=true
      local begin=JAC_AbsorptionFlowGameplay.Begin
      JAC_AbsorptionFlowGameplay.Begin=function(plan)
        if FAIL_BEFORE_JOURNAL then error('inventory snapshot failed before journalling') end
        return begin(plan)
      end
      OFFER.Plan.Label='original-intent'
      N.DefineOffer(OFFER);N.Tick()
      local receipt=N.Get('leader-offer',OFFER.ID)
      assert(receipt.Status=='STARTING' and receipt.Plan.Label=='original-intent')
      assert(not Game:GetProperty('JAC_ABSORPTION_FLOW_V1') and BEGIN_COUNT==0)
      ORIGINAL=JAC_AbsorptionGameplay.Rules.Copy(receipt.Plan)
      GOLD=0;RELEASE=false;FAIL_BEFORE_JOURNAL=false
    "#,
    )
    .exec()?;
    load(&lua)?;
    lua.load(
        r#"
      N.Tick()
      local receipt=N.Get('leader-offer',OFFER.ID)
      assert(receipt.Status=='WAITING' and not receipt.Plan and BEGIN_COUNT==0)
      GOLD=100;RELEASE=true;N.Tick()
      receipt=N.Get('leader-offer',OFFER.ID)
      assert(receipt.Status=='STARTED' and BEGIN_COUNT==1)
      assert(JAC_AbsorptionGameplay.Rules.Equal(receipt.Plan,ORIGINAL))
      N.Tick();assert(BEGIN_COUNT==1)
    "#,
    )
    .exec()
}

#[test]
fn an_unjournaled_intent_releases_its_reservation_when_its_provider_or_source_is_gone(
) -> mlua::Result<()> {
    for missing_provider in [true, false] {
        let lua = world()?;
        lua.load(r#"
          N.Register('external',function() return {From=1,To=2,Turn=TURN,Mode='MERGER',AreaKm2=10000,
            Requirements={{ID='live',Met=true}},RelationsResolved=true} end)
          JAC_AbsorptionFlowGameplay.Begin=function() error('failed before the flow journal') end
          N.Request('external','intent',1,2)
          local saved=Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1')
          for _,request in pairs(saved.Requests) do request.LastError=nil end
          Game:SetProperty('JAC_ABSORPTION_ADMISSION_V1',saved)
          assert(N.IsOccupied(1) and N.IsOccupied(2))
        "#).exec()?;
        if missing_provider {
            load(&lua)?;
        } else {
            lua.load("Players[1].IsAlive=function() return false end")
                .exec()?;
        }
        lua.load(
            r#"
          N.Tick();local request=N.Get('external','intent')
          assert((request.Status=='WAITING' or request.Status=='UNAVAILABLE') and not request.Plan)
          assert(not N.IsOccupied(1) and not N.IsOccupied(2) and BEGIN_COUNT==0)
          N.Cancel('external','intent');assert(N.Get('external','intent').Status=='CANCELLED')
        "#,
        )
        .exec()?;
    }
    Ok(())
}

#[test]
fn busy_participants_clear_legacy_unstarted_plans_and_reread_after_release() -> mlua::Result<()> {
    for (other_from, other_to) in [(1, 3), (3, 2)] {
        let lua = world()?;
        lua.globals().set("OTHER_FROM", other_from)?;
        lua.globals().set("OTHER_TO", other_to)?;
        lua.load(
            r#"
          GOLD=100;RELEASE=true
          FLOWS.other={ID='other',From=OTHER_FROM,To=OTHER_TO,Phase='CHOICE',Phase1Complete=false}
          Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
          N.DefineOffer(OFFER);local waiting=N.Request('leader-offer',OFFER.ID,1,2)
          assert(waiting.Status=='WAITING' and not waiting.Plan and BEGIN_COUNT==0)
          local saved=Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1')
          for _,request in pairs(saved.Requests) do
            request.Status='STARTING';request.LastError='legacy conflict'
            request.Plan={ID=request.FlowID,From=1,To=2,Mode='SMALL_STATE',Label='stale'}
          end
          Game:SetProperty('JAC_ABSORPTION_ADMISSION_V1',saved);N.Tick()
          assert(not N.Get('leader-offer',OFFER.ID).Plan and BEGIN_COUNT==0)
          GOLD=0;RELEASE=false;FLOWS.other.Phase='DONE'
          Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS});N.Tick()
          assert(N.Get('leader-offer',OFFER.ID).Status=='WAITING' and BEGIN_COUNT==0)
          GOLD=100;RELEASE=true;N.Tick()
          local current=N.Get('leader-offer',OFFER.ID)
          assert(current.Status=='STARTED' and current.Plan.Label~='stale' and BEGIN_COUNT==1)
        "#,
        )
        .exec()?;
    }
    Ok(())
}

#[test]
fn starting_reservation_blocks_a_reentrant_request_before_the_flow_journal() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(r#"
      N.Register('competitor',function(_,from,to) return {From=from,To=to,Turn=TURN,
        Mode='MERGER',AreaKm2=10000,Requirements={{ID='live',Met=RELEASE}},RelationsResolved=true} end)
      local begin=JAC_AbsorptionFlowGameplay.Begin
      JAC_AbsorptionFlowGameplay.Begin=function(plan)
        assert(N.IsOccupied(1) and N.IsOccupied(2))
        local same=N.Request('leader-offer',OFFER.ID,1,2)
        assert(same.Status=='STARTING' and same.Plan.ID==plan.ID and BEGIN_COUNT==0)
        local peer=N.Request('competitor','peer',1,3)
        assert(peer.Status=='WAITING' and not peer.Plan and BEGIN_COUNT==0)
        return begin(plan)
      end
      GOLD=100;RELEASE=true;N.DefineOffer(OFFER);N.Tick()
      assert(BEGIN_COUNT==1 and not N.Get('competitor','peer').Plan)
      JAC_AbsorptionFlowGameplay.Begin=begin
      for _,flow in pairs(FLOWS) do flow.Phase='DONE' end
      Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
      RELEASE=false;N.Tick();assert(BEGIN_COUNT==1)
      RELEASE=true;N.Tick();assert(BEGIN_COUNT==2)
    "#).exec()
}

#[test]
fn legacy_waiting_save_is_read_without_writing_and_lifecycle_updates_upgrade_its_version(
) -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
      N.DefineOffer(OFFER);N.Tick()
      local saved=Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1')
      saved.Version=1;Game:SetProperty('JAC_ABSORPTION_ADMISSION_V1',saved)
      local save=Game.SetProperty
      Game.SetProperty=function() error('legacy read must not save') end
      assert(N.Get('leader-offer',OFFER.ID).Status=='WAITING')
      N.Tick()
      Game.SetProperty=save
      assert(Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1').Version==1)
      N.CancelOffer(OFFER.ID)
      assert(Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1').Version==2)
      assert(N.Get('leader-offer',OFFER.ID).Status=='CANCELLED' and BEGIN_COUNT==0)
    "#,
    )
    .exec()
}
