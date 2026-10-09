//! Bounded real-engine recovery fixture; not normal admission or a full Flow proof.
//! Generate only the adjacent scenario. No game, installation, or catalogue mutation.
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

const BASE: &str = include_str!("../../../civ6-autotest/scenarios/absorption-native-custody.json");
const OUTPUT: &str = "absorption-native-carrier-hotseat-save.json";
const KEY: &str = "JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996";
#[cfg(test)]
const TOKEN: &str = "fixture-native-hotseat-996";
const HERO_UI: &str = "JAC_Absorption_HeroKeys";

const START_READY: &str = r#"return GameConfiguration.IsHotseat() and GameConfiguration.IsAnyMultiplayer()
 and Game.GetLocalPlayer()==0 and Players[0]:IsHuman() and Players[1]:IsHuman()
 and not Players[2]:IsHuman() and Players[0]:IsTurnActive()
 and Players[0]:GetCities():GetCapitalCity()~=nil and Players[2]:GetCities():GetCapitalCity()~=nil
 and ExposedMembers.JAC_Absorption~=nil and ExposedMembers.JAC_HeroReadAPI~=nil"#;

const SITE: &str = r#"assert(Game.GetLocalPlayer()==0 and Players[0]:IsTurnActive(),'human0 turn required')
local range=assert(tonumber(GlobalParameters.CITY_MIN_RANGE),'city spacing unavailable')
local site
for i=0,Map.GetPlotCount()-1 do
 local q=Map.GetPlotByIndex(i)
 if q:GetOwner()==0 and not q:IsWater() and not q:IsMountain() and not q:IsImpassable()
  and not q:IsNaturalWonder() and not q:IsCity() and q:GetDistrictType()==-1 and q:GetImprovementType()==-1 then
  local near=99999
  for _,owner in ipairs(PlayerManager.GetAliveIDs()) do
   for _,city in Players[owner]:GetCities():Members() do
    near=math.min(near,Map.GetPlotDistance(q:GetX(),q:GetY(),city:GetX(),city:GetY()))
   end
  end
  if near>range and (not site or near>site.Distance) then site={X=q:GetX(),Y=q:GetY(),Index=i,Distance=near} end
 end
end
assert(site,'fixture preparation incomplete: no valid owned land beyond city spacing')
ExposedMembers.JAC996Site=site
return 'selected owned empty land, nearest city distance '..site.Distance"#;

const CREATE: &str = r#"local A=assert(JAC_AbsorptionGameplay)
assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996')==nil,'fixture already exists')
assert(GameConfiguration.IsHotseat() and GameConfiguration.IsAnyMultiplayer(),'hotseat required')
assert(Players[0]:IsHuman() and Players[1]:IsHuman() and not Players[2]:IsHuman(),'need human0/human1/AI2')
local site=assert(ExposedMembers.JAC996Site,'detached UI site absent')
local q=assert(Map.GetPlotByIndex(site.Index))
assert(q:GetX()==site.X and q:GetY()==site.Y and q:GetOwner()==0 and not q:IsCity()
 and not q:IsWater() and not q:IsMountain() and not q:IsImpassable()
 and q:GetDistrictType()==-1 and q:GetImprovementType()==-1,'selected city plot changed')
local range=assert(tonumber(GlobalParameters.CITY_MIN_RANGE))
for _,owner in ipairs(PlayerManager.GetAliveIDs()) do for _,city in Players[owner]:GetCities():Members() do
 assert(Map.GetPlotDistance(site.X,site.Y,city:GetX(),city:GetY())>range,'selected plot violates city spacing')
end end
local cities=Players[0]:GetCities();local capital=assert(cities:GetCapitalCity());local n=cities:GetCount()
local source=assert(Players[2]:GetCities():GetCapitalCity())
-- Stock TunerMapPanel.lua:111-121 uses player:GetCities():Create(x,y).
-- Do not guess a CityManager.InitCity signature or synthesize a city/slot.
assert(type(cities.Create)=='function','missing stock native city creation: Cities:Create(x,y)')
cities:Create(site.X,site.Y)
local target=assert(CityManager.GetCityAt(site.X,site.Y),'native city creation rejected owned land')
assert(target:GetOwner()==0 and cities:GetCount()==n+1 and target:GetID()~=capital:GetID(),'native noncapital creation failed')
assert(cities:GetCapitalCity():GetID()==capital:GetID(),'recipient capital changed')
local function identity(c) return {Owner=c:GetOwner(),ID=c:GetID(),X=c:GetX(),Y=c:GetY()} end
local e={Label='DEDICATED_EXISTING_ORIGINAL_ARCHIVE_SEED_NOT_ADMISSION_OR_FLOW',Recipient=0,Donor=2,
 Token='fixture-native-hotseat-996',Source=identity(source),Target=identity(target),Capital=identity(capital),
 FreeCity=assert(PlayerManager.GetFreeCitiesPlayerID()),Fixtures={}}
Game:SetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996',e)
return 'created genuine extra recipient city; fixture archive seed is separate from normal admission'"#;

const PLAN_TAIL: &str = r#"local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local target=assert(CityManager.GetCityAt(e.Target.X,e.Target.Y))
assert(target:GetOwner()==0 and target:GetID()==e.Target.ID,'fresh native city not published')
ExposedMembers.JAC996TemplePlan=F.Plan(target,'carrier')
return 'planned stock Temple on a plot purchased by the exact extra recipient city'"#;

const PERSIST_PLAN: &str = r#"local A=assert(JAC_AbsorptionGameplay);local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local plan=assert(ExposedMembers.JAC996TemplePlan)
local c=assert(CityManager.GetCityAt(plan.X,plan.Y))
assert(c:GetOwner()==0 and c:GetID()==e.Target.ID and plan.ID==e.Target.ID,'detached Temple plan changed')
e.Fixtures.carrier=A.Rules.Copy(plan)
-- Use the existing registered governance property to make restoration non-vacuous.
assert(c:GetProperty(A.Rules.GOVERNANCE_KEY)==nil,'fresh city has unexpected governance')
c:SetProperty(A.Rules.GOVERNANCE_KEY,{ID='fixture-native-hotseat-996',From=2,To=0,FrictionPerTurn=0,
 Fixture='DEDICATED_ARCHIVE_SEED',Nested={Original=true,Value=996}})
Game:SetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996',e)
return 'persisted detached stock Temple plan and registered city property sentinel'"#;

const PALACE: &str = r#"local A=assert(ExposedMembers.JAC_Absorption)
local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local c=assert(CityManager.GetCityAt(e.Source.X,e.Source.Y));local palace=assert(GameInfo.Buildings.BUILDING_PALACE).Index
assert(c:GetOwner()==2 and c:GetID()==e.Source.ID and Players[2]:GetCities():GetCapitalCity():GetID()==c:GetID(),'AI source capital changed')
local b=c:GetBuildings()
assert(b:HasBuilding(palace) and b:GetNumGreatWorkSlots(palace)==1 and b:GetGreatWorkInSlot(palace,0)==-1,'AI Palace is not empty')
local before={}
for _,owner in ipairs(PlayerManager.GetAliveIDs()) do for _,city in Players[owner]:GetCities():Members() do
 local buildings=city:GetBuildings()
 for row in GameInfo.Buildings() do if buildings:HasBuilding(row.Index) then
  for slot=0,buildings:GetNumGreatWorkSlots(row.Index)-1 do
   local index=buildings:GetGreatWorkInSlot(row.Index,slot);if index>=0 then before[index]=true end
  end
 end end
end end
ExposedMembers.JAC996GrantPlan={Before=before,Palace=palace,Owner=2,City=c:GetID(),X=c:GetX(),Y=c:GetY()}
local target=assert(CityManager.GetCityAt(e.Target.X,e.Target.Y))
local temple=assert(GameInfo.Buildings.BUILDING_TEMPLE).Index
ExposedMembers.JAC996CityBefore=A.ReadNativeCarrier(target,{Building=temple,Slot=0,X=e.Target.X,Y=e.Target.Y,Owner=0})
assert(ExposedMembers.JAC996CityBefore.Work==-1,'fresh recipient Temple occupied')
local cap=assert(Players[0]:GetCities():GetCapitalCity())
local count=0
for _,s in ipairs(assert(ExposedMembers.JAC_HeroReadAPI).FindCompatibleNativeSlots(0,'GREATWORKOBJECT_RELIC')) do
 if s.City~=cap:GetID() then
  assert(s.City==e.Target.ID and s.Building==temple and s.Slot==0,'another recipient noncapital slot would bypass the fixture city')
  count=count+1
 end
end
assert(count==1,'exactly one eligible noncapital stock relic destination required')
local units={};local keys=A.GetPropertyKeys()
for _,u in Players[0]:GetUnits():Members() do
 units[u:GetID()]={Owner=u:GetOwner(),ID=u:GetID(),Type=u:GetType(),X=u:GetX(),Y=u:GetY(),
  Name=u:GetName(),Damage=u:GetDamage(),Moves=u:GetMovesRemaining(),BuildCharges=u:GetBuildCharges(),
  Person=u:GetProperty(keys.PERSON_KEY),Token=u:GetProperty(keys.TOKEN_KEY)}
end
ExposedMembers.JAC996RecipientUnits=units
return 'observed empty AI Palace and sole eligible recipient noncapital Temple'"#;

const GRANT: &str = r#"local A=assert(JAC_AbsorptionGameplay);local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local p=assert(ExposedMembers.JAC996GrantPlan);local c=assert(CityManager.GetCityAt(p.X,p.Y))
local r=assert(ExposedMembers.JAC_NativeTempleObservation.carrier);local req=assert(e.Fixtures.carrier.Request)
assert(req.Stage=='Temple' and r.Token=='carrier:grant' and r.Read>req.Read and r.Temple and r.SlotValue==-1,'fresh stock Temple publication missing')
assert(p.Owner==2 and c:GetOwner()==2 and c:GetID()==e.Source.ID and p.City==e.Source.ID,'Palace grant plan changed')
assert(not e.GrantIssued,'official modifier may be used exactly once')
e.GrantIssued=true;e.Before=A.Rules.Copy(p.Before);e.Palace=p.Palace;e.CityBefore=A.Rules.Copy(assert(ExposedMembers.JAC996CityBefore))
e.RecipientUnits=A.Rules.Copy(assert(ExposedMembers.JAC996RecipientUnits))
Game:SetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996',e)
Players[2]:AttachModifierByID('GOODY_CULTURE_GRANT_ONE_RELIC')
return 'issued exactly one official relic grant to AI2, without creating any Great Work'"#;

const ORIGINAL: &str = r#"local A=assert(ExposedMembers.JAC_Absorption);local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local source=assert(CityManager.GetCityAt(e.Source.X,e.Source.Y))
assert(source:GetOwner()==2 and source:GetID()==e.Source.ID,'source capital changed before original observation')
local index=source:GetBuildings():GetGreatWorkInSlot(e.Palace,0)
if index==-1 then return false end
assert(type(index)=='number' and index>=0 and not e.Before[index],'official grant did not produce a fresh actual index')
local found=A.ReadNativeLocations(index)
assert(#found==1,'new original index is missing or duplicated in actual world')
local w=found[1]
assert(w.Owner==2 and w.CityID==e.Source.ID and w.X==e.Source.X and w.Y==e.Source.Y
 and w.Building==e.Palace and w.BuildingType=='BUILDING_PALACE' and w.Slot==0,'new original did not occupy AI Palace')
assert(w.ObjectType=='GREATWORKOBJECT_RELIC','official grant is not a relic')
for _,key in ipairs({'Index','GreatWorkType','ObjectType','CreatorName','TurnCreated','CreatedByPlayer','Effects'}) do
 assert(w[key]~=nil,'actual original metadata missing: '..key)
end
ExposedMembers.JAC996Original=w
return true"#;

const ARM_BORROW: &str = r#"assert(JAC996BorrowObserver==nil,'borrow observer already armed')
local original=assert(JAC_ObserveNativeCarrierMP,'product MP observer missing in HeroKeys UI context')
JAC996BorrowObserver=original
JAC_ObserveNativeCarrierMP=function(c)
 if c and c.MP and c.MP.Recovery=='fixture-native-hotseat-996' and c.MP.Phase=='BORROWED' then return false end
 return original(c)
end
return 'transient HeroKeys observer stops only this fixture at BORROWED; load removes wrapper'"#;

const SEED: &str = r#"local A=assert(JAC_AbsorptionGameplay);local R=A.Rules
local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local original=assert(ExposedMembers.JAC996Original,'actual original UI observation absent')
assert(e.GrantIssued and original.Owner==2 and original.CityID==e.Source.ID and not e.Before[original.Index],'fixture original changed')
e.Original=R.Copy(original);e.AssetID='native-work:'..original.Index
local s=A.GetState();assert(not s.Archive[e.AssetID] and not s.Recoveries[e.Token],'fixture identity already admitted')
-- Dedicated fixture archive seed of an EXISTING native original. This is not
-- normal admission, a scatter outcome, or evidence that full Flow succeeded.
s.Archive[e.AssetID]={ID=e.AssetID,Kind='ITEM',Owner=-1,NativeOwnerMoves=0,NativeWork=R.Copy(original),
 FixtureSeed=e.Label}
Game:SetProperty(R.STATE_KEY,s);Game:SetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996',e)
local r=A.RecoverArchived(e.AssetID,0,e.Token)
assert(r.Status=='WAITING_UI' and r.ObservationToken==e.Token..':inventory','public recovery did not request synchronized NativeInventory')
assert(not A.GetState().Recoveries[e.Token] or not A.GetState().Recoveries[e.Token].NativeCarrier,'initial inventory bypassed synchronized publication')
return 'public RecoverArchived waiting for product NativeInventory; no Gameplay UI fallback'"#;

const BORROWED: &str = r#"local A=assert(ExposedMembers.JAC_Absorption);local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local r=A.GetState().Recoveries[e.Token];local c=r and r.NativeCarrier;local m=c and c.MP
if not m or m.Phase~='BORROWED' then return false end
assert(r.Status=='WAITING_UI' and m.Actor==0 and m.MoveOwner==2 and m.SourceOwnerMove==true,'wrong MP recovery actor or MOVE owner')
assert(c.Index==e.Original.Index and #m.Loans==2 and not m.Claimed and not c.MoveToken,'MOVE was claimed before mid-loan save')
local l=m.Loans[2];local target=assert(CityManager.GetCityAt(e.Target.X,e.Target.Y))
assert(l.Owner==0 and l.ID==e.Target.ID and l.X==e.Target.X and l.Y==e.Target.Y and l.Issued==true
 and l.BorrowedID==target:GetID() and target:GetOwner()==2 and not l.ReturnIssued,'actual destination city not borrowed to AI2')
local source=assert(CityManager.GetCityAt(e.Source.X,e.Source.Y));local first=m.Loans[1]
assert(source:GetOwner()==2 and source:GetID()==e.Source.ID and first.Owner==2 and not first.Issued
 and Players[2]:GetCities():GetCapitalCity():GetID()==e.Source.ID,'AI source capital was borrowed or replaced')
assert(source:GetBuildings():GetGreatWorkInSlot(e.Palace,e.Original.Slot)==e.Original.Index,'original left Palace before mid-loan save')
local cap=assert(Players[0]:GetCities():GetCapitalCity())
assert(cap:GetID()==e.Capital.ID and cap:GetX()==e.Capital.X and cap:GetY()==e.Capital.Y,'recipient source capital lost')
ExposedMembers.JAC996Borrowed={Owner=target:GetOwner(),ID=target:GetID(),Token=c.Token,Nonce=m.Nonce,
 SourceID=source:GetID(),TargetOriginalID=l.ID,Phase=m.Phase}
return true"#;

const SAVE_BORROWED: &str = r#"local A=assert(JAC_AbsorptionGameplay);local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local b=assert(ExposedMembers.JAC996Borrowed,'fresh actual borrowed observation absent')
local r=assert(A.GetState().Recoveries[e.Token]);local m=assert(r.NativeCarrier.MP)
local city=assert(CityManager.GetCityAt(e.Target.X,e.Target.Y))
assert(m.Phase=='BORROWED' and b.Phase==m.Phase and b.Nonce==m.Nonce and city:GetOwner()==b.Owner
 and city:GetID()==b.ID and b.Owner==2 and not m.Claimed,'borrowed checkpoint changed')
e.Borrowed=A.Rules.Copy(b)
Game:SetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996',e)
return A.Canonical({Borrowed=e.Borrowed,Recovery=r})"#;

const ARM_MOVE: &str = r#"assert(JAC996BorrowObserver==nil,'transient borrow wrapper survived actual load')
assert(type(JAC_ObserveNativeCarrierMP)=='function','unmodified product observer absent after load')
local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local r=assert(ExposedMembers.JAC_Absorption.GetState().Recoveries[e.Token]);local m=assert(r.NativeCarrier.MP)
-- The load-step hook runs before Begin Game publishes LoadScreenClose or
-- accepts the HotseatStartTurn. Keep the phase guard as a real-engine check.
assert(m.Phase=='BORROWED' or m.Phase=='READY',
 'native MOVE observation unavailable: load advanced to '..tostring(m.Phase)..' before event hook; driver pre-publication hook required')
assert(not m.Claimed and e.Borrowed.Phase=='BORROWED','loaded checkpoint was already dispatched')
local events={}
local expectedType=assert(GameInfo.GreatWorks[e.Original.GreatWorkType]).Index
local targetBuilding=assert(m.Loans[2]).Building
local function moved(fromOwner,fromCity,toOwner,toCity,building,greatWorkType)
 -- GreatWorksOverview.lua:1045 defines these six parameters. City ownership
 -- notifications and unrelated works must not count as this native MOVE.
 if fromOwner~=2 or fromCity~=e.Source.ID or toOwner~=2 or toCity~=e.Borrowed.ID
  or building~=targetBuilding or greatWorkType~=expectedType then return end
 local row={fromOwner,fromCity,toOwner,toCity,building,greatWorkType}
 events[#events+1]={Turn=Game.GetCurrentGameTurn(),Arguments=row}
 print('[JAC_NATIVE_HOTSEAT_MOVE_EVENT] count='..#events..' args='..ExposedMembers.JAC_Absorption.Canonical(row))
end
assert(Events.GreatWorkMoved and Events.GreatWorkMoved.Add,'native GreatWorkMoved event unavailable')
Events.GreatWorkMoved.Add(moved)
ExposedMembers.JAC996MoveEvents=events
JAC996StopMoves=function() Events.GreatWorkMoved.Remove(moved) end
return true"#;

const RETURNED: &str = r#"local A=assert(ExposedMembers.JAC_Absorption);local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local r=A.GetState().Recoveries[e.Token];local c=r and r.NativeCarrier;local m=c and c.MP
if not m or m.Phase~='DONE' then return false end
local city=assert(CityManager.GetCityAt(e.Target.X,e.Target.Y));local l=assert(m.Loans[2])
assert(c.Phase=='PROVED' and l.ReturnIssued and l.RestoredCore and city:GetOwner()==0 and city:GetID()==l.FinalID,'actual borrowed city not returned and restored')
return true"#;

const FINALIZE: &str = r#"local A=assert(JAC_AbsorptionGameplay);local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local before=assert(A.GetState().Recoveries[e.Token]);assert(before.NativeCarrier.MP.Phase=='DONE','unmodified executor did not finish')
local events=assert(ExposedMembers.JAC996MoveEvents,'native MOVE event observer absent')
assert(#events==1,'expected exactly one real native GreatWorkMoved event, observed '..#events)
e.NativeMoveEvents=A.Rules.Copy(events);e.NativeMoveCount=#events
Game:SetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996',e)
-- Finalize the public recovery after automatic MP executor completion.
-- No direct MP.Advance, TransferCity, MOVE request, or record patching.
local r=A.RecoverArchived(e.AssetID,0,e.Token)
assert(r.Status=='COMPLETE' and r.NativePhysicalMoved and r.NativeMoveEvidenceKind=='SYNCHRONIZED_ACTOR_OBSERVATION','public native recovery incomplete')
return 'public recovery complete, with one observed native move event'"#;

const VERIFY: &str = r#"local A=assert(ExposedMembers.JAC_Absorption);local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local function equal(a,b) return A.Canonical(a)==A.Canonical(b) end
local state=A.GetState();local r=assert(state.Recoveries[e.Token]);local c=assert(r.NativeCarrier);local m=assert(c.MP)
assert(e.Label=='DEDICATED_EXISTING_ORIGINAL_ARCHIVE_SEED_NOT_ADMISSION_OR_FLOW','fixture seed label missing')
assert(e.NativeMoveCount==1 and #e.NativeMoveEvents==1,'native event count must equal one')
assert(r.Status=='COMPLETE' and r.NativePhysicalMoved and not r.NativeAlreadyOwned
 and r.NativeMoveEvidenceKind=='SYNCHRONIZED_ACTOR_OBSERVATION','missing native MOVE recovery proof')
assert(c.Phase=='PROVED' and m.Phase=='DONE' and m.Actor==0 and m.MoveOwner==2 and m.SourceOwnerMove
 and m.Claimed and type(m.ClaimNonce)=='number' and type(m.DispatchSession)=='string' and type(c.MoveToken)=='string'
 and not m.AbortReason and (m.AbortCycles or 0)==0,'expected one successful product dispatch claim')
local moved=assert(m.ReportedMovedObservation);local final=assert(m.FinalActorObservation)
assert(#moved.Locations==1 and moved.Locations[1].Index==e.Original.Index and moved.Locations[1].Owner==2,'missing synchronized borrowed MOVE evidence')
assert(#final.Locations==1 and final.Locations[1].Index==e.Original.Index and final.Locations[1].Owner==0,'missing synchronized final world uniqueness evidence')
local l=assert(m.Loans[2]);local target=assert(CityManager.GetCityAt(e.Target.X,e.Target.Y))
assert(l.Owner==0 and l.ID==e.Target.ID and l.BorrowedID==e.Borrowed.ID and l.Issued and l.ReturnIssued
 and l.FinalID==target:GetID() and l.RestoredCore and target:GetOwner()==0,'loan identities do not match actual saved/returned city')
local source=assert(CityManager.GetCityAt(e.Source.X,e.Source.Y));local cap=assert(Players[2]:GetCities():GetCapitalCity())
assert(source:GetOwner()==2 and source:GetID()==e.Source.ID and cap:GetID()==e.Source.ID and not m.Loans[1].Issued,'original AI capital was not retained')
assert(source:GetBuildings():GetGreatWorkInSlot(e.Palace,e.Original.Slot)==-1,'original Palace slot was reused')
local recipientCap=assert(Players[0]:GetCities():GetCapitalCity())
assert(recipientCap:GetID()==e.Capital.ID and recipientCap:GetX()==e.Capital.X and recipientCap:GetY()==e.Capital.Y,'recipient capital changed')
local found=A.ReadNativeLocations(e.Original.Index)
assert(#found==1,'actual original duplicated or missing in current world')
local w=found[1]
assert(w.Owner==0 and w.CityID==target:GetID() and w.X==e.Target.X and w.Y==e.Target.Y
 and w.Building==l.Building and w.Slot==l.Slot and w.BuildingType=='BUILDING_TEMPLE','actual original missed returned native Temple')
for _,key in ipairs({'Index','GreatWorkType','ObjectType','CreatorName','TurnCreated','CreatedByPlayer','Effects'}) do
 assert(equal(w[key],e.Original[key]),'original provenance/effects changed: '..key)
end
local record=assert(A.GetRoster(0)[e.AssetID])
assert(record.RecoveryToken==e.Token and record.NativeOwnerMoves==1 and record.FixtureSeed==e.Label,'recovery roster differs from fixture original')
assert(equal(record.NativeWork,w) and record.NativeCustody.Owner==0 and record.NativeCustody.City==target:GetID()
 and record.NativeCustody.Building==w.Building and record.NativeCustody.Slot==w.Slot,'roster differs from actual world')
local after=A.ReadNativeCarrier(target,l)
assert(after.Work==e.Original.Index and equal(after.State,e.CityBefore.State)
 and equal(after.Properties,e.CityBefore.Properties),'recipient city assets, religion, properties, units or purchased plots changed')
assert(equal(l.CoreBefore.Properties,l.RestoredCore.Properties) and equal(l.CoreBefore.UnitsOnCenter,l.RestoredCore.UnitsOnCenter),'trusted Core properties/units not preserved')
assert(equal(l.ActorBefore.State,after.State),'actual city differs from saved actor baseline')
local units={};local keys=A.GetPropertyKeys()
for _,u in Players[0]:GetUnits():Members() do
 units[u:GetID()]={Owner=u:GetOwner(),ID=u:GetID(),Type=u:GetType(),X=u:GetX(),Y=u:GetY(),
  Name=u:GetName(),Damage=u:GetDamage(),Moves=u:GetMovesRemaining(),BuildCharges=u:GetBuildCharges(),
  Person=u:GetProperty(keys.PERSON_KEY),Token=u:GetProperty(keys.TOKEN_KEY)}
end
assert(equal(units,e.RecipientUnits),'recipient native units changed during the saved city loan')
print('[JAC_NATIVE_HOTSEAT_SAVE_PROOF] index='..w.Index..' moves='..e.NativeMoveCount..' borrowedID='..l.BorrowedID..' returnedID='..l.FinalID)
return 'actual original recovered once; native city returned with provenance and world uniqueness'"#;

const SNAPSHOT: &str = r#"local A=assert(ExposedMembers.JAC_Absorption);local e=assert(Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'))
local world={};local rosters={};local owners={};local seen={}
for _,id in ipairs(PlayerManager.GetAliveIDs()) do if not seen[id] then owners[#owners+1]=id;seen[id]=true end end
if not seen[e.FreeCity] then owners[#owners+1]=e.FreeCity end;table.sort(owners)
for _,owner in ipairs(owners) do
 local p=assert(Players[owner]);local cities={};local units={};rosters[owner]=A.GetRoster(owner)
 for _,city in p:GetCities():Members() do
  local b=city:GetBuildings();local buildings={}
  for row in GameInfo.Buildings() do if b:HasBuilding(row.Index) then
   local works={};for slot=0,b:GetNumGreatWorkSlots(row.Index)-1 do works[slot]=b:GetGreatWorkInSlot(row.Index,slot) end
   buildings[row.BuildingType]={Pillaged=b:IsPillaged(row.Index),Works=works}
  end end
  local plots={}
  for _,index in pairs(assert(Map.GetCityPlots():GetPurchasedPlots(city))) do
   local q=assert(Map.GetPlotByIndex(index));local buyer=assert(Cities.GetPlotPurchaseCity(q))
   plots[#plots+1]={Index=index,Owner=q:GetOwner(),City=buyer:GetID(),X=buyer:GetX(),Y=buyer:GetY(),
    District=q:GetDistrictType(),Improvement=q:GetImprovementType(),Terrain=q:GetTerrainType()}
  end
  table.sort(plots,function(a,b) return a.Index<b.Index end)
  local religions={};for _,row in pairs(city:GetReligion():GetReligionsInCity()) do religions[tostring(row.Religion)]=row.Followers end
  cities[city:GetID()]={Owner=city:GetOwner(),ID=city:GetID(),X=city:GetX(),Y=city:GetY(),Name=city:GetName(),
   Population=city:GetPopulation(),Buildings=buildings,PurchasedPlots=plots,Religions=religions,
   Loyalty=city:GetCulturalIdentity():GetLoyalty(),Faith=city:GetProperty('JAC_ABSORPTION_CITY_FAITH_V1'),
   Governance=city:GetProperty('JAC_ABSORPTION_GOVERNANCE_V1')}
 end
 for _,u in p:GetUnits():Members() do
  units[u:GetID()]={Owner=u:GetOwner(),ID=u:GetID(),Type=u:GetType(),X=u:GetX(),Y=u:GetY(),
   Name=u:GetName(),Damage=u:GetDamage(),Moves=u:GetMovesRemaining(),BuildCharges=u:GetBuildCharges()}
 end
 local cap=p:GetCities():GetCapitalCity()
 world[owner]={Capital=cap and cap:GetID(),Cities=cities,Units=units}
end
return A.Canonical({Ledger=A.GetState(),Rosters=rosters,
 NativeInventory=Game:GetProperty('JAC_NATIVE_INVENTORY_V1'),Boundary=Game:GetProperty('JAC_CARRIER_BOUNDARY_V1'),
 Fixture=e,World=world,OriginalLocations=A.ReadNativeLocations(e.Original.Index)})"#;

fn ui(state: &str, code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code.into()})
}
fn wait(state: &str, code: impl Into<String>, timeout: u32) -> Value {
    json!({"action":"wait_for","timeout":timeout,"condition":{"type":"lua","state_name":state,"code":code.into()}})
}

fn generate() -> Value {
    let base: Value = serde_json::from_str(BASE).unwrap();
    let original_steps = base["steps"].as_array().unwrap();
    let mut steps = original_steps[..5].to_vec();
    steps[1]["build_setup"] = json!(true);
    for (i, p) in steps[1]["players"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        p["human"] = json!(i < 2);
    }
    steps[4] = wait("InGame", START_READY, 240);
    steps.extend([ui("InGame", SITE), ui("JAC_Absorption_Entry", CREATE)]);
    steps.push(wait("InGame", "local e=Game:GetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996');local c=e and CityManager.GetCityAt(e.Target.X,e.Target.Y);return c and c:GetOwner()==0 and c:GetID()==e.Target.ID", 45));
    // Reuse the audited stock getters and fresh publication checks verbatim.
    let temple = original_steps[6]["code"].as_str().unwrap();
    let prefix = temple
        .split("\nlocal p=Players[Game.GetLocalPlayer()]")
        .next()
        .unwrap();
    assert!(prefix.ends_with("ExposedMembers.JAC_NativeTempleFixture=F"));
    let fixture = format!("{prefix}\n{PLAN_TAIL}").replace("JAC_NATIVE_CUSTODY_PROBE", KEY);
    steps.push(ui("InGame", fixture));
    steps.push(ui("JAC_Absorption_Entry", PERSIST_PLAN));
    steps.push(ui(
        "JAC_Absorption_Entry",
        original_steps[8]["code"]
            .as_str()
            .unwrap()
            .replace("JAC_NATIVE_CUSTODY_PROBE", KEY),
    ));
    for stage in ["Holy", "Shrine", "Temple"] {
        steps.push(ui("InGame", format!("local r=assert(ExposedMembers.JAC_NativeTempleFixture).Observe('carrier','{stage}');return 'observed carrier {stage} city '..r.ID")));
        steps.push(ui(
            "JAC_Absorption_Entry",
            format!("return assert(ExposedMembers.JAC_NativeTempleRequest)('carrier','{stage}')"),
        ));
        steps.push(wait(
            "InGame",
            format!(
                "return assert(ExposedMembers.JAC_NativeTempleFixture).Ready('carrier','{stage}')"
            ),
            45,
        ));
    }
    steps.extend([
        ui("InGame", "local r=assert(ExposedMembers.JAC_NativeTempleFixture).Observe('carrier','grant');assert(r.Temple and r.SlotValue==-1,'stock Temple not empty');return 'fresh empty stock Temple published'"),
        ui("InGame", PALACE), ui("JAC_Absorption_Entry", GRANT), wait("InGame", ORIGINAL, 45),
        ui(HERO_UI, ARM_BORROW), ui("JAC_Absorption_Entry", SEED), wait("InGame", BORROWED, 120),
        ui("JAC_Absorption_Entry", SAVE_BORROWED),
    ]);
    steps.extend([
        json!({"action":"save","name":"JAC-native-hotseat-996-borrowed","timeout":90}),
        json!({"action":"exit_to_menu","timeout":90}),
        json!({"action":"load","name":"JAC-native-hotseat-996-borrowed","timeout":600,
          "before_begin_lua":{"state_name":HERO_UI,"code":ARM_MOVE}}),
        wait("InGame", RETURNED, 120), ui("JAC_Absorption_Entry", FINALIZE),
        ui(HERO_UI, "assert(JAC996StopMoves)();JAC996StopMoves=nil;return 'removed fixture native event observer'"),
        ui("InGame", VERIFY),
    ]);
    let mut snapshot = ui("InGame", SNAPSHOT);
    snapshot["capture_as"] = json!("NATIVE_HOTSEAT_COMPLETE_LEDGER_WORLD");
    steps.push(snapshot);
    steps.extend([
        json!({"action":"save","name":"JAC-native-hotseat-996-complete","timeout":90}),
        json!({"action":"exit_to_menu","timeout":90}),
        json!({"action":"load","name":"JAC-native-hotseat-996-complete","timeout":600}),
        wait("InGame", START_READY, 120),
        ui("InGame", VERIFY),
    ]);
    let mut compare = ui("InGame", SNAPSHOT);
    compare["equals_capture"] = json!("NATIVE_HOTSEAT_COMPLETE_LEDGER_WORLD");
    steps.push(compare);
    json!({"name":"Native carrier hotseat mid-loan save/load with dedicated existing-original archive seed",
      "judge_profile":"generic","session_type":"hotseat","strict_lua_errors":true,"steps":steps})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scenario = generate();
    civ6_autotest_rs::scenario::validate_scenario(&scenario)?;
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../civ6-autotest/scenarios")
        .join(OUTPUT);
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&scenario)?),
    )?;
    println!("{}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test doubles exercise rejection of evidence. They are never emitted into
    // the real-game scenario and are not a claim of native engine acceptance.
    fn verifier_world() -> mlua::Lua {
        let lua = mlua::Lua::new();
        lua.load(include_str!(
            "../../../../poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Rules.lua"
        ))
        .exec()
        .unwrap();
        lua.load(r#"
local R=JAC_AbsorptionRules
W={Index=37,Owner=0,CityID=23,X=4,Y=5,Building=7,BuildingType='BUILDING_TEMPLE',Slot=0,
 GreatWorkType='REAL_RELIC',ObjectType='GREATWORKOBJECT_RELIC',CreatorName='creator',TurnCreated=1,CreatedByPlayer=2,Effects={'YIELD_FAITH 4','TOURISM 8'}}
local original=R.Copy(W);original.Owner=2;original.CityID=2;original.X=1;original.Y=1;original.Building=8;original.BuildingType='BUILDING_PALACE'
CITY_STATE={Name='native city',Population=1,Assets={Temple={Works={[0]=-1}}},Religion={},Loyalty=100,
 Governance={Fixture='DEDICATED_ARCHIVE_SEED'},UnitsOnCenter={},PurchasedPlots={11,12}}
AFTER={Work=37,State=R.Copy(CITY_STATE),Properties={sentinel={Value=996}}}
local l={Owner=0,ID=10,X=4,Y=5,Building=7,Slot=0,BorrowedID=22,FinalID=23,Issued=true,ReturnIssued=true,
 CoreBefore={Properties=R.Copy(AFTER.Properties),UnitsOnCenter={}},RestoredCore={Properties=R.Copy(AFTER.Properties),UnitsOnCenter={}},
 ActorBefore={State=R.Copy(CITY_STATE)}}
E={Label='DEDICATED_EXISTING_ORIGINAL_ARCHIVE_SEED_NOT_ADMISSION_OR_FLOW',Token='fixture-native-hotseat-996',AssetID='native-work:37',
 NativeMoveCount=1,NativeMoveEvents={{Turn=1,Arguments={37}}},Original=original,Palace=8,
 Source={ID=2,X=1,Y=1},Target={ID=10,X=4,Y=5},Capital={ID=1,X=8,Y=8},Borrowed={ID=22},
 CityBefore={State=R.Copy(CITY_STATE),Properties=R.Copy(AFTER.Properties)},RecipientUnits={}}
M={Phase='DONE',Actor=0,MoveOwner=2,SourceOwnerMove=true,Claimed=true,ClaimNonce=4,DispatchSession='loaded-ui-session',
 Loans={{Owner=2,ID=2},l},ReportedMovedObservation={Locations={{Index=37,Owner=2}}},FinalActorObservation={Locations={R.Copy(W)}}}
C={Phase='PROVED',MP=M,MoveToken='fixture-move-once'}
RECOVERY={Status='COMPLETE',NativePhysicalMoved=true,NativeMoveEvidenceKind='SYNCHRONIZED_ACTOR_OBSERVATION',NativeCarrier=C}
STATE={Recoveries={['fixture-native-hotseat-996']=RECOVERY}}
RECORD={RecoveryToken=E.Token,NativeOwnerMoves=1,FixtureSeed=E.Label,NativeWork=R.Copy(W),NativeCustody={Owner=0,City=23,Building=7,Slot=0}}
local function city(owner,id,x,y)
 return {GetOwner=function() return owner end,GetID=function() return id end,GetX=function() return x end,GetY=function() return y end,
 GetBuildings=function() return {GetGreatWorkInSlot=function() return -1 end} end}
end
SOURCE=city(2,2,1,1);TARGET=city(0,23,4,5);CAPITAL=city(0,1,8,8)
CityManager={GetCityAt=function(x,y) if x==1 and y==1 then return SOURCE elseif x==4 and y==5 then return TARGET end end}
Players={[2]={GetCities=function() return {GetCapitalCity=function() return SOURCE end} end},
 [0]={GetCities=function() return {GetCapitalCity=function() return CAPITAL end} end,
 GetUnits=function() return {Members=function() return function() return nil end end} end}}
LOCATIONS={W}
Game={GetProperty=function(_,k) assert(k=='JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996');return E end}
ExposedMembers={JAC_Absorption={Canonical=R.Canonical,GetState=function() return STATE end,
 GetRoster=function() return {[E.AssetID]=RECORD} end,ReadNativeLocations=function() return LOCATIONS end,
 ReadNativeCarrier=function() return AFTER end,GetPropertyKeys=function() return {} end}}
print=function() end
"#).exec().unwrap();
        lua
    }

    #[test]
    fn verifier_rejects_missing_or_corrupted_native_evidence() {
        let good = verifier_world();
        good.load(VERIFY).eval::<String>().unwrap();
        for (mutation, reason) in [
            ("E.NativeMoveCount=0", "native event count"),
            ("E.NativeMoveEvents[2]={}", "native event count"),
            ("M.Claimed=false", "dispatch claim"),
            (
                "M.ReportedMovedObservation.Locations={}",
                "borrowed MOVE evidence",
            ),
            (
                "M.FinalActorObservation.Locations[2]=W",
                "final world uniqueness",
            ),
            ("M.Loans[2].BorrowedID=99", "loan identities"),
            ("LOCATIONS={W,W}", "duplicated or missing"),
            (
                "W.CreatorName='replacement creator'",
                "provenance/effects changed",
            ),
            ("W.Effects={'YIELD_FAITH 0'}", "provenance/effects changed"),
            ("W.CityID=99", "returned native Temple"),
            ("AFTER.Properties.sentinel.Value=0", "recipient city assets"),
            ("AFTER.State.PurchasedPlots={11}", "recipient city assets"),
            (
                "AFTER.State.UnitsOnCenter={{Owner=0,ID=99}}",
                "recipient city assets",
            ),
            (
                "E.RecipientUnits={[9]={Owner=0,ID=9}}",
                "recipient native units changed",
            ),
        ] {
            let lua = verifier_world();
            lua.load(mutation).exec().unwrap();
            let error = lua.load(VERIFY).eval::<String>().unwrap_err();
            assert!(error.to_string().contains(reason), "{mutation}: {error}");
        }
    }

    #[test]
    fn transient_observer_only_holds_the_matching_borrowed_fixture() {
        let lua = mlua::Lua::new();
        lua.load("CALLS=0;JAC_ObserveNativeCarrierMP=function(c) CALLS=CALLS+1;return true end")
            .exec()
            .unwrap();
        lua.load(ARM_BORROW).eval::<String>().unwrap();
        lua.load(r#"
local c={MP={Recovery='fixture-native-hotseat-996',Phase='BORROWED'}}
assert(not JAC_ObserveNativeCarrierMP(c) and CALLS==0 and c.MP.Phase=='BORROWED')
c.MP.Phase='PREPARE';assert(JAC_ObserveNativeCarrierMP(c) and CALLS==1)
c.MP.Phase='BORROWED';c.MP.Recovery='other';assert(JAC_ObserveNativeCarrierMP(c) and CALLS==2)
assert(JAC_ObserveNativeCarrierMP(nil) and CALLS==3)
JAC_ObserveNativeCarrierMP=JAC996BorrowObserver;JAC996BorrowObserver=nil
assert(JAC_ObserveNativeCarrierMP({MP={Recovery='fixture-native-hotseat-996',Phase='BORROWED'}}) and CALLS==4)
"#).exec().unwrap();
    }

    #[test]
    fn pre_begin_event_observer_filters_exact_native_source_target_and_type() {
        let lua = verifier_world();
        lua.load(r#"
          M.Phase='BORROWED';M.Claimed=nil;M.Loans[2].Building=7
          E.Source.ID=11;E.Borrowed.Phase='BORROWED';E.Borrowed.ID=22
          E.Original.GreatWorkType='WORK_RELIC'
          GameInfo={GreatWorks={WORK_RELIC={Index=9}}}
          Game.GetCurrentGameTurn=function() return 1 end
          JAC_ObserveNativeCarrierMP=function() return true end
          Events={GreatWorkMoved={Add=function(fn) MOVE_HANDLER=fn end,Remove=function(fn) assert(fn==MOVE_HANDLER) end}}
        "#).exec().unwrap();
        assert!(lua.load(ARM_MOVE).eval::<bool>().unwrap());
        lua.load(
            r#"
          MOVE_HANDLER(2,11,0,22,7,9) -- city return/ownership event
          MOVE_HANDLER(2,12,2,22,7,9) -- another source
          MOVE_HANDLER(2,11,2,23,7,9) -- another target
          MOVE_HANDLER(2,11,2,22,8,9) -- another building
          MOVE_HANDLER(2,11,2,22,7,10) -- another work type
          assert(#ExposedMembers.JAC996MoveEvents==0)
          MOVE_HANDLER(2,11,2,22,7,9)
          assert(#ExposedMembers.JAC996MoveEvents==1)
          JAC996StopMoves()
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn post_load_observation_race_is_explicitly_rejected() {
        let lua = verifier_world();
        lua.load("JAC_ObserveNativeCarrierMP=function() return true end;HOOKS=0;Events={GreatWorkMoved={Add=function() HOOKS=HOOKS+1 end}}")
            .exec().unwrap();
        let error = lua.load(ARM_MOVE).eval::<String>().unwrap_err();
        assert!(error
            .to_string()
            .contains("driver pre-publication hook required"));
        assert_eq!(lua.globals().get::<i64>("HOOKS").unwrap(), 0);
    }

    #[test]
    fn scenario_validates_real_hotseat_and_all_lua51_chunks() {
        let scenario = generate();
        civ6_autotest_rs::scenario::validate_scenario(&scenario).unwrap();
        let lua = mlua::Lua::new();
        for step in scenario["steps"].as_array().unwrap() {
            if let Some(code) = step
                .get("code")
                .or_else(|| step["condition"].get("code"))
                .or_else(|| step["before_begin_lua"].get("code"))
                .and_then(Value::as_str)
            {
                lua.load(code).into_function().unwrap();
            }
        }
        let config = &scenario["steps"][1];
        assert_eq!(config["players"].as_array().unwrap().len(), 22);
        assert_eq!(
            config["players"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p["human"] == true)
                .count(),
            2
        );
        assert_eq!(scenario["strict_lua_errors"], true);
        assert_eq!(config["build_setup"], true);
        for mutation in ["one_human", "single_player", "invalid_strict"] {
            let mut bad = scenario.clone();
            match mutation {
                "one_human" => bad["steps"][1]["players"][1]["human"] = json!(false),
                "single_player" => bad["session_type"] = json!("single_player"),
                _ => bad["strict_lua_errors"] = json!(1),
            }
            assert!(
                civ6_autotest_rs::scenario::validate_scenario(&bad).is_err(),
                "{mutation}"
            );
        }
    }

    #[test]
    fn shipped_output_matches_and_save_load_names_use_driver_suffixes() {
        let scenario = generate();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../civ6-autotest/scenarios")
            .join(OUTPUT);
        assert_eq!(
            scenario,
            serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap()
        );
        let steps = scenario["steps"].as_array().unwrap();
        let saves: Vec<_> = steps.iter().filter(|s| s["action"] == "save").collect();
        let loads: Vec<_> = steps.iter().filter(|s| s["action"] == "load").collect();
        assert_eq!(saves.len(), 2);
        assert_eq!(loads.len(), 2);
        for (save, load) in saves.into_iter().zip(loads) {
            assert_eq!(save["name"], load["name"]);
            assert!(save.get("exact_name").is_none() && load.get("exact_name").is_none());
        }
        let grant_code = steps
            .iter()
            .filter_map(|s| s["code"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            grant_code
                .matches("AttachModifierByID('GOODY_CULTURE_GRANT_ONE_RELIC')")
                .count(),
            1
        );
        assert!(!grant_code.contains("UI.RequestPlayerOperation"));
        assert!(!grant_code.contains("CityManager.TransferCity("));
        assert!(grant_code.contains(TOKEN));
    }
}
