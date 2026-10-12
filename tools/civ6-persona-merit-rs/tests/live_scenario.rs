use std::{fs, process::Command};

#[test]
fn generated_live_acceptance_loads_the_exact_two_checkpoints_and_compiles_lua51() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("qualified.json");
    assert!(Command::new(env!("CARGO_BIN_EXE_prepare-merit-live"))
        .arg(&path)
        .status()
        .unwrap()
        .success());
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let steps = value["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 18);
    let saves: Vec<_> = steps.iter().filter(|s| s["action"] == "save").collect();
    let loads: Vec<_> = steps.iter().filter(|s| s["action"] == "load").collect();
    assert_eq!(saves.len(), 2);
    assert_eq!(loads.len(), 3);
    assert_eq!(saves[0]["name"], "JAC-Todo181-Qualified");
    assert_eq!(saves[1]["name"], "JAC-Todo181-Settled");
    for (save, load) in saves.iter().zip(loads.iter().skip(1)) {
        assert_eq!(save["name"], load["name"]);
        for step in [*save, *load] {
            assert_eq!(step["checkpoint"], true);
            assert_eq!(step["exact_name"], true);
        }
    }
    assert_eq!(loads[0]["exact_name"], true);
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["equals_capture"] == "TODO181_SETTLED")
            .count(),
        3
    );
    let lua = mlua::Lua::new();
    assert_eq!(
        lua.load("return _VERSION").eval::<String>().unwrap(),
        "Lua 5.1"
    );
    let mut chunks = 0;
    for step in steps {
        let code = step["code"]
            .as_str()
            .or_else(|| step["condition"]["code"].as_str());
        if let Some(code) = code {
            lua.load(code).into_function().unwrap();
            chunks += 1;
        }
    }
    assert_eq!(chunks, 10);
}

#[test]
fn production_adapter_is_exactly_the_rust_generated_source() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua");
    let source = fs::read_to_string(path).unwrap();
    let block = source
        .split_once("-- BEGIN RUST PERSONA MERIT ADAPTER\n")
        .unwrap()
        .1;
    let emitted = block
        .split_once("-- END RUST PERSONA MERIT ADAPTER\n")
        .unwrap()
        .0;
    assert_eq!(emitted, civ6_persona_merit_rs::adapter());
    assert_eq!(source.matches("MeritQueue = MeritQueue").count(), 1);
    mlua::Lua::new().load(source).into_function().unwrap();
}
