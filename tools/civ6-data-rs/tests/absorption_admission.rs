use civ6_data_rs::{absorption_admission, Repository};
use mlua::Lua;

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
        JAC_AbsorptionGameplay={Rules={Copy=copy, Equal=equal, Keys=keys}}
        TURN, GOLD, FAITH, ENVOYS, TILES, BEGIN_COUNT, FAIL_BEGIN = 7, 99, 10, 2, 2, 0, false
        local props={}
        Game={GetProperty=function(_,k) return copy(props[k]) end,
          SetProperty=function(_,k,v) props[k]=copy(v) end,
          GetCurrentGameTurn=function() return TURN end}
        Players={}
        for id=1,3 do
          Players[id]={IsAlive=function() return true end,IsHuman=function() return id==2 end,
            GetTreasury=function() return {GetGoldBalance=function() return GOLD end} end,
            GetReligion=function() return {GetFaithBalance=function() return FAITH end} end,
            GetInfluence=function() return {GetTokensReceived=function(_,sender)
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
        JAC_AbsorptionFlowGameplay={GetRequest=function(id) return FLOWS[id] and copy(FLOWS[id].Request) end,
          Begin=function(plan)
            BEGIN_COUNT=BEGIN_COUNT+1
            FLOWS[plan.ID]=FLOWS[plan.ID] or {ID=plan.ID, Request=copy(plan)}
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
      OFFER.ID='reviewed-terms'; OFFER.RelationsReviewed=true
      N.DefineOffer(OFFER); N.Tick(); assert(BEGIN_COUNT==1)
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
        {Turn=7,Mode='MERGER',AreaKm2=0/0}}) do
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
      N.DefineOffer(OFFER); OFFER.Requirements[1].Minimum=0
      assert(not pcall(N.DefineOffer,OFFER))
      assert(not pcall(N.Request,'unregistered','x',1,2))
      assert(not pcall(N.DefineOffer,{ID='x',From=1,To=2,AreaKm2=10,
        Requirements={{ID='a',Kind='GOLD_MINIMUM'},[3]={ID='b',Kind='GOLD_MINIMUM'}},RelationRequirements={}}))
    "#).exec()
}
