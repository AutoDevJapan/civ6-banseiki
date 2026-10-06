use super::*;
use std::{
    os::windows::process::CommandExt,
    process::{Child, Command},
    time::{Duration, Instant},
};

#[unsafe(no_mangle)]
static BOOT_FIXTURE_ORIGINAL_CALLS: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
static BOOT_FIXTURE_SET_CALLS: AtomicU64 = AtomicU64::new(0);
core::arch::global_asm!(
    ".text",
    ".p2align 4",
    ".rept 16",
    "int3",
    ".endr",
    ".globl boot_fixture",
    ".seh_proc boot_fixture",
    "boot_fixture:",
    ".byte 0x40,0x53",
    ".seh_pushreg rbx",
    "sub rsp,32",
    ".seh_stackalloc 32",
    ".seh_endprologue",
    "lock inc qword ptr [rip + BOOT_FIXTURE_ORIGINAL_CALLS]",
    "lea rax,[rcx+7]",
    "add rsp,32",
    "pop rbx",
    "ret",
    ".seh_endproc",
    ".p2align 4",
    ".rept 16",
    "int3",
    ".endr",
    ".globl boot_set_fixture",
    ".seh_proc boot_set_fixture",
    "boot_set_fixture:",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    "push rdi",
    ".seh_pushreg rdi",
    "sub rsp,32",
    ".seh_stackalloc 32",
    ".seh_endprologue",
    "lock inc qword ptr [rip + BOOT_FIXTURE_SET_CALLS]",
    "lea rax,[rcx+rdx+9]",
    "mov rbx,[rsp+48]",
    "add rsp,32",
    "pop rdi",
    "ret",
    ".seh_endproc",
);
extern "system" {
    fn boot_fixture(a: usize, b: usize) -> usize;
    fn boot_set_fixture(a: usize, b: usize) -> usize;
}
extern "system" fn set_fixture_wrapper(a: usize, b: usize, ret: usize) -> usize {
    assert_ne!(ret, 0);
    (unsafe { boot_set_original(a, b) }) + 1
}
static WRAPPER_CALLS: AtomicU64 = AtomicU64::new(0);
extern "system" fn fixture_wrapper(a: usize, b: usize, ret: usize) -> usize {
    assert_ne!(ret, 0);
    WRAPPER_CALLS.fetch_add(1, Ordering::Relaxed);
    (unsafe { boot_release_original(a, b) }) + 1
}

#[test]
fn invalid_prepare_uses_actual_os_identity_and_never_installs() {
    assert_eq!(RustBridgeBootstrapPrepare(std::ptr::null()), 1);
    let input = Prepare {
        version: VERSION,
        size: size_of::<Prepare>() as u32,
        pid: unsafe { GetCurrentProcessId() },
        observation_seconds: 30,
        created_filetime: created(),
        bridge_module: 0,
        bridge_sha256: [0; 32],
        expected_context: 0,
    };
    assert_eq!(RustBridgeBootstrapPrepare(&input), 1); // real EXE is test binary
    assert!(STATE.lock().unwrap().is_none());
    assert_eq!(TRACE.reserved.load(Ordering::Acquire), 0);
}

#[test]
#[ignore = "explicit owned OS child; parent test below always executes it"]
fn owned_hotpatch_child() {
    let entry = boot_fixture as *const () as usize;
    assert_eq!(unsafe { boot_fixture(3, 0) }, 10);
    assert_eq!(BOOT_FIXTURE_ORIGINAL_CALLS.load(Ordering::Acquire), 1);
    let bytes = read(entry, 6).unwrap();
    assert!(hotpatch::prepare(entry, &[0; 6], fixture_wrapper as *const () as usize).is_err());
    assert_eq!(read(entry, 6).unwrap(), bytes);
    BOOT_RELEASE_CONTINUE.store(entry + 2, Ordering::Release);
    let hook = hotpatch::prepare(
        entry,
        &[0x40, 0x53, 0x48, 0x83, 0xec, 0x20],
        fixture_wrapper as *const () as usize,
    )
    .unwrap();
    hook.publish().unwrap();
    assert_eq!(unsafe { boot_fixture(3, 0) }, 11);
    assert_eq!(BOOT_FIXTURE_ORIGINAL_CALLS.load(Ordering::Acquire), 2);
    assert_eq!(WRAPPER_CALLS.load(Ordering::Acquire), 1);
    hook.restore().unwrap();
    assert_eq!(read(entry, 6).unwrap(), bytes);
    assert_eq!(unsafe { boot_fixture(3, 0) }, 10);
    assert_eq!(BOOT_FIXTURE_ORIGINAL_CALLS.load(Ordering::Acquire), 3);
    let entry = boot_set_fixture as *const () as usize;
    BOOT_SET_CONTINUE.store(entry + 5, Ordering::Release);
    let bytes = read(entry, 10).unwrap();
    let hook = hotpatch::prepare(
        entry,
        &[0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83, 0xec, 0x20],
        set_fixture_wrapper as *const () as usize,
    )
    .unwrap();
    hook.publish().unwrap();
    assert_eq!(unsafe { boot_set_fixture(3, 4) }, 17);
    assert_eq!(BOOT_FIXTURE_SET_CALLS.load(Ordering::Acquire), 1);
    hook.restore().unwrap();
    assert_eq!(read(entry, 10).unwrap(), bytes);
    assert_eq!(unsafe { boot_set_fixture(3, 4) }, 16);
    for address in [
        boot_release_original as *const () as u64,
        boot_set_original as *const () as u64,
    ] {
        let mut base = 0;
        assert!(!unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry(
                address,
                &mut base,
                std::ptr::null_mut(),
            )
        }
        .is_null());
    }
    println!("owned_hotpatch_original_once=true restored=true runtime_api_calls=0 game_live=false");
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
#[test]
fn actual_owned_hotpatch_original_once_and_reap() {
    let mut child = OwnedChild(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows::tests::owned_hotpatch_child",
                "--ignored",
                "--nocapture",
            ])
            .creation_flags(0x08000000)
            .spawn()
            .unwrap(),
    );
    let pid = child.0.id();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "owned child deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(child);
    println!("owned_child_pid={pid} reaped=true runtime_api_calls=0 game_live=false");
}
