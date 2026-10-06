//! Ordinary Gameplay source. All mutations run in the synchronized context.
pub(super) const LUA: &str = r#"
local A,N,F=assert(JAC_AbsorptionGameplay),assert(JAC_AbsorptionAdmissionGameplay),assert(JAC_AbsorptionFlowGameplay)
local R=A.Rules
local P={}
local KEY='JAC_ABSORPTION_PRODUCT_V1'
local RELATIONS='JAC_COUNTRY_RELATIONS_V1'
local busy=false
local function State()
  local s=R.Copy(Game:GetProperty(KEY) or {Version=2,Pairs={},Escrows={},Envoys={},Operations={}})
  assert((s.Version==1 or s.Version==2) and type(s.Pairs)=='table' and type(s.Escrows)=='table'
    and type(s.Envoys)=='table' and type(s.Operations)=='table','unsupported product save')
  s.AiEnvoyTurns=s.AiEnvoyTurns or {}
  assert(type(s.AiEnvoyTurns)=='table','unsupported AI envoy turn journal')
  -- Preserve every old payment/refund intent. New settlement enums always
  -- save as v2, which the v1 writer rejects before any currency operation.
  local sourceVersion=s.Version
  s.SourceVersion=s.SourceVersion or sourceVersion;s.Version=2
  for _,escrow in pairs(s.Escrows) do escrow.SourceVersion=escrow.SourceVersion or sourceVersion end
  return s
end
local function Save(s)
  -- Keep unchanged waiting/replayed receipts read-only across every caller.
  -- The decision and currency journals still save each actual transition.
  if not R.Equal(Game:GetProperty(KEY),s) then Game:SetProperty(KEY,R.Copy(s)) end
end
local function Num(v) return type(v)=='number' and v==v and v>=0 and v<math.huge end
local function Integer(v) return Num(v) and v==math.floor(v) end
local function Id(v) return type(v)=='string' and v~='' and #v<=256 end
local function Pair(from,to) return tostring(from)..':'..tostring(to) end
function P.Country(actor)
  local p,config=Players[actor],PlayerConfigurations[actor]
  if not p or not config or not config.GetCivilizationTypeName or not config.GetLeaderTypeName then return nil end
  local civilization,leader=config:GetCivilizationTypeName(),config:GetLeaderTypeName()
  local profile=C.Profiles[tostring(civilization)..':'..tostring(leader)]
  if not profile then return nil end
  local country=R.Copy(assert(C.Countries[profile.CountryID]))
  country.Actor=actor;country.Profile=R.Copy(profile)
  -- Character overrides, when present, are ordinary persisted person data.
  local roster=A.GetRoster(actor)
  local character=roster and roster['leader:'..actor]
  if character and character.Personality then
    assert(C.Personalities[character.Personality],'unknown Japanese personality in person record')
    country.Profile.Personality=character.Personality
    country.Profile.PersonalityLabel=C.Personalities[character.Personality].Label
  end
  if character and character.Trait then
    assert(character.Trait==profile.Trait,'lifetime trait cannot change')
  end
  return country
end
local function Major(actor)
  return Players[actor] and Players[actor]:IsMajor() == true
end
local function Actors(majorOnly)
  local ids={}
  for actor,player in pairs(Players) do
    if type(actor)=='number' and player:IsAlive() and (not majorOnly or Major(actor)) and P.Country(actor) then ids[#ids+1]=actor end
  end
  table.sort(ids);return ids
end
local function Balance(actor,currency)
  local p=assert(Players[actor]);local value
  if currency=='Gold' then value=p:GetTreasury():GetGoldBalance()
  else value=A.PlayerFaithBalance(actor) end
  assert(Num(value),'live currency balance unavailable');return value
end
local function Change(actor,currency,amount)
  if currency=='Gold' then Players[actor]:GetTreasury():ChangeGoldBalance(amount)
  else Players[actor]:GetReligion():ChangeFaithBalance(amount) end
end
local function NativeDiplomacy(from,to)
  local api=assert(ExposedMembers.JAC_AbsorptionProductNativeRead,'native diplomacy UI reader unavailable')
  local data=R.Copy(api.GetDiplomacy(from,to))
  assert(data and data.From==from and data.To==to and data.Turn==Game.GetCurrentGameTurn()
    and Id(data.StateType) and Num(data.Visibility),'stale or invalid native diplomacy read')
  return data
end
local function Metric(actor,metric,other)
  local p=assert(Players[actor]);local value=0
  if metric=='SCIENCE_YIELD' then value=p:GetTechs():GetScienceYield()
  elseif metric=='FAITH_YIELD' then value=p:GetReligion():GetFaithYield()
  elseif metric=='ACCESS' then value=NativeDiplomacy(actor,other).Visibility
  elseif metric=='POPULATION' or metric=='FOOD_YIELD' then
    for _,city in p:GetCities():Members() do
      local v=metric=='POPULATION' and city:GetPopulation() or city:GetYield(YieldTypes.FOOD)
      assert(Num(v),'live city measure unavailable');value=value+v
    end
  elseif metric=='COMBAT_UNITS' or metric=='PEAK_COMBAT' then
    for _,unit in p:GetUnits():Members() do
      local info=assert(GameInfo.Units[unit:GetType()],'unit definition missing')
      local strength=math.max(info.Combat or 0,info.RangedCombat or 0)
      if metric=='COMBAT_UNITS' then if strength>0 then value=value+1 end
      else value=value+strength end
    end
  elseif metric=='TECHNOLOGIES' then
    for info in GameInfo.Technologies() do if p:GetTechs():HasTech(info.Index) then value=value+1 end end
  else error('unknown product measure') end
  assert(Num(value),'live product measure unavailable');return value
end
local function DiplomaticSituation(from,to)
  local d=Players[to]:GetDiplomacy()
  if d:IsAtWarWith(from) then return 'war' end
  if d:HasAllied(from) then return 'allied' end
  local info=NativeDiplomacy(from,to)
  if info.StateType=='DIPLO_STATE_DENOUNCED' then return 'denounced' end
  if info.StateType=='DIPLO_STATE_DECLARED_FRIEND' then return 'friend' end
  if info.StateType=='DIPLO_STATE_UNFRIENDLY' then return 'unfriendly' end
  if info.StateType=='DIPLO_STATE_FRIENDLY' then return 'friendly' end
  return 'neutral'
end
local function ActorForCountry(id)
  local found
  for _,actor in ipairs(Actors()) do if P.Country(actor).ID==id then
    assert(not found,'ambiguous country/actor mapping');found=actor
  end end
  return found
end
local function RelationState(from,to)
  local country=assert(P.Country(from),'product country missing')
  local reviewed=country.RelationsReviewed
  local rows={}
  -- A real saved relationship ledger supersedes catalog review only with its
  -- explicit country review. Missing or unreviewed entries stay unreviewed.
  local ledger=Game:GetProperty(RELATIONS)
  if ledger then
    assert(ledger.Version==1 and type(ledger.Countries)=='table' and type(ledger.Relations)=='table','unsupported country relationship ledger')
    reviewed=ledger.Countries[country.ID] and ledger.Countries[country.ID].Reviewed==true or false
    for _,relation in pairs(ledger.Relations) do
      assert(Id(relation.ID) and Id(relation.From) and Id(relation.To)
        and type(relation.Active)=='boolean' and Id(relation.Kind),'unconfirmed relationship record')
      if relation.Active and (relation.From==country.ID or relation.To==country.ID) then
        rows[#rows+1]={ID=relation.ID,Met=false,Label=relation.Kind..' '..relation.From..' / '..relation.To}
      end
    end
  end
  -- Japanese vassal obligations are represented by the accepted initial
  -- alliance. Its expiry is read from Gameplay, never from a UI checkbox.
  local historic=Game:GetProperty('JAC_JAPAN_1534_RELATIONS')
  local startState=Game:GetProperty('JAC_JAPAN_START_STATE')
  if startState and not historic then reviewed=false end
  if historic and (historic.Failed or 0)>0 then reviewed=false end
  for _,relation in ipairs(C.InitialRelations.relations or {}) do
    if relation.type=='vassal' and (relation.a==country.ID or relation.b==country.ID) then
      -- Field names are validated by the Rust generator's catalog tests.
      local a,b=relation.a,relation.b
      local small,large=a<b and a or b,a<b and b or a
      local start=Game:GetProperty('JAC_1534_ALLIANCE_'..small..'-'..large)
      local aActor,bActor=ActorForCountry(a),ActorForCountry(b)
      if historic and aActor and bActor and start==nil then reviewed=false end
      if type(start)=='number' then
        rows[#rows+1]={ID=relation.id,Met=false,Label='LOC_JAC_ADMISSION_VASSAL',LabelArgs={C.Countries[a].Name,C.Countries[b].Name}}
      end
    end
  end
  -- Current third-party alliances remain prerequisites even after initial
  -- relationships expire. The other party is not silently discarded.
  for actor,p in pairs(Players) do
    if type(actor)=='number' and actor~=from and actor~=to and p:IsAlive()
      and Players[from]:GetDiplomacy():HasAllied(actor) then
      local other=P.Country(actor)
      rows[#rows+1]={ID='alliance:'..actor,Met=false,Label='LOC_JAC_ADMISSION_ALLIANCE',LabelArgs={other and other.Name or 'LOC_JAC_ADMISSION_OTHER_COUNTRY'}}
    end
  end
  local met=reviewed
  local seen={};for _,row in ipairs(rows) do assert(not seen[row.ID],'duplicate relationship identity');seen[row.ID]=true end
  table.sort(rows,function(a,b) return a.ID<b.ID end)
  for _,row in ipairs(rows) do met=met and row.Met end
  return {Reviewed=reviewed,Met=met,Rows=rows,Source=country.RelationSource}
end
local function OwnedTiles(from)
  local count=0
  for i=0,Map.GetPlotCount()-1 do local plot=Map.GetPlotByIndex(i)
    if plot and plot:GetOwner()==from then count=count+1 end
  end
  return count
end
local function Occupied(actor,ownID)
  return N.IsOccupied(actor,ownID)
end
local function OfficialEnvoyEligible(from,to)
  local api=assert(ExposedMembers.JAC_AbsorptionProductNativeRead,'native envoy UI reader unavailable')
  local data=R.Copy(api.GetEnvoyEligibility(from,to))
  assert(data and data.From==from and data.To==to and data.Turn==Game.GetCurrentGameTurn()
    and type(data.CanGive)=='boolean' and type(data.CanGiveTo)=='boolean','stale or invalid native envoy read')
  return data.CanGive and data.CanGiveTo
end
local function Envoys(from,to)
  local country=assert(P.Country(from),'envoy target not mapped')
  assert(country.AreaKm2<10000,'envoys are for small countries')
  local influence=Players[from]:GetInfluence()
  if not Major(from) then
    assert(influence and influence:CanReceiveInfluence(),'official minor cannot receive influence')
    local count=influence:GetTokensReceived(to);assert(Integer(count),'official envoy count unavailable')
    return {Envoys=count,Eligible=OfficialEnvoyEligible(from,to),Route='OFFICIAL'}
  end
  -- FULL_CIV never claims to receive official influence. It uses its own
  -- synchronized assignments, paid from the same official sender token pool.
  local entry=State().Envoys[Pair(from,to)]
  if entry then assert(entry.CountryID==country.ID and entry.CivilizationType==country.Profile.CivilizationType,'envoy country actor changed') end
  return {Envoys=entry and entry.Count or 0,Eligible=true,Route='PRODUCT'}
end
local function EnvoyIdentity(from,to,route)
  local country=assert(P.Country(from))
  local config=assert(PlayerConfigurations[to])
  return {From=from,To=to,Route=route,CountryID=country.ID,
    CivilizationType=country.Profile.CivilizationType,LeaderType=country.Profile.LeaderType,
    SenderCivilizationType=config:GetCivilizationTypeName(),SenderLeaderType=config:GetLeaderTypeName()}
end
local function PendingEnvoy(to,except)
  for key,op in pairs(State().Operations) do
    if key~=except and op.Identity.Command=='SEND_ENVOY' and op.Identity.To==to
      and not op.Done and not (op.EnvoyReceipt and op.EnvoyReceipt.Done) then return true end
  end
  return false
end
local function AiEnvoyUsed(to,except)
  local used=State().AiEnvoyTurns[tostring(to)]
  return used and used.Turn==Game.GetCurrentGameTurn() and used.Operation~=except
end
local function Input(from,to)
  local country=assert(P.Country(from),'country/leader is not registered')
  assert(ActorForCountry(country.ID)==from,'country is not uniquely bound to the current actor')
  local target=assert(Players[to],'receiving actor missing')
  assert(from~=to and Players[from]:IsAlive() and target:IsAlive(),'admission actor unavailable')
  assert(Major(to),'admission recipient must be a major civilization')
  assert(not Players[from]:IsHuman(),'current absorption engine requires an AI source')
  assert(target:GetDiplomacy():HasMet(from),'admission target has not been met')
  local situation=DiplomaticSituation(from,to)
  return {Country=country,From=from,To=to,Version=C.Version,Situation=situation,
    FreeEpoch=math.floor(Game.GetCurrentGameTurn()/C.Balance.free_change_turns)}
end
local function Terms(input)
  local c=input.Country
  local personality=C.Personalities[c.Profile.Personality]
  local trait=C.Traits[c.Profile.Trait]
  local merger=c.AreaKm2>=10000
  local scale=personality.scale_bps/10000*C.Balance.diplomacy_bps[input.Situation]/10000
  if personality.stability=='changing' then scale=scale*({0.9,1,1.1})[input.FreeEpoch%3+1] end
  local source=Metric(input.From,trait.metric,input.To)
  local minimum=math.ceil(math.max(C.Balance.metric_minima[trait.metric],source*(merger and C.Balance.merger_ratio_bps or C.Balance.metric_ratio_bps)/10000)*scale)
  if trait.metric=='ACCESS' then minimum=math.min(4,minimum) end
  local demands={{ID='leader:'..trait.metric,Kind='PRODUCT_METRIC',Metric=trait.metric,Minimum=minimum,Label=trait.label}}
  local deposit={Gold=0,Faith=0}
  if personality.deposit~='none' then
    deposit.Gold=math.ceil(math.max(merger and C.Balance.merger_gold_deposit or C.Balance.small_gold_deposit,
      Balance(input.From,'Gold')*C.Balance.deposit_source_share_bps/10000)*scale)
    if personality.deposit=='gold_faith' then
      deposit.Faith=math.ceil(math.max(merger and C.Balance.merger_faith_deposit or C.Balance.small_faith_deposit,
        Balance(input.From,'Faith')*C.Balance.deposit_source_share_bps/10000)*scale)
      demands[#demands+1]={ID='reserve:gold',Kind='GOLD_MINIMUM',Minimum=C.Balance.reserve_gold,Label='LOC_JAC_ADMISSION_RESERVE'}
    end
    demands[#demands+1]={ID='deposit',Kind='PRODUCT_DEPOSIT',Gold=deposit.Gold,Faith=deposit.Faith,Label='LOC_JAC_ADMISSION_DEPOSIT'}
  end
  demands[#demands+1]={ID='identity',Kind='PRODUCT_IDENTITY',Profile=R.Copy(c.Profile),AreaKm2=c.AreaKm2,Label='LOC_JAC_ADMISSION_IDENTITY'}
  demands[#demands+1]={ID='terms',Kind='PRODUCT_CONTEXT',Input=R.Copy(input),Label='LOC_JAC_ADMISSION_TERMS'}
  table.sort(demands,function(a,b) return a.ID<b.ID end)
  return {Input=R.Copy(input),Requirements=demands,Deposit=deposit,Relationship=RelationState(input.From,input.To)}
end
local function SameTermsInput(a,b)
  if not R.Equal(a.Country,b.Country) or a.Version~=b.Version or a.From~=b.From or a.To~=b.To then return false end
  local stability=C.Personalities[a.Country.Profile.Personality].stability
  return stability=='fixed' or a.Situation==b.Situation and (stability~='changing' or a.FreeEpoch==b.FreeEpoch)
end
local function PaidLeg(escrow,currency)
  if escrow[currency]==0 then return true end
  local intent=escrow['Collect'..currency]
  return intent and intent.Done==true and Num(intent.Before) and Num(intent.After)
    and intent.Before-intent.After==escrow[currency] or false
end
local function Evaluate(from,to,demand,id,afterDeposit)
  if demand.Kind=='PRODUCT_METRIC' then
    local current=Metric(to,demand.Metric,from);return current>=demand.Minimum,current,demand.Minimum
  elseif demand.Kind=='GOLD_MINIMUM' then
    local current=Balance(to,'Gold')-(afterDeposit or 0);return current>=demand.Minimum,current,demand.Minimum
  elseif demand.Kind=='PRODUCT_IDENTITY' then
    local c=P.Country(from);return c and R.Equal(c.Profile,demand.Profile) and c.AreaKm2==demand.AreaKm2 or false,nil,nil
  elseif demand.Kind=='PRODUCT_CONTEXT' then
    if not demand.OfferID then return SameTermsInput(demand.Input,Input(from,to)),nil,nil end
    local pair=State().Pairs[Pair(from,to)]
    return pair~=nil and pair.ID==demand.OfferID and not pair.Cancelled
      and SameTermsInput(demand.Input,Input(from,to)),nil,nil
  elseif demand.Kind=='PRODUCT_DEPOSIT' then
    local escrow=id and State().Escrows[id]
    return escrow~=nil and (escrow.Status=='HELD' or escrow.Status=='CONSUMED')
      and escrow.ID==id and escrow.From==from and escrow.To==to and PaidLeg(escrow,'Gold') and PaidLeg(escrow,'Faith')
      and escrow.Gold==demand.Gold and escrow.Faith==demand.Faith,
      escrow and escrow.Status or 'NOT_PAID',demand.Gold
  end
  error('unknown product demand')
end
local function NextOffer(pair,terms)
  local seq=(pair.Sequence or 0)+1
  local id='product:'..Pair(terms.Input.From,terms.Input.To)..':'..seq
  local requirements=R.Copy(terms.Requirements)
  for _,d in ipairs(requirements) do d.OfferID=id end
  local c=terms.Input.Country
  return {ID=id,From=terms.Input.From,To=terms.Input.To,AreaKm2=c.AreaKm2,
    Requirements=requirements,RelationRequirements={{ID='relations',Kind='PRODUCT_RELATIONS'}},
    RelationsReviewed=RelationState(terms.Input.From,terms.Input.To).Reviewed,EnvoySource='product',ProductTerms=R.Copy(terms),
    Plan={LeaderProfile=R.Copy(c.Profile),ProductDeposit={OfferID=id,From=terms.Input.From,To=terms.Input.To,
      Gold=terms.Deposit.Gold,Faith=terms.Deposit.Faith,Policy='CONSUME_ON_VERIFIED_RESULT_V2'}}},seq
end
-- Currency journal: save intended before/after values before each engine
-- call. A call which mutates then throws is recovered by its observed value.
-- An unrelated balance is ambiguous and is never charged/refunded again.
local function MoveCurrency(escrow,side,currency,amount)
  if amount==0 then return end
  local key=side..currency
  local s=State();escrow=assert(s.Escrows[escrow.ID])
  local intent=escrow[key]
  if intent and intent.Done then return end
  local current=Balance(escrow.To,currency)
  if not intent then
    local after=current+(side=='Collect' and -amount or amount)
    assert(after>=0,'insufficient actual deposit funds')
    intent={Before=current,After=after};escrow[key]=intent;Save(s)
  end
  if current==intent.Before then Change(escrow.To,currency,intent.After-intent.Before)
  else assert(current==intent.After,'currency journal has an ambiguous engine balance') end
  assert(Balance(escrow.To,currency)==intent.After,'engine did not settle deposit')
  s=State();s.Escrows[escrow.ID][key].Done=true;Save(s)
end
local function Collect(offer)
  local s=State();local escrow=s.Escrows[offer.ID]
  local amounts=offer.ProductTerms.Deposit
  if amounts.Gold==0 and amounts.Faith==0 then return end
  if not escrow then
    assert(Balance(offer.To,'Gold')>=amounts.Gold and Balance(offer.To,'Faith')>=amounts.Faith,'insufficient actual deposit funds')
    escrow={ID=offer.ID,From=offer.From,To=offer.To,Gold=amounts.Gold,Faith=amounts.Faith,Status='COLLECTING',SourceVersion=2}
    s.Escrows[offer.ID]=escrow;Save(s)
  end
  if escrow.Status=='HELD' or escrow.Status=='RETURNED' or escrow.Status=='CONSUMED' or escrow.Status=='LEGACY_SETTLED' then return end
  assert(escrow.Status=='COLLECTING','deposit cannot be collected again')
  MoveCurrency(escrow,'Collect','Gold',escrow.Gold)
  MoveCurrency(escrow,'Collect','Faith',escrow.Faith)
  s=State();s.Escrows[offer.ID].Status='HELD';Save(s)
end
local function Refund(id,reason)
  local s=State();local escrow=s.Escrows[id]
  if not escrow or escrow.Status=='RETURNED' then return end
  if escrow.Status=='COLLECTING' then
    -- Cancellation returns only the legs that actually settled. It must not
    -- charge an unfinished second currency merely to return the first one.
    for _,currency in ipairs({'Gold','Faith'}) do
      local intent=escrow['Collect'..currency]
      local paid=0
      if intent then
        if intent.Done then paid=escrow[currency]
        else
          local current=Balance(escrow.To,currency)
          assert(current==intent.Before or current==intent.After,'ambiguous partially collected deposit')
          if current==intent.After then paid=escrow[currency] end
        end
      end
      escrow['Return'..currency..'Amount']=paid
    end
    escrow.Status,escrow.ReturnReason='RETURNING',reason;Save(s)
  end
  if escrow.Status=='HELD' then
    escrow.ReturnGoldAmount,escrow.ReturnFaithAmount=escrow.Gold,escrow.Faith
    escrow.Status,escrow.ReturnReason='RETURNING',reason;Save(s)
  end
  assert(escrow.Status=='RETURNING','invalid deposit refund phase')
  MoveCurrency(escrow,'Return','Gold',escrow.ReturnGoldAmount)
  MoveCurrency(escrow,'Return','Faith',escrow.ReturnFaithAmount)
  s=State();s.Escrows[id].Status='RETURNED';Save(s)
end
local function Consume(offer,request,flow)
  local s=State();local escrow=s.Escrows[offer.ID]
  local amounts=offer.ProductTerms.Deposit
  if amounts.Gold==0 and amounts.Faith==0 then return end
  assert(escrow and escrow.ID==offer.ID and escrow.From==offer.From and escrow.To==offer.To
    and escrow.Gold==amounts.Gold and escrow.Faith==amounts.Faith,'deposit settlement payer or amounts changed')
  assert(request.Status=='STARTED' and request.Plan and flow.ID==request.FlowID and flow.From==escrow.From and flow.To==escrow.To
    and R.Equal(flow.Request,request.Plan) and flow.Mode==request.Plan.Mode and flow.Phase1Complete and flow.FinalRollsComplete
    and (flow.Phase=='RESULT' or flow.Phase=='DONE')
    and Integer(flow.CompletedTurn) and not flow.Error and not flow.WaitingFor,'deposit needs a verified matching flow result')
  local tx=A.GetState().Transactions[flow.ID]
  assert(tx and tx.Status=='COMPLETE' and tx.ID==flow.ID and tx.From==escrow.From and tx.To==escrow.To and tx.Mode==flow.Mode,
    'deposit needs the complete asset-transfer receipt')
  for _,currency in ipairs({'Gold','Faith'}) do
    local amount=escrow[currency]
    if amount>0 then
      local paid=escrow['Collect'..currency]
      assert(paid and paid.Done and Num(paid.Before) and Num(paid.After) and paid.Before-paid.After==amount,
        'deposit collection leg is not verified')
    end
  end
  if escrow.Status=='CONSUMED' or escrow.Status=='LEGACY_SETTLED' then
    local receipt=escrow.Consumption
    assert(receipt and receipt.FlowID==flow.ID and receipt.From==escrow.From and receipt.To==escrow.To
      and receipt.CompletedTurn==flow.CompletedTurn and receipt.Policy=='CONSUME_ON_VERIFIED_RESULT_V2'
      and receipt.Gold+receipt.LegacyRefunded.Gold==escrow.Gold and receipt.Faith+receipt.LegacyRefunded.Faith==escrow.Faith,
      'deposit consumption receipt changed');return
  end
  assert(escrow.Status=='HELD' or escrow.SourceVersion==1 and escrow.ReturnReason=='VERIFIED_RESULT'
    and (escrow.Status=='RETURNING' or escrow.Status=='RETURNED'),'deposit was not fully held for this result')
  local returned={Gold=0,Faith=0}
  for _,currency in ipairs({'Gold','Faith'}) do
    local amount=escrow[currency]
    -- A prior v1 result-refund may have already credited one currency. Never
    -- reclaim it or credit the other leg. Record the exact legacy exception.
    local intent=escrow['Return'..currency]
    if intent then
      local refund=escrow['Return'..currency..'Amount']
      assert(Num(refund) and refund<=amount and intent.After-intent.Before==refund,'invalid legacy refund leg')
      if intent.Done then returned[currency]=refund
      else
        local current=Balance(escrow.To,currency)
        assert(current==intent.Before or current==intent.After,'ambiguous legacy result refund')
        if current==intent.After then returned[currency]=refund end
      end
    end
  end
  escrow.Consumption={FlowID=flow.ID,From=escrow.From,To=escrow.To,CompletedTurn=flow.CompletedTurn,
    Gold=escrow.Gold-returned.Gold,Faith=escrow.Faith-returned.Faith,LegacyRefunded=returned,
    Policy='CONSUME_ON_VERIFIED_RESULT_V2'}
  escrow.Status=(returned.Gold>0 or returned.Faith>0) and 'LEGACY_SETTLED' or 'CONSUMED'
  Save(s) -- completion only: no new debit and no currency credit
end
local function Refresh(from,to)
  local s=State();local key=Pair(from,to);local pair=s.Pairs[key]
  if not pair then return end
  local offer=assert(N.GetOffer(pair.ID))
  local request=N.Get('leader-offer',pair.ID)
  if request and (request.Status=='STARTED' or request.Plan and not N.CanCancelOffer(pair.ID)) then return end
  if pair.Cancelled then return end
  local input=Input(from,to)
  if SameTermsInput(offer.ProductTerms.Input,input) then
    local reviewed=RelationState(from,to).Reviewed
    if offer.RelationsReviewed~=reviewed then offer.RelationsReviewed=reviewed;N.DefineOffer(offer) end
    return
  end
  -- The new complete snapshot is built and validated before replacing the old
  -- admission. N.ReplaceOffer atomically cancels/publishes in its v2 save.
  local replacement,seq=NextOffer(pair,Terms(input))
  N.ReplaceOffer(pair.ID,replacement)
  Refund(pair.ID,'REPLACED')
  s=State();pair=s.Pairs[key];pair.ID,pair.Sequence=replacement.ID,seq;Save(s)
end
local function CanPrepare(offer,ignoreEnvoys)
  if not Major(offer.To) or PendingEnvoy(offer.To) then return false end
  local request=N.Get('leader-offer',offer.ID)
  local flowID=request and request.FlowID
  if Occupied(offer.From,flowID) or Occupied(offer.To,flowID) then return false end
  if not RelationState(offer.From,offer.To).Met or DiplomaticSituation(offer.From,offer.To)=='war' then return false end
  if offer.AreaKm2<10000 and not ignoreEnvoys then
    local envoys=Envoys(offer.From,offer.To)
    if not envoys.Eligible or envoys.Envoys<math.max(1,OwnedTiles(offer.From)) then return false end
  end
  for _,d in ipairs(offer.Requirements) do
    if d.Kind~='PRODUCT_DEPOSIT' and d.Kind~='PRODUCT_CONTEXT' then
      if not Evaluate(offer.From,offer.To,d,offer.ID,offer.ProductTerms.Deposit.Gold) then return false end
    end
  end
  local escrow=State().Escrows[offer.ID]
  if not escrow then
    if Balance(offer.To,'Gold')<offer.ProductTerms.Deposit.Gold
      or Balance(offer.To,'Faith')<offer.ProductTerms.Deposit.Faith then return false end
  end
  return true
end
function P.Get(from,to)
  local ok,value=pcall(function()
    local s=State();local pair=s.Pairs[Pair(from,to)]
    local offer=pair and N.GetOffer(pair.ID)
    local request=pair and N.Get('leader-offer',pair.ID)
    if request and request.Plan and request.Status~='CANCELLED' then
      -- The absorbed actor may have disappeared in an AI result. Continue
      -- exposing its authoritative frozen receipt rather than reading an
      -- eliminated source or pretending that conditions were re-evaluated.
      local terms=assert(offer.ProductTerms)
      local rows={}
      for _,d in ipairs(offer.Requirements) do
        local escrow=d.Kind=='PRODUCT_DEPOSIT' and s.Escrows[offer.ID]
        rows[#rows+1]={ID=d.ID,Kind=d.Kind,Label=d.Label,Met=d.Kind~='PRODUCT_DEPOSIT' or Evaluate(from,to,d,offer.ID),Minimum=d.Minimum,Gold=d.Gold,Faith=d.Faith,Frozen=true,Settlement=escrow and escrow.Status}
      end
      rows[#rows+1]={ID='relations',Label='LOC_JAC_ADMISSION_FROZEN_RELATIONS',Met=true,Reviewed=true,Details=terms.Relationship.Rows,Frozen=true}
      local envoys=request.Decision.Mode=='SMALL_STATE' and {Envoys=request.Decision.Envoys,Required=math.max(1,request.Decision.OwnedTiles),Route='FROZEN'} or nil
      return {From=from,To=to,Country=R.Copy(terms.Input.Country),Mode=request.Plan.Mode,
        Rows=rows,Envoys=envoys,Deposit=R.Copy(terms.Deposit),Status=request.Status,OfferID=pair.ID,
        Input=R.Copy(terms.Input),Frozen=true,Ready=true,CanCancel=N.CanCancelOffer(pair.ID),
        Error=request.LastError or pair.LastError}
    end
    local input=Input(from,to)
    local terms=offer and offer.ProductTerms or Terms(input)
    local rows={};local met=true
    for _,d in ipairs(offer and offer.Requirements or terms.Requirements) do
      local achieved,current,minimum=Evaluate(from,to,d,pair and pair.ID)
      rows[#rows+1]={ID=d.ID,Kind=d.Kind,Label=d.Label,Met=achieved,Current=current,Minimum=minimum,Gold=d.Gold,Faith=d.Faith,Settlement=d.Kind=='PRODUCT_DEPOSIT' and current or nil}
      met=met and achieved
    end
    local relations=RelationState(from,to)
    rows[#rows+1]={ID='relations',Label='LOC_JAC_ADMISSION_RELATIONS',Met=relations.Met,Reviewed=relations.Reviewed,Details=relations.Rows}
    local envoys=input.Country.AreaKm2<10000 and Envoys(from,to) or nil
    if envoys then envoys.Required=math.max(1,OwnedTiles(from));met=met and envoys.Eligible and not PendingEnvoy(to) and envoys.Envoys>=envoys.Required end
    return {From=from,To=to,Country=R.Copy(input.Country),Mode=input.Country.AreaKm2<10000 and 'SMALL_STATE' or 'MERGER',
      Rows=rows,Envoys=envoys,Deposit=R.Copy(terms.Deposit),Status=request and request.Status or pair and pair.Cancelled and 'CANCELLED' or 'AVAILABLE',
      OfferID=pair and pair.ID,Input=R.Copy(terms.Input),LiveInput=input,
      CanCancel=pair~=nil and not pair.Cancelled and N.CanCancelOffer(pair.ID),
      Ready=met and relations.Met and input.Situation~='war',Error=pair and pair.LastError}
  end)
  if not ok then
    local pair=State().Pairs[Pair(from,to)]
    return {From=from,To=to,Status='UNAVAILABLE',Error=tostring(value),OfferID=pair and pair.ID,
      CanCancel=pair and not pair.Cancelled and N.CanCancelOffer(pair.ID) or false}
  end
  return R.Copy(value)
end
function P.GetSmallCountries(to)
  local rows={}
  for _,from in ipairs(Actors()) do
    if from~=to and P.Country(from).AreaKm2<10000 then
      local view=P.Get(from,to);if view.Status~='UNAVAILABLE' then rows[#rows+1]=view end
    end
  end
  return rows
end
local function SendEnvoy(from,to,opkey,expected)
  local envoys=Envoys(from,to)
  local identity=EnvoyIdentity(from,to,envoys.Route)
  assert(envoys.Route=='PRODUCT' or not Players[to]:IsHuman(),'human official minors use native envoy placement')
  local s=State();local operation=assert(s.Operations[opkey])
  assert(R.Equal(operation.EnvoyIdentity,identity),'envoy sender, recipient or route changed')
  local receipt=operation.EnvoyReceipt
  if receipt and receipt.Done then return end
  assert(envoys.Eligible,'official envoy sender or recipient is not eligible')
  assert(not PendingEnvoy(to,opkey),'another envoy assignment is unsettled')
  if not Players[to]:IsHuman() then
    assert(not AiEnvoyUsed(to,opkey),'AI already assigned an envoy this turn')
    local turn=Game.GetCurrentGameTurn();assert(Integer(turn),'actual envoy turn unavailable')
    -- Claim before either native mutation. An interrupted charge/grant and
    -- its recovery use the same recipient budget, including after reload.
    s.AiEnvoyTurns[tostring(to)]={Turn=turn,Operation=opkey}
  end
  local pool=Players[to]:GetInfluence();local count=pool:GetTokensToGive()
  assert(Integer(count),'official envoy pool unavailable')
  if not receipt then
    assert(expected==envoys.Envoys,'stale envoy assignment')
    assert(count>=1,'no unassigned envoys')
    -- An old unfinished PRODUCT receipt cannot be charged a second time.
    local legacy=s.Envoys[Pair(from,to)]
    if legacy then for _,old in pairs(legacy.Receipts or {}) do assert(old.Done,'legacy envoy assignment is unsettled') end end
    receipt={Before=count,After=count-1,ReceivedBefore=expected,ReceivedAfter=expected+1}
    operation.EnvoyReceipt=receipt
  end
  Save(s)
  if not receipt.ChargeDone then
    -- Observe even a mutate-then-throw call before returning its error. If
    -- that observation was not saved, an altered pool on replay is ambiguous.
    assert(count==receipt.Before and envoys.Envoys==receipt.ReceivedBefore,'envoy charge journal is ambiguous')
    local ok,err=pcall(pool.ChangeTokensToGive,pool,-1)
    assert(pool:GetTokensToGive()==receipt.After and Envoys(from,to).Envoys==receipt.ReceivedBefore,
      'official envoy pool was not consumed exactly once')
    assert(R.Equal(identity,EnvoyIdentity(from,to,Envoys(from,to).Route)),'envoy identity changed during charge')
    s=State();s.Operations[opkey].EnvoyReceipt.ChargeDone=true;Save(s)
    if not ok then error(err) end
  end
  s=State();receipt=s.Operations[opkey].EnvoyReceipt
  assert(pool:GetTokensToGive()==receipt.After,'envoy pool journal is ambiguous')
  assert(Envoys(from,to).Envoys==receipt.ReceivedBefore,'envoy grant journal is ambiguous')
  receipt.GrantIntent=true;Save(s)
  if identity.Route=='OFFICIAL' then
    -- Official scenario API, not a replacement for native policy/first-meet
    -- operations. Only an observed exact +1 is attributed to this grant.
    local ok,err=pcall(pool.GiveFreeTokenToPlayer,pool,from)
    assert(pool:GetTokensToGive()==receipt.After and Envoys(from,to).Envoys==receipt.ReceivedAfter,
      'official envoy grant journal is ambiguous')
    assert(R.Equal(identity,EnvoyIdentity(from,to,Envoys(from,to).Route)),'envoy identity changed during grant')
    s=State();receipt=s.Operations[opkey].EnvoyReceipt
    receipt.GrantDone,receipt.Done=true,true;Save(s)
    if not ok then error(err) end
  else
    -- PRODUCT assignments and their DONE receipt share one property write.
    s=State();local key=Pair(from,to);local entry=s.Envoys[key]
    if not entry then entry={CountryID=identity.CountryID,CivilizationType=identity.CivilizationType,Count=0,Receipts={}};s.Envoys[key]=entry end
    assert(entry.Count==receipt.ReceivedBefore,'product envoy grant journal is ambiguous')
    entry.Count=receipt.ReceivedAfter
    receipt=s.Operations[opkey].EnvoyReceipt;receipt.GrantDone,receipt.Done=true,true;Save(s)
  end
end
function P.Command(actor,params)
  assert(Integer(actor) and Players[actor] and Players[actor]:IsAlive(),'actual player actor required')
  assert(type(params)=='table' and Id(params.Command) and Id(params.Nonce),'typed product operation required')
  assert(params.Command=='REGISTER' or params.Command=='CANCEL' or params.Command=='SEND_ENVOY','unknown product operation')
  assert(params.To==nil or params.To==actor,'forged receiving actor')
  for _,field in ipairs({'Requirements','AreaKm2','RelationsReviewed','Met','Plan','Deposit'}) do assert(params[field]==nil,'UI cannot supply gameplay conditions') end
  local from=params.From
  assert(Integer(from),'product source actor required')
  if params.Command~='CANCEL' then assert(Major(actor),'admission recipient must be a major civilization') end
  local opkey=tostring(actor)..':'..params.Nonce
  local s=State();local previous=s.Operations[opkey]
  local identity={Command=params.Command,From=from,To=actor,ExpectedEnvoys=params.ExpectedEnvoys,OfferID=params.OfferID}
  local envoyIdentity=params.Command=='SEND_ENVOY' and EnvoyIdentity(from,actor,Envoys(from,actor).Route) or nil
  if previous and envoyIdentity then assert(R.Equal(previous.EnvoyIdentity,envoyIdentity),'envoy sender, recipient or route changed') end
  if previous then assert(R.Equal(previous.Identity,identity),'operation nonce reused with changed intent');if previous.Done then return P.Get(from,actor) end
  end
  local input
  if params.Command=='CANCEL' then
    local pair=assert(s.Pairs[Pair(from,actor)],'no registered admission')
    local offer=assert(N.GetOffer(pair.ID),'cancelled offer missing')
    assert(offer.From==from and offer.To==actor,'cancellation actor differs from saved offer')
    -- Cancellation authenticates the saved payer/offer. An eliminated source
    -- or lost contact must not strand an unstarted escrow after preflight fails.
    input=offer.ProductTerms.Input
  else input=Input(from,actor) end
  if not previous then
    if params.Command=='CANCEL' then
      local pair=assert(s.Pairs[Pair(from,actor)],'no registered admission')
      assert(params.OfferID==pair.ID and N.CanCancelOffer(pair.ID),'stale or frozen cancellation')
    elseif params.Command=='SEND_ENVOY' then
      assert(not Occupied(from) and not Occupied(actor),'cannot send envoys during absorption')
      local envoys=Envoys(from,actor)
      assert(envoys.Eligible,'official envoy sender or recipient is not eligible')
      assert((envoys.Route=='PRODUCT' or not Players[actor]:IsHuman()) and params.ExpectedEnvoys==envoys.Envoys,'stale or unofficial envoy assignment')
      assert(not PendingEnvoy(actor),'another envoy assignment is unsettled')
      assert(Players[actor]:IsHuman() or not AiEnvoyUsed(actor),'AI already assigned an envoy this turn')
      assert(Players[actor]:GetInfluence():GetTokensToGive()>=1,'no unassigned envoys')
    end
    s.Operations[opkey]={Identity=identity,Nonce=params.Nonce,EnvoyIdentity=envoyIdentity};Save(s)
  end
  if params.Command=='SEND_ENVOY' then
    SendEnvoy(from,actor,opkey,params.ExpectedEnvoys)
  elseif params.Command=='REGISTER' then
    s=State();local key=Pair(from,actor);local pair=s.Pairs[key]
    if not pair or pair.Cancelled then
      local intent=s.Operations[opkey].OfferIntent
      if not intent then
        local offer,seq=NextOffer(pair or {},Terms(input))
        intent={Offer=offer,Sequence=seq};s.Operations[opkey].OfferIntent=intent;Save(s)
      end
      local offer,seq=intent.Offer,intent.Sequence
      N.DefineOffer(offer)
      pair={ID=offer.ID,Sequence=seq};s.Pairs[key]=pair;Save(s)
    else Refresh(from,actor) end
    pair=State().Pairs[key]
    -- Register a waiting request. Deposit preparation occurs in the normal
    -- turn handler; the UI does not supply a ready flag or a BEGIN command.
    N.Request('leader-offer',pair.ID,from,actor)
  elseif params.Command=='CANCEL' then
    local pair=assert(State().Pairs[Pair(from,actor)],'no registered admission')
    assert(params.OfferID==pair.ID,'stale cancellation')
    N.CancelOffer(pair.ID);Refund(pair.ID,'CANCELLED')
    s=State();s.Pairs[Pair(from,actor)].Cancelled=true;Save(s)
  else error('unknown product operation') end
  s=State();s.Operations[opkey].Done=true;Save(s)
  return P.Get(from,actor)
end
local function Operation(actor,params)
  local ok,result=pcall(P.Command,actor,params)
  if Players[actor] then Players[actor]:SetProperty('JAC_ABSORPTION_PRODUCT_LAST_OPERATION_V1',
    {Accepted=ok,Nonce=type(params)=='table' and params.Nonce or nil,Error=not ok and tostring(result) or nil}) end
end
function P.Tick()
  if busy then return end;busy=true
  for _,key in ipairs(R.Keys(State().Operations)) do
    local operation=State().Operations[key]
    if not operation.Done then
      local command=R.Copy(operation.Identity)
      command.Nonce=operation.Nonce
      local ok,err=pcall(P.Command,command.To,command)
      local s=State();s.Operations[key].LastError=not ok and tostring(err) or nil;Save(s)
    end
  end
  -- Reconcile interrupted engine calls and orphaned v2 replacements before
  -- evaluating any new terms. The old frozen admission is never rerolled.
  for _,key in ipairs(R.Keys(State().Pairs)) do
    local pair=State().Pairs[key]
    local offer=N.GetOffer(pair.ID)
    local ok,err=pcall(function()
      local request=N.Get('leader-offer',pair.ID)
      if request and request.ReplacedBy then
        Refund(pair.ID,'REPLACED')
        local s=State();s.Pairs[key].ID=request.ReplacedBy;s.Pairs[key].Sequence=pair.Sequence+1;Save(s)
        pair=State().Pairs[key];offer=assert(N.GetOffer(pair.ID))
      end
      request=N.Get('leader-offer',pair.ID)
      if request and request.Status=='CANCELLED' then
        Refund(pair.ID,'CANCELLED');local s=State();s.Pairs[key].Cancelled=true;Save(s);return
      end
      if request and request.Plan then
        local flow=F.Get(request.FlowID)
        if flow and (flow.Phase=='RESULT' or flow.Phase=='DONE') then Consume(offer,request,flow) end
        if not N.CanCancelOffer(pair.ID) then return end
      end
      if not Players[offer.From] or not Players[offer.From]:IsAlive() or not Major(offer.To) then
        -- A failed preflight has no irreversible flow. A vanished source
        -- cannot fulfil it, so release the saved payer's refundable deposit.
        if N.CanCancelOffer(pair.ID) then
          N.CancelOffer(pair.ID);Refund(pair.ID,not Major(offer.To) and 'INVALID_RECIPIENT' or 'SOURCE_ELIMINATED')
          local s=State();s.Pairs[key].Cancelled=true;Save(s)
        end
        return
      end
      Refresh(offer.From,offer.To)
      pair=State().Pairs[key];offer=N.GetOffer(pair.ID)
      local escrow=State().Escrows[pair.ID]
      if escrow and escrow.Status=='COLLECTING' or CanPrepare(offer) then Collect(offer) end
      N.Request('leader-offer',pair.ID,offer.From,offer.To)
    end)
    local s=State();s.Pairs[key].LastError=not ok and tostring(err) or nil;Save(s)
  end
  -- AI uses the same operations, token pool, demands, settlement, and N gate.
  -- It only registers offers whose non-deposit conditions already hold.
  for _,to in ipairs(Actors(true)) do if not Players[to]:IsHuman() and not Occupied(to) and not AiEnvoyUsed(to) then
    for _,from in ipairs(Actors()) do if from~=to and not Players[from]:IsHuman() and not Occupied(from) then
      local ok,acted=pcall(function()
        local view=P.Get(from,to)
        if view.Status=='UNAVAILABLE' or view.Status=='STARTED' or view.Status=='CANCELLED' then return end
        local pair=State().Pairs[Pair(from,to)]
        local offer=pair and N.GetOffer(pair.ID) or NextOffer({},Terms(Input(from,to)))
        if not CanPrepare(offer,true) then return end
        if view.Envoys and view.Envoys.Envoys<view.Envoys.Required then
          if not view.Envoys.Eligible then return end
          if Players[to]:GetInfluence():GetTokensToGive()>0 then
            P.Command(to,{Command='SEND_ENVOY',From=from,Nonce='ai-envoy:'..from..':'..view.Envoys.Envoys,ExpectedEnvoys=view.Envoys.Envoys})
          end
        end
        if not pair then
          if CanPrepare(offer) then P.Command(to,{Command='REGISTER',From=from,Nonce='ai-register:'..from}) end
        end
        return true
      end)
      -- Concentrate on one eligible country per recipient each turn. A token
      -- is never spent on every unrelated country while this one is pending.
      if ok and acted or AiEnvoyUsed(to) or PendingEnvoy(to) then break end
    end end
  end end
  busy=false
end
N.RegisterRequirement('PRODUCT_METRIC',function(from,to,d) return Evaluate(from,to,d) end)
N.RegisterRequirement('PRODUCT_IDENTITY',function(from,to,d) return Evaluate(from,to,d) end)
N.RegisterRequirement('PRODUCT_CONTEXT',function(from,to,d) return Evaluate(from,to,d) end)
N.RegisterRequirement('PRODUCT_DEPOSIT',function(from,to,d) return Evaluate(from,to,d,d.OfferID) end)
N.RegisterRequirement('PRODUCT_RELATIONS',function(from,to) return RelationState(from,to).Met and DiplomaticSituation(from,to)~='war' end)
N.RegisterEnvoys('product',function(from,to) local value=Envoys(from,to);value.Eligible=value.Eligible and not PendingEnvoy(to);return value end)
GameEvents.JAC_AbsorptionProduct_Operation.Add(Operation)
GameEvents.OnGameTurnStarted.Add(P.Tick)
-- Read-only exports only. Mutating functions are never available to UI.
ExposedMembers.JAC_AbsorptionProduct={Get=P.Get,GetSmallCountries=P.GetSmallCountries,Country=P.Country}
JAC_AbsorptionProductGameplay=P
print('[JAC_ABSORPTION_PRODUCT] loaded')
return P
"#;
