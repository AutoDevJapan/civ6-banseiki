//! Governor-only observed call correspondence. No World permit or task ownership.
use super::*;
use civ6_bootstrap_rs::protocol::{
    SUBMISSION_BIRTHS, SUBMISSION_BIRTH_WORDS, SUBMISSION_CHAIN_ROWS, SUBMISSION_EXTRA_ROWS,
    SUBMISSION_HEADER, SUBMISSION_ROWS,
};
pub(in super::super) const EXTRA: [(usize, bool, [u8; 8], usize); 4] = [
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
    budget: [AtomicU64; 8],
    pub(in super::super) rows: [[AtomicU64; 48]; SUBMISSION_ROWS + SUBMISSION_EXTRA_ROWS],
    births: [[AtomicU64; SUBMISSION_BIRTH_WORDS]; SUBMISSION_BIRTHS],
}
pub(in super::super) static TRACE: ObservedTrace = ObservedTrace {
    header: [const { AtomicU64::new(0) }; 32],
    sites: [const { [const { AtomicU64::new(0) }; 8] }; 6],
    budget: [
        AtomicU64::new(SUBMISSION_ROWS as u64),
        AtomicU64::new(SUBMISSION_CHAIN_ROWS as u64),
        AtomicU64::new(SUBMISSION_BIRTHS as u64),
        AtomicU64::new(SUBMISSION_BIRTH_WORDS as u64),
        AtomicU64::new(SUBMISSION_EXTRA_ROWS as u64),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
    ],
    rows: [const { [const { AtomicU64::new(0) }; 48] }; SUBMISSION_ROWS + SUBMISSION_EXTRA_ROWS],
    births: [const { [const { AtomicU64::new(0) }; SUBMISSION_BIRTH_WORDS] }; SUBMISSION_BIRTHS],
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
const HOST_PUSH_START: u64 = 8192;
const HOST_PUSH_RETURN: u64 = 16384;
#[derive(Clone, Copy)]
struct Birth {
    w: [u64; SUBMISSION_BIRTH_WORDS],
}
const COMPLETE: u64 = BORN
    | SUBMITTED
    | BOUND
    | PUBLISHED
    | PRODUCER_RETURN
    | HOST_START
    | CORE_START
    | CORE_RETURN
    | HOST_RETURN
    | HOST_PUSH_START
    | HOST_PUSH_RETURN;
#[derive(Clone, Copy)]
struct Provisional {
    id: u64,
    row: [u64; 48],
    attempted: bool,
    outside_window: bool,
}
thread_local! {
    static PROVISIONAL:std::cell::RefCell<[Provisional;DEPTH]>=const {std::cell::RefCell::new(
        [Provisional {id:0,row:[0;48],attempted:false,outside_window:false};DEPTH])};
}
// Completion is observed full-frame completion only. No heap reread, flags,
// sameSetWait, ownership, destructor or World authority is inferred here.
fn completed(b: &Birth) -> bool {
    if b.w[11] != COMPLETE
        || b.w[15] != 0
        || b.w[2] == 0
        || b.w[19] != 63
        || b.w[20] != 63
        || b.w[21] != 0
    {
        return false;
    }
    for (token, kind) in [
        (b.w[3], 6),
        (b.w[4], 4),
        (b.w[5], 5),
        (b.w[6], 7),
        (b.w[7], 8),
        (b.w[16], 9),
    ] {
        let Some(i) = token
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|i| i + 1 < SUBMISSION_ROWS)
        else {
            return false;
        };
        let a = &TRACE.rows[i];
        let z = &TRACE.rows[i + 1];
        let get = |r: &[AtomicU64; 48], j: usize| r[j].load(Ordering::Acquire);
        if get(a, 0) != token
            || get(z, 0) != token + 1
            || get(a, 1) != kind
            || get(z, 1) != kind
            || get(z, 2) != 2
            || get(a, 20) != 0
            || get(z, 20) != 0
            || get(a, 35) != b.w[0]
            || get(z, 35) != b.w[0]
            || get(a, 36) != b.w[1]
            || get(a, 5) == 0
            || get(a, 6) == 0
            || get(a, 5) != get(z, 5)
            || get(a, 6) != get(z, 6)
            || get(a, 7) != b.w[10]
            || get(z, 7) != b.w[10]
        {
            return false;
        }
        if kind == 4
            && (get(a, 24) != 32
                || get(a, 25) != 0
                || get(a, 26) != 16
                || get(a, 27) != 0
                || get(a, 30) == 0
                || get(a, 31) == 0)
        {
            return false;
        }
        if kind == 7 && (get(a, 38) != 16 || get(a, 39) != 1 || get(a, 41) != b.w[1]) {
            return false;
        }
        if kind == 8 && (get(a, 38) != 64 || get(a, 39) != 1) {
            return false;
        }
    }
    true
}
fn completed_before(b: &Birth, event: u64) -> bool {
    completed(b)
        && [b.w[3], b.w[4], b.w[5], b.w[6], b.w[7], b.w[16]]
            .into_iter()
            .all(|token| TRACE.rows[token as usize][23].load(Ordering::Acquire) < event)
}
fn completed_reuse(book: &[Birth; SUBMISSION_BIRTHS], p: u64, t: Option<u64>, event: u64) -> bool {
    book.iter()
        .filter(|b| b.w[0] != 0 && (b.w[1] == p || t.is_some_and(|t| t != 0 && b.w[2] == t)))
        .all(|b| completed_before(b, event))
}
fn reserve_rows(count: u64) -> Option<u64> {
    h(10)
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            n.checked_add(count)
                .filter(|n| *n <= SUBMISSION_ROWS as u64)
        })
        .ok()
}
fn role(kind: u64) -> Option<usize> {
    match kind {
        6 => Some(0),
        4 => Some(1),
        9 => Some(2),
        5 => Some(3),
        7 => Some(4),
        8 => Some(5),
        _ => None,
    }
}
fn reserve_extra() -> Option<u64> {
    TRACE.budget[5]
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
            v.checked_add(2)
                .filter(|n| *n <= SUBMISSION_EXTRA_ROWS as u64)
        })
        .ok()
        .map(|v| v + SUBMISSION_ROWS as u64)
}
fn live_t(t: u64) -> bool {
    t != 0
        && TRACE.births.iter().any(|b| {
            b[21].load(Ordering::Acquire) & 1 != 0
                && b[0].load(Ordering::Acquire) != 0
                && b[11].load(Ordering::Acquire) & BOUND != 0
                && b[2].load(Ordering::Acquire) == t
        })
}
fn acquire_call(tls: bool, t: Option<u64>) -> bool {
    h(12)
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
            if (v & ROOT_CLOSED != 0 && ((!tls && !t.is_some_and(live_t)) || v & !ROOT_CLOSED == 0))
                || v & !ROOT_CLOSED == ROOT_CLOSED - 1
            {
                None
            } else {
                Some(v + 1)
            }
        })
        .is_ok()
}
fn commit_entry(index: usize, mut row: [u64; 48]) {
    row[0] = 0;
    for (dst, v) in TRACE.rows[index].iter().zip(row) {
        dst.store(v, Ordering::Relaxed);
    }
    TRACE.rows[index][0].store(index as u64 + 1, Ordering::Release);
}
static BOOK: Mutex<[Birth; SUBMISSION_BIRTHS]> = Mutex::new(
    [Birth {
        w: [0; SUBMISSION_BIRTH_WORDS],
    }; SUBMISSION_BIRTHS],
);
#[cfg(test)]
static FAIL_THREAD_LOOKUP: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
#[derive(Clone)]
struct BookWait {
    tid: u64,
    arrived: std::sync::Arc<std::sync::Barrier>,
    go: std::sync::Arc<std::sync::Barrier>,
    locking: std::sync::Arc<std::sync::Barrier>,
}
#[cfg(test)]
static BOOK_WAIT_TEST: Mutex<Option<BookWait>> = Mutex::new(None);
#[cfg(test)]
static SEALED_AFTER_LOCK_TEST: AtomicBool = AtomicBool::new(false);
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
    // Exclusion/read failures are writes too. They acquire the SAME flight
    // before filtering; accepted reservations transfer it to normal/SEH finish.
    struct EntryFlight(bool);
    impl Drop for EntryFlight {
        fn drop(&mut self) {
            if self.0 {
                h(12).fetch_sub(1, Ordering::Release);
            }
        }
    }
    let mut flight = EntryFlight(false);
    let mut admitted = false;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let saved = unsafe { &mut *raw };
        let Some(bind) = BIND.get() else { return };
        let kind = saved.invocation.kind;
        let stack = STACK.with(Cell::get);
        let parent = stack.depth.checked_sub(1).map(|n| stack.frames[n]);
        if matches!(kind, 4 | 5)
            && TRACE.budget[7].load(Ordering::Acquire) == 1
            && parent.is_some_and(|p| p.cookie == COOKIE && p.kind == 6 && p.token == 0)
        {
            // A pre-seal provisional producer was never among the first 16
            // dispatches. Its actual original call continues, without capture.
            PROVISIONAL.with(|p| p.borrow_mut()[stack.depth - 1].outside_window = true);
            return;
        }
        let tls_inherit = parent.is_some_and(|p| {
            p.cookie == COOKIE && matches!((kind, p.kind), (4 | 5, 6) | (9, 4) | (8, 7))
        });
        let reserved_t = (kind == 7).then_some(saved.gpr[0]);
        let may_inherit = tls_inherit || reserved_t.is_some_and(live_t);
        if !acquire_call(tls_inherit, reserved_t) {
            return;
        }
        flight.0 = true;
        admitted = true;
        if !may_inherit && Instant::now() >= bind.until {
            h(12).fetch_or(ROOT_CLOSED, Ordering::AcqRel);
            return;
        }
        #[cfg(test)]
        let identity = if FAIL_THREAD_LOOKUP.load(Ordering::Acquire) {
            None
        } else {
            thread_key()
        };
        #[cfg(not(test))]
        let identity = thread_key();
        let Some((tid, ft)) = identity else {
            unknown();
            return;
        };
        saved.invocation.tid = tid;
        saved.invocation.thread_created = ft;
        saved.invocation.epoch = h(9).load(Ordering::Acquire);
        if kind == 6 {
            // Linearize producer admission with dispatch publication's BOOK
            // lock. An entry admitted before the seal remains provisional;
            // an entry after it cannot create a serial/cookie or read receiver.
            let _limit = BOOK
                .lock()
                .unwrap_or_else(|_| panic!("observed book poisoned"));
            if TRACE.budget[7].load(Ordering::Acquire) == 1
                || h(29).load(Ordering::Acquire) >= SUBMISSION_BIRTHS as u64
            {
                return;
            }
            if stack.depth == DEPTH {
                h(11).fetch_add(1, Ordering::Relaxed);
                unknown();
                return;
            }
            let Ok(serial) =
                h(21).fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            else {
                h(11).fetch_add(1, Ordering::Relaxed);
                unknown();
                return;
            };
            let inv = &mut saved.invocation;
            inv.world = serial + 1;
            inv.manager = saved.gpr[0].checked_add(0xd0).unwrap_or(0);
            if inv.manager == 0 {
                fail(inv, REUSED);
            }
            if h(19).load(Ordering::Acquire) != 3 || h(15).load(Ordering::Acquire) != 63 {
                fail(inv, 8);
                h(22).fetch_add(1, Ordering::Relaxed);
            }
            let mut row = entry_snapshot(saved, saved.invocation, error);
            row[23] = h(24).fetch_add(1, Ordering::AcqRel) + 1;
            row[35] = saved.invocation.world;
            row[36] = saved.invocation.manager;
            PROVISIONAL.with(|p| {
                p.borrow_mut()[stack.depth] = Provisional {
                    id: saved.invocation.world,
                    row,
                    attempted: false,
                    outside_window: false,
                }
            });
            saved.invocation.cookie = COOKIE;
            let mut next = stack;
            next.frames[next.depth] = saved.invocation;
            next.depth += 1;
            STACK.with(|s| s.set(next));
            flight.0 = false; // before-fetch frame exists; no BOOK or output rows yet
            if saved.invocation.errors != 0 {
                unknown();
            }
            return;
        }
        #[cfg(test)]
        if matches!(kind, 4 | 5) {
            let wait = BOOK_WAIT_TEST.lock().unwrap().clone();
            if let Some(wait) = wait.filter(|w| w.tid == saved.invocation.tid) {
                wait.arrived.wait();
                wait.go.wait();
                wait.locking.wait();
            }
        }
        let mut book = BOOK
            .lock()
            .map_err(|_| ())
            .unwrap_or_else(|_| panic!("observed book poisoned"));
        // The fastpath may have observed seal=0 before waiting for BOOK. Window
        // eligibility and registration are one decision under this same lock.
        // Already promoted parent tokens retain their existing reservation.
        if matches!(kind, 4 | 5)
            && TRACE.budget[7].load(Ordering::Acquire) == 1
            && parent.is_some_and(|p| p.cookie == COOKIE && p.kind == 6 && p.token == 0)
        {
            PROVISIONAL.with(|p| p.borrow_mut()[stack.depth - 1].outside_window = true);
            #[cfg(test)]
            SEALED_AFTER_LOCK_TEST.store(true, Ordering::Release);
            return;
        }
        let mut snapshot = None;
        let mut p_snapshot = None;
        let mut index = None;
        let mut promotion = None;
        let mut detached = false;
        if kind == 4 || kind == 5 {
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
            index = book.iter().position(|b| b.w[0] != 0 && b.w[0] == p.world);
            saved.invocation.parent = p.token;
            if kind == 4 && p.token == 0 {
                let prior = PROVISIONAL.with(|tls| {
                    let mut entries = tls.borrow_mut();
                    let entry = &mut entries[stack.depth - 1];
                    if entry.id != p.world {
                        return None;
                    }
                    entry.attempted = true;
                    Some(entry.row)
                });
                if let Some(row) = prior {
                    if !completed_reuse(&book, p.manager, None, row[23]) {
                        fail(&mut saved.invocation, REUSED);
                        detached = true;
                    } else if let Some(slot) = book.iter().position(|b| b.w[0] == 0) {
                        promotion = Some((slot, p, row));
                    } else {
                        h(11).fetch_add(1, Ordering::Relaxed);
                        fail(&mut saved.invocation, 0x2000);
                    }
                } else {
                    fail(&mut saved.invocation, 0x20);
                }
            }
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
            index = book.iter().position(|b| b.w[0] != 0 && b.w[0] == p.world);
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
            // CURRENT Host input only: this original frame writes P+8/+10 and
            // then reads P.vft. Never follow a historical/returned descriptor.
            let active = book.iter().any(|b| {
                b.w[0] != 0
                    && b.w[11] & BOUND != 0
                    && b.w[2] == saved.gpr[0]
                    && b.w[1] == p
                    && !completed(b)
            });
            let (pbytes, pn, pe, pok) = if n == 16 && ok != 0 && e == 0 {
                read_once::<8>(p)
            } else {
                ([0; 8], 0, 0, 0)
            };
            let pvft = u64::from_le_bytes(pbytes);
            p_snapshot = Some((pvft, pn, pe, pok));
            let identified = pn == 8 && pe == 0 && pok != 0;
            let governor = identified && pvft == bind.core as u64 + 0xa06648;
            if identified && !governor && !active {
                exclude();
                return; // explicit other-native-job exclusion inside this admission flight
            }
            if !governor {
                fail(&mut saved.invocation, 0x400);
            }
            let mut exact = book.iter().enumerate().filter(|(_, b)| {
                b.w[0] != 0
                    && b.w[11] & BOUND != 0
                    && b.w[2] != 0
                    && b.w[2] == saved.gpr[0]
                    && b.w[1] == p
                    && b.w[6] == 0
                    && !completed(b)
            });
            index = exact.next().map(|(i, _)| i);
            if exact.next().is_some() {
                index = None;
            }
            if n != 16 || ok == 0 || e != 0 || !identified {
                h(31).fetch_add(1, Ordering::Relaxed);
            }
            if index.is_none() {
                detached = true;
                fail(&mut saved.invocation, EARLY | REUSED);
                saved.invocation.manager = p;
                saved.invocation.set = saved.gpr[0];
            }
        } else if kind == 8 {
            if let Some(p) = parent.filter(|p| valid_parent(*p, 7, &saved.invocation)) {
                saved.invocation.parent = p.token;
                index = p
                    .list
                    .checked_sub(1)
                    .and_then(|n| usize::try_from(n).ok())
                    .filter(|i| *i < SUBMISSION_BIRTHS)
                    .filter(|i| {
                        book[*i].w[0] == p.world
                            && book[*i].w[1] == saved.gpr[0]
                            && book[*i].w[2] == p.set
                            && book[*i].w[6] == p.token
                            && book[*i].w[7] == 0
                    });
                if saved.invocation.caller != h(8).load(Ordering::Acquire) + 0x985ef4
                    || saved.invocation.caller_rsp.checked_add(0x30) != Some(p.native_rsp)
                {
                    fail(&mut saved.invocation, 0x800);
                }
            } else {
                if !book.iter().any(|b| b.w[0] != 0 && b.w[1] == saved.gpr[0]) {
                    exclude();
                    return;
                }
                fail(&mut saved.invocation, EARLY);
            }
            if index.is_none() {
                detached = true;
                fail(&mut saved.invocation, EARLY);
                saved.invocation.manager = saved.gpr[0];
            }
            let (bytes, n, e, ok) = read_once::<64>(saved.gpr[0]);
            snapshot = Some((bytes.to_vec(), n, e, ok));
        } else {
            exclude();
            return;
        }
        let mut row;
        if let Some((slot, mut producer, mut entry)) = promotion {
            let Some(base) = reserve_rows(SUBMISSION_CHAIN_ROWS as u64) else {
                h(11).fetch_add(1, Ordering::Relaxed);
                unknown();
                return;
            };
            row = base;
            producer.token = row + 1;
            producer.list = slot as u64 + 1;
            let mut b = Birth {
                w: [0; SUBMISSION_BIRTH_WORDS],
            };
            b.w[0] = producer.world;
            b.w[1] = producer.manager;
            b.w[3] = producer.token;
            b.w[8] = producer.tid;
            b.w[9] = producer.thread_created;
            b.w[10] = producer.epoch;
            b.w[11] = BORN;
            b.w[12] = producer.native_rsp;
            b.w[15] = producer.errors;
            b.w[18] = base;
            b.w[19] = 1;
            b.w[21] = 1;
            // Own one sampling flight until all reserved roles settle, or an
            // erroneous/SEH cycle has no accepted live frames. Never job cancel.
            assert!(acquire(true));
            book[slot] = b;
            index = Some(slot);
            let stored = h(29).fetch_add(1, Ordering::AcqRel) + 1;
            if stored == SUBMISSION_BIRTHS as u64 {
                TRACE.budget[7].store(1, Ordering::Release);
                h(12).fetch_or(ROOT_CLOSED, Ordering::AcqRel);
            }
            entry[3] = producer.token;
            entry[20] |= producer.errors;
            commit_entry(row as usize, entry);
            STACK.with(|s| {
                let mut current = s.get();
                current.frames[current.depth - 1] = producer;
                s.set(current);
            });
            saved.invocation.parent = producer.token;
            row += 2;
            b.w[19] |= 2;
            book[slot] = b;
        } else if let Some(slot) = index {
            let r = role(kind).unwrap();
            let mask = 1u64 << r;
            if book[slot].w[19] & mask == 0 {
                row = book[slot].w[18] + (r * 2) as u64;
                book[slot].w[19] |= mask;
            } else {
                index = None;
                detached = true;
                fail(&mut saved.invocation, REUSED);
                let Some(extra) = reserve_extra() else {
                    TRACE.budget[6].fetch_add(1, Ordering::Relaxed);
                    h(11).fetch_add(1, Ordering::Relaxed);
                    unknown();
                    return;
                };
                row = extra;
            }
        } else {
            let Some(extra) = reserve_extra() else {
                TRACE.budget[6].fetch_add(1, Ordering::Relaxed);
                h(11).fetch_add(1, Ordering::Relaxed);
                unknown();
                return;
            };
            row = extra;
            if saved.invocation.errors == 0 {
                fail(&mut saved.invocation, EARLY);
            }
        }
        let inv = &mut saved.invocation;
        inv.cookie = COOKIE;
        flight.0 = false; // reservation owns this flight until finish/unwind
        inv.token = row + 1;
        if h(19).load(Ordering::Acquire) != 3 || h(15).load(Ordering::Acquire) != 63 {
            fail(inv, 8);
            h(22).fetch_add(1, Ordering::Relaxed);
        }
        if stack.depth == DEPTH {
            fail(inv, 0x10);
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
        if index.is_none() && !detached {
            fail(&mut saved.invocation, 0x2000);
        }
        drop(book);
        let mut r = entry_snapshot(saved, saved.invocation, error);
        saved.invocation.errors |= r[20];
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
        if let Some((vft, n, e, ok)) = p_snapshot {
            r[42] = vft;
            r[43] = n as u64;
            r[44] = u64::from(e);
            r[45] = ok;
        }
        r[20] = saved.invocation.errors;
        if r[20] != 0 {
            unknown();
        }
        let mut next = STACK.with(Cell::get);
        if next.depth < DEPTH {
            next.frames[next.depth] = saved.invocation;
            next.depth += 1;
            STACK.with(|s| s.set(next));
        }
        commit(row as usize, r);
    }));
    if result.is_err() && admitted {
        unknown();
        unsafe {
            (*raw).invocation.errors |= 0x20;
        }
    }
    unsafe { SetLastError(error) };
}
pub(super) fn finish(raw: &mut Saved, edge: u64, error: u32) {
    let inv = raw.invocation;
    if inv.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0;
    let _release = Release;
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        finish_admitted(raw, inv, edge, error)
    }))
    .is_err()
    {
        // Unknown must be published BEFORE the accepted flight reaches zero.
        unknown();
    }
}
fn finish_admitted(raw: &mut Saved, mut inv: Invocation, edge: u64, error: u32) {
    let mut attempted = false;
    let mut outside_window = false;
    let _ = STACK.try_with(|s| {
        let mut stack = s.get();
        if stack.depth > 0
            && stack.frames[stack.depth - 1].kind == inv.kind
            && stack.frames[stack.depth - 1].world == inv.world
            && (stack.frames[stack.depth - 1].token == inv.token
                || (inv.kind == 6 && inv.token == 0))
        {
            if inv.kind == 6 {
                let current = stack.frames[stack.depth - 1];
                inv.token = current.token;
                inv.list = current.list;
                inv.set = current.set;
                inv.errors |= current.errors;
                PROVISIONAL.with(|p| {
                    let mut entries = p.borrow_mut();
                    let entry = &mut entries[stack.depth - 1];
                    attempted = entry.attempted;
                    outside_window = entry.outside_window;
                    if entry.id != inv.world {
                        fail(&mut inv, 0x20);
                    }
                    *entry = Provisional {
                        id: 0,
                        row: [0; 48],
                        attempted: false,
                        outside_window: false,
                    };
                });
            }
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
    if inv.kind == 6 && inv.token == 0 {
        if edge == 2 && inv.errors == 0 && outside_window {
            return;
        }
        if edge == 2 && inv.errors == 0 && !attempted {
            h(20).fetch_add(1, Ordering::Relaxed);
        } else {
            unknown();
        }
        return; // no-submit/failed provisional frame consumes no BOOK or raw rows
    }
    if inv.list > 0 {
        if let Ok(mut book) = BOOK.lock() {
            let index = inv.list as usize - 1;
            let entered = if inv.kind == 4 {
                book[index].w[3]
                    .checked_sub(1)
                    .and_then(|n| TRACE.rows.get(n as usize))
                    .map(|r| r[23].load(Ordering::Acquire))
                    .unwrap_or(0)
            } else {
                0
            };
            let reused_t = inv.kind == 4
                && book.iter().enumerate().any(|(i, other)| {
                    i != index
                        && other.w[2] == raw.gpr[2]
                        && raw.gpr[2] != 0
                        && !completed_before(other, entered)
                });
            let b = &mut book[index];
            if edge == 3 {
                b.w[11] |= EXCEPTION;
            } else {
                match inv.kind {
                    6 => {
                        b.w[11] |= PRODUCER_RETURN;
                    }
                    4 => {
                        let t = raw.gpr[2];
                        if inv.errors != 0
                            || b.w[15] != 0
                            || b.w[11] & HOST_PUSH_RETURN == 0
                            || b.w[16] == 0
                            || b.w[17] != t
                        {
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
    if inv.list > 0 {
        if let Ok(mut book) = BOOK.lock() {
            let b = &mut book[inv.list as usize - 1];
            b.w[20] |= 1 << role(inv.kind).unwrap();
            if edge == 3 {
                b.w[21] |= 2;
            }
            let retire =
                b.w[21] & 1 != 0 && (b.w[20] == 63 || (b.w[15] != 0 && (b.w[19] & !b.w[20]) == 0));
            if retire {
                b.w[21] &= !1;
            }
            publish(inv.list as usize - 1, *b);
            if retire {
                h(12).fetch_sub(1, Ordering::Release);
            }
        } else {
            unknown();
        }
    }
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
    ".globl observed_producer_original_body",
    "observed_producer_original_body:",
    "mov [rsp+8],rax",
    "mov rax,[rip+OBSERVED_CONTINUE]",
    "mov [rsp+0x10],rax",
    "mov rax,[rsp+8]",
    ".globl observed_producer_original_branch",
    "observed_producer_original_branch:",
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
    ".globl observed_host_original_body",
    "observed_host_original_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+OBSERVED_CONTINUE+8]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    ".globl observed_host_original_branch",
    "observed_host_original_branch:",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl observed_push_host_original",
    ".seh_proc observed_push_host_original",
    "observed_push_host_original:",
    "sub rsp,0x38",
    ".seh_stackalloc 0x38",
    ".seh_endprologue",
    ".globl observed_push_host_original_body",
    "observed_push_host_original_body:",
    "movups xmm0,[r9]",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+OBSERVED_CONTINUE+24]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    ".globl observed_push_host_original_branch",
    "observed_push_host_original_branch:",
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
    ".globl observed_core_original_body",
    "observed_core_original_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+OBSERVED_CONTINUE+16]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    ".globl observed_core_original_branch",
    "observed_core_original_branch:",
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
#[cfg(test)]
pub(in super::super) fn fixture_gateways() -> [usize; 4] {
    [
        observed_producer_gateway as *const () as usize,
        observed_host_gateway as *const () as usize,
        observed_core_gateway as *const () as usize,
        observed_push_host_gateway as *const () as usize,
    ]
}
#[cfg(test)]
pub(in super::super) fn fixture_originals() -> [usize; 4] {
    [
        observed_producer_original as *const () as usize,
        observed_host_original as *const () as usize,
        observed_core_original as *const () as usize,
        observed_push_host_original as *const () as usize,
    ]
}
#[cfg(test)]
pub(in super::super) fn fixture_continue(i: usize, target: usize) {
    OBSERVED_CONTINUE[i].store(target as u64, Ordering::Release);
}
#[cfg(test)]
pub(in super::super) fn fixture_birth(p: u64, t: u64) {
    let mut b = Birth {
        w: [0; SUBMISSION_BIRTH_WORDS],
    };
    b.w[0] = 1;
    b.w[1] = p;
    b.w[2] = t;
    b.w[10] = h(9).load(Ordering::Acquire);
    b.w[11] = BORN | SUBMITTED | BOUND;
    b.w[18] = reserve_rows(SUBMISSION_CHAIN_ROWS as u64).unwrap();
    BOOK.lock().unwrap()[0] = b;
    publish(0, b);
    h(29).store(1, Ordering::Release);
    h(21).fetch_max(1, Ordering::Release);
}
#[cfg(test)]
pub(in super::super) fn fixture_book(index: usize) -> [u64; 18] {
    BOOK.lock().unwrap()[index].w[..18].try_into().unwrap()
}
#[cfg(test)]
pub(in super::super) fn fixture_extra_next() -> usize {
    TRACE.budget[5].load(Ordering::Acquire) as usize
}
#[cfg(test)]
pub(in super::super) fn fixture_last_entry() -> usize {
    TRACE
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r[0].load(Ordering::Acquire) != 0 && r[2].load(Ordering::Acquire) == 1)
        .max_by_key(|(_, r)| r[23].load(Ordering::Acquire))
        .unwrap()
        .0
}
#[cfg(test)]
pub(in super::super) fn fixture_budget(index: usize) -> u64 {
    TRACE.budget[index].load(Ordering::Acquire)
}
#[cfg(test)]
pub(in super::super) fn fixture_book_wait(
    tid: u64,
    arrived: std::sync::Arc<std::sync::Barrier>,
    go: std::sync::Arc<std::sync::Barrier>,
    locking: std::sync::Arc<std::sync::Barrier>,
) {
    *BOOK_WAIT_TEST.lock().unwrap() = Some(BookWait {
        tid,
        arrived,
        go,
        locking,
    });
}
#[cfg(test)]
pub(in super::super) fn fixture_with_book_lock(action: impl FnOnce()) {
    let _lock = BOOK.lock().unwrap();
    action();
}
#[cfg(test)]
pub(in super::super) fn fixture_sealed_after_lock() -> bool {
    SEALED_AFTER_LOCK_TEST.load(Ordering::Acquire)
}
#[cfg(test)]
pub(in super::super) fn fixture_bytes() -> Vec<u8> {
    TRACE
        .header
        .iter()
        .chain(TRACE.sites.iter().flatten())
        .chain(TRACE.budget.iter())
        .chain(TRACE.rows.iter().flatten())
        .chain(TRACE.births.iter().flatten())
        .flat_map(|v| v.load(Ordering::Acquire).to_le_bytes())
        .collect()
}
#[cfg(test)]
pub(in super::super) fn fixture_poison_book() {
    let _ = std::panic::catch_unwind(|| {
        let _lock = BOOK.lock().unwrap();
        panic!("fixture poison");
    });
}
#[cfg(test)]
pub(in super::super) fn fixture_lookup_fail() {
    FAIL_THREAD_LOOKUP.store(true, Ordering::Release);
}
#[cfg(test)]
pub(in super::super) fn fixture_parent(kind: u64, input: u64, rsp: u64) {
    if !matches!(kind, 8 | 9) || !acquire(false) {
        return;
    }
    let (tid, ft) = thread_key().unwrap();
    let mut s = STACK.with(Cell::get);
    s.frames[0] = Invocation {
        cookie: COOKIE,
        kind: if kind == 8 { 7 } else { 4 },
        token: 999,
        tid,
        thread_created: ft,
        epoch: h(9).load(Ordering::Acquire),
        world: 1,
        manager: if kind == 8 { input } else { 0x5566 },
        set: if kind == 8 { 1 } else { 0 },
        native_rsp: rsp + if kind == 8 { 0x30 } else { 0x40 },
        ..Invocation::default()
    };
    s.depth = 1;
    STACK.with(|x| x.set(s));
    if kind == 9 {
        let mut b = BOOK.lock().unwrap();
        b[0].w[13] = 0x7788;
        b[0].w[14] = input;
    }
}
#[cfg(test)]
pub(in super::super) fn fixture_parent_end() {
    STACK.with(|x| {
        let mut s = x.get();
        if s.depth == 1 && s.frames[0].token == 999 {
            s.depth = 0;
            s.frames[0] = Invocation::default();
            x.set(s);
            h(12).fetch_sub(1, Ordering::Release);
        }
    });
}
#[cfg(test)]
pub(in super::super) fn fixture_owners() -> [bool; 4] {
    std::array::from_fn(|i| EXTRA_HOOKS[i].lock().unwrap().is_some())
}
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
        #[cfg(test)]
        if result.exchanged && FAIL_AFTER_PUBLISH.load(Ordering::Acquire) == i + 3 {
            return Err(site_error(
                i + 2,
                16,
                "injected observed partial publication",
            ));
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
