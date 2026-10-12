//! Explicit passive root capture. Never installed by BridgeRuntimeInstall.
//! The only receivers retained are within the current native invocation.
use super::*;
use civ6_bootstrap_rs::protocol::{
    RootConfig, ROOT_BYTES, ROOT_CLOSED, ROOT_CONFIG_VERSION, ROOT_MAGIC, ROOT_ROWS, ROOT_VERSION,
};
use std::fs::OpenOptions;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::{
    cell::Cell,
    sync::OnceLock,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, SetLastError},
    System::{
        LibraryLoader::{
            GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_PIN,
        },
        Threading::{GetCurrentThread, GetThreadTimes},
    },
};

const COOKIE: u64 = 0x880c_0110_ca11_0001;
const DEPTH: usize = 8;
const SITE: [usize; 3] = [0x63e160, 0x4a1150, 0x9922c0];
const WORDS: [[u8; 8]; 3] = [
    [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0xb9],
    [0x48, 0x89, 0x5c, 0x24, 0x10, 0x56, 0x48, 0x83],
    [0x48, 0x89, 0x5c, 0x24, 0x08, 0x57, 0x48, 0x83],
];
// Complete local manager interval through the instruction following its Wait.
const MANAGER_PREFIX: &[u8] = &[
    0x48, 0x89, 0x5c, 0x24, 0x10, 0x56, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8d, 0x05, 0x5f, 0x4e, 0x52,
    0x00, 0xc7, 0x81, 0xa8, 0x38, 0, 0, 3, 0, 0, 0, 0x48, 0x89, 0x44, 0x24, 0x20, 0x48, 0x8d, 0x54,
    0x24, 0x20, 0x48, 0x8b, 0xd9, 0x33, 0xf6, 0x48, 0x89, 0x74, 0x24, 0x28, 0x48, 0x81, 0xc1, 0, 4,
    0, 0, 0x0f, 0x28, 0x44, 0x24, 0x20, 0x66, 0x0f, 0x7f, 0x44, 0x24, 0x20, 0xe8, 0x2a, 0x11, 0x4f,
    0, 0x40, 0x88, 0xb3, 0xac, 0x38, 0, 0,
];
const WORLD_CALL: &[u8] = &[
    0x48, 0x8b, 0x8b, 0x50, 2, 0, 0, 0xe8, 0x7b, 0x2f, 0xe6, 0xff,
];
// RCX=[RDI+250], CALL4A1150, followed by CALL3D50B0. No world type
// is inferred from RDI. The caller continues after manager return.
const LOCAL_MANAGER_CALL: &[u8] = &[
    0x48, 0x8b, 0x8f, 0x50, 2, 0, 0, 0xe8, 0x08, 0x3d, 0xe6, 0xff,
];
const LOCAL_MANAGER_CONTINUATION: &[u8] = &[
    0xe8, 0x63, 0x7c, 0xd9, 0xff, 0x4c, 0x8b, 0xb4, 0x24, 0xa0, 0, 0, 0, 0x84, 0xc0, 0x74, 0x0c,
    0x48, 0x8b, 0x8f, 0x50, 2, 0, 0, 0xe8, 0x5b, 0xf3, 0xe5, 0xff,
];
fn verify_call_spans(core: usize) -> Result<(), &'static str> {
    for (rva, bytes) in [
        (0x4a1150, MANAGER_PREFIX),
        (0x63e1c9, WORLD_CALL),
        (0x63d43c, LOCAL_MANAGER_CALL),
        (0x63d448, LOCAL_MANAGER_CONTINUATION),
    ] {
        for (offset, byte) in bytes.iter().enumerate() {
            if copy_current::<1>((core + rva + offset) as u64) != Some([*byte]) {
                return Err("root fixed caller/continuation bytes differ");
            }
        }
    }
    for (call, bytes, target) in [
        (0x63e1d0, &WORLD_CALL[7..12], 0x4a1150),
        (0x63d443, &LOCAL_MANAGER_CALL[7..12], 0x4a1150),
        (0x63d448, &LOCAL_MANAGER_CONTINUATION[..5], 0x3d50b0),
    ] {
        let actual = (core + call + 5)
            .checked_add_signed(i32::from_le_bytes(bytes[1..].try_into().unwrap()) as isize);
        if bytes[0] != 0xe8 || actual != core.checked_add(target) {
            return Err("root fixed direct caller target differs");
        }
    }
    Ok(())
}

#[repr(C)]
pub(super) struct RootTrace {
    header: [AtomicU64; 32],
    sites: [[AtomicU64; 8]; 3],
    rows: [[AtomicU64; 24]; ROOT_ROWS],
}
static TRACE: RootTrace = RootTrace {
    header: [const { AtomicU64::new(0) }; 32],
    sites: [const { [const { AtomicU64::new(0) }; 8] }; 3],
    rows: [const { [const { AtomicU64::new(0) }; 24] }; ROOT_ROWS],
};
const _: () = assert!(std::mem::size_of::<RootTrace>() == ROOT_BYTES);
#[unsafe(no_mangle)]
pub(super) static BRIDGE_ROOT_OBSERVATION_TRACE: &RootTrace = &TRACE;
#[unsafe(no_mangle)]
pub static BRIDGE_ROOT_OBSERVATION_TRACE_SIZE: u64 = ROOT_BYTES as u64;
fn h(i: usize) -> &'static AtomicU64 {
    if submission::active() {
        &submission::TRACE.header[i]
    } else {
        &TRACE.header[i]
    }
}
fn site_row(i: usize) -> &'static [AtomicU64; 8] {
    if submission::active() {
        &submission::TRACE.sites[i]
    } else {
        &TRACE.sites[i]
    }
}
fn site_rvas() -> [usize; 3] {
    if submission::active() {
        [0x992210, 0x992280, 0]
    } else {
        SITE
    }
}
fn site_count() -> usize {
    if submission::active() {
        2
    } else {
        3
    }
}
fn entry_words() -> [[u8; 8]; 3] {
    if submission::active() {
        submission::WORDS
    } else {
        WORDS
    }
}
fn unknown() {
    h(14).fetch_add(1, Ordering::Relaxed);
}
struct Unpublished(*mut u8);
impl Drop for Unpublished {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { VirtualFree(self.0.cast(), 0, MEM_RELEASE) };
        }
    }
}
fn site_error(i: usize, code: u64, message: &'static str) -> &'static str {
    site_row(i)[7].store(code, Ordering::Release);
    h(16).fetch_or(1 << i, Ordering::Release);
    message
}

struct Binding {
    core: usize,
    manager_return: u64,
    local_manager_return: u64,
    wait_return: u64,
    until: Instant,
}
static BIND: OnceLock<Binding> = OnceLock::new();
static USED: AtomicBool = AtomicBool::new(false);
static INSTALL: Mutex<()> = Mutex::new(());
struct RootHook {
    hook: Hook,
    protection: u32,
}
static HOOKS: [Mutex<Option<RootHook>>; 3] = [const { Mutex::new(None) }; 3];
static SOURCES: Mutex<Vec<File>> = Mutex::new(Vec::new());
#[cfg(test)]
static FAIL_AFTER_PUBLISH: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Invocation {
    cookie: u64,
    kind: u64,
    token: u64,
    parent: u64,
    tid: u64,
    thread_created: u64,
    epoch: u64,
    caller: u64,
    caller_rsp: u64,
    native_rsp: u64,
    world: u64,
    manager: u64,
    set: u64,
    list: u64,
    errors: u64,
    entry_rbx: u64,
}
const _: () = assert!(std::mem::size_of::<Invocation>() == 0x80);
#[repr(C)]
struct Saved {
    // RCX RDX R8 R9 RAX R10 R11 RBX; SIMD lives next to these in assembly.
    gpr: [u64; 8],
    xmm: [[u8; 16]; 6],
    invocation: Invocation,
    metadata: u64,
}
const _: () = assert!(std::mem::offset_of!(Saved, invocation) == 0xa0);
const _: () = assert!(std::mem::offset_of!(Saved, metadata) == 0x120);
#[derive(Clone, Copy)]
struct Stack {
    depth: usize,
    frames: [Invocation; DEPTH],
}
thread_local! {
    static STACK: Cell<Stack> = const { Cell::new(Stack {
        depth: 0, frames: [Invocation {
            cookie:0,kind:0,token:0,parent:0,tid:0,thread_created:0,epoch:0,caller:0,
            caller_rsp:0,native_rsp:0,world:0,manager:0,set:0,list:0,errors:0,entry_rbx:0,
        }; DEPTH],
    }) };
}
fn thread_key() -> Option<(u64, u64)> {
    let mut c = FILETIME::default();
    let mut e = c;
    let mut k = c;
    let mut u = c;
    if unsafe { GetThreadTimes(GetCurrentThread(), &mut c, &mut e, &mut k, &mut u) } == 0 {
        return None;
    }
    Some((
        unsafe { GetCurrentThreadId() } as u64,
        (u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime),
    ))
}
fn acquire(inherited: bool) -> bool {
    h(12)
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
            if (!inherited && v & ROOT_CLOSED != 0) || v & !ROOT_CLOSED == ROOT_CLOSED - 1 {
                None
            } else {
                Some(v + 1)
            }
        })
        .is_ok()
}
struct Release;
impl Drop for Release {
    fn drop(&mut self) {
        h(12).fetch_sub(1, Ordering::Release);
    }
}
fn sample(inv: Invocation, rax: u64, last_error: u32, rdx: u64, edge: u64) {
    if inv.token == 0 {
        return;
    }
    let index = inv.token as usize - 1 + usize::from(edge != 1);
    let mut fields = [0u64; 24];
    fields[1] = inv.kind;
    fields[2] = edge;
    fields[3] = inv.token;
    fields[4] = inv.parent;
    fields[5] = inv.tid;
    fields[6] = inv.thread_created;
    fields[7] = inv.epoch;
    fields[8] = inv.caller;
    fields[9] = inv.caller_rsp;
    fields[10] = inv.world;
    fields[11] = inv.manager;
    fields[12] = inv.set;
    fields[13] = inv.list;
    fields[18] = if edge == 3 { 0 } else { rax };
    fields[19] = inv.errors;
    fields[20] = last_error as u64;
    fields[21] = inv.native_rsp;
    fields[22] = inv.entry_rbx;
    // Never dereference a world after its original function has returned.
    if inv.kind == 2 || inv.kind == 3 {
        let set = copy_current::<16>(inv.set);
        let list = inv.manager.checked_add(0x410).and_then(copy_current::<8>);
        match (set, list) {
            (Some(s), Some(l)) => {
                fields[13] = u64::from_le_bytes(l);
                fields[14] = u64::from_le_bytes(s[..8].try_into().unwrap());
                fields[15] = u64::from_le_bytes(s[8..].try_into().unwrap());
            }
            _ => fields[19] |= 1,
        }
    }
    // Metadata is borrowed only during this Wait invocation; after native
    // return use the original input RDX retained in the current frame.
    if inv.kind == 3 {
        if let Some(m) = copy_current::<16>(rdx) {
            fields[16] = u64::from_le_bytes(m[..8].try_into().unwrap());
            fields[17] = u64::from_le_bytes(m[8..].try_into().unwrap());
        } else {
            fields[19] |= 1;
        }
    }
    fields[23] = h(24).fetch_add(1, Ordering::AcqRel) + 1;
    for (dst, value) in TRACE.rows[index].iter().zip(fields) {
        dst.store(value, Ordering::Relaxed);
    }
    TRACE.rows[index][0].store(index as u64 + 1, Ordering::Release);
}

extern "system" fn begin(raw: *mut Saved) {
    if submission::active() {
        submission::begin(raw);
        return;
    }
    let error = unsafe { GetLastError() };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let saved = unsafe { &mut *raw };
        let inv = &mut saved.invocation;
        let Some(bind) = BIND.get() else { return };
        if bind.core as u64 != h(7).load(Ordering::Acquire) {
            return;
        }
        let stack = STACK.with(Cell::get);
        let parent = stack.depth.checked_sub(1).map(|n| stack.frames[n]);
        // The globally used Wait function is transparent at all other sites.
        if inv.kind == 3 && inv.caller != bind.wait_return {
            return;
        }
        if inv.kind == 2
            && inv.caller != bind.manager_return
            && inv.caller != bind.local_manager_return
        {
            if Instant::now() >= bind.until {
                h(12).fetch_or(ROOT_CLOSED, Ordering::AcqRel);
            }
            if acquire(false) {
                let _release = Release;
                unknown();
            }
            return;
        }
        let local_manager = inv.kind == 2 && inv.caller == bind.local_manager_return;
        let inherited = inv.kind != 1
            && !local_manager
            && parent.is_some_and(|p| {
                p.cookie == COOKIE
                    && p.epoch == h(9).load(Ordering::Acquire)
                    && ((inv.kind == 2 && p.kind == 1) || (inv.kind == 3 && p.kind == 2))
            });
        if !inherited && Instant::now() >= bind.until {
            h(12).fetch_or(ROOT_CLOSED, Ordering::AcqRel);
        }
        if !acquire(inherited) {
            return;
        }
        let reservation = h(10).fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n <= (ROOT_ROWS - 2) as u64).then_some(n + 2)
        });
        let Ok(row) = reservation else {
            h(11).fetch_add(1, Ordering::Relaxed);
            h(12).fetch_sub(1, Ordering::Release);
            return;
        };
        inv.token = row + 1;
        inv.cookie = COOKIE;
        inv.entry_rbx = saved.gpr[7];
        inv.epoch = h(9).load(Ordering::Acquire);
        if let Some((tid, created)) = thread_key() {
            inv.tid = tid;
            inv.thread_created = created;
        } else {
            inv.errors |= 2;
        }
        if h(19).load(Ordering::Acquire) != 3 {
            inv.errors |= 2;
            h(22).fetch_add(1, Ordering::Relaxed);
        }
        if local_manager {
            // A standalone call owns only its current manager receiver.
            // Never borrow a world token or dereference the caller's RDI.
            inv.manager = saved.gpr[0];
            inv.set = inv.manager.checked_add(0x400).unwrap_or(0);
            if parent.is_some() || inv.manager == 0 || inv.set == 0 {
                inv.errors |= 2;
            }
        } else if inv.kind == 1 {
            inv.world = saved.gpr[0];
        } else {
            if let Some(p) = parent {
                inv.parent = p.token;
                inv.world = p.world;
                if p.epoch != inv.epoch
                    || p.tid != inv.tid
                    || p.thread_created != inv.thread_created
                {
                    inv.errors |= 2;
                }
                if inv.kind == 2 {
                    inv.manager = saved.gpr[0];
                    let current = p
                        .world
                        .checked_add(0x250)
                        .and_then(copy_current::<8>)
                        .map(u64::from_le_bytes);
                    if p.kind != 1
                        || inv.entry_rbx != p.world
                        || current != Some(inv.manager)
                        || inv.caller_rsp.checked_add(0x30) != Some(p.native_rsp)
                    {
                        inv.errors |= 2;
                    }
                } else {
                    inv.manager = p.manager;
                    if p.kind != 2
                        || inv.entry_rbx != p.manager
                        || saved.gpr[0].checked_sub(0x400) != Some(p.manager)
                        || inv.caller_rsp.checked_add(0x40) != Some(p.native_rsp)
                    {
                        inv.errors |= 2;
                    }
                }
            } else {
                inv.errors |= 2;
                inv.manager = if inv.kind == 2 {
                    saved.gpr[0]
                } else {
                    saved.gpr[7]
                };
            }
            inv.set = inv.manager.checked_add(0x400).unwrap_or(0);
        }
        if stack.depth == DEPTH {
            inv.errors |= 4;
        }
        if inv.errors != 0 {
            unknown();
        }
        sample(*inv, saved.gpr[4], error, saved.gpr[1], 1);
        if stack.depth < DEPTH {
            let mut next = stack;
            next.frames[next.depth] = *inv;
            next.depth += 1;
            STACK.with(|s| s.set(next));
        }
    }));
    if result.is_err() {
        unknown();
        // If reservation succeeded, retain it for original return/SEH cleanup.
        let inv = unsafe { &mut (*raw).invocation };
        inv.errors |= 8;
    }
    unsafe { SetLastError(error) };
}
fn finish(raw: &mut Saved, edge: u64, error: u32) {
    if submission::active() {
        submission::finish(raw, edge, error);
        return;
    }
    let mut inv = raw.invocation;
    if inv.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0; // exactly one normal/SEH settlement
    let _release = Release;
    let _ = STACK.try_with(|s| {
        let mut stack = s.get();
        if stack.depth > 0 && stack.frames[stack.depth - 1].token == inv.token {
            stack.depth -= 1;
            stack.frames[stack.depth] = Invocation::default();
            s.set(stack);
        } else if inv.errors & 4 == 0 {
            inv.errors |= 8;
            unknown();
        }
    });
    if thread_key() != Some((inv.tid, inv.thread_created)) {
        inv.errors |= 8;
        unknown();
    }
    if edge == 3 {
        inv.errors |= 16;
        h(13).fetch_add(1, Ordering::Relaxed);
    }
    // RDX output has to be preserved; metadata address is stored separately
    // in the current gateway frame, not taken from native output RDX.
    let metadata = raw.metadata;
    sample(inv, raw.gpr[4], error, metadata, edge);
}
extern "system" fn end(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        finish(unsafe { &mut *raw }, 2, error)
    }))
    .is_err()
    {
        unknown();
    }
    unsafe { SetLastError(error) };
}
unsafe extern "system" fn unwind(
    record: *const windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_RECORD,
    frame: usize,
    _context: *mut windows_sys::Win32::System::Diagnostics::Debug::CONTEXT,
    _dispatcher: *mut c_void,
) -> i32 {
    let error = unsafe { GetLastError() };
    if !record.is_null() && unsafe { (*record).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0 {
        let raw = (frame + 0x20) as *mut Saved;
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finish(unsafe { &mut *raw }, 3, error)
        }))
        .is_err()
        {
            unknown();
        }
    }
    unsafe { SetLastError(error) };
    1 // ExceptionContinueSearch; never synthesize a native return.
}

#[unsafe(no_mangle)]
static ROOT_CONTINUE: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
// These are actual assembly frames with static unwind tables, not guessed
// dynamic trampolines. Each displaced instruction executes exactly once.
// A RIP-relative JMP (ModRM.mod=00) is recognized as an epilogue even though
// this frame has NOT been released. Use ModRM.mod=01 JMP [rsp+18h] instead.
// The targets' checked ABI has no caller-home inputs. These call-local home
// slots preserve RAX while forming the branch; every added instruction is MOV.
global_asm!(
    ".globl root_world_original",
    ".seh_proc root_world_original",
    "root_world_original:",
    "push rbx",
    ".seh_pushreg rbx",
    ".globl root_world_original_pushed",
    "root_world_original_pushed:",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    ".globl root_world_original_body",
    "root_world_original_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+ROOT_CONTINUE]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    ".globl root_world_original_branch",
    "root_world_original_branch:",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl root_manager_original",
    ".seh_proc root_manager_original",
    "root_manager_original:",
    "mov [rsp+0x10],rbx",
    ".seh_savereg rbx,0x10",
    ".seh_endprologue",
    ".globl root_manager_original_body",
    "root_manager_original_body:",
    "mov [rsp+8],rax",
    "mov rax,[rip+ROOT_CONTINUE+8]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+8]",
    ".globl root_manager_original_branch",
    "root_manager_original_branch:",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl root_wait_original",
    ".seh_proc root_wait_original",
    "root_wait_original:",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    ".seh_endprologue",
    ".globl root_wait_original_body",
    "root_wait_original_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+ROOT_CONTINUE+16]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    ".globl root_wait_original_branch",
    "root_wait_original_branch:",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
);
// CALL pushes a helper return address. From helper entry, the gateway's flags
// slot is +1c8h. Each temporary PUSH belongs to this helper's unwind prologue,
// never to the fixed-size gateway body. POP memory computes an RSP-based
// destination AFTER incrementing RSP, hence +1c8h (not +1d0h).
global_asm!(
    ".globl root_flags_restore",
    ".seh_proc root_flags_restore",
    "root_flags_restore:",
    "push qword ptr [rsp+0x1c8]",
    ".seh_stackalloc 8",
    ".seh_endprologue",
    ".globl root_flags_restore_pushed",
    "root_flags_restore_pushed:",
    // Owned OS children replace exactly this inert window with CALL fault.
    // Both builds execute the same instructions and have the same unwind table.
    "nop",
    "nop",
    "nop",
    "nop",
    "nop",
    ".globl root_flags_restore_pop",
    "root_flags_restore_pop:",
    "popfq",
    ".globl root_flags_restore_ret",
    "root_flags_restore_ret:",
    "ret",
    ".seh_endproc",
    ".globl root_flags_save",
    ".seh_proc root_flags_save",
    "root_flags_save:",
    "pushfq",
    ".seh_stackalloc 8",
    ".seh_endprologue",
    ".globl root_flags_save_pushed",
    "root_flags_save_pushed:",
    "nop",
    "nop",
    "nop",
    "nop",
    "nop",
    ".globl root_flags_save_pop",
    "root_flags_save_pop:",
    "pop qword ptr [rsp+0x1c8]",
    ".globl root_flags_save_ret",
    "root_flags_save_ret:",
    "ret",
    ".seh_endproc",
);
unsafe extern "system" {
    fn root_flags_restore();
    fn root_flags_save();
}
macro_rules! gateway {
    ($name:literal,$kind:literal,$original:ident) => {
        global_asm!(concat!(
            ".globl ",$name,"\n.seh_proc ",$name,"\n",$name,":\n",
            // pushfq precedes SUB: input flags must not be sampled after SUB.
            "pushfq\n.seh_stackalloc 8\nsub rsp,0x1c0\n.seh_stackalloc 0x1c0\n",
            "mov qword ptr [rsp+0xc0],0\n.seh_handler {unwind}, @unwind\n.seh_endprologue\n",
            ".globl ",$name,"_body\n",$name,"_body:\n",
            "mov [rsp+0x20],rcx\nmov [rsp+0x28],rdx\nmov [rsp+0x30],r8\nmov [rsp+0x38],r9\n",
            "mov [rsp+0x40],rax\nmov [rsp+0x48],r10\nmov [rsp+0x50],r11\nmov [rsp+0x58],rbx\n",
            "movdqu [rsp+0x60],xmm0\nmovdqu [rsp+0x70],xmm1\nmovdqu [rsp+0x80],xmm2\n",
            "movdqu [rsp+0x90],xmm3\nmovdqu [rsp+0xa0],xmm4\nmovdqu [rsp+0xb0],xmm5\n",
            // All Invocation fields start zero; retained addresses are call-local.
            "lea rax,[rsp+0xc0]\nmov ecx,16\n2:\nmov qword ptr [rax],0\nlea rax,[rax+8]\ndec ecx\njnz 2b\n",
            "mov qword ptr [rsp+0xc8],",$kind,"\n",
            "lea rax,[rsp+0x1c8]\nmov [rsp+0x100],rax\nmov rax,[rax]\nmov [rsp+0xf8],rax\n",
            // At the following CALL the ORIGINAL receives rsp-8. This value,
            // not gateway entry RSP, is the manager parent for Wait's -40h.
            "lea rax,[rsp-8]\nmov [rsp+0x108],rax\n",
            "mov rax,[rsp+0x28]\nmov [rsp+0x140],rax\n",
            "lea rcx,[rsp+0x20]\ncall {begin}\n",
            "mov rcx,[rsp+0x20]\nmov rdx,[rsp+0x28]\nmov r8,[rsp+0x30]\nmov r9,[rsp+0x38]\n",
            "mov rax,[rsp+0x40]\nmov r10,[rsp+0x48]\nmov r11,[rsp+0x50]\n",
            "movdqu xmm0,[rsp+0x60]\nmovdqu xmm1,[rsp+0x70]\nmovdqu xmm2,[rsp+0x80]\n",
            "movdqu xmm3,[rsp+0x90]\nmovdqu xmm4,[rsp+0xa0]\nmovdqu xmm5,[rsp+0xb0]\n",
            "call {restore_flags}\n.globl ",$name,"_flags_input_return\n",$name,"_flags_input_return:\ncall {original}\n",
            // Preserve the ORIGINAL outputs before any helper or flag change.
            "call {save_flags}\n.globl ",$name,"_flags_save_return\n",$name,"_flags_save_return:\n",
            "mov [rsp+0x20],rcx\nmov [rsp+0x28],rdx\nmov [rsp+0x30],r8\nmov [rsp+0x38],r9\n",
            "mov [rsp+0x40],rax\nmov [rsp+0x48],r10\nmov [rsp+0x50],r11\n",
            "movdqu [rsp+0x60],xmm0\nmovdqu [rsp+0x70],xmm1\nmovdqu [rsp+0x80],xmm2\n",
            "movdqu [rsp+0x90],xmm3\nmovdqu [rsp+0xa0],xmm4\nmovdqu [rsp+0xb0],xmm5\n",
            "lea rcx,[rsp+0x20]\ncall {end}\n",
            "mov rcx,[rsp+0x20]\nmov rdx,[rsp+0x28]\nmov r8,[rsp+0x30]\nmov r9,[rsp+0x38]\n",
            "mov rax,[rsp+0x40]\nmov r10,[rsp+0x48]\nmov r11,[rsp+0x50]\n",
            "movdqu xmm0,[rsp+0x60]\nmovdqu xmm1,[rsp+0x70]\nmovdqu xmm2,[rsp+0x80]\n",
            "movdqu xmm3,[rsp+0x90]\nmovdqu xmm4,[rsp+0xa0]\nmovdqu xmm5,[rsp+0xb0]\n",
            "call {restore_flags}\n.globl ",$name,"_flags_output_return\n",$name,"_flags_output_return:\n",
            // This exact LEA/RET is an epilogue: the flags helper's CALL frame
            // is already gone before the gateway releases its fixed frame.
            "lea rsp,[rsp+0x1c8]\nret\n.seh_endproc\n"
        ), begin=sym begin,end=sym end,unwind=sym unwind,original=sym $original,
            restore_flags=sym root_flags_restore,save_flags=sym root_flags_save);
    };
}
// Keep the child after gateway!'s textual definition: it reuses this macro,
// including the exact shared flags helpers and unwind handler.
mod submission;
unsafe extern "system" {
    fn root_world_original();
    fn root_manager_original();
    fn root_wait_original();
    fn root_world_gateway();
    fn root_manager_gateway();
    fn root_wait_gateway();
}
#[cfg(not(test))]
gateway!("root_world_gateway", "1", root_world_original);
#[cfg(not(test))]
gateway!("root_manager_gateway", "2", root_manager_original);
#[cfg(not(test))]
gateway!("root_wait_gateway", "3", root_wait_original);
// State-preservation tests substitute a native body that samples flags before
// a native SUB. Real-CAS and SEH tests use the exact displaced-prologue shims.
#[cfg(test)]
#[unsafe(no_mangle)]
static ROOT_TEST_ORIGINALS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
#[cfg(test)]
global_asm!(
    ".globl root_test_world_original",
    "root_test_world_original:",
    "jmp qword ptr [rip+ROOT_TEST_ORIGINALS]",
    ".globl root_test_manager_original",
    "root_test_manager_original:",
    "jmp qword ptr [rip+ROOT_TEST_ORIGINALS+8]",
    ".globl root_test_wait_original",
    "root_test_wait_original:",
    "jmp qword ptr [rip+ROOT_TEST_ORIGINALS+16]",
);
#[cfg(test)]
unsafe extern "system" {
    fn root_test_world_original();
    fn root_test_manager_original();
    fn root_test_wait_original();
}
#[cfg(test)]
gateway!("root_world_gateway", "1", root_test_world_original);
#[cfg(test)]
gateway!("root_manager_gateway", "2", root_test_manager_original);
#[cfg(test)]
gateway!("root_wait_gateway", "3", root_test_wait_original);
fn gateways() -> [usize; 3] {
    if submission::active() {
        return submission::gateways();
    }
    [
        root_world_gateway as *const () as usize,
        root_manager_gateway as *const () as usize,
        root_wait_gateway as *const () as usize,
    ]
}

fn pin(path: &str, expected: &str) -> Result<File, &'static str> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut f = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|_| "root source open")?;
    let mut hash = Sha256::new();
    let mut b = [0u8; 65536];
    loop {
        let n = f.read(&mut b).map_err(|_| "root source read")?;
        if n == 0 {
            break;
        }
        hash.update(&b[..n]);
    }
    if format!("{:x}", hash.finalize()) != expected {
        return Err("root source SHA");
    }
    Ok(f)
}
fn initialize(config: RootConfig, core: usize, exe: usize) {
    if submission::active() {
        submission::initialize(config, core, exe);
        return;
    }
    let values = [
        ROOT_MAGIC,
        ROOT_VERSION,
        ROOT_BYTES as u64,
        ROOT_ROWS as u64,
        192,
        config.pid as u64,
        config.created_filetime,
        core as u64,
        exe as u64,
        1,
    ];
    for (i, value) in values.into_iter().enumerate() {
        h(i).store(value, Ordering::Release);
    }
    h(19).store(1, Ordering::Release);
    for i in 0..3 {
        site_row(i)[0].store(SITE[i] as u64, Ordering::Relaxed);
        site_row(i)[1].store(u64::from_le_bytes(WORDS[i]), Ordering::Relaxed);
    }
}
unsafe fn install_sites(core: usize, expected: [[u8; 8]; 3]) -> Result<(), &'static str> {
    let sites = site_rvas();
    let count = site_count();
    // ALL sites and complete local call evidence are checked before publication.
    for i in 0..count {
        let actual = copy_current::<8>((core + sites[i]) as u64).ok_or("root unreadable entry")?;
        site_row(i)[2].store(u64::from_le_bytes(actual), Ordering::Release);
        if actual != expected[i] || !(core + sites[i]).is_multiple_of(8) {
            site_row(i)[7].store(1, Ordering::Release);
            return Err("root fixed entry differs");
        }
        site_row(i)[3].store(1, Ordering::Release);
    }
    let callbacks = gateways();
    h(19).store(2, Ordering::Release);
    for i in 0..count {
        let entry = (core + sites[i]) as *mut u8;
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(entry.cast(), &mut info, std::mem::size_of_val(&info)) } == 0 {
            return Err(site_error(i, 10, "root entry protection"));
        }
        let displaced = if (submission::active() && i == 1) || (!submission::active() && i == 0) {
            6
        } else {
            5
        };
        let code = unsafe { executable_near(entry as usize, entry as usize, displaced) }
            .ok_or_else(|| site_error(i, 11, "root relay unavailable"))?;
        let mut unpublished = Unpublished(code);
        unsafe {
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), code, 6);
            ptr::copy_nonoverlapping((callbacks[i] as u64).to_le_bytes().as_ptr(), code.add(6), 8);
        }
        let mut old = 0;
        if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
            || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), 14) } == 0
        {
            // Unreachable allocation; do not mutate any entry.
            return Err(site_error(i, 12, "root relay publication"));
        }
        let mut patched = expected[i];
        patched[..5].copy_from_slice(
            &rel32_jump(entry as usize, code as usize)
                .ok_or_else(|| site_error(i, 13, "root relay range"))?,
        );
        if displaced == 6 {
            patched[5] = 0x90;
        } // 6-byte displaced prologue, cmp starts at+6
        let continuation = core + sites[i] + displaced;
        ROOT_CONTINUE[i].store(continuation as u64, Ordering::Release);
        // Ownership MUST precede a CAS that might succeed then report failure.
        *HOOKS[i]
            .lock()
            .map_err(|_| site_error(i, 14, "root hook owner poisoned"))? = Some(RootHook {
            hook: Hook {
                target: entry,
                original: expected[i],
                patched,
                prefix: None,
                _code: code,
            },
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        let result = unsafe {
            exchange_entry(
                entry,
                u64::from_le_bytes(expected[i]),
                u64::from_le_bytes(patched),
            )
        }
        .map_err(|e| site_error(i, 15, e))?;
        if result.exchanged {
            h(15).fetch_or(1 << i, Ordering::Release);
            site_row(i)[4].store(1, Ordering::Release);
        }
        if !result.exchanged
            || !result.protection_restored
            || !result.cache_flushed
            || copy_current::<8>(entry as u64) != Some(patched)
        {
            site_row(i)[7].store(2, Ordering::Release);
            h(16).fetch_or(1 << i, Ordering::Release);
            return Err("root entry publication unverified");
        }
        #[cfg(test)]
        if FAIL_AFTER_PUBLISH.load(Ordering::Acquire) == i + 1 {
            return Err(site_error(i, 20, "owned publication failure"));
        }
    }
    h(19).store(3, Ordering::Release);
    Ok(())
}
unsafe fn restore_all() -> Result<(), &'static str> {
    h(12).fetch_or(ROOT_CLOSED, Ordering::AcqRel);
    let mut failed = false;
    if submission::active() && unsafe { submission::restore_extra() }.is_err() {
        failed = true;
    }
    for (i, slot) in HOOKS.iter().take(site_count()).enumerate() {
        h(17).fetch_or(1 << i, Ordering::Release);
        site_row(i)[5].store(1, Ordering::Release);
        let Ok(mut owner) = slot.lock() else {
            failed = true;
            continue;
        };
        let Some(owned) = owner.as_ref() else {
            continue;
        };
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
            h(18).fetch_or(1 << i, Ordering::Release);
            site_row(i)[6].store(1, Ordering::Release);
            *owner = None; // code remains allocated; continuation never redirected
        } else {
            failed = true;
            h(16).fetch_or(1 << i, Ordering::Release);
            site_row(i)[7].store(3, Ordering::Release);
        }
    }
    if failed {
        Err("critical: root rollback incomplete")
    } else {
        Ok(())
    }
}
fn install(config: &mut RootConfig) -> Result<(), &'static str> {
    if (config.version != ROOT_CONFIG_VERSION
        && config.version != civ6_bootstrap_rs::protocol::SUBMISSION_CONFIG_VERSION)
        || config.size as usize != std::mem::size_of::<RootConfig>()
        || config.reserved != 0
        || config.trace_address != 0
        || config.trace_size != 0
        || !(1..=MAX_ACTIVE_SECONDS).contains(&config.max_seconds)
        || config.pid != unsafe { GetCurrentProcessId() }
        || current_created_filetime() != Some(config.created_filetime)
        || !supports_atomic_promotion_fetch()
    {
        return Err("root config/identity/CPU");
    }
    let core = unsafe { GetModuleHandleW(wide("GameCore_XP2_FinalRelease.dll").as_ptr()) };
    let exe = unsafe { GetModuleHandleW(null_mut()) };
    if core.is_null()
        || core as u64 != config.core
        || !module_path(core).is_some_and(|p| same_file(&p, GAMECORE))
        || !module_path(exe).is_some_and(|p| same_file(&p, GAME_EXE))
    {
        return Err("root loaded images");
    }
    let mut files = [
        pin(GAMECORE, crate::PINNED_GAMECORE_SHA256)?,
        pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?,
    ];
    submission::PROFILE.store(
        config.version == civ6_bootstrap_rs::protocol::SUBMISSION_CONFIG_VERSION,
        Ordering::Release,
    );
    initialize(*config, core as usize, exe as usize);
    if USED.swap(true, Ordering::AcqRel) {
        return Err("root lifetime already consumed");
    }
    let mut entry_mismatch = false;
    let sites = site_rvas();
    let words = entry_words();
    for i in 0..site_count() {
        match copy_current::<8>((core as usize + sites[i]) as u64) {
            Some(actual) => {
                site_row(i)[2].store(u64::from_le_bytes(actual), Ordering::Release);
                if actual == words[i] {
                    site_row(i)[3].store(1, Ordering::Release);
                } else {
                    entry_mismatch = true;
                    site_row(i)[7].store(1, Ordering::Release);
                }
            }
            None => {
                entry_mismatch = true;
                site_row(i)[7].store(4, Ordering::Release);
            }
        }
    }
    if entry_mismatch {
        return Err("root pre-publication entry witness differs");
    }
    // Hash-pinned .text RVA1000/raw400. Verify the complete THREE displaced
    // native functions against the same retained file, not just entry bytes.
    let lengths = if submission::active() {
        [0x6cusize, 0x33, 0]
    } else {
        [0x17fusize, 0x58f, 0x4c]
    };
    for (i, length) in lengths.into_iter().take(site_count()).enumerate() {
        files[0]
            .seek(SeekFrom::Start((sites[i] - 0xc00) as u64))
            .map_err(|_| "root ABI file seek")?;
        let mut disk = vec![0u8; length];
        files[0]
            .read_exact(&mut disk)
            .map_err(|_| "root ABI file read")?;
        for (offset, byte) in disk.into_iter().enumerate() {
            if copy_current::<1>((core as usize + sites[i] + offset) as u64) != Some([byte]) {
                return Err(site_error(i, 21, "root complete native function differs"));
            }
        }
    }
    if submission::active() {
        submission::verify(&mut files[0], core as usize)?;
        let (core_file, exe_file) = files.split_at_mut(1);
        submission::verify_r8_flow(
            &mut core_file[0],
            &mut exe_file[0],
            core as usize,
            exe as usize,
        )?;
    } else {
        verify_call_spans(core as usize)?;
    }
    // Pin exact modules before any executable publication. Files alone do not
    // keep an image mapped. No hook or unwind target is ever freed/unpinned.
    for address in [
        core as usize,
        exe as usize,
        BridgeRootObservationInstall as *const () as usize,
    ] {
        let mut pinned = null_mut();
        if unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                address as *const u16,
                &mut pinned,
            )
        } == 0
        {
            return Err("root module pin");
        }
    }
    SOURCES
        .lock()
        .map_err(|_| "root source owner")?
        .extend(files);
    BIND.set(Binding {
        core: core as usize,
        manager_return: core as u64 + 0x63e1d5,
        local_manager_return: core as u64 + 0x63d448,
        wait_return: core as u64 + 0x4a1196,
        until: Instant::now() + Duration::from_secs(config.max_seconds as u64),
    })
    .map_err(|_| "root binding already set")?;
    unsafe { install_sites(core as usize, words) }?;
    if submission::active() {
        unsafe { submission::install_extra(core as usize) }?;
    }
    config.trace_address = if submission::active() {
        std::ptr::addr_of!(submission::TRACE) as u64
    } else {
        std::ptr::addr_of!(TRACE) as u64
    };
    config.trace_size = if submission::active() {
        civ6_bootstrap_rs::protocol::SUBMISSION_BYTES as u32
    } else {
        ROOT_BYTES as u32
    };
    Ok(())
}
#[unsafe(no_mangle)]
pub extern "system" fn BridgeRootObservationInstall(input: *mut c_void) -> u32 {
    install_export(input, ROOT_CONFIG_VERSION)
}
#[unsafe(no_mangle)]
pub extern "system" fn BridgeSubmissionObservationInstall(input: *mut c_void) -> u32 {
    install_export(
        input,
        civ6_bootstrap_rs::protocol::SUBMISSION_CONFIG_VERSION,
    )
}
fn install_export(input: *mut c_void, version: u32) -> u32 {
    if !committed_range(input as usize, std::mem::size_of::<RootConfig>(), 8, true) {
        return 1;
    }
    let Ok(_guard) = INSTALL.lock() else { return 2 };
    if USED.load(Ordering::Acquire) {
        return 1;
    }
    let result = std::panic::catch_unwind(|| {
        let mut config = unsafe { ptr::read(input.cast::<RootConfig>()) };
        if config.version != version {
            return Err("observation export/config profile mismatch");
        }
        install(&mut config)?;
        unsafe { ptr::write(input.cast(), config) };
        Ok::<_, &'static str>(())
    });
    if result.is_ok_and(|r| r.is_ok()) {
        return 0;
    }
    if !USED.load(Ordering::Acquire) {
        // Pure argument/identity rejection did not own a publication. Do not
        // close a future valid capture or rewrite its source witness header.
        return 1;
    }
    h(19).store(4, Ordering::Release);
    unknown();
    match std::panic::catch_unwind(|| unsafe { restore_all() }) {
        Ok(Ok(())) => 1,
        _ => 2,
    }
}
#[unsafe(no_mangle)]
pub extern "system" fn BridgeRootObservationStop(_: *mut c_void) -> u32 {
    if submission::active() {
        return 1;
    }
    stop_export()
}
#[unsafe(no_mangle)]
pub extern "system" fn BridgeSubmissionObservationStop(_: *mut c_void) -> u32 {
    if !submission::active() {
        return 1;
    }
    stop_export()
}
fn stop_export() -> u32 {
    let deadline = Instant::now() + Duration::from_secs(2);
    let _guard = loop {
        match INSTALL.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::Poisoned(_)) => return 2,
            Err(std::sync::TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return 2;
                }
                std::thread::yield_now();
            }
        }
    };
    if h(19).load(Ordering::Acquire) != 3 {
        return 1;
    }
    stop_at(deadline)
}
fn stop_at(deadline: Instant) -> u32 {
    h(12).fetch_or(ROOT_CLOSED, Ordering::AcqRel);
    h(23).store(1, Ordering::Release);
    loop {
        if Instant::now() >= deadline {
            h(23).store(3, Ordering::Release);
            return 2;
        }
        if h(12).load(Ordering::Acquire) == ROOT_CLOSED {
            h(23).store(2, Ordering::Release);
            return 0;
        }
        std::thread::yield_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Diagnostics::Debug::{
        RtlLookupFunctionEntry, CONTEXT, EXCEPTION_RECORD, IMAGE_RUNTIME_FUNCTION_ENTRY,
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
        fn RtlAddFunctionTable(
            entries: *mut IMAGE_RUNTIME_FUNCTION_ENTRY,
            count: u32,
            base: u64,
        ) -> u8;
        fn RtlDeleteFunctionTable(entries: *mut IMAGE_RUNTIME_FUNCTION_ENTRY) -> u8;
    }
    fn child(name: &str) -> bool {
        if std::env::var_os("CIV6_ROOT_OWNED_CHILD").is_some() {
            return true;
        }
        use std::os::windows::process::CommandExt;
        let mut process = std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env("CIV6_ROOT_OWNED_CHILD", "1")
            .args([
                "--exact",
                &format!("windows::root_observation::tests::{name}"),
                "--nocapture",
            ])
            .spawn()
            .unwrap();
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = process.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if Instant::now() >= until {
                let _ = process.kill();
                let _ = process.wait();
                panic!("owned root child deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
    fn config(core: usize) -> RootConfig {
        RootConfig {
            version: ROOT_CONFIG_VERSION,
            size: std::mem::size_of::<RootConfig>() as u32,
            pid: unsafe { GetCurrentProcessId() },
            max_seconds: 30,
            created_filetime: current_created_filetime().unwrap(),
            core: core as u64,
            trace_address: 0,
            trace_size: 0,
            reserved: 0,
        }
    }
    fn bind_fixture(core: usize, manager_return: u64, wait_return: u64) {
        initialize(config(core), core, 1);
        assert!(BIND
            .set(Binding {
                core,
                manager_return,
                local_manager_return: core as u64 + 0x63d448,
                wait_return,
                until: Instant::now() + Duration::from_secs(30)
            })
            .is_ok());
        h(19).store(3, Ordering::Release);
        for (i, target) in [
            root_world_original as *const () as u64,
            root_manager_original as *const () as u64,
            root_wait_original as *const () as u64,
        ]
        .into_iter()
        .enumerate()
        {
            ROOT_TEST_ORIGINALS[i].store(target, Ordering::Release);
        }
    }
    fn bytes() -> Vec<u8> {
        let mut words = Vec::new();
        words.extend(TRACE.header.iter().map(|x| x.load(Ordering::Acquire)));
        for site in &TRACE.sites {
            words.extend(site.iter().map(|x| x.load(Ordering::Acquire)));
        }
        for row in &TRACE.rows {
            words.extend(row.iter().map(|x| x.load(Ordering::Acquire)));
        }
        words.into_iter().flat_map(u64::to_le_bytes).collect()
    }
    // Input flags are recorded by the native fixture itself BEFORE its SUB.
    // Outputs are recorded by its assembly caller immediately after returning.
    // Calls to the recorder/SetLastError cannot stand in for these assertions.
    global_asm!(
        ".macro root_probe_inputs",
        "mov [rcx],rax","mov [rcx+8],rdx","mov [rcx+16],r8","mov [rcx+24],r9",
        "mov [rcx+32],r10","mov [rcx+40],r11","mov [rcx+48],rcx",
        "pushfq","pop qword ptr [rcx+56]",
        "movdqu [rcx+0x100],xmm0","movdqu [rcx+0x110],xmm1","movdqu [rcx+0x120],xmm2",
        "movdqu [rcx+0x130],xmm3","movdqu [rcx+0x140],xmm4","movdqu [rcx+0x150],xmm5",
        ".endm",
        ".macro root_probe_outputs",
        "mov rbx,rcx","call {get_error}","mov [rbx+0x90],rax",
        "inc qword ptr [rbx+0x80]","mov ecx,0x880","call {set_error}",
        "xor eax,eax","mov rax,0x1122","mov rdx,0x2233","mov r8,0x3344","mov r9,0x4455",
        "mov r10,0x5566","mov r11,0x6677","mov rcx,0x7788",
        "pcmpeqb xmm0,xmm0","pxor xmm1,xmm1","pcmpeqb xmm2,xmm2",
        "pxor xmm3,xmm3","pcmpeqb xmm4,xmm4","pxor xmm5,xmm5",
        "stc","pushfq","pop qword ptr [rbx+0x88]",".endm",
        ".p2align 4",".globl root_probe_world",".seh_proc root_probe_world","root_probe_world:",
        ".byte 0x40,0x53",".seh_pushreg rbx","sub rsp,0x20",".seh_stackalloc 0x20",
        ".seh_endprologue","root_probe_inputs","root_probe_outputs",
        "mov rbx,[rsp+0x20]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
        ".p2align 4",".globl root_probe_manager",".seh_proc root_probe_manager","root_probe_manager:",
        "mov [rsp+0x10],rbx",".seh_savereg rbx,0x10",
        "root_probe_inputs","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
        "root_probe_outputs","mov rbx,[rsp+0x38]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
        ".p2align 4",".globl root_probe_wait",".seh_proc root_probe_wait","root_probe_wait:",
        "mov [rsp+8],rbx",".seh_savereg rbx,8",
        "root_probe_inputs","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
        "root_probe_outputs","mov rbx,[rsp+0x30]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
        ".globl root_probe_driver",".seh_proc root_probe_driver","root_probe_driver:",
        "push rbx",".seh_pushreg rbx","sub rsp,0x40",".seh_stackalloc 0x40",".seh_endprologue",
        "mov rbx,rcx","mov [rsp+0x20],rdx","mov rax,0x11223344",
        "mov rdx,0x5566","mov r8,0x7788","mov r9,0x99aa","mov r10,0xbbcc","mov r11,0xddee",
        "pcmpeqb xmm0,xmm0","pcmpeqb xmm1,xmm1","pcmpeqb xmm2,xmm2",
        "pcmpeqb xmm3,xmm3","pcmpeqb xmm4,xmm4","pcmpeqb xmm5,xmm5",
        "cmp rsp,rsp","stc","pushfq","pop qword ptr [rbx+0x98]",
        "call qword ptr [rsp+0x20]",".globl root_probe_return","root_probe_return:",
        "pushfq","pop qword ptr [rbx+0x280]",
        "mov [rbx+0x200],rax","mov [rbx+0x208],rdx","mov [rbx+0x210],r8","mov [rbx+0x218],r9",
        "mov [rbx+0x220],r10","mov [rbx+0x228],r11","mov [rbx+0x230],rcx",
        "movdqu [rbx+0x300],xmm0","movdqu [rbx+0x310],xmm1","movdqu [rbx+0x320],xmm2",
        "movdqu [rbx+0x330],xmm3","movdqu [rbx+0x340],xmm4","movdqu [rbx+0x350],xmm5",
        "add rsp,0x40","pop rbx","ret",".seh_endproc",
        set_error=sym SetLastError,get_error=sym GetLastError,
    );
    unsafe extern "system" {
        fn root_probe_world();
        fn root_probe_manager();
        fn root_probe_wait();
        fn root_probe_return();
        fn root_probe_driver(data: *mut u8, target: usize);
    }
    fn u64_at(b: &[u8], p: usize) -> u64 {
        u64::from_le_bytes(b[p..p + 8].try_into().unwrap())
    }
    #[test]
    fn root_gateway_input_output_flags_and_last_error() {
        if !child("root_gateway_input_output_flags_and_last_error") {
            return;
        }
        let ret = root_probe_return as *const () as u64;
        bind_fixture(1, ret, ret);
        for (i, original) in [
            root_probe_manager as *const () as usize,
            root_probe_manager as *const () as usize,
            root_probe_wait as *const () as usize,
        ]
        .into_iter()
        .enumerate()
        {
            ROOT_CONTINUE[i].store(
                (original + if i == 0 { 6 } else { 5 }) as u64,
                Ordering::Release,
            );
            ROOT_TEST_ORIGINALS[i].store(original as u64, Ordering::Release);
            let mut baseline = vec![0u8; 0x500];
            let mut observed = vec![0u8; 0x500];
            unsafe {
                SetLastError(0x1234);
                root_probe_driver(baseline.as_mut_ptr(), original);
            }
            let next = h(10).load(Ordering::Acquire);
            unsafe {
                SetLastError(0x1234);
                root_probe_driver(observed.as_mut_ptr(), gateways()[i]);
            }
            assert_eq!(unsafe { GetLastError() }, 0x880);
            assert_eq!(
                u64_at(&observed, 0x90),
                0x1234,
                "native body receives entry LastError"
            );
            assert_eq!(
                u64_at(&observed, 0x80),
                1,
                "actual native body executes once"
            );
            assert_eq!(&observed[..48], &baseline[..48]);
            assert_eq!(u64_at(&observed, 0), 0x11223344);
            assert_eq!(u64_at(&observed, 32), 0xbbcc);
            assert_eq!(u64_at(&observed, 40), 0xddee);
            assert_eq!(u64_at(&observed, 56), u64_at(&baseline, 56));
            assert_eq!(u64_at(&observed, 56), u64_at(&observed, 0x98));
            for j in 0..6 {
                assert_eq!(&observed[0x100 + j * 16..0x110 + j * 16], &[0xff; 16]);
            }
            for (j, expected) in [0x1122, 0x2233, 0x3344, 0x4455, 0x5566, 0x6677, 0x7788]
                .into_iter()
                .enumerate()
            {
                assert_eq!(u64_at(&observed, 0x200 + j * 8), expected);
            }
            for j in 0..6 {
                assert_eq!(
                    &observed[0x300 + j * 16..0x310 + j * 16],
                    &[if j % 2 == 0 { 0xff } else { 0 }; 16]
                );
            }
            assert_eq!(
                u64_at(&observed, 0x280),
                u64_at(&observed, 0x88),
                "native return flags survive helper"
            );
            assert_eq!(h(10).load(Ordering::Acquire), next + 2);
        }
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
        let next = h(10).load(Ordering::Acquire);
        let mut late = vec![0u8; 0x500];
        unsafe {
            root_probe_driver(late.as_mut_ptr(), gateways()[0]);
        }
        assert_eq!(u64_at(&late, 0x80), 1);
        assert_eq!(h(10).load(Ordering::Acquire), next);
        assert_eq!(h(12).load(Ordering::Acquire), ROOT_CLOSED);
    }
    #[test]
    fn root_capacity_and_depth_overflow_preserve_original() {
        if !child("root_capacity_and_depth_overflow_preserve_original") {
            return;
        }
        let ret = root_probe_return as *const () as u64;
        bind_fixture(1, ret, ret);
        ROOT_CONTINUE[0].store(root_probe_world as *const () as u64 + 6, Ordering::Release);
        STACK.with(|s| {
            s.set(Stack {
                depth: DEPTH,
                frames: [Invocation::default(); DEPTH],
            })
        });
        let mut data = vec![0u8; 0x500];
        unsafe {
            root_probe_driver(data.as_mut_ptr(), gateways()[0]);
        }
        assert_eq!(u64_at(&data, 0x80), 1);
        assert_eq!(STACK.with(|s| s.get().depth), DEPTH);
        assert!(h(14).load(Ordering::Acquire) > 0);
        STACK.with(|s| {
            s.set(Stack {
                depth: 0,
                frames: [Invocation::default(); DEPTH],
            })
        });
        for _ in 0..64 {
            unsafe {
                root_probe_driver(data.as_mut_ptr(), gateways()[0]);
            }
        }
        assert_eq!(u64_at(&data, 0x80), 65);
        assert_eq!(h(10).load(Ordering::Acquire), 128);
        assert_eq!(h(11).load(Ordering::Acquire), 1);
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
        assert_eq!(h(12).load(Ordering::Acquire), ROOT_CLOSED);
    }

    #[test]
    fn root_expired_unknown_manager_runs_original_without_dirtying_closed_capture() {
        if !child("root_expired_unknown_manager_runs_original_without_dirtying_closed_capture") {
            return;
        }
        initialize(config(1), 1, 1);
        assert!(BIND
            .set(Binding {
                core: 1,
                manager_return: 1 + 0x63e1d5,
                local_manager_return: 1 + 0x63d448,
                wait_return: 1 + 0x4a1196,
                until: Instant::now() - Duration::from_secs(1),
            })
            .is_ok());
        h(19).store(3, Ordering::Release);
        ROOT_TEST_ORIGINALS[1].store(root_probe_manager as *const () as u64, Ordering::Release);
        let mut data = vec![0u8; 0x500];
        unsafe {
            SetLastError(0x1234);
            root_probe_driver(data.as_mut_ptr(), gateways()[1]);
        }
        assert_eq!(
            u64_at(&data, 0x80),
            1,
            "expired capture preserves original call"
        );
        assert_eq!(u64_at(&data, 0x90), 0x1234);
        assert_eq!(u64_at(&data, 0x280), u64_at(&data, 0x88));
        assert_eq!(
            h(14).load(Ordering::Acquire),
            0,
            "deadline forbids a late unknown counter"
        );
        assert_eq!(h(10).load(Ordering::Acquire), 0);
        assert_eq!(h(12).load(Ordering::Acquire), ROOT_CLOSED);
        let frozen = bytes();
        unsafe {
            root_probe_driver(data.as_mut_ptr(), gateways()[1]);
        }
        assert_eq!(u64_at(&data, 0x80), 2);
        assert_eq!(bytes(), frozen);
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
        let stopped = bytes();
        unsafe {
            root_probe_driver(data.as_mut_ptr(), gateways()[1]);
        }
        assert_eq!(u64_at(&data, 0x80), 3);
        assert_eq!(bytes(), stopped);
    }

    #[test]
    fn root_unknown_manager_within_budget_records_once_then_stop_freezes_it() {
        if !child("root_unknown_manager_within_budget_records_once_then_stop_freezes_it") {
            return;
        }
        bind_fixture(1, 1 + 0x63e1d5, 1 + 0x4a1196);
        ROOT_TEST_ORIGINALS[1].store(root_probe_manager as *const () as u64, Ordering::Release);
        let mut data = vec![0u8; 0x500];
        unsafe {
            root_probe_driver(data.as_mut_ptr(), gateways()[1]);
        }
        assert_eq!(u64_at(&data, 0x80), 1);
        assert_eq!(
            h(14).load(Ordering::Acquire),
            1,
            "unrecognized in-window call is not ignored"
        );
        assert_eq!(h(12).load(Ordering::Acquire), 0);
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
        let frozen = bytes();
        unsafe {
            root_probe_driver(data.as_mut_ptr(), gateways()[1]);
        }
        assert_eq!(u64_at(&data, 0x80), 2);
        assert_eq!(bytes(), frozen);
    }
    #[test]
    fn root_displaced_shims_preserve_native_rax_inputs_and_output_state() {
        if !child("root_displaced_shims_preserve_native_rax_inputs_and_output_state") {
            return;
        }
        let ret = root_probe_return as *const () as u64;
        bind_fixture(1, ret, ret);
        for (i, body) in [
            root_probe_world as *const () as usize,
            root_probe_manager as *const () as usize,
            root_probe_wait as *const () as usize,
        ]
        .into_iter()
        .enumerate()
        {
            ROOT_CONTINUE[i].store(
                (body + if i == 0 { 6 } else { 5 }) as u64,
                Ordering::Release,
            );
            // Unlike the generic state test, this path executes every actual
            // displaced-prologue shim, including its call-local branch load.
            let mut original = vec![0u8; 0x500];
            let mut observed = vec![0u8; 0x500];
            unsafe {
                SetLastError(0x1234);
                root_probe_driver(original.as_mut_ptr(), body);
            }
            unsafe {
                SetLastError(0x1234);
                root_probe_driver(observed.as_mut_ptr(), gateways()[i]);
            }
            assert_eq!(
                &observed[..48],
                &original[..48],
                "native volatile inputs at shim {i}"
            );
            assert_eq!(
                u64_at(&observed, 0),
                0x11223344,
                "native RAX input at shim {i}"
            );
            assert_eq!(u64_at(&observed, 32), 0xbbcc);
            assert_eq!(u64_at(&observed, 40), 0xddee);
            if i != 0 {
                assert_eq!(
                    u64_at(&observed, 56),
                    u64_at(&observed, 0x98),
                    "MOV-only shim preserves flags"
                );
            }
            assert_eq!(u64_at(&observed, 0x80), 1);
            assert_eq!(u64_at(&observed, 0x90), 0x1234);

            assert_eq!(unsafe { GetLastError() }, 0x880);
            assert_eq!(&observed[0x200..0x238], &original[0x200..0x238]);
            assert_eq!(&observed[0x300..0x360], &original[0x300..0x360]);
            assert_eq!(u64_at(&observed, 0x280), u64_at(&observed, 0x88));
        }
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
    }
    global_asm!(
        ".globl root_unwind_driver",".seh_proc root_unwind_driver","root_unwind_driver:",
        "push rbx",".seh_pushreg rbx","sub rsp,0x80",".seh_stackalloc 0x80",".seh_endprologue",
        "mov rbx,rcx","mov [rcx],rsp","lea rax,[rip+root_unwind_landed]","mov [rcx+8],rax",
        "call rdx","xor eax,eax","jmp 2f",".globl root_unwind_landed","root_unwind_landed:",
        "mov eax,1","2:","add rsp,0x80","pop rbx","ret",".seh_endproc",
        ".p2align 4",".globl root_unwind_original",".seh_proc root_unwind_original","root_unwind_original:",
        ".byte 0x40,0x53",".seh_pushreg rbx","sub rsp,0x20",".seh_stackalloc 0x20",
        "sub rsp,0x520",".seh_stackalloc 0x520",".seh_endprologue",
        "mov rdx,[rcx+8]","mov rcx,[rcx]","lea r10,[rsp+0x40]","mov r11d,0x9a",
        "1:","mov qword ptr [r10],0","lea r10,[r10+8]","dec r11d","jnz 1b",
        "mov [rsp+0x20],r10","lea r10,[rsp+0x40]","mov [rsp+0x20],r10",
        "mov qword ptr [rsp+0x28],0","xor r8d,r8d","xor r9d,r9d",
        "call {rtl_unwind}","int3",".seh_endproc",
        rtl_unwind=sym RtlUnwindEx,
    );
    unsafe extern "system" {
        fn root_unwind_driver(state: *mut u64, target: usize) -> u64;
        fn root_unwind_original();
    }
    #[test]
    fn root_actual_seh_unwind_settles_reserved_row_without_return() {
        if !child("root_actual_seh_unwind_settles_reserved_row_without_return") {
            return;
        }
        bind_fixture(1, 0, 0);
        ROOT_CONTINUE[0].store(
            root_unwind_original as *const () as u64 + 6,
            Ordering::Release,
        );
        let mut state = [0u64; 2];
        let landed = unsafe { root_unwind_driver(state.as_mut_ptr(), gateways()[0]) };
        assert_eq!(landed, 1);
        assert_eq!(h(13).load(Ordering::Acquire), 1);
        assert_eq!(STACK.with(|s| s.get().depth), 0);
        assert_eq!(h(10).load(Ordering::Acquire), 2);
        assert_eq!(TRACE.rows[1][2].load(Ordering::Acquire), 3);
        assert_eq!(TRACE.rows[1][18].load(Ordering::Acquire), 0);
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
        assert_eq!(h(12).load(Ordering::Acquire), ROOT_CLOSED);
    }
    #[test]
    fn root_virtual_unwind_gateway_and_displaced_prologues() {
        if !child("root_virtual_unwind_gateway_and_displaced_prologues") {
            return;
        }
        unsafe extern "system" {
            fn root_world_gateway_body();
            fn root_world_original_pushed();
            fn root_world_original_body();
            fn root_world_original_branch();
            fn root_manager_original_body();
            fn root_manager_original_branch();
            fn root_wait_original_body();
            fn root_wait_original_branch();
        }
        let targets = [
            (
                root_world_gateway_body as *const () as usize,
                0x1c8usize,
                None,
                "gateway body",
            ),
            (
                root_world_original as *const () as usize,
                0,
                None,
                "world entry",
            ),
            (
                root_world_original_pushed as *const () as usize,
                8,
                Some(0),
                "world after push",
            ),
            (
                root_world_original_body as *const () as usize,
                0x28,
                Some(0x20),
                "world body",
            ),
            (
                root_world_original_branch as *const () as usize,
                0x28,
                Some(0x20),
                "world branch",
            ),
            (
                root_manager_original as *const () as usize,
                0,
                None,
                "manager entry",
            ),
            (
                root_manager_original_body as *const () as usize,
                0,
                Some(0x10),
                "manager body",
            ),
            (
                root_manager_original_branch as *const () as usize,
                0,
                Some(0x10),
                "manager branch",
            ),
            (
                root_wait_original as *const () as usize,
                0,
                None,
                "wait entry",
            ),
            (
                root_wait_original_body as *const () as usize,
                0,
                Some(8),
                "wait body",
            ),
            (
                root_wait_original_branch as *const () as usize,
                0,
                Some(8),
                "wait branch",
            ),
        ];
        for (address, allocation, saved_rbx, point) in targets {
            let mut stack = [0u64; 128];
            stack[allocation / 8] = 0x11223344;
            if let Some(saved_rbx) = saved_rbx {
                stack[saved_rbx / 8] = 0x55667788;
            }
            let mut base = 0;
            let entry = unsafe { RtlLookupFunctionEntry(address as u64, &mut base, null_mut()) };
            assert!(!entry.is_null(), "{point}");
            let mut ctx: CONTEXT = unsafe { std::mem::zeroed() };
            ctx.Rip = address as u64;
            ctx.Rsp = stack.as_mut_ptr() as u64;
            ctx.Rbx = if saved_rbx.is_some() {
                0xaabbccdd
            } else {
                0x55667788
            };
            let mut handler = null_mut();
            let mut frame = 0;
            unsafe {
                RtlVirtualUnwind(
                    0,
                    base,
                    ctx.Rip,
                    entry,
                    &mut ctx,
                    &mut handler,
                    &mut frame,
                    null_mut(),
                );
            }
            assert_eq!(ctx.Rip, 0x11223344, "{point}: caller RIP");
            assert_eq!(
                ctx.Rsp,
                stack.as_mut_ptr() as u64 + allocation as u64 + 8,
                "{point}: caller RSP"
            );
            assert_eq!(ctx.Rbx, 0x55667788, "{point}: saved RBX restored");
        }
        for branch in [
            root_world_original_branch as *const () as u64,
            root_manager_original_branch as *const () as u64,
            root_wait_original_branch as *const () as u64,
        ] {
            assert_eq!(
                copy_current::<4>(branch),
                Some([0xff, 0x64, 0x24, 0x18]),
                "stack-relative JMP mod01 must not be replaced by epilogue-like mod00"
            );
        }
    }
    unsafe extern "system" {
        fn root_flags_restore_pushed();
        fn root_flags_restore_pop();
        fn root_flags_restore_ret();
        fn root_flags_save_pushed();
        fn root_flags_save_pop();
        fn root_flags_save_ret();
        fn root_world_gateway_flags_input_return();
        fn root_world_gateway_flags_save_return();
        fn root_world_gateway_flags_output_return();
    }
    #[test]
    fn root_flags_helpers_virtual_unwind_all_boundaries_and_invocation_settlement() {
        if !child("root_flags_helpers_virtual_unwind_all_boundaries_and_invocation_settlement") {
            return;
        }
        bind_fixture(1, 0, 0);
        let helpers = [
            (
                root_flags_restore as *const () as usize,
                root_flags_restore_pushed as *const () as usize,
                root_flags_restore_pop as *const () as usize,
                root_flags_restore_ret as *const () as usize,
                root_world_gateway_flags_input_return as *const () as u64,
                false,
            ),
            (
                root_flags_save as *const () as usize,
                root_flags_save_pushed as *const () as usize,
                root_flags_save_pop as *const () as usize,
                root_flags_save_ret as *const () as usize,
                root_world_gateway_flags_save_return as *const () as u64,
                false,
            ),
            (
                root_flags_restore as *const () as usize,
                root_flags_restore_pushed as *const () as usize,
                root_flags_restore_pop as *const () as usize,
                root_flags_restore_ret as *const () as usize,
                root_world_gateway_flags_output_return as *const () as u64,
                true,
            ),
        ];
        for (entry, pushed, pop, ret, caller, already_settled) in helpers {
            for (pc, after_push) in [(entry, false), (pushed, true), (pop, true), (ret, false)] {
                let mut storage = [0u64; 160];
                let gw = unsafe { storage.as_mut_ptr().add(2) };
                storage[0] = 0x202;
                storage[1] = caller;
                storage[2 + 0x1c8 / 8] = 0x11223344;
                let raw = unsafe { (gw.cast::<u8>()).add(0x20).cast::<Saved>() };
                unsafe {
                    ptr::write(
                        raw,
                        Saved {
                            gpr: [0u64; 8],
                            xmm: [[0; 16]; 6],
                            invocation: Invocation {
                                kind: 1,
                                caller: 0x11223344,
                                caller_rsp: gw as u64 + 0x1c8,
                                native_rsp: gw as u64 - 8,
                                ..Invocation::default()
                            },
                            metadata: 0,
                        },
                    );
                }
                begin(raw);
                let token = unsafe { (*raw).invocation.token };
                let binding = unsafe { (*raw).invocation };
                assert!(token > 0);
                if already_settled {
                    end(raw);
                }
                assert_eq!(
                    h(12).load(Ordering::Acquire),
                    if already_settled { 0 } else { 1 }
                );
                let mut ctx: CONTEXT = unsafe { std::mem::zeroed() };
                ctx.Rip = pc as u64;
                ctx.Rsp = storage.as_mut_ptr() as u64 + if after_push { 0 } else { 8 };
                ctx.Rbx = 0xaabbccdd;
                let mut base = 0;
                let table = unsafe { RtlLookupFunctionEntry(ctx.Rip, &mut base, null_mut()) };
                assert!(!table.is_null());
                let mut data = null_mut();
                let mut frame = 0;
                unsafe {
                    RtlVirtualUnwind(
                        0,
                        base,
                        ctx.Rip,
                        table,
                        &mut ctx,
                        &mut data,
                        &mut frame,
                        null_mut(),
                    );
                }
                assert_eq!(ctx.Rip, caller, "helper at {pc:#x}");
                assert_eq!(ctx.Rsp, gw as u64, "helper returns exact gateway RSP");
                assert_eq!(ctx.Rbx, 0xaabbccdd);
                let table = unsafe { RtlLookupFunctionEntry(ctx.Rip, &mut base, null_mut()) };
                assert!(!table.is_null());
                // Obtain the real gateway language handler. VirtualUnwind
                // returns its address; it does not execute the language handler.
                let handler = unsafe {
                    RtlVirtualUnwind(
                        2,
                        base,
                        ctx.Rip,
                        table,
                        &mut ctx,
                        &mut data,
                        &mut frame,
                        null_mut(),
                    )
                };
                assert_eq!(ctx.Rip, 0x11223344);
                assert_eq!(ctx.Rsp, gw as u64 + 0x1d0);
                if !already_settled {
                    assert_eq!(handler, unwind as *const () as usize);
                    assert_eq!(frame, gw as u64);
                    let mut exception: EXCEPTION_RECORD = unsafe { std::mem::zeroed() };
                    exception.ExceptionFlags = 2;
                    let language: unsafe extern "system" fn(
                        *const EXCEPTION_RECORD,
                        usize,
                        *mut CONTEXT,
                        *mut c_void,
                    ) -> i32 = unsafe {
                        std::mem::transmute::<
                            usize,
                            unsafe extern "system" fn(
                                *const EXCEPTION_RECORD,
                                usize,
                                *mut CONTEXT,
                                *mut c_void,
                            ) -> i32,
                        >(handler)
                    };
                    unsafe {
                        language(&exception, frame as usize, &mut ctx, null_mut());
                    }
                    assert_eq!(TRACE.rows[token as usize][2].load(Ordering::Acquire), 3);
                } else {
                    // Original exit was committed before output-flags restore.
                    // A later unwind must not overwrite its row or settle twice.
                    assert_eq!(TRACE.rows[token as usize][2].load(Ordering::Acquire), 2);
                }
                assert_eq!(h(12).load(Ordering::Acquire), 0);
                assert_eq!(STACK.with(|s| s.get().depth), 0);
                assert_eq!(
                    TRACE.rows[token as usize][5].load(Ordering::Acquire),
                    binding.tid
                );
                assert_eq!(
                    TRACE.rows[token as usize][6].load(Ordering::Acquire),
                    binding.thread_created
                );
                assert_eq!(
                    TRACE.rows[token as usize][7].load(Ordering::Acquire),
                    binding.epoch
                );
                assert_eq!(
                    TRACE.rows[token as usize][8].load(Ordering::Acquire),
                    binding.caller
                );
            }
        }
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
    }
    #[unsafe(no_mangle)]
    static ROOT_HELPER_FAULT_HITS: AtomicU64 = AtomicU64::new(0);
    #[unsafe(no_mangle)]
    static ROOT_HELPER_FAULT_TARGET: AtomicU64 = AtomicU64::new(0);
    #[unsafe(no_mangle)]
    static ROOT_HELPER_FAULT_STATE: AtomicU64 = AtomicU64::new(0);
    #[unsafe(no_mangle)]
    static ROOT_HELPER_NATIVE_CALLS: AtomicU64 = AtomicU64::new(0);
    global_asm!(
        ".globl root_helper_fault",".seh_proc root_helper_fault","root_helper_fault:",
        "sub rsp,0x538",".seh_stackalloc 0x538",".seh_endprologue",
        "mov [rsp+0x20],rax","mov [rsp+0x28],rdx",
        "lock inc qword ptr [rip+ROOT_HELPER_FAULT_HITS]",
        "mov rax,[rip+ROOT_HELPER_FAULT_HITS]",
        "cmp rax,[rip+ROOT_HELPER_FAULT_TARGET]","jne 2f",
        "mov rax,[rip+ROOT_HELPER_FAULT_STATE]","mov rdx,[rax+8]","mov rcx,[rax]",
        "lea r10,[rsp+0x40]","mov r11d,0x9a",
        "1:","mov qword ptr [r10],0","lea r10,[r10+8]","dec r11d","jnz 1b",
        "lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0",
        "xor r8d,r8d","xor r9d,r9d","call {rtl_unwind}","int3",
        "2:","mov rax,[rsp+0x20]","mov rdx,[rsp+0x28]","lea rsp,[rsp+0x538]","ret",".seh_endproc",
        ".globl root_helper_native",".seh_proc root_helper_native","root_helper_native:",
        ".byte 0x40,0x53",".seh_pushreg rbx","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
        "lock inc qword ptr [rip+ROOT_HELPER_NATIVE_CALLS]","mov rax,0x55ff",
        "add rsp,0x20","pop rbx","ret",".seh_endproc",
        rtl_unwind=sym RtlUnwindEx,
    );
    unsafe extern "system" {
        fn root_helper_fault();
        fn root_helper_native();
    }
    struct HelperFaultPatch {
        address: usize,
        original: [u8; 5],
    }
    impl HelperFaultPatch {
        fn at(address: usize) -> Self {
            let original = copy_current::<5>(address as u64).unwrap();
            assert_eq!(original, [0x90; 5]);
            let displacement = i32::try_from(
                root_helper_fault as *const () as usize as i128 - address as i128 - 5,
            )
            .unwrap();
            let mut call = [0xe8, 0, 0, 0, 0];
            call[1..].copy_from_slice(&displacement.to_le_bytes());
            Self::write(address, call);
            Self { address, original }
        }
        fn write(address: usize, bytes: [u8; 5]) {
            let mut old = 0;
            let mut ignored = 0;
            assert_ne!(
                unsafe { VirtualProtect(address as _, 5, PAGE_EXECUTE_READWRITE, &mut old) },
                0
            );
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, 5);
            }
            assert_ne!(
                unsafe { VirtualProtect(address as _, 5, old, &mut ignored) },
                0
            );
            assert_ne!(
                unsafe { FlushInstructionCache(GetCurrentProcess(), address as _, 5) },
                0
            );
        }
    }
    impl Drop for HelperFaultPatch {
        fn drop(&mut self) {
            Self::write(self.address, self.original);
        }
    }
    #[test]
    fn root_flags_helpers_actual_seh_before_pop_and_single_settlement() {
        if !child("root_flags_helpers_actual_seh_before_pop_and_single_settlement") {
            return;
        }
        bind_fixture(1, 0, 0);
        ROOT_CONTINUE[0].store(
            root_helper_native as *const () as u64 + 6,
            Ordering::Release,
        );
        for (stage, window, hit, native, edge, exceptions) in [
            (
                1,
                root_flags_restore_pushed as *const () as usize,
                1,
                0,
                3,
                1,
            ),
            (2, root_flags_save_pushed as *const () as usize, 1, 1, 3, 2),
            (
                3,
                root_flags_restore_pushed as *const () as usize,
                2,
                2,
                2,
                2,
            ),
        ] {
            let mut state = [0u64; 2];
            ROOT_HELPER_FAULT_HITS.store(0, Ordering::Release);
            ROOT_HELPER_FAULT_TARGET.store(hit, Ordering::Release);
            ROOT_HELPER_FAULT_STATE.store(state.as_mut_ptr() as u64, Ordering::Release);
            let token = h(10).load(Ordering::Acquire) + 1;
            let patch = HelperFaultPatch::at(window);
            let result = unsafe { root_unwind_driver(state.as_mut_ptr(), gateways()[0]) };
            drop(patch);
            assert_eq!(
                result, 1,
                "stage {stage} must reach the real outer SEH target"
            );
            assert_eq!(ROOT_HELPER_NATIVE_CALLS.load(Ordering::Acquire), native);
            assert_eq!(
                h(12).load(Ordering::Acquire),
                0,
                "stage {stage} sample settled once"
            );
            assert_eq!(STACK.with(|s| s.get().depth), 0);
            assert_eq!(TRACE.rows[token as usize][2].load(Ordering::Acquire), edge);
            assert_eq!(h(13).load(Ordering::Acquire), exceptions);
            for field in [3, 5, 6, 7, 8, 9, 10, 21, 22] {
                assert_eq!(
                    TRACE.rows[token as usize - 1][field].load(Ordering::Acquire),
                    TRACE.rows[token as usize][field].load(Ordering::Acquire),
                    "stage {stage} binding {field}"
                );
            }
        }
        ROOT_HELPER_FAULT_STATE.store(0, Ordering::Release);
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
        assert_eq!(h(10).load(Ordering::Acquire), 6);
    }
    // Metadata-only functions provide SDK-generated full native unwind tables
    // for the three owned code fixtures. Their addresses are never game RVAs.
    global_asm!(
        ".globl root_meta_world",
        ".seh_proc root_meta_world",
        "root_meta_world:",
        ".byte 0x40,0x53",
        ".seh_pushreg rbx",
        "sub rsp,0x20",
        ".seh_stackalloc 0x20",
        ".seh_endprologue",
        "add rsp,0x20",
        "pop rbx",
        "ret",
        ".seh_endproc",
        ".globl root_meta_local_caller",
        ".seh_proc root_meta_local_caller",
        "root_meta_local_caller:",
        "push rdi",
        ".seh_pushreg rdi",
        "push r14",
        ".seh_pushreg r14",
        "sub rsp,0x28",
        ".seh_stackalloc 0x28",
        ".seh_endprologue",
        "add rsp,0x28",
        "pop r14",
        "pop rdi",
        "ret",
        ".seh_endproc",
        ".globl root_meta_manager",
        ".seh_proc root_meta_manager",
        "root_meta_manager:",
        "mov [rsp+0x10],rbx",
        ".seh_savereg rbx,0x10",
        "push rsi",
        ".seh_pushreg rsi",
        "sub rsp,0x30",
        ".seh_stackalloc 0x30",
        ".seh_endprologue",
        "mov rbx,[rsp+0x48]",
        "add rsp,0x30",
        "pop rsi",
        "ret",
        ".seh_endproc",
        ".globl root_meta_wait",
        ".seh_proc root_meta_wait",
        "root_meta_wait:",
        "mov [rsp+8],rbx",
        ".seh_savereg rbx,8",
        "push rdi",
        ".seh_pushreg rdi",
        "sub rsp,0x30",
        ".seh_stackalloc 0x30",
        ".seh_endprologue",
        "mov rbx,[rsp+0x40]",
        "add rsp,0x30",
        "pop rdi",
        "ret",
        ".seh_endproc",
    );
    unsafe extern "system" {
        fn root_meta_world();
        fn root_meta_manager();
        fn root_meta_wait();
        fn root_meta_local_caller();
    }
    struct OwnedCore {
        base: *mut u8,
        table: Box<[IMAGE_RUNTIME_FUNCTION_ENTRY; 4]>,
    }
    impl OwnedCore {
        fn new() -> Self {
            let base = unsafe {
                VirtualAlloc(
                    null_mut(),
                    0xa10000,
                    MEM_RESERVE | MEM_COMMIT,
                    PAGE_READWRITE,
                )
            } as *mut u8;
            assert!(!base.is_null());
            let mut world = vec![0x90u8; 0x180];
            world[..6].copy_from_slice(&WORDS[0][..6]);
            world[6..23].copy_from_slice(&[
                0x80, 0xb9, 0xa4, 2, 0, 0, 0, 0x48, 0x8b, 0xd9, 0x0f, 0x84, 0x63, 1, 0, 0, 0x90,
            ]);
            // Original conditional jump ends at+16h, not+17h.
            world[23..30].copy_from_slice(&[0x48, 0xff, 0x81, 0x80, 0, 0, 0]);
            world[0x69..0x75].copy_from_slice(WORLD_CALL);
            world[0x75..0x7b].copy_from_slice(&[0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3]);
            world[0x179..0x17f].copy_from_slice(&[0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3]);
            let mut manager = MANAGER_PREFIX.to_vec();
            manager.extend([
                0x48, 0xff, 0x83, 0x80, 0, 0, 0, // original manager counter
                0x48, 0x8b, 0x5c, 0x24, 0x48, 0x48, 0x83, 0xc4, 0x30, 0x5e, 0xc3,
            ]);
            let wait = vec![
                0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83, 0xec, 0x30, 0x48, 0xff,
                0x01, // original Wait counter in set+0
                0x48, 0xc7, 0x41, 8, 0, 0, 0, 0, // owned fixture's post-call value
                0xb8, 0xff, 0x55, 0, 0, 0x48, 0x8b, 0x5c, 0x24, 0x40, 0x48, 0x83, 0xc4, 0x30, 0x5f,
                0xc3,
            ];
            const LOCAL_ENTRY: usize = 0x63d424;
            let mut local = vec![0x57, 0x41, 0x56, 0x48, 0x83, 0xec, 0x28, 0x48, 0x8b, 0xf9];
            local.resize(0x63d43c - LOCAL_ENTRY, 0x90);
            local.extend_from_slice(LOCAL_MANAGER_CALL);
            local.extend_from_slice(LOCAL_MANAGER_CONTINUATION);
            local.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0x41, 0x5e, 0x5f, 0xc3]);
            // Only test-owned downstream bodies. Their role is not attributed
            // to the real functions at these same RVAs.
            unsafe {
                *base.add(0x3d50b0) = 0xc3;
                *base.add(0x49c7c0) = 0xc3;
            }
            let code = [world, manager, wait, local];
            let starts = [SITE[0], SITE[1], SITE[2], LOCAL_ENTRY];
            let metadata = [
                root_meta_world as *const () as usize,
                root_meta_manager as *const () as usize,
                root_meta_wait as *const () as usize,
                root_meta_local_caller as *const () as usize,
            ];
            let mut table =
                Box::new(unsafe { std::mem::zeroed::<[IMAGE_RUNTIME_FUNCTION_ENTRY; 4]>() });
            for i in 0..4 {
                unsafe {
                    ptr::copy_nonoverlapping(code[i].as_ptr(), base.add(starts[i]), code[i].len());
                }
                let mut image = 0;
                let entry =
                    unsafe { RtlLookupFunctionEntry(metadata[i] as u64, &mut image, null_mut()) };
                assert!(!entry.is_null());
                let unwind_address = image + unsafe { (*entry).Anonymous.UnwindData as u64 };
                let header = copy_current::<4>(unwind_address).unwrap();
                assert_eq!(header[0] >> 3, 0, "fixture has no handler/chained metadata");
                let length = (4 + usize::from(header[2]) * 2 + 3) & !3;
                assert!(length <= 64);
                let raw =
                    unsafe { std::slice::from_raw_parts(unwind_address as *const u8, length) };
                let rva = 0xa00000 + i * 64;
                unsafe {
                    ptr::copy_nonoverlapping(raw.as_ptr(), base.add(rva), length);
                }
                table[i].BeginAddress = starts[i] as u32;
                table[i].EndAddress = (starts[i] + code[i].len()) as u32;
                table[i].Anonymous.UnwindData = rva as u32;
            }
            table.sort_by_key(|e| e.BeginAddress);
            let mut old = 0;
            assert_ne!(
                unsafe { VirtualProtect(base.cast(), 0xa10000, PAGE_EXECUTE_READ, &mut old) },
                0
            );
            assert_ne!(
                unsafe { FlushInstructionCache(GetCurrentProcess(), base.cast(), 0xa10000) },
                0
            );
            assert_ne!(
                unsafe { RtlAddFunctionTable(table.as_mut_ptr(), 4, base as u64) },
                0
            );
            Self { base, table }
        }
        fn prepare(&self) {
            let core = self.base as usize;
            bind_fixture(core, core as u64 + 0x63e1d5, core as u64 + 0x4a1196);
            assert!(unsafe { install_sites(core, WORDS) }.is_ok());
        }
        fn invoke(&self, world: *mut u8) -> u64 {
            let function = unsafe {
                std::mem::transmute::<*mut u8, unsafe extern "system" fn(*mut u8) -> u64>(
                    self.base.add(SITE[0]),
                )
            };
            unsafe { function(world) }
        }
        fn mutate_word(&self, i: usize, value: [u8; 8]) {
            let target = unsafe { self.base.add(SITE[i]) };
            let mut old = 0;
            let mut ignored = 0;
            assert_ne!(
                unsafe { VirtualProtect(target.cast(), 8, PAGE_EXECUTE_READWRITE, &mut old) },
                0
            );
            unsafe {
                ptr::copy_nonoverlapping(value.as_ptr(), target, 8);
            }
            assert_ne!(
                unsafe { VirtualProtect(target.cast(), 8, old, &mut ignored) },
                0
            );
            assert_ne!(
                unsafe { FlushInstructionCache(GetCurrentProcess(), target.cast(), 8) },
                0
            );
        }
    }
    impl Drop for OwnedCore {
        fn drop(&mut self) {
            assert!(unsafe { restore_all() }.is_ok());
            assert_ne!(
                unsafe { RtlDeleteFunctionTable(self.table.as_mut_ptr()) },
                0
            );
            // The child retains original code too. No late fetched continuation
            // becomes an executable pointer into a freed owned fixture.
        }
    }
    #[test]
    fn root_three_real_cas_sites_same_manager_native_rsp_and_return() {
        if !child("root_three_real_cas_sites_same_manager_native_rsp_and_return") {
            return;
        }
        let core = OwnedCore::new();
        core.prepare();
        let mut world = vec![0u8; 0x400];
        let mut manager = vec![0u8; 0x3900];
        world[0x2a4] = 1;
        world[0x250..0x258].copy_from_slice(&(manager.as_ptr() as u64).to_le_bytes());
        manager[0x408..0x410].copy_from_slice(&0xaabbu64.to_le_bytes());
        manager[0x410..0x418].copy_from_slice(&9u64.to_le_bytes());
        assert_eq!(core.invoke(world.as_mut_ptr()), 0x55ff);
        assert_eq!(u64_at(&world, 0x80), 1);
        assert_eq!(u64_at(&manager, 0x80), 1);
        assert_eq!(u64_at(&manager, 0x400), 1);
        assert_eq!(h(10).load(Ordering::Acquire), 6);
        assert_eq!(
            TRACE.rows[4][8].load(Ordering::Acquire),
            core.base as u64 + 0x4a1196
        );
        assert_eq!(
            TRACE.rows[4][9].load(Ordering::Acquire) + 0x40,
            TRACE.rows[2][21].load(Ordering::Acquire)
        );
        assert_ne!(
            TRACE.rows[2][9].load(Ordering::Acquire),
            TRACE.rows[2][21].load(Ordering::Acquire),
            "gateway raw RSP is not native manager entry RSP"
        );
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
        assert_eq!(h(12).load(Ordering::Acquire), ROOT_CLOSED);
        let trace = civ6_bootstrap_rs::protocol::root_trace(
            &bytes(),
            config(1).pid,
            config(1).created_filetime,
        )
        .unwrap();
        assert_eq!(trace["same_manager_wait_normal_returns"], 1);
        assert_eq!(
            trace["capture_complete"], true,
            "synthetic call correspondence only"
        );
        assert!(
            trace["not_established_by_this_capture"]
                .as_array()
                .unwrap()
                .len()
                >= 8
        );
        assert!(unsafe { restore_all() }.is_ok());
    }
    #[test]
    fn root_local_manager_three_cas_wait_is_separate_from_world_scope() {
        if !child("root_local_manager_three_cas_wait_is_separate_from_world_scope") {
            return;
        }
        let core = OwnedCore::new();
        assert!(verify_call_spans(core.base as usize).is_ok());
        core.prepare();
        let mut manager = vec![0u8; 0x3900];
        let address = manager.as_mut_ptr() as u64;
        let mut opaque_caller_receiver = vec![0u8; 0x400];
        opaque_caller_receiver[0x250..0x258].copy_from_slice(&address.to_le_bytes());
        let base = core.base as usize;
        let worker = std::thread::spawn(move || {
            let call = unsafe {
                std::mem::transmute::<usize, unsafe extern "system" fn(*mut u8) -> u64>(
                    base + 0x63d424,
                )
            };
            unsafe { call(opaque_caller_receiver.as_mut_ptr()) }
        });
        assert_eq!(worker.join().unwrap(), 0x55ff);
        assert_eq!(h(10).load(Ordering::Acquire), 4);
        assert_eq!(h(14).load(Ordering::Acquire), 0);
        let local = civ6_bootstrap_rs::protocol::root_trace(
            &bytes(),
            config(1).pid,
            config(1).created_filetime,
        )
        .unwrap();
        assert_eq!(local["local_manager_wait_normal_returns"], 1);
        assert_eq!(local["same_manager_wait_normal_returns"], 0);
        assert_eq!(local["capture_complete"], false);
        assert_eq!(local["rows"][0]["parent"], 0);
        assert_eq!(local["rows"][0]["world"], 0);
        assert_eq!(local["rows"][2]["parent"], 1);
        assert_eq!(local["rows"][2]["world"], 0);
        assert_eq!(
            local["rows"][2]["gateway_entry_rsp"].as_u64().unwrap() + 0x40,
            local["rows"][0]["native_entry_rsp"].as_u64().unwrap()
        );
        let mut world = vec![0u8; 0x400];
        world[0x2a4] = 1;
        world[0x250..0x258].copy_from_slice(&address.to_le_bytes());
        assert_eq!(core.invoke(world.as_mut_ptr()), 0x55ff);
        assert_eq!(
            u64_at(&manager, 0x80),
            2,
            "each original manager called once"
        );
        assert_eq!(u64_at(&manager, 0x400), 2, "each original Wait called once");
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
        let both = civ6_bootstrap_rs::protocol::root_trace(
            &bytes(),
            config(1).pid,
            config(1).created_filetime,
        )
        .unwrap();
        assert_eq!(both["same_manager_wait_normal_returns"], 1);
        assert_eq!(both["local_manager_wait_normal_returns"], 1);
        assert_eq!(
            both["capture_complete"], true,
            "fixture call coverage, not world certification"
        );
        assert_ne!(both["rows"][0]["tid"], both["rows"][4]["tid"]);
        assert_eq!(both["rows"][4]["parent"], 0);
        assert_eq!(h(10).load(Ordering::Acquire), 10);
    }
    #[test]
    fn root_local_manager_spans_reject_modified_call_and_continuation() {
        if !child("root_local_manager_spans_reject_modified_call_and_continuation") {
            return;
        }
        let core = OwnedCore::new();
        let base = core.base as usize;
        assert!(verify_call_spans(base).is_ok());
        for rva in [0x63d443, 0x63d448, 0x63d460] {
            let original = copy_current::<5>((base + rva) as u64).unwrap();
            let mut bad = original;
            bad[1] ^= 1;
            HelperFaultPatch::write(base + rva, bad);
            let rejected = verify_call_spans(base).is_err();
            HelperFaultPatch::write(base + rva, original);
            assert!(rejected, "complete second-caller span at {rva:#x}");
            assert!(verify_call_spans(base).is_ok());
        }
        assert_eq!(h(15).load(Ordering::Acquire), 0);
    }
    #[test]
    fn root_local_manager_current_call_rejects_bad_rsp_lifetime_and_orphan_wait() {
        if !child("root_local_manager_current_call_rejects_bad_rsp_lifetime_and_orphan_wait") {
            return;
        }
        bind_fixture(1, 1 + 0x63e1d5, 1 + 0x4a1196);
        let mut manager = vec![0u8; 0x3900];
        let address = manager.as_mut_ptr() as u64;
        let metadata = [3u64, 4];
        let frame = |kind, caller, rsp, native, rcx, rbx| Saved {
            gpr: [rcx, metadata.as_ptr() as u64, 0, 0, 0, 0, 0, rbx],
            xmm: [[0; 16]; 6],
            invocation: Invocation {
                kind,
                caller,
                caller_rsp: rsp,
                native_rsp: native,
                ..Invocation::default()
            },
            metadata: metadata.as_ptr() as u64,
        };
        let mut m = frame(2, 1 + 0x63d448, 0x8100, 0x8000, address, 0xdead);
        begin(&mut m);
        assert_eq!(m.invocation.errors, 0);
        assert_eq!(m.invocation.world, 0);
        assert_eq!(m.invocation.parent, 0);
        let mut wrong = frame(3, 1 + 0x4a1196, 0x7fc8, 0x7e00, address + 0x400, address);
        begin(&mut wrong);
        assert_ne!(wrong.invocation.errors & 2, 0);
        end(&mut wrong);
        let original_parent = STACK.with(Cell::get);
        STACK.with(|s| {
            let mut f = s.get();
            f.frames[0].thread_created += 1;
            s.set(f);
        });
        let mut wrong = frame(3, 1 + 0x4a1196, 0x7fc0, 0x7e00, address + 0x400, address);
        begin(&mut wrong);
        assert_ne!(wrong.invocation.errors & 2, 0);
        end(&mut wrong);
        STACK.with(|s| s.set(original_parent));
        end(&mut m);
        let mut unknown_manager = frame(2, 1 + 0x63d449, 0x8100, 0x8000, address, 0xdead);
        let next = h(10).load(Ordering::Acquire);
        begin(&mut unknown_manager);
        assert_eq!(unknown_manager.invocation.token, 0);
        assert_eq!(h(10).load(Ordering::Acquire), next);
        let mut orphan = frame(3, 1 + 0x4a1196, 0x7fc0, 0x7e00, address + 0x400, address);
        begin(&mut orphan);
        assert_ne!(orphan.invocation.errors & 2, 0);
        assert_eq!(orphan.invocation.parent, 0);
        end(&mut orphan);
        assert_eq!(BridgeRootObservationStop(null_mut()), 0);
        let unknown = h(14).load(Ordering::Acquire);
        begin(&mut unknown_manager);
        assert_eq!(
            h(14).load(Ordering::Acquire),
            unknown,
            "closed capture remains immutable at unknown manager entries"
        );
        let capture = civ6_bootstrap_rs::protocol::root_trace(
            &bytes(),
            config(1).pid,
            config(1).created_filetime,
        )
        .unwrap();
        assert_eq!(capture["capture_complete"], false);
        assert_eq!(capture["local_manager_wait_normal_returns"], 0);
    }
    #[test]
    fn root_rollback_continues_after_failed_first_compare_and_retains_owner() {
        if !child("root_rollback_continues_after_failed_first_compare_and_retains_owner") {
            return;
        }
        let core = OwnedCore::new();
        core.prepare();
        let original_patched = HOOKS[0].lock().unwrap().as_ref().unwrap().hook.patched;
        let mut foreign = original_patched;
        foreign[7] ^= 1;
        core.mutate_word(0, foreign);
        assert!(unsafe { restore_all() }.is_err());
        assert!(HOOKS[0].lock().unwrap().is_some());
        assert!(HOOKS[1].lock().unwrap().is_none());
        assert!(HOOKS[2].lock().unwrap().is_none());
        assert_eq!(h(17).load(Ordering::Acquire), 7);
        assert_eq!(h(18).load(Ordering::Acquire) & 6, 6);
        assert_eq!(
            copy_current::<8>(core.base as u64 + SITE[1] as u64),
            Some(WORDS[1])
        );
        assert_eq!(
            copy_current::<8>(core.base as u64 + SITE[2] as u64),
            Some(WORDS[2])
        );
        core.mutate_word(0, original_patched);
        assert!(unsafe { restore_all() }.is_ok());
    }
    #[test]
    fn root_old_config_is_refused_without_hook_publication() {
        if !child("root_old_config_is_refused_without_hook_publication") {
            return;
        }
        let mut invalid = config(0);
        invalid.version = super::super::runtime::RUNTIME_CONFIG_VERSION;
        assert_ne!(
            BridgeRootObservationInstall((&mut invalid as *mut RootConfig).cast()),
            0
        );
        assert_eq!(h(15).load(Ordering::Acquire), 0);
        assert!(!USED.load(Ordering::Acquire));
        assert!(HOOKS.iter().all(|slot| slot.lock().unwrap().is_none()));
    }
    fn submission_fixture(core: usize) {
        submission::LEGACY_FIXTURE.store(true, Ordering::Release);
        submission::PROFILE.store(true, Ordering::Release);
        bind_fixture(core, 0, 0);
    }
    global_asm!(
        ".p2align 4",
        ".globl submission_fixture_push",
        ".seh_proc submission_fixture_push",
        "submission_fixture_push:",
        "mov [rsp+8],rbx",
        ".seh_savereg rbx,8",
        "mov [rsp+0x10],rbp",
        ".seh_savereg rbp,0x10",
        "mov [rsp+0x18],rsi",
        ".seh_savereg rsi,0x18",
        "push rdi",
        ".seh_pushreg rdi",
        "sub rsp,0x30",
        ".seh_stackalloc 0x30",
        ".seh_endprologue",
        "inc qword ptr [rcx+0x80]",
        "mov rax,[rdx]",
        "movups xmm0,[r9]",
        "mov rbx,[rsp+0x40]",
        "mov rbp,[rsp+0x48]",
        "mov rsi,[rsp+0x50]",
        "lea rsp,[rsp+0x30]",
        "pop rdi",
        "ret",
        ".seh_endproc",
        ".globl submission_fixture_push_end",
        "submission_fixture_push_end:",
        ".p2align 4",
        ".globl submission_fixture_publish",
        ".seh_proc submission_fixture_publish",
        "submission_fixture_publish:",
        ".byte 0x40,0x53",
        ".seh_pushreg rbx",
        "sub rsp,0x20",
        ".seh_stackalloc 0x20",
        ".seh_endprologue",
        ".byte 0x48,0x8b,0xc2",
        "inc qword ptr [rcx+0x80]",
        "add rsp,0x20",
        "pop rbx",
        "jmp rax",
        ".seh_endproc",
        ".globl submission_fixture_publish_end",
        "submission_fixture_publish_end:",
        ".globl submission_fixture_tail",
        "submission_fixture_tail:",
        "inc qword ptr [rcx+0x88]",
        "mov rax,0x12345678",
        "ret",
        ".globl submission_seh_tail_forward",
        ".seh_proc submission_seh_tail_forward",
        "submission_seh_tail_forward:",
        ".byte 0x40,0x53",
        ".seh_pushreg rbx",
        "sub rsp,0x20",
        ".seh_stackalloc 0x20",
        ".seh_endprologue",
        "lea rax,[rip+root_unwind_original]",
        "add rsp,0x20",
        "pop rbx",
        "jmp rax",
        ".seh_endproc",
    );
    unsafe extern "system" {
        fn submission_fixture_push();
        fn submission_fixture_push_end();
        fn submission_fixture_publish();
        fn submission_fixture_publish_end();
        fn submission_fixture_tail();
        fn submission_seh_tail_forward();
    }
    #[test]
    fn submission_owned_os_real_two_cas_and_entire_tail_frame_once() {
        if !child("submission_owned_os_real_two_cas_and_entire_tail_frame_once") {
            return;
        }
        let core = OwnedCore::new();
        let base = core.base as usize;
        submission_fixture(base);
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(core.base.cast(), 0xa10000, PAGE_EXECUTE_READWRITE, &mut old) },
            0
        );
        let mut table: Box<[IMAGE_RUNTIME_FUNCTION_ENTRY; 2]> =
            Box::new(unsafe { std::mem::zeroed() });
        for (i, (start, end)) in [
            (
                submission_fixture_push as *const () as usize,
                submission_fixture_push_end as *const () as usize,
            ),
            (
                submission_fixture_publish as *const () as usize,
                submission_fixture_publish_end as *const () as usize,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(end > start && end - start < 256);
            let code = unsafe { std::slice::from_raw_parts(start as *const u8, end - start) };
            assert_eq!(&code[..8], &submission::WORDS[i]);
            let mut image = 0;
            let entry = unsafe { RtlLookupFunctionEntry(start as u64, &mut image, null_mut()) };
            assert!(!entry.is_null());
            let unwind = image + unsafe { (*entry).Anonymous.UnwindData as u64 };
            let header = copy_current::<4>(unwind).unwrap();
            assert_eq!(header[0], 1);
            let length = (4 + usize::from(header[2]) * 2 + 3) & !3;
            assert!(length <= 64);
            let raw = unsafe { std::slice::from_raw_parts(unwind as *const u8, length) };
            let metadata = 0xa0f000 + i * 64;
            unsafe {
                ptr::copy_nonoverlapping(code.as_ptr(), core.base.add(site_rvas()[i]), code.len());
                ptr::copy_nonoverlapping(raw.as_ptr(), core.base.add(metadata), length);
            }
            table[i].BeginAddress = site_rvas()[i] as u32;
            table[i].EndAddress = (site_rvas()[i] + code.len()) as u32;
            table[i].Anonymous.UnwindData = metadata as u32;
        }
        assert_ne!(
            unsafe { VirtualProtect(core.base.cast(), 0xa10000, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        assert_ne!(
            unsafe { FlushInstructionCache(GetCurrentProcess(), core.base.cast(), 0xa10000) },
            0
        );
        assert_ne!(
            unsafe { RtlAddFunctionTable(table.as_mut_ptr(), 2, base as u64) },
            0
        );
        // Test gateways route through the same real displaced shims as release.
        submission::SUBMISSION_TEST_ORIGINALS[0].store(
            submission::submission_push_original as *const () as u64,
            Ordering::Release,
        );
        submission::SUBMISSION_TEST_ORIGINALS[1].store(
            submission::submission_publish_original as *const () as u64,
            Ordering::Release,
        );
        assert!(unsafe { install_sites(base, submission::WORDS) }.is_ok());
        let mut data = [0u64; 32];
        let job = [0xaabbccdd_u64; 4];
        let meta = [0x1122_u64, 0x3344];
        let submit = unsafe {
            std::mem::transmute::<
                usize,
                unsafe extern "system" fn(*mut u64, *const u64, u64, *const u64) -> u64,
            >(base + site_rvas()[0])
        };
        assert_eq!(
            unsafe { submit(data.as_mut_ptr(), job.as_ptr(), 7, meta.as_ptr()) },
            job[0]
        );
        let publish = unsafe {
            std::mem::transmute::<usize, unsafe extern "system" fn(*mut u64, usize, u64, u64) -> u64>(
                base + site_rvas()[1],
            )
        };
        assert_eq!(
            unsafe {
                publish(
                    data.as_mut_ptr(),
                    submission_fixture_tail as *const () as usize,
                    0,
                    0,
                )
            },
            0x12345678
        );
        assert_eq!(
            data[16], 2,
            "both complete native bodies called exactly once"
        );
        assert_eq!(
            data[17], 1,
            "publication tail target returned through the gateway once"
        );
        assert_eq!(h(15).load(Ordering::Acquire), 3);
        assert_eq!(h(10).load(Ordering::Acquire), 4);
        assert_eq!(
            submission::TRACE.rows[0][14].load(Ordering::Acquire),
            job[0]
        );
        assert_eq!(
            submission::TRACE.rows[0][18].load(Ordering::Acquire),
            meta[0]
        );
        assert_eq!(
            submission::TRACE.rows[3][21].load(Ordering::Acquire),
            0x12345678
        );
        assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
        // Continuations/shim code stay live after rollback and late admission.
        assert!(unsafe { restore_all() }.is_ok());
        assert_ne!(unsafe { RtlDeleteFunctionTable(table.as_mut_ptr()) }, 0);
    }
    #[test]
    fn submission_owned_os_four_args_flags_outputs_and_original_once() {
        if !child("submission_owned_os_four_args_flags_outputs_and_original_once") {
            return;
        }
        submission_fixture(1);
        for i in 0..2 {
            // Samples native INPUT flags before its first SUB, and outputs at
            // the assembly caller immediately after gateway RET. These are
            // actual four-argument native executions, not recorder reflection.
            submission::SUBMISSION_TEST_ORIGINALS[i]
                .store(root_probe_manager as *const () as u64, Ordering::Release);
            let mut baseline = vec![0; 0x500];
            let mut observed = vec![0; 0x500];
            unsafe {
                SetLastError(0x1234);
                root_probe_driver(
                    baseline.as_mut_ptr(),
                    root_probe_manager as *const () as usize,
                );
            }
            unsafe {
                SetLastError(0x1234);
                root_probe_driver(observed.as_mut_ptr(), gateways()[i]);
            }
            assert_eq!(unsafe { GetLastError() }, 0x880);
            assert_eq!(u64_at(&observed, 0x80), 1);
            assert_eq!(u64_at(&observed, 0x90), 0x1234);
            assert_eq!(
                submission::TRACE.rows[i * 2 + 1][34].load(Ordering::Acquire),
                0x3344,
                "native R8 output is captured before the end helper"
            );
            assert_eq!(&observed[..48], &baseline[..48]);
            assert_eq!(u64_at(&observed, 0), 0x11223344);
            assert_eq!(u64_at(&observed, 32), 0xbbcc);
            assert_eq!(u64_at(&observed, 40), 0xddee);
            assert_eq!(u64_at(&observed, 56), u64_at(&observed, 0x98));
            assert_eq!(u64_at(&observed, 56), u64_at(&baseline, 56));
            assert_eq!(&observed[0x100..0x160], &baseline[0x100..0x160]);
            assert_eq!(&observed[0x200..0x238], &baseline[0x200..0x238]);
            assert_eq!(&observed[0x300..0x360], &baseline[0x300..0x360]);
            assert_eq!(u64_at(&observed, 0x280), u64_at(&observed, 0x88));
            assert_eq!(u64_at(&observed, 0x280), u64_at(&baseline, 0x280));
        }
        assert_eq!(h(10).load(Ordering::Acquire), 4);
        assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
        let mut late = vec![0; 0x500];
        unsafe {
            root_probe_driver(late.as_mut_ptr(), gateways()[1]);
        }
        assert_eq!(u64_at(&late, 0x80), 1);
        assert_eq!(h(10).load(Ordering::Acquire), 4);
        assert_eq!(h(12).load(Ordering::Acquire), ROOT_CLOSED);
    }
    #[test]
    fn submission_owned_os_metadata_error_capacity_and_frame_overlap() {
        if !child("submission_owned_os_metadata_error_capacity_and_frame_overlap") {
            return;
        }
        submission_fixture(1);
        let mut job = [0x1122334455667788u64; 4];
        let mut a = Saved {
            gpr: [7, job.as_mut_ptr() as u64, 19, 1, 0, 0, 0, 0],
            xmm: [[0; 16]; 6],
            invocation: Invocation {
                kind: 4,
                caller_rsp: 0x1000,
                native_rsp: 0x800,
                ..Invocation::default()
            },
            metadata: 0,
        };
        begin(&mut a);
        assert_eq!(submission::TRACE.rows[0][24].load(Ordering::Acquire), 32);
        assert_eq!(
            submission::TRACE.rows[0][14].load(Ordering::Acquire),
            job[0]
        );
        assert_ne!(submission::TRACE.rows[0][27].load(Ordering::Acquire), 0);
        let mut overlap = Saved {
            gpr: [7, 0, 0, 0, 0, 0, 0, 0],
            xmm: [[0; 16]; 6],
            invocation: Invocation {
                kind: 5,
                caller_rsp: 0x1000,
                native_rsp: 0x800,
                ..Invocation::default()
            },
            metadata: 0,
        };
        begin(&mut overlap);
        assert_ne!(overlap.invocation.errors & 0x100, 0);
        finish(&mut overlap, 2, 0);
        // Make input memory unavailable after entry. Finishing must use the
        // frozen entry bytes, even if both register outputs are unrelated.
        job.fill(0);
        a.gpr[1] = u64::MAX;
        a.gpr[3] = u64::MAX;
        finish(&mut a, 2, 0);
        assert_eq!(
            submission::TRACE.rows[1][14].load(Ordering::Acquire),
            0x1122334455667788
        );
        assert_eq!(submission::TRACE.rows[1][24].load(Ordering::Acquire), 32);
        let mut metadata = [0x8877u64; 2];
        a.gpr[1] = job.as_ptr() as u64;
        a.gpr[3] = metadata.as_mut_ptr() as u64;
        for _ in 0..ROOT_ROWS {
            a.invocation = Invocation {
                kind: 4,
                caller_rsp: 0x1000,
                native_rsp: 0x800,
                ..Invocation::default()
            };
            begin(&mut a);
            finish(&mut a, 2, 0);
        }
        assert_eq!(h(10).load(Ordering::Acquire), ROOT_ROWS as u64);
        assert!(h(11).load(Ordering::Acquire) > 0);
        assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    }
    #[test]
    fn submission_owned_os_actual_seh_settles_without_native_return() {
        if !child("submission_owned_os_actual_seh_settles_without_native_return") {
            return;
        }
        submission_fixture(1);
        ROOT_CONTINUE[1].store(
            root_unwind_original as *const () as u64 + 6,
            Ordering::Release,
        );
        submission::SUBMISSION_TEST_ORIGINALS[1].store(
            submission::submission_publish_original as *const () as u64,
            Ordering::Release,
        );
        let mut state = [0; 2];
        let landed = unsafe { root_unwind_driver(state.as_mut_ptr(), gateways()[1]) };
        assert_eq!(landed, 1);
        assert_eq!(h(13).load(Ordering::Acquire), 1);
        assert_eq!(STACK.with(|s| s.get().depth), 0);
        assert_eq!(h(10).load(Ordering::Acquire), 2);
        assert_eq!(submission::TRACE.rows[1][2].load(Ordering::Acquire), 3);
        assert_eq!(submission::TRACE.rows[1][21].load(Ordering::Acquire), 0);
        assert_ne!(
            submission::TRACE.rows[1][20].load(Ordering::Acquire) & 0x40,
            0
        );
        assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    }
    #[test]
    fn submission_owned_os_tail_target_seh_crosses_entire_wrapper() {
        if !child("submission_owned_os_tail_target_seh_crosses_entire_wrapper") {
            return;
        }
        submission_fixture(1);
        ROOT_CONTINUE[1].store(
            submission_seh_tail_forward as *const () as u64 + 6,
            Ordering::Release,
        );
        submission::SUBMISSION_TEST_ORIGINALS[1].store(
            submission::submission_publish_original as *const () as u64,
            Ordering::Release,
        );
        let mut state = [0; 2];
        assert_eq!(
            unsafe { root_unwind_driver(state.as_mut_ptr(), gateways()[1]) },
            1
        );
        assert_eq!(h(13).load(Ordering::Acquire), 1);
        assert_eq!(h(10).load(Ordering::Acquire), 2);
        assert_eq!(submission::TRACE.rows[1][2].load(Ordering::Acquire), 3);
        assert_eq!(submission::TRACE.rows[1][21].load(Ordering::Acquire), 0);
        assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    }
    #[test]
    fn submission_owned_os_safe_sixteen_bytes_and_no_post_return_reads() {
        if !child("submission_owned_os_safe_sixteen_bytes_and_no_post_return_reads") {
            return;
        }
        submission_fixture(1);
        let memory =
            unsafe { VirtualAlloc(null_mut(), 0x2000, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE) }
                as *mut u8;
        assert!(!memory.is_null());
        let mut old = 0;
        let prefix = [0x55u8; 16];
        unsafe {
            ptr::copy_nonoverlapping(prefix.as_ptr(), memory.add(0xff0), 16);
        }
        assert_ne!(
            unsafe {
                VirtualProtect(
                    memory.add(0x1000).cast(),
                    0x1000,
                    windows_sys::Win32::System::Memory::PAGE_NOACCESS,
                    &mut old,
                )
            },
            0
        );
        let meta = [0x7788u64; 2];
        let mut raw = Saved {
            gpr: [
                7,
                unsafe { memory.add(0xff0) } as u64,
                9,
                meta.as_ptr() as u64,
                0,
                0,
                0,
                0,
            ],
            xmm: [[0; 16]; 6],
            invocation: Invocation {
                kind: 4,
                caller_rsp: 0x1000,
                native_rsp: 0x800,
                ..Invocation::default()
            },
            metadata: 0,
        };
        begin(&mut raw);
        assert_eq!(submission::TRACE.rows[0][24].load(Ordering::Acquire), 16);
        assert_eq!(submission::TRACE.rows[0][25].load(Ordering::Acquire), 0);
        assert_eq!(
            submission::TRACE.rows[0][14].load(Ordering::Acquire),
            0x5555555555555555
        );
        assert_ne!(
            unsafe {
                VirtualProtect(
                    memory.cast(),
                    0x1000,
                    windows_sys::Win32::System::Memory::PAGE_NOACCESS,
                    &mut old,
                )
            },
            0
        );
        raw.gpr[1] = 0;
        raw.gpr[3] = 1;
        finish(&mut raw, 2, 0);
        assert_eq!(
            submission::TRACE.rows[1][14].load(Ordering::Acquire),
            0x5555555555555555
        );
        assert_eq!(submission::TRACE.rows[1][24].load(Ordering::Acquire), 16);
        assert_eq!(submission::TRACE.rows[1][20].load(Ordering::Acquire), 0);
        assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
        assert_ne!(unsafe { VirtualFree(memory.cast(), 0, MEM_RELEASE) }, 0);
    }
    #[test]
    fn submission_owned_os_depth_overflow_keeps_every_reservation_settlement() {
        if !child("submission_owned_os_depth_overflow_keeps_every_reservation_settlement") {
            return;
        }
        submission_fixture(1);
        let mut frames = Vec::new();
        for _ in 0..DEPTH + 2 {
            let mut raw = Saved {
                gpr: [0; 8],
                xmm: [[0; 16]; 6],
                invocation: Invocation {
                    kind: 5,
                    caller_rsp: 0x1000,
                    native_rsp: 0x800,
                    ..Invocation::default()
                },
                metadata: 0,
            };
            begin(&mut raw);
            frames.push(raw);
        }
        assert_ne!(frames.last().unwrap().invocation.errors & 0x10, 0);
        while let Some(mut raw) = frames.pop() {
            finish(&mut raw, 2, 0);
        }
        assert_eq!(STACK.with(|s| s.get().depth), 0);
        assert_eq!(h(12).load(Ordering::Acquire), 0);
        assert_eq!(h(10).load(Ordering::Acquire), ((DEPTH + 2) * 2) as u64);
        assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    }
    fn submission_partial(name: &str, site: usize) {
        if !child(name) {
            return;
        }
        let core = OwnedCore::new();
        let base = core.base as usize;
        submission_fixture(base);
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(core.base.cast(), 0xa10000, PAGE_EXECUTE_READWRITE, &mut old) },
            0
        );
        for i in 0..2 {
            unsafe {
                ptr::copy_nonoverlapping(
                    submission::WORDS[i].as_ptr(),
                    core.base.add(site_rvas()[i]),
                    8,
                );
            }
        }
        assert_ne!(
            unsafe { VirtualProtect(core.base.cast(), 0xa10000, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        FAIL_AFTER_PUBLISH.store(site, Ordering::Release);
        assert!(unsafe { install_sites(base, submission::WORDS) }.is_err());
        assert_eq!(h(15).load(Ordering::Acquire), (1 << site) - 1);
        assert!(unsafe { restore_all() }.is_ok());
        assert_eq!(h(17).load(Ordering::Acquire), 3);
        for (i, hook) in HOOKS.iter().enumerate().take(2) {
            assert_eq!(
                copy_current::<8>((base + site_rvas()[i]) as u64),
                Some(submission::WORDS[i])
            );
            assert!(hook.lock().unwrap().is_none());
        }
    }
    #[test]
    fn submission_owned_os_partial_submit_cas_rollback() {
        submission_partial("submission_owned_os_partial_submit_cas_rollback", 1);
    }
    #[test]
    fn submission_owned_os_partial_publish_cas_rollback() {
        submission_partial("submission_owned_os_partial_publish_cas_rollback", 2);
    }
    #[test]
    fn submission_owned_os_virtual_unwind_new_shims_and_gateways() {
        if !child("submission_owned_os_virtual_unwind_new_shims_and_gateways") {
            return;
        }
        unsafe extern "system" {
            fn submission_push_original_body();
            fn submission_push_original_branch();
            fn submission_publish_original_body();
            fn submission_publish_original_branch();
            fn submission_push_gateway_body();
            fn submission_publish_gateway_body();
        }
        for (pc, allocation, slot) in [
            (
                submission::submission_push_original as *const () as usize,
                0usize,
                None,
            ),
            (
                submission_push_original_body as *const () as usize,
                0,
                Some(8usize),
            ),
            (
                submission_push_original_branch as *const () as usize,
                0,
                Some(8),
            ),
            (
                submission::submission_publish_original as *const () as usize,
                0,
                None,
            ),
            (
                submission_publish_original_body as *const () as usize,
                0x28,
                Some(0x20),
            ),
            (
                submission_publish_original_branch as *const () as usize,
                0x28,
                Some(0x20),
            ),
            (
                submission_push_gateway_body as *const () as usize,
                0x1c8,
                None,
            ),
            (
                submission_publish_gateway_body as *const () as usize,
                0x1c8,
                None,
            ),
        ] {
            let mut stack = [0u64; 128];
            stack[allocation / 8] = 0x11223344;
            if let Some(slot) = slot {
                stack[slot / 8] = 0x55667788;
            }
            let mut base = 0;
            let entry = unsafe { RtlLookupFunctionEntry(pc as u64, &mut base, null_mut()) };
            assert!(!entry.is_null());
            let mut ctx: CONTEXT = unsafe { std::mem::zeroed() };
            ctx.Rip = pc as u64;
            ctx.Rsp = stack.as_mut_ptr() as u64;
            ctx.Rbx = if slot.is_some() {
                0xaabbccdd
            } else {
                0x55667788
            };
            let initial = ctx.Rsp;
            let mut data = null_mut();
            let mut frame = 0;
            unsafe {
                RtlVirtualUnwind(
                    0,
                    base,
                    pc as u64,
                    entry,
                    &mut ctx,
                    &mut data,
                    &mut frame,
                    null_mut(),
                );
            }
            assert_eq!(ctx.Rip, 0x11223344, "PC{pc:x}");
            assert_eq!(ctx.Rsp, initial + allocation as u64 + 8);
            assert_eq!(ctx.Rbx, 0x55667788, "PC{pc:x}");
        }
    }
    #[test]
    fn submission_owned_os_stop_deadline_retains_late_reserved_return() {
        if !child("submission_owned_os_stop_deadline_retains_late_reserved_return") {
            return;
        }
        submission_fixture(1);
        ROOT_CONTINUE[1].store(
            root_delay_original as *const () as u64 + 6,
            Ordering::Release,
        );
        submission::SUBMISSION_TEST_ORIGINALS[1].store(
            submission::submission_publish_original as *const () as u64,
            Ordering::Release,
        );
        let delay = std::sync::Arc::new(Delay {
            entered: AtomicU64::new(0),
            release: AtomicU64::new(0),
            calls: AtomicU64::new(0),
        });
        let target = delay.clone();
        let worker = std::thread::spawn(move || {
            let call = unsafe {
                std::mem::transmute::<usize, unsafe extern "system" fn(*const Delay)>(gateways()[1])
            };
            unsafe {
                call(std::sync::Arc::as_ptr(&target));
            }
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while delay.entered.load(Ordering::Acquire) == 0 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let status = stop_at(Instant::now());
        let pending = h(12).load(Ordering::Acquire) != ROOT_CLOSED;
        let uncommitted = submission::TRACE.rows[1][0].load(Ordering::Acquire) == 0;
        delay.release.store(1, Ordering::Release);
        worker.join().unwrap();
        assert_eq!(status, 2);
        assert!(pending && uncommitted);
        assert_eq!(delay.calls.load(Ordering::Acquire), 1);
        assert_eq!(submission::TRACE.rows[1][0].load(Ordering::Acquire), 2);
        assert_eq!(submission::TRACE.rows[1][2].load(Ordering::Acquire), 2);
        assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    }
    fn partial_failure(name: &str, site: usize) {
        if !child(name) {
            return;
        }
        let core = OwnedCore::new();
        let base = core.base as usize;
        bind_fixture(base, base as u64 + 0x63e1d5, base as u64 + 0x4a1196);
        FAIL_AFTER_PUBLISH.store(site, Ordering::Release);
        assert!(unsafe { install_sites(base, WORDS) }.is_err());
        assert_eq!(h(15).load(Ordering::Acquire), (1 << site) - 1);
        assert!(unsafe { restore_all() }.is_ok());
        assert_eq!(h(17).load(Ordering::Acquire), 7);
        for (i, word) in WORDS.into_iter().enumerate() {
            assert_eq!(copy_current::<8>(base as u64 + SITE[i] as u64), Some(word));
            assert!(HOOKS[i].lock().unwrap().is_none());
        }
    }
    #[test]
    fn root_partial_publication_world_rollback() {
        partial_failure("root_partial_publication_world_rollback", 1);
    }
    #[test]
    fn root_partial_publication_manager_rollback() {
        partial_failure("root_partial_publication_manager_rollback", 2);
    }
    #[test]
    fn root_partial_publication_wait_rollback() {
        partial_failure("root_partial_publication_wait_rollback", 3);
    }
    #[repr(C)]
    struct Delay {
        entered: AtomicU64,
        release: AtomicU64,
        calls: AtomicU64,
    }
    global_asm!(
        ".globl root_delay_original",
        ".seh_proc root_delay_original",
        "root_delay_original:",
        ".byte 0x40,0x53",
        ".seh_pushreg rbx",
        "sub rsp,0x20",
        ".seh_stackalloc 0x20",
        ".seh_endprologue",
        "mov qword ptr [rcx],1",
        "1:",
        "cmp qword ptr [rcx+8],0",
        "je 1b",
        "inc qword ptr [rcx+16]",
        "add rsp,0x20",
        "pop rbx",
        "ret",
        ".seh_endproc",
    );
    unsafe extern "system" {
        fn root_delay_original();
    }
    #[test]
    fn root_stop_deadline_then_reserved_return_after_close() {
        if !child("root_stop_deadline_then_reserved_return_after_close") {
            return;
        }
        bind_fixture(1, 0, 0);
        ROOT_CONTINUE[0].store(
            root_delay_original as *const () as u64 + 6,
            Ordering::Release,
        );
        let delay = std::sync::Arc::new(Delay {
            entered: AtomicU64::new(0),
            release: AtomicU64::new(0),
            calls: AtomicU64::new(0),
        });
        let target = delay.clone();
        let worker = std::thread::spawn(move || {
            let call = unsafe {
                std::mem::transmute::<usize, unsafe extern "system" fn(*const Delay)>(gateways()[0])
            };
            unsafe {
                call(std::sync::Arc::as_ptr(&target));
            }
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while delay.entered.load(Ordering::Acquire) == 0 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let status = stop_at(Instant::now());
        let pending = h(12).load(Ordering::Acquire) != ROOT_CLOSED;
        let not_committed = TRACE.rows[1][0].load(Ordering::Acquire) == 0;
        delay.release.store(1, Ordering::Release);
        worker.join().unwrap();
        assert_eq!(status, 2);
        assert!(pending && not_committed);
        assert_eq!(delay.calls.load(Ordering::Acquire), 1);
        assert_eq!(TRACE.rows[1][0].load(Ordering::Acquire), 2);
        assert_eq!(TRACE.rows[1][2].load(Ordering::Acquire), 2);
        assert_eq!(stop_at(Instant::now() + Duration::from_secs(1)), 0);
        assert_eq!(h(12).load(Ordering::Acquire), ROOT_CLOSED);
    }
    include!("root_observation/submission/observed_tests.rs");
}
