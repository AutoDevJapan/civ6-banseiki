//! Real CLI/disk checks only. Never invokes parent/observe or any game process.
#[cfg(windows)]
mod order_capture {
    use serde_json::Value;
    use std::process::Command;
    fn command() -> Command {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new(env!("CARGO_BIN_EXE_civ6-order-observer"));
        c.creation_flags(0x08000000);
        c
    }
    #[test]
    fn real_cli_pinned_disk_snapshot_preserves_unknown_live_evidence() {
        let output = command()
            .args(["snapshot", civ6_native_probe_rs::PINNED_DX11_EXE])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["static_sites"].as_array().unwrap().len(), 8);
        assert_eq!(value["checkout_sources_match_binary"], true);
        assert_eq!(value["gamecore_identity_verified"], true);
        assert_eq!(value["live_runs"], 0);
        assert_eq!(value["common_boundary_proof"], "unknown");
        for site in value["static_sites"].as_array().unwrap() {
            assert!(site["runtime_hits"].is_null());
            assert!(site["original_instruction_executions"].is_null());
        }
    }
    #[test]
    fn real_cli_structure_validation_does_not_claim_identity() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../civ6-pe-rs/tests/fixtures/order-request-no-live.json");
        let output = command().arg("validate").arg(fixture).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"schema":"todo880-order-v1","manifest_valid":true,"process_identity_verified":false,"live_runs":0})
        );
    }
    #[test]
    fn real_cli_wrong_pe_is_refused_without_observation() {
        let core = std::path::Path::new(civ6_native_probe_rs::GAME_ROOT)
            .join("DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll");
        let output = command().arg("snapshot").arg(core).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("EXE SHA mismatch"));
        assert!(output.stdout.is_empty());
    }
}
