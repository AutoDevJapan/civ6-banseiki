use super::{
    transaction::{self, Backend, Outcome},
    *,
};
use crate::{
    click,
    startup::{GameProcess, RecoveryLease},
    time,
};
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    io::Read,
    mem::size_of,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, FILETIME, HANDLE, INVALID_HANDLE_VALUE, NTSTATUS},
    Storage::FileSystem::{GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED, VOLUME_NAME_DOS},
    System::{
        Diagnostics::{
            Debug::{
                FlushInstructionCache, GetThreadContext, ReadProcessMemory, WriteProcessMemory,
                CONTEXT, CONTEXT_CONTROL_AMD64,
            },
            ToolHelp::{
                CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Thread32First,
                Thread32Next, MODULEENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPTHREAD, THREADENTRY32,
            },
        },
        Memory::{
            VirtualProtectEx, VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE,
            PAGE_EXECUTE_READWRITE, PAGE_GUARD, PAGE_NOACCESS,
        },
        Threading::{
            GetProcessIdOfThread, GetProcessTimes, IsWow64Process, OpenProcess, OpenThread,
            TerminateProcess, WaitForSingleObject, PROCESS_QUERY_INFORMATION,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SUSPEND_RESUME, PROCESS_SYNCHRONIZE,
            PROCESS_TERMINATE, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
            THREAD_GET_CONTEXT, THREAD_QUERY_INFORMATION,
        },
    },
    UI::WindowsAndMessaging::GetWindowThreadProcessId,
};

#[link(name = "ntdll")]
#[allow(non_snake_case)]
unsafe extern "system" {
    fn NtSuspendProcess(process: HANDLE) -> NTSTATUS;
    fn NtResumeProcess(process: HANDLE) -> NTSTATUS;
}

const KNOWN_EXE: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Binaries/Win64Steam/CivilizationVI.exe";
const KNOWN_DLL: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll";
const MAX_ATTEMPTS: usize = 16;
const MAX_THREADS: usize = 512;
const MAX_STACK_BYTES: usize = 256 * 1024;
const WAIT_TIMEOUT: u32 = 258;

struct Handle(HANDLE);
impl Handle {
    fn new(raw: HANDLE, what: &str) -> Result<Self> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(win_error(what))
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

fn win_error(what: &str) -> Error {
    Error::input(format!("exact removal {what}: Win32 error {}", unsafe {
        GetLastError()
    }))
}
fn nt_error(what: &str, status: NTSTATUS) -> Error {
    Error::input(format!(
        "exact removal {what}: NTSTATUS 0x{:08x}",
        status as u32
    ))
}
fn path_eq(a: &Path, b: &Path) -> bool {
    a.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
}
fn wide_text(bytes: &[u16]) -> Result<String> {
    let end = bytes
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(|| Error::input("truncated native module path"))?;
    String::from_utf16(&bytes[..end]).map_err(|_| Error::input("invalid native module path"))
}
fn final_path(file: &File) -> Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle() as HANDLE,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(win_error("final file path"));
    }
    let text = String::from_utf16(&buffer[..length])
        .map_err(|_| Error::input("invalid final file path"))?;
    Ok(PathBuf::from(text))
}
fn pinned_file(path: &Path) -> Result<(File, PathBuf)> {
    // The source DLL is never opened for writing. Deny writes and rename/delete
    // until this action finishes, so the hashed file cannot be replaced.
    let file = OpenOptions::new().read(true).share_mode(1).open(path)?;
    let actual = final_path(&file)?;
    let expected = path.canonicalize()?;
    if !path_eq(&actual, &expected) {
        return Err(Error::input("pinned file final path differs"));
    }
    Ok((file, actual))
}
fn creation(handle: HANDLE) -> Result<u64> {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    if unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return Err(win_error("GetProcessTimes"));
    }
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}
fn unix_start(created: u64) -> f64 {
    created as f64 / 10_000_000.0 - 11_644_473_600.0
}
fn deadline(deadline: Instant) -> Result<()> {
    time::check_interrupt()?;
    if Instant::now() >= deadline {
        return Err(Error::input("exact removal finite deadline expired"));
    }
    Ok(())
}

#[derive(Clone)]
struct Module {
    base: usize,
    size: usize,
    path: PathBuf,
}
fn module(pid: u32, expected: &Path, until: Instant) -> Result<Module> {
    let snapshot = Handle::new(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid) },
        "module snapshot",
    )?;
    let mut row = MODULEENTRY32W {
        dwSize: size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    let mut more = unsafe { Module32FirstW(snapshot.0, &mut row) } != 0;
    let mut found = None;
    let mut count = 0;
    while more {
        deadline(until)?;
        count += 1;
        if count > 4096 {
            return Err(Error::input("module enumeration exceeded finite limit"));
        }
        if wide_text(&row.szModule)?.eq_ignore_ascii_case(MODULE_NAME) {
            let path = PathBuf::from(wide_text(&row.szExePath)?).canonicalize()?;
            if row.th32ProcessID != pid
                || found.is_some()
                || !path_eq(&path, expected)
                || row.modBaseAddr.is_null()
                || row.modBaseSize as usize != IMAGE_SIZE
            {
                return Err(Error::input(
                    "unknown, duplicate or relocated-to-wrong-path GameCore module",
                ));
            }
            found = Some(Module {
                base: row.modBaseAddr as usize,
                size: row.modBaseSize as usize,
                path,
            });
        }
        more = unsafe { Module32NextW(snapshot.0, &mut row) } != 0;
    }
    if unsafe { GetLastError() } != 18 {
        return Err(win_error("incomplete module enumeration"));
    }
    found.ok_or_else(|| Error::input("audited XP2 GameCore module is not loaded"))
}

fn thread_ids(pid: u32) -> Result<Vec<u32>> {
    let snapshot = Handle::new(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) },
        "thread snapshot",
    )?;
    let mut row = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut more = unsafe { Thread32First(snapshot.0, &mut row) } != 0;
    let mut ids = Vec::new();
    let mut count = 0;
    while more {
        count += 1;
        if count > 131072 {
            return Err(Error::input("thread enumeration exceeded finite limit"));
        }
        if row.th32OwnerProcessID == pid {
            ids.push(row.th32ThreadID);
            if ids.len() > MAX_THREADS {
                return Err(Error::input("too many threads to suspend-inspect"));
            }
        }
        more = unsafe { Thread32Next(snapshot.0, &mut row) } != 0;
    }
    if unsafe { GetLastError() } != 18 {
        return Err(win_error("incomplete thread enumeration"));
    }
    ids.sort_unstable();
    if ids.is_empty() || ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(Error::input("invalid pinned process thread snapshot"));
    }
    Ok(ids)
}

#[repr(C, align(16))]
struct AlignedContext(CONTEXT);

struct Native {
    process: Handle,
    game: GameProcess,
    created: u64,
    exe: PathBuf,
    _exe_file: File,
    _module_file: File,
    dll: Vec<u8>,
    sha: String,
    module: Module,
    lease: RecoveryLease,
    until: Instant,
    suspend_depth: u8,
    resume_uncertain: bool,
    terminated: bool,
}

impl Native {
    fn open(game: GameProcess, until: Instant) -> Result<Self> {
        if game.pid == 0
            || game.pid == std::process::id()
            || !game.started.is_finite()
            || game.started <= 0.0
        {
            return Err(Error::input("invalid caller-pinned game process"));
        }
        let mut lease = RecoveryLease::acquire()?;
        lease.check()?;
        deadline(until)?;
        if click::civ_process_ids()?
            .iter()
            .copied()
            .collect::<Vec<_>>()
            != [game.pid]
        {
            return Err(Error::input(
                "exact removal requires the single pinned Civ6",
            ));
        }
        let access = PROCESS_QUERY_LIMITED_INFORMATION
            | PROCESS_QUERY_INFORMATION
            | PROCESS_SYNCHRONIZE
            | PROCESS_SUSPEND_RESUME
            | PROCESS_VM_READ
            | PROCESS_VM_WRITE
            | PROCESS_VM_OPERATION
            | PROCESS_TERMINATE;
        let process = Handle::new(unsafe { OpenProcess(access, 0, game.pid) }, "OpenProcess")?;
        let created = creation(process.0)?;
        if unix_start(created) != game.started
            || unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(Error::input("pinned game PID/creation time changed"));
        }
        let image = PathBuf::from(click::image_path(process.0 as isize)?).canonicalize()?;
        let (exe_file, exe) = pinned_file(Path::new(KNOWN_EXE))?;
        if !path_eq(&image, &exe) {
            return Err(Error::input(
                "game exe real path is not the audited DX11 installation",
            ));
        }
        let mut wow64 = 0;
        if unsafe { IsWow64Process(process.0, &mut wow64) } == 0 || wow64 != 0 {
            return Err(Error::input("target is not native Win64"));
        }
        let window = click::find_civ_window()?;
        let mut window_pid = 0;
        unsafe {
            GetWindowThreadProcessId(window as HANDLE, &mut window_pid);
        }
        if window_pid != game.pid {
            return Err(Error::input("Civ6 window belongs to another process"));
        }
        crate::restart::require_dx11_window(window)?;
        let (mut dll_file, dll_path) = pinned_file(Path::new(KNOWN_DLL))?;
        if dll_file.metadata()?.len() != FILE_SIZE as u64 {
            return Err(Error::input("unknown GameCore file size"));
        }
        let mut dll = Vec::with_capacity(FILE_SIZE);
        (&mut dll_file)
            .take(FILE_SIZE as u64 + 1)
            .read_to_end(&mut dll)?;
        deadline(until)?;
        let sha = format!("{:x}", Sha256::digest(&dll));
        if dll.len() != FILE_SIZE || sha != MODULE_SHA256 {
            return Err(Error::input("unknown GameCore full-file SHA256"));
        }
        validate_snapshot(
            &sha,
            &dll[..HEADER_SIZE],
            &PATCHES
                .iter()
                .map(|p| p.original.to_vec())
                .collect::<Vec<_>>(),
        )?;
        for patch in PATCHES {
            if disk_range(&dll, patch.rva, patch.original.len())? != patch.original {
                return Err(Error::input("audited disk instruction bytes differ"));
            }
        }
        let module = module(game.pid, &dll_path, until)?;
        let mut native = Self {
            process,
            game,
            created,
            exe,
            _exe_file: exe_file,
            _module_file: dll_file,
            dll,
            sha,
            module,
            lease,
            until,
            suspend_depth: 0,
            resume_uncertain: false,
            terminated: false,
        };
        native.check()?;
        Ok(native)
    }

    fn pin_alive(&self) -> Result<()> {
        if creation(self.process.0)? != self.created
            || unix_start(self.created) != self.game.started
            || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(Error::input(
                "owned game handle is no longer the pinned live process",
            ));
        }
        let current = PathBuf::from(click::image_path(self.process.0 as isize)?).canonicalize()?;
        if !path_eq(&current, &self.exe) {
            return Err(Error::input("pinned exe real path changed"));
        }
        Ok(())
    }
    fn address(&self, rva: usize, len: usize) -> Result<usize> {
        if len == 0
            || rva
                .checked_add(len)
                .is_none_or(|end| end > self.module.size)
        {
            return Err(Error::input("memory request escapes audited module image"));
        }
        self.module
            .base
            .checked_add(rva)
            .ok_or_else(|| Error::input("module address overflow"))
    }
    fn region(&self, address: usize, len: usize) -> Result<MEMORY_BASIC_INFORMATION> {
        let mut info = MEMORY_BASIC_INFORMATION::default();
        let got = unsafe {
            VirtualQueryEx(
                self.process.0,
                address as *const c_void,
                &mut info,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if got != size_of::<MEMORY_BASIC_INFORMATION>() {
            return Err(win_error("VirtualQueryEx"));
        }
        let end = address
            .checked_add(len)
            .ok_or_else(|| Error::input("memory range overflow"))?;
        let region_end = (info.BaseAddress as usize)
            .checked_add(info.RegionSize)
            .ok_or_else(|| Error::input("memory region overflow"))?;
        if address < info.BaseAddress as usize
            || end > region_end
            || info.State != MEM_COMMIT
            || info.Protect & (PAGE_GUARD | PAGE_NOACCESS) != 0
        {
            return Err(Error::input(
                "remote memory is not a complete readable committed region",
            ));
        }
        Ok(info)
    }
    fn image_region(&self, rva: usize, len: usize) -> Result<usize> {
        let address = self.address(rva, len)?;
        let info = self.region(address, len)?;
        if info.AllocationBase as usize != self.module.base || info.Type != MEM_IMAGE {
            return Err(Error::input(
                "remote range is not backed by the pinned module image",
            ));
        }
        Ok(address)
    }
    fn read_address(&self, address: usize, len: usize) -> Result<Vec<u8>> {
        let mut bytes = vec![0; len];
        let mut read = 0;
        let ok = unsafe {
            ReadProcessMemory(
                self.process.0,
                address as *const c_void,
                bytes.as_mut_ptr() as *mut c_void,
                len,
                &mut read,
            )
        };
        if ok == 0 || read != len {
            return Err(win_error("ReadProcessMemory exact length"));
        }
        Ok(bytes)
    }
    fn in_function(&self, address: u64) -> bool {
        let Some(rva) = (address as usize).checked_sub(self.module.base) else {
            return false;
        };
        FUNCTIONS
            .iter()
            .any(|(start, end)| (*start..*end).contains(&rva))
    }
    fn stack_has_return(&self, stack: u64) -> Result<bool> {
        let address =
            usize::try_from(stack).map_err(|_| Error::input("thread stack address overflow"))?;
        if address & 7 != 0 {
            return Err(Error::input("unaligned thread stack cannot be inspected"));
        }
        let region = self.region(address, 8)?;
        let end = (region.BaseAddress as usize)
            .checked_add(region.RegionSize)
            .ok_or_else(|| Error::input("thread stack range overflow"))?;
        let length = end - address;
        if region.Type == MEM_IMAGE || length > MAX_STACK_BYTES {
            return Err(Error::input("thread stack cannot be boundedly inspected"));
        }
        let bytes = self.read_address(address, length)?;
        // A thread inside floorf can return into a half-applied removal. A
        // conservative scan above RSP prevents this even when RIP is outside
        // GameCore. False positives merely consume the finite retry budget.
        Ok(bytes
            .chunks_exact(8)
            .any(|b| self.in_function(u64::from_le_bytes(b.try_into().expect("8-byte chunk")))))
    }
    fn inspect_threads(&self, until: Instant) -> Result<bool> {
        if self.suspend_depth == 0 {
            return Err(Error::input("thread context read requires suspension"));
        }
        let ids = thread_ids(self.game.pid)?;
        let mut busy = false;
        for &id in &ids {
            deadline(until)?;
            let thread = Handle::new(
                unsafe { OpenThread(THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, 0, id) },
                "OpenThread",
            )?;
            if unsafe { GetProcessIdOfThread(thread.0) } != self.game.pid {
                return Err(Error::input("thread ID no longer belongs to pinned Civ6"));
            }
            let mut context = AlignedContext(CONTEXT {
                ContextFlags: CONTEXT_CONTROL_AMD64,
                ..Default::default()
            });
            if unsafe { GetThreadContext(thread.0, &mut context.0) } == 0 {
                return Err(win_error("GetThreadContext"));
            }
            busy |= self.in_function(context.0.Rip) || self.stack_has_return(context.0.Rsp)?;
        }
        if ids != thread_ids(self.game.pid)? {
            return Err(Error::input(
                "thread set changed during suspended inspection",
            ));
        }
        Ok(busy)
    }
    fn module_unchanged(&self, until: Instant) -> Result<()> {
        let current = module(self.game.pid, &self.module.path, until)?;
        if current.base != self.module.base || current.size != self.module.size {
            return Err(Error::input("GameCore module changed after pinning"));
        }
        Ok(())
    }
}

fn disk_range(dll: &[u8], rva: usize, len: usize) -> Result<&[u8]> {
    let offset = if (0x1000..0x9b6627).contains(&rva) {
        rva.checked_sub(0xc00)
    } else if (0x9b7000..0xb5f124).contains(&rva) {
        rva.checked_sub(0x1400)
    } else {
        None
    };
    let off = offset.ok_or_else(|| Error::input("unknown audited image section"))?;
    dll.get(
        off..off
            .checked_add(len)
            .ok_or_else(|| Error::input("disk range overflow"))?,
    )
    .ok_or_else(|| Error::input("audited disk image is incomplete"))
}

impl Backend for Native {
    fn check(&mut self) -> Result<()> {
        deadline(self.until)?;
        self.lease.check()?;
        self.pin_alive()?;
        if click::civ_process_ids()?
            .iter()
            .copied()
            .collect::<Vec<_>>()
            != [self.game.pid]
        {
            return Err(Error::input(
                "another Civ6 appeared or the pinned game disappeared",
            ));
        }
        Ok(())
    }
    fn suspend(&mut self) -> Result<()> {
        self.check()?;
        if self.suspend_depth != 0 {
            return Err(Error::input("nested initial process suspension refused"));
        }
        self.suspend_depth = 1; // Cleanup also covers a failed/partly effected call.
        let status = unsafe { NtSuspendProcess(self.process.0) };
        if status < 0 {
            self.resume_uncertain = true;
            let cause = nt_error("NtSuspendProcess", status);
            if self.resume().is_err() {
                let _ = self.terminate();
                let _ = self.resume();
            }
            return Err(cause);
        }
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        while self.suspend_depth > 0 {
            let status = unsafe { NtResumeProcess(self.process.0) };
            if status < 0 {
                if self.terminated && unsafe { WaitForSingleObject(self.process.0, 0) } == 0 {
                    self.suspend_depth = 0;
                    self.resume_uncertain = false;
                    return Ok(());
                }
                self.resume_uncertain = true;
                return Err(nt_error("NtResumeProcess", status));
            }
            self.suspend_depth -= 1;
        }
        self.resume_uncertain = false;
        Ok(())
    }
    fn busy(&mut self) -> Result<bool> {
        self.inspect_threads(self.until)
    }
    fn verify(&mut self) -> Result<PatchState> {
        if self.suspend_depth == 0 {
            return Err(Error::input("instruction validation requires suspension"));
        }
        self.module_unchanged(self.until)?;
        let header = self.read(0, HEADER_SIZE)?;
        if header != self.dll[..HEADER_SIZE] {
            return Err(Error::input(
                "remote PE headers differ from pinned source DLL",
            ));
        }
        let mut instructions = Vec::with_capacity(PATCHES.len());
        for p in PATCHES {
            instructions.push(self.read(p.rva, p.original.len())?);
        }
        let state = validate_snapshot(&self.sha, &header, &instructions)?;
        for (start, end) in FUNCTIONS {
            deadline(self.until)?;
            let mut expected = disk_range(&self.dll, start, end - start)?.to_vec();
            if state == PatchState::Patched {
                for p in PATCHES {
                    if (start..end).contains(&p.rva) {
                        expected[p.rva - start..p.rva - start + p.replacement.len()]
                            .copy_from_slice(p.replacement);
                    }
                }
            }
            if self.read(start, end - start)? != expected {
                return Err(Error::input("unknown bytes inside audited effect function"));
            }
        }
        for (rva, constant) in CONSTANTS {
            if disk_range(&self.dll, rva, constant.len())? != constant
                || self.read(rva, constant.len())? != constant
            {
                return Err(Error::input(
                    "shared fixed-point constant differs; no writes allowed",
                ));
            }
        }
        Ok(state)
    }
    fn read(&mut self, rva: usize, len: usize) -> Result<Vec<u8>> {
        let address = self.image_region(rva, len)?;
        self.read_address(address, len)
    }
    fn writable(&mut self, rva: usize, len: usize) -> Result<u32> {
        if self.suspend_depth == 0 || self.resume_uncertain {
            return Err(Error::input(
                "write protection change requires a certain suspension",
            ));
        }
        let address = self.image_region(rva, len)?;
        let mut previous = 0;
        if unsafe {
            VirtualProtectEx(
                self.process.0,
                address as *const c_void,
                len,
                PAGE_EXECUTE_READWRITE,
                &mut previous,
            )
        } == 0
        {
            return Err(win_error("VirtualProtectEx writable"));
        }
        Ok(previous)
    }
    fn restore(&mut self, rva: usize, len: usize, protection: u32) -> Result<()> {
        let address = self.image_region(rva, len)?;
        let mut discarded = 0;
        if unsafe {
            VirtualProtectEx(
                self.process.0,
                address as *const c_void,
                len,
                protection,
                &mut discarded,
            )
        } == 0
        {
            return Err(win_error("VirtualProtectEx original protection"));
        }
        if self.region(address, len)?.Protect != protection {
            return Err(Error::input("original page protection readback differs"));
        }
        Ok(())
    }
    fn write(&mut self, rva: usize, bytes: &[u8]) -> Result<()> {
        if self.suspend_depth == 0 || self.resume_uncertain {
            return Err(Error::input(
                "WriteProcessMemory requires a certain suspension",
            ));
        }
        let address = self.image_region(rva, bytes.len())?;
        let mut written = 0;
        if unsafe {
            WriteProcessMemory(
                self.process.0,
                address as *const c_void,
                bytes.as_ptr() as *const c_void,
                bytes.len(),
                &mut written,
            )
        } == 0
            || written != bytes.len()
        {
            return Err(win_error("WriteProcessMemory exact length"));
        }
        Ok(())
    }
    fn flush(&mut self, rva: usize, len: usize) -> Result<()> {
        let address = self.address(rva, len)?;
        if unsafe { FlushInstructionCache(self.process.0, address as *const c_void, len) } == 0 {
            return Err(win_error("FlushInstructionCache"));
        }
        Ok(())
    }
    fn prepare_rollback(&mut self) -> Result<()> {
        // Ignore cancellation/expired mutation deadline ONLY for bounded cleanup.
        // The lock handle and process handle remain pinned throughout cleanup.
        self.pin_alive()?;
        let resume_was_uncertain = self.resume_uncertain;
        if self.resume_uncertain {
            if self.suspend_depth >= 3 {
                return Err(Error::input("cleanup suspend count limit reached"));
            }
            self.suspend_depth += 1;
            let status = unsafe { NtSuspendProcess(self.process.0) };
            if status < 0 {
                return Err(nt_error("cleanup NtSuspendProcess", status));
            }
            self.resume_uncertain = false;
        }
        if self.suspend_depth == 0 {
            return Err(Error::input("rollback would target a running process"));
        }
        time::with_interrupt_cleanup(|| {
            let cleanup_deadline = Instant::now() + Duration::from_secs(2);
            self.module_unchanged(cleanup_deadline)?;
            if self.read(0, HEADER_SIZE)? != self.dll[..HEADER_SIZE] {
                return Err(Error::input("rollback module header changed"));
            }
            if resume_was_uncertain && self.inspect_threads(cleanup_deadline)? {
                return Err(Error::input(
                    "cannot rollback while a resumed effect may be active",
                ));
            }
            Ok(())
        })
    }
    fn terminate(&mut self) -> Result<()> {
        // Never reopen a PID here: the original kernel handle is the authority.
        if creation(self.process.0)? != self.created {
            return Err(Error::input(
                "termination refused: pinned creation time differs",
            ));
        }
        if unsafe { WaitForSingleObject(self.process.0, 0) } == 0 {
            self.terminated = true;
            return Ok(());
        }
        if unsafe { TerminateProcess(self.process.0, 3) } == 0 {
            let error = win_error("terminate pinned game after unsafe rollback");
            if unsafe { WaitForSingleObject(self.process.0, 0) } == 0 {
                self.terminated = true;
                return Ok(());
            }
            return Err(error);
        }
        if unsafe { WaitForSingleObject(self.process.0, 1000) } != 0 {
            return Err(Error::input(
                "pinned game termination did not complete within cleanup limit",
            ));
        }
        self.terminated = true;
        Ok(())
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        if self.suspend_depth > 0 && self.resume().is_err() {
            let _ = self.terminate();
            let _ = self.resume();
        }
    }
}

pub(super) fn apply(game: GameProcess) -> Result<Value> {
    // The parent supplies diagnostic-only/non-MP proof through validate(). The
    // actual driver invocation must independently be in --test-mode as well.
    if !std::env::args_os().any(|arg| arg == "--test-mode") {
        return Err(Error::scenario(
            "native exact removal requires the driver's --test-mode invocation",
        ));
    }
    let until = Instant::now() + Duration::from_secs(10);
    catch_unwind(AssertUnwindSafe(|| {
        let mut native = Native::open(game, until)?;
        for attempt in 1..=MAX_ATTEMPTS {
            match transaction::attempt(&mut native)? {
                Outcome::Busy => {
                    deadline(until)?;
                    std::thread::sleep(Duration::from_millis(25));
                }
                outcome => {
                    let mut evidence = plan();
                    evidence["status"] = json!(if outcome == Outcome::Applied { "applied" } else { "already_patched" });
                    evidence["attempts"] = json!(attempt);
                    evidence["writes"] = json!(if outcome == Outcome::Applied { PATCHES.len() } else { 0 });
                    evidence["pid"] = json!(native.game.pid);
                    evidence["started"] = json!(native.game.started);
                    evidence["creation_filetime"] = json!(native.created);
                    evidence["module_base"] = json!(native.module.base);
                    evidence["module_path"] = json!(native.module.path);
                    evidence["exe_path"] = json!(native.exe);
                    evidence["original_protections_restored"] = json!(true);
                    return Ok(evidence);
                }
            }
        }
        Err(Error::input("effect function remained active within finite retry limit; zero writes"))
    })).unwrap_or_else(|_| Err(Error::input(json!({
        "action":"oath_exact_removal", "status":"refused", "accepted":false,
        "quantity_verified":false, "reason":"panic during native exact-removal action; RAII cleanup attempted"
    }).to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_sdk_context_and_memory_layouts_are_preserved() {
        assert_eq!(size_of::<FILETIME>(), 8);
        assert_eq!(size_of::<CONTEXT>(), 1232);
        assert_eq!(std::mem::offset_of!(CONTEXT, Rip), 248);
        assert_eq!(std::mem::offset_of!(CONTEXT, Rsp), 152);
        assert_eq!(std::mem::align_of::<AlignedContext>(), 16);
        assert_eq!(size_of::<AlignedContext>(), 1232);
        assert_eq!(size_of::<MEMORY_BASIC_INFORMATION>(), 48);
        assert_eq!(
            std::mem::offset_of!(MEMORY_BASIC_INFORMATION, RegionSize),
            24
        );
    }
}
