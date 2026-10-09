//! Bounded, external x64 observer. No game commands, injection, or memory writes.
//!
//! Run only against the PID/creation FILETIME supplied by the live_chain owner.
//! `ready` (stdout and OUTPUT.ready) means that all four hardware probes are
//! installed, not that any runtime value or notification has been identified.
//! Output contains raw fields only. Even a completed observation is not product
//! acceptance. Forced termination of this debugger cannot run Rust cleanup;
//! use its deadline or Ctrl-C for an orderly restore/continue/detach instead.
//! Pointer types, numeric scale, getter mapping, and notification semantics
//! remain unverified; raw read errors are samples, not target exceptions.

use clap::Parser;
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
use serde::Serialize;
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
use std::path::Path;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Parser)]
#[command(about = "Observe one pinned Civ6 process using four hardware execution probes")]
struct Args {
    #[arg(long)]
    pid: u32,
    /// Decimal Windows creation FILETIME (not Unix time).
    #[arg(long)]
    created: u64,
    /// Absolute, previously nonexistent JSONL file; OUTPUT.ready must also be new.
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 240)]
    seconds: u64,
}

#[cfg(any(all(windows, target_arch = "x86_64"), test))]
impl Args {
    fn validate(&self) -> Result<()> {
        if self.pid == 0 || self.created == 0 {
            return Err("--pid and --created must be nonzero".into());
        }
        if !(1..=240).contains(&self.seconds) {
            return Err("--seconds must be in 1..=240".into());
        }
        if !self.output.is_absolute()
            || self.output.extension().and_then(|s| s.to_str()) != Some("jsonl")
        {
            return Err("--output must be an absolute path ending in .jsonl".into());
        }
        Ok(())
    }

    fn ready_path(&self) -> PathBuf {
        let mut path = self.output.as_os_str().to_os_string();
        path.push(".ready");
        PathBuf::from(path)
    }
}

// These helpers also compile on non-Windows, so safety policies can be tested
// without attaching, launching, or inspecting any real process.
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
mod policy {
    use super::*;

    pub const MODULE_NAME: &str = "GameCore_XP2_FinalRelease.dll";
    pub const MODULE_SHA256: &str =
        "324C51E9EA3531758842E16C69E6CDDBEFBB226C5B675C3D6E60111646C2E98C";
    pub const RVAS: [u64; 4] = [0x713E3D, 0x713E42, 0x70AF4A, 0x115357];
    pub const RF: u32 = 1 << 16;
    pub const GETTER_LIMIT: u32 = 32;
    pub const CANDIDATE_LIMIT: u32 = 8;

    #[derive(Default, Serialize)]
    pub struct AttachBreak {
        pub initial_tid: Option<u32>,
        consumed: bool,
    }

    impl AttachBreak {
        pub fn consume(&mut self, code: u32, first_chance: u32, tid: u32) -> bool {
            if !self.consumed
                && code == 0x80000003
                && first_chance != 0
                && self.initial_tid == Some(tid)
            {
                self.consumed = true;
                true
            } else {
                false
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
    pub struct Identity {
        pub pid: u32,
        pub created: u64,
        pub image_path: String,
    }

    pub fn match_identity(expected: &Identity, actual: &Identity, alive: bool) -> Result<()> {
        if !alive {
            return Err("pinned process has exited".into());
        }
        if expected != actual {
            return Err("PID, creation FILETIME, or pinned image path changed".into());
        }
        Ok(())
    }

    pub fn expected_module(image: &Path) -> Result<PathBuf> {
        let file = image.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !["CivilizationVI.exe", "CivilizationVI_DX12.exe"]
            .iter()
            .any(|name| file.eq_ignore_ascii_case(name))
        {
            return Err("target image is not a supported Civ6 executable".into());
        }
        let mut root = image.parent().ok_or("image has no parent")?;
        for suffix in ["Win64Steam", "Binaries", "Base"] {
            if !root
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|part| part.eq_ignore_ascii_case(suffix))
            {
                return Err(
                    "image is outside the supported Base/Binaries/Win64Steam layout".into(),
                );
            }
            root = root.parent().ok_or("incomplete Civ6 image path")?;
        }
        Ok(root.join("DLC/Expansion2/Binaries/Win64").join(MODULE_NAME))
    }

    pub fn breakpoint_addresses(base: u64, size: u32) -> Result<[u64; 4]> {
        if base == 0 || RVAS.iter().any(|&rva| rva >= u64::from(size)) {
            return Err("probe RVAs are outside the pinned module".into());
        }
        let mut addresses = [0; 4];
        for (address, rva) in addresses.iter_mut().zip(RVAS) {
            *address = base.checked_add(rva).ok_or("module address overflow")?;
        }
        Ok(addresses)
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
    pub struct DebugRegisters {
        pub address: [u64; 4],
        pub dr6: u64,
        pub dr7: u64,
    }

    impl DebugRegisters {
        pub fn unused(self) -> bool {
            // DR6's architectural reset/reserved ones and DR7's fixed bit 10
            // may be returned by Windows even when no debugger uses the thread.
            // All addresses, control bits, and meaningful status must be zero.
            self.address == [0; 4] && matches!(self.dr6, 0 | 0xFFFF0FF0) && self.dr7 & !0x400 == 0
        }

        pub fn installed(self, addresses: [u64; 4]) -> Self {
            Self {
                address: addresses,
                dr6: self.dr6,
                dr7: self.dr7 | 0x55, // local enable, execution, length 1 in all slots
            }
        }

        pub fn own_hit(self, addresses: [u64; 4], rip: u64, flags: u32) -> Option<usize> {
            // B0..B3 are sticky CPU status, not a one-hot event ID. Select the
            // current execution slot from RIP, then check that slot's B bit.
            let slot = addresses.iter().position(|&address| address == rip)?;
            if self.address != addresses
                || self.dr7 & !0x400 != 0x55
                || self.dr6 & (1 << slot) == 0
                || self.dr6 & 0xE000 != 0 // BD, BS, BT are not ours
                || flags & 0x100 != 0
            // do not swallow another single-step source
            {
                return None;
            }
            Some(slot)
        }
    }

    pub const FIRST_CHANCE_RECORD_LIMIT: u32 = 16;

    #[derive(Clone, Copy, Debug, Serialize)]
    pub struct ProbeSnapshot {
        pub armed: bool,
        pub registers: DebugRegisters,
        pub rip: u64,
        pub flags: u32,
    }

    #[derive(Clone, Copy, Debug, Serialize)]
    pub struct RawException {
        pub tid: u32,
        pub code: u32,
        pub first_chance: u32,
        pub exception_address: u64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
    pub enum ExceptionDecision {
        AttachBreak,
        Probe(usize),
        RemoveOwnedProbe,
        ForwardFirstChance { record: bool },
        ForwardSecondChance,
    }

    impl ExceptionDecision {
        pub fn handled(self) -> bool {
            matches!(
                self,
                Self::AttachBreak | Self::Probe(_) | Self::RemoveOwnedProbe
            )
        }

        pub fn stop_observing(self) -> bool {
            matches!(self, Self::RemoveOwnedProbe | Self::ForwardSecondChance)
        }

        pub fn resume_debug_flags(self, dr6: u64, flags: u32) -> Option<(u64, u32)> {
            matches!(self, Self::Probe(_) | Self::RemoveOwnedProbe)
                .then_some((dr6 & !0xF, flags | RF))
        }
    }

    #[derive(Default, Serialize)]
    pub struct ExceptionPolicy {
        pub attach: AttachBreak,
        pub unknown_first_chance_count: u64,
        pub unknown_first_chance_records: u32,
        pub second_chance_count: u64,
    }

    impl ExceptionPolicy {
        pub fn decide(
            &mut self,
            exception: RawException,
            snapshot: Option<ProbeSnapshot>,
            addresses: [u64; 4],
            owns_probes: bool,
        ) -> ExceptionDecision {
            if self
                .attach
                .consume(exception.code, exception.first_chance, exception.tid)
            {
                return ExceptionDecision::AttachBreak;
            }
            if exception.code == 0x80000004 && owns_probes {
                if exception.first_chance != 0 {
                    if let Some(snapshot) = snapshot.filter(|snapshot| {
                        snapshot.armed && snapshot.rip == exception.exception_address
                    }) {
                        if let Some(slot) =
                            snapshot
                                .registers
                                .own_hit(addresses, snapshot.rip, snapshot.flags)
                        {
                            return ExceptionDecision::Probe(slot);
                        }
                    }
                }
                // Never turn our own hardware exception into a fatal forwarded
                // SINGLE_STEP when identity/context readback is inconsistent.
                // Remove our probes; do not change RIP or repair game state.
                if addresses.contains(&exception.exception_address)
                    || snapshot
                        .is_some_and(|snapshot| snapshot.armed && addresses.contains(&snapshot.rip))
                {
                    return ExceptionDecision::RemoveOwnedProbe;
                }
            }
            if exception.first_chance != 0 {
                self.unknown_first_chance_count = self.unknown_first_chance_count.saturating_add(1);
                let record = self.unknown_first_chance_records < FIRST_CHANCE_RECORD_LIMIT;
                if record {
                    self.unknown_first_chance_records += 1;
                }
                ExceptionDecision::ForwardFirstChance { record }
            } else {
                self.second_chance_count = self.second_chance_count.saturating_add(1);
                ExceptionDecision::ForwardSecondChance
            }
        }
    }

    // A remote read error is diagnostic data, not a Windows exception from the
    // target. No caller can propagate it as an observation/session failure.
    pub fn raw_read<T: Serialize>(result: Result<T>) -> serde_json::Value {
        match result {
            Ok(value) => serde_json::to_value(value).unwrap_or_else(|error|
                serde_json::json!({"read_error":format!("raw field serialization: {error}")})),
            Err(error) => serde_json::json!({"read_error":error}),
        }
    }

    #[derive(Clone, Copy, Debug, Serialize)]
    pub struct Trigger {
        pub cache_city: u64,
        pub receiver: u64,
        pub tid: u32,
        pub rsp: u64,
        pub returned: bool,
    }

    #[derive(Debug, Default, Serialize)]
    pub struct Observation {
        pub trigger: Option<Trigger>,
        pub later_setters_ignored: u64,
        pub another_setter_for_trigger_cache: bool,
        pub getters_before_return: u64,
        pub getters_recorded: u32,
        pub candidates_recorded: u32,
    }

    impl Observation {
        pub fn setter(&mut self, cache: u64, rcx: u64, tid: u32, rsp: u64) -> Result<bool> {
            if let Some(trigger) = self.trigger {
                self.later_setters_ignored = self.later_setters_ignored.saturating_add(1);
                self.another_setter_for_trigger_cache |= trigger.cache_city == cache;
                return Ok(false);
            }
            if cache == 0 || cache.checked_add(0x1630) != Some(rcx) {
                return Err("first setter RCX does not equal RBX + 0x1630".into());
            }
            self.trigger = Some(Trigger {
                cache_city: cache,
                receiver: rcx,
                tid,
                rsp,
                returned: false,
            });
            Ok(true)
        }

        pub fn returned(&mut self, cache: u64, tid: u32, rsp: u64) -> bool {
            let Some(trigger) = self.trigger.as_mut() else {
                return false;
            };
            if trigger.returned
                || (trigger.cache_city, trigger.tid, trigger.rsp) != (cache, tid, rsp)
            {
                return false;
            }
            trigger.returned = true;
            true
        }

        pub fn getter(&mut self) -> bool {
            if !self.trigger.is_some_and(|trigger| trigger.returned) {
                self.getters_before_return = self.getters_before_return.saturating_add(1);
                return false;
            }
            // Getter and setter receivers have different types/resolution
            // paths. Only temporal correlation is known; do not infer a cache
            // offset or any city's identity from the getter's RAX.
            if self.getters_recorded >= GETTER_LIMIT {
                return false;
            }
            self.getters_recorded += 1;
            true
        }

        pub fn candidate(&mut self, receiver: u64, tid: u32) -> bool {
            if !self.trigger.is_some_and(|trigger| {
                !trigger.returned && trigger.receiver == receiver && trigger.tid == tid
            }) || self.candidates_recorded >= CANDIDATE_LIMIT
            {
                return false;
            }
            self.candidates_recorded += 1;
            true
        }
    }

    #[derive(Debug, Serialize, PartialEq, Eq)]
    pub enum RestoreOutcome {
        Restored,
        ThreadExited,
    }

    #[derive(Debug, Default, Serialize)]
    pub struct CleanupReport {
        pub threads_restored: u32,
        pub threads_exited: u32,
        pub errors: Vec<String>,
        pub continue_pending_ok: bool,
        pub debug_active_process_stop_ok: bool,
        pub debug_active_process_stop_error: Option<String>,
        pub process_exited: bool,
    }

    impl CleanupReport {
        pub fn complete(&self) -> bool {
            self.errors.is_empty()
                && self.continue_pending_ok
                && (self.debug_active_process_stop_ok || self.process_exited)
        }
    }

    pub trait CleanupBackend {
        fn freeze(&mut self) -> Vec<String>;
        fn armed_threads(&self) -> Vec<u32>;
        fn restore(&mut self, tid: u32) -> Result<RestoreOutcome>;
        fn continue_pending(&mut self) -> Result<()>;
        fn drain_events(&mut self) -> Result<()>;
        fn detach(&mut self) -> Result<bool>;
        fn detach_diagnostic(&self) -> Option<String> {
            None
        }
        fn resume(&mut self) -> Vec<String>;
        fn process_exited(&self) -> bool;
    }

    pub fn cleanup(backend: &mut impl CleanupBackend) -> CleanupReport {
        let mut report = CleanupReport {
            errors: backend.freeze(),
            ..Default::default()
        };
        // No `?` here: one failed restore must not skip other threads, the
        // outstanding event, or the detach attempt. No log I/O in this path.
        for tid in backend.armed_threads() {
            match backend.restore(tid) {
                Ok(RestoreOutcome::Restored) => report.threads_restored += 1,
                Ok(RestoreOutcome::ThreadExited) => report.threads_exited += 1,
                Err(error) => report.errors.push(format!("restore thread {tid}: {error}")),
            }
        }
        match backend.continue_pending() {
            Ok(()) => report.continue_pending_ok = true,
            Err(error) => report.errors.push(error),
        }
        if let Err(error) = backend.drain_events() {
            report.errors.push(error);
        }
        match backend.detach() {
            Ok(ok) => report.debug_active_process_stop_ok = ok,
            Err(error) => report.errors.push(error),
        }
        report.debug_active_process_stop_error = backend.detach_diagnostic();
        report.errors.extend(backend.resume());
        report.process_exited = backend.process_exited();
        report
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod platform {
    use super::{policy::*, *};
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{
        collections::BTreeMap,
        ffi::{c_void, OsString},
        fs::{File, OpenOptions},
        io::{Read, Write},
        mem::size_of,
        os::windows::{ffi::OsStringExt, fs::OpenOptionsExt, io::AsRawHandle},
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, GetLastError, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED,
            ERROR_NO_MORE_FILES, ERROR_SEM_TIMEOUT, EXCEPTION_SINGLE_STEP, FILETIME, HANDLE,
            INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        Storage::FileSystem::{GetFinalPathNameByHandleW, FILE_SHARE_READ},
        System::{
            Diagnostics::{
                Debug::{
                    ContinueDebugEvent, DebugActiveProcess, DebugActiveProcessStop,
                    DebugSetProcessKillOnExit, GetThreadContext, ReadProcessMemory,
                    SetThreadContext, WaitForDebugEvent, CONTEXT, CONTEXT_CONTROL_AMD64,
                    CONTEXT_DEBUG_REGISTERS_AMD64, CONTEXT_INTEGER_AMD64,
                    CREATE_PROCESS_DEBUG_EVENT, CREATE_THREAD_DEBUG_EVENT, DEBUG_EVENT,
                    EXCEPTION_DEBUG_EVENT, EXIT_PROCESS_DEBUG_EVENT, EXIT_THREAD_DEBUG_EVENT,
                    LOAD_DLL_DEBUG_EVENT, RIP_EVENT, UNLOAD_DLL_DEBUG_EVENT,
                },
                ToolHelp::{
                    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Thread32First,
                    Thread32Next, MODULEENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPTHREAD,
                    THREADENTRY32,
                },
            },
            Threading::{
                GetProcessId, GetProcessIdOfThread, GetProcessTimes, IsWow64Process, OpenProcess,
                OpenThread, QueryFullProcessImageNameW, ResumeThread, SuspendThread,
                WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
                PROCESS_VM_READ, THREAD_GET_CONTEXT, THREAD_QUERY_LIMITED_INFORMATION,
                THREAD_SET_CONTEXT, THREAD_SUSPEND_RESUME, THREAD_SYNCHRONIZE,
            },
        },
    };

    fn win_error(operation: &str) -> String {
        // SAFETY: GetLastError only reads this caller's thread-local status.
        let code = unsafe { GetLastError() };
        format!("{operation}: Windows error {code}")
    }

    #[derive(Debug)]
    struct Handle(HANDLE);

    impl Handle {
        fn new(raw: HANDLE, operation: &str) -> Result<Self> {
            if raw.is_null() || raw == INVALID_HANDLE_VALUE {
                Err(win_error(operation))
            } else {
                Ok(Self(raw))
            }
        }

        // ContinueDebugEvent automatically closes these original debug-event
        // process/thread handles when their EXIT event is continued.
        fn closed_by_continue(mut self) {
            self.0 = std::ptr::null_mut();
        }

        fn alive(&self) -> Result<bool> {
            // SAFETY: the handle is owned and has synchronization access.
            match unsafe { WaitForSingleObject(self.0, 0) } {
                WAIT_TIMEOUT => Ok(true),
                WAIT_OBJECT_0 => Ok(false),
                _ => Err(win_error("WaitForSingleObject")),
            }
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: owned handle, or original event handle whose EXIT
                // has not been continued; it is closed exactly once.
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    fn canonical(path: &Path) -> Result<PathBuf> {
        std::fs::canonicalize(path).map_err(|e| format!("canonicalize pinned path: {e}"))
    }

    fn path_key(path: &Path) -> Result<String> {
        path.to_str()
            .map(|s| s.to_lowercase())
            .ok_or_else(|| "pinned path is not valid Unicode".into())
    }

    fn wide_path(value: &[u16]) -> Result<PathBuf> {
        let end = value
            .iter()
            .position(|&c| c == 0)
            .ok_or("truncated Toolhelp path")?;
        Ok(PathBuf::from(OsString::from_wide(&value[..end])))
    }

    fn file_path(raw: HANDLE) -> Result<PathBuf> {
        let mut buffer = vec![0u16; 32768];
        // SAFETY: borrowed valid file handle, bounded UTF-16 destination.
        let len =
            unsafe { GetFinalPathNameByHandleW(raw, buffer.as_mut_ptr(), buffer.len() as u32, 0) };
        if len == 0 {
            return Err(win_error("GetFinalPathNameByHandleW"));
        }
        if len as usize >= buffer.len() {
            return Err("file path exceeds bounded buffer".into());
        }
        canonical(&PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
    }

    fn identity(raw: HANDLE) -> Result<Identity> {
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: borrowed process handle and valid FILETIME destinations.
        if unsafe { GetProcessTimes(raw, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
            return Err(win_error("GetProcessTimes"));
        }
        let mut buffer = vec![0u16; 32768];
        let mut len = buffer.len() as u32;
        // SAFETY: same borrowed process handle, bounded UTF-16 destination.
        if unsafe { QueryFullProcessImageNameW(raw, 0, buffer.as_mut_ptr(), &mut len) } == 0 {
            return Err(win_error("QueryFullProcessImageNameW"));
        }
        let image = canonical(&PathBuf::from(OsString::from_wide(&buffer[..len as usize])))?;
        // SAFETY: same process handle; no PID is opened a second time.
        let pid = unsafe { GetProcessId(raw) };
        if pid == 0 {
            return Err(win_error("GetProcessId"));
        }
        Ok(Identity {
            pid,
            created: (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime),
            image_path: path_key(&image)?,
        })
    }

    struct Process {
        handle: Handle,
        pinned: Identity,
    }

    impl Process {
        fn open(args: &Args) -> Result<Self> {
            // SAFETY: only the specified PID, read/query/synchronize access;
            // deliberately no VM_WRITE, VM_OPERATION, or injection rights.
            let handle = Handle::new(
                unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ | PROCESS_SYNCHRONIZE,
                        0,
                        args.pid,
                    )
                },
                "OpenProcess",
            )?;
            if !handle.alive()? {
                return Err("specified process has exited".into());
            }
            let pinned = identity(handle.0)?;
            let expected = Identity {
                pid: args.pid,
                created: args.created,
                image_path: pinned.image_path.clone(),
            };
            match_identity(&expected, &pinned, handle.alive()?)?;
            expected_module(Path::new(&pinned.image_path))?;
            let mut wow64 = 0;
            // SAFETY: owned process handle and valid BOOL destination.
            if unsafe { IsWow64Process(handle.0, &mut wow64) } == 0 {
                return Err(win_error("IsWow64Process"));
            }
            if wow64 != 0 {
                return Err("target is not a native x64 process".into());
            }
            Ok(Self { handle, pinned })
        }

        fn verify(&self) -> Result<()> {
            match_identity(
                &self.pinned,
                &identity(self.handle.0)?,
                self.handle.alive()?,
            )
        }

        fn read<const N: usize>(&self, address: u64) -> Result<[u8; N]> {
            if !(1..=8).contains(&N) || address == 0 || address.checked_add(N as u64).is_none() {
                return Err("invalid bounded field read".into());
            }
            let mut bytes = [0; N];
            let mut received = 0;
            // SAFETY: Windows validates the remote address; local destination
            // is exactly N bytes. This boundary accepts only fields <= 8 bytes.
            if unsafe {
                ReadProcessMemory(
                    self.handle.0,
                    address as *const c_void,
                    bytes.as_mut_ptr().cast(),
                    N,
                    &mut received,
                )
            } == 0
            {
                return Err(win_error("ReadProcessMemory(field <= 8 bytes)"));
            }
            if received != N {
                return Err("short remote field read".into());
            }
            Ok(bytes)
        }

        fn at(base: u64, offset: u64) -> Result<u64> {
            if base == 0 {
                return Err("null field base".into());
            }
            base.checked_add(offset)
                .ok_or_else(|| "field address overflow".into())
        }

        fn u64_at(&self, base: u64, offset: u64) -> Result<u64> {
            Ok(u64::from_le_bytes(self.read::<8>(Self::at(base, offset)?)?))
        }

        fn i32_at(&self, base: u64, offset: u64) -> Result<i32> {
            Ok(i32::from_le_bytes(self.read::<4>(Self::at(base, offset)?)?))
        }

        fn u16_at(&self, base: u64, offset: u64) -> Result<u16> {
            Ok(u16::from_le_bytes(self.read::<2>(Self::at(base, offset)?)?))
        }

        fn cache_fields(&self, trigger: Trigger) -> Value {
            let pointer = self.u64_at(trigger.cache_city, 0x16C0);
            let backpointer = self.u64_at(trigger.receiver, 0x90);
            // Same address is read through both supplied layouts so disagreement
            // is visible; neither pointer is named as a proven semantic object.
            let pointee = match &pointer {
                Ok(0) => Value::Null,
                Ok(pointer) => {
                    json!({"raw_pointer_plus_a8_i32": raw_read(self.i32_at(*pointer, 0xA8)),
                    "raw_pointer_plus_d8_u16": raw_read(self.u16_at(*pointer, 0xD8))})
                }
                Err(error) => json!({"read_error":format!("pointee unavailable: {error}")}),
            };
            json!({
                "raw_cache_plus_16d8_i32": raw_read(self.i32_at(trigger.cache_city, 0x16D8)),
                "raw_cache_plus_16f0_i32": raw_read(self.i32_at(trigger.cache_city, 0x16F0)),
                "raw_cache_plus_16c0_pointer": raw_read(pointer),
                "raw_receiver_plus_90_pointer": raw_read(backpointer),
                "raw_receiver_plus_a8_i32": raw_read(self.i32_at(trigger.receiver, 0xA8)),
                "raw_pointee_fields": pointee,
            })
        }
    }

    #[derive(Debug, PartialEq, Eq, Serialize)]
    struct ModuleIdentity {
        path: String,
        base: u64,
        size: u32,
    }

    fn find_module(process: &Process) -> Result<ModuleIdentity> {
        process.verify()?;
        // SAFETY: metadata snapshot for the pinned PID; no heap/memory dump.
        let snapshot = Handle::new(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, process.pinned.pid) },
            "CreateToolhelp32Snapshot(modules)",
        )?;
        let mut entry = MODULEENTRY32W {
            dwSize: size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        // SAFETY: valid snapshot and correctly sized destination.
        if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
            return Err(win_error("Module32FirstW"));
        }
        let expected_path = path_key(&canonical(&expected_module(Path::new(
            &process.pinned.image_path,
        ))?)?)?;
        let mut found = None;
        loop {
            let name = wide_path(&entry.szModule)?;
            let name = name.to_str().ok_or("module name is not Unicode")?;
            if name.to_ascii_lowercase().starts_with("gamecore_") {
                if !name.eq_ignore_ascii_case(MODULE_NAME) {
                    return Err(
                        "another GameCore module is loaded; refusing unsupported DLL".into(),
                    );
                }
                let module = ModuleIdentity {
                    path: path_key(&canonical(&wide_path(&entry.szExePath)?)?)?,
                    base: entry.modBaseAddr as u64,
                    size: entry.modBaseSize,
                };
                if found.is_some()
                    || module.path != expected_path
                    || entry.th32ProcessID != process.pinned.pid
                {
                    return Err("duplicate module or module path/PID mismatch".into());
                }
                breakpoint_addresses(module.base, module.size)?;
                found = Some(module);
            }
            // SAFETY: same metadata snapshot and sized destination.
            if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
                // SAFETY: capture this API's error before another call.
                if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                    return Err(win_error("Module32NextW"));
                }
                break;
            }
        }
        process.verify()?;
        found.ok_or_else(|| "specified XP2 module is not loaded".into())
    }

    struct PinnedModule {
        identity: ModuleIdentity,
        // Deny write/delete sharing throughout the session to pin the hashed
        // on-disk module. This SHA is not a hash of all loaded process memory.
        _file: File,
        addresses: [u64; 4],
    }

    impl PinnedModule {
        fn open(process: &Process) -> Result<Self> {
            let identity = find_module(process)?;
            let mut file = OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(&identity.path)
                .map_err(|e| format!("open pinned DLL read-only: {e}"))?;
            if path_key(&file_path(file.as_raw_handle())?)? != identity.path {
                return Err("module file handle path mismatch".into());
            }
            let mut hasher = Sha256::new();
            let mut bytes = [0; 65536];
            loop {
                let n = file
                    .read(&mut bytes)
                    .map_err(|e| format!("hash pinned DLL file: {e}"))?;
                if n == 0 {
                    break;
                }
                hasher.update(&bytes[..n]);
            }
            let sha = format!("{:X}", hasher.finalize());
            if sha != MODULE_SHA256 {
                return Err(format!("module file SHA256 mismatch: {sha}"));
            }
            let addresses = breakpoint_addresses(identity.base, identity.size)?;
            let module = Self {
                identity,
                _file: file,
                addresses,
            };
            module.verify(process)?;
            Ok(module)
        }

        fn verify(&self, process: &Process) -> Result<()> {
            if find_module(process)? != self.identity {
                return Err("pinned module path, ASLR base, or size changed".into());
            }
            Ok(())
        }
    }

    fn context(handle: &Handle, flags: u32) -> Result<CONTEXT> {
        // Windows CONTEXT requires 16-byte alignment, which windows-sys' C
        // layout alone does not express on every compiler/target.
        #[repr(C, align(16))]
        struct Aligned(CONTEXT);
        let mut value = Aligned(CONTEXT {
            ContextFlags: flags,
            ..Default::default()
        });
        // SAFETY: owned thread is stopped by a debug event or SuspendThread;
        // destination has the required alignment and requested register groups.
        if unsafe { GetThreadContext(handle.0, &mut value.0) } == 0 {
            return Err(win_error("GetThreadContext"));
        }
        Ok(value.0)
    }

    fn set_context(handle: &Handle, value: CONTEXT) -> Result<()> {
        #[repr(C, align(16))]
        struct Aligned(CONTEXT);
        let value = Aligned(value);
        // SAFETY: owned stopped thread; aligned CONTEXT. Callers select only
        // debug/control groups, preserve control registers, and only add RF.
        if unsafe { SetThreadContext(handle.0, &value.0) } == 0 {
            Err(win_error("SetThreadContext"))
        } else {
            Ok(())
        }
    }

    fn registers(value: &CONTEXT) -> DebugRegisters {
        DebugRegisters {
            address: [value.Dr0, value.Dr1, value.Dr2, value.Dr3],
            dr6: value.Dr6,
            dr7: value.Dr7,
        }
    }

    fn replace_registers(value: &mut CONTEXT, registers: DebugRegisters) {
        [value.Dr0, value.Dr1, value.Dr2, value.Dr3] = registers.address;
        value.Dr6 = registers.dr6;
        value.Dr7 = registers.dr7;
    }

    struct Suspension<'a> {
        handle: &'a Handle,
        resumed: bool,
    }

    impl<'a> Suspension<'a> {
        fn new(handle: &'a Handle) -> Result<Self> {
            // SAFETY: owned target thread, one reversible suspend-count increment.
            if unsafe { SuspendThread(handle.0) } == u32::MAX {
                return Err(win_error("SuspendThread"));
            }
            Ok(Self {
                handle,
                resumed: false,
            })
        }

        fn resume(&mut self) -> Result<()> {
            if self.resumed {
                return Ok(());
            }
            // SAFETY: exactly one increment owned by this guard is removed.
            if unsafe { ResumeThread(self.handle.0) } == u32::MAX {
                return Err(win_error("ResumeThread"));
            }
            self.resumed = true;
            Ok(())
        }
    }

    impl Drop for Suspension<'_> {
        fn drop(&mut self) {
            if !self.resumed {
                let _ = self.resume();
            }
        }
    }

    fn preflight_threads(process: &Process) -> Result<()> {
        // SAFETY: thread metadata only; filter ownership before opening handles.
        let snapshot = Handle::new(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) },
            "thread snapshot",
        )?;
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        // SAFETY: valid snapshot and correctly sized entry.
        if unsafe { Thread32First(snapshot.0, &mut entry) } == 0 {
            return Err(win_error("Thread32First"));
        }
        let mut count = 0;
        loop {
            if entry.th32OwnerProcessID == process.pinned.pid {
                // SAFETY: only a thread reported as owned by the pinned PID.
                let thread = Handle::new(
                    unsafe {
                        OpenThread(
                            THREAD_GET_CONTEXT
                                | THREAD_SUSPEND_RESUME
                                | THREAD_QUERY_LIMITED_INFORMATION
                                | THREAD_SYNCHRONIZE,
                            0,
                            entry.th32ThreadID,
                        )
                    },
                    "OpenThread(preflight)",
                )?;
                // SAFETY: verify the same handle before any suspension; TID reuse
                // must never lead to touching an unrelated process.
                if unsafe { GetProcessIdOfThread(thread.0) } != process.pinned.pid {
                    return Err("preflight thread owner mismatch".into());
                }
                process.verify()?;
                let mut suspension = Suspension::new(&thread)?;
                let observed = context(&thread, CONTEXT_DEBUG_REGISTERS_AMD64);
                suspension.resume()?;
                let observed = observed?;
                if !registers(&observed).unused() {
                    return Err(format!(
                        "pre-existing debug registers on thread {}",
                        entry.th32ThreadID
                    ));
                }
                count += 1;
            }
            // SAFETY: same snapshot and sized entry.
            if unsafe { Thread32Next(snapshot.0, &mut entry) } == 0 {
                // SAFETY: capture enumeration error immediately.
                if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                    return Err(win_error("Thread32Next"));
                }
                break;
            }
        }
        if count == 0 {
            return Err("pinned process has no accessible threads".into());
        }
        process.verify()
    }

    struct Thread {
        handle: Handle,
        original: DebugRegisters,
        armed: bool,
        suspended: bool,
        cleanup_snapshot: Option<ProbeSnapshot>,
    }

    struct Pending {
        event: DEBUG_EVENT,
        status: i32,
    }

    struct Session {
        process: Process,
        module: PinnedModule,
        threads: BTreeMap<u32, Thread>,
        event_threads: BTreeMap<u32, Handle>,
        debug_process: Option<Handle>,
        exceptions: ExceptionPolicy,
        owns_probes: bool,
        cleanup_exception_records: Vec<Value>,
        module_event_seen: bool,
        ready: bool,
        pending: Option<Pending>,
        cleaned: bool,
        observation: Observation,
        installed_total: u32,
        detach_error: Option<String>,
    }

    impl Session {
        fn attach(process: Process, module: PinnedModule) -> Result<Self> {
            preflight_threads(&process)?;
            module.verify(&process)?;
            process.verify()?;
            // SAFETY: only the PID whose creation/path/module were checked
            // against the still-owned process handle immediately above.
            if unsafe { DebugActiveProcess(process.pinned.pid) } == 0 {
                return Err(win_error("DebugActiveProcess"));
            }
            // No fallible operation between successful attach and its cleanup
            // owner. All subsequent errors pass through Session::finish/Drop.
            Ok(Self {
                process,
                module,
                threads: BTreeMap::new(),
                event_threads: BTreeMap::new(),
                debug_process: None,
                exceptions: ExceptionPolicy::default(),
                owns_probes: false,
                cleanup_exception_records: Vec::new(),
                module_event_seen: false,
                ready: false,
                pending: None,
                cleaned: false,
                observation: Observation::default(),
                installed_total: 0,
                detach_error: None,
            })
        }

        fn finish(&mut self) -> CleanupReport {
            self.cleaned = true;
            cleanup(self)
        }

        fn add_thread(&mut self, tid: u32, raw: HANDLE) -> Result<()> {
            let event_handle = Handle::new(raw, "debug event thread handle")?;
            if self.event_threads.contains_key(&tid) {
                return Err("duplicate live debug thread ID".into());
            }
            self.event_threads.insert(tid, event_handle);
            // Own a separate minimal-rights handle; Windows only auto-closes
            // the original event handle. It cannot close this cleanup handle.
            // SAFETY: explicit event TID, owner verified before any context set.
            let handle = Handle::new(
                unsafe {
                    OpenThread(
                        THREAD_GET_CONTEXT
                            | THREAD_SET_CONTEXT
                            | THREAD_SUSPEND_RESUME
                            | THREAD_QUERY_LIMITED_INFORMATION
                            | THREAD_SYNCHRONIZE,
                        0,
                        tid,
                    )
                },
                "OpenThread(debug event)",
            )?;
            // SAFETY: query the same owned thread handle before modifying it.
            if unsafe { GetProcessIdOfThread(handle.0) } != self.process.pinned.pid {
                return Err("debug-event thread owner mismatch".into());
            }
            self.threads.insert(
                tid,
                Thread {
                    handle,
                    original: DebugRegisters::default(),
                    armed: false,
                    suspended: false,
                    cleanup_snapshot: None,
                },
            );
            let thread = self.threads.get_mut(&tid).ok_or("missing new thread")?;
            let value = context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?;
            let original = registers(&value);
            thread.original = original;
            if !original.unused() {
                return Err(format!(
                    "pre-existing debug registers on attached/new thread {tid}"
                ));
            }
            if self.ready && !self.cleaned {
                self.install(tid)?;
            }
            Ok(())
        }

        fn install(&mut self, tid: u32) -> Result<()> {
            let thread = self
                .threads
                .get_mut(&tid)
                .ok_or("unknown thread to install")?;
            let mut value = context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?;
            if registers(&value) != thread.original || !thread.original.unused() {
                return Err("debug registers changed before installation".into());
            }
            replace_registers(&mut value, thread.original.installed(self.module.addresses));
            // Record intent BEFORE the API; even a failed/partial set must restore.
            thread.armed = true;
            self.owns_probes = true;
            set_context(&thread.handle, value)?;
            let installed = registers(&context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?);
            if installed.address != self.module.addresses || installed.dr7 & !0x400 != 0x55 {
                return Err("hardware probe installation readback mismatch".into());
            }
            self.installed_total += 1;
            Ok(())
        }

        fn observe(
            &mut self,
            args: &Args,
            output: &mut File,
            cancelled: &AtomicBool,
        ) -> Result<&'static str> {
            // SAFETY: called on the attaching/debug-event thread, immediately
            // before any event is continued. Never allow debugger exit to kill Civ6.
            if unsafe { DebugSetProcessKillOnExit(0) } == 0 {
                return Err(win_error("DebugSetProcessKillOnExit(false)"));
            }
            let deadline = Instant::now() + Duration::from_secs(args.seconds);
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    return Ok("cancelled");
                }
                let now = Instant::now();
                if now >= deadline {
                    return Ok("deadline");
                }
                let timeout = (deadline - now).as_millis().clamp(1, 100) as u32;
                let mut event = DEBUG_EVENT::default();
                // SAFETY: bounded wait on the same thread that attached.
                if unsafe { WaitForDebugEvent(&mut event, timeout) } == 0 {
                    // SAFETY: capture wait error before other API calls.
                    if unsafe { GetLastError() } == ERROR_SEM_TIMEOUT {
                        if !self.process.handle.alive()? {
                            return Ok("process_exited");
                        }
                        continue;
                    }
                    return Err(win_error("WaitForDebugEvent"));
                }
                // Own the event before any validation/I/O can fail. Exception
                // status starts NOT_HANDLED and only our positively identified
                // attach break/hardware probe can change it.
                self.pending = Some(Pending {
                    event,
                    status: if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                        DBG_EXCEPTION_NOT_HANDLED
                    } else {
                        DBG_CONTINUE
                    },
                });
                let reason = self.event(args, output, &event)?;
                if let Some(reason) = reason {
                    // Keep this event stopped until every original DR is restored.
                    return Ok(reason);
                }
                self.continue_pending()?;
            }
        }

        fn event(
            &mut self,
            args: &Args,
            output: &mut File,
            event: &DEBUG_EVENT,
        ) -> Result<Option<&'static str>> {
            if event.dwProcessId != self.process.pinned.pid {
                return Err("debug event belongs to an unexpected process".into());
            }
            match event.dwDebugEventCode {
                CREATE_PROCESS_DEBUG_EVENT => {
                    // SAFETY: union arm selected by the debug event code.
                    let info = unsafe { event.u.CreateProcessInfo };
                    // Acquire all supplied handles before verification can fail.
                    let image_file = if info.hFile.is_null() {
                        None
                    } else {
                        Some(Handle::new(info.hFile, "debug image file")?)
                    };
                    self.debug_process = Some(Handle::new(info.hProcess, "debug process")?);
                    self.exceptions.attach.initial_tid = Some(event.dwThreadId);
                    let thread_result = self.add_thread(event.dwThreadId, info.hThread);
                    let actual = identity(
                        self.debug_process
                            .as_ref()
                            .ok_or("missing debug process")?
                            .0,
                    )?;
                    match_identity(&self.process.pinned, &actual, self.process.handle.alive()?)?;
                    if info.lpStartAddress.is_some() {
                        return Err("event is not an existing-process attach".into());
                    }
                    if let Some(image_file) = image_file {
                        if path_key(&file_path(image_file.0)?)? != self.process.pinned.image_path {
                            return Err("debug image file path mismatch".into());
                        }
                    }
                    thread_result?;
                }
                CREATE_THREAD_DEBUG_EVENT => {
                    // SAFETY: arm selected by event code. All threads are stopped.
                    self.add_thread(event.dwThreadId, unsafe { event.u.CreateThread.hThread })?;
                }
                LOAD_DLL_DEBUG_EVENT => {
                    // SAFETY: arm selected by event code; close every hFile,
                    // including unrelated DLLs, without reading their memory.
                    let info = unsafe { event.u.LoadDll };
                    let file = if info.hFile.is_null() {
                        None
                    } else {
                        Some(Handle::new(info.hFile, "debug DLL file")?)
                    };
                    if info.lpBaseOfDll as u64 == self.module.identity.base {
                        let file = file.ok_or("pinned DLL debug event has no file handle")?;
                        if path_key(&file_path(file.0)?)? != self.module.identity.path {
                            return Err("DLL debug-event file path/base mismatch".into());
                        }
                        self.module_event_seen = true;
                    }
                }
                // SAFETY: the pattern selects the union arm before the guard.
                UNLOAD_DLL_DEBUG_EVENT
                    if unsafe { event.u.UnloadDll.lpBaseOfDll } as u64
                        == self.module.identity.base =>
                {
                    return Err("pinned DLL unloaded".into());
                }
                EXIT_THREAD_DEBUG_EVENT => {
                    // Original event handle is retired only after Continue succeeds.
                }
                EXIT_PROCESS_DEBUG_EVENT => {
                    // SAFETY: arm selected by event code, no memory read.
                    write_json(
                        output,
                        &json!({"event":"target_exit", "raw_exit_code": unsafe { event.u.ExitProcess.dwExitCode }}),
                    )?;
                    return Ok(Some("process_exited"));
                }
                EXCEPTION_DEBUG_EVENT => return self.exception(args, output, event),
                RIP_EVENT => return Err("Windows reported a debugger RIP event".into()),
                _ => {} // no remote reads of debug strings, names, or whole objects
            }
            Ok(None)
        }

        fn apply_decision(&mut self, decision: ExceptionDecision) {
            if let Some(pending) = &mut self.pending {
                pending.status = if decision.handled() {
                    DBG_CONTINUE
                } else {
                    DBG_EXCEPTION_NOT_HANDLED
                };
            }
        }

        fn exception(
            &mut self,
            args: &Args,
            output: &mut File,
            event: &DEBUG_EVENT,
        ) -> Result<Option<&'static str>> {
            // SAFETY: caller selected EXCEPTION_DEBUG_EVENT.
            let exception = unsafe { event.u.Exception };
            let code = exception.ExceptionRecord.ExceptionCode;
            let raw = RawException {
                tid: event.dwThreadId,
                code: code as u32,
                first_chance: exception.dwFirstChance,
                exception_address: exception.ExceptionRecord.ExceptionAddress as u64,
            };
            let thread = self.threads.get(&event.dwThreadId);
            let context_result = if code == EXCEPTION_SINGLE_STEP && self.owns_probes {
                thread.map(|thread| {
                    context(
                        &thread.handle,
                        CONTEXT_DEBUG_REGISTERS_AMD64
                            | CONTEXT_CONTROL_AMD64
                            | CONTEXT_INTEGER_AMD64,
                    )
                })
            } else {
                None
            };
            let context_error = context_result
                .as_ref()
                .and_then(|result| result.as_ref().err());
            let snapshot = context_result
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(|value| ProbeSnapshot {
                    armed: thread.is_some_and(|thread| thread.armed),
                    registers: registers(value),
                    rip: value.Rip,
                    flags: value.EFlags,
                });
            let decision =
                self.exceptions
                    .decide(raw, snapshot, self.module.addresses, self.owns_probes);
            // Set continuation BEFORE logging, raw reads, or context updates.
            // The same policy is used when draining queued events in cleanup.
            self.apply_decision(decision);
            if decision == ExceptionDecision::AttachBreak {
                // Attach's synthetic break is the only software exception we handle.
                if !self.module_event_seen {
                    return Err("attach break arrived without the pinned DLL event".into());
                }
                self.process.verify()?;
                self.module.verify(&self.process)?;
                let tids: Vec<_> = self.threads.keys().copied().collect();
                if tids.is_empty() {
                    return Err("no attach threads were reported".into());
                }
                for tid in tids {
                    self.install(tid)?;
                }
                self.ready = true;
                let ready = json!({"event":"ready", "pid":args.pid, "created":args.created,
                    "image_path":self.process.pinned.image_path, "module":self.module.identity,
                    "module_file_sha256":MODULE_SHA256, "probe_rvas":RVAS,
                    "threads_installed":self.installed_total, "ready_path":args.ready_path(),
                    "semantics":"raw_fields_only"});
                let mut marker = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(args.ready_path())
                    .map_err(|e| format!("create_new ready file: {e}"))?;
                write_json(&mut marker, &ready)?;
                write_json(output, &ready)?;
                write_json(&mut std::io::stdout().lock(), &ready)?;
                return Ok(None);
            }
            if let ExceptionDecision::ForwardFirstChance { record } = decision {
                if record {
                    write_json(
                        output,
                        &json!({"event":"forwarded_first_chance", "raw":raw,
                    "context_error":context_error, "continuation":"not_handled"}),
                    )?;
                }
                return Ok(None);
            }
            if decision == ExceptionDecision::ForwardSecondChance {
                write_json(
                    output,
                    &json!({"event":"forwarded_second_chance", "raw":raw,
                    "continuation":"not_handled"}),
                )?;
                return Ok(Some("second_chance_exception"));
            }
            let mismatch = (decision == ExceptionDecision::RemoveOwnedProbe).then(|| {
                json!({"event":"owned_probe_identity_mismatch", "raw":raw,
                    "raw_context":snapshot, "context_error":context_error,
                    "continuation":"handled_remove_our_probes"})
            });
            let mut value = match context_result.and_then(std::result::Result::ok) {
                Some(value) => value,
                None => {
                    if let Some(mismatch) = &mismatch {
                        write_json(output, mismatch)?;
                    }
                    return Ok(Some("owned_probe_context_unavailable"));
                }
            };
            value.ContextFlags = CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64;
            let (dr6, flags) = decision
                .resume_debug_flags(value.Dr6, value.EFlags)
                .ok_or("exception policy did not authorize probe resume")?;
            value.Dr6 = dr6;
            value.EFlags = flags;
            set_context(
                &self
                    .threads
                    .get(&event.dwThreadId)
                    .ok_or("thread disappeared")?
                    .handle,
                value,
            )?;
            // RF and all four B bits are updated before any mismatch-log I/O
            // can fail. A logging error still continues an owned trap safely.
            if let Some(mismatch) = &mismatch {
                write_json(output, mismatch)?;
            }
            if decision.stop_observing() {
                return Ok(Some("owned_probe_identity_mismatch"));
            }
            let ExceptionDecision::Probe(slot) = decision else {
                return Err("unexpected observation decision".into());
            };
            let fields = match slot {
                0 if self.observation.setter(
                    value.Rbx,
                    value.Rcx,
                    event.dwThreadId,
                    value.Rsp,
                )? =>
                {
                    let trigger = self.observation.trigger.ok_or("missing setter trigger")?;
                    Some(json!({"event":"setter_before", "trigger":trigger,
                        "raw_rdx_pointer":value.Rdx, "raw_delta_i32":raw_read(self.process.i32_at(value.Rdx, 0)),
                        "fields":self.process.cache_fields(trigger)}))
                }
                1 if self
                    .observation
                    .returned(value.Rbx, event.dwThreadId, value.Rsp) =>
                {
                    Some(
                        json!({"event":"setter_return", "trigger":self.observation.trigger,
                        "fields":self.process.cache_fields(self.observation.trigger.ok_or("missing trigger")?)}),
                    )
                }
                2 if self.observation.getter() => Some(
                    json!({"event":"getter_raw", "ordinal":self.observation.getters_recorded,
                        "raw_rax_pointer":value.Rax, "raw_rax_plus_8_i32":raw_read(self.process.i32_at(value.Rax, 8)),
                        "correlation":"after_first_setter_return_only", "city_identity_verified":false}),
                ),
                3 if self.observation.candidate(value.Rbx, event.dwThreadId) => {
                    let record = self.process.read::<8>(value.Rdx);
                    let header = self.process.read::<8>(value.Rcx);
                    Some(
                        json!({"event":"candidate_call_raw", "ordinal":self.observation.candidates_recorded,
                        "raw_rbx_receiver":value.Rbx, "raw_rdx_pointer":value.Rdx, "raw_rcx_pointer":value.Rcx,
                        "raw_record_bytes":raw_read(record.clone()), "raw_header_bytes":raw_read(header.clone()),
                        "raw_record_0_u16":raw_read(record.clone().map(|r| u16::from_le_bytes([r[0], r[1]]))),
                        "raw_record_2_u16":raw_read(record.clone().map(|r| u16::from_le_bytes([r[2], r[3]]))),
                        "raw_record_4_u32":raw_read(record.map(|r| u32::from_le_bytes([r[4], r[5], r[6], r[7]]))),
                        "raw_header_0_u32":raw_read(header.clone().map(|h| u32::from_le_bytes([h[0], h[1], h[2], h[3]]))),
                        "raw_header_4_u32":raw_read(header.map(|h| u32::from_le_bytes([h[4], h[5], h[6], h[7]]))),
                        "raw_receiver_plus_90_pointer":raw_read(self.process.u64_at(value.Rbx, 0x90))}),
                    )
                }
                _ => None,
            };
            if let Some(mut fields) = fields {
                fields["tid"] = json!(event.dwThreadId);
                fields["raw_rip"] = json!(value.Rip);
                fields["probe_rva"] = json!(RVAS[slot]);
                write_json(output, &fields)?;
            }
            if self.observation.another_setter_for_trigger_cache {
                return Ok(Some("another_setter_for_trigger_cache"));
            }
            if self.observation.getters_recorded >= GETTER_LIMIT {
                return Ok(Some("getter_limit"));
            }
            if self.observation.candidates_recorded >= CANDIDATE_LIMIT {
                return Ok(Some("candidate_limit"));
            }
            Ok(None)
        }
    }

    impl CleanupBackend for Session {
        fn freeze(&mut self) -> Vec<String> {
            let mut errors = Vec::new();
            for (&tid, thread) in &mut self.threads {
                if !thread.armed {
                    continue;
                }
                match thread.handle.alive() {
                    Ok(false) => continue,
                    Err(error) => {
                        errors.push(format!("freeze thread {tid}: {error}"));
                        continue;
                    }
                    Ok(true) => {}
                }
                // SAFETY: same owned target thread handle. Retain exactly one
                // suspend count across restore, event drainage, and detach.
                if unsafe { SuspendThread(thread.handle.0) } == u32::MAX {
                    errors.push(format!(
                        "freeze thread {tid}: {}",
                        win_error("SuspendThread")
                    ));
                    continue;
                }
                thread.suspended = true;
                match context(
                    &thread.handle,
                    CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64,
                ) {
                    Ok(value) => {
                        // Preserve the pre-restore evidence. The shared policy
                        // will select a slot even when B0..B3 have accumulated.
                        thread.cleanup_snapshot = Some(ProbeSnapshot {
                            armed: thread.armed,
                            registers: registers(&value),
                            rip: value.Rip,
                            flags: value.EFlags,
                        });
                    }
                    Err(error) => {
                        errors.push(format!("capture queued probe on thread {tid}: {error}"))
                    }
                }
            }
            errors
        }

        fn armed_threads(&self) -> Vec<u32> {
            self.threads
                .iter()
                .filter_map(|(&tid, thread)| thread.armed.then_some(tid))
                .collect()
        }

        fn restore(&mut self, tid: u32) -> Result<RestoreOutcome> {
            let thread = self.threads.get_mut(&tid).ok_or("missing cleanup thread")?;
            if !thread.handle.alive()? {
                thread.armed = false;
                return Ok(RestoreOutcome::ThreadExited);
            }
            if !thread.suspended {
                // SAFETY: retry the reversible freeze on this same target
                // handle; a failed freeze must not lead to setting a running context.
                if unsafe { SuspendThread(thread.handle.0) } == u32::MAX {
                    let error = win_error("SuspendThread(restore)");
                    if !thread.handle.alive()? {
                        return Ok(RestoreOutcome::ThreadExited);
                    }
                    return Err(error);
                }
                thread.suspended = true;
            }
            let mut value = context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?;
            replace_registers(&mut value, thread.original);
            set_context(&thread.handle, value)?;
            if registers(&context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?)
                != thread.original
            {
                return Err("original debug-register readback mismatch".into());
            }
            thread.armed = false;
            Ok(RestoreOutcome::Restored)
        }

        fn continue_pending(&mut self) -> Result<()> {
            let Some(pending) = &self.pending else {
                return Ok(());
            };
            let event = pending.event;
            // SAFETY: exactly the event returned to this attaching thread;
            // status was defaulted conservatively before any fallible handler.
            if unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, pending.status) }
                == 0
            {
                return Err(win_error("ContinueDebugEvent"));
            }
            self.pending = None;
            if matches!(
                event.dwDebugEventCode,
                EXIT_THREAD_DEBUG_EVENT | EXIT_PROCESS_DEBUG_EVENT
            ) {
                if let Some(handle) = self.event_threads.remove(&event.dwThreadId) {
                    handle.closed_by_continue();
                }
                self.threads.remove(&event.dwThreadId);
            }
            if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                if let Some(process) = self.debug_process.take() {
                    process.closed_by_continue();
                }
            }
            Ok(())
        }

        fn drain_events(&mut self) -> Result<()> {
            if self.pending.is_some() {
                return Err("cannot drain events after a failed ContinueDebugEvent".into());
            }
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut errors = Vec::new();
            // All threads with our probes retain their own suspend count, so
            // they cannot generate more hits. A previously queued hit must be
            // continued after its DRs were restored, before detaching.
            for _ in 0..8192 {
                let mut event = DEBUG_EVENT::default();
                // SAFETY: nonblocking drain on the same attaching thread.
                if unsafe { WaitForDebugEvent(&mut event, 0) } == 0 {
                    // SAFETY: capture this API's error immediately.
                    if unsafe { GetLastError() } != ERROR_SEM_TIMEOUT {
                        errors.push(win_error("WaitForDebugEvent(cleanup)"));
                    }
                    return if errors.is_empty() {
                        Ok(())
                    } else {
                        Err(errors.join("; "))
                    };
                }
                self.pending = Some(Pending {
                    event,
                    status: if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                        DBG_EXCEPTION_NOT_HANDLED
                    } else {
                        DBG_CONTINUE
                    },
                });
                let handled = (|| -> Result<()> {
                    if event.dwProcessId != self.process.pinned.pid {
                        return Err("unexpected process in cleanup event".into());
                    }
                    match event.dwDebugEventCode {
                        CREATE_PROCESS_DEBUG_EVENT => {
                            // SAFETY: event code selects this union arm.
                            let info = unsafe { event.u.CreateProcessInfo };
                            let _file = if info.hFile.is_null() {
                                None
                            } else {
                                Some(Handle::new(info.hFile, "cleanup image file")?)
                            };
                            self.debug_process =
                                Some(Handle::new(info.hProcess, "cleanup process handle")?);
                            self.event_threads.insert(
                                event.dwThreadId,
                                Handle::new(info.hThread, "cleanup initial thread")?,
                            );
                            self.exceptions.attach.initial_tid = Some(event.dwThreadId);
                        }
                        CREATE_THREAD_DEBUG_EVENT => {
                            // SAFETY: event code selects this union arm. Never
                            // install probes while cleaning up a queued new thread.
                            self.event_threads.insert(
                                event.dwThreadId,
                                Handle::new(
                                    unsafe { event.u.CreateThread.hThread },
                                    "cleanup new thread",
                                )?,
                            );
                        }
                        LOAD_DLL_DEBUG_EVENT => {
                            // SAFETY: file handle from this union arm is ours.
                            let file = unsafe { event.u.LoadDll.hFile };
                            if !file.is_null() {
                                let _file = Handle::new(file, "cleanup DLL file")?;
                            }
                        }
                        EXCEPTION_DEBUG_EVENT => {
                            // SAFETY: event code selects this union arm.
                            let exception = unsafe { event.u.Exception };
                            let raw = RawException {
                                tid: event.dwThreadId,
                                code: exception.ExceptionRecord.ExceptionCode as u32,
                                first_chance: exception.dwFirstChance,
                                exception_address: exception.ExceptionRecord.ExceptionAddress
                                    as u64,
                            };
                            let snapshot = self
                                .threads
                                .get(&event.dwThreadId)
                                .and_then(|thread| thread.cleanup_snapshot);
                            let decision = self.exceptions.decide(
                                raw,
                                snapshot,
                                self.module.addresses,
                                self.owns_probes,
                            );
                            self.apply_decision(decision);
                            if matches!(
                                decision,
                                ExceptionDecision::ForwardFirstChance { record: true }
                                    | ExceptionDecision::ForwardSecondChance
                            ) {
                                self.cleanup_exception_records
                                    .push(json!({"raw":raw, "decision":decision,
                                    "continuation":"not_handled"}));
                            }
                            if matches!(
                                decision,
                                ExceptionDecision::Probe(_) | ExceptionDecision::RemoveOwnedProbe
                            ) {
                                let handle = self
                                    .threads
                                    .get(&event.dwThreadId)
                                    .map(|thread| &thread.handle)
                                    .or_else(|| self.event_threads.get(&event.dwThreadId))
                                    .ok_or("owned cleanup probe has no thread handle")?;
                                // Our DR addresses are already restored. Only
                                // clear B0..B3 and add RF to the current context;
                                // never put the captured installed DRs back.
                                let mut value = context(
                                    handle,
                                    CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64,
                                )?;
                                let (dr6, flags) = decision
                                    .resume_debug_flags(value.Dr6, value.EFlags)
                                    .ok_or("cleanup policy did not authorize RF")?;
                                value.Dr6 = dr6;
                                value.EFlags = flags;
                                set_context(handle, value)?;
                            }
                        }
                        _ => {}
                    }
                    Ok(())
                })();
                if let Err(error) = handled {
                    errors.push(error);
                }
                // Continue is attempted even if capture/validation failed.
                if let Err(error) = self.continue_pending() {
                    errors.push(error);
                    return Err(errors.join("; "));
                }
                if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                    return if errors.is_empty() {
                        Ok(())
                    } else {
                        Err(errors.join("; "))
                    };
                }
                // A genuine second-chance target exception ends observation;
                // it was continued NOT_HANDLED, never converted into read_error.
                if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                    // SAFETY: selected union arm.
                    if unsafe { event.u.Exception.dwFirstChance } == 0 {
                        return if errors.is_empty() {
                            Ok(())
                        } else {
                            Err(errors.join("; "))
                        };
                    }
                }
                if Instant::now() >= deadline {
                    break;
                }
            }
            errors.push("bounded cleanup event-drain limit reached".into());
            Err(errors.join("; "))
        }

        fn detach(&mut self) -> Result<bool> {
            // SAFETY: same PID/debugger thread; no termination or memory write.
            if unsafe { DebugActiveProcessStop(self.process.pinned.pid) } != 0 {
                return Ok(true);
            }
            let error = win_error("DebugActiveProcessStop");
            self.detach_error = Some(error.clone());
            if !self.process.handle.alive()? {
                // Still report the API's actual failure, without calling an
                // already exited target a successful detach.
                return Ok(false);
            }
            Err(error)
        }

        fn detach_diagnostic(&self) -> Option<String> {
            self.detach_error.clone()
        }

        fn resume(&mut self) -> Vec<String> {
            let mut errors = Vec::new();
            for (&tid, thread) in &mut self.threads {
                if !thread.suspended {
                    continue;
                }
                // SAFETY: remove exactly the suspend count owned by cleanup,
                // after detach has released the debugger's separate event stop.
                if unsafe { ResumeThread(thread.handle.0) } == u32::MAX {
                    let error = win_error("ResumeThread");
                    if thread.handle.alive() != Ok(false) {
                        errors.push(format!("resume thread {tid}: {error}"));
                    }
                } else {
                    thread.suspended = false;
                }
            }
            errors
        }

        fn process_exited(&self) -> bool {
            self.process.handle.alive() == Ok(false)
        }
    }

    impl Drop for Session {
        fn drop(&mut self) {
            if !self.cleaned {
                let report = self.finish();
                // Fallback after an unexpected unwind, independent of file I/O.
                eprintln!("debug session cleanup: {report:?}");
            }
        }
    }

    fn write_json(writer: &mut impl Write, value: &Value) -> Result<()> {
        serde_json::to_writer(&mut *writer, value).map_err(|e| format!("write JSON: {e}"))?;
        writer
            .write_all(b"\n")
            .map_err(|e| format!("write JSON newline: {e}"))?;
        writer.flush().map_err(|e| format!("flush JSON: {e}"))
    }

    pub fn run(args: &Args) -> Result<()> {
        let setup = (|| -> Result<File> {
            args.validate()?;
            match std::fs::symlink_metadata(args.ready_path()) {
                Ok(_) => return Err("ready file already exists; use a new output name".into()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("check new ready path: {error}")),
            }
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&args.output)
                .map_err(|e| format!("create_new output: {e}"))
        })();
        let mut output = match setup {
            Ok(file) => file,
            Err(error) => {
                write_json(
                    &mut std::io::stdout().lock(),
                    &json!({"event":"exit", "reason":"error",
                    "error":error, "ready_reached":false, "cleanup_complete":true,
                    "attach_attempted":false, "semantics":"raw_fields_only", "product_acceptance":false}),
                )?;
                return Err(error);
            }
        };
        let started = Instant::now();
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&cancelled);
        let mut session = None;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            ctrlc::set_handler(move || {
                signal.store(true, Ordering::Relaxed);
            })
            .map_err(|e| format!("install cancellation handler: {e}"))?;
            let process = Process::open(args)?;
            let module = PinnedModule::open(&process)?;
            session = Some(Session::attach(process, module)?);
            session
                .as_mut()
                .ok_or("missing debug session")?
                .observe(args, &mut output, &cancelled)
        }))
        .unwrap_or_else(|_| Err("observer panicked; running centralized cleanup".into()));
        // The only orderly cleanup path, before any completion output can fail.
        let cleanup_report = session.as_mut().map(Session::finish);
        let complete = cleanup_report.as_ref().is_none_or(CleanupReport::complete);
        let exit = json!({"event":"exit", "pid":args.pid, "created":args.created,
            "reason":outcome.as_ref().copied().unwrap_or("error"), "error":outcome.as_ref().err(),
            "ready_reached":session.as_ref().is_some_and(|s| s.ready),
            "elapsed_ms":started.elapsed().as_millis(), "cleanup_complete":complete,
            "cleanup":cleanup_report, "observation":session.as_ref().map(|s| &s.observation),
            "threads_installed_total":session.as_ref().map_or(0, |s| s.installed_total),
            "exceptions":session.as_ref().map(|s| &s.exceptions),
            "cleanup_exception_records":session.as_ref().map(|s| &s.cleanup_exception_records),
            "semantics":"raw_fields_only", "product_acceptance":false});
        let file_result = write_json(&mut output, &exit);
        let stdout_result = write_json(&mut std::io::stdout().lock(), &exit);
        file_result?;
        stdout_result?;
        outcome?;
        if !complete {
            return Err("debug session cleanup incomplete; inspect exit report".into());
        }
        Ok(())
    }
}

fn main() {
    let args = Args::parse();
    #[cfg(all(windows, target_arch = "x86_64"))]
    let result = platform::run(&args);
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    let result: Result<()> = {
        let _ = args;
        Err("loyalty-native-observer CLI is available only on x64 Windows".into())
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{policy::*, *};

    fn identity() -> Identity {
        Identity {
            pid: 123,
            created: 0x0123456789abcdef,
            image_path: "pinned-game-image".into(),
        }
    }

    #[test]
    fn pid_reuse_exit_and_image_swap_are_rejected() {
        let pinned = identity();
        assert!(match_identity(&pinned, &pinned, true).is_ok());
        assert!(match_identity(&pinned, &pinned, false).is_err());
        for other in [
            Identity {
                pid: 124,
                ..pinned.clone()
            },
            Identity {
                created: pinned.created + 1,
                ..pinned.clone()
            },
            Identity {
                image_path: "another-image".into(),
                ..pinned.clone()
            },
        ] {
            assert!(match_identity(&pinned, &other, true).is_err());
        }
    }

    #[test]
    fn image_layout_and_aslr_range_are_pinned() {
        for name in ["CivilizationVI.exe", "CivilizationVI_DX12.exe"] {
            let image = PathBuf::from("game/Base/Binaries/Win64Steam").join(name);
            assert_eq!(
                expected_module(&image).unwrap(),
                PathBuf::from("game/DLC/Expansion2/Binaries/Win64").join(MODULE_NAME)
            );
        }
        assert!(expected_module(Path::new("game/Base/Binaries/Win64Steam/Other.exe")).is_err());
        assert!(expected_module(Path::new("game/CivilizationVI.exe")).is_err());
        assert!(breakpoint_addresses(u64::MAX - 1, u32::MAX).is_err());
        assert!(breakpoint_addresses(0x180000000, RVAS[0] as u32).is_err());
        let base = 0x7ffd00000000;
        assert_eq!(
            breakpoint_addresses(base, 0x800000).unwrap(),
            RVAS.map(|rva| base + rva)
        );
        assert_eq!(MODULE_SHA256.len(), 64);
    }

    #[test]
    fn only_unused_debug_registers_can_be_overlaid() {
        for original in [
            DebugRegisters::default(),
            DebugRegisters {
                dr6: 0xFFFF0FF0,
                dr7: 0x400,
                ..Default::default()
            },
        ] {
            assert!(original.unused());
            let installed = original.installed([11, 22, 33, 44]);
            assert_eq!(installed.dr6, original.dr6);
            assert_eq!(installed.dr7 & !0x400, 0x55);
        }
        for registers in [
            DebugRegisters {
                address: [0, 0, 1, 0],
                ..Default::default()
            },
            DebugRegisters {
                dr6: 1,
                ..Default::default()
            },
            DebugRegisters {
                dr6: 0x4000,
                ..Default::default()
            },
            DebugRegisters {
                dr7: 1,
                ..Default::default()
            },
            DebugRegisters {
                dr7: 1 << 16,
                ..Default::default()
            },
        ] {
            assert!(!registers.unused());
        }
    }

    #[test]
    fn hardware_hits_select_rip_slot_and_exclude_foreign_step() {
        let addresses = [11, 22, 33, 44];
        let installed = DebugRegisters::default().installed(addresses);
        for slot in 0..4 {
            let hit = DebugRegisters {
                dr6: 1 << slot,
                ..installed
            };
            assert_eq!(hit.own_hit(addresses, addresses[slot], 0), Some(slot));
            assert_eq!(hit.own_hit(addresses, addresses[(slot + 1) % 4], 0), None);
            assert_eq!(hit.own_hit(addresses, addresses[slot], 0x100), None);
            assert_eq!(
                DebugRegisters {
                    dr6: hit.dr6 | 0x4000,
                    ..hit
                }
                .own_hit(addresses, addresses[slot], 0),
                None
            );
        }
        assert_eq!(
            DebugRegisters {
                dr6: 3,
                ..installed
            }
            .own_hit(addresses, 11, 0),
            Some(0)
        );
        assert_eq!(installed.own_hit(addresses, 11, 0), None);
        assert_eq!(
            DebugRegisters {
                dr6: 1,
                dr7: 0,
                ..installed
            }
            .own_hit(addresses, 11, 0),
            None
        );
        assert_eq!(RF, 0x10000);
    }

    #[test]
    fn accumulated_dr6_slot_zero_then_one_uses_rip_and_clears_all_b_bits() {
        let addresses = [11, 22, 33, 44];
        let mut policy = ExceptionPolicy::default();
        for (slot, dr6) in [(0, 0xFFFF0FF1), (1, 0xFFFF0FF3)] {
            let exception = RawException {
                tid: 9,
                code: 0x80000004,
                first_chance: 1,
                exception_address: addresses[slot],
            };
            let snapshot = ProbeSnapshot {
                armed: true,
                registers: DebugRegisters {
                    dr6,
                    ..DebugRegisters::default().installed(addresses)
                },
                rip: addresses[slot],
                flags: 0x202,
            };
            let decision = policy.decide(exception, Some(snapshot), addresses, true);
            assert_eq!(decision, ExceptionDecision::Probe(slot));
            assert!(decision.handled());
            assert!(!decision.stop_observing());
            assert_eq!(
                decision.resume_debug_flags(dr6, 0x202),
                Some((0xFFFF0FF0, 0x10202))
            );
        }
        assert_eq!(policy.unknown_first_chance_count, 0);
    }

    #[test]
    fn inconsistent_owned_address_is_removed_but_foreign_addresses_are_forwarded() {
        let addresses = [11, 22, 33, 44];
        let exception = RawException {
            tid: 9,
            code: 0x80000004,
            first_chance: 1,
            exception_address: 22,
        };
        let snapshot = ProbeSnapshot {
            armed: true,
            registers: DebugRegisters {
                address: [11, 99, 33, 44],
                dr6: 3,
                dr7: 0x55,
            },
            rip: 22,
            flags: 0x202,
        };
        let mut policy = ExceptionPolicy::default();
        for snapshot in [
            Some(snapshot),
            Some(ProbeSnapshot {
                armed: false,
                ..snapshot
            }),
            None,
        ] {
            let decision = policy.decide(exception, snapshot, addresses, true);
            assert_eq!(decision, ExceptionDecision::RemoveOwnedProbe);
            assert!(decision.handled());
            assert!(decision.stop_observing());
            assert_eq!(
                decision.resume_debug_flags(0xE003, 0x202),
                Some((0xE000, 0x10202))
            );
        }
        // ExceptionAddress mismatch while stopped at our RIP also removes only
        // our instrumentation. It must not forward a potentially owned trap.
        assert_eq!(
            policy.decide(
                RawException {
                    exception_address: 99,
                    ..exception
                },
                Some(snapshot),
                addresses,
                true
            ),
            ExceptionDecision::RemoveOwnedProbe
        );
        for owns_probes in [true, false] {
            let foreign = ProbeSnapshot {
                rip: 99,
                ..snapshot
            };
            let decision = policy.decide(
                RawException {
                    exception_address: 99,
                    ..exception
                },
                Some(foreign),
                addresses,
                owns_probes,
            );
            assert_eq!(
                decision,
                ExceptionDecision::ForwardFirstChance { record: true }
            );
            assert!(!decision.handled());
            assert!(!decision.stop_observing());
            assert_eq!(decision.resume_debug_flags(3, 0x202), None);
        }
        // A matching static RVA alone does not authorize a probe before any
        // hardware registers have actually been installed by this session.
        assert_eq!(
            policy.decide(exception, Some(snapshot), addresses, false),
            ExceptionDecision::ForwardFirstChance { record: true }
        );
    }

    #[test]
    fn unknown_first_chance_records_are_bounded_and_second_chance_ends_observation() {
        let mut policy = ExceptionPolicy::default();
        let addresses = [11, 22, 33, 44];
        for ordinal in 0..1000 {
            let code = if ordinal % 2 == 0 {
                0xE06D7363
            } else {
                0x406D1388
            };
            let exception = RawException {
                tid: 9,
                code,
                first_chance: 1,
                exception_address: 11,
            };
            let decision = policy.decide(exception, None, addresses, true);
            assert_eq!(
                decision,
                ExceptionDecision::ForwardFirstChance {
                    record: ordinal < 16
                }
            );
            assert!(!decision.handled());
            assert!(!decision.stop_observing());
        }
        assert_eq!(policy.unknown_first_chance_count, 1000);
        assert_eq!(
            policy.unknown_first_chance_records,
            FIRST_CHANCE_RECORD_LIMIT
        );
        let decision = policy.decide(
            RawException {
                tid: 9,
                code: 0xC0000005,
                first_chance: 0,
                exception_address: 11,
            },
            None,
            addresses,
            true,
        );
        assert_eq!(decision, ExceptionDecision::ForwardSecondChance);
        assert!(!decision.handled());
        assert!(decision.stop_observing());
        assert_eq!(policy.second_chance_count, 1);
    }

    #[test]
    fn raw_read_error_keeps_observing_and_is_not_a_genuine_access_exception() {
        let addresses = [11, 22, 33, 44];
        let mut policy = ExceptionPolicy::default();
        let mut observation = Observation::default();
        observation.setter(0x10000, 0x11630, 9, 100).unwrap();
        observation.returned(0x10000, 9, 100);
        let snapshot = ProbeSnapshot {
            armed: true,
            registers: DebugRegisters {
                dr6: 4,
                ..DebugRegisters::default().installed(addresses)
            },
            rip: 33,
            flags: 0x202,
        };
        for read_result in [Err("mock null or unreadable RAX field".into()), Ok(123i32)] {
            let decision = policy.decide(
                RawException {
                    tid: 9,
                    code: 0x80000004,
                    first_chance: 1,
                    exception_address: 33,
                },
                Some(snapshot),
                addresses,
                true,
            );
            assert_eq!(decision, ExceptionDecision::Probe(2));
            assert!(decision.handled());
            assert!(!decision.stop_observing());
            assert!(observation.getter());
            let raw = raw_read(read_result);
            if observation.getters_recorded == 1 {
                assert!(raw["read_error"].is_string());
            } else {
                assert_eq!(raw, serde_json::json!(123));
            }
        }
        assert_eq!(observation.getters_recorded, 2);
        assert_eq!(policy.unknown_first_chance_count, 0);
        let decision = policy.decide(
            RawException {
                tid: 9,
                code: 0xC0000005,
                first_chance: 1,
                exception_address: 33,
            },
            Some(snapshot),
            addresses,
            true,
        );
        assert_eq!(
            decision,
            ExceptionDecision::ForwardFirstChance { record: true }
        );
        assert!(!decision.handled()); // never convert a target AV into a read error
    }

    #[test]
    fn attach_break_is_only_consumed_once_on_the_initial_thread() {
        let mut policy = AttachBreak::default();
        assert!(!policy.consume(0x80000003, 1, 9));
        policy.initial_tid = Some(9);
        assert!(!policy.consume(0x80000003, 0, 9));
        assert!(!policy.consume(0x80000003, 1, 10));
        assert!(!policy.consume(0xc0000005, 1, 9));
        assert!(!policy.consume(0x80000004, 1, 9));
        assert!(policy.consume(0x80000003, 1, 9));
        // Failed setup before ready must not turn a later breakpoint into
        // another handled attach break during cleanup.
        assert!(!policy.consume(0x80000003, 1, 9));
        assert!(!policy.consume(0x80000004, 1, 9));
    }

    #[test]
    fn first_setter_pins_receiver_thread_and_stack_without_retargeting() {
        let mut state = Observation::default();
        assert!(state.setter(0x10000, 0x11630, 9, 100).unwrap());
        assert!(!state.setter(0x20000, 0x21630, 8, 200).unwrap());
        assert!(!state.returned(0x20000, 9, 100));
        assert!(!state.returned(0x10000, 8, 100));
        assert!(!state.returned(0x10000, 9, 200));
        assert!(!state.candidate(0x21630, 9));
        assert!(!state.candidate(0x11630, 8));
        assert!(state.candidate(0x11630, 9));
        assert!(state.returned(0x10000, 9, 100));
        assert!(!state.returned(0x10000, 9, 100));
        assert!(!state.candidate(0x11630, 9));
        assert_eq!(state.trigger.unwrap().cache_city, 0x10000);
        assert!(Observation::default().setter(u64::MAX, 0, 1, 1).is_err());
        assert!(Observation::default().setter(0, 0x1630, 1, 1).is_err());
    }

    #[test]
    fn repeated_setter_and_candidate_volume_end_the_first_call_window() {
        let mut state = Observation::default();
        state.setter(0x10000, 0x11630, 9, 100).unwrap();
        for _ in 0..CANDIDATE_LIMIT {
            assert!(state.candidate(0x11630, 9));
        }
        assert!(!state.candidate(0x11630, 9));
        assert_eq!(state.candidates_recorded, CANDIDATE_LIMIT);
        assert!(!state.setter(0x20000, 0x21630, 9, 110).unwrap());
        assert!(!state.another_setter_for_trigger_cache);
        assert!(!state.setter(0x10000, 0x11630, 9, 120).unwrap());
        assert!(state.another_setter_for_trigger_cache);
        assert_eq!(state.trigger.unwrap().rsp, 100);
    }

    #[test]
    fn natural_getters_do_not_consume_setter_log_budget() {
        let mut state = Observation::default();
        for _ in 0..10000 {
            assert!(!state.getter());
        }
        assert_eq!(state.getters_recorded, 0);
        state.setter(0x10000, 0x11630, 9, 100).unwrap();
        assert!(!state.getter()); // before the first wrapper return
        state.returned(0x10000, 9, 100);
        for _ in 0..GETTER_LIMIT {
            assert!(state.getter());
        }
        assert!(!state.getter());
        assert_eq!(state.getters_recorded, GETTER_LIMIT);
        assert_eq!(state.getters_before_return, 10001);
    }

    #[derive(Default)]
    struct MockCleanup {
        calls: Vec<String>,
        fail_freeze: bool,
        fail_restore: bool,
        fail_continue: bool,
        fail_drain: bool,
        fail_detach: bool,
        fail_resume: bool,
        exited: bool,
    }

    impl CleanupBackend for MockCleanup {
        fn freeze(&mut self) -> Vec<String> {
            self.calls.push("freeze".into());
            if self.fail_freeze {
                vec!["injected freeze failure".into()]
            } else {
                Vec::new()
            }
        }
        fn armed_threads(&self) -> Vec<u32> {
            vec![10, 20, 30]
        }
        fn restore(&mut self, tid: u32) -> Result<RestoreOutcome> {
            self.calls.push(format!("restore:{tid}"));
            if self.fail_restore && tid == 10 {
                Err("injected context failure".into())
            } else if tid == 20 {
                Ok(RestoreOutcome::ThreadExited)
            } else {
                Ok(RestoreOutcome::Restored)
            }
        }
        fn continue_pending(&mut self) -> Result<()> {
            self.calls.push("continue".into());
            if self.fail_continue {
                Err("injected continue failure".into())
            } else {
                Ok(())
            }
        }
        fn detach(&mut self) -> Result<bool> {
            self.calls.push("detach".into());
            if self.fail_detach {
                Err("injected detach failure".into())
            } else {
                Ok(!self.exited)
            }
        }
        fn drain_events(&mut self) -> Result<()> {
            self.calls.push("drain".into());
            if self.fail_drain {
                Err("injected drain failure".into())
            } else {
                Ok(())
            }
        }
        fn resume(&mut self) -> Vec<String> {
            self.calls.push("resume".into());
            if self.fail_resume {
                vec!["injected resume failure".into()]
            } else {
                Vec::new()
            }
        }
        fn process_exited(&self) -> bool {
            self.exited
        }
    }

    #[test]
    fn cleanup_restores_all_before_continue_and_detach_even_on_failure() {
        for failures in 0..64 {
            let mut mock = MockCleanup {
                fail_restore: failures & 1 != 0,
                fail_continue: failures & 2 != 0,
                fail_detach: failures & 4 != 0,
                fail_freeze: failures & 8 != 0,
                fail_drain: failures & 16 != 0,
                fail_resume: failures & 32 != 0,
                ..Default::default()
            };
            let report = cleanup(&mut mock);
            assert_eq!(
                mock.calls,
                [
                    "freeze",
                    "restore:10",
                    "restore:20",
                    "restore:30",
                    "continue",
                    "drain",
                    "detach",
                    "resume"
                ]
            );
            assert_eq!(report.complete(), failures == 0);
            assert_eq!(report.errors.len(), (failures as u32).count_ones() as usize);
            assert_eq!(report.threads_exited, 1);
        }
        let mut mock = MockCleanup {
            exited: true,
            ..Default::default()
        };
        let report = cleanup(&mut mock);
        assert!(report.complete());
        assert!(!report.debug_active_process_stop_ok);
        assert!(report.process_exited);
    }

    #[test]
    fn cli_requires_finite_duration_and_absolute_new_jsonl_name() {
        let mut args = Args {
            pid: 123,
            created: 456,
            output: std::env::current_dir().unwrap().join("probe.jsonl"),
            seconds: 240,
        };
        assert!(args.validate().is_ok());
        assert!(args.ready_path().ends_with("probe.jsonl.ready"));
        args.seconds = 0;
        assert!(args.validate().is_err());
        args.seconds = 241;
        assert!(args.validate().is_err());
        args.seconds = 240;
        args.output = PathBuf::from("relative.jsonl");
        assert!(args.validate().is_err());
    }
}
