//! Owned native bodies, separate from licensed-image/static verification.
use super::*;
use std::os::windows::process::CommandExt;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlUnwindEx(
        frame: *const c_void,
        ip: *const c_void,
        record: *const EXCEPTION_RECORD,
        value: *const c_void,
        context: *const CONTEXT,
        history: *const c_void,
    );
    fn RtlVirtualUnwind(
        kind: u32,
        base: u64,
        pc: u64,
        entry: *const IMAGE_RUNTIME_FUNCTION_ENTRY,
        context: *mut CONTEXT,
        data: *mut *mut c_void,
        frame: *mut u64,
        pointers: *mut c_void,
    ) -> usize;
}
static PROBED_ACTIVE: AtomicU64 = AtomicU64::new(0);
type Block = (std::sync::mpsc::Sender<u64>, std::sync::mpsc::Receiver<()>);
static BLOCK: Mutex<Option<Block>> = Mutex::new(None);

fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_NATIVE_OUTER_CHILD";
    if std::env::var(ENV).as_deref() == Ok(name) {
        return true;
    }
    let full = format!("windows::root_observation::native_frames::outer_tests::{name}");
    let output = crate::windows::test_child_output::run(
        std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env(ENV, name)
            .args(["--exact", &full, "--include-ignored", "--nocapture"]),
        Duration::from_secs(30),
    )
    .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "outer child {}\n{stdout}\n{stderr}",
        output.status
    );
    assert!(
        stdout.contains("running 1 test")
            && stdout.contains(&full)
            && stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"),
        "outer exact test was not executed: {stdout}\n{stderr}"
    );
    print!("{stdout}");
    false
}

extern "system" fn probe(value: *mut u64) -> u64 {
    unsafe {
        *value += 1;
    }
    let active = TRACE.0[5].load(Ordering::Acquire);
    if let Some((entered, resume)) = BLOCK.lock().unwrap().take() {
        entered.send(active).unwrap();
        resume.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    PROBED_ACTIVE.fetch_add(
        u64::from(TRACE.0[5].load(Ordering::Acquire) != 0),
        Ordering::Relaxed,
    );
    unsafe {
        SetLastError(0x55667788);
    }
    0x1122
}

fn extra_bodies() -> [(usize, Vec<u8>, &'static [u8]); 5] {
    [
        (
            0x35800,
            vec![0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83, 0xec, 0x20],
            &[
                0x48, 0x83, 0xc4, 0x20, 0x5f, 0x48, 0x8b, 0x5c, 0x24, 8, 0xc3,
            ],
        ),
        (
            0x383e0,
            vec![
                0x4c, 0x8b, 0xdc, 0x55, 0x57, 0x49, 0x8d, 0xab, 0x28, 0xff, 0xff, 0xff, 0x48, 0x81,
                0xec, 0xc8, 1, 0, 0,
            ],
            &[0x48, 0x81, 0xc4, 0xc8, 1, 0, 0, 0x5f, 0x5d, 0xc3],
        ),
        (
            0x39650,
            vec![0x40, 0x55, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8b, 0xe9],
            &[0x48, 0x83, 0xc4, 0x30, 0x5d, 0xc3],
        ),
        (
            0x413b70,
            vec![
                0x48, 0x8b, 0xc4, 0x48, 0x89, 0x48, 8, 0x55, 0x41, 0x56, 0x48, 0x8d, 0xa8, 0x88,
                0xfe, 0xff, 0xff, 0x48, 0x81, 0xec, 0x68, 2, 0, 0,
                // The original saved RCX home must survive the displaced shim.
                0x48, 0x39, 0x8c, 0x24, 0x80, 2, 0, 0, 0x74, 2, 0x0f, 0x0b,
            ],
            &[0x48, 0x81, 0xc4, 0x68, 2, 0, 0, 0x41, 0x5e, 0x5d, 0xc3],
        ),
        (
            0x41a400,
            vec![0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83, 0xec, 0x20],
            &[
                0x48, 0x83, 0xc4, 0x20, 0x5f, 0x48, 0x8b, 0x5c, 0x24, 8, 0xc3,
            ],
        ),
    ]
}

fn seed_six(base: usize) {
    let tails: [&[u8]; 6] = [
        &[0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3],
        &[0x48, 0x83, 0xc4, 0x50, 0x5e, 0xc3],
        &[0x5f, 0x5e, 0x5d, 0x48, 0x8b, 0x5c, 0x24, 0x18, 0xc3],
        &[0x48, 0x81, 0xc4, 0x90, 2, 0, 0, 0x5f, 0x5d, 0x5b, 0xc3],
        &[0x41, 0x5e, 0x5d, 0xc3],
        &[0x48, 0x81, 0xc4, 0xd8, 0, 0, 0, 0x5f, 0x5b, 0xc3],
    ];
    for (index, (rva, prefix, n)) in SITES.into_iter().take(6).enumerate() {
        let mut bytes = prefix[..n].to_vec();
        if index < 2 {
            bytes.extend([0x48, 0x8b, if index == 0 { 0xd9 } else { 0xf1 }]);
        }
        if index == 2 {
            bytes.extend([0x55, 0x56, 0x57]);
        }
        bytes.extend([0xb8, 0x22, 0x11, 0, 0]);
        bytes.extend(tails[index]);
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), (base + rva) as *mut u8, bytes.len());
        }
    }
}
pub(super) fn seed_extra(base: usize) {
    seed_extra_with_inner(base, false);
}
fn seed_extra_with_inner(base: usize, inner: bool) {
    for (rva, prefix, tail) in extra_bodies() {
        let mut code = prefix;
        if inner {
            let next = rva + code.len() + 5;
            code.push(0xe8);
            code.extend(((SITES[0].0 as i64 - next as i64) as i32).to_le_bytes());
        }
        code.extend([0x48, 0xb8]);
        code.extend((probe as *const () as u64).to_le_bytes());
        code.extend([0xff, 0xd0]);
        code.extend(tail);
        unsafe {
            ptr::copy_nonoverlapping(code.as_ptr(), (base + rva) as *mut u8, code.len());
        }
    }
}

#[test]
fn five_outer_callers_keep_the_original_once_and_hold_the_complete_native_body() {
    if !child("five_outer_callers_keep_the_original_once_and_hold_the_complete_native_body") {
        return;
    }
    let Some(_fixture) = crate::windows::atomic_fixture::require(
        "five_outer_callers_keep_the_original_once_and_hold_the_complete_native_body",
    ) else {
        return;
    };
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0x420000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    seed_six(base);
    let bodies = extra_bodies();
    seed_extra(base);
    let mut old = 0;
    assert_ne!(
        unsafe { VirtualProtect(base as _, 0x420000, PAGE_EXECUTE_READ, &mut old) },
        0
    );
    assert_ne!(
        unsafe { FlushInstructionCache(GetCurrentProcess(), base as _, 0x420000) },
        0
    );
    unsafe { fixture_install(base) }.unwrap();
    let mut observed = Vec::new();
    for (rva, _, _) in &bodies {
        let invoke: unsafe extern "system" fn(*mut u64) -> u64 =
            unsafe { std::mem::transmute(base + rva) };
        let mut value = 0;
        let result = unsafe { invoke(&mut value) };
        observed.push((result, value, unsafe { GetLastError() }));
    }
    let counts = counts();
    let active = PROBED_ACTIVE.load(Ordering::Acquire);
    unsafe { restore() }.unwrap();
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    assert_eq!(observed, vec![(0x1122, 1, 0x55667788); 5]);
    assert_eq!(
        active, 5,
        "the five original callers lacked their full native frame"
    );
    assert_eq!(counts, [5, 5, 0, 0, 0]);
}

#[test]
fn five_displaced_outer_shims_unwind_at_each_prologue_boundary_and_before_tail_branch() {
    type Phase = (usize, usize, [Option<usize>; 3]); // PC offset, stack size, RBX/RBP/RDI slots
    let home: &[Phase] = &[(0, 0, [None, None, None]), (5, 0, [Some(8), None, None])];
    let pair: &[Phase] = &[
        (0, 0, [None, None, None]),
        (3, 0, [None, None, None]),
        (4, 8, [None, Some(0), None]),
        (5, 16, [None, Some(8), Some(0)]),
    ];
    let stack: &[Phase] = &[
        (0, 0, [None, None, None]),
        (2, 8, [None, Some(0), None]),
        (6, 56, [None, Some(48), None]),
    ];
    let owner: &[Phase] = &[
        (0, 0, [None, None, None]),
        (3, 0, [None, None, None]),
        (7, 0, [None, None, None]),
        (8, 8, [None, Some(0), None]),
    ];
    for (address, phases) in [
        (native_frame_35800_original as *const () as usize, home),
        (native_frame_383e0_original as *const () as usize, pair),
        (native_frame_39650_original as *const () as usize, stack),
        (native_frame_413b70_original as *const () as usize, owner),
        (native_frame_41a400_original as *const () as usize, home),
    ] {
        let mut base = 0;
        let entry = unsafe { RtlLookupFunctionEntry(address as u64, &mut base, null_mut()) };
        assert!(!entry.is_null());
        let tail = unsafe { base as usize + (*entry).EndAddress as usize - address - 4 };
        let mut checks = phases.to_vec();
        let (_, allocation, registers) = *phases.last().unwrap();
        checks.push((tail, allocation, registers));
        for (pc, allocation, registers) in checks {
            let mut stack = [0u64; 128];
            stack[allocation / 8] = 0x11223344;
            let mut ctx: CONTEXT = unsafe { std::mem::zeroed() };
            ctx.Rip = (address + pc) as u64;
            ctx.Rsp = stack.as_mut_ptr() as u64;
            ctx.Rbx = 0x55667700;
            ctx.Rbp = 0x55667701;
            ctx.Rdi = 0x55667702;
            for (i, offset) in registers.into_iter().enumerate() {
                if let Some(offset) = offset {
                    stack[offset / 8] = 0x55667700 + i as u64;
                    match i {
                        0 => ctx.Rbx = 0,
                        1 => ctx.Rbp = 0,
                        2 => ctx.Rdi = 0,
                        _ => unreachable!(),
                    }
                }
            }
            let mut data = null_mut();
            let mut frame = 0;
            unsafe {
                RtlVirtualUnwind(
                    0,
                    base,
                    ctx.Rip,
                    entry,
                    &mut ctx,
                    &mut data,
                    &mut frame,
                    null_mut(),
                );
            }
            assert_eq!(ctx.Rip, 0x11223344, "outer shim {address:x}+{pc:x}");
            assert_eq!(ctx.Rsp, stack.as_ptr() as u64 + allocation as u64 + 8);
            assert_eq!(
                [ctx.Rbx, ctx.Rbp, ctx.Rdi],
                [0x55667700, 0x55667701, 0x55667702]
            );
        }
    }
}

global_asm!(
    ".globl outer_unwind_driver",
    ".seh_proc outer_unwind_driver",
    "outer_unwind_driver:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x80",
    ".seh_stackalloc 0x80",
    ".seh_endprologue",
    "mov rbx,rcx",
    "mov [rcx],rsp",
    "lea rax,[rip+outer_unwind_landed]",
    "mov [rcx+8],rax",
    "call rdx",
    "xor eax,eax",
    "jmp 2f",
    "outer_unwind_landed:",
    "mov eax,1",
    "2:",
    "add rsp,0x80",
    "pop rbx",
    "ret",
    ".seh_endproc",
);
macro_rules! unwind_original {
    ($name:literal,$($prologue:literal),+) => {
        global_asm!(concat!(".globl ",$name,"\n.seh_proc ",$name,"\n",$name,":\n",
            $($prologue,"\n",)+
            "sub rsp,0x520\n.seh_stackalloc 0x520\n.seh_endprologue\n",
            "mov rdx,[rcx+8]\nmov rcx,[rcx]\nlea r10,[rsp+0x40]\nmov r11d,0x9a\n",
            "1:\nmov qword ptr [r10],0\nlea r10,[r10+8]\ndec r11d\njnz 1b\n",
            "lea r10,[rsp+0x40]\nmov [rsp+0x20],r10\nmov qword ptr [rsp+0x28],0\n",
            "xor r8d,r8d\nxor r9d,r9d\ncall {unwind_api}\nint3\n.seh_endproc\n"),
            unwind_api=sym RtlUnwindEx);
    };
}
unwind_original!(
    "outer_35800_unwind_original",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    "push rdi",
    ".seh_pushreg rdi",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20"
);
unwind_original!(
    "outer_383e0_unwind_original",
    "mov r11,rsp",
    "push rbp",
    ".seh_pushreg rbp",
    "push rdi",
    ".seh_pushreg rdi",
    "lea rbp,[r11-0xd8]",
    "sub rsp,0x1c8",
    ".seh_stackalloc 0x1c8"
);
unwind_original!(
    "outer_39650_unwind_original",
    ".byte 0x40,0x55",
    ".seh_pushreg rbp",
    "sub rsp,0x30",
    ".seh_stackalloc 0x30",
    "mov rbp,rcx"
);
unwind_original!(
    "outer_413b70_unwind_original",
    "mov rax,rsp",
    "mov [rax+8],rcx",
    "push rbp",
    ".seh_pushreg rbp",
    "push r14",
    ".seh_pushreg r14",
    "lea rbp,[rax-0x178]",
    "sub rsp,0x268",
    ".seh_stackalloc 0x268"
);
unwind_original!(
    "outer_41a400_unwind_original",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    "push rdi",
    ".seh_pushreg rdi",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20"
);
unsafe extern "system" {
    fn outer_unwind_driver(state: *mut u64, target: usize) -> u64;
    fn outer_35800_unwind_original();
    fn outer_383e0_unwind_original();
    fn outer_39650_unwind_original();
    fn outer_413b70_unwind_original();
    fn outer_41a400_unwind_original();
}
fn actual_seh(name: &str, index: usize, original: usize, displaced: usize) {
    if !child(name) {
        return;
    }
    let target = fixture(index, original + displaced);
    let mut state = [0u64; 2];
    assert_eq!(
        unsafe { outer_unwind_driver(state.as_mut_ptr(), target) },
        1
    );
    assert_eq!(counts(), [1, 0, 1, 0, 0]);
}
#[test]
fn outer_35800_actual_seh_settles_without_native_return() {
    actual_seh(
        "outer_35800_actual_seh_settles_without_native_return",
        6,
        outer_35800_unwind_original as *const () as usize,
        5,
    );
}
#[test]
fn outer_383e0_actual_seh_settles_without_native_return() {
    actual_seh(
        "outer_383e0_actual_seh_settles_without_native_return",
        7,
        outer_383e0_unwind_original as *const () as usize,
        5,
    );
}
#[test]
fn outer_39650_actual_seh_settles_without_native_return() {
    actual_seh(
        "outer_39650_actual_seh_settles_without_native_return",
        8,
        outer_39650_unwind_original as *const () as usize,
        6,
    );
}
#[test]
fn outer_413b70_actual_seh_settles_without_native_return() {
    actual_seh(
        "outer_413b70_actual_seh_settles_without_native_return",
        9,
        outer_413b70_unwind_original as *const () as usize,
        8,
    );
}
#[test]
fn outer_41a400_actual_seh_settles_without_native_return() {
    actual_seh(
        "outer_41a400_actual_seh_settles_without_native_return",
        10,
        outer_41a400_unwind_original as *const () as usize,
        5,
    );
}

fn blocking_outer(name: &'static str, rva: usize) {
    if !child(name) {
        return;
    }
    let Some(_fixture) = crate::windows::atomic_fixture::require(name) else {
        return;
    };
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0x420000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    seed_six(base);
    seed_extra_with_inner(base, true);
    let mut old = 0;
    assert_ne!(
        unsafe { VirtualProtect(base as _, 0x420000, PAGE_EXECUTE_READ, &mut old) },
        0
    );
    assert_ne!(
        unsafe { FlushInstructionCache(GetCurrentProcess(), base as _, 0x420000) },
        0
    );
    unsafe { fixture_install(base) }.unwrap();
    let (entered, seen) = std::sync::mpsc::channel();
    let (release, resume) = std::sync::mpsc::channel();
    *BLOCK.lock().unwrap() = Some((entered, resume));
    let worker = std::thread::spawn(move || {
        let call: unsafe extern "system" fn(*mut u64) -> u64 =
            unsafe { std::mem::transmute(base + rva) };
        let mut value = 0;
        let result = unsafe { call(&mut value) };
        (result, value)
    });
    let active = seen.recv_timeout(Duration::from_secs(5)).unwrap();
    let refused = close(Instant::now() + Duration::from_millis(25));
    release.send(()).unwrap();
    let result = worker.join().unwrap();
    let settled = close(Instant::now() + Duration::from_secs(2));
    let counts = counts();
    unsafe { restore() }.unwrap();
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    assert_eq!(
        active, 1,
        "the inner frame returned while its outer frame was still live"
    );
    assert!(!refused, "inner completion must not drain the outer body");
    assert!(settled);
    assert_eq!(result, (0x1122, 1));
    // The intentionally expired drain remains a failure in the trace even
    // after its reserved originals finally return; do not erase that event.
    assert_eq!(counts, [2, 2, 0, 0, 1]);
}
macro_rules! blocking_test {
    ($name:ident,$rva:literal) => {
        #[test]
        fn $name() {
            blocking_outer(stringify!($name), $rva);
        }
    };
}
blocking_test!(outer_35800_remains_pending_after_inner_return, 0x35800);
blocking_test!(outer_383e0_remains_pending_after_inner_return, 0x383e0);
blocking_test!(outer_39650_remains_pending_after_inner_return, 0x39650);
blocking_test!(outer_413b70_remains_pending_after_inner_return, 0x413b70);
blocking_test!(outer_41a400_remains_pending_after_inner_return, 0x41a400);

#[test]
#[ignore = "explicit licensed local primary outer-frame verification"]
fn primary_five_complete_outer_bodies_unwind_mutations_and_eleven_owned_entries() {
    if !child("primary_five_complete_outer_bodies_unwind_mutations_and_eleven_owned_entries") {
        return;
    }
    let Some(_fixture) = crate::windows::atomic_fixture::require(
        "primary_five_complete_outer_bodies_unwind_mutations_and_eleven_owned_entries",
    ) else {
        return;
    };
    let image = super::primary_image::Image::new(GAMECORE, civ6_bootstrap_rs::CORE_SHA);
    for frame in outer_fixed::FRAMES {
        let address = frame.begin as usize + 16;
        let original = copy_current::<1>((image.base + address) as u64).unwrap()[0];
        image.replace_byte(address, original ^ 1);
        assert_eq!(
            unsafe { install(image.base) },
            Err("scheduler whole code differs")
        );
        assert!(FRAMES.get().is_none() && HOOKS.lock().unwrap().is_empty());
        image.replace_byte(address, original);
        for part in frame.parts {
            let address = part.unwind as usize + 1;
            let original = copy_current::<1>((image.base + address) as u64).unwrap()[0];
            image.replace_byte(address, original ^ 1);
            assert_eq!(
                unsafe { install(image.base) },
                Err("scheduler unwind differs")
            );
            assert!(FRAMES.get().is_none() && HOOKS.lock().unwrap().is_empty());
            image.replace_byte(address, original);
        }
    }
    let host = super::primary_image::Image::new(GAME_EXE, civ6_bootstrap_rs::EXE_SHA);
    unsafe { prepare_host(host.base) }.unwrap();
    let installed = unsafe { install(image.base) };
    if let Err(error) = installed {
        let restored = unsafe { restore() };
        panic!("production native install: {error}; restore={restored:?}");
    }
    assert!(bind_host(image.base));
    assert_eq!(TRACE.0[7].load(Ordering::Acquire), (1 << SITES.len()) - 1);
    assert!(owns_hook_entries(&HOOKS, SITES.len()));
    let every_domain_owned = all_hook_entries_owned();
    assert!(
        every_domain_owned,
        "production install omitted a native ownership domain"
    );
    assert!(
        !control::ready(),
        "installation alone cannot prove a World shutdown"
    );
    assert!(
        !reopen(),
        "G1 ownership alone cannot authorize a product rebuild"
    );
    assert!(
        !resume_unqualified_observation(),
        "an unstarted World has no retirement receipt"
    );
    assert!(runtime::fixture_product_inactive());
    assert_eq!(TRACE.0[12].load(Ordering::Acquire), 0);
    // Each negative uses one retained, patched entry from a different table.
    // This proves ownership collection only, not World/producer quiescence.
    for (name, is_core, rva) in [
        ("frames", true, 0x35f70usize),
        ("jobs", false, 0x986480),
        ("consumers", false, 0xe417d0),
        ("worlds", true, 0x58fb40),
        ("control", true, 0x63e160),
        ("operations", true, 0x373e0),
        ("sessions", false, 0x7f8cd0),
        ("loads", false, 0x273f40),
        ("wire", false, 0x80f230),
        ("scheduler", false, 0x9858b0),
        ("closure", true, 0x49c7c0),
        ("registry", false, 0x7f9220),
        ("transport", false, 0x81b330),
        ("input", false, 0xc66350),
        ("deferred", false, 0x847510),
    ] {
        let owned = if is_core { &image } else { &host };
        let original = copy_current::<1>((owned.base + rva) as u64).unwrap()[0];
        owned.replace_byte(rva, original ^ 1);
        let rejected = !all_hook_entries_owned();
        owned.replace_byte(rva, original);
        assert!(rejected, "changed {name} entry was accepted");
        assert!(
            all_hook_entries_owned(),
            "restored owned {name} entry was not recognized"
        );
    }
    control::fixture_observation_resume_guards(resume_unqualified_observation, || {
        runtime::BridgeRuntimeRebuild(null_mut()) == BridgeStatus::BoundaryUnproven as u32
    });
    assert!(
        !control::ready(),
        "observer resumption never certifies full coverage"
    );
    // Only metadata and entry publication run; original game bodies do not.
    assert_eq!(counts(), [0; 5]);
    unsafe { restore() }.unwrap();
    assert!(
        !all_hook_entries_owned(),
        "a restored installation no longer owns its entries"
    );
    unsafe { scheduler::verify_frames(image.base, outer_fixed::FRAMES) }.unwrap();
    println!("PRIMARY_OUTER_COMPLETE_BODIES=5 UNWIND_PARTITIONS=11 OWNED_FRAME_ENTRIES=11 OWNED_DOMAINS=15 OWNERSHIP_KNOCKOUTS=15 OBSERVER_REBUILD_STATUS=7; controlled lifetime facts, native game bodies not executed");
}
