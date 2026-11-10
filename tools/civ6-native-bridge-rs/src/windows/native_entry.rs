//! Publish only bytes within the first instruction. A caller already inside
//! the original prologue must keep executing unchanged instructions.
//!
//! Short entries use verified executable padding. When no short island is
//! available, a resident breakpoint handler changes only RIP to the existing
//! whole-call gateway. This is entry publication, not a native drain proof.
use super::*;
use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
use windows_sys::Win32::System::Diagnostics::Debug::{
    AddVectoredExceptionHandler, RtlLookupFunctionEntry, EXCEPTION_POINTERS,
};

const TRAP_SLOTS: usize = 128;
struct Trap {
    target: AtomicU64,
    gateway: AtomicU64,
}
static TRAPS: [Trap; TRAP_SLOTS] = [const {
    Trap {
        target: AtomicU64::new(0),
        gateway: AtomicU64::new(0),
    }
}; TRAP_SLOTS];
static TRAP_SETUP: Mutex<()> = Mutex::new(());
static HANDLER: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

pub(super) type EntryPlan = ([u8; 8], Option<(*mut u8, [u8; PROPERTY_PREFIX_LEN])>);

/// Only instruction forms in the pinned, independently checked entry tables.
/// Unknown bytes never fall back to a multi-instruction overwrite.
fn first_instruction(bytes: &[u8; 8]) -> Option<usize> {
    match *bytes {
        [0x40, 0x50..=0x57, ..] => Some(2),
        [0x48, 0x8b, 0xc4 | 0xca, ..] | [0x4c, 0x8b, 0xdc, ..] | [0x48, 0x85, 0xd2, ..] => Some(3),
        [0x48, 0x83, 0xec, ..] | [0x48, 0x8b, 0x49 | 0x51, 0x08, ..] => Some(4),
        [0x44 | 0x48 | 0x4c, 0x89 | 0x8b, 0x44 | 0x4c | 0x54 | 0x5c | 0x74 | 0x7c, 0x24, ..] => {
            Some(5)
        }
        [0x8b, 0x81, ..] => Some(6),
        [0x48, 0x8b, 0x81, ..] => Some(7),
        _ => None,
    }
}

fn padding_slot(target: *mut u8) -> Option<*mut u8> {
    let address = target as usize;
    let mut entry_info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe {
        VirtualQuery(
            target.cast(),
            &mut entry_info,
            std::mem::size_of_val(&entry_info),
        )
    } == 0
    {
        return None;
    }
    let rva = address.checked_sub(entry_info.AllocationBase as usize)?;
    // Nonadjacent islands in the pinned Host/Core images. The adjacent case
    // is tried first. Each candidate is rechecked against the live mapping.
    let island = match rva {
        0x5f470 => address.checked_add(98),
        0x81b330 => address.checked_add(82),
        0x39920 | 0x39650 | 0x63e160 => address.checked_sub(53),
        _ => None,
    };
    for slot in [address.checked_sub(PROPERTY_PREFIX_LEN), island]
        .into_iter()
        .flatten()
    {
        if i8::try_from(slot as i128 - address as i128 - 2).is_err() {
            continue;
        }
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(slot as _, &mut info, std::mem::size_of_val(&info)) } == 0
            || info.AllocationBase != entry_info.AllocationBase
            || info.State != MEM_COMMIT
            || info.Protect & PAGE_GUARD != 0
            || info.Protect & 0xff != PAGE_EXECUTE_READ
            || slot.checked_add(PROPERTY_PREFIX_LEN)? > info.BaseAddress as usize + info.RegionSize
            || copy_current::<PROPERTY_PREFIX_LEN>(slot as u64) != Some(PROPERTY_PREFIX_PAD)
        {
            continue;
        }
        let mut base = 0;
        if (0..PROPERTY_PREFIX_LEN).any(|offset| {
            !(unsafe { RtlLookupFunctionEntry((slot + offset) as u64, &mut base, null_mut()) })
                .is_null()
        }) {
            continue;
        }
        return Some(slot as *mut u8);
    }
    None
}

pub(super) fn plan(
    target: *mut u8,
    original: [u8; 8],
    relay: *mut u8,
) -> Result<EntryPlan, &'static str> {
    if target.is_null()
        || relay.is_null()
        || !target.addr().is_multiple_of(8)
        || copy_current::<8>(target as u64) != Some(original)
    {
        return Err("native entry identity/alignment changed");
    }
    let width = first_instruction(&original).ok_or("native entry instruction is not pinned")?;
    let mut patched = original;
    if width >= 5 {
        patched[..5].copy_from_slice(
            &rel32_jump(target as usize, relay as usize).ok_or("native entry relay range")?,
        );
        return Ok((patched, None));
    }
    if let Some(slot) = padding_slot(target) {
        let jump = rel32_jump(slot as usize, relay as usize).ok_or("native prefix relay range")?;
        let delta = i8::try_from(slot as i128 - target as i128 - 2)
            .map_err(|_| "native short entry range")?;
        patched[..2].copy_from_slice(&[0xeb, delta as u8]);
        return Ok((patched, Some((slot, jump))));
    }
    // No call-site census or pointer scan is substituted for the entry: all
    // direct/indirect callers still enter at this same original address.
    if copy_current::<6>(relay as u64) != Some([0xff, 0x25, 0, 0, 0, 0]) {
        return Err("native breakpoint gateway relay differs");
    }
    patched[0] = 0xcc;
    Ok((patched, None))
}

fn trap_gateway(target: u64) -> Option<u64> {
    TRAPS
        .iter()
        .find_map(|slot| {
            (slot.target.load(Ordering::Acquire) == target)
                .then(|| slot.gateway.load(Ordering::Acquire))
        })
        .filter(|gateway| *gateway != 0)
}

unsafe extern "system" fn breakpoint(info: *mut EXCEPTION_POINTERS) -> i32 {
    let error = unsafe { GetLastError() };
    let mut result = 0; // EXCEPTION_CONTINUE_SEARCH
    if !info.is_null() {
        let info = unsafe { &mut *info };
        if !info.ExceptionRecord.is_null() && !info.ContextRecord.is_null() {
            let record = unsafe { &*info.ExceptionRecord };
            let context = unsafe { &mut *info.ContextRecord };
            let target = record.ExceptionAddress as u64;
            if record.ExceptionCode as u32 == 0x80000003
                && record.ExceptionFlags & 1 == 0
                // Windows' saved x64 context identifies the INT3 itself,
                // not the following byte (verified by the owned raw probe).
                && target == context.Rip
            {
                if let Some(gateway) = trap_gateway(target) {
                    context.Rip = gateway;
                    result = -1; // EXCEPTION_CONTINUE_EXECUTION
                }
            }
        }
    }
    unsafe { SetLastError(error) };
    result
}

fn register_trap(target: *mut u8, relay: *mut u8) -> Result<(), &'static str> {
    let _setup = TRAP_SETUP
        .lock()
        .map_err(|_| "native breakpoint owner poisoned")?;
    if copy_current::<6>(relay as u64) != Some([0xff, 0x25, 0, 0, 0, 0]) {
        return Err("native breakpoint relay changed");
    }
    let gateway = copy_current::<8>(relay as u64 + 6)
        .map(u64::from_le_bytes)
        .filter(|n| *n != 0)
        .ok_or("native breakpoint gateway absent")?;
    if unsafe { windows_sys::Win32::System::Diagnostics::Debug::IsDebuggerPresent() } != 0 {
        return Err("native breakpoint entry cannot be published with an attached debugger");
    }
    let target_pinned = resident_module::pin_module(target.cast());
    if !target_pinned {
        #[cfg(not(test))]
        return Err("native breakpoint target module pin failed");
        #[cfg(test)]
        {
            let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { VirtualQuery(target.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
                || info.Type != windows_sys::Win32::System::Memory::MEM_PRIVATE
            {
                return Err("owned synthetic breakpoint allocation missing");
            }
        }
    }
    if let Some(existing) = trap_gateway(target as u64) {
        return if existing == gateway {
            Ok(())
        } else {
            Err("native breakpoint target has another owner")
        };
    }
    let slot = TRAPS
        .iter()
        .find(|slot| slot.target.load(Ordering::Acquire) == 0)
        .ok_or("native breakpoint capacity")?;
    if HANDLER.load(Ordering::Acquire).is_null() {
        // Handler code, tables, gateways and delayed exception destinations
        // must outlive restored entries. Never unregister/free them on drain.
        if !resident_module::pin_module(breakpoint as *const () as *const c_void) {
            return Err("native breakpoint module pin failed");
        }
        let handler = unsafe { AddVectoredExceptionHandler(1, Some(breakpoint)) };
        if handler.is_null() {
            return Err("native breakpoint registration failed");
        }
        HANDLER.store(handler, Ordering::Release);
    }
    slot.gateway.store(gateway, Ordering::Release);
    slot.target.store(target as u64, Ordering::Release);
    Ok(())
}

/// The caller has already retained the Hook and relay before this can publish.
///
/// # Safety
/// The caller validates the pinned image, CPU policy and gateway ABI, and
/// keeps entry owners, original module, callbacks and continuations resident.
/// Entry publication does not establish a drain of older native callers.
pub(super) unsafe fn publish(hook: &Hook) -> Result<PatchOutcome, &'static str> {
    if copy_current::<8>(hook.target as u64) != Some(hook.original) {
        return Err("critical: native entry changed before publication");
    }
    if let Some((slot, jump)) = hook.prefix {
        unsafe { publish_property_prefix(slot, PROPERTY_PREFIX_PAD, jump) }?;
    } else if hook.patched[0] == 0xcc {
        register_trap(hook.target, hook._code)?;
    }
    // The remaining bytes are identical. An original instruction boundary
    // after the first instruction is never replaced by jump displacement/NOP.
    unsafe {
        exchange_entry(
            hook.target,
            u64::from_le_bytes(hook.original),
            u64::from_le_bytes(hook.patched),
        )
    }
}

pub(super) fn owned(hook: &Hook) -> bool {
    copy_current::<8>(hook.target as u64) == Some(hook.patched)
        && hook.prefix.is_none_or(|(slot, jump)| {
            copy_current::<PROPERTY_PREFIX_LEN>(slot as u64) == Some(jump)
        })
        && (hook.patched[0] != 0xcc
            || (!HANDLER.load(Ordering::Acquire).is_null()
                && copy_current::<8>(hook._code as u64 + 6)
                    .map(u64::from_le_bytes)
                    .is_some_and(|gateway| {
                        gateway != 0 && Some(gateway) == trap_gateway(hook.target as u64)
                    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Diagnostics::Debug::RemoveVectoredExceptionHandler;
    struct Page(*mut u8);
    impl Page {
        fn new() -> Self {
            let pointer = unsafe {
                VirtualAlloc(
                    null_mut(),
                    CODE_PAGE,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                )
            }
            .cast::<u8>();
            assert!(!pointer.is_null());
            Self(pointer)
        }
        fn protect(&self, protection: u32) {
            let mut previous = 0;
            assert_ne!(
                unsafe { VirtualProtect(self.0.cast(), CODE_PAGE, protection, &mut previous) },
                0
            );
        }
    }
    impl Drop for Page {
        fn drop(&mut self) {
            assert_ne!(unsafe { VirtualFree(self.0.cast(), 0, MEM_RELEASE) }, 0);
        }
    }
    #[test]
    fn pinned_instruction_widths_keep_every_following_instruction_unchanged() {
        let page = Page::new();
        let target = unsafe { page.0.add(32) };
        let relay = unsafe { page.0.add(128) };
        for (original, width) in [
            ([0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x90, 0xc3], 2),
            ([0x48, 0x8b, 0xc4, 0x53, 0x55, 0x56, 0x57, 0x90], 3),
            ([0x4c, 0x8b, 0xdc, 0x55, 0x56, 0x57, 0x90, 0xc3], 3),
            ([0x48, 0x85, 0xd2, 0x74, 0x08, 0x90, 0x90, 0xc3], 3),
            ([0x48, 0x83, 0xec, 0x38, 0x48, 0x8b, 0xc1, 0x90], 4),
            ([0x48, 0x8b, 0x49, 0x08, 0x48, 0x85, 0xc9, 0xc3], 4),
            ([0x48, 0x89, 0x5c, 0x24, 0x08, 0x57, 0x90, 0xc3], 5),
            ([0x8b, 0x81, 0x34, 0x12, 0x00, 0x00, 0x90, 0xc3], 6),
            ([0x48, 0x8b, 0x81, 0x34, 0x12, 0x00, 0x00, 0xc3], 7),
        ] {
            page.protect(PAGE_READWRITE);
            unsafe {
                ptr::copy_nonoverlapping(original.as_ptr(), target, 8);
                ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
            }
            page.protect(PAGE_EXECUTE_READ);
            assert_eq!(first_instruction(&original), Some(width));
            let (patched, prefix) = plan(target, original, relay).unwrap();
            assert!(prefix.is_none());
            assert_eq!(&patched[width..], &original[width..]);
            if width >= 5 {
                assert_eq!(
                    &patched[..5],
                    &rel32_jump(target as usize, relay as usize).unwrap()
                );
            } else {
                assert_eq!(patched[0], 0xcc);
                assert_eq!(&patched[1..], &original[1..]);
            }
        }
    }
    #[test]
    fn unknown_instruction_changed_bytes_alignment_and_relay_are_refused_before_publication() {
        let page = Page::new();
        let target = unsafe { page.0.add(32) };
        let relay = unsafe { page.0.add(128) };
        let original = [0x48, 0x8b, 0xc4, 0x53, 0x55, 0x56, 0x57, 0x90];
        unsafe {
            ptr::copy_nonoverlapping(original.as_ptr(), target, 8);
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
        }
        page.protect(PAGE_EXECUTE_READ);
        assert!(plan(target, original, relay).is_ok());
        assert!(plan(null_mut(), original, relay).is_err());
        assert!(plan(target, original, null_mut()).is_err());
        let unaligned = unsafe { target.add(1) };
        page.protect(PAGE_READWRITE);
        unsafe {
            ptr::copy_nonoverlapping(original.as_ptr(), unaligned, 8);
        }
        page.protect(PAGE_EXECUTE_READ);
        let unaligned_plan = plan(unaligned, original, relay);
        page.protect(PAGE_READWRITE);
        unsafe {
            ptr::copy_nonoverlapping(original.as_ptr(), target, 8);
        }
        page.protect(PAGE_EXECUTE_READ);
        assert!(
            unaligned_plan.is_err(),
            "a valid instruction at a misaligned entry must be refused for alignment itself"
        );
        let mut changed = original;
        changed[7] ^= 1;
        assert!(plan(target, changed, relay).is_err());
        page.protect(PAGE_READWRITE);
        unsafe {
            *relay = 0x90;
        }
        page.protect(PAGE_EXECUTE_READ);
        assert!(plan(target, original, relay).is_err());
        for unknown in [
            [0x90; 8],
            [0xe9, 1, 2, 3, 4, 0x90, 0x90, 0xc3],
            [0x48, 0x8b, 0x49, 0x09, 0x90, 0x90, 0x90, 0xc3],
        ] {
            page.protect(PAGE_READWRITE);
            unsafe {
                ptr::copy_nonoverlapping(unknown.as_ptr(), target, 8);
            }
            page.protect(PAGE_EXECUTE_READ);
            assert!(
                plan(target, unknown, relay).is_err(),
                "unknown instruction must never be treated as a 5-byte overwrite"
            );
            assert_eq!(copy_current::<8>(target as u64), Some(unknown));
        }
    }
    #[test]
    fn padding_requires_read_execute_protection_and_owned_checks_its_exact_published_bytes() {
        let page = Page::new();
        let target = unsafe { page.0.add(32) };
        let slot = unsafe { target.sub(PROPERTY_PREFIX_LEN) };
        let relay = unsafe { page.0.add(128) };
        let original = [0x48, 0x8b, 0xc4, 0x53, 0x55, 0x56, 0x57, 0x90];
        unsafe {
            ptr::copy_nonoverlapping(original.as_ptr(), target, 8);
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), slot, PROPERTY_PREFIX_LEN);
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
        }
        for protection in [PAGE_READWRITE, PAGE_EXECUTE_READWRITE, PAGE_READONLY] {
            page.protect(protection);
            assert_eq!(padding_slot(target), None);
        }
        page.protect(PAGE_EXECUTE_READ);
        assert_eq!(padding_slot(target), Some(slot));
        let (patched, prefix) = plan(target, original, relay).unwrap();
        let (_, jump) = prefix.unwrap();
        assert_eq!(patched[0], 0xeb);
        let hook = Hook {
            target,
            original,
            patched,
            prefix,
            _code: relay,
        };
        page.protect(PAGE_READWRITE);
        unsafe {
            ptr::copy_nonoverlapping(patched.as_ptr(), target, 8);
            ptr::copy_nonoverlapping(jump.as_ptr(), slot, PROPERTY_PREFIX_LEN);
        }
        assert!(owned(&hook));
        unsafe {
            *slot ^= 1;
        }
        assert!(
            !owned(&hook),
            "matching entry bytes cannot hide a foreign prefix"
        );
        unsafe {
            *slot ^= 1;
            *target ^= 1;
        }
        assert!(!owned(&hook));
        unsafe {
            *target ^= 1;
        }
        assert!(owned(&hook));
    }
    #[test]
    fn adjacent_padding_in_another_allocation_is_never_borrowed() {
        const GRANULARITY: usize = 65536;
        let mut pair = None;
        for _ in 0..5 {
            let reserved =
                unsafe { VirtualAlloc(null_mut(), 2 * GRANULARITY, MEM_RESERVE, PAGE_NOACCESS) };
            assert!(!reserved.is_null());
            assert_ne!(unsafe { VirtualFree(reserved, 0, MEM_RELEASE) }, 0);
            let left = unsafe {
                VirtualAlloc(
                    reserved,
                    GRANULARITY,
                    MEM_RESERVE | MEM_COMMIT,
                    PAGE_READWRITE,
                )
            }
            .cast::<u8>();
            let right = unsafe {
                VirtualAlloc(
                    (reserved as usize + GRANULARITY) as _,
                    GRANULARITY,
                    MEM_RESERVE | MEM_COMMIT,
                    PAGE_READWRITE,
                )
            }
            .cast::<u8>();
            if !left.is_null() && !right.is_null() {
                pair = Some((Page(left), Page(right)));
                break;
            }
            for pointer in [left, right]
                .into_iter()
                .filter(|pointer| !pointer.is_null())
            {
                assert_ne!(unsafe { VirtualFree(pointer.cast(), 0, MEM_RELEASE) }, 0);
            }
        }
        let (left, right) =
            pair.expect("owned adjacent allocations must be established, not skipped");
        let target = right.0;
        let slot = unsafe { left.0.add(GRANULARITY - PROPERTY_PREFIX_LEN) };
        let relay = unsafe { right.0.add(128) };
        let original = [0x48, 0x8b, 0xc4, 0x53, 0x55, 0x56, 0x57, 0x90];
        unsafe {
            ptr::copy_nonoverlapping(original.as_ptr(), target, 8);
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), slot, PROPERTY_PREFIX_LEN);
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
        }
        for page in [&left, &right] {
            let mut previous = 0;
            assert_ne!(
                unsafe {
                    VirtualProtect(page.0.cast(), GRANULARITY, PAGE_EXECUTE_READ, &mut previous)
                },
                0
            );
        }
        assert_eq!(slot as usize + PROPERTY_PREFIX_LEN, target as usize);
        assert_eq!(
            copy_current::<PROPERTY_PREFIX_LEN>(slot as u64),
            Some(PROPERTY_PREFIX_PAD)
        );
        assert_eq!(padding_slot(target), None);
        let (patched, prefix) = plan(target, original, relay).unwrap();
        assert!(prefix.is_none());
        assert_eq!(patched[0], 0xcc);
    }
    static PROBE_TARGET: AtomicU64 = AtomicU64::new(0);
    static PROBE_RIP: AtomicU64 = AtomicU64::new(u64::MAX);
    #[test]
    fn padding_must_exclude_every_byte_of_a_registered_function() {
        use windows_sys::Win32::System::Diagnostics::Debug::{
            RtlAddFunctionTable, RtlDeleteFunctionTable, IMAGE_RUNTIME_FUNCTION_ENTRY,
        };
        let page = unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        }
        .cast::<u8>();
        assert!(!page.is_null());
        let target = unsafe { page.add(32) };
        let relay = unsafe { page.add(128) };
        let original = [0x48, 0x8b, 0xc4, 0x8d, 0x41, 0x30, 0xc3, 0xcc];
        unsafe {
            ptr::copy_nonoverlapping(original.as_ptr(), target, 8);
            ptr::copy_nonoverlapping(
                PROPERTY_PREFIX_PAD.as_ptr(),
                target.sub(PROPERTY_PREFIX_LEN),
                PROPERTY_PREFIX_LEN,
            );
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
            ptr::copy_nonoverlapping([1, 0, 0, 0].as_ptr(), page.add(512), 4);
        }
        let mut previous = 0;
        assert_ne!(
            unsafe { VirtualProtect(page.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut previous) },
            0
        );
        let mut function: IMAGE_RUNTIME_FUNCTION_ENTRY = unsafe { std::mem::zeroed() };
        function.BeginAddress = 29;
        function.EndAddress = 30;
        function.Anonymous.UnwindData = 512;
        assert!(unsafe { RtlAddFunctionTable(&function, 1, page as u64) });
        let mut image_base = 0;
        let middle_covered =
            !(unsafe { RtlLookupFunctionEntry(page as u64 + 29, &mut image_base, null_mut()) })
                .is_null();
        let planned = plan(target, original, relay);
        let removed = unsafe { RtlDeleteFunctionTable(&function) };
        let released = unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) };
        assert!(removed && released != 0 && middle_covered);
        let (patched, prefix) = planned.unwrap();
        assert!(
            prefix.is_none(),
            "an interior function is not unused padding, even when both endpoints are uncovered"
        );
        assert_eq!(patched[0], 0xcc);
    }
    core::arch::global_asm!(
        ".text",
        ".p2align 4",
        ".fill 16,1,0x90",
        ".global native_entry_owned_module_fixture",
        "native_entry_owned_module_fixture:",
        ".byte 0x48,0x8b,0xc4,0x8d,0x41,0x30,0xc3,0xcc",
    );
    unsafe extern "system" {
        fn native_entry_owned_module_fixture(argument: usize) -> u32;
    }
    extern "system" fn owned_module_gateway(argument: usize) -> u32 {
        argument.wrapping_add(0x30) as u32
    }
    #[test]
    fn breakpoint_entry_pins_an_actual_owned_image_and_keeps_late_exception_destination_resident() {
        let Some(_executed)=crate::windows::atomic_fixture::require("breakpoint_entry_pins_an_actual_owned_image_and_keeps_late_exception_destination_resident") else {return};
        let target = native_entry_owned_module_fixture as *const () as *mut u8;
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe { VirtualQuery(target.cast(), &mut info, std::mem::size_of_val(&info)) },
            0
        );
        assert_eq!(
            info.Type,
            windows_sys::Win32::System::Memory::MEM_IMAGE,
            "this is the real module-pin branch, not the private-page test exemption"
        );
        let relay = unsafe { executable_near(target as usize, target as usize, 3) }.unwrap();
        unsafe {
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
            ptr::copy_nonoverlapping(
                (owned_module_gateway as *const () as u64)
                    .to_le_bytes()
                    .as_ptr(),
                relay.add(6),
                8,
            );
        }
        let mut previous = 0;
        assert_ne!(
            unsafe { VirtualProtect(relay.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut previous) },
            0
        );
        assert_ne!(
            unsafe { FlushInstructionCache(GetCurrentProcess(), relay.cast(), 14) },
            0
        );
        let original = [0x48, 0x8b, 0xc4, 0x8d, 0x41, 0x30, 0xc3, 0xcc];
        let (patched, prefix) = plan(target, original, relay).unwrap();
        assert!(prefix.is_none());
        assert_eq!(patched[0], 0xcc);
        let hook = Hook {
            target,
            original,
            patched,
            prefix,
            _code: relay,
        };
        let published = unsafe { publish(&hook) }.unwrap();
        assert!(published.exchanged && published.protection_restored && published.cache_flushed);
        unsafe { SetLastError(0x87654321) };
        let result = unsafe { native_entry_owned_module_fixture(0x12340000) };
        let error = unsafe { GetLastError() };
        assert!(owned(&hook));
        // An unchanged entry cannot prove ownership when its late exception
        // destination no longer matches the retained mapping.
        let destination = unsafe { ptr::read_unaligned(relay.add(6).cast::<u64>()) };
        assert_ne!(
            unsafe { VirtualProtect(relay.cast(), CODE_PAGE, PAGE_READWRITE, &mut previous) },
            0
        );
        unsafe {
            ptr::write_unaligned(relay.add(6).cast::<u64>(), destination ^ 8);
        }
        let mismatched_destination_owned = owned(&hook);
        unsafe {
            ptr::write_unaligned(relay.add(6).cast::<u64>(), destination);
        }
        assert_ne!(
            unsafe { VirtualProtect(relay.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut previous) },
            0
        );
        assert!(!mismatched_destination_owned);
        assert!(owned(&hook));
        let restored = unsafe {
            exchange_entry(
                target,
                u64::from_le_bytes(patched),
                u64::from_le_bytes(original),
            )
        }
        .unwrap();
        assert!(restored.exchanged && restored.protection_restored && restored.cache_flushed);
        assert_eq!(result, 0x12340030);
        assert_eq!(error, 0x87654321);
        assert_eq!(copy_current::<8>(target as u64), Some(original));
        assert!(!owned(&hook));
        assert_eq!(
            trap_gateway(target as u64),
            Some(owned_module_gateway as *const () as u64)
        );
        // A trap fetched before restoration still names pinned module code.
        // This destination lifetime is not an all-native completion receipt.
        use windows_sys::Win32::System::Diagnostics::Debug::{CONTEXT, EXCEPTION_RECORD};
        let mut context: CONTEXT = unsafe { std::mem::zeroed() };
        context.Rip = target as u64;
        let mut record: EXCEPTION_RECORD = unsafe { std::mem::zeroed() };
        record.ExceptionCode = 0x80000003u32 as _;
        record.ExceptionAddress = target.cast();
        let mut exception = EXCEPTION_POINTERS {
            ExceptionRecord: &mut record,
            ContextRecord: &mut context,
        };
        assert_eq!(unsafe { breakpoint(&mut exception) }, -1);
        assert_eq!(context.Rip, owned_module_gateway as *const () as u64);
        assert_eq!(
            unsafe { native_entry_owned_module_fixture(0x12340000) },
            0x12340030
        );
        // Retain relay storage with the resident handler until process exit.
    }
    unsafe extern "system" fn probe_breakpoint(info: *mut EXCEPTION_POINTERS) -> i32 {
        let error = unsafe { GetLastError() };
        let info = unsafe { &mut *info };
        let record = unsafe { &*info.ExceptionRecord };
        let context = unsafe { &mut *info.ContextRecord };
        let target = PROBE_TARGET.load(Ordering::Acquire);
        let handled =
            record.ExceptionCode as u32 == 0x80000003 && record.ExceptionAddress as u64 == target;
        if handled {
            PROBE_RIP.store(context.Rip, Ordering::Release);
            context.Rip = target + 1;
        }
        unsafe { SetLastError(error) };
        if handled {
            -1
        } else {
            0
        }
    }
    #[test]
    fn owned_int3_reports_its_actual_windows_context_without_running_game_code() {
        let page = unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        }
        .cast::<u8>();
        assert!(!page.is_null());
        unsafe {
            ptr::copy_nonoverlapping([0xcc, 0xb8, 0x78, 0x56, 0x34, 0x12, 0xc3].as_ptr(), page, 7)
        };
        let mut previous = 0;
        assert_ne!(
            unsafe { VirtualProtect(page.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut previous) },
            0
        );
        assert_ne!(
            unsafe { FlushInstructionCache(GetCurrentProcess(), page.cast(), 7) },
            0
        );
        PROBE_TARGET.store(page as u64, Ordering::Release);
        let handler = unsafe { AddVectoredExceptionHandler(1, Some(probe_breakpoint)) };
        assert!(!handler.is_null());
        let call: unsafe extern "system" fn() -> u32 = unsafe { std::mem::transmute(page) };
        unsafe { SetLastError(0x87654321) };
        let result = unsafe { call() };
        let error = unsafe { GetLastError() };
        let target = page as u64;
        let rip = PROBE_RIP.load(Ordering::Acquire);
        assert_ne!(unsafe { RemoveVectoredExceptionHandler(handler) }, 0);
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
        assert_eq!(result, 0x12345678);
        assert_eq!(error, 0x87654321);
        println!(
            "OWNED_INT3_CONTEXT target={target:#x} rip={rip:#x} delta={}",
            rip.wrapping_sub(target)
        );
        assert_eq!(
            rip, target,
            "the production handler requires the observed Windows x64 fault address"
        );
    }
    #[test]
    fn breakpoint_changes_only_owned_rip_and_never_consumes_unrelated_exceptions() {
        use windows_sys::Win32::System::Diagnostics::Debug::{CONTEXT, EXCEPTION_RECORD};
        let slot = &TRAPS[TRAP_SLOTS - 1];
        let target = 0x10000000000011u64;
        slot.gateway.store(0x23456789, Ordering::Release);
        slot.target.store(target, Ordering::Release);
        for (address, rip, code, flags, handled) in [
            (target, target, 0x80000003u32, 0u32, true),
            (target + 8, target + 8, 0x80000003, 0, false),
            (target, target + 1, 0x80000003, 0, false),
            (target, target, 0xc0000005, 0, false),
            (target, target, 0x80000003, 1, false),
        ] {
            let mut context: CONTEXT = unsafe { std::mem::zeroed() };
            context.Rip = rip;
            context.Rsp = 0x1111;
            context.Rax = 1;
            context.Rbx = 2;
            context.Rcx = 3;
            context.Rdx = 4;
            context.Rsi = 5;
            context.Rdi = 6;
            context.Rbp = 7;
            context.R8 = 8;
            context.R9 = 9;
            context.R10 = 10;
            context.R11 = 11;
            context.R12 = 12;
            context.R13 = 13;
            context.R14 = 14;
            context.R15 = 15;
            context.EFlags = 0x246;
            context.MxCsr = 0x1f80;
            let registers = |c: &CONTEXT| {
                [
                    c.Rsp,
                    c.Rax,
                    c.Rbx,
                    c.Rcx,
                    c.Rdx,
                    c.Rsi,
                    c.Rdi,
                    c.Rbp,
                    c.R8,
                    c.R9,
                    c.R10,
                    c.R11,
                    c.R12,
                    c.R13,
                    c.R14,
                    c.R15,
                    u64::from(c.EFlags),
                    u64::from(c.MxCsr),
                ]
            };
            let before = registers(&context);
            let mut record: EXCEPTION_RECORD = unsafe { std::mem::zeroed() };
            record.ExceptionCode = code as _;
            record.ExceptionAddress = address as *mut _;
            record.ExceptionFlags = flags;
            let mut info = EXCEPTION_POINTERS {
                ExceptionRecord: &mut record,
                ContextRecord: &mut context,
            };
            unsafe { SetLastError(0x87654321) };
            let result = unsafe { breakpoint(&mut info) };
            let error = unsafe { GetLastError() };
            assert_eq!(result, if handled { -1 } else { 0 });
            assert_eq!(context.Rip, if handled { 0x23456789 } else { rip });
            assert_eq!(registers(&context), before);
            assert_eq!(error, 0x87654321);
        }
        slot.target.store(0, Ordering::Release);
        slot.gateway.store(0, Ordering::Release);
    }
}
