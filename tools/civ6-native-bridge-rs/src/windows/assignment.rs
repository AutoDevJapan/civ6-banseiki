//! Stage A only: observe the natural four-register CanAssign call and preserve
//! its entire RAX. Addresses are evidence for this invocation, never authority.
use super::*;
use crate::governor_payment::{self, CurrentMemory, PROMOTION_WORDS};

#[repr(C)]
pub struct PromotionRow {
    /// 0 is a copied snapshot, 1..6 are ReadFailure, 7 means no snapshot.
    pub status: AtomicU32,
    pub owner: AtomicU32,
    pub index: AtomicU32,
    pub count: AtomicU32,
    pub owned: [AtomicU64; PROMOTION_WORDS],
}

impl Default for PromotionRow {
    fn default() -> Self {
        Self {
            status: AtomicU32::new(7),
            owner: AtomicU32::new(u32::MAX),
            index: AtomicU32::new(u32::MAX),
            count: AtomicU32::new(0),
            owned: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

struct InvocationMemory;
impl CurrentMemory for InvocationMemory {
    fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]> {
        copy_current(address)
    }
}

pub const PINNED_CAN_ASSIGN_RVA: usize = 0x2d_d880;
pub const ASSIGN_FILE_OFFSET: u64 = 0x2d_cc80;
pub const ASSIGN_PROLOGUE: [u8; 10] = [0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0x6c, 0x24, 0x10];
pub const ASSIGN_CAPACITY: usize = 1024;
pub const ASSIGN_MAGIC: u32 = 0x4a41_4353;

#[repr(C)]
#[derive(Default)]
pub struct AssignRow {
    pub sequence: AtomicU32,
    pub thread_id: AtomicU32,
    pub governors_this: AtomicU64,
    pub governor_pointer: AtomicU64,
    pub city_pointer: AtomicU64,
    pub raw_r9: AtomicU64,
    pub return_address: AtomicU64,
    pub player_address: AtomicU64,
    pub player_id: AtomicU32,
    pub governor_index: AtomicU32,
    /// Bit 0: owner fields; bit 1: Governor+4; bit 2: City fields copied.
    pub read_flags: AtomicU32,
    /// Raw fields used by pinned CanAssign; names await live Lua correlation.
    pub city_a8: AtomicU32,
    pub city_c0: AtomicU32,
    pub city_c4: AtomicU32,
    pub city_d8: AtomicU32,
    pub original_al: AtomicU32,
    pub original_rax: AtomicU64,
    pub promotions: PromotionRow,
}

#[repr(C)]
pub struct AssignCalibrationTrace {
    pub magic: u32,
    pub version: u32,
    pub next: AtomicU32,
    pub dropped: AtomicU32,
    pub in_flight: AtomicU32,
    pub reserved: u32,
    pub rows: [AssignRow; ASSIGN_CAPACITY],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignCalibrationObservation {
    pub sequence: u32,
    pub thread_id: u32,
    pub governors_this: u64,
    pub governor_pointer: u64,
    pub city_pointer: u64,
    pub raw_r9: u64,
    pub return_address: u64,
    pub player_address: u64,
    pub player_id: u32,
    pub governor_index: u32,
    pub read_flags: u32,
    pub city_a8: u32,
    pub city_c0: u32,
    pub city_c4: u32,
    pub city_d8: u32,
    pub original_al: u32,
    pub original_rax: u64,
    pub promotion_status: u32,
    pub promotion_owner: u32,
    pub promotion_index: u32,
    pub promotion_count: u32,
    pub promotion_owned: [u64; PROMOTION_WORDS],
}

impl AssignCalibrationTrace {
    pub const VERSION: u32 = 2;

    pub fn boxed() -> Box<Self> {
        let mut trace = Box::<Self>::new_uninit();
        unsafe {
            Self::initialize_at(trace.as_mut_ptr());
            trace.assume_init()
        }
    }

    /// # Safety
    /// `trace` is aligned writable storage for Self, not yet shared with any
    /// callback. Initialize rows in place without a large Windows stack value.
    pub(super) unsafe fn initialize_at(trace: *mut Self) {
        unsafe {
            ptr::addr_of_mut!((*trace).magic).write(ASSIGN_MAGIC);
            ptr::addr_of_mut!((*trace).version).write(Self::VERSION);
            ptr::addr_of_mut!((*trace).next).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*trace).dropped).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*trace).in_flight).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*trace).reserved).write(0);
            let rows = ptr::addr_of_mut!((*trace).rows).cast::<AssignRow>();
            for index in 0..ASSIGN_CAPACITY {
                rows.add(index).write(AssignRow::default());
            }
        }
    }
    pub fn complete_rows(&self) -> Result<Vec<AssignCalibrationObservation>, &'static str> {
        if self.magic != ASSIGN_MAGIC || self.version != Self::VERSION {
            return Err("assignment mapping version mismatch");
        }
        if self.in_flight.load(Ordering::Acquire) != 0 {
            return Err("assignment calls remain in flight");
        }
        if self.dropped.load(Ordering::Acquire) != 0 {
            return Err("assignment capacity exceeded");
        }
        let next = self.next.load(Ordering::Acquire) as usize;
        if next > ASSIGN_CAPACITY {
            return Err("assignment sequence exceeded capacity");
        }
        let mut rows = Vec::with_capacity(next);
        for (slot, row) in self.rows[..next].iter().enumerate() {
            if row.sequence.load(Ordering::Acquire) != 1 {
                return Err("assignment call has no original return");
            }
            rows.push(AssignCalibrationObservation {
                sequence: slot as u32 + 1,
                thread_id: row.thread_id.load(Ordering::Relaxed),
                governors_this: row.governors_this.load(Ordering::Relaxed),
                governor_pointer: row.governor_pointer.load(Ordering::Relaxed),
                city_pointer: row.city_pointer.load(Ordering::Relaxed),
                raw_r9: row.raw_r9.load(Ordering::Relaxed),
                return_address: row.return_address.load(Ordering::Relaxed),
                player_address: row.player_address.load(Ordering::Relaxed),
                player_id: row.player_id.load(Ordering::Relaxed),
                governor_index: row.governor_index.load(Ordering::Relaxed),
                read_flags: row.read_flags.load(Ordering::Relaxed),
                city_a8: row.city_a8.load(Ordering::Relaxed),
                city_c0: row.city_c0.load(Ordering::Relaxed),
                city_c4: row.city_c4.load(Ordering::Relaxed),
                city_d8: row.city_d8.load(Ordering::Relaxed),
                original_al: row.original_al.load(Ordering::Relaxed),
                original_rax: row.original_rax.load(Ordering::Relaxed),
                promotion_status: row.promotions.status.load(Ordering::Relaxed),
                promotion_owner: row.promotions.owner.load(Ordering::Relaxed),
                promotion_index: row.promotions.index.load(Ordering::Relaxed),
                promotion_count: row.promotions.count.load(Ordering::Relaxed),
                promotion_owned: std::array::from_fn(|i| {
                    row.promotions.owned[i].load(Ordering::Relaxed)
                }),
            });
        }
        Ok(rows)
    }
}

#[unsafe(no_mangle)]
pub(super) static ASSIGN_TRACE_PTR: AtomicPtr<AssignCalibrationTrace> = AtomicPtr::new(null_mut());
#[unsafe(no_mangle)]
pub(super) static ASSIGN_TRAMPOLINE_PTR: AtomicPtr<u8> = AtomicPtr::new(null_mut());

fn field32(base: u64, offset: u64) -> Option<u32> {
    copy_current::<4>(base.checked_add(offset)?).map(u32::from_le_bytes)
}

pub(super) unsafe extern "system" fn assign_calibration_begin(
    governors: u64,
    governor: u64,
    city: u64,
    raw_r9: u64,
    caller: u64,
    fence: *mut runtime::CallFence,
) -> u64 {
    if !fence.is_null() {
        unsafe {
            ptr::write(fence, runtime::call_begin(governors));
        }
    }
    let generation = u64::from(runtime::generation()) << 32;
    let Some(trace) = (unsafe { ASSIGN_TRACE_PTR.load(Ordering::Acquire).as_ref() }) else {
        return generation;
    };
    trace.in_flight.fetch_add(1, Ordering::AcqRel);
    if !runtime::sample_begin() {
        return generation;
    }
    // Saturating sequence: capacity exhaustion cannot wrap and overwrite rows.
    let slot = trace
        .next
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < ASSIGN_CAPACITY as u32).then_some(n + 1)
        });
    let Ok(slot) = slot else {
        trace.dropped.fetch_add(1, Ordering::Relaxed);
        runtime::sample_end();
        return generation;
    };
    let row = &trace.rows[slot as usize];
    row.thread_id
        .store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    row.governors_this.store(governors, Ordering::Relaxed);
    row.governor_pointer.store(governor, Ordering::Relaxed);
    row.city_pointer.store(city, Ordering::Relaxed);
    row.raw_r9.store(raw_r9, Ordering::Relaxed);
    row.return_address.store(caller, Ordering::Relaxed);
    row.player_id.store(u32::MAX, Ordering::Relaxed);
    row.governor_index.store(u32::MAX, Ordering::Relaxed);
    let mut flags = 0;
    if let Some(player) = governors
        .checked_add(0x70)
        .and_then(copy_current::<8>)
        .map(u64::from_le_bytes)
        .filter(|p| *p != 0)
    {
        row.player_address.store(player, Ordering::Relaxed);
        if let Some(id) = field32(player, 0xd8) {
            row.player_id.store(id, Ordering::Relaxed);
            flags |= 1;
        }
    }
    if governor != 0 {
        if let Some(index) = field32(governor, 4) {
            row.governor_index.store(index, Ordering::Relaxed);
            flags |= 2;
        }
    }
    if city != 0 {
        if let (Some(a8), Some(c0), Some(c4), Some(d8)) = (
            field32(city, 0xa8),
            field32(city, 0xc0),
            field32(city, 0xc4),
            field32(city, 0xd8),
        ) {
            row.city_a8.store(a8, Ordering::Relaxed);
            row.city_c0.store(c0, Ordering::Relaxed);
            row.city_c4.store(c4, Ordering::Relaxed);
            row.city_d8.store(d8, Ordering::Relaxed);
            flags |= 4;
        }
    }
    row.read_flags.store(flags, Ordering::Relaxed);
    match governor_payment::read_current(&InvocationMemory, governors, governor) {
        Ok(snapshot) => {
            row.promotions
                .owner
                .store(snapshot.owner, Ordering::Relaxed);
            row.promotions
                .index
                .store(snapshot.governor_index, Ordering::Relaxed);
            row.promotions
                .count
                .store(snapshot.count, Ordering::Relaxed);
            for (field, value) in row.promotions.owned.iter().zip(snapshot.owned) {
                field.store(value, Ordering::Relaxed);
            }
            row.promotions.status.store(0, Ordering::Relaxed);
        }
        Err(failure) => row
            .promotions
            .status
            .store(failure as u32, Ordering::Relaxed),
    }
    generation | (slot as u64 + 1)
}

#[cfg(test)]
static PAID_SYNTHETIC_GATE: AtomicBool = AtomicBool::new(false);

pub(super) unsafe extern "system" fn assign_calibration_end(
    token: u64,
    original: u64,
    _governors: u64,
    _governor: u64,
    fence: *const runtime::CallFence,
) -> u64 {
    #[cfg(test)]
    let result = if PAID_SYNTHETIC_GATE.load(Ordering::Acquire) {
        PaidAssignmentConsumer::restrict_current(original, _governors, _governor)
    } else {
        original
    };
    #[cfg(not(test))]
    let result = original;
    let result = if composite::ENABLED.load(Ordering::Acquire) {
        let current = || {
            !runtime::owned()
                || (!fence.is_null()
                    && runtime::call_current(unsafe { ptr::read(fence) }, _governors))
        };
        if !current() {
            original & !0xff
        } else {
            let result = PaidAssignmentConsumer::restrict_current(original, _governors, _governor);
            if current() {
                result
            } else {
                original & !0xff
            }
        }
    } else {
        result
    };
    if composite::ENABLED.load(Ordering::Acquire) {
        composite::record_effective(token, result);
    }
    if let Some(trace) = unsafe { ASSIGN_TRACE_PTR.load(Ordering::Acquire).as_ref() } {
        let token = token & 0xffff_ffff;
        if token > 0 && token <= ASSIGN_CAPACITY as u64 {
            let row = &trace.rows[token as usize - 1];
            row.original_rax.store(original, Ordering::Relaxed);
            row.original_al
                .store((original & 0xff) as u32, Ordering::Relaxed);
            row.sequence.store(1, Ordering::Release);
            runtime::sample_end();
        }
        trace.in_flight.fetch_sub(1, Ordering::Release);
    }
    result
}

// Dedicated four-argument gateway. The fifth callback argument occupies
// [rsp+20h], and its sixth argument at 28h. The 24-byte lifetime fence lives
// at E0h, outside shadow/arguments and saved SIMD. Rows remain observational.
// RSP is 16-byte aligned for all calls. Nonvolatile registers are untouched.
global_asm!(
    ".globl assign_calibration_gateway", ".seh_proc assign_calibration_gateway",
    "assign_calibration_gateway:", "sub rsp, 0x108", ".seh_stackalloc 0x108", ".seh_endprologue",
    "mov [rsp+0x30], rcx", "mov [rsp+0x38], rdx", "mov [rsp+0x40], r8", "mov [rsp+0x48], r9",
    "mov [rsp+0x50], rax", "mov [rsp+0x58], r10", "mov [rsp+0x60], r11",
    "movdqu [rsp+0x80], xmm0", "movdqu [rsp+0x90], xmm1", "movdqu [rsp+0xa0], xmm2",
    "movdqu [rsp+0xb0], xmm3", "movdqu [rsp+0xc0], xmm4", "movdqu [rsp+0xd0], xmm5",
    "mov rax, [rsp+0x108]", "mov [rsp+0x20], rax",
    "lea rax, [rsp+0xe0]", "mov [rsp+0x28], rax", "call {begin}", "mov [rsp+0x68], rax",
    "mov rcx, [rsp+0x30]", "mov rdx, [rsp+0x38]", "mov r8, [rsp+0x40]", "mov r9, [rsp+0x48]",
    "mov rax, [rsp+0x50]", "mov r10, [rsp+0x58]", "mov r11, [rsp+0x60]",
    "movdqu xmm0, [rsp+0x80]", "movdqu xmm1, [rsp+0x90]", "movdqu xmm2, [rsp+0xa0]",
    "movdqu xmm3, [rsp+0xb0]", "movdqu xmm4, [rsp+0xc0]", "movdqu xmm5, [rsp+0xd0]",
    "call qword ptr [rip+ASSIGN_TRAMPOLINE_PTR]", "mov [rsp+0x70], rax",
    "mov rcx, [rsp+0x68]", "mov rdx, rax",
    "mov r8, [rsp+0x30]", "mov r9, [rsp+0x38]",
    "lea rax, [rsp+0xe0]", "mov [rsp+0x20], rax", "call {end}",
    "add rsp, 0x108", "ret", ".seh_endproc",
    begin = sym assign_calibration_begin, end = sym assign_calibration_end,
);
unsafe extern "system" {
    pub(super) fn assign_calibration_gateway();
}

pub(super) unsafe fn install_assign_calibration_at(
    target: *mut u8,
    trace: *mut AssignCalibrationTrace,
    expected: [u8; 8],
) -> Result<(), &'static str> {
    if trace.is_null()
        || unsafe { (*trace).magic } != ASSIGN_MAGIC
        || unsafe { (*trace).version } != AssignCalibrationTrace::VERSION
    {
        return Err("assignment mapping version mismatch");
    }
    if copy_current::<10>(target as u64) != Some(ASSIGN_PROLOGUE) {
        return Err("pinned assignment prologue differs");
    }
    unsafe {
        install_hook(
            target,
            trace.cast(),
            expected,
            BridgeMode::AssignCalibration,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;

    #[unsafe(no_mangle)]
    static ASSIGN_TEST_ENTRY: AtomicPtr<u8> = AtomicPtr::new(null_mut());
    global_asm!(
        ".globl assign_register_fixture",
        ".seh_proc assign_register_fixture",
        "assign_register_fixture:",
        "sub rsp, 0x28",
        ".seh_stackalloc 0x28",
        ".seh_endprologue",
        "mov rax, 0x11223344",
        "mov r10, 0x22334455",
        "mov r11, 0x33445566",
        "pcmpeqd xmm0, xmm0",
        "pcmpeqd xmm1, xmm1",
        "pcmpeqd xmm2, xmm2",
        "pcmpeqd xmm3, xmm3",
        "pcmpeqd xmm4, xmm4",
        "pcmpeqd xmm5, xmm5",
        "call qword ptr [rip+ASSIGN_TEST_ENTRY]",
        "add rsp, 0x28",
        "ret",
        ".seh_endproc",
    );
    unsafe extern "system" {
        fn assign_register_fixture(a: u64, b: u64, c: u64, d: u64) -> u64;
    }

    struct AssignmentCallers {
        running: std::sync::Arc<AtomicBool>,
        workers: Vec<std::thread::JoinHandle<()>>,
    }

    impl AssignmentCallers {
        fn stop_and_join(&mut self) -> Result<(), &'static str> {
            self.running.store(false, Ordering::Release);
            let mut failed = false;
            for worker in self.workers.drain(..) {
                failed |= worker.join().is_err();
            }
            if failed {
                Err("assignment caller panicked")
            } else {
                Ok(())
            }
        }
    }

    impl Drop for AssignmentCallers {
        fn drop(&mut self) {
            // Workers can still call the patched entry while this test unwinds.
            // Collect them before restoring the hook and before the trace drops.
            let _ = self.stop_and_join();
            let _ = unsafe { uninstall() };
        }
    }

    fn spawn_assignment_callers(
        call: extern "system" fn(u64, u64, u64, u64) -> u64,
        running: &std::sync::Arc<AtomicBool>,
        ready_tx: std::sync::mpsc::Sender<()>,
        after_spawn: impl Fn(usize),
    ) -> AssignmentCallers {
        let mut callers = AssignmentCallers {
            running: running.clone(),
            workers: Vec::new(),
        };
        for _ in 0..4 {
            let running = running.clone();
            let ready_tx = ready_tx.clone();
            callers.workers.push(std::thread::spawn(move || {
                let mut own = [0u64; 32];
                let own_ptr = own.as_mut_ptr() as u64;
                assert_eq!(call(own_ptr, 0, 0, 0x9901), 0x9901);
                ready_tx.send(()).unwrap();
                while running.load(Ordering::Acquire) {
                    assert_eq!(call(own_ptr, 0, 0, 0x9901), 0x9901);
                }
            }));
            after_spawn(callers.workers.len());
        }
        callers
    }

    #[test]
    fn assignment_runs_in_an_owned_win64_child() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows::assignment::tests::assignment_synthetic_child",
                "--nocapture",
            ])
            .env("JAC_ASSIGNMENT_SYNTHETIC_CHILD", "1")
            .creation_flags(0x0800_0000)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "owned synthetic child failed: {status}");
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("owned synthetic child exceeded 30 seconds");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn assignment_synthetic_child() {
        if std::env::var("JAC_ASSIGNMENT_SYNTHETIC_CHILD").as_deref() != Ok("1") {
            return;
        }
        let mut image_base = 0;
        let entry = unsafe {
            RtlLookupFunctionEntry(
                assign_calibration_gateway as *const () as u64,
                &mut image_base,
                null_mut(),
            )
        };
        assert!(!entry.is_null());
        let unwind = unsafe {
            ptr::read((image_base + (*entry).Anonymous.UnwindData as u64) as *const [u8; 8])
        };
        assert_eq!(unwind[0] & 7, 1);
        assert_eq!(unwind[5] & 15, 1);
        assert_eq!(u16::from_le_bytes([unwind[6], unwind[7]]), 0x108 / 8);
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
        let mut bytes = ASSIGN_PROLOGUE.to_vec();
        bytes.extend_from_slice(&[
            0x48, 0x89, 0x49, 0x20, // mov [rcx+20],rcx
            0x48, 0x89, 0x51, 0x28, // mov [rcx+28],rdx
            0x4c, 0x89, 0x41, 0x30, // mov [rcx+30],r8
            0x4c, 0x89, 0x49, 0x38, // mov [rcx+38],r9
            0xff, 0x41, 0x78, // inc owned count
            0x48, 0x89, 0x81, 0x80, 0, 0, 0, // mov [rcx+80],rax
            0x4c, 0x89, 0x91, 0x88, 0, 0, 0, // mov [rcx+88],r10
            0x4c, 0x89, 0x99, 0x90, 0, 0, 0, // mov [rcx+90],r11
            0xf3, 0x0f, 0x7f, 0x81, 0xa0, 0, 0, 0, // XMM0..5 before original return
            0xf3, 0x0f, 0x7f, 0x89, 0xb0, 0, 0, 0, 0xf3, 0x0f, 0x7f, 0x91, 0xc0, 0, 0, 0, 0xf3,
            0x0f, 0x7f, 0x99, 0xd0, 0, 0, 0, 0xf3, 0x0f, 0x7f, 0xa1, 0xe0, 0, 0, 0, 0xf3, 0x0f,
            0x7f, 0xa9, 0xf0, 0, 0, 0, 0x4c, 0x89, 0xc8,
            0xc3, // mov rax,r9; ret (full sentinel, AL 0/1)
        ]);
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), page, bytes.len());
        }
        let original: [u8; 8] = bytes[..8].try_into().unwrap();
        let call: extern "system" fn(u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(page) };
        let mut trace = AssignCalibrationTrace::boxed();
        // Changed second instruction, wrong trace version and unaligned entry
        // must refuse before mutation.
        unsafe {
            *page.add(9) = 0x18;
        }
        assert!(unsafe { install_assign_calibration_at(page, trace.as_mut(), original) }.is_err());
        unsafe {
            *page.add(9) = 0x10;
        }
        trace.version = 0;
        assert!(unsafe { install_assign_calibration_at(page, trace.as_mut(), original) }.is_err());
        trace.version = AssignCalibrationTrace::VERSION;
        assert!(
            unsafe { install_assign_calibration_at(page.add(1), trace.as_mut(), original) }
                .is_err()
        );
        assert_eq!(
            unsafe { ptr::read(page.cast::<[u8; 10]>()) },
            ASSIGN_PROLOGUE
        );
        unsafe { install_assign_calibration_at(page, trace.as_mut(), original) }.unwrap();
        let mut governors = [0u64; 32];
        let mut player = [0u8; 0xdc];
        let mut next_player = [0u8; 0xdc];
        player[0xd8..].copy_from_slice(&7u32.to_le_bytes());
        next_player[0xd8..].copy_from_slice(&13u32.to_le_bytes());
        governors[14] = player.as_ptr() as u64;
        let mut governor = [0u32; 32];
        governor[0] = 0x10001;
        governor[1] = 22;
        governor[0x18 / 4] = 7;
        let promotion_flags = [1u8, 0, 0, 1, 0, 0, 0, 1];
        let flag_address = promotion_flags.as_ptr() as u64;
        governor[26] = flag_address as u32;
        governor[27] = (flag_address >> 32) as u32;
        governor[28] = 8;
        governor[30] = 8;
        let member_node = [governor.as_ptr() as u64, 0, 0];
        governors[11] = member_node.as_ptr() as u64;
        let mut city = [0u32; 55];
        city[0xa8 / 4] = 91;
        city[0xc0 / 4] = 17;
        city[0xc4 / 4] = 29;
        city[0xd8 / 4] = 7;
        let this = governors.as_ptr() as u64;
        let gp = governor.as_ptr() as u64;
        let cp = city.as_ptr() as u64;
        ASSIGN_TEST_ENTRY.store(page, Ordering::Release);
        for result in [0xaabb_ccdd_1122_3300, 0xaabb_ccdd_1122_3301] {
            assert_eq!(
                unsafe { assign_register_fixture(this, gp, cp, result) },
                result
            );
            assert_eq!(&governors[4..8], &[this, gp, cp, result]);
            assert_eq!(&governors[16..19], &[0x11223344, 0x22334455, 0x33445566]);
            assert_eq!(&governors[20..32], &[u64::MAX; 12]);
            governors[14] = next_player.as_ptr() as u64;
            governor[0x18 / 4] = 13;
            governor[1] = 23;
            city[0xd8 / 4] = 13;
        }
        governors[14] = 1;
        assert_eq!(call(this, 1, 1, 0x5544_3301), 0x5544_3301);
        assert_eq!(call(this, 0, 0, 0), 0);
        let rows = trace.complete_rows().unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            (
                rows[0].player_id,
                rows[0].governor_index,
                rows[0].city_d8,
                rows[0].read_flags
            ),
            (7, 22, 7, 7)
        );
        assert_eq!(
            (
                rows[1].player_id,
                rows[1].governor_index,
                rows[1].city_d8,
                rows[1].read_flags
            ),
            (13, 23, 13, 7)
        );
        assert_eq!((rows[0].original_al, rows[1].original_al), (0, 1));
        assert_eq!((rows[0].promotion_status, rows[1].promotion_status), (0, 0));
        assert_eq!((rows[0].promotion_owner, rows[1].promotion_owner), (7, 13));
        assert_eq!((rows[0].promotion_index, rows[1].promotion_index), (22, 23));
        assert_eq!((rows[0].promotion_count, rows[1].promotion_count), (8, 8));
        assert_eq!(rows[0].promotion_owned[0], 0b1000_1001);
        assert!(rows[0].promotion_owned[1..].iter().all(|word| *word == 0));
        assert_ne!(rows[2].promotion_status, 0);
        assert_ne!(rows[3].promotion_status, 0);
        assert_eq!(rows[2].read_flags, 0);
        assert_eq!(rows[3].read_flags, 0);
        assert!(rows
            .iter()
            .all(|r| r.thread_id != 0 && r.return_address != 0));
        assert_eq!(governors[15], 4, "original ran exactly once");
        for _ in 0..ASSIGN_CAPACITY {
            assert_eq!(call(this, 0, 0, 0x9901), 0x9901);
        }
        assert_eq!(governors[15], ASSIGN_CAPACITY as u64 + 4);
        assert_eq!(trace.next.load(Ordering::Acquire), ASSIGN_CAPACITY as u32);
        assert_eq!(trace.dropped.load(Ordering::Acquire), 4);
        assert_eq!(trace.in_flight.load(Ordering::Acquire), 0);
        assert_eq!(
            trace.complete_rows().unwrap_err(),
            "assignment capacity exceeded"
        );
        // Restore while four synthetic callers are entering the patched entry.
        let running = std::sync::Arc::new(AtomicBool::new(true));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let mut callers = spawn_assignment_callers(call, &running, ready_tx, |_| {});
        for _ in 0..4 {
            ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        unsafe { uninstall() }.unwrap();
        callers.stop_and_join().unwrap();
        assert_eq!(trace.in_flight.load(Ordering::Acquire), 0);
        assert_eq!(
            unsafe { ptr::read(page.cast::<[u8; 10]>()) },
            ASSIGN_PROLOGUE
        );
        let count = governors[15];
        assert_eq!(call(this, gp, cp, 0x1101), 0x1101);
        assert_eq!(governors[15], count + 1);
        drop(callers);

        // B1 uses the real four-argument gateway in this owned child only.
        // There is no public mode that enables this test switch in a game.
        let mut paid_trace = AssignCalibrationTrace::boxed();
        unsafe { install_assign_calibration_at(page, paid_trace.as_mut(), original) }.unwrap();
        // Use an actually aligned fixture for the Core property receiver.
        let mut paid_player = [0u64; 0xaa];
        paid_player[0xd8 / 8] = 7;
        governors[14] = paid_player.as_ptr() as u64;
        governor[0] = 0x10001;
        governor[1] = 22;
        governor[0x18 / 4] = 7;
        let receiver = (paid_player.as_ptr() as u64).checked_add(0x528).unwrap();
        assert!(product_activate());
        let session = PRODUCT_SESSION.load(Ordering::Acquire);
        product_bind(7, receiver, session, product_next_ticket().unwrap());
        let publish = |key, value| {
            let read = PaidAssignmentConsumer::begin(receiver, key).unwrap();
            PaidAssignmentConsumer::observe(read, 1, 1, value, 0);
        };
        publish(PAID_CLOSE_BASE + 22, 1);
        publish(PAID_ALLOW_BASE + 22, 0);
        publish(PAID_MANIFEST_BASE + 22, 1);
        PAID_SYNTHETIC_GATE.store(true, Ordering::Release);
        let count = governors[15];
        let sentinel = 0xaabb_ccdd_1122_3301;
        assert_eq!(
            unsafe { assign_register_fixture(this, gp, cp, sentinel) },
            sentinel & !0xff
        );
        assert_eq!(governors[15], count + 1, "denied call runs original once");
        assert_eq!(&governors[4..8], &[this, gp, cp, sentinel]);
        assert_eq!(&governors[16..19], &[0x11223344, 0x22334455, 0x33445566]);
        assert_eq!(&governors[20..32], &[u64::MAX; 12]);
        publish(PAID_ALLOW_BASE + 22, 1);
        assert_eq!(call(this, gp, cp, sentinel), sentinel);
        assert_eq!(call(this, gp, cp, sentinel & !0xff), sentinel & !0xff);
        unsafe { ptr::write_volatile(governor.as_mut_ptr().add(1), 23) };
        assert_eq!(
            call(this, gp, cp, sentinel),
            sentinel,
            "ordinary governor preserves original"
        );
        assert_eq!(
            paid_trace.rows[3].governor_index.load(Ordering::Acquire),
            23
        );
        assert_eq!(
            paid_trace.rows[3].promotions.status.load(Ordering::Acquire),
            0
        );
        unsafe { ptr::write_volatile(governor.as_mut_ptr().add(30), 0) };
        assert_eq!(
            call(this, gp, cp, sentinel),
            sentinel,
            "ordinary governor needs no persona promotion layout"
        );
        assert_eq!(
            paid_trace.rows[4].governor_index.load(Ordering::Acquire),
            23
        );
        assert_ne!(
            paid_trace.rows[4].promotions.status.load(Ordering::Acquire),
            0
        );
        unsafe { ptr::write_volatile(governor.as_mut_ptr().add(30), 8) };
        unsafe { ptr::write_volatile(governor.as_mut_ptr().add(1), 22) };
        publish(PAID_CLOSE_BASE + 22, 1);
        assert_eq!(call(this, gp, cp, sentinel), sentinel & !0xff);
        // Capacity exhaustion cannot disable the restrictive decision.
        for _ in 0..ASSIGN_CAPACITY {
            assert_eq!(call(this, gp, cp, sentinel), sentinel & !0xff);
        }
        assert!(paid_trace.dropped.load(Ordering::Acquire) > 0);
        PAID_SYNTHETIC_GATE.store(false, Ordering::Release);
        unsafe { uninstall() }.unwrap();
        assert_eq!(call(this, gp, cp, sentinel), sentinel);

        let mut panic_trace = AssignCalibrationTrace::boxed();
        unsafe { install_assign_calibration_at(page, panic_trace.as_mut(), original) }.unwrap();
        let running = std::sync::Arc::new(AtomicBool::new(true));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = spawn_assignment_callers(call, &running, ready_tx, |spawned| {
                if spawned == 2 {
                    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    panic!("injected failure before all assignment callers are spawned");
                }
            });
        }));
        assert!(panic.is_err());
        assert!(!running.load(Ordering::Acquire));
        assert_eq!(std::sync::Arc::strong_count(&running), 1);
        assert_eq!(panic_trace.in_flight.load(Ordering::Acquire), 0);
        assert_eq!(
            unsafe { ptr::read(page.cast::<[u8; 10]>()) },
            ASSIGN_PROLOGUE
        );
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }
}
