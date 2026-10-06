use civ6_data_rs::{absorption_admission, absorption_product as product, Repository};
use mlua::Lua;

const WORLD: &str = r#"
local function copy(v) if type(v)~='table' then return v end local o={} for k,x in pairs(v) do o[k]=copy(x) end return o end
local function equal(a,b) if type(a)~=type(b) then return false end if type(a)~='table' then return a==b end
  for k,v in pairs(a) do if not equal(v,b[k]) then return false end end for k in pairs(b) do if a[k]==nil then return false end end return true end
local function keys(t) local o={} for k in pairs(t) do o[#o+1]=k end table.sort(o) return o end
local function event() local hs={} return {Add=function(fn) hs[#hs+1]=fn end,Call=function(...) for _,fn in ipairs(hs) do fn(...) end end} end
TURN,GOLD,FAITH,POOL,TILES,COMBAT,PEAK,SCIENCE,POP,ACCESS=7,{[1]=100,[2]=5000},{[1]=20,[2]=1000},10,2,1,10,1,1,0
HUMAN,ALIVE,ALLIES,STATES,OFFICIAL,OFFICIAL_ENVOYS={false,true,false},{true,true,false},{},{},false,0
FAIL_CHARGE,FAIL_REFUND,FAIL_ENVOY,FAIL_BEGIN=false,false,false,false
CHARGES,REFUNDS,SENDS,STARTS=0,0,0,0
local props={}
Game={GetProperty=function(_,k) return copy(props[k]) end,SetProperty=function(_,k,v) props[k]=copy(v) end,GetCurrentGameTurn=function() return TURN end}
Players,PlayerConfigurations={},{}
for actor=1,3 do local id=actor
  Players[id]={IsAlive=function() return ALIVE[id] end,IsHuman=function() return HUMAN[id] end,IsTurnActive=function() return true end,
    GetProperty=function(_,k) return copy(props[id..':'..k]) end,SetProperty=function(_,k,v) props[id..':'..k]=copy(v) end,
    GetTreasury=function() return {GetGoldBalance=function() return GOLD[id] or 0 end,ChangeGoldBalance=function(_,n)
      GOLD[id]=GOLD[id]+n;if n<0 then CHARGES=CHARGES+1;if FAIL_CHARGE then FAIL_CHARGE=false;error('charge mutated then threw') end
      else REFUNDS=REFUNDS+1;if FAIL_REFUND then FAIL_REFUND=false;error('refund mutated then threw') end end end} end,
    GetReligion=function() return {GetFaithBalance=function() return FAITH[id] or 0 end,ChangeFaithBalance=function(_,n) FAITH[id]=FAITH[id]+n end,GetFaithYield=function() return SCIENCE end} end,
    GetTechs=function() return {GetScienceYield=function() return SCIENCE end,HasTech=function() return false end} end,
    GetUnits=function() return {Members=function() local n=0;return function() n=n+1;if n<=(id==1 and 1 or COMBAT) then return n,{GetType=function() return id==1 and 0 or 1 end} end end end} end,
    GetCities=function() return {Members=function() local done=false;return function() if done then return end;done=true;return 1,{GetPopulation=function() return id==1 and 1 or POP end,GetYield=function() return SCIENCE end} end end} end,
    GetDiplomacy=function() return {HasMet=function() return true end,IsAtWarWith=function(_,other) return STATES['war:'..id..':'..other]==true end,
      IsAlliedWith=function(_,other) return ALLIES[id..':'..other]==true end,GetVisibilityOn=function() return id==1 and 1 or ACCESS end} end,
    GetDiplomaticAI=function() return {GetDiplomaticStateIndex=function() return STATES[id] or 0 end} end,
    GetInfluence=function() return {CanReceiveInfluence=function() return id==1 and OFFICIAL end,GetTokensReceived=function(_,sender) assert(id==1 and sender==2);return OFFICIAL_ENVOYS end,
      GetTokensToGive=function() return POOL end,ChangeTokensToGive=function(_,n) POOL=POOL+n;SENDS=SENDS+1;if FAIL_ENVOY then FAIL_ENVOY=false;error('envoy spent then threw') end end} end}
end
PlayerConfigurations[1]={GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_ROKKAKU' end,GetLeaderTypeName=function() return 'LEADER_JAC_ROKKAKU_SADAYORI' end}
PlayerConfigurations[2]={GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_ODA' end,GetLeaderTypeName=function() return 'LEADER_JAC_ODA_NOBUHIDE' end}
PlayerConfigurations[3]=PlayerConfigurations[2]
Map={GetPlotCount=function() return TILES end,GetPlotByIndex=function() return {GetOwner=function() return 1 end} end}
YieldTypes={FOOD=0}
GameInfo={Units={[0]={Combat=10},[1]={Combat=20}},Technologies=function() return function() end end,
  DiplomaticStates={[0]={StateType='DIPLO_STATE_NEUTRAL'},[1]={StateType='DIPLO_STATE_FRIENDLY'},[2]={StateType='DIPLO_STATE_DENOUNCED'}}}
GameEvents={JAC_Absorption_AdmissionRequested=event(),JAC_Absorption_LeaderOffer=event(),OnGameTurnStarted=event(),JAC_AbsorptionProduct_Operation=event()}
function RESET_EVENTS() for key in pairs(GameEvents) do GameEvents[key]=event() end end
ExposedMembers={}
JAC_AbsorptionGameplay={Rules={Copy=copy,Equal=equal,Keys=keys},GetRoster=function() return {} end}
FLOWS={}
JAC_AbsorptionFlowGameplay={Begin=function(plan)
  STARTS=STARTS+1;FLOWS[plan.ID]=FLOWS[plan.ID] or {ID=plan.ID,From=plan.From,To=plan.To,Request=copy(plan),Phase='CHOICE',Phase1Complete=true}
  Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
  if FAIL_BEGIN then FAIL_BEGIN=false;error('P1 interrupted') end
  return copy(FLOWS[plan.ID]) end,Get=function(id) return copy(FLOWS[id]) end}
function READY() COMBAT=10;ACCESS=4;POP=10;SCIENCE=100 end
function CMD(command,nonce,extras) local p=extras or {};p.Command=command;p.Nonce=nonce;p.From=p.From or 1
  return JAC_AbsorptionProductGameplay.Command(2,p) end
function REGISTER() return CMD('REGISTER','register') end
function ENVOY(n) return CMD('SEND_ENVOY','envoy'..n,{ExpectedEnvoys=n}) end
function VIEW() return ExposedMembers.JAC_AbsorptionProduct.Get(1,2) end
"#;

fn load(lua: &Lua, area: f64) -> anyhow::Result<()> {
    lua.load("RESET_EVENTS()").exec()?;
    lua.load(absorption_admission::lua()).exec()?;
    lua.load("N=JAC_AbsorptionAdmissionGameplay").exec()?;
    let mut catalog = product::catalog(&Repository::checked_in())?;
    catalog.runtime["Countries"]["ROKKAKU"]["AreaKm2"] = serde_json::json!(area);
    lua.load(product::render_gameplay(&catalog)).exec()?;
    Ok(())
}
fn world(area: f64) -> anyhow::Result<Lua> {
    let lua = Lua::new();
    lua.load(WORLD).exec()?;
    load(&lua, area)?;
    Ok(lua)
}

#[test]
fn typed_catalog_maps_every_current_product_leader_and_all_japanese_demand_types(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let catalog = product::catalog(&repo)?;
    assert_eq!(catalog.leaders.len(), 24);
    let sql = format!(
        "{}{}",
        repo.read("poc/JAC_PoC_CustomCivs/Data/JAC_CustomCivs_Gameplay.sql")?,
        repo.read("poc/JAC_PoC_CustomCivs/Data/JAC_Japan1534_Gameplay.sql")?
    );
    for l in catalog
        .leaders
        .iter()
        .filter(|l| l.civilization.starts_with("CIVILIZATION_JAC_"))
    {
        assert!(sql.contains(&format!("'{}'", l.leader)));
        assert!(sql.contains(&format!("'{}'", l.civilization)));
        assert!(l.area_km2 > 0.);
    }
    assert_eq!(catalog.runtime["Traits"]["教者"]["metric"], "POPULATION");
    assert_eq!(catalog.runtime["Traits"]["霊神"]["metric"], "FAITH_YIELD");
    assert_eq!(
        catalog.runtime["Countries"]["SINGAPORE"]["RelationsReviewed"],
        false
    );
    for (path, expected) in product::generate(&repo)? {
        assert_eq!(repo.read(&path)?, expected, "stale {}", path.display());
    }
    Ok(())
}
#[test]
fn small_major_consumes_real_envoys_and_waits_for_every_product_demand() -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(r#"
      assert(VIEW().Mode=='SMALL_STATE' and VIEW().Envoys.Route=='PRODUCT')
      assert(not Players[1]:GetInfluence():CanReceiveInfluence())
      REGISTER();GameEvents.OnGameTurnStarted.Call();assert(STARTS==0 and CHARGES==0)
      READY();GameEvents.OnGameTurnStarted.Call();assert(STARTS==0 and CHARGES==0)
      ENVOY(0);assert(POOL==9 and VIEW().Envoys.Envoys==1)
      GameEvents.OnGameTurnStarted.Call();assert(STARTS==0)
      ENVOY(1);assert(POOL==8)
      GameEvents.OnGameTurnStarted.Call();assert(STARTS==1 and CHARGES==1 and GOLD[2]<5000 and FAITH[2]<1000)
      assert(VIEW().Status=='STARTED');GameEvents.OnGameTurnStarted.Call();assert(STARTS==1 and CHARGES==1)
    "#).exec()?;
    Ok(())
}
#[test]
fn real_area_boundary_and_current_owned_tiles_are_independent() -> anyhow::Result<()> {
    for (area, expected) in [(9999., "SMALL_STATE"), (10000., "MERGER")] {
        let lua = world(area)?;
        lua.globals().set("EXPECTED", expected)?;
        lua.load("READY();TILES=9;REGISTER();GameEvents.OnGameTurnStarted.Call();assert(VIEW().Mode==EXPECTED)") .exec()?;
        let starts: i64 = lua.globals().get("STARTS")?;
        assert_eq!(starts, if area < 10000. { 0 } else { 1 });
        if area < 10000. {
            lua.load("TILES=0;ENVOY(0);GameEvents.OnGameTurnStarted.Call();assert(STARTS==1 and VIEW().Envoys.Required==1)").exec()?;
        }
    }
    Ok(())
}
#[test]
fn actor_forgery_and_ui_conditions_are_rejected_without_money_or_registration() -> anyhow::Result<()>
{
    let lua = world(9999.)?;
    lua.load(r#"
      GameEvents.JAC_AbsorptionProduct_Operation.Call(2,{Command='REGISTER',Nonce='fake',From=1,To=3})
      assert(not Players[2]:GetProperty('JAC_ABSORPTION_PRODUCT_LAST_OPERATION_V1').Accepted)
      for _,field in ipairs({'AreaKm2','Requirements','RelationsReviewed','Met','Deposit','Plan'}) do
        local p={Command='REGISTER',From=1,Nonce=field};p[field]=true
        GameEvents.JAC_AbsorptionProduct_Operation.Call(2,p)
        assert(not Players[2]:GetProperty('JAC_ABSORPTION_PRODUCT_LAST_OPERATION_V1').Accepted)
      end
      assert(STARTS==0 and CHARGES==0 and VIEW().Status=='AVAILABLE')
    "#).exec()?;
    Ok(())
}
#[test]
fn unreviewed_relations_and_active_federation_block_even_with_money_and_metrics(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(r#"
      READY();Game:SetProperty('JAC_COUNTRY_RELATIONS_V1',{Version=1,Countries={ROKKAKU={Reviewed=false}},Relations={}})
      REGISTER();GameEvents.OnGameTurnStarted.Call();assert(STARTS==0 and CHARGES==0)
      assert(VIEW().Rows[#VIEW().Rows].Reviewed==false)
      Game:SetProperty('JAC_COUNTRY_RELATIONS_V1',{Version=1,Countries={ROKKAKU={Reviewed=true}},Relations={{ID='federation',From='ROKKAKU',To='ODA',Kind='連邦',Active=true}}})
      GameEvents.OnGameTurnStarted.Call();assert(STARTS==0 and CHARGES==0)
      Game:SetProperty('JAC_COUNTRY_RELATIONS_V1',{Version=1,Countries={ROKKAKU={Reviewed=true}},Relations={{ID='federation',From='ROKKAKU',To='ODA',Kind='連邦',Active=false}}})
      GameEvents.OnGameTurnStarted.Call();assert(STARTS==1 and CHARGES==1)
    "#).exec()?;
    Ok(())
}
#[test]
fn deposit_is_actual_settlement_not_stock_and_interrupted_charge_resumes_once() -> anyhow::Result<()>
{
    let lua = world(10000.)?;
    lua.load(
        r#"
      REGISTER();READY();GOLD[2]=1000;GameEvents.OnGameTurnStarted.Call()
      assert(STARTS==0 and CHARGES==0,'prudent deposit must leave its separate reserve')
      GOLD[2]=5000;FAIL_CHARGE=true;GameEvents.OnGameTurnStarted.Call()
      assert(STARTS==0 and CHARGES==1 and GOLD[2]<5000)
    "#,
    )
    .exec()?;
    load(&lua, 10000.)?;
    lua.load("GameEvents.OnGameTurnStarted.Call();assert(STARTS==1 and CHARGES==1);REGISTER();assert(CHARGES==1 and STARTS==1)").exec()?;
    Ok(())
}
#[test]
fn interrupted_envoy_consumption_resumes_after_reload_and_replay_is_idempotent(
) -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(
        "FAIL_ENVOY=true;assert(not pcall(ENVOY,0));assert(POOL==9 and VIEW().Envoys.Envoys==0)",
    )
    .exec()?;
    load(&lua, 9999.)?;
    lua.load("ENVOY(0);ENVOY(0);assert(POOL==9 and SENDS==1 and VIEW().Envoys.Envoys==1);assert(not pcall(CMD,'SEND_ENVOY','envoy0',{ExpectedEnvoys=1}));assert(POOL==9)").exec()?;
    Ok(())
}
#[test]
fn frozen_plan_keeps_paid_deposit_and_single_phase_one_on_reload() -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load("REGISTER();READY();FAIL_BEGIN=true;GameEvents.OnGameTurnStarted.Call();assert(CHARGES==1 and STARTS>=1);BEFORE=VIEW().OfferID;STATES[1]=2").exec()?;
    load(&lua, 10000.)?;
    lua.load("GameEvents.OnGameTurnStarted.Call();assert(VIEW().OfferID==BEFORE and CHARGES==1);assert(not pcall(CMD,'CANCEL','cancel',{OfferID=BEFORE}))").exec()?;
    Ok(())
}
#[test]
fn human_and_ai_recipients_use_identical_demands_envoy_cost_and_deposit() -> anyhow::Result<()> {
    let human = world(9999.)?;
    let ai = world(9999.)?;
    human
        .load("REGISTER();READY();ENVOY(0);ENVOY(1);GameEvents.OnGameTurnStarted.Call()")
        .exec()?;
    ai.load(
        "HUMAN[2]=false;REGISTER();READY();ENVOY(0);ENVOY(1);GameEvents.OnGameTurnStarted.Call()",
    )
    .exec()?;
    for name in ["POOL", "GOLD", "FAITH", "CHARGES", "STARTS"] {
        let a: String = human
            .load(format!(
                "return tostring({name}==GOLD and GOLD[2] or {name}==FAITH and FAITH[2] or {name})"
            ))
            .eval()?;
        let b: String = ai
            .load(format!(
                "return tostring({name}==GOLD and GOLD[2] or {name}==FAITH and FAITH[2] or {name})"
            ))
            .eval()?;
        assert_eq!(a, b, "{name}");
    }
    Ok(())
}

#[test]
fn cancellation_refunds_partly_collected_deposit_and_reload_does_not_refund_twice(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(
        r#"
      REGISTER();READY();FAIL_CHARGE=true;GameEvents.OnGameTurnStarted.Call()
      assert(CHARGES==1 and STARTS==0);local id=VIEW().OfferID
      FAIL_REFUND=true;assert(not pcall(CMD,'CANCEL','cancel',{OfferID=id}))
      assert(GOLD[2]==5000 and REFUNDS==1)
    "#,
    )
    .exec()?;
    load(&lua, 10000.)?;
    lua.load(
        r#"
      GameEvents.OnGameTurnStarted.Call();assert(VIEW().Status=='CANCELLED')
      assert(GOLD[2]==5000 and FAITH[2]==1000 and REFUNDS==1)
      GameEvents.OnGameTurnStarted.Call();assert(STARTS==0 and REFUNDS==1)
    "#,
    )
    .exec()?;
    Ok(())
}
#[test]
fn diplomatic_term_replacement_is_versioned_atomic_and_does_not_accept_old_identity(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(r#"
      REGISTER();OLD=VIEW().OfferID
      local before=N.GetOffer(OLD).Requirements[1].Minimum
      STATES[1]=2;GameEvents.OnGameTurnStarted.Call()
      NEW=VIEW().OfferID;assert(OLD~=NEW)
      assert(N.Get('leader-offer',OLD).Status=='CANCELLED' and N.Get('leader-offer',OLD).ReplacedBy==NEW)
      assert(N.GetOffer(NEW).Requirements[1].Minimum>before and CHARGES==0 and STARTS==0)
      assert(not pcall(CMD,'CANCEL','old-cancel',{OfferID=OLD}))
    "#).exec()?;
    load(&lua, 10000.)?;
    lua.load("GameEvents.OnGameTurnStarted.Call();assert(VIEW().OfferID==NEW and CHARGES==0)")
        .exec()?;
    Ok(())
}
#[test]
fn verified_result_refund_recovers_engine_failure_without_second_payment() -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(
        r#"
      REGISTER();READY();GameEvents.OnGameTurnStarted.Call();assert(STARTS==1 and CHARGES==1)
      local id=N.Get('leader-offer',VIEW().OfferID).FlowID;FLOWS[id].Phase='RESULT'
      FAIL_REFUND=true;GameEvents.OnGameTurnStarted.Call();assert(GOLD[2]==5000 and REFUNDS==1)
    "#,
    )
    .exec()?;
    load(&lua, 10000.)?;
    lua.load("GameEvents.OnGameTurnStarted.Call();assert(GOLD[2]==5000 and FAITH[2]==1000 and REFUNDS==1);REGISTER();assert(STARTS==1 and CHARGES==1)").exec()?;
    Ok(())
}
#[test]
fn official_minor_uses_received_tokens_without_product_assignments() -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(
        r#"
      OFFICIAL=true;READY();REGISTER();assert(VIEW().Envoys.Route=='OFFICIAL')
      assert(not pcall(ENVOY,0) and POOL==10 and SENDS==0)
      OFFICIAL_ENVOYS=1;GameEvents.OnGameTurnStarted.Call();assert(STARTS==0)
      OFFICIAL_ENVOYS=2;GameEvents.OnGameTurnStarted.Call();assert(STARTS==1 and SENDS==0)
    "#,
    )
    .exec()?;
    Ok(())
}
#[test]
fn published_views_are_copies_and_character_trait_and_actor_mapping_are_authoritative(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(
        r#"
      local v=VIEW();v.Country.AreaKm2=1;v.Country.Profile.Trait='霊神';v.Rows[1].Minimum=0
      assert(VIEW().Country.AreaKm2==10000 and VIEW().Country.Profile.Trait=='武人')
      PlayerConfigurations[1].GetLeaderTypeName=function() return 'LEADER_JAC_TAKEDA_NOBUTORA' end
      assert(VIEW().Status=='UNAVAILABLE' and not pcall(REGISTER) and CHARGES==0)
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn cancelled_partial_payment_does_not_charge_an_unpaid_currency_and_ambiguous_charge_blocks(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(
        r#"
      REGISTER();READY();FAIL_CHARGE=true;GameEvents.OnGameTurnStarted.Call()
      assert(STARTS==0 and FAITH[2]==1000)
      FAITH[2]=0
      local id=VIEW().OfferID;CMD('CANCEL','cancel-partial',{OfferID=id})
      assert(GOLD[2]==5000 and FAITH[2]==0 and VIEW().Status=='CANCELLED')
    "#,
    )
    .exec()?;
    let lua = world(10000.)?;
    lua.load(
        r#"
      REGISTER();READY();FAIL_CHARGE=true;GameEvents.OnGameTurnStarted.Call()
      GOLD[2]=GOLD[2]+1
      GameEvents.OnGameTurnStarted.Call()
      assert(STARTS==0 and CHARGES==1 and VIEW().Error:find('ambiguous',1,true))
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn authoritative_ai_result_remains_readable_after_source_elimination() -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(r#"
      REGISTER();READY();GameEvents.OnGameTurnStarted.Call();assert(STARTS==1)
      ALIVE[1]=false
      local v=VIEW();assert(v.Status=='STARTED' and v.Frozen and v.Country.Profile.Personality=='慎重')
      assert(v.Rows[1].Frozen and v.CanCancel==false)
      REGISTER();assert(CHARGES==1 and STARTS==1)
    "#).exec()?;
    Ok(())
}

#[test]
fn preflight_failure_without_a_flow_can_be_cancelled_after_the_source_is_eliminated(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(r#"
      JAC_AbsorptionFlowGameplay.Begin=function() error('inventory unavailable before flow journal') end
      REGISTER();READY();GameEvents.OnGameTurnStarted.Call()
      assert(CHARGES==1 and VIEW().CanCancel and N.Get('leader-offer',VIEW().OfferID).LastError)
      ALIVE[1]=false
      local id=VIEW().OfferID;CMD('CANCEL','cancel-unavailable',{OfferID=id})
      assert(N.Get('leader-offer',id).Status=='CANCELLED' and GOLD[2]==5000 and FAITH[2]==1000)
      CMD('CANCEL','cancel-unavailable',{OfferID=id});assert(REFUNDS==1)
    "#).exec()?;
    Ok(())
}

#[test]
fn four_product_scenes_use_real_actors_and_approved_screen_handlers_without_diagnostic_demands(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = Lua::new();
    for small in [true, false] {
        for human in [true, false] {
            let value: serde_json::Value =
                serde_json::from_str(&product::acceptance_scenario(&repo, small, human)?)?;
            let steps = value["steps"].as_array().unwrap();
            assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 2);
            assert_eq!(steps.iter().filter(|s| s["action"] == "load").count(), 2);
            assert!(value.get("test_mode").is_none());
            for step in steps {
                if let Some(code) = step["code"]
                    .as_str()
                    .or_else(|| step["condition"]["code"].as_str())
                {
                    lua.load(code).into_function()?;
                    for forbidden in [
                        "DefineOffer(",
                        "F.Begin(",
                        "JAC_Absorption_TestCall",
                        "RelationsReviewed=",
                        "FLOW_TEST_MODE",
                    ] {
                        assert!(
                            !code.contains(forbidden),
                            "scene bypassed product scope: {forbidden}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn unchanged_product_waiting_turns_do_not_rewrite_the_saved_ledger() -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(
        r#"
      REGISTER();assert(VIEW().Status=='WAITING')
      local write=Game.SetProperty;PRODUCT_WRITES=0
      Game.SetProperty=function(self,key,value)
        if key=='JAC_ABSORPTION_PRODUCT_V1' then PRODUCT_WRITES=PRODUCT_WRITES+1 end
        return write(self,key,value)
      end
      for i=1,3 do GameEvents.OnGameTurnStarted.Call() end
      assert(PRODUCT_WRITES==0 and VIEW().Status=='WAITING' and CHARGES==0 and STARTS==0)
      READY();GameEvents.OnGameTurnStarted.Call()
      assert(PRODUCT_WRITES>0 and CHARGES==1 and STARTS==1)
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn cancelled_preflight_view_does_not_claim_that_its_refunded_deposit_is_held() -> anyhow::Result<()>
{
    let lua = world(10000.)?;
    lua.load(r#"
      JAC_AbsorptionFlowGameplay.Begin=function() error('preflight before journal') end
      REGISTER();READY();GameEvents.OnGameTurnStarted.Call()
      local old=VIEW().OfferID;CMD('CANCEL','cancel-view',{OfferID=old})
      local view=VIEW();assert(view.Status=='CANCELLED' and not view.CanCancel and not view.Frozen)
      for _,row in ipairs(view.Rows) do if row.Kind=='PRODUCT_DEPOSIT' then assert(not row.Met) end end
      assert(GOLD[2]==5000 and FAITH[2]==1000)
      CMD('REGISTER','register-again');assert(VIEW().OfferID~=old and VIEW().Status=='WAITING')
    "#).exec()?;
    Ok(())
}

#[test]
fn eliminated_source_releases_a_failed_preflight_deposit_once_on_the_normal_turn(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(
        r#"
      JAC_AbsorptionFlowGameplay.Begin=function() error('preflight before journal') end
      REGISTER();READY();GameEvents.OnGameTurnStarted.Call()
      local id=VIEW().OfferID;assert(CHARGES==1 and VIEW().CanCancel)
      ALIVE[1]=false;GameEvents.OnGameTurnStarted.Call()
      assert(N.Get('leader-offer',id).Status=='CANCELLED' and GOLD[2]==5000 and FAITH[2]==1000)
      assert(REFUNDS==1)
      GameEvents.OnGameTurnStarted.Call();assert(REFUNDS==1 and CHARGES==1)
    "#,
    )
    .exec()?;
    Ok(())
}
