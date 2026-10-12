//! Portable real-CLI checks. Installed PE validation is the explicit snapshot
//! command documented in todo880-bootstrap-primary.md, never a Cargo test.
mod order_capture {
    use civ6_native_probe_rs::order_capture::disk_sites;
    use serde_json::{json, Value};
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        process::{Command, Output},
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct OwnedTemp(PathBuf);
    impl OwnedTemp {
        fn new() -> Self {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "todo880-order-cli-{}-{id}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            file.write_all(bytes).unwrap();
            path
        }
        fn finish(self) {
            fs::remove_dir_all(&self.0).unwrap();
            assert!(!self.0.exists());
        }
    }
    impl Drop for OwnedTemp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn command() -> Command {
        let c = Command::new(env!("CARGO_BIN_EXE_civ6-order-observer"));
        #[cfg(windows)]
        let c = {
            use std::os::windows::process::CommandExt;
            let mut c = c;
            c.creation_flags(0x08000000);
            c
        };
        c
    }
    fn reject(output: Output, error: &str) {
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(
            !output.stderr.is_empty(),
            "rejection must include a diagnostic"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(error),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn manifest(dir: &Path) -> Value {
        let mut value: Value =
            serde_json::from_str(include_str!("fixtures/order-request-no-live.json")).unwrap();
        value["source_dll"] = json!(dir.join("absent.dll"));
        value["output"] = json!(dir.join("must-not-create.jsonl"));
        value
    }
    #[test]
    fn real_cli_invalid_pe_table_matches_byte_validator() {
        let temp = OwnedTemp::new();
        for (index, bytes) in [b"".as_slice(), b"not a PE", b"MZ\0\0"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(disk_sites(bytes).unwrap_err(), "EXE SHA mismatch");
            let path = temp.write(&format!("invalid-{index}.exe"), bytes);
            for mode in ["snapshot", "summary"] {
                reject(
                    command().arg(mode).arg(&path).output().unwrap(),
                    "EXE SHA mismatch",
                );
                assert_eq!(fs::read(&path).unwrap(), bytes);
            }
        }
        temp.finish();
    }
    #[test]
    fn real_cli_structure_validation_does_not_claim_identity() {
        let temp = OwnedTemp::new();
        let value = manifest(&temp.0);
        let path = temp.write("request.json", &serde_json::to_vec(&value).unwrap());
        let output = command().arg("validate").arg(path).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            json!({"schema":"todo880-order-v1","manifest_valid":true,"process_identity_verified":false,"live_runs":0})
        );
        assert!(!temp.0.join("absent.dll").exists());
        assert!(!temp.0.join("must-not-create.jsonl").exists());
        temp.finish();
    }
    #[test]
    fn real_cli_rejects_invalid_manifest_and_commands() {
        let temp = OwnedTemp::new();
        for (index, key, value, error) in [
            (
                0,
                "extra",
                json!(true),
                "exact order-v1 request fields required",
            ),
            (1, "pid", json!(0), "positive integer pid required"),
            (
                2,
                "max_seconds",
                json!(301),
                "invalid capture limits or identity",
            ),
            (
                3,
                "phase",
                json!("unknown"),
                "phase must be front, load, native-outer or native-shutdown",
            ),
        ] {
            let mut request = manifest(&temp.0);
            request[key] = value;
            let path = temp.write(
                &format!("invalid-{index}.json"),
                &serde_json::to_vec(&request).unwrap(),
            );
            reject(
                command().arg("validate").arg(&path).output().unwrap(),
                error,
            );
        }
        let missing = temp.0.join("no-input.json");
        reject(
            command()
                .arg("unknown-command")
                .arg(&missing)
                .output()
                .unwrap(),
            "unknown command",
        );
        reject(command().output().unwrap(), "usage:");
        reject(
            command().arg("validate").arg(&missing).output().unwrap(),
            "",
        );
        assert!(!temp.0.join("must-not-create.jsonl").exists());
        temp.finish();
    }
}
