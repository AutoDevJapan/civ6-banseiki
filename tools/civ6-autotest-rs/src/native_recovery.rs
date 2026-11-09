//! Ownership retained after an unconfirmed native-observer shutdown.
//! This owner never closes a live target. It reaps its controller only after
//! the exact pinned target has exited, then releases the associated file pins.

/// Keep native recovery ownership alive through normal CLI exit and unwinding.
/// Entry points using a native Session must drop this guard before process::exit.
pub struct ExitGuard;
pub const RECOVERY_REQUIRED: &str = "CIV6_NATIVE_RECOVERY_REQUIRED_V1";
impl Drop for ExitGuard {
    fn drop(&mut self) {
        wait_for_pending_recoveries();
    }
}

pub fn wait_for_pending_recoveries() {
    #[cfg(windows)]
    platform::wait_for_pending();
}

pub fn reap_completed_recoveries() -> crate::Result<usize> {
    #[cfg(windows)]
    {
        platform::reap_completed()
    }
    #[cfg(not(windows))]
    {
        Ok(0)
    }
}

#[cfg(windows)]
pub(crate) use platform::{retain, Held};
#[cfg(windows)]
pub(crate) type CloseAuthority = Box<dyn FnMut(u32, f64) -> crate::Result<()> + Send>;
#[cfg(windows)]
pub(crate) fn require_idle() -> crate::Result<()> {
    crate::time::check_interrupt()?;
    let remaining = reap_completed_recoveries()?;
    crate::time::check_interrupt()?;
    match remaining {
        0 => Ok(()),
        count => Err(crate::Error::input(format!("unresolved native ownership ({count} records); refusing another native attachment until exact target/controller exit"))),
    }
}

#[cfg(windows)]
mod platform {
    use crate::{Error, Result};
    use std::{
        io::Write,
        ops::{Deref, DerefMut},
        os::windows::io::{AsHandle, AsRawHandle, OwnedHandle},
        process::Child,
        sync::{LazyLock, Mutex},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{GetProcessId, GetProcessTimes, WaitForSingleObject},
    };

    /// A Session keeps ordinary field access while allowing Drop to transfer
    /// ownership without cloning a Child, forgetting resources, or unsafe moves.
    pub(crate) struct Held<T> {
        value: Option<T>,
    }
    impl<T> From<T> for Held<T> {
        fn from(value: T) -> Self {
            Self { value: Some(value) }
        }
    }
    impl<T> Held<T> {
        pub(crate) fn is_present(&self) -> bool {
            self.value.is_some()
        }
        pub(crate) fn take_owned(&mut self) -> T {
            self.value
                .take()
                .expect("native ownership already transferred")
        }
    }
    impl<T> Deref for Held<T> {
        type Target = T;
        fn deref(&self) -> &T {
            self.value.as_ref().expect("native ownership transferred")
        }
    }
    impl<T> DerefMut for Held<T> {
        fn deref_mut(&mut self) -> &mut T {
            self.value.as_mut().expect("native ownership transferred")
        }
    }

    trait Target: AsHandle + Send {}
    impl<T: AsHandle + Send> Target for T {}
    struct Pending {
        id: u64,
        description: String,
        child: Child,
        target: Box<dyn Target>,
        kill_requested: bool,
        controller_grace: Option<Instant>,
    }
    impl Pending {
        fn target_exited(&self) -> Result<bool> {
            match unsafe {
                WaitForSingleObject(self.target.as_handle().as_raw_handle() as HANDLE, 0)
            } {
                WAIT_OBJECT_0 => Ok(true),
                WAIT_TIMEOUT => Ok(false),
                _ => Err(Error::input(format!(
                    "native recovery target wait is unknown: {}",
                    std::io::Error::last_os_error()
                ))),
            }
        }
        fn reap(&mut self) -> Result<bool> {
            if !self.target_exited()? {
                return Ok(false);
            }
            if self.child.try_wait()?.is_some() {
                return Ok(true);
            }
            // Allow the controller to record its terminal output after the
            // target exits. This grace never permits more target operations.
            let grace = self
                .controller_grace
                .get_or_insert_with(|| Instant::now() + Duration::from_secs(2));
            if Instant::now() < *grace {
                return Ok(false);
            }
            // The target no longer has executable native state. Only our exact
            // spawned controller handle is eligible for this final reap.
            if !self.kill_requested {
                let killed = self.child.kill();
                self.kill_requested = killed.is_ok();
                if self.child.try_wait()?.is_some() {
                    return Ok(true);
                }
                killed?;
            }
            Ok(self.child.try_wait()?.is_some())
        }
        fn wait_handle(&self) -> Result<OwnedHandle> {
            Ok(if self.target_exited()? {
                self.child.as_handle().try_clone_to_owned()?
            } else {
                self.target.as_handle().try_clone_to_owned()?
            })
        }
    }
    #[derive(Default)]
    struct Registry {
        next: u64,
        pending: Vec<Pending>,
    }
    static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));

    fn diagnostic(args: std::fmt::Arguments<'_>) {
        // Diagnostic consumers can disappear during cancellation. An output
        // failure must never unwind a resource owner or its exit guard.
        let _ = writeln!(std::io::stderr().lock(), "{args}");
    }

    pub(crate) fn retain<T: AsHandle + Send + 'static>(
        child: Child,
        target: T,
        description: String,
    ) -> u64 {
        let mut registry = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        // This number is diagnostic, never process authority. Retention must
        // not fail by panicking after taking ownership from a Session.
        registry.next = registry.next.saturating_add(1);
        let id = registry.next;
        let controller = child.id();
        registry.pending.push(Pending {
            id,
            description,
            child,
            target: Box::new(target),
            kill_requested: false,
            controller_grace: None,
        });
        drop(registry);
        diagnostic(format_args!("native recovery {id}: retaining controller {controller} and pinned target ownership; completion is unconfirmed"));
        id
    }
    pub(super) fn reap_completed() -> Result<usize> {
        reap_in(&REGISTRY)
    }
    fn reap_in(owner: &Mutex<Registry>) -> Result<usize> {
        let mut registry = owner.lock().unwrap_or_else(|e| e.into_inner());
        let mut failure = None;
        let mut completed = Vec::new();
        let mut index = 0;
        while index < registry.pending.len() {
            match registry.pending[index].reap() {
                Ok(true) => completed.push(registry.pending.swap_remove(index)),
                Ok(false) => index += 1,
                Err(error) => {
                    let pending = &registry.pending[index];
                    failure = Some(Error::input(format!(
                        "native recovery {} ({}): {error}",
                        pending.id, pending.description
                    )));
                    index += 1;
                }
            }
        }
        drop(registry);
        // Closing authority can contain caller-owned RAII state. Release it
        // only after OS confirmation and outside the registry mutex.
        for completed in completed {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(completed))).is_err() {
                failure = Some(Error::input("native recovery resource destructor panicked; any newly retained ownership remains pending"));
            }
        }
        // A caller's destructor can register another owner. A snapshot from
        // before external Drop must never end the exit wait or allow attach.
        let remaining = owner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending
            .len();
        match failure {
            Some(error) => Err(error),
            None => Ok(remaining),
        }
    }
    pub(super) fn wait_for_pending() {
        wait_for(&REGISTRY);
    }
    fn notify_supervisor(owner: &Mutex<Registry>) {
        let Ok(nonce) = std::env::var("CIV6_NATIVE_RECOVERY_NONCE") else {
            return;
        };
        if nonce.is_empty()
            || nonce.len() > 64
            || !nonce.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        {
            return;
        }
        let targets = {
            let registry = owner.lock().unwrap_or_else(|e| e.into_inner());
            registry
                .pending
                .iter()
                .filter_map(|pending| {
                    let handle = pending.target.as_handle().as_raw_handle() as HANDLE;
                    let pid = unsafe { GetProcessId(handle) };
                    let mut created = FILETIME::default();
                    let mut exited = FILETIME::default();
                    let mut kernel = FILETIME::default();
                    let mut user = FILETIME::default();
                    let known = unsafe {
                        GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user)
                    } != 0;
                    let created = (u64::from(created.dwHighDateTime) << 32)
                        | u64::from(created.dwLowDateTime);
                    (known && pid != 0 && created != 0).then_some((pid, created))
                })
                .collect::<Vec<_>>()
        };
        for (pid, created) in targets {
            diagnostic(format_args!(
                "{} {nonce} {pid} {created}",
                super::RECOVERY_REQUIRED
            ));
        }
    }
    fn wait_for(owner: &Mutex<Registry>) {
        let mut announced = false;
        let mut last_failure = None;
        let mut interrupted = false;
        loop {
            match reap_in(owner) {
                Ok(0) => return,
                Ok(count) if !announced => {
                    // This is final exit/unwind, never the middle of a Runner
                    // step. Only now can the supervisor start game closure.
                    diagnostic(format_args!("native recovery: {count} unresolved ownership records; waiting for exact process exit before releasing pins"));
                }
                Err(error) => {
                    let text = error.to_string();
                    if last_failure.as_deref() != Some(text.as_str()) {
                        diagnostic(format_args!("{text}; ownership remains retained"));
                        last_failure = Some(text);
                    }
                }
                _ => {}
            }
            if !announced {
                notify_supervisor(owner);
                announced = true;
            }
            if !interrupted && crate::time::check_interrupt().is_err() {
                diagnostic(format_args!("native recovery: work is interrupted; waiting for the owning supervisor to close the exact target before resource release"));
                interrupted = true;
            }
            let waiter = {
                let registry = owner.lock().unwrap_or_else(|e| e.into_inner());
                registry.pending.first().map(Pending::wait_handle)
            };
            match waiter {
                None => return,
                Some(Ok(handle)) => {
                    let state =
                        unsafe { WaitForSingleObject(handle.as_raw_handle() as HANDLE, 250) };
                    if state == WAIT_OBJECT_0 {
                        // A signaled target can still have a controller inside
                        // its bounded terminal-output grace.
                        std::thread::sleep(Duration::from_millis(10));
                    } else if state != WAIT_TIMEOUT {
                        diagnostic(format_args!(
                            "native recovery wait is unknown; ownership remains retained"
                        ));
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
                Some(Err(error)) => {
                    diagnostic(format_args!("{error}; ownership remains retained"));
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{
            fs::{File, OpenOptions},
            os::windows::{fs::OpenOptionsExt, io::BorrowedHandle, process::CommandExt},
            process::{Command, Stdio},
            sync::{mpsc, Arc},
        };

        struct Fixture(Held<Child>);
        impl Fixture {
            fn spawn() -> Self {
                Self(
                    Command::new("powershell.exe")
                        .args([
                            "-NoProfile",
                            "-NonInteractive",
                            "-Command",
                            "Start-Sleep -Seconds 60",
                        ])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .creation_flags(0x08000000)
                        .spawn()
                        .unwrap()
                        .into(),
                )
            }
            fn stop(&mut self) {
                if self.0.try_wait().unwrap().is_none() {
                    self.0.kill().unwrap();
                }
                self.0.wait().unwrap();
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                if self.0.is_present() {
                    let _ = self.0.kill();
                    let _ = self.0.wait();
                }
            }
        }
        struct ControllerGuard(OwnedHandle);
        impl Drop for ControllerGuard {
            fn drop(&mut self) {
                let handle = self.0.as_raw_handle() as HANDLE;
                if unsafe { WaitForSingleObject(handle, 0) } != WAIT_OBJECT_0 {
                    let _ = unsafe {
                        windows_sys::Win32::System::Threading::TerminateProcess(handle, 1)
                    };
                }
                let _ = unsafe { WaitForSingleObject(handle, 3_000) };
            }
        }
        struct HookedTarget {
            handle: OwnedHandle,
            _pin: File,
            entered: Mutex<Option<mpsc::SyncSender<()>>>,
            release: Mutex<mpsc::Receiver<()>>,
        }
        impl AsHandle for HookedTarget {
            fn as_handle(&self) -> BorrowedHandle<'_> {
                if let Some(entered) = self.entered.lock().unwrap().take() {
                    entered.send(()).unwrap();
                    let _ = self.release.lock().unwrap().recv();
                }
                self.handle.as_handle()
            }
        }

        #[test]
        #[ignore = "owned recovery fixture, invoked by its parent test"]
        fn broken_stderr_retention_worker() {
            use std::io::{Read, Write};
            let mut start = [0u8];
            std::io::stdin().read_exact(&mut start).unwrap();
            let mut target = Fixture::spawn();
            let mut controller = Fixture::spawn();
            let controller_handle =
                ControllerGuard(controller.0.as_handle().try_clone_to_owned().unwrap());
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("broken-stderr-pin.bin");
            std::fs::write(&path, [7u8; 48]).unwrap();
            let pin = OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&path)
                .unwrap();
            let retained = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                retain(
                    controller.0.take_owned(),
                    HookedTarget {
                        handle: target.0.as_handle().try_clone_to_owned().unwrap(),
                        _pin: pin,
                        entered: Mutex::new(None),
                        release: Mutex::new(mpsc::channel().1),
                    },
                    "broken diagnostic pipe fixture".into(),
                )
            }))
            .is_ok();
            let registered = REGISTRY
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pending
                .len();
            let held = OpenOptions::new().write(true).open(&path).is_err();
            // A failed native owner must block another native attachment,
            // while leaving unrelated ordinary game paths alone.
            let close = || Box::new(|_, _| Ok(())) as crate::native_recovery::CloseAuthority;
            let bridge = crate::native_bridge::Session::start(
                0,
                0.0,
                folder.path(),
                "unstarted",
                1,
                "can-ever",
                close(),
            )
            .err()
            .unwrap()
            .to_string();
            let trace = crate::native_trace::Session::start(
                0,
                0.0,
                folder.path(),
                "unstarted",
                1,
                "lifecycle",
                close(),
            )
            .err()
            .unwrap()
            .to_string();
            let bootstrap = crate::native_bridge::Session::start_bootstrap(
                0,
                0.0,
                folder.path(),
                &serde_json::json!({}),
                close(),
                Instant::now() + Duration::from_secs(5),
            )
            .err()
            .unwrap()
            .to_string();
            target.stop();
            wait_for_pending();
            let can_attach_after_exit = crate::native_recovery::require_idle().is_ok();
            let released = OpenOptions::new().write(true).open(&path).is_ok();
            let controller_exited =
                unsafe { WaitForSingleObject(controller_handle.0.as_raw_handle() as HANDLE, 0) }
                    == WAIT_OBJECT_0;
            writeln!(std::io::stdout(), "RECOVERY_RESULT retained={retained} registered={registered} held={held} released={released} controller_exited={controller_exited}").unwrap();
            writeln!(
                std::io::stdout(),
                "REATTACH bridge={bridge:?} trace={trace:?} bootstrap={bootstrap:?}"
            )
            .unwrap();
            assert!(retained && registered == 1 && held && released && controller_exited);
            for error in [bridge, trace, bootstrap] {
                assert!(error.contains("unresolved native ownership"), "{error}");
            }
            assert!(can_attach_after_exit);
        }

        #[test]
        fn broken_stderr_cannot_lose_registered_processes_or_file_pins() {
            use std::io::Write;
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "native_recovery::platform::tests::broken_stderr_retention_worker",
                    "--nocapture",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(0x08000000)
                .spawn()
                .unwrap();
            // Synchronize the broken pipe before production retain() runs.
            drop(child.stderr.take());
            child.stdin.take().unwrap().write_all(&[1]).unwrap();
            let result = child.wait_with_output().unwrap();
            let output = String::from_utf8_lossy(&result.stdout);
            assert!(result.status.success(), "{output}");
            assert!(output.contains("RECOVERY_RESULT retained=true registered=1 held=true released=true controller_exited=true"), "{output}");
        }

        #[test]
        fn target_exit_allows_controller_to_flush_terminal_output_before_forced_reap() {
            use std::io::{BufRead, BufReader, Read, Write};
            let mut target = Fixture::spawn();
            let mut controller = Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "[Console]::WriteLine('controller ready'); [Console]::Out.Flush(); [Console]::ReadLine() | Out-Null; [Console]::Write('terminal complete')",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .spawn()
                .unwrap();
            let guard = ControllerGuard(controller.as_handle().try_clone_to_owned().unwrap());
            let mut input = controller.stdin.take().unwrap();
            let output = controller.stdout.take().unwrap();
            let (ready_tx, ready_rx) = mpsc::channel();
            let reader = std::thread::spawn(move || {
                let mut output = BufReader::new(output);
                let mut ready = String::new();
                let result = output.read_line(&mut ready).map(|_| ready);
                let _ = ready_tx.send(result);
                let mut text = String::new();
                output.read_to_string(&mut text).map(|_| text)
            });
            // The real controller is already READY before target shutdown.
            // Keep process startup out of the terminal-output grace assertion.
            assert_eq!(
                ready_rx
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap()
                    .unwrap()
                    .trim(),
                "controller ready"
            );
            let owner = Mutex::new(Registry {
                next: 1,
                pending: vec![Pending {
                    id: 1,
                    description: "terminal flush fixture".into(),
                    child: controller,
                    target: Box::new(target.0.as_handle().try_clone_to_owned().unwrap()),
                    kill_requested: false,
                    controller_grace: None,
                }],
            });
            target.stop();
            let pending = reap_in(&owner).unwrap();
            let alive = unsafe { WaitForSingleObject(guard.0.as_raw_handle() as HANDLE, 0) }
                == WAIT_TIMEOUT;
            input.write_all(b"target exited\n").unwrap();
            drop(input);
            wait_for(&owner);
            let text = reader.join().unwrap().unwrap();
            assert_eq!(pending, 1);
            assert!(alive);
            assert_eq!(text, "terminal complete");
        }

        fn reentrant_destructor_case(panic_after: bool) {
            struct RegisterOnDrop {
                handle: OwnedHandle,
                owner: Arc<Mutex<Registry>>,
                pending: Option<Pending>,
                panic_after: bool,
            }
            impl AsHandle for RegisterOnDrop {
                fn as_handle(&self) -> BorrowedHandle<'_> {
                    self.handle.as_handle()
                }
            }
            impl Drop for RegisterOnDrop {
                fn drop(&mut self) {
                    if let Some(pending) = self.pending.take() {
                        self.owner.lock().unwrap().pending.push(pending);
                    }
                    assert!(
                        !self.panic_after,
                        "owned destructor fixture panic after registration"
                    );
                }
            }
            let owner = Arc::new(Mutex::new(Registry::default()));
            let mut old_target = Fixture::spawn();
            let mut old_controller = Fixture::spawn();
            let mut next_target = Fixture::spawn();
            let mut next_controller = Fixture::spawn();
            let next_guard =
                ControllerGuard(next_controller.0.as_handle().try_clone_to_owned().unwrap());
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("reentrant-retention-pin.bin");
            std::fs::write(&path, [9u8; 48]).unwrap();
            let pin = OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&path)
                .unwrap();
            old_target.stop();
            old_controller.stop();
            owner.lock().unwrap().pending.push(Pending {
                id: 1,
                description: "completed owner with reentrant destructor".into(),
                child: old_controller.0.take_owned(),
                target: Box::new(RegisterOnDrop {
                    handle: old_target.0.as_handle().try_clone_to_owned().unwrap(),
                    owner: Arc::clone(&owner),
                    panic_after,
                    pending: Some(Pending {
                        id: 2,
                        description: "registered during resource destruction".into(),
                        child: next_controller.0.take_owned(),
                        target: Box::new(HookedTarget {
                            handle: next_target.0.as_handle().try_clone_to_owned().unwrap(),
                            _pin: pin,
                            entered: Mutex::new(None),
                            release: Mutex::new(mpsc::channel().1),
                        }),
                        kill_requested: false,
                        controller_grace: None,
                    }),
                }),
                kill_requested: false,
                controller_grace: None,
            });
            let reported =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reap_in(&owner)));
            let actual = owner.lock().unwrap().pending.len();
            let held = OpenOptions::new().write(true).open(&path).is_err();
            let alive = unsafe { WaitForSingleObject(next_guard.0.as_raw_handle() as HANDLE, 0) }
                == WAIT_TIMEOUT;
            next_target.stop();
            wait_for(&owner);
            let released = OpenOptions::new().write(true).open(&path).is_ok();
            assert!(held && alive && released);
            assert_eq!(actual, 1);
            assert!(
                reported.is_ok(),
                "resource destructor escaped the recovery owner"
            );
            let reported = reported.unwrap();
            if panic_after {
                assert!(reported.is_err());
            } else {
                assert_eq!(
                    reported.unwrap(),
                    actual,
                    "reap returned a pre-destructor snapshot"
                );
            }
        }

        #[test]
        fn destructor_registration_cannot_make_reap_return_a_stale_zero() {
            reentrant_destructor_case(false);
        }

        #[test]
        fn panicking_resource_destructor_keeps_new_ownership_and_returns_an_error() {
            reentrant_destructor_case(true);
        }

        #[test]
        fn exit_wait_keeps_pins_and_peer_then_reaps_controller_after_target_exit() {
            let mut target = Fixture::spawn();
            let mut peer = Fixture::spawn();
            let mut controller = Fixture::spawn();
            let controller_handle =
                ControllerGuard(controller.0.as_handle().try_clone_to_owned().unwrap());
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("owned-recovery-pin.bin");
            std::fs::write(&path, [3u8; 48]).unwrap();
            let pin = OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&path)
                .unwrap();
            let (entered_tx, entered_rx) = mpsc::sync_channel(0);
            let (release_tx, release_rx) = mpsc::channel();
            // This local owner executes the same wait/reap implementation as
            // ExitGuard without interacting with other concurrent test owners.
            let owner = Arc::new(Mutex::new(Registry {
                next: 1,
                pending: vec![Pending {
                    id: 1,
                    description: "owned exit-wait fixture".into(),
                    child: controller.0.take_owned(),
                    target: Box::new(HookedTarget {
                        handle: target.0.as_handle().try_clone_to_owned().unwrap(),
                        _pin: pin,
                        entered: Mutex::new(Some(entered_tx)),
                        release: Mutex::new(release_rx),
                    }),
                    kill_requested: false,
                    controller_grace: None,
                }],
            }));
            let waiting_owner = Arc::clone(&owner);
            let (done_tx, done_rx) = mpsc::channel();
            let waiter = std::thread::spawn(move || {
                wait_for(&waiting_owner);
                done_tx.send(()).unwrap();
            });
            entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let pin_held = OpenOptions::new().write(true).open(&path).is_err();
            let controller_alive =
                unsafe { WaitForSingleObject(controller_handle.0.as_raw_handle() as HANDLE, 0) }
                    == WAIT_TIMEOUT;
            let peer_alive = peer.0.try_wait().unwrap().is_none();
            release_tx.send(()).unwrap();
            target.stop();
            done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            waiter.join().unwrap();
            let controller_exited =
                unsafe { WaitForSingleObject(controller_handle.0.as_raw_handle() as HANDLE, 0) }
                    == WAIT_OBJECT_0;
            let pin_released = OpenOptions::new().write(true).open(&path).is_ok();
            let peer_still_alive = peer.0.try_wait().unwrap().is_none();
            peer.stop();
            assert!(pin_held && controller_alive && peer_alive);
            assert!(controller_exited && pin_released && peer_still_alive);
            assert!(owner.lock().unwrap().pending.is_empty());
        }
    }
}
