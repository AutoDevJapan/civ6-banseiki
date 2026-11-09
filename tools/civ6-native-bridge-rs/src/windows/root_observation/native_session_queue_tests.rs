use super::*;
use std::os::windows::process::CommandExt;
use std::sync::atomic::AtomicUsize;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
#[link(name = "kernel32")]
unsafe extern "system" {
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
    fn RtlUnwindEx(
        frame: *const c_void,
        ip: *const c_void,
        record: *const EXCEPTION_RECORD,
        value: *const c_void,
        context: *const CONTEXT,
        history: *const c_void,
    );
}
static FIXTURE: AtomicBool = AtomicBool::new(false);
pub(super) fn caller_override(rva: u64) -> Option<u64> {
    if !FIXTURE.load(Ordering::Acquire) {
        return None;
    }
    Some(match rva {
        0x842581 => queue_fixture_enqueue_caller_return as *const () as u64,
        0x8475b2 => queue_fixture_move_return as *const () as u64,
        0x83d81e => queue_fixture_decode_return as *const () as u64,
        0x83d855 => queue_fixture_free_return as *const () as u64,
        0x83d888 => queue_fixture_admission_return as *const () as u64,
        0x8473e8 => queue_fixture_cancel_free_return as *const () as u64,
        _ => return None,
    })
}
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_DEFERRED_QUEUE_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let mut p = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!("windows::root_observation::native_frames::sessions::deferred::tests::{name}"),
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = p.try_wait().unwrap() {
            assert!(status.success(), "owned queue child {status}");
            return false;
        }
        if Instant::now() >= deadline {
            let _ = p.kill();
            let _ = p.wait();
            panic!("owned queue fixture timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn scope_enter(epoch: u64) -> Result<u64, Error> {
    FRAMES
        .get()
        .unwrap()
        .enter_existing_epoch(epoch)
        .map_err(|_| Error::Stale)
}
fn scope_leave(hold: u64, exceptional: bool) -> Result<(), Error> {
    let f = FRAMES.get().unwrap();
    if exceptional {
        f.mark_unresolved();
    }
    f.leave(hold).map_err(|_| Error::Stale)
}
static SESSION: AtomicU64 = AtomicU64::new(0);
static DESTINATION: AtomicU64 = AtomicU64::new(0);
static DECODED: AtomicU64 = AtomicU64::new(0);
static ENQUEUE_COUNT: AtomicUsize = AtomicUsize::new(0);
static FREE_COUNT: AtomicUsize = AtomicUsize::new(0);
static ADMISSION_WORLD: Mutex<Option<SessionWorldReceipt>> = Mutex::new(None);
static FULL_HOLD: AtomicBool = AtomicBool::new(false);
static BEFORE_PUBLISH: AtomicBool = AtomicBool::new(false);
static GROW_MAP: AtomicU64 = AtomicU64::new(0);
extern "system" fn before_publication(queue: u64) {
    ENQUEUE_COUNT.fetch_add(1, Ordering::AcqRel);
    let address = DESTINATION.load(Ordering::Acquire);
    BEFORE_PUBLISH.store(
        slots().queued_addresses(queue).contains(&address) && read64(queue + 0x20) == Ok(0),
        Ordering::Release,
    );
}
extern "system" fn admission_body(session: u64, message: u64) {
    let _native_inputs = (session, message);
    *ADMISSION_WORLD.lock().unwrap() = super::super::NativeSessionApi::current_world();
    FULL_HOLD.store(
        FRAMES
            .get()
            .unwrap()
            .seal_and_wait(Instant::now() + Duration::from_secs(1))
            == Err(crate::native_flight::Error::Reentrant),
        Ordering::Release,
    );
}
extern "system" fn free_body(_: u64) {
    FREE_COUNT.fetch_add(1, Ordering::AcqRel);
}
extern "system" fn growth_body(queue: u64) {
    unsafe {
        *((queue + 8) as *mut u64) = GROW_MAP.load(Ordering::Acquire);
        *((queue + 0x10) as *mut u64) = 4;
    }
}
global_asm!(
    ".p2align 4",".globl queue_fixture_enqueue",".seh_proc queue_fixture_enqueue","queue_fixture_enqueue:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi",
    "sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rbx,rcx","mov rcx,[rip+{dest}]","call {movew}",
    ".globl queue_fixture_move_return","queue_fixture_move_return:",
    "mov rcx,rbx","call {published}","inc qword ptr [rbx+0x20]",
    "mov rbx,[rsp+0x30]","lea rsp,[rsp+0x20]","pop rdi","ret",".seh_endproc",
    ".p2align 4",".globl queue_fixture_move",".seh_proc queue_fixture_move","queue_fixture_move:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8",".seh_endprologue",
    "mov rax,[rdx]","mov [rcx],rax","mov rax,[rdx+8]","mov [rcx+8],rax",
    "mov rax,[rdx+0x10]","mov [rcx+0x10],rax",
    "mov qword ptr [rdx],0","mov qword ptr [rdx+8],0","mov qword ptr [rdx+0x10],0",
    "mov rax,rcx","mov rbx,[rsp+8]","ret",".seh_endproc",
    ".globl queue_fixture_enqueue_caller",".seh_proc queue_fixture_enqueue_caller","queue_fixture_enqueue_caller:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {enqueue}",
    ".globl queue_fixture_enqueue_caller_return","queue_fixture_enqueue_caller_return:",
    "lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl queue_fixture_pump",".seh_proc queue_fixture_pump","queue_fixture_pump:",
    ".byte 0x40,0x55",".seh_pushreg rbp","push rbx",".seh_pushreg rbx","push rsi",".seh_pushreg rsi",
    "push rdi",".seh_pushreg rdi","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov rbx,rcx","mov rdi,[rip+{dest}]","mov rcx,rdi","mov edx,1","mov r8d,2","call {decode}",
    ".globl queue_fixture_decode_return","queue_fixture_decode_return:",
    "mov rsi,rax","lea rcx,[rdi+0x18]","mov rdx,[rdi]","call {freew}",
    ".globl queue_fixture_free_return","queue_fixture_free_return:",
    "dec qword ptr [rbx+0xf8]","inc qword ptr [rbx+0xf0]",
    "mov rcx,rbx","mov rdx,rsi","call {admitted}",
    ".globl queue_fixture_admission_return","queue_fixture_admission_return:",
    "mov rax,0x8811","lea rsp,[rsp+0x28]","pop rdi","pop rsi","pop rbx","pop rbp","ret",".seh_endproc",
    ".p2align 4",".globl queue_fixture_decode",".seh_proc queue_fixture_decode","queue_fixture_decode:",
    "mov [rsp+0x18],r8d",".seh_endprologue","mov rax,[rip+{decoded}]","ret",".seh_endproc",
    ".p2align 4",".globl queue_fixture_free",".seh_proc queue_fixture_free","queue_fixture_free:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {freed}",
    "lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl queue_fixture_clear",".seh_proc queue_fixture_clear","queue_fixture_clear:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov rbx,rcx","mov rcx,[rip+{dest}]","mov rdx,[rcx]","lea rcx,[rcx+0x18]","call {freew}",
    ".globl queue_fixture_cancel_free_return","queue_fixture_cancel_free_return:",
    "mov qword ptr [rbx+0x20],0","mov rbx,[rsp+0x30]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    dest=sym DESTINATION,decoded=sym DECODED,movew=sym native_session_queue_move_gateway,
    published=sym before_publication,enqueue=sym native_session_queue_enqueue_gateway,decode=sym native_session_queue_decode_gateway,
    freew=sym native_session_queue_free_gateway,admitted=sym super::super::native_session_admission_gateway,freed=sym free_body,
);
global_asm!(
    ".p2align 4",".globl queue_fixture_admission_original",".seh_proc queue_fixture_admission_original","queue_fixture_admission_original:",
    ".byte 0x40,0x55",".seh_pushreg rbp","push rbx",".seh_pushreg rbx","push rsi",".seh_pushreg rsi",
    "push rdi",".seh_pushreg rdi","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {body}","lea rsp,[rsp+0x28]","pop rdi","pop rsi","pop rbx","pop rbp","ret",".seh_endproc",
    body=sym admission_body,
);
unsafe extern "system" {
    fn queue_fixture_admission_original();
}
unsafe extern "system" {
    fn queue_fixture_enqueue();
    fn queue_fixture_move();
    fn queue_fixture_pump();
    fn queue_fixture_decode();
    fn queue_fixture_free();
    fn queue_fixture_clear();
    fn queue_fixture_enqueue_caller(queue: u64, source: u64);
    fn queue_fixture_enqueue_caller_return();
    fn queue_fixture_move_return();
    fn queue_fixture_decode_return();
    fn queue_fixture_free_return();
    fn queue_fixture_admission_return();
    fn queue_fixture_cancel_free_return();
}
global_asm!(
    ".p2align 4",".globl queue_fixture_growth",".seh_proc queue_fixture_growth","queue_fixture_growth:",
    ".byte 0x40,0x53",".seh_pushreg rbx","push r14",".seh_pushreg r14",
    "sub rsp,0x38",".seh_stackalloc 0x38",".seh_endprologue","call {body}",
    "lea rsp,[rsp+0x38]","pop r14","pop rbx","ret",".seh_endproc",
    body=sym growth_body,
);
unsafe extern "system" {
    fn queue_fixture_growth();
}
struct Fixture {
    exe: usize,
    session: Box<[u64; 80]>,
    slot: Box<[u64; 6]>,
    source: Box<[u64; 6]>,
    message: Message,
    lease: Lease,
}
impl Fixture {
    fn new() -> Self {
        FIXTURE.store(true, Ordering::Release);
        FRAMES.get_or_init(NativeFrames::new);
        assert!(SCOPE
            .set(SessionScopeHooks {
                enter_existing_epoch: scope_enter,
                leave: scope_leave
            })
            .is_ok());
        CORE.store(1, Ordering::Release);
        let exe = unsafe {
            VirtualAlloc(
                null_mut(),
                0x1050000,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        assert_ne!(exe, 0);
        EXE.store(exe as u64, Ordering::Release);
        unsafe {
            *((exe + 0x10432c0) as *mut u64) = (exe + 0x842030) as u64;
        }
        let mut session = Box::new([0u64; 80]);
        session[0] = exe as u64 + 0x1043088;
        super::super::NATIVE_SESSION_CONTINUE[3].store(
            queue_fixture_admission_original as *const () as u64 + 5,
            Ordering::Release,
        );
        let slot = Box::new([0u64; 6]);
        let source = Box::new([0x1000, 0x1010, 0x1020, 0, 1, 2]);
        let mut s = state();
        let lease = s.birth(0x10, Some(1), 0x20, 2).unwrap();
        s.factory(lease, 1, 0x20, session.as_ptr() as u64, 0x40)
            .unwrap();
        s.published(lease, 1).unwrap();
        s.bind_world(
            lease,
            WorldReceipt {
                binding_id: 8,
                epoch: 1,
            },
        )
        .unwrap();
        s.bind_transport(lease, 0x50, 0x60, 0x70).unwrap();
        let message = s.reserve(lease).unwrap();
        s.dispatch(message, 0x40, 1).unwrap();
        drop(s);
        DESTINATION.store(slot.as_ptr() as u64, Ordering::Release);
        SESSION.store(session.as_ptr() as u64, Ordering::Release);
        DECODED.store(0x9988, Ordering::Release);
        for (i, p) in [
            queue_fixture_enqueue as *const (),
            queue_fixture_move as *const (),
            queue_fixture_pump as *const (),
            queue_fixture_decode as *const (),
        ]
        .into_iter()
        .enumerate()
        {
            NATIVE_SESSION_QUEUE_CONTINUE[i].store(p as u64 + 5, Ordering::Release);
        }
        NATIVE_SESSION_QUEUE_CONTINUE[4]
            .store(queue_fixture_free as *const () as u64, Ordering::Release);
        NATIVE_SESSION_QUEUE_CONTINUE[5].store(
            queue_fixture_clear as *const () as u64 + 5,
            Ordering::Release,
        );
        Self {
            exe,
            session,
            slot,
            source,
            message,
            lease,
        }
    }
    fn enqueue(&mut self) {
        let c = super::super::Call {
            serial: 1,
            kind: 73,
            lease: Some(self.lease),
            hold: None,
            message: Some(self.message),
            registry: 0,
            owner: 0,
            id: 0,
            transport: None,
            native_transport: false,
            thread: 0,
            unproven_hold: None,
        };
        super::super::CALLS.with(|s| s.borrow_mut().push(c));
        unsafe {
            queue_fixture_enqueue_caller(
                self.session.as_ptr() as u64 + 0xd8,
                self.source.as_ptr() as u64,
            )
        };
        super::super::CALLS.with(|s| {
            s.borrow_mut().pop();
        });
        self.session[31] = 1;
    }
    fn pump(&self) -> u64 {
        let f: unsafe extern "system" fn(u64) -> u64 =
            unsafe { std::mem::transmute(gateways()[2]) };
        unsafe { f(self.session.as_ptr() as u64) }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_ne!(unsafe { VirtualFree(self.exe as _, 0, MEM_RELEASE) }, 0);
    }
}
#[test]
fn actual_move_binds_before_count_and_pump_keeps_old_receipt_until_full_return() {
    if !child("actual_move_binds_before_count_and_pump_keeps_old_receipt_until_full_return") {
        return;
    }
    let mut f = Fixture::new();
    f.enqueue();
    assert!(BEFORE_PUBLISH.load(Ordering::Acquire));
    assert_eq!(ENQUEUE_COUNT.load(Ordering::Acquire), 1);
    assert_eq!(&f.source[..3], &[0, 0, 0]);
    assert_eq!(&f.slot[..3], &[0x1000, 0x1010, 0x1020]);
    assert_eq!(pending(), Ok(1));
    state().dispatched(f.message).unwrap();
    assert_eq!(f.pump(), 0x8811);
    assert_eq!(FREE_COUNT.load(Ordering::Acquire), 1);
    assert!(FULL_HOLD.load(Ordering::Acquire));
    assert_eq!(
        *ADMISSION_WORLD.lock().unwrap(),
        Some(WorldReceipt {
            binding_id: 8,
            epoch: 1
        })
    );
    assert_eq!(pending(), Ok(0));
    FRAMES
        .get()
        .unwrap()
        .seal_and_wait(Instant::now() + Duration::from_secs(1))
        .unwrap();
}
#[test]
fn actual_clear_only_settles_reserved_packet_after_normal_storage_return() {
    if !child("actual_clear_only_settles_reserved_packet_after_normal_storage_return") {
        return;
    }
    let mut f = Fixture::new();
    f.enqueue();
    assert_eq!(pending(), Ok(1));
    let clear: unsafe extern "system" fn(u64) = unsafe { std::mem::transmute(gateways()[5]) };
    unsafe { clear(f.session.as_ptr() as u64 + 0xd8) };
    assert_eq!(pending(), Ok(0));
    assert_eq!(FREE_COUNT.load(Ordering::Acquire), 1);
    assert_eq!(
        state().message_world(f.message).unwrap(),
        WorldReceipt {
            binding_id: 8,
            epoch: 1
        }
    );
}

#[test]
fn actual_growth_relocates_map_without_changing_reserved_slot_receipt() {
    if !child("actual_growth_relocates_map_without_changing_reserved_slot_receipt") {
        return;
    }
    let mut f = Fixture::new();
    f.enqueue();
    let mut newmap = [0u64; 4];
    newmap[3] = f.slot.as_ptr() as u64;
    GROW_MAP.store(newmap.as_ptr() as u64, Ordering::Release);
    NATIVE_SESSION_QUEUE_CONTINUE[6].store(
        queue_fixture_growth as *const () as u64 + 8,
        Ordering::Release,
    );
    let grow: unsafe extern "system" fn(u64, u64) = unsafe { std::mem::transmute(gateways()[6]) };
    unsafe { grow(f.session.as_ptr() as u64 + 0xd8, 1) };
    assert_eq!(pending(), Ok(1));
    assert_eq!(f.pump(), 0x8811);
    assert_eq!(
        *ADMISSION_WORLD.lock().unwrap(),
        Some(WorldReceipt {
            binding_id: 8,
            epoch: 1
        })
    );
    assert_eq!(pending(), Ok(0));
}
#[test]
fn slot_address_reuse_during_free_keeps_old_serial_until_old_full_return() {
    let mut s = slots::Ledger::new();
    let old = s.reserve(1, 11u64).unwrap();
    s.bind(old, 0x80).unwrap();
    s.published(old).unwrap();
    assert_eq!(s.retain(1, 0x80), Ok(Some((old, 11))));
    s.free_begin(old).unwrap();
    let new = s.reserve(1, 22).unwrap();
    s.bind(new, 0x80).unwrap();
    s.published(new).unwrap();
    assert_ne!(old, new);
    assert_eq!(s.executed(old), Err(slots::Error::Busy));
    s.free_returned(old).unwrap();
    assert_eq!(s.executed(old), Ok(11));
    assert_eq!(s.retain(1, 0x80), Ok(Some((new, 22))));
    assert_eq!(s.pending(), Ok(1));
}
#[test]
fn growth_keeps_slot_identity_and_clear_cannot_cancel_active_execution() {
    let mut s = slots::Ledger::new();
    let id = s.reserve(1, 11u64).unwrap();
    s.bind(id, 0x80).unwrap();
    s.published(id).unwrap();
    assert_eq!(s.queued_addresses(1), vec![0x80]);
    assert_eq!(s.retain(1, 0x80), Ok(Some((id, 11))));
    assert_eq!(s.clear_begin(1), Err(slots::Error::Busy));
    s.unresolved();
    assert_eq!(s.pending(), Err(slots::Error::Unresolved));
}

fn file_bytes(file: &mut File, rva: u32, len: usize) -> Vec<u8> {
    file.seek(SeekFrom::Start(0x3c)).unwrap();
    let mut d = [0; 4];
    file.read_exact(&mut d).unwrap();
    let nt = u32::from_le_bytes(d) as u64;
    let mut h = [0; 2];
    file.seek(SeekFrom::Start(nt + 6)).unwrap();
    file.read_exact(&mut h).unwrap();
    let n = u16::from_le_bytes(h);
    file.seek(SeekFrom::Start(nt + 20)).unwrap();
    file.read_exact(&mut h).unwrap();
    let table = nt + 24 + u64::from(u16::from_le_bytes(h));
    for i in 0..n {
        file.seek(SeekFrom::Start(table + u64::from(i) * 40 + 12))
            .unwrap();
        let mut row = [0; 12];
        file.read_exact(&mut row).unwrap();
        let a = u32::from_le_bytes(row[..4].try_into().unwrap());
        let size = u32::from_le_bytes(row[4..8].try_into().unwrap());
        if rva >= a && rva - a < size {
            file.seek(SeekFrom::Start(u64::from(
                u32::from_le_bytes(row[8..12].try_into().unwrap()) + rva - a,
            )))
            .unwrap();
            let mut b = vec![0; len];
            file.read_exact(&mut b).unwrap();
            return b;
        }
    }
    panic!("primary RVA missing");
}
#[test]
#[ignore = "explicit pinned Host input; separate from owned gateway fixtures"]
fn fixed_primary_full_frames_unwind_leaf_and_publication_callers_match() {
    let mut file = super::super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    for f in fixed_queue::FRAMES {
        let b = file_bytes(&mut file, f.begin, (f.end - f.begin) as usize);
        assert!(code_matches(&b, f));
        assert_eq!(b, unhex(f.specimen));
        for p in f.parts {
            assert_eq!(
                file_bytes(&mut file, p.unwind, p.hex.len() / 2),
                unhex(p.hex)
            );
        }
    }
    for (rva, word, _) in SITES {
        assert_eq!(file_bytes(&mut file, rva as u32, 8), word);
    }
    for (rva, hex) in [
        (0x84257c, "e88f4f0000"),
        (0x8475ad, "e88efecdff"),
        (0x8475b2, "8b46208947208b46248947240fb6462888472848ff4320"),
        (0x83d819, "e812c4fbff"),
        (0x83d850, "e83b472100"),
        (0x83d882, "ff9038020000"),
        (0x8473e3, "e8a8ab2000"),
    ] {
        assert_eq!(file_bytes(&mut file, rva, hex.len() / 2), unhex(hex));
    }
}
#[test]
fn fixed_complete_code_mutations_reject_moved_publication_free_and_epilogue() {
    let mut checked = 0;
    for f in fixed_queue::FRAMES {
        let mut b = unhex(f.specimen);
        assert!(code_matches(&b, f));
        for i in [0, 4, b.len() / 2, b.len() - 1] {
            b[i] ^= 1;
            assert!(!code_matches(&b, f));
            b[i] ^= 1;
            checked += 1;
        }
        assert!(!code_matches(&b[..b.len() - 1], f));
    }
    assert_eq!(checked, 24);
}

unsafe extern "system" {
    fn native_session_queue_enqueue_original_body();
    fn native_session_queue_move_original_body();
    fn native_session_queue_pump_original_body();
    fn native_session_queue_decode_original_body();
    fn native_session_queue_free_original_body();
    fn native_session_queue_clear_original_body();
    fn native_session_queue_growth_original_body();
}
#[test]
fn all_displaced_unwind_shims_restore_original_stack_and_nonvolatile_saves() {
    for (i, pc) in [
        native_session_queue_enqueue_original_body as *const (),
        native_session_queue_move_original_body as *const (),
        native_session_queue_pump_original_body as *const (),
        native_session_queue_decode_original_body as *const (),
        native_session_queue_free_original_body as *const (),
        native_session_queue_clear_original_body as *const (),
        native_session_queue_growth_original_body as *const (),
    ]
    .into_iter()
    .enumerate()
    {
        let mut stack = [0u64; 40];
        let entry = unsafe { stack.as_mut_ptr().add(12) } as u64;
        unsafe {
            *(entry as *mut u64) = 0x12345678;
        }
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rsp = entry;
        c.Rip = pc as u64;
        c.Rbx = 0xbbbb;
        match i {
            0 | 1 | 5 => unsafe {
                *((entry + 8) as *mut u64) = 0xbbbb;
            },
            2 => {
                c.Rsp -= 32;
                for (j, v) in [0xdddd, 0xcccc, 0xbbbb, 0xaaaa].into_iter().enumerate() {
                    unsafe {
                        *((c.Rsp + j as u64 * 8) as *mut u64) = v;
                    }
                }
            }
            6 => {
                c.Rsp -= 0x48;
                unsafe {
                    *((entry - 8) as *mut u64) = 0xbbbb;
                    *((entry - 16) as *mut u64) = 0xeeee;
                }
            }
            _ => {}
        }
        let mut base = 0;
        let row = unsafe { RtlLookupFunctionEntry(c.Rip, &mut base, null_mut()) };
        assert!(!row.is_null(), "shim{i}");
        let mut data = null_mut();
        let mut frame = 0;
        unsafe {
            RtlVirtualUnwind(
                0,
                base,
                c.Rip,
                row,
                &mut c,
                &mut data,
                &mut frame,
                null_mut(),
            )
        };
        assert_eq!(c.Rip, 0x12345678, "shim{i}");
        assert_eq!(c.Rsp, entry + 8, "shim{i}");
        assert_eq!(c.Rbx, 0xbbbb, "shim{i}");
        if i == 2 {
            assert_eq!((c.Rbp, c.Rsi, c.Rdi), (0xaaaa, 0xcccc, 0xdddd));
        }
        if i == 6 {
            assert_eq!(c.R14, 0xeeee);
        }
    }
}

static UNWIND_STATE: AtomicU64 = AtomicU64::new(0);
global_asm!(
    ".globl queue_unwind_driver",".seh_proc queue_unwind_driver","queue_unwind_driver:",
    "push rbx",".seh_pushreg rbx","sub rsp,0x80",".seh_stackalloc 0x80",".seh_endprologue",
    "mov rbx,rcx","mov [rcx],rsp","lea rax,[rip+queue_unwind_landed]","mov [rcx+8],rax",
    "mov rcx,r8","call rdx","xor eax,eax","jmp 2f",
    "queue_unwind_landed:","mov eax,1","2:","lea rsp,[rsp+0x80]","pop rbx","ret",".seh_endproc",
    ".p2align 4",".globl queue_unwind_free",".seh_proc queue_unwind_free","queue_unwind_free:",
    "sub rsp,0x528",".seh_stackalloc 0x528",".seh_endprologue",
    "mov r10,[rip+{state}]","mov rcx,[r10]","mov rdx,[r10+8]",
    "lea r10,[rsp+0x40]","mov r11d,0x9a","1:","mov qword ptr [r10],0","lea r10,[r10+8]","dec r11d","jnz 1b",
    "lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0",
    "xor r8d,r8d","xor r9d,r9d","call {unwind_ex}","int3",".seh_endproc",
    state=sym UNWIND_STATE,unwind_ex=sym RtlUnwindEx,
);
unsafe extern "system" {
    fn queue_unwind_driver(state: *mut u64, target: usize, receiver: u64) -> u64;
    fn queue_unwind_free();
}
#[test]
fn actual_seh_during_storage_free_keeps_packet_unresolved_and_releases_full_pump_hold() {
    if !child("actual_seh_during_storage_free_keeps_packet_unresolved_and_releases_full_pump_hold")
    {
        return;
    }
    let mut f = Fixture::new();
    f.enqueue();
    state().dispatched(f.message).unwrap();
    let mut landing = [0u64; 2];
    UNWIND_STATE.store(landing.as_mut_ptr() as u64, Ordering::Release);
    NATIVE_SESSION_QUEUE_CONTINUE[4]
        .store(queue_unwind_free as *const () as u64, Ordering::Release);
    assert_eq!(
        unsafe {
            queue_unwind_driver(
                landing.as_mut_ptr(),
                gateways()[2],
                f.session.as_ptr() as u64,
            )
        },
        1
    );
    assert_eq!(pending(), Err("deferred queue unresolved"));
    assert!(QUEUE_CALLS.with(|s| s.borrow().is_empty()));
    assert!(!FRAMES.get().unwrap().healthy());
    assert_eq!(
        FRAMES.get().unwrap().reopen(),
        Err(crate::native_flight::Error::Unresolved)
    );
}

#[test]
fn pending_old_packet_blocks_same_session_rebind_until_cancel_then_never_mints_new_epoch() {
    if !child(
        "pending_old_packet_blocks_same_session_rebind_until_cancel_then_never_mints_new_epoch",
    ) {
        return;
    }
    let mut f = Fixture::new();
    f.enqueue();
    let old = WorldReceipt {
        binding_id: 8,
        epoch: 1,
    };
    let new = WorldReceipt {
        binding_id: 9,
        epoch: 2,
    };
    state().dispatched(f.message).unwrap();
    assert_eq!(state().retire_world(old), Err(Error::Busy));
    assert_eq!(
        state().loaded_world(f.lease, Some(old), new),
        Err(Error::Busy)
    );
    let clear: unsafe extern "system" fn(u64) = unsafe { std::mem::transmute(gateways()[5]) };
    unsafe { clear(f.session.as_ptr() as u64 + 0xd8) };
    state().retire_world(old).unwrap();
    state().loaded_world(f.lease, Some(old), new).unwrap();
    assert_eq!(state().world(f.lease), Ok(new));
    assert_eq!(state().message_world(f.message), Err(Error::Unknown));
    assert_eq!(
        state().dispatch_deferred(f.message, f.session.as_ptr() as u64),
        Err(Error::Unknown)
    );
    let message = state().reserve(f.lease).unwrap();
    assert_eq!(state().message_world(message), Ok(new));
    state().cancel_before_dispatch(message).unwrap();
    assert_eq!(pending(), Ok(0));
}

#[test]
fn unknown_deferred_slot_never_borrows_an_unrelated_outer_incoming_receipt() {
    if !child("unknown_deferred_slot_never_borrows_an_unrelated_outer_incoming_receipt") {
        return;
    }
    let mut f = Fixture::new();
    f.slot[0] = 0x1000;
    f.slot[1] = 0x1010;
    f.slot[2] = 0x1020;
    f.session[31] = 1;
    let c = super::super::Call {
        serial: 1,
        kind: 72,
        lease: Some(f.lease),
        hold: None,
        message: Some(f.message),
        registry: 0,
        owner: 0,
        id: 0,
        transport: None,
        native_transport: false,
        thread: 0,
        unproven_hold: None,
    };
    super::super::CALLS.with(|s| s.borrow_mut().push(c));
    assert_eq!(f.pump(), 0x8811);
    super::super::CALLS.with(|s| {
        s.borrow_mut().pop();
    });
    assert_eq!(
        *ADMISSION_WORLD.lock().unwrap(),
        None,
        "unknown83D888 must not fall back to outer72"
    );
    assert!(
        FULL_HOLD.load(Ordering::Acquire),
        "unknown original frame must still block teardown"
    );
    assert!(!FRAMES.get().unwrap().unproven_on_current_thread());
    assert_eq!(FREE_COUNT.load(Ordering::Acquire), 1);
    assert_eq!(pending(), Ok(0));
}

static CAS_HITS: AtomicUsize = AtomicUsize::new(0);
extern "system" fn cas_hit() {
    CAS_HITS.fetch_add(1, Ordering::AcqRel);
    unsafe { SetLastError(0x7788) };
}
global_asm!(
    ".p2align 4",".globl queue_cas_free",".seh_proc queue_cas_free","queue_cas_free:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {hit}",
    "mov rax,0x8877665544332211","stc","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".globl queue_cas_driver",".seh_proc queue_cas_driver","queue_cas_driver:",
    "push rbx",".seh_pushreg rbx","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rbx,r8","mov r10,rcx","mov rcx,rdx","xor edx,edx","xor r8d,r8d","call r10",
    "mov [rbx],rax","pushfq","pop rax","mov [rbx+8],rax","lea rsp,[rsp+0x20]","pop rbx","ret",".seh_endproc",
    hit=sym cas_hit,
);
unsafe extern "system" {
    fn queue_cas_free();
    fn queue_cas_driver(target: usize, receiver: *mut u64, result: *mut u64);
}
fn cas_stub(i: usize, word: [u8; 8]) -> Vec<u8> {
    if i == 4 {
        return word.to_vec();
    }
    let mut b = word.to_vec();
    match i {
        0 | 5 => b.extend([0x24, 0x10]),
        1 => b.extend([0xec, 0x20]),
        2 => b.push(0x56),
        3 => b.push(0x10),
        6 => {}
        _ => unreachable!(),
    };
    let shadow = if i == 1 || i == 6 { 0x20 } else { 0x28 };
    b.extend([0x48, 0x83, 0xec, shadow]);
    b.extend([0x48, 0xb8]);
    b.extend((cas_hit as *const () as u64).to_le_bytes());
    b.extend([0xff, 0xd0]);
    b.extend([0x48, 0x8d, 0x64, 0x24, shadow]);
    b.extend([0x48, 0xb8]);
    b.extend(0x8877665544332211u64.to_le_bytes());
    match i {
        0 | 5 => b.extend(unhex("488b5c2408488b742410")),
        1 => b.extend(unhex("488b5c2430488d6424205f")),
        2 => b.extend(unhex("415e415c5f5e5b5d")),
        3 => {}
        6 => b.extend(unhex("488d642438415e5b")),
        _ => unreachable!(),
    }
    b.extend([0xf9, 0xc3]);
    b
}
#[test]
fn all_seven_installed_entries_execute_original_once_preserve_result_flags_and_restore() {
    if !child("all_seven_installed_entries_execute_original_once_preserve_result_flags_and_restore")
    {
        return;
    }
    assert!(supports_atomic_promotion_fetch());
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0xa60000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    for (i, (rva, word, _)) in SITES.into_iter().enumerate() {
        let b = cas_stub(i, word);
        unsafe { ptr::copy_nonoverlapping(b.as_ptr(), (base + rva) as *mut u8, b.len()) };
    }
    let free_target = queue_cas_free as *const () as u64;
    unsafe {
        ptr::copy_nonoverlapping(
            [0xff, 0x25, 0, 0, 0, 0].as_ptr(),
            (base + 0x9825f0) as *mut u8,
            6,
        );
        ptr::copy_nonoverlapping(
            free_target.to_le_bytes().as_ptr(),
            (base + 0x9825f6) as *mut u8,
            8,
        );
    }
    let mut old = 0;
    assert_ne!(
        unsafe { VirtualProtect(base as _, 0xa60000, PAGE_EXECUTE_READ, &mut old) },
        0
    );
    unsafe { install_at(base) }.unwrap();
    let mut receiver = [0u64; 8];
    let mut result = [0u64; 2];
    for (i, (rva, _, _)) in SITES.into_iter().enumerate() {
        unsafe {
            SetLastError(0x1234);
            queue_cas_driver(base + rva, receiver.as_mut_ptr(), result.as_mut_ptr())
        };
        assert_eq!(result[0], 0x8877665544332211, "entry{i}");
        assert_eq!(result[1] & 1, 1, "entry{i} output flags");
        assert_eq!(unsafe { GetLastError() }, 0x7788, "entry{i}");
        assert_eq!(CAS_HITS.load(Ordering::Acquire), i + 1);
    }
    unsafe { restore() }.unwrap();
    for (rva, word, _) in SITES {
        assert_eq!(copy_current::<8>((base + rva) as u64), Some(word));
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe { VirtualQuery((base + rva) as _, &mut info, std::mem::size_of_val(&info)) },
            0
        );
        assert_eq!(info.Protect, PAGE_EXECUTE_READ);
    }
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}
