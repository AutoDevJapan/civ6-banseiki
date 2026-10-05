-- Only gameplay mutates state. Official evidence and unimplemented native adapters: documents/poc/assassin.md.
include "JAC_Assassin_Rules"
local R=JAC_ASN
-- Unsupported policies must never silently keep using the PoC implementation after a constant changes.
assert(R.DECISIONS.TrainingModifiers=="EVERY_TURN","JAC_ASN: training policy not implemented")
assert(R.DECISIONS.Snapshot=="COMMAND","JAC_ASN: snapshot policy not implemented")
assert(R.PROVISIONAL.FailureDisposition=="DEATH","JAC_ASN: captivity rules not implemented")
assert(R.DECISIONS.ActorLoss=="CANCEL_ALL_RESULTS_FIXED_PENALTY","JAC_ASN: actor loss policy not implemented")
assert(R.POST_DEATH.FreeState=="ENGINE_FREE_CITIES" and
  R.POST_DEATH.Absorption=="WEIGHTED_NEIGHBOR_DISBAND" and
  R.POST_DEATH.Congress=="FAVOR_WEIGHTED_JAPAN_BALLOT" and
  R.POST_DEATH.Succession=="KIN_FIRST_SINGLE_ROLE" and
  R.POST_DEATH.MissingRecipient=="FREE_STATE" and R.POST_DEATH.MissingSuccessor=="FREE_STATE" and
  R.POST_DEATH.CongressTie=="FREE_STATE","JAC_ASN: post-death decision not implemented")
local function Log(s) print("[JAC_ASN] "..s) end
local function Save(id,s)
  Players[id]:SetProperty(R.STATE,s)
  if s.Production or s.Mission then
    local actors=Game:GetProperty(R.ACTORS) or {}
    local found=false; for _,actor in ipairs(actors) do if actor==id then found=true end end
    if not found then actors[#actors+1]=id; table.sort(actors); Game:SetProperty(R.ACTORS,actors) end
  end
end
local function Majors()
  local ids={}; for _,p in ipairs(PlayerManager.GetAliveMajors()) do ids[#ids+1]=p:GetID() end
  table.sort(ids); return ids
end
local function WorkIDs()
  -- A saved job belongs to a country slot even after that country is eliminated.
  local ids,seen={},{}
  for _,id in ipairs(Majors()) do ids[#ids+1]=id; seen[id]=true end
  for _,id in ipairs(Game:GetProperty(R.ACTORS) or {}) do
    if not seen[id] and Players[id] then ids[#ids+1]=id; seen[id]=true end
  end
  table.sort(ids); return ids
end
local function Notify(id,key,...)
  local data={
    [ParameterTypes.MESSAGE]=Locale.Lookup("LOC_JAC_ASN_TITLE"),
    [ParameterTypes.SUMMARY]=Locale.Lookup(key,unpack({...})) }
  local ok,err=pcall(NotificationManager.SendNotification,id,NotificationTypes.USER_DEFINED_1,data)
  if not ok then Log("notification failed: "..tostring(err)) end
end
local function Token(unit)
  local token=unit:GetProperty(R.TOKEN)
  if token==nil then
    local serial=(Game:GetProperty("JAC_ASN_INDIVIDUAL_SERIAL") or 0)+1
    Game:SetProperty("JAC_ASN_INDIVIDUAL_SERIAL",serial)
    token=tostring(unit:GetOwner())..":"..unit:GetID()..":"..serial
    unit:SetProperty(R.TOKEN,token)
  end
  return token
end
function R.StartProduction(id,cityID)
  if not Players[id] or not Players[id]:IsAlive() then return false,"TARGET_GONE" end
  local s=R.State(id); local ok,reason=R.CanProduce(s,R.Turn())
  if not ok then return ok,reason end
  local city=Players[id]:GetCities():FindID(cityID)
  if not city then return false,"CITY_GONE" end
  local turns=R.ProductionTurns(R.ProductionInput(id,city))
  s.Production={City=cityID,Start=R.Turn(),Due=R.Turn()+turns,Turns=turns,Speed=R.Speed(),EvaluatedTurn=R.Turn()}
  Save(id,s); Log("training started player="..id.." city="..cityID.." turns="..turns)
  return true
end
function R.CancelProduction(id)
  local s=R.State(id); s.Production=nil; Save(id,s)
  -- Cancellation never clears the nationwide completion lock; elapsed progress is discarded.
end
function R.TestRevealTile(id,x,y)
  if Game:GetProperty(R.TEST)~=1 then return false,"TEST_MODE_OFF" end
  -- Resolve the plot in Gameplay, as in the official AlexanderScenario. The
  -- FireTuner caller can have different Map bindings, particularly after load.
  local vision=PlayersVisibility[id]
  if not vision then return false,"TEST_VISIBILITY_UNAVAILABLE" end
  if not vision:IsVisible(x,y) then
    local plot=Map.GetPlot(x,y)
    if not plot then return false,"TEST_TILE_UNAVAILABLE:"..tostring(x)..","..tostring(y) end
    vision:ChangeVisibilityCount(plot:GetIndex(),1)
  end
  if not vision:IsRevealed(x,y) or not vision:IsVisible(x,y) then
    return false,"TEST_VISIBILITY_NOT_UPDATED"
  end
  return true
end
function R.TestAdvanceDeadlines(id,clocks)
  if Game:GetProperty(R.TEST)~=1 then return false,"TEST_MODE_OFF" end
  if not Players[id] then return false,"TARGET_GONE" end
  local s=R.State(id); local due=R.Turn()+1
  local function selected(key) return clocks==nil or clocks[key]==true end
  if selected("Training") and s.Production then
    s.Production.TestDue=math.min(s.Production.Due,due); s.Production.Due=s.Production.TestDue
  end
  if selected("Mission") and s.Mission then s.Mission.Due=math.min(s.Mission.Due,due) end
  if selected("ProductionLock") and s.ProductionUnlock then s.ProductionUnlock=math.min(s.ProductionUnlock,due) end
  if selected("CommandLock") and s.CommandUnlock then s.CommandUnlock=math.min(s.CommandUnlock,due) end
  Save(id,s) -- no Tick / grant / resolution here: the real next turn processes the saved clocks
  return true,due
end
-- BEGIN RUST GENERATED TRAINING ORIGIN FIXTURE
local ORIGIN_TEST_KEY="JAC_ASN_DIAGNOSTIC_ORIGIN_"
function R.TestTrainingOrigin(id,cityID,x,y)
  if Game:GetProperty(R.TEST)~=1 then return false,"TEST_MODE_OFF" end
  local player=Players[id]
  local city=player and player:IsAlive() and player:GetCities():FindID(cityID)
  if not city or city:GetOwner()~=id then return false,"CITY_GONE" end
  local key=ORIGIN_TEST_KEY..tostring(id)
  if x==nil and y==nil then Game:SetProperty(key,nil); return true end
  if type(x)~="number" or type(y)~="number" or x~=math.floor(x) or y~=math.floor(y) then
    return false,"TEST_ORIGIN_INVALID"
  end
  local plot=Map.GetPlot(x,y)
  if not plot or plot:IsCity() then return false,"TEST_ORIGIN_INVALID" end
  local production=R.State(id).Production
  if production and production.City~=cityID then return false,"TEST_ORIGIN_WRONG_CITY" end
  Game:SetProperty(key,{City=cityID,X=x,Y=y})
  return true
end
function R.TrainingOrigin(id,city)
  -- Ordinary production ignores even a stale diagnostic property.
  if Game:GetProperty(R.TEST)==1 then
    local origin=Game:GetProperty(ORIGIN_TEST_KEY..tostring(id))
    if origin then
      assert(type(origin)=="table" and origin.City==city:GetID() and
        type(origin.X)=="number" and type(origin.Y)=="number" and
        origin.X==math.floor(origin.X) and origin.Y==math.floor(origin.Y),"invalid diagnostic origin")
      local plot=Map.GetPlot(origin.X,origin.Y)
      assert(plot and not plot:IsCity(),"diagnostic origin no longer valid")
      return origin.X,origin.Y
    end
  end
  return city:GetX(),city:GetY()
end
-- END RUST GENERATED TRAINING ORIGIN FIXTURE
local function Copy(v)
  if type(v)~="table" then return v end
  local t={}; for k,x in pairs(v) do t[k]=Copy(x) end; return t
end
local function RegisteredDead(input,people,identity)
  if people[identity] and people[identity].Dead==1 then return true end
  local single=input.Successor
  if type(single)=="table" and single.Identity==identity and single.Dead==1 then return true end
  for _,p in ipairs(input.Successors or {}) do
    if p.Identity==identity and p.Dead==1 then return true end
  end
  return false
end
local function BindGreatPerson(target,person)
  if not person.Unit then return nil end
  local u=Players[target]:GetUnits():FindID(person.Unit)
  if not u or R.Classify(u)~="GREAT_PERSON" then return nil end
  local token=Token(u)
  if person.Token and person.Token~=token then return nil end
  local bindings=Players[target]:GetProperty(R.BINDINGS) or {}
  local binding=bindings[person.Identity]
  if binding and (binding.Unit~=person.Unit or binding.Token~=token) then return nil end
  local identity=u:GetProperty(R.PERSON)
  if identity and identity~=person.Identity then return nil end
  -- The physical unit's canonical person ID is independent of eligibility/death.
  -- Bind before filtering, so a dead registered GP cannot reappear under its unit token.
  if not identity then u:SetProperty(R.PERSON,person.Identity) end
  if not binding then
    bindings[person.Identity]={Unit=person.Unit,Token=token}
    Players[target]:SetProperty(R.BINDINGS,bindings)
  end
  person.Token=token
  return u
end
local function Candidates(target)
  local input=R.Input(Players[target]); local candidates={}
  local nation=R.Nation(target)
  local currentLeader=nation and nation.CurrentLeader or PlayerConfigurations[target]:GetLeaderTypeName()
  local people=Players[target]:GetProperty(R.PEOPLE) or {}
  for _,person in ipairs(input.Successors or {}) do
    if person.Identity then candidates[#candidates+1]=Copy(person) end
  end
  if input.Successor then
    local person=type(input.Successor)=="table" and Copy(input.Successor) or {Identity=input.Successor,Kind="PERSON"}
    if person.Identity then candidates[#candidates+1]=person end
  end
  -- Candidates must be supplied by the people provider; never invent a blood relative or a name.
  local valid={}
  for _,person in ipairs(candidates) do
    local u=person.Kind=="GREAT_PERSON" and BindGreatPerson(target,person) or nil
    local present=person.Identity~=currentLeader and person.Dead~=1 and not RegisteredDead(input,people,person.Identity)
    if person.Kind=="GREAT_PERSON" then
      present=present and u~=nil
    elseif person.Kind=="GOVERNOR" then
      local g=(input.Governors or {})[tostring(person.Governor)]
      present=present and g and g.Dead~=1 and g.Retired~=1 and g.Identity==person.Identity
    end
    if present then valid[#valid+1]=person end
  end
  table.sort(valid,function(a,b)
    if (a.Kin==1)~=(b.Kin==1) then return a.Kin==1 end
    return tostring(a.Identity)<tostring(b.Identity)
  end)
  return valid
end
local function Allied(a,b)
  -- Gameplay getter, unlike the UI-only GetAllianceType. Official VikingScenario.lua:77.
  return Players[a]:GetDiplomacy():HasAllied(b)
end
local function Religion(id)
  local player=Players[id]
  local religion=player.GetReligion and player:GetReligion()
  local value=religion and religion.GetReligionInMajorityOfCities and religion:GetReligionInMajorityOfCities()
  local row=type(value)=="number" and value>=0 and GameInfo.Religions and GameInfo.Religions[value]
  -- Generic pantheons are not a shared majority religion (Religions.xml:38).
  if row and row.Pantheon~=true and row.Pantheon~=1 then return value end
end
local function Culture(id)
  -- A supplied cultural group takes precedence over the provisional ethnicity proxy.
  local group=R.Input(Players[id]).CultureGroup
  local civ=GameInfo.Civilizations and GameInfo.Civilizations[PlayerConfigurations[id]:GetCivilizationTypeName()]
  group=group or (civ and civ.Ethnicity)
  return group~="" and group or nil
end
local function Borders(target)
  if not Map.GetPlotCount or not Map.GetPlotByIndex or not Map.GetAdjacentPlots then return nil end
  local borders={}
  for i=0,Map.GetPlotCount()-1 do
    local plot=Map.GetPlotByIndex(i)
    if plot:GetOwner()==target then
      -- Official gameplay enumeration: PiratesScenario_Shared_Script.lua:159,161.
      for _,other in ipairs(Map.GetAdjacentPlots(plot:GetX(),plot:GetY())) do
        local owner=other:GetOwner()
        if owner and owner>=0 and owner~=target then borders[owner]=(borders[owner] or 0)+1 end
      end
    end
  end
  return borders
end
local function Snapshot(target)
  local input=R.Input(Players[target]); local neighbors={}
  local targetCities=Players[target]:GetCities()
  local borders=Borders(target)
  local religion,culture=Religion(target),Culture(target)
  local selection={Weights=Copy(R.PROVISIONAL.RecipientWeights),
    MilitaryPerCity=R.PROVISIONAL.RecipientMilitaryPerCity,NeighborDistance=R.PROVISIONAL.NeighborDistance,
    MaxStrength=1,MaxBorder=1}
  for _,id in ipairs(Majors()) do
    if id~=target then
      local distance=math.huge
      for _,a in targetCities:Members() do for _,b in Players[id]:GetCities():Members() do
        distance=math.min(distance,Map.GetPlotDistance(a:GetX(),a:GetY(),b:GetX(),b:GetY()))
      end end
      local border=borders and (borders[id] or 0)
      if (border and border>0) or distance<=selection.NeighborDistance then
        local player=Players[id]; local relation=R.Relation(target,id)
        local stats=player.GetStats and player:GetStats()
        local military=stats and stats.GetMilitaryStrength and stats:GetMilitaryStrength() or 0
        local cities=player:GetCities():GetCount()
        local otherReligion,otherCulture=Religion(id),Culture(id)
        local n={Player=id,Cities=cities,Military=military,Strength=cities+military/selection.MilitaryPerCity,
          Relation=relation,Allied=Allied(target,id),Border=border,Distance=distance,
          MajorityReligion=otherReligion,CultureGroup=otherCulture,
          Religion=religion and otherReligion and (religion==otherReligion and 1 or 0),
          Culture=culture and otherCulture and (culture==otherCulture and 1 or 0)}
        n.Friendship=relation>=1 and 0 or (n.Allied and 1 or (1-relation)/2)
        selection.MaxStrength=math.max(selection.MaxStrength,n.Strength)
        selection.MaxBorder=math.max(selection.MaxBorder,border or 0)
        neighbors[#neighbors+1]=n
      end
    end
  end
  for _,n in ipairs(neighbors) do n.Score,n.Components,n.AvailableWeight=R.RecipientScore(n,selection) end
  table.sort(neighbors,function(a,b)
    if a.Score~=b.Score then return a.Score>b.Score end
    return a.Player<b.Player
  end)
  local candidates=Candidates(target)
  -- Japan disables CAPABILITY_WORLD_CONGRESS. Owner-approved equivalent uses native
  -- diplomacy Favor in gameplay (Debug/Player.ltp:694), without opening a native session.
  local congress=not input.Congress or input.Congress.Enabled~=false
  local recipient=neighbors[1] and neighbors[1].Player
  local votes,ballots={},{}
  if congress then
    for _,voter in ipairs(Majors()) do
      local choice=((input.Congress or {}).Votes or {})[tostring(voter)]
      if choice==nil then
        assert(R.PROVISIONAL.CongressDefaultBallot=="ALLY_SUCCESSOR_RECIPIENT_ABSORPTION_OTHER_FREE")
        if candidates[1] and (voter==target or Allied(target,voter)) then choice="SUCCESSION"
        elseif voter==recipient then choice="ABSORPTION" else choice="FREE_STATE" end
      end
      local favor=math.max(0,Players[voter]:GetDiplomacy():GetFavor())
      votes[tostring(voter)]=choice
      ballots[tostring(voter)]={Choice=choice,Favor=favor,Weight=R.FavorVotes(favor)}
    end
  end
  return {Security=input.Security or R.PROVISIONAL.SecurityDefault,
    Cities=targetCities:GetCount(),Regions=Copy(input.Regions),Neighbors=neighbors,
    MajorityReligion=religion,CultureGroup=culture,
    Selection=selection,Neighbor=#neighbors>0,Recipient=recipient,
    Congress=congress or false,Voters=congress and Majors() or nil,
    Votes=congress and votes or nil,Ballots=congress and ballots or nil,
    VotingRules=congress and {FreeVotes=R.PROVISIONAL.CongressFreeVotes,FavorStep=R.PROVISIONAL.CongressFavorStep,
      DefaultBallot=R.PROVISIONAL.CongressDefaultBallot} or nil,
    Successor=candidates[1],Candidates=candidates,
    RecordedTurn=R.Turn()}
end
local function Plan(branch,snapshot)
  local successor=Copy(snapshot.Successor)
  if type(successor)=="string" then successor={Identity=successor,Kind="PERSON"} end
  local recipient=snapshot.Recipient
  if recipient==nil then -- earlier PoC saves stored neighbors but no selected recipient
    local neighbors=Copy(snapshot.Neighbors or {})
    table.sort(neighbors,function(a,b)
      if a.Score and b.Score then
        if a.Score~=b.Score then return a.Score>b.Score end
        return a.Player<b.Player
      end
      if a.Cities~=b.Cities then return a.Cities>b.Cities end
      if (a.Relation or 0)~=(b.Relation or 0) then return (a.Relation or 0)<(b.Relation or 0) end
      return a.Player<b.Player
    end)
    recipient=neighbors[1] and neighbors[1].Player
  end
  local plan={Branch=branch,Policy=Copy(R.POST_DEATH),Recipient=recipient,Successor=successor}
  if branch=="CONGRESS" then
    local tally={FREE_STATE=0,ABSORPTION=0,SUCCESSION=0}
    for _,voter in ipairs(snapshot.Voters or {}) do
      local ballot=(snapshot.Ballots or {})[tostring(voter)]
      local choice=ballot and ballot.Choice or (snapshot.Votes or {})[tostring(voter)]
      if tally[choice] and (choice~="ABSORPTION" or snapshot.Neighbor) and
        (choice~="SUCCESSION" or snapshot.Successor) then
        tally[choice]=tally[choice]+(ballot and ballot.Weight or 1) -- legacy saves keep their accepted ballots
      end
    end
    local winner,high,tied="FREE_STATE",0,false
    for _,choice in ipairs({"FREE_STATE","ABSORPTION","SUCCESSION"}) do
      if tally[choice]>high then winner,high,tied=choice,tally[choice],false
      elseif tally[choice]==high then tied=true end
    end
    plan.Ballots=Copy(snapshot.Ballots); plan.VotingRules=Copy(snapshot.VotingRules); plan.Tally=tally
    plan.Voting=snapshot.Ballots and "FAVOR_WEIGHTED_EQUIVALENT" or "LEGACY_SAVED_BALLOTS"
    plan.Resolution=tied and R.POST_DEATH.CongressTie or winner
  end
  return plan
end
function R.StartAttempt(id,unitID,target,kind,objectID)
  local ok,reason,destination,distance=R.CheckAttempt(id,unitID,target,kind,objectID)
  if not ok then return ok,reason end
  local s=R.State(id); local f=R.Input(Players[target])
  local actor=Players[id]:GetUnits():FindID(unitID)
  local actorIdentity=Token(actor)
  if kind~="LEADER" and kind~="GOVERNOR" then
    destination.Identity=Token(Players[target]:GetUnits():FindID(objectID))
  end
  local chance=R.SuccessChance(kind,{Security=f.Security,Relation=R.Relation(id,target),Guard=f.Guard})
  local successRoll=Game.GetRandNum(10000,"JAC assassination outcome")
  local detectionRoll=Game.GetRandNum(10000,"JAC assassination detection")
  local snapshot=kind=="LEADER" and Snapshot(target) or nil
  local branchRoll
  if snapshot then
    local _,total=R.BranchWeights(snapshot)
    branchRoll=Game.GetRandNum(total,"JAC leader death branch")
  end
  local m={ID=tostring(id)..":"..R.Turn(),Unit=unitID,ActorIdentity=actorIdentity,Target=destination,
    Start=R.Turn(),Due=R.Turn()+R.MissionTurns(kind,distance),
    Chance=chance,SuccessRoll=successRoll,DetectionRoll=detectionRoll,BranchRoll=branchRoll,
    Success=successRoll<chance*100,Detected=detectionRoll<R.PROVISIONAL.DetectionPercent*100,
    Snapshot=snapshot,Branch=snapshot and R.Branch(snapshot,branchRoll) or nil,
    FailureDisposition=R.PROVISIONAL.FailureDisposition,
    PenaltyFriendship=R.PROVISIONAL.FriendshipPenalty,PenaltyGrievances=R.PROVISIONAL.Grievances}
  -- Forced outcomes cannot be supplied by ordinary UI operations. Test input is one-shot, saved in GameCore.
  if Game:GetProperty(R.TEST)==1 then
    local forced=Players[id]:GetProperty("JAC_ASN_FORCE_NEXT")
    if forced then
      if forced.Success~=nil then m.Success=forced.Success==1 end
      if forced.Detected~=nil then m.Detected=forced.Detected==1 end
      if forced.Branch then
        assert(snapshot,"JAC_ASN: branch override requires leader target")
        assert(R.PROVISIONAL.Weights[forced.Branch],"JAC_ASN: unknown forced branch")
        assert(forced.Branch~="CONGRESS" or snapshot.Congress,"JAC_ASN: Congress absent")
        assert(forced.Branch~="SUCCESSION" or snapshot.Successor,"JAC_ASN: successor absent")
        assert(forced.Branch~="ABSORPTION" or snapshot.Neighbor,"JAC_ASN: neighbor absent")
        m.Branch=forced.Branch
      end
      Players[id]:SetProperty("JAC_ASN_FORCE_NEXT",nil)
    end
  end
  if snapshot then m.PostDeathPlan=Plan(m.Branch,snapshot) end
  s.Mission=m; s.LastCommand=R.Turn()
  s.CommandUnlock=R.Turn()+R.WaitTurns(R.CommandInput(id,target))
  Save(id,s); Log("mission started player="..id.." target="..target.." kind="..kind.." due="..m.Due)
  return true
end
local function Penalties(id,m,actorGone)
  if actorGone or not m.Success or m.Detected then
    local target=Players[m.Target.Owner]
    local ledger=target:GetProperty("JAC_ASN_DIPLOMACY_V1") or {Grievances={},Friendship={},WarReason={}}
    ledger.Applied=ledger.Applied or {}
    if not ledger.Applied[m.ID] then
      ledger.Grievances[tostring(id)]=(ledger.Grievances[tostring(id)] or 0)+m.PenaltyGrievances
      ledger.Friendship[tostring(id)]=(ledger.Friendship[tostring(id)] or 0)-m.PenaltyFriendship
      if m.Detected and not actorGone then ledger.WarReason[tostring(id)]=m.Due end
      ledger.Applied[m.ID]=1
      target:SetProperty("JAC_ASN_DIPLOMACY_V1",ledger)
    end
    if m.Detected and not actorGone then
      for _,other in ipairs(Majors()) do if other~=id and other~=m.Target.Owner then
        Players[other]:SetProperty("JAC_ASN_DENOUNCE_"..id,m.Due)
      end end
    end
  end
  if not actorGone and not m.Success and Players[id]:IsAlive() then
    local assassin=Players[id]:GetUnits():FindID(m.Unit)
    if assassin and assassin:GetProperty(R.TOKEN)==m.ActorIdentity then UnitManager.Kill(assassin) end
  end
end
local function SaveNation(id,n)
  Players[id]:SetProperty(R.NATION,n)
  -- Retain the history independently of the eliminated native player slot.
  local nations=Game:GetProperty(R.NATIONS) or {}; nations[tostring(id)]=n
  Game:SetProperty(R.NATIONS,nations)
end
local function SuccessorPresent(id,person)
  if not person then return false end
  local input=R.Input(Players[id])
  if RegisteredDead(input,Players[id]:GetProperty(R.PEOPLE) or {},person.Identity) then return false end
  if person.Kind=="GREAT_PERSON" then
    return BindGreatPerson(id,person)~=nil
  elseif person.Kind=="GOVERNOR" then
    local g=(input.Governors or {})[tostring(person.Governor)]
    return g and g.Dead~=1 and g.Retired~=1 and g.Identity==person.Identity
  end
  return true -- non-map people are alive until a provider records their death
end
local function ApplyNation(m)
  local id=m.Target.Owner; local target=Players[id]
  local plan=m.PostDeathPlan or Plan(m.Branch,m.Snapshot) -- earlier PoC pending saves
  local outcome=plan.Resolution or plan.Branch
  if outcome=="ABSORPTION" and (plan.Recipient==nil or not Players[plan.Recipient] or
    not Players[plan.Recipient]:IsAlive()) then outcome=plan.Policy.MissingRecipient end
  if outcome=="SUCCESSION" and not SuccessorPresent(id,plan.Successor) then outcome=plan.Policy.MissingSuccessor end
  local previous=R.Nation(id)
  local n={LeaderDead=1,Leader=m.Target.Identity,DeathTurn=R.Turn(),Branch=m.Branch,
    Snapshot=m.Snapshot,Plan=plan,State=outcome,Slot="RETAINED",AI="NATIVE_SLOT",
    Diplomacy="RETAIN_EXISTING",Previous=previous,CountryName=previous and previous.CountryName}
  local housePeople=target:GetProperty(R.PEOPLE) or {}
  local dead=housePeople[m.Target.Identity] or {Identity=m.Target.Identity}
  dead.Dead=1; dead.Role="DEAD_LEADER"; housePeople[m.Target.Identity]=dead
  target:SetProperty(R.PEOPLE,housePeople)
  if outcome=="SUCCESSION" then
    local person=plan.Successor
    n.CurrentLeader=person.Identity; n.CurrentLeaderName=person.Name or "LOC_JAC_ASN_SUCCESSOR"
    n.Government="SUCCESSOR"
    if person.Kin~=1 and person.CountryName then n.CountryName=person.CountryName end
    local input=R.Input(target)
    if person.Kind=="GREAT_PERSON" then
      UnitManager.Kill(target:GetUnits():FindID(person.Unit)) -- one role, not leader + map GP
    elseif person.Kind=="GOVERNOR" then
      input.Governors[tostring(person.Governor)].Retired=1
    end
    local people=target:GetProperty(R.PEOPLE) or {}
    people[person.Identity]={Identity=person.Identity,Role="LEADER",Name=person.Name}
    target:SetProperty(R.PEOPLE,people)
    local single=input.Successor
    if (type(single)=="table" and single.Identity==person.Identity) or single==person.Identity then
      input.Successor=nil
    end
    local rest={}; for _,p in ipairs(input.Successors or {}) do
      if p.Identity~=person.Identity then rest[#rest+1]=p end
    end
    input.Successors=rest; target:SetProperty(R.INPUT,input)
  elseif outcome=="ABSORPTION" or outcome=="FREE_STATE" then
    local free=outcome=="FREE_STATE"
    -- Official free-city engine entry point: Debug/City.ltp:277; free player: BlackDeathScenario.lua:46.
    local recipient=free and PlayerManager.GetFreeCitiesPlayerID() or plan.Recipient
    if free then
      assert(recipient and recipient>=0 and Players[recipient] and CityManager.TransferCityToFreeCities,
        "JAC_ASN: free-city engine unavailable")
      n.FreeCitiesPlayer=recipient
    else n.Recipient=recipient end
    n.Slot="RETIRED"; n.AI=free and "FREE_CITIES_ENGINE" or "RECIPIENT_SLOT"
    n.Diplomacy=free and "NATIVE_ELIMINATION" or "RECIPIENT_EXISTING"
    n.Government=free and "FREE_CITIES" or "MERGED"; n.Cities={}; n.DisbandedUnits={}
    local cities,units={},{}
    for _,city in target:GetCities():Members() do cities[#cities+1]={ID=city:GetID(),X=city:GetX(),Y=city:GetY()} end
    for _,u in target:GetUnits():Members() do units[#units+1]=u:GetID() end
    table.sort(cities,function(a,b) return a.ID<b.ID end); table.sort(units)
    local people=free and Copy(housePeople) or (Players[recipient]:GetProperty(R.PEOPLE) or {})
    local registeredUnits={}
    local sourceInput=R.Input(target)
    if free then
      for identity,p in pairs(people) do
        p.Identity=p.Identity or identity; p.House=id
        if RegisteredDead(sourceInput,housePeople,p.Identity) then p.Dead=1 end
        p.Role=p.Dead==1 and (p.Role=="DEAD_LEADER" and "DEAD_LEADER" or "DEAD") or "UNAFFILIATED"
        p.SourceUnit=p.Unit or p.SourceUnit; p.SourceGovernor=p.Governor or p.SourceGovernor
        p.Unit=nil; p.Governor=nil; p.City=nil
      end
    end
    local function Preserve(p)
      if p.Identity and p.Identity~=m.Target.Identity and p.Dead~=1 and
        not RegisteredDead(sourceInput,housePeople,p.Identity) then
        local entry=Copy(p); entry.House=id; entry.Role=free and "UNAFFILIATED" or "MEMBER"; entry.City=nil
        entry.SourceUnit=entry.Unit or entry.SourceUnit; entry.SourceGovernor=entry.Governor or entry.SourceGovernor
        entry.Unit=nil; entry.Governor=nil -- archived roles must not refer to removed map objects
        people[p.Identity]=entry
        if p.Kind=="GREAT_PERSON" and p.Unit then registeredUnits[p.Unit]=p.Identity end
      end
    end
    for _,p in ipairs(m.Snapshot.Candidates or {}) do if SuccessorPresent(id,p) then Preserve(p) end end
    for _,p in ipairs(Candidates(id)) do Preserve(p) end -- people added during the mission also survive
    for _,p in pairs(target:GetProperty(R.PEOPLE) or {}) do Preserve(p) end
    for key,g in pairs(R.Input(target).Governors or {}) do
      local person=Copy(g); person.Governor=tonumber(key); Preserve(person)
    end
    for _,uid in ipairs(units) do
      local u=target:GetUnits():FindID(uid)
      if R.Classify(u)=="GREAT_PERSON" and not registeredUnits[uid] then
        Preserve({Identity=u:GetProperty(R.PERSON) or Token(u),Kind="GREAT_PERSON",Type=u:GetType(),Unit=uid})
      end
      n.DisbandedUnits[#n.DisbandedUnits+1]=uid
      UnitManager.Kill(u)
    end
    n.People=Copy(people) -- archive before the old native slot loses its final city
    if free then target:SetProperty(R.PEOPLE,people) else Players[recipient]:SetProperty(R.PEOPLE,people) end
    local input=R.Input(target); input.Governors=nil; input.Successor=nil; input.Successors=nil
    target:SetProperty(R.INPUT,input)
    if not free then target:SetProperty(R.PEOPLE,nil) end
    -- Kill/disband before the final city changes hands; last-city elimination may remove units.
    SaveNation(id,n)
    for _,c in ipairs(cities) do
      local city=target:GetCities():FindID(c.ID)
      if free then CityManager.TransferCityToFreeCities(city) else CityManager.TransferCity(city,recipient) end
      n.Cities[#n.Cities+1]={X=c.X,Y=c.Y,Owner=recipient}
    end
    n.NativeAlive=target:IsAlive() -- elimination/AI shutdown are checked by the live scenario
  end
  SaveNation(id,n)
  return "LEADER_"..outcome
end
local function ApplySuccess(m)
  local t=m.Target; local target=Players[t.Owner]
  if t.Kind=="LEADER" then
    return ApplyNation(m)
  elseif t.Kind=="GOVERNOR" then
    local input=R.Input(target); input.Governors[tostring(t.ID)].Dead=1
    target:SetProperty(R.INPUT,input)
    return "GOVERNOR_DEAD_PENDING"
  else
    local unit=target:GetUnits():FindID(t.ID); UnitManager.Kill(unit)
    return "UNIT_KILLED"
  end
end
function R.Tick()
  local turn=R.Turn()
  -- Persist the guard: a second event or loading on this turn does not complete twice.
  if Game:GetProperty("JAC_ASN_TICK")==turn then return end
  Game:SetProperty("JAC_ASN_TICK",turn)
  for _,id in ipairs(WorkIDs()) do
    local alive=Players[id]:IsAlive()
    local s=R.State(id); local p=s.Production
    if p then
      local city=alive and Players[id]:GetCities():FindID(p.City) or nil
      if not city then s.Production=nil; Log("training city lost player="..id)
      else
        R.RefreshTraining(id,p,city,turn)
        if turn>=p.Due then
        -- InitUnitValidAdjacentHex returns no Unit object (verified live, assassin-01/02; the official
        -- DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:193 ignores its result), so find the new unit.
        local had={}
        for _,u in Players[id]:GetUnits():Members() do had[u:GetID()]=true end
        local spawnX,spawnY=R.TrainingOrigin(id,city)
        UnitManager.InitUnitValidAdjacentHex(id,R.UnitForPlayer(id),spawnX,spawnY,1)
        local unit
        local want=GameInfo.Units[R.UnitForPlayer(id)]
        for _,u in Players[id]:GetUnits():Members() do
          if not had[u:GetID()] and want and u:GetType()==want.Index then unit=u end
        end
        if unit and (unit:GetX()<0 or unit:GetY()<0) then
          -- An ID can exist without a map placement (-9999 in the native UI).
          -- Remove the failed spawn and retry training without starting the lock.
          UnitManager.Kill(unit); unit=nil
        end
        if unit then
          s.LastCompletion=turn; s.ProductionUnlock=turn+R.Scale(R.PRODUCTION_LOCK)
          s.LastProducedUnit=unit:GetID(); s.Production=nil
          Save(id,s) -- commit before notification; a UI failure must not respawn the unit
          Log("training complete player="..id.." unit="..unit:GetID().." unlock="..s.ProductionUnlock)
          Notify(id,"LOC_JAC_ASN_PRODUCED",s.ProductionUnlock-turn)
        end -- if all spawn plots are blocked, retry; the lock starts at ACTUAL completion
        end
      end
    end
    local m=s.Mission
    if m and turn>=m.Due then
      local current=R.ResolveTarget(m.Target.Owner,m.Target.Kind,m.Target.ID)
      local status="TARGET_GONE"
      local actor=alive and Players[id]:GetUnits():FindID(m.Unit) or nil
      local actorPresent=R.IsAssassin(actor) and actor:GetProperty(R.TOKEN)==m.ActorIdentity
      if not actorPresent then status="ACTOR_GONE"
      elseif current and current.Identity==m.Target.Identity then
        if m.Success then status=ApplySuccess(m) else status="FAILED" end
      end
      -- A lost actor has only the fixed common penalty; target loss retains accepted consequences.
      Penalties(id,m,not actorPresent)
      m.Cancelled=not actorPresent
      s.LastMission=m; s.LastMission.Result=status; s.LastMission.Resolved=turn; s.Mission=nil
      Save(id,s)
      Log("mission result player="..id.." result="..status.." detected="..tostring(m.Detected))
      if alive then Notify(id,"LOC_JAC_ASN_RESULT",Locale.Lookup("LOC_JAC_ASN_RESULT_"..status)) end
    end
    Save(id,s)
  end
end
local function Operation(id,params)
  local action=params.Action
  local ok,reason
  if action=="PRODUCE" then ok,reason=R.StartProduction(id,tonumber(params.City))
  elseif action=="CANCEL" then R.CancelProduction(id); ok=true
  elseif action=="ATTEMPT" then
    ok,reason=R.StartAttempt(id,tonumber(params.Unit),tonumber(params.Target),params.Kind,tonumber(params.Object))
  else ok,reason=false,"TARGET_KIND" end
  Players[id]:SetProperty("JAC_ASN_LAST_OPERATION",{Action=action,Accepted=ok,Reason=reason,Turn=R.Turn()})
  if not ok then Log("rejected player="..id.." reason="..reason) end
end
GameEvents.JAC_AssassinOperation.Add(Operation)
GameEvents.OnGameTurnStarted.Add(R.Tick)
ExposedMembers.JAC_Assassin=R -- read/test entry point; ordinary UI uses synchronized operations
Log("loaded")
