//! Ownership retained after an unconfirmed native-observer shutdown.
//! This owner never closes a live target. It reaps its controller only after
//! the exact pinned target has exited, then releases the associated file pins.

/// Keep native recovery ownership alive through normal CLI exit and unwinding.
/// Entry points using a native Session must drop this guard before process::exit.
pub struct ExitGuard;
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
mod platform {
    use crate::{Error, Result};
    use std::{
        ops::{Deref, DerefMut},
        os::windows::io::{AsHandle, AsRawHandle, OwnedHandle},
        process::Child,
        sync::{LazyLock, Mutex},
        time::Duration,
    };
    use windows_sys::Win32::{
        Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::WaitForSingleObject,
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
        eprintln!("native recovery {id}: retaining controller {} and pinned target ownership ({description}); completion is unconfirmed", child.id());
        registry.pending.push(Pending {
            id,
            description,
            child,
            target: Box::new(target),
            kill_requested: false,
        });
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
        let remaining = registry.pending.len();
        drop(registry);
        // Closing authority can contain caller-owned RAII state. Release it
        // only after OS confirmation and outside the registry mutex.
        drop(completed);
        match failure {
            Some(error) => Err(error),
            None => Ok(remaining),
        }
    }
    pub(super) fn wait_for_pending() {
        wait_for(&REGISTRY);
    }
    fn wait_for(owner: &Mutex<Registry>) {
        let mut announced = false;
        let mut last_failure = None;
        loop {
            match reap_in(owner) {
                Ok(0) => return,
                Ok(count) if !announced => {
                    eprintln!("native recovery: {count} unresolved ownership records; waiting for exact process exit before releasing pins");
                    announced = true;
                }
                Err(error) => {
                    let text = error.to_string();
                    if last_failure.as_deref() != Some(text.as_str()) {
                        eprintln!("{text}; ownership remains retained");
                        last_failure = Some(text);
                    }
                }
                _ => {}
            }
            let waiter = {
                let registry = owner.lock().unwrap_or_else(|e| e.into_inner());
                registry.pending.first().map(Pending::wait_handle)
            };
            match waiter {
                None => return,
                Some(Ok(handle)) => {
                    if unsafe { WaitForSingleObject(handle.as_raw_handle() as HANDLE, u32::MAX) }
                        != WAIT_OBJECT_0
                    {
                        eprintln!("native recovery wait is unknown; ownership remains retained");
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
                Some(Err(error)) => {
                    eprintln!("{error}; ownership remains retained");
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
