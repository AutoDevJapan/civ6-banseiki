//! One current-game SP case. The ordinary parent republish is initialization,
//! not a join, payment, forced balance, or inferred appointment.
use serde_json::{Value, json};
pub const SP05_SAVE: &str =
    "JAC-SP-Reassigned-Preserved-ed4b6d5031724956b78502760cc78dc4-0000000000000001";
pub const SP05_SHA: &str = "0e7ad9105948f3cd767f78dfefa3fd903ba34a159d2139bc19504d864e94ca51";

pub fn load_scene(assets: &crate::native_bridge_assets::Selection) -> crate::Result<Value> {
    let original: Value = serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/leader-persona-15km.json"
    ))?;
    let mut case = scenario();
    case["name"] =
        "AI1466 SP05 exact save -> actual reader; phase unknown; no payment permission".into();
    case["native_bridge_assets"] = assets.value();
    case["source_evidence"] = json!({"save":"SP05","turn":8,"proof":"D:/CC/Work/todo880/sp-reassign-05-proof.json",
        "source":"6eba1e5d00a2539bfe00ee9a81a008c5d3d27ab6","human_date_paid":true,"ai_join_prepared":false});
    let mut prefix = vec![
        json!({"action":"restart_game","timeout":420}),
        original["steps"][0].clone(),
        json!({"action":"restart_game","timeout":420}),
        json!({"action":"load","name":SP05_SAVE,"exact_name":true,"save_sha256":SP05_SHA,"timeout":300}),
        json!({"action":"wait_for","condition":{"type":"lua","state_name":"InGame",
            "code":"return not UI.IsGameCoreBusy() and Players[0] and Players[0]:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersona)=='table'"},"timeout":90}),
    ];
    prefix.extend(case["steps"].as_array().unwrap().iter().cloned());
    case["steps"] = prefix.into();
    crate::scenario::validate_scenario(&case)?;
    Ok(case)
}

/// Offline staging only. Source save must be the independently proven SP05
/// bytes; copies only the two native assets into a fresh dedicated directory.
pub fn prepare_load_scene(
    save: &std::path::Path,
    native_from: &std::path::Path,
    output: &std::path::Path,
) -> crate::Result<Value> {
    use sha2::{Digest, Sha256};
    use std::{
        fs::{File, OpenOptions},
        io::{self, Seek, SeekFrom, Write},
    };
    fn pin(path: &std::path::Path) -> io::Result<File> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        options.open(path)
    }
    let mut source = pin(save)?;
    let mut hasher = Sha256::new();
    io::copy(&mut source, &mut hasher)?;
    if save.file_stem().and_then(|s| s.to_str()) != Some(SP05_SAVE)
        || format!("{:x}", hasher.finalize()) != SP05_SHA
    {
        return Err(crate::Error::input(
            "source must be the exact independently verified SP05 save",
        ));
    }
    let mut controller = pin(&native_from.join("civ6-native-bridge.exe"))?;
    let mut dll = pin(&native_from.join("civ6_native_bridge_rs.dll"))?;
    let directory = output.join("native");
    std::fs::create_dir(&directory)?;
    let copy = |input: &mut File, name: &str| -> crate::Result<String> {
        input.seek(SeekFrom::Start(0))?;
        let mut dest = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(name))?;
        io::copy(input, &mut dest)?;
        dest.flush()?;
        dest.sync_all()?;
        input.seek(SeekFrom::Start(0))?;
        let mut hash = Sha256::new();
        io::copy(input, &mut hash)?;
        Ok(format!("{:x}", hash.finalize()))
    };
    let assets = crate::native_bridge_assets::Selection {
        directory: directory.canonicalize()?,
        controller_sha256: copy(&mut controller, "civ6-native-bridge.exe")?,
        dll_sha256: copy(&mut dll, "civ6_native_bridge_rs.dll")?,
    };
    load_scene(&assets)
}
pub fn scenario() -> Value {
    json!({"name":"AI1466 actual native reader; normal ledger preserved; no payment permission",
    "session_type":"single_player","judge_profile":"generic","strict_lua_errors":true,
    "steps":[
        {"action":"ui_lua","state_name":"InGame","code":crate::runner::AI_READER_READ,"capture_as":"AI1466_READER_PREFLIGHT","timeout":15},
        {"action":"start_native_paid_runtime","name":"ai1466-reader","max_seconds":300,"timeout":30},
        {"action":"ui_lua","state_name":"InGame","timeout":15,
         "code":"assert(Game.GetLocalPlayer()==0 and Players[0]:IsTurnActive(),'human turn required'); UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaRepublish'}); return 'one existing normal ledger refresh; no join or payment requested'"},
        {"action":"wait_for","condition":{"type":"lua","state_name":"InGame","code":"return not UI.IsGameCoreBusy() and Players[0]:IsTurnActive()"},"timeout":30},
        {"action":"start_native_ai_reader","timeout":60},
        {"action":"end_turn","turns":1,"timeout":120},
        {"action":"stop_native_ai_reader","timeout":30},
        {"action":"stop_native_paid_runtime_observation","timeout":30}
    ]})
}
