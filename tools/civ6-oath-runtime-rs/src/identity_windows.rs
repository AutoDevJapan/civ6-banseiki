//! Identity gate for the current process, before any Oath entry is published.
//! Process ownership, fresh launch, actual ruleset and single-player mode are
//! additionally checked by the owning driver before invoking the export.

use crate::native::{c_void, copy_current};
use crate::oath_record::{OathCall, OATH_PREFIX_PAD};
use crate::preflight::{ImageMemory, SourcePlan, CORE_PATH, EXE_PATH, EXE_SHA256};
use crate::runtime::{own_process, Gate, OathRuntimeConfig, OathStatus, Verified};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::windows::{ffi::OsStringExt, fs::OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GetModuleHandleW, GET_MODULE_HANDLE_EX_FLAG_PIN,
};
use windows_sys::Win32::System::Memory::{
    VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE,
};

// Deny write/delete sharing for the lifetime of any published gateway. The
// loader module pins are taken by runtime.rs before it changes an entry.
static SOURCE_FILES: Mutex<Option<Vec<File>>> = Mutex::new(None);
const MAX_FILE: u64 = 64 * 1024 * 1024;

fn module_path(module: *mut c_void) -> Result<PathBuf, OathStatus> {
    let mut path = vec![0u16; 32768];
    let len = unsafe { GetModuleFileNameW(module, path.as_mut_ptr(), path.len() as u32) } as usize;
    if len == 0 || len >= path.len() {
        return Err(OathStatus::ImageIdentity);
    }
    Ok(PathBuf::from(OsString::from_wide(&path[..len])))
}

fn same_path(actual: &Path, expected: &str) -> bool {
    let (Ok(actual), Ok(expected)) = (actual.canonicalize(), Path::new(expected).canonicalize())
    else {
        return false;
    };
    actual
        .as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.as_os_str().to_string_lossy())
}

fn pinned_disk(path: &str) -> Result<(File, Vec<u8>), OathStatus> {
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|_| OathStatus::ImageIdentity)?;
    let size = file
        .metadata()
        .map_err(|_| OathStatus::ImageIdentity)?
        .len();
    if size == 0 || size > MAX_FILE {
        return Err(OathStatus::ImageIdentity);
    }
    let mut bytes = Vec::with_capacity(size as usize);
    (&mut file)
        .take(size + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| OathStatus::ImageIdentity)?;
    if bytes.len() as u64 != size {
        return Err(OathStatus::ImageIdentity);
    }
    Ok((file, bytes))
}

fn image_region(base: usize, at: usize, length: usize) -> bool {
    let Some(end) = at.checked_add(length) else {
        return false;
    };
    let mut cursor = at;
    while cursor < end {
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe {
            VirtualQuery(
                cursor as *const c_void,
                &mut info,
                std::mem::size_of_val(&info),
            )
        } != std::mem::size_of_val(&info)
            || info.State != MEM_COMMIT
            || info.Type != MEM_IMAGE
            || info.AllocationBase as usize != base
        {
            return false;
        }
        let Some(next) = (info.BaseAddress as usize).checked_add(info.RegionSize) else {
            return false;
        };
        if next <= cursor {
            return false;
        }
        cursor = next.min(end);
    }
    true
}

struct CurrentImage(usize);
impl ImageMemory for CurrentImage {
    fn copy(&mut self, address: u64, data: &mut [u8]) -> Result<(), &'static str> {
        if !image_region(self.0, address as usize, data.len()) {
            return Err("window is not the pinned loaded image");
        }
        let mut read = 0usize;
        let ok = unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory(
                windows_sys::Win32::System::Threading::GetCurrentProcess(),
                address as *const c_void,
                data.as_mut_ptr().cast(),
                data.len(),
                &mut read,
            )
        } != 0;
        if !ok || read != data.len() {
            return Err("loaded image copy incomplete");
        }
        Ok(())
    }
}

fn header_matches(base: usize, disk: &[u8]) -> bool {
    if !image_region(base, base, 0x400) {
        return false;
    }
    let Some(expected) = disk.get(..0x400) else {
        return false;
    };
    copy_current::<0x400>(base as u64).is_some_and(|observed| observed.as_slice() == expected)
}

pub(crate) struct CurrentGameGate;
impl Gate for CurrentGameGate {
    fn verify(&self, config: &OathRuntimeConfig) -> Result<Verified, OathStatus> {
        own_process(config)?;
        let exe = unsafe { GetModuleHandleW(std::ptr::null()) };
        if exe.is_null() || !same_path(&module_path(exe)?, EXE_PATH) {
            return Err(OathStatus::ProcessIdentity);
        }
        let (exe_file, exe_disk) = pinned_disk(EXE_PATH)?;
        if format!("{:x}", Sha256::digest(&exe_disk)) != EXE_SHA256
            || !header_matches(exe as usize, &exe_disk)
        {
            return Err(OathStatus::ProcessIdentity);
        }
        let name: Vec<u16> = "GameCore_XP2_FinalRelease.dll\0".encode_utf16().collect();
        // Resolve and pin atomically: a borrowed handle could be unloaded or
        // reused while hashing the source or copying a loaded image window.
        let mut core = std::ptr::null_mut();
        if unsafe { GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_PIN, name.as_ptr(), &mut core) }
            == 0
        {
            return Err(OathStatus::ImageIdentity);
        }
        if core.is_null() || !same_path(&module_path(core)?, CORE_PATH) {
            return Err(OathStatus::ImageIdentity);
        }
        let (core_file, core_disk) = pinned_disk(CORE_PATH)?;
        let plan = SourcePlan::from_disk(&core_disk).map_err(|_| OathStatus::ImageIdentity)?;
        let base = core as usize;
        if !header_matches(base, &core_disk) {
            return Err(OathStatus::ImageIdentity);
        }
        plan.validate_image(base as u64, &mut CurrentImage(base))
            .map_err(|_| OathStatus::EntryBytes)?;
        for call in [OathCall::Apply, OathCall::Remove] {
            let Some(entry) = base.checked_add(call.pinned_rva()) else {
                return Err(OathStatus::EntryBytes);
            };
            if entry & 1 != 0 {
                return Err(OathStatus::EntryBytes);
            }
            let mut image_base = 0u64;
            for offset in 1..=OATH_PREFIX_PAD.len() {
                if !unsafe {
                    RtlLookupFunctionEntry(
                        (entry - offset) as u64,
                        &mut image_base,
                        std::ptr::null_mut(),
                    )
                }
                .is_null()
                {
                    return Err(OathStatus::EntryBytes);
                }
            }
        }
        let Ok(mut files) = SOURCE_FILES.lock() else {
            return Err(OathStatus::Panic);
        };
        if files.is_some() {
            return Err(OathStatus::WrongState);
        }
        *files = Some(vec![exe_file, core_file]);
        Ok(Verified {
            image_base: base,
            core_module: base,
            apply_entry: base + OathCall::Apply.pinned_rva(),
            remove_entry: base + OathCall::Remove.pinned_rva(),
            recipient_slots: Some(std::array::from_fn(|index| {
                let (address, original) =
                    crate::recipient_trace::pinned_slot(index, base as u64).unwrap();
                (address as usize, original)
            })),
        })
    }
}
