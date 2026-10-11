//! Preserve bytes, not a claim that the configured renderer was selected.
use crate::{Paths, Runner};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) struct Run {
    artifacts: PathBuf,
    started: SystemTime,
}

fn stamp(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339()
}

fn create_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("create {}: {e}", path.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("save {}: {e}", path.display()))
}

fn log_path(p: &Paths) -> PathBuf {
    // Match the driver's LOCALAPPDATA for the real profile, without reading
    // the owner's log when an injected fixture uses a different profile.
    let local = if std::env::var_os("USERPROFILE").map(PathBuf::from).as_ref() == Some(&p.profile) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| p.profile.join("AppData/Local"))
    } else {
        p.profile.join("AppData/Local")
    };
    local.join("Firaxis Games/Sid Meier's Civilization VI/Logs/Renderer.log")
}

/// Read identities only from the existing native driver's records. Never
/// discover a PID and retroactively call it the game that produced this run.
fn receipt_game(
    artifacts: &Path,
    owner: &Value,
    evidence: &mut Value,
) -> Result<Option<(u32, u64)>, String> {
    let mut game = None;
    let mut chain_bound = false;
    let mut files = fs::read_dir(artifacts)
        .map_err(|e| e.to_string())?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    files.sort();
    for path in files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let (pid_key, created_key, request) = if name.ends_with(".driver-request.json") {
            ("pid", "created_filetime", true)
        } else if name.ends_with(".driver.json") {
            ("game_pid", "created_filetime", false)
        } else if name.ends_with(".parent.json") {
            ("game_pid", "game_created_filetime", false)
        } else {
            continue;
        };
        let row: Value = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("invalid game receipt {}: {e}", path.display()))?;
        if name.ends_with(".parent.json") && row["type"] != "order_parent_receipt" {
            continue;
        }
        let pid = row[pid_key]
            .as_u64()
            .and_then(|pid| u32::try_from(pid).ok())
            .filter(|pid| *pid > 0)
            .ok_or("game receipt lacks PID")?;
        let created = row[created_key]
            .as_u64()
            .filter(|created| *created > 0)
            .ok_or("game receipt lacks creation FILETIME")?;
        evidence["receipts"].as_array_mut().unwrap().push(json!({
            "path": path, "game_pid": pid, "game_created_filetime": created,
            "chain_pid": row.get("chain_pid"),
            "chain_created_filetime": row.get("chain_created_filetime")
        }));
        if game.is_some_and(|expected| expected != (pid, created)) {
            return Err("game receipt generations disagree".into());
        }
        game = Some((pid, created));
        if request {
            if row["chain_pid"] != owner["chain_pid"]
                || row["chain_created_filetime"] != owner["pid_created_filetime"]
                || row["chain_created_filetime"]
                    .as_u64()
                    .is_none_or(|n| n == 0)
            {
                return Err(
                    "driver request belongs to a different or unknown chain generation".into(),
                );
            }
            chain_bound = true;
        }
    }
    if game.is_some() && !chain_bound {
        return Err("run has no driver request bound to this chain generation".into());
    }
    Ok(game)
}

fn filetime(time: SystemTime) -> Option<u64> {
    let ticks = time.duration_since(UNIX_EPOCH).ok()?.as_nanos() / 100;
    u64::try_from(ticks)
        .ok()?
        .checked_add(116_444_736_000_000_000)
}

fn verify_game(r: &mut impl Runner, (pid, created): (u32, u64)) -> Result<(), String> {
    let identity = r
        .process_identity(pid, 2.0)?
        .ok_or("receipt game is no longer observable; log attribution unknown")?;
    if !super::CIV_EXECUTABLES
        .iter()
        .any(|image| identity.image.eq_ignore_ascii_case(image))
        || identity.created_filetime != Some(created)
    {
        return Err("receipt game generation differs from the observed process".into());
    }
    Ok(())
}

impl Run {
    pub(super) fn new(artifacts: &Path) -> Self {
        Self {
            artifacts: artifacts.into(),
            started: SystemTime::now(),
        }
    }

    pub(super) fn capture(
        &self,
        r: &mut impl Runner,
        p: &Paths,
        owner: &Value,
        stage: &str,
    ) -> Result<(), String> {
        let directory = self.artifacts.join("renderer-evidence");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let source = log_path(p);
        let acquired = SystemTime::now();
        let raw = directory.join(format!("{stage}.Renderer.log"));
        let mut evidence = json!({
            "schema_version": 1, "source": source, "run_artifacts": self.artifacts,
            "stage": stage, "run_started_at": stamp(self.started),
            "acquired_at": stamp(acquired), "chain_pid": owner["chain_pid"],
            "chain_created_filetime": owner["pid_created_filetime"],
            "capture_status": "failed", "attribution_status": "unknown",
            "renderer_selection_verified": false, "receipts": []
        });
        let captured = (|| -> Result<(SystemTime, bool), String> {
            let mut file = File::open(&source).map_err(|e| format!("open Renderer.log: {e}"))?;
            let before = file.metadata().map_err(|e| e.to_string())?;
            let modified = before.modified().map_err(|e| e.to_string())?;
            evidence["source_mtime"] = json!(stamp(modified));
            evidence["source_mtime_filetime"] = json!(filetime(modified));
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|e| format!("read Renderer.log: {e}"))?;
            let after = file.metadata().map_err(|e| e.to_string())?;
            // SHA is over exactly the preserved bytes, including invalid UTF-8.
            create_new(&raw, &bytes)?;
            evidence["raw_path"] = json!(raw);
            evidence["sha256"] = json!(sha256(&bytes));
            evidence["byte_length"] = json!(bytes.len());
            evidence["capture_status"] = json!("saved");
            Ok((
                modified,
                before.len() == after.len()
                    && after.len() == bytes.len() as u64
                    && after.modified().ok() == Some(modified),
            ))
        })();
        let attribution = match captured {
            Ok((modified, stable)) => (|| -> Result<(), String> {
                let mut game = receipt_game(&self.artifacts, owner, &mut evidence)?;
                // An existing interrupted-exit receipt must not silently bind
                // a newer game's log to the older game's run.
                match fs::read(p.here.join("steam-exit-recovery.json")) {
                    Ok(bytes) => {
                        let row: Value =
                            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                        evidence["pending_exit_games"] = row["games"].clone();
                        let games: super::GameGeneration =
                            serde_json::from_value(row["games"].clone())
                                .map_err(|e| e.to_string())?;
                        if row["version"] != 2
                            || games.len() != 1
                            || !super::CIV_EXECUTABLES.contains(&games[0].0.as_str())
                            || games[0].1 == 0
                            || games[0].2 == 0
                        {
                            return Err(
                                "pending exit receipt has an unknown game generation".into()
                            );
                        }
                        let pending = (games[0].1, games[0].2);
                        if game.is_some_and(|game| game != pending) {
                            return Err(
                                "pending exit receipt belongs to a different game generation"
                                    .into(),
                            );
                        }
                        game = Some(pending);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("read pending exit receipt: {e}")),
                }
                let game = game.ok_or("no canonical driver/chain game generation receipt")?;
                evidence["receipt_game_pid"] = json!(game.0);
                evidence["receipt_game_created_filetime"] = json!(game.1);
                verify_game(r, game)?;
                if !stable
                    || modified < self.started
                    || modified > SystemTime::now()
                    || filetime(modified).is_none_or(|mtime| mtime < game.1)
                {
                    return Err("Renderer.log changed during capture or its timestamp is outside this game/run".into());
                }
                verify_game(r, game)?;
                Ok(())
            })(),
            Err(error) => {
                evidence["capture_error"] = json!(error);
                Err("raw Renderer.log was not saved".into())
            }
        };
        match attribution {
            Ok(()) => evidence["attribution_status"] = json!("matched_receipt_generation"),
            Err(error) => evidence["attribution_error"] = json!(error),
        }
        // A collision never overwrites another run or a prior capture. Even a
        // missing/unreadable source gets an explicit immutable failure record.
        create_new(
            &directory.join(format!("{stage}.json")),
            &serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
        )?;
        if evidence["capture_status"] == "saved" {
            Ok(())
        } else {
            Err(evidence["capture_error"]
                .as_str()
                .unwrap_or("Renderer capture failed")
                .into())
        }
    }
}

// SHA-256 without a new crate dependency (cc-live's manifest stays unchanged).
fn sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&(bytes.len() as u64).wrapping_mul(8).to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (word, input) in w[..16].iter_mut().zip(chunk.chunks_exact(4)) {
            *word = u32::from_be_bytes(input.try_into().unwrap());
        }
        for i in 16..64 {
            let a = w[i - 15];
            let b = w[i - 2];
            let s0 = a.rotate_right(7) ^ a.rotate_right(18) ^ (a >> 3);
            let s1 = b.rotate_right(17) ^ b.rotate_right(19) ^ (b >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let t1 = hh
                .wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25))
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let t2 = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22))
                .wrapping_add((a & b) ^ (a & c) ^ (b & c));
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (word, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *word = word.wrapping_add(value);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{process::ProcessIdentity, Call, Output};
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Fixture {
        root: PathBuf,
        p: Paths,
        run: Run,
        owner: Value,
    }
    impl Fixture {
        fn new() -> Self {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "renderer-evidence-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            let p = Paths {
                here: root.join("here"),
                profile: root.join("profile"),
                ..Paths::default()
            };
            fs::create_dir_all(&p.here).unwrap();
            let run = Run::new(&root.join("run"));
            fs::create_dir_all(&run.artifacts).unwrap();
            fs::create_dir_all(log_path(&p).parent().unwrap()).unwrap();
            fs::write(
                log_path(&p),
                b"Renderer::CreateD3DDevice()\r\n\xff\0CPU\r\n",
            )
            .unwrap();
            let owner = json!({"chain_pid": 42, "pid_created_filetime": 7});
            Self {
                root,
                p,
                run,
                owner,
            }
        }
        fn receipt(&self, pid: u32, created: u64) {
            fs::write(
                self.run.artifacts.join("capture.driver-request.json"),
                json!({"pid": pid, "created_filetime": created,
                    "chain_pid":42,"chain_created_filetime":7})
                .to_string(),
            )
            .unwrap();
            fs::write(
                self.run.artifacts.join("capture.driver.json"),
                json!({"game_pid":pid,"created_filetime":created}).to_string(),
            )
            .unwrap();
        }
        fn evidence(&self, stage: &str) -> Value {
            serde_json::from_slice(
                &fs::read(
                    self.run
                        .artifacts
                        .join("renderer-evidence")
                        .join(format!("{stage}.json")),
                )
                .unwrap(),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    struct Model {
        created: Option<u64>,
        calls: usize,
        replace_on_second: bool,
    }
    impl Runner for Model {
        fn run(&mut self, _: Call) -> Result<Output, String> {
            panic!("no subprocesses")
        }
        fn now(&mut self) -> f64 {
            0.0
        }
        fn sleep(&mut self, _: f64) -> Result<(), String> {
            panic!("no sleeps")
        }
        fn process_identity(
            &mut self,
            pid: u32,
            timeout: f64,
        ) -> Result<Option<ProcessIdentity>, String> {
            assert_eq!((pid, timeout), (11, 2.0));
            self.calls += 1;
            Ok(self.created.map(|created| ProcessIdentity {
                image: "CivilizationVI.exe".into(),
                created_filetime: Some(
                    created + u64::from(self.replace_on_second && self.calls == 2),
                ),
            }))
        }
    }
    fn model() -> Model {
        Model {
            created: Some(116_444_736_000_000_001),
            calls: 0,
            replace_on_second: false,
        }
    }

    #[test]
    fn raw_bytes_sha_and_metadata_survive_the_next_game_overwrite() {
        let f = Fixture::new();
        let mut r = model();
        f.receipt(11, r.created.unwrap());
        let original = fs::read(log_path(&f.p)).unwrap();
        f.run
            .capture(&mut r, &f.p, &f.owner, "driver-return")
            .unwrap();
        fs::write(log_path(&f.p), b"another game's GPU log").unwrap();
        let saved = f.evidence("driver-return");
        assert_eq!(
            fs::read(saved["raw_path"].as_str().unwrap()).unwrap(),
            original
        );
        assert_eq!(saved["sha256"], sha256(&original));
        assert_eq!(saved["byte_length"], original.len());
        assert_eq!(saved["attribution_status"], "matched_receipt_generation");
        assert_eq!(saved["receipt_game_pid"], 11);
        assert_eq!(saved["receipt_game_created_filetime"], r.created.unwrap());
        assert!(saved["source_mtime_filetime"].as_u64().unwrap() > r.created.unwrap());
        assert!(saved["acquired_at"].as_str().is_some());
        assert_eq!(saved["renderer_selection_verified"], false);
        assert_eq!(r.calls, 2);
    }

    #[test]
    fn canonical_chain_exit_receipt_works_without_a_native_driver_capture() {
        let f = Fixture::new();
        let mut r = model();
        fs::write(
            f.p.here.join("steam-exit-recovery.json"),
            json!({"version":2,"games":[["CivilizationVI.exe",11,r.created.unwrap()]]}).to_string(),
        )
        .unwrap();
        f.run
            .capture(&mut r, &f.p, &f.owner, "chain-before-close")
            .unwrap();
        assert_eq!(
            f.evidence("chain-before-close")["attribution_status"],
            "matched_receipt_generation"
        );
    }

    #[test]
    fn missing_conflicting_and_reused_generations_never_verify_adapter_selection() {
        for case in [
            "missing",
            "driver-conflict",
            "chain-conflict",
            "exit-conflict",
            "reuse",
            "exited",
            "second-query-reuse",
            "old-log",
        ] {
            let mut f = Fixture::new();
            let mut r = model();
            if case != "missing" {
                f.receipt(11, r.created.unwrap());
            }
            match case {
                "driver-conflict" => fs::write(f.run.artifacts.join("capture.parent.json"),
                    json!({"type":"order_parent_receipt","game_pid":12,"game_created_filetime":r.created.unwrap()}).to_string()).unwrap(),
                "chain-conflict" => f.owner["pid_created_filetime"] = json!(8),
                "exit-conflict" => fs::write(f.p.here.join("steam-exit-recovery.json"),
                    json!({"version":2,"games":[["CivilizationVI.exe",12,r.created.unwrap()]]}).to_string()).unwrap(),
                "reuse" => r.created = r.created.map(|n| n+1),
                "exited" => r.created = None,
                "second-query-reuse" => r.replace_on_second = true,
                "old-log" => f.run.started = SystemTime::now() + std::time::Duration::from_secs(60),
                _ => {}
            }
            f.run
                .capture(&mut r, &f.p, &f.owner, "driver-return")
                .unwrap();
            let saved = f.evidence("driver-return");
            assert_eq!(saved["capture_status"], "saved", "{case}");
            assert_eq!(saved["attribution_status"], "unknown", "{case}");
            assert!(saved["attribution_error"].as_str().is_some(), "{case}");
            assert_eq!(saved["renderer_selection_verified"], false, "{case}");
        }
    }

    #[test]
    fn missing_log_and_create_new_collisions_keep_explicit_failures_and_original_bytes() {
        let f = Fixture::new();
        let mut r = model();
        fs::remove_file(log_path(&f.p)).unwrap();
        assert!(f.run.capture(&mut r, &f.p, &f.owner, "missing").is_err());
        assert_eq!(f.evidence("missing")["capture_status"], "failed");
        fs::write(log_path(&f.p), b"first").unwrap();
        f.run.capture(&mut r, &f.p, &f.owner, "once").unwrap();
        let before = fs::read(f.run.artifacts.join("renderer-evidence/once.json")).unwrap();
        fs::write(log_path(&f.p), b"replacement").unwrap();
        assert!(f.run.capture(&mut r, &f.p, &f.owner, "once").is_err());
        assert_eq!(
            fs::read(f.run.artifacts.join("renderer-evidence/once.Renderer.log")).unwrap(),
            b"first"
        );
        assert_eq!(
            fs::read(f.run.artifacts.join("renderer-evidence/once.json")).unwrap(),
            before
        );
    }

    #[test]
    fn sha256_known_vectors_cover_padding_and_multiple_blocks() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            sha256(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
