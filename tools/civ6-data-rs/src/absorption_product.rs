//! Typed, Japanese-authoritative product terms and ordinary-game integration.
use crate::{Outputs, Repository};
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const DATA: &str = "documents/spec/data/absorption-product.json";
pub const GAMEPLAY: &str = "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Product.lua";
pub const UI: &str = "poc/JAC_PoC_Absorption/UI/JAC_Absorption_AdmissionUI.lua";
pub const CITY_STATES: &str = "poc/JAC_PoC_Absorption/UI/JAC_Absorption_CityStates.lua";
pub const DIPLOMACY: &str = "poc/JAC_PoC_CustomCivs/UI/JAC_DiplomacyActionView.lua";
pub const TEXT: &str = texts::OUTPUT;
pub const NATIVE_READ_UI: &str = "poc/JAC_PoC_Absorption/UI/JAC_Absorption_ProductNativeRead.lua";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    CombatUnits,
    PeakCombat,
    Access,
    Technologies,
    Population,
    FaithYield,
    FoodYield,
    ScienceYield,
}
impl Metric {
    fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "COMBAT_UNITS" => Self::CombatUnits,
            "PEAK_COMBAT" => Self::PeakCombat,
            "ACCESS" => Self::Access,
            "TECHNOLOGIES" => Self::Technologies,
            "POPULATION" => Self::Population,
            "FAITH_YIELD" => Self::FaithYield,
            "FOOD_YIELD" => Self::FoodYield,
            "SCIENCE_YIELD" => Self::ScienceYield,
            _ => bail!("unknown product metric {value}"),
        })
    }
}
#[derive(Clone, Debug)]
pub struct CountryLeader {
    pub country: String,
    pub civilization: String,
    pub leader: String,
    pub area_km2: f64,
    pub personality: String,
    pub trait_name: String,
    pub metric: Metric,
}
#[derive(Clone, Debug)]
pub struct Catalog {
    pub leaders: Vec<CountryLeader>,
    pub runtime: Value,
}

pub fn catalog(repo: &Repository) -> Result<Catalog> {
    let data = repo.json(DATA)?;
    ensure!(data["version"] == 1, "unsupported product catalog");
    let areas = repo.json("documents/spec/data/absorption-country-areas.json")?;
    let personalities = data["personalities"].as_array().context("personalities")?;
    let traits = data["traits"].as_array().context("traits")?;
    ensure!(
        personalities.len() == 8 && traits.len() == 8,
        "SSOT04 eight personalities and traits required"
    );
    let expected_personalities: BTreeSet<_> = [
        "勇敢", "冷静", "慎重", "陽気", "温厚", "強気", "自由", "忠実",
    ]
    .into_iter()
    .collect();
    ensure!(
        personalities
            .iter()
            .map(|v| v["name"].as_str().unwrap_or(""))
            .collect::<BTreeSet<_>>()
            == expected_personalities,
        "Japanese personality names required"
    );
    let expected_traits: BTreeSet<_> = [
        "武人", "鬼神", "策士", "魔神", "教者", "霊神", "凡人", "天才",
    ]
    .into_iter()
    .collect();
    ensure!(
        traits
            .iter()
            .map(|v| v["name"].as_str().unwrap_or(""))
            .collect::<BTreeSet<_>>()
            == expected_traits,
        "Japanese trait names required"
    );
    let mut pmap = BTreeMap::new();
    let mut tmap = BTreeMap::new();
    for p in personalities {
        ensure!(
            p["scale_bps"].as_u64().is_some_and(|n| n > 0)
                && ["none", "gold", "gold_faith"].contains(&p["deposit"].as_str().unwrap_or("")),
            "invalid personality balance"
        );
        pmap.insert(p["name"].as_str().unwrap().to_owned(), p.clone());
    }
    for t in traits {
        Metric::parse(t["metric"].as_str().context("metric")?)?;
        tmap.insert(t["name"].as_str().unwrap().to_owned(), t.clone());
    }
    for (name, entry) in &mut pmap {
        entry["Label"] =
            json!(texts::label(name, "PERSONALITY_").context("personality display label")?);
    }
    for (name, entry) in &mut tmap {
        entry["Label"] = json!(texts::label(name, "TRAIT_").context("trait display label")?);
        entry["label"] = json!(format!(
            "LOC_JAC_ADMISSION_METRIC_{}",
            entry["metric"].as_str().unwrap()
        ));
    }
    let mut leaders = Vec::new();
    let mut countries = BTreeMap::new();
    let mut profiles = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let seats = repo.json("tools/civ6-assets-rs/tests/fixtures/constants.json")?;
    for row in data["leaders"].as_array().context("leaders")? {
        let r = row.as_array().context("typed leader row")?;
        ensure!(
            r.len() == 5,
            "explicit leader assignment and basis required"
        );
        let country = r[0].as_str().context("country ID")?;
        let leader = r[1].as_str().context("leader type")?;
        let personality = r[2].as_str().context("personality")?;
        let trait_name = r[3].as_str().context("trait")?;
        ensure!(
            pmap.contains_key(personality) && tmap.contains_key(trait_name),
            "unknown Japanese personality/trait"
        );
        let minor = data["minor_countries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == country);
        let area = minor
            .map(|m| &m["area_km2"])
            .unwrap_or(&areas["areas_km2"][country])
            .as_f64()
            .context("explicit geographic area required")?;
        ensure!(area.is_finite() && area > 0., "invalid real country area");
        let civilization = if minor.is_some() {
            format!("CIVILIZATION_{country}")
        } else {
            format!("CIVILIZATION_JAC_{country}")
        };
        let key = format!("{civilization}:{leader}");
        ensure!(seen.insert(key.clone()), "duplicate actor mapping");
        let metric = Metric::parse(tmap[trait_name]["metric"].as_str().unwrap())?;
        leaders.push(CountryLeader {
            country: country.to_owned(),
            civilization: civilization.clone(),
            leader: leader.to_owned(),
            area_km2: area,
            personality: personality.to_owned(),
            trait_name: trait_name.to_owned(),
            metric,
        });
        profiles.insert(key,json!({"CountryID":country,"CivilizationType":civilization,"LeaderType":leader,"Personality":personality,"Trait":trait_name,"PersonalityLabel":pmap[personality]["Label"],"TraitLabel":tmap[trait_name]["Label"],"Basis":r[4]}));
        countries.insert(country.to_owned(),json!({"ID":country,"Name":format!("LOC_{}_NAME",civilization),"AreaKm2":area,"AreaSource":minor.map(|m|m["source"].clone()).unwrap_or(json!("documents/spec/data/absorption-country-areas.json")),"RelationsReviewed":country!="SINGAPORE","RelationSource":if minor.is_some(){"SSOT03/14.4の親国・協定監査"}else{"現製品の1534年関係台帳。実行時の関係も再読取"}}));
    }
    for seat in seats["seats"].as_array().unwrap() {
        let key = format!(
            "{}:{}",
            seat["civ"].as_str().unwrap(),
            seat["leader"].as_str().unwrap()
        );
        ensure!(
            profiles.contains_key(&key),
            "current product leader unconnected: {key}"
        );
    }
    // Keep the accepted 1534 relationship inventory; neither translate vassals
    // into independence nor use an empty list to mark unreviewed actors ready.
    let mut relations = repo.json("documents/spec/data/japan-1534-relations.json")?;
    for relation in relations["relations"]
        .as_array()
        .context("initial relation inventory")?
    {
        ensure!(
            relation["id"].is_string()
                && relation["a"].is_string()
                && relation["b"].is_string()
                && relation["type"].is_string(),
            "explicit initial relationship identity required"
        );
    }
    relations["relations"]
        .as_array_mut()
        .unwrap()
        .sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let runtime = json!({"Version":1,"Balance":data["balance"],"Personalities":pmap,"Traits":tmap,"Countries":countries,"Profiles":profiles,"InitialRelations":relations,"DepositPolicy":data["deposit_policy"]});
    Ok(Catalog { leaders, runtime })
}

pub(super) fn to_lua(v: &Value) -> String {
    match v {
        Value::Null => "nil".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            // Preserve internal canonical UTF-8 values in Lua 5.1 exactly.
            // All display text uses LOC keys, independent of these bytes.
            let mut value = String::from("\"");
            for byte in s.bytes() {
                match byte {
                    b'\\' => value.push_str("\\\\"),
                    b'"' => value.push_str("\\\""),
                    32..=126 => value.push(char::from(byte)),
                    _ => value.push_str(&format!("\\{byte:03}")),
                }
            }
            value.push('"');
            value
        }
        Value::Array(a) => format!("{{{}}}", a.iter().map(to_lua).collect::<Vec<_>>().join(",")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("[{}]={}", to_lua(&json!(k)), to_lua(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}
pub fn gameplay_lua(repo: &Repository) -> Result<String> {
    Ok(render_gameplay(&catalog(repo)?))
}
pub fn render_gameplay(catalog: &Catalog) -> String {
    let mut runtime = catalog.runtime.clone();
    runtime["InitialRelations"]["relations"]
        .as_array_mut()
        .expect("validated relation catalog")
        .sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    format!(
        "-- Generated by absorption-product-generate (Rust). Do not edit.\nlocal C={}\n{}",
        to_lua(&runtime),
        GAMEPLAY_BODY
    )
}
pub fn generate(repo: &Repository) -> Result<Outputs> {
    let mut out = crate::absorption_admission::generate(repo)?;
    out.insert(GAMEPLAY.into(), gameplay_lua(repo)?);
    let ui = ui::LUA.replace(
        "-- Shared personality/trait locale lookup.",
        &texts::lookup_lua(),
    );
    out.insert(UI.into(), ui.clone());
    out.insert(CITY_STATES.into(), citystates::LUA.to_owned());
    out.insert(NATIVE_READ_UI.into(), native_ui::LUA.to_owned());
    out.insert(texts::OUTPUT.into(), texts::xml());
    // CustomCivs also runs when absorption is disabled. Keep the adapter in
    // this replacement so it never imports a file from a disabled MOD.
    let adapter = ui
        .strip_suffix("return U\n")
        .context("UI adapter must finish with its module return")?;
    out.insert(
        DIPLOMACY.into(),
        diplomacy::LUA.replacen(
            "include('JAC_Absorption_AdmissionUI')",
            &format!("do\n{adapter}\nend"),
            1,
        ),
    );
    let manifest = "poc/JAC_PoC_Absorption/JAC_PoC_Absorption.modinfo";
    let mut source = repo.read(manifest)?;
    ensure!(
        source.matches("</InGameActions>").count() == 1
            && source.matches("<Files>").count() == 1
            && source.matches("</Files>").count() == 1,
        "unique absorption action and file sections required"
    );
    for (id, action) in [
        (
            "JAC_Absorption_Product_NativeRead",
            r#"    <ImportFiles id="JAC_Absorption_Product_NativeRead" criteria="JAC_Absorption_Banseiki"><File>UI/JAC_Absorption_ProductNativeRead.lua</File></ImportFiles>"#,
        ),
        (
            "JAC_Absorption_Product_UI_Files",
            r#"    <ImportFiles id="JAC_Absorption_Product_UI_Files" criteria="JAC_Absorption_Banseiki"><File>UI/JAC_Absorption_AdmissionUI.lua</File><File>UI/JAC_Absorption_CityStates.lua</File></ImportFiles>"#,
        ),
        (
            "JAC_Absorption_Product_CityStates",
            r#"    <ReplaceUIScript id="JAC_Absorption_Product_CityStates" criteria="JAC_Absorption_Banseiki"><Properties><LoadOrder>220</LoadOrder><LuaContext>CityStates</LuaContext><LuaReplace>UI/JAC_Absorption_CityStates.lua</LuaReplace></Properties></ReplaceUIScript>"#,
        ),
        (
            "JAC_Absorption_Admission_Text",
            r#"    <UpdateText id="JAC_Absorption_Admission_Text" criteria="JAC_Absorption_Banseiki"><File>Text/JAC_Absorption_Admission_Text.xml</File></UpdateText>"#,
        ),
    ] {
        if !source.contains(&format!("id=\"{id}\"")) {
            source = source.replacen(
                "  </InGameActions>",
                &format!("{action}\n  </InGameActions>"),
                1,
            );
        }
    }
    for file in [
        "UI/JAC_Absorption_AdmissionUI.lua",
        "UI/JAC_Absorption_CityStates.lua",
        "UI/JAC_Absorption_ProductNativeRead.lua",
        "Text/JAC_Absorption_Admission_Text.xml",
    ] {
        let start = source.find("<Files>").unwrap();
        let declaration = format!("<File>{file}</File>");
        if !source[start..].contains(&declaration) {
            source = source.replacen("  </Files>", &format!("    {declaration}\n  </Files>"), 1);
        }
    }
    out.insert(manifest.into(), source);
    let entry = "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Entry.lua";
    let source = repo.read(entry)?;
    let include = "include(\"JAC_Absorption_Product\")";
    let result = if source.contains(include) {
        source
    } else {
        source.replace(
            "include(\"JAC_Absorption_Admission\")",
            &format!("include(\"JAC_Absorption_Admission\")\n{include}"),
        )
    };
    ensure!(result.contains(include), "ordinary Gameplay entry missing");
    out.insert(entry.into(), result);
    // Inline the same pure lookup in FlowUI. It has no dependency on the
    // admission module or its VFS/load order, including other flow features.
    let flow_ui = "poc/JAC_PoC_Absorption/UI/JAC_AbsorptionFlow.lua";
    let mut source = repo.read(flow_ui)?;
    let native_include = "include(\"JAC_Absorption_ProductNativeRead\")";
    if !source.contains(native_include) {
        source = source.replacen(
            "include(\"InstanceManager\")",
            &format!("include(\"InstanceManager\")\n{native_include}"),
            1,
        );
    }
    let localized = "local function Localized(value)";
    let end = source
        .find(localized)
        .context("FlowUI localization entry")?;
    let display = format!("{}\n{}\n", texts::lookup_lua(), texts::FLOW_DEPOSIT);
    if let Some(start) = source[..end].find("local DisplayLocaleKey =") {
        source.replace_range(start..end, &display);
    } else {
        source.insert_str(end, &display);
    }
    let old = "  return s:sub(1, 4) == \"LOC_\" and Locale.Lookup(s) or s";
    let new = "  local key = DisplayLocaleKey(s)\n  return key:sub(1, 4) == \"LOC_\" and Locale.Lookup(key) or s";
    if !source.contains(new) {
        ensure!(
            source.matches(old).count() == 1,
            "FlowUI locale logic changed"
        );
        source = source.replacen(old, new, 1);
    }
    let hint = "  rendered.hint = Controls.Hint:GetText()";
    let policy = "  rendered.deposit = DepositText()\n  if rendered.deposit then Controls.Hint:SetText(Controls.Hint:GetText() .. \"[NEWLINE]\" .. rendered.deposit) end\n";
    if !source.contains(policy) {
        ensure!(
            source.matches(hint).count() == 1,
            "FlowUI hint entry changed"
        );
        source = source.replacen(hint, &format!("{policy}{hint}"), 1);
    }
    out.insert(flow_ui.into(), source);
    // Generate the small additions to the established asset engines as well.
    // Their code remains the authority for inventory, random rolls and replay.
    for (path,before,after) in [
      ("poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Flow.lua",
       "      Loyalty = 50 }, Leader = true,",
       "      Loyalty = 50, Personality = plan.LeaderProfile and plan.LeaderProfile.Personality,\n      Trait = plan.LeaderProfile and plan.LeaderProfile.Trait,\n      CountryID = plan.LeaderProfile and plan.LeaderProfile.CountryID }, Leader = true,"),
      ("poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
       "    LeaderType = PlayerConfigurations[plan.From]:GetLeaderTypeName(), Persistent = true,",
       "    LeaderType = PlayerConfigurations[plan.From]:GetLeaderTypeName(), Persistent = true,\n    Personality = plan.LeaderProfile and plan.LeaderProfile.Personality,\n    Trait = plan.LeaderProfile and plan.LeaderProfile.Trait,\n    CountryID = plan.LeaderProfile and plan.LeaderProfile.CountryID,")
    ] {
       let source=repo.read(path)?;
       let result=if source.contains(after){source}else{ensure!(source.matches(before).count()==1,"leader propagation source changed: {path}");source.replacen(before,after,1)};
       out.insert(path.into(),result);
    }
    for small in [true, false] {
        for human in [true, false] {
            let name = format!(
                "absorption-product-{}-{}.json",
                if small { "small" } else { "merger" },
                if human { "human" } else { "ai" }
            );
            out.insert(
                format!("tools/civ6-autotest/scenarios/{name}").into(),
                acceptance_scenario(repo, small, human)?,
            );
        }
    }
    Ok(out)
}

/// Assets are prepared for acceptance, but country/profile/area/requirements
/// and admission come only from the ordinary product source and synchronized
/// operation. This is not a diagnostic offer substituted for product terms.
pub fn acceptance_scenario(repo: &Repository, small: bool, human: bool) -> Result<String> {
    let base = repo.json("tools/civ6-autotest/scenarios/absorption-flow.json")?;
    let mut steps = base["steps"].as_array().unwrap()[..6].to_vec();
    steps.push(crate::absorption_admission::start_calendar_step());
    let from = if small {
        "CIVILIZATION_JAC_ODA"
    } else {
        "CIVILIZATION_JAC_TAKEDA"
    };
    let to = if small {
        "CIVILIZATION_JAC_TAKEDA"
    } else {
        "CIVILIZATION_JAC_ODA"
    };
    for player in steps[1]["players"].as_array_mut().unwrap() {
        player["human"] = json!(if human {
            player["civ"] == to
        } else {
            player["civ"] == "CIVILIZATION_JAC_ASHINA"
        });
    }
    fn core(code: String) -> Value {
        json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":code})
    }
    fn ui(code: String) -> Value {
        json!({"action":"ui_lua","state_name":"InGame","code":code})
    }
    fn wait(code: String) -> Value {
        json!({"action":"wait_for","timeout":600,"condition":{"type":"lua","state_name":"InGame","code":code}})
    }
    steps.push(wait(
        "return ExposedMembers.JAC_AbsorptionProductNativeRead~=nil and ExposedMembers.JAC_HeroReadAPI and type(ExposedMembers.JAC_HeroReadAPI.ReadPlayer)=='function'".into(),
    ));
    steps.push(core(format!(r#"
local P=assert(JAC_AbsorptionProductGameplay)
local from,to
for _,player in ipairs(PlayerManager.GetAliveMajors()) do
  local civ=PlayerConfigurations[player:GetID()]:GetCivilizationTypeName()
  if civ=='{from}' then from=player:GetID() elseif civ=='{to}' then to=player:GetID() end
end
assert(from and to and not Players[from]:IsHuman(),'ordinary source actor required')
local failures,checked={{}},0
for _,other in ipairs(PlayerManager.GetAliveMajors()) do
  if other:GetID()~=to and not other:IsHuman() then
    local candidate=P.Get(other:GetID(),to);checked=checked+1
    if candidate.Status=='UNAVAILABLE' then failures[#failures+1]=tostring(other:GetID())..': '..tostring(candidate.Error) end
  end
end
assert(checked>0 and #failures==0,'native country reads unavailable: '..table.concat(failures,' | '))
local view=P.Get(from,to);assert(view.Status=='AVAILABLE',view.Error)
assert(view.Mode=='{mode}' and view.Country.Profile.LeaderType==PlayerConfigurations[from]:GetLeaderTypeName())
assert(view.Country.AreaKm2>0 and (view.Country.AreaKm2<10000)=={small})
Game:SetProperty('JAC_PRODUCT_ACCEPTANCE_V1',{{From=from,To=to,Mode=view.Mode,Initial=JAC_AbsorptionGameplay.Rules.Copy(view)}})
return 'ordinary country, current leader, geographic area, personality and trait verified'
"#,mode=if small{"SMALL_STATE"}else{"MERGER"},small=small)));
    // Use the approved existing screen and the very handler bound to its
    // action control. The server still derives all terms and authenticates
    // the actor; no test-created demand or ready flag enters registration.
    if human {
        let screen = if small {
            "CityStates"
        } else {
            "DiplomacyActionView"
        };
        let event = if small {
            "CityStates_OpenOverview"
        } else {
            "TopPanel_OpenDiplomacyActionView"
        };
        let handler = if small {
            "JAC_AbsorptionCityStatesAction"
        } else {
            "JAC_AbsorptionDiplomacyAction"
        };
        steps.push(ui(format!("local t=Game:GetProperty('JAC_PRODUCT_ACCEPTANCE_V1');assert(t.To==Game.GetLocalPlayer());LuaEvents.{event}(t.From);return 'ordinary admission screen opened'")));
        steps.push(json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":screen,"code":format!("return not ContextPtr:IsHidden() and type({handler})=='function'")}}));
        steps.push(json!({"action":"ui_lua","state_name":screen,"code":format!("local t=Game:GetProperty('JAC_PRODUCT_ACCEPTANCE_V1');local v=assert(JAC_AbsorptionAdmissionUI.Get(t.From));assert(v.Mode==t.Mode and #v.Rows>0);assert({handler}(t.From,'REGISTER'),'visible screen registration unavailable');OnClose();return 'existing screen showed product terms and submitted its registration action'")}));
    }
    steps.push(core(
        r#"
local t=assert(Game:GetProperty('JAC_PRODUCT_ACCEPTANCE_V1'))
local p=Players[t.To]
local treasury=p:GetTreasury();treasury:ChangeGoldBalance(-treasury:GetGoldBalance())
-- Keep the separate official token pool, rather than assigning a fake count.
local influence=p:GetInfluence();influence:ChangeTokensToGive(-influence:GetTokensToGive())
return 'actual deposit funds and unassigned envoy pool are unmet'
"#
        .into(),
    ));
    steps.push(json!({"action":"end_turn","turns":1,"timeout":600}));
    steps.push(core(r#"
local t=assert(Game:GetProperty('JAC_PRODUCT_ACCEPTANCE_V1'))
local v=JAC_AbsorptionProductGameplay.Get(t.From,t.To)
assert(v.Status~='STARTED','unmet actual product demands started absorption')
assert(Players[t.To]:GetTreasury():GetGoldBalance()<v.Deposit.Gold,'unmet deposit fixture did not hold')
local cap=Players[t.To]:GetCities():GetCapitalCity()
-- Supply real native troops with normal unit definitions. No reader/ready
-- flags or diagnostic requirements are installed.
local kind
for def in GameInfo.Units() do
  local combat=math.max(def.Combat or 0,def.RangedCombat or 0)
  if combat>0 and (not kind or combat>kind.Power or combat==kind.Power and def.UnitType<kind.Type) then kind={Type=def.UnitType,Power=combat} end
end
assert(kind,'normal combat unit missing')
local minimum=0;local metric
for _,row in ipairs(v.Rows) do if row.Kind=='PRODUCT_METRIC' then minimum=row.Minimum;metric=row.ID end end
assert(metric and minimum>0,'ordinary leader demand missing')
local count=metric=='leader:PEAK_COMBAT' and math.ceil(minimum/kind.Power)+2 or minimum+2
for i=1,count do UnitManager.InitUnitValidAdjacentHex(t.To,kind.Type,cap:GetX(),cap:GetY(),20) end
local gold=v.Deposit.Gold+10000;local faith=v.Deposit.Faith+10000
Players[t.To]:GetTreasury():ChangeGoldBalance(gold-Players[t.To]:GetTreasury():GetGoldBalance())
Players[t.To]:GetReligion():ChangeFaithBalance(faith-Players[t.To]:GetReligion():GetFaithBalance())
if v.Envoys then Players[t.To]:GetInfluence():ChangeTokensToGive(v.Envoys.Required+2) end
t.GoldBefore=Players[t.To]:GetTreasury():GetGoldBalance();t.FaithBefore=Players[t.To]:GetReligion():GetFaithBalance()
Game:SetProperty('JAC_PRODUCT_ACCEPTANCE_V1',t)
return 'real troops, currencies and official unassigned envoys prepared for ordinary terms'
"#.into()));
    if human && small {
        steps.push(ui("local t=Game:GetProperty('JAC_PRODUCT_ACCEPTANCE_V1');LuaEvents.CityStates_OpenOverview(t.From);return 'small-country screen reopened for envoy assignment'".into()));
        steps.push(json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"CityStates","code":"return not ContextPtr:IsHidden() and type(JAC_AbsorptionCityStatesAction)=='function'"}}));
        steps.push(
            json!({"action":"ui_lua","state_name":"CityStates","code":r#"
local t=Game:GetProperty('JAC_PRODUCT_ACCEPTANCE_V1')
local send;local lastSent=-1
send=function()
  local view=assert(JAC_AbsorptionAdmissionUI.Get(t.From))
  assert(view.Envoys.Route=='PRODUCT' and not Players[t.From]:GetInfluence():CanReceiveInfluence())
  if view.Envoys.Envoys>=view.Envoys.Required then
    Events.GameCoreEventPublishComplete.Remove(send);JAC_PRODUCT_ENVOYS_FINISHED=true;return
  end
  -- Publications can arrive before this operation's assignment is visible.
  -- Send the next one only after the actual count confirms the previous one.
  if view.Envoys.Envoys<=lastSent or not Players[t.To]:IsTurnActive() then return end
  lastSent=view.Envoys.Envoys
  if not JAC_AbsorptionCityStatesAction(t.From,'SEND_ENVOY') then
    Events.GameCoreEventPublishComplete.Remove(send)
    JAC_PRODUCT_ENVOYS_ERROR='visible envoy action unavailable at received='..lastSent
  end
end
JAC_PRODUCT_ENVOYS_FINISHED=false;JAC_PRODUCT_ENVOYS_ERROR=nil
Events.GameCoreEventPublishComplete.Add(send);send()
return 'existing screen envoy action advances after each real game-core publication'
"#}),
        );
        steps.push(json!({"action":"wait_for","timeout":180,"condition":{"type":"lua","state_name":"CityStates","code":"assert(not JAC_PRODUCT_ENVOYS_ERROR,JAC_PRODUCT_ENVOYS_ERROR);return JAC_PRODUCT_ENVOYS_FINISHED==true"}}));
        steps.push(json!({"action":"ui_lua","state_name":"CityStates","code":"OnClose();return 'completed envoy screen closed'"}));
    }
    // AI sends at most one envoy to a given country per normal turn. These
    // scenes use real turn advancement, with a bound from the real territory.
    steps.push(json!({"action":"save","name":format!("JAC-product-{}-{}-waiting",if small{"small"}else{"merger"},if human{"human"}else{"ai"})}));
    steps.push(json!({"action":"load","name":format!("JAC-product-{}-{}-waiting",if small{"small"}else{"merger"},if human{"human"}else{"ai"}),"timeout":600}));
    steps.push(wait("return ExposedMembers.JAC_AbsorptionProduct~=nil and Players[Game.GetLocalPlayer()]:IsTurnActive()".into()));
    steps.push(json!({"action":"end_turn","turns":if small&&!human{12}else{1},"timeout":1200}));
    steps.push(wait(
        r#"
local t=Game:GetProperty('JAC_PRODUCT_ACCEPTANCE_V1')
local v=ExposedMembers.JAC_AbsorptionProduct.Get(t.From,t.To)
if v.Status=='STARTED' then return true end
local state=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1')
local pair=state and state.Pairs[t.From..':'..t.To]
local admission=Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1')
if pair and admission then for _,request in pairs(admission.Requests) do
  if request.ID==pair.ID and request.Status=='STARTED' then return true end
end end
return false
"#
        .into(),
    ));
    steps.push(core(r#"
local t=Game:GetProperty('JAC_PRODUCT_ACCEPTANCE_V1');local state=Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1')
local pair=assert(state.Pairs[t.From..':'..t.To]);local receipt=assert(JAC_AbsorptionAdmissionGameplay.Get('leader-offer',pair.ID))
assert(receipt.Status=='STARTED' and receipt.Plan.Mode==t.Mode and receipt.Plan.LeaderProfile.LeaderType==t.Initial.Country.Profile.LeaderType)
local escrow=assert(state.Escrows[pair.ID]);assert(escrow.Gold>0 and (escrow.Status=='HELD' or escrow.Status=='CONSUMED'))
assert(escrow.CollectGold.Done and escrow.CollectGold.Before-escrow.CollectGold.After==escrow.Gold,'real gold leg did not settle')
if escrow.Faith>0 then assert(escrow.CollectFaith.Done and escrow.CollectFaith.Before-escrow.CollectFaith.After==escrow.Faith) end
local flow=assert(JAC_AbsorptionFlowGameplay.Get(receipt.FlowID));assert(flow.Phase1Complete)
t.OfferID=pair.ID;t.FlowID=receipt.FlowID;t.Snapshot=JAC_AbsorptionGameplay.Canonical(flow)
Game:SetProperty('JAC_PRODUCT_ACCEPTANCE_V1',t)
return 'ordinary product requirements, actual deposit, and normal three-stage flow accepted'
"#.into()));
    let save = format!(
        "JAC-product-{}-{}-choice",
        if small { "small" } else { "merger" },
        if human { "human" } else { "ai" }
    );
    steps.push(json!({"action":"save","name":save}));
    steps.push(json!({"action":"load","name":save,"timeout":600}));
    steps.push(wait("return ExposedMembers.JAC_AbsorptionProduct~=nil and ExposedMembers.JAC_AbsorptionFlow~=nil".into()));
    steps.push(core(r#"
local t=Game:GetProperty('JAC_PRODUCT_ACCEPTANCE_V1');local flow=JAC_AbsorptionFlowGameplay.Get(t.FlowID)
assert(JAC_AbsorptionGameplay.Canonical(flow)==t.Snapshot,'saved product P1 changed')
local before=Players[t.To]:GetTreasury():GetGoldBalance()
JAC_AbsorptionAdmissionGameplay.Request('leader-offer',t.OfferID,t.From,t.To)
assert(JAC_AbsorptionGameplay.Canonical(JAC_AbsorptionFlowGameplay.Get(t.FlowID))==t.Snapshot)
assert(Players[t.To]:GetTreasury():GetGoldBalance()==before,'product replay charged twice')
return 'product save/load and duplicate admission preserve first-stage rolls and payment'
"#.into()));
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(
            &json!({"name":format!("Product {} admission with {} recipient",if small{"small-country"}else{"merger"},if human{"human"}else{"AI"}),"judge_profile":"generic","steps":steps})
        )?
    ))
}

#[path = "absorption_product_gameplay.rs"]
mod gameplay;
const GAMEPLAY_BODY: &str = gameplay::LUA;
#[path = "absorption_product_citystates.rs"]
mod citystates;
#[path = "absorption_product_diplomacy.rs"]
mod diplomacy;
#[path = "absorption_product_native_ui.rs"]
mod native_ui;
#[path = "absorption_product_text.rs"]
mod texts;
#[path = "absorption_product_ui.rs"]
mod ui;
