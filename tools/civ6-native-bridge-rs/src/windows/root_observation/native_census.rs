//! Diagnostic-only gateway census. Counts never grant World authority or
//! certify quiescence; a read may overlap concurrent native calls.
use super::*;

pub(in super::super) const MODULES: usize = 16;
pub(in super::super) const KINDS: usize = 160;
pub(in super::super) const SIZE: usize = 128 + MODULES * KINDS * 24;
#[repr(C)]
struct Counters {
    entered: AtomicU64,
    returned: AtomicU64,
    terminal: AtomicU64,
}
#[repr(C)]
pub struct Census {
    header: [AtomicU64; 16],
    rows: [Counters; MODULES * KINDS],
}
static DATA: Census = Census {
    header: [
        AtomicU64::new(1),
        AtomicU64::new(SIZE as u64),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(MODULES as u64),
        AtomicU64::new(KINDS as u64),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
    ],
    rows: [const {
        Counters {
            entered: AtomicU64::new(0),
            returned: AtomicU64::new(0),
            terminal: AtomicU64::new(0),
        }
    }; MODULES * KINDS],
};
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_NATIVE_CENSUS: &Census = &DATA;
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_NATIVE_CENSUS_SIZE: u64 = SIZE as u64;

const fn contains(text: &str, part: &str) -> bool {
    let text = text.as_bytes();
    let part = part.as_bytes();
    let mut start = 0;
    while start + part.len() <= text.len() {
        let mut n = 0;
        while n < part.len() && text[start + n] == part[n] {
            n += 1;
        }
        if n == part.len() {
            return true;
        }
        start += 1;
    }
    false
}
pub(in super::super) const fn module(path: &str) -> usize {
    if !contains(path, "::native_frames") {
        return 0;
    }
    if contains(path, "::scheduler::closure") {
        return 15;
    }
    if contains(path, "::sessions::registry") {
        return 9;
    }
    if contains(path, "::sessions::transports") {
        return 10;
    }
    if contains(path, "::sessions::input") {
        return 11;
    }
    if contains(path, "::sessions::deferred") {
        return 12;
    }
    if contains(path, "::jobs") {
        2
    } else if contains(path, "::control") {
        3
    } else if contains(path, "::operations") {
        4
    } else if contains(path, "::consumers") {
        5
    } else if contains(path, "::worlds") {
        6
    } else if contains(path, "::loads") {
        7
    } else if contains(path, "::sessions") {
        8
    } else if contains(path, "::wire") {
        13
    } else if contains(path, "::scheduler") {
        14
    } else {
        1
    }
}
pub(in super::super) fn identity(pid: u64, created: u64) {
    DATA.header[2].store(pid, Ordering::Release);
    DATA.header[3].store(created, Ordering::Release);
}
pub(in super::super) fn prepared() {
    DATA.header[8].store(1, Ordering::Release);
}
pub(in super::super) fn installed() {
    DATA.header[9].store(1, Ordering::Release);
}
pub(in super::super) fn restored() {
    DATA.header[10].store(1, Ordering::Release);
}
fn row(module: usize, kind: u64) -> Option<&'static Counters> {
    if module == 0 || module >= MODULES || kind >= KINDS as u64 {
        return None;
    }
    DATA.rows.get(module * KINDS + kind as usize)
}
/// These helpers touch only preallocated atomics and the gateway's own saved
/// stack frame. They make no Win32 calls and leave LastError unchanged.
pub(in super::super) unsafe extern "system" fn entered(raw: *const Saved, module: usize) {
    let kind = unsafe { (*raw).invocation.kind };
    if let Some(row) = row(module, kind) {
        row.entered.fetch_add(1, Ordering::Relaxed);
        if DATA.header[8].load(Ordering::Acquire) == 0 {
            DATA.header[11].fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub(in super::super) unsafe extern "system" fn returned(raw: *const Saved, module: usize) {
    if let Some(row) = row(module, unsafe { (*raw).invocation.kind }) {
        row.returned.fetch_add(1, Ordering::Relaxed);
    }
}
pub(in super::super) fn terminal(module: usize, kind: u64) {
    if let Some(row) = row(module, kind) {
        row.terminal.fetch_add(1, Ordering::Relaxed);
    }
}
pub(in super::super) fn fault(module: usize, line: u32) {
    if module == 0 || module >= MODULES {
        return;
    }
    // Publish module and source line together. A crashing process cannot
    // leave a reader with a module from one fault and an unpublished line.
    let packed = ((module as u64) << 32) | u64::from(line);
    let _ = DATA.header[6].compare_exchange(0, packed, Ordering::AcqRel, Ordering::Acquire);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_native_modules_have_distinct_census_rows() {
        assert_eq!(module("c::windows::root_observation"), 0);
        assert_eq!(
            module("c::windows::root_observation::native_frames::sessions::input"),
            11
        );
        assert_eq!(
            module("c::windows::root_observation::native_frames::sessions::registry"),
            9
        );
        assert_eq!(
            module("c::windows::root_observation::native_frames::scheduler::closure"),
            15
        );
        assert!(
            row(0, 10).is_none() && row(MODULES, 10).is_none() && row(1, KINDS as u64).is_none()
        );
        assert_eq!(std::mem::size_of::<Census>(), SIZE);
    }
}
