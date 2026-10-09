//! Fixed native-carrier fixture producer. The parent owns transport/control.
//! Frame body is JSON; `canonical` is the complete product/fixture observation,
//! not the growth witness's 49-field record. No runtime Lua source is accepted.
use crate::{Error, Result};
use serde::Serialize;
use serde_json::Value;
use std::{fs, path::Path};

pub const NONCE_KEY: &str = "JAC_NATIVE_CARRIER_WITNESS_NONCE";
pub const STATE_KEY: &str = "JAC_NATIVE_CARRIER_WITNESS_V1";
pub const LOAD_GENERATION_KEY: &str = "JAC_NATIVE_CARRIER_LOAD_GENERATION";
pub const ENGINE_LOAD_READY: &str = "LoadScreen: OnLoadGameViewStateDone";
pub const EVENT: &str = "JAC_NativeCarrierWitness_Operation";
pub const MID_SAVE_STEP: usize = 30;
pub const RETURNED_SAVE_STEP: usize = 38;
pub const LAST_STEP: usize = 43;
pub const ROUTE: &str = "HUMAN0_PALACE_TO_HUMAN1_TEMPLE";
const FIXTURE: &str =
    include_str!("../../civ6-autotest/scenarios/absorption-native-carrier-hotseat-save.json");
const COMMON: &str = include_str!("native_carrier_witness/common.lua");
const GAMEPLAY: &str = include_str!("native_carrier_witness/gameplay.lua");
const UI: &str = include_str!("native_carrier_witness/ui.lua");
const STAGING: &str = include_str!("native_carrier_witness/staging.lua");
const RULES: &str =
    include_str!("../../../poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Rules.lua");

#[derive(Clone, Debug, Serialize)]
pub struct EmittedWitness {
    pub nonce_key: &'static str,
    pub state_key: &'static str,
    pub load_generation_key: &'static str,
    pub route: &'static str,
    pub nonce: String,
    pub mid_save: String,
    pub returned_save: String,
    pub files: Vec<String>,
}

pub fn save_names(nonce: &str) -> Result<(String, String)> {
    if !crate::hotseat_bridge::nonce_valid(nonce) {
        return Err(Error::scenario(
            "native-carrier nonce must be 32 hex digits",
        ));
    }
    Ok((
        format!("JAC-NativeCarrier-MidLoan-{nonce}"),
        format!("JAC-NativeCarrier-Returned-{nonce}"),
    ))
}

/// Exact fixed JSON, with a runtime restart at stage 1 supplied by the parent.
/// Thus the checked JSON's 42 steps occupy protocol stages 2..43.
pub fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("checked native carrier fixture")
}

// Explicit semantic substitutions in the selected fixed command only. Never
// replace player IDs across a whole scenario: actor0 and the actual AI2 slot
// checks must stay intact. Source changes fail generation if an anchor moves.
fn human_route(stage: usize, mut code: String) -> String {
    let substitutions: &[(&str,&str)] = match stage {
        7 => &[("q:GetOwner()==0","q:GetOwner()==1")],
        8 => &[
            ("q:GetOwner()==0","q:GetOwner()==1"),
            ("local cities=Players[0]:GetCities()","local cities=Players[1]:GetCities()"),
            ("local source=assert(Players[2]:GetCities():GetCapitalCity())","local source=assert(Players[0]:GetCities():GetCapitalCity())"),
            ("target:GetOwner()==0","target:GetOwner()==1"),
            ("Recipient=0,Donor=2","Recipient=1,Donor=0,Diagnostic='CORE_CREATES_INACTIVE_HUMAN1_CITY_AND_STOCK_TEMPLE'"),
        ],
        9 => &[("c:GetOwner()==0","c:GetOwner()==1")],
        10 => &[("target:GetOwner()==0","target:GetOwner()==1")],
        11 => &[("c:GetOwner()==0","c:GetOwner()==1"),("From=2,To=0","From=0,To=1")],
        23 => &[
            ("c:GetOwner()==2","c:GetOwner()==0"),
            ("Players[2]:GetCities():GetCapitalCity()","Players[0]:GetCities():GetCapitalCity()"),
            ("Owner=2,City=c:GetID()","Owner=0,City=c:GetID()"),
            ("Owner=0})","Owner=1})"),
            ("local cap=assert(Players[0]:GetCities():GetCapitalCity())","local cap=assert(Players[1]:GetCities():GetCapitalCity())"),
            ("FindCompatibleNativeSlots(0,","FindCompatibleNativeSlots(1,"),
            ("Players[0]:GetUnits():Members()","Players[1]:GetUnits():Members()"),
        ],
        24 => &[
            ("p.Owner==2 and c:GetOwner()==2","p.Owner==0 and c:GetOwner()==0"),
            ("Players[2]:AttachModifierByID","Players[0]:AttachModifierByID"),
            ("grant to AI2","grant to human0"),
        ],
        25 => &[("source:GetOwner()==2","source:GetOwner()==0"),("w.Owner==2","w.Owner==0")],
        27 => &[("original.Owner==2","original.Owner==0"),("A.RecoverArchived(e.AssetID,0,e.Token)","A.RecoverArchived(e.AssetID,1,e.Token)")],
        28 => &[
            ("m.Actor==0 and m.MoveOwner==2","m.Actor==0 and m.MoveOwner==0"),
            ("l.Owner==0","l.Owner==1"),("target:GetOwner()==2","target:GetOwner()==0"),
            ("source:GetOwner()==2","source:GetOwner()==0"),("first.Owner==2","first.Owner==0"),
            ("Players[2]:GetCities():GetCapitalCity()","Players[0]:GetCities():GetCapitalCity()"),
            ("local cap=assert(Players[0]:GetCities():GetCapitalCity())","local cap=assert(Players[1]:GetCities():GetCapitalCity())"),
        ],
        29 => &[("b.Owner==2","b.Owner==0")],
        32 => &[("fromOwner~=2","fromOwner~=0"),("toOwner~=2","toOwner~=0"),
            ("m.Phase=='BORROWED' or m.Phase=='READY'","m.Phase=='BORROWED'")],
        33 => &[("city:GetOwner()==0","city:GetOwner()==1")],
        34 => &[("A.RecoverArchived(e.AssetID,0,e.Token)","A.RecoverArchived(e.AssetID,1,e.Token)")],
        36 | 42 => &[
            ("m.MoveOwner==2","m.MoveOwner==0"),
            ("moved.Locations[1].Owner==2","moved.Locations[1].Owner==0"),
            ("final.Locations[1].Owner==0","final.Locations[1].Owner==1"),
            ("l.Owner==0","l.Owner==1"),("target:GetOwner()==0","target:GetOwner()==1"),
            ("source:GetOwner()==2","source:GetOwner()==0"),
            ("local cap=assert(Players[2]:GetCities():GetCapitalCity())","local cap=assert(Players[0]:GetCities():GetCapitalCity())"),
            ("local recipientCap=assert(Players[0]:GetCities():GetCapitalCity())","local recipientCap=assert(Players[1]:GetCities():GetCapitalCity())"),
            ("w.Owner==0","w.Owner==1"),("A.GetRoster(0)","A.GetRoster(1)"),
            ("record.NativeCustody.Owner==0","record.NativeCustody.Owner==1"),
            ("Players[0]:GetUnits():Members()","Players[1]:GetUnits():Members()"),
        ],
        _ => &[],
    };
    for (old, new) in substitutions {
        assert_eq!(
            code.matches(old).count(),
            1,
            "human-route anchor at stage {stage}: {old}"
        );
        code = code.replacen(old, new, 1);
    }
    code = code
        .replace("AI source capital", "human0 source capital")
        .replace("AI Palace", "human0 Palace")
        .replace("borrowed to AI2", "borrowed to human0");
    if stage == 6 || stage == 41 {
        code.push_str("\n and Players[1]:GetCities():GetCapitalCity()~=nil");
    }
    code
}

/// The diagnostic human-source route; the original AI2 Tuner JSON stays exact.
pub fn witness_fixture() -> Value {
    let mut data = fixture();
    for (index, step) in data["steps"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
        .skip(4)
    {
        let stage = index + 2;
        if let Some(code) = step["code"].as_str() {
            step["code"] = human_route(stage, code.into()).into();
        }
        if let Some(code) = step["condition"]["code"].as_str() {
            step["condition"]["code"] = human_route(stage, code.into()).into();
        }
        if let Some(code) = step["before_begin_lua"]["code"].as_str() {
            step["before_begin_lua"]["code"] = human_route(stage, code.into()).into();
        }
    }
    data
}

fn plan() -> (String, String) {
    let data = witness_fixture();
    let steps = data["steps"].as_array().expect("fixed steps");
    assert_eq!(steps.len() + 1, LAST_STEP);
    let mut gp = String::from("local operations={}\nlocal operation_order={}\n");
    let mut ui = String::from("local actions={}\n");
    for (index, step) in steps.iter().enumerate().skip(4) {
        let n = index + 2;
        let action = step["action"].as_str().unwrap();
        match action {
            "ui_lua" if step["state_name"] == "JAC_Absorption_Entry" => {
                gp.push_str(&format!("operations[{n}]=function()\n{}\nend\noperation_order[#operation_order+1]={n}\n",step["code"].as_str().unwrap()));
                ui.push_str(&format!("actions[{n}]={{kind='GP'}}\n"));
            }
            "ui_lua" | "wait_for" => {
                let code = if action == "wait_for" {
                    step["condition"]["code"].as_str().unwrap()
                } else {
                    step["code"].as_str().unwrap()
                };
                let capture = step["capture_as"].as_str().unwrap_or("");
                let compare = step["equals_capture"].as_str().unwrap_or("");
                ui.push_str(&format!(
                    "actions[{n}]={{kind={},capture={},compare={},run=function()\n{code}\nend}}\n",
                    crate::scenario::lua_string(action),
                    crate::scenario::lua_string(capture),
                    crate::scenario::lua_string(compare)
                ));
            }
            "save" => {
                assert!([MID_SAVE_STEP, RETURNED_SAVE_STEP].contains(&n));
                gp.push_str(&format!("operations[{n}]=function() return prepareSave({}) end\noperation_order[#operation_order+1]={n}\n",if n==MID_SAVE_STEP {1} else {2}));
                ui.push_str(&format!(
                    "actions[{n}]={{kind='save',generation={}}}\n",
                    if n == MID_SAVE_STEP { 1 } else { 2 }
                ));
            }
            "exit_to_menu" => ui.push_str(&format!("actions[{n}]={{kind='exit'}}\n")),
            "load" => ui.push_str(&format!("actions[{n}]={{kind='external_load'}}\n")),
            _ => panic!("unregistered fixed fixture action at {n}"),
        }
    }
    let hook = steps[30]["before_begin_lua"]["code"]
        .as_str()
        .expect("checked early hook");
    ui.push_str(&format!("local function armLoadedMove()\n{hook}\nend\n"));
    // A forward declaration lets the fixed preparation closures share the
    // same Gameplay context as the operation receipt handler.
    (format!("local prepareSave\n{gp}"), ui)
}

pub fn gameplay_binding() -> String {
    let (plan, _) = plan();
    format!("-- Generated by native_carrier_witness.rs; fixed fixture only.\n{COMMON}\n{plan}\n{GAMEPLAY}")
}
pub fn ui_binding() -> String {
    let (_, plan) = plan();
    format!(
        "-- Generated by native_carrier_witness.rs; HeroKeys context only.\n{COMMON}\n{plan}\nlocal function initialize()\n{UI}\nend\nlocal initialized,initialError=pcall(initialize)\nif not initialized then\n local reported=false\n ContextPtr:SetUpdate(function()\n  if not reported and Game.GetLocalPlayer()>=0 then\n   reported=true\n   local saved=Game:GetProperty(bridgeKey)\n   emit('FAIL',saved and saved.Generation or 0,0,JAC_AbsorptionRules.Canonical({{Error=tostring(initialError)}}),{{code='EARLY_UI_INITIALIZATION',error=tostring(initialError)}})\n  end\n end)\nend\n"
    )
}
pub fn staging_binding() -> String {
    format!("-- Generated by native_carrier_witness.rs; existing Japanese staging context.\n{COMMON}\n{RULES}\n{STAGING}")
}

/// Emit repository-relative deploy files into a parent-owned staging directory.
/// No game installation or live MOD tree is accessed by this API.
pub fn emit(root: &Path, nonce: &str) -> Result<EmittedWitness> {
    let (mid_save, returned_save) = save_names(nonce)?;
    let scripts = [
        (
            "poc/JAC_PoC_Absorption/Scripts/JAC_NativeCarrierWitness_GP.lua",
            gameplay_binding(),
        ),
        (
            "poc/JAC_PoC_Absorption/UI/JAC_NativeCarrierWitness_UI.lua",
            ui_binding(),
        ),
        (
            "poc/JAC_PoC_JapanMapScale/UI/JAC_NativeCarrierWitness_Staging.lua",
            staging_binding(),
        ),
    ];
    let mut files = Vec::new();
    for (relative, source) in scripts {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, source)?;
        files.push(relative.to_owned());
    }
    Ok(EmittedWitness {
        nonce_key: NONCE_KEY,
        state_key: STATE_KEY,
        load_generation_key: LOAD_GENERATION_KEY,
        route: ROUTE,
        nonce: nonce.into(),
        mid_save,
        returned_save,
        files,
    })
}
