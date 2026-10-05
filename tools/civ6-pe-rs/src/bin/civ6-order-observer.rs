use civ6_native_probe_rs::order_capture::{disk_sites, Request};
use sha2::{Digest, Sha256};
use std::io::Write;
fn output(value: &str) -> Result<(), String> {
    writeln!(std::io::stdout(), "{value}")
        .and_then(|_| std::io::stdout().flush())
        .map_err(|e| e.to_string())
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: civ6-order-observer snapshot|summary <EXE> | validate|observe|parent <request.json>".into());
    }
    if args[0] == "snapshot" || args[0] == "summary" {
        let bytes = std::fs::read(&args[1]).map_err(|e| e.to_string())?;
        let mut proof = disk_sites(&bytes)?;
        let mut git = std::process::Command::new("git");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            git.creation_flags(0x08000000);
        }
        let head = git
            .args(["-C", env!("CARGO_MANIFEST_DIR"), "rev-parse", "HEAD"])
            .output()
            .map_err(|e| e.to_string())?;
        if !head.status.success() {
            return Err("cannot identify source checkout HEAD".into());
        }
        proof["checkout_head_at_snapshot"] =
            serde_json::json!(String::from_utf8_lossy(&head.stdout).trim());
        proof["checkout_path"] = serde_json::json!(env!("CARGO_MANIFEST_DIR"));
        let tools = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("source root missing")?;
        for row in proof["source"]["source_files"]
            .as_array()
            .ok_or("source list missing")?
        {
            let path = tools.join(row["path"].as_str().ok_or("source path missing")?);
            let actual = std::fs::read(path).map_err(|e| e.to_string())?;
            if format!("{:x}", Sha256::digest(actual)) != row["sha256"].as_str().unwrap_or("") {
                return Err("checkout source differs from compiled observer".into());
            }
        }
        proof["checkout_sources_match_binary"] = serde_json::json!(true);
        let core = std::path::Path::new(civ6_native_probe_rs::GAME_ROOT)
            .join("DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll");
        let actual = format!(
            "{:x}",
            Sha256::digest(std::fs::read(core).map_err(|e| e.to_string())?)
        );
        if actual != civ6_native_probe_rs::order_capture::CORE_SHA {
            return Err("GameCore SHA mismatch".into());
        }
        proof["gamecore_identity_verified"] = serde_json::json!(true);
        if args[0] == "summary" {
            let summary = serde_json::json!({"schema":proof["schema"],"checkout_head":proof["checkout_head_at_snapshot"],"compiled_source_sha256":proof["source"]["compiled_source_sha256"],"static_site_count":proof["static_sites"].as_array().unwrap().len(),"source_file_count":proof["source"]["source_files"].as_array().unwrap().len(),"sources_match_binary":true,"gamecore_identity_verified":true,"calibration_cap":4,"runtime_hit_count":null,"common_boundary_proof":"unknown","live_runs":0,"memory_writes":0,"dll_install_calls":0,"old_prefix_clones":0});
            output(&serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?)?;
            return Ok(());
        }
        output(&serde_json::to_string_pretty(&proof).map_err(|e| e.to_string())?)?;
        return Ok(());
    }
    if args[0] != "observe" && args[0] != "parent" && args[0] != "validate" {
        return Err("unknown command".into());
    }
    let request: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    Request::parse(&request)?;
    if args[0] == "validate" {
        output(&serde_json::json!({"schema":"todo880-order-v1","manifest_valid":true,"process_identity_verified":false,"live_runs":0}).to_string())?;
        return Ok(());
    }
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        if args[0] == "parent" {
            civ6_native_probe_rs::windows::order_capture::parent(request)
        } else {
            civ6_native_probe_rs::windows::order_capture::observe(Request::parse(&request)?)
        }
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = request;
        Err("observation requires Windows x64".into())
    }
}
fn main() {
    if let Err(e) = run() {
        let _ = writeln!(std::io::stderr(), "{e}");
        std::process::exit(1);
    }
}
