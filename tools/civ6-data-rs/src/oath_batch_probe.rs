//! One launch and one native save/load measure at least ten yield-only oaths,
//! each on the real cities of a different house (the PR114 batching policy).
//! Generated Lua is test code, never a MOD script.
//!
//! Quantities keep the single-oath audit contract: `LUA_HELPERS` step inference,
//! a native 100-percent reference and the same tolerance per city. One house at a
//! time goes OFF→ON→OFF while every other house keeps its state, so every other
//! city is a control relative to its own house's state. A last phase switches all
//! houses on together, where each house legitimately carries its own effect. The
//! human house chooses through the real DedicationPopup; the AI houses receive the
//! audit's ledger fixture and diagnostic flags, never reported as an official
//! selection. The verifier recomputes every verdict from the recorded samples and
//! accepts only what both the game and Rust accept.
use crate::oath_effect_audit::{first_steps, BASE, LUA_HELPERS, POPUP};
use crate::oath_expiry_probe::Frame;
use crate::oath_save_probe::{
    core_code, loaded_ready, lua, recorded_save_name, write_new, HELPERS,
};
use crate::{oaths, Repository};
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

/// Houses with three yield-only oaths need three batches; each keeps at least ten.
pub const BATCHES: usize = 3;
const MIN_CASES: usize = 10;
const CITIES: usize = 56;
const END: &str = "|END-JAC-OATH-SAVE";
/// Per-city values in comparison order: six yields, overall and other growth.
const KEYS: [&str; 8] = [
    "production",
    "gold",
    "science",
    "culture",
    "faith",
    "food",
    "growth",
    "other_growth",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Yield {
    Production,
    Gold,
    Science,
    Culture,
    Faith,
    Food,
}

impl Yield {
    pub const ALL: [Yield; 6] = [
        Yield::Production,
        Yield::Gold,
        Yield::Science,
        Yield::Culture,
        Yield::Faith,
        Yield::Food,
    ];
    pub fn key(self) -> &'static str {
        KEYS[self as usize]
    }
    fn parse(kind: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|y| y.key() == kind)
    }
}

/// A yield-only oath of one house. Growth, military, naval and walls stay outside.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub house: String,
    pub civ: String,
    pub oath: String,
    pub effects: [(Yield, i64); 2],
}

fn lua_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serializes")
}

/// Every catalogue oath whose two effects are city yields, in catalogue order.
pub fn eligible(repo: &Repository) -> Result<Vec<Case>> {
    let id = |value: &Value| -> Result<String> {
        let id = value.as_str().context("catalogue id")?;
        ensure!(
            id.starts_with(|c: char| c.is_ascii_uppercase())
                && id
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'),
            "invalid catalogue id {id}"
        );
        Ok(id.to_owned())
    };
    let houses = oaths::catalogue(repo)?;
    ensure!(houses.len() == 22, "expected the 22 map houses");
    let (mut cases, mut total) = (Vec::new(), 0);
    for house in &houses {
        let house_id = id(&house["id"])?;
        for oath in house["oaths"].as_array().context("house oaths")? {
            total += 1;
            let effects = oath["effects"]
                .as_array()
                .context("oath effects")?
                .iter()
                .map(|pair| {
                    let kind = pair[0].as_str().context("effect kind")?;
                    let amount = pair[1]
                        .as_i64()
                        .filter(|amount| *amount > 0)
                        .context("positive effect amount")?;
                    Ok(Yield::parse(kind).map(|y| (y, amount)))
                })
                .collect::<Result<Vec<_>>>()?;
            ensure!(effects.len() == 2, "an oath has exactly two effects");
            if let [Some(first), Some(second)] = effects[..] {
                ensure!(first.0 != second.0, "an oath's effects use distinct yields");
                cases.push(Case {
                    oath: format!("JAC_OATH_{house_id}_{}", id(&oath["id"])?),
                    civ: format!("CIVILIZATION_JAC_{house_id}"),
                    house: house_id.clone(),
                    effects: [first, second],
                });
            }
        }
    }
    ensure!(total == 66, "expected the 66-oath catalogue");
    Ok(cases)
}

/// Round-robin over the eligible oaths in catalogue order: a house has at most
/// three, so its oaths always land in different batches.
pub fn batch(repo: &Repository, index: usize) -> Result<Vec<Case>> {
    ensure!(
        (1..=BATCHES).contains(&index),
        "batch must be 1..={BATCHES}"
    );
    let cases: Vec<Case> = eligible(repo)?
        .into_iter()
        .skip(index - 1)
        .step_by(BATCHES)
        .collect();
    let houses: BTreeSet<_> = cases.iter().map(|c| &c.house).collect();
    ensure!(
        houses.len() == cases.len(),
        "batch {index} would give one house two oaths"
    );
    ensure!(
        cases.len() >= MIN_CASES,
        "batch {index} has fewer than {MIN_CASES} oaths"
    );
    Ok(cases)
}

/// The human house selects its oath through the real UI; the others are fixtures.
fn selection_path(case: usize) -> &'static str {
    if case == 0 {
        "real_ui_selection"
    } else {
        "ledger_fixture_selection"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Baseline,
    PrecisionOne,
    PrecisionTwo,
    PrecisionOff,
    Reference,
    ReferenceOff,
    /// Only this case's flags are on.
    On(usize),
    /// Every flag is off again after `On(case)`.
    Off(usize),
    /// Every case's flags are on, as the ledger records them.
    AllOn,
    /// Not measured: the cleared state the native save writes, and its reload.
    Saved,
    Loaded,
}

impl Phase {
    pub fn name(self) -> String {
        match self {
            Phase::Baseline => "baseline".into(),
            Phase::PrecisionOne => "precision_one".into(),
            Phase::PrecisionTwo => "precision_two".into(),
            Phase::PrecisionOff => "precision_off".into(),
            Phase::Reference => "reference".into(),
            Phase::ReferenceOff => "reference_off".into(),
            Phase::On(case) => format!("on_{}", case + 1),
            Phase::Off(case) => format!("off_{}", case + 1),
            Phase::AllOn => "all_on".into(),
            Phase::Saved => "saved".into(),
            Phase::Loaded => "loaded".into(),
        }
    }
    /// Lua set of the 1-based cases whose flags are on.
    fn on_lua(self, cases: usize) -> String {
        let on: Vec<usize> = match self {
            Phase::On(case) => vec![case],
            Phase::AllOn | Phase::Saved | Phase::Loaded => (0..cases).collect(),
            _ => vec![],
        };
        let entries: Vec<_> = on.iter().map(|c| format!("[{}]=true", c + 1)).collect();
        format!("{{{}}}", entries.join(","))
    }
}

/// Calibration as in the single-oath audit, then OFF→ON→OFF per case, then all on.
pub fn schedule(cases: usize) -> Vec<Phase> {
    let mut phases = vec![
        Phase::Baseline,
        Phase::PrecisionOne,
        Phase::PrecisionTwo,
        Phase::PrecisionOff,
        Phase::Reference,
        Phase::ReferenceOff,
    ];
    for case in 0..cases {
        phases.extend([Phase::On(case), Phase::Off(case)]);
    }
    phases.push(Phase::AllOn);
    phases
}

/// What a generated step is for. The verifier finds steps by role, never by number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Start,
    Meter,
    Fixture,
    Snapshot(Phase),
    Synced(Phase),
    Measure(Phase),
    Popup,
    Chosen,
    LedgerChoice,
    Flag,
    Compare,
    CoreFrame { after: bool },
    UiFrame { after: bool },
    Save,
    Exit,
    Load,
    Ready,
}

const PRELUDE: &str = r#"
local BATCH=__BATCH__
local CASES=__CASES__
local KINDS={'production','gold','science','culture','faith','food'}
local function majors()
 local list=PlayerManager.GetAliveMajors(); assert(#list==22,'expected 22 actual map houses')
 return list
end
local SUBJECT={}
for i,case in ipairs(CASES) do
 local found=nil
 for _,p in ipairs(majors()) do
  if PlayerConfigurations[p:GetID()]:GetCivilizationTypeName()==case.civ then
   assert(found==nil,'duplicate batch house '..case.civ); found=p:GetID()
  end
 end
 assert(found~=nil,'batch house is not an alive major: '..case.civ)
 assert(SUBJECT[found]==nil,'two batch oaths share one house')
 SUBJECT[found]=i; case.player=found
end
assert(CASES[1].player==0,'the first batch oath belongs to the human house')
local function cities()
 local rows={}
 for _,p in ipairs(majors()) do
  local id=p:GetID()
  for _,c in p:GetCities():Members() do
   local plot=assert(Map.GetPlot(c:GetX(),c:GetY()))
   assert(c:GetOwner()==id and plot:GetOwner()==id,'actual owner/plot mismatch')
   rows[#rows+1]={city=c,plot=plot,index=plot:GetIndex(),id=c:GetID(),owner=id}
  end
 end
 table.sort(rows,function(a,b) return a.index<b.index end)
 assert(#rows==56,'15km map city count changed: '..#rows)
 for i=2,#rows do assert(rows[i].index~=rows[i-1].index,'duplicate city plot') end
 return rows
end
local function yield_index(kind) return assert(GameInfo.Yields['YIELD_'..string.upper(kind)],'yield definition missing').Index end
"#;

const PHASE: &str = r#"
local PHASE=__PHASE__
local ON=__ON__
-- Every one of the 66 flags of a real city: only its own house's oath, only when on.
local function flags_match(row)
 local case=SUBJECT[row.owner]; local total=0; local ok=true
 for entry in GameInfo.JAC_SengokuOaths() do
  total=total+1
  local want=(case and ON[case] and entry.OathType==CASES[case].oath) and 1 or 0
  if (row.plot:GetProperty(entry.OathType..'_ON') or 0)~=want then ok=false end
 end
 assert(total==66,'loaded oath inventory changed')
 return ok
end
"#;

const ATTACH: &str = r#"
assert(not Game:GetProperty('JAC_TODO134_METER_ATTACHED') and not Game:GetProperty('JAC_TODO134_BATCH_METER_ATTACHED'),'reference modifiers already attached')
for _,case in ipairs(CASES) do
 local p=assert(Players[case.player])
 assert(type(p.AttachModifierByID)=='function','official player modifier attachment unavailable')
 for _,kind in ipairs(KINDS) do
  for _,mode in ipairs({'BASE','CAL','FINE','SHIFT'}) do p:AttachModifierByID('JAC_TODO134_'..mode..'_'..string.upper(kind)) end
 end
end
Game:SetProperty('JAC_TODO134_BATCH_METER_ATTACHED',true)
"#;

/// The audit's meter inputs, on the cities of every batch house only.
const METER: &str = r#"
assert(Game:GetProperty('JAC_TODO134_BATCH_METER_ATTACHED'),'batch reference not attached')
local fine,shift,cal,base=__FINE__,__SHIFT__,__CAL__,__BASE__
for _,r in ipairs(cities()) do
 if SUBJECT[r.owner] then
  r.plot:SetProperty('JAC_TODO134_FINE_ON',fine); r.plot:SetProperty('JAC_TODO134_SHIFT_ON',shift); r.plot:SetProperty('JAC_TODO134_CAL_ON',cal)
  for _,kind in ipairs(KINDS) do r.plot:SetProperty('JAC_TODO134_BASE_'..string.upper(kind),base) end
 end
end
return string.format('batch diagnostic meter fine=%d shift=%d cal=%d base=%d on %d houses; no natural balance claim',fine,shift,cal,base,#CASES)
"#;

const FIXTURE: &str = r#"
local l=assert(Game:GetProperty('JAC_AGES_LEDGER'),'saved age ledger missing')
assert(l.reign==0,'fixture must start in Tenbun before first judgement')
assert(Players[0]:IsHuman(),'player 0 must be the human house')
local catalogue=0
for _ in GameInfo.JAC_SengokuOaths() do catalogue=catalogue+1 end
assert(catalogue==66,'loaded catalogue is not the 66-oath inventory')
local owned={}
for _,r in ipairs(cities()) do
 owned[r.owner]=(owned[r.owner] or 0)+1
 for entry in GameInfo.JAC_SengokuOaths() do assert((r.plot:GetProperty(entry.OathType..'_ON') or 0)==0,'another oath active before fixture') end
end
for _,p in ipairs(majors()) do assert((owned[p:GetID()] or 0)>0,'house has no owned city: '..p:GetID()) end
local turn=Game.GetCurrentGameTurn()
l.reign=1
for i,case in ipairs(CASES) do
 local h=assert(l.houses['p'..case.player],'batch house missing from ledger')
 assert(h.civ==case.civ,'ledger house differs from the batch oath')
 assert(Players[case.player]:IsHuman()==(i==1),'only the first batch house may be human')
 -- Only the age is forced; no oath is chosen here.
 h.age='GOLDEN'; h.oaths={}; h.oathReign=1; h.oathChosen=nil; h.oathDue=turn+20
end
Game:SetProperty('JAC_AGES_LEDGER',l)
for _,r in ipairs(cities()) do if SUBJECT[r.owner] then r.plot:SetProperty('JAC_AGE_GOLDEN',1) end end
return 'synthetic golden-age fixture for '..#CASES..' houses; no oath chosen'
"#;

/// Core first: identity, all 66 flags and six yields of every real city. Rows
/// use string keys, as every live-proven saved property does (`p0`, `plot123`).
const SNAPSHOT: &str = r#"
local snapshot={}; local count=0
for _,r in ipairs(cities()) do
 assert(flags_match(r),'Core oath flags differ from phase '..PHASE..' at plot '..r.index)
 local y={}
 for _,kind in ipairs(KINDS) do y[kind]=number(r.city:GetYield(yield_index(kind)),'Core '..kind) end
 count=count+1; snapshot['c'..count]={id=r.id,owner=r.owner,plot=r.index,y=y}
end
Game:SetProperty('JAC_TODO134_BATCH_CORE',{batch=BATCH,phase=PHASE,turn=Game.GetCurrentGameTurn(),count=count,cities=snapshot})
return '{"scope":"core_identity_flags_and_yields","batch":'..BATCH..',"phase":"'..PHASE..'","cities":'..count..'}'
"#;

/// Waits until the UI shows the same flags and yields as the Core snapshot.
const SYNCED: &str = r#"
local core=Game:GetProperty('JAC_TODO134_BATCH_CORE')
if not core or core.batch~=BATCH or core.phase~=PHASE then return false end
assert(core.turn==Game.GetCurrentGameTurn(),'batch observation turn changed')
local rows=cities(); assert(core.count==#rows,'Core batch inventory incomplete')
for i,r in ipairs(rows) do
 local c=assert(core.cities['c'..i],'Core batch city missing')
 assert(c.id==r.id and c.owner==r.owner and c.plot==r.index,'Core/UI batch city identity changed')
 if not flags_match(r) then return false end
 for _,kind in ipairs(KINDS) do
  local v=r.city:GetYield(yield_index(kind))
  if type(v)~='number' or not (math.abs(v-c.y[kind])<=0.00002) then return false end
 end
end
return true
"#;

/// The audit's UI observation of every real city, recorded with exact doubles.
const MEASURE: &str = r#"
local core=assert(Game:GetProperty('JAC_TODO134_BATCH_CORE'),'Core batch snapshot missing')
assert(core.batch==BATCH and core.phase==PHASE and core.turn==Game.GetCurrentGameTurn(),'Core batch snapshot is not this phase')
if PHASE=='baseline' then assert(JAC_OATH_BATCH==nil,'batch UI samples already exist'); JAC_OATH_BATCH={batch=BATCH,samples={},order={}} end
local B=assert(JAC_OATH_BATCH,'baseline batch UI context missing')
assert(B.batch==BATCH and B.samples[PHASE]==nil,'do not overwrite a batch observation')
local rows=cities(); assert(core.count==#rows,'Core batch inventory incomplete')
local samples,parts={},{}
for i,r in ipairs(rows) do
 local c=assert(core.cities['c'..i],'Core batch city missing')
 assert(c.id==r.id and c.owner==r.owner and c.plot==r.index,'Core/UI batch city identity changed')
 assert(flags_match(r),'oath flag differs from phase '..PHASE..' at plot '..r.index)
 local growth=assert(r.city:GetGrowth(),'city growth API unavailable')
 local s={id=r.id,plot=r.index,owner=r.owner,y={}}
 for _,kind in ipairs(KINDS) do
  s.y[kind]=number(r.city:GetYield(yield_index(kind)),'city '..r.id..' '..kind)
  assert(math.abs(s.y[kind]-c.y[kind])<=0.00002,'Core and UI yields differ at city '..r.id..' '..kind)
 end
 s.growth=number(growth:GetOverallGrowthModifier(),'overall growth modifier')
 s.other=number(growth:GetOtherGrowthModifier(),'other growth modifier')
 s.housing=number(growth:GetHousingGrowthModifier(),'housing growth modifier')
 s.happiness=number(growth:GetHappinessGrowthModifier(),'happiness growth modifier')
 s.surplus=number(growth:GetFoodSurplus(),'food surplus')
 s.occupied=r.city:IsOccupied(); assert(type(s.occupied)=='boolean','occupation API not boolean')
 s.loyalty=number(growth:GetLoyaltyGrowthModifier(),'loyalty growth modifier')
 samples[i]=s
 parts[i]=string.format('{"city":%d,"plot":%d,"owner":%d,"growth":%.17g,"other_growth":%.17g,"housing":%.17g,"happiness":%.17g,"food_surplus":%.17g,"occupied":%s,"loyalty":%.17g,"production":%.17g,"gold":%.17g,"science":%.17g,"culture":%.17g,"faith":%.17g,"food":%.17g}',s.id,s.plot,s.owner,s.growth,s.other,s.housing,s.happiness,s.surplus,tostring(s.occupied),s.loyalty,s.y.production,s.y.gold,s.y.science,s.y.culture,s.y.faith,s.y.food)
end
B.samples[PHASE]=samples; B.order[#B.order+1]=PHASE
return '{"batch":'..BATCH..',"phase":"'..PHASE..'","cities":['..table.concat(parts,',')..']}'
"#;

/// The audit's fixture method for houses the UI cannot select for: a legal
/// golden choice written to the ledger, flags left to the diagnostic switch.
const LEDGER_CHOICE: &str = r#"
local l=assert(Game:GetProperty('JAC_AGES_LEDGER')); assert(l.reign==1,'fixture reign changed')
local function count(t) local n=0; for _ in pairs(t or {}) do n=n+1 end; return n end
local human=assert(l.houses.p0)
assert(human.civ==CASES[1].civ and human.oathChosen==1 and human.oaths[CASES[1].oath]==true and count(human.oaths)==1,'real UI selection of the human house is missing')
local rows,ages={},{}
for row in GameInfo.JAC_SengokuOaths() do rows[row.OathType]=row end
for row in GameInfo.JAC_SengokuOathAges() do ages[row.OathType..'|'..row.AgeType]=true end
for i=2,#CASES do
 local case=CASES[i]; local h=assert(l.houses['p'..case.player])
 assert(h.civ==case.civ and h.age=='GOLDEN' and h.oathReign==1 and h.oathChosen==nil and count(h.oaths)==0,'ledger fixture must start without a choice')
 local row=assert(rows[case.oath],'batch oath missing from the loaded catalogue')
 assert(row.CivilizationType==case.civ and ages[case.oath..'|GOLDEN'],'batch oath is not a legal golden choice of its house')
 h.oaths={[case.oath]=true}; h.oathChosen=1
end
Game:SetProperty('JAC_AGES_LEDGER',l)
return 'ledger fixture choice for '..(#CASES-1)..' AI houses; official UI selection only for the human house'
"#;

const FLAG: &str = r#"
local targets=__TARGETS__; local desired=__FLAG__
local l=assert(Game:GetProperty('JAC_AGES_LEDGER'))
local function count(t) local n=0; for _ in pairs(t or {}) do n=n+1 end; return n end
for _,i in ipairs(targets) do
 local case=CASES[i]; local h=assert(l.houses['p'..case.player])
 assert(h.oathChosen==1 and h.oaths[case.oath]==true and count(h.oaths)==1,'selected oath was not persisted')
 local switched=0
 for _,r in ipairs(cities()) do
  if r.owner==case.player then
   r.plot:SetProperty(case.oath..'_ON',desired)
   assert(r.plot:GetProperty(case.oath..'_ON')==desired,'diagnostic flag did not switch'); switched=switched+1
  end
 end
 assert(switched>0,'batch house has no owned city')
end
return 'diagnostic flag='..desired..' for '..#targets..' houses; saved selections unchanged'
"#;

/// Per case: its own OFF→ON→OFF against the phase just before it, every other
/// city as a control in that same window, the audit's quantity contract against
/// the native 100-percent reference, and the joint phase against each house's own
/// ON. Calibration faults invalidate the whole batch; case faults only the case.
const COMPARE: &str = r#"
local ORDER=__ORDER__
local B=assert(JAC_OATH_BATCH,'batch UI samples missing')
assert(B.batch==BATCH and not B.compared,'batch comparison already ran')
assert(#B.order==#ORDER,'batch phase count differs')
for i,name in ipairs(ORDER) do assert(B.order[i]==name,'batch phase order differs') end
local function close(a,b) return finite(a) and finite(b) and math.abs(a-b)<=0.00002 end
local S=B.samples
local base=assert(S.baseline)
local position={}
for i,name in ipairs(ORDER) do
 position[name]=i
 local rows=assert(S[name],'batch phase missing: '..name); assert(#rows==#base,'batch city inventory changed in '..name)
 for j,r in ipairs(rows) do local a=base[j]; assert(r.id==a.id and r.plot==a.plot and r.owner==a.owner,'batch city identity changed in '..name) end
end
local function differs(a,b)
 for _,kind in ipairs(KINDS) do if not close(a.y[kind],b.y[kind]) then return kind end end
 if not close(a.growth,b.growth) then return 'growth' end
 if not close(a.other,b.other) then return 'other_growth' end
 return nil
end
for i,a in ipairs(base) do
 for _,name in ipairs({'precision_off','reference_off'}) do
  local kind=differs(a,S[name][i]); assert(kind==nil,'diagnostic meter did not restore '..tostring(kind)..' at city '..a.id)
 end
 if not SUBJECT[a.owner] then
  for _,name in ipairs({'precision_one','precision_two','reference'}) do
   local kind=differs(a,S[name][i]); assert(kind==nil,'diagnostic meter leaked to non-batch city '..a.id..' '..tostring(kind))
  end
 end
end
local verdicts,leaked={},{}
for ci,case in ipairs(CASES) do
 local reasons,results={},{}
 local function fail(text) reasons[#reasons+1]=text end
 leaked[ci]={}
 local prev,on,off=S[ORDER[position['on_'..ci]-1]],S['on_'..ci],S['off_'..ci]
 local amounts,proven={},{}
 for _,e in ipairs(case.effects) do amounts[e.kind]=e.amount; proven[e.kind]=0 end
 for i,a in ipairs(base) do
  local kind=differs(prev[i],off[i]); if kind then fail('restore:'..a.id..':'..kind) end
  if a.owner==case.player then
   local drift=differs(a,prev[i]); if drift then fail('drift:'..a.id..':'..drift) end
   -- As in the audit, overall growth may follow a food oath; the other-growth modifier may not.
   for _,k in ipairs(KINDS) do if amounts[k]==nil and not close(prev[i].y[k],on[i].y[k]) then fail('non_target:'..a.id..':'..k) end end
   if not close(prev[i].other,on[i].other) then fail('non_target:'..a.id..':other_growth') end
   for _,e in ipairs(case.effects) do
    local av,bv=a.y[e.kind],on[i].y[e.kind]; local observed=bv-av
    local ref=S.reference[i].y[e.kind]; local hundred=ref-av
    local step,tolerance,expected,quantity,reason=nil,nil,nil,false,nil
    if not (finite(hundred) and hundred>0.03) then
     reason='reference_unmeasurable'; fail('reference:'..a.id..':'..e.kind)
    else
     step=reported_step({av,ref,S.precision_one[i].y[e.kind],S.precision_two[i].y[e.kind]})
     tolerance=step and 2*step*(1+e.amount/100)+0.00004
     if tolerance and tolerance<hundred*0.005 then
      expected=hundred*e.amount/100
      if math.abs(observed-expected)<=tolerance then quantity=true; reason='native_100_percent_reference_delta'; proven[e.kind]=proven[e.kind]+1
      else reason='wrong_quantity'; fail('quantity:'..a.id..':'..e.kind) end
     else reason='native_reference_resolution_insufficient'; fail('resolution:'..a.id..':'..e.kind) end
    end
    results[#results+1]=string.format('{"city":%d,"kind":"%s","off":%.17g,"on":%.17g,"off_again":%.17g,"expected_percent":%d,"reference_delta":%.17g,"expected_delta":%s,"observed_delta":%.17g,"reported_step_upper_bound":%s,"tolerance":%s,"quantity_verified":%s,"reason":"%s"}',a.id,e.kind,av,bv,off[i].y[e.kind],e.amount,hundred,expected and string.format('%.17g',expected) or 'null',observed,step and string.format('%.17g',step) or 'null',tolerance and string.format('%.17g',tolerance) or 'null',tostring(quantity),reason)
   end
  else
   local leak=differs(prev[i],on[i]); if leak then fail('leak:'..a.id..':'..leak); leaked[ci][a.id]=true end
  end
 end
 for _,e in ipairs(case.effects) do if proven[e.kind]==0 then fail('unproven:'..e.kind) end end
 verdicts[ci]={reasons=reasons,results=results}
end
-- All houses on: each city carries exactly its own house's effect.
local last=S[ORDER[position.all_on-1]]
for i,a in ipairs(base) do
 local owner=SUBJECT[a.owner]
 local kind=differs(owner and S['on_'..owner][i] or last[i],S.all_on[i])
 if kind then
  local blamed=false
  for ci in ipairs(CASES) do
   if leaked[ci][a.id] then blamed=true; local r=verdicts[ci].reasons; r[#r+1]='simultaneous:'..a.id..':'..kind end
  end
  if not blamed and owner then local r=verdicts[owner].reasons; r[#r+1]='simultaneous:'..a.id..':'..kind
  elseif not blamed then for _,v in ipairs(verdicts) do v.reasons[#v.reasons+1]='simultaneous_unattributed:'..a.id..':'..kind end end
 end
end
local parts,verified={},0
for ci,case in ipairs(CASES) do
 local v=verdicts[ci]; local ok=#v.reasons==0; if ok then verified=verified+1 end
 local reasons={}; for _,r in ipairs(v.reasons) do reasons[#reasons+1]=jstr(r) end
 parts[ci]='{"oath":'..jstr(case.oath)..',"house":'..jstr(case.house)..',"player":'..case.player..',"selection_path":"'..(ci==1 and 'real_ui_selection' or 'ledger_fixture_selection')..'","quantity_verified":'..tostring(ok)..',"reasons":['..table.concat(reasons,',')..'],"results":['..table.concat(v.results,',')..']}'
end
B.compared=true; B.case_count=#CASES
return '{"batch":'..BATCH..',"quantity_verified_count":'..verified..',"cases":['..table.concat(parts,',')..']}'
"#;

/// The save probe's UI frame without queue targets: yield-only oaths build nothing.
const UI_FRAME: &str = r#"
local before=__BEFORE__
if before then assert(JAC_OATH_BATCH and JAC_OATH_BATCH.batch==__BATCH__ and JAC_OATH_BATCH.compared==true and JAC_OATH_BATCH.case_count==__COUNT__,'the batch quantity comparison must precede the native save') end
assert(Players[0]:IsTurnActive() and Players[0]:IsTurnActiveComplete(),'human start not complete')
assert(UI.CanEndTurn() and not UI.HasSentTurnComplete() and not UI.IsProcessingMessages(),'human UI not ready')
local cities={}
for plotKey,r in pairs(inventory()) do
 local c=r.object; local g=assert(c:GetGrowth()); local q=assert(c:GetBuildQueue())
 local row={id=r.id,owner=r.owner,yields={},growth={},queue={hash=q:GetCurrentProductionTypeHash()}}
 for _,kind in ipairs({'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}) do row.yields[kind]=number(c:GetYield(GameInfo.Yields['YIELD_'..kind].Index)) end
 for _,method in ipairs({'GetOverallGrowthModifier','GetOtherGrowthModifier','GetHousingGrowthModifier','GetHappinessGrowthModifier','GetFoodSurplus','GetLoyaltyGrowthModifier'}) do row.growth[method]=number(g[method](g)) end
 cities[plotKey]=row
end
local signature=ser({turn=Game.GetCurrentGameTurn(),cities=cities})
return 'JAC_OATH_BATCH_UI___PHASE__|batch-__BATCH__|'..#signature..':'..signature..'|END-JAC-OATH-SAVE'
"#;

/// A generated batch: the typed cases and every step tagged with its role.
pub struct Probe {
    pub batch: usize,
    pub cases: Vec<Case>,
    steps: Vec<(Role, Value)>,
}

impl Probe {
    pub fn new(repo: &Repository, batch: usize) -> Result<Self> {
        let mut probe = Probe {
            batch,
            cases: self::batch(repo, batch)?,
            steps: Vec::new(),
        };
        for step in first_steps(&repo.json(BASE)?, &probe.cases[0].civ)? {
            probe.steps.push((Role::Start, step));
        }
        probe.core(
            Role::Meter,
            format!("{}{ATTACH}{}", probe.prelude(), meter_code([0, 0, 0, 1])),
        );
        probe.core(Role::Fixture, format!("{}{FIXTURE}", probe.prelude()));
        let n = probe.cases.len();
        let all: Vec<usize> = (0..n).collect();
        for phase in schedule(n) {
            match phase {
                Phase::Baseline => {}
                Phase::PrecisionOne => probe.meter([1, 0, 0, 1]),
                Phase::PrecisionTwo => probe.meter([1, 1, 0, 1]),
                Phase::PrecisionOff | Phase::ReferenceOff => probe.meter([0, 0, 0, 1]),
                Phase::Reference => probe.meter([0, 0, 1, 1]),
                Phase::On(0) => probe.select(),
                Phase::On(case) => probe.flag(&[case], 1),
                Phase::Off(case) => probe.flag(&[case], 0),
                Phase::AllOn => probe.flag(&all, 1),
                Phase::Saved | Phase::Loaded => bail!("save phases are not measured"),
            }
            probe.measure(phase);
        }
        let order: Vec<_> = schedule(n).iter().map(|p| lua_string(&p.name())).collect();
        let compare = COMPARE.replace("__ORDER__", &format!("{{{}}}", order.join(",")));
        probe.ui(
            Role::Compare,
            format!("{LUA_HELPERS}{}{compare}", probe.prelude()),
        );
        probe.meter([0, 0, 0, 0]);
        probe.save_suffix();
        Ok(probe)
    }

    fn prelude(&self) -> String {
        let cases: Vec<_> = self
            .cases
            .iter()
            .map(|c| {
                let effects: Vec<_> = c
                    .effects
                    .iter()
                    .map(|(kind, amount)| {
                        format!("{{kind={},amount={amount}}}", lua_string(kind.key()))
                    })
                    .collect();
                format!(
                    "{{oath={},civ={},house={},effects={{{}}}}}",
                    lua_string(&c.oath),
                    lua_string(&c.civ),
                    lua_string(&c.house),
                    effects.join(",")
                )
            })
            .collect();
        PRELUDE
            .replace("__BATCH__", &self.batch.to_string())
            .replace("__CASES__", &format!("{{{}}}", cases.join(",")))
    }

    fn phased(&self, phase: Phase) -> String {
        let code = PHASE
            .replace("__PHASE__", &lua_string(&phase.name()))
            .replace("__ON__", &phase.on_lua(self.cases.len()));
        format!("{}{code}", self.prelude())
    }

    fn core(&mut self, role: Role, code: String) {
        self.steps.push((
            role,
            json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":code}),
        ));
    }

    fn ui(&mut self, role: Role, code: String) {
        self.steps.push((
            role,
            json!({"action":"ui_lua","state_name":"InGame","code":code}),
        ));
    }

    fn wait(&mut self, role: Role, state: &str, code: String) {
        self.steps.push((
            role,
            json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":state,"code":code}}),
        ));
    }

    fn meter(&mut self, state: [u8; 4]) {
        let code = format!("{}{}", self.prelude(), meter_code(state));
        self.core(Role::Meter, code);
    }

    /// Core snapshot, then wait until the UI shows the same flags and yields.
    fn sync(&mut self, phase: Phase) {
        let phased = self.phased(phase);
        self.core(
            Role::Snapshot(phase),
            format!("{LUA_HELPERS}{phased}{SNAPSHOT}"),
        );
        self.wait(Role::Synced(phase), "InGame", format!("{phased}{SYNCED}"));
    }

    fn measure(&mut self, phase: Phase) {
        self.sync(phase);
        let code = format!("{LUA_HELPERS}{}{MEASURE}", self.phased(phase));
        self.ui(Role::Measure(phase), code);
    }

    /// The human house through the real DedicationPopup, then the AI fixture.
    fn select(&mut self) {
        let human = &self.cases[0];
        let (oath, civ) = (lua_string(&human.oath), lua_string(&human.civ));
        let popup = POPUP.replace("__OATH__", &oath).replace("__CIV__", &civ);
        self.steps.push((
            Role::Popup,
            json!({"action":"ui_lua","state_name":"DedicationPopup","code":popup}),
        ));
        self.wait(
            Role::Chosen,
            "GameCore_Tuner",
            format!("local l=Game:GetProperty('JAC_AGES_LEDGER'); local h=l and l.houses.p0; return h and h.oathChosen==1 and h.oaths[{oath}]==true"),
        );
        self.core(
            Role::LedgerChoice,
            format!("{}{LEDGER_CHOICE}", self.prelude()),
        );
    }

    fn flag(&mut self, cases: &[usize], value: u8) {
        let targets: Vec<_> = cases.iter().map(|c| (c + 1).to_string()).collect();
        let code = FLAG
            .replace("__TARGETS__", &format!("{{{}}}", targets.join(",")))
            .replace("__FLAG__", &value.to_string());
        self.core(Role::Flag, format!("{}{code}", self.prelude()));
    }

    /// The save probe's native save, menu exit and reload around strict frames.
    /// The UI frames are read only once the UI shows the Core state.
    fn save_suffix(&mut self) {
        let oath = self.cases[0].oath.clone();
        let name = format!("JAC-Todo134-Batch-{}", self.batch);
        for after in [false, true] {
            if after {
                self.steps.extend([
                    (
                        Role::Save,
                        json!({"action":"save","name":name,"timeout":180}),
                    ),
                    (Role::Exit, json!({"action":"exit_to_menu","timeout":180})),
                    (
                        Role::Load,
                        json!({"action":"load","name":name,"timeout":600}),
                    ),
                    (Role::Ready, loaded_ready()),
                ]);
            } else {
                self.sync(Phase::Saved);
            }
            self.steps.push((
                Role::CoreFrame { after },
                lua(core_code(&oath, after), "GameCore_Tuner"),
            ));
            if after {
                self.sync(Phase::Loaded);
            }
            let frame = UI_FRAME
                .replace("__BEFORE__", if after { "false" } else { "true" })
                .replace("__PHASE__", if after { "AFTER" } else { "BEFORE" })
                .replace("__BATCH__", &self.batch.to_string())
                .replace("__COUNT__", &self.cases.len().to_string());
            self.steps.push((
                Role::UiFrame { after },
                lua(format!("{HELPERS}{frame}"), "InGame"),
            ));
        }
    }

    fn position(&self, role: Role) -> Result<usize> {
        let found: Vec<usize> = (0..self.steps.len())
            .filter(|&i| self.steps[i].0 == role)
            .collect();
        ensure!(
            found.len() == 1,
            "generated probe needs exactly one {role:?} step"
        );
        Ok(found[0])
    }

    fn identity(&self) -> Value {
        let cases: Vec<_> = self
            .cases
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let effects: Vec<_> = c
                    .effects
                    .iter()
                    .map(|(kind, amount)| json!({"kind":kind.key(),"expected_percent":amount}))
                    .collect();
                json!({"oath":c.oath,"house":c.house,"civilization":c.civ,"selection_path":selection_path(i),"effects":effects})
            })
            .collect();
        let phases: Vec<_> = schedule(self.cases.len())
            .iter()
            .map(|p| p.name())
            .collect();
        json!({"batch":self.batch,"batches":BATCHES,"scope":"yield_quantities_and_native_save_only","cases":cases,"phases":phases,"requires_offline_verification":true,"task_accepted":false,"remaining":["natural_age_transition","official_ui_selection_for_ledger_fixture_houses"]})
    }

    pub fn scenario(&self) -> Result<Value> {
        let steps: Vec<_> = self.steps.iter().map(|(_, step)| step.clone()).collect();
        let scenario = json!({
            "name": format!("誓約のまとめ測定{}：産出だけの誓約{}件を1回の起動と1回の正式保存で測る", self.batch, self.cases.len()),
            "judge_profile": "generic",
            "diagnostic_only": true,
            "finite_turns": 0,
            "batch_probe": self.identity(),
            "steps": steps,
        });
        civ6_autotest_rs::scenario::validate_scenario(&scenario)?;
        Ok(scenario)
    }
}

fn meter_code([fine, shift, cal, base]: [u8; 4]) -> String {
    METER
        .replace("__FINE__", &fine.to_string())
        .replace("__SHIFT__", &shift.to_string())
        .replace("__CAL__", &cal.to_string())
        .replace("__BASE__", &base.to_string())
}

/// The audit's `reported_step`, with Lua 5.1 number semantics.
pub(crate) fn reported_step(values: &[f64]) -> Option<f64> {
    if values.len() < 3 || values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let gcd = |mut a: f64, mut b: f64| {
        while b != 0.0 {
            (a, b) = (b, a - (a / b).floor() * b);
        }
        a
    };
    let mut upper: Option<f64> = None;
    for scale in [
        1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0, 1024.0, 2048.0, 4096.0, 10.0,
        100.0, 1000.0, 10000.0,
    ] {
        let mut divisor = 0.0;
        let mut represented = true;
        for value in &values[1..] {
            let difference = value - values[0];
            let ticks = (difference * scale + 0.5).floor();
            if ticks.abs() > 9007199254740991.0 || (difference - ticks / scale).abs() > 0.00001 {
                represented = false;
                break;
            }
            divisor = gcd(divisor, ticks.abs());
        }
        if represented && divisor > 0.0 {
            upper = Some(upper.unwrap_or(0.0).max(divisor / scale));
        }
    }
    upper.map(|u| u + 0.00002)
}

/// One measured city: identity and the `KEYS` values.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sample {
    pub city: i64,
    pub plot: i64,
    pub owner: i64,
    pub values: [f64; 8],
}

fn close(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= 0.00002
}

fn differs(a: &Sample, b: &Sample) -> Option<&'static str> {
    (0..KEYS.len())
        .find(|&k| !close(a.values[k], b.values[k]))
        .map(|k| KEYS[k])
}

/// Rust recomputation of the in-game comparison: per-case reasons (empty means
/// verified). Calibration faults are errors, as they are assertions in game.
pub(crate) fn evaluate(
    cases: &[Case],
    players: &[i64],
    order: &[Phase],
    samples: &BTreeMap<String, Vec<Sample>>,
) -> Result<Vec<Vec<String>>> {
    let get = |phase: Phase| {
        samples
            .get(&phase.name())
            .with_context(|| format!("batch phase missing: {}", phase.name()))
    };
    let base = get(Phase::Baseline)?;
    for phase in order {
        let rows = get(*phase)?;
        ensure!(rows.len() == base.len(), "batch city inventory changed");
        ensure!(
            rows.iter()
                .zip(base)
                .all(|(r, a)| (r.city, r.plot, r.owner) == (a.city, a.plot, a.owner)),
            "batch city identity changed in {}",
            phase.name()
        );
    }
    let subject = |owner: i64| players.iter().position(|p| *p == owner);
    for (i, a) in base.iter().enumerate() {
        for phase in [Phase::PrecisionOff, Phase::ReferenceOff] {
            if let Some(kind) = differs(a, &get(phase)?[i]) {
                bail!("diagnostic meter did not restore {kind} at city {}", a.city);
            }
        }
        if subject(a.owner).is_none() {
            for phase in [Phase::PrecisionOne, Phase::PrecisionTwo, Phase::Reference] {
                if let Some(kind) = differs(a, &get(phase)?[i]) {
                    bail!(
                        "diagnostic meter leaked to non-batch city {} {kind}",
                        a.city
                    );
                }
            }
        }
    }
    let before = |phase: Phase| -> Result<&Vec<Sample>> {
        let at = order
            .iter()
            .position(|p| *p == phase)
            .context("phase order")?;
        get(order[at.checked_sub(1).context("phase has no predecessor")?])
    };
    let (reference, one, two) = (
        get(Phase::Reference)?,
        get(Phase::PrecisionOne)?,
        get(Phase::PrecisionTwo)?,
    );
    let mut reasons = vec![Vec::new(); cases.len()];
    let mut leaked = vec![BTreeSet::new(); cases.len()];
    for (ci, case) in cases.iter().enumerate() {
        let (prev, on, off) = (
            before(Phase::On(ci))?,
            get(Phase::On(ci))?,
            get(Phase::Off(ci))?,
        );
        let fail = &mut reasons[ci];
        let mut proven = [0; 2];
        for (i, a) in base.iter().enumerate() {
            if let Some(kind) = differs(&prev[i], &off[i]) {
                fail.push(format!("restore:{}:{kind}", a.city));
            }
            if a.owner != players[ci] {
                if let Some(kind) = differs(&prev[i], &on[i]) {
                    fail.push(format!("leak:{}:{kind}", a.city));
                    leaked[ci].insert(a.city);
                }
                continue;
            }
            if let Some(kind) = differs(a, &prev[i]) {
                fail.push(format!("drift:{}:{kind}", a.city));
            }
            // Six yields, then other growth; overall growth may follow a food oath.
            for slot in (0..6).chain([7]) {
                let target = case.effects.iter().any(|(kind, _)| *kind as usize == slot);
                if !target && !close(prev[i].values[slot], on[i].values[slot]) {
                    fail.push(format!("non_target:{}:{}", a.city, KEYS[slot]));
                }
            }
            for (e, (kind, amount)) in case.effects.iter().enumerate() {
                let slot = *kind as usize;
                let (av, observed) = (a.values[slot], on[i].values[slot] - a.values[slot]);
                let hundred = reference[i].values[slot] - av;
                if !(hundred.is_finite() && hundred > 0.03) {
                    fail.push(format!("reference:{}:{}", a.city, kind.key()));
                    continue;
                }
                let step = reported_step(&[
                    av,
                    reference[i].values[slot],
                    one[i].values[slot],
                    two[i].values[slot],
                ]);
                let amount = *amount as f64;
                match step.map(|s| 2.0 * s * (1.0 + amount / 100.0) + 0.00004) {
                    Some(tolerance) if tolerance < hundred * 0.005 => {
                        if (observed - hundred * amount / 100.0).abs() <= tolerance {
                            proven[e] += 1;
                        } else {
                            fail.push(format!("quantity:{}:{}", a.city, kind.key()));
                        }
                    }
                    _ => fail.push(format!("resolution:{}:{}", a.city, kind.key())),
                }
            }
        }
        for (e, (kind, _)) in case.effects.iter().enumerate() {
            if proven[e] == 0 {
                fail.push(format!("unproven:{}", kind.key()));
            }
        }
    }
    let (last, joint) = (before(Phase::AllOn)?, get(Phase::AllOn)?);
    for (i, a) in base.iter().enumerate() {
        let owner = subject(a.owner);
        let expected = match owner {
            Some(ci) => &get(Phase::On(ci))?[i],
            None => &last[i],
        };
        let Some(kind) = differs(expected, &joint[i]) else {
            continue;
        };
        let blamed: Vec<usize> = (0..cases.len())
            .filter(|&ci| leaked[ci].contains(&a.city))
            .collect();
        let reason = format!("simultaneous:{}:{kind}", a.city);
        match (blamed.is_empty(), owner) {
            (false, _) => blamed
                .iter()
                .for_each(|&ci| reasons[ci].push(reason.clone())),
            (true, Some(ci)) => reasons[ci].push(reason),
            (true, None) => reasons
                .iter_mut()
                .for_each(|r| r.push(format!("simultaneous_unattributed:{}:{kind}", a.city))),
        }
    }
    Ok(reasons)
}

/// Driver completion details: one START equal to the executed step and one OK
/// per step, in order, no retained failure, ending with the diagnostic exit 3.
fn completions(records: &[Value], steps: &[Value]) -> Result<Vec<String>> {
    ensure!(
        !records.iter().any(|r| r["status"] == "FAIL"),
        "failed driver observation retained"
    );
    let mut details = Vec::new();
    let mut last: Option<usize> = None;
    for (index, step) in steps.iter().enumerate() {
        let action = format!(
            "{}/{} {}",
            index + 1,
            steps.len(),
            step["action"].as_str().context("step action")?
        );
        let (starts, oks) = (
            records_of(records, &action, "START"),
            records_of(records, &action, "OK"),
        );
        ensure!(
            starts.len() == 1 && oks.len() == 1,
            "missing or duplicated driver record: {action}"
        );
        let started: Value =
            serde_json::from_str(starts[0].1["detail"].as_str().context("start detail")?)?;
        ensure!(
            &started == step,
            "executed step does not match the scenario: {action}"
        );
        ensure!(
            starts[0].0 < oks[0].0 && last.is_none_or(|prior| prior < starts[0].0),
            "driver record order differs: {action}"
        );
        last = Some(oks[0].0);
        details.push(
            oks[0].1["detail"]
                .as_str()
                .context("driver return")?
                .to_owned(),
        );
    }
    let observed: Vec<_> = records
        .iter()
        .enumerate()
        .filter(|(_, r)| r["action"] == "diagnostic")
        .collect();
    ensure!(
        observed.len() == 1
            && observed[0].1["status"] == "OBSERVED"
            && observed[0].1["detail"]
                .as_str()
                .is_some_and(|d| d.ends_with("exit 3"))
            && last.is_some_and(|l| l < observed[0].0),
        "the run did not end as a completed diagnostic observation (exit 3)"
    );
    Ok(details)
}

fn records_of<'a>(records: &'a [Value], action: &str, status: &str) -> Vec<(usize, &'a Value)> {
    records
        .iter()
        .enumerate()
        .filter(|(_, r)| r["action"] == action && r["status"] == status)
        .collect()
}

/// A length-prefixed frame between its exact marker and terminator.
fn frame<'a>(detail: &'a str, context: &str, marker: &str) -> Result<&'a str> {
    let rest = detail
        .strip_prefix(&format!("ran Lua in {context}; return={marker}"))
        .with_context(|| format!("{marker} frame missing"))?;
    let (length, body) = rest.split_once(':').context("frame length missing")?;
    let body = body.strip_suffix(END).context("frame terminator missing")?;
    ensure!(
        !length.is_empty()
            && length.bytes().all(|b| b.is_ascii_digit())
            && length.parse::<usize>()? == body.len()
            && body.starts_with('{')
            && body.ends_with('}'),
        "incomplete frame return"
    );
    Ok(body)
}

fn measured(detail: &str, batch: usize, phase: Phase) -> Result<Vec<Sample>> {
    let body = detail
        .strip_prefix("ran Lua in InGame; return=")
        .context("measurement return missing")?;
    let data: Value = serde_json::from_str(body)?;
    ensure!(
        data["batch"] == batch && data["phase"] == phase.name().as_str(),
        "measurement belongs to another phase"
    );
    let rows = data["cities"].as_array().context("measured cities")?;
    ensure!(rows.len() == CITIES, "measurement lacks the 56 real cities");
    let mut samples = Vec::new();
    for row in rows {
        let number = |key: &str| {
            row[key]
                .as_f64()
                .filter(|v| v.is_finite())
                .with_context(|| format!("finite measured {key}"))
        };
        for key in ["housing", "happiness", "food_surplus", "loyalty"] {
            number(key)?;
        }
        ensure!(row["occupied"].is_boolean(), "measured occupation");
        let mut values = [0.0; 8];
        for (slot, key) in values.iter_mut().zip(KEYS) {
            *slot = number(key)?;
        }
        let int = |key: &str| row[key].as_i64().with_context(|| format!("measured {key}"));
        samples.push(Sample {
            city: int("city")?,
            plot: int("plot")?,
            owner: int("owner")?,
            values,
        });
    }
    ensure!(
        samples.windows(2).all(|w| w[0].plot < w[1].plot),
        "measurement is not ordered by plot"
    );
    Ok(samples)
}

/// The saved Core frame: every batch house holds exactly its golden choice, no
/// other house holds one, and each real city carries exactly its own house's
/// flag among 66. Returns each case's player and each city plot's (id, owner),
/// read from the frame independently of the game's Lua.
type Saved = (Vec<i64>, BTreeMap<i64, (i64, i64)>);

fn saved_selections(core: &str, cases: &[Case]) -> Result<Saved> {
    let frame = Frame::parse(core)?;
    let keys = frame
        .entries()?
        .iter()
        .map(|(k, _, _)| k.text())
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        keys == BTreeSet::from(["cities", "ledger", "oath", "start", "turn"]),
        "Core frame lacks oath, turn, start, ledger or cities"
    );
    ensure!(
        frame.get("oath")?.text()? == cases[0].oath,
        "Core frame belongs to another human oath"
    );
    let ledger = frame.get("ledger")?;
    ensure!(
        ledger.get("reign")?.int()? == 1,
        "saved ledger left the fixture reign"
    );
    let houses = ledger.get("houses")?.entries()?;
    ensure!(houses.len() == 22, "saved ledger needs the 22 houses");
    let mut civs = BTreeMap::new();
    for (key, house, _) in houses {
        let id: i64 = key
            .text()?
            .strip_prefix('p')
            .context("house key")?
            .parse()?;
        civs.insert(id, house);
    }
    let mut players = Vec::new();
    for case in cases {
        let found: Vec<i64> = civs
            .iter()
            .filter(|(_, h)| h.get("civ").and_then(Frame::text).ok() == Some(case.civ.as_str()))
            .map(|(id, _)| *id)
            .collect();
        ensure!(
            found.len() == 1,
            "saved ledger lacks exactly one {}",
            case.civ
        );
        players.push(found[0]);
    }
    ensure!(players[0] == 0, "the human batch house is not player 0");
    for (id, house) in &civs {
        let oaths = match house.get("oaths") {
            Ok(oaths) => oaths.entries()?,
            Err(_) => &[],
        };
        match players.iter().position(|p| p == id) {
            Some(ci) => ensure!(
                house.get("age")?.text()? == "GOLDEN"
                    && house.get("oathChosen")?.int()? == 1
                    && oaths.len() == 1
                    && oaths[0].0 == Frame::Str(cases[ci].oath.clone())
                    && oaths[0].1 == Frame::Bool(true),
                "batch house {} lacks its saved golden choice",
                cases[ci].house
            ),
            None => ensure!(
                oaths.is_empty(),
                "a house outside the batch has a saved oath"
            ),
        }
    }
    let cities = frame.get("cities")?.entries()?;
    ensure!(
        cities.len() == CITIES,
        "saved frame lacks the 56 real cities"
    );
    let mut owned = vec![0; cases.len()];
    let mut identity = BTreeMap::new();
    for (plot, city, _) in cities {
        let owner = city.get("owner")?.int()?;
        identity.insert(plot.text()?.parse()?, (city.get("id")?.int()?, owner));
        let case = players.iter().position(|p| *p == owner);
        if let Some(ci) = case {
            owned[ci] += 1;
        }
        let flags = city.get("flags")?.entries()?;
        ensure!(flags.len() == 66, "a saved city lacks the 66 oath flags");
        for (oath, flag, _) in flags {
            let on = case.is_some_and(|ci| cases[ci].oath == oath.text().unwrap_or_default());
            ensure!(
                flag.int()? == i64::from(on),
                "saved flag {} differs from the batch selection",
                oath.text()?
            );
        }
    }
    ensure!(
        owned.iter().all(|n| *n > 0),
        "a batch house has no saved city"
    );
    Ok((players, identity))
}

struct Compared {
    verified: bool,
    reasons: Vec<String>,
    results: Value,
}

fn compared(detail: &str, probe: &Probe, players: &[i64]) -> Result<Vec<Compared>> {
    let body = detail
        .strip_prefix("ran Lua in InGame; return=")
        .context("comparison return missing")?;
    let data: Value = serde_json::from_str(body)?;
    let rows = data["cases"].as_array().context("compared cases")?;
    ensure!(
        data["batch"] == probe.batch && rows.len() == probe.cases.len(),
        "comparison belongs to another batch"
    );
    let mut out = Vec::new();
    for (i, (row, case)) in rows.iter().zip(&probe.cases).enumerate() {
        ensure!(
            row["oath"] == case.oath.as_str()
                && row["house"] == case.house.as_str()
                && row["player"] == players[i]
                && row["selection_path"] == selection_path(i)
                && row["results"].is_array(),
            "in-game comparison describes another case"
        );
        let reasons = row["reasons"]
            .as_array()
            .context("in-game reasons")?
            .iter()
            .map(|r| r.as_str().map(str::to_owned).context("in-game reason"))
            .collect::<Result<Vec<_>>>()?;
        let verified = row["quantity_verified"]
            .as_bool()
            .context("in-game verdict")?;
        ensure!(
            verified == reasons.is_empty(),
            "in-game verdict contradicts its reasons"
        );
        out.push(Compared {
            verified,
            reasons,
            results: row["results"].clone(),
        });
    }
    ensure!(
        data["quantity_verified_count"] == out.iter().filter(|c| c.verified).count(),
        "in-game verified count differs"
    );
    Ok(out)
}

/// Reads only the recorded run: the executed scenario must be in the artifact.
pub fn verify(repo: &Repository, dir: &Path) -> Result<Value> {
    let path = dir.join("scenario.json");
    ensure!(
        path.is_file(),
        "{} lacks the executed scenario.json; a reconstructed scenario is not evidence",
        dir.display()
    );
    let read = |name: &str| -> Result<Value> {
        Ok(serde_json::from_str(&fs::read_to_string(dir.join(name))?)?)
    };
    let scenario = read("scenario.json")?;
    let batch = scenario["batch_probe"]["batch"]
        .as_u64()
        .context("artifact batch identity")? as usize;
    let probe = Probe::new(repo, batch)?;
    let expected = probe.scenario()?;
    ensure!(
        scenario["diagnostic_only"] == true
            && scenario["batch_probe"] == expected["batch_probe"]
            && scenario["name"] == expected["name"],
        "artifact scope changed"
    );
    let actual = scenario["steps"].as_array().context("artifact steps")?;
    let generated = expected["steps"].as_array().context("generated steps")?;
    let offset = actual
        .len()
        .checked_sub(generated.len())
        .context("artifact steps missing")?;
    // Shipping may prepend a restart and suffix the save name; nothing else.
    ensure!(
        offset == 0 || actual[..offset] == [json!({"action":"restart_game","timeout":420})],
        "unknown lifecycle prefix"
    );
    let mut normalized = actual[offset..].to_vec();
    let mut saved = None;
    for (step, source) in normalized.iter_mut().zip(generated) {
        if source["action"] == "save" || source["action"] == "load" {
            let base = source["name"].as_str().context("generated save name")?;
            let name = step["name"].as_str().context("artifact save/load name")?;
            ensure!(
                name == base || name.starts_with(&format!("{base}-")),
                "wrong artifact save name"
            );
            if source["action"] == "save" {
                saved = Some(name.to_owned());
            } else {
                ensure!(saved.as_deref() == Some(name), "save/load names differ");
            }
            step["name"] = source["name"].clone();
        }
    }
    ensure!(
        normalized == *generated,
        "artifact does not contain the generated batch scenario"
    );
    let result = read("result.json")?;
    ensure!(
        result["diagnostic_only"] == true
            && result["observation_completed"] == true
            && result["passed"] == false
            && result["acceptance_passed"] == false
            && result["failure"].is_null()
            && result["completed_steps"].as_u64() == Some(actual.len() as u64),
        "driver did not complete the batch probe"
    );
    let checks = result["checks"].as_array().context("driver checks")?;
    ensure!(
        !checks.is_empty() && checks.iter().all(|c| c["passed"] == true),
        "driver log checks failed or are absent"
    );
    let records = fs::read_to_string(dir.join("steps.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<Vec<Value>, _>>()?;
    let details = completions(&records, actual)?;
    let at = |role: Role| -> Result<usize> { Ok(offset + probe.position(role)?) };
    let saved_name = recorded_save_name(&details[at(Role::Save)?], &actual[at(Role::Save)?])?;
    ensure!(
        details[at(Role::Load)?] == format!("loaded {saved_name}"),
        "load OK record does not name the recorded save"
    );
    let human = &probe.cases[0].oath;
    let mut frames = Vec::new();
    for after in [false, true] {
        let phase = if after { "AFTER" } else { "BEFORE" };
        frames.push((
            frame(
                &details[at(Role::CoreFrame { after })?],
                "GameCore_Tuner",
                &format!("JAC_OATH_SAVE_CORE_{phase}|{human}|"),
            )?,
            frame(
                &details[at(Role::UiFrame { after })?],
                "InGame",
                &format!("JAC_OATH_BATCH_UI_{phase}|batch-{batch}|"),
            )?,
        ));
    }
    ensure!(
        frames[0].0 == frames[1].0,
        "Core ledger, history, flags or yields changed across save/load"
    );
    ensure!(
        frames[0].1 == frames[1].1,
        "UI yields or growth changed across save/load"
    );
    ensure!(
        Frame::parse(frames[1].1)?.get("cities")?.entries()?.len() == CITIES,
        "UI frame lacks the 56 real cities"
    );
    let (players, identity) = saved_selections(frames[1].0, &probe.cases)?;
    let order = schedule(probe.cases.len());
    let mut samples = BTreeMap::new();
    for phase in &order {
        let rows = measured(&details[at(Role::Measure(*phase))?], batch, *phase)?;
        samples.insert(phase.name(), rows);
    }
    ensure!(
        samples["baseline"]
            .iter()
            .map(|s| (s.plot, (s.city, s.owner)))
            .collect::<BTreeMap<_, _>>()
            == identity,
        "measured cities differ from the saved real cities"
    );
    let rust = evaluate(&probe.cases, &players, &order, &samples)?;
    let game = compared(&details[at(Role::Compare)?], &probe, &players)?;
    let mut cases = Vec::new();
    let mut verified = Vec::new();
    let mut remaining = vec![
        "natural_age_transition".to_owned(),
        "official_ui_selection_for_ledger_fixture_houses".to_owned(),
        format!(
            "{} catalogue oaths outside this batch",
            66 - probe.cases.len()
        ),
    ];
    for (i, case) in probe.cases.iter().enumerate() {
        let agree = game[i].reasons == rust[i];
        let ok = game[i].verified && rust[i].is_empty() && agree;
        let mut reasons = rust[i].clone();
        if !agree {
            reasons.push("lua_rust_disagreement".into());
        }
        let mut left = Vec::new();
        if ok {
            verified.push(case.oath.clone());
        } else {
            left.push("quantity_and_native_save");
            remaining.push(format!("quantity_and_native_save:{}", case.oath));
        }
        if i > 0 {
            left.push("official_ui_selection");
        }
        left.push("natural_age_transition");
        let effects: Vec<_> = case
            .effects
            .iter()
            .map(|(kind, amount)| json!({"kind":kind.key(),"expected_percent":amount}))
            .collect();
        cases.push(json!({"oath":case.oath,"house":case.house,"civilization":case.civ,"player":players[i],"selection_path":selection_path(i),"effects":effects,"quantity_and_native_save_verified":ok,"reasons":reasons,"in_game_reasons":game[i].reasons,"results":game[i].results,"remaining":left}));
    }
    Ok(json!({
        "batch": batch,
        "scope": "yield_quantities_and_native_save_only",
        "saved_name": saved_name,
        "house_count": 22,
        "city_count": CITIES,
        "flags_per_city": 66,
        "native_save_frames_equal": true,
        "batch_size": probe.cases.len(),
        "verified_count": verified.len(),
        "verified": verified,
        "cases": cases,
        "catalogue_total": 66,
        "accepted_all_66": false,
        "task_accepted": false,
        "remaining": remaining,
    }))
}

pub fn run(repo: &Repository, args: &[String]) -> Result<()> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = |batch: usize, path: &str| -> Result<()> {
        let probe = Probe::new(repo, batch)?;
        write_new(Path::new(path), &probe.scenario()?)?;
        println!(
            "batch {batch}: {} yield-only oaths, {} steps; diagnostic only, live unverified",
            probe.cases.len(),
            probe.steps.len()
        );
        Ok(())
    };
    match args[..] {
        ["--check"] => {
            let vm = mlua::Lua::new();
            let mut sizes = Vec::new();
            let mut covered = BTreeSet::new();
            for batch in 1..=BATCHES {
                let probe = Probe::new(repo, batch)?;
                for step in probe.scenario()?["steps"].as_array().context("steps")? {
                    if let Some(code) = step["code"]
                        .as_str()
                        .or_else(|| step["condition"]["code"].as_str())
                    {
                        vm.load(code).into_function()?;
                    }
                }
                covered.extend(probe.cases.iter().map(|c| c.oath.clone()));
                sizes.push(probe.cases.len().to_string());
            }
            ensure!(
                covered.len() == eligible(repo)?.len(),
                "the batches must cover every yield-only oath once"
            );
            println!(
                "{BATCHES} batch probes ({} yield-only oaths of distinct houses): driver schema and Lua 5.1 compile passed; live unverified",
                sizes.join("+")
            );
            Ok(())
        }
        ["--verify", dir] => {
            println!(
                "{}",
                serde_json::to_string_pretty(&verify(repo, Path::new(dir))?)?
            );
            Ok(())
        }
        ["--output", path] => output(1, path),
        ["--batch", batch, "--output", path] => {
            output(batch.parse().context("batch number")?, path)
        }
        _ => bail!("usage: generate-oath-batch-probes --check | [--batch 1..={BATCHES}] --output NEW.json | --verify ARTIFACT_DIR"),
    }
}

#[cfg(test)]
#[path = "oath_batch_probe_tests.rs"]
mod tests;
