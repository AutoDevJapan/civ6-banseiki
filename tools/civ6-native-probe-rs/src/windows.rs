//! Windows x64 hardware breakpoint observer. No target memory or executable
//! bytes are written. The only target changes are per-thread debug registers,
//! which are snapshotted and restored before detach.
use crate::{
    bool_from_al, direct_call_rva, known_path, CAN_APPOINT_RVA, CAN_EVER_RVA, EXPECTED_SHA256,
    MAX_ROWS,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File},
    io::{BufRead, BufWriter, Read, Write},
    mem::{size_of, zeroed},
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, EXCEPTION_BREAKPOINT,
        EXCEPTION_SINGLE_STEP, FILETIME, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    System::{
        Diagnostics::{
            Debug::{
                ContinueDebugEvent, DebugActiveProcess, DebugActiveProcessStop,
                DebugSetProcessKillOnExit, GetThreadContext, ReadProcessMemory, SetThreadContext,
                WaitForDebugEventEx, CONTEXT, CONTEXT_CONTROL_AMD64, CONTEXT_DEBUG_REGISTERS_AMD64,
                CONTEXT_INTEGER_AMD64, CREATE_PROCESS_DEBUG_EVENT, CREATE_THREAD_DEBUG_EVENT,
                DEBUG_EVENT, EXCEPTION_DEBUG_EVENT, EXIT_PROCESS_DEBUG_EVENT,
                EXIT_THREAD_DEBUG_EVENT, LOAD_DLL_DEBUG_EVENT, UNLOAD_DLL_DEBUG_EVENT,
            },
            ToolHelp::{
                CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Thread32First,
                Thread32Next, MODULEENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
                TH32CS_SNAPTHREAD, THREADENTRY32,
            },
        },
        Threading::{
            GetProcessTimes, OpenProcess, OpenThread, QueryFullProcessImageNameW, ResumeThread,
            SuspendThread, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, PROCESS_VM_READ, THREAD_GET_CONTEXT, THREAD_QUERY_INFORMATION,
            THREAD_SET_CONTEXT, THREAD_SUSPEND_RESUME,
        },
    },
};

const GAME_ROOT: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI";
const MODULE_REL: &str = "DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll";
const EVER_FILE_OFFSET: usize = 0x2dd1d0;
const APPOINT_FILE_OFFSET: usize = 0x2dcc20;
const DEBUG_CONTEXT: u32 =
    CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64 | CONTEXT_INTEGER_AMD64;
const RF: u32 = 1 << 16;
const THREAD_RIGHTS: u32 =
    THREAD_GET_CONTEXT | THREAD_SET_CONTEXT | THREAD_SUSPEND_RESUME | THREAD_QUERY_INFORMATION;

#[derive(Clone, Debug)]
pub struct ObserverConfig {
    pub pid: u32,
    pub created: u64,
    pub exe: PathBuf,
    pub chain_pid: u32,
    pub lock: PathBuf,
    pub controller_pid: u32,
    pub controller_created: u64,
    pub output: PathBuf,
    pub seconds: u64,
}

struct Handle(HANDLE);
impl Handle {
    fn new(raw: HANDLE, action: &str) -> Result<Self, String> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(win_error(action))
        } else {
            Ok(Self(raw))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn win_error(action: &str) -> String {
    format!("{action}: {}", std::io::Error::last_os_error())
}
fn normal(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}
fn same_path(a: &Path, b: &Path) -> bool {
    normal(a) == normal(b)
}
fn wide_path(chars: &[u16]) -> Result<PathBuf, String> {
    let len = chars
        .iter()
        .position(|c| *c == 0)
        .ok_or("Windows path was truncated")?;
    Ok(PathBuf::from(OsString::from_wide(&chars[..len])))
}
fn process(pid: u32, read_memory: bool) -> Result<Handle, String> {
    if pid == 0 {
        return Err("zero PID is not allowed".into());
    }
    let rights = PROCESS_QUERY_LIMITED_INFORMATION
        | PROCESS_SYNCHRONIZE
        | if read_memory { PROCESS_VM_READ } else { 0 };
    Handle::new(unsafe { OpenProcess(rights, 0, pid) }, "OpenProcess")
}
fn process_created(handle: &Handle) -> Result<u64, String> {
    let (mut a, mut b, mut c, mut d): (FILETIME, FILETIME, FILETIME, FILETIME) =
        unsafe { zeroed() };
    if unsafe { GetProcessTimes(handle.0, &mut a, &mut b, &mut c, &mut d) } == 0 {
        return Err(win_error("GetProcessTimes"));
    }
    Ok((u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime))
}
fn process_path(handle: &Handle) -> Result<PathBuf, String> {
    let mut buf = vec![0u16; 32768];
    let mut len = buf.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, buf.as_mut_ptr(), &mut len) } == 0 {
        return Err(win_error("QueryFullProcessImageNameW"));
    }
    if len == 0 || len as usize >= buf.len() {
        return Err("process image path truncated".into());
    }
    Ok(PathBuf::from(OsString::from_wide(&buf[..len as usize])))
}
fn alive(handle: &Handle) -> Result<bool, String> {
    match unsafe { WaitForSingleObject(handle.0, 0) } {
        WAIT_TIMEOUT => Ok(true),
        WAIT_OBJECT_0 => Ok(false),
        _ => Err(win_error("WaitForSingleObject")),
    }
}
fn exact_process(pid: u32, created: u64, path: &Path, read_memory: bool) -> Result<Handle, String> {
    let h = process(pid, read_memory)?;
    if !alive(&h)?
        || created == 0
        || process_created(&h)? != created
        || !same_path(&process_path(&h)?, path)
    {
        return Err(format!("process {pid} identity or liveness changed"));
    }
    Ok(h)
}
fn filetime_as_unix(v: u64) -> f64 {
    v as f64 / 10_000_000.0 - 11_644_473_600.0
}

fn verify_lock(lock: &Path, chain_pid: u32, chain: &Handle) -> Result<(), String> {
    let expected = Path::new("D:/CC/Tools/civ6-live/civ6.lock");
    if !same_path(lock, expected) {
        return Err("standard live_chain lock path required".into());
    }
    let value: Value = serde_json::from_slice(
        &fs::read(lock).map_err(|e| format!("live_chain lock unavailable: {e}"))?,
    )
    .map_err(|_| "invalid live_chain lock JSON")?;
    if value["live_chain"] != true
        || value["chain_pid"].as_u64() != Some(u64::from(chain_pid))
        || value["pid"].as_u64() != Some(u64::from(chain_pid))
    {
        return Err("live_chain lock owner changed".into());
    }
    let acquired = value["acquired_at"]
        .as_f64()
        .ok_or("live_chain acquisition time missing")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock unavailable")?
        .as_secs_f64();
    if !acquired.is_finite()
        || acquired > now
        || now >= acquired + 1800.0
        || filetime_as_unix(process_created(chain)?) > acquired + 1.0
    {
        return Err("live_chain 30 minute lease or identity is invalid".into());
    }
    if !alive(chain)?
        || !same_path(
            &process_path(chain)?,
            Path::new("D:/CC/Tools/civ6-live/live_chain.exe"),
        )
    {
        return Err("live_chain process identity changed".into());
    }
    Ok(())
}

struct Module {
    base: u64,
    size: u64,
    path: PathBuf,
}
fn loaded_module(pid: u32, expected: &Path) -> Result<Module, String> {
    let snapshot = Handle::new(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) },
        "module snapshot",
    )?;
    let mut entry = MODULEENTRY32W {
        dwSize: size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
        return Err(win_error("Module32FirstW"));
    }
    loop {
        let path = wide_path(&entry.szExePath)?;
        if path.file_name().is_some_and(|n| {
            n.to_string_lossy()
                .eq_ignore_ascii_case("GameCore_XP2_FinalRelease.dll")
        }) {
            if !same_path(&path, expected) {
                return Err("loaded GameCore module is at an unexpected path".into());
            }
            return Ok(Module {
                base: entry.modBaseAddr as u64,
                size: u64::from(entry.modBaseSize),
                path,
            });
        }
        if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }
    Err("GameCore_XP2_FinalRelease.dll is not loaded".into())
}
fn sha256(path: &Path) -> Result<String, String> {
    let mut input = File::open(path).map_err(|e| format!("GameCore file unavailable: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = input
            .read(&mut buf)
            .map_err(|e| format!("GameCore read failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
fn read_exact<const N: usize>(handle: &Handle, address: u64) -> Result<[u8; N], String> {
    let mut bytes = [0u8; N];
    let mut got = 0usize;
    if unsafe {
        ReadProcessMemory(
            handle.0,
            address as *const _,
            bytes.as_mut_ptr() as *mut _,
            N,
            &mut got,
        )
    } == 0
        || got != N
    {
        return Err(win_error(
            "ReadProcessMemory of target instruction/return address",
        ));
    }
    Ok(bytes)
}
fn verify_entry_bytes(game: &Handle, module: &Module) -> Result<(), String> {
    let mut file = File::open(&module.path).map_err(|e| e.to_string())?;
    use std::io::{Seek, SeekFrom};
    for (rva, offset) in [
        (CAN_EVER_RVA, EVER_FILE_OFFSET),
        (CAN_APPOINT_RVA, APPOINT_FILE_OFFSET),
    ] {
        if rva + 16 > module.size {
            return Err("GameCore module too small for target RVA".into());
        }
        file.seek(SeekFrom::Start(offset as u64))
            .map_err(|e| e.to_string())?;
        let mut disk = [0u8; 16];
        file.read_exact(&mut disk).map_err(|e| e.to_string())?;
        if read_exact::<16>(game, module.base + rva)? != disk {
            return Err(format!(
                "GameCore instruction bytes differ at RVA 0x{rva:x}"
            ));
        }
    }
    Ok(())
}

struct Verified {
    game: Handle,
    chain: Handle,
    controller: Handle,
    module: Module,
    config: ObserverConfig,
}
fn verify(config: ObserverConfig) -> Result<Verified, String> {
    if !(1..=crate::MAX_SECONDS).contains(&config.seconds) {
        return Err("native trace duration must be 1..300 seconds".into());
    }
    if config.pid == config.chain_pid
        || config.pid == config.controller_pid
        || config.chain_pid == config.controller_pid
    {
        return Err("game, chain and controller PIDs must differ".into());
    }
    let expected_module = Path::new(GAME_ROOT).join(MODULE_REL);
    if !normal(&config.exe).starts_with(&(normal(Path::new(GAME_ROOT)) + "/"))
        || config.exe.file_name().is_none_or(|n| {
            !n.to_string_lossy()
                .eq_ignore_ascii_case("CivilizationVI.exe")
        })
    {
        return Err("target is not the exact Steam DX11 Civ6 executable".into());
    }
    let chain = process(config.chain_pid, false)?;
    verify_lock(&config.lock, config.chain_pid, &chain)?;
    let controller = process(config.controller_pid, false)?;
    if !alive(&controller)? || process_created(&controller)? != config.controller_created {
        return Err("observer controller identity changed".into());
    }
    let game = exact_process(config.pid, config.created, &config.exe, true)?;
    let module = loaded_module(config.pid, &expected_module)?;
    if sha256(&module.path)? != EXPECTED_SHA256 {
        return Err("GameCore SHA-256 differs from PR86 pinned build".into());
    }
    verify_entry_bytes(&game, &module)?;
    Ok(Verified {
        game,
        chain,
        controller,
        module,
        config,
    })
}

#[derive(Clone, Copy)]
struct Registers {
    dr: [u64; 4],
    dr6: u64,
    dr7: u64,
}
impl Registers {
    fn from(c: &CONTEXT) -> Self {
        Self {
            dr: [c.Dr0, c.Dr1, c.Dr2, c.Dr3],
            dr6: c.Dr6,
            dr7: c.Dr7,
        }
    }
    fn write(self, c: &mut CONTEXT) {
        [c.Dr0, c.Dr1, c.Dr2, c.Dr3] = self.dr;
        c.Dr6 = self.dr6;
        c.Dr7 = self.dr7;
    }
}
fn context(thread: &Handle) -> Result<CONTEXT, String> {
    let mut c: CONTEXT = unsafe { zeroed() };
    c.ContextFlags = DEBUG_CONTEXT;
    if unsafe { GetThreadContext(thread.0, &mut c) } == 0 {
        return Err(win_error("GetThreadContext"));
    }
    Ok(c)
}
fn set_context(thread: &Handle, c: &CONTEXT) -> Result<(), String> {
    let mut changes = *c;
    changes.ContextFlags = CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64;
    if unsafe { SetThreadContext(thread.0, &changes) } == 0 {
        return Err(win_error("SetThreadContext"));
    }
    Ok(())
}
fn thread_handle(tid: u32) -> Result<Handle, String> {
    Handle::new(unsafe { OpenThread(THREAD_RIGHTS, 0, tid) }, "OpenThread")
}
fn thread_ids(pid: u32) -> Result<BTreeSet<u32>, String> {
    let snapshot = Handle::new(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) },
        "thread snapshot",
    )?;
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    if unsafe { Thread32First(snapshot.0, &mut entry) } == 0 {
        return Err(win_error("Thread32First"));
    }
    let mut ids = BTreeSet::new();
    loop {
        if entry.th32OwnerProcessID == pid {
            ids.insert(entry.th32ThreadID);
        }
        if unsafe { Thread32Next(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }
    Ok(ids)
}

#[derive(Clone)]
struct Pending {
    id: u64,
    entry: &'static str,
    return_address: u64,
    expected_rsp: u64,
}
struct ThreadState {
    handle: Handle,
    original: Registers,
    pending: Vec<Pending>,
}
struct Jsonl {
    writer: BufWriter<File>,
    rows: usize,
}
impl Jsonl {
    fn new(path: &Path) -> Result<Self, String> {
        let file = File::create_new(path).map_err(|e| format!("trace output must be new: {e}"))?;
        Ok(Self {
            writer: BufWriter::new(file),
            rows: 0,
        })
    }
    fn row(&mut self, value: Value) -> Result<(), String> {
        if self.rows >= MAX_ROWS {
            return Err("trace row limit reached".into());
        }
        serde_json::to_writer(&mut self.writer, &value).map_err(|e| e.to_string())?;
        self.writer.write_all(b"\n").map_err(|e| e.to_string())?;
        self.writer.flush().map_err(|e| e.to_string())?;
        self.rows += 1;
        Ok(())
    }
}

struct Debugger {
    pid: u32,
    game: Handle,
    module_base: u64,
    module_size: u64,
    threads: BTreeMap<u32, ThreadState>,
    output: Jsonl,
    next_id: u64,
    attached: bool,
    ready: bool,
}
impl Debugger {
    fn attach(
        pid: u32,
        game: Handle,
        module_base: u64,
        module_size: u64,
        output: Jsonl,
    ) -> Result<Self, String> {
        if unsafe { DebugActiveProcess(pid) } == 0 {
            return Err(win_error("DebugActiveProcess"));
        }
        // Windows requires an active debugger for this call. Keep this as the
        // first operation after attach and detach immediately if it fails.
        if unsafe { DebugSetProcessKillOnExit(0) } == 0 {
            let error = win_error("DebugSetProcessKillOnExit(FALSE)");
            unsafe {
                DebugActiveProcessStop(pid);
            }
            return Err(error);
        }
        Ok(Self {
            pid,
            game,
            module_base,
            module_size,
            threads: BTreeMap::new(),
            output,
            next_id: 1,
            attached: true,
            ready: false,
        })
    }
    fn add_thread(&mut self, tid: u32) -> Result<(), String> {
        if self.threads.contains_key(&tid) {
            return Ok(());
        }
        let handle = thread_handle(tid)?;
        let mut c = context(&handle)?;
        let original = Registers::from(&c);
        if c.Dr7 & 0xff != 0 {
            return Err(format!("thread {tid} already uses hardware breakpoints"));
        }
        c.Dr0 = self.module_base + CAN_EVER_RVA;
        c.Dr1 = self.module_base + CAN_APPOINT_RVA;
        c.Dr2 = 0;
        c.Dr3 = 0;
        c.Dr6 &= !0xf;
        c.Dr7 = (c.Dr7 & !0xff & !(0xffff << 16)) | 0b0101;
        set_context(&handle, &c)?;
        self.threads.insert(
            tid,
            ThreadState {
                handle,
                original,
                pending: Vec::new(),
            },
        );
        Ok(())
    }
    fn refresh_returns(state: &mut ThreadState, c: &mut CONTEXT) -> Result<(), String> {
        let mut sites = Vec::new();
        for item in state.pending.iter().rev() {
            if !sites.contains(&item.return_address) {
                sites.push(item.return_address);
            }
        }
        if sites.len() > 2 {
            return Err("more than two distinct pending return sites".into());
        }
        c.Dr2 = sites.first().copied().unwrap_or(0);
        c.Dr3 = sites.get(1).copied().unwrap_or(0);
        c.Dr7 = (c.Dr7 & !0xf0)
            | if !sites.is_empty() { 0x10 } else { 0 }
            | if sites.len() > 1 { 0x40 } else { 0 };
        c.Dr6 &= !0xf;
        c.EFlags |= RF;
        set_context(&state.handle, c)
    }
    fn entry(
        &mut self,
        tid: u32,
        c: &mut CONTEXT,
        entry: &'static str,
        target_rva: u64,
    ) -> Result<(), String> {
        let return_address = u64::from_le_bytes(read_exact::<8>(&self.game, c.Rsp)?);
        let return_rva = return_address
            .checked_sub(self.module_base)
            .filter(|r| *r < self.module_size);
        let call_rva = return_rva.filter(|r| *r >= 5).and_then(|r| {
            let bytes = read_exact::<5>(&self.game, return_address - 5).ok()?;
            direct_call_rva(r, bytes, target_rva)
        });
        let tail = entry == "can_ever"
            && self.threads[&tid]
                .pending
                .iter()
                .any(|p| p.entry == "can_appoint" && p.return_address == return_address);
        let path = known_path(call_rva, return_rva, tail);
        let id = self.next_id;
        self.next_id += 1;
        let expected_rsp = c.Rsp.checked_add(8).ok_or("stack pointer overflow")?;
        let state = self
            .threads
            .get_mut(&tid)
            .ok_or("debug event thread missing")?;
        let mut sites: BTreeSet<u64> = state.pending.iter().map(|p| p.return_address).collect();
        sites.insert(return_address);
        let armed = return_rva.is_some() && sites.len() <= 2;
        if armed {
            state.pending.push(Pending {
                id,
                entry,
                return_address,
                expected_rsp,
            });
            Self::refresh_returns(state, c)?;
        } else {
            c.Dr6 &= !0xf;
            c.EFlags |= RF;
            set_context(&state.handle, c)?;
        }
        self.output.row(
            json!({"type":"entry","id":id,"thread_id":tid,"function":entry,
            "function_rva":target_rva,"return_site_rva":return_rva,"caller_rva":call_rva,
            "path_candidate":path,"tail_from_can_appoint":tail,"this":format!("0x{:016x}",c.Rcx),
            "governor_index":c.Rdx as u32,"return_breakpoint_armed":armed}),
        )
    }
    fn returned(&mut self, tid: u32, c: &mut CONTEXT) -> Result<(), String> {
        let state = self
            .threads
            .get_mut(&tid)
            .ok_or("return event thread missing")?;
        let mut matched = Vec::new();
        state.pending.retain(|p| {
            if p.return_address == c.Rip && p.expected_rsp == c.Rsp {
                matched.push(p.clone());
                false
            } else {
                true
            }
        });
        Self::refresh_returns(state, c)?;
        for p in matched {
            let al = c.Rax as u8;
            self.output.row(
                json!({"type":"return","id":p.id,"thread_id":tid,"function":p.entry,
                "al":al,"bool_candidate":bool_from_al(al)}),
            )?;
        }
        Ok(())
    }
    fn handle_event(&mut self, event: &DEBUG_EVENT) -> Result<i32, String> {
        let tid = event.dwThreadId;
        match event.dwDebugEventCode {
            CREATE_PROCESS_DEBUG_EVENT => {
                let info = unsafe { event.u.CreateProcessInfo };
                if !info.hFile.is_null() {
                    unsafe {
                        CloseHandle(info.hFile);
                    }
                }
                self.add_thread(tid)?;
            }
            CREATE_THREAD_DEBUG_EVENT => self.add_thread(tid)?,
            EXIT_THREAD_DEBUG_EVENT => {
                self.threads.remove(&tid);
            }
            LOAD_DLL_DEBUG_EVENT => {
                let info = unsafe { event.u.LoadDll };
                if !info.hFile.is_null() {
                    unsafe {
                        CloseHandle(info.hFile);
                    }
                }
            }
            EXIT_PROCESS_DEBUG_EVENT => return Ok(DBG_CONTINUE),
            UNLOAD_DLL_DEBUG_EVENT => {
                if unsafe { event.u.UnloadDll.lpBaseOfDll } as u64 == self.module_base {
                    return Err("observed GameCore module unloaded".into());
                }
            }
            EXCEPTION_DEBUG_EVENT => {
                let info = unsafe { event.u.Exception };
                let code = info.ExceptionRecord.ExceptionCode;
                if code == EXCEPTION_BREAKPOINT && !self.ready && info.dwFirstChance != 0 {
                    self.ready = true;
                } else if code == EXCEPTION_SINGLE_STEP {
                    let state = self
                        .threads
                        .get(&tid)
                        .ok_or("single-step thread was not registered")?;
                    let mut c = context(&state.handle)?;
                    let hit = c.Dr6 & 0xf;
                    if hit & 1 != 0 && c.Rip == self.module_base + CAN_EVER_RVA {
                        self.entry(tid, &mut c, "can_ever", CAN_EVER_RVA)?;
                    } else if hit & 2 != 0 && c.Rip == self.module_base + CAN_APPOINT_RVA {
                        self.entry(tid, &mut c, "can_appoint", CAN_APPOINT_RVA)?;
                    } else if hit & 0xc != 0
                        && state.pending.iter().any(|p| p.return_address == c.Rip)
                    {
                        self.returned(tid, &mut c)?;
                    } else {
                        return Ok(DBG_EXCEPTION_NOT_HANDLED);
                    }
                } else {
                    return Ok(DBG_EXCEPTION_NOT_HANDLED);
                }
            }
            _ => {}
        }
        Ok(DBG_CONTINUE)
    }
    fn status_after_error(&self, event: &DEBUG_EVENT) -> i32 {
        if event.dwDebugEventCode != EXCEPTION_DEBUG_EVENT {
            return DBG_CONTINUE;
        }
        let info = unsafe { event.u.Exception };
        if info.ExceptionRecord.ExceptionCode == EXCEPTION_BREAKPOINT && !self.ready {
            return DBG_CONTINUE;
        }
        // A return trap may already have been removed from `pending` before
        // JSONL writing fails. Pass one single-step exception as handled while
        // restoring all breakpoints, rather than crashing the target on stop.
        if info.ExceptionRecord.ExceptionCode == EXCEPTION_SINGLE_STEP {
            return DBG_CONTINUE;
        }
        DBG_EXCEPTION_NOT_HANDLED
    }
    fn cleanup(&mut self, pending_event: Option<(&DEBUG_EVENT, i32)>) -> Result<(), String> {
        if !self.attached {
            return Ok(());
        }
        // Suspend every thread before restoring registers. Re-enumeration catches
        // threads created while earlier threads were still running.
        let mut held: BTreeMap<u32, Handle> = BTreeMap::new();
        let mut error: Option<String> = None;
        for _ in 0..16 {
            let ids = match thread_ids(self.pid) {
                Ok(v) => v,
                Err(e) => {
                    error.get_or_insert(e);
                    break;
                }
            };
            let additions: Vec<_> = ids
                .difference(&held.keys().copied().collect())
                .copied()
                .collect();
            if additions.is_empty() {
                break;
            }
            for tid in additions {
                match thread_handle(tid) {
                    Ok(h) => {
                        if unsafe { SuspendThread(h.0) } == u32::MAX {
                            error.get_or_insert_with(|| win_error("SuspendThread during detach"));
                        } else {
                            held.insert(tid, h);
                        }
                    }
                    Err(e) => {
                        error.get_or_insert(e);
                    }
                }
            }
        }
        for (tid, state) in &self.threads {
            if !held.contains_key(tid) {
                continue;
            }
            match context(&state.handle) {
                Ok(mut c) => {
                    state.original.write(&mut c);
                    if let Err(e) = set_context(&state.handle, &c) {
                        error.get_or_insert(e);
                    }
                }
                Err(e) => {
                    error.get_or_insert(e);
                }
            }
        }
        if let Some((event, status)) = pending_event {
            if unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status) } == 0 {
                error.get_or_insert_with(|| win_error("ContinueDebugEvent before detach"));
            }
        }
        if unsafe { DebugActiveProcessStop(self.pid) } == 0 {
            error.get_or_insert_with(|| win_error("DebugActiveProcessStop"));
        } else {
            self.attached = false;
        }
        for h in held.values() {
            if unsafe { ResumeThread(h.0) } == u32::MAX {
                error.get_or_insert_with(|| win_error("ResumeThread after detach"));
            }
        }
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(())
        }
    }
    fn run(
        &mut self,
        stop: Receiver<()>,
        budget: Duration,
        mut guard: impl FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        let started = Instant::now();
        loop {
            if stop.try_recv().is_ok() || started.elapsed() >= budget {
                self.cleanup(None)?;
                return Ok(());
            }
            if let Err(e) = guard() {
                let cleanup = self.cleanup(None);
                return Err(format!("{e}; detach: {cleanup:?}"));
            }
            let mut event = DEBUG_EVENT::default();
            if unsafe { WaitForDebugEventEx(&mut event, 100) } == 0 {
                if std::io::Error::last_os_error().raw_os_error() == Some(121) {
                    continue;
                }
                let error = win_error("WaitForDebugEventEx");
                let cleanup = self.cleanup(None);
                return Err(format!("{error}; detach: {cleanup:?}"));
            }
            let status = match self.handle_event(&event) {
                Ok(status) => status,
                Err(e) => {
                    let status = self.status_after_error(&event);
                    let cleanup = self.cleanup(Some((&event, status)));
                    return Err(format!("{e}; detach: {cleanup:?}"));
                }
            };
            if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                unsafe {
                    ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status);
                }
                self.attached = false;
                return Err("target process exited during observation".into());
            }
            if unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status) } == 0 {
                let error = win_error("ContinueDebugEvent");
                let cleanup = self.cleanup(None);
                return Err(format!("{error}; detach: {cleanup:?}"));
            }
            if self.ready {
                self.output
                    .row(json!({"type":"ready","thread_count":self.threads.len()}))?;
                println!("{}", json!({"ready":true,"pid":self.pid}));
                std::io::stdout().flush().map_err(|e| e.to_string())?;
                self.ready = false;
                // The attach breakpoint is the only point at which ready is sent.
                break;
            }
            if started.elapsed() > Duration::from_secs(15) {
                let cleanup = self.cleanup(None);
                return Err(format!(
                    "debugger attach handshake timed out; detach: {cleanup:?}"
                ));
            }
        }
        loop {
            if stop.try_recv().is_ok() || started.elapsed() >= budget {
                self.cleanup(None)?;
                return Ok(());
            }
            if let Err(e) = guard() {
                let cleanup = self.cleanup(None);
                return Err(format!("{e}; detach: {cleanup:?}"));
            }
            let mut event = DEBUG_EVENT::default();
            if unsafe { WaitForDebugEventEx(&mut event, 100) } == 0 {
                if std::io::Error::last_os_error().raw_os_error() == Some(121) {
                    continue;
                }
                let error = win_error("WaitForDebugEventEx");
                let cleanup = self.cleanup(None);
                return Err(format!("{error}; detach: {cleanup:?}"));
            }
            let status = match self.handle_event(&event) {
                Ok(v) => v,
                Err(e) => {
                    let status = self.status_after_error(&event);
                    let cleanup = self.cleanup(Some((&event, status)));
                    return Err(format!("{e}; detach: {cleanup:?}"));
                }
            };
            if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                unsafe {
                    ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status);
                }
                self.attached = false;
                return Err("target process exited during observation".into());
            }
            if unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status) } == 0 {
                let error = win_error("ContinueDebugEvent");
                let cleanup = self.cleanup(None);
                return Err(format!("{error}; detach: {cleanup:?}"));
            }
        }
    }
}
impl Drop for Debugger {
    fn drop(&mut self) {
        if self.attached {
            let _ = self.cleanup(None);
        }
    }
}

pub fn run_observer(config: ObserverConfig) -> Result<(), String> {
    let verified = verify(config)?;
    let output = Jsonl::new(&verified.config.output)?;
    let mut debugger = Debugger::attach(
        verified.config.pid,
        verified.game,
        verified.module.base,
        verified.module.size,
        output,
    )?;
    debugger
        .output
        .row(json!({"type":"identity","game_pid":verified.config.pid,
        "game_created_filetime":verified.config.created,"game_exe":verified.config.exe,
        "module_path":verified.module.path,"module_base":format!("0x{:016x}",verified.module.base),
        "module_sha256":EXPECTED_SHA256,"abi":"candidate_only"}))?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::stdin().lock().read_line(&mut line);
        let _ = tx.send(());
    });
    let game_guard = exact_process(
        verified.config.pid,
        verified.config.created,
        &verified.config.exe,
        false,
    )?;
    let guard = || {
        verify_lock(
            &verified.config.lock,
            verified.config.chain_pid,
            &verified.chain,
        )?;
        if !alive(&verified.controller)?
            || process_created(&verified.controller)? != verified.config.controller_created
        {
            return Err("observer controller exited or changed".into());
        }
        if !alive(&game_guard)? {
            return Err("game process exited".into());
        }
        Ok(())
    };
    let result = debugger.run(rx, Duration::from_secs(verified.config.seconds), guard);
    debugger.output.row(json!({"type":"stop","detached":!debugger.attached,"result":result.as_ref().map(|_| "ok").unwrap_or("error")}))?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufReader, BufWriter},
        process::{Child, ChildStdin, ChildStdout, Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc::Sender,
            Arc,
        },
    };

    #[inline(never)]
    extern "C" fn fixture_target(this: u64, governor: u32) -> u8 {
        let this = std::hint::black_box(this);
        let governor = std::hint::black_box(governor);
        if this == 0x1234 && governor == 7 {
            1
        } else {
            0
        }
    }

    fn child_registers(tid: u32, sentinel: bool) -> String {
        thread::spawn(move || {
            let handle = thread_handle(tid).unwrap();
            assert_ne!(unsafe { SuspendThread(handle.0) }, u32::MAX);
            let mut c = context(&handle).unwrap();
            if sentinel {
                c.Dr2 = fixture_target as *const () as u64;
                let set = set_context(&handle, &c);
                assert_ne!(unsafe { ResumeThread(handle.0) }, u32::MAX);
                set.unwrap();
                assert_ne!(unsafe { SuspendThread(handle.0) }, u32::MAX);
                c = context(&handle).unwrap();
            }
            assert_ne!(unsafe { ResumeThread(handle.0) }, u32::MAX);
            let r = Registers::from(&c);
            format!(
                "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
                r.dr[0], r.dr[1], r.dr[2], r.dr[3], r.dr6, r.dr7
            )
        })
        .join()
        .unwrap()
    }

    #[test]
    fn fixture_child_worker() {
        if std::env::var_os("PR86_PROBE_TEST_CHILD").is_none() {
            return;
        }
        let main_tid = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        let baseline = child_registers(main_tid, true);
        println!(
            "PROBE_FIXTURE_READY:{}:{}",
            fixture_target as *const () as usize, baseline
        );
        std::io::stdout().flush().unwrap();
        let input = std::io::stdin();
        for line in input.lock().lines() {
            match line.unwrap().as_str() {
                "T" => println!("PROBE_OUT:{}", fixture_target(0x1234, 7)),
                "F" => println!("PROBE_OUT:{}", fixture_target(0x1234, 8)),
                "M" => {
                    let a = thread::spawn(|| fixture_target(0x1234, 7));
                    let b = thread::spawn(|| fixture_target(0x1234, 8));
                    println!("PROBE_OUT:M{}{}", a.join().unwrap(), b.join().unwrap());
                }
                "R" => println!("PROBE_REG:{}", child_registers(main_tid, false)),
                "X" => break,
                _ => panic!("invalid fixture command"),
            }
            std::io::stdout().flush().unwrap();
        }
    }

    struct Fixture {
        child: Child,
        input: BufWriter<ChildStdin>,
        output: BufReader<ChildStdout>,
        entry: u64,
        original_regs: String,
    }
    impl Fixture {
        fn start() -> Self {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "windows::tests::fixture_child_worker",
                    "--nocapture",
                ])
                .env("PR86_PROBE_TEST_CHILD", "1")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let input = BufWriter::new(child.stdin.take().unwrap());
            let mut output = BufReader::new(child.stdout.take().unwrap());
            let ready = loop {
                let mut line = String::new();
                assert_ne!(
                    output.read_line(&mut line).unwrap(),
                    0,
                    "fixture child exited before ready"
                );
                if let Some(rest) = line.trim().strip_prefix("PROBE_FIXTURE_READY:") {
                    break rest.to_string();
                }
            };
            let (address, regs) = ready.split_once(':').unwrap();
            Self {
                child,
                input,
                output,
                entry: address.parse().unwrap(),
                original_regs: regs.into(),
            }
        }
        fn command(&mut self, cmd: &str, prefix: &str) -> String {
            writeln!(self.input, "{cmd}").unwrap();
            self.input.flush().unwrap();
            loop {
                let mut line = String::new();
                assert_ne!(
                    self.output.read_line(&mut line).unwrap(),
                    0,
                    "fixture child exited during {cmd}"
                );
                if let Some(rest) = line.trim().strip_prefix(prefix) {
                    return rest.to_string();
                }
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = writeln!(self.input, "X");
            let _ = self.input.flush();
            let _ = self.child.wait();
        }
    }

    fn run_case(timeout: Duration, trigger: bool, fail_guard: bool, fail_recording: bool) {
        let mut child = Fixture::start();
        let probe = process(child.child.id(), true).unwrap();
        let created = process_created(&probe).unwrap();
        let image = process_path(&probe).unwrap();
        assert!(alive(&probe).unwrap());
        assert!(exact_process(child.child.id(), created, &image, true).is_ok());
        let path = std::env::temp_dir().join(format!(
            "pr86-probe-test-{}-{}.jsonl",
            std::process::id(),
            child.child.id()
        ));
        let _ = fs::remove_file(&path);
        let mut output = Jsonl::new(&path).unwrap();
        if fail_recording {
            output.rows = MAX_ROWS - 1;
        }
        let base = child.entry - CAN_EVER_RVA;
        let (stop_tx, stop_rx): (Sender<()>, Receiver<()>) = mpsc::channel();
        let pid = child.child.id();
        let fail = Arc::new(AtomicBool::new(false));
        let fail_in_observer = Arc::clone(&fail);
        let observer = thread::spawn(move || {
            let game = process(pid, true).unwrap();
            let mut debugger = Debugger::attach(pid, game, base, 0x500000, output).unwrap();
            let result = debugger.run(stop_rx, timeout, || {
                if fail_in_observer.load(Ordering::SeqCst) {
                    Err("fixture guard lost".into())
                } else {
                    Ok(())
                }
            });
            (result, debugger.attached)
        });
        let started = Instant::now();
        loop {
            if fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "fixture debugger did not become ready"
            );
            thread::sleep(Duration::from_millis(20));
        }
        if fail_guard {
            fail.store(true, Ordering::SeqCst);
        }
        if fail_recording {
            assert_eq!(
                child.command("T", "PROBE_OUT:"),
                "1",
                "child died on recording failure"
            );
        }
        if trigger {
            assert_eq!(child.command("T", "PROBE_OUT:"), "1");
            assert_eq!(child.command("F", "PROBE_OUT:"), "0");
            assert_eq!(child.command("M", "PROBE_OUT:"), "M10");
            stop_tx.send(()).unwrap();
        }
        let (result, attached) = observer.join().unwrap();
        if fail_guard || fail_recording {
            assert!(result.is_err(), "failure did not abort the observer");
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
        assert!(!attached);
        let rows: Vec<Value> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        if trigger {
            let entries: Vec<_> = rows
                .iter()
                .filter(|r| r["type"] == "entry" && r["function"] == "can_ever")
                .collect();
            let returns: Vec<_> = rows
                .iter()
                .filter(|r| r["type"] == "return" && r["function"] == "can_ever")
                .collect();
            assert!(entries.len() >= 4, "entries: {rows:?}");
            assert!(
                returns.iter().any(|r| r["bool_candidate"] == true),
                "returns: {rows:?}"
            );
            assert!(
                returns.iter().any(|r| r["bool_candidate"] == false),
                "returns: {rows:?}"
            );
            assert!(entries.iter().all(|r| r["this"] == "0x0000000000001234"));
            assert!(entries.iter().any(|r| r["governor_index"] == 7));
            assert!(entries.iter().any(|r| r["governor_index"] == 8));
            let tids: BTreeSet<_> = entries
                .iter()
                .map(|r| r["thread_id"].as_u64().unwrap())
                .collect();
            assert!(tids.len() >= 2, "new threads were not traced: {rows:?}");
        }
        assert_eq!(
            child.command("R", "PROBE_REG:"),
            child.original_regs,
            "debug registers were not restored"
        );
        assert_eq!(
            child.command("T", "PROBE_OUT:"),
            "1",
            "child did not survive detach"
        );
        drop(child);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn entry_return_threads_and_normal_detach_restore_registers() {
        run_case(Duration::from_secs(20), true, false, false);
    }
    #[test]
    fn timeout_detach_preserves_the_child_and_registers() {
        run_case(Duration::from_millis(500), false, false, false);
    }
    #[test]
    fn failure_detach_preserves_the_child_and_registers() {
        run_case(Duration::from_secs(20), false, true, false);
    }
    #[test]
    fn recording_failure_detaches_without_killing_the_child() {
        run_case(Duration::from_secs(20), false, false, true);
    }
}
