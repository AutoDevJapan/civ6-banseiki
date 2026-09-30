-- SSOT 9.5 / 1.3. Pure rules shared by gameplay and UI; mutations are gameplay only.
JAC_ASN = {}
local R = JAC_ASN
R.UNIT = "UNIT_JAC_JAPAN_SENGOKU_ASSASSIN"
R.COMMON_UNIT = "UNIT_JAC_ASSASSIN"
R.STATE = "JAC_ASN_STATE_V1"
R.INPUT = "JAC_ASN_INPUT_V1"
R.NATION = "JAC_ASN_NATION_V1"
R.NATIONS = "JAC_ASN_NATIONS_V1"
R.PEOPLE = "JAC_ASN_HOUSE_PEOPLE_V1"
R.PERSON = "JAC_ASN_PERSON_V1"
R.BINDINGS = "JAC_ASN_PERSON_UNITS_V1"
R.TOKEN = "JAC_ASN_INDIVIDUAL_V1"
R.ACTORS = "JAC_ASN_ACTORS_V1"
R.TEST = "JAC_ASN_TEST_MODE"
R.BASE_PRODUCTION, R.MIN_PRODUCTION, R.PRODUCTION_LOCK = 100, 20, 10
R.BASE_WAIT, R.MIN_WAIT = 50, 1
R.CHANCES = { LEADER=1, GOVERNOR=15, GREAT_PERSON=30, HERO=50, UNIT=50, BUILDER=90 }
R.DURATIONS = { LEADER=5, GOVERNOR=3, GREAT_PERSON=2 }

-- OWNER_UNDECIDED: these are PoC switches/coefficients, not SSOT decisions.
R.PROVISIONAL = {
  UnitTurns=2, HeroTurns=2, Near=1, Middle=3, MaximumDistance=6,
  SecurityDefault=50, ProductionSecurity=20,
  ChanceSecurityDivisor=100, ChanceRelation=0.25, ChanceGuard=0.5,
  BranchBase=100, BranchSecurity=1, BranchUnrest=1, BranchStrength=100, BranchRelation=25,
  NeighborDistance=6,
  RecipientMilitaryPerCity=100,
  RecipientWeights={Strength=35,Friendship=25,Border=15,Distance=15,Religion=5,Culture=5},
  CongressFreeVotes=1, CongressFavorStep=10,
  CongressDefaultBallot="ALLY_SUCCESSOR_RECIPIENT_ABSORPTION_OTHER_FREE",
  ProductionBuilding=10, ProductionDistrict=10, ProductionPolicy=10,
  ProductionGovernment=10, ProductionTrait=15, ProductionJob=15,
  WaitSecurity=10, WaitRelation=10, WaitSchool=10, WaitGovernor=7, WaitHero=6, WaitGreatPerson=6,
  DetectionPercent=25, FriendshipPenalty=10, Grievances=25,
  FailureDisposition="DEATH", -- capture needs a prisoner/release specification
  Visibility="REVEALED_CITY_VISIBLE_UNIT", -- OWNER_UNDECIDED: do not disclose hidden units
  Weights={ FREE_STATE=1, ABSORPTION=1, CONGRESS=1, SUCCESSION=1 },
}
-- Owner answers, 2026-09-30 / 2026-10-01 (SSOT 9.5). Numeric coefficients remain provisional.
R.DECISIONS={Snapshot="COMMAND",TrainingModifiers="EVERY_TURN",
  ActorLoss="CANCEL_ALL_RESULTS_FIXED_PENALTY",AllowNonWarTargets=true}
R.POST_DEATH={FreeState="ENGINE_FREE_CITIES",Absorption="WEIGHTED_NEIGHBOR_DISBAND",
  Congress="FAVOR_WEIGHTED_JAPAN_BALLOT",Succession="KIN_FIRST_SINGLE_ROLE",
  MissingRecipient="FREE_STATE",MissingSuccessor="FREE_STATE",CongressTie="FREE_STATE"}
R.SPEEDS={ GAMESPEED_ONLINE=0.5, GAMESPEED_QUICK=0.67, GAMESPEED_STANDARD=1,
           GAMESPEED_EPIC=1.5, GAMESPEED_MARATHON=3 }
-- Confirmed Japanese house participants, not a map-wide or ethnicity-wide name assignment.
R.JAPANESE_HOUSES={ODA=true,DATE=true,ASHINA=true,HOJO=true,YAMANOUCHI_UESUGI=true,
  TAKEDA=true,NAGAO=true,IMAGAWA=true,MATSUDAIRA=true,ASAKURA=true,ASHIKAGA=true,MIYOSHI=true,
  ROKKAKU=true,HONGANJI=true,OUCHI=true,AMAGO=true,MORI=true,CHOSOKABE=true,OTOMO=true,SHIMAZU=true}
function R.UnitForPlayer(id)
  local civ=PlayerConfigurations[id]:GetCivilizationTypeName()
  local house=string.match(civ,"^CIVILIZATION_JAC_(.+)$")
  return (civ=="CIVILIZATION_JAPAN" or R.JAPANESE_HOUSES[house]) and R.UNIT or R.COMMON_UNIT
end
function R.Clamp(n, lo, hi) return math.max(lo, math.min(hi, n)) end
function R.Turn() return Game.GetCurrentGameTurn() end
function R.Speed()
  local row = GameInfo.GameSpeeds[GameConfiguration.GetGameSpeedType()]
  local factor = row and R.SPEEDS[row.GameSpeedType]
  assert(factor, "JAC_ASN: unsupported game speed")
  return factor
end
function R.Scale(turns, speed) return math.ceil(turns * (speed or R.Speed())) end
function R.ProductionTurns(f, speed)
  f=f or {}; local p=R.PROVISIONAL
  local security=R.Clamp(f.Security or p.SecurityDefault,0,100)
  local turns=R.BASE_PRODUCTION + p.ProductionSecurity*(security-50)/50
  for _,kind in ipairs({"Building","District","Policy","Government","Trait","Job"}) do
    turns=turns - (f[kind] or 0)*p["Production"..kind]
  end
  return R.Scale(math.max(R.MIN_PRODUCTION,turns),speed)
end
function R.WaitTurns(f, speed)
  f=f or {}; local p=R.PROVISIONAL
  local badSecurity=R.Clamp((50-(f.Security or p.SecurityDefault))/50,0,1)
  local reduction=p.WaitSecurity*badSecurity + p.WaitRelation*R.Clamp(f.Relation or 0,0,1)
  for _,kind in ipairs({"School","Governor","Hero","GreatPerson"}) do
    reduction=reduction + p["Wait"..kind]*R.Clamp(f[kind] or 0,0,1)
  end
  -- 49 turns total reduction only when ALL six factors reach their maximum.
  return R.Scale(math.max(R.MIN_WAIT,R.BASE_WAIT-reduction),speed)
end
function R.MissionTurns(kind, distance, speed)
  local base=R.DURATIONS[kind]
  if kind=="HERO" then base=R.PROVISIONAL.HeroTurns end
  if kind=="UNIT" or kind=="BUILDER" then base=R.PROVISIONAL.UnitTurns end
  if not base then return nil end
  local multiplier=distance<=R.PROVISIONAL.Near and 1 or
                   (distance<=R.PROVISIONAL.Middle and 1.5 or 2)
  -- Ceiling after distance, then SSOT 1.3 speed conversion (both clocks integral).
  return R.Scale(math.ceil(base*multiplier),speed)
end
function R.SuccessChance(kind, f)
  local base=R.CHANCES[kind]; if not base then return nil end
  f=f or {}
  local security=R.Clamp(f.Security or R.PROVISIONAL.SecurityDefault,0,100)
  local p=R.PROVISIONAL
  return R.Clamp(base*(1+(50-security)/p.ChanceSecurityDivisor)*(1+p.ChanceRelation*(f.Relation or 0))
                 *(1-p.ChanceGuard*R.Clamp(f.Guard or 0,0,1)),0,100)
end
function R.CanProduce(s, turn)
  if s.Production then return false,"PRODUCING" end
  if turn < (s.ProductionUnlock or -1) then return false,"PRODUCTION_LOCK" end
  return true
end
function R.CanCommand(s, turn)
  if s.Mission then return false,"MISSION_ACTIVE" end
  if turn < (s.CommandUnlock or -1) then return false,"COMMAND_LOCK" end
  return true
end
function R.RecipientScore(n,selection)
  local f={Strength=n.Strength/math.max(1,selection.MaxStrength),Friendship=n.Friendship,
    Border=n.Border and n.Border/math.max(1,selection.MaxBorder) or nil,
    Distance=R.Clamp(1-n.Distance/selection.NeighborDistance,0,1),Religion=n.Religion,Culture=n.Culture}
  local score,weight=0,0
  for _,key in ipairs({"Strength","Friendship","Border","Distance","Religion","Culture"}) do
    if f[key]~=nil then
      score=score+selection.Weights[key]*f[key]; weight=weight+selection.Weights[key]
    end
  end
  return score/weight,f,weight
end
function R.FavorVotes(favor)
  -- Equivalent budget weighting, not a native Congress operation. First vote free;
  -- extra votes have increasing provisional costs 10,20,30,... (total step*k*(k+1)/2).
  local step=R.PROVISIONAL.CongressFavorStep
  return R.PROVISIONAL.CongressFreeVotes+math.floor((math.sqrt(1+8*math.max(0,favor)/step)-1)/2)
end
function R.BranchWeights(snapshot)
  local p=R.PROVISIONAL; local weights=p.Weights
  local security=R.Clamp(snapshot.Security or 50,0,100)
  local unrest=R.Clamp((snapshot.Regions or {}).Unrest or 0,0,100)
  local strength=p.BranchBase
  for _,neighbor in ipairs(snapshot.Neighbors or {}) do
    strength=math.max(strength,math.floor(p.BranchStrength*neighbor.Cities/math.max(1,snapshot.Cities or 1)
      + p.BranchRelation*(neighbor.Relation or 0)))
  end
  local rows={ {"FREE_STATE",weights.FREE_STATE*(p.BranchBase+p.BranchSecurity*(100-security)+p.BranchUnrest*unrest)},
    {"ABSORPTION",snapshot.Neighbor and weights.ABSORPTION*strength or 0},
    {"CONGRESS",snapshot.Congress and weights.CONGRESS*p.BranchBase or 0},
    {"SUCCESSION",snapshot.Successor and weights.SUCCESSION*(p.BranchBase+p.BranchSecurity*security) or 0} }
  local total=0; for _,row in ipairs(rows) do
    row[2]=math.max(0,math.floor(row[2])); total=total+row[2]
  end
  assert(total>0,"JAC_ASN: no post-death branch")
  return rows,total
end
function R.Branch(snapshot, roll)
  local rows,total=R.BranchWeights(snapshot)
  local n=roll % total
  for _,row in ipairs(rows) do n=n-row[2]; if n<0 then return row[1] end end
end
function R.State(id) return Players[id]:GetProperty(R.STATE) or {Version=1} end
function R.Nation(id)
  return (Game:GetProperty(R.NATIONS) or {})[tostring(id)] or
    (Players[id] and Players[id]:GetProperty(R.NATION))
end
function R.Input(object) return object:GetProperty(R.INPUT) or {} end
function R.IsAssassin(unit)
  if not unit then return false end
  local kind=GameInfo.Units[unit:GetType()].UnitType
  return kind==R.UNIT or kind==R.COMMON_UNIT
end
function R.KnownTarget(actor,target)
  if R.PROVISIONAL.Visibility=="NONE" then return true end
  local vision=PlayersVisibility[actor]
  if target.Kind=="LEADER" or target.Kind=="GOVERNOR" then
    return vision:IsRevealed(target.X,target.Y)
  end
  return vision:IsVisible(target.X,target.Y)
end
function R.Relation(a,b)
  if Players[a]:GetDiplomacy():IsAtWarWith(b) then return 1 end
  -- Relations is HOSTILITY (+1 worst, -1 best). Friendship uses the opposite sign.
  local hostility=(R.Input(Players[a]).Relations or {})[tostring(b)] or 0
  local ledger=Players[b]:GetProperty("JAC_ASN_DIPLOMACY_V1") or {}
  local friendship=(ledger.Friendship or {})[tostring(a)] or 0
  return R.Clamp(hostility-friendship/100,-1,1)
end
function R.ProductionInput(id, city)
  local a,b=R.Input(Players[id]),R.Input(city)
  return { Security=a.Security, Building=b.Building, District=b.District,
    Policy=a.Policy, Government=a.Government, Trait=b.Trait, Job=b.Job }
end
function R.RefreshTraining(id,p,city,turn)
  -- Re-evaluate total time against elapsed time; no accumulated fractions or RNG.
  -- Start and Speed are saved, so loading cannot reset progress or change rounding.
  p.Speed=p.Speed or R.Speed() -- migration of the earlier PoC save format
  p.Turns=R.ProductionTurns(R.ProductionInput(id,city),p.Speed)
  p.Due=p.Start+p.Turns
  -- A saved test deadline survives reload and every-turn recomputation. Normal games ignore it.
  if Game:GetProperty(R.TEST)==1 and p.TestDue then p.Due=math.min(p.Due,p.TestDue) end
  p.EvaluatedTurn=turn
end
function R.CommandInput(id,target)
  local a=R.Input(Players[id])
  return { Security=a.Security, Relation=R.Relation(id,target), School=a.School,
    Governor=a.Governor, Hero=a.Hero, GreatPerson=a.GreatPerson }
end
function R.Classify(unit)
  local row=GameInfo.Units[unit:GetType()]
  -- Heroes mode uses UnitType, not an undocumented Unit:GetHero() method.
  if string.sub(row.UnitType,1,10)=="UNIT_HERO_" or R.Input(unit).Hero==1 then return "HERO" end
  if row.BuildCharges and row.BuildCharges>0 and (row.Combat or 0)==0 then return "BUILDER" end
  if string.sub(row.UnitType,1,11)=="UNIT_GREAT_" then return "GREAT_PERSON" end
  return "UNIT"
end
function R.ResolveTarget(id,kind,objectID)
  if not Players[id] or not Players[id]:IsAlive() then return nil,"TARGET_GONE" end
  if kind=="LEADER" then
    local nation=R.Nation(id)
    if nation and nation.LeaderDead==1 and not nation.CurrentLeader then return nil,"TARGET_GONE" end
    local city=Players[id]:GetCities():GetCapitalCity()
    if not city then return nil,"TARGET_GONE" end
    return {Kind=kind,Owner=id,ID=city:GetID(),X=city:GetX(),Y=city:GetY(),
      Identity=nation and nation.CurrentLeader or PlayerConfigurations[id]:GetLeaderTypeName()}
  elseif kind=="GOVERNOR" then
    -- Personised Governors are registered by their content provider, with stable IDs and city IDs.
    local governors=R.Input(Players[id]).Governors or {}
    local g=governors[tostring(objectID)]
    if not g or g.Dead==1 or g.Retired==1 then return nil,"TARGET_GONE" end
    local city=Players[id]:GetCities():FindID(g.City)
    if not city then return nil,"TARGET_GONE" end
    return {Kind=kind,Owner=id,ID=objectID,X=city:GetX(),Y=city:GetY(),Identity=g.Identity}
  end
  local unit=Players[id]:GetUnits():FindID(objectID)
  if not unit then return nil,"TARGET_GONE" end
  local actual=R.Classify(unit)
  if actual~=kind then return nil,"TARGET_KIND" end
  return {Kind=actual,Owner=id,ID=objectID,X=unit:GetX(),Y=unit:GetY(),
    Identity=unit:GetProperty(R.TOKEN) or unit:GetType()}
end
function R.CheckAttempt(id,unitID,target,kind,objectID)
  if not Players[id] or not Players[id]:IsAlive() then return false,"TARGET_GONE" end
  local ok,reason=R.CanCommand(R.State(id),R.Turn()); if not ok then return ok,reason end
  if id==target or not R.CHANCES[kind] then return false,"TARGET_KIND" end
  local unit=Players[id]:GetUnits():FindID(unitID)
  if not R.IsAssassin(unit) then return false,"ASSASSIN_REQUIRED" end
  local destination,why=R.ResolveTarget(target,kind,objectID)
  if not destination then return false,why end
  if not R.KnownTarget(id,destination) then return false,"TARGET_UNKNOWN" end
  if not R.DECISIONS.AllowNonWarTargets and not Players[id]:GetDiplomacy():IsAtWarWith(target) then
    return false,"TARGET_KIND"
  end
  local distance=Map.GetPlotDistance(unit:GetX(),unit:GetY(),destination.X,destination.Y)
  if distance>R.PROVISIONAL.MaximumDistance then return false,"TOO_FAR" end
  if not R.MissionTurns(kind,distance) then return false,"DURATION_UNSET" end
  return true,nil,destination,distance
end
