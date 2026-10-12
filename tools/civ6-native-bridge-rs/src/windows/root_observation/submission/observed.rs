//! Governor-only observed call correspondence. No World permit or task ownership.
use super::*;
use civ6_bootstrap_rs::protocol::{SUBMISSION_BIRTHS, SUBMISSION_HEADER};
pub(super) const EXTRA: [(usize, bool, [u8; 8], usize); 4] = [
    (
        0x4a79b0,
        false,
        [0x48, 0x89, 0x74, 0x24, 0x18, 0x57, 0x48, 0x83],
        5,
    ),
    (
        0x985ec0,
        true,
        [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b],
        6,
    ),
    (
        0x4a8530,
        false,
        [0x40, 0x57, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b],
        6,
    ),
    (
        0x986060,
        true,
        [0x48, 0x83, 0xec, 0x38, 0x41, 0x0f, 0x10, 0x01],
        8,
    ),
];
#[repr(C)]
pub struct ObservedTrace {
    pub(in super::super) header: [AtomicU64; 32],
    pub(in super::super) sites: [[AtomicU64; 8]; 6],
    pub(in super::super) rows: [[AtomicU64; 48]; ROOT_ROWS],
    births: [[AtomicU64; 18]; SUBMISSION_BIRTHS],
}
pub(in super::super) static TRACE: ObservedTrace = ObservedTrace {
    header: [const { AtomicU64::new(0) }; 32],
    sites: [const { [const { AtomicU64::new(0) }; 8] }; 6],
    rows: [const { [const { AtomicU64::new(0) }; 48] }; ROOT_ROWS],
    births: [const { [const { AtomicU64::new(0) }; 18] }; SUBMISSION_BIRTHS],
};
const _: () = assert!(std::mem::size_of::<ObservedTrace>() == SUBMISSION_BYTES);
const _: () = assert!(std::mem::offset_of!(ObservedTrace, rows) == SUBMISSION_HEADER);
const BORN: u64 = 1;
const SUBMITTED: u64 = 2;
const BOUND: u64 = 4;
const PUBLISHED: u64 = 8;
const PRODUCER_RETURN: u64 = 16;
const HOST_START: u64 = 32;
const CORE_START: u64 = 64;
const CORE_RETURN: u64 = 128;
const HOST_RETURN: u64 = 256;
const EXCEPTION: u64 = 512;
const EARLY: u64 = 1024;
const REUSED: u64 = 2048;
const NO_SUBMIT: u64 = 4096;
const HOST_PUSH_START: u64 = 8192;
const HOST_PUSH_RETURN: u64 = 16384;
#[derive(Clone, Copy)]
struct Birth {
    w: [u64; 18],
}
static BOOK: Mutex<[Birth; SUBMISSION_BIRTHS]> =
    Mutex::new([Birth { w: [0; 18] }; SUBMISSION_BIRTHS]);
fn publish(index: usize, b: Birth) {
    for (dst, v) in TRACE.births[index].iter().zip(b.w) {
        dst.store(v, Ordering::Release);
    }
}
fn exclude() {
    h(30).fetch_add(1, Ordering::Relaxed);
}
fn fail(inv: &mut Invocation, bit: u64) {
    inv.errors |= bit;
}
fn valid_parent(p: Invocation, kind: u64, inv: &Invocation) -> bool {
    p.cookie == COOKIE
        && p.kind == kind
        && p.tid == inv.tid
        && p.thread_created == inv.thread_created
        && p.epoch == inv.epoch
}
pub(super) extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let saved = unsafe { &mut *raw };
        let Some(bind) = BIND.get() else { return };
        let kind = saved.invocation.kind;
        let Some((tid, ft)) = thread_key() else {
            unknown();
            return;
        };
        saved.invocation.tid = tid;
        saved.invocation.thread_created = ft;
        saved.invocation.epoch = h(9).load(Ordering::Acquire);
        let stack = STACK.with(Cell::get);
        let parent = stack.depth.checked_sub(1).map(|n| stack.frames[n]);
        let mut book = BOOK
            .lock()
            .map_err(|_| ())
            .unwrap_or_else(|_| panic!("observed book poisoned"));
        let mut snapshot = None;
        let mut index = None;
        let mut inherited = false;
        if kind == 6 {
            // ID is reserved at this function entry BEFORE any receiver pointer fetch.
            // P is only a scalar offset here. It is not a World/ownership assertion.
        } else if kind == 4 || kind == 5 {
            let expected = if kind == 4 { 0x4a114a } else { 0x4a7a58 };
            let Some(p) = parent.filter(|p| valid_parent(*p, 6, &saved.invocation)) else {
                exclude();
                return;
            };
            if saved.invocation.caller != bind.core as u64 + expected
                || saved
                    .invocation
                    .caller_rsp
                    .checked_add(if kind == 4 { 0x70 } else { 0x30 })
                    != Some(p.native_rsp)
            {
                exclude();
                return;
            }
            index = book.iter().position(|b| b.w[0] == p.world);
            inherited = true;
            saved.invocation.parent = p.token;
            if kind == 4 && saved.gpr[1] != p.manager {
                fail(&mut saved.invocation, 0x400);
            }
        } else if kind == 9 {
            let Some(p) = parent.filter(|p| valid_parent(*p, 4, &saved.invocation)) else {
                exclude();
                return;
            };
            if saved.invocation.caller != bind.core as u64 + 0x992267
                || saved.invocation.caller_rsp.checked_add(0x40) != Some(p.native_rsp)
                || saved.gpr[1] != p.manager
            {
                exclude();
                return;
            }
            index = book.iter().position(|b| b.w[0] == p.world);
            inherited = true;
            saved.invocation.parent = p.token;
        } else if kind == 7 {
            // One read of the CURRENT native input T, not a historical P read.
            let (bytes, n, e, ok) = read_once::<16>(saved.gpr[0]);
            snapshot = Some((bytes.to_vec(), n, e, ok));
            let p = if n == 16 && ok != 0 {
                u64::from_le_bytes(bytes[8..].try_into().unwrap())
            } else {
                0
            };
            index = book
                .iter()
                .position(|b| b.w[2] != 0 && b.w[2] == saved.gpr[0])
                .or_else(|| book.iter().position(|b| b.w[1] != 0 && b.w[1] == p));
            if index.is_none() {
                exclude();
                if n != 16 || ok == 0 {
                    h(31).fetch_add(1, Ordering::Relaxed);
                }
                return;
            }
        } else if kind == 8 {
            index = book
                .iter()
                .position(|b| b.w[1] != 0 && b.w[1] == saved.gpr[0]);
            if index.is_none() {
                exclude();
                return;
            }
            if let Some(p) = parent.filter(|p| valid_parent(*p, 7, &saved.invocation)) {
                inherited = true;
                saved.invocation.parent = p.token;
                if saved.invocation.caller != h(8).load(Ordering::Acquire) + 0x985ef4
                    || saved.invocation.caller_rsp.checked_add(0x30) != Some(p.native_rsp)
                {
                    fail(&mut saved.invocation, 0x800);
                }
            } else {
                fail(&mut saved.invocation, EARLY);
            }
            let (bytes, n, e, ok) = read_once::<64>(saved.gpr[0]);
            snapshot = Some((bytes.to_vec(), n, e, ok));
        } else {
            exclude();
            return;
        }
        if !inherited && Instant::now() >= bind.until {
            h(12).fetch_or(ROOT_CLOSED, Ordering::AcqRel);
        }
        if !acquire(inherited) {
            exclude();
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
        if h(19).load(Ordering::Acquire) != 3 || h(15).load(Ordering::Acquire) != 63 {
            fail(inv, 8);
            h(22).fetch_add(1, Ordering::Relaxed);
        }
        if stack.depth == DEPTH {
            fail(inv, 0x10);
        }
        if kind == 6 {
            let id = h(29).fetch_add(1, Ordering::AcqRel) + 1;
            let p = saved.gpr[0].checked_add(0xd0).unwrap_or(0);
            inv.world = id;
            inv.manager = p;
            if let Some(slot) = book.iter().position(|b| b.w[0] == 0) {
                let reused = book.iter().any(|b| b.w[1] == p && p != 0);
                let mut b = Birth { w: [0; 18] };
                b.w[0] = id;
                b.w[1] = p;
                b.w[3] = inv.token;
                b.w[8] = tid;
                b.w[9] = ft;
                b.w[10] = inv.epoch;
                b.w[11] = BORN;
                b.w[12] = inv.native_rsp;
                if reused || p == 0 {
                    b.w[11] |= REUSED;
                    fail(inv, REUSED);
                }
                book[slot] = b;
                index = Some(slot);
            } else {
                fail(inv, 0x2000);
            }
        }
        if let Some(slot) = index {
            inv.list = slot as u64 + 1;
            let b = &mut book[slot];
            inv.world = b.w[0];
            inv.manager = b.w[1];
            inv.set = b.w[2];
            if kind == 4 {
                if b.w[4] != 0 {
                    fail(inv, REUSED);
                    b.w[11] |= REUSED;
                }
                b.w[4] = inv.token;
                b.w[11] |= SUBMITTED;
                b.w[13] = saved.gpr[2];
                b.w[14] = saved.gpr[0];
            } else if kind == 5 {
                if b.w[5] != 0 {
                    fail(inv, REUSED);
                }
                b.w[5] = inv.token;
                b.w[11] |= PUBLISHED;
                if b.w[2] == 0 || b.w[14] != saved.gpr[0] {
                    fail(inv, 0x400);
                }
            } else if kind == 7 {
                let (bytes, n, e, ok) = snapshot.as_ref().unwrap();
                if *n != 16 || *ok == 0 || *e != 0 {
                    fail(inv, 1);
                }
                let p = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
                let table = u64::from_le_bytes(bytes[..8].try_into().unwrap());
                if table != h(8).load(Ordering::Acquire) + 0x106b3c8 || p != b.w[1] {
                    fail(inv, 0x400);
                }
                if b.w[2] == 0 {
                    fail(inv, EARLY);
                    b.w[11] |= EARLY;
                } else if b.w[2] != saved.gpr[0] {
                    fail(inv, REUSED);
                }
                if b.w[6] != 0 {
                    fail(inv, REUSED);
                }
                b.w[6] = inv.token;
                b.w[11] |= HOST_START;
                inv.set = saved.gpr[0];
            } else if kind == 8 {
                if b.w[7] != 0 || b.w[2] == 0 {
                    fail(inv, EARLY);
                }
                if parent.is_none_or(|p| p.world != b.w[0] || p.set != b.w[2]) {
                    fail(inv, 0x800);
                }
                b.w[7] = inv.token;
                b.w[11] |= CORE_START;
            } else if kind == 9 {
                if b.w[16] != 0 {
                    fail(inv, REUSED);
                }
                if b.w[13] != saved.gpr[2] || b.w[14] != saved.gpr[0] {
                    fail(inv, 0x400);
                }
                b.w[16] = inv.token;
                b.w[11] |= HOST_PUSH_START;
            }
            b.w[15] |= inv.errors;
            publish(slot, *b);
        }
        if index.is_none() {
            fail(&mut saved.invocation, 0x2000);
        }
        drop(book);
        let mut r = entry_snapshot(saved, saved.invocation, error);
        r[35] = saved.invocation.world;
        r[36] = saved.invocation.manager;
        r[37] = saved.invocation.set;
        if let Some((bytes, n, e, ok)) = snapshot {
            r[38] = n as u64;
            r[39] = (u64::from(e) << 32) | u64::from(ok != 0);
            for (j, chunk) in bytes.chunks_exact(8).enumerate() {
                r[40 + j] = u64::from_le_bytes(chunk.try_into().unwrap());
            }
            if ok == 0 || e != 0 || n != if kind == 7 { 16 } else { 64 } {
                saved.invocation.errors |= 1;
            }
        }
        r[20] = saved.invocation.errors;
        if r[20] != 0 {
            unknown();
        }
        let mut next = stack;
        if next.depth < DEPTH {
            next.frames[next.depth] = saved.invocation;
            next.depth += 1;
            STACK.with(|s| s.set(next));
        }
        commit(row as usize, r);
    }));
    if result.is_err() {
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
            fail(&mut inv, 0x20);
            unknown();
        }
    });
    if thread_key() != Some((inv.tid, inv.thread_created)) {
        fail(&mut inv, 0x20);
        unknown();
    }
    if edge == 3 {
        fail(&mut inv, 0x40);
        h(13).fetch_add(1, Ordering::Relaxed);
    }
    if inv.list > 0 {
        if let Ok(mut book) = BOOK.lock() {
            let index = inv.list as usize - 1;
            let reused_t = inv.kind == 4
                && book
                    .iter()
                    .enumerate()
                    .any(|(i, other)| i != index && other.w[2] == raw.gpr[2] && raw.gpr[2] != 0);
            let b = &mut book[index];
            if edge == 3 {
                b.w[11] |= EXCEPTION;
            } else {
                match inv.kind {
                    6 => {
                        b.w[11] |= PRODUCER_RETURN;
                        if b.w[4] == 0 {
                            b.w[11] |= NO_SUBMIT;
                        }
                    }
                    4 => {
                        let t = raw.gpr[2];
                        if b.w[11] & HOST_PUSH_RETURN == 0 || b.w[16] == 0 || b.w[17] != t {
                            fail(&mut inv, 0x4000); // actual per-invocation callee witness missing
                        } else if t == 0 || b.w[2] != 0 || reused_t {
                            fail(&mut inv, REUSED);
                        } else {
                            b.w[2] = t;
                            b.w[11] |= BOUND;
                            inv.set = t;
                        }
                    }
                    7 => {
                        b.w[11] |= HOST_RETURN;
                        if b.w[11] & CORE_RETURN == 0 {
                            fail(&mut inv, EARLY);
                        }
                    }
                    8 => b.w[11] |= CORE_RETURN,
                    9 => {
                        b.w[11] |= HOST_PUSH_RETURN;
                        b.w[17] = raw.gpr[2];
                    }
                    _ => {}
                }
            }
            b.w[15] |= inv.errors;
            publish(index, *b);
        } else {
            fail(&mut inv, 0x20);
            unknown();
        }
    }
    if inv.errors != raw.invocation.errors && edge != 3 {
        unknown();
    }
    let index = inv.token as usize - 1;
    let mut r = [0; 48];
    for (dst, src) in r.iter_mut().zip(&TRACE.rows[index]) {
        *dst = src.load(Ordering::Acquire);
    }
    r[2] = edge;
    r[20] |= inv.errors;
    r[21] = if edge == 3 { 0 } else { raw.gpr[4] };
    r[22] = u64::from(error);
    r[28] = if edge == 3 { 0 } else { raw.gpr[1] };
    r[34] = if edge == 2 { raw.gpr[2] } else { 0 };
    r[37] = inv.set;
    // All snapshot fields are cloned; neither P nor T is followed after return.
    commit(index + 1, r);
}
#[unsafe(no_mangle)]
static OBSERVED_CONTINUE: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
global_asm!(
    ".globl observed_producer_original",
    ".seh_proc observed_producer_original",
    "observed_producer_original:",
    "mov [rsp+0x18],rsi",
    ".seh_savereg rsi,0x18",
    ".seh_endprologue",
    "mov [rsp+8],rax",
    "mov rax,[rip+OBSERVED_CONTINUE]",
    "mov [rsp+0x10],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x10]",
    ".seh_endproc",
    ".globl observed_host_original",
    ".seh_proc observed_host_original",
    "observed_host_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+OBSERVED_CONTINUE+8]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl observed_push_host_original",
    ".seh_proc observed_push_host_original",
    "observed_push_host_original:",
    "sub rsp,0x38",
    ".seh_stackalloc 0x38",
    ".seh_endprologue",
    "movups xmm0,[r9]",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+OBSERVED_CONTINUE+24]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl observed_core_original",
    ".seh_proc observed_core_original",
    "observed_core_original:",
    "push rdi",
    ".seh_pushreg rdi",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+OBSERVED_CONTINUE+16]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn observed_producer_original();
    fn observed_host_original();
    fn observed_core_original();
    fn observed_producer_gateway();
    fn observed_host_gateway();
    fn observed_core_gateway();
    fn observed_push_host_original();
    fn observed_push_host_gateway();
}
gateway!("observed_producer_gateway", "6", observed_producer_original);
gateway!("observed_host_gateway", "7", observed_host_original);
gateway!("observed_core_gateway", "8", observed_core_original);
gateway!(
    "observed_push_host_gateway",
    "9",
    observed_push_host_original
);
static EXTRA_HOOKS: [Mutex<Option<RootHook>>; 4] = [const { Mutex::new(None) }; 4];
pub(super) unsafe fn install_extra(core: usize) -> Result<(), &'static str> {
    h(19).store(2, Ordering::Release);
    let exe = h(8).load(Ordering::Acquire) as usize;
    let targets = [
        observed_producer_gateway as *const () as usize,
        observed_host_gateway as *const () as usize,
        observed_core_gateway as *const () as usize,
        observed_push_host_gateway as *const () as usize,
    ];
    for (i, &(rva, host, word, _)) in EXTRA.iter().enumerate() {
        let base = if host { exe } else { core };
        TRACE.sites[i + 2][0].store(rva as u64, Ordering::Release);
        TRACE.sites[i + 2][1].store(u64::from_le_bytes(word), Ordering::Release);
        let actual = copy_current::<8>((base + rva) as u64)
            .ok_or_else(|| site_error(i + 2, 1, "observed site unreadable"))?;
        TRACE.sites[i + 2][2].store(u64::from_le_bytes(actual), Ordering::Release);
        if actual != word || !(base + rva).is_multiple_of(8) {
            return Err(site_error(i + 2, 1, "observed site differs"));
        }
        TRACE.sites[i + 2][3].store(1, Ordering::Release);
    }
    for (i, &(rva, host, word, displaced)) in EXTRA.iter().enumerate() {
        let entry = ((if host { exe } else { core }) + rva) as *mut u8;
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(entry.cast(), &mut info, std::mem::size_of_val(&info)) } == 0 {
            return Err(site_error(i + 2, 10, "observed protection"));
        }
        let code = unsafe { executable_near(entry as usize, entry as usize, displaced) }
            .ok_or_else(|| site_error(i + 2, 11, "observed near relay"))?;
        let mut unpublished = Unpublished(code);
        unsafe {
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), code, 6);
            ptr::copy_nonoverlapping((targets[i] as u64).to_le_bytes().as_ptr(), code.add(6), 8);
        }
        let mut old = 0;
        if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
            || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), 14) } == 0
        {
            return Err(site_error(i + 2, 12, "observed relay publication"));
        }
        let mut patched = word;
        patched[..5].copy_from_slice(
            &rel32_jump(entry as usize, code as usize)
                .ok_or_else(|| site_error(i + 2, 13, "observed range"))?,
        );
        patched[5..displaced].fill(0x90);
        OBSERVED_CONTINUE[i].store(entry as u64 + displaced as u64, Ordering::Release);
        *EXTRA_HOOKS[i]
            .lock()
            .map_err(|_| site_error(i + 2, 14, "observed hook owner"))? = Some(RootHook {
            hook: Hook {
                target: entry,
                original: word,
                patched,
                prefix: None,
                _code: code,
            },
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        let result =
            unsafe { exchange_entry(entry, u64::from_le_bytes(word), u64::from_le_bytes(patched)) }
                .map_err(|e| site_error(i + 2, 15, e))?;
        if result.exchanged {
            h(15).fetch_or(1 << (i + 2), Ordering::Release);
            TRACE.sites[i + 2][4].store(1, Ordering::Release);
        }
        if !result.exchanged
            || !result.protection_restored
            || !result.cache_flushed
            || copy_current::<8>(entry as u64) != Some(patched)
        {
            return Err(site_error(i + 2, 2, "observed CAS publication unverified"));
        }
    }
    h(19).store(3, Ordering::Release);
    Ok(())
}
pub(super) unsafe fn restore_extra() -> Result<(), &'static str> {
    let mut failed = false;
    for (i, slot) in EXTRA_HOOKS.iter().enumerate() {
        let Ok(mut owner) = slot.lock() else {
            failed = true;
            h(17).fetch_or(1 << (i + 2), Ordering::Release);
            TRACE.sites[i + 2][5].store(1, Ordering::Release);
            site_error(i + 2, 3, "critical observed rollback owner poisoned");
            continue;
        };
        let Some(owned) = owner.as_ref() else {
            continue;
        };
        h(17).fetch_or(1 << (i + 2), Ordering::Release);
        TRACE.sites[i + 2][5].store(1, Ordering::Release);
        let hook = &owned.hook;
        let current = copy_current::<8>(hook.target as u64);
        let restored = if current == Some(hook.original) {
            true
        } else if current == Some(hook.patched) {
            unsafe {
                exchange_entry(
                    hook.target,
                    u64::from_le_bytes(hook.patched),
                    u64::from_le_bytes(hook.original),
                )
            }
            .is_ok_and(|r| r.exchanged)
        } else {
            false
        };
        let mut old = 0;
        let protected =
            unsafe { VirtualProtect(hook.target.cast(), 8, owned.protection, &mut old) } != 0;
        let flushed =
            unsafe { FlushInstructionCache(GetCurrentProcess(), hook.target.cast(), 8) } != 0;
        if restored
            && protected
            && flushed
            && copy_current::<8>(hook.target as u64) == Some(hook.original)
        {
            h(18).fetch_or(1 << (i + 2), Ordering::Release);
            TRACE.sites[i + 2][6].store(1, Ordering::Release);
            *owner = None;
        } else {
            failed = true;
            site_error(i + 2, 3, "critical observed rollback");
        }
    }
    if failed {
        Err("critical: observed rollback incomplete")
    } else {
        Ok(())
    }
}
pub(super) fn verify(
    core_file: &mut File,
    exe_file: &mut File,
    core: usize,
    exe: usize,
) -> Result<(), &'static str> {
    for (i, &(rva, host, word, _)) in EXTRA.iter().enumerate() {
        let (file, base, len) = if host {
            (&mut *exe_file, exe, if i == 3 { 0x1cusize } else { 0x3c })
        } else {
            (&mut *core_file, core, if i == 0 { 0x11f } else { 0x8f })
        };
        TRACE.sites[i + 2][0].store(rva as u64, Ordering::Release);
        TRACE.sites[i + 2][1].store(u64::from_le_bytes(word), Ordering::Release);
        if let Some(actual) = copy_current::<8>((base + rva) as u64) {
            TRACE.sites[i + 2][2].store(u64::from_le_bytes(actual), Ordering::Release);
        }
        let source = disk(file, rva, len)?;
        for (n, b) in source.iter().enumerate() {
            if copy_current::<1>((base + rva + n) as u64) != Some([*b]) {
                return Err(site_error(i + 2, 21, "observed whole body differs"));
            }
        }
        // CHAININFO is checked in producer's manager verifier; executor has
        // two additional chained fragments, not one short function range.
        use windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;
        for pc in if i == 2 {
            vec![rva, rva + 0x18, rva + 0x85]
        } else {
            vec![rva]
        } {
            let mut image = 0;
            let entry =
                unsafe { RtlLookupFunctionEntry((base + pc) as u64, &mut image, null_mut()) };
            if entry.is_null() || image != base as u64 {
                return Err(site_error(i + 2, 30, "observed pdata absent"));
            }
            let actual = copy_current::<12>(entry as u64).ok_or("observed pdata read")?;
            if disk(file, entry as usize - base, 12)? != actual {
                return Err(site_error(i + 2, 32, "observed pdata source differs"));
            }
            let unwind = u32::from_le_bytes(actual[8..].try_into().unwrap()) as usize;
            let head = copy_current::<4>((base + unwind) as u64).ok_or("observed unwind head")?;
            if head[0] & 7 != 1 {
                return Err("observed unwind version");
            }
            let size =
                4 + usize::from(head[2]).div_ceil(2) * 4 + if head[0] & 0x20 != 0 { 12 } else { 0 };
            let bytes = disk(file, unwind, size)?;
            for (n, b) in bytes.iter().enumerate() {
                if copy_current::<1>((base + unwind + n) as u64) != Some([*b]) {
                    return Err("observed complete unwind differs");
                }
            }
        }
        h(25).fetch_or(1 << (i + 2), Ordering::Release);
        h(26).fetch_or(1 << (i + 2), Ordering::Release);
    }
    Ok(())
}
