//! Test-only hotseat witness bundle and strict Lua.log transport.
//! No FireTuner command is sent from this module while a game is running.
use crate::{judge, scenario, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub const QUERY_STEPS: [usize; 9] = [4, 5, 7, 8, 12, 14, 15, 17, 21];
// Native Windows keys; Lua binds the separate Keys.VK_F* constants from the game.
pub const KEY_END_TURN: u16 = 0x85; // F22
pub const KEY_SAVE: u16 = 0x86; // F23
pub const KEY_EXIT: u16 = 0x87; // F24
const ACTIONS: [&str; 21] = [
    "enable_mods",
    "configure",
    "start_game",
    "ui_lua",
    "ui_lua",
    "end_turn",
    "ui_lua",
    "ui_lua",
    "save",
    "exit_to_menu",
    "load",
    "ui_lua",
    "end_turn",
    "ui_lua",
    "ui_lua",
    "end_turn",
    "ui_lua",
    "save",
    "exit_to_menu",
    "load",
    "ui_lua",
];
const WITNESS: &str = include_str!("../../../poc/JAC_Test_HotseatBridge/UI/JAC_HotseatWitness.lua");
const XML: &str = include_str!("../../../poc/JAC_Test_HotseatBridge/UI/JAC_HotseatWitness.xml");
const ORIGINAL_SCENARIO: &str =
    include_str!("../../civ6-autotest/scenarios/multiplayer-hotseat-growth.json");
const BANSEIKI_SCENARIO: &str =
    include_str!("../../civ6-autotest/scenarios/multiplayer-banseiki-hotseat-growth.json");
const JAPAN_SETTLERS_SCENARIO: &str = include_str!(
    "../../civ6-autotest/scenarios/multiplayer-japan-15km-settlers-hotseat-growth.json"
);
const POOL_MOD_ID: &str = "c8fb8042-8e3e-4b35-b02a-781947194700";
/// Growth queries whose captured value also carries the pool record.
pub const POOL_RECORD_STEPS: [usize; 6] = [5, 8, 12, 15, 17, 21];
const GROWTH_RETURN: &str = "return table.concat(rows,'|')";
const POOL_LEDGER: &str = include_str!("hotseat_pool/ledger.lua");
const POOL_FIXED: &str = include_str!("hotseat_pool/fixed.lua");
const POOL_RANDOM: &str = include_str!("hotseat_pool/random.lua");
/// Sent once in pregame, only for the registered pool random canonical.
pub const POOL_RANDOM_PREGAME: &str = include_str!("hotseat_pool/random_pregame.lua");
pub const POOL_RANDOM_PREGAME_RESULT: &str =
    "pool random leaders 0=RANDOM,1=RANDOM,2=RANDOM,3=RANDOM";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Canonical {
    Growth,
    BanseikiGrowth,
    JapanSettlers,
    Pool,
    PoolRandom,
}

const POOL_NAME: &str = "Banseiki pool hotseat bridge: two ledger people of one civilization, growth and saved person records";
const POOL_RANDOM_NAME: &str = "Banseiki pool hotseat bridge: random leaders for two humans and two AI, growth and saved person records";
/// Two humans (H040 and O008, both Oda), then the two AI of the random variant.
const POOL_PLAYERS: &str = r#"[
  {"civ": "CIVILIZATION_JAC_ODA", "leader": "LEADER_JAC_ODA_NOBUHIDE", "human": true},
  {"civ": "CIVILIZATION_JAC_ODA", "leader": "LEADER_JAC_POOL_O008", "human": true},
  {"civ": "CIVILIZATION_JAC_TAKEDA", "leader": "LEADER_JAC_TAKEDA_NOBUTORA", "human": false},
  {"civ": "CIVILIZATION_JAPAN", "leader": "LEADER_JAC_POOL_H042", "human": false}
]"#;
const POOL_RETURN: &str = "return growth..'|JAC_POOL|'..table.concat(poolParts,';')";

/// The pool canonicals are the Banseiki growth template with the pool MOD,
/// fixed players, and the pool record appended to every growth capture.
pub fn pool_scenario(random: bool) -> Value {
    let mut data: Value = serde_json::from_str(BANSEIKI_SCENARIO).unwrap();
    let (name, variant, count) = if random {
        (POOL_RANDOM_NAME, POOL_RANDOM, 4)
    } else {
        (POOL_NAME, POOL_FIXED, 2)
    };
    let mut players: Vec<Value> = serde_json::from_str(POOL_PLAYERS).unwrap();
    players.truncate(count);
    data["name"] = name.into();
    data["steps"][0]["ids"]
        .as_array_mut()
        .unwrap()
        .push(POOL_MOD_ID.into());
    data["steps"][1]["players"] = players.into();
    for step in POOL_RECORD_STEPS {
        let growth = data["steps"][step - 1]["code"]
            .as_str()
            .and_then(|code| code.strip_suffix(GROWTH_RETURN))
            .expect("growth query ends with its record return");
        let code = format!(
            "{growth}local growth=table.concat(rows,'|')\n{}\n{}\n{POOL_RETURN}",
            POOL_LEDGER.trim_end(),
            variant.trim_end()
        );
        data["steps"][step - 1]["code"] = code.into();
    }
    data
}

fn registered() -> [(Canonical, Value); 5] {
    [
        (
            Canonical::Growth,
            serde_json::from_str(ORIGINAL_SCENARIO).unwrap(),
        ),
        (
            Canonical::BanseikiGrowth,
            serde_json::from_str(BANSEIKI_SCENARIO).unwrap(),
        ),
        (Canonical::Pool, pool_scenario(false)),
        (Canonical::PoolRandom, pool_scenario(true)),
        (
            Canonical::JapanSettlers,
            serde_json::from_str(JAPAN_SETTLERS_SCENARIO).unwrap(),
        ),
    ]
}

/// The registered pregame helper, if any. Never derived from scenario text.
pub fn pregame_helper(data: &Value) -> Result<Option<&'static str>> {
    Ok(if canonical(data)? == Canonical::PoolRandom {
        Some(POOL_RANDOM_PREGAME)
    } else {
        None
    })
}

pub fn check_scenario(data: &Value) -> Result<()> {
    canonical(data).map(|_| ())
}

pub fn canonical(data: &Value) -> Result<Canonical> {
    scenario::validate_scenario(data)?;
    if data["session_type"] != "hotseat" || data["steps"].as_array().map(Vec::len) != Some(21) {
        return Err(Error::scenario(
            "hotseat bridge requires a registered 21-step hotseat scenario",
        ));
    }
    for (index, action) in ACTIONS.iter().enumerate() {
        let step = &data["steps"][index];
        if step["action"] != *action || (*action == "ui_lua" && step["state_name"] != "InGame") {
            return Err(Error::scenario(format!(
                "hotseat bridge does not support step {}",
                index + 1
            )));
        }
    }
    if data["steps"][8]["name"] != "JAC-Hotseat-Mid-Round"
        || data["steps"][10]["name"] != data["steps"][8]["name"]
        || data["steps"][17]["name"] != "JAC-Hotseat-Next-Player"
        || data["steps"][19]["name"] != data["steps"][17]["name"]
    {
        return Err(Error::scenario("hotseat save/load pairs changed"));
    }
    for (kind, mut expected) in registered() {
        if let Some(nonce) = data.get("hotseat_bridge_nonce").and_then(Value::as_str) {
            let ids = data["steps"][0]["ids"]
                .as_array()
                .ok_or_else(|| Error::scenario("hotseat MOD IDs missing"))?;
            let extra = ids
                .last()
                .filter(|id| {
                    id.as_str()
                        .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok())
                })
                .ok_or_else(|| Error::scenario("prepared hotseat MOD ID missing"))?;
            expected["steps"][0]["ids"]
                .as_array_mut()
                .unwrap()
                .push(extra.clone());
            expected["hotseat_bridge_nonce"] = nonce.into();
        }
        if data == &expected {
            return Ok(kind);
        }
    }
    Err(Error::scenario(
        "hotseat bridge requires one unchanged registered 21-step scenario",
    ))
}

pub fn nonce_valid(run_id: &str) -> bool {
    run_id.len() == 32 && run_id.bytes().all(|c| c.is_ascii_hexdigit())
}

fn plan(data: &Value, run_id: &str, input_diagnostic: bool) -> String {
    let mut lua = String::from(
        "local used_sdk_keys={}\nlocal function sdk_key(name)\n  local key=Keys and Keys[name]\n  if type(key)~='number' or used_sdk_keys[key] then error('distinct game key required: '..name) end\n  used_sdk_keys[key]=true\n  return key\nend\n",
    );
    lua.push_str(&format!(
        "JAC_HOTSEAT_PLAN = {{nonce={}, queries={{}}, query_keys={{}}, action_keys={{[sdk_key('VK_F22')]='end_turn',[sdk_key('VK_F23')]='save',[sdk_key('VK_F24')]='exit_to_menu'}}, action_steps={{end_turn={{['0:0']=6,['0:1']=13,['1:0']=16}}, save={{['0:1']=9,['1:1']=18}}, exit_to_menu={{['0:1']=10,['1:1']=19}}}}, save_names={{[9]={},[18]={}}}}}\n",
        scenario::lua_string(run_id),
        scenario::lua_string(&format!("JAC-Hotseat-Mid-Round-{run_id}")),
        scenario::lua_string(&format!("JAC-Hotseat-Next-Player-{run_id}"))
    ));
    for (i, step) in QUERY_STEPS.iter().enumerate() {
        let code = data["steps"][step - 1]["code"].as_str().unwrap();
        lua.push_str(&format!(
            "local key=sdk_key('VK_F{}')\nJAC_HOTSEAT_PLAN.query_keys[key]={step}\n",
            13 + i
        ));
        if i == 0 {
            lua.push_str("JAC_HOTSEAT_PLAN.input_probe_key=key\n");
        }
        lua.push_str(&format!(
            "JAC_HOTSEAT_PLAN.queries[{step}]=function()\n{code}\nend\n"
        ));
    }
    if input_diagnostic {
        lua.push_str("JAC_HOTSEAT_PLAN.input_diagnostic=true\n");
    }
    lua
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bundle {
    pub nonce: String,
    pub mod_id: String,
    pub mod_folder: String,
    pub scenario: String,
    #[serde(default)]
    pub input_diagnostic: bool,
}

fn manifest_text(mod_id: &str, run_id: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Mod id="{mod_id}" version="1">
  <Properties><Name>JAC Hotseat Test Witness {run_id}</Name><Description>Test-only log witness</Description><Teaser>Hotseat test only</Teaser><Authors>JAC</Authors></Properties>
  <InGameActions>
    <AddUserInterfaces id="JAC_HotseatWitness_UI"><Properties><Context>InGame</Context></Properties><File>UI/JAC_HotseatWitness.xml</File></AddUserInterfaces>
    <ImportFiles id="JAC_HotseatWitness_Files"><File>UI/JAC_HotseatWitness.xml</File><File>UI/JAC_HotseatWitness.lua</File><File>UI/JAC_HotseatPlan.lua</File></ImportFiles>
  </InGameActions>
  <Files><File>UI/JAC_HotseatWitness.xml</File><File>UI/JAC_HotseatWitness.lua</File><File>UI/JAC_HotseatPlan.lua</File></Files>
</Mod>
"#
    )
}

/// Build an installable, per-run MOD beside a scenario copy; never touch the user's Mods folder.
pub fn prepare(data: &Value, output: &Path, run_id: &str, mod_id: &str) -> Result<Bundle> {
    prepare_mode(data, output, run_id, mod_id, false)
}

pub fn prepare_mode(
    data: &Value,
    output: &Path,
    run_id: &str,
    mod_id: &str,
    input_diagnostic: bool,
) -> Result<Bundle> {
    check_scenario(data)?;
    if !nonce_valid(run_id) || uuid::Uuid::parse_str(mod_id).is_err() {
        return Err(Error::scenario("invalid hotseat nonce or MOD UUID"));
    }
    if output.exists() && output.read_dir()?.next().is_some() {
        return Err(Error::scenario("hotseat bundle destination must be empty"));
    }
    fs::create_dir_all(output)?;
    let output = output.canonicalize()?;
    let mod_folder = output.join("JAC_Test_HotseatBridge");
    fs::create_dir_all(mod_folder.join("UI"))?;
    fs::write(
        mod_folder.join("JAC_Test_HotseatBridge.modinfo"),
        manifest_text(mod_id, run_id),
    )?;
    fs::write(mod_folder.join("UI/JAC_HotseatWitness.xml"), XML)?;
    fs::write(mod_folder.join("UI/JAC_HotseatWitness.lua"), WITNESS)?;
    fs::write(
        mod_folder.join("UI/JAC_HotseatPlan.lua"),
        plan(data, run_id, input_diagnostic),
    )?;
    let mut scenario = data.clone();
    scenario["steps"][0]["ids"]
        .as_array_mut()
        .unwrap()
        .push(mod_id.into());
    scenario["hotseat_bridge_nonce"] = run_id.into();
    scenario::validate_scenario(&scenario)?;
    let scenario_path = output.join("multiplayer-hotseat-growth.bridge.json");
    fs::write(&scenario_path, serde_json::to_vec_pretty(&scenario)?)?;
    let bundle = Bundle {
        nonce: run_id.into(),
        mod_id: mod_id.into(),
        mod_folder: mod_folder.display().to_string(),
        scenario: scenario_path.display().to_string(),
        input_diagnostic,
    };
    fs::write(
        output.join("bundle.json"),
        serde_json::to_vec_pretty(&bundle)?,
    )?;
    Ok(bundle)
}

pub fn verify_installed(bundle: &Bundle, data: &Value, mods: &Path) -> Result<()> {
    check_scenario(data)?;
    if data["hotseat_bridge_nonce"] != bundle.nonce
        || data["steps"][0]["ids"]
            .as_array()
            .and_then(|ids| ids.last())
            .and_then(Value::as_str)
            != Some(bundle.mod_id.as_str())
    {
        return Err(Error::scenario("installed hotseat bundle identity differs"));
    }
    // Per-run witnesses can coexist with another task's installed witness.
    // Identify our exact MOD ID rather than assuming ownership of a folder name.
    let mut candidates = Vec::new();
    for entry in fs::read_dir(mods)? {
        let path = entry?.path();
        let manifest = path.join("JAC_Test_HotseatBridge.modinfo");
        if !path.is_dir() || !manifest.is_file() {
            continue;
        }
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        let Ok(doc) = roxmltree::Document::parse(text.trim_start_matches('\u{feff}')) else {
            continue;
        };
        if doc
            .root_element()
            .attribute("id")
            .is_some_and(|id| id.eq_ignore_ascii_case(&bundle.mod_id))
        {
            candidates.push(path);
        }
    }
    if candidates.len() != 1 {
        return Err(Error::scenario(
            "Exactly one installed witness with this run's MOD ID is required",
        ));
    }
    let root = &candidates[0];
    for (relative, expected) in [
        (
            "JAC_Test_HotseatBridge.modinfo",
            manifest_text(&bundle.mod_id, &bundle.nonce),
        ),
        ("UI/JAC_HotseatWitness.xml", XML.to_owned()),
        ("UI/JAC_HotseatWitness.lua", WITNESS.to_owned()),
        (
            "UI/JAC_HotseatPlan.lua",
            plan(data, &bundle.nonce, bundle.input_diagnostic),
        ),
    ] {
        if fs::read(root.join(relative))? != expected.as_bytes() {
            return Err(Error::scenario(format!(
                "installed hotseat file differs from the checked source: {relative}"
            )));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub nonce: String,
    pub scene: String,
    pub serial: usize,
    pub step: usize,
    pub kind: String,
    pub body: String,
}

#[derive(Clone, Debug)]
struct Chunk {
    nonce: String,
    scene: String,
    serial: usize,
    step: usize,
    kind: String,
    part: usize,
    total: usize,
    hex: String,
}

fn parse_chunk(line: &str) -> Result<Option<Chunk>> {
    let Some(mark) = line.find("JAC_HS|") else {
        return Ok(None);
    };
    let fields: Vec<_> = line[mark..].trim().split('|').collect();
    if fields.len() != 9 || fields[0] != "JAC_HS" || fields[1] != "1" {
        return Err(Error::scenario("malformed hotseat log chunk"));
    }
    let parts: Vec<_> = fields[7].split('/').collect();
    if parts.len() != 2 {
        return Err(Error::scenario("malformed hotseat chunk index"));
    }
    let chunk = Chunk {
        nonce: fields[2].into(),
        scene: fields[3].into(),
        serial: fields[4]
            .parse()
            .map_err(|_| Error::scenario("invalid hotseat serial"))?,
        step: fields[5]
            .parse()
            .map_err(|_| Error::scenario("invalid hotseat step"))?,
        kind: fields[6].into(),
        part: parts[0]
            .parse()
            .map_err(|_| Error::scenario("invalid hotseat part"))?,
        total: parts[1]
            .parse()
            .map_err(|_| Error::scenario("invalid hotseat total"))?,
        hex: fields[8].into(),
    };
    if !nonce_valid(&chunk.nonce)
        || chunk.scene.is_empty()
        || chunk.scene.len() > 128
        || !chunk
            .scene
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.')
        || chunk.serial == 0
        || chunk.step > 21
        || chunk.kind.is_empty()
        || !chunk
            .kind
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c == b'_')
        || chunk.total == 0
        || chunk.total > 1024
        || chunk.part == 0
        || chunk.part > chunk.total
        || chunk.hex.len() > 192
        || !chunk.hex.len().is_multiple_of(2)
        || !chunk.hex.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err(Error::scenario("invalid hotseat chunk fields"));
    }
    Ok(Some(chunk))
}

/// Reject interleaved, missing, duplicate, and out-of-order chunks. A stale nonce is an error.
pub fn parse_frames(log: &str, run_id: &str) -> Result<Vec<Frame>> {
    parse_frames_inner(log, run_id, false)
}

/// Log writes are not atomic as a whole frame. Polling may observe the final
/// line or frame before the witness has finished printing its remaining parts.
pub fn parse_available_frames(log: &str, run_id: &str) -> Result<Vec<Frame>> {
    parse_frames_inner(log, run_id, true)
}

fn parse_frames_inner(log: &str, run_id: &str, allow_incomplete_tail: bool) -> Result<Vec<Frame>> {
    if !nonce_valid(run_id) {
        return Err(Error::scenario("invalid expected nonce"));
    }
    let mut frames = Vec::new();
    let mut last_serial = None;
    let mut current: Option<Chunk> = None;
    let mut hex = String::new();
    let lines: Vec<_> = log.split_inclusive('\n').collect();
    for (index, line) in lines.iter().enumerate() {
        if allow_incomplete_tail && index + 1 == lines.len() && !line.ends_with('\n') {
            break;
        }
        let chunk = parse_chunk(line)?;
        let Some(chunk) = chunk else {
            continue;
        };
        if chunk.nonce != run_id {
            return Err(Error::scenario("hotseat nonce mismatch"));
        }
        if let Some(first) = &current {
            if chunk.part != first.part + 1
                || chunk.total != first.total
                || chunk.scene != first.scene
                || chunk.serial != first.serial
                || chunk.step != first.step
                || chunk.kind != first.kind
            {
                return Err(Error::scenario(
                    "missing, duplicate or out-of-order hotseat chunk",
                ));
            }
        } else if chunk.part != 1 {
            return Err(Error::scenario("hotseat chunk does not begin at one"));
        }
        hex.push_str(&chunk.hex);
        if chunk.part == chunk.total {
            let bytes = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16)
                        .map_err(|_| Error::scenario("invalid hotseat hex"))
                })
                .collect::<Result<Vec<_>>>()?;
            let body =
                String::from_utf8(bytes).map_err(|_| Error::scenario("invalid hotseat UTF-8"))?;
            if last_serial.is_some_and(|previous| chunk.serial != previous + 1) {
                return Err(Error::scenario(
                    "duplicate or out-of-order hotseat frame serial",
                ));
            }
            last_serial = Some(chunk.serial);
            frames.push(Frame {
                nonce: chunk.nonce,
                scene: chunk.scene,
                serial: chunk.serial,
                step: chunk.step,
                kind: chunk.kind,
                body,
            });
            current = None;
            hex.clear();
        } else {
            current = Some(chunk)
        }
    }
    if current.is_some() && !allow_incomplete_tail {
        return Err(Error::scenario("incomplete hotseat log frame"));
    }
    Ok(frames)
}

/// Unlike the generic judge, rotation is an error: old bytes cannot prove a new ACK.
pub fn fresh_frames(log_dir: &Path, baseline: &Value, run_id: &str) -> Result<Vec<Frame>> {
    let path = log_dir.join("Lua.log");
    let (bytes, inode) = judge::read_log_file(&path)?;
    let marker = &baseline["files"]["Lua.log"];
    if !marker.is_object() || judge::log_marker_replaced(&bytes, marker, inode) {
        return Err(Error::scenario(
            "Lua.log generation changed after hotseat baseline",
        ));
    }
    parse_available_frames(&judge::slice_log_bytes(&bytes, marker, inode), run_id)
}

/// The witness's fixed-length values preserve nil, types, and all four complete growth records.
pub fn decode_fields(body: &str) -> Result<Vec<String>> {
    let mut rest = body.as_bytes();
    let mut values = Vec::new();
    while !rest.is_empty() {
        let split = rest
            .iter()
            .position(|&byte| byte == b':')
            .ok_or_else(|| Error::scenario("invalid witness field length"))?;
        if split == 0 || split > 6 || !rest[..split].iter().all(u8::is_ascii_digit) {
            return Err(Error::scenario("invalid witness field length"));
        }
        let len: usize = std::str::from_utf8(&rest[..split])
            .unwrap()
            .parse()
            .map_err(|_| Error::scenario("invalid witness field length"))?;
        rest = &rest[split + 1..];
        if len > rest.len() {
            return Err(Error::scenario("truncated witness field"));
        }
        let value = String::from_utf8(rest[..len].to_vec())
            .map_err(|_| Error::scenario("invalid witness field UTF-8"))?;
        values.push(value);
        rest = &rest[len..];
    }
    Ok(values)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WitnessSnapshot {
    pub turn: i64,
    pub start_turn: i64,
    pub local: i64,
    pub active: bool,
    pub human: bool,
    pub hotseat: bool,
    pub paused: bool,
    pub dialog: bool,
    pub dialog_ready: bool,
    pub fields: Vec<String>,
}

impl WitnessSnapshot {
    pub fn parse(body: &str) -> Result<Self> {
        let fields = decode_fields(body)?;
        if fields.len() != 49 {
            return Err(Error::scenario(format!(
                "expected 49 witness fields, got {}",
                fields.len()
            )));
        }
        let number = |i: usize| -> Result<i64> {
            fields[i]
                .strip_prefix("number:")
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| Error::scenario("invalid witness number"))
        };
        let boolean = |i: usize| -> Result<bool> {
            match fields[i].as_str() {
                "boolean:true" => Ok(true),
                "boolean:false" => Ok(false),
                _ => Err(Error::scenario("invalid witness boolean")),
            }
        };
        let snapshot = Self {
            turn: number(0)?,
            start_turn: number(1)?,
            local: number(2)?,
            active: boolean(3)?,
            human: boolean(4)?,
            hotseat: boolean(5)?,
            paused: boolean(6)?,
            dialog: boolean(7)?,
            dialog_ready: boolean(8)?,
            fields,
        };
        if !snapshot.hotseat || !(0..=1).contains(&snapshot.local) || !snapshot.human {
            return Err(Error::scenario("witness is not a two-human hotseat state"));
        }
        if snapshot.dialog_ready && (!snapshot.dialog || !snapshot.paused || !snapshot.active) {
            return Err(Error::scenario("PlayerChange ready state is inconsistent"));
        }
        for pid in 0..2 {
            if snapshot.fields[9 + pid * 20] != "boolean:true" {
                return Err(Error::scenario("witness player is not human"));
            }
            for unit in 0..2 {
                let base = 11 + pid * 20 + unit * 9;
                if snapshot.fields[base] != format!("number:{pid}") {
                    return Err(Error::scenario("growth unit owner mismatch"));
                }
                for index in [1, 2, 3, 4, 5, 6, 7, 8] {
                    if snapshot.fields[base + index] == "nil" && index != 7 && index != 8 {
                        return Err(Error::scenario("missing required growth field"));
                    }
                }
            }
        }
        Ok(snapshot)
    }

    pub fn growth_key(&self) -> Vec<&str> {
        let mut key = vec![self.fields[0].as_str(), self.fields[2].as_str()];
        for pid in 0..2 {
            for unit in 0..2 {
                let base = 11 + pid * 20 + unit * 9;
                key.extend(self.fields[base..base + 9].iter().map(String::as_str));
            }
        }
        key
    }
}

pub fn matching_capture(
    captures: &BTreeMap<String, (String, WitnessSnapshot)>,
    name: &str,
    value: &str,
    snapshot: &WitnessSnapshot,
) -> Result<()> {
    let (expected_value, expected_snapshot) = captures
        .get(name)
        .ok_or_else(|| Error::scenario("missing hotseat capture"))?;
    if value != expected_value || snapshot.growth_key() != expected_snapshot.growth_key() {
        return Err(Error::scenario(format!(
            "hotseat full growth snapshot differs after load: {name}"
        )));
    }
    Ok(())
}

pub fn validate_load_epoch(
    saved: &WitnessSnapshot,
    boot: &Frame,
    expected_nonce: &str,
    prior_boots: usize,
    observed_boots: usize,
) -> Result<WitnessSnapshot> {
    if boot.nonce != expected_nonce
        || boot.kind != "BOOT"
        || boot.step != 0
        || observed_boots != prior_boots + 1
    {
        return Err(Error::scenario(
            "load did not produce exactly one fresh nonce-matched InGame scene",
        ));
    }
    let loaded = WitnessSnapshot::parse(&boot.body)?;
    if loaded.growth_key() != saved.growth_key() {
        return Err(Error::scenario(
            "load scene differs from full four-unit save snapshot",
        ));
    }
    Ok(loaded)
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowOwner {
    pub pid: u32,
    pub started: f64,
    pub hwnd: isize,
}

pub trait TransportIo {
    fn owner(&mut self) -> Result<WindowOwner>;
    fn baseline(&mut self) -> Result<Value>;
    fn frames(&mut self, baseline: &Value, run_id: &str) -> Result<Vec<Frame>>;
    fn key(&mut self, vk: u16, deadline: f64) -> Result<()>;
    fn now(&mut self) -> f64;
    fn sleep(&mut self, seconds: f64);
}

pub struct SystemTransport<'a> {
    pub logs: &'a Path,
}

impl TransportIo for SystemTransport<'_> {
    fn owner(&mut self) -> Result<WindowOwner> {
        let (pid, started) = crate::restart::single_civ_process(0.0)?
            .ok_or_else(|| Error::input("Civ6 process is missing"))?;
        let hwnd = crate::click::find_civ_window()?;
        Ok(WindowOwner { pid, started, hwnd })
    }
    fn baseline(&mut self) -> Result<Value> {
        judge::snapshot_logs(self.logs)
    }
    fn frames(&mut self, baseline: &Value, run_id: &str) -> Result<Vec<Frame>> {
        fresh_frames(self.logs, baseline, run_id)
    }
    fn key(&mut self, vk: u16, deadline: f64) -> Result<()> {
        crate::click::press_key(vk, Some(deadline))
    }
    fn now(&mut self) -> f64 {
        crate::time::monotonic()
    }
    fn sleep(&mut self, seconds: f64) {
        std::thread::sleep(std::time::Duration::from_secs_f64(seconds))
    }
}

#[derive(Debug)]
pub struct StepReceipt {
    pub step: usize,
    pub kind: String,
    pub response_body: String,
    pub value: Option<String>,
    pub before: WitnessSnapshot,
    pub dialog_open: Option<WitnessSnapshot>,
}

pub struct Controller<I> {
    pub io: I,
    pub nonce: String,
    pub scene: String,
    pub owner: WindowOwner,
    pub sent: BTreeSet<usize>,
    pub return_sent: BTreeSet<usize>,
    pub captures: BTreeMap<String, (String, WitnessSnapshot)>,
}

impl<I: TransportIo> Controller<I> {
    pub fn new(mut io: I, run_id: &str, boot: &Frame) -> Result<Self> {
        if !nonce_valid(run_id) || boot.nonce != run_id || boot.step != 0 || boot.kind != "BOOT" {
            return Err(Error::scenario("fresh hotseat BOOT required"));
        }
        WitnessSnapshot::parse(&boot.body)?;
        let owner = io.owner()?;
        Ok(Self {
            io,
            nonce: run_id.into(),
            scene: boot.scene.clone(),
            owner,
            sent: BTreeSet::new(),
            return_sent: BTreeSet::new(),
            captures: BTreeMap::new(),
        })
    }

    pub fn verify_owner(&mut self) -> Result<()> {
        if self.io.owner()? != self.owner {
            return Err(Error::input(
                "hotseat PID, creation time, or window ownership changed",
            ));
        }
        Ok(())
    }

    fn action(step: usize) -> Result<(&'static str, u16)> {
        match step {
            4 | 5 | 7 | 8 | 12 | 14 | 15 | 17 | 21 => {
                let index = QUERY_STEPS.iter().position(|&v| v == step).unwrap();
                Ok(("ui_lua", 0x7c + index as u16))
            }
            6 | 13 | 16 => Ok(("end_turn", KEY_END_TURN)),
            9 | 18 => Ok(("save", KEY_SAVE)),
            10 | 19 => Ok(("exit_to_menu", KEY_EXIT)),
            _ => Err(Error::scenario(format!(
                "hotseat bridge cannot send step {step}; setup/load needs a separately verified route"
            ))),
        }
    }

    /// Record the step as spent before input. A missing ACK is unknown and is never resent.
    pub fn step(
        &mut self,
        step: usize,
        scenario: &Value,
        deadline: f64,
        mut mark_spent: impl FnMut(usize) -> Result<()>,
    ) -> Result<StepReceipt> {
        check_scenario(scenario)?;
        let (action, vk) = Self::action(step)?;
        if self.sent.contains(&step) {
            return Err(Error::scenario("hotseat step already sent"));
        }
        self.verify_owner()?;
        let baseline = self.io.baseline()?;
        mark_spent(step)?;
        self.sent.insert(step);
        self.verify_owner()?;
        self.io.key(vk, deadline).map_err(|e| {
            Error::unknown(format!(
                "hotseat step outcome unknown after key attempt: {e}"
            ))
        })?;
        let mut before = None;
        let mut answer = None;
        let mut dialog_open = None;
        let mut save_returned = false;
        while self.io.now() < deadline {
            self.verify_owner()?;
            for frame in self.io.frames(&baseline, &self.nonce)? {
                if frame.scene != self.scene {
                    return Err(Error::scenario("old or foreign hotseat scene"));
                }
                if frame.kind == "WITNESS_ERROR"
                    || frame.kind == "DUPLICATE"
                    || frame.kind == "ERROR"
                {
                    return Err(Error::scenario(format!(
                        "hotseat witness failed at step {}: {}",
                        frame.step, frame.body
                    )));
                }
                if frame.step == step {
                    match frame.kind.as_str() {
                        "BEFORE" => before = Some(WitnessSnapshot::parse(&frame.body)?),
                        "SAVE_RETURNED" => save_returned = true,
                        "RESULT" | "REQUESTED" | "SAVE_COMPLETE"
                            if frame.kind != "REQUESTED"
                                || answer
                                    .as_ref()
                                    .is_none_or(|v: &(String, String)| v.0 != "SAVE_COMPLETE") =>
                        {
                            answer = Some((frame.kind.clone(), frame.body.clone()))
                        }
                        _ => {}
                    }
                } else if frame.step != 0 {
                    return Err(Error::scenario("hotseat ACK belongs to another step"));
                }
                if frame.kind == "DIALOG_OPEN" {
                    let state = WitnessSnapshot::parse(&frame.body)?;
                    if !state.dialog || !state.dialog_ready || !state.paused {
                        return Err(Error::scenario("PlayerChange was not visibly paused"));
                    }
                    dialog_open = Some(state);
                } else if frame.kind == "DIALOG_WAIT" || frame.kind == "DIALOG_CLOSED" {
                    dialog_open = None;
                }
            }
            let complete = match action {
                "end_turn" => {
                    answer.as_ref().is_some_and(|v| v.0 == "REQUESTED") && dialog_open.is_some()
                }
                "save" => save_returned && answer.as_ref().is_some_and(|v| v.0 == "SAVE_COMPLETE"),
                "ui_lua" => answer.as_ref().is_some_and(|v| v.0 == "RESULT"),
                _ => answer.as_ref().is_some_and(|v| v.0 == "REQUESTED"),
            };
            if let (Some(before), Some((kind, body))) = (before.clone(), answer.clone()) {
                if complete {
                    let value = if action == "ui_lua" {
                        Some(
                            body.strip_prefix("string:")
                                .ok_or_else(|| Error::scenario("ui_lua did not return a string"))?
                                .to_string(),
                        )
                    } else {
                        None
                    };
                    if let Some(value) = &value {
                        let source_step = &scenario["steps"][step - 1];
                        if let Some(name) = source_step["equals_capture"].as_str() {
                            matching_capture(&self.captures, name, value, &before)?;
                        }
                        if let Some(name) = source_step["capture_as"].as_str() {
                            self.captures
                                .insert(name.into(), (value.clone(), before.clone()));
                        }
                    }
                    return Ok(StepReceipt {
                        step,
                        kind,
                        response_body: body,
                        value,
                        before,
                        dialog_open,
                    });
                }
            }
            self.io.sleep(0.1);
        }
        Err(Error::unknown(format!(
            "hotseat step {step} outcome unknown; input will not be resent"
        )))
    }

    /// Only a witnessed, visible PlayerChange dialog can receive one official Return.
    pub fn accept_turn(
        &mut self,
        after_step: usize,
        opened: &WitnessSnapshot,
        deadline: f64,
        mut mark_spent: impl FnMut(usize) -> Result<()>,
    ) -> Result<WitnessSnapshot> {
        if !opened.dialog
            || !opened.dialog_ready
            || !opened.paused
            || self.return_sent.contains(&after_step)
        {
            return Err(Error::scenario(
                "PlayerChange Return requires a fresh visible paused dialog",
            ));
        }
        self.verify_owner()?;
        let baseline = self.io.baseline()?;
        mark_spent(after_step)?;
        self.return_sent.insert(after_step);
        self.verify_owner()?;
        self.io
            .key(0x0d, deadline)
            .map_err(|e| Error::unknown(format!("PlayerChange Return outcome unknown: {e}")))?;
        let mut saw_closed = false;
        while self.io.now() < deadline {
            self.verify_owner()?;
            for frame in self.io.frames(&baseline, &self.nonce)? {
                if frame.scene != self.scene {
                    return Err(Error::scenario("PlayerChange scene changed"));
                }
                if frame.kind == "DIALOG_CLOSED" || frame.kind == "TURN_READY" {
                    let closed = WitnessSnapshot::parse(&frame.body)?;
                    if frame.step != 0
                        || closed.dialog
                        || closed.dialog_ready
                        || !closed.human
                        || (closed.turn, closed.local) != (opened.turn, opened.local)
                    {
                        return Err(Error::scenario(
                            "PlayerChange close belongs to another turn or dialog",
                        ));
                    }
                    if frame.kind == "DIALOG_CLOSED" {
                        saw_closed = true;
                    } else if !saw_closed {
                        return Err(Error::scenario("turn ready before witnessed dialog close"));
                    }
                    // The official UI broadcasts the pause change separately
                    // from closing its popup. Only a fresh ready state completes it.
                    if !closed.paused && closed.active {
                        return Ok(closed);
                    }
                }
                if frame.kind == "ERROR" || frame.kind == "WITNESS_ERROR" {
                    return Err(Error::scenario(format!(
                        "PlayerChange witness error: {}",
                        frame.body
                    )));
                }
            }
            self.io.sleep(0.1);
        }
        Err(Error::unknown(
            "PlayerChange Return outcome unknown; input will not be resent",
        ))
    }
}
