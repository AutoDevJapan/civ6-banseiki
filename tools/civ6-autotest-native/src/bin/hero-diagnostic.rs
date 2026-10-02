//! Prepare a finite observation scenario from the unchanged acceptance steps.
//! This program writes JSON; game access is only through the standard live_chain.
use serde_json::{Value, json};
use std::{fs, path::Path};

const TRACE_START: &str = r#"
local api=assert(ExposedMembers.JAC_Absorption)
local e=assert(Game:GetProperty('JAC_ABSORPTION_LIVE_RECALL_V1'))
assert(not Players[e.Owner]:IsHuman(),'diagnostic requires the original AI owner')
if JAC_NATIVE_HERO_FAITH_HANDLER then Events.FaithChanged.Remove(JAC_NATIVE_HERO_FAITH_HANDLER) end
if JAC_NATIVE_HERO_COMMAND_HANDLER then Events.CityCommandStarted.Remove(JAC_NATIVE_HERO_COMMAND_HANDLER) end
if JAC_NATIVE_HERO_UNIT_HANDLER then Events.UnitAddedToMap.Remove(JAC_NATIVE_HERO_UNIT_HANDLER) end
local function snapshot()
 local m=Game.GetHeroesManager();local origin=assert(m:GetHeroOriginCityID(e.Class))
 local city=assert(Players[e.Owner]:GetCities():FindID(origin.id))
 local native=GameInfo.Units[GameInfo.HeroClasses[e.Class].UnitType]
 local units={}
 for _,u in Players[e.Owner]:GetUnits():Members() do
  if u:IsHero() and u:GetHeroClassType()==e.Class then
   units[#units+1]={ID=u:GetID(),Class=u:GetHeroClassType(),X=u:GetX(),Y=u:GetY(),
    Lifespan=u:GetProperty(ExposedMembers.JAC_HeroLifespanKey),Person=u:GetProperty(api.Rules.PERSON_KEY),Token=u:GetProperty(api.Rules.TOKEN_KEY)}
  end
 end
 return {Turn=Game.GetCurrentGameTurn(),Era=Game.GetEras():GetCurrentEra(),Owner=e.Owner,Class=e.Class,
  Faith=Players[e.Owner]:GetReligion():GetFaithBalance(),Cost=city:GetGold():GetPurchaseCost(GameInfo.Yields.YIELD_FAITH.Index,native.Hash,MilitaryFormationTypes.STANDARD_MILITARY_FORMATION),
  Claim=m:GetHeroClaimPlayer(e.Class),Origin=origin,Units=units,Record=api.GetRoster(e.Owner)['live:hero']}
end
JAC_NATIVE_HERO_TRACE={Owner=e.Owner,Class=e.Class,Trace={},Before=snapshot()}
assert(#JAC_NATIVE_HERO_TRACE.Before.Units==0,'hero already alive before observation window')
local function observe(kind,data)
 local p=JAC_NATIVE_HERO_TRACE
 if p.Error then return end
 local ok,err=pcall(function()
  assert(#p.Trace<8192,'too many observation events')
  data.Kind=kind;data.Order=#p.Trace+1;data.Turn=Game.GetCurrentGameTurn();data.Era=Game.GetEras():GetCurrentEra()
  data.Active=Players[e.Owner]:IsTurnActive();data.LocalPlayer=Game.GetLocalPlayer()
  p.Trace[#p.Trace+1]=data
  print('[JAC_NATIVE_HERO_TRACE_EVENT] '..api.Canonical(data))
 end)
 if not ok then p.Error=tostring(err);print('[JAC_NATIVE_HERO_TRACE_ERROR] '..p.Error) end
end
JAC_NATIVE_HERO_FAITH_HANDLER=function(owner,yield,balance)
 if owner==e.Owner then observe('FAITH',{Owner=owner,Yield=yield,Balance=balance}) end
end
JAC_NATIVE_HERO_COMMAND_HANDLER=function(owner,city,districtOwner,district,command,data)
 if owner==e.Owner then observe('COMMAND',{Owner=owner,City=city,Command=command,Data=data}) end
end
JAC_NATIVE_HERO_UNIT_HANDLER=function(owner,id,x,y)
 if owner~=e.Owner then return end
 local kind,class
 for _,u in Players[owner]:GetUnits():Members() do
  if u:GetID()==id then kind=GameInfo.Units[u:GetType()].UnitType;class=u:GetHeroClassType() end
 end
 observe('UNIT',{Owner=owner,ID=id,Class=class,Type=kind,X=x,Y=y})
end
Events.FaithChanged.Add(JAC_NATIVE_HERO_FAITH_HANDLER)
Events.CityCommandStarted.Add(JAC_NATIVE_HERO_COMMAND_HANDLER)
Events.UnitAddedToMap.Add(JAC_NATIVE_HERO_UNIT_HANDLER)
JAC_NATIVE_HERO_SNAPSHOT=snapshot
return api.Canonical(JAC_NATIVE_HERO_TRACE.Before)
"#;

const TRACE_FINISH: &str = r#"
local api=assert(ExposedMembers.JAC_Absorption)
local p=assert(JAC_NATIVE_HERO_TRACE)
Events.FaithChanged.Remove(JAC_NATIVE_HERO_FAITH_HANDLER)
Events.CityCommandStarted.Remove(JAC_NATIVE_HERO_COMMAND_HANDLER)
Events.UnitAddedToMap.Remove(JAC_NATIVE_HERO_UNIT_HANDLER)
assert(not p.Error,p.Error)
p.After=JAC_NATIVE_HERO_SNAPSHOT()
assert(p.After.Turn>p.Before.Turn and p.After.Era>p.Before.Era,'era window was not processed')
assert(#p.Trace>0,'no observation evidence')
print('[JAC_NATIVE_HERO_TRACE_COMPLETE] '..api.Canonical(p))
return api.Canonical(p)
"#;

fn prepare(original: &Value, save: &str) -> Result<Value, String> {
    if !regex::Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$")
        .unwrap()
        .is_match(save)
    {
        return Err("invalid exact save name".into());
    }
    let steps = original["steps"]
        .as_array()
        .ok_or("original steps missing")?;
    if steps.len() != 127
        || steps[0]["action"] != "enable_mods"
        || steps[111]["action"] != "autoplay"
        || steps[111]["turns"] != 80
    {
        return Err("acceptance scenario layout changed; re-evaluate the diagnostic".into());
    }
    let mut out = vec![
        steps[0].clone(),
        json!({"action":"load","name":save,"exact_name":true,"timeout":600}),
        steps[100].clone(),
        json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":"local A=assert(ExposedMembers.JAC_Absorption);assert(A.GetState().Transactions['japan-live-absorption'].Status=='COMPLETE','wrong checkpoint');return 'verified completed absorption checkpoint'"}),
    ];
    // Original funded living/same-era checks and retirement remain unchanged.
    for (index, step) in steps[102..111].iter().enumerate() {
        out.push(step.clone());
        if index == 4 {
            out.push(json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":"local e=Game:GetProperty('JAC_ABSORPTION_LIVE_RECALL_V1');return type(e)=='table' and type(e.Owner)=='number' and type(e.Class)=='number' and type(e.EraBefore)=='number' and type(e.CityID)=='number'"}}));
        }
    }
    out.push(json!({"action":"ui_lua","state_name":"InGame","code":TRACE_START}));
    out.push(steps[111].clone());
    out.push(json!({"action":"ui_lua","state_name":"InGame","code":TRACE_FINISH}));
    Ok(
        json!({"name":"PR87 Rust diagnostic: observe target AI hero generation and faith across era window; not full acceptance","judge_profile":"generic","steps":out,"test_mode":{"skipped_turns":0,"continuous_performance_valid":false}}),
    )
}

fn main() {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    let result = (|| -> Result<(), String> {
        if a.len() != 3 {
            return Err(
                "usage: hero-diagnostic ORIGINAL_JSON EXACT_CHECKPOINT_SAVE OUTPUT_JSON".into(),
            );
        }
        let original = serde_json::from_slice(&fs::read(&a[0]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let diagnostic = prepare(&original, &a[1])?;
        let path = Path::new(&a[2]);
        if path.exists() {
            return Err("output already exists; refusing overwrite".into());
        }
        fs::write(
            path,
            serde_json::to_vec_pretty(&diagnostic).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!(
            "prepared {} diagnostic steps; full acceptance remains pending",
            diagnostic["steps"].as_array().unwrap().len()
        );
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_checks_and_turn_window_are_preserved() {
        let source: Value = serde_json::from_str(include_str!(
            "../../../civ6-autotest/scenarios/absorption-japan.json"
        ))
        .unwrap();
        let d = prepare(&source, "owned-checkpoint").unwrap();
        assert_eq!(d["steps"].as_array().unwrap().len(), 17);
        assert_eq!(d["steps"][0], source["steps"][0]);
        assert_eq!(d["steps"][1]["exact_name"], true);
        for i in 0..9 {
            assert_eq!(
                d["steps"][4 + i + usize::from(i > 4)],
                source["steps"][102 + i]
            );
        }
        assert_eq!(d["steps"][9]["action"], "wait_for");
        assert_eq!(d["steps"][15], source["steps"][111]);
    }
    #[test]
    fn observation_chunks_compile_without_executing() {
        let lua = mlua::Lua::new();
        for code in [TRACE_START, TRACE_FINISH] {
            lua.load(code).into_function().unwrap();
        }
        assert!(!TRACE_START.contains("RequestCommand"));
        assert!(!TRACE_START.contains("ChangeFaithBalance"));
        assert!(!TRACE_START.contains("UnitManager.Kill"));
    }
    #[test]
    fn input_drift_and_unsafe_paths_are_refused() {
        let mut source: Value = serde_json::from_str(include_str!(
            "../../../civ6-autotest/scenarios/absorption-japan.json"
        ))
        .unwrap();
        assert!(prepare(&source, "../save").is_err());
        source["steps"][111]["turns"] = json!(79);
        assert!(prepare(&source, "owned-save").is_err());
    }
}
