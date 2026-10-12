//! Two-wrapper opt-in capture, sharing the root gateway/CAS/stop machinery.
//! Snapshot addresses are opaque scalars. No job pointer is followed at exit.
use super::*;
use civ6_bootstrap_rs::protocol::{SUBMISSION_BYTES, SUBMISSION_MAGIC, SUBMISSION_ROW};
pub(super) static PROFILE: AtomicBool = AtomicBool::new(false);
pub(super) fn active() -> bool {
    PROFILE.load(Ordering::Acquire)
}
pub(super) const WORDS: [[u8; 8]; 3] = [
    [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x6c],
    [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b],
    [0; 8],
];
#[repr(C)]
pub struct SubmissionTrace {
    pub(super) header: [AtomicU64; 32],
    pub(super) sites: [[AtomicU64; 8]; 3],
    pub(super) rows: [[AtomicU64; 35]; ROOT_ROWS],
}
pub(super) static TRACE: SubmissionTrace = SubmissionTrace {
    header: [const { AtomicU64::new(0) }; 32],
    sites: [const { [const { AtomicU64::new(0) }; 8] }; 3],
    rows: [const { [const { AtomicU64::new(0) }; 35] }; ROOT_ROWS],
};
const _: () = assert!(std::mem::size_of::<SubmissionTrace>() == SUBMISSION_BYTES);
#[unsafe(no_mangle)]
pub static BRIDGE_SUBMISSION_OBSERVATION_TRACE: &SubmissionTrace = &TRACE;
#[unsafe(no_mangle)]
pub static BRIDGE_SUBMISSION_OBSERVATION_TRACE_SIZE: u64 = SUBMISSION_BYTES as u64;
pub(super) fn initialize(config: RootConfig, core: usize, exe: usize) {
    for (i, v) in [
        SUBMISSION_MAGIC,
        3,
        SUBMISSION_BYTES as u64,
        ROOT_ROWS as u64,
        SUBMISSION_ROW as u64,
        u64::from(config.pid),
        config.created_filetime,
        core as u64,
        exe as u64,
        1,
    ]
    .into_iter()
    .enumerate()
    {
        h(i).store(v, Ordering::Release);
    }
    h(19).store(1, Ordering::Release);
    for (i, words) in WORDS.iter().enumerate().take(2) {
        site_row(i)[0].store(site_rvas()[i] as u64, Ordering::Release);
        site_row(i)[1].store(u64::from_le_bytes(*words), Ordering::Release);
    }
}
fn commit(index: usize, mut row: [u64; 35]) {
    row[23] = h(24).fetch_add(1, Ordering::AcqRel) + 1;
    for (dst, v) in TRACE.rows[index].iter().zip(row) {
        dst.store(v, Ordering::Relaxed);
    }
    TRACE.rows[index][0].store(index as u64 + 1, Ordering::Release);
}
// Preserve the exact copied prefix and NT/Win32 read result. Unread bytes in
// the POD padding are never labelled captured. No retry follows a failed read.
fn read_once<const N: usize>(address: u64) -> ([u8; N], usize, u32, u64) {
    let mut bytes = [0; N];
    let mut copied = 0;
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            address as *const c_void,
            bytes.as_mut_ptr().cast(),
            N,
            &mut copied,
        )
    };
    let error = if ok == 0 {
        unsafe { GetLastError() }
    } else {
        0
    };
    (bytes, copied, error, ok as u32 as u64)
}
fn entry_snapshot(saved: &Saved, inv: Invocation, error: u32) -> [u64; 35] {
    let mut r = [0; 35];
    r[1] = inv.kind;
    r[2] = 1;
    r[3] = inv.token;
    r[4] = inv.parent;
    r[5] = inv.tid;
    r[6] = inv.thread_created;
    r[7] = inv.epoch;
    r[8] = inv.caller;
    r[9] = inv.caller_rsp;
    r[10..14].copy_from_slice(&saved.gpr[..4]);
    r[20] = inv.errors;
    r[21] = saved.gpr[4];
    r[22] = u64::from(error);
    r[29] = inv.native_rsp;
    if inv.kind == 4
        && BIND
            .get()
            .and_then(|b| (b.core as u64).checked_add(0x4a114a))
            == Some(inv.caller)
    {
        let slot = inv.caller_rsp.checked_add(0x40).unwrap_or(0);
        let (bytes, n, e, ok) = read_once::<8>(slot);
        r[32] = u64::from_le_bytes(bytes);
        r[33] = (u64::from(e) << 32) | (u64::from(ok != 0) << 8) | n.min(255) as u64;
        if n != 8 || ok == 0 || e != 0 {
            r[20] |= 0x200;
        }
    }
    if inv.kind == 4 {
        let address = saved.gpr[1];
        let wanted = if committed_range(address as usize, 32, 1, false) {
            32
        } else {
            16
        };
        let (bytes, n, e, ok) = if wanted == 32 {
            read_once::<32>(address)
        } else {
            let (prefix, n, e, ok) = read_once::<16>(address);
            let mut bytes = [0; 32];
            bytes[..16].copy_from_slice(&prefix);
            (bytes, n, e, ok)
        };
        for j in 0..4 {
            r[14 + j] = u64::from_le_bytes(bytes[j * 8..j * 8 + 8].try_into().unwrap());
        }
        r[24] = n as u64;
        r[25] = u64::from(e);
        r[30] = ok;
        if ok == 0 || n != wanted || e != 0 {
            r[20] |= 1;
        }
        let (m, n, e, ok) = read_once::<16>(saved.gpr[3]);
        r[18] = u64::from_le_bytes(m[..8].try_into().unwrap());
        r[19] = u64::from_le_bytes(m[8..].try_into().unwrap());
        r[26] = n as u64;
        r[27] = u64::from(e);
        r[31] = ok;
        if ok == 0 || n != 16 || e != 0 {
            r[20] |= 2;
        }
    }
    r
}
pub(super) extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let saved = unsafe { &mut *raw };
        let Some(bind) = BIND.get() else { return };
        if !matches!(saved.invocation.kind, 4 | 5) {
            return;
        }
        if Instant::now() >= bind.until {
            h(12).fetch_or(ROOT_CLOSED, Ordering::AcqRel);
        }
        if !acquire(false) {
            return;
        }
        let Ok(row) = h(10).fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n <= (ROOT_ROWS - 2) as u64).then_some(n + 2)
        }) else {
            h(11).fetch_add(1, Ordering::Relaxed);
            h(12).fetch_sub(1, Ordering::Release);
            return;
        };
        let inv = &mut saved.invocation;
        inv.cookie = COOKIE;
        inv.token = row + 1;
        inv.epoch = h(9).load(Ordering::Acquire);
        if let Some((tid, ft)) = thread_key() {
            inv.tid = tid;
            inv.thread_created = ft;
        } else {
            inv.errors |= 4;
        }
        if h(19).load(Ordering::Acquire) != 3 || bind.core as u64 != h(7).load(Ordering::Acquire) {
            inv.errors |= 8;
        }
        let mut stack = STACK.with(Cell::get);
        if stack.depth > 0 {
            inv.parent = stack.frames[stack.depth - 1].token;
            inv.errors |= 0x100;
        }
        if stack.depth == DEPTH {
            inv.errors |= 0x10;
        }
        let mut r = entry_snapshot(saved, saved.invocation, error);
        saved.invocation.errors = r[20];
        if r[20] != 0 {
            unknown();
        }
        if stack.depth < DEPTH {
            stack.frames[stack.depth] = saved.invocation;
            stack.depth += 1;
            STACK.with(|s| s.set(stack));
        }
        // No pointer-bearing external object is stored in STACK. Only the
        // assembly's current call frame still holds native input registers.
        r[20] = saved.invocation.errors;
        commit(row as usize, r);
    }));
    if outcome.is_err() {
        unknown();
        unsafe {
            (*raw).invocation.errors |= 0x20;
        }
    }
    unsafe { SetLastError(error) };
}
pub(super) fn finish(raw: &mut Saved, edge: u64, error: u32) {
    let mut inv = raw.invocation;
    if inv.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0;
    let _release = Release;
    let _ = STACK.try_with(|s| {
        let mut stack = s.get();
        if stack.depth > 0 && stack.frames[stack.depth - 1].token == inv.token {
            stack.depth -= 1;
            stack.frames[stack.depth] = Invocation::default();
            s.set(stack);
        } else if inv.errors & 0x10 == 0 {
            inv.errors |= 0x20;
            unknown();
        }
    });
    if thread_key() != Some((inv.tid, inv.thread_created)) {
        inv.errors |= 0x20;
        unknown();
    }
    if edge == 3 {
        inv.errors |= 0x40;
        h(13).fetch_add(1, Ordering::Relaxed);
    }
    // Clone the already copied entry row; NEVER reread RDX, R9, set or list.
    let index = inv.token as usize - 1;
    let mut r = [0; 35];
    for (dst, src) in r.iter_mut().zip(&TRACE.rows[index]) {
        *dst = src.load(Ordering::Acquire);
    }
    r[2] = edge;
    r[20] |= inv.errors;
    r[21] = if edge == 3 { 0 } else { raw.gpr[4] };
    r[22] = u64::from(error);
    r[28] = if edge == 3 { 0 } else { raw.gpr[1] };
    r[34] = if edge == 2 { raw.gpr[2] } else { 0 }; // R8 captured BEFORE end helper; no heap reread
    commit(index + 1, r);
}

global_asm!(
    ".globl submission_push_original",
    ".seh_proc submission_push_original",
    "submission_push_original:",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    ".seh_endprologue",
    ".globl submission_push_original_body",
    "submission_push_original_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+ROOT_CONTINUE]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    ".globl submission_push_original_branch",
    "submission_push_original_branch:",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl submission_publish_original",
    ".seh_proc submission_publish_original",
    "submission_publish_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    ".globl submission_publish_original_body",
    "submission_publish_original_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+ROOT_CONTINUE+8]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    ".globl submission_publish_original_branch",
    "submission_publish_original_branch:",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
);
unsafe extern "system" {
    pub(super) fn submission_push_original();
    pub(super) fn submission_publish_original();
    fn submission_push_gateway();
    fn submission_publish_gateway();
}
#[cfg(not(test))]
gateway!("submission_push_gateway", "4", submission_push_original);
#[cfg(not(test))]
gateway!(
    "submission_publish_gateway",
    "5",
    submission_publish_original
);
#[cfg(test)]
#[unsafe(no_mangle)]
pub(super) static SUBMISSION_TEST_ORIGINALS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
#[cfg(test)]
global_asm!(
    ".globl submission_test_push",
    "submission_test_push:",
    "jmp qword ptr [rip+SUBMISSION_TEST_ORIGINALS]",
    ".globl submission_test_publish",
    "submission_test_publish:",
    "jmp qword ptr [rip+SUBMISSION_TEST_ORIGINALS+8]",
);
#[cfg(test)]
unsafe extern "system" {
    fn submission_test_push();
    fn submission_test_publish();
}
#[cfg(test)]
gateway!("submission_push_gateway", "4", submission_test_push);
#[cfg(test)]
gateway!("submission_publish_gateway", "5", submission_test_publish);
pub(super) fn gateways() -> [usize; 3] {
    [
        submission_push_gateway as *const () as usize,
        submission_publish_gateway as *const () as usize,
        0,
    ]
}

fn disk(file: &mut File, rva: usize, len: usize) -> Result<Vec<u8>, &'static str> {
    if len > 0x100000 {
        return Err("submission disk range cap");
    }
    let mut headers = [0u8; 4096];
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "submission seek")?;
    file.read_exact(&mut headers)
        .map_err(|_| "submission headers")?;
    let u16at = |p: usize| {
        headers
            .get(p..p + 2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
    };
    let u32at = |p: usize| {
        headers
            .get(p..p + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
    };
    let pe = u32at(0x3c).ok_or("submission DOS")?;
    if headers.get(pe..pe + 4) != Some(b"PE\0\0") || u16at(pe + 24) != Some(0x20b) {
        return Err("submission PE64");
    }
    let count = u16at(pe + 6).ok_or("submission sections")? as usize;
    let start = pe + 24 + u16at(pe + 20).ok_or("submission optional header")? as usize;
    let end = rva.checked_add(len).ok_or("submission RVA overflow")?;
    for i in 0..count {
        let p = start + i * 40;
        let va = u32at(p + 12).ok_or("submission section RVA")?;
        let size = u32at(p + 16).ok_or("submission raw size")?;
        if rva >= va && end <= va.checked_add(size).ok_or("submission section end")? {
            let raw = u32at(p + 20).ok_or("submission section raw")?;
            let offset = raw.checked_add(rva - va).ok_or("submission raw overflow")?;
            let mut bytes = vec![0; len];
            file.seek(SeekFrom::Start(offset as u64))
                .map_err(|_| "submission raw seek")?;
            file.read_exact(&mut bytes)
                .map_err(|_| "submission raw read")?;
            return Ok(bytes);
        }
    }
    Err("submission RVA outside raw section")
}
const CALLER: &[u8] = &[
    0x49, 0x8b, 0x8d, 0x50, 1, 0, 0, 0x4c, 0x8d, 0x4c, 0x24, 0x30, 0x4d, 0x8d, 0x85, 0x40, 1, 0, 0,
    0x66, 0x0f, 0x7f, 0x74, 0x24, 0x30, 0x48, 0x8b, 0xd3, 0xe8, 0x9f, 0x99, 0x60, 0, 0x48, 0x83,
    0xc3, 0x50, 0x48, 0x3b, 0xdf, 0x75, 0xd6, 0x0f, 0x28, 0xb4, 0x24, 0x60, 1, 0, 0, 0x49, 0x8b,
    0x8d, 0x50, 1, 0, 0, 0xe8, 0xf2, 0x99, 0x60, 0,
];
// Same three complete spans as ai_operation/task_audit.rs MANAGER_FLOW_BLOCKS.
// No shared-global pointer is followed; only instructions/rel32 are checked.
const MANAGER_BLOCKS: &[(usize, &[u8])] = &[
    (
        0x4a7a27,
        &[
            0x48, 0x8b, 0x05, 0x3a, 0x30, 0x6e, 0, 0x48, 0x8d, 0x96, 0xd0, 0, 0, 0, 0x48, 0x89,
            0x5c, 0x24, 0x38, 0x4c, 0x8d, 0x05, 0xdf, 0xee, 0x55, 0, 0x48, 0x8b, 0x98, 0x50, 2, 0,
            0, 0x48, 0x8b, 0xcb, 0xe8, 0xb0, 0x96, 0xff, 0xff, 0x48, 0x8b, 0xcb, 0xe8, 0x58, 0x96,
            0xff, 0xff,
        ],
    ),
    (
        0x4a1100,
        &[
            0x48, 0x83, 0xec, 0x38, 0x4d, 0x85, 0xc0, 0xc7, 0x44, 0x24, 0x28, 0, 0, 0, 0, 0xc7,
            0x44, 0x24, 0x2c, 0x1f, 0, 0, 0, 0x48, 0x8d, 0x05, 0xa2, 0x4e, 0x52, 0, 0x49, 0x0f,
            0x45, 0xc0, 0x4c, 0x8d, 0x4c, 0x24, 0x20, 0x48, 0x89, 0x44, 0x24, 0x20, 0x4c, 0x8d,
            0x81, 0, 4, 0, 0, 0x0f, 0x28, 0x44, 0x24, 0x20, 0x48, 0x8b, 0x89, 0x10, 4, 0, 0, 0x66,
            0x0f, 0x7f, 0x44, 0x24, 0x20, 0xe8, 0xc6, 0x10, 0x4f, 0, 0x48, 0x83, 0xc4, 0x38, 0xc3,
        ],
    ),
    (
        0x4a10b0,
        &[
            0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0xb9, 0xac, 0x38, 0, 0, 0, 0x48, 0x8b, 0xd9,
            0x75, 0x2b, 0x48, 0x8b, 0x0d, 0xb7, 0x99, 0x6e, 0, 0x48, 0x8b, 0x01, 0xff, 0x50, 0x30,
            0x88, 0x83, 0xad, 0x38, 0, 0, 0x84, 0xc0, 0x74, 0x0d, 0x48, 0x8b, 0x0d, 0xa0, 0x99,
            0x6e, 0, 0x48, 0x8b, 0x01, 0xff, 0x50, 0x38, 0xc6, 0x83, 0xac, 0x38, 0, 0, 1, 0x48,
            0x8b, 0x8b, 0x10, 4, 0, 0, 0x48, 0x83, 0xc4, 0x20, 0x5b, 0xe9, 0x82, 0x11, 0x4f, 0,
        ],
    ),
];
fn check_span(file: &mut File, core: usize, rva: usize, bytes: &[u8]) -> Result<(), &'static str> {
    if disk(file, rva, bytes.len())? != bytes {
        return Err("manager source complete span differs");
    }
    for (n, b) in bytes.iter().enumerate() {
        if copy_current::<1>((core + rva + n) as u64) != Some([*b]) {
            return Err("manager live complete span differs");
        }
    }
    Ok(())
}
fn check_unwind(
    file: &mut File,
    core: usize,
    pc: usize,
    chain: &[(u32, u32, u32, &[u8])],
) -> Result<(), &'static str> {
    use windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;
    let mut image = 0;
    let entry = unsafe { RtlLookupFunctionEntry((core + pc) as u64, &mut image, null_mut()) };
    if entry.is_null() || image != core as u64 {
        return Err("manager live pdata missing");
    }
    let mut actual = copy_current::<12>(entry as u64).ok_or("manager pdata read")?;
    let entry_rva = (entry as usize)
        .checked_sub(core)
        .ok_or("manager pdata outside source")?;
    if disk(file, entry_rva, 12)? != actual {
        return Err("manager pdata source differs");
    }
    for (depth, &(start, end, unwind, expected)) in chain.iter().enumerate() {
        let mut encoded = [0u8; 12];
        encoded[..4].copy_from_slice(&start.to_le_bytes());
        encoded[4..8].copy_from_slice(&end.to_le_bytes());
        encoded[8..].copy_from_slice(&unwind.to_le_bytes());
        if actual != encoded {
            return Err("manager full unwind ownership chain differs");
        }
        check_span(file, core, unwind as usize, expected)?;
        let prologue_len = usize::from(expected[1]);
        let prologue = disk(file, start as usize, prologue_len)?;
        check_span(file, core, start as usize, &prologue)?;
        let chained = expected[0] & 0x20 != 0;
        if chained != (depth + 1 < chain.len()) {
            return Err("manager unwind chain termination differs");
        }
        if chained {
            let offset = 4 + usize::from(expected[2]).div_ceil(2) * 4;
            actual = copy_current::<12>((core + unwind as usize + offset) as u64)
                .ok_or("manager chained entry read")?;
        }
    }
    Ok(())
}
fn verify_manager(file: &mut File, core: usize) -> Result<(), &'static str> {
    for &(rva, bytes) in MANAGER_BLOCKS {
        check_span(file, core, rva, bytes)?;
    }
    for (rva, op, target) in [
        (0x4a7a4busize, 0xe8, 0x4a1100),
        (0x4a7a53, 0xe8, 0x4a10b0),
        (0x4a1145, 0xe8, 0x992210),
        (0x4a10f9, 0xe9, 0x992280),
    ] {
        let bytes = disk(file, rva, 5)?;
        if bytes[0] != op
            || (rva + 5)
                .checked_add_signed(i32::from_le_bytes(bytes[1..].try_into().unwrap()) as isize)
                != Some(target)
        {
            return Err("manager computed direct call/tail differs");
        }
    }
    // Metadata selected by nonzero incoming R8 at the producer. No getter call.
    let bytes = disk(file, 0x4a7a3a, 7)?;
    if (0x4a7a41usize)
        .checked_add_signed(i32::from_le_bytes(bytes[3..].try_into().unwrap()) as isize)
        != Some(0xa06920)
    {
        return Err("manager metadata RIP target differs");
    }
    check_unwind(
        file,
        core,
        0x4a1100,
        &[(0x4a1100, 0x4a114f, 0xb578e8, &[1, 4, 1, 0, 4, 0x62, 0, 0])],
    )?;
    check_unwind(
        file,
        core,
        0x4a10b0,
        &[(
            0x4a10b0,
            0x4a10fe,
            0xb45024,
            &[1, 6, 2, 0, 6, 0x32, 2, 0x30],
        )],
    )?;
    let owner: (u32, u32, u32, &[u8]) = (
        0x4a79b0,
        0x4a7a35,
        0xacedf8,
        &[1, 0xa, 4, 0, 0xa, 0x64, 8, 0, 0xa, 0x32, 6, 0x70],
    );
    check_unwind(file, core, 0x4a7a27, &[owner])?;
    check_unwind(
        file,
        core,
        0x4a7a4b,
        &[
            (
                0x4a7a35,
                0x4a7aa6,
                0xb09cac,
                &[
                    0x21, 5, 2, 0, 5, 0x34, 7, 0, 0xb0, 0x79, 0x4a, 0, 0x35, 0x7a, 0x4a, 0, 0xf8,
                    0xed, 0xac, 0,
                ],
            ),
            owner,
        ],
    )?;
    // Checked sub38 + CALL8 => ancestor slot +40 / metadata +28; the
    // complete Spawn add20/popRBX/JMP removes its frame before992280 entry.
    check_span(
        file,
        core,
        0x4a79b0,
        &[0x48, 0x89, 0x74, 0x24, 0x18, 0x57, 0x48, 0x83, 0xec, 0x20],
    )?;
    h(28).store(1, Ordering::Release);
    Ok(())
}
pub(super) fn verify(file: &mut File, core: usize) -> Result<(), &'static str> {
    use windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;
    for (i, (rva, len)) in [(0x992210usize, 0x6c), (0x992280, 0x33)]
        .into_iter()
        .enumerate()
    {
        let mut base = 0;
        let p = unsafe { RtlLookupFunctionEntry((core + rva) as u64, &mut base, null_mut()) };
        if p.is_null() || base != core as u64 {
            return Err(site_error(i, 30, "submission pdata absent"));
        }
        let entry = copy_current::<12>(p as u64).ok_or("submission pdata unreadable")?;
        if u32::from_le_bytes(entry[..4].try_into().unwrap()) as usize != rva
            || u32::from_le_bytes(entry[4..8].try_into().unwrap()) as usize != rva + len
        {
            return Err(site_error(i, 31, "submission exact function range differs"));
        }
        let pdata_rva = (p as usize)
            .checked_sub(core)
            .ok_or("submission pdata source")?;
        if disk(file, pdata_rva, 12)? != entry {
            return Err(site_error(i, 32, "submission live pdata differs"));
        }
        let unwind = u32::from_le_bytes(entry[8..].try_into().unwrap()) as usize;
        let head = copy_current::<4>((core + unwind) as u64).ok_or("submission unwind header")?;
        if head[0] != 1 || head[2] > 32 {
            return Err(site_error(
                i,
                33,
                "submission unsupported unwind/chained shape",
            ));
        }
        let bytes = disk(file, unwind, 4 + usize::from(head[2]).div_ceil(2) * 4)?;
        for (n, b) in bytes.into_iter().enumerate() {
            if copy_current::<1>((core + unwind + n) as u64) != Some([b]) {
                return Err(site_error(i, 34, "submission unwind bytes differ"));
            }
        }
        h(25).fetch_or(1 << i, Ordering::Release);
        h(26).fetch_or(1 << i, Ordering::Release); // caller install verified complete function bytes
    }
    if disk(file, 0x388850, CALLER.len())? != CALLER {
        return Err("submission source caller differs");
    }
    for (n, b) in CALLER.iter().enumerate() {
        if copy_current::<1>((core + 0x388850 + n) as u64) != Some([*b]) {
            return Err("submission complete live caller differs");
        }
    }
    for (call, target) in [(0x38886cusize, 0x992210), (0x388889, 0x992280)] {
        let bytes = &CALLER[call - 0x388850..call - 0x388850 + 5];
        if bytes[0] != 0xe8
            || (call + 5)
                .checked_add_signed(i32::from_le_bytes(bytes[1..].try_into().unwrap()) as isize)
                != Some(target)
        {
            return Err("submission computed direct-call target differs");
        }
    }
    h(27).store(1, Ordering::Release);
    verify_manager(file, core).map_err(|e| site_error(0, 35, e))?;
    Ok(())
}
pub(super) fn verify_r8_flow(
    core_file: &mut File,
    exe_file: &mut File,
    core: usize,
    exe: usize,
) -> Result<(), &'static str> {
    for (file, base, rva, length) in [
        (&mut *core_file, core, 0x992210, 0x6c),
        (&mut *exe_file, exe, 0x985b20, 0xca),
    ] {
        let source = disk(file, rva, length)?;
        for (n, b) in source.iter().enumerate() {
            if copy_current::<1>((base + rva + n) as u64) != Some([*b]) {
                return Err("complete R8 forwarding function differs from pinned source");
            }
        }
    }
    // Read-only fixed return suffixes: output R8 is not changed after T creation.
    for (base, rva, bytes) in [
        (
            exe,
            0x985b7e,
            &[0x4c, 0x8b, 0xc0, 0x48, 0x85, 0xc0, 0x74, 0x14][..],
        ),
        (exe, 0x985b94, &[0x49, 0x89, 0x78, 8, 0xeb, 3][..]),
        (exe, 0x986077, &[0x48, 0x83, 0xc4, 0x38, 0xc3][..]),
        (
            core,
            0x992267,
            &[
                0x48, 0x8b, 0x5c, 0x24, 0x40, 0x48, 0x8b, 0x6c, 0x24, 0x48, 0x48, 0x8b, 0x74, 0x24,
                0x50, 0x48, 0x83, 0xc4, 0x30, 0x5f, 0xc3,
            ][..],
        ),
    ] {
        for (n, b) in bytes.iter().enumerate() {
            if copy_current::<1>((base + rva + n) as u64) != Some([*b]) {
                return Err("fixed R8 return continuation differs");
            }
        }
    }
    let prefix = disk(exe_file, 0x986060, 0x1c)?;
    for (n, b) in prefix.iter().enumerate() {
        if copy_current::<1>((exe + 0x986060 + n) as u64) != Some([*b]) {
            return Err("complete host push export differs");
        }
    }
    if prefix[18] != 0xe8
        || (0x986077usize)
            .checked_add_signed(i32::from_le_bytes(prefix[19..23].try_into().unwrap()) as isize)
            != Some(0x985b20)
    {
        return Err("R8 host direct-call target differs");
    }
    h(29).store(1, Ordering::Release);
    Ok(())
}
