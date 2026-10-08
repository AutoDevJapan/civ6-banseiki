use super::*;
use crate::paid_catalog::{self, BITS, COMMIT_KEY, COUNT, WIDTH};
use std::os::windows::process::CommandExt;
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, CONTEXT, EXCEPTION_RECORD, IMAGE_RUNTIME_FUNCTION_ENTRY,
};

pub(super) static END_PAUSE: AtomicBool = AtomicBool::new(false);
pub(super) static END_PAUSED: AtomicBool = AtomicBool::new(false);
pub(super) static END_RELEASE: AtomicBool = AtomicBool::new(false);
const ORIGINAL: u64 = 0xaabb_ccdd_1122_3301;

#[test]
fn unpublished_relay_is_freed_but_transferred_storage_is_retained() {
    if !child("unpublished_relay_is_freed_but_transferred_storage_is_retained") {
        return;
    }
    let allocate = || {
        let page = unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        }
        .cast::<u8>();
        assert!(!page.is_null());
        page
    };
    let state = |page: *mut u8| {
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe { VirtualQuery(page.cast(), &mut info, std::mem::size_of_val(&info)) },
            0
        );
        info.State
    };
    let page = allocate();
    drop(UnpublishedRelay(page));
    assert_eq!(state(page), windows_sys::Win32::System::Memory::MEM_FREE);
    let page = allocate();
    let mut unpublished = UnpublishedRelay(page);
    unpublished.0 = null_mut(); // Same ownership boundary as Slot publication.
    drop(unpublished);
    assert_eq!(
        state(page),
        MEM_COMMIT,
        "a potentially reached relay must survive"
    );
    // This fixture has published no call operand and is its sole page owner.
    assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
}

#[test]
fn promotion_primary_image_matches_both_callers_arguments_and_untouched_callee() {
    if !Path::new(GAMECORE).is_file() {
        println!("fixed primary GameCore NOT AVAILABLE; witness check NOT EXECUTED");
        return;
    }
    // A loaded-image witness fixture, copied from the actual pinned disk image.
    // No GameCore code is loaded or run. verify checks the real complete SHA.
    let mut file = File::open(GAMECORE).unwrap();
    let mut image = vec![0u8; AI_CALL_RVA + 16];
    for (rva, len) in [
        (
            PINNED_PROMOTION_CALL_RVA - OFFICIAL_ARGS.len(),
            OFFICIAL_ARGS.len() + 16,
        ),
        (AI_CALL_RVA - AI_ARGS.len(), AI_ARGS.len() + 16),
        (PINNED_PROMOTION_CALLEE_RVA - 5, CALLEE_BOUNDARY.len()),
    ] {
        let offset = pe_file_offset(&mut file, rva as u32).unwrap();
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.read_exact(&mut image[rva..rva + len]).unwrap();
    }
    let base = image.as_ptr() as usize;
    assert!(verify(base));
    for offset in [
        AI_CALL_RVA + 1,
        AI_CALL_RVA + 5,
        AI_CALL_RVA - AI_ARGS.len(),
        PINNED_PROMOTION_CALL_RVA - 3,
        PINNED_PROMOTION_CALLEE_RVA,
    ] {
        image[offset] ^= 1;
        assert!(!verify(base), "loaded witness changed at {offset:#x}");
        image[offset] ^= 1;
    }
    println!("fixed SHA, both PE call mappings/argument witnesses and callee boundary verified");
}

struct OwnedChild(std::process::Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

// Each test runs itself in an owned Win64 child. No inventory-only helper is
// counted as an executed gateway fixture. Child output retains its exact count.
fn child(name: &str) -> bool {
    if !supports_atomic_promotion_fetch() {
        // CI on another CPU verifies refusal before mutation. It does not
        // count as an executed AMD/Win64 gateway fixture in the evidence.
        assert_eq!(
            unsafe { install_sites([(null_mut(), [0; 16]); 2], null_mut()) },
            Err("runtime promotion requires AMD atomic operand fetch")
        );
        assert!(!used());
        println!("AMD gateway child NOT EXECUTED; unsupported-CPU refusal verified");
        return false;
    }
    if std::env::var("JAC_RUNTIME_PROMOTION_CHILD").as_deref() == Ok(name) {
        return true;
    }
    let name = format!("windows::promotion::tests::{name}");
    let mut child = OwnedChild(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(
                "JAC_RUNTIME_PROMOTION_CHILD",
                name.rsplit("::").next().unwrap(),
            )
            .creation_flags(0x08000000)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success(), "promotion child failed: {status}");
            return false;
        }
        assert!(
            Instant::now() < deadline,
            "promotion child exceeded deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

unsafe fn page() -> *mut u8 {
    let p = unsafe {
        VirtualAlloc(
            null_mut(),
            CODE_PAGE,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
    }
    .cast::<u8>();
    assert!(!p.is_null());
    p
}

type Call = unsafe extern "system" fn(u64, u64, u32, u64) -> u64;

// Real aligned E8 sites, with a caller-owned home area and intact post-call
// instructions. The private installer accepts synthetic expected bytes only
// in these tests; the runtime installer always verifies the fixed primary SHA.
unsafe fn sites(p: *mut u8, target: *mut u8) -> ([(*mut u8, [u8; 16]); 2], [Call; 2]) {
    let mut result = [(null_mut(), [0; 16]); 2];
    let mut calls = [0usize; 2];
    for (i, offset) in [0x207, 0x287].into_iter().enumerate() {
        let call = unsafe { p.add(offset) };
        let entry = unsafe { call.sub(7) };
        let pre = [0x48, 0x83, 0xec, 0x28, 0x90, 0x90, 0x90];
        let mut expected = [0x90; 16];
        expected[0] = 0xe8;
        expected[1..5].copy_from_slice(
            &promotion_operand(call as usize, target as usize)
                .unwrap()
                .to_le_bytes(),
        );
        expected[5..11].copy_from_slice(&[0x48, 0x8d, 0x64, 0x24, 0x28, 0xc3]);
        unsafe {
            ptr::copy_nonoverlapping(pre.as_ptr(), entry, pre.len());
            ptr::copy_nonoverlapping(expected.as_ptr(), call, expected.len());
        }
        result[i] = (call, expected);
        calls[i] = entry as usize;
    }
    // These synthetic callers allocate 0x28 bytes. Give the OS their actual
    // unwind metadata, just as the pinned GameCore callers have .pdata.
    let mut table = Box::new(unsafe { std::mem::zeroed::<[IMAGE_RUNTIME_FUNCTION_ENTRY; 2]>() });
    for (i, entry) in table.iter_mut().enumerate() {
        entry.BeginAddress = (calls[i] - p as usize) as u32;
        entry.EndAddress = entry.BeginAddress + 0x17;
        entry.Anonymous.UnwindData = (0x380 + i * 16) as u32;
        unsafe {
            ptr::copy_nonoverlapping(
                [1u8, 4, 1, 0, 4, 0x42, 0, 0].as_ptr(),
                p.add(0x380 + i * 16),
                8,
            )
        };
    }
    assert_ne!(
        unsafe { RtlAddFunctionTable(table.as_mut_ptr(), 2, p as u64) },
        0
    );
    let _ = Box::into_raw(table); // retained with these process-owned callers
    (result, unsafe {
        calls.map(|p| std::mem::transmute::<usize, Call>(p))
    })
}

struct Fixture {
    _page: *mut u8,
    get: unsafe extern "system" fn(u64, u32) -> u64,
    raw: Box<[u64; 3]>,
    player: Box<[u64; 192]>,
    governors: Box<[u64; 32]>,
    governor: Box<[u64; 16]>,
    flags: Box<[u8; 128]>,
    _node: Box<[u64; 3]>,
    words: [u64; COUNT],
    trace: *mut CompositeTrace,
}

impl Fixture {
    fn new() -> Self {
        let p = unsafe { page() };
        let can = unsafe { p.add(16) };
        let property = unsafe { p.add(128) };
        let assign = unsafe { p.add(768) };
        let can_code = [0x48, 0x89, 0x5c, 0x24, 0x08, 0xb8, 1, 0, 0, 0, 0xc3];
        let property_code = [
            0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30, 0xff, 0x41, 0x48, 0xc3,
        ];
        let assign_code = [
            0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0x6c, 0x24, 0x10, 0x4c, 0x89, 0xc8, 0xc3,
        ];
        unsafe {
            ptr::copy_nonoverlapping(can_code.as_ptr(), can, can_code.len());
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), property.sub(5), 5);
            ptr::copy_nonoverlapping(property_code.as_ptr(), property, property_code.len());
            ptr::copy_nonoverlapping(assign_code.as_ptr(), assign, assign_code.len());
        }
        let trace = runtime::allocate_trace().unwrap();
        unsafe {
            composite::install(
                can,
                property,
                assign,
                trace,
                can_code[..8].try_into().unwrap(),
                PROPERTY_PROLOGUE,
                assign_code[..8].try_into().unwrap(),
            )
        }
        .unwrap();
        runtime::start_observation(MAX_ACTIVE_SECONDS);
        let raw = Box::new([1u64 | (1u64 << 32), 0, 0]);
        let mut player = Box::new([0u64; 192]);
        player[0xd8 / 8] = 2;
        player[(0x528 + 0x40) / 8] = raw.as_ptr() as u64;
        let flags = Box::new([0u8; 128]);
        let mut governor = Box::new([0u64; 16]);
        governor[0] = 0x10001 | (77u64 << 32);
        governor[0x18 / 8] = 2;
        governor[0x68 / 8] = flags.as_ptr() as u64;
        governor[0x70 / 8] = flags.len() as u64;
        governor[0x78 / 8] = flags.len() as u64;
        let node = Box::new([governor.as_ptr() as u64, 0, 0]);
        let mut governors = Box::new([0u64; 32]);
        governors[0x58 / 8] = node.as_ptr() as u64;
        governors[0x70 / 8] = player.as_ptr() as u64;
        let mut words = std::array::from_fn(|i| i as u64 | 256 | ((100 + i as u64) << 10));
        words[0] = 77 | (3 << 8) | (1 << 10) | (3 << 22) | (7 << 34);
        words[1] = 78 | (2 << 8) | (2 << 10) | (4 << 22);
        words[2] = 79 | (1 << 8) | (5 << 10);
        Self {
            _page: p,
            get: unsafe {
                std::mem::transmute::<*mut u8, unsafe extern "system" fn(u64, u32) -> u64>(property)
            },
            raw,
            player,
            governors,
            governor,
            flags,
            _node: node,
            words,
            trace,
        }
    }
    fn receiver(&self) -> u64 {
        self.player.as_ptr() as u64 + 0x528
    }
    fn this(&self) -> u64 {
        self.governors.as_ptr() as u64
    }
    fn person(&self) -> u64 {
        self.governor.as_ptr() as u64
    }
    fn publish(&mut self, key: u32, value: bool) {
        self.raw[1] = u64::from(value);
        assert_eq!(
            unsafe { (self.get)(self.receiver(), key) },
            self.raw.as_ptr() as u64
        );
    }
    fn catalog(&mut self) {
        self.publish(PAID_RESET_BASE + 2, true);
        self.publish(COMMIT_KEY, false);
        for bit in 0..BITS {
            self.publish(
                paid_catalog::bit_key(bit),
                self.words[bit / WIDTH] & (1 << (bit % WIDTH)) != 0,
            );
        }
        self.publish(COMMIT_KEY, true);
        assert_eq!(runtime::phase(2), 2);
    }
    fn admit(&mut self, index: u32, allow: bool) {
        self.publish(PRODUCTION_CLOSE_BASE + index, true);
        self.publish(PRODUCTION_ALLOW_BASE + index, false);
        self.publish(PRODUCTION_MANIFEST_BASE + index, true);
        if allow {
            self.publish(PRODUCTION_ALLOW_BASE + index, true);
        }
    }
    fn check(&self, calls: [Call; 2], requested: u32, expected: bool) {
        for call in calls {
            let before = self.governors[0x78 / 8];
            assert_eq!(
                unsafe { call(self.this(), self.person(), requested, ORIGINAL) },
                if expected { ORIGINAL } else { ORIGINAL & !255 }
            );
            assert_eq!(
                self.governors[0x78 / 8],
                before + 1,
                "original called exactly once"
            );
            assert_eq!(
                unsafe { call(self.this(), self.person(), requested, ORIGINAL & !255) },
                ORIGINAL & !255,
                "original false never becomes true"
            );
        }
    }
    fn select(&mut self, index: u32, acquired: &[usize]) {
        self.governor[0] = 0x10001 | (u64::from(index) << 32);
        self.flags.fill(0);
        for i in acquired {
            self.flags[*i] = 1;
        }
    }
}

#[test]
fn promotion_both_callers_use_partial_payment_admission_and_load_fences() {
    if !child("promotion_both_callers_use_partial_payment_admission_and_load_fences") {
        return;
    }
    assert!(
        supports_atomic_promotion_fetch(),
        "fixture requires the product AMD condition"
    );
    let mut f = Fixture::new();
    let target = unsafe { f._page.add(1024) };
    // Diagnostic original increments a count and returns the supplied sentinel.
    let code = [0xff, 0x41, 0x78, 0x4c, 0x89, 0xc8, 0xc3];
    unsafe { ptr::copy_nonoverlapping(code.as_ptr(), target, code.len()) };
    let (s, calls) = unsafe { sites(f._page, target) };
    unsafe { install_sites(s, target) }.unwrap();
    f.select(77, &[1]);
    f.check(calls, 3, false); // uninitialized catalog is never Standard
    f.catalog();
    f.check(calls, 3, false); // joined/secured proof absent
    f.admit(78, true);
    f.check(calls, 3, false); // other person cannot authorize this person
    f.admit(77, true);
    f.publish(PAID_CLOSE_BASE + 77, true);
    f.publish(PAID_ALLOW_BASE + 77, false);
    f.publish(PAID_MANIFEST_BASE + 77, true);
    f.check(calls, 3, true); // paid=false still permits next TITLE
    for requested in [1, 7, 8, 127, u32::MAX] {
        f.check(calls, requested, false);
    }
    f.select(77, &[1, 7]);
    f.check(calls, 3, false); // out-of-order official flags
    f.select(77, &[1, 3]);
    f.check(calls, 3, false); // no repeat payment
    f.check(calls, 7, true);
    f.select(77, &[1, 3, 7]);
    for requested in [3, 7, 8] {
        f.check(calls, requested, false);
    }
    f.select(78, &[2]);
    f.check(calls, 4, true); // saved cost2
    f.check(calls, 7, false);
    f.select(78, &[2, 4]);
    f.check(calls, 4, false);
    f.select(79, &[5]);
    f.admit(79, true);
    for requested in [5, 3, 7, 8] {
        f.check(calls, requested, false);
    } // cost1 has no unpaid TITLE
    f.select(200, &[]);
    f.governor[0x68 / 8] = 0; // ordinary vectors need not match persona layout
    f.check(calls, 8, true);
    f.governor[0x68 / 8] = f.flags.as_ptr() as u64;
    f.select(77, &[1]);
    f.governor[0x18 / 8] = 3;
    f.check(calls, 3, false);
    f.governor[0x18 / 8] = 2;
    f.flags[1] = 2;
    f.check(calls, 3, false);
    f.flags[1] = 1;
    // Authority remains after observation stops, and never touches its frozen
    // allocation. No new promotion rows/counters are hidden in the trace.
    assert_eq!(
        runtime::BridgeRuntimeStopObservation(null_mut()),
        BridgeStatus::Ok as u32
    );
    let frozen = unsafe {
        std::slice::from_raw_parts(f.trace.cast::<u8>(), std::mem::size_of::<CompositeTrace>())
    }
    .to_vec();
    f.check(calls, 3, true);
    assert_eq!(
        unsafe { std::slice::from_raw_parts(f.trace.cast::<u8>(), frozen.len()) },
        frozen
    );
    // Same-owner close/regrant during the original call cannot lend newer
    // per-person tickets to that call, even without changing the catalog.
    for reset_catalog in [false, true] {
        END_PAUSED.store(false, Ordering::Release);
        END_RELEASE.store(false, Ordering::Release);
        END_PAUSE.store(true, Ordering::Release);
        let (a, b) = (f.this(), f.person());
        let call = calls[usize::from(reset_catalog)];
        let worker = std::thread::spawn(move || unsafe { call(a, b, 3, ORIGINAL) });
        let deadline = Instant::now() + Duration::from_secs(3);
        while !END_PAUSED.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        let paused = END_PAUSED.load(Ordering::Acquire);
        if paused {
            if reset_catalog {
                f.catalog();
            }
            f.admit(77, true);
        }
        END_RELEASE.store(true, Ordering::Release);
        assert_eq!(worker.join().unwrap(), ORIGINAL & !255);
        assert!(paused);
        f.check(calls, 3, true);
    }
    assert_eq!(
        runtime::BridgeRuntimeClose(null_mut()),
        BridgeStatus::Ok as u32
    );
    f.select(200, &[]);
    f.check(calls, 8, false); // unknown after close must not become Standard
    assert_eq!(
        runtime::BridgeRuntimeRebuild(null_mut()),
        BridgeStatus::Ok as u32
    );
    f.check(calls, 8, false);
    f.catalog();
    f.select(77, &[1, 3]);
    f.admit(77, true);
    f.check(calls, 3, false); // saved prefix survives rebuild: no resend
    f.check(calls, 7, true);
    let before = s.map(|(p, _)| copy_current::<16>(p as u64).unwrap());
    assert!(unsafe { install_sites(s, target) }.is_err());
    assert_eq!(
        s.map(|(p, _)| copy_current::<16>(p as u64).unwrap()),
        before
    );
    assert_eq!(
        BridgeUninstall(null_mut()),
        BridgeStatus::InvalidInput as u32
    );
    unsafe { uninstall() }.unwrap();
    assert_eq!(
        s.map(|(p, _)| copy_current::<16>(p as u64).unwrap()),
        s.map(|(_, w)| w)
    );
}

#[test]
fn promotion_partial_install_restores_first_operand() {
    if !child("promotion_partial_install_restores_first_operand") {
        return;
    }
    let p = unsafe { page() };
    let target = unsafe { p.add(1024) };
    unsafe { target.write(0xc3) };
    let (mut s, _) = unsafe { sites(p, target) };
    let originals = s.map(|(_, w)| w);
    s[1].1[15] ^= 1;
    assert!(unsafe { install_sites(s, target) }.is_err());
    for (i, (p, _)) in s.into_iter().enumerate() {
        assert_eq!(copy_current::<16>(p as u64), Some(originals[i]));
    }
    assert!(SLOTS.lock().unwrap().is_empty());
    assert!(used());
    assert_eq!(
        RUNTIME_PROMOTION_TARGET.load(Ordering::Acquire),
        target,
        "target remains immutable for fetched relays"
    );
}

#[test]
fn promotion_restore_failure_keeps_failed_owner_and_restores_other_operand() {
    if !child("promotion_restore_failure_keeps_failed_owner_and_restores_other_operand") {
        return;
    }
    let p = unsafe { page() };
    let target = unsafe { p.add(1024) };
    unsafe { target.write(0xc3) };
    let (s, _) = unsafe { sites(p, target) };
    unsafe { install_sites(s, target) }.unwrap();
    let patched = copy_current::<16>(s[1].0 as u64).unwrap();
    let operand = u32::from_le_bytes(patched[1..5].try_into().unwrap());
    unsafe {
        s[1].0
            .add(1)
            .cast::<AtomicU32>()
            .as_ref()
            .unwrap()
            .store(operand ^ 1, Ordering::SeqCst)
    };
    assert!(unsafe { restore() }.is_err());
    assert_eq!(SLOTS.lock().unwrap().len(), 1);
    assert_eq!(copy_current::<16>(s[0].0 as u64), Some(s[0].1));
    assert_ne!(copy_current::<16>(s[1].0 as u64), Some(s[1].1));
    unsafe {
        s[1].0
            .add(1)
            .cast::<AtomicU32>()
            .as_ref()
            .unwrap()
            .store(operand, Ordering::SeqCst)
    };
    assert_eq!(unsafe { restore() }, Ok(true));
    assert_eq!(copy_current::<16>(s[1].0 as u64), Some(s[1].1));
    assert_eq!(unsafe { restore() }, Ok(false));
    assert!(used());
}

#[unsafe(no_mangle)]
static PROMOTION_TEST_ENTRY: AtomicU64 = AtomicU64::new(0);
global_asm!(
    ".globl promotion_abi_original",
    "promotion_abi_original:",
    "inc qword ptr [r9]",
    "mov [r9+8],rax",
    "mov [r9+16],rcx",
    "mov [r9+24],rdx",
    "mov [r9+32],r8",
    "mov [r9+40],r10",
    "mov [r9+48],r11",
    "pushfq",
    "pop qword ptr [r9+56]",
    "mov rax,rsp",
    "and eax,15",
    "mov [r9+64],rax",
    "movdqu [r9+0x80],xmm0",
    "movdqu [r9+0x90],xmm1",
    "movdqu [r9+0xa0],xmm2",
    "movdqu [r9+0xb0],xmm3",
    "movdqu [r9+0xc0],xmm4",
    "movdqu [r9+0xd0],xmm5",
    "mov rcx,0x123456",
    "mov rdx,0x9876",
    "mov r8,0x1234",
    "mov r10,0xabc",
    "mov r11,0xdef",
    "pxor xmm0,xmm0",
    "pxor xmm1,xmm1",
    "pxor xmm2,xmm2",
    "pxor xmm3,xmm3",
    "pxor xmm4,xmm4",
    "pxor xmm5,xmm5",
    "mov rax,0xaabbccdd11223301",
    "stc",
    "ret",
    ".globl promotion_abi_driver",
    ".seh_proc promotion_abi_driver",
    "promotion_abi_driver:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x40",
    ".seh_stackalloc 0x40",
    ".seh_endprologue",
    "mov rbx,r9",
    "mov [rbx+0x1e0],rsp",
    "mov qword ptr [rsp+0x20],0x556677",
    "mov qword ptr [rsp+0x28],0x112233",
    "mov rax,0x11223344",
    "mov r10,0x22334455",
    "mov r11,0x33445566",
    "pcmpeqd xmm0,xmm0",
    "pcmpeqd xmm1,xmm1",
    "pcmpeqd xmm2,xmm2",
    "pcmpeqd xmm3,xmm3",
    "pcmpeqd xmm4,xmm4",
    "pcmpeqd xmm5,xmm5",
    "stc",
    "call qword ptr [rip+PROMOTION_TEST_ENTRY]",
    "mov [rbx+0x100],rax",
    "mov [rbx+0x108],rcx",
    "mov [rbx+0x110],rdx",
    "mov [rbx+0x118],r8",
    "mov [rbx+0x120],r9",
    "mov [rbx+0x128],r10",
    "mov [rbx+0x130],r11",
    "pushfq",
    "pop qword ptr [rbx+0x138]",
    "movdqu [rbx+0x180],xmm0",
    "movdqu [rbx+0x190],xmm1",
    "movdqu [rbx+0x1a0],xmm2",
    "movdqu [rbx+0x1b0],xmm3",
    "movdqu [rbx+0x1c0],xmm4",
    "movdqu [rbx+0x1d0],xmm5",
    "mov [rbx+0x1e8],rsp",
    "mov rax,[rsp+0x20]",
    "mov [rbx+0x1f0],rax",
    "mov rax,[rsp+0x28]",
    "mov [rbx+0x1f8],rax",
    "add rsp,0x40",
    "pop rbx",
    "ret",
    ".seh_endproc",
);
unsafe extern "system" {
    fn RtlAddFunctionTable(table: *mut IMAGE_RUNTIME_FUNCTION_ENTRY, count: u32, base: u64) -> u8;
    fn promotion_abi_original();
    fn promotion_abi_driver(a: u64, b: u64, c: u32, d: u64);
    fn runtime_promotion_return();
}

#[test]
fn promotion_gateway_preserves_original_registers_flags_xmm_and_caller_stack() {
    if !child("promotion_gateway_preserves_original_registers_flags_xmm_and_caller_stack") {
        return;
    }
    let mut f = Fixture::new();
    f.catalog();
    f.select(200, &[]);
    let target = promotion_abi_original as *const () as *mut u8;
    let p = unsafe { promotion_relay_near(target as usize | 3) }.unwrap();
    let (s, calls) = unsafe { sites(p, target) };
    let mut out = [0u64; 64];
    let capture = |entry: usize, out: &mut [u64; 64]| {
        out.fill(0);
        PROMOTION_TEST_ENTRY.store(entry as u64, Ordering::Release);
        unsafe { promotion_abi_driver(f.this(), f.person(), 8, out.as_mut_ptr() as u64) };
        assert_eq!(out[0], 1);
        assert_eq!(out[64 / 8], 8);
        assert_eq!(out[0x1e0 / 8], out[0x1e8 / 8]);
        assert_eq!(out[0x1f0 / 8], 0x556677);
        assert_eq!(out[0x1f8 / 8], 0x112233);
        *out
    };
    let before = capture(calls[0] as usize, &mut out);
    unsafe { install_sites(s, target) }.unwrap();
    for call in calls {
        let after = capture(call as usize, &mut out);
        assert_eq!(
            after, before,
            "original entry/return volatile registers, XMM, flags or stack differ"
        );
    }
    let mut base = 0;
    let entry = unsafe {
        RtlLookupFunctionEntry(
            runtime_promotion_return as *const () as u64,
            &mut base,
            null_mut(),
        )
    };
    assert!(!entry.is_null());
    let mut stack = [0u64; 80];
    stack[0x1c8 / 8] = 0x11223344;
    let mut context: CONTEXT = unsafe { std::mem::zeroed() };
    context.Rsp = stack.as_ptr() as u64;
    context.Rip = runtime_promotion_return as *const () as u64;
    let (mut data, mut frame) = (null_mut(), 0);
    unsafe {
        RtlVirtualUnwind(
            0,
            base,
            context.Rip,
            entry,
            &mut context,
            &mut data,
            &mut frame,
            null_mut(),
        )
    };
    assert_eq!(context.Rsp, stack.as_ptr() as u64 + 0x1d0);
    assert_eq!(context.Rip, 0x11223344);
    unsafe { uninstall() }.unwrap();
}

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
global_asm!(
    ".globl promotion_unwind_driver", ".seh_proc promotion_unwind_driver", "promotion_unwind_driver:",
    "push rbx", ".seh_pushreg rbx", "sub rsp,0x40", ".seh_stackalloc 0x40", ".seh_endprologue",
    "mov rbx,rcx", "mov [rcx],rsp", "lea rax,[rip+promotion_unwind_landed]", "mov [rcx+8],rax",
    "call rdx", "mov eax,0", "jmp 2f", "promotion_unwind_landed:", "mov eax,1",
    "2:", "add rsp,0x40", "pop rbx", "ret", ".seh_endproc",
    ".globl promotion_original_unwinds", ".seh_proc promotion_original_unwinds", "promotion_original_unwinds:",
    "sub rsp,0x518", ".seh_stackalloc 0x518", ".seh_endprologue",
    "mov rdx,[rcx+8]", "mov rcx,[rcx]", "lea r10,[rsp+0x40]", "mov r11d,0x9a",
    "1:", "mov qword ptr [r10],0", "add r10,8", "dec r11d", "jnz 1b",
    "lea r10,[rsp+0x40]", "mov [rsp+0x20],r10", "mov qword ptr [rsp+0x28],0",
    "xor r8d,r8d", "xor r9d,r9d", "call {unwind}", "add rsp,0x518", "ret", ".seh_endproc",
    unwind = sym RtlUnwindEx,
);
unsafe extern "system" {
    fn promotion_unwind_driver(state: *mut u64, target: usize) -> u64;
    fn promotion_original_unwinds();
}
#[test]
fn promotion_gateway_seh_unwinds_to_original_caller_without_retained_invocation() {
    if !child("promotion_gateway_seh_unwinds_to_original_caller_without_retained_invocation") {
        return;
    }
    let target = promotion_original_unwinds as *const () as *mut u8;
    let p = unsafe { promotion_relay_near(target as usize | 3) }.unwrap();
    let (s, calls) = unsafe { sites(p, target) };
    unsafe { install_sites(s, target) }.unwrap();
    let mut state = [0u64; 2];
    for call in calls {
        assert_eq!(
            unsafe { promotion_unwind_driver(state.as_mut_ptr(), call as usize) },
            1
        );
        assert_eq!(
            unsafe { promotion_unwind_driver(state.as_mut_ptr(), call as usize) },
            1,
            "unwound invocation cannot retain per-thread authority"
        );
    }
    unsafe { restore() }.unwrap();
}

#[test]
fn promotion_atomic_operands_preserve_concurrent_calls_and_delayed_relay_lifetime() {
    if !child("promotion_atomic_operands_preserve_concurrent_calls_and_delayed_relay_lifetime") {
        return;
    }
    let p = unsafe { page() };
    let target = unsafe { p.add(1024) };
    let code = [0xf0, 0x48, 0xff, 0x01, 0x4c, 0x89, 0xc8, 0xc3]; // lock inc [rcx]; mov rax,r9
    unsafe { ptr::copy_nonoverlapping(code.as_ptr(), target, code.len()) };
    let (s, calls) = unsafe { sites(p, target) };
    let running = std::sync::Arc::new(AtomicBool::new(true));
    let mut workers = Vec::new();
    let (tx, rx) = std::sync::mpsc::channel();
    for i in 0..4 {
        let running = running.clone();
        let tx = tx.clone();
        let call = calls[i % 2];
        workers.push(std::thread::spawn(move || {
            let mut counter = [0u64; 32];
            let mut count = 0;
            tx.send(()).unwrap();
            while running.load(Ordering::Acquire) {
                let result = unsafe { call(counter.as_mut_ptr() as u64, 0, 0, ORIGINAL) };
                assert!(result == ORIGINAL || result == ORIGINAL & !255);
                count += 1;
            }
            assert_eq!(counter[0], count, "no lost or duplicate original call");
            count
        }));
    }
    for _ in 0..4 {
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
    }
    unsafe { install_sites(s, target) }.unwrap();
    let relay = SLOTS.lock().unwrap()[0]._relay as usize;
    let mut counter = [0u64; 32];
    assert_eq!(
        unsafe { calls[0](counter.as_mut_ptr() as u64, 0, 0, ORIGINAL) },
        ORIGINAL & !255
    );
    assert_eq!(unsafe { restore() }, Ok(true));
    assert_eq!(
        unsafe { calls[0](counter.as_mut_ptr() as u64, 0, 0, ORIGINAL) },
        ORIGINAL
    );
    // Emulate a thread that fetched the patched operand just before restore.
    let delayed: Call = unsafe { std::mem::transmute(relay) };
    assert_eq!(
        unsafe { delayed(counter.as_mut_ptr() as u64, 0, 0, ORIGINAL) },
        ORIGINAL & !255
    );
    assert_eq!(counter[0], 3);
    running.store(false, Ordering::Release);
    let total: u64 = workers.into_iter().map(|w| w.join().unwrap()).sum();
    assert!(total > 0);
    println!("concurrent original calls: {total}");
    assert_eq!(
        s.map(|(p, _)| copy_current::<16>(p as u64).unwrap()),
        s.map(|(_, w)| w)
    );
    assert_eq!(RUNTIME_PROMOTION_TARGET.load(Ordering::Acquire), target);
}
