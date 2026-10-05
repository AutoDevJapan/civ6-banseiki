//! Bounded, synchronous qualification diagnostic, including 21 AI owners.
//! This is not AI-initiated payment, startup/load, unpaid assignment or a suite pass.
//! No runtime/native implementation or permission authority is introduced here.
//! Native mode 6 retains only a 1024-call calibration prefix; it is never a
//! complete-history verdict. Full coverage comes from the asserted Core/UI
//! separately asserted receipts, within the one finite observation window.
//! Gameplay has no verified CanEver/CanAppoint binding. Its receipt observes
//! the normal 901xxx projection and ledger, not a Core native getter verdict.
//! UI reads the real getters independently. Native acceptance remains the
//! parent's mode-6 artifact measurement, including original call counters.
use crate::{payment_scenario::READ_CLOSED_ADMISSION, scenario::validate_scenario, Error, Result};
use civ6_native_bridge_rs::paid_catalog::COUNT;
use serde_json::{json, Value};

pub const NORMAL: &str = include_str!("../../civ6-autotest/scenarios/leader-persona-15km.json");

/// Keep the normal setup as the sole source of the source-slot mapping.
pub fn header() -> String {
    let normal: Value = serde_json::from_str(NORMAL).expect("normal scenario JSON");
    let mut out = String::from("local core=true\nlocal fullMatrix=true\nlocal expected={\n");
    for p in normal["steps"][1]["players"].as_array().unwrap() {
        out.push_str(&format!(
            "{{leader='{}',civ='{}',human={}}},\n",
            p["leader"].as_str().unwrap(),
            p["civ"].as_str().unwrap(),
            p["human"]
        ));
    }
    out.push_str(r#"}
local function integer(v) return type(v)=='number' and v>=0 and v%1==0 end
local rows,indices,leaders,hashes,costs={},{},{},{},{}
local n=0
for row in GameInfo.JAC_LeaderPersonas() do
 local d=assert(GameInfo.Governors[row.GovernorType],'Governor definition missing')
 assert(type(row.LeaderType)=='string' and not leaders[row.LeaderType],'duplicate leader')
 assert(integer(d.Index) and d.Index<256 and not indices[d.Index],'invalid Governor Index')
 assert(type(d.Hash)=='number' and not hashes[d.Hash],'invalid Governor Hash')
 assert(integer(row.EvolutionCost) and row.EvolutionCost>=1 and row.EvolutionCost<=3,'invalid cost')
 leaders[row.LeaderType]=row; indices[d.Index]=true; hashes[d.Hash]=true; costs[row.EvolutionCost]=true; n=n+1
end
assert(n==22,'requires the complete normal catalog')
assert(costs[1] and costs[2] and costs[3],'requires all three normal costs')
local alive,count={},0
for position,id in pairs(PlayerManager.GetAliveMajorIDs()) do
 assert(integer(position) and position>=1 and position<=22 and integer(id) and id<22 and not alive[id],'invalid alive owners')
 alive[id]=true; count=count+1
end
assert(count==22,'requires 22 alive owners')
local catalog,balance={},{}
for source=0,21 do
 local e=expected[source+1]; local row=assert(leaders[e.leader],'normal leader missing')
 local c=assert(PlayerConfigurations[source]); local p=assert(Players[source])
 assert(alive[source] and p:IsAlive()==true and p:IsMajor()==true and p:IsHuman()==e.human,'normal owner identity differs')
 assert(c:GetLeaderTypeName()==e.leader and c:GetCivilizationTypeName()==e.civ and row.CivilizationType==e.civ,'source catalog differs')
 local city=assert(p:GetCities():GetCapitalCity(),'every owner requires a real capital')
 assert(city:GetOwner()==source and integer(city:GetID()),'capital owner differs')
 local g=assert(p:GetGovernors(),'Governor collection unavailable')
 for _,method in ipairs({'GetGovernorPoints','GetGovernorPointsSpent'}) do
  assert(type(g[method])=='function','real getter unavailable: '..method)
 end
 local total,spent=g:GetGovernorPoints(),g:GetGovernorPointsSpent()
 assert(integer(total) and spent==0,'titles unknown or already spent')
 local d=GameInfo.Governors[row.GovernorType]
 rows[source+1]={row=row,def=d,source=source,owner=(source+1)%22}
 catalog[#catalog+1]=source..':'..e.leader..':'..e.civ..':'..d.Index..':'..d.Hash..':'..row.EvolutionCost..':'..city:GetID()
 balance[#balance+1]=source..':'..total..':'..spent
end
assert(Game:GetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1')==nil,'paid assignment fixture active')
assert(Game:GetProperty('JAC_TEST853_ADMISSION_RECEIPT')==nil,'private admission fixture active')
local turn=Game.GetCurrentGameTurn(); assert(integer(turn),'turn unavailable')
local function ledger(entry,joined,evolving)
 local row,owner=entry.row,entry.owner
 assert(owner~=entry.source,'same-owner source forbidden')
 local prefix='JAC_PERSONA_'..row.LeaderType..'_'
 local function get(k) return Game:GetProperty(prefix..k) end
 if joined then
  assert(get('OWNER')==owner and get('SOURCE')==entry.source,'Game owner/source differs')
  assert(get('EVOLUTION_COST')==row.EvolutionCost,'Game cost differs')
  assert(get('STATE')==(evolving and 'EVOLVING' or 'GREAT_PERSON'),'Game state pending or unexpected')
  assert(integer(get('UNIT')),'real Great Person missing')
  if evolving then assert(get('EVOLUTION_SURCHARGE')==row.EvolutionCost-1,'Game reservation differs') end
 else
  assert(get('OWNER')==nil and get('STATE')==nil and get('UNIT')==nil,'baseline must be unjoined')
 end
 -- Count actual Great People across all owners, without a UI-only IsGreatPerson binding.
 local individual=assert(GameInfo.GreatPersonIndividuals[row.PersonType])
 local class=assert(GameInfo.GreatPersonClasses[individual.GreatPersonClassType])
 local unitType=assert(GameInfo.Units[class.UnitType])
 local found=0
 for id=0,21 do
  for _,unit in Players[id]:GetUnits():Members() do
   if unit:GetType()==unitType.Index then
    local gp=assert(unit:GetGreatPerson()); local index=gp:GetIndividual()
    local actual=index~=nil and index~=-1 and GameInfo.GreatPersonIndividuals[index]
    if actual and actual.GreatPersonIndividualType==row.PersonType then
     found=found+1
     assert(joined and id==owner and unit:GetOwner()==owner and unit:GetID()==get('UNIT'),'Great Person world uniqueness differs')
    end
   end
  end
 end
 assert(found==(joined and 1 or 0),'Great Person count differs')
end
local rawQualifications={}
local function qualification(entry,joined,evolving)
 ledger(entry,joined,evolving)
 local positives,negatives,bits,measured=0,0,{},0
 for id=0,21 do
  local g=Players[id]:GetGovernors(); local expected=evolving and id==entry.owner
  local allowed
  if core then
   -- Observe the existing synchronized projection only. Never substitute this
   -- for an absent Core CanEver binding or a native trace verdict.
   allowed=Players[id]:GetProperty(901000+entry.def.Index)
   assert(type(allowed)=='boolean','unknown Gameplay projection')
  elseif fullMatrix or (evolving and id==entry.owner) then
   assert(type(g.CanEverAppointGovernor)=='function' and type(g.CanAppointGovernor)=='function','UI getter unavailable')
   local ever=g:CanEverAppointGovernor(entry.def.Hash)
   local now=g:CanAppointGovernor(entry.def.Hash)
   assert(type(ever)=='boolean' and type(now)=='boolean','unknown UI qualification return')
   assert(ever==expected and now==expected,'UI owner qualification differs')
   allowed=ever
  end
  if core or fullMatrix or (evolving and id==entry.owner) then
   assert(allowed==expected,'owner qualification differs')
   measured=measured+1
   if expected then positives=positives+1 else negatives=negatives+1 end
   bits[#bits+1]=allowed and '1' or '0'
  else bits[#bits+1]='?' end
  -- Gameplay has no verified HasGovernor binding. Only the actual UI getter
  -- can assert absence of a native appointment; the Core ledger stays separate.
  if not core then
   assert(type(g.HasGovernor)=='function','UI appointment getter unavailable')
   assert(g:HasGovernor(entry.def.Hash)==false,'unexpected appointment')
  end
 end
 if core or fullMatrix then
  assert(measured==22 and positives==(evolving and 1 or 0) and negatives==(evolving and 21 or 22),'qualification coverage differs')
 else assert(measured==(evolving and 1 or 0) and positives==(evolving and 1 or 0) and negatives==0,'priority qualification coverage differs') end
 local ownerGov=Players[entry.owner]:GetGovernors()
 if evolving then assert(ownerGov:GetGovernorPoints()>=entry.row.EvolutionCost,'official titles not available') end
 local identity=entry.source..':'..entry.owner..':'..entry.def.Index..':'..entry.row.LeaderType..':'..entry.row.EvolutionCost
 local measured='CATALOG880 '..(core and 'GAMEPLAY_PROJECTION' or 'UI_NATIVE_QUALIFICATION')..' '..identity..' positive='..positives..' negative='..negatives..' raw='..table.concat(bits)
 rawQualifications[#rawQualifications+1]=measured; print(measured)
 local prefix='JAC_PERSONA_'..entry.row.LeaderType..'_'
 return identity..':'..tostring(Game:GetProperty(prefix..'STATE'))..':'..tostring(Game:GetProperty(prefix..'UNIT'))..':'..tostring(Game:GetProperty(prefix..'EVOLUTION_SURCHARGE'))..':'..ownerGov:GetGovernorPoints()..':'..ownerGov:GetGovernorPointsSpent(),positives,negatives
end
"#);
    out
}

pub fn stable_code() -> String {
    format!("{}\nreturn 'CATALOG880 STABLE turn='..turn..' catalog='..table.concat(catalog,';')..' titles='..table.concat(balance,';')", header())
}

/// These two existing flags suppress only automatic UI operation submission
/// (View.lua Update). ACTIVE also enables the legacy Ashina projection, but
/// mode 6 never consumes it, and Ashina's owner is 3, not private owner 0.
/// Neither that projection nor any paid flag is read as evidence.
pub fn prepare_code() -> String {
    format!(
        r#"{}
assert(Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE')==nil and Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT')==nil,'UI fixture already active')
for _,entry in ipairs(rows) do ledger(entry,false,false) end
Game:SetProperty('JAC_TEST853_ADMISSION_ACTIVE',1)
Game:SetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT',1)
local civics,seen={{}},{{}}
for item in GameInfo.JAC_PersonaTitleCivics() do
 assert(type(item.CivicType)=='string' and not seen[item.CivicType],'invalid official title civic')
 seen[item.CivicType]=true; civics[#civics+1]=item.CivicType
end
table.sort(civics); assert(#civics==15,'official title sources differ')
for _,entry in ipairs(rows) do
 local p=Players[entry.owner]; local g=p:GetGovernors(); local culture=p:GetCulture()
 for _,kind in ipairs(civics) do
  if g:GetGovernorPoints()>=entry.row.EvolutionCost then break end
  local civic=assert(GameInfo.Civics[kind])
  if not culture:HasCivic(civic.Index) then culture:SetCulturalProgress(civic.Index,culture:GetCultureCost(civic.Index)) end
 end
 assert(g:GetGovernorPointsSpent()==0 and g:GetGovernorPoints()>=entry.row.EvolutionCost,'official civic progress pending or insufficient')
end
return 'CATALOG880 official civics only; UI automatic submission paused; no game turn requested'
"#,
        header()
    )
}

pub fn pair_code(source: usize, evolving: bool, mutate: bool) -> Result<String> {
    Ok(format!(
        "{}\n{}\nreturn 'CATALOG880 {} '..receipt",
        if mutate { header() } else { ui_code(header()) },
        pair_body(source, evolving, mutate)?,
        if evolving { "EVOLVING" } else { "JOINED" }
    ))
}

fn pair_body(source: usize, evolving: bool, mutate: bool) -> Result<String> {
    if source >= COUNT {
        return Err(Error::input("source outside normal 22-person catalog"));
    }
    let mut code = format!("fullMatrix=false\nlocal entry=rows[{}]\n", source + 1);
    if mutate {
        code.push_str("assert(Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE')==1 and Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT')==1,'automatic UI submission not paused')\nlocal api=assert(ExposedMembers.JAC_LeaderPersona,'Gameplay API missing')\n");
        if evolving {
            code.push_str("ledger(entry,true,false)\nassert(type(api.PrepareEvolution)=='function' and api.PrepareEvolution(entry.owner,entry.row.LeaderType)=='EVOLVING','one synchronous PrepareEvolution failed or returned unknown')\n");
        } else {
            code.push_str("ledger(entry,false,false)\nassert(type(api.Join)=='function' and api.Join(entry.owner,entry.source)=='JOINED','one synchronous Join failed or returned unknown')\n");
        }
    }
    code.push_str(&format!(
        "local receipt=qualification(entry,true,{evolving})"
    ));
    Ok(code)
}

/// Four people per transport call keep the single 300-second observation
/// bounded. Each person's Join/qualification and Evolve/qualification still
/// occurs once, with an independent UI read before the next mutation phase.
pub fn batch_code(first: usize, end: usize, evolving: bool, mutate: bool) -> Result<String> {
    if first >= end || end > COUNT || end - first > 4 {
        return Err(Error::input("catalog batch requires one to four sources"));
    }
    let mut code = format!(
        "{}\nlocal receipts={{}}\n",
        if mutate { header() } else { ui_code(header()) }
    );
    for source in first..end {
        code.push_str("do\n");
        code.push_str(&pair_body(source, evolving, mutate)?);
        code.push_str("\nreceipts[#receipts+1]=receipt\nend\n");
    }
    code.push_str(&format!(
        "return 'CATALOG880 {} '..table.concat(receipts,';')",
        if evolving { "EVOLVING" } else { "JOINED" }
    ));
    Ok(code)
}

pub fn matrix_code(evolving: bool) -> String {
    format!(
        r#"{}
local receipts,positive,negative={{}},0,0
for _,entry in ipairs(rows) do
 local receipt,p,n=qualification(entry,{evolving},{evolving})
 receipts[#receipts+1]=receipt; positive=positive+p; negative=negative+n
end
assert(positive=={positive} and negative=={negative},'full matrix incomplete')
print('CATALOG880 '..(core and 'GAMEPLAY_PROJECTION' or 'UI_NATIVE_QUALIFICATION')..' MATRIX positive='..positive..' negative='..negative)
return 'CATALOG880 MATRIX identities=22 '..table.concat(receipts,';')
"#,
        header(),
        positive = if evolving { 22 } else { 0 },
        negative = if evolving { 462 } else { 484 }
    )
}

fn lua(code: String, core: bool) -> Value {
    json!({"action":"ui_lua","state_name":if core {"GameCore_Tuner"} else {"InGame"},"code":code,"timeout":30,"lua_timeout":30})
}

fn ui_code(code: String) -> String {
    code.replacen("local core=true", "local core=false", 1)
}

fn compare(steps: &mut Vec<Value>, core: String, ui: String, key: &str) {
    let mut a = lua(core, true);
    a["capture_as"] = key.into();
    steps.push(a);
    let mut b = lua(ui_code(ui), false);
    b["equals_capture"] = key.into();
    steps.push(b);
}

pub fn build(template: &Value) -> Result<Value> {
    let normal: Value = serde_json::from_str(NORMAL)?;
    let prefix = template["steps"]
        .as_array()
        .filter(|v| v.len() >= 4)
        .ok_or_else(|| Error::input("normal setup missing"))?;
    if prefix[..4] != normal["steps"].as_array().unwrap()[..4] {
        return Err(Error::input("normal 22-player setup must remain unchanged"));
    }
    let mut steps = vec![prefix[0].clone(), json!({"action":"restart_game"})];
    steps.extend_from_slice(&prefix[1..4]);
    steps.push(lua(prepare_code(), true));
    compare(
        &mut steps,
        stable_code(),
        stable_code(),
        "CATALOG880_STABLE",
    );
    // Existing action maps to BridgeMode::ProductionAdmission (6), normal
    // production_admission=true. No private scope or paid-assignment mode.
    steps.push(json!({"action":"start_native_production_admission","name":"normal-catalog22","max_seconds":300,"timeout":30}));
    steps.push(lua(READ_CLOSED_ADMISSION.into(), true));
    // The prior closed-UI proof remains separate. Spending 968 UI getter calls
    // here would exhaust the mode-6 prefix before any positive could appear.
    let mut closed = lua(matrix_code(false), true);
    closed["capture_as"] = "CATALOG880_CLOSED".into();
    steps.push(closed);
    for source in (0..COUNT).step_by(4) {
        let end = (source + 4).min(COUNT);
        for evolving in [false, true] {
            compare(
                &mut steps,
                batch_code(source, end, evolving, true)?,
                batch_code(source, end, evolving, false)?,
                &format!(
                    "CATALOG880_{}_{}",
                    source,
                    if evolving { "EVOLVE" } else { "JOIN" }
                ),
            );
        }
    }
    compare(
        &mut steps,
        matrix_code(true),
        matrix_code(true),
        "CATALOG880_FINAL",
    );
    // Preserve the two differently sourced full matrices directly in steps.jsonl,
    // in addition to the per-person raw Lua log. Never compare them as one type.
    for core in [true, false] {
        let raw = matrix_code(true).replace(
            "return 'CATALOG880 MATRIX identities=22 '..table.concat(receipts,';')",
            "return 'positive='..positive..' negative='..negative..' '..table.concat(rawQualifications,';')",
        );
        steps.push(lua(if core { raw } else { ui_code(raw) }, core));
    }
    for core in [true, false] {
        let mut step = lua(stable_code(), core);
        step["equals_capture"] = "CATALOG880_STABLE".into();
        steps.push(step);
    }
    steps.push(json!({"action":"stop_native_production_admission","timeout":30}));
    steps.push(json!({"action":"exit_to_menu"}));
    let result = json!({"name":"Normal 22-person qualification: Gameplay closed484; UI owned positives FIRST then final484. Initial UI closed484 and per-join UI negatives UNMEASURED (separate prior proof). Cyclic owners (21 AI), synchronous Join/PrepareEvolution, zero turns/payment/assignment operations. Parent must verify native positive22 coverage in mode6 original counters/1024-prefix; prefix is NOT full-history evidence. Not AI autonomous payment, startup/load, unpaid placement or 113-suite acceptance","judge_profile":"generic","strict_lua_errors":true,"steps":steps});
    validate_scenario(&result)?;
    Ok(result)
}
