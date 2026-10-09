use super::{
    validate_evidence, validate_finish_evidence, Identity, Manifest, Session, LOCK_PATH, MAX_RECORD,
};
use crate::{Error, Result};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::windows::ffi::OsStrExt,
    os::windows::fs::OpenOptionsExt,
    path::Path,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        GetProcessTimes, OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_SYNCHRONIZE,
    },
};

pub fn publish_request(tmp: &Path, target: &Path) -> Result<()> {
    let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { windows_sys::Win32::Storage::FileSystem::MoveFileW(from.as_ptr(), to.as_ptr()) }
        == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

pub fn check_completion_chain(pid: u32, created: u64) -> Result<()> {
    if std::env::var(crate::startup::CHAIN_PID_ENV)
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        != Some(pid)
    {
        return Err(Error::input(
            "native observer completion belongs to another live chain",
        ));
    }
    let process = Process::open(pid)?;
    let identity = process.identity()?;
    if !identity.alive || identity.created != created {
        return Err(Error::input(
            "native observer completion chain identity changed",
        ));
    }
    Ok(())
}

struct Process {
    handle: HANDLE,
    pid: u32,
}
// These handles are owned here and used only through &mut Session.
unsafe impl Send for Process {}
impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
impl Process {
    fn open(pid: u32) -> Result<Self> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if handle.is_null() {
            return Err(Error::input("cannot retain native observer process handle"));
        }
        Ok(Self { handle, pid })
    }
    fn identity(&self) -> Result<Identity> {
        let (mut start, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        if unsafe { GetProcessTimes(self.handle, &mut start, &mut exit, &mut kernel, &mut user) }
            == 0
        {
            return Err(Error::input("native observer process creation unavailable"));
        }
        let alive = match unsafe { WaitForSingleObject(self.handle, 0) } {
            WAIT_TIMEOUT => true,
            WAIT_OBJECT_0 => false,
            _ => return Err(Error::input("native observer process wait failed")),
        };
        Ok(Identity {
            pid: self.pid,
            created: (u64::from(start.dwHighDateTime) << 32) | u64::from(start.dwLowDateTime),
            alive,
        })
    }
}
fn read(file: &mut File) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take((MAX_RECORD + 1) as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}
struct Pinned {
    chain: Process,
    game: Process,
    observer: Process,
    lease: Option<File>,
    ready: File,
}
impl Session for Pinned {
    fn check(&mut self, m: &Manifest, game_pid: u32) -> Result<()> {
        crate::time::check_interrupt()?;
        validate_evidence(
            m,
            &read(self.lease.as_mut().ok_or_else(|| {
                Error::input("native observer acknowledgement lease pin was released")
            })?)?,
            &read(&mut self.ready)?,
            [
                self.chain.identity()?,
                self.game.identity()?,
                self.observer.identity()?,
            ],
            game_pid,
        )
    }
    fn after_ack(&mut self) {
        self.lease = None;
    }
    fn check_finish(&mut self, m: &Manifest, game_pid: u32) -> Result<bool> {
        crate::time::check_interrupt()?;
        // Reopen the current lease for a short read, so the monitor and chain
        // cleanup are not blocked throughout the observer's finite lifetime.
        let mut lease = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(LOCK_PATH)?;
        validate_finish_evidence(
            m,
            &read(&mut lease)?,
            &read(&mut self.ready)?,
            [
                self.chain.identity()?,
                self.game.identity()?,
                self.observer.identity()?,
            ],
            game_pid,
        )
    }
}
pub fn pin(m: &Manifest) -> Result<Box<dyn Session>> {
    // Deny write/delete sharing for the whole preparation and acknowledgement.
    let lease = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(LOCK_PATH)?;
    let ready = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&m.ready_path)?;
    let pinned = Pinned {
        chain: Process::open(m.chain_pid)?,
        game: Process::open(m.game_pid)?,
        observer: Process::open(m.observer_pid)?,
        lease: Some(lease),
        ready,
    };
    let image = crate::click::image_path(pinned.chain.handle as isize)?;
    if !image
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .eq_ignore_ascii_case("live_chain.exe")
    {
        return Err(Error::input(
            "native observer lease owner is not live_chain.exe",
        ));
    }
    // Bind to the chain which actually launched this runner, not another valid lease.
    if std::env::var(crate::startup::CHAIN_PID_ENV)
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        != Some(m.chain_pid)
    {
        return Err(Error::input(
            "native observer handoff belongs to another live chain",
        ));
    }
    Ok(Box::new(pinned))
}
