use super::*;
use crate::lifecycle_trace::{self, LifecycleTrace};
use crate::lifecycle_transport::SharedTrace;
use crate::runtime::{OathRuntimeConfig, OATH_MODE_DIAGNOSTIC, OATH_RUNTIME_VERSION};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlAddFunctionTable, CONTEXT, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

pub(super) struct Synthetic {
    pub core: std::sync::atomic::AtomicUsize,
    pub clock: AtomicU64,
    context: AtomicU64,
    factory: AtomicU64,
    calls: AtomicU32,
    installs: AtomicU32,
    continued: AtomicU32,
    case: String,
    trace: usize,
    call: usize,
    slots: [AtomicU64; 2],
}
pub(super) static SYNTHETIC: OnceLock<Synthetic> = OnceLock::new();
type Call = unsafe extern "system-unwind" fn(usize, usize, usize, usize) -> usize;
const CORE: usize = 0x800000;
const RESULT: usize = 0x12345678_abcd0001;
const ARGS: [usize; 4] = [0x1000_0001, 0x2000_0002, 0x3000_0003, 0x4000_0004];
unsafe extern "system-unwind" fn original(a: usize, b: usize, c: usize, d: usize) -> usize {
    let t = SYNTHETIC.get().unwrap();
    t.calls.fetch_add(1, Ordering::SeqCst);
    assert_eq!([a, b, c, d], ARGS);
    assert_eq!(RECEIPT.state.load(Ordering::Acquire), PREPARED);
    assert_eq!(t.installs.load(Ordering::Acquire), 0);
    assert_eq!(
        unsafe { GetLastError() },
        1234,
        "incoming LastError preserved"
    );
    if t.case == "unwind" {
        panic!("synthetic cold original unwind");
    }
    if t.case == "reentry" {
        let f: Call = unsafe { std::mem::transmute(t.call) };
        unsafe {
            f(a, b, c, d);
        }
    }
    if t.case != "nonxp2" && t.case != "nocore" {
        t.core.store(CORE, Ordering::Release);
    }
    if t.case == "context" {
        t.context.store(1, Ordering::Release);
    }
    if t.case == "factory" {
        t.factory.store(1, Ordering::Release);
    }
    if t.case == "deadline" {
        t.clock.store(
            RECEIPT.deadline_tick.load(Ordering::Acquire),
            Ordering::Release,
        );
    }
    unsafe { SetLastError(5678) };
    if t.case == "failed" {
        RESULT & !0xff
    } else {
        RESULT
    }
}
unsafe extern "system-unwind" fn root(a: u64, b: u64, c: u64, d: u64) -> u64 {
    a ^ b ^ c ^ d
}
impl Synthetic {
    pub(super) fn verify_boundary(
        &self,
        s: &Resident,
        core: usize,
        ret: usize,
    ) -> Result<(), OathStatus> {
        if self.context.load(Ordering::Acquire) != 0
            || self.factory.load(Ordering::Acquire) != CORE as u64 + 0x752d50
            || core != CORE
            || !direct_caller(s.exe, ret)
        {
            return Err(OathStatus::EntryBytes);
        }
        Ok(())
    }
    pub(super) fn install(&self, request: &mut SharedLifecycleConfig) -> u32 {
        assert_eq!(
            self.continued.load(Ordering::Acquire),
            0,
            "caller still held"
        );
        assert_eq!(self.calls.load(Ordering::Acquire), 1, "original ran once");
        assert_eq!(self.installs.fetch_add(1, Ordering::AcqRel), 0);
        lifecycle_trace::install_shared_verified(
            (request as *mut SharedLifecycleConfig).cast(),
            |_| {
                if self.case == "installfail" {
                    return Err(OathStatus::HookFailure);
                }
                Ok(std::array::from_fn(|i| {
                    (
                        &self.slots[i] as *const _ as usize,
                        root as *const () as u64,
                    )
                }))
            },
        )
    }
    pub(super) fn before_start(&self) {
        let trace = unsafe { &*(self.trace as *const LifecycleTrace) };
        if self.case == "startfail" {
            trace.stopped.store(2, Ordering::Release);
        }
    }
    pub(super) fn after_start(&self, deadline: u64) {
        let trace = unsafe { &*(self.trace as *const LifecycleTrace) };
        assert_eq!(
            trace.deadline_ms.load(Ordering::Acquire),
            deadline,
            "no reset at Start"
        );
        if self.case == "lateinstall" {
            self.clock.store(deadline, Ordering::Release);
        }
    }
}
fn config(handle: u64) -> ColdPrepareConfig {
    ColdPrepareConfig {
        shared: SharedLifecycleConfig {
            config: OathRuntimeConfig {
                version: OATH_RUNTIME_VERSION,
                size: 48,
                game_pid: unsafe { GetCurrentProcessId() },
                max_seconds: 300,
                created_filetime: crate::runtime::current_created_filetime().unwrap(),
                mode: OATH_MODE_DIAGNOSTIC,
                ..OathRuntimeConfig::default()
            },
            mapping_handle: handle,
            mapping_size: MAPPING_SIZE,
            reserved: 0,
        },
        deadline_tick: now() + 298000,
        reserved: 0,
    }
}
fn receipt() -> ColdReceipt {
    let out = ColdReceipt::new();
    assert_eq!(
        Civ6OathColdReadReceipt((&out as *const ColdReceipt as *mut ColdReceipt).cast()),
        0
    );
    out
}

// Owns synthetic native code only in a separate, hidden child. Both original
// and caller have real registered unwind information. No public gate bypass.
fn synthetic_image() -> (usize, usize) {
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0x250000,
            MEM_RESERVE | MEM_COMMIT,
            PAGE_EXECUTE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    let entry = base + LOADER;
    let mut code = PREFIX.to_vec();
    code.extend_from_slice(&[0x48, 0x81, 0xc4, 0x48, 0x02, 0, 0]); // add rsp,248
    code.extend_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
    code.extend_from_slice(&(original as *const () as u64).to_le_bytes());
    unsafe {
        ptr::write_bytes((entry - 5) as *mut u8, 0xcc, 5);
        ptr::copy_nonoverlapping(code.as_ptr(), entry as *mut u8, code.len());
    }
    let call = base + CALLER_RETURN_RVA as usize - 9;
    let mut caller = vec![0x48, 0x83, 0xec, 0x28];
    caller.extend_from_slice(&rel32_jump(call + 4, entry).unwrap());
    caller[4] = 0xe8; // checked fixed call24B52D
    caller.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
    unsafe {
        ptr::copy_nonoverlapping(caller.as_ptr(), call as *mut u8, caller.len());
    }
    let table = [
        IMAGE_RUNTIME_FUNCTION_ENTRY {
            BeginAddress: LOADER as u32,
            EndAddress: (LOADER + code.len()) as u32,
            Anonymous:
                windows_sys::Win32::System::Diagnostics::Debug::IMAGE_RUNTIME_FUNCTION_ENTRY_0 {
                    UnwindData: 0x100,
                },
        },
        IMAGE_RUNTIME_FUNCTION_ENTRY {
            BeginAddress: (call - base) as u32,
            EndAddress: (call - base + caller.len()) as u32,
            Anonymous:
                windows_sys::Win32::System::Diagnostics::Debug::IMAGE_RUNTIME_FUNCTION_ENTRY_0 {
                    UnwindData: 0x120,
                },
        },
    ];
    unsafe {
        ptr::copy_nonoverlapping(
            [1u8, 7, 2, 0, 7, 1, 0x49, 0].as_ptr(),
            (base + 0x100) as *mut u8,
            8,
        );
        ptr::copy_nonoverlapping(
            [1u8, 4, 1, 0, 4, 0x42, 0, 0].as_ptr(),
            (base + 0x120) as *mut u8,
            8,
        );
        ptr::copy_nonoverlapping(
            table.as_ptr(),
            (base + 0x200) as *mut IMAGE_RUNTIME_FUNCTION_ENTRY,
            2,
        );
        assert!(RtlAddFunctionTable((base + 0x200) as _, 2, base as u64));
        let mut old = 0;
        assert_ne!(
            VirtualProtect(base as _, 0x250000, PAGE_EXECUTE_READ, &mut old),
            0
        );
        assert_ne!(
            FlushInstructionCache(GetCurrentProcess(), base as _, 0x250000),
            0
        );
    }
    (base, call)
}

#[test]
fn child() {
    let Ok(case) = std::env::var("CC_COLD_LIFECYCLE_CASE") else {
        return;
    };
    let shared = SharedTrace::new().unwrap();
    let handle = shared
        .duplicate_into(unsafe { GetCurrentProcess() })
        .unwrap();
    let mut request = config(handle);
    let before = request;
    if case.starts_with("public") {
        let expected = match case.as_str() {
            "public-invalid" => {
                request.shared.config.max_seconds = 301;
                OathStatus::InvalidInput
            }
            "public-pid" => {
                request.shared.config.game_pid += 1;
                OathStatus::ProcessIdentity
            }
            "public-filetime" => {
                request.shared.config.created_filetime += 1;
                OathStatus::ProcessIdentity
            }
            "public-mapping" => {
                request.shared.mapping_size -= 8;
                OathStatus::InvalidInput
            }
            "public-zero-deadline" => {
                request.deadline_tick = 0;
                OathStatus::InvalidInput
            }
            "public-expired-deadline" => {
                request.deadline_tick = now();
                OathStatus::InvalidInput
            }
            "public-future-deadline" => {
                request.deadline_tick = now() + 301000;
                OathStatus::InvalidInput
            }
            "public-tail" => {
                request.reserved = 1;
                OathStatus::InvalidInput
            }
            "public-dirty-mapping" => {
                unsafe { &*shared.as_ptr() }
                    .next
                    .store(1, Ordering::Release);
                OathStatus::InvalidInput
            }
            _ => OathStatus::ProcessIdentity,
        };
        let before = request;
        assert_eq!(
            Civ6OathColdPrepare((&mut request as *mut ColdPrepareConfig).cast()),
            expected as u32
        );
        assert_eq!(request, before);
        assert_eq!(receipt().state.load(Ordering::Acquire), FAILED);
        assert_eq!(
            Civ6OathColdPrepare((&mut request as *mut ColdPrepareConfig).cast()),
            OathStatus::WrongState as u32
        );
        return;
    }
    let (base, call) = synthetic_image();
    let trace = unsafe { MappedTrace::map(handle as _, MAPPING_SIZE) }
        .unwrap()
        .retain();
    assert!(SYNTHETIC
        .set(Synthetic {
            core: std::sync::atomic::AtomicUsize::new(0),
            clock: AtomicU64::new(0),
            context: AtomicU64::new(0),
            factory: AtomicU64::new(CORE as u64 + 0x752d50),
            calls: AtomicU32::new(0),
            installs: AtomicU32::new(0),
            continued: AtomicU32::new(0),
            case: case.clone(),
            trace: trace as *const _ as usize,
            call,
            slots: std::array::from_fn(|_| AtomicU64::new(root as *const () as u64))
        })
        .is_ok());
    // Simulate elapsed preparation; the same deadline must survive Start.
    let initialized = now().saturating_sub(1500);
    USED.store(1, Ordering::Release);
    assert_eq!(
        prepare_verified(
            (&mut request as *mut ColdPrepareConfig).cast(),
            initialized,
            |_| Ok((base, vec![]))
        ),
        Ok(())
    );
    assert_eq!(request.shared.config.trace_size, 32816);
    assert_ne!(request.shared.config.out_trace_address, 0);
    assert_eq!(request.shared.config.out_trace_address % 8, 0);
    let mut normalized = request;
    normalized.shared.config.out_trace_address = 0;
    normalized.shared.config.trace_size = 0;
    assert_eq!(normalized, before, "only output fields changed");
    assert!(
        trace.initialized(),
        "Prepare does not publish lifecycle TRACE or start"
    );
    assert_eq!(
        lifecycle_trace::Civ6OathLifecycleStart(),
        OathStatus::WrongState as u32
    );
    let prepared = receipt();
    assert_eq!(prepared.state.load(Ordering::Acquire), PREPARED);
    assert_eq!(prepared.core_base.load(Ordering::Acquire), 0);
    assert_eq!(prepared.ready_tick.load(Ordering::Acquire), 0);
    // Serial libtest prefix must never share a line with this protocol marker.
    println!("\nCOLD_PREPARED");
    assert_eq!(
        Civ6OathColdPrepare((&mut request as *mut ColdPrepareConfig).cast()),
        OathStatus::WrongState as u32
    );
    assert_eq!(receipt().state.load(Ordering::Acquire), PREPARED);
    let t = SYNTHETIC.get().unwrap();
    if case == "wrongcaller" {
        let f: Call = unsafe { std::mem::transmute(base + LOADER) };
        unsafe {
            SetLastError(1234);
            f(ARGS[0], ARGS[1], ARGS[2], ARGS[3]);
        }
        panic!("wrong successful XP2 caller resumed");
    }
    let f: Call = unsafe { std::mem::transmute(call) };
    unsafe { SetLastError(1234) };
    if case == "unwind" {
        assert!(
            std::panic::catch_unwind(|| unsafe { f(ARGS[0], ARGS[1], ARGS[2], ARGS[3]) }).is_err()
        );
        assert_eq!(t.calls.load(Ordering::Acquire), 1);
        assert_eq!(receipt().state.load(Ordering::Acquire), FAILED);
        assert_eq!(FLIGHT.load(Ordering::Acquire), 0);
        assert_eq!(t.installs.load(Ordering::Acquire), 0);
        return;
    }
    let result = unsafe { f(ARGS[0], ARGS[1], ARGS[2], ARGS[3]) };
    let error = unsafe { GetLastError() };
    t.continued.fetch_add(1, Ordering::AcqRel);
    assert_eq!(t.calls.load(Ordering::Acquire), 1);
    assert_eq!(error, 5678);
    assert_eq!(
        result,
        if case == "failed" {
            RESULT & !0xff
        } else {
            RESULT
        }
    );
    let read = receipt();
    if case == "nocore" || case == "nonxp2" {
        assert_eq!(read.state.load(Ordering::Acquire), PREPARED);
        assert_eq!(read.status.load(Ordering::Acquire), OUTSIDE_COVERAGE);
        assert_eq!(read.ready_tick.load(Ordering::Acquire), 0);
        assert_eq!(t.installs.load(Ordering::Acquire), 0);
    } else if case == "failed" {
        assert_eq!(read.state.load(Ordering::Acquire), FAILED);
        assert_eq!(t.installs.load(Ordering::Acquire), 0);
    } else {
        assert_eq!(read.magic.load(Ordering::Acquire), MAGIC);
        assert_eq!(read.version.load(Ordering::Acquire), VERSION);
        assert_eq!(read.state.load(Ordering::Acquire), READY);
        assert_eq!(read.status.load(Ordering::Acquire), 0);
        assert_eq!(
            read.caller_return_rva.load(Ordering::Acquire),
            CALLER_RETURN_RVA
        );
        assert_ne!(read.thread.load(Ordering::Acquire), 0);
        assert_eq!(read.core_base.load(Ordering::Acquire), CORE as u64);
        assert_eq!(read.initialized_tick.load(Ordering::Acquire), initialized);
        assert!(
            read.ready_tick.load(Ordering::Acquire) < read.deadline_tick.load(Ordering::Acquire)
        );
        assert_eq!(
            read.deadline_tick.load(Ordering::Acquire),
            before.deadline_tick
        );
        assert_eq!(t.installs.load(Ordering::Acquire), 1);
        assert_eq!(
            trace.next.load(Ordering::Acquire),
            0,
            "READY is not first-root observation"
        );
        let root_call: unsafe extern "system-unwind" fn(u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(t.slots[0].load(Ordering::Acquire)) };
        assert_eq!(unsafe { root_call(1, 2, 3, 4) }, 4);
        assert_eq!(
            trace.next.load(Ordering::Acquire),
            1,
            "root separately observed after caller resumes"
        );
        assert_eq!(lifecycle_trace::Civ6OathLifecycleStop(), 0);
        assert_eq!(lifecycle_trace::Civ6OathLifecyclePoll(), 0);
    }
    assert_eq!(read.reserved0.load(Ordering::Acquire), 0);
    assert_eq!(read.reserved1.load(Ordering::Acquire), 0);
}

#[test]
fn synthetic_gateway_and_public_gate_subprocesses() {
    for case in [
        "normal",
        "nonxp2",
        "nocore",
        "failed",
        "unwind",
        "wrongcaller",
        "context",
        "factory",
        "deadline",
        "reentry",
        "installfail",
        "startfail",
        "lateinstall",
        "public-valid",
        "public-invalid",
        "public-pid",
        "public-filetime",
        "public-mapping",
        "public-zero-deadline",
        "public-expired-deadline",
        "public-future-deadline",
        "public-tail",
        "public-dirty-mapping",
    ] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "cold_lifecycle::tests::child", "--nocapture"])
            .env("CC_COLD_LIFECYCLE_CASE", case)
            .creation_flags(0x08000000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let limit = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= limit {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("cold child timed out: {case}");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let abort = matches!(
            case,
            "wrongcaller"
                | "context"
                | "factory"
                | "deadline"
                | "reentry"
                | "installfail"
                | "startfail"
                | "lateinstall"
        );
        if abort {
            assert_eq!(
                status.code().map(|n| n as u32),
                Some(0xc0000409),
                "{case} must abort, not just panic/return"
            );
        } else {
            assert!(status.success(), "{case} failed: {status}");
        }
    }
}

unsafe extern "system" {
    fn RtlVirtualUnwind(
        kind: u32,
        base: u64,
        pc: u64,
        entry: *const c_void,
        context: *mut CONTEXT,
        data: *mut *mut c_void,
        frame: *mut u64,
        pointers: *mut c_void,
    ) -> *mut c_void;
}
#[test]
fn static_original_and_gateway_unwind_restore_native_caller() {
    for (address, frame_size) in [
        (cold_original as *const () as u64, 0x248usize),
        (cold_gateway as *const () as u64, 0x38),
    ] {
        let mut base = 0;
        let entry = unsafe { RtlLookupFunctionEntry(address, &mut base, null_mut()) };
        assert!(!entry.is_null());
        let codes =
            copy_current::<4>(base + u64::from(unsafe { (*entry).Anonymous.UnwindData })).unwrap();
        let mut stack = [0u64; 80];
        stack[frame_size / 8] = 0x123456789abcdef0;
        let mut context: CONTEXT = unsafe { std::mem::zeroed() };
        context.Rsp = stack.as_ptr() as u64;
        context.Rip = address + u64::from(codes[1]);
        context.Rbx = 0x1122;
        context.Rbp = 0x3344;
        context.Rdi = 0x5566;
        let mut data = null_mut();
        let mut frame = 0;
        unsafe {
            RtlVirtualUnwind(
                0,
                base,
                context.Rip,
                entry.cast(),
                &mut context,
                &mut data,
                &mut frame,
                null_mut(),
            );
        }
        assert_eq!(context.Rip, stack[frame_size / 8]);
        assert_eq!(context.Rsp, stack.as_ptr() as u64 + frame_size as u64 + 8);
        assert_eq!(
            (context.Rbx, context.Rbp, context.Rdi),
            (0x1122, 0x3344, 0x5566)
        );
    }
}
#[test]
fn receipt_layout_and_rejection_of_invalid_output() {
    assert_eq!(std::mem::size_of::<ColdReceipt>(), 96);
    assert_eq!(std::mem::offset_of!(ColdReceipt, pid), 16);
    assert_eq!(std::mem::offset_of!(ColdReceipt, filetime), 24);
    assert_eq!(std::mem::offset_of!(ColdReceipt, caller_return_rva), 48);
    assert_eq!(std::mem::offset_of!(ColdReceipt, reserved1), 88);
    assert_eq!(
        Civ6OathColdReadReceipt(null_mut()),
        OathStatus::InvalidInput as u32
    );
}
