//! Run live scenarios one after another (only one Civ6 can run). Each item checks out its branch in the live worktree.
//!
//! live_chain.exe name:scenario.json [name:scenario.json ...]
//! Log: %TEMP%/batch1/live_chain.log (lines "RESULT <name> <scenario> <exit> <artifact dir>")
use crate::*;
#[path = "renderer_evidence.rs"]
mod renderer_evidence;
use serde_json::json;
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
pub const CIV_EXECUTABLES: [&str; 2] = ["CivilizationVI.exe", "CivilizationVI_DX12.exe"];

fn live_chain_image(image: &str) -> bool {
    let image = image.to_ascii_lowercase();
    image == "live_chain.exe" || (image.starts_with("live_chain_") && image.ends_with(".exe"))
}

#[cfg(windows)]
pub fn process_created_filetime(pid: u32) -> Result<Option<u64>, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        let e = std::io::Error::last_os_error();
        return if e.raw_os_error() == Some(87) {
            Ok(None)
        } else {
            Err(format!("open process {pid} to verify creation time: {e}"))
        };
    }
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    let ok = unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
    let error = if ok == 0 {
        Some(std::io::Error::last_os_error())
    } else {
        None
    };
    unsafe { CloseHandle(handle) };
    if let Some(e) = error {
        return Err(format!("read process {pid} creation time: {e}"));
    }
    Ok(Some(
        (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime),
    ))
}
pub const REPORTER_COMMAND: &str = r#"
    $civ6 = Get-Process -Name CivilizationVI,CivilizationVI_DX12 -ErrorAction SilentlyContinue
    if ($civ6) { exit 0 }
    $expected = "C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI\Base\Binaries\Win64Steam\FiraxisBugReporter.exe"
    Get-CimInstance Win32_Process -Filter "Name = 'FiraxisBugReporter.exe'" | ForEach-Object {
        if ($_.ExecutablePath -eq $expected) {
            $parent = Get-Process -Id $_.ParentProcessId -ErrorAction SilentlyContinue
            if (-not $parent -or $parent.ProcessName -notin @('CivilizationVI','CivilizationVI_DX12')) {
                try {
                    Stop-Process -Id $_.ProcessId -ErrorAction Stop
                } catch {
                    # The reporter can exit between enumeration and termination.
                    # Keep access failures and other errors visible.
                    if ($_.FullyQualifiedErrorId -notlike 'NoProcessFoundForGivenId,*') { throw }
                }
            }
        }
    }
    "#;

/// Match tasklist's image field for either renderer, never its diagnostic text.
pub fn civ6_running(r: &mut impl Runner) -> Result<bool, String> {
    for executable in CIV_EXECUTABLES {
        let out = r.run(
            Call::new(args(&[
                "tasklist",
                "/FI",
                &format!("IMAGENAME eq {executable}"),
                "/FO",
                "CSV",
                "/NH",
            ]))
            .timeout(30.0),
        )?;
        if out.code != 0 {
            return Err(format!(
                "Could not inspect Civ6 processes: {}",
                tail(&out.stderr, 300)
            ));
        }
        if out
            .stdout
            .lines()
            .any(|line| csv_first(line).eq_ignore_ascii_case(executable))
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn csv_first(line: &str) -> String {
    if let Some(s) = line.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    out.push('"');
                } else {
                    break;
                }
            } else {
                out.push(c);
            }
        }
        out
    } else {
        line.split(',').next().unwrap_or("").into()
    }
}
const GRACEFUL_CLOSE_SECONDS: f64 = 30.0;
const FORCED_CLOSE_SECONDS: f64 = 75.0;
const CLOSE_POLL_SECONDS: f64 = 0.5;

/// Called only by the chain while it holds the shared Civ6 lock.
/// The CLI uses close_civ6_with_paths so the tuner belongs to its live worktree.
pub fn close_civ6(r: &mut impl Runner) -> Result<(), String> {
    close_civ6_with_paths(r, &Paths::default())
}

/// The caller must hold the shared Civ6 lock for the entire close and wait.
pub fn close_civ6_with_paths(r: &mut impl Runner, p: &Paths) -> Result<(), String> {
    close_civ6_with_evidence(r, p, &mut json!({}))
}

type GameGeneration = Vec<(String, u32, u64)>;

fn game_generation(r: &mut impl Runner) -> Result<GameGeneration, String> {
    let mut games = Vec::new();
    let mut candidates = Vec::new();
    for image in CIV_EXECUTABLES {
        let out = r.run(
            Call::new(args(&[
                "tasklist",
                "/FI",
                &format!("IMAGENAME eq {image}"),
                "/FO",
                "CSV",
                "/NH",
            ]))
            .timeout(30.0),
        )?;
        if out.code != 0 {
            return Err("Could not inspect Civ6 exit generation".into());
        }
        for line in out
            .stdout
            .lines()
            .filter(|line| csv_first(line).eq_ignore_ascii_case(image))
        {
            let pid: u32 = line
                .split(',')
                .nth(1)
                .ok_or("game PID missing")?
                .trim_matches('"')
                .parse()
                .map_err(|_| "invalid game PID")?;
            let identity = match r.process_identity(pid, 2.0) {
                Ok(identity) => identity,
                Err(error) => {
                    candidates.push(json!({"pid":pid,"snapshot_image":image,
                        "exit_state":"unknown","created_filetime":null,"inspection_error":error}));
                    return Err(format!(
                        "game identity inspection failed; process_candidates={}",
                        json!(candidates)
                    ));
                }
            };
            let Some(identity) = identity else {
                candidates.push(json!({"pid":pid,"snapshot_image":image,
                    "exit_state":"absent_or_signaled","created_filetime":null,"inspection_error":null}));
                let _ = r.error(&format!("process_candidates={}", json!(candidates)));
                continue;
            };
            candidates.push(json!({"pid":pid,"snapshot_image":image,"image_matches_snapshot":identity.image.eq_ignore_ascii_case(image),
                "exit_state":if identity.created_filetime.is_some(){"not_signaled"}else{"unknown"},
                "created_filetime":identity.created_filetime,
                "inspection_error":if identity.created_filetime.is_some(){None}else{Some("creation FILETIME unavailable; name-only snapshot is not live identity")}}));
            let created = identity
                .created_filetime
                .filter(|created| *created > 0)
                .ok_or_else(|| {
                    format!(
                        "game creation time unavailable; process_candidates={}",
                        json!(candidates)
                    )
                })?;
            if !identity.image.eq_ignore_ascii_case(image) {
                return Err(format!(
                    "game identity changed during exit generation capture; process_candidates={}",
                    json!(candidates)
                ));
            }
            games.push((image.to_owned(), pid, created));
        }
    }
    if games.len() > 1 {
        let _ = r.error(&format!("process_candidates={}", json!(candidates)));
    }
    Ok(games)
}

#[cfg(test)]
mod game_candidate_tests {
    use super::*;
    struct CandidateRunner {
        identity: Result<Option<process::ProcessIdentity>, String>,
        queried: Vec<u32>,
        diagnostics: Vec<String>,
    }
    impl Runner for CandidateRunner {
        fn run(&mut self, call: Call) -> Result<Output, String> {
            assert_eq!(call.args[0], "tasklist");
            Ok(Output {
                stdout: if call.args[2].ends_with("CivilizationVI.exe") {
                    "\"CivilizationVI.exe\",\"12\"".into()
                } else {
                    String::new()
                },
                ..Output::default()
            })
        }
        fn process_identity(
            &mut self,
            pid: u32,
            _: f64,
        ) -> Result<Option<process::ProcessIdentity>, String> {
            self.queried.push(pid);
            self.identity.clone()
        }
        fn now(&mut self) -> f64 {
            0.0
        }
        fn sleep(&mut self, _: f64) -> Result<(), String> {
            panic!("no waits")
        }
        fn error(&mut self, text: &str) -> Result<(), String> {
            self.diagnostics.push(text.into());
            Ok(())
        }
    }
    #[test]
    fn exit_candidate_preserves_unknown_errors_and_skips_only_proven_not_live() {
        for (identity, succeeds) in [
            (Ok(None), true),
            (
                Ok(Some(process::ProcessIdentity {
                    image: "CivilizationVI.exe".into(),
                    created_filetime: Some(41),
                })),
                true,
            ),
            (
                Ok(Some(process::ProcessIdentity {
                    image: "CivilizationVI.exe".into(),
                    created_filetime: None,
                })),
                false,
            ),
            (Err("OpenProcess os_error=5".into()), false),
            (
                Ok(Some(process::ProcessIdentity {
                    image: "other.exe".into(),
                    created_filetime: Some(41),
                })),
                false,
            ),
        ] {
            let mut r = CandidateRunner {
                identity: identity.clone(),
                queried: vec![],
                diagnostics: vec![],
            };
            let result = game_generation(&mut r);
            assert_eq!(r.queried, [12]);
            assert_eq!(result.is_ok(), succeeds);
            match identity {
                Ok(None) => {
                    assert!(result.unwrap().is_empty());
                    assert!(r.diagnostics[0].contains("absent_or_signaled"));
                }
                Ok(Some(process::ProcessIdentity {
                    created_filetime: Some(41),
                    image,
                })) if image == "CivilizationVI.exe" => {
                    assert_eq!(result.unwrap(), [("CivilizationVI.exe".into(), 12, 41)]);
                }
                _ => {
                    let error = result.unwrap_err();
                    assert!(error.contains("\"pid\":12"));
                    assert!(error.contains("\"created_filetime\":"));
                    assert!(error.contains("process_candidates="));
                }
            }
        }
    }
}

struct ExitGenerationRunner<'a, R> {
    runner: &'a mut R,
    games: &'a GameGeneration,
}

#[cfg(test)]
mod exit_generation_deadline_tests {
    use super::*;
    struct Model {
        time: f64,
        query_cost: f64,
        termination_entries: usize,
    }
    impl Runner for Model {
        fn run(&mut self, call: Call) -> Result<Output, String> {
            assert_eq!(call.args[0], "tasklist");
            self.time += self.query_cost.min(call.timeout.unwrap());
            Ok(Output {
                stdout: if call.args[2].ends_with("CivilizationVI.exe") {
                    "\"CivilizationVI.exe\",\"11\"".into()
                } else {
                    String::new()
                },
                ..Output::default()
            })
        }
        fn process_identity(
            &mut self,
            _: u32,
            _: f64,
        ) -> Result<Option<process::ProcessIdentity>, String> {
            Ok(Some(process::ProcessIdentity {
                image: "CivilizationVI.exe".into(),
                created_filetime: Some(1),
            }))
        }
        fn terminate_game(
            &mut self,
            _: u32,
            _: &str,
            _: u64,
            deadline: f64,
        ) -> Result<Output, String> {
            self.termination_entries += 1;
            assert!(self.time < deadline);
            Ok(Output::default())
        }
        fn now(&mut self) -> f64 {
            self.time
        }
        fn sleep(&mut self, seconds: f64) -> Result<(), String> {
            self.time += seconds;
            Ok(())
        }
    }
    #[test]
    fn generation_expiry_blocks_api_entry_with_a_positive_nonexpired_control() {
        let games = vec![("CivilizationVI.exe".into(), 11, 1)];
        for query_cost in [0.1, 1.0] {
            let mut model = Model {
                time: 0.0,
                query_cost,
                termination_entries: 0,
            };
            let result = ExitGenerationRunner {
                runner: &mut model,
                games: &games,
            }
            .run(Call::new(args(&["taskkill", "/IM", "CivilizationVI.exe", "/F"])).timeout(1.0));
            assert_eq!(result.is_ok(), query_cost == 0.1);
            assert_eq!(model.termination_entries, usize::from(query_cost == 0.1));
        }
    }
}
impl<R: Runner> Runner for ExitGenerationRunner<'_, R> {
    fn run(&mut self, mut call: Call) -> Result<Output, String> {
        // An old cursor may never close a game that appeared after its mark.
        // Recheck at every external observation/action in the exit protocol.
        let deadline = call
            .deadline
            .unwrap_or(f64::MAX)
            .min(self.runner.monotonic() + call.timeout.unwrap_or(30.0));
        let current = game_generation(&mut WithinDeadline {
            runner: self.runner,
            deadline,
        })?;
        if current.iter().any(|game| !self.games.contains(game)) {
            return Err(
                "Civ6 exit generation changed; refusing to close or restore another game".into(),
            );
        }
        if call.args.first().is_some_and(|arg| arg == "taskkill")
            && call.args.get(1).is_some_and(|arg| arg == "/IM")
        {
            if let Some((image, pid, created)) = current
                .iter()
                .find(|(image, _, _)| image.eq_ignore_ascii_case(&call.args[2]))
            {
                time_left(self.runner, deadline)?;
                return self.runner.terminate_game(*pid, image, *created, deadline);
            } else {
                return Ok(Output::default());
            }
        }
        call.timeout = Some(time_left(self.runner, deadline)?);
        self.runner.run(call)
    }
    fn now(&mut self) -> f64 {
        self.runner.now()
    }
    fn monotonic(&mut self) -> f64 {
        self.runner.monotonic()
    }
    fn sleep(&mut self, seconds: f64) -> Result<(), String> {
        self.runner.sleep(seconds)
    }
    fn print(&mut self, text: &str) -> Result<(), String> {
        self.runner.print(text)
    }
    fn error(&mut self, text: &str) -> Result<(), String> {
        self.runner.error(text)
    }
}

fn close_civ6_with_evidence(
    r: &mut impl Runner,
    p: &Paths,
    evidence: &mut Value,
) -> Result<(), String> {
    close_civ6_with_renderer(r, p, evidence, None)
}

fn close_civ6_with_renderer(
    r: &mut impl Runner,
    p: &Paths,
    evidence: &mut Value,
    renderer: Option<(&renderer_evidence::Run, &str, &str)>,
) -> Result<(), String> {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut journal = p.temp.join(format!(
        "civ6-steam-exit-{}-{stamp}-{}.jsonl",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let sync_tool = p.live().join("tools/target/release/civ6-steam-sync.exe");
    let source = p
        .live()
        .join("tools/civ6-autotest-rs/src/bin/civ6-steam-sync.rs");
    let receipt = p.here.join("steam-exit-recovery.json");
    let mut expected_games: GameGeneration = Vec::new();
    let pending = match fs::read(&receipt) {
        Ok(bytes) => {
            let state: Value = serde_json::from_slice(&bytes)
                .map_err(|e| format!("invalid pending Steam exit: {e}"))?;
            if state["version"] != 2
                || !state["cursor"]["cloud"].is_object()
                || !state["cursor"]["process"].is_object()
            {
                return Err("invalid pending Steam exit; refusing a new generation".into());
            }
            expected_games = serde_json::from_value(state["games"].clone())
                .map_err(|e| format!("invalid pending game generation: {e}"))?;
            if expected_games.is_empty()
                || expected_games.iter().any(|(image, pid, created)| {
                    !CIV_EXECUTABLES.contains(&image.as_str()) || *pid == 0 || *created == 0
                })
            {
                return Err("pending Steam exit lacks its original game generation".into());
            }
            journal = state["journal"]
                .as_str()
                .ok_or("pending Steam exit lacks journal")?
                .into();
            Some(state["cursor"].to_string())
        }
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Err(format!("read pending Steam exit: {e}")),
    };
    let recovering = pending.is_some();
    let cursor = (|| -> Result<Option<String>, String> {
        if let Some(cursor) = pending {
            return Ok(Some(cursor));
        }
        if source.is_file() && sync_tool.is_file() {
            expected_games = game_generation(r)?;
            if expected_games.is_empty() {
                return Ok(None);
            }
            // Mark both files inside the native helper, before asking the game to exit.
            let out = r.run(
                Call::new(args(&[&path(&sync_tool), "--mark"]))
                    .managed()
                    .timeout(10.0),
            )?;
            if out.code != 0 {
                return Err(format!(
                    "cannot mark Steam sync before exit: {} {}",
                    out.stdout, out.stderr
                ));
            }
            let value: serde_json::Value = serde_json::from_str(out.stdout.trim())
                .map_err(|e| format!("invalid Steam mark: {e}"))?;
            if !value["cloud"].is_object() || !value["process"].is_object() {
                return Err("Steam mark lacks generation cursors".into());
            }
            // Keep the original pre-exit generation across failed waits and
            // process restarts. Publish before asking the game to exit.
            let bytes = serde_json::to_vec(&json!({
                "version": 2, "cursor": value, "journal": path(&journal), "games": expected_games
            }))
            .map_err(|e| e.to_string())?;
            crate::software_renderer::publish_backup(&receipt, &bytes)
                .map_err(|e| format!("publish pending Steam exit: {e}"))?;
            Ok(Some(out.stdout.trim().to_owned()))
        } else if source.is_file() {
            Err("Steam sync helper source exists but its executable is missing".into())
        } else {
            Ok(None)
        }
    })()?;
    // The normal exit receipt now contains the canonical game generation.
    // Preserve the log before close, even when the later Cloud wait fails.
    if let Some((run, token, stage)) = renderer {
        retain_renderer(r, p, Some(token), Some(run), stage);
    }
    let outcome = if cursor.is_some() || source.is_file() {
        close_civ6_outcome(
            &mut ExitGenerationRunner {
                runner: r,
                games: &expected_games,
            },
            p,
        )?
    } else {
        close_civ6_outcome(r, p)?
    };
    evidence["game_close_outcome"] = json!(outcome);
    // A diagnostic sink cannot be allowed to interrupt the actual recovery.
    let mut printed = cleanup_print(r, &format!("CIV6_CLOSE outcome={outcome}"));
    let sync_result = (|| -> Result<(), String> {
        if outcome == "absent" && cursor.is_none() {
            evidence["sync_phase"] = json!("not_needed_game_absent");
            return Ok(());
        }
        if source.is_file() {
            let Some(cursor) = cursor else {
                return Err("Steam sync helper source exists but its executable is missing; game closed, refusing an unobserved sync".into());
            };
            evidence["sync_journal"] = json!(path(&journal));
            let argv = args(&[
                &path(sync_tool),
                "--cursor",
                &cursor,
                "--journal",
                &path(&journal),
            ]);
            let out = ExitGenerationRunner {
                runner: r,
                games: &expected_games,
            }
            .run(
                Call::new(argv)
                    .env("CIV6_LIVE_CHAIN_PID", std::process::id().to_string())
                    .env("CIV6_LIVE_LOCK_PATH", path(p.here.join("civ6.lock")))
                    .managed()
                    .timeout(50.0),
            )?;
            if let Ok(text) = fs::read_to_string(&journal) {
                if let Some(last) = text.lines().rfind(|line| !line.trim().is_empty()) {
                    let event: Value = serde_json::from_str(last)
                        .map_err(|e| format!("invalid exit journal: {e}"))?;
                    evidence["sync_phase"] = event["phase"].clone();
                }
            }
            if out.code != 0 {
                return Err(format!(
                    "Steam exit sync did not finish: {} {}",
                    out.stdout, out.stderr
                ));
            }
            if !matches!(
                evidence["sync_phase"].as_str(),
                Some("exit_sync_complete" | "steam_released_without_new_sync_request")
            ) {
                return Err("Steam sync helper returned success without a valid terminal journal; refusing an unproven exit".into());
            }
            fs::remove_file(&receipt).map_err(|e| format!("clear confirmed Steam exit: {e}"))?;
        } else if recovering {
            return Err("pending Steam exit needs its native helper before recovery".into());
        } else {
            evidence["sync_phase"] = json!("legacy_helper_absent");
            let message = cleanup_print(r,"STEAM_SYNC skipped: previous live checkout has no helper source; the prepared acceptance checkout must contain it");
            printed = printed.clone().and(message);
        }
        Ok(())
    })();
    evidence["sync_result"] = json!(if sync_result.is_ok() {
        "success"
    } else {
        "failed"
    });
    evidence["sync_error"] = json!(sync_result.as_ref().err());
    // Game absence alone does not complete an earlier Cloud barrier. Never
    // move the owner's originals back while the original exit is unresolved.
    let restored = if sync_result.is_ok() {
        restore_autosaves(r, p, &mut printed)
    } else {
        Err("autosave restoration waits for confirmed Steam release".into())
    };
    evidence["autosave_restore_result"] = json!(if restored.is_ok() {
        "success"
    } else {
        "failed"
    });
    evidence["autosave_restore_error"] = json!(restored.as_ref().err());
    evidence["recovery_receipt_remaining"] =
        json!(p.here.join("reliability-auto-state.json").exists());
    evidence["cleanup_confirmed"] = json!(sync_result.is_ok() && restored.is_ok());
    evidence["output_error"] = json!(printed.as_ref().err());
    let sync_result = sync_result.and(printed);
    match (sync_result, restored) {
        (Ok(()), r) => r,
        (Err(sync), Ok(())) => Err(sync),
        (Err(sync), Err(recovery)) => Err(format!("{sync}; {recovery}")),
    }
}
fn cleanup_print(r: &mut impl Runner, message: &str) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| r.print(message)))
        .unwrap_or_else(|_| Err("cleanup output panicked".into()))
}
fn close_cleanup_confirmed(evidence: &Value) -> bool {
    evidence["cleanup_confirmed"] == true
}
fn restore_autosaves(
    r: &mut impl Runner,
    p: &Paths,
    printed: &mut Result<(), String>,
) -> Result<(), String> {
    let auto_state = p.here.join("reliability-auto-state.json");
    if auto_state.is_file() {
        let helper = p
            .live()
            .join("tools/target/release/civ6-save-isolation.exe");
        let out = r.run(
            Call::new(args(&[
                &path(helper),
                "--restore",
                "--state",
                &path(auto_state),
            ]))
            .env("CIV6_LIVE_CHAIN_PID", std::process::id().to_string())
            .env("CIV6_LIVE_LOCK_PATH", path(p.here.join("civ6.lock")))
            .managed()
            .timeout(30.0),
        )?;
        if out.code != 0 {
            return Err(format!(
                "owner autosave recovery failed: {} {}",
                out.stdout, out.stderr
            ));
        }
        let message = cleanup_print(
            r,
            "AUTOSAVE original folder restored; test autosaves retained with artifacts",
        );
        *printed = printed.clone().and(message);
    }
    // Helper success must not silently leave an unresolved original receipt.
    for name in ["reliability-auto-state.json", "steam-exit-recovery.json"] {
        if p.here
            .join(name)
            .try_exists()
            .map_err(|e| format!("inspect recovery receipt {name}: {e}"))?
        {
            return Err(format!(
                "recovery receipt remains after restoration: {name}"
            ));
        }
    }
    Ok(())
}

fn close_civ6_outcome(r: &mut impl Runner, p: &Paths) -> Result<&'static str, String> {
    if !civ6_running(r)? {
        close_stale_civ6_reporters(r)?;
        return Ok("absent");
    }
    let deadline = r.monotonic() + GRACEFUL_CLOSE_SECONDS;
    let requested = request_graceful_close(
        &mut WithinDeadline {
            runner: r,
            deadline,
        },
        p,
    );
    match requested {
        Ok(()) => {
            // Closing can disconnect FireTuner before it returns a Lua response.
            // Only process disappearance proves that shutdown completed.
            if wait_civ6_exit(r, deadline)? {
                close_stale_civ6_reporters(r)?;
                return Ok("closed");
            }
        }
        Err(error) => r.print(&format!("Civ6 graceful close unavailable: {error}"))?,
    }
    // A failed request, or a process exiting exactly at the graceful deadline,
    // must not cause us to force-kill a game which has already disappeared.
    if !civ6_running(r)? {
        close_stale_civ6_reporters(r)?;
        return Ok("closed");
    }
    let deadline = r.monotonic() + FORCED_CLOSE_SECONDS;
    let mut budget = WithinDeadline {
        runner: r,
        deadline,
    };
    for executable in CIV_EXECUTABLES {
        budget.run(Call::new(args(&["taskkill", "/IM", executable, "/F"])).timeout(30.0))?;
    }
    // taskkill can fail (e.g. access denied). Never treat that as a closed game.
    let exit_deadline = deadline.min(budget.monotonic() + 15.0);
    if !wait_civ6_exit(&mut budget, exit_deadline)? {
        return Err(
            "Civ6 did not exit after taskkill; refusing to continue the live scenario".into(),
        );
    }
    close_stale_civ6_reporters(&mut budget)?;
    Ok("killed")
}

fn request_graceful_close(r: &mut impl Runner, p: &Paths) -> Result<(), String> {
    let tuner = driver::tuner(p, &p.live())?;
    let mut states = tuner.clone();
    states.push("states".into());
    let out = r.run(Call::new(states).timeout(10.0))?;
    if out.code != 0 {
        return Err(format!(
            "FireTuner states failed: {}",
            tail(&out.stderr, 300)
        ));
    }
    // civ6-tuner prints an identity followed by `index: name` pairs. Select a
    // unique official UI context, never an identity or a similarly named mod.
    let states: Vec<_> = out
        .stdout
        .lines()
        .filter_map(|line| {
            let (index, name) = line.split_once(':')?;
            index.trim().parse::<i64>().ok()?;
            Some(name.trim())
        })
        .collect();
    let state = ["FrontEnd", "InGame"]
        .into_iter()
        .find(|name| {
            states
                .iter()
                .filter(|state| state.eq_ignore_ascii_case(name))
                .count()
                == 1
        })
        .ok_or("FireTuner has no unique FrontEnd or InGame context")?;
    let mut argv = tuner;
    argv.extend(args(&[
        "lua",
        "--state",
        state,
        "--code",
        "Events.UserConfirmedClose()\nreturn true",
    ]));
    match r.run(Call::new(argv).timeout(15.0)) {
        Ok(out) if out.code == 0 => {}
        Ok(out) => r.print(&format!(
            "Civ6 quit response unavailable (exit {}): {}",
            out.code,
            tail(&out.stderr, 300)
        ))?,
        Err(error) => r.print(&format!("Civ6 quit response unavailable: {error}"))?,
    }
    // The Lua call may have reached the game even when its response was lost.
    // Do not resend it or force-kill before allowing the bounded exit wait.
    Ok(())
}

fn wait_civ6_exit(r: &mut impl Runner, deadline: f64) -> Result<bool, String> {
    loop {
        if r.monotonic() >= deadline {
            return Ok(false);
        }
        let running = civ6_running(&mut WithinDeadline {
            runner: r,
            deadline,
        });
        match running {
            Ok(false) => return Ok(true),
            Ok(true) => {}
            // The first renderer query may use up the phase budget before
            // the second can start. The caller rechecks before taskkill.
            Err(_) if r.monotonic() >= deadline => return Ok(false),
            Err(error) => return Err(error),
        }
        let left = deadline - r.monotonic();
        if left <= 0.0 {
            return Ok(false);
        }
        r.sleep(CLOSE_POLL_SECONDS.min(left))?;
    }
}
/// Steam counts an orphaned Civ6 crash reporter as a running game.
///
/// Check the exact Civ6 executable path and skip reporters whose Civ6 parent
/// still lives. Other Firaxis games and active Civ6 processes are untouched.
pub fn close_stale_civ6_reporters(r: &mut impl Runner) -> Result<(), String> {
    let out = r.run(
        Call::new(args(&[
            "powershell",
            "-NoProfile",
            "-Command",
            REPORTER_COMMAND,
        ]))
        .timeout(30.0),
    )?;
    if out.code != 0 {
        return Err(format!(
            "Could not close the stale Civ6 crash reporter: {}",
            tail(&out.stderr, 300)
        ));
    }
    Ok(())
}
/// Close the previous game under the shared lock before restart_game relaunches it.
pub fn ensure_responsive_civ6(r: &mut impl Runner, p: &Paths) -> Result<&'static str, String> {
    close_civ6_outcome(r, p)
}
/// Job records take precedence over a PID; the game and Steam cannot own a run.
pub fn holder_alive(r: &mut impl Runner, p: &Paths, info: &Value) -> Result<bool, String> {
    let mut has_job = false;
    for key in ["owner_job", "job"] {
        if !ship::truthy(&info[key]) {
            continue;
        }
        has_job = true;
        let Some(job) = info[key].as_str() else {
            return Ok(false);
        };
        let bytes = match fs::read(p.jobs.join(job).join("job.json")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.to_string()),
        };
        // A temporarily unreadable record is not evidence that its owner died.
        let job: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if !matches!(
            job["status"].as_str(),
            Some("queued" | "starting" | "running")
        ) {
            return Ok(false);
        }
    }
    if has_job && info["live_chain"] != true {
        return Ok(true);
    }
    let Some(pid) = info["pid"]
        .as_u64()
        .or_else(|| info["pid"].as_str()?.parse::<u64>().ok())
        .filter(|pid| *pid > 0 && *pid <= u32::MAX as u64)
    else {
        return Ok(false);
    };
    let Some(identity) = r.process_identity(pid as u32, 30.0)? else {
        return Ok(false);
    };
    let image_matches = (info["live_chain"] != true || live_chain_image(&identity.image))
        && ![
            "CivilizationVI.exe",
            "CivilizationVI_DX12.exe",
            "FiraxisBugReporter.exe",
            "steam.exe",
        ]
        .iter()
        .any(|excluded| identity.image.eq_ignore_ascii_case(excluded));
    if !image_matches {
        return Ok(false);
    }
    if info["live_chain"] == true {
        if let Some(created) = identity.created_filetime {
            let expected = info["pid_created_filetime"]
                .as_u64()
                .ok_or("live_chain creation time missing; process ownership is unknown")?;
            return Ok(created == expected);
        }
    }
    #[cfg(windows)]
    if info["live_chain"] == true {
        if let Some(expected) = info["pid_created_filetime"].as_u64() {
            let created = match identity.created_filetime {
                Some(created) => Some(created),
                None => process_created_filetime(pid as u32)?,
            };
            return Ok(created == Some(expected));
        }
    }
    Ok(true)
}
/// Multi-hour runs (200/300-turn health and tuning runs) yield to short PR checks (todo 139, 2026-10-01).
pub fn is_long(items: &[String]) -> bool {
    let re = regex::Regex::new(r"(?:^|[^0-9])(1[5-9]\d|[2-9]\d\d)turn").unwrap();
    items.iter().any(|s| re.is_match(s))
}
/// Short runs start first until a long run has waited one lease. Each waiter
/// has one effective arrival time, so every claimant sees the same total order.
pub fn my_turn(r: &mut impl Runner, p: &Paths, me: &Path) -> Result<bool, String> {
    let mine = read_json(me)?;
    let mut registered = Vec::new();
    let entries = fs::read_dir(p.here.join("queue")).map_err(|e| e.to_string())?;
    for entry in entries.filter_map(Result::ok) {
        let other = entry.path();
        if other == me || other.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(info) = read_json(&other) else {
            continue;
        };
        if !holder_alive(r, p, &info)? {
            remove_if_exists(&other)?;
            continue;
        }
        registered.push(info.clone());
        let rank = |v: &Value| -> Result<(f64, f64), String> {
            let since = v["since"].as_f64().ok_or("waiter since missing")?;
            let long = v["long"].as_i64().ok_or("waiter long missing")?;
            let effective = if long == 1 && v["fifo"] != true {
                since + fairness::LEASE_SECONDS
            } else {
                since
            };
            if !since.is_finite() || !effective.is_finite() {
                return Err("waiter since is not finite".into());
            }
            Ok((effective, since))
        };
        let theirs = rank(&info)?;
        let ours = rank(&mine)?;
        let before = theirs
            .0
            .total_cmp(&ours.0)
            .then_with(|| theirs.1.total_cmp(&ours.1))
            .then_with(|| other.as_path().cmp(me))
            == std::cmp::Ordering::Less;
        if before {
            return Ok(false);
        }
    }
    let items = fairness::items(&fairness::todo_path(p))?;
    let now = if items.is_empty() { 0. } else { r.now() };
    for item in items {
        let assigned = json!({"job":item["scheduler"]["job"]});
        if fairness::same_owner(&mine, &assigned) {
            continue;
        }
        // Once a dispatched job reaches the queue, its queue rank governs.
        // Reserving it again as a ToDo would make an earlier queue claimant
        // wait for this job while this job waits for that earlier claimant.
        if registered
            .iter()
            .any(|owner| fairness::same_owner(owner, &assigned))
        {
            continue;
        }
        if fairness::ready_item(&item, now)
            && fairness::item_since(&item, now) <= mine["since"].as_f64().unwrap_or(now)
        {
            return Ok(false);
        }
        // Assignment also spans builds and reviews. Only an actual queue or
        // lease reserves the game for an assigned worker; otherwise two active
        // workers can prevent each other from ever entering their first run.
    }
    Ok(true)
}
fn remove_if_exists(p: &Path) -> Result<(), String> {
    match fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

static LOCK_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const RELEASE_WAIT_SECONDS: f64 = 5.0;
const RELEASE_RETRY_SECONDS: f64 = 0.05;

fn new_lock_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{}-{nanos:x}-{:x}",
        std::process::id(),
        LOCK_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn queue_entry_name() -> String {
    format!("{}.json", new_lock_token())
}

#[cfg(windows)]
fn open_for_owned_delete(path: &Path) -> std::io::Result<File> {
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, DELETE, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING,
        },
    };

    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // DELETE is requested at open time. While this handle lives, a writer or
    // renamer cannot change the path between ownership check and disposition.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE | DELETE,
            FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_handle(handle) })
    }
}

fn open_for_owned_protection(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Compatible with SDK readData/ShareRW. Never request DELETE while
        // helpers run, and never permit another handle to delete/rename.
        options.share_mode(3);
    }
    options.open(path)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LockFileId(u64, u64);
fn lock_file_id(file: &File) -> Result<LockFileId, String> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        if unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut info) } == 0 {
            return Err(format!(
                "inspect protected lock file ID: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(LockFileId(
            u64::from(info.dwVolumeSerialNumber),
            (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        ))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let info = file
            .metadata()
            .map_err(|e| format!("inspect protected lock file ID: {e}"))?;
        Ok(LockFileId(info.dev(), info.ino()))
    }
}
fn lock_actor(info: &Value) -> Value {
    json!({"live_chain":info["live_chain"],"pid":info["pid"],"chain_pid":info["chain_pid"],
        "pid_created_filetime":info["pid_created_filetime"],"lock_token":info["lock_token"],
        "acquired_at":info["acquired_at"],"job":info["job"],"owner_job":info["owner_job"]})
}
struct CloseProtection {
    file: File,
    id: LockFileId,
    actor: Value,
    #[cfg(unix)]
    _guard: fairness::UnixGuard,
}
impl CloseProtection {
    fn check(&self, r: &mut impl Runner, path: &Path) -> Result<(), String> {
        let current = protected_lock_snapshot(r, &self.file, path)?;
        if lock_actor(&current) != self.actor {
            return Err("protected lock actor identity changed".into());
        }
        Ok(())
    }
    fn release(self, r: &mut impl Runner, path: &Path, token: &str) -> Result<(), String> {
        self.check(r, path)?;
        let id = self.id;
        let actor = self.actor.clone();
        drop(self);
        // All our deny-delete pins and all managed helper readers are gone.
        // Reopening does not trust the pathname: bind the DELETE handle to
        // the same file ID AND acquisition actor before disposition.
        let mut lock =
            open_owned_lock(r, path, token)?.ok_or("lock ownership changed at final release")?;
        if lock_file_id(&lock.file)? != id {
            return Err("lock file identity changed at final release".into());
        }
        lock.file
            .seek(SeekFrom::Start(0))
            .map_err(|e| format!("rewind release identity: {e}"))?;
        let current: Value = serde_json::from_reader(&mut lock.file)
            .map_err(|e| format!("read release identity: {e}"))?;
        if lock_actor(&current) != actor {
            return Err("lock actor identity changed at final release".into());
        }
        delete_owned_handle(lock, path, r, token)
    }
}
fn protected_lock_snapshot(r: &mut impl Runner, pin: &File, path: &Path) -> Result<Value, String> {
    let deadline = r.monotonic() + RELEASE_WAIT_SECONDS;
    loop {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        match options.open(path) {
            Ok(mut reader) => {
                if lock_file_id(pin)? != lock_file_id(&reader)? {
                    return Err("protected lock file identity changed".into());
                }
                let mut bytes = Vec::new();
                (&mut reader)
                    .take(32769)
                    .read_to_end(&mut bytes)
                    .map_err(|e| format!("read protected lock snapshot: {e}"))?;
                if bytes.len() > 32768 {
                    return Err("protected lock record too large".into());
                }
                return serde_json::from_slice(&bytes)
                    .map_err(|e| format!("parse protected lock snapshot: {e}"));
            }
            Err(error) => {
                if !cfg!(windows) || error.raw_os_error() != Some(32) || r.monotonic() >= deadline {
                    return Err(format!("open protected lock snapshot: {error}"));
                }
                r.sleep(RELEASE_RETRY_SECONDS)?;
            }
        }
    }
}
fn open_close_protection(
    r: &mut impl Runner,
    path: &Path,
    token: &str,
) -> Result<Option<CloseProtection>, String> {
    #[cfg(unix)]
    let guard = fairness::UnixGuard::acquire(path)?;
    let file = match open_for_owned_protection(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("open read-only close protection: {error}")),
    };
    let info = protected_lock_snapshot(r, &file, path)?;
    if info["live_chain"] != true
        || info["pid"].as_u64() != Some(u64::from(std::process::id()))
        || info["chain_pid"].as_u64() != Some(u64::from(std::process::id()))
        || info["lock_token"].as_str() != Some(token)
    {
        return Ok(None);
    }
    #[cfg(windows)]
    {
        let created = process_created_filetime(std::process::id())?
            .ok_or("close guard process identity unavailable")?;
        if info["pid_created_filetime"].as_u64() != Some(created) {
            return Err("close guard process FILETIME differs or is missing".into());
        }
    }
    Ok(Some(CloseProtection {
        id: lock_file_id(&file)?,
        actor: lock_actor(&info),
        file,
        #[cfg(unix)]
        _guard: guard,
    }))
}

#[cfg(all(test, windows))]
mod close_scope_tests {
    use super::*;
    use std::{
        os::windows::process::CommandExt,
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };

    struct OwnedDir(std::path::PathBuf);
    impl OwnedDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("todo880-close-scope-{}", new_lock_token()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for OwnedDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    struct Reader {
        child: Child,
        release: std::path::PathBuf,
    }
    impl Reader {
        fn start(root: &Path) -> Self {
            Self {
                child: Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "chain::close_scope_tests::close_scope_sdk_reader_child",
                    ])
                    .env("CC_CLOSE_SCOPE_FIXTURE", root)
                    .creation_flags(process::CREATE_NO_WINDOW)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
                release: root.join("release"),
            }
        }
        fn report(&mut self, root: &Path) -> Value {
            let deadline = Instant::now() + Duration::from_secs(5);
            let file = root.join("report.json");
            while !file.exists() {
                assert!(
                    self.child.try_wait().unwrap().is_none(),
                    "owned reader exited before its report"
                );
                assert!(Instant::now() < deadline, "owned reader report deadline");
                std::thread::sleep(Duration::from_millis(10));
            }
            // Child publishes its complete report with one rename.
            read_json(file).unwrap()
        }
        fn finish(&mut self) {
            fs::write(&self.release, b"release").unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    assert!(status.success());
                    return;
                }
                assert!(Instant::now() < deadline, "owned reader reaping deadline");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    impl Drop for Reader {
        fn drop(&mut self) {
            let _ = fs::write(&self.release, b"release");
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
    #[test]
    #[ignore = "only spawned by the owned close-scope regressions"]
    fn close_scope_sdk_reader_child() {
        use std::os::windows::fs::OpenOptionsExt;
        let Some(root) = std::env::var_os("CC_CLOSE_SCOPE_FIXTURE") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let path = root.join("civ6.lock");
        let pin = OpenOptions::new().read(true).share_mode(3).open(&path);
        let report = match &pin {
            Err(error) => json!({"pin_open_error":error.raw_os_error(),"game_launches":0}),
            Ok(_) => {
                let snapshot = OpenOptions::new().read(true).share_mode(1).open(&path);
                match snapshot {
                    Err(error) => {
                        json!({"pin_open_error":null,"snapshot_open_error":error.raw_os_error(),"game_launches":0})
                    }
                    Ok(file) => {
                        json!({"pin_open_error":null,"snapshot_open_error":null,"record":serde_json::from_reader::<_,Value>(file).unwrap(),"game_launches":0})
                    }
                }
            }
        };
        let mut file = File::create_new(root.join("report.tmp")).unwrap();
        serde_json::to_writer(&mut file, &report).unwrap();
        file.sync_all().unwrap();
        drop(file);
        fs::rename(root.join("report.tmp"), root.join("report.json")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !root.join("release").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(pin);
    }
    #[test]
    fn close_scope_legacy_delete_handle_reproduces_sdk_child_os32() {
        let directory = OwnedDir::new();
        let path = directory.0.join("civ6.lock");
        fs::write(&path, b"{}").unwrap();
        let protection = open_for_owned_delete(&path).unwrap();
        let mut child = Reader::start(&directory.0);
        let report = child.report(&directory.0);
        assert_eq!(report["pin_open_error"], 32);
        println!("TODO880_LOCK_OS_RECEIPT={report}");
        child.finish();
        drop(protection);
    }
    #[test]
    fn close_scope_protection_must_allow_sdk_child_read_proof_without_delete_sharing() {
        let directory = OwnedDir::new();
        let path = directory.0.join("civ6.lock");
        let owner = json!({"live_chain":true,"pid":std::process::id(),"chain_pid":std::process::id(),"lock_token":"owned"});
        fs::write(&path, owner.to_string()).unwrap();
        let protection = open_for_owned_protection(&path).unwrap();
        let mut child = Reader::start(&directory.0);
        let report = child.report(&directory.0);
        assert!(report["pin_open_error"].is_null(), "{report}");
        assert!(report["snapshot_open_error"].is_null(), "{report}");
        assert_eq!(report["record"], owner);
        assert_eq!(fs::remove_file(&path).unwrap_err().raw_os_error(), Some(32));
        child.finish();
        drop(protection);
        let release = open_for_owned_delete(&path).unwrap();
        drop(release);
        fs::remove_file(&path).unwrap();
    }
    struct ScopeRunner {
        time: f64,
        reader: Option<Reader>,
        waits: usize,
    }
    impl Runner for ScopeRunner {
        fn run(&mut self, _: Call) -> Result<Output, String> {
            panic!("no game or external command in lock-scope test");
        }
        fn now(&mut self) -> f64 {
            self.time
        }
        fn sleep(&mut self, seconds: f64) -> Result<(), String> {
            self.waits += 1;
            if let Some(reader) = self.reader.as_mut() {
                reader.finish();
            }
            self.time += seconds;
            Ok(())
        }
    }
    fn owned_record() -> Value {
        json!({"live_chain":true,"pid":std::process::id(),"chain_pid":std::process::id(),
            "pid_created_filetime":process_created_filetime(std::process::id()).unwrap().unwrap(),
            "lock_token":"owned","acquired_at":1000})
    }
    #[test]
    fn close_scope_final_delete_waits_for_sdk_reader_and_keeps_same_file_actor() {
        let directory = OwnedDir::new();
        let path = directory.0.join("civ6.lock");
        let owner = owned_record();
        fs::write(&path, owner.to_string()).unwrap();
        let mut runner = ScopeRunner {
            time: 0.0,
            reader: None,
            waits: 0,
        };
        let guard = open_close_protection(&mut runner, &path, "owned")
            .unwrap()
            .unwrap();
        let mut reader = Reader::start(&directory.0);
        assert_eq!(reader.report(&directory.0)["record"], owner);
        runner.reader = Some(reader);
        assert_eq!(
            open_for_owned_delete(&path).unwrap_err().raw_os_error(),
            Some(32)
        );
        guard.release(&mut runner, &path, "owned").unwrap();
        assert_eq!(runner.waits, 1);
        assert!(!path.exists());
    }
    #[test]
    fn close_scope_actor_change_or_unknown_filetime_prevents_release() {
        let directory = OwnedDir::new();
        let path = directory.0.join("civ6.lock");
        let owner = owned_record();
        fs::write(&path, owner.to_string()).unwrap();
        let mut runner = ScopeRunner {
            time: 0.0,
            reader: None,
            waits: 0,
        };
        let guard = open_close_protection(&mut runner, &path, "owned")
            .unwrap()
            .unwrap();
        let mut changed = owner.clone();
        changed["lock_token"] = json!("peer");
        fs::write(&path, changed.to_string()).unwrap();
        assert!(guard
            .release(&mut runner, &path, "owned")
            .unwrap_err()
            .contains("actor identity changed"));
        assert_eq!(read_json(&path).unwrap(), changed);
        fs::write(&path, owner.to_string()).unwrap();
        let guard = open_close_protection(&mut runner, &path, "owned")
            .unwrap()
            .unwrap();
        let mut changed = owner.clone();
        changed["pid_created_filetime"] = json!(1);
        fs::write(&path, changed.to_string()).unwrap();
        assert!(guard
            .check(&mut runner, &path)
            .unwrap_err()
            .contains("actor identity changed"));
        drop(guard);
        assert!(open_close_protection(&mut runner, &path, "owned").is_err());
        assert!(path.exists());
    }
}

#[cfg(not(windows))]
fn open_for_owned_delete(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).write(true).open(path)
}

struct OwnedLock {
    file: File,
    #[cfg(unix)]
    _guard: fairness::UnixGuard,
}

/// Inspect through the same protected handle that will perform the deletion.
/// An absent or different owner is never ours to close or release.
fn file_is_owned(file: &mut File, path: &Path, token: &str) -> Result<bool, String> {
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("read owned lock {}: {e}", path.display()))?;
    let info: Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse owned lock {}: {e}", path.display()))?;
    Ok(info["live_chain"] == true
        && info["chain_pid"].as_u64() == Some(std::process::id() as u64)
        && info["lock_token"].as_str() == Some(token))
}

fn open_owned_lock(
    r: &mut impl Runner,
    path: &Path,
    token: &str,
) -> Result<Option<OwnedLock>, String> {
    #[cfg(unix)]
    let _ = r;
    #[cfg(unix)]
    let guard = fairness::UnixGuard::acquire(path)?;
    #[cfg(windows)]
    let mut first_conflict = None;
    loop {
        match open_for_owned_delete(path) {
            Ok(mut file) => {
                if file_is_owned(&mut file, path, token)? {
                    return Ok(Some(OwnedLock {
                        file,
                        #[cfg(unix)]
                        _guard: guard,
                    }));
                }
                return Ok(None);
            }
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            #[cfg(windows)]
            Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => {
                let since = *first_conflict.get_or_insert_with(|| r.monotonic());
                if r.monotonic() - since >= RELEASE_WAIT_SECONDS {
                    return Err(format!(
                        "open owned lock {} for release: {e}",
                        path.display()
                    ));
                }
                r.sleep(RELEASE_RETRY_SECONDS).map_err(|e| {
                    format!(
                        "wait to open owned lock {} for release: {e}",
                        path.display()
                    )
                })?;
            }
            Err(e) => {
                return Err(format!(
                    "open owned lock {} for release: {e}",
                    path.display()
                ))
            }
        }
    }
}

#[cfg(windows)]
fn delete_owned_handle(
    lock: OwnedLock,
    path: &Path,
    r: &mut impl Runner,
    _token: &str,
) -> Result<(), String> {
    use std::{mem::size_of, os::windows::io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };

    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    let mut first_conflict = None;
    loop {
        let ok = unsafe {
            SetFileInformationByHandle(
                lock.file.as_raw_handle(),
                FileDispositionInfo,
                &disposition as *const _ as *const _,
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        };
        if ok != 0 {
            drop(lock); // The verified file is deleted when its final handle closes.
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        if matches!(e.raw_os_error(), Some(32 | 33)) {
            let since = *first_conflict.get_or_insert_with(|| r.monotonic());
            if r.monotonic() - since < RELEASE_WAIT_SECONDS {
                r.sleep(RELEASE_RETRY_SECONDS).map_err(|e| {
                    format!(
                        "wait to mark owned lock {} for deletion: {e}",
                        path.display()
                    )
                })?;
                continue;
            }
        }
        return Err(format!(
            "mark owned lock {} for deletion: {e}",
            path.display()
        ));
    }
}

#[cfg(not(windows))]
fn delete_owned_handle(
    mut lock: OwnedLock,
    path: &Path,
    _r: &mut impl Runner,
    token: &str,
) -> Result<(), String> {
    // On Unix, reject a replaced path before unlinking. Windows uses the
    // handle operation above so replacement cannot race the final check.
    use std::os::unix::fs::MetadataExt;
    let opened = lock
        .file
        .metadata()
        .map_err(|e| format!("stat owned lock {}: {e}", path.display()))?;
    let current =
        fs::metadata(path).map_err(|e| format!("stat owned lock path {}: {e}", path.display()))?;
    if (opened.dev(), opened.ino()) != (current.dev(), current.ino()) {
        return Err(format!("owned lock path {} was replaced", path.display()));
    }
    lock.file
        .seek(SeekFrom::Start(0))
        .map_err(|e| format!("read owned lock {}: {e}", path.display()))?;
    if !file_is_owned(&mut lock.file, path, token)? {
        return Err(format!(
            "owned lock {} changed before unlink",
            path.display()
        ));
    }
    fs::remove_file(path).map_err(|e| format!("unlink owned lock {}: {e}", path.display()))
}
/// Only one live Civ6 run at a time across sessions and Codex jobs (single game instance).
pub fn acquire(
    r: &mut impl Runner,
    p: &Paths,
    owner: &Value,
    wait_s: f64,
    long_run: bool,
) -> Result<(), String> {
    let deadline = r.now() + wait_s;
    fs::create_dir_all(p.here.join("queue")).map_err(|e| e.to_string())?;
    let me = p.here.join("queue").join(queue_entry_name());
    let mut mine = owner.clone();
    mine["long"] = json!(i32::from(long_run));
    mine["since"] = json!(r.now());
    // A Unix claimant must publish its priority under the same persistent
    // guard that protects another claimant's my_turn check and lock creation.
    #[cfg(unix)]
    {
        let _guard = fairness::UnixGuard::acquire(&p.here.join("civ6.lock"))?;
        write_json(&me, &mine)?;
    }
    #[cfg(windows)]
    write_json(&me, &mine)?;
    let result = acquire_inner(r, p, owner, deadline, &me);
    #[cfg(unix)]
    let cleanup = {
        let _guard = fairness::UnixGuard::acquire(&p.here.join("civ6.lock"))?;
        remove_if_exists(&me)
    };
    #[cfg(windows)]
    let cleanup = remove_if_exists(&me);
    result.and(cleanup)
}
fn acquire_inner(
    r: &mut impl Runner,
    p: &Paths,
    owner: &Value,
    deadline: f64,
    me: &Path,
) -> Result<(), String> {
    let lock = p.here.join("civ6.lock");
    // ERROR_ACCESS_DENIED can mean a deleted lock still has a compatible
    // reader. Keep this much shorter than the ordinary six-hour queue wait:
    // an ACL denial must fail with its path and operation, not wait for hours.
    #[cfg(windows)]
    let mut access_denied_since = None;
    loop {
        #[cfg(unix)]
        let guard = fairness::UnixGuard::acquire(&lock)?;
        if !my_turn(r, p, me)? {
            #[cfg(windows)]
            {
                access_denied_since = None;
            }
            if r.now() > deadline {
                return Err("civ6.lock: gave up waiting behind higher-priority runs".into());
            }
            #[cfg(unix)]
            drop(guard);
            r.sleep(30.0)?;
            continue;
        }
        let mut opts = OpenOptions::new();
        opts.read(true).write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            opts.share_mode(1); // FILE_SHARE_READ: protect inspection and owner replacement.
        }
        match opts.clone().create_new(true).open(&lock) {
            Ok(mut fd) => {
                let owner = acquired_owner(r, owner);
                fd.write_all(python_json(&owner).as_bytes())
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let mut fd = match opts.open(&lock) {
                    Ok(fd) => fd,
                    Err(e) if e.kind() == ErrorKind::NotFound => continue,
                    #[cfg(windows)]
                    Err(e) if is_delete_pending_or_denied(&e) => {
                        wait_for_transient_open_denial(
                            r,
                            &lock,
                            &mut access_denied_since,
                            "open",
                            e,
                        )?;
                        continue;
                    }
                    // A competing claimant or the reaper owns the write handle.
                    #[cfg(windows)]
                    Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => {
                        access_denied_since = None;
                        if r.now() > deadline {
                            return Err("civ6.lock is being claimed by another process".into());
                        }
                        r.sleep(0.5)?;
                        continue;
                    }
                    Err(e) => return Err(e.to_string()),
                };
                #[cfg(windows)]
                {
                    access_denied_since = None;
                }
                let mut bytes = Vec::new();
                fd.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                let info = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));
                if !holder_alive(r, p, &info)? {
                    // Adopt in place under the protected handle: never delete a
                    // path another claimant could have replaced after our check.
                    fd.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
                    fd.set_len(0).map_err(|e| e.to_string())?;
                    let owner = acquired_owner(r, owner);
                    fd.write_all(python_json(&owner).as_bytes())
                        .and_then(|_| fd.sync_all())
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
                drop(fd);
                if r.now() > deadline {
                    return Err(format!("civ6.lock held by {}", python_repr(&info)));
                }
                r.print(&format!(
                    "waiting for civ6.lock held by {}",
                    python_repr(&info)
                ))?;
                #[cfg(unix)]
                drop(guard);
                r.sleep(60.0)?;
            }
            #[cfg(windows)]
            Err(e) if is_delete_pending_or_denied(&e) => {
                wait_for_transient_open_denial(r, &lock, &mut access_denied_since, "create", e)?;
            }
            #[cfg(windows)]
            Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => {
                access_denied_since = None;
                if r.now() > deadline {
                    return Err(format!("create civ6.lock {}: {e}", lock.display()));
                }
                r.sleep(0.5)?;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(windows)]
fn is_delete_pending_or_denied(error: &std::io::Error) -> bool {
    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_DELETE_PENDING};
    matches!(
        error.raw_os_error(),
        Some(code) if code == ERROR_ACCESS_DENIED as i32 || code == ERROR_DELETE_PENDING as i32
    )
}

#[cfg(windows)]
fn wait_for_transient_open_denial(
    r: &mut impl Runner,
    path: &Path,
    since: &mut Option<f64>,
    operation: &str,
    error: std::io::Error,
) -> Result<(), String> {
    let first = *since.get_or_insert_with(|| r.monotonic());
    if r.monotonic() - first >= RELEASE_WAIT_SECONDS {
        return Err(format!(
            "{operation} civ6.lock {} after bounded access-denied wait: {error}",
            path.display()
        ));
    }
    r.sleep(RELEASE_RETRY_SECONDS).map_err(|e| {
        format!(
            "wait to {operation} civ6.lock {} after access denied: {e}",
            path.display()
        )
    })
}
fn acquired_owner(r: &mut impl Runner, owner: &Value) -> Value {
    let mut owner = owner.clone();
    if owner["live_chain"] == true {
        owner["acquired_at"] = json!(r.now());
    }
    owner
}
fn python_repr(v: &Value) -> String {
    match v {
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("'{}': {}", k, python_repr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::String(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        _ => v.to_string(),
    }
}

pub fn main(r: &mut impl Runner, p: &Paths, a: &[String]) -> Result<i32, String> {
    let options = Options::parse(a)?;
    if options.help {
        r.print(HELP)?;
        return Ok(0);
    }
    if options.reliability {
        return cli(r, p, a);
    }
    if options.software_renderer {
        return Err("software rendering requires the standard owned cli lease".into());
    }
    main_owned(r, p, &options, None, None, &mut RunCleanup::default())
}

struct Reliability {
    revision: Option<String>,
    summary_path: std::path::PathBuf,
    summary: Value,
}
impl Reliability {
    fn round(&mut self) -> &mut Value {
        self.summary["rounds"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()
    }
    fn save(&self) -> Result<(), String> {
        write_json(&self.summary_path, &self.summary)
    }
}

/// Preserve the original tracked inputs alongside the prepared scenario.
fn source_manifest(
    r: &mut impl Runner,
    p: &Paths,
    scenario: &str,
    prepared: &str,
    deadline: f64,
) -> Result<Value, String> {
    let timeout = 30.0_f64.min(time_left(r, deadline)?);
    let out = r.run(
        Call::new(args(&[
            "git",
            "-C",
            &path(p.live()),
            "show",
            "--format=%H%n%T",
            "--no-patch",
            "HEAD",
        ]))
        .managed()
        .timeout(timeout),
    )?;
    let ids: Vec<_> = out
        .stdout
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if out.code != 0
        || ids.len() != 2
        || ids
            .iter()
            .any(|id| !matches!(id.len(), 40 | 64) || !id.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(format!(
            "could not identify prepared SOURCE (exit {}): {} {}",
            out.code, out.stdout, out.stderr
        ));
    }
    let (revision, tree) = (ids[0], ids[1]);
    let mut files = serde_json::Map::new();
    let scenario_key = format!("tools/civ6-autotest/scenarios/{scenario}");
    for relative in [
        driver::MANIFEST.to_owned(),
        "tools/Cargo.toml".into(),
        "tools/Cargo.lock".into(),
        "tools/civ6-autotest-rs/Cargo.lock".into(),
        scenario_key.clone(),
    ] {
        // Match ship::live: join the supplied path separately so an absolute
        // generated input replaces the base instead of becoming a filename.
        let file = if relative == scenario_key {
            p.live()
                .join("tools/civ6-autotest/scenarios")
                .join(scenario)
        } else {
            p.live().join(&relative)
        };
        let contents = match fs::read_to_string(&file) {
            Ok(text) => json!(text),
            Err(error) if error.kind() == ErrorKind::NotFound && relative != scenario_key => {
                Value::Null
            }
            Err(error) => return Err(format!("read SOURCE {}: {error}", file.display())),
        };
        files.insert(relative, contents);
    }
    Ok(
        json!({"revision":revision,"source_tree":tree,"worktree":path(p.live()),"files":files,
        "prepared_scenario_path":prepared,"prepared_scenario":read_json(prepared)?}),
    )
}

pub(crate) struct WithinDeadline<'a, R> {
    pub(crate) runner: &'a mut R,
    pub(crate) deadline: f64,
}
impl<R: Runner> Runner for WithinDeadline<'_, R> {
    fn terminate_game(
        &mut self,
        pid: u32,
        image: &str,
        created: u64,
        deadline: f64,
    ) -> Result<Output, String> {
        let deadline = deadline.min(self.deadline);
        time_left(self.runner, deadline)?;
        self.runner.terminate_game(pid, image, created, deadline)
    }
    fn process_identity(
        &mut self,
        pid: u32,
        timeout: f64,
    ) -> Result<Option<process::ProcessIdentity>, String> {
        let left = time_left(self.runner, self.deadline)?;
        let result = self.runner.process_identity(pid, timeout.min(left));
        time_left(self.runner, self.deadline)?;
        result
    }
    fn run(&mut self, mut call: Call) -> Result<Output, String> {
        let left = time_left(self.runner, self.deadline)?;
        call.timeout = Some(call.timeout.unwrap_or(left).min(left));
        call.deadline = Some(
            call.deadline
                .map_or(self.deadline, |deadline| deadline.min(self.deadline)),
        );
        self.runner.run(call)
    }
    fn now(&mut self) -> f64 {
        self.runner.now()
    }
    fn monotonic(&mut self) -> f64 {
        self.runner.monotonic()
    }
    fn sleep(&mut self, seconds: f64) -> Result<(), String> {
        let left = time_left(self.runner, self.deadline)?;
        if seconds >= left {
            self.runner.sleep(left)?;
            return Err("Civ6 scenario deadline exhausted while waiting".into());
        }
        self.runner.sleep(seconds)
    }
    fn print(&mut self, text: &str) -> Result<(), String> {
        self.runner.print(text)
    }
    fn error(&mut self, text: &str) -> Result<(), String> {
        self.runner.error(text)
    }
}

const HELP: &str = "Usage: live_chain [--test-mode] [--stop-on-failure] [--reliability] [--software-rendering] name:scenario.json [...]\n--reliability: run exactly one scenario 10 times, release the lock after each run, stop on any failure.\n--software-rendering: CPU renderer under the standard lease; restore original settings after game closure.\n--cleanup-only: restore original autosaves under the shared lock; requires Civ6 absent, no checkout or launch.\n-h, --help: show this help without acquiring the Civ6 lock.";
const CLEANUP_RESERVE_SECONDS: f64 = 240.0;
#[derive(Default)]
struct Options {
    items: Vec<String>,
    test_mode: bool,
    stop: bool,
    reliability: bool,
    help: bool,
    cleanup_only: bool,
    software_renderer: bool,
}
impl Options {
    fn parse(a: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        for arg in a {
            match arg.as_str() {
                "--help" | "-h" => options.help = true,
                "--test-mode" => options.test_mode = true,
                "--stop-on-failure" => options.stop = true,
                "--reliability" => options.reliability = true,
                "--cleanup-only" => options.cleanup_only = true,
                "--software-rendering" => options.software_renderer = true,
                _ if arg.starts_with('-') => return Err(format!("unknown argument: {arg}")),
                _ => {
                    let (name, scenario) =
                        arg.split_once(':').ok_or("expected name:scenario.json")?;
                    if name.is_empty()
                        || scenario.is_empty()
                        || name.trim() != name
                        || scenario.trim() != scenario
                    {
                        return Err(format!("expected nonempty name:scenario.json: {arg}"));
                    }
                    options.items.push(arg.clone());
                }
            }
        }
        if options.reliability && options.items.len() != 1 && !options.help {
            return Err("--reliability needs exactly one name:scenario.json".into());
        }
        if options.cleanup_only
            && (!options.items.is_empty()
                || options.reliability
                || options.test_mode
                || options.software_renderer
                || options.stop)
        {
            return Err("--cleanup-only takes no scenarios or other run flags".into());
        }
        if options.items.is_empty() && !options.help && !options.cleanup_only {
            return Err("at least one name:scenario.json is required; use --help for usage".into());
        }
        Ok(options)
    }
}

/// Reserve bounded cleanup time and account for a competitor already waiting.
/// A waiter arriving during the synchronous child run still waits at most one
/// scenario window (30 minutes including preparation and cleanup).
fn preparation_deadline(
    r: &mut impl Runner,
    p: &Paths,
    lease: Option<&process::LeaseLimit>,
) -> Result<f64, String> {
    let anchor = r.monotonic();
    let epoch = r.now();
    let mut deadline = anchor + fairness::LEASE_SECONDS - CLEANUP_RESERVE_SECONDS;
    if let Some(limit) = lease {
        let mut lock =
            fairness::Lock::open(p)?.ok_or("owned Civ6 lock unavailable before preparation")?;
        if lock.info["lock_token"].as_str() != Some(limit.token.as_str()) {
            return Err("Civ6 lock ownership changed before preparation".into());
        }
        lock.expired(r, p, &fairness::todo_path(p))?;
        if let Some(since) = lock.info["competitor_since"].as_f64() {
            let hard_deadline = anchor + fairness::LEASE_SECONDS - (epoch - since).max(0.0);
            limit.record_probe(&process::LeaseProbe::Contended(hard_deadline));
            deadline = deadline.min(limit.work_deadline().unwrap());
        }
    }
    if deadline <= r.monotonic() {
        return Err("Civ6 lease has insufficient time for another scenario and cleanup".into());
    }
    Ok(deadline)
}

fn time_left(r: &mut impl Runner, deadline: f64) -> Result<f64, String> {
    let remaining = deadline - r.monotonic();
    if remaining > 0.0 {
        Ok(remaining)
    } else {
        Err("Civ6 scenario deadline exhausted".into())
    }
}

#[derive(Default)]
struct RunCleanup {
    game_closed: bool,
    cleanup_refused_before_work: bool,
    deadline: Option<f64>,
    artifacts: Option<std::path::PathBuf>,
    software_profile: Option<crate::software_renderer::Profile>,
    renderer: Option<renderer_evidence::Run>,
}
fn pending_recovery_records(p: &Paths) -> Result<bool, String> {
    for name in [
        "reliability-auto-state.json",
        "steam-exit-recovery.json",
        "software-renderer-original.bin",
    ] {
        if p.here.join(name).try_exists().map_err(|error| {
            format!("inspect recovery records before unstarted lease release: {error}")
        })? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn main_owned(
    r: &mut impl Runner,
    p: &Paths,
    options: &Options,
    token: Option<&str>,
    mut reliability: Option<&mut Reliability>,
    cleanup: &mut RunCleanup,
) -> Result<i32, String> {
    let RunCleanup {
        game_closed,
        cleanup_refused_before_work,
        deadline: cleanup_deadline,
        artifacts: cleanup_artifacts,
        software_profile,
        renderer,
    } = cleanup;
    if options.cleanup_only {
        if civ6_running(r)? {
            // No close/restore helper was dispatched. Refusing an unrelated
            // running game is not a failed restoration of our original files.
            *cleanup_refused_before_work = true;
            return Err(
                "cleanup-only requires a closed Civ6; refusing to close an unrelated game".into(),
            );
        }
        let mut evidence = json!({});
        let closed = close_civ6_with_renderer(r, p, &mut evidence, None);
        *game_closed = close_cleanup_confirmed(&evidence);
        closed?;
        return Ok(0);
    }
    let mut first_failure = 0;
    for item in &options.items {
        *cleanup_artifacts = None;
        *game_closed = false;
        if let Some(token) = token {
            yield_if_expired(r, p, token)?;
        }
        let (name, scenario) = item.split_once(':').ok_or("expected name:scenario.json")?;
        // One lease spans preparation, the initial observation, the driver,
        // and cleanup. Never replace already known contention with a fresh lease.
        let lease = token.map(|token| process::LeaseLimit::new(p, token, CLEANUP_RESERVE_SECONDS));
        let deadline = preparation_deadline(r, p, lease.as_ref())?;
        let window = deadline - r.monotonic();
        *cleanup_deadline = Some(deadline + CLEANUP_RESERVE_SECONDS);
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["prepare_window_seconds"] = json!(window);
        }
        // Close while holding the acquired shared lock, before checkout can
        // touch mod files. Never close and replay a failed preparation.
        let protection = if let Some(token) = token {
            Some(
                open_close_protection(
                    &mut WithinDeadline {
                        runner: r,
                        deadline,
                    },
                    &p.here.join("civ6.lock"),
                    token,
                )?
                .ok_or("Civ6 lock ownership changed before preparation close")?,
            )
        } else {
            None
        };
        close_civ6_with_paths(
            &mut WithinDeadline {
                runner: r,
                deadline,
            },
            p,
        )?;
        if let Some(protection) = protection.as_ref() {
            protection.check(
                &mut WithinDeadline {
                    runner: r,
                    deadline,
                },
                &p.here.join("civ6.lock"),
            )?;
        }
        drop(protection);
        *game_closed = true;
        if software_profile.is_none() {
            if let Some(token) = token {
                // Recover an interrupted CPU run even for an ordinary next run.
                *software_profile = crate::software_renderer::Profile::recover(p, token)?;
                if let Some(profile) = software_profile.as_mut() {
                    profile
                        .restore()
                        .map_err(|e| format!("software renderer recovery pending: {e}"))?;
                    *software_profile = None;
                }
            }
        }
        let ship = std::env::var("CC_LIVE_SHIP").unwrap_or_else(|_| path(p.here.join("ship.exe")));
        if options.software_renderer && software_profile.is_none() {
            // The game was closed by this standard chain under its real lock.
            // Keep the original settings until final cleanup, including failed
            // preparation/driver calls. Do not use a copied chain or new lock.
            let token = token.ok_or("software rendering requires an owned lease")?;
            *software_profile = Some(crate::software_renderer::Profile::apply(p, token)?);
            r.print("CPU rendering configured: DeviceID=140 AllowSWRenderer=1; actual adapter requires Renderer.log evidence")?;
        }
        let prepare_args = args(&[&ship, "live", name, scenario]);
        let prepare_timeout = time_left(r, deadline)?.min(1200.0);
        let out = r
            .run(Call::new(prepare_args).timeout(prepare_timeout).managed())
            .map_err(|e| format!("prepare {name} {scenario} failed: {e}"))?;
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["prepare_exit_code"] = json!(windows_exit_code(out.code));
            state.round()["prepare_stderr"] = json!(out.stderr);
            state.round()["prepare_elapsed_seconds"] =
                json!((r.monotonic() - (deadline - window)).max(0.0));
        }
        let lines: Vec<_> = out.stdout.trim().lines().collect();
        if out.code != 0 || lines.len() < 2 {
            let code = if out.code == 0 { 1 } else { out.code };
            r.print(&format!(
                "RESULT {name} {scenario} prepare-failed {} stdout={} stderr={}",
                windows_exit_code(code),
                tail(out.stdout.trim(), 800),
                tail(out.stderr.trim(), 1600)
            ))?;
            if options.stop || options.reliability {
                return Ok(code);
            }
            if first_failure == 0 {
                first_failure = code;
            }
            // Preparation never launched the driver and the owned game is
            // still closed. Restore before the next preparation boundary.
            if let Some(profile) = software_profile.as_mut() {
                profile
                    .restore()
                    .map_err(|e| format!("software renderer restoration pending: {e}"))?;
                *software_profile = None;
            }
            continue;
        }
        let (live_json, artifacts) = (lines[lines.len() - 2], lines[lines.len() - 1]);
        let scenario_timeout = driver::scenario_timeout(Path::new(live_json))?;
        let auto_source = p
            .live()
            .join("tools/civ6-autotest-rs/src/bin/civ6-save-isolation.rs");
        if !auto_source.is_file() && p.live().join(".git").exists() {
            return Err("live checkout lacks autosave isolation source; refusing to run against original autosaves".into());
        }
        // Reserve ship's sequence directory before running the driver. This
        // works with the installed ship protocol (only two output lines) and
        // keeps its next save/load suffix unique, including failed spawns.
        let artifacts = if let Some(state) = reliability.as_deref_mut() {
            let id = state.round()["run_id"].as_str().unwrap().to_owned();
            let root = Path::new(artifacts);
            fs::create_dir_all(root.parent().ok_or("artifact root has no parent")?)
                .map_err(|e| e.to_string())?;
            fs::create_dir(root)
                .map_err(|e| format!("reserve reliability artifacts {}: {e}", root.display()))?;
            let artifacts = path(root.join(&id));
            state.round()["artifacts"] = json!(artifacts);
            let source = source_manifest(r, p, scenario, live_json, deadline)?;
            let source_path = root.join("source-manifest.json");
            write_json(&source_path, &source)?;
            let revision = source["revision"].as_str().unwrap().to_owned();
            let tree = source["source_tree"].as_str().unwrap().to_owned();
            state.round()["source"] = source;
            state.round()["source_manifest_path"] = json!(path(source_path));
            r.print(&format!("SOURCE {name} {scenario} {revision}"))?;
            if let Some(expected) = &state.revision {
                if *expected != tree {
                    return Err(format!("reliability SOURCE changed: expected tree {expected}, prepared {tree}; refusing another driver run"));
                }
            } else {
                state.revision = Some(tree.clone());
                state.summary["source_revision"] = json!(revision);
                state.summary["source_tree"] = json!(tree);
            }
            artifacts
        } else if auto_source.is_file() {
            // Reserve ship's sequence root for the autosave archive, while
            // leaving the driver's leaf absent for its CREATE_NEW contract.
            // RESULT continues to identify the actual driver output directory.
            let root = Path::new(artifacts);
            fs::create_dir_all(root.parent().ok_or("artifact root has no parent")?)
                .map_err(|e| e.to_string())?;
            fs::create_dir(root)
                .map_err(|e| format!("reserve ordinary artifacts {}: {e}", root.display()))?;
            path(root.join("run"))
        } else {
            artifacts.into()
        };
        *cleanup_artifacts = Some(Path::new(&artifacts).to_path_buf());
        r.print(&format!(
            "START {name} {scenario} civ6=closed-before-prepare"
        ))?;
        // Use the driver of the branch under test: main's driver has moved past the old driver-diplo worktree
        // (popup closers from #65, save dismissing the tutorial advisor from #64).
        let mut argv = driver::scenario(p, &p.live())?;
        argv.extend(args(&[
            live_json,
            "--no-auto-restart",
            "--output",
            &artifacts,
        ]));
        if options.test_mode {
            argv.push("--test-mode".into());
        }
        let timeout = scenario_timeout;
        if let Some(limit) = &lease {
            process::monitor_lease(r, limit)?;
            if let Some(deadline) = limit.work_deadline() {
                if r.monotonic() >= deadline {
                    return Err(format!(
                        "{}: waiting peer requires the slot before another driver run",
                        process::LEASE_YIELD
                    ));
                }
            }
        }
        let cleanup_end = r.monotonic() + timeout + CLEANUP_RESERVE_SECONDS;
        *cleanup_deadline = Some(
            lease
                .as_ref()
                .and_then(|limit| limit.hard_deadline())
                .map_or(cleanup_end, |deadline| cleanup_end.min(deadline)),
        );
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["scenario_budget_seconds"] = json!(scenario_timeout);
            state.round()["driver_timeout_seconds"] = json!(timeout);
        }
        let mut command = Call::new(argv)
            .env("CIV6_LIVE_CHAIN_PID", std::process::id().to_string())
            .env("CIV6_LIVE_LOCK_PATH", path(p.here.join("civ6.lock")))
            .merged()
            .managed()
            .timeout(timeout);
        command.lease = lease.clone();
        if auto_source.is_file() {
            // begin can move the original folder before returning an error.
            // Arm recovery before dispatch, including unknown/timeout outcomes.
            *game_closed = false;
            let auto = p
                .live()
                .join("tools/target/release/civ6-save-isolation.exe");
            let state = p.here.join("reliability-auto-state.json");
            let archive = Path::new(&artifacts)
                .parent()
                .ok_or("autosave isolation artifact parent missing")?;
            let scenario = read_json(live_json)?;
            let mut isolate = args(&[
                &path(auto),
                "--begin",
                "--state",
                &path(state),
                "--archive",
                &path(archive),
            ]);
            match scenario.get("session_type") {
                None => {}
                Some(session) if session == "single_player" => {}
                Some(session) if session == "hotseat" => {
                    isolate.extend(args(&["--session-type", "hotseat"]))
                }
                _ => return Err("unsupported autosave session; driver launch refused".into()),
            }
            let out = r.run(
                Call::new(isolate)
                    .env("CIV6_LIVE_CHAIN_PID", std::process::id().to_string())
                    .env("CIV6_LIVE_LOCK_PATH", path(p.here.join("civ6.lock")))
                    .managed()
                    .timeout(30.0),
            )?;
            if out.code != 0 {
                return Err(format!(
                    "owner autosave isolation failed: {} {}",
                    out.stdout, out.stderr
                ));
            }
            r.print("AUTOSAVE isolated: original files will not be overwritten")?;
        }
        let driver_started = r.monotonic();
        *game_closed = false;
        // Arm before dispatch: outer cleanup also preserves evidence if run
        // panics or any subsequent bookkeeping fails. Do not create the
        // driver's CREATE_NEW output directory until the child has returned.
        *renderer = token.map(|_| renderer_evidence::Run::new(Path::new(&artifacts)));
        let execution = r.run(command);
        retain_renderer(r, p, token, renderer.as_ref(), "driver-return");
        if let Some(deadline) = lease.as_ref().and_then(|limit| limit.hard_deadline()) {
            *cleanup_deadline = Some(cleanup_deadline.unwrap().min(deadline));
            if let Some(state) = reliability.as_deref_mut() {
                state.round()["contention_deadline_monotonic"] = json!(deadline);
            }
        }
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["driver_elapsed_seconds"] =
                json!((r.monotonic() - driver_started).max(0.0));
        }
        let run = match execution {
            Ok(run) => run,
            Err(error) => {
                if let Some(state) = reliability.as_deref_mut() {
                    state.round()["driver_error"] = json!(error);
                }
                let error = format!("driver {name} {scenario} failed (artifacts {artifacts}, deadline {timeout:.1}s): {error}");
                let logged = write_text(format!("{artifacts}.log"), &error);
                r.print(&format!(
                    "RESULT {name} {scenario} driver-error {artifacts}"
                ))?;
                return Err(match logged {
                    Ok(()) => error,
                    Err(log_error) => format!("{error}; log failed: {log_error}"),
                });
            }
        };
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["driver_exit_code"] = json!(windows_exit_code(run.code));
        }
        // A long-running Civ6 slows the PC down; close it as soon as each run ends (owner, 2026-10-01).
        write_text(
            format!("{artifacts}.log"),
            &format!("{}{}", run.stdout, run.stderr),
        )?;
        let mut exit_evidence = json!({});
        let closed = (|| -> Result<(), String> {
            let mut budget = WithinDeadline {
                runner: r,
                deadline: cleanup_deadline.unwrap(),
            };
            let protection = token
                .map(|token| {
                    open_close_protection(&mut budget, &p.here.join("civ6.lock"), token).and_then(
                        |p| p.ok_or_else(|| "lock ownership changed before driver close".into()),
                    )
                })
                .transpose()?;
            close_civ6_with_renderer(
                &mut budget,
                p,
                &mut exit_evidence,
                renderer
                    .as_ref()
                    .zip(token)
                    .map(|(run, token)| (run, token, "chain-before-close")),
            )?;
            if let Some(protection) = protection {
                protection.check(&mut budget, &p.here.join("civ6.lock"))?;
            }
            Ok(())
        })();
        retain_renderer(r, p, token, renderer.as_ref(), "after-close");
        let saved = save_exit_evidence(Path::new(&artifacts), &mut exit_evidence);
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["exit"] = exit_evidence;
        }
        *game_closed = closed.is_ok();
        exit_result(windows_exit_code(run.code), closed, saved)?;
        *renderer = None;
        if let Some(profile) = software_profile.as_mut() {
            profile
                .restore()
                .map_err(|e| format!("software renderer restoration pending: {e}"))?;
            *software_profile = None;
        }
        for line in run.stdout.lines().filter(|s| {
            ["[OK]", "[FAIL]", "PASS", "FAIL"]
                .iter()
                .any(|p| s.starts_with(p))
        }) {
            r.print(&format!("  {}", head(line, 300)))?;
        }
        r.print(&format!(
            "RESULT {name} {scenario} {} {artifacts}",
            windows_exit_code(run.code)
        ))?;
        if (options.stop || options.reliability) && run.code != 0 {
            r.print("CHAIN STOPPED ON FAILURE")?;
            return Ok(run.code);
        }
        if first_failure == 0 && run.code != 0 {
            first_failure = run.code;
        }
    }
    r.print(if first_failure == 0 {
        "CHAIN DONE"
    } else {
        "CHAIN FAILED"
    })?;
    Ok(first_failure)
}
/// Called only at a scenario boundary; never revoke a running chain from tick.
#[doc(hidden)]
pub fn exit_result(
    driver_code: u32,
    closed: Result<(), String>,
    saved: Result<(), String>,
) -> Result<(), String> {
    let mut failures = Vec::new();
    if let Err(error) = closed {
        failures.push(format!("Civ6 close failed: {error}"));
    }
    if let Err(error) = saved {
        failures.push(format!("evidence save failed: {error}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "driver exit {driver_code}; {}",
            failures.join("; ")
        ))
    }
}

#[doc(hidden)]
pub fn save_exit_evidence(artifacts: &Path, evidence: &mut Value) -> Result<(), String> {
    save_exit_evidence_named(
        artifacts,
        evidence,
        "steam-exit.jsonl",
        "exit-evidence.json",
    )
}

#[doc(hidden)]
pub fn save_cleanup_exit_evidence(artifacts: &Path, evidence: &mut Value) -> Result<(), String> {
    save_exit_evidence_named(
        artifacts,
        evidence,
        "steam-exit-cleanup.jsonl",
        "exit-evidence-cleanup.json",
    )
}

fn save_exit_evidence_named(
    artifacts: &Path,
    evidence: &mut Value,
    journal_name: &str,
    evidence_name: &str,
) -> Result<(), String> {
    let mut errors = Vec::new();
    if let Some(journal) = evidence["sync_journal"].as_str() {
        let target = artifacts.join(journal_name);
        let saved = (|| -> Result<(), String> {
            let mut source = File::open(journal).map_err(|e| format!("read exit journal: {e}"))?;
            fs::create_dir_all(artifacts).map_err(|e| format!("prepare exit artifacts: {e}"))?;
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| format!("reserve exit journal: {e}"))?;
            std::io::copy(&mut source, &mut out).map_err(|e| format!("copy exit journal: {e}"))?;
            out.sync_all()
                .map_err(|e| format!("sync exit journal: {e}"))
        })();
        match saved {
            Ok(()) => evidence["artifact_journal"] = json!(path(target)),
            Err(error) => {
                evidence["artifact_journal_error"] = json!(error);
                errors.push(error);
            }
        }
    }
    let saved = (|| -> Result<(), String> {
        fs::create_dir_all(artifacts).map_err(|e| format!("prepare exit artifacts: {e}"))?;
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(artifacts.join(evidence_name))
            .map_err(|e| format!("reserve exit evidence: {e}"))?;
        serde_json::to_writer_pretty(&mut out, evidence)
            .map_err(|e| format!("write exit evidence: {e}"))?;
        out.sync_all()
            .map_err(|e| format!("sync exit evidence: {e}"))
    })();
    if let Err(error) = saved {
        evidence["artifact_evidence_error"] = json!(error);
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub fn yield_if_expired(r: &mut impl Runner, p: &Paths, token: &str) -> Result<bool, String> {
    let path = p.here.join("civ6.lock");
    let mut first_conflict = None;
    let mut lock = loop {
        if let Some(lock) = fairness::Lock::open(p)? {
            break lock;
        }
        let since = *first_conflict.get_or_insert_with(|| r.monotonic());
        if r.monotonic() - since >= RELEASE_WAIT_SECONDS {
            return Err(format!(
                "open owned lock {} before scenario: unavailable",
                path.display()
            ));
        }
        r.sleep(RELEASE_RETRY_SECONDS).map_err(|e| {
            format!(
                "wait to open owned lock {} before scenario: {e}",
                path.display()
            )
        })?;
    };
    if lock.info["live_chain"] != true
        || lock.info["chain_pid"].as_u64() != Some(std::process::id() as u64)
        || lock.info["lock_token"].as_str() != Some(token)
    {
        return Err(format!(
            "civ6.lock ownership changed before scenario: {}",
            path.display()
        ));
    }
    if !lock.expired(r, p, &fairness::todo_path(p))? {
        return Ok(false);
    }
    let mut owner = lock.info.clone();
    owner.as_object_mut().unwrap().remove("competitor_since");
    owner["fifo"] = json!(true);
    // The current scenario has ended and its game has already been closed.
    drop(lock);
    let file = open_owned_lock(r, &path, token)?.ok_or_else(|| {
        format!(
            "civ6.lock ownership changed before yield: {}",
            path.display()
        )
    })?;
    delete_owned_handle(file, &path, r, token)?;
    r.print("Civ6 lease expired; re-queued behind waiting runs")?;
    owner["acquired_at"] = json!(r.now());
    acquire(r, p, &owner, 6. * 3600., false)?;
    Ok(true)
}
pub fn cli(r: &mut impl Runner, p: &Paths, a: &[String]) -> Result<i32, String> {
    // All syntax is checked before process inspection, queue registration or
    // lock acquisition. An invalid later item cannot launch an earlier one.
    let options = Options::parse(a)?;
    if options.help {
        r.print(HELP)?;
        return Ok(0);
    }
    ship::check_live_maintenance(p, std::env::var("CODEX_JOB_ID").ok().as_deref())?;
    if !options.reliability {
        return cli_once(r, p, &options, None);
    }
    let session = new_lock_token();
    let mut reliability = Reliability {
        revision: None,
        summary_path: p
            .temp
            .join(format!("live_chain_reliability_{session}.json")),
        summary: json!({"schema_version":1,"session":session,"scenario":options.items[0],
            "required_consecutive_successes":10,"consecutive_successes":0,"result":"running","rounds":[]}),
    };
    reliability.save()?;
    r.print(&format!(
        "RELIABILITY SUMMARY {}",
        path(&reliability.summary_path)
    ))?;
    for round in 1..=10 {
        let id = format!("reliability-{session}-{round:02}");
        reliability.summary["rounds"]
            .as_array_mut()
            .unwrap()
            .push(json!({"round":round,"run_id":id,"result":"running"}));
        r.print(&format!(
            "RELIABILITY START {round}/10 {}",
            options.items[0]
        ))?;
        let result = cli_once(r, p, &options, Some(&mut reliability));
        let code = match &result {
            Ok(code) => *code,
            Err(_) => 1,
        };
        reliability.round()["exit_code"] = json!(windows_exit_code(code));
        let yielded = result
            .as_ref()
            .err()
            .is_some_and(|e| e.contains(process::LEASE_YIELD));
        reliability.round()["result"] = json!(if code == 0 {
            "success"
        } else if yielded {
            "incomplete_shared_slot_yield"
        } else {
            "failed"
        });
        if let Err(error) = &result {
            reliability.round()["error"] = json!(error);
        }
        if code != 0 {
            reliability.summary["result"] = json!(if yielded {
                "incomplete_shared_slot_yield"
            } else {
                "failed"
            });
            let saved = reliability.save();
            r.print(&format!(
                "RELIABILITY FAILED {round}/10 exit={}",
                windows_exit_code(code)
            ))?;
            return match (result, saved) {
                (Err(error), Err(save)) => Err(format!("{error}; summary failed: {save}")),
                (_, Err(save)) => Err(save),
                (result, Ok(())) => result,
            };
        }
        reliability.summary["consecutive_successes"] = json!(round);
        reliability.summary["result"] = json!(if round == 10 { "success" } else { "running" });
        reliability.save()?;
        // Only a completed scenario AND successful cleanup/release count.
        r.print(&format!(
            "RELIABILITY SUCCESS {round}/10 {}",
            options.items[0]
        ))?;
    }
    r.print("RELIABILITY DONE 10/10")?;
    Ok(0)
}

/// Evidence failures are diagnostic only: preserve the driver's original exit
/// or error and keep cleanup running. Every caller is still inside its lease.
fn retain_renderer(
    r: &mut impl Runner,
    p: &Paths,
    token: Option<&str>,
    run: Option<&renderer_evidence::Run>,
    stage: &str,
) {
    let (Some(token), Some(run)) = (token, run) else {
        return;
    };
    let saved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let lock_path = p.here.join("civ6.lock");
        let mut lock = File::open(&lock_path).map_err(|e| e.to_string())?;
        if !file_is_owned(&mut lock, &lock_path, token)? {
            return Err("Renderer capture refused: shared lease ownership changed".into());
        }
        lock.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let owner: Value = serde_json::from_reader(&lock).map_err(|e| e.to_string())?;
        run.capture(r, p, &owner, stage)
    }))
    .unwrap_or_else(|_| Err("Renderer capture panicked".into()));
    if let Err(error) = saved {
        let _ = r.error(&format!("Renderer evidence {stage}: {error}"));
    }
}

fn cli_once(
    r: &mut impl Runner,
    p: &Paths,
    options: &Options,
    mut reliability: Option<&mut Reliability>,
) -> Result<i32, String> {
    process::begin_cleanup_cycle();
    let mut owner = std::env::var("CODEX_JOB_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|job| json!({"job":job}))
        .unwrap_or_else(|| json!({"pid":std::process::id()}));
    owner["live_chain"] = json!(true);
    owner["pid"] = json!(std::process::id());
    owner["chain_pid"] = json!(std::process::id());
    // Rejoin the existing priority queue at every reliability boundary;
    // preserve its short-run priority and aging rules.
    if reliability.is_some() {
        owner["fifo"] = json!(true);
    }
    #[cfg(windows)]
    {
        owner["pid_created_filetime"] = json!(process_created_filetime(std::process::id())?
            .ok_or("live_chain process disappeared before registration")?);
    }
    let token = new_lock_token();
    owner["lock_token"] = json!(token);
    acquire(r, p, &owner, 6.0 * 3600.0, is_long(&options.items))?;
    let mut cleanup_state = RunCleanup::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        main_owned(
            r,
            p,
            options,
            Some(&token),
            reliability.as_deref_mut(),
            &mut cleanup_state,
        )
    }))
    .unwrap_or_else(|_| Err("live chain panicked".into()));
    let RunCleanup {
        mut game_closed,
        cleanup_refused_before_work,
        deadline: cleanup_deadline,
        artifacts: cleanup_artifacts,
        mut software_profile,
        renderer,
    } = cleanup_state;
    process::clear_interrupt_for_cleanup();
    // Still inside the acquired lease, before CPU restoration or closing the
    // game. This is required when dispatch panicked before driver-return.
    retain_renderer(r, p, Some(&token), renderer.as_ref(), "outer-before-close");
    // Cleanup runs after a driver failure, under a guard compatible with every
    // SDK helper. DELETE is deferred until closure and restoration succeed.
    let path = p.here.join("civ6.lock");
    let mut lock_released = false;
    let cleanup_started = r.monotonic();
    let mut cleanup_exit = json!(null);
    let cleanup_end = cleanup_deadline.unwrap_or_else(|| r.monotonic() + CLEANUP_RESERVE_SECONDS);
    let cleanup_needed = !game_closed;
    let mut cleanup = (|| {
        let mut budget = WithinDeadline {
            runner: r,
            deadline: cleanup_end,
        };
        let r = &mut budget;
        let protection = open_close_protection(r, &path, &token)?
            .ok_or("lock ownership changed before cleanup")?;
        if options.cleanup_only && cleanup_refused_before_work && !pending_recovery_records(p)? {
            // Roll back only this unstarted acquisition. The original refusal
            // remains Err, and game_closed remains false for the live game.
            let released = protection.release(r, &path, &token);
            lock_released = released.is_ok();
            return released;
        }
        let mut diagnostic_error = None;
        // The profile denies DELETE access. Restore under its read protection
        // before opening the normal owned-delete handle, even after a failed
        // driver/panic where the owned game is still alive.
        if let Some(profile) = software_profile.as_mut() {
            if !game_closed {
                let closed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    close_civ6_with_renderer(
                        r,
                        p,
                        &mut cleanup_exit,
                        renderer
                            .as_ref()
                            .map(|run| (run, token.as_str(), "outer-chain-before-close")),
                    )
                }))
                .unwrap_or_else(|_| Err("Civ6 cleanup panicked".into()));
                retain_renderer(
                    r,
                    p,
                    Some(&token),
                    renderer.as_ref(),
                    "outer-profile-after-close",
                );
                if !close_cleanup_confirmed(&cleanup_exit) {
                    closed?;
                } else {
                    diagnostic_error = closed.err();
                }
                game_closed = true;
            }
            protection.check(r, &path)?;
            profile
                .restore()
                .map_err(|e| format!("software renderer restoration pending: {e}"))?;
            software_profile = None;
        }
        let closed = if options.cleanup_only {
            if game_closed {
                Ok(())
            } else {
                Err("cleanup-only did not close the running game; recovery is incomplete".into())
            }
        } else if game_closed {
            Ok(())
        } else {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match cleanup_deadline {
                Some(deadline) => close_civ6_with_renderer(
                    &mut WithinDeadline {
                        runner: r,
                        deadline,
                    },
                    p,
                    &mut cleanup_exit,
                    renderer
                        .as_ref()
                        .map(|run| (run, token.as_str(), "outer-chain-before-close")),
                ),
                None => close_civ6_with_renderer(
                    r,
                    p,
                    &mut cleanup_exit,
                    renderer
                        .as_ref()
                        .map(|run| (run, token.as_str(), "outer-chain-before-close")),
                ),
            }))
            .unwrap_or_else(|_| Err("Civ6 cleanup panicked".into()))
        };
        game_closed = closed.is_ok() || close_cleanup_confirmed(&cleanup_exit);
        if !game_closed {
            closed?;
        } else if let Err(error) = closed {
            diagnostic_error = Some(error);
        }
        protection.check(r, &path)?;
        let restored = if game_closed && !options.cleanup_only {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                deployment::restore(r, p, &p.live())
            }))
            .unwrap_or_else(|_| Err("ruleset restoration panicked".into()))
        } else {
            Ok(())
        };
        retain_renderer(r, p, Some(&token), renderer.as_ref(), "outer-after-close");
        restored?;
        let released = protection.release(r, &path, &token);
        lock_released = released.is_ok();
        released?;
        diagnostic_error.map_or(Ok(()), Err)
    })();
    if cleanup_needed {
        if let Some(artifacts) = cleanup_artifacts {
            cleanup_exit["primary_chain_error"] = json!(result.as_ref().err());
            cleanup_exit["cleanup_error"] = json!(cleanup.as_ref().err());
            if let Err(save) = save_cleanup_exit_evidence(&artifacts, &mut cleanup_exit) {
                cleanup = Err(match cleanup {
                    Ok(()) => format!("cleanup evidence save failed: {save}"),
                    Err(error) => format!("{error}; cleanup evidence save failed: {save}"),
                });
            }
        }
    }
    if software_profile.is_none() {
        process::finish_cleanup();
    }
    if let Some(state) = reliability {
        state.round()["cleanup"] = json!({"game_closed":game_closed,"lock_released":lock_released,
            "exit":cleanup_exit,
            "status":if cleanup.is_ok() { "success" } else { "failed" },"error":cleanup.as_ref().err(),
            "elapsed_seconds":(r.monotonic() - cleanup_started).max(0.0)});
    }
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; cleanup failed: {cleanup}")),
        (Ok(code), Err(cleanup)) if code != 0 => Err(format!(
            "chain failed (exit {}); cleanup failed: {cleanup}",
            windows_exit_code(code)
        )),
        (_, Err(cleanup)) => Err(cleanup),
        (result, Ok(())) => result,
    }
}
/// Python represents Windows process status as an unsigned DWORD.
pub fn windows_exit_code(code: i32) -> u32 {
    code as u32
}
/// Python sys.exit on Windows converts a unsigned child DWORD through signed C long.
/// Out-of-range values exit as -1; RESULT retains the unsigned child status.
pub fn python_cli_exit(code: i32) -> i32 {
    if code < 0 {
        -1
    } else {
        code
    }
}

#[cfg(test)]
mod source_manifest_tests {
    use super::*;

    struct GitOnly;
    impl Runner for GitOnly {
        fn run(&mut self, call: Call) -> Result<Output, String> {
            assert_eq!(call.args[0], "git");
            assert_eq!(call.args[3], "show");
            Ok(Output {
                stdout: format!("{}\n{}\n", "a".repeat(40), "b".repeat(40)),
                ..Output::default()
            })
        }
        fn now(&mut self) -> f64 {
            0.0
        }
        fn sleep(&mut self, _: f64) -> Result<(), String> {
            panic!("source capture must not sleep or operate a game")
        }
    }

    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn source_capture_reads_relative_and_absolute_generated_inputs_exactly() {
        let fixture =
            Fixture(std::env::temp_dir().join(format!("cc-live-source-{}", new_lock_token())));
        let paths = Paths {
            wt: fixture.0.join("wt"),
            ..Paths::default()
        };
        let directory = paths.live().join("tools/civ6-autotest/scenarios");
        fs::create_dir_all(&directory).unwrap();
        let prepared = fixture.0.join("prepared.json");
        fs::write(&prepared, r#"{"steps":[{"action":"pause","seconds":1}]}"#).unwrap();
        for (scenario, input) in [
            ("relative.json".to_owned(), directory.join("relative.json")),
            (
                path(fixture.0.join("生成 入力.json")),
                fixture.0.join("生成 入力.json"),
            ),
        ] {
            let text = "{\"name\":\"SHA固定の生成入力\",\"steps\":[]}\n";
            fs::write(&input, text).unwrap();
            let manifest =
                source_manifest(&mut GitOnly, &paths, &scenario, &path(&prepared), 30.0).unwrap();
            assert_eq!(
                manifest["files"][format!("tools/civ6-autotest/scenarios/{scenario}")],
                text
            );
            assert_eq!(manifest["prepared_scenario"]["steps"][0]["seconds"], 1);
            fs::remove_file(&input).unwrap();
            assert!(
                source_manifest(&mut GitOnly, &paths, &scenario, &path(&prepared), 30.0)
                    .unwrap_err()
                    .contains("read SOURCE")
            );
        }
    }
}

#[cfg(test)]
mod queue_entry_tests {
    use super::*;

    #[test]
    fn preparation_contention_survives_the_first_driver_observation_timeout() {
        struct Probe {
            mono: f64,
            budgets: Vec<f64>,
        }
        impl Runner for Probe {
            fn now(&mut self) -> f64 {
                self.mono + 1090.0
            }
            fn monotonic(&mut self) -> f64 {
                self.mono
            }
            fn run(&mut self, call: Call) -> Result<Output, String> {
                assert_eq!(call.args[0], "tasklist");
                let budget = call.timeout.unwrap();
                self.budgets.push(budget);
                self.mono += budget;
                Err("command timed out".into())
            }
            fn sleep(&mut self, _: f64) -> Result<(), String> {
                panic!("no wait expected")
            }
            fn error(&mut self, _: &str) -> Result<(), String> {
                Ok(())
            }
        }
        let root = std::env::temp_dir().join(format!("cc-live-prepare-{}", new_lock_token()));
        let p = Paths {
            here: root.join("here"),
            jobs: root.join("jobs"),
            wt: root.join("wt"),
            temp: root.join("temp"),
            profile: root.join("user"),
            python: "unused".into(),
        };
        fs::create_dir_all(p.here.join("queue")).unwrap();
        fs::create_dir_all(p.jobs.join("waiter")).unwrap();
        write_json(
            p.here.join("civ6.lock"),
            &json!({"lock_token":"owned","job":"owner","acquired_at":1000}),
        )
        .unwrap();
        write_json(p.jobs.join("waiter/job.json"), &json!({"status":"running"})).unwrap();
        let queue = p.here.join("queue/waiter.json");
        write_json(&queue, &json!({"job":"waiter","since":1000})).unwrap();
        let limit = process::LeaseLimit::new(&p, "owned", CLEANUP_RESERVE_SECONDS);
        let mut r = Probe {
            mono: 10.0,
            budgets: vec![],
        };
        assert_eq!(
            preparation_deadline(&mut r, &p, Some(&limit)).unwrap(),
            1470.0
        );
        assert_eq!(limit.hard_deadline(), Some(1710.0));
        // Preparation used nearly all its window; the first monitor must use
        // this prior absolute deadline even when its peer query is unknown.
        r.mono = 1469.0;
        write_json(queue, &json!({"pid":4321,"since":1000})).unwrap();
        let driver_limit = limit.clone();
        assert!(process::monitor_lease(&mut r, &driver_limit)
            .unwrap_err()
            .contains(process::LEASE_YIELD));
        assert_eq!(r.budgets, vec![1.0]);
        assert_eq!(r.mono, 1470.0);
        assert_eq!(driver_limit.hard_deadline(), Some(1710.0));
        // Changes made by the driver's monitor must remain visible to cleanup.
        assert_eq!(
            limit.observation_deadline(),
            driver_limit.observation_deadline()
        );
        assert!(limit.observation_deadline().is_some());
        driver_limit.record_probe(&process::LeaseProbe::Clear);
        assert_eq!(limit.hard_deadline(), None);
        assert_eq!(limit.observation_deadline(), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_pid_registrations_have_distinct_paths_and_old_cleanup_keeps_new_entry() {
        let root = std::env::temp_dir().join(format!("cc-live-queue-{}", new_lock_token()));
        fs::create_dir_all(&root).unwrap();
        let old = root.join(queue_entry_name());
        let new = root.join(queue_entry_name());
        assert_ne!(old, new);
        fs::write(&old, "old").unwrap();
        fs::write(&new, "new").unwrap();
        fs::remove_file(&old).unwrap();
        assert_eq!(fs::read_to_string(&new).unwrap(), "new");
        fs::remove_file(new).unwrap();
        fs::remove_dir(root).unwrap();
    }
}

#[cfg(all(test, unix))]
mod unix_guard_tests {
    use super::*;
    use std::{
        fs::TryLockError,
        sync::mpsc,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: std::path::PathBuf,
        paths: Paths,
    }
    impl Fixture {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root =
                std::env::temp_dir().join(format!("cc-live-unix-{}-{unique}", std::process::id()));
            let here = root.join("here");
            fs::create_dir_all(&here).unwrap();
            let paths = Paths {
                here,
                wt: root.join("wt"),
                jobs: root.join("jobs"),
                temp: root.join("temp"),
                profile: root.join("profile"),
                python: "unused-test-value".into(),
            };
            Self { root, paths }
        }
        fn lock(&self) -> std::path::PathBuf {
            self.paths.here.join("civ6.lock")
        }
        fn guard(&self) -> std::path::PathBuf {
            self.paths.here.join("civ6.lock.guard")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    struct RunnerWithWriter {
        lock: std::path::PathBuf,
        guard: std::path::PathBuf,
        writer: Option<thread::JoinHandle<()>>,
        kills: usize,
    }
    impl Runner for RunnerWithWriter {
        fn run(&mut self, call: Call) -> Result<Output, String> {
            if call.args[0] == "tasklist"
                && call.args[2] == "IMAGENAME eq CivilizationVI.exe"
                && self.kills == 0
            {
                return Ok(Output {
                    stdout: "\"CivilizationVI.exe\",\"42\"".into(),
                    ..Output::default()
                });
            }
            if call.args[0] == "taskkill" {
                self.kills += 1;
                if self.writer.is_none() {
                    let probe = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&self.guard)
                        .unwrap();
                    assert!(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
                    let lock = self.lock.clone();
                    let guard = self.guard.clone();
                    let (attempted, received) = mpsc::channel();
                    self.writer = Some(thread::spawn(move || {
                        let probe = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .open(guard)
                            .unwrap();
                        attempted
                            .send(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)))
                            .unwrap();
                        let _guard = fairness::UnixGuard::acquire(&lock).unwrap();
                        write_json(&lock, &json!({"live_chain":true,"lock_token":"peer"})).unwrap();
                    }));
                    assert!(received.recv_timeout(Duration::from_secs(2)).unwrap());
                }
            }
            Ok(Output::default())
        }
        fn now(&mut self) -> f64 {
            1000.0
        }
        fn sleep(&mut self, _: f64) -> Result<(), String> {
            panic!("unexpected wait")
        }
        fn print(&mut self, _: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn cleanup_holds_guard_until_own_game_is_closed_and_lock_unlinked() {
        let f = Fixture::new();
        let mut r = RunnerWithWriter {
            lock: f.lock(),
            guard: f.guard(),
            writer: None,
            kills: 0,
        };
        assert_eq!(
            cli(
                &mut r,
                &f.paths,
                &args(&["fixture:s.json", "--stop-on-failure"])
            )
            .unwrap(),
            1
        );
        r.writer.take().unwrap().join().unwrap();
        assert_eq!(r.kills, 2);
        assert_eq!(read_json(f.lock()).unwrap()["lock_token"], "peer");
        assert!(f.guard().exists());
    }

    #[test]
    fn acquisition_and_fairness_updates_use_the_same_persistent_guard() {
        struct Inspecting {
            guard: std::path::PathBuf,
        }
        impl Runner for Inspecting {
            fn run(&mut self, _: Call) -> Result<Output, String> {
                let probe = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&self.guard)
                    .unwrap();
                assert!(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
                Ok(Output::default())
            }
            fn now(&mut self) -> f64 {
                1000.0
            }
            fn sleep(&mut self, _: f64) -> Result<(), String> {
                panic!("unexpected wait")
            }
            fn print(&mut self, _: &str) -> Result<(), String> {
                Ok(())
            }
        }
        let f = Fixture::new();
        write_json(f.lock(), &json!({"pid":42})).unwrap();
        acquire(
            &mut Inspecting { guard: f.guard() },
            &f.paths,
            &json!({"pid":77}),
            1.0,
            false,
        )
        .unwrap();
        assert_eq!(read_json(f.lock()).unwrap(), json!({"pid":77}));
        let mut lock = fairness::Lock::open(&f.paths).unwrap().unwrap();
        let probe = OpenOptions::new()
            .read(true)
            .write(true)
            .open(f.guard())
            .unwrap();
        assert!(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
        lock.info["lease_probe"] = json!(true);
        lock.save().unwrap();
        drop(lock);
        probe.try_lock().unwrap();
        assert_eq!(read_json(f.lock()).unwrap()["lease_probe"], true);
    }

    #[test]
    fn queue_registration_waits_for_the_guard_before_publishing_priority() {
        struct QueueRunner {
            clock_reads: usize,
            ready: mpsc::Sender<()>,
            proceed: mpsc::Receiver<()>,
        }
        impl Runner for QueueRunner {
            fn run(&mut self, _: Call) -> Result<Output, String> {
                panic!("no process query is needed for an empty queue")
            }
            fn now(&mut self) -> f64 {
                self.clock_reads += 1;
                if self.clock_reads == 2 {
                    self.ready.send(()).unwrap();
                    self.proceed.recv().unwrap();
                }
                1000.0
            }
            fn sleep(&mut self, _: f64) -> Result<(), String> {
                panic!("an empty queue must not sleep")
            }
            fn print(&mut self, _: &str) -> Result<(), String> {
                Ok(())
            }
        }

        let f = Fixture::new();
        let guard = fairness::UnixGuard::acquire(&f.lock()).unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let paths = f.paths.clone();
        let worker = thread::spawn(move || {
            acquire(
                &mut QueueRunner {
                    clock_reads: 0,
                    ready: ready_tx,
                    proceed: proceed_rx,
                },
                &paths,
                &json!({"pid":77}),
                1.0,
                false,
            )
        });
        ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        proceed_tx.send(()).unwrap();
        let queue = f.paths.here.join("queue");
        thread::sleep(Duration::from_millis(100));
        assert_eq!(
            fs::read_dir(&queue).unwrap().count(),
            0,
            "priority was published outside the guard"
        );
        drop(guard);
        worker.join().unwrap().unwrap();
        assert_eq!(read_json(f.lock()).unwrap(), json!({"pid":77}));
        assert!(
            fs::read_dir(&queue).unwrap().next().is_none(),
            "successful claim must remove its queue entry"
        );
        assert!(f.guard().exists(), "guard inode must remain persistent");
    }
}
