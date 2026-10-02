use super::*;
use crate::startup_idle::{self, DisplayInput, IdleProof};
use crate::{click, time};
#[path = "startup_pdh.rs"]
mod pdh;
use std::{
    ffi::OsStr,
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle, process::CommandExt},
    path::PathBuf,
    process::{Child, Command, Stdio},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, FILETIME, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::{
            Debug::{
                MiniDumpFilterModulePaths, MiniDumpNormal, MiniDumpWithThreadInfo,
                MiniDumpWriteDump,
            },
            ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
        },
        Threading::{GetProcessTimes, OpenProcess, WaitForSingleObject},
    },
    UI::{
        Input::KeyboardAndMouse::{keybd_event, GetAsyncKeyState},
        WindowsAndMessaging::GetWindowThreadProcessId,
    },
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn open_process(pid: u32, access: u32) -> Result<Handle> {
    let handle = unsafe { OpenProcess(access, 0, pid) };
    if handle.is_null() {
        return Err(Error::input(format!("cannot inspect process {pid}")));
    }
    Ok(Handle(handle))
}
fn created(handle: HANDLE) -> Result<u64> {
    let (mut start, mut exit, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    if unsafe { GetProcessTimes(handle, &mut start, &mut exit, &mut kernel, &mut user) } == 0 {
        return Err(Error::input("process creation time unavailable"));
    }
    Ok((u64::from(start.dwHighDateTime) << 32) | u64::from(start.dwLowDateTime))
}
fn unix_start(handle: HANDLE) -> Result<f64> {
    Ok(created(handle)? as f64 / 10_000_000.0 - 11_644_473_600.0)
}

fn process_rows() -> Result<Vec<crate::restart::ProcessRow>> {
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(Error::input("process snapshot unavailable"));
    }
    let _handle = Handle(handle);
    let mut row = PROCESSENTRY32W::default();
    row.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut more = unsafe { Process32FirstW(handle, &mut row) } != 0;
    let mut rows = Vec::new();
    while more {
        let end = row
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(row.szExeFile.len());
        rows.push(crate::restart::ProcessRow {
            pid: row.th32ProcessID,
            parent_pid: row.th32ParentProcessID,
            name: String::from_utf16_lossy(&row.szExeFile[..end]),
        });
        more = unsafe { Process32NextW(handle, &mut row) } != 0;
    }
    if unsafe { GetLastError() } != 18 {
        return Err(Error::input("incomplete process snapshot"));
    }
    Ok(rows)
}

/// Pin a read-only Windows handle which denies writes/deletion until recovery ends.
/// This never acquires, modifies, replaces or releases the chain's lock record.
pub struct RecoveryLease {
    file: File,
    chain_pid: u32,
    chain_created: u64,
}
impl RecoveryLease {
    pub fn acquire() -> Result<Self> {
        let chain_pid = std::env::var(CHAIN_PID_ENV)
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|pid| *pid != 0)
            .ok_or_else(|| Error::input("live-chain owner PID was not supplied"))?;
        let path = std::env::var_os(LOCK_PATH_ENV)
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| Error::input("absolute shared Civ6 lock path was not supplied"))?;
        let file = OpenOptions::new().read(true).share_mode(1).open(path)?; // FILE_SHARE_READ only
        let owner = open_process(chain_pid, 0x101000)?;
        let mut lease = Self {
            file,
            chain_pid,
            chain_created: created(owner.0)?,
        };
        lease.check()?;
        Ok(lease)
    }
    pub fn check(&mut self) -> Result<()> {
        time::check_interrupt()?;
        self.file.seek(SeekFrom::Start(0))?;
        let mut record = Vec::new();
        (&mut self.file).take(32769).read_to_end(&mut record)?;
        if record.len() > 32768 {
            return Err(Error::input("shared Civ6 lock record too large"));
        }
        verify_lock_record(&serde_json::from_slice(&record)?, self.chain_pid)?;
        let owner = open_process(self.chain_pid, 0x101000)?;
        if created(owner.0)? != self.chain_created
            || unsafe { WaitForSingleObject(owner.0, 0) } != 258
        {
            return Err(Error::input("live-chain lock owner exited or changed"));
        }
        let image = click::image_path(owner.0 as isize)?;
        if !image
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .eq_ignore_ascii_case("live_chain.exe")
        {
            return Err(Error::input("lock owner is not live_chain.exe"));
        }
        let rows = process_rows()?;
        let mut child = std::process::id();
        for _ in 0..32 {
            if child == self.chain_pid {
                return Ok(());
            }
            let row = rows
                .iter()
                .find(|row| row.pid == child)
                .ok_or_else(|| Error::input("live-chain ancestry is unknown"))?;
            let child_handle = open_process(child, 0x101000)?;
            let parent_handle = open_process(row.parent_pid, 0x101000)?;
            if created(parent_handle.0)? > created(child_handle.0)? {
                return Err(Error::input("live-chain ancestor PID was reused"));
            }
            child = row.parent_pid;
        }
        Err(Error::input(
            "driver is not a descendant of the shared lock owner",
        ))
    }
}

fn pinned(game: GameProcess, access: u32) -> Result<Handle> {
    if crate::restart::single_civ_process(0.0)? != Some((game.pid, game.started)) {
        return Err(Error::input(
            "just-launched Civ6 process disappeared or changed",
        ));
    }
    let handle = open_process(game.pid, access | 0x101000)?;
    if unix_start(handle.0)? != game.started || unsafe { WaitForSingleObject(handle.0, 0) } != 258 {
        return Err(Error::input("just-launched Civ6 creation time changed"));
    }
    let image = click::image_path(handle.0 as isize)?;
    if !image
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .eq_ignore_ascii_case("CivilizationVI.exe")
    {
        return Err(Error::input(
            "just-launched process is not the DX11 Civ6 executable",
        ));
    }
    Ok(handle)
}
fn own_window(game: GameProcess) -> Result<isize> {
    let hwnd = click::find_civ_window()?;
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd as HANDLE, &mut pid);
    }
    if pid != game.pid {
        return Err(Error::input(
            "startup window belongs to a different process",
        ));
    }
    crate::restart::require_dx11_window(hwnd)?;
    Ok(hwnd)
}
pub struct PinnedGame {
    pub game: GameProcess,
    handle: isize,
}
impl Drop for PinnedGame {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle as HANDLE);
        }
    }
}
impl PinnedGame {
    pub fn check_remaining(&self) -> Result<()> {
        let ids = click::civ_process_ids()?;
        if ids.len() > 1
            || ids.iter().any(|pid| *pid != self.game.pid)
            || unix_start(self.handle as HANDLE)? != self.game.started
            || (ids.contains(&self.game.pid)
                && unsafe { WaitForSingleObject(self.handle as HANDLE, 0) } != 258)
        {
            return Err(Error::input(
                "pinned startup process changed or another Civ6 appeared",
            ));
        }
        Ok(())
    }
}
pub fn pin_window_process(hwnd: isize, floor: f64) -> Result<PinnedGame> {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd as HANDLE, &mut pid);
    }
    if pid == 0 {
        return Err(Error::input(
            "startup window disappeared before process pinning",
        ));
    }
    let handle = open_process(pid, 0x101000)?;
    let game = GameProcess {
        pid,
        started: unix_start(handle.0)?,
    };
    if game.started < floor {
        return Err(Error::input(
            "startup window process predates this Steam launch",
        ));
    }
    let _identity = pinned(game, 0x1000)?;
    let mut current = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd as HANDLE, &mut current);
    }
    if current != pid {
        return Err(Error::input(
            "startup window PID changed before process pinning",
        ));
    }
    let raw = handle.0 as isize;
    std::mem::forget(handle); // Ownership transferred to the process pin, including during pending termination.
    Ok(PinnedGame { game, handle: raw })
}
pub fn loading_evidence(
    game: GameProcess,
    logs: &Path,
    port: u16,
    session: &mut DiagnosticSession,
) -> Result<Value> {
    let _game = pinned(game, 0x1000)?;
    let window = own_window(game)?;
    if port != 4318 {
        return Err(Error::input(
            "startup recovery requires the configured FireTuner port 4318",
        ));
    }
    let options = logs
        .parent()
        .ok_or_else(|| Error::input("startup options path unavailable"))?
        .join("AppOptions.txt");
    let mut bytes = Vec::new();
    File::open(options)?
        .take(256 * 1024)
        .read_to_end(&mut bytes)?;
    if !tuner_enabled(&String::from_utf8_lossy(&bytes)) {
        return Err(Error::input(
            "FireTuner configuration is disabled, ambiguous or unreadable",
        ));
    }
    // Current renderer activity is supporting evidence; Startup.log's last line is irrelevant.
    let renderer = logs.join("Renderer.log");
    let modified = fs::metadata(&renderer)?
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::input("renderer timestamp unavailable"))?
        .as_secs_f64();
    let mut bytes = Vec::new();
    File::open(renderer)?
        .take(LOG_LIMIT)
        .read_to_end(&mut bytes)?;
    if modified < game.started
        || !String::from_utf8_lossy(&bytes).contains("Renderer::CreateD3DDevice()")
    {
        return Err(Error::input(
            "current DX11 renderer startup evidence is missing",
        ));
    }
    let image_path =
        std::env::temp_dir().join(format!("civ6-loading-{}.json", uuid::Uuid::new_v4()));
    struct Remove(PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
            let _ = fs::remove_file(self.0.with_extension("png"));
        }
    }
    let _remove = Remove(image_path.clone());
    let captured = worker(
        &format!("loading-screen@{window}"),
        game,
        &image_path,
        session,
    )?;
    if captured["result"]["status"] != "ok" {
        return Err(Error::input("current loading screen could not be observed"));
    }
    let mut bytes = Vec::new();
    File::open(&image_path)?
        .take(4097)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(Error::input("loading screen evidence exceeded size limit"));
    }
    let screen: Value = serde_json::from_slice(&bytes)?;
    verify_capture_window(window, own_window(game)?, &screen)?;
    if screen["status"] != "recognized" {
        return Err(Error::input(
            "displayed image is not a confirmed initial loading screen",
        ));
    }
    pinned(game, 0x1000)?;
    Ok(
        json!({"process_verified":true,"window_verified":true,"tuner_enabled":true,
        "renderer_current":true,"classification":"new-process loading wait expired before FrontEnd","screen":screen}),
    )
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.try_wait();
        }
    }
}
#[derive(Default)]
pub struct DiagnosticSession {
    // Keep the original OS handles until completion; never reopen an unconfirmed PID.
    pending: Vec<(OwnedChild, bool)>,
}
impl DiagnosticSession {
    fn before_reset(&mut self) -> Result<()> {
        let mut unknown = false;
        self.pending
            .retain_mut(|(child, _)| match child.0.try_wait() {
                Ok(Some(_)) => false,
                Ok(None) => true,
                Err(_) => {
                    unknown = true;
                    true
                }
            });
        if unknown {
            return Err(Error::input("diagnostic child exit could not be inspected"));
        }
        if self.pending.iter().any(|(_, gpu_probe)| !gpu_probe) {
            return Err(Error::input(
                "diagnostic writer or helper exit is unconfirmed; display recovery refused",
            ));
        }
        // A terminating, owned, read-only NVIDIA probe can itself be stuck in the
        // graphics stack. It remains pinned and must finish AFTER safe recovery.
        Ok(())
    }
    pub fn finish(&mut self) -> Result<Value> {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let mut unknown = false;
            self.pending
                .retain_mut(|(child, _)| match child.0.try_wait() {
                    Ok(Some(_)) => false,
                    Ok(None) => true,
                    Err(_) => {
                        unknown = true;
                        true
                    }
                });
            if unknown {
                return Err(Error::input("diagnostic child exit could not be inspected"));
            }
            if self.pending.is_empty() {
                return Ok(json!({"status":"all_exited"}));
            }
            if Instant::now() >= end {
                return Err(Error::input(
                    "owned diagnostic children did not exit; refusing another launch",
                ));
            }
            time::check_interrupt()?;
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn stop_child(
    mut child: OwnedChild,
    session: &mut DiagnosticSession,
    gpu_probe: bool,
) -> Result<Value> {
    let pid = child.0.id();
    if child.0.kill().is_err() {
        if matches!(child.0.try_wait(), Ok(Some(_))) {
            return Ok(json!({"exit_confirmed":true,"pid":pid}));
        }
        session.pending.push((child, false));
        return Err(Error::input(
            "diagnostic child termination could not be requested",
        ));
    }
    let end = Instant::now() + Duration::from_millis(500);
    while Instant::now() < end {
        match child.0.try_wait() {
            Ok(Some(_)) => return Ok(json!({"exit_confirmed":true,"pid":pid})),
            Ok(None) => {}
            Err(error) => {
                session.pending.push((child, false));
                return Err(error.into());
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // Move the same handle into the session, rather than discarding ownership.
    session.pending.push((child, gpu_probe));
    Ok(
        json!({"exit_confirmed":false,"termination_requested":true,"pid":pid,"owned_gpu_probe":gpu_probe}),
    )
}

/// No pipes to deadlock, no shell, and only this call's child is ever terminated.
fn bounded_command(
    mut command: Command,
    timeout: Duration,
    session: &mut DiagnosticSession,
    gpu_probe: bool,
) -> Result<Value> {
    command.creation_flags(0x08000000).stdin(Stdio::null()); // CREATE_NO_WINDOW
    let started = Instant::now();
    let mut child = OwnedChild(command.spawn()?);
    loop {
        let exit = match child.0.try_wait() {
            Ok(exit) => exit,
            Err(error) => {
                let _ = stop_child(child, session, false);
                return Err(error.into());
            }
        };
        if let Some(exit) = exit {
            return Ok(json!({"status":if exit.success() {"ok"} else {"failed"},
                "exit_code":exit.code(),"elapsed_ms":started.elapsed().as_millis()}));
        }
        if started.elapsed() >= timeout {
            let termination = stop_child(child, session, gpu_probe)?;
            return Ok(
                json!({"status":"timeout","termination":termination,"elapsed_ms":started.elapsed().as_millis()}),
            );
        }
        if let Err(error) = time::check_interrupt() {
            let _ = stop_child(child, session, false);
            return Err(error);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn gpu_probe(output: &Path, session: &mut DiagnosticSession) -> Result<GpuSnapshot> {
    let result = (|| -> Result<GpuSnapshot> {
        let system =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:/Windows".into()));
        let program = PathBuf::from(
            std::env::var_os("ProgramW6432").unwrap_or_else(|| "C:/Program Files".into()),
        );
        let executable = [
            system.join("System32/nvidia-smi.exe"),
            program.join("NVIDIA Corporation/NVSMI/nvidia-smi.exe"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| Error::input("installed nvidia-smi is unavailable"))?;
        // Retain partial output if a terminating GPU probe cannot finish until recovery.
        let path = output.join("nvidia-state.xml");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut command = Command::new(executable);
        command
            .args(["-q", "-x"])
            .stdout(Stdio::from(file))
            .stderr(Stdio::null());
        let status = bounded_command(command, Duration::from_secs(3), session, true)?;
        if status["status"] != "ok" {
            return Ok(GpuSnapshot {
                status: status["status"].as_str().unwrap_or("unknown").into(),
                elapsed_ms: status["elapsed_ms"].as_u64().unwrap_or(0),
                detail: "bounded nvidia-smi query failed; independent PDH safety proof is required"
                    .into(),
                ..Default::default()
            });
        }
        let mut bytes = Vec::new();
        File::open(&path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err(Error::input("GPU probe output exceeded size limit"));
        }
        let mut gpu = parse_nvidia_xml(&String::from_utf8_lossy(&bytes))?;
        gpu.elapsed_ms = status["elapsed_ms"].as_u64().unwrap_or(0);
        Ok(gpu)
    })();
    match result {
        Err(e) if e.kind == crate::ErrorKind::Interrupted => Err(e),
        other => Ok(other.unwrap_or_else(|e| GpuSnapshot {
            status: "unknown".into(),
            detail: e.to_string(),
            ..Default::default()
        })),
    }
}

fn invoke_worker(
    executable: &Path,
    kind: &str,
    game: GameProcess,
    output: &Path,
    session: &mut DiagnosticSession,
) -> Result<Value> {
    let mut command = Command::new(executable);
    command
        .arg(kind)
        .arg(game.pid.to_string())
        .arg(game.started.to_string())
        .arg(output)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    bounded_command(
        command,
        Duration::from_secs(if kind == "gpu-idle" { 6 } else { 5 }),
        session,
        false,
    )
}
fn worker(
    kind: &str,
    game: GameProcess,
    output: &Path,
    session: &mut DiagnosticSession,
) -> Result<Value> {
    let result = (|| -> Result<Value> {
        let executable = std::env::current_exe()?.with_file_name("civ6-startup-diagnostics.exe");
        if !executable.is_file() {
            return Err(Error::input("native diagnostic worker was not built"));
        }
        let _binary = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&executable)?;
        let identity_path =
            std::env::temp_dir().join(format!("civ6-helper-{}.json", uuid::Uuid::new_v4()));
        struct Remove(PathBuf);
        impl Drop for Remove {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }
        let _remove = Remove(identity_path.clone());
        let identity = invoke_worker(&executable, "identity", game, &identity_path, session)?;
        if identity["status"] != "ok" {
            return Err(Error::input("diagnostic helper identity check failed"));
        }
        let mut bytes = Vec::new();
        File::open(&identity_path)?
            .take(4097)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 4096 {
            return Err(Error::input(
                "diagnostic helper identity exceeded size limit",
            ));
        }
        verify_helper_identity(&serde_json::from_slice(&bytes)?)?;
        let status = invoke_worker(&executable, kind, game, output, session)?;
        Ok(
            json!({"operation":kind,"complete":status["status"]=="ok","result":status,"artifact":output.file_name().map(OsStr::to_string_lossy),
                "bytes":fs::metadata(output).ok().map(|m|m.len())}),
        )
    })();
    match result {Err(e) if e.kind==crate::ErrorKind::Interrupted=>Err(e),
        other=>Ok(other.unwrap_or_else(|e|json!({"operation":kind,"status":"unavailable","complete":false,"reason":e.to_string()})))}
}

pub fn collect_diagnostics(
    game: GameProcess,
    output: &Path,
    session: &mut DiagnosticSession,
) -> Result<Value> {
    let _game = pinned(game, 0x1000)?;
    let rows = process_rows()?;
    let related: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.pid == game.pid
                || row.name.to_ascii_lowercase().contains("steam")
                || [
                    "ollama",
                    "jev",
                    "classifier",
                    "vmmem",
                    "wsl",
                    "nvidia",
                    "nvcontainer",
                ]
                .iter()
                .any(|part| row.name.to_ascii_lowercase().contains(part))
        })
        .map(|row| json!({"pid":row.pid,"parent_pid":row.parent_pid,"image_name":row.name}))
        .collect();
    let memory = crate::metrics::process_memory(game.pid)
        .unwrap_or_else(|e| json!({"unavailable":e.to_string()}));
    // A dump is collected before screenshot APIs which may block on the display driver.
    let dump = worker("dump", game, &output.join("startup.dmp"), session)?;
    let screenshot = worker("screenshot", game, &output.join("startup.png"), session)?;
    let gpu = gpu_probe(output, session)?;
    Ok(
        json!({"process":game,"metadata":related,"memory":memory,"dump":dump,
        "screenshot":screenshot,"gpu":gpu,"display_safety":"requires fresh independent native PDH proof"}),
    )
}

pub fn reset_display(
    lease: &mut RecoveryLease,
    pin: &PinnedGame,
    session: &mut DiagnosticSession,
) -> Result<Value> {
    lease.check()?;
    pin.check_remaining()?;
    session.before_reset()?;
    let game = pin.game;
    let path = std::env::temp_dir().join(format!("civ6-idle-{}.json", uuid::Uuid::new_v4()));
    struct Remove(PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let _remove = Remove(path.clone());
    let worker_result = worker("gpu-idle", game, &path, session)?;
    session.before_reset()?;
    let proof = (|| -> Result<IdleProof> {
        if worker_result["result"]["status"] != "ok" {
            return Err(Error::input(
                "bounded native PDH worker failed or timed out",
            ));
        }
        let mut bytes = Vec::new();
        File::open(&path)?
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(Error::input("PDH proof exceeded size limit"));
        }
        Ok(serde_json::from_slice(&bytes)?)
    })()
    .unwrap_or_else(|e| IdleProof {
        source: startup_idle::IDLE_SOURCE.into(),
        status: "unknown".into(),
        detail: e.to_string(),
        ..Default::default()
    });
    struct NativeInput<'a>(&'a mut RecoveryLease, &'a PinnedGame);
    impl DisplayInput for NativeInput<'_> {
        fn check_lock(&mut self) -> Result<()> {
            self.0.check()?;
            self.1.check_remaining()
        }
        fn unix_now(&mut self) -> f64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(f64::NAN)
        }
        fn key_is_down(&mut self, key: u8) -> bool {
            unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
        }
        fn key_event(&mut self, key: u8, up: bool) {
            unsafe {
                keybd_event(key, 0, if up { 2 } else { 0 }, 0);
            }
        }
        fn sleep_100ms(&mut self) {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let mut result = startup_idle::send_recovery_shortcut(&mut NativeInput(lease, pin), &proof)?;
    result["provider"] = worker_result;
    Ok(result)
}

pub fn diagnostic_worker(args: &[String]) -> Result<()> {
    if args.len() != 4 {
        return Err(Error::input(
            "expected operation, PID, creation time, artifact path",
        ));
    }
    let game = GameProcess {
        pid: args[1]
            .parse()
            .map_err(|_| Error::input("invalid diagnostic PID"))?,
        started: args[2]
            .parse()
            .map_err(|_| Error::input("invalid creation time"))?,
    };
    if !game.started.is_finite() || game.pid == 0 {
        return Err(Error::input("invalid diagnostic process identity"));
    }
    let path = Path::new(&args[3]);
    match args[0].as_str() {
        "classify-file" => {
            let mut bytes = Vec::new();
            File::open(path)?
                .take(crate::startup_screen::MAX_PNG_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            println!("{}", crate::startup_screen::classify_png(&bytes)?);
            Ok(())
        }
        operation if operation.starts_with("loading-screen@") => {
            let _handle = pinned(game, 0x1000)?;
            let hwnd: isize = operation
                .strip_prefix("loading-screen@")
                .unwrap()
                .parse()
                .map_err(|_| Error::input("invalid startup capture window"))?;
            if hwnd == 0 || own_window(game)? != hwnd {
                return Err(Error::input(
                    "startup capture window changed before capture",
                ));
            }
            let png = path.with_extension("png");
            crate::capture::capture_owned_window(
                &png,
                hwnd,
                game.pid,
                Some(time::monotonic() + 3.0),
            )?;
            let mut bytes = Vec::new();
            File::open(&png)?
                .take(crate::startup_screen::MAX_PNG_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            let mut screen = crate::startup_screen::classify_png(&bytes)?;
            pinned(game, 0x1000)?;
            if own_window(game)? != hwnd {
                return Err(Error::input("startup capture window changed after capture"));
            }
            screen["hwnd"] = json!(hwnd);
            let file = OpenOptions::new().write(true).create_new(true).open(path)?;
            serde_json::to_writer(file, &screen)?;
            Ok(())
        }
        "identity" => {
            let file = OpenOptions::new().write(true).create_new(true).open(path)?;
            serde_json::to_writer(
                file,
                &json!({"protocol":1,"source_sha256":helper_identity()}),
            )?;
            Ok(())
        }
        "gpu-idle" => {
            let proof = pdh::collect_idle_proof()?;
            let refusal = startup_idle::idle_refusal(&proof);
            println!("GPU_IDLE_POLICY={}", refusal.unwrap_or("allowed"));
            if path == Path::new("-") {
                println!("{}", serde_json::to_string(&proof)?);
                return Ok(());
            }
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            use std::io::Write;
            file.write_all(&serde_json::to_vec(&proof)?)?;
            Ok(())
        }
        "dump" => {
            let handle = pinned(game, 0x410)?; // QUERY_INFORMATION | VM_READ, no termination rights
            let file = OpenOptions::new().write(true).create_new(true).open(path)?;
            if unsafe {
                MiniDumpWriteDump(
                    handle.0,
                    game.pid,
                    file.as_raw_handle(),
                    MiniDumpNormal | MiniDumpWithThreadInfo | MiniDumpFilterModulePaths,
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                )
            } == 0
            {
                return Err(Error::input("native minidump collection failed"));
            }
            Ok(())
        }
        "screenshot" => {
            let _handle = pinned(game, 0x1000)?;
            let hwnd = own_window(game)?;
            crate::capture::capture_owned_window(
                path,
                hwnd,
                game.pid,
                Some(time::monotonic() + 4.0),
            )
        }
        _ => Err(Error::input("unknown native diagnostic operation")),
    }
}

#[cfg(test)]
mod supervision_tests {
    use super::*;
    fn fixture_command() -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "startup::platform::supervision_tests::diagnostic_child_fixture",
                "--nocapture",
            ])
            .env("CIV6_DIAGNOSTIC_CHILD_FIXTURE", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
    #[test]
    fn diagnostic_child_fixture() {
        if std::env::var("CIV6_DIAGNOSTIC_CHILD_FIXTURE").as_deref() == Ok("1") {
            std::thread::sleep(Duration::from_secs(20));
        }
    }
    #[test]
    fn timeout_confirms_owned_child_exit_and_keeps_a_control_process_running() {
        let mut control = OwnedChild(
            fixture_command()
                .creation_flags(0x08000000)
                .spawn()
                .unwrap(),
        );
        let mut session = DiagnosticSession::default();
        let result = bounded_command(
            fixture_command(),
            Duration::from_millis(50),
            &mut session,
            false,
        )
        .unwrap();
        assert_eq!(result["status"], "timeout");
        assert_eq!(result["termination"]["exit_confirmed"], true);
        assert!(control.0.try_wait().unwrap().is_none());
        assert_eq!(session.finish().unwrap()["status"], "all_exited");
    }
    #[test]
    fn unconfirmed_writer_blocks_recovery_until_the_same_handle_exits() {
        let child = OwnedChild(
            fixture_command()
                .creation_flags(0x08000000)
                .spawn()
                .unwrap(),
        );
        let mut session = DiagnosticSession {
            pending: vec![(child, false)],
        };
        assert!(session
            .before_reset()
            .unwrap_err()
            .to_string()
            .contains("exit is unconfirmed"));
        session.pending[0].0 .0.kill().unwrap();
        assert_eq!(session.finish().unwrap()["status"], "all_exited");
        session.before_reset().unwrap();
    }
}
