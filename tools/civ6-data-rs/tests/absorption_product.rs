use civ6_data_rs::{absorption_admission, absorption_product as product, Repository};
use mlua::Lua;

#[test]
fn terminal_wait_records_changed_pending_state_without_advancing_transfer() -> anyhow::Result<()> {
    let lua = Lua::new();
    lua.load(format!(r#"
local function canonical(v)
  if type(v)~='table' then return tostring(v) end
  local keys={{}};for k in pairs(v) do keys[#keys+1]=k end;table.sort(keys)
  local out={{}};for _,k in ipairs(keys) do out[#out+1]=k..'='..canonical(v[k]) end
  return '{{'..table.concat(out,',')..'}}'
end
local f={{From=1,To=2,Phase='APPLYING',WaitingFor='WAITING_UI'}}
local e={{Status='HELD'}}
local tx={{Status='WAITING_UI',AwaitingPreparation=true,SourceReadyToken='source:1',
  SourceSpreadReads={{Person='observation:1'}},Units={{{{ID=3,PlacementComplete=false}}}},
  Cities={{{{ID=4,X=5,Y=6,TransferTurn=7}}}},Plots={{1,2}}}}
local state={{Transactions={{flow=tx}}}};local product={{Escrows={{offer=e}}}}
local before=canonical({{f,e,state,product}})
local lines={{}};print=function(line) lines[#lines+1]=line end
Game={{GetProperty=function(_,key) assert(key=='JAC_ABSORPTION_PRODUCT_V1');return product end}}
Players={{[2]={{GetProperty=function(_,key)
  if key=='JAC_ABSORPTION_FLOW_LAST_OPERATION_V1' then return {{Accepted=false,Reason='rejected resolve'}} end
  assert(key=='JAC_ABSORPTION_PRODUCT_LAST_OPERATION_V1');return {{Accepted=true,Nonce='product:1'}}
end}}}}
JAC_AbsorptionGameplay={{GetState=function() return state end,Canonical=canonical}}
JAC_AbsorptionFlowGameplay={{Get=function(id) assert(id=='flow' or id=='missing');if id=='flow' then return f end end}}
{}
TraceCompletionWait('flow','offer');TraceCompletionWait('flow','offer')
assert(#lines==1 and lines[1]:find('Phase=APPLYING',1,true)
  and lines[1]:find('WaitingFor=WAITING_UI',1,true)
  and lines[1]:find('EscrowStatus=HELD',1,true)
  and lines[1]:find('SourceReadyToken=source:1',1,true)
  and lines[1]:find('Reason=rejected resolve',1,true)
  and lines[1]:find('PlacementComplete=false',1,true)
  and lines[1]:find('TransferTurn=7',1,true) and lines[1]:find('PlotCount=2',1,true))
assert(not CompletionReady('flow','offer'))
assert(canonical({{f,e,state,product}})==before,'diagnostic changed saved transfer state')
tx.AwaitingPreparation=false;tx.NativeWorks={{{{Carrier={{Phase='RETURN',BorrowIssued=true}}}}}}
TraceCompletionWait('flow','offer');assert(#lines==2 and lines[2]:find('Phase=RETURN',1,true))
f.Phase='RESULT';f.WaitingFor=nil;tx.Status='COMPLETE';e.Status='CONSUMED'
TraceCompletionWait('flow','offer');assert(#lines==3 and CompletionReady('flow','offer'))
TraceCompletionWait('missing','absent');assert(#lines==4 and not CompletionReady('missing','absent'))
f.Phase='BLOCKED';f.Error='asset failure'
TraceCompletionWait('flow','offer');assert(#lines==5 and lines[5]:find('FlowError=asset failure',1,true))
local ok,err=pcall(CompletionReady,'flow','offer');assert(not ok and tostring(err):find('asset failure',1,true))
-- Emulate the measured Civ6 print limit, including multibyte text and errors
-- containing physical line breaks. A truncated tail must fail this check.
local long=string.rep('漢字🙂\\\r\n\t',600)..'TAIL確認'
f.Error=long;local start=#lines+1
TraceCompletionWait('flow','offer')
local parts={{}};local total
for i=start,#lines do
  local line=lines[i]:sub(1,2047)
  assert(#lines[i]<2047,'Civ6 would truncate diagnostic output')
  assert(not line:find('\n',1,true) and not line:find('\r',1,true),'physical newline broke a frame')
  local index,count,body=line:match('^JAC_PRODUCT_COMPLETION_WAIT %d+ (%d+)/(%d+) (.*)$')
  index,count=tonumber(index),tonumber(count)
  assert(index==i-start+1 and (not total or count==total),'diagnostic chunk order changed')
  total=count;parts[index]=body
  -- Rust below also checks each framed line independently for valid UTF-8.
end
assert(total>1 and #parts==total,'large record was not completely framed')
local escaped=long:gsub('\\','\\\\'):gsub('\r','\\r'):gsub('\n','\\n'):gsub('\t','\\t')
assert(table.concat(parts):find(escaped,1,true),'Unicode, escaped controls or final tail was lost')
TraceCompletionWait('flow','offer');assert(#lines==start+total-1,'large unchanged record was logged again')
CHUNK_LINES=lines
"#, product::COMPLETION_EVIDENCE)).exec()?;
    let lines: mlua::Table = lua.globals().get("CHUNK_LINES")?;
    for line in lines.sequence_values::<mlua::String>() {
        std::str::from_utf8(line?.as_bytes().as_ref())?;
    }
    Ok(())
}

#[test]
fn terminal_acceptance_rejects_partial_transfer_and_misattributed_settlement() -> anyhow::Result<()>
{
    const FIXTURE: &str = r#"
local function copy(v) if type(v)~='table' then return v end local o={} for k,x in pairs(v) do o[k]=copy(x) end return o end
local function equal(a,b) if type(a)~=type(b) then return false end if type(a)~='table' then return a==b end
for k,v in pairs(a) do if not equal(v,b[k]) then return false end end for k in pairs(b) do if a[k]==nil then return false end end return true end
local policy='CONSUME_ON_VERIFIED_RESULT_V2'
local plan={ProductDeposit={OfferID='offer',From=1,To=2,Gold=100,Faith=20,Policy=policy}}
FLOW={ID='flow',From=1,To=2,Mode='SMALL_STATE',Phase='RESULT',Phase1Complete=true,FinalRollsComplete=true,
  CompletedTurn=4,Request=copy(plan),AppliedPlan=copy(plan),Entries={}}
TX={ID='flow',From=1,To=2,Mode='SMALL_STATE',Status='COMPLETE',Request=copy(plan),Units={},Cities={},Plots={}}
ESCROW={ID='offer',From=1,To=2,Gold=100,Faith=20,Status='CONSUMED',
  CollectGold={Before=1000,After=900,Done=true},CollectFaith={Before=1020,After=1000,Done=true},
  Consumption={FlowID='flow',From=1,To=2,Gold=100,Faith=20,CompletedTurn=4,Policy=policy,LegacyRefunded={Gold=0,Faith=0}}}
Game={GetProperty=function() return {Escrows={offer=ESCROW}} end}
Players={[2]={GetTreasury=function() return {GetGoldBalance=function() return 900 end} end}}
JAC_AbsorptionGameplay={Rules={Copy=copy,Equal=equal},GetState=function() return {Transactions={flow=TX}} end,
  PlayerFaithBalance=function() return 1000 end}
JAC_AbsorptionFlowGameplay={Get=function(id) assert(id=='flow');return FLOW end}
JAC_AbsorptionAdmissionGameplay={Get=function(kind,id) assert(kind=='leader-offer' and id=='offer');return {Status='STARTED',FlowID='flow',Plan=copy(plan)} end}
"#;
    for corruption in [
        "FLOW.Phase='CHOICE'",
        "FLOW.Phase1Complete=false",
        "FLOW.FinalRollsComplete=false",
        "FLOW.CompletedTurn=nil",
        "FLOW.Error='blocked transfer'",
        "FLOW.WaitingFor='WAITING_UI'",
        "TX.Status='STAGED'",
        "TX.From=7",
        "TX.ID='other-flow'",
        "TX.Request.ProductDeposit.Gold=0",
        "ESCROW.Status='HELD'",
        "ESCROW.ID='other-offer'",
        "ESCROW.To=7",
        "ESCROW.Gold=0;ESCROW.Consumption.Gold=0",
        "ESCROW.Consumption=nil",
        "ESCROW.Consumption.FlowID='other-flow'",
        "ESCROW.Consumption.Gold=0",
        "ESCROW.Consumption.CompletedTurn=3",
        "ESCROW.Consumption.Policy='REFUND'",
        "ESCROW.Consumption.LegacyRefunded.Faith=20",
        "ESCROW.CollectGold.After=1000",
        "ESCROW.CollectFaith.Done=false",
    ] {
        let lua = Lua::new();
        let accepted: bool=lua.load(format!("{FIXTURE}\n{}\n{}\nassert(CompletionReady('flow','offer'));assert(CompletionEvidence('flow','offer').Transfer.Status=='COMPLETE')\n{corruption}\nreturn pcall(CompletionEvidence,'flow','offer')",product::P1_EVIDENCE,product::COMPLETION_EVIDENCE)).eval()?;
        assert!(!accepted, "false terminal success: {corruption}");
    }
    let lua = Lua::new();
    lua.load(format!("{FIXTURE}\n{}\n{}\nFLOW.Phase='CHOICE';ESCROW.Status='HELD';assert(not CompletionReady('flow','offer'))",product::P1_EVIDENCE,product::COMPLETION_EVIDENCE)).exec()?;
    Ok(())
}

const WORLD: &str = r#"
local function copy(v) if type(v)~='table' then return v end local o={} for k,x in pairs(v) do o[k]=copy(x) end return o end
local function equal(a,b) if type(a)~=type(b) then return false end if type(a)~='table' then return a==b end
  for k,v in pairs(a) do if not equal(v,b[k]) then return false end end for k in pairs(b) do if a[k]==nil then return false end end return true end
local function keys(t) local o={} for k in pairs(t) do o[#o+1]=k end table.sort(o) return o end
local function event() local hs={} return {Add=function(fn) hs[#hs+1]=fn end,Call=function(...) for _,fn in ipairs(hs) do fn(...) end end} end
TURN,GOLD,FAITH,POOL,TILES,COMBAT,PEAK,SCIENCE,POP,ACCESS=7,{[1]=100,[2]=5000},{[1]=20,[2]=1000},10,2,1,10,1,1,0
HUMAN,ALIVE,ALLIES,STATES,OFFICIAL,OFFICIAL_ENVOYS={false,true,false},{true,true,false},{},{},false,0
MAJOR={true,true,true}
FAIL_CHARGE,FAIL_REFUND,FAIL_ENVOY,FAIL_BEGIN=false,false,false,false
CHARGES,REFUNDS,SENDS,STARTS=0,0,0,0
GRANTS,FAIL_GRANT,GRANT_DELTA=0,false,1
CAN_GIVE,CAN_GIVE_TO=true,true
local props={}
Game={GetProperty=function(_,k) return copy(props[k]) end,SetProperty=function(_,k,v) props[k]=copy(v) end,GetCurrentGameTurn=function() return TURN end}
START_TURN=0
GameConfiguration={GetStartTurn=function() return START_TURN end}
Players,PlayerConfigurations={},{}
for actor=1,3 do local id=actor
  Players[id]={IsAlive=function() return ALIVE[id] end,IsHuman=function() return HUMAN[id] end,IsMajor=function() return MAJOR[id] and not (id==1 and OFFICIAL) end,IsTurnActive=function() return true end,
    GetProperty=function(_,k) return copy(props[id..':'..k]) end,SetProperty=function(_,k,v) props[id..':'..k]=copy(v) end,
    GetTreasury=function() return {GetGoldBalance=function() return GOLD[id] or 0 end,ChangeGoldBalance=function(_,n)
      GOLD[id]=GOLD[id]+n;if n<0 then CHARGES=CHARGES+1;if FAIL_CHARGE then FAIL_CHARGE=false;error('charge mutated then threw') end
      else REFUNDS=REFUNDS+1;if FAIL_REFUND then FAIL_REFUND=false;error('refund mutated then threw') end end end} end,
    GetReligion=function() return {GetFaithBalance=function() return FAITH[id] or 0 end,ChangeFaithBalance=function(_,n) FAITH[id]=FAITH[id]+n end,GetFaithYield=function() return id==1 and 1 or SCIENCE end} end,
    GetTechs=function() return {GetScienceYield=function() return SCIENCE end,HasTech=function() return false end} end,
    GetUnits=function() return {Members=function() local n=0;return function() n=n+1;if n<=(id==1 and 1 or COMBAT) then return n,{GetType=function() return id==1 and 0 or 1 end} end end end} end,
    GetCities=function() return {Members=function() local done=false;return function() if done then return end;done=true;return 1,{GetPopulation=function() return id==1 and 1 or POP end,GetYield=function() return SCIENCE end} end end} end,
    GetDiplomacy=function() return {HasMet=function() return true end,IsAtWarWith=function(_,other) return STATES['war:'..id..':'..other]==true end,
      HasAllied=function(_,other) return ALLIES[id..':'..other]==true end,GetVisibilityOn=function() return id==1 and 1 or ACCESS end} end,
    GetDiplomaticAI=function() return {GetDiplomaticStateIndex=function() return STATES[id] or 0 end} end,
    GetInfluence=function() return {CanReceiveInfluence=function() return id==1 and OFFICIAL end,GetTokensReceived=function(_,sender) assert(id==1 and sender==2);return OFFICIAL_ENVOYS end,
      CanGiveInfluence=function() return CAN_GIVE end,CanGiveTokensToPlayer=function() return CAN_GIVE_TO end,
      GetTokensToGive=function() return POOL end,ChangeTokensToGive=function(_,n) POOL=POOL+n;SENDS=SENDS+1;if FAIL_ENVOY then FAIL_ENVOY=false;error('envoy spent then threw') end end,
      GiveFreeTokenToPlayer=function(_,recipient) assert(id==2 and recipient==1);OFFICIAL_ENVOYS=OFFICIAL_ENVOYS+GRANT_DELTA;GRANTS=GRANTS+1;if FAIL_GRANT then FAIL_GRANT=false;error('grant mutated then threw') end end} end}
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
TRANSACTIONS={}
JAC_AbsorptionGameplay={Rules={Copy=copy,Equal=equal,Keys=keys},GetRoster=function() return {} end,
 PlayerFaithBalance=function(owner) return FAITH[owner] or 0 end,GetState=function() return {Transactions=copy(TRANSACTIONS)} end}
FLOWS={}
JAC_AbsorptionFlowGameplay={Begin=function(plan)
  STARTS=STARTS+1;FLOWS[plan.ID]=FLOWS[plan.ID] or {ID=plan.ID,From=plan.From,To=plan.To,Request=copy(plan),Phase='CHOICE',Phase1Complete=true}
  Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
  if FAIL_BEGIN then FAIL_BEGIN=false;error('P1 interrupted') end
  return copy(FLOWS[plan.ID]) end,Get=function(id) return copy(FLOWS[id]) end,Tick=function() end}
function READY() COMBAT=10;ACCESS=4;POP=10;SCIENCE=100 end
function CMD(command,nonce,extras) local p=extras or {};p.Command=command;p.Nonce=nonce;p.From=p.From or 1
  return JAC_AbsorptionProductGameplay.Command(2,p) end
function REGISTER() return CMD('REGISTER','register') end
function ENVOY(n) return CMD('SEND_ENVOY','envoy'..n,{ExpectedEnvoys=n}) end
function VIEW() return ExposedMembers.JAC_AbsorptionProduct.Get(1,2) end
function OFFICIAL_SOURCE()
  OFFICIAL=true
  PlayerConfigurations[1].GetCivilizationTypeName=function() return 'CIVILIZATION_VATICAN_CITY' end
  PlayerConfigurations[1].GetLeaderTypeName=function() return 'LEADER_MINOR_CIV_VATICAN_CITY' end
end
function RESULT()
  local request=N.Get('leader-offer',VIEW().OfferID);local flow=FLOWS[request.FlowID]
  flow.Phase,flow.FinalRollsComplete,flow.CompletedTurn='RESULT',true,TURN
  TRANSACTIONS[flow.ID]={ID=flow.ID,From=flow.From,To=flow.To,Mode=flow.Request.Mode,Status='COMPLETE'}
  flow.Mode=flow.Request.Mode
  Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
  return flow
end
function METRIC_DEMAND(offer) for _,d in ipairs(offer.Requirements) do if d.Kind=='PRODUCT_METRIC' then return d end end error('metric demand missing') end
function CANONICAL(value)
  if type(value)~='table' then return type(value)..':'..tostring(value) end
  local parts={};for _,k in ipairs(keys(value)) do parts[#parts+1]=CANONICAL(k)..'='..CANONICAL(value[k]) end
  return '{'..table.concat(parts,',')..'}'
end
JAC_AbsorptionGameplay.Canonical=CANONICAL
"#;

fn load(lua: &Lua, area: f64) -> anyhow::Result<()> {
    lua.load("RESET_EVENTS()").exec()?;
    lua.load(absorption_admission::lua()).exec()?;
    lua.load("N=JAC_AbsorptionAdmissionGameplay").exec()?;
    lua.load(Repository::checked_in().read(product::NATIVE_READ_UI)?)
        .exec()?;
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
fn changing_terms_follow_elapsed_ten_turn_windows_at_zero_and_historical_starts(
) -> anyhow::Result<()> {
    for start in [0, 16603] {
        let lua = world(10000.)?;
        lua.globals().set("START_TURN", start)?;
        lua.globals().set("TURN", start)?;
        lua.load(r#"
          JAC_AbsorptionGameplay.GetRoster=function(actor)
            return actor==1 and {['leader:1']={Personality='自由'}} or {} end
          REGISTER();local current=VIEW().OfferID
          assert(N.GetOffer(current).ProductTerms.Input.FreeEpoch==0,'first window must start at 0.9')
          for _,elapsed in ipairs({9,10,19,20,29,30}) do
            TURN=START_TURN+elapsed;JAC_AbsorptionProductGameplay.Tick()
            local expected=math.floor(elapsed/10)
            local next=VIEW().OfferID
            assert(N.GetOffer(next).ProductTerms.Input.FreeEpoch==expected,'elapsed window mismatch')
            if elapsed%10==0 then
              assert(next~=current and N.Get('leader-offer',current).Status=='CANCELLED')
            else assert(next==current,'terms changed before ten elapsed turns') end
            current=next
          end
        "#).exec()?;
        load(&lua, 10000.)?;
        lua.load("local before=VIEW().OfferID;JAC_AbsorptionProductGameplay.Tick();assert(VIEW().OfferID==before and CHARGES==0 and STARTS==0)").exec()?;
    }
    Ok(())
}

#[test]
fn changing_terms_do_not_replace_saved_phase_one_at_a_historical_window_boundary(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(
        r#"
      START_TURN=16603;TURN=START_TURN
      JAC_AbsorptionGameplay.GetRoster=function(actor)
        return actor==1 and {['leader:1']={Personality='自由'}} or {} end
      REGISTER();READY();JAC_AbsorptionProductGameplay.Tick();assert(STARTS==1)
      ORIGINAL_OFFER=VIEW().OfferID
      ORIGINAL_PLAN=N.Get('leader-offer',ORIGINAL_OFFER).Plan
      ORIGINAL_FLOW=JAC_AbsorptionFlowGameplay.Get(ORIGINAL_PLAN.ID)
      assert(N.GetOffer(ORIGINAL_OFFER).ProductTerms.Input.FreeEpoch==0)
      TURN=START_TURN+10
    "#,
    )
    .exec()?;
    load(&lua, 10000.)?;
    lua.load(r#"
      JAC_AbsorptionProductGameplay.Tick();GameEvents.OnGameTurnStarted.Call()
      assert(VIEW().OfferID==ORIGINAL_OFFER and STARTS==1)
      assert(JAC_AbsorptionGameplay.Rules.Equal(N.Get('leader-offer',ORIGINAL_OFFER).Plan,ORIGINAL_PLAN))
      assert(JAC_AbsorptionGameplay.Rules.Equal(JAC_AbsorptionFlowGameplay.Get(ORIGINAL_PLAN.ID),ORIGINAL_FLOW))
    "#).exec()?;
    Ok(())
}

#[test]
fn deposit_share_uses_source_balances_independently_of_recipient_balances() -> anyhow::Result<()> {
    for recipient in [100000, 500000] {
        let lua = world(10000.)?;
        lua.globals().set("RECIPIENT_BALANCE", recipient)?;
        lua.load(
            r#"
          GOLD[1]=80000;FAITH[1]=8000;GOLD[2]=RECIPIENT_BALANCE;FAITH[2]=60000
          local quote=VIEW();assert(quote.Deposit.Gold==23000 and quote.Deposit.Faith==2300)
          REGISTER();READY();JAC_AbsorptionProductGameplay.Tick()
          assert(GOLD[1]==80000 and FAITH[1]==8000,'source funds must remain untouched')
          assert(GOLD[2]==RECIPIENT_BALANCE-23000 and FAITH[2]==57700 and CHARGES==1)
        "#,
        )
        .exec()?;
    }
    Ok(())
}

#[test]
fn ui_only_diplomacy_reads_stay_in_the_native_ui_context_and_reject_stale_turns(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(r#"
      UI_READ_CONTEXT=setmetatable({Players={}}, {__index=_G})
      for id,player in pairs(Players) do
        UI_READ_CONTEXT.Players[id]={GetDiplomacy=player.GetDiplomacy,GetDiplomaticAI=player.GetDiplomaticAI}
        local diplomacy=player.GetDiplomacy
        player.GetDiplomacy=function() local value=diplomacy()
          value.GetVisibilityOn=function() error('UI visibility called in Gameplay') end;return value end
        player.GetDiplomaticAI=function() error('UI diplomatic AI called in Gameplay') end
      end
    "#).exec()?;
    lua.load(Repository::checked_in().read(product::NATIVE_READ_UI)?)
        .set_environment(lua.globals().get("UI_READ_CONTEXT")?)
        .exec()?;
    lua.load(
        r#"
      assert(VIEW().Status=='AVAILABLE',VIEW().Error)
      local read=ExposedMembers.JAC_AbsorptionProductNativeRead.GetDiplomacy
      ExposedMembers.JAC_AbsorptionProductNativeRead.GetDiplomacy=function(from,to)
        local result=read(from,to);result.Turn=TURN-1;return result end
      local view=VIEW();assert(view.Status=='UNAVAILABLE' and view.Error:find('stale',1,true))
      assert(CHARGES==0 and SENDS==0 and STARTS==0)
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn product_registration_during_another_flow_does_not_freeze_a_stale_preflight_plan(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(r#"
      local P=JAC_AbsorptionProductGameplay
      COMBAT=100;PEAK=100
      local begin=JAC_AbsorptionFlowGameplay.Begin
      JAC_AbsorptionFlowGameplay.Begin=function() error('inventory unavailable before journalling') end
      P.Command(2,{Command='REGISTER',From=1,Nonce='initial-register'});P.Tick()
      local request=N.Get('leader-offer',VIEW().OfferID)
      assert(request.Plan and request.LastError and STARTS==0)
      local charged,paidGold,paidFaith=CHARGES,GOLD[2],FAITH[2]
      assert(paidGold<5000 or paidFaith<1000)
      FLOWS.other={ID='other',From=3,To=2,Phase='CHOICE',Phase1Complete=false}
      Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
      P.Command(2,{Command='REGISTER',From=1,Nonce='busy-register'})
      request=N.Get('leader-offer',VIEW().OfferID)
      assert(request.Status=='WAITING' and not request.Plan and CHARGES==charged and STARTS==0)
      assert(GOLD[2]==paidGold and FAITH[2]==paidFaith)
      COMBAT=0;FLOWS.other.Phase='DONE'
      Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
      JAC_AbsorptionFlowGameplay.Begin=begin;N.Tick();P.Tick()
      assert(not N.Get('leader-offer',VIEW().OfferID).Plan and STARTS==0 and CHARGES==charged)
    "#).exec()?;
    Ok(())
}

#[test]
fn product_faith_charges_and_cancel_returns_use_the_existing_context_aware_reader(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(r#"
      for _,actor in ipairs({1,2}) do local religion=Players[actor].GetReligion
        Players[actor].GetReligion=function() local value=religion();value.GetFaithBalance=nil;return value end end
      READY();local original=FAITH[2]
      JAC_AbsorptionFlowGameplay.Begin=function() error('preflight before flow journal') end
      REGISTER();JAC_AbsorptionProductGameplay.Tick()
      local view=VIEW();local escrow=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1').Escrows[view.OfferID]
      assert(escrow.Faith>0 and FAITH[2]==original-escrow.Faith)
      CMD('CANCEL','faith-ui-cancel',{OfferID=view.OfferID})
      assert(FAITH[2]==original and STARTS==0)
    "#).exec()?;
    Ok(())
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
        "HUMAN[2]=false;REGISTER();READY();ENVOY(0);TURN=TURN+1;ENVOY(1);GameEvents.OnGameTurnStarted.Call()",
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
fn registration_nonce_remains_available_until_cancel_refund_is_observed() -> anyhow::Result<()> {
    for currency in ["Gold", "Faith"] {
        let lua = world(10000.)?;
        lua.globals().set("PENDING_CURRENCY", currency)?;
        lua.load(r#"
          JAC_AbsorptionFlowGameplay.Begin=function() error('preflight before native journal') end
          REGISTER();READY();GameEvents.OnGameTurnStarted.Call()
          OLD=VIEW().OfferID;assert(CHARGES==1 and VIEW().CanCancel)
          local gold=Players[2].GetTreasury
          local faith=JAC_AbsorptionGameplay.PlayerFaithBalance
          REPORTED_GOLD,REPORTED_FAITH=GOLD[2],FAITH[2]
          Players[2].GetTreasury=function()
            local treasury=gold()
            treasury.GetGoldBalance=function() return PENDING_CURRENCY=='Gold' and REPORTED_GOLD or GOLD[2] end
            return treasury
          end
          JAC_AbsorptionGameplay.PlayerFaithBalance=function(owner)
            return owner==2 and PENDING_CURRENCY=='Faith' and REPORTED_FAITH or faith(owner)
          end
          assert(not pcall(CMD,'CANCEL','pending-cancel',{OfferID=OLD}))
          local view=VIEW();assert(view.Status=='CANCELLED' and view.CanRegister==false)
          for i=1,3 do
            local ok,why=pcall(CMD,'REGISTER','same-register',{OfferID=OLD})
            assert(not ok and tostring(why):find('awaits refund',1,true))
            local state=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1')
            assert(state.Operations['2:same-register']==nil and state.Pairs['1:2'].ID==OLD)
          end
          REFUND_COUNT=REFUNDS
        "#).exec()?;
        load(&lua, 10000.)?;
        lua.load(
            r#"
          assert(not VIEW().CanRegister and not pcall(CMD,'REGISTER','same-register',{OfferID=OLD}))
          REPORTED_GOLD,REPORTED_FAITH=GOLD[2],FAITH[2]
          COMBAT=0
          GameEvents.OnGameTurnStarted.Call()
          assert(VIEW().Status=='CANCELLED' and VIEW().CanRegister and REFUNDS==REFUND_COUNT)
          assert(GOLD[2]==5000 and FAITH[2]==1000)
          local view=CMD('REGISTER','same-register',{OfferID=OLD})
          assert(view.OfferID~=OLD and view.Status=='WAITING')
          local state=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1')
          assert(state.Operations['2:same-register'].Done and state.Pairs['1:2'].Sequence==2)
          NEW=view.OfferID
          CMD('REGISTER','same-register',{OfferID=OLD})
          assert(VIEW().OfferID==NEW and GOLD[2]==5000 and FAITH[2]==1000 and CHARGES==1)
        "#,
        )
        .exec()?;
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
      local before=METRIC_DEMAND(N.GetOffer(OLD)).Minimum
      STATES[1]=2;GameEvents.OnGameTurnStarted.Call()
      NEW=VIEW().OfferID;assert(OLD~=NEW)
      assert(N.Get('leader-offer',OLD).Status=='CANCELLED' and N.Get('leader-offer',OLD).ReplacedBy==NEW)
      assert(METRIC_DEMAND(N.GetOffer(NEW)).Minimum>before and CHARGES==0 and STARTS==0)
      assert(not pcall(CMD,'CANCEL','old-cancel',{OfferID=OLD}))
    "#).exec()?;
    load(&lua, 10000.)?;
    lua.load("GameEvents.OnGameTurnStarted.Call();assert(VIEW().OfferID==NEW and CHARGES==0)")
        .exec()?;
    Ok(())
}
#[test]
fn verified_result_consumes_paid_deposit_without_currency_credit_or_second_payment(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(
        r#"
      REGISTER();READY();GameEvents.OnGameTurnStarted.Call();assert(STARTS==1 and CHARGES==1)
      PAID_GOLD,PAID_FAITH=GOLD[2],FAITH[2];RESULT()
      FAIL_REFUND=true;GameEvents.OnGameTurnStarted.Call();assert(GOLD[2]==PAID_GOLD and FAITH[2]==PAID_FAITH and REFUNDS==0)
      local state=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1');local escrow=state.Escrows[VIEW().OfferID]
      assert(state.Version==2 and escrow.SourceVersion==2 and escrow.Status=='CONSUMED')
      assert(escrow.Consumption.Gold==escrow.Gold and escrow.Consumption.Faith==escrow.Faith and FAIL_REFUND)
    "#,
    )
    .exec()?;
    load(&lua, 10000.)?;
    lua.load("GameEvents.OnGameTurnStarted.Call();assert(GOLD[2]==PAID_GOLD and FAITH[2]==PAID_FAITH and REFUNDS==0);REGISTER();assert(STARTS==1 and CHARGES==1)").exec()?;
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
            // Waiting, frozen first-stage choices, then verified terminal settlement.
            assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 3);
            assert_eq!(steps.iter().filter(|s| s["action"] == "load").count(), 3);
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

#[test]
fn official_is_major_rejects_minor_receivers_at_every_public_gate_without_assets_or_starts(
) -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(r#"
      READY();HUMAN[2]=false;MAJOR[2]=false
      -- Even a major-looking catalog mapping and non-receiving influence do
      -- not confer the official civilization class on this actor.
      assert(not Players[2]:GetInfluence():CanReceiveInfluence())
      assert(VIEW().Status=='UNAVAILABLE' and #JAC_AbsorptionProductGameplay.GetSmallCountries(2)==0)
      for _,command in ipairs({'REGISTER','SEND_ENVOY'}) do
        assert(not pcall(CMD,command,'minor:'..command,{ExpectedEnvoys=0}))
        GameEvents.JAC_AbsorptionProduct_Operation.Call(2,{Command=command,From=1,Nonce='event:'..command,ExpectedEnvoys=0})
        assert(not Players[2]:GetProperty('JAC_ABSORPTION_PRODUCT_LAST_OPERATION_V1').Accepted)
      end
      GameEvents.OnGameTurnStarted.Call()
      assert(CHARGES==0 and REFUNDS==0 and SENDS==0 and GRANTS==0 and STARTS==0 and POOL==10)
      assert(GOLD[1]==100 and GOLD[2]==5000 and FAITH[2]==1000)
      assert(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1')==nil and Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1')==nil)
      MAJOR[2]=true;assert(#JAC_AbsorptionProductGameplay.GetSmallCountries(2)==1)
      OFFICIAL_SOURCE();assert(#JAC_AbsorptionProductGameplay.GetSmallCountries(2)==1)
      assert(VIEW().Envoys.Route=='OFFICIAL','minor sources retain their real-area classification')
    "#).exec()?;
    Ok(())
}

#[test]
fn old_invalid_receiver_escrow_returns_to_the_saved_payer_once_by_cancel_or_turn(
) -> anyhow::Result<()> {
    for cancel in [false, true] {
        let lua = world(10000.)?;
        lua.load("REGISTER();READY();FAIL_CHARGE=true;GameEvents.OnGameTurnStarted.Call();OLD=VIEW().OfferID;MAJOR[2]=false;assert(CHARGES==1 and STARTS==0)").exec()?;
        load(&lua, 10000.)?;
        lua.globals().set("MANUAL_CANCEL", cancel)?;
        lua.load(r#"
          assert(VIEW().Status=='UNAVAILABLE' and VIEW().CanCancel)
          assert(not pcall(REGISTER) and not pcall(ENVOY,0))
          ALIVE[3]=true;assert(not pcall(JAC_AbsorptionProductGameplay.Command,3,{Command='CANCEL',From=1,Nonce='forged',OfferID=OLD}));ALIVE[3]=false
          if MANUAL_CANCEL then CMD('CANCEL','old-payer',{OfferID=OLD}) else GameEvents.OnGameTurnStarted.Call() end
          assert(N.Get('leader-offer',OLD).Status=='CANCELLED' and GOLD[2]==5000 and FAITH[2]==1000)
          GameEvents.OnGameTurnStarted.Call();assert(REFUNDS==1 and CHARGES==1 and STARTS==0 and SENDS==0)
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn official_minor_ai_uses_real_pool_real_received_getter_and_all_admission_demands(
) -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(r#"
      OFFICIAL_SOURCE();HUMAN[2]=false
      GameEvents.OnGameTurnStarted.Call();assert(SENDS==0 and GRANTS==0 and CHARGES==0 and STARTS==0)
      READY();GOLD[2]=0;GameEvents.OnGameTurnStarted.Call();assert(SENDS==0 and GRANTS==0)
      GOLD[2]=5000;FAITH[2]=0;GameEvents.OnGameTurnStarted.Call();assert(SENDS==0 and GRANTS==0)
      FAITH[2]=1000;STATES['war:2:1']=true;GameEvents.OnGameTurnStarted.Call();assert(SENDS==0 and GRANTS==0)
      STATES['war:2:1']=nil
      Game:SetProperty('JAC_COUNTRY_RELATIONS_V1',{Version=1,Countries={VATICAN_CITY={Reviewed=false}},Relations={}})
      GameEvents.OnGameTurnStarted.Call();assert(SENDS==0 and GRANTS==0)
      Game:SetProperty('JAC_COUNTRY_RELATIONS_V1',{Version=1,Countries={VATICAN_CITY={Reviewed=true}},Relations={}})
      GameEvents.OnGameTurnStarted.Call()
      assert(POOL==9 and SENDS==1 and GRANTS==1 and VIEW().Envoys.Envoys==1 and CHARGES==0 and STARTS==0)
      TURN=TURN+1
      GameEvents.OnGameTurnStarted.Call()
      assert(POOL==8 and SENDS==2 and GRANTS==2 and VIEW().Envoys.Envoys==2 and STARTS==0)
      assert(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1').Envoys['1:2']==nil,'OFFICIAL must not use a counterfeit count')
      GameEvents.OnGameTurnStarted.Call();assert(CHARGES==1 and STARTS==1 and GOLD[2]<5000 and FAITH[2]<1000)
      GameEvents.OnGameTurnStarted.Call();assert(SENDS==2 and GRANTS==2 and CHARGES==1 and STARTS==1)
    "#).exec()?;
    load(&lua, 9999.)?;
    lua.load("GameEvents.OnGameTurnStarted.Call();assert(POOL==8 and SENDS==2 and GRANTS==2 and CHARGES==1 and STARTS==1)").exec()?;
    Ok(())
}

#[test]
fn official_envoy_mutate_then_throw_reloads_without_second_debit_or_grant() -> anyhow::Result<()> {
    for grant in [false, true] {
        let lua = world(9999.)?;
        lua.globals().set("FAIL_AT_GRANT", grant)?;
        lua.load(r#"
          OFFICIAL_SOURCE();HUMAN[2]=false
          OBSERVED={}
          RECORD_OBSERVATION=function(stage,from,to,opkey,nonce,turn)
            assert(from==1 and to==2 and opkey=='2:envoy0' and nonce=='envoy0' and turn==TURN)
            OBSERVED[#OBSERVED+1]={Stage=stage,Pool=Players[to]:GetInfluence():GetTokensToGive(),
              Received=Players[from]:GetInfluence():GetTokensReceived(to)}
          end
          JAC_AbsorptionProductGameplay.NativeEnvoyObserver=RECORD_OBSERVATION
          if FAIL_AT_GRANT then FAIL_GRANT=true else FAIL_ENVOY=true end
          assert(not pcall(ENVOY,0));assert(POOL==9 and SENDS==1 and GRANTS==(FAIL_AT_GRANT and 1 or 0))
          assert(#OBSERVED==1 and OBSERVED[1].Stage=='BEFORE'
            and OBSERVED[1].Pool==10 and OBSERVED[1].Received==0)
        "#).exec()?;
        load(&lua, 9999.)?;
        lua.load(r#"
          assert(JAC_AbsorptionProductGameplay.NativeEnvoyObserver==nil)
          JAC_AbsorptionProductGameplay.NativeEnvoyObserver=RECORD_OBSERVATION
          ENVOY(0);ENVOY(0);assert(POOL==9 and SENDS==1 and GRANTS==1 and VIEW().Envoys.Envoys==1)
          assert(#OBSERVED==2 and OBSERVED[2].Stage=='AFTER'
            and OBSERVED[2].Pool==9 and OBSERVED[2].Received==1,
            'recovery replaced the original before sample or missed the settled grant')
          local op=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1').Operations['2:envoy0']
          assert(op.Done and op.EnvoyReceipt.ChargeDone and op.EnvoyReceipt.GrantIntent and op.EnvoyReceipt.GrantDone and op.EnvoyReceipt.Done)
          assert(op.EnvoyIdentity.From==1 and op.EnvoyIdentity.To==2 and op.EnvoyIdentity.Route=='OFFICIAL')
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn official_sender_native_permissions_are_checked_before_any_journal_or_debit() -> anyhow::Result<()>
{
    for permission in ["sender", "recipient", "stale", "identity"] {
        let lua = world(9999.)?;
        lua.globals().set("PERMISSION", permission)?;
        lua.load(r#"
          OFFICIAL_SOURCE();HUMAN[2]=false;READY()
          if PERMISSION=='sender' then CAN_GIVE=false
          elseif PERMISSION=='recipient' then CAN_GIVE_TO=false
          else local api=ExposedMembers.JAC_AbsorptionProductNativeRead;local read=api.GetEnvoyEligibility
            api.GetEnvoyEligibility=function(from,to) local result=read(from,to)
              if PERMISSION=='stale' then result.Turn=TURN-1 else result.From=3 end;return result end
          end
          assert(not pcall(ENVOY,0),'official native restriction was bypassed')
          JAC_AbsorptionProductGameplay.Tick()
          assert(SENDS==0 and GRANTS==0 and POOL==10 and STARTS==0 and CHARGES==0)
          local state=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1')
          assert(not state or next(state.Operations)==nil,'rejected permission left a pending operation')
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn publication_reconciles_saved_ai_asset_application_without_a_second_begin() -> anyhow::Result<()>
{
    let lua = world(10000.)?;
    lua.load(r#"
      REGISTER();READY()
      local begin=JAC_AbsorptionFlowGameplay.Begin
      JAC_AbsorptionFlowGameplay.Begin=function(plan) local flow=begin(plan);FLOWS[plan.ID].Phase='APPLYING';return FLOWS[plan.ID] end
      local reconciled=0
      JAC_AbsorptionFlowGameplay.Tick=function()
        local request=N.Get('leader-offer',VIEW().OfferID)
        if request and request.Status=='STARTED' then
          local flow=FLOWS[request.FlowID]
          if flow.Phase=='APPLYING' then flow.Phase='DONE';reconciled=reconciled+1 end
        end
      end
      assert(JAC_AbsorptionProductGameplay.RecheckPublished(2,TURN))
      assert(STARTS==1 and CHARGES==1 and reconciled==1)
      for i=1,3 do JAC_AbsorptionProductGameplay.RecheckPublished(2,TURN) end
      assert(STARTS==1 and CHARGES==1 and reconciled==1)
    "#).exec()?;
    Ok(())
}

#[test]
fn p1_checkpoint_preserves_decisions_while_application_advances() -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(format!("{}\n{}", product::P1_EVIDENCE, r#"
      local original={ID='original',From=1,To=2,Mode='MERGER',Turn=7,
        Request={ID='original',From=1,To=2,Mode='MERGER',LeaderProfile={LeaderType='LEADER_JAC_ROKKAKU_SADAYORI'}},
        RosterAtBegin={person={Owner=1}},Inventory={UnitIDs={11}},Phase1Complete=true,
        Entries={{ID='person',Kind='UNIT',Name='original name',Portrait='portrait',Source={Owner=1},
          UnitID=11,UnitType='UNIT_WARRIOR',AllowedActions={'SURRENDER','INCORPORATE','DISSOLVE'},Abilities={},
          EscapeOutcome='ESCAPE',P1Chance=3200,
          Rolls={P1_ESCAPE={Raw=22,Used=22,Label='escape',Debug=false,Turn=7},
                 P1_BRANCH={Raw=44,Used=44,Label='branch',Debug=false,Turn=7}}},
          {ID='leader:1',Kind='GREAT_PERSON',Leader=true,AllowedActions={'RECRUIT'},Source={Owner=1,Loyalty=50},
            SelectedAction='RECRUIT',FinalOutcome='RECRUIT',P1Chance=800,
            Rolls={P1_ESCAPE={Raw=8000,Used=8000,Label='leader escape',Debug=false,Turn=7},
              P3_SUCCESS={Raw=91,Used=91,Label='action',Debug=false,Turn=7}}},
          {ID='native-work:4',Kind='ITEM',NativeWork={Index=4},AllowedActions={'TRANSFER','LOOT','SEIZE','RETURN'},Source={Owner=1,NativeWork={Index=4}},
            P1Chance=1600,Rolls={P1_ESCAPE={Raw=9000,Used=9000,Label='item escape',Debug=false,Turn=7}}},
          {ID='native-hero:1:HEROCLASS_JAC_NOBUTORA',Kind='HERO',DormantClass=3,AllowedActions={'RECRUIT','PROTECT','RECONTRACT','SUICIDE'},Source={Owner=1,HeroClassType='HEROCLASS_JAC_NOBUTORA'},
            P1Chance=2000,Rolls={P1_ESCAPE={Raw=7000,Used=7000,Label='hero escape',Debug=false,Turn=7}}}},Phase='APPLYING',
        FinalRollsComplete=true,AppliedPlan={ID='original',From=1,To=2,Mode='MERGER',Assets={},Units={}}}
      local baseline=CANONICAL(P1Evidence(original))
      local advanced=JAC_AbsorptionGameplay.Rules.Copy(original)
      advanced.Phase='DONE';advanced.AcknowledgedTurn=8;advanced.CompletedTurn=8
      advanced.AppliedPlan={ID='original',From=1,To=2,Mode='MERGER',Assets={},Units={}};advanced.FinalRollsComplete=true
      advanced.Entries[1].FinalOutcome='ESCAPE'
      assert(CANONICAL(P1Evidence(advanced))==baseline)
      local mutations={
        function(f) f.ID='different' end,function(f) f.From=3 end,function(f) f.To=3 end,
        function(f) f.Mode='SMALL_STATE' end,function(f) f.Turn=8 end,
        function(f) f.Request.LeaderProfile.LeaderType='different' end,
        function(f) f.RosterAtBegin.person.Owner=2 end,function(f) f.Inventory.UnitIDs[1]=12 end,
        function(f) f.Entries[1].ID='different' end,function(f) f.Entries[1].Kind='HERO' end,
        function(f) f.Entries[1].Name='different' end,function(f) f.Entries[1].Portrait='different' end,
        function(f) f.Entries[1].Source.Owner=2 end,function(f) f.Entries[1].UnitID=12 end,
        function(f) f.Entries[1].UnitType='UNIT_SCOUT' end,
        function(f) f.Entries[1].AllowedActions[1]='ESCAPE' end,
        function(f) f.Entries[1].Abilities[1]='different' end,
        function(f) f.Entries[2].Leader=false end,
        function(f) f.Entries[3].NativeWork.Index=5 end,
        function(f) f.Entries[4].DormantClass=4 end,
        function(f) f.Entries[1].EscapeOutcome=nil end,
        function(f) f.Entries[1].P1Chance=0.9 end,
        function(f) f.Entries[1].Rolls.P1_ESCAPE.Raw=23 end,
        function(f) f.Entries[1].Rolls.P1_BRANCH.Used=45 end,
        function(f) f.Entries[1].Rolls.P1_ESCAPE.Label='different' end,
        function(f) f.Entries[1].Rolls.P1_BRANCH.Debug=true end,
        function(f) f.Entries[1].Rolls.P1_BRANCH.Turn=8 end,
        function(f) f.Entries={} end}
      for i,mutate in ipairs(mutations) do
        local changed=JAC_AbsorptionGameplay.Rules.Copy(original);mutate(changed)
        assert(CANONICAL(P1Evidence(changed))~=baseline,'P1 mutation accepted '..i)
      end
      original.Phase1Complete=false
      assert(not pcall(P1Evidence,original))
      local inventory=CANONICAL(InventoryEvidence(original))
      local initialMutations={
        function(f) f.Entries[1].Name='different' end,
        function(f) f.Entries[1].Portrait='different' end,
        function(f) f.Entries[1].AllowedActions[1]='RECRUIT' end,
        function(f) f.Entries[1].Abilities[1]='different' end,
        function(f) f.Entries[2].Leader=false end,
        function(f) f.Entries[3].NativeWork.Index=5 end,
        function(f) f.Entries[4].DormantClass=4 end}
      for i,mutate in ipairs(initialMutations) do
        local changed=JAC_AbsorptionGameplay.Rules.Copy(original);mutate(changed)
        assert(CANONICAL(InventoryEvidence(changed))~=inventory,'initial metadata mutation accepted '..i)
      end
      original.Phase1Complete=true
      assert(CANONICAL(InventoryEvidence(original))==inventory,'P1 completion altered initial inventory projection')
    "#)).exec()?;
    for small in [true, false] {
        let scene: serde_json::Value = serde_json::from_str(&product::acceptance_scenario(
            &Repository::checked_in(),
            small,
            false,
        )?)?;
        let codes: Vec<&str> = scene["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|step| step["code"].as_str())
            .collect();
        assert!(codes
            .iter()
            .any(|code| code.contains("Canonical(P1Evidence(flow))==t.Snapshot")));
        assert!(codes
            .iter()
            .any(|code| code.contains("product replay charged twice")));
    }
    Ok(())
}

#[test]
fn diagnostic_envoy_getter_failure_does_not_interrupt_the_operation() -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(
        r#"
      OFFICIAL_SOURCE();HUMAN[2]=false;READY()
      local stages={}
      JAC_AbsorptionProductGameplay.NativeEnvoyObserver=function(stage,from,to,opkey,nonce,turn)
        stages[#stages+1]=stage
        assert(from==1 and to==2 and opkey=='2:envoy0' and nonce=='envoy0' and turn==TURN)
        local getter=Game.GetCurrentGameTurn
        Game.GetCurrentGameTurn=function() error('diagnostic getter unavailable') end
        local ok=pcall(Game.GetCurrentGameTurn)
        Game.GetCurrentGameTurn=getter
        assert(ok)
      end
      ENVOY(0)
      assert(SENDS==1 and GRANTS==1 and POOL==9 and OFFICIAL_ENVOYS==1)
      assert(#stages==2 and stages[1]=='BEFORE' and stages[2]=='AFTER')
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn ordinary_turn_does_not_construct_unregistered_native_events_for_diagnostics(
) -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(r#"
      OFFICIAL_SOURCE();HUMAN[2]=false;READY()
      setmetatable(GameEvents,{__index=function(_,name) error('unregistered native event: '..name) end})
      GameEvents.OnGameTurnStarted.Call(TURN)
      assert(SENDS==1 and GRANTS==1 and POOL==9 and OFFICIAL_ENVOYS==1)
      assert(JAC_AbsorptionProductGameplay.NativeEnvoyObserver==nil)
    "#).exec()?;
    Ok(())
}

#[test]
fn operation_observation_reads_native_ai_changes_without_changing_product_or_error_flow(
) -> anyhow::Result<()> {
    for fail_observer in [false, true] {
        let lua = world(9999.)?;
        lua.globals().set("FAIL_OBSERVER", fail_observer)?;
        lua.load(r#"
          OFFICIAL_SOURCE();HUMAN[2]=false;READY()
          -- The engine's own two assignments happen before our ordinary handler.
          POOL,OFFICIAL_ENVOYS,TILES=8,2,6
          local rows={}
          JAC_AbsorptionProductGameplay.NativeEnvoyObserver=function(stage,from,to,opkey,nonce,turn)
            assert(from==1 and to==2 and opkey=='2:ai-envoy:1:2' and nonce=='ai-envoy:1:2')
            rows[#rows+1]={Stage=stage,Turn=turn,
              Pool=Players[to]:GetInfluence():GetTokensToGive(),
              Received=Players[from]:GetInfluence():GetTokensReceived(to)}
            if FAIL_OBSERVER then error('read-only observer failed') end
          end
          GameEvents.OnGameTurnStarted.Call(TURN)
          assert(#rows==2 and rows[1].Stage=='BEFORE' and rows[2].Stage=='AFTER')
          assert(rows[1].Turn==TURN and rows[2].Turn==TURN)
          assert(rows[1].Pool==8 and rows[1].Received==2 and rows[2].Pool==7 and rows[2].Received==3)
          assert(SENDS==1 and GRANTS==1)
          local s=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1')
          local op=s.Operations['2:ai-envoy:1:2']
          assert(op and op.EnvoyReceipt.Before==8 and op.EnvoyReceipt.After==7)
          assert(op.EnvoyReceipt.ReceivedBefore==2 and op.EnvoyReceipt.ReceivedAfter==3)
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn envoy_gateway_after_noop_turn_and_fresh_install_observes_only_the_real_operation(
) -> anyhow::Result<()> {
    for via_event in [false, true] {
        let lua = world(9999.)?;
        lua.globals().set("VIA_EVENT", via_event)?;
        lua.load(
            r#"
          OFFICIAL_SOURCE();HUMAN[2]=false;POOL=30;OFFICIAL_ENVOYS=3;TILES=6
          JAC_AbsorptionProductGameplay.NativeEnvoyObserver=function() error('old VM callback') end
          GameEvents.OnGameTurnStarted.Call(TURN)
          assert(SENDS==0 and GRANTS==0 and POOL==30 and OFFICIAL_ENVOYS==3)
        "#,
        )
        .exec()?;
        load(&lua, 9999.)?;
        lua.load(r#"
          assert(JAC_AbsorptionProductGameplay.NativeEnvoyObserver==nil)
          READY()
          local rows={}
          JAC_AbsorptionProductGameplay.NativeEnvoyObserver=function(stage,from,to,opkey,nonce,turn)
            assert(from==1 and to==2 and opkey=='2:later' and nonce=='later' and turn==TURN)
            rows[#rows+1]={Stage=stage,Pool=Players[to]:GetInfluence():GetTokensToGive(),
              Received=Players[from]:GetInfluence():GetTokensReceived(to)}
          end
          local command={Command='SEND_ENVOY',From=1,Nonce='later',ExpectedEnvoys=3}
          if VIA_EVENT then
            GameEvents.JAC_AbsorptionProduct_Operation.Call(2,command)
            assert(Players[2]:GetProperty('JAC_ABSORPTION_PRODUCT_LAST_OPERATION_V1').Accepted)
          else JAC_AbsorptionProductGameplay.Command(2,command) end
          assert(#rows==2 and rows[1].Stage=='BEFORE' and rows[1].Pool==30 and rows[1].Received==3)
          assert(rows[2].Stage=='AFTER' and rows[2].Pool==29 and rows[2].Received==4)
          assert(SENDS==1 and GRANTS==1 and POOL==29 and OFFICIAL_ENVOYS==4)
          local operation=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1').Operations['2:later']
          assert(operation.Done and operation.EnvoyReceipt.Done and operation.EnvoyReceipt.Before==30
            and operation.EnvoyReceipt.After==29 and operation.EnvoyReceipt.ReceivedBefore==3
            and operation.EnvoyReceipt.ReceivedAfter==4)
          GameEvents.OnGameTurnStarted.Call(TURN)
          JAC_AbsorptionProductGameplay.Command(2,command)
          assert(#rows==2 and SENDS==1 and GRANTS==1,'no-op tick or nonce replay replaced observation')
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn real_turn_event_argument_still_runs_the_normal_ai_envoy_scan() -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(
        r#"
      OFFICIAL_SOURCE();HUMAN[2]=false;READY()
      GameEvents.OnGameTurnStarted.Call(TURN)
      assert(SENDS==1 and GRANTS==1 and POOL==9 and OFFICIAL_ENVOYS==1)
      GameEvents.OnGameTurnStarted.Call(TURN)
      assert(SENDS==1 and GRANTS==1 and POOL==9 and OFFICIAL_ENVOYS==1)
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn same_turn_publication_finishes_observed_currency_receipt_without_recharging(
) -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(r#"
      REGISTER();READY();UI_FAITH=FAITH[2]
      JAC_AbsorptionGameplay.PlayerFaithBalance=function(owner) return owner==2 and UI_FAITH or FAITH[owner] end
      assert(JAC_AbsorptionProductGameplay.RecheckPublished(2,TURN))
      assert(STARTS==0 and CHARGES==1 and FAITH[2]<1000)
      local before=CANONICAL({Gold=GOLD[2],Faith=FAITH[2]})
      for i=1,3 do JAC_AbsorptionProductGameplay.RecheckPublished(2,TURN) end
      assert(CANONICAL({Gold=GOLD[2],Faith=FAITH[2]})==before,'stale UI retried a currency mutation')
      local key=JAC_AbsorptionProductGameplay.PublicationKey()
      UI_FAITH=FAITH[2]
      assert(JAC_AbsorptionProductGameplay.PublicationKey()~=key,'observed UI balance cannot wake pending receipt')
      assert(JAC_AbsorptionProductGameplay.RecheckPublished(2,TURN))
      assert(STARTS==1 and CHARGES==1 and VIEW().Status=='STARTED')
      assert(CANONICAL({Gold=GOLD[2],Faith=FAITH[2]})==before,'receipt observation charged again')
      local property=CANONICAL(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1'))
      for i=1,5 do JAC_AbsorptionProductGameplay.RecheckPublished(2,TURN) end
      assert(CANONICAL(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1'))==property and STARTS==1 and CHARGES==1)
    "#).exec()?;
    Ok(())
}

#[test]
fn publication_reentry_during_phase_one_does_not_reenter_admission() -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(
        r#"
      REGISTER();READY()
      local begin=JAC_AbsorptionFlowGameplay.Begin
      JAC_AbsorptionFlowGameplay.Begin=function(plan)
        assert(not JAC_AbsorptionProductGameplay.RecheckPublished(2,TURN))
        return begin(plan)
      end
      assert(JAC_AbsorptionProductGameplay.RecheckPublished(2,TURN))
      assert(STARTS==1 and CHARGES==1 and VIEW().Status=='STARTED')
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn published_operation_delivered_after_the_ui_turn_releases_still_rechecks_ai() -> anyhow::Result<()>
{
    let lua = world(9999.)?;
    lua.load(r#"
      OFFICIAL_SOURCE();HUMAN[2]=false;HUMAN[3]=true;ALIVE[3]=true;READY()
      Players[3].IsTurnActive=function() return false end
      GameEvents.JAC_AbsorptionProduct_Operation.Call(3,{Command='RECHECK_PUBLISHED',Turn=TURN})
      assert(SENDS==1 and GRANTS==1 and POOL==9 and OFFICIAL_ENVOYS==1)
      GameEvents.JAC_AbsorptionProduct_Operation.Call(3,{Command='RECHECK_PUBLISHED',Turn=TURN})
      assert(SENDS==1 and GRANTS==1 and POOL==9 and OFFICIAL_ENVOYS==1)
      assert(not pcall(JAC_AbsorptionProductGameplay.RecheckPublished,2,TURN),'AI forged human publication')
      HUMAN[3]=false;assert(not pcall(JAC_AbsorptionProductGameplay.RecheckPublished,3,TURN))
    "#).exec()?;
    Ok(())
}

#[test]
fn published_native_turn_retries_saved_admission_once_and_preserves_reload() -> anyhow::Result<()> {
    let lua = world(10000.)?;
    lua.load(r#"
      REGISTER();READY();TURN=TURN+1;UI_TURN=TURN-1
      local function event() local handlers={} return {Add=function(f) handlers[#handlers+1]=f end,
        Call=function() for _,f in ipairs(handlers) do f() end end} end
      UI_ENV=setmetatable({Game={GetCurrentGameTurn=function() return UI_TURN end,GetLocalPlayer=function() return 2 end},
        Events={GameCoreEventPublishComplete=event(),LocalPlayerTurnBegin=event(),LoadScreenClose=event()},
        PlayerOperations={EXECUTE_SCRIPT=1},UI={RequestPlayerOperation=function(actor,kind,params)
          assert(actor==2 and kind==1 and params.OnStart=='JAC_AbsorptionProduct_Operation')
          PUBLICATION_SENDS=PUBLICATION_SENDS+1;GameEvents.JAC_AbsorptionProduct_Operation.Call(actor,params)
        end}}, {__index=_G})
      PUBLICATION_SENDS=0
    "#).exec()?;
    lua.load(Repository::checked_in().read(product::NATIVE_READ_UI)?)
        .set_environment(lua.globals().get("UI_ENV")?)
        .exec()?;
    lua.load(r#"
      GameEvents.OnGameTurnStarted.Call();assert(STARTS==0 and CHARGES==0)
      assert(not pcall(JAC_AbsorptionProductGameplay.RecheckPublished,2,TURN-1))
      assert(not pcall(JAC_AbsorptionProductGameplay.RecheckPublished,2,TURN))
      assert(not pcall(JAC_AbsorptionProductGameplay.RecheckPublished,1,TURN))
      assert(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1').PublicationTurns['2']==nil)
      UI_TURN=TURN;UI_ENV.Events.GameCoreEventPublishComplete.Call()
      assert(STARTS==1 and CHARGES==1 and VIEW().Status=='STARTED' and PUBLICATION_SENDS==1)
      SNAPSHOT=CANONICAL(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1'))
      for i=1,5 do UI_ENV.Events.GameCoreEventPublishComplete.Call();UI_ENV.Events.LocalPlayerTurnBegin.Call() end
      assert(PUBLICATION_SENDS==2 and STARTS==1 and CHARGES==1 and CANONICAL(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1'))==SNAPSHOT)
    "#).exec()?;
    load(&lua, 10000.)?;
    lua.load(Repository::checked_in().read(product::NATIVE_READ_UI)?)
        .set_environment(lua.globals().get("UI_ENV")?)
        .exec()?;
    lua.load(
        r#"
      UI_ENV.Events.LoadScreenClose.Call()
      assert(PUBLICATION_SENDS==3 and STARTS==1 and CHARGES==1)
      assert(CANONICAL(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1'))==SNAPSHOT)
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn product_ui_uses_native_actor_identity_when_gameplay_properties_are_unpublished(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    for small in [false, true] {
        for human in [false, true] {
            let scene: serde_json::Value =
                serde_json::from_str(&product::acceptance_scenario(&repo, small, human)?)?;
            let lua = Lua::new();
            lua.globals().set("SMALL", small)?;
            lua.load(r#"
              Game={GetLocalPlayer=function() return 22 end,
                GetProperty=function() error('Gameplay properties have not been published to UI') end}
              -- Absorption has removed the source from the living actor list.
              PlayerManager={GetWasEverAliveCount=function() return 23 end,
                GetAliveMajors=function() return {{GetID=function() return 22 end}} end}
              PlayerConfigurations={
                [11]={GetCivilizationTypeName=function() return SMALL and 'CIVILIZATION_JAC_ODA' or 'CIVILIZATION_JAC_TAKEDA' end},
                [22]={GetCivilizationTypeName=function() return SMALL and 'CIVILIZATION_JAC_TAKEDA' or 'CIVILIZATION_JAC_ODA' end}}
              OPENED=0
              local function open(from) assert(from==11);OPENED=OPENED+1 end
              LuaEvents={CityStates_OpenOverview=open,TopPanel_OpenDiplomacyActionView=open}
              ExposedMembers={JAC_AbsorptionProduct={Get=function(from,to)
                assert(from==11 and to==22);return {Status='STARTED'} end}}
            "#).exec()?;
            for step in scene["steps"].as_array().unwrap() {
                let state = step["state_name"].as_str().unwrap_or("");
                if let Some(code) = step["code"].as_str() {
                    if !state.is_empty() && state != "JAC_Absorption_Entry" {
                        assert!(
                            !code.contains("Game:GetProperty"),
                            "UI relies on delayed Gameplay property publication: {code}"
                        );
                    }
                    if code.contains("screen opened") || code.contains("screen reopened") {
                        lua.load(code).exec()?;
                    }
                }
                if let Some(code) = step["condition"]["code"]
                    .as_str()
                    .filter(|_| step["condition"]["state_name"] != "JAC_Absorption_Entry")
                {
                    assert!(
                        !code.contains("Game:GetProperty"),
                        "UI wait relies on Gameplay property publication: {code}"
                    );
                    if code.contains("v.Status=='STARTED'") {
                        assert!(lua.load(code).eval::<bool>()?);
                    }
                }
            }
            assert_eq!(
                lua.globals().get::<i32>("OPENED")?,
                if human {
                    if small {
                        2
                    } else {
                        1
                    }
                } else {
                    0
                }
            );
        }
    }
    Ok(())
}

#[test]
fn every_generated_product_reload_exits_the_running_game_before_loading() -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    for small in [false, true] {
        for human in [false, true] {
            let scene: serde_json::Value =
                serde_json::from_str(&product::acceptance_scenario(&repo, small, human)?)?;
            let steps = scene["steps"].as_array().unwrap();
            let mut count = 0;
            for (index, step) in steps.iter().enumerate() {
                if step["action"] == "load" {
                    count += 1;
                    assert_eq!(steps[index - 1]["action"], "exit_to_menu");
                    assert_eq!(steps[index - 2]["action"], "save");
                    assert_eq!(steps[index - 2]["name"], step["name"]);
                }
            }
            assert_eq!(count, 3);
        }
    }
    Ok(())
}

#[test]
fn official_envoy_permissions_are_read_in_ui_context_and_not_from_gameplay_getters(
) -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(r#"
      OFFICIAL_SOURCE();HUMAN[2]=false
      ENVOY_UI_CONTEXT=setmetatable({Players={}}, {__index=_G})
      for id,player in pairs(Players) do
        ENVOY_UI_CONTEXT.Players[id]={GetInfluence=player.GetInfluence,GetDiplomacy=player.GetDiplomacy,GetDiplomaticAI=player.GetDiplomaticAI}
        local influence=player.GetInfluence
        player.GetInfluence=function() local value=influence()
          value.CanGiveInfluence=function() error('UI sender eligibility called in Gameplay') end
          value.CanGiveTokensToPlayer=function() error('UI recipient eligibility called in Gameplay') end
          return value end
      end
    "#).exec()?;
    let context: mlua::Table = lua.globals().get("ENVOY_UI_CONTEXT")?;
    lua.load(Repository::checked_in().read(product::NATIVE_READ_UI)?)
        .set_environment(context)
        .exec()?;
    lua.load("ENVOY(0);assert(POOL==9 and SENDS==1 and GRANTS==1 and OFFICIAL_ENVOYS==1)")
        .exec()?;
    Ok(())
}

#[test]
fn ai_envoy_budget_survives_exceptions_repeated_ticks_and_reload() -> anyhow::Result<()> {
    for failure in ["none", "charge", "grant", "saved-grant"] {
        let lua = world(9999.)?;
        lua.globals().set("FAILURE", failure)?;
        lua.load(r#"
          OFFICIAL_SOURCE();HUMAN[2]=false;ALIVE[3]=true
          Players[3]=JAC_AbsorptionGameplay.Rules.Copy(Players[1])
          PlayerConfigurations[3]={GetCivilizationTypeName=function() return 'CIVILIZATION_SINGAPORE' end,
            GetLeaderTypeName=function() return 'LEADER_MINOR_CIV_SINGAPORE' end}
          Game:SetProperty('JAC_COUNTRY_RELATIONS_V1',{Version=1,
            Countries={VATICAN_CITY={Reviewed=true},SINGAPORE={Reviewed=true}},Relations={}})
          RECEIVED={[1]=0,[3]=0}
          for _,actor in ipairs({1,3}) do local id=actor
            Players[id].GetInfluence=function() return {CanReceiveInfluence=function() return true end,
              GetTokensReceived=function(_,sender) assert(sender==2);return RECEIVED[id] end} end
          end
          Players[2].GetInfluence=function() return {GetTokensToGive=function() return POOL end,
            CanGiveInfluence=function() return true end,CanGiveTokensToPlayer=function() return true end,
            ChangeTokensToGive=function(_,n) POOL=POOL+n;SENDS=SENDS+1
              if FAILURE=='charge' then FAILURE='none';error('charge changed then threw') end end,
            GiveFreeTokenToPlayer=function(_,recipient) RECEIVED[recipient]=RECEIVED[recipient]+1;GRANTS=GRANTS+1
              if FAILURE=='grant' then FAILURE='none';error('grant changed then threw') end end} end
          if FAILURE=='saved-grant' then local write=Game.SetProperty;local once=true
            Game.SetProperty=function(self,key,value)
              write(self,key,value)
              if once and key=='JAC_ABSORPTION_PRODUCT_V1' then
                for _,op in pairs(value.Operations) do if op.EnvoyReceipt and op.EnvoyReceipt.Done then
                  once=false;error('grant receipt saved then threw') end end
              end
            end
          end
          READY();JAC_AbsorptionProductGameplay.Tick()
          assert(SENDS==1 and POOL==9 and RECEIVED[3]==0,'AI spent a second token after an interrupted first country')
          JAC_AbsorptionProductGameplay.Tick();JAC_AbsorptionProductGameplay.Tick()
          assert(SENDS==1 and GRANTS==1 and RECEIVED[1]==1 and RECEIVED[3]==0,'same turn replay spent another envoy')
        "#).exec()?;
        load(&lua, 9999.)?;
        lua.load(r#"
          JAC_AbsorptionProductGameplay.Tick()
          assert(SENDS==1 and GRANTS==1 and POOL==9 and RECEIVED[3]==0,'reload forgot the current turn budget')
          TURN=TURN+1;JAC_AbsorptionProductGameplay.Tick()
          assert(SENDS==2 and GRANTS==2 and POOL==8,'next actual turn did not release the budget')
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn delayed_ai_envoy_completion_uses_the_recovery_turn_budget() -> anyhow::Result<()> {
    let lua = world(9999.)?;
    lua.load(
        r#"
      OFFICIAL_SOURCE();HUMAN[2]=false;FAIL_ENVOY=true
      assert(not pcall(ENVOY,0));assert(SENDS==1 and GRANTS==0)
      TURN=TURN+1;READY();JAC_AbsorptionProductGameplay.Tick()
      assert(SENDS==1 and GRANTS==1 and OFFICIAL_ENVOYS==1 and POOL==9)
      assert(not pcall(ENVOY,1),'recovery and another grant happened in the same turn')
      TURN=TURN+1;ENVOY(1);assert(SENDS==2 and GRANTS==2 and POOL==8)
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn envoy_save_failure_never_recharges_or_invents_an_unobserved_official_grant() -> anyhow::Result<()>
{
    for official in [false, true] {
        for phase in ["ChargeDone", "Done"] {
            for after in [false, true] {
                let lua = world(9999.)?;
                lua.globals().set("OFFICIAL_ROUTE", official)?;
                lua.globals().set("FAIL_PHASE", phase)?;
                lua.globals().set("FAIL_AFTER_SAVE", after)?;
                lua.load(r#"
                  if OFFICIAL_ROUTE then OFFICIAL_SOURCE() end;HUMAN[2]=false
                  local write=Game.SetProperty;local once=true
                  Game.SetProperty=function(self,key,value)
                    local op=key=='JAC_ABSORPTION_PRODUCT_V1' and value.Operations['2:envoy0']
                    if once and op and op.EnvoyReceipt and op.EnvoyReceipt[FAIL_PHASE] then
                      once=false;if FAIL_AFTER_SAVE then write(self,key,value) end;error('save failed at '..FAIL_PHASE)
                    end
                    return write(self,key,value)
                  end
                  assert(not pcall(ENVOY,0));assert(POOL==9 and SENDS==1)
                "#).exec()?;
                load(&lua, 9999.)?;
                lua.load(r#"
                  local recovered=pcall(ENVOY,0)
                  local ambiguous=not FAIL_AFTER_SAVE and (FAIL_PHASE=='ChargeDone' or OFFICIAL_ROUTE)
                  assert(recovered==not ambiguous and POOL==9 and SENDS==1)
                  assert(GRANTS==(OFFICIAL_ROUTE and (FAIL_PHASE=='Done' or FAIL_AFTER_SAVE) and 1 or 0))
                  if ambiguous then
                    assert(not pcall(CMD,'SEND_ENVOY','another',{ExpectedEnvoys=VIEW().Envoys.Envoys}))
                    READY();REGISTER();GameEvents.OnGameTurnStarted.Call();assert(STARTS==0 and CHARGES==0)
                  else
                    ENVOY(0);assert(VIEW().Envoys.Envoys==1 and POOL==9 and SENDS==1)
                  end
                "#).exec()?;
            }
        }
    }
    Ok(())
}

#[test]
fn official_envoy_identity_and_ambiguous_engine_changes_block_without_another_mutation(
) -> anyhow::Result<()> {
    for change in ["pool", "received", "sender", "recipient", "route", "bonus"] {
        let lua = world(9999.)?;
        lua.globals().set("CHANGE", change)?;
        lua.load(r#"
          OFFICIAL_SOURCE();HUMAN[2]=false
          if CHANGE=='bonus' then GRANT_DELTA=2 else FAIL_ENVOY=true end
          assert(not pcall(ENVOY,0));assert(POOL==9 and SENDS==1)
          if CHANGE=='pool' then POOL=8
          elseif CHANGE=='received' then OFFICIAL_ENVOYS=1
          elseif CHANGE=='sender' then PlayerConfigurations[2].GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_TAKEDA' end
          elseif CHANGE=='recipient' then PlayerConfigurations[1].GetLeaderTypeName=function() return 'LEADER_MINOR_CIV_SINGAPORE' end
          elseif CHANGE=='route' then OFFICIAL=false end
          local pool,received,grants=POOL,OFFICIAL_ENVOYS,GRANTS
          assert(not pcall(ENVOY,0));assert(POOL==pool and OFFICIAL_ENVOYS==received and SENDS==1 and GRANTS==grants and STARTS==0)
          assert(not Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1').Operations['2:envoy0'].EnvoyReceipt.Done)
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn relation_declarations_players_and_saved_inputs_have_identical_canonical_order(
) -> anyhow::Result<()> {
    let catalog = product::catalog(&Repository::checked_in())?;
    let mut reverse = catalog.clone();
    reverse.runtime["InitialRelations"]["relations"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_eq!(
        product::render_gameplay(&catalog),
        product::render_gameplay(&reverse)
    );
    let mut canonical = Vec::new();
    for reverse in [false, true] {
        let lua = world(10000.)?;
        lua.globals().set("REVERSE", reverse)?;
        lua.load(r#"
          READY();ALIVE[3]=true;PlayerConfigurations[3].GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_DATE' end
          PlayerConfigurations[3].GetLeaderTypeName=function() return 'LEADER_JAC_DATE_TANEMUNE' end
          ALLIES['1:3']=true
          local a={ID='a',From='ROKKAKU',To='ODA',Kind='alliance',Active=true}
          local z={ID='z',From='DATE',To='ROKKAKU',Kind='federation',Active=true}
          local relations=REVERSE and {z,a} or {a,z}
          if REVERSE then
            local original=pairs
            pairs=function(t) local list={};for k in original(t) do list[#list+1]=k end
              table.sort(list,function(a,b) return tostring(a)>tostring(b) end)
              local i=0;return function() i=i+1;local k=list[i];if k~=nil then return k,t[k] end end
            end
            local players={};players[3]=Players[3];players[2]=Players[2];players[1]=Players[1];Players=players
          end
          Game:SetProperty('JAC_COUNTRY_RELATIONS_V1',{Version=1,Countries={ROKKAKU={Reviewed=true}},Relations=relations})
          REGISTER();local details=VIEW().Rows[#VIEW().Rows].Details
          assert(details[1].ID=='a' and details[2].ID=='alliance:3' and details[3].ID=='z')
          BLOCKED=CANONICAL(N.GetOffer(VIEW().OfferID))
          ALLIES['1:3']=nil;a.Active=false;z.Active=false
          Game:SetProperty('JAC_COUNTRY_RELATIONS_V1',{Version=1,Countries={ROKKAKU={Reviewed=true}},Relations=relations})
          GameEvents.OnGameTurnStarted.Call();assert(STARTS==1)
          FINAL=CANONICAL(N.Get('leader-offer',VIEW().OfferID))..CANONICAL(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1'))
        "#).exec()?;
        canonical.push((
            lua.globals().get::<String>("BLOCKED")?,
            lua.globals().get::<String>("FINAL")?,
        ));
    }
    assert_eq!(canonical[0], canonical[1]);
    Ok(())
}

#[test]
fn consumed_cost_save_failures_replay_without_debit_credit_or_replacing_paid_legs(
) -> anyhow::Result<()> {
    for after in [false, true] {
        let lua = world(10000.)?;
        lua.globals().set("SAVE_AFTER", after)?;
        lua.load(r#"
          REGISTER();READY();GameEvents.OnGameTurnStarted.Call();RESULT()
          COST_GOLD,COST_FAITH=GOLD[2],FAITH[2]
          OLD_PAID=CANONICAL(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1').Escrows[VIEW().OfferID].CollectGold)
          local write=Game.SetProperty;local once=true
          Game.SetProperty=function(self,key,value)
            local escrow=key=='JAC_ABSORPTION_PRODUCT_V1' and value.Escrows[VIEW().OfferID]
            if once and escrow and escrow.Status=='CONSUMED' then
              once=false;if SAVE_AFTER then write(self,key,value) end;error('consumption save failed')
            end
            return write(self,key,value)
          end
          GameEvents.OnGameTurnStarted.Call()
          assert(GOLD[2]==COST_GOLD and FAITH[2]==COST_FAITH and REFUNDS==0 and CHARGES==1)
        "#).exec()?;
        load(&lua, 10000.)?;
        lua.load(r#"
          GameEvents.OnGameTurnStarted.Call();GameEvents.OnGameTurnStarted.Call()
          local escrow=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1').Escrows[VIEW().OfferID]
          assert(escrow.Status=='CONSUMED' and CANONICAL(escrow.CollectGold)==OLD_PAID)
          assert(GOLD[2]==COST_GOLD and FAITH[2]==COST_FAITH and REFUNDS==0 and CHARGES==1 and STARTS==1)
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn false_results_wrong_payers_and_partial_currency_proofs_never_finish_consumption(
) -> anyhow::Result<()> {
    for invalid in [
        "no-final-rolls",
        "no-transaction",
        "incomplete-transaction",
        "wrong-flow",
        "wrong-payer",
        "gold",
        "faith",
        "partial",
        "error",
        "blocked",
    ] {
        let lua = world(10000.)?;
        lua.globals().set("INVALID", invalid)?;
        lua.load(r#"
          REGISTER();READY();GameEvents.OnGameTurnStarted.Call();local flow=RESULT()
          local s=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1');local escrow=s.Escrows[VIEW().OfferID]
          if INVALID=='no-final-rolls' then flow.FinalRollsComplete=false
          elseif INVALID=='no-transaction' then TRANSACTIONS[flow.ID]=nil
          elseif INVALID=='incomplete-transaction' then TRANSACTIONS[flow.ID].Status='STAGED'
          elseif INVALID=='wrong-flow' then flow.Request.To=3
          elseif INVALID=='wrong-payer' then escrow.To=3
          elseif INVALID=='gold' then escrow.CollectGold.Done=false
          elseif INVALID=='faith' then escrow.CollectFaith.After=escrow.CollectFaith.After+1
          elseif INVALID=='partial' then escrow.Status='COLLECTING'
          elseif INVALID=='error' then flow.Error='unverified transfer'
          elseif INVALID=='blocked' then flow.Phase='BLOCKED' end
          Game:SetProperty('JAC_ABSORPTION_PRODUCT_V1',s)
          Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows=FLOWS})
          local gold,faith=GOLD[2],FAITH[2]
          GameEvents.OnGameTurnStarted.Call()
          local e=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1').Escrows[VIEW().OfferID]
          assert(e.Status~='CONSUMED' and not e.Consumption)
          assert(GOLD[2]==gold and FAITH[2]==faith and CHARGES==1 and REFUNDS==0 and STARTS==1)
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn version_one_collect_and_cancel_refund_journals_migrate_with_exact_paid_legs(
) -> anyhow::Result<()> {
    for refund in [false, true] {
        let lua = world(10000.)?;
        lua.globals().set("REFUNDING", refund)?;
        lua.load(r#"
          REGISTER();READY();FAIL_CHARGE=true;GameEvents.OnGameTurnStarted.Call()
          LEGACY_ID=VIEW().OfferID
          if REFUNDING then FAIL_REFUND=true;assert(not pcall(CMD,'CANCEL','legacy-cancel',{OfferID=LEGACY_ID})) end
          local s=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1');s.Version=1;s.SourceVersion=nil;s.Escrows[LEGACY_ID].SourceVersion=nil
          Game:SetProperty('JAC_ABSORPTION_PRODUCT_V1',s)
        "#).exec()?;
        load(&lua, 10000.)?;
        lua.load(r#"
          GameEvents.OnGameTurnStarted.Call()
          local s=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1');local escrow=s.Escrows[LEGACY_ID]
          assert(s.Version==2 and s.SourceVersion==1 and escrow.SourceVersion==1)
          if REFUNDING then assert(escrow.Status=='RETURNED' and GOLD[2]==5000 and FAITH[2]==1000 and REFUNDS==1 and STARTS==0)
          else assert(escrow.Status=='HELD' and STARTS==1) end
        "#).exec()?;
    }
    Ok(())
}

#[test]
fn legacy_result_refund_records_existing_credit_and_consumes_remaining_amount_without_new_currency_calls(
) -> anyhow::Result<()> {
    for legacy in [
        "not-called",
        "gold-mutated",
        "gold-done",
        "both-done",
        "ambiguous",
    ] {
        let lua = world(10000.)?;
        lua.globals().set("LEGACY", legacy)?;
        lua.load(r#"
          REGISTER();READY();GameEvents.OnGameTurnStarted.Call();RESULT()
          local s=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1');local e=s.Escrows[VIEW().OfferID]
          s.Version=1;s.SourceVersion=nil;e.SourceVersion=nil;e.Status='RETURNING';e.ReturnReason='VERIFIED_RESULT'
          e.ReturnGoldAmount,e.ReturnFaithAmount=e.Gold,e.Faith
          e.ReturnGold={Before=GOLD[2],After=GOLD[2]+e.Gold}
          if LEGACY~='not-called' then GOLD[2]=e.ReturnGold.After end
          if LEGACY=='gold-done' or LEGACY=='both-done' then e.ReturnGold.Done=true end
          if LEGACY=='both-done' then e.ReturnFaith={Before=FAITH[2],After=FAITH[2]+e.Faith,Done=true};FAITH[2]=e.ReturnFaith.After;e.Status='RETURNED' end
          if LEGACY=='ambiguous' then GOLD[2]=GOLD[2]+1 end
          LEGACY_GOLD,LEGACY_FAITH=GOLD[2],FAITH[2]
          Game:SetProperty('JAC_ABSORPTION_PRODUCT_V1',s)
        "#).exec()?;
        load(&lua, 10000.)?;
        lua.load(r#"
          GameEvents.OnGameTurnStarted.Call();GameEvents.OnGameTurnStarted.Call()
          local s=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1');local e=s.Escrows[VIEW().OfferID]
          assert(s.Version==2 and e.SourceVersion==1 and GOLD[2]==LEGACY_GOLD and FAITH[2]==LEGACY_FAITH and REFUNDS==0 and CHARGES==1)
          if LEGACY=='ambiguous' then assert(e.Status=='RETURNING' and not e.Consumption)
          else
            assert(e.Status==(LEGACY=='not-called' and 'CONSUMED' or 'LEGACY_SETTLED'))
            assert(e.Consumption.Gold==(LEGACY=='not-called' and e.Gold or 0))
            assert(e.Consumption.Faith==(LEGACY=='both-done' and 0 or e.Faith))
          end
        "#).exec()?;
    }
    Ok(())
}
