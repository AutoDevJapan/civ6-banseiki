//! Scope build processes to one operation, including grandchildren.
use std::process::{Child, Command};

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{
        os::windows::{io::AsRawHandle, process::CommandExt},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                THREADENTRY32,
            },
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
                JobObjectExtendedLimitInformation, QueryInformationJobObject,
                SetInformationJobObject, TerminateJobObject,
                JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Threading::{OpenThread, ResumeThread, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME},
        },
    };

    pub struct Tree(Option<HANDLE>);
    impl Drop for Tree {
        fn drop(&mut self) {
            if let Some(job) = self.0.take() {
                unsafe {
                    CloseHandle(job);
                }
            }
        }
    }
    fn error() -> String {
        std::io::Error::last_os_error().to_string()
    }

    impl Tree {
        pub fn spawn(
            cmd: &mut Command,
            flags: u32,
            managed: bool,
        ) -> Result<(Child, Self), String> {
            if !managed {
                cmd.creation_flags(flags);
                return cmd
                    .spawn()
                    .map(|child| (child, Self(None)))
                    .map_err(|e| e.to_string());
            }
            let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job.is_null() {
                return Err(error());
            }
            let tree = Self(Some(job));
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    std::mem::size_of_val(&limits) as u32,
                )
            } == 0
            {
                return Err(error());
            }
            // Assign the child before any application code can create grandchildren.
            cmd.creation_flags(flags | CREATE_SUSPENDED);
            let mut child = cmd.spawn().map_err(|e| e.to_string())?;
            if unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) } == 0 {
                let reason = error();
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Could not contain build process: {reason}"));
            }
            if let Err(reason) = resume(child.id()) {
                unsafe {
                    TerminateJobObject(job, 1);
                }
                let _ = child.wait();
                return Err(reason);
            }
            Ok((child, tree))
        }

        pub fn stop(&mut self, child: &mut Child) -> Result<(), String> {
            let Some(job) = self.0 else {
                return Ok(());
            };
            if unsafe { TerminateJobObject(job, 1) } == 0 {
                return Err(error());
            }
            let _ = child.wait();
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION =
                    unsafe { std::mem::zeroed() };
                if unsafe {
                    QueryInformationJobObject(
                        job,
                        JobObjectBasicAccountingInformation,
                        &mut accounting as *mut _ as *mut _,
                        std::mem::size_of_val(&accounting) as u32,
                        std::ptr::null_mut(),
                    )
                } == 0
                {
                    return Err(error());
                }
                if accounting.ActiveProcesses == 0 {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err("Build process descendants did not exit".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    fn resume(pid: u32) -> Result<(), String> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(error());
        }
        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut found = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        while found {
            if entry.th32OwnerProcessID == pid {
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                unsafe {
                    CloseHandle(snapshot);
                }
                if thread.is_null() {
                    return Err(error());
                }
                let count = unsafe { ResumeThread(thread) };
                let reason = if count == u32::MAX {
                    Some(error())
                } else {
                    None
                };
                unsafe {
                    CloseHandle(thread);
                }
                return reason.map_or(Ok(()), Err);
            }
            found = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        unsafe {
            CloseHandle(snapshot);
        }
        Err("Could not find the suspended build process thread".into())
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub struct Tree;
    impl Tree {
        pub fn spawn(cmd: &mut Command, _: u32, managed: bool) -> Result<(Child, Self), String> {
            if managed {
                return Err(
                    "This Windows-only live tool requires Windows process-tree containment".into(),
                );
            }
            cmd.spawn()
                .map(|child| (child, Self))
                .map_err(|e| e.to_string())
        }
        pub fn stop(&mut self, _: &mut Child) -> Result<(), String> {
            Ok(())
        }
    }
}

pub use platform::Tree;
