use super::*;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::process::CommandExt;
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
}

const WORLD: u64 = 0x1000;
const MANAGER: u64 = 0x2000;
const SET: u64 = 0x2400;
const LIST: u32 = 5;

fn waited(l: &mut Ledger) {
    l.init_returned(MANAGER, LIST).unwrap();
    l.close_begin(WORLD, MANAGER, LIST).unwrap();
    l.wait_returned(MANAGER).unwrap();
}

#[test]
fn pending_publication_is_incarnation_bound_and_rejected_without_consuming_another() {
    let mut l = Ledger::new();
    assert_eq!(l.begin_publication(SET), Ok(None));
    let serial = l.init_returned(MANAGER, LIST).unwrap();
    let first = l.begin_publication(SET).unwrap().unwrap();
    assert_eq!(first.serial, serial);
    assert_eq!(
        l.init_returned(MANAGER, LIST),
        Err(Violation::PublicationPending)
    );
    l.close_begin(WORLD, MANAGER, LIST).unwrap();
    let second = l.begin_publication(SET).unwrap().unwrap();
    assert_eq!(l.wait_returned(MANAGER), Err(Violation::PublicationPending));
    for bad in [
        Publication {
            manager: MANAGER + 1,
            ..first
        },
        Publication {
            serial: serial + 1,
            ..first
        },
        Publication {
            id: u64::MAX,
            ..first
        },
    ] {
        assert_eq!(
            l.finish_publication(bad),
            Err(Violation::PublicationIdentity)
        );
        assert_eq!(l.publications.len(), 2);
    }
    l.finish_publication(first).unwrap();
    assert_eq!(
        l.finish_publication(first),
        Err(Violation::PublicationIdentity)
    );
    assert_eq!(l.wait_returned(MANAGER), Err(Violation::PublicationPending));
    l.finish_publication(second).unwrap();
    l.wait_returned(MANAGER).unwrap();
    assert_eq!(l.begin_publication(SET), Err(Violation::LateSubmit));
    l.list_released(LIST, true, MANAGER).unwrap();
    l.close_end(MANAGER).unwrap();
    let receipt = l.world_closed(WORLD).unwrap();
    assert!(receipt.publication_overlap && !receipt.complete());
    assert!(l.init_returned(MANAGER, LIST + 1).unwrap() > serial);
    let next = l.begin_publication(SET).unwrap().unwrap();
    assert_eq!(
        l.finish_publication(second),
        Err(Violation::PublicationIdentity)
    );
    assert_eq!(l.publications, vec![next]);
    l.finish_publication(next).unwrap();
    assert!(l.publications.is_empty());
}

#[test]
fn other_manager_publication_does_not_block_a_drained_manager() {
    let mut l = Ledger::new();
    l.init_returned(MANAGER, 0).unwrap();
    l.init_returned(0x6000, 0).unwrap();
    let other = l.begin_publication(0x6400).unwrap().unwrap();
    l.close_begin(WORLD, MANAGER, 0).unwrap();
    l.wait_returned(MANAGER).unwrap();
    l.close_end(MANAGER).unwrap();
    assert!(l.world_closed(WORLD).unwrap().complete());
    assert_eq!(l.publications, vec![other]);
    l.finish_publication(other).unwrap();
}

#[test]
fn spawn_publication_keeps_its_exact_incarnation_until_original_return() {
    let mut l = Ledger::new();
    assert_eq!(l.begin_spawn_publication(LIST), Ok(None));
    let serial = l.init_returned(MANAGER, LIST).unwrap();
    assert_eq!(l.begin_spawn_publication(0), Ok(None));
    assert_eq!(l.begin_spawn_publication(LIST + 1), Ok(None));
    let publication = l.begin_spawn_publication(LIST).unwrap().unwrap();
    assert_eq!((publication.manager, publication.serial), (MANAGER, serial));
    assert_eq!(
        l.init_returned(MANAGER, LIST),
        Err(Violation::PublicationPending)
    );
    for bad in [
        Publication {
            manager: MANAGER + 1,
            ..publication
        },
        Publication {
            serial: serial + 1,
            ..publication
        },
        Publication {
            id: u64::MAX,
            ..publication
        },
    ] {
        assert_eq!(
            l.finish_publication(bad),
            Err(Violation::PublicationIdentity)
        );
        assert_eq!(l.publications, vec![publication]);
    }
    l.close_begin(WORLD, MANAGER, LIST).unwrap();
    assert_eq!(l.wait_returned(MANAGER), Err(Violation::PublicationPending));
    l.finish_publication(publication).unwrap();
    assert_eq!(
        l.finish_publication(publication),
        Err(Violation::PublicationIdentity)
    );
    l.wait_returned(MANAGER).unwrap();
    l.list_released(LIST, true, MANAGER).unwrap();
    assert_eq!(l.begin_spawn_publication(LIST), Ok(None));
    l.close_end(MANAGER).unwrap();
    assert!(!l.world_closed(WORLD).unwrap().complete());
    l.init_returned(MANAGER, LIST).unwrap();
    let next = l.begin_spawn_publication(LIST).unwrap().unwrap();
    assert_eq!(
        l.finish_publication(publication),
        Err(Violation::PublicationIdentity)
    );
    assert_eq!(l.publications, vec![next]);
    l.finish_publication(next).unwrap();
    l.close_begin(WORLD + 1, MANAGER, LIST).unwrap();
    l.wait_returned(MANAGER).unwrap();
    l.list_released(LIST, true, MANAGER).unwrap();
    l.close_end(MANAGER).unwrap();
    assert!(l.world_closed(WORLD + 1).unwrap().complete());
}

#[test]
fn spawn_publication_uses_the_live_list_owner_and_does_not_block_another_manager() {
    let mut l = Ledger::new();
    l.init_returned(MANAGER, LIST).unwrap();
    l.init_returned(0x6000, LIST + 1).unwrap();
    let other = l.begin_spawn_publication(LIST + 1).unwrap().unwrap();
    l.close_begin(WORLD, MANAGER, LIST).unwrap();
    let own = l.begin_spawn_publication(LIST).unwrap().unwrap();
    l.finish_publication(own).unwrap();
    l.wait_returned(MANAGER).unwrap();
    l.list_released(LIST, true, MANAGER).unwrap();
    l.close_end(MANAGER).unwrap();
    assert!(l.world_closed(WORLD).unwrap().complete());
    assert_eq!(l.publications, vec![other]);
    l.finish_publication(other).unwrap();

    // An owned Waited list wins over an older Open record with the same id.
    let mut l = Ledger::new();
    l.init_returned(0x8000, LIST).unwrap();
    l.init_returned(MANAGER, LIST).unwrap();
    l.close_begin(WORLD, MANAGER, LIST).unwrap();
    l.wait_returned(MANAGER).unwrap();
    assert_eq!(l.begin_spawn_publication(LIST), Ok(None));
    assert!(!l.managers[0].late_spawn && l.managers[1].late_spawn);
    assert!(l.publications.is_empty());
    l.list_released(LIST, true, MANAGER).unwrap();
    l.close_end(MANAGER).unwrap();
    assert!(!l.world_closed(WORLD).unwrap().complete());
    // After release the same id may belong to the other native owner.
    let live = l.begin_spawn_publication(LIST).unwrap().unwrap();
    assert_eq!(live.manager, 0x8000);
    l.finish_publication(live).unwrap();
}

#[test]
fn closed_manager_is_a_tombstone_until_a_real_new_initialization() {
    let mut l = Ledger::new();
    let first = l.init_returned(MANAGER, LIST).unwrap();
    assert_eq!(l.submit(SET), Ok(()));
    l.close_begin(WORLD, MANAGER, LIST).unwrap();
    assert_eq!(l.submit(SET), Ok(()));
    assert_eq!(l.submit(SET), Ok(()));
    assert_eq!(l.spawn(LIST), Ok(()));
    l.wait_returned(MANAGER).unwrap();
    assert_eq!(l.list_released(LIST, true, MANAGER), Ok(true));
    l.close_end(MANAGER).unwrap();
    let receipt = l.world_closed(WORLD).unwrap();
    assert_eq!(
        receipt,
        Receipt {
            world: WORLD,
            manager: MANAGER,
            serial: first,
            set: SET,
            list: LIST,
            init_observed: true,
            repeated: false,
            list_released: true,
            late_spawn: false,
            publication_overlap: false,
        }
    );
    assert!(receipt.complete());
    // The tombstone keeps the post-Wait seal after the close receipt.
    assert_eq!(l.submit(SET), Err(Violation::LateSubmit));
    // A freed list id may belong to another subsystem and is not attributed.
    assert_eq!(l.spawn(LIST), Ok(()));
    // Raw World/manager/list address reuse does not start an incarnation.
    assert_eq!(l.submit(SET), Err(Violation::LateSubmit));
    assert_eq!(l.world_closed(WORLD), Err(Violation::WorldNotReturned));
    // Only a real manager initialization returns the address to Open.
    let second = l.init_returned(MANAGER, 9).unwrap();
    assert!(second > first);
    assert_eq!(l.submit(SET), Ok(()));
    assert_eq!(l.spawn(9), Ok(()));
    assert_eq!(l.spawn(LIST), Ok(()));
}

#[test]
fn out_of_order_close_events_are_violations_not_receipts() {
    let mut l = Ledger::new();
    assert_eq!(l.wait_returned(MANAGER), Err(Violation::WaitOutOfOrder));
    l.init_returned(MANAGER, LIST).unwrap();
    l.close_begin(WORLD, MANAGER, LIST).unwrap();
    assert_eq!(
        l.close_begin(WORLD, MANAGER, LIST),
        Err(Violation::DuplicateClose)
    );
    assert_eq!(
        l.init_returned(MANAGER, LIST),
        Err(Violation::InitDuringClose)
    );
    assert_eq!(
        l.list_released(LIST, true, MANAGER),
        Err(Violation::ReleaseOutOfOrder)
    );
    assert_eq!(l.close_end(MANAGER), Err(Violation::CloseOutOfOrder));
    l.wait_returned(MANAGER).unwrap();
    assert_eq!(l.wait_returned(MANAGER), Err(Violation::WaitOutOfOrder));
    assert_eq!(
        l.list_released(LIST, false, 0),
        Err(Violation::ReleaseCaller)
    );
    assert_eq!(l.close_end(MANAGER), Err(Violation::ListNotReleased));
    assert_eq!(l.world_closed(WORLD), Err(Violation::WorldNotReturned));
    assert_eq!(l.list_released(LIST, true, MANAGER), Ok(true));
    assert_eq!(
        l.list_released(LIST, true, MANAGER),
        Err(Violation::DoubleRelease)
    );
    assert_eq!(l.world_closed(0), Err(Violation::WorldNotReturned));
    l.close_end(MANAGER).unwrap();
    // After Returned, a released id is no longer attributed to this manager.
    assert_eq!(l.list_released(LIST, true, MANAGER), Ok(false));
    assert_eq!(l.spawn(LIST), Ok(()));
    assert_eq!(l.submit(SET), Err(Violation::LateSubmit));
    assert!(l.world_closed(WORLD).unwrap().complete());
}

#[test]
fn late_submit_after_wait_is_rejected_and_late_spawn_makes_the_receipt_incomplete() {
    let mut l = Ledger::new();
    waited(&mut l);
    assert_eq!(l.submit(SET), Err(Violation::LateSubmit));
    assert_eq!(l.spawn(LIST), Ok(()));
    assert!(l.managers[0].late_spawn);
    // Other managers and lists remain transparent.
    assert_eq!(l.submit(SET + 0x4000), Ok(()));
    assert_eq!(l.spawn(LIST + 1), Ok(()));
    assert_eq!(l.spawn(0), Ok(()));
    assert_eq!(l.list_released(LIST + 1, true, 0x7000), Ok(false));
    assert_eq!(l.list_released(LIST, true, MANAGER), Ok(true));
    l.close_end(MANAGER).unwrap();
    assert!(!l.world_closed(WORLD).unwrap().complete());
}

#[test]
fn unobserved_init_and_repeated_close_never_become_a_fresh_world_certificate() {
    let mut l = Ledger::new();
    l.close_begin(WORLD, MANAGER, 0).unwrap();
    l.wait_returned(MANAGER).unwrap();
    l.close_end(MANAGER).unwrap();
    let first = l.world_closed(WORLD).unwrap();
    assert!(!first.init_observed && !first.repeated && !first.list_released);
    assert!(!first.complete());
    // A second shutdown of the same tombstone has no new incarnation.
    l.close_begin(0x1800, MANAGER, 0).unwrap();
    assert_eq!(l.submit(SET), Err(Violation::LateSubmit));
    l.wait_returned(MANAGER).unwrap();
    l.close_end(MANAGER).unwrap();
    let second = l.world_closed(0x1800).unwrap();
    assert!(second.repeated);
    assert!(!second.complete());
    assert_eq!(second.serial, first.serial);
    assert_eq!(l.submit(SET), Err(Violation::LateSubmit));
}

#[test]
fn shutdown_outside_the_tracked_world_close_is_recorded_only() {
    let mut l = Ledger::new();
    l.init_returned(MANAGER, LIST).unwrap();
    // 4A1150 reached by another caller frees the list outside close_begin.
    assert_eq!(l.list_released(LIST, true, MANAGER), Ok(false));
    assert_eq!(l.unowned, 1);
    assert!(l.last.is_none());
    assert_eq!(l.submit(SET), Ok(()));
    // The stale id is now another owner's id and is ignored.
    assert_eq!(l.list_released(LIST, false, 0), Ok(false));
    assert_eq!(l.spawn(LIST), Ok(()));
    let again = l.init_returned(MANAGER, 6).unwrap();
    assert!(again > 1);
    assert_eq!(l.list_released(6, false, 0), Ok(false));
}

#[test]
fn a_closing_list_wins_when_a_stale_open_record_has_the_same_id() {
    let mut l = Ledger::new();
    l.init_returned(0x8000, LIST).unwrap();
    l.init_returned(MANAGER, LIST).unwrap();
    l.close_begin(WORLD, MANAGER, LIST).unwrap();
    l.wait_returned(MANAGER).unwrap();
    assert_eq!(l.spawn(LIST), Ok(()));
    // The Waited record owns the id. The stale Open record is not attributed.
    assert!(!l.managers[0].late_spawn && l.managers[1].late_spawn);
    assert_eq!(l.list_released(LIST, true, MANAGER), Ok(true));
    l.close_end(MANAGER).unwrap();
    let receipt = l.world_closed(WORLD).unwrap();
    assert!(receipt.list_released && receipt.late_spawn && !receipt.complete());
}

#[test]
fn a_released_lease_is_not_attributed_to_the_old_manager_when_the_id_is_recycled() {
    let mut l = Ledger::new();
    waited(&mut l);
    assert_eq!(l.list_released(LIST, true, MANAGER), Ok(true));
    // Another domain recycles the id before 4A1150 has returned.
    assert_eq!(l.list_released(LIST, false, 0), Ok(false));
    assert_eq!(l.spawn(LIST), Ok(()));
    assert!(!l.managers[0].late_spawn);
    // The native owner identity still catches a second release.
    assert_eq!(
        l.list_released(LIST, true, MANAGER),
        Err(Violation::DoubleRelease)
    );
    l.close_end(MANAGER).unwrap();
    assert!(l.world_closed(WORLD).unwrap().complete());
}

#[test]
fn the_same_call_site_for_a_recycled_id_belongs_to_the_manager_in_rbx() {
    const OTHER: u64 = 0x6000;
    let mut l = Ledger::new();
    waited(&mut l);
    assert_eq!(l.list_released(LIST, true, MANAGER), Ok(true));
    l.init_returned(OTHER, LIST).unwrap();
    assert_eq!(l.list_released(LIST, true, OTHER), Ok(false));
    assert_eq!((l.managers[1].list, l.unowned), (0, 1));
    assert!(l.managers[0].released);
    l.close_end(MANAGER).unwrap();
    assert!(l.world_closed(WORLD).unwrap().complete());
}

#[test]
fn a_nested_shutdown_of_another_manager_never_touches_the_closing_lease() {
    const OTHER: u64 = 0x6000;
    let mut l = Ledger::new();
    l.init_returned(OTHER, 9).unwrap();
    waited(&mut l);
    assert_eq!(l.list_released(9, true, OTHER), Ok(false));
    assert_eq!((l.unowned, l.managers[0].list), (1, 0));
    assert!(!l.managers[1].released);
    // An unknown native receiver is neither attributed nor credited.
    assert_eq!(l.list_released(LIST, true, 0x7000), Ok(false));
    assert_eq!(l.close_end(MANAGER), Err(Violation::ListNotReleased));
    assert_eq!(l.list_released(LIST, true, MANAGER), Ok(true));
    l.close_end(MANAGER).unwrap();
    assert!(l.world_closed(WORLD).unwrap().complete());
}

#[test]
fn the_closing_manager_releasing_a_different_id_is_a_violation_without_credit() {
    let mut l = Ledger::new();
    waited(&mut l);
    assert_eq!(
        l.list_released(LIST + 1, true, MANAGER),
        Err(Violation::ReleaseList)
    );
    assert!(!l.managers[0].released);
    assert_eq!(l.close_end(MANAGER), Err(Violation::ListNotReleased));
}

#[test]
fn terminal_exception_is_sticky_and_blocks_new_incarnations() {
    let mut l = Ledger::new();
    l.init_returned(MANAGER, LIST).unwrap();
    l.unresolved = true;
    assert_eq!(l.init_returned(0x9000, LIST), Err(Violation::Unwound));
    assert_eq!(l.close_begin(WORLD, MANAGER, LIST), Err(Violation::Unwound));
}

fn child(name: &str, abort: bool, marker: &str) -> bool {
    const ENV: &str = "CIV6_CLOSURE_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let output = crate::windows::test_child_output::run(
        std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env(ENV, "1")
            .args([
                "--exact",
                &format!(
                    "windows::root_observation::native_frames::scheduler::closure::tests::{name}"
                ),
                "--nocapture",
            ]),
        Duration::from_secs(20),
    )
    .unwrap();
    let status = output.status;
    let out = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if abort {
        assert!(!status.success(), "expected abort: {out}\n{stderr}");
        assert_eq!(
            status.code().map(|code| code as u32),
            Some(0xc0000409),
            "wrong termination before the owned fatal boundary: {out}\n{stderr}"
        );
        assert!(
            !stderr.contains("panicked"),
            "fixture panic is not the expected fatal rejection: {stderr}"
        );
        assert!(out.contains(marker), "missing {marker}: {out}\n{stderr}");
    } else {
        assert!(status.success(), "child failed: {out}\n{stderr}");
    }
    false
}

global_asm!(
    ".globl closure_free_tail",
    "closure_free_tail:",
    "add rsp,0x40",
    "pop rbp",
    "mov eax,0x7777",
    "ret",
    ".globl closure_free_driver",
    ".seh_proc closure_free_driver",
    "closure_free_driver:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    // 4A1150 keeps its manager in RBX across the FreeJobList call at 4A1209.
    "mov rbx,rdx",
    "call {free}",
    ".globl closure_free_return",
    "closure_free_return:",
    "add rsp,0x20",
    "pop rbx",
    "ret",
    ".seh_endproc",
    ".globl closure_free_other_driver",
    ".seh_proc closure_free_other_driver",
    "closure_free_other_driver:",
    "sub rsp,0x28",
    ".seh_stackalloc 0x28",
    ".seh_endprologue",
    "call {free}",
    ".globl closure_free_other_return",
    "closure_free_other_return:",
    "lea rsp,[rsp+0x28]",
    "ret",
    ".seh_endproc",
    ".globl closure_home_tail",
    "closure_home_tail:",
    "mov eax,0x5151",
    "ret",
    ".globl closure_spawn_tail",
    "closure_spawn_tail:",
    "add rsp,0x30",
    "pop rbx",
    "mov eax,0x6262",
    "ret",
    ".globl closure_add_driver",
    ".seh_proc closure_add_driver",
    "closure_add_driver:",
    "sub rsp,0x28",
    ".seh_stackalloc 0x28",
    ".seh_endprologue",
    "call {add}",
    "lea rsp,[rsp+0x28]",
    "ret",
    ".seh_endproc",
    ".globl closure_push_driver",
    ".seh_proc closure_push_driver",
    "closure_push_driver:",
    "sub rsp,0x28",
    ".seh_stackalloc 0x28",
    ".seh_endprologue",
    "call {push}",
    "lea rsp,[rsp+0x28]",
    "ret",
    ".seh_endproc",
    ".globl closure_spawn_driver",
    ".seh_proc closure_spawn_driver",
    "closure_spawn_driver:",
    "sub rsp,0x28",
    ".seh_stackalloc 0x28",
    ".seh_endprologue",
    "call {spawn}",
    "lea rsp,[rsp+0x28]",
    "ret",
    ".seh_endproc",
    ".globl closure_init_abi_driver",
    ".seh_proc closure_init_abi_driver",
    "closure_init_abi_driver:",
    "push rbx",
    ".seh_pushreg rbx",
    "push rbp",
    ".seh_pushreg rbp",
    "push rsi",
    ".seh_pushreg rsi",
    "push rdi",
    ".seh_pushreg rdi",
    "push r12",
    ".seh_pushreg r12",
    "push r13",
    ".seh_pushreg r13",
    "sub rsp,0x28",
    ".seh_stackalloc 0x28",
    ".seh_endprologue",
    "mov r13,r8",
    "mov rax,rcx",
    "mov rcx,rdx",
    "mov ebx,0x11",
    "mov ebp,0x22",
    "mov esi,0x33",
    "mov edi,0x44",
    "mov r12d,0x55",
    "call rax",
    "mov [r13],rbx",
    "mov [r13+8],rbp",
    "mov [r13+16],rsi",
    "mov [r13+24],rdi",
    "mov [r13+32],r12",
    "add rsp,0x28",
    "pop r13",
    "pop r12",
    "pop rdi",
    "pop rsi",
    "pop rbp",
    "pop rbx",
    "ret",
    ".seh_endproc",
    free = sym scheduler_free_gateway,
    add = sym scheduler_add_gateway,
    push = sym scheduler_push_gateway,
    spawn = sym scheduler_spawn_gateway,
);
unsafe extern "system" {
    fn closure_free_tail();
    fn closure_free_driver(list: u64, receiver: u64, c: u64, d: u64) -> u64;
    fn closure_free_return();
    fn closure_free_other_driver(list: u64, b: u64, c: u64, d: u64) -> u64;
    fn closure_home_tail();
    fn closure_spawn_tail();
    fn closure_add_driver(payload: u64, set: u64, c: u64, d: u64) -> u64;
    fn closure_push_driver(list: u64, payload: u64, set: u64, d: u64) -> u64;
    fn closure_spawn_driver(list: u64, b: u64, c: u64, d: u64) -> u64;
    fn closure_init_abi_driver(target: usize, manager: *mut u64, out: *mut u64) -> u64;
}
fn last_receipt() -> Option<Receipt> {
    LEDGER.lock().unwrap().last
}
fn managers() -> Vec<Manager> {
    LEDGER.lock().unwrap().managers.clone()
}
fn route(index: usize, tail: unsafe extern "system" fn()) {
    NATIVE_SCHEDULER_CONTINUE[index].store(tail as usize as u64, Ordering::Release);
}

#[test]
fn manager_release_return_is_credited_through_the_real_free_job_list_gateway() {
    if !child(
        "manager_release_return_is_credited_through_the_real_free_job_list_gateway",
        false,
        "",
    ) {
        return;
    }
    assert!(FRAMES.get().is_none());
    bind_core((closure_free_return as *const () as usize as u64) - RELEASE_RETURN);
    route(7, closure_free_tail);
    init_returned(MANAGER, LIST).unwrap();
    close_begin(WORLD, MANAGER, LIST).unwrap();
    wait_returned(MANAGER).unwrap();
    unsafe { SetLastError(0x880123) };
    assert_eq!(
        unsafe { closure_free_driver(u64::from(LIST), MANAGER, 0, 0) },
        0x7777
    );
    assert_eq!(unsafe { GetLastError() }, 0x880123);
    close_end(MANAGER).unwrap();
    let receipt = world_closed(WORLD).unwrap();
    assert!(receipt.list_released && receipt.list == LIST);
    assert_eq!(last_receipt(), Some(receipt));
    assert!(!FAILED.load(Ordering::Acquire));
}

#[test]
fn free_job_list_from_another_return_address_aborts_while_the_manager_owns_the_id() {
    if !child(
        "free_job_list_from_another_return_address_aborts_while_the_manager_owns_the_id",
        true,
        "CLOSURE_VIOLATION=ReleaseCaller",
    ) {
        return;
    }
    bind_core((closure_free_return as *const () as usize as u64) - RELEASE_RETURN);
    route(7, closure_free_tail);
    init_returned(MANAGER, LIST).unwrap();
    close_begin(WORLD, MANAGER, LIST).unwrap();
    wait_returned(MANAGER).unwrap();
    unsafe { closure_free_other_driver(u64::from(LIST), 0, 0, 0) };
    unreachable!();
}

#[test]
fn foreign_reuse_of_a_released_list_id_is_transparent_and_keeps_the_receipt_complete() {
    if !child(
        "foreign_reuse_of_a_released_list_id_is_transparent_and_keeps_the_receipt_complete",
        false,
        "",
    ) {
        return;
    }
    bind_core((closure_free_return as *const () as usize as u64) - RELEASE_RETURN);
    route(7, closure_free_tail);
    route(2, closure_spawn_tail);
    init_returned(MANAGER, LIST).unwrap();
    close_begin(WORLD, MANAGER, LIST).unwrap();
    wait_returned(MANAGER).unwrap();
    assert_eq!(
        unsafe { closure_free_driver(u64::from(LIST), MANAGER, 0, 0) },
        0x7777
    );
    assert!(managers()[0].released);
    // The id is free again. Another Host domain may take it, spawn on it and
    // release it from its own call site before 4A1150 has returned.
    assert_eq!(
        unsafe { closure_spawn_driver(u64::from(LIST), 0, 0, 0) },
        0x6262
    );
    assert_eq!(
        unsafe { closure_free_other_driver(u64::from(LIST), 0, 0, 0) },
        0x7777
    );
    assert!(!managers()[0].late_spawn);
    assert!(!FAILED.load(Ordering::Acquire));
    close_end(MANAGER).unwrap();
    let receipt = world_closed(WORLD).unwrap();
    assert!(receipt.list_released && !receipt.late_spawn && receipt.complete());
}

#[test]
fn the_same_native_return_address_for_another_manager_is_not_a_double_release() {
    if !child(
        "the_same_native_return_address_for_another_manager_is_not_a_double_release",
        false,
        "",
    ) {
        return;
    }
    const OTHER: u64 = 0x6000;
    bind_core((closure_free_return as *const () as usize as u64) - RELEASE_RETURN);
    route(7, closure_free_tail);
    init_returned(MANAGER, LIST).unwrap();
    close_begin(WORLD, MANAGER, LIST).unwrap();
    wait_returned(MANAGER).unwrap();
    assert_eq!(
        unsafe { closure_free_driver(u64::from(LIST), MANAGER, 0, 0) },
        0x7777
    );
    // Another manager is initialized with the recycled id and is shut down
    // outside this World close through the very same native call site.
    init_returned(OTHER, LIST).unwrap();
    assert_eq!(
        unsafe { closure_free_driver(u64::from(LIST), OTHER, 0, 0) },
        0x7777
    );
    let all = managers();
    assert!(all[0].released && all[0].phase == Phase::Waited);
    assert_eq!((all[1].manager, all[1].list), (OTHER, 0));
    assert!(!FAILED.load(Ordering::Acquire));
    close_end(MANAGER).unwrap();
    assert!(world_closed(WORLD).unwrap().complete());
}

#[test]
fn a_second_owned_release_by_the_same_manager_still_aborts() {
    if !child(
        "a_second_owned_release_by_the_same_manager_still_aborts",
        true,
        "CLOSURE_VIOLATION=DoubleRelease",
    ) {
        return;
    }
    bind_core((closure_free_return as *const () as usize as u64) - RELEASE_RETURN);
    route(7, closure_free_tail);
    init_returned(MANAGER, LIST).unwrap();
    close_begin(WORLD, MANAGER, LIST).unwrap();
    wait_returned(MANAGER).unwrap();
    assert_eq!(
        unsafe { closure_free_driver(u64::from(LIST), MANAGER, 0, 0) },
        0x7777
    );
    unsafe { closure_free_driver(u64::from(LIST), MANAGER, 0, 0) };
    unreachable!();
}

#[test]
fn add_push_and_spawn_gateways_count_during_close_and_stay_transparent_otherwise() {
    if !child(
        "add_push_and_spawn_gateways_count_during_close_and_stay_transparent_otherwise",
        false,
        "",
    ) {
        return;
    }
    route(0, closure_home_tail);
    route(1, closure_home_tail);
    route(2, closure_spawn_tail);
    init_returned(MANAGER, LIST).unwrap();
    // Open managers are transparent and uncounted.
    unsafe { SetLastError(0x1234) };
    assert_eq!(unsafe { closure_push_driver(5, 0x100, SET, 0) }, 0x5151);
    assert_eq!(managers()[0].closing_submits, 0);
    close_begin(WORLD, MANAGER, LIST).unwrap();
    assert_eq!(unsafe { closure_add_driver(0x100, SET, 0, 0) }, 0x5151);
    assert_eq!(unsafe { closure_push_driver(5, 0x100, SET, 0) }, 0x5151);
    assert_eq!(unsafe { closure_spawn_driver(5, 0, 0, 0) }, 0x6262);
    assert_eq!(unsafe { GetLastError() }, 0x1234);
    assert_eq!(managers()[0].closing_submits, 3);
    // Unrelated sets and lists do not touch the ledger.
    assert_eq!(
        unsafe { closure_push_driver(9, 0x100, SET + 0x4000, 0) },
        0x5151
    );
    assert_eq!(unsafe { closure_spawn_driver(9, 0, 0, 0) }, 0x6262);
    assert_eq!(managers()[0].closing_submits, 3);
    assert!(!FAILED.load(Ordering::Acquire));
}

#[test]
fn push_after_wait_aborts_the_owned_process() {
    if !child(
        "push_after_wait_aborts_the_owned_process",
        true,
        "CLOSURE_VIOLATION=LateSubmit",
    ) {
        return;
    }
    route(1, closure_home_tail);
    init_returned(MANAGER, LIST).unwrap();
    close_begin(WORLD, MANAGER, LIST).unwrap();
    wait_returned(MANAGER).unwrap();
    unsafe { closure_push_driver(5, 0x100, SET, 0) };
    unreachable!();
}

#[test]
fn spawn_after_wait_marks_the_receipt_incomplete_without_aborting() {
    if !child(
        "spawn_after_wait_marks_the_receipt_incomplete_without_aborting",
        false,
        "",
    ) {
        return;
    }
    route(2, closure_spawn_tail);
    init_returned(MANAGER, LIST).unwrap();
    close_begin(WORLD, MANAGER, LIST).unwrap();
    wait_returned(MANAGER).unwrap();
    assert_eq!(
        unsafe { closure_spawn_driver(u64::from(LIST), 0, 0, 0) },
        0x6262
    );
    assert!(managers()[0].late_spawn);
    assert!(!FAILED.load(Ordering::Acquire));
    assert_eq!(
        LEDGER.lock().unwrap().list_released(LIST, true, MANAGER),
        Ok(true)
    );
    close_end(MANAGER).unwrap();
    let receipt = world_closed(WORLD).unwrap();
    assert!(receipt.late_spawn && receipt.list_released);
    assert!(!receipt.complete());
    assert!(!FAILED.load(Ordering::Acquire));
}

fn synthetic_core() -> usize {
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0x4b0000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    let mut body = ENTRY.to_vec();
    // push r12 completes; count the original body; create list id 7; epilogue.
    body.push(0x54);
    body.extend_from_slice(&[0xf0, 0x48, 0xff, 0x81, 0x00, 0x06, 0, 0]);
    body.extend_from_slice(&[0xc7, 0x81, 0x10, 0x04, 0, 0, 7, 0, 0, 0]);
    body.extend_from_slice(&[0x41, 0x5c, 0x5f, 0x5e, 0x5b, 0x5d]);
    body.extend_from_slice(&[0xb8, 0x34, 0x12, 0, 0, 0xc3]);
    unsafe { ptr::copy_nonoverlapping(body.as_ptr(), (base + INIT) as *mut u8, body.len()) };
    let mut old = 0;
    assert_ne!(
        unsafe {
            VirtualProtect(
                (base + (INIT & !0xfff)) as _,
                0x1000,
                PAGE_EXECUTE_READ,
                &mut old,
            )
        },
        0
    );
    base
}

#[test]
fn init_gateway_runs_original_once_publishes_new_incarnations_and_restores() {
    if !child(
        "init_gateway_runs_original_once_publishes_new_incarnations_and_restores",
        false,
        "",
    ) {
        return;
    }
    let base = synthetic_core();
    unsafe { install_at(base) }.unwrap();
    assert_eq!(
        unsafe { install_at(base) },
        Err("closure init hook already installed")
    );
    let mut manager = vec![0u64; 0x100];
    let pointer = manager.as_mut_ptr();
    let address = pointer as u64;
    let mut saved = [0u64; 5];
    unsafe { SetLastError(0x123456) };
    assert_eq!(
        unsafe { closure_init_abi_driver(base + INIT, pointer, saved.as_mut_ptr()) },
        0x1234
    );
    assert_eq!(unsafe { GetLastError() }, 0x123456);
    // Original body once; callee-saved registers are the caller's values.
    assert_eq!(manager[0xc0], 1);
    assert_eq!(saved, [0x11, 0x22, 0x33, 0x44, 0x55]);
    let first = managers();
    assert_eq!(first.len(), 1);
    assert_eq!(
        (
            first[0].manager,
            first[0].list,
            first[0].phase,
            first[0].init_observed
        ),
        (address, 7, Phase::Open, true)
    );
    // A second normal init at the same address is a new incarnation.
    unsafe { closure_init_abi_driver(base + INIT, pointer, saved.as_mut_ptr()) };
    assert_eq!(manager[0xc0], 2);
    assert!(managers()[0].serial > first[0].serial);
    // Close it and prove that only another real init reopens the tombstone.
    close_begin(WORLD, address, 7).unwrap();
    wait_returned(address).unwrap();
    // The gateway frees nothing here; this fixture releases through the ledger.
    LEDGER
        .lock()
        .unwrap()
        .list_released(7, true, address)
        .unwrap();
    close_end(address).unwrap();
    world_closed(WORLD).unwrap();
    assert_eq!(submit(address + 0x400), Err(Violation::LateSubmit));
    unsafe { closure_init_abi_driver(base + INIT, pointer, saved.as_mut_ptr()) };
    assert_eq!(submit(address + 0x400), Ok(()));
    unsafe { restore() }.unwrap();
    assert_eq!(copy_current::<8>((base + INIT) as u64), Some(ENTRY));
    assert!(!FAILED.load(Ordering::Acquire));
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}

#[test]
fn init_during_a_tracked_close_aborts_the_owned_process() {
    if !child(
        "init_during_a_tracked_close_aborts_the_owned_process",
        true,
        "CLOSURE_VIOLATION=InitDuringClose",
    ) {
        return;
    }
    let base = synthetic_core();
    unsafe { install_at(base) }.unwrap();
    let mut manager = vec![0u64; 0x100];
    let pointer = manager.as_mut_ptr();
    close_begin(WORLD, pointer as u64, 0).unwrap();
    let mut saved = [0u64; 5];
    unsafe { closure_init_abi_driver(base + INIT, pointer, saved.as_mut_ptr()) };
    unreachable!();
}

#[test]
fn init_shim_unwinds_exactly_the_two_displaced_pushes() {
    let mut stack = [0u64; 32];
    let entry = unsafe { stack.as_mut_ptr().add(16) } as u64;
    unsafe {
        *(entry as *mut u64) = 0xabcdef;
        *((entry - 8) as *mut u64) = 0xb2;
        *((entry - 16) as *mut u64) = 0xb1;
    }
    let mut context: CONTEXT = unsafe { std::mem::zeroed() };
    context.Rip = native_closure_init_original as *const () as usize as u64 + 5;
    context.Rsp = entry - 16;
    let mut base = 0;
    let row = unsafe { RtlLookupFunctionEntry(context.Rip, &mut base, null_mut()) };
    assert!(!row.is_null());
    let mut data = null_mut();
    let mut frame = 0;
    unsafe {
        RtlVirtualUnwind(
            0,
            base,
            context.Rip,
            row,
            &mut context,
            &mut data,
            &mut frame,
            null_mut(),
        );
    }
    assert_eq!(context.Rsp, entry + 8);
    assert_eq!(context.Rip, 0xabcdef);
    assert_eq!((context.Rbx, context.Rbp), (0xb1, 0xb2));
}

#[test]
fn terminal_exception_through_init_is_unresolved_and_creates_no_incarnation() {
    if !child(
        "terminal_exception_through_init_is_unresolved_and_creates_no_incarnation",
        false,
        "",
    ) {
        return;
    }
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.cookie = COOKIE;
    raw.invocation.manager = MANAGER;
    finish(&mut raw, true);
    assert!(FAILED.load(Ordering::Acquire));
    assert!(managers().is_empty());
    assert_eq!(init_returned(MANAGER, LIST), Err(Violation::Unwound));
}

fn file_offset(file: &mut std::fs::File, rva: u32) -> u64 {
    let mut word = [0; 4];
    file.seek(SeekFrom::Start(0x3c)).unwrap();
    file.read_exact(&mut word).unwrap();
    let nt = u32::from_le_bytes(word) as u64;
    let mut short = [0; 2];
    file.seek(SeekFrom::Start(nt + 6)).unwrap();
    file.read_exact(&mut short).unwrap();
    let sections = u16::from_le_bytes(short);
    file.seek(SeekFrom::Start(nt + 20)).unwrap();
    file.read_exact(&mut short).unwrap();
    let optional = u16::from_le_bytes(short) as u64;
    for index in 0..sections {
        file.seek(SeekFrom::Start(nt + 24 + optional + u64::from(index) * 40))
            .unwrap();
        let mut row = [0; 40];
        file.read_exact(&mut row).unwrap();
        let start = u32::from_le_bytes(row[12..16].try_into().unwrap());
        let len = u32::from_le_bytes(row[16..20].try_into().unwrap());
        if rva >= start && rva - start < len {
            return u64::from(u32::from_le_bytes(row[20..24].try_into().unwrap()))
                + u64::from(rva - start);
        }
    }
    panic!("missing primary RVA");
}

#[test]
#[ignore = "explicit licensed local primary-image acceptance"]
fn init_frame_matches_the_primary_core_and_rejects_mutations() {
    let mut file = pin(
        GAMECORE,
        "092abfbb2176f8df04b5212f819dc269dd83aea866ff05aaf489904a1ff7c41f",
    )
    .unwrap();
    assert_eq!(fixed::FRAMES.len(), 1);
    for frame in fixed::FRAMES {
        assert_eq!((frame.begin as usize, ENTRY.len()), (INIT, 8));
        let location = file_offset(&mut file, frame.begin);
        file.seek(SeekFrom::Start(location)).unwrap();
        let mut code = vec![0; (frame.end - frame.begin) as usize];
        file.read_exact(&mut code).unwrap();
        let expected: Vec<_> = (0..frame.specimen.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&frame.specimen[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(code, expected);
        assert_eq!(&code[..8], &ENTRY);
        assert_eq!(format!("{:x}", Sha256::digest(&code)), frame.sha);
        let last = code.len() - 1;
        code[last] ^= 1;
        assert_ne!(format!("{:x}", Sha256::digest(&code)), frame.sha);
        for part in frame.parts {
            let at = file_offset(&mut file, part.unwind);
            file.seek(SeekFrom::Start(at)).unwrap();
            let wanted: Vec<_> = (0..part.hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&part.hex[i..i + 2], 16).unwrap())
                .collect();
            let mut data = vec![0; wanted.len()];
            file.read_exact(&mut data).unwrap();
            assert_eq!(data, wanted);
            // The last two unwind slots are the displaced push rbx/push rbp.
            assert_eq!(&data[4 + 2 * 11..4 + 2 * 13], &[0x05, 0x30, 0x04, 0x50]);
        }
    }
    // The only Core release site is the displaced-call witness for 4A1209.
    let call = file_offset(&mut file, 0x4a1209);
    file.seek(SeekFrom::Start(call)).unwrap();
    let mut bytes = [0; 5];
    file.read_exact(&mut bytes).unwrap();
    let target = 0x4a1209i64 + 5 + i64::from(i32::from_le_bytes(bytes[1..].try_into().unwrap()));
    assert_eq!((bytes[0], target), (0xe8, 0x9921d0));
    assert_eq!(0x4a1209 + 5, RELEASE_RETURN);
    // Owner identity of that release: 4A1150 moves its manager from RCX to RBX
    // at 4A1175 and from then on only reads through RBX up to the call, and the
    // Core thunk restores the caller's RBX before its tail jump to the Host.
    // These are exact image bytes, not a decoder result.
    let manager_to_rbx = file_offset(&mut file, 0x4a1175);
    let thunk = file_offset(&mut file, 0x9921d0);
    for (at, hex) in [
        (
            manager_to_rbx,
            "488bd933f648897424284881c1000400000f28442420660f7f442420e82a114f004088b3ac3800004038b3ad380000740d488b0dd3986e00488b01ff5040488d8b800100004088b3ad380000e84a460800488d8bc8020000e8ae27fbff488d8b68030000e8a227fbff488d8b90030000e89627fbff488d8b40030000e88a27fbff488d8bf0020000e8ae27fbff488b8b10040000e8c20f4f00",
        ),
        (
            thunk,
            "40534883ec20488b05c3e41f00488bd94885c07513488d0d7cb91200e85f130000488905a8e41f00488bcb4883c4205b48ffe0",
        ),
    ] {
        let wanted: Vec<_> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let mut data = vec![0; wanted.len()];
        file.seek(SeekFrom::Start(at)).unwrap();
        file.read_exact(&mut data).unwrap();
        assert_eq!(data, wanted);
    }
    assert_eq!(0x4a1175 + 0x99, 0x4a120e);
    assert_eq!(0x9921d0 + 0x33, 0x992203);
}
