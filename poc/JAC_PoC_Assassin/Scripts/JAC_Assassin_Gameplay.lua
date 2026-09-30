-- Only gameplay mutates state. Official evidence and unimplemented native adapters: documents/poc/assassin.md.
include "JAC_Assassin_Rules"
local R=JAC_ASN
-- Unsupported policies must never silently keep using the PoC implementation after a constant changes.
assert(R.PROVISIONAL.PostDeath=="RECORD_ONLY","JAC_ASN: native governance adapter not implemented")
assert(R.PROVISIONAL.ProductionSnapshot=="START","JAC_ASN: dynamic production factors not specified")
assert(R.PROVISIONAL.Snapshot=="COMMAND","JAC_ASN: other snapshot policies need an owner decision")
assert(R.PROVISIONAL.FailureDisposition=="DEATH","JAC_ASN: captivity rules not implemented")
assert(R.PROVISIONAL.ActorLoss=="ABORT_SUCCESS_KEEP_FIXED_PENALTIES","JAC_ASN: actor loss policy not implemented")
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
  s.Production={City=cityID,Start=R.Turn(),Due=R.Turn()+turns,Turns=turns}
  Save(id,s); Log("production started player="..id.." city="..cityID.." turns="..turns)
  return true
end
function R.CancelProduction(id)
  local s=R.State(id); s.Production=nil; Save(id,s)
  -- Cancellation never clears the nationwide completion lock; elapsed progress is discarded.
end
local function Snapshot(target)
  local input=R.Input(Players[target]); local neighbors={}
  local targetCities=Players[target]:GetCities()
  for _,id in ipairs(Majors()) do
    if id~=target then
      local close=false
      for _,a in targetCities:Members() do for _,b in Players[id]:GetCities():Members() do
        if Map.GetPlotDistance(a:GetX(),a:GetY(),b:GetX(),b:GetY())<=R.PROVISIONAL.NeighborDistance then close=true end
      end end
      if close then neighbors[#neighbors+1]={Player=id,Cities=Players[id]:GetCities():GetCount(),
        Relation=R.Relation(target,id)} end
    end
  end
  -- Congress is disabled on the Japan maps. Do not invent a Court replacement for 9.5.
  return {Security=input.Security or R.PROVISIONAL.SecurityDefault,
    Cities=targetCities:GetCount(),Regions=input.Regions,Neighbors=neighbors,
    Neighbor=#neighbors>0,Congress=false,Successor=input.Successor,
    RecordedTurn=R.Turn()}
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
  s.Mission=m; s.LastCommand=R.Turn()
  s.CommandUnlock=R.Turn()+R.WaitTurns(R.CommandInput(id,target))
  Save(id,s); Log("mission started player="..id.." target="..target.." kind="..kind.." due="..m.Due)
  return true
end
local function Penalties(id,m)
  if not m.Success or m.Detected then
    local target=Players[m.Target.Owner]
    local ledger=target:GetProperty("JAC_ASN_DIPLOMACY_V1") or {Grievances={},Friendship={},WarReason={}}
    ledger.Applied=ledger.Applied or {}
    if not ledger.Applied[m.ID] then
      ledger.Grievances[tostring(id)]=(ledger.Grievances[tostring(id)] or 0)+m.PenaltyGrievances
      ledger.Friendship[tostring(id)]=(ledger.Friendship[tostring(id)] or 0)-m.PenaltyFriendship
      if m.Detected then ledger.WarReason[tostring(id)]=m.Due end
      ledger.Applied[m.ID]=1
      target:SetProperty("JAC_ASN_DIPLOMACY_V1",ledger)
    end
    if m.Detected then
      for _,other in ipairs(Majors()) do if other~=id and other~=m.Target.Owner then
        Players[other]:SetProperty("JAC_ASN_DENOUNCE_"..id,m.Due)
      end end
    end
  end
  if not m.Success and Players[id]:IsAlive() then
    local assassin=Players[id]:GetUnits():FindID(m.Unit)
    if assassin and assassin:GetProperty(R.TOKEN)==m.ActorIdentity then UnitManager.Kill(assassin) end
  end
end
local function ApplySuccess(m)
  local t=m.Target; local target=Players[t.Owner]
  if t.Kind=="LEADER" then
    -- Logical death / pending governance only: native player is NOT killed and no city is reassigned.
    target:SetProperty(R.NATION,{LeaderDead=1,Leader=t.Identity,DeathTurn=R.Turn(),
      State="PENDING_"..m.Branch,Branch=m.Branch,Snapshot=m.Snapshot,DecisionRequired=1})
    return "LEADER_DEAD_PENDING"
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
      if not city then s.Production=nil; Log("production city lost player="..id)
      elseif turn>=p.Due then
        local unit=UnitManager.InitUnitValidAdjacentHex(id,R.UnitForPlayer(id),city:GetX(),city:GetY(),1)
        if unit then
          s.LastCompletion=turn; s.ProductionUnlock=turn+R.Scale(R.PRODUCTION_LOCK)
          s.LastProducedUnit=unit:GetID(); s.Production=nil
          Save(id,s) -- commit before notification; a UI failure must not respawn the unit
          Log("production complete player="..id.." unit="..unit:GetID().." unlock="..s.ProductionUnlock)
          Notify(id,"LOC_JAC_ASN_PRODUCED",s.ProductionUnlock-turn)
        end -- if all spawn plots are blocked, retry; the lock starts at ACTUAL completion
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
      -- Fixed failure/detection has consequences even if the actor or physical target vanished.
      Penalties(id,m)
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
