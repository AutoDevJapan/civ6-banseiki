use civ6_native_bridge_rs::{
    BridgeConfig, BridgeStatus, SharedTrace, GAMECORE, GAME_EXE, MAX_ACTIVE_SECONDS,
    PINNED_CAN_EVER_RVA, PINNED_GAMECORE_SHA256,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::ffi::{c_void, OsString};
use std::fs::{self, File};
use std::io::{BufRead, BufWriter, Read, Seek, SeekFrom, Write};
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Foundation::{
    CloseHandle, FreeLibrary, FILETIME, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Diagnostics::{
    Debug::{ReadProcessMemory, WriteProcessMemory},
    ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
        MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
        TH32CS_SNAPPROCESS,
    },
};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LoadLibraryExW, DONT_RESOLVE_DLL_REFERENCES,
};
use windows_sys::Win32::System::Memory::{VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE};
use windows_sys::Win32::System::Threading::{
    CreateRemoteThread, GetCurrentProcessId, GetExitCodeThread, GetProcessTimes, OpenProcess,
    QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_CREATE_THREAD,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_VM_OPERATION, PROCESS_VM_READ,
    PROCESS_VM_WRITE,
};

const LOCK: &str = "D:/CC/Tools/civ6-live/civ6.lock";
const CHAIN: &str = "D:/CC/Tools/civ6-live/live_chain.exe";
const BRIDGE_DLL: &str = "civ6_native_bridge_rs.dll";
const REMOTE_CALL_SECONDS: u32 = 8;
const CAN_EVER_FILE_OFFSET: u64 = 0x2dd1d0;

struct Handle(HANDLE);
impl Handle {
    fn checked(value: HANDLE, label: &str) -> Result<Self, String> {
        if value.is_null() || value == INVALID_HANDLE_VALUE {
            Err(format!("{label}: {}", std::io::Error::last_os_error()))
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

struct Args {
    pid: u32,
    created: u64,
    duration: u32,
    out: PathBuf,
}

fn args(raw: &[String]) -> Result<Args, String> {
    if raw.len() != 8 {
        return Err("usage: civ6-native-bridge --pid N --created-filetime N --duration 1..300 --out ABSOLUTE.jsonl".into());
    }
    let mut fields = std::collections::BTreeMap::new();
    for pair in raw.chunks_exact(2) {
        if !["--pid", "--created-filetime", "--duration", "--out"].contains(&pair[0].as_str())
            || fields.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err("duplicate or unknown bridge argument".into());
        }
    }
    let pid = fields["--pid"].parse::<u32>().map_err(|_| "invalid PID")?;
    let created = fields["--created-filetime"]
        .parse::<u64>()
        .map_err(|_| "invalid creation FILETIME")?;
    let duration = fields["--duration"]
        .parse::<u32>()
        .map_err(|_| "invalid duration")?;
    let out = PathBuf::from(fields["--out"]);
    if pid == 0
        || created == 0
        || !(1..=MAX_ACTIVE_SECONDS).contains(&duration)
        || !out.is_absolute()
        || out.extension().is_none_or(|ext| ext != "jsonl")
        || !out.parent().is_some_and(Path::is_dir)
        || out.exists()
    {
        return Err("invalid PID, duration, or new absolute JSONL output".into());
    }
    Ok(Args {
        pid,
        created,
        duration,
        out,
    })
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn from_wide(value: &[u16]) -> PathBuf {
    let len = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    PathBuf::from(OsString::from_wide(&value[..len]))
}
fn same_path(actual: &Path, expected: &Path) -> bool {
    let (Ok(actual), Ok(expected)) = (actual.canonicalize(), expected.canonicalize()) else {
        return false;
    };
    actual
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.to_string_lossy())
}
fn sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("cannot read pinned file: {e}"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let len = file
            .read(&mut buffer)
            .map_err(|e| format!("hash read: {e}"))?;
        if len == 0 {
            break;
        }
        hash.update(&buffer[..len]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn filetime(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}
fn process_created(handle: &Handle) -> Result<u64, String> {
    let mut created: FILETIME = unsafe { zeroed() };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        return Err("cannot read process creation time".into());
    }
    Ok(filetime(created))
}
fn process_path(handle: &Handle) -> Result<PathBuf, String> {
    let mut buffer = vec![0u16; 32768];
    let mut len = buffer.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, buffer.as_mut_ptr(), &mut len) } == 0
        || len == 0
        || len as usize >= buffer.len()
    {
        return Err("cannot read complete process image path".into());
    }
    Ok(PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
}
fn alive(handle: &Handle) -> bool {
    (unsafe { WaitForSingleObject(handle.0, 0) }) == WAIT_TIMEOUT
}
fn open_process(pid: u32, rights: u32) -> Result<Handle, String> {
    Handle::checked(unsafe { OpenProcess(rights, 0, pid) }, "OpenProcess")
}
fn parent_pid(pid: u32) -> Result<u32, String> {
    let snapshot = Handle::checked(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) },
        "process snapshot",
    )?;
    let mut entry: PROCESSENTRY32W = unsafe { zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 {
        return Err("process snapshot empty".into());
    }
    loop {
        if entry.th32ProcessID == pid {
            return Ok(entry.th32ParentProcessID);
        }
        if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }
    Err("game missing from process snapshot".into())
}
fn now_unix() -> Result<f64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock unavailable")?
        .as_secs_f64())
}
fn verify_lease(chain_pid: u32, chain: &Handle, duration: u32) -> Result<(), String> {
    let actual_lock =
        std::env::var("CIV6_LIVE_LOCK_PATH").map_err(|_| "live_chain lock environment missing")?;
    if !same_path(Path::new(&actual_lock), Path::new(LOCK)) {
        return Err("standard live_chain lock required".into());
    }
    let value: Value = serde_json::from_slice(&fs::read(LOCK).map_err(|e| e.to_string())?)
        .map_err(|_| "live_chain lock JSON invalid")?;
    if value["live_chain"] != true
        || value["chain_pid"].as_u64() != Some(u64::from(chain_pid))
        || value["pid"].as_u64() != Some(u64::from(chain_pid))
        || !alive(chain)
        || !same_path(&process_path(chain)?, Path::new(CHAIN))
    {
        return Err("live_chain lock owner changed".into());
    }
    let acquired = value["acquired_at"]
        .as_f64()
        .ok_or("live_chain acquisition time missing")?;
    let now = now_unix()?;
    let created = process_created(chain)? as f64 / 10_000_000.0 - 11_644_473_600.0;
    if !acquired.is_finite()
        || acquired > now
        || now + f64::from(duration) + 15.0 >= acquired + 1800.0
        || created > acquired + 1.0
    {
        return Err("live_chain lease cannot cover the bounded bridge run".into());
    }
    Ok(())
}

struct Module {
    base: usize,
    size: usize,
    path: PathBuf,
}
fn modules(pid: u32) -> Result<Vec<Module>, String> {
    let snapshot = Handle::checked(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) },
        "module snapshot",
    )?;
    let mut entry: MODULEENTRY32W = unsafe { zeroed() };
    entry.dwSize = size_of::<MODULEENTRY32W>() as u32;
    if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
        return Err("module snapshot empty".into());
    }
    let mut result = Vec::new();
    loop {
        result.push(Module {
            base: entry.modBaseAddr as usize,
            size: entry.modBaseSize as usize,
            path: from_wide(&entry.szExePath),
        });
        if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }
    Ok(result)
}

fn pinned_target(pid: u32, game: &Handle) -> Result<usize, String> {
    if !same_path(&process_path(game)?, Path::new(GAME_EXE)) {
        return Err("unpinned game executable".into());
    }
    let module = modules(pid)?
        .into_iter()
        .find(|m| same_path(&m.path, Path::new(GAMECORE)))
        .ok_or("pinned GameCore not loaded")?;
    if sha256(Path::new(GAMECORE))? != PINNED_GAMECORE_SHA256
        || module.size <= PINNED_CAN_EVER_RVA + 8
    {
        return Err("GameCore hash or image size differs".into());
    }
    Ok(module.base + PINNED_CAN_EVER_RVA)
}

fn read_remote(game: &Handle, address: usize, output: &mut [u8]) -> Result<(), String> {
    let mut read = 0usize;
    if unsafe {
        ReadProcessMemory(
            game.0,
            address as *const c_void,
            output.as_mut_ptr().cast(),
            output.len(),
            &mut read,
        )
    } == 0
        || read != output.len()
    {
        return Err("bounded target read failed".into());
    }
    Ok(())
}
fn write_remote(game: &Handle, address: usize, input: &[u8]) -> Result<(), String> {
    let mut written = 0usize;
    if unsafe {
        WriteProcessMemory(
            game.0,
            address as *mut c_void,
            input.as_ptr().cast(),
            input.len(),
            &mut written,
        )
    } == 0
        || written != input.len()
    {
        return Err("bounded controller allocation write failed".into());
    }
    Ok(())
}
fn remote_alloc(game: &Handle, bytes: usize) -> Result<usize, String> {
    let result = unsafe {
        VirtualAllocEx(
            game.0,
            null_mut(),
            bytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    (!result.is_null())
        .then_some(result as usize)
        .ok_or_else(|| "controller allocation in pinned game failed".into())
}
fn remote_call(game: &Handle, entry: usize, parameter: usize, label: &str) -> Result<u32, String> {
    if entry == 0 {
        return Err(format!("{label} address missing"));
    }
    let start = unsafe {
        std::mem::transmute::<usize, unsafe extern "system" fn(*mut c_void) -> u32>(entry)
    };
    let mut tid = 0u32;
    let thread = Handle::checked(
        unsafe {
            CreateRemoteThread(
                game.0,
                null_mut(),
                0,
                Some(start),
                parameter as *const c_void,
                0,
                &mut tid,
            )
        },
        label,
    )?;
    if unsafe { WaitForSingleObject(thread.0, REMOTE_CALL_SECONDS * 1000) } != WAIT_OBJECT_0 {
        return Err(format!("{label} timed out; close the pinned game"));
    }
    let mut code = 0u32;
    if unsafe { GetExitCodeThread(thread.0, &mut code) } == 0 {
        return Err(format!(
            "{label} exit code unavailable; close the pinned game"
        ));
    }
    Ok(code)
}

struct LocalDll(*mut c_void);
impl Drop for LocalDll {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0) };
    }
}
fn exported_rvas(path: &Path) -> Result<(usize, usize), String> {
    let dll = LocalDll(unsafe {
        LoadLibraryExW(
            wide(&path.to_string_lossy()).as_ptr(),
            null_mut(),
            DONT_RESOLVE_DLL_REFERENCES,
        )
    });
    if dll.0.is_null() {
        return Err("cannot inspect local bridge DLL exports".into());
    }
    let install = unsafe { GetProcAddress(dll.0, c"BridgeInstall".as_ptr().cast()) }
        .ok_or("bridge install export missing")? as usize;
    let uninstall = unsafe { GetProcAddress(dll.0, c"BridgeUninstall".as_ptr().cast()) }
        .ok_or("bridge uninstall export missing")? as usize;
    Ok((install - dll.0 as usize, uninstall - dll.0 as usize))
}
fn remote_load_library(pid: u32) -> Result<usize, String> {
    let local = unsafe { GetModuleHandleW(wide("kernel32.dll").as_ptr()) };
    if local.is_null() {
        return Err("local kernel32 unavailable".into());
    }
    let function = unsafe { GetProcAddress(local, c"LoadLibraryW".as_ptr().cast()) }
        .ok_or("LoadLibraryW export unavailable")? as usize;
    let local_module = modules(unsafe { GetCurrentProcessId() })?
        .into_iter()
        .find(|m| function >= m.base && function - m.base < m.size)
        .ok_or("LoadLibraryW owner module unknown")?;
    let remote_module = modules(pid)?
        .into_iter()
        .find(|m| same_path(&m.path, &local_module.path))
        .ok_or("remote LoadLibraryW owner module missing")?;
    if remote_module.base != local_module.base {
        return Err("system loader base differs across processes".into());
    }
    Ok(function)
}
fn bridge_dll() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| format!("controller path: {e}"))?;
    let path = executable.with_file_name(BRIDGE_DLL);
    if !path.is_file() || path.file_name().is_none_or(|name| name != BRIDGE_DLL) {
        return Err("bridge DLL must sit beside this controller".into());
    }
    path.canonicalize()
        .map_err(|e| format!("bridge DLL path: {e}"))
}
fn pinned_entry_bytes(game: &Handle, target: usize) -> Result<[u8; 8], String> {
    let mut file = File::open(GAMECORE).map_err(|e| format!("pinned GameCore read: {e}"))?;
    file.seek(SeekFrom::Start(CAN_EVER_FILE_OFFSET))
        .map_err(|e| format!("pinned entry seek: {e}"))?;
    let mut disk = [0u8; 8];
    file.read_exact(&mut disk)
        .map_err(|e| format!("pinned entry read: {e}"))?;
    let mut memory = [0u8; 8];
    read_remote(game, target, &mut memory)?;
    if memory != disk || disk[..5] != [0x48, 0x89, 0x5c, 0x24, 0x08] {
        return Err("pinned in-memory GameCore entry differs".into());
    }
    Ok(disk)
}
fn write_json(writer: &mut BufWriter<File>, value: Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, &value).map_err(|e| e.to_string())?;
    writer.write_all(b"\n").map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())
}

pub fn run() -> Result<(), String> {
    let parsed = args(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let chain_pid = std::env::var("CIV6_LIVE_CHAIN_PID")
        .map_err(|_| "live_chain PID environment missing")?
        .parse::<u32>()
        .map_err(|_| "invalid live_chain PID")?;
    let controller_pid = std::env::var("CIV6_AUTOTEST_CONTROLLER_PID")
        .map_err(|_| "runner parent PID environment missing")?
        .parse::<u32>()
        .map_err(|_| "invalid runner parent PID")?;
    if controller_pid == 0 || parent_pid(unsafe { GetCurrentProcessId() })? != controller_pid {
        return Err("runner is not this controller's OS parent".into());
    }
    let runner = open_process(
        controller_pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
    )?;
    let expected_runner = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("civ6-autotest.exe");
    if !alive(&runner) || !same_path(&process_path(&runner)?, &expected_runner) {
        return Err("pinned runner process identity changed".into());
    }
    let chain = open_process(
        chain_pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
    )?;
    verify_lease(chain_pid, &chain, parsed.duration)?;
    let game = open_process(
        parsed.pid,
        PROCESS_QUERY_LIMITED_INFORMATION
            | PROCESS_SYNCHRONIZE
            | PROCESS_CREATE_THREAD
            | PROCESS_VM_OPERATION
            | PROCESS_VM_WRITE
            | PROCESS_VM_READ,
    )?;
    if !alive(&game) || process_created(&game)? != parsed.created {
        return Err("pinned game identity differs".into());
    }
    let target = pinned_target(parsed.pid, &game)?;
    let original = pinned_entry_bytes(&game, target)?;
    let dll = bridge_dll()?;
    let bridge_sha = sha256(&dll)?;
    let (install_rva, uninstall_rva) = exported_rvas(&dll)?;
    let loader = remote_load_library(parsed.pid)?;
    let output = File::create_new(&parsed.out).map_err(|e| format!("new JSONL output: {e}"))?;
    let mut writer = BufWriter::new(output);
    write_json(
        &mut writer,
        json!({"kind":"identity","pid":parsed.pid,"created_filetime":parsed.created,
            "gamecore_sha256":PINNED_GAMECORE_SHA256,"can_ever_rva":PINNED_CAN_EVER_RVA,
            "entry_bytes":format!("{:02x?}",original),"bridge_sha256":bridge_sha,
            "duration_limit_seconds":parsed.duration}),
    )?;
    let path_wide = wide(&dll.to_string_lossy());
    let path_bytes =
        unsafe { std::slice::from_raw_parts(path_wide.as_ptr().cast::<u8>(), path_wide.len() * 2) };
    let path_remote = remote_alloc(&game, path_bytes.len())?;
    write_remote(&game, path_remote, path_bytes)?;
    let _ = remote_call(&game, loader, path_remote, "LoadLibraryW")?;
    let bridge = modules(parsed.pid)?
        .into_iter()
        .find(|m| same_path(&m.path, &dll))
        .ok_or("bridge DLL not loaded into the pinned game")?;
    if install_rva >= bridge.size || uninstall_rva >= bridge.size {
        return Err("bridge exports outside remote DLL image".into());
    }
    let trace_remote = remote_alloc(&game, size_of::<SharedTrace>())?;
    let mut config = BridgeConfig::new(parsed.pid, parsed.created, trace_remote as u64);
    config.max_seconds = parsed.duration;
    let config_bytes = unsafe {
        std::slice::from_raw_parts(
            (&config as *const BridgeConfig).cast::<u8>(),
            size_of::<BridgeConfig>(),
        )
    };
    let config_remote = remote_alloc(&game, config_bytes.len())?;
    write_remote(&game, config_remote, config_bytes)?;
    let install_status = remote_call(
        &game,
        bridge.base + install_rva,
        config_remote,
        "BridgeInstall",
    )?;
    write_json(
        &mut writer,
        json!({"kind":"install","status":install_status}),
    )?;
    if install_status != BridgeStatus::Ok as u32 {
        return Err(
            "bridge installation refused; a critical status requires closing the pinned game"
                .into(),
        );
    }
    println!("READY pid={} max_seconds={}", parsed.pid, parsed.duration);
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = std::io::stdin().lock().read_line(&mut line);
        let _ = sender.send(result.map(|_| line));
    });
    let deadline = Instant::now() + Duration::from_secs(u64::from(parsed.duration));
    let mut control_error = None;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match receiver.recv_timeout(remaining.min(Duration::from_secs(1))) {
            Ok(Ok(line)) if line.trim() == "stop" => break,
            Ok(_) => {
                control_error = Some("bridge stop pipe closed or invalid".to_string());
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                control_error = Some("bridge stop pipe disconnected".to_string());
                break;
            }
        }
        if verify_lease(chain_pid, &chain, 0).is_err()
            || !alive(&game)
            || process_created(&game) != Ok(parsed.created)
        {
            control_error = Some("bridge lost its pinned game or live_chain lease".into());
            break;
        }
    }
    let uninstall_status = remote_call(&game, bridge.base + uninstall_rva, 0, "BridgeUninstall")?;
    write_json(
        &mut writer,
        json!({"kind":"uninstall","status":uninstall_status}),
    )?;
    if uninstall_status != BridgeStatus::Ok as u32 {
        return Err("bridge entry was not restored; close the pinned game".into());
    }
    let mut snapshot = Box::new(SharedTrace::default());
    let snapshot_bytes = unsafe {
        std::slice::from_raw_parts_mut(
            (&mut *snapshot as *mut SharedTrace).cast::<u8>(),
            size_of::<SharedTrace>(),
        )
    };
    read_remote(&game, trace_remote, snapshot_bytes)?;
    if snapshot.magic != 0x4a41_4342 || snapshot.version != 1 {
        return Err("bridge trace version differs".into());
    }
    let gamecore_size = modules(parsed.pid)?
        .into_iter()
        .find(|m| same_path(&m.path, Path::new(GAMECORE)))
        .ok_or("GameCore unloaded before trace collection")?
        .size;
    for row in snapshot.rows() {
        let return_rva = (row.return_address as usize)
            .checked_sub(target - PINNED_CAN_EVER_RVA)
            .filter(|rva| *rva < gamecore_size);
        write_json(
            &mut writer,
            json!({"kind":"entry","sequence":row.sequence,"governor_index":row.governor_index,
                "governors_this":format!("0x{:x}",row.governors_this),
                "return_site_rva":return_rva}),
        )?;
    }
    write_json(
        &mut writer,
        json!({"kind":"stop","rows":snapshot.rows().len(),
            "dropped":snapshot.dropped.load(std::sync::atomic::Ordering::Acquire),
            "result":if control_error.is_none() {"ok"} else {"error"}}),
    )?;
    if let Some(error) = control_error {
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_arguments_reject_unpinned_or_overlong_runs_before_process_access() {
        let base = [
            "--pid",
            "7",
            "--created-filetime",
            "1",
            "--duration",
            "300",
            "--out",
            "C:/missing/trace.jsonl",
        ];
        assert!(args(&base.map(str::to_string)).is_err());
        let mut bad = base.map(str::to_string);
        bad[5] = "301".into();
        assert!(args(&bad).is_err());
        bad[4] = "--pid".into();
        assert!(args(&bad).is_err());
    }

    #[test]
    fn remote_loader_reaches_only_the_own_test_process() {
        let pid = unsafe { GetCurrentProcessId() };
        let process = open_process(
            pid,
            PROCESS_QUERY_LIMITED_INFORMATION
                | PROCESS_SYNCHRONIZE
                | PROCESS_CREATE_THREAD
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
        )
        .unwrap();
        let system_dll = modules(pid)
            .unwrap()
            .into_iter()
            .find(|m| {
                m.path
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("kernel32.dll"))
            })
            .expect("kernel32 must already be loaded in this test process")
            .path;
        let loader = remote_load_library(pid).unwrap();
        let name = wide(&system_dll.to_string_lossy());
        let bytes =
            unsafe { std::slice::from_raw_parts(name.as_ptr().cast::<u8>(), name.len() * 2) };
        let path_remote = remote_alloc(&process, bytes.len()).unwrap();
        write_remote(&process, path_remote, bytes).unwrap();
        let _ = remote_call(&process, loader, path_remote, "test LoadLibraryW").unwrap();
        assert!(modules(pid)
            .unwrap()
            .into_iter()
            .any(|m| same_path(&m.path, &system_dll)));
    }
}
