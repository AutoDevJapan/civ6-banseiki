// Included in the existing owned-OS test module: every case has its own child.
use super::submission::observed as submission_observed;

fn submission_observed_fixture(core: usize, exe: usize) {
    submission::LEGACY_FIXTURE.store(false, Ordering::Release);
    submission::PROFILE.store(true, Ordering::Release);
    bind_fixture(core, 0, 0);
    h(8).store(exe as u64, Ordering::Release);
    h(15).store(63, Ordering::Release);
}
extern "system" fn submission_observed_prepare(input: u64, kind: u64, rsp: u64) {
    let error = unsafe { GetLastError() };
    submission_observed::fixture_parent(kind, input, rsp);
    unsafe { SetLastError(error) };
}
extern "system" fn submission_observed_cleanup() {
    submission_observed::fixture_parent_end();
}
global_asm!(
    ".globl submission_observed_probe_driver",".seh_proc submission_observed_probe_driver",
    "submission_observed_probe_driver:","push rbx",".seh_pushreg rbx",
    "sub rsp,0x40",".seh_stackalloc 0x40",".seh_endprologue",
    "mov rbx,rcx","mov [rsp+0x20],rdx","mov [rsp+0x28],r8",
    "mov rdx,r8","lea r8,[rsp-8]","call {prepare}",
    "mov rcx,rbx","mov rax,0x11223344","mov rdx,0x5566","mov r8,0x7788",
    "lea r9,[rbx+0x400]","mov r10,0xbbcc","mov r11,0xddee",
    "pcmpeqb xmm0,xmm0","pcmpeqb xmm1,xmm1","pcmpeqb xmm2,xmm2",
    "pcmpeqb xmm3,xmm3","pcmpeqb xmm4,xmm4","pcmpeqb xmm5,xmm5",
    "cmp rsp,rsp","stc","pushfq","pop qword ptr [rbx+0x98]",
    "call qword ptr [rsp+0x20]",".globl submission_observed_probe_return",
    "submission_observed_probe_return:",
    "pushfq","pop qword ptr [rbx+0x280]",
    "mov [rbx+0x200],rax","mov [rbx+0x208],rdx","mov [rbx+0x210],r8",
    "mov [rbx+0x218],r9","mov [rbx+0x220],r10","mov [rbx+0x228],r11","mov [rbx+0x230],rcx",
    "movdqu [rbx+0x300],xmm0","movdqu [rbx+0x310],xmm1","movdqu [rbx+0x320],xmm2",
    "movdqu [rbx+0x330],xmm3","movdqu [rbx+0x340],xmm4","movdqu [rbx+0x350],xmm5",
    "call {cleanup}","lea rsp,[rsp+0x40]","pop rbx","ret",".seh_endproc",
    // The real MOV-only producer shim is used. The fixture samples flags
    // before its later stack allocation, not from the recorder's Saved copy.
    ".globl submission_observed_probe_producer",".seh_proc submission_observed_probe_producer",
    "submission_observed_probe_producer:","mov [rsp+0x18],rsi",".seh_savereg rsi,0x18",
    "root_probe_inputs","push rbx",".seh_pushreg rbx","sub rsp,0x20",".seh_stackalloc 0x20",
    ".seh_endprologue","root_probe_outputs","mov rbx,[rsp+0x20]","mov rsi,[rsp+0x40]",
    "lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".globl submission_observed_probe_host",".seh_proc submission_observed_probe_host",
    "submission_observed_probe_host:",".byte 0x40,0x53",".seh_pushreg rbx",
    "sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "root_probe_inputs","root_probe_outputs","mov rbx,[rsp+0x20]",
    "lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".globl submission_observed_probe_core",".seh_proc submission_observed_probe_core",
    "submission_observed_probe_core:",".byte 0x40,0x57",".seh_pushreg rdi",
    "sub rsp,0x20",".seh_stackalloc 0x20","mov [rsp+0x10],rbx",".seh_savereg rbx,0x10",
    ".seh_endprologue","root_probe_inputs","root_probe_outputs",
    "mov rbx,[rsp+0x10]","mov rdi,[rsp+0x20]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".globl submission_observed_probe_push_host",".seh_proc submission_observed_probe_push_host",
    "submission_observed_probe_push_host:","sub rsp,0x38",".seh_stackalloc 0x38",
    "movups xmm0,[r9]","mov [rsp+0x30],rbx",".seh_savereg rbx,0x30",
    ".seh_endprologue","root_probe_inputs","root_probe_outputs",
    "mov rbx,[rsp+0x30]","lea rsp,[rsp+0x38]","ret",".seh_endproc",
    prepare=sym submission_observed_prepare,cleanup=sym submission_observed_cleanup,
);
unsafe extern "system" {
    fn submission_observed_probe_driver(data: *mut u8, target: usize, kind: u64);
    fn submission_observed_probe_return();
    fn submission_observed_probe_producer();
    fn submission_observed_probe_host();
    fn submission_observed_probe_core();
    fn submission_observed_probe_push_host();
}
fn submission_observed_seed(data: &mut [u8], kind: u64, core: u64, exe: u64) {
    let p = data.as_mut_ptr() as u64;
    match kind {
        7 => {
            data[..8].copy_from_slice(&(exe + 0x106b3c8).to_le_bytes());
            data[8..16].copy_from_slice(&(p + 0xd0).to_le_bytes());
            data[0xd0..0xd8].copy_from_slice(&(core + 0xa06648).to_le_bytes());
            submission_observed::fixture_birth(p + 0xd0, p);
        }
        8 => {
            data[..8].copy_from_slice(&(core + 0xa06648).to_le_bytes());
            submission_observed::fixture_birth(p, 1);
        }
        9 => submission_observed::fixture_birth(0x5566, 0),
        _ => {}
    }
}
#[test]
fn submission_owned_os_observed_four_gateways_real_shims_state_and_normal_return() {
    if !child("submission_owned_os_observed_four_gateways_real_shims_state_and_normal_return") {
        return;
    }
    let ret = submission_observed_probe_return as *const () as u64;
    let core = ret - 0x992267;
    let exe = ret - 0x985ef4;
    submission_observed_fixture(core as usize, exe as usize);
    for (i, body) in [
        submission_observed_probe_producer as *const () as usize,
        submission_observed_probe_host as *const () as usize,
        submission_observed_probe_core as *const () as usize,
        submission_observed_probe_push_host as *const () as usize,
    ]
    .into_iter()
    .enumerate()
    {
        let kind = [6, 7, 8, 9][i];
        submission_observed::fixture_continue(i, body + [5, 6, 6, 8][i]);
        let mut plain = vec![0u8; 0x500];
        let mut hooked = vec![0u8; 0x500];
        submission_observed_seed(&mut plain, kind, core, exe);
        unsafe {
            SetLastError(0x1234);
            submission_observed_probe_driver(plain.as_mut_ptr(), body, kind);
        }
        submission_observed_seed(&mut hooked, kind, core, exe);
        let first = h(10).load(Ordering::Acquire) as usize;
        let extra_before = submission_observed::fixture_extra_next();
        unsafe {
            SetLastError(0x1234);
            submission_observed_probe_driver(
                hooked.as_mut_ptr(),
                submission_observed::fixture_gateways()[i],
                kind,
            );
        }
        assert_eq!(unsafe { GetLastError() }, 0x880);
        assert_eq!(u64_at(&hooked, 0x80), 1, "original body once, gateway {i}");
        assert_eq!(u64_at(&hooked, 0x90), 0x1234);
        assert_eq!(u64_at(&hooked, 0), 0x11223344);
        assert_eq!(&hooked[..24], &plain[..24]);
        assert_eq!(&hooked[32..48], &plain[32..48]);
        assert_eq!(&hooked[0x100..0x160], &plain[0x100..0x160]);
        assert_eq!(&hooked[0x200..0x238], &plain[0x200..0x238]);
        assert_eq!(&hooked[0x300..0x360], &plain[0x300..0x360]);
        assert_eq!(u64_at(&hooked, 0x280), u64_at(&hooked, 0x88));
        if i == 0 {
            assert_eq!(u64_at(&hooked, 56), u64_at(&hooked, 0x98));
        }
        if i == 0 {
            assert_eq!(h(10).load(Ordering::Acquire), first as u64);
            assert_eq!(
                h(20).load(Ordering::Acquire),
                1,
                "no-submit aggregate; no rows"
            );
        } else {
            assert_eq!(
                h(10).load(Ordering::Acquire),
                first as u64,
                "real entry accepted"
            );
            let recorded = submission_observed::fixture_last_entry();
            let expected = if i == 2 {
                civ6_bootstrap_rs::protocol::SUBMISSION_ROWS + extra_before
            } else {
                first - 12 + if i == 1 { 8 } else { 4 }
            };
            assert_eq!(
                recorded, expected,
                "static role pair inside its existing reservation"
            );
            assert_eq!(
                submission::TRACE.rows[recorded][1].load(Ordering::Acquire),
                kind
            );
            assert_eq!(
                submission::TRACE.rows[recorded + 1][2].load(Ordering::Acquire),
                2
            );
            assert_eq!(
                submission::TRACE.rows[recorded + 1][34].load(Ordering::Acquire),
                0x3344
            );
        }
        assert_eq!(h(12).load(Ordering::Acquire), 0);
    }
    assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    let frozen = submission_observed::fixture_bytes();
    // No frame priming occurs after close; all four original shims still run.
    for (i, kind) in [6, 7, 8, 9].into_iter().enumerate() {
        let mut late = vec![0u8; 0x500];
        unsafe {
            submission_observed_probe_driver(
                late.as_mut_ptr(),
                submission_observed::fixture_gateways()[i],
                kind,
            );
        }
        assert_eq!(u64_at(&late, 0x80), 1);
        assert_eq!(submission_observed::fixture_bytes(), frozen);
    }
}
#[test]
fn submission_owned_os_observed_stop_freezes_exclusions_lookup_failure_and_book_poison() {
    if !child("submission_owned_os_observed_stop_freezes_exclusions_lookup_failure_and_book_poison")
    {
        return;
    }
    let ret = submission_observed_probe_return as *const () as u64;
    submission_observed_fixture((ret - 0x992267) as usize, (ret - 0x985ef4) as usize);
    for (i, body) in [
        submission_observed_probe_producer as *const () as usize,
        submission_observed_probe_host as *const () as usize,
        submission_observed_probe_core as *const () as usize,
        submission_observed_probe_push_host as *const () as usize,
    ]
    .into_iter()
    .enumerate()
    {
        submission_observed::fixture_continue(i, body + [5, 6, 6, 8][i]);
    }
    let mut native = vec![0u8; 0x500];
    let other_p = Box::new([ret - 0x992267 + 0xa06650]);
    native[..8].copy_from_slice(&(ret - 0x985ef4 + 0x106b3c8).to_le_bytes());
    native[8..16].copy_from_slice(&(other_p.as_ptr() as u64).to_le_bytes());
    // A non-Governor real call is excluded while open and releases its flight.
    unsafe {
        submission_observed_probe_driver(
            native.as_mut_ptr(),
            submission_observed::fixture_gateways()[1],
            0,
        );
    }
    assert_eq!(h(30).load(Ordering::Acquire), 1);
    assert_eq!(h(12).load(Ordering::Acquire), 0);
    submission_observed::fixture_poison_book();
    unsafe {
        submission_observed_probe_driver(
            native.as_mut_ptr(),
            submission_observed::fixture_gateways()[1],
            0,
        );
    }
    assert!(
        h(14).load(Ordering::Acquire) > 0,
        "panic becomes unknown inside admission"
    );
    assert_eq!(h(12).load(Ordering::Acquire), 0);
    submission_observed::fixture_lookup_fail();
    let previous = h(14).load(Ordering::Acquire);
    unsafe {
        submission_observed_probe_driver(
            native.as_mut_ptr(),
            submission_observed::fixture_gateways()[2],
            0,
        );
    }
    assert_eq!(h(14).load(Ordering::Acquire), previous + 1);
    assert_eq!(h(12).load(Ordering::Acquire), 0);
    assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    let frozen = submission_observed::fixture_bytes();
    for i in 0..4 {
        let before = u64_at(&native, 0x80);
        unsafe {
            submission_observed_probe_driver(
                native.as_mut_ptr(),
                submission_observed::fixture_gateways()[i],
                0,
            );
        }
        assert_eq!(u64_at(&native, 0x80), before + 1);
        assert_eq!(
            submission_observed::fixture_bytes(),
            frozen,
            "closed rejects all recorder writes, gateway{i}"
        );
    }
}
global_asm!(
    ".globl submission_observed_delay_producer",
    ".seh_proc submission_observed_delay_producer",
    "submission_observed_delay_producer:",
    "mov [rsp+0x18],rsi",
    ".seh_savereg rsi,0x18",
    "push rdi",
    ".seh_pushreg rdi",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    "mov qword ptr [rcx],1",
    "1:",
    "cmp qword ptr [rcx+8],0",
    "je 1b",
    "inc qword ptr [rcx+16]",
    "mov r8,0x3344",
    "mov rsi,[rsp+0x40]",
    "lea rsp,[rsp+0x20]",
    "pop rdi",
    "ret",
    ".seh_endproc",
    ".globl submission_observed_delay_push_host",
    ".seh_proc submission_observed_delay_push_host",
    "submission_observed_delay_push_host:",
    "sub rsp,0x38",
    ".seh_stackalloc 0x38",
    "movups xmm0,[r9]",
    ".seh_endprologue",
    "mov qword ptr [rcx],1",
    "2:",
    "cmp qword ptr [rcx+8],0",
    "je 2b",
    "inc qword ptr [rcx+16]",
    "mov r8,0x3344",
    "lea rsp,[rsp+0x38]",
    "ret",
    ".seh_endproc",
);
unsafe extern "system" {
    fn submission_observed_delay_producer();
    fn submission_observed_delay_push_host();
}
fn submission_observed_late(name: &str, index: usize) {
    if !child(name) {
        return;
    }
    let ret = submission_observed_probe_return as *const () as u64;
    submission_observed_fixture((ret - 0x992267) as usize, (ret - 0x985ef4) as usize);
    let kind = if index == 0 { 6 } else { 9 };
    let body = if index == 0 {
        submission_observed_delay_producer as *const () as usize
    } else {
        submission_observed_delay_push_host as *const () as usize
    };
    submission_observed::fixture_continue(index, body + if index == 0 { 5 } else { 8 });
    let data = std::sync::Arc::new([const { AtomicU64::new(0) }; 160]);
    if index == 3 {
        submission_observed::fixture_birth(0x5566, 0);
    }
    let target = data.clone();
    let worker = std::thread::spawn(move || unsafe {
        submission_observed_probe_driver(
            target.as_ptr() as *mut u8,
            submission_observed::fixture_gateways()[index],
            kind,
        );
    });
    let until = Instant::now() + Duration::from_secs(5);
    while data[0].load(Ordering::Acquire) == 0 {
        assert!(Instant::now() < until);
        std::thread::yield_now();
    }
    let status = stop_at(Instant::now());
    let pending = h(12).load(Ordering::Acquire) & !ROOT_CLOSED;
    let pendingrow = if index == 0 {
        0
    } else {
        submission_observed::fixture_last_entry() + 1
    };
    let uncommitted = submission::TRACE.rows[pendingrow][0].load(Ordering::Acquire) == 0;
    data[1].store(1, Ordering::Release);
    worker.join().unwrap();
    assert_eq!(status, 2);
    assert!(pending > 0 && uncommitted);
    assert_eq!(data[2].load(Ordering::Acquire), 1);
    if index == 0 {
        assert_eq!(h(20).load(Ordering::Acquire), 1);
        assert_eq!(h(10).load(Ordering::Acquire), 0);
    } else {
        assert_eq!(
            submission::TRACE.rows[pendingrow][2].load(Ordering::Acquire),
            2
        );
        assert_eq!(
            submission::TRACE.rows[pendingrow][34].load(Ordering::Acquire),
            0x3344
        );
    }
    assert_eq!(h(12).load(Ordering::Acquire), ROOT_CLOSED);
    assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
}
#[test]
fn submission_owned_os_observed_producer_timeout_preserves_late_reserved_return() {
    submission_observed_late(
        "submission_owned_os_observed_producer_timeout_preserves_late_reserved_return",
        0,
    );
}
#[test]
fn submission_owned_os_observed_actual_callee_timeout_preserves_late_witness_return() {
    submission_observed_late(
        "submission_owned_os_observed_actual_callee_timeout_preserves_late_witness_return",
        3,
    );
}
#[test]
fn submission_owned_os_observed_virtual_unwind_four_displaced_shims_and_gateways() {
    if !child("submission_owned_os_observed_virtual_unwind_four_displaced_shims_and_gateways") {
        return;
    }
    unsafe extern "system" {
        fn observed_producer_original_body();
        fn observed_producer_original_branch();
        fn observed_host_original_body();
        fn observed_host_original_branch();
        fn observed_core_original_body();
        fn observed_core_original_branch();
        fn observed_push_host_original_body();
        fn observed_push_host_original_branch();
        fn observed_producer_gateway_body();
        fn observed_host_gateway_body();
        fn observed_core_gateway_body();
        fn observed_push_host_gateway_body();
    }
    let bodies = [
        observed_producer_original_body as *const () as usize,
        observed_host_original_body as *const () as usize,
        observed_core_original_body as *const () as usize,
        observed_push_host_original_body as *const () as usize,
    ];
    let branches = [
        observed_producer_original_branch as *const () as usize,
        observed_host_original_branch as *const () as usize,
        observed_core_original_branch as *const () as usize,
        observed_push_host_original_branch as *const () as usize,
    ];
    let gateways = [
        observed_producer_gateway_body as *const () as usize,
        observed_host_gateway_body as *const () as usize,
        observed_core_gateway_body as *const () as usize,
        observed_push_host_gateway_body as *const () as usize,
    ];
    for i in 0..4 {
        for (pc, allocation, saved) in [
            (submission_observed::fixture_originals()[i], 0, None),
            (bodies[i], [0, 0x28, 0x28, 0x38][i], Some(i)),
            (branches[i], [0, 0x28, 0x28, 0x38][i], Some(i)),
            (gateways[i], 0x1c8, None),
        ] {
            let mut stack = [0u64; 128];
            stack[allocation / 8] = 0x11223344;
            if let Some(reg) = saved {
                match reg {
                    0 => stack[0x18 / 8] = 0x55667788,
                    1 | 2 => stack[0x20 / 8] = 0x55667788,
                    _ => {}
                }
            }
            let mut ctx: CONTEXT = unsafe { std::mem::zeroed() };
            ctx.Rip = pc as u64;
            ctx.Rsp = stack.as_mut_ptr() as u64;
            ctx.Rbx = 0x55667788;
            ctx.Rsi = 0x55667788;
            ctx.Rdi = 0x55667788;
            if let Some(reg) = saved {
                match reg {
                    0 => ctx.Rsi = 0xaabbccdd,
                    1 => ctx.Rbx = 0xaabbccdd,
                    2 => ctx.Rdi = 0xaabbccdd,
                    _ => {}
                }
            }
            let mut image = 0;
            let entry = unsafe { RtlLookupFunctionEntry(ctx.Rip, &mut image, null_mut()) };
            assert!(!entry.is_null());
            let mut data = null_mut();
            let mut frame = 0;
            unsafe {
                RtlVirtualUnwind(
                    0,
                    image,
                    ctx.Rip,
                    entry,
                    &mut ctx,
                    &mut data,
                    &mut frame,
                    null_mut(),
                );
            }
            assert_eq!(ctx.Rip, 0x11223344, "gateway{i} PC{pc:x}");
            assert_eq!(ctx.Rsp, stack.as_ptr() as u64 + allocation as u64 + 8);
            assert_eq!(ctx.Rbx, 0x55667788);
            assert_eq!(ctx.Rsi, 0x55667788);
            assert_eq!(ctx.Rdi, 0x55667788);
        }
    }
}
fn submission_observed_write_word(address: usize, bytes: [u8; 8]) {
    let mut old = 0;
    assert_ne!(
        unsafe {
            VirtualProtect(
                address as *const c_void,
                8,
                PAGE_EXECUTE_READWRITE,
                &mut old,
            )
        },
        0
    );
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, 8);
    }
    let mut discard = 0;
    assert_ne!(
        unsafe { VirtualProtect(address as *const c_void, 8, old, &mut discard) },
        0
    );
    assert_ne!(
        unsafe { FlushInstructionCache(GetCurrentProcess(), address as *const c_void, 8) },
        0
    );
}
fn submission_observed_cas_fixture(core: &OwnedCore, host: &OwnedCore) {
    submission_observed_fixture(core.base as usize, host.base as usize);
    h(15).store(0, Ordering::Release);
    for (i, word) in submission::WORDS.into_iter().take(2).enumerate() {
        submission_observed_write_word(core.base as usize + site_rvas()[i], word);
    }
    for (rva, is_host, word, _) in submission_observed::EXTRA {
        submission_observed_write_word(
            if is_host {
                host.base as usize + rva
            } else {
                core.base as usize + rva
            },
            word,
        );
    }
    assert!(unsafe { install_sites(core.base as usize, submission::WORDS) }.is_ok());
}
#[test]
fn submission_owned_os_observed_mixed_host_partial_cas_rolls_back_owned_sites() {
    if !child("submission_owned_os_observed_mixed_host_partial_cas_rolls_back_owned_sites") {
        return;
    }
    let core = OwnedCore::new();
    let host = OwnedCore::new();
    submission_observed_cas_fixture(&core, &host);
    FAIL_AFTER_PUBLISH.store(4, Ordering::Release);
    assert!(unsafe { submission::install_extra(core.base as usize) }.is_err());
    assert_eq!(h(15).load(Ordering::Acquire), 15);
    assert_eq!(
        submission_observed::fixture_owners(),
        [true, true, false, false]
    );
    assert!(unsafe { restore_all() }.is_ok());
    assert_eq!(submission_observed::fixture_owners(), [false; 4]);
    for (rva, is_host, word, _) in submission_observed::EXTRA {
        assert_eq!(
            copy_current::<8>(if is_host {
                host.base as u64 + rva as u64
            } else {
                core.base as u64 + rva as u64
            }),
            Some(word)
        );
    }
}
#[test]
fn submission_owned_os_observed_rollback_continues_after_failed_compare_and_retains_owner() {
    if !child(
        "submission_owned_os_observed_rollback_continues_after_failed_compare_and_retains_owner",
    ) {
        return;
    }
    let core = OwnedCore::new();
    let host = OwnedCore::new();
    submission_observed_cas_fixture(&core, &host);
    assert!(unsafe { submission::install_extra(core.base as usize) }.is_ok());
    let entry = core.base as usize + 0x4a79b0;
    let patched = copy_current::<8>(entry as u64).unwrap();
    let mut foreign = patched;
    foreign[7] ^= 1;
    submission_observed_write_word(entry, foreign);
    assert!(unsafe { restore_all() }.is_err());
    assert_eq!(
        submission_observed::fixture_owners(),
        [true, false, false, false]
    );
    assert_eq!(h(17).load(Ordering::Acquire), 63);
    assert_eq!(h(18).load(Ordering::Acquire) & !4, 59);
    submission_observed_write_word(entry, patched);
    assert!(unsafe { restore_all() }.is_ok());
    assert_eq!(submission_observed::fixture_owners(), [false; 4]);
}
global_asm!(
    ".macro submission_observed_seh_body",
    "inc qword ptr [rcx+0x80]","mov rdx,[rcx+0x28]","mov rcx,[rcx+0x20]",
    "lea r10,[rsp+0x40]","mov r11d,0x9a","1:","mov qword ptr [r10],0",
    "lea r10,[r10+8]","dec r11d","jnz 1b","lea r10,[rsp+0x40]",
    "mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0","xor r8d,r8d","xor r9d,r9d",
    "call {rtl_unwind}","int3",".endm",
    ".globl submission_observed_seh_producer",".seh_proc submission_observed_seh_producer",
    "submission_observed_seh_producer:","mov [rsp+0x18],rsi",".seh_savereg rsi,0x18",
    "push rdi",".seh_pushreg rdi","sub rsp,0x20",".seh_stackalloc 0x20",
    "sub rsp,0x520",".seh_stackalloc 0x520",".seh_endprologue",
    "submission_observed_seh_body",".seh_endproc",
    ".globl submission_observed_seh_host",".seh_proc submission_observed_seh_host",
    "submission_observed_seh_host:",".byte 0x40,0x53",".seh_pushreg rbx",
    "sub rsp,0x20",".seh_stackalloc 0x20","sub rsp,0x520",".seh_stackalloc 0x520",
    ".seh_endprologue","submission_observed_seh_body",".seh_endproc",
    ".globl submission_observed_seh_core",".seh_proc submission_observed_seh_core",
    "submission_observed_seh_core:",".byte 0x40,0x57",".seh_pushreg rdi",
    "sub rsp,0x20",".seh_stackalloc 0x20","sub rsp,0x520",".seh_stackalloc 0x520",
    ".seh_endprologue","submission_observed_seh_body",".seh_endproc",
    ".globl submission_observed_seh_push_host",".seh_proc submission_observed_seh_push_host",
    "submission_observed_seh_push_host:","sub rsp,0x38",".seh_stackalloc 0x38",
    "movups xmm0,[r9]","sub rsp,0x520",".seh_stackalloc 0x520",
    ".seh_endprologue","submission_observed_seh_body",".seh_endproc",
    ".globl submission_observed_seh_driver",".seh_proc submission_observed_seh_driver",
    "submission_observed_seh_driver:","push rbx",".seh_pushreg rbx",
    "sub rsp,0x80",".seh_stackalloc 0x80",".seh_endprologue",
    "mov rbx,rcx","mov [rsp+0x20],rdx","mov [rcx+0x20],rsp",
    "lea rax,[rip+submission_observed_seh_landed]","mov [rcx+0x28],rax",
    "mov rdx,r8","lea r8,[rsp-8]","call {prepare}",
    "mov rcx,rbx","mov rdx,0x5566","mov r8,0x7788","lea r9,[rbx+0x400]",
    "call qword ptr [rsp+0x20]",".globl submission_observed_seh_return",
    "submission_observed_seh_return:","mov dword ptr [rsp+0x30],0","jmp 2f",
    "submission_observed_seh_landed:","mov dword ptr [rsp+0x30],1",
    "2:","call {cleanup}","mov eax,[rsp+0x30]","lea rsp,[rsp+0x80]","pop rbx","ret",".seh_endproc",
    rtl_unwind=sym RtlUnwindEx,prepare=sym submission_observed_prepare,cleanup=sym submission_observed_cleanup,
);
unsafe extern "system" {
    fn submission_observed_seh_driver(data: *mut u8, target: usize, kind: u64) -> u64;
    fn submission_observed_seh_return();
    fn submission_observed_seh_producer();
    fn submission_observed_seh_host();
    fn submission_observed_seh_core();
    fn submission_observed_seh_push_host();
}
fn submission_observed_seh(name: &str, index: usize) {
    if !child(name) {
        return;
    }
    let ret = submission_observed_seh_return as *const () as u64;
    let core = ret - 0x992267;
    let exe = ret - 0x985ef4;
    submission_observed_fixture(core as usize, exe as usize);
    let body = [
        submission_observed_seh_producer as *const () as usize,
        submission_observed_seh_host as *const () as usize,
        submission_observed_seh_core as *const () as usize,
        submission_observed_seh_push_host as *const () as usize,
    ][index];
    submission_observed::fixture_continue(index, body + [5, 6, 6, 8][index]);
    let mut data = vec![0u8; 0x500];
    let kind = [6, 7, 8, 9][index];
    submission_observed_seed(&mut data, kind, core, exe);
    let result = unsafe {
        submission_observed_seh_driver(
            data.as_mut_ptr(),
            submission_observed::fixture_gateways()[index],
            kind,
        )
    };
    assert_eq!(
        result, 1,
        "actual RtlUnwindEx landed outside complete original frame"
    );
    assert_eq!(u64_at(&data, 0x80), 1);
    assert_eq!(
        h(10).load(Ordering::Acquire),
        if index == 0 { 0 } else { 12 }
    );
    assert_eq!(h(13).load(Ordering::Acquire), 1);
    assert_eq!(h(12).load(Ordering::Acquire), 0);
    assert_eq!(STACK.with(Cell::get).depth, 0);
    if index == 0 {
        assert_eq!(h(20).load(Ordering::Acquire), 0);
        assert!(h(14).load(Ordering::Acquire) > 0);
    } else {
        let row = submission_observed::fixture_last_entry() + 1;
        assert_eq!(submission::TRACE.rows[row][2].load(Ordering::Acquire), 3);
        assert_eq!(submission::TRACE.rows[row][21].load(Ordering::Acquire), 0);
        assert_eq!(submission::TRACE.rows[row][34].load(Ordering::Acquire), 0);
    }
    assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
}
#[test]
fn submission_owned_os_observed_producer_actual_seh_no_return() {
    submission_observed_seh(
        "submission_owned_os_observed_producer_actual_seh_no_return",
        0,
    );
}
#[test]
fn submission_owned_os_observed_host_exec_actual_seh_no_return() {
    submission_observed_seh(
        "submission_owned_os_observed_host_exec_actual_seh_no_return",
        1,
    );
}
#[test]
fn submission_owned_os_observed_core_exec_actual_seh_no_return() {
    submission_observed_seh(
        "submission_owned_os_observed_core_exec_actual_seh_no_return",
        2,
    );
}
#[test]
fn submission_owned_os_observed_actual_callee_actual_seh_no_return() {
    submission_observed_seh(
        "submission_owned_os_observed_actual_callee_actual_seh_no_return",
        3,
    );
}

// Owned synthetic code at the same finite caller RVAs. No game DLL, scheduler,
// constructor fields or World interface is invoked. All six real CAS gateways
// and their release displaced shims execute; their original fixture bodies run
// once. Counters/control bytes below belong only to this owned fixture.
struct SubmissionPipeline {
    core: OwnedCore,
    host: OwnedCore,
    owner: Box<[u64; 160]>,
    task: Box<[u64; 2]>,
}
impl SubmissionPipeline {
    fn new() -> Self {
        let core = OwnedCore::new();
        let host = OwnedCore::new();
        let mut owner = Box::new([0u64; 160]);
        let mut task = Box::new([0u64; 2]);
        owner[0xd0 / 8] = core.base as u64 + 0xa06648;
        task[0] = host.base as u64 + 0x106b3c8;
        let mut this = Self {
            core,
            host,
            owner,
            task,
        };
        this.prepare_code();
        this
    }
    fn write(&self, host: bool, rva: usize, bytes: &[u8]) {
        let p = (if host {
            self.host.base as usize
        } else {
            self.core.base as usize
        }) + rva;
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(p as _, bytes.len(), PAGE_EXECUTE_READWRITE, &mut old) },
            0
        );
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), p as *mut u8, bytes.len());
        }
        let mut discard = 0;
        assert_ne!(
            unsafe { VirtualProtect(p as _, bytes.len(), old, &mut discard) },
            0
        );
        assert_ne!(
            unsafe { FlushInstructionCache(GetCurrentProcess(), p as _, bytes.len()) },
            0
        );
    }
    fn branch(&self, host: bool, rva: usize, op: u8, to: usize) {
        let mut bytes = vec![op];
        bytes
            .extend_from_slice(&(i32::try_from(to as i64 - rva as i64 - 5).unwrap()).to_le_bytes());
        self.write(host, rva, &bytes);
    }
    fn prepare_code(&mut self) {
        submission_observed_fixture(self.core.base as usize, self.host.base as usize);
        h(15).store(0, Ordering::Release);
        // Test gateways normally use injected originals. This pipeline must
        // replay the real displaced prologues and their installed continuation.
        submission::SUBMISSION_TEST_ORIGINALS[0].store(
            submission::submission_push_original as *const () as u64,
            Ordering::Release,
        );
        submission::SUBMISSION_TEST_ORIGINALS[1].store(
            submission::submission_publish_original as *const () as u64,
            Ordering::Release,
        );
        let mut p = vec![0x48, 0x89, 0x74, 0x24, 0x18, 0x57, 0x48, 0x83, 0xec, 0x20];
        // RSI=owner, RBX=list2, count++, own submit-control byte.
        p.extend_from_slice(&[
            0x48, 0x89, 0x5c, 0x24, 0x38, 0x48, 0x8b, 0xf1, 0x48, 0xff, 0x86, 0x80, 0, 0, 0, 0x80,
            0xbe, 0x88, 0, 0, 0, 0, 0x0f, 0x84,
        ]);
        let jz = p.len();
        p.extend_from_slice(&[0; 4]);
        p.extend_from_slice(&[
            0x48, 0x8d, 0x96, 0xd0, 0, 0, 0, 0xbb, 2, 0, 0, 0, 0x48, 0x8b, 0xcb,
        ]);
        let jmp = p.len();
        p.extend_from_slice(&[0xe9, 0, 0, 0, 0]);
        p[jz..jz + 4].copy_from_slice(&(0x4a7a58i32 - (0x4a79b0 + jz + 4) as i32).to_le_bytes());
        p[jmp + 1..jmp + 5]
            .copy_from_slice(&(0x4a7a4bi32 - (0x4a79b0 + jmp + 5) as i32).to_le_bytes());
        self.write(false, 0x4a79b0, &p);
        self.branch(false, 0x4a7a4b, 0xe8, 0x4a1100);
        self.write(false, 0x4a7a50, &[0x48, 0x8b, 0xcb]);
        self.branch(false, 0x4a7a53, 0xe8, 0x4a10b0);
        self.write(
            false,
            0x4a7a58,
            &[
                0x48, 0x8b, 0x5c, 0x24, 0x38, 0x48, 0x8b, 0x74, 0x24, 0x40, 0x48, 0x8d, 0x64, 0x24,
                0x20, 0x5f, 0xc3,
            ],
        );
        let mut wrapper = vec![0x48, 0x83, 0xec, 0x38, 0x48, 0xb8];
        wrapper.extend_from_slice(&(self.core.base as u64 + 0xa06920).to_le_bytes());
        wrapper.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20, 0x48, 0xb8]);
        wrapper.extend_from_slice(&0x1f00000000u64.to_le_bytes());
        wrapper.extend_from_slice(&[
            0x48, 0x89, 0x44, 0x24, 0x28, 0x4c, 0x8d, 0x4c, 0x24, 0x20, 0x41, 0xb8, 0, 0, 3, 0,
        ]);
        self.write(false, 0x4a1100, &wrapper);
        self.branch(false, 0x4a1100 + wrapper.len(), 0xe9, 0x4a1145);
        self.branch(false, 0x4a1145, 0xe8, 0x992210);
        self.write(false, 0x4a114a, &[0x48, 0x83, 0xc4, 0x38, 0xc3]);
        // Publication's native tail restores its full frame before forwarding.
        self.write(
            false,
            0x4a10b0,
            &[0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0xd9],
        );
        self.branch(false, 0x4a10b9, 0xe9, 0x4a10f4);
        self.write(false, 0x4a10f4, &[0x48, 0x83, 0xc4, 0x20, 0x5b]);
        self.branch(false, 0x4a10f9, 0xe9, 0x992280);
        self.write(
            false,
            0x992210,
            &[
                0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x6c, 0x24, 0x10, 0x48, 0x89, 0x74, 0x24,
                0x18, 0x57, 0x48, 0x83, 0xec, 0x30,
            ],
        );
        let mut target = vec![0x48, 0xb8];
        target.extend_from_slice(&(self.host.base as u64 + 0x986060).to_le_bytes());
        self.write(false, 0x992224, &target);
        self.branch(false, 0x99222e, 0xe9, 0x992265);
        self.write(
            false,
            0x992265,
            &[
                0xff, 0xd0, 0x48, 0x8b, 0x5c, 0x24, 0x40, 0x48, 0x8d, 0x64, 0x24, 0x30, 0x5f, 0xc3,
            ],
        );
        self.write(
            false,
            0x992280,
            &[
                0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0xc1, 0x48, 0x8d, 0x64, 0x24, 0x20,
                0x5b, 0xc3,
            ],
        );
        self.write(
            false,
            0x4a8530,
            &[
                0x40, 0x57, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0xc1, 0x48, 0x8d, 0x64, 0x24, 0x20,
                0x5f, 0xc3,
            ],
        );
        self.write(
            true,
            0x985ec0,
            &[0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0x49, 8],
        );
        self.branch(true, 0x985eca, 0xe9, 0x985ee0);
        let mut call = vec![0x48, 0xb8];
        call.extend_from_slice(&(self.core.base as u64 + 0x4a8530).to_le_bytes());
        self.write(true, 0x985ee0, &call);
        self.branch(true, 0x985eea, 0xe9, 0x985ef2);
        self.write(
            true,
            0x985ef2,
            &[0xff, 0xd0, 0x48, 0x8d, 0x64, 0x24, 0x20, 0x5b, 0xc3],
        );
        self.bind_task();
        assert!(unsafe { install_sites(self.core.base as usize, submission::WORDS) }.is_ok());
        assert!(unsafe { submission::install_extra(self.core.base as usize) }.is_ok());
    }
    fn bind_task(&self) {
        let mut bytes = vec![0x48, 0x83, 0xec, 0x38, 0x41, 0x0f, 0x10, 1, 0x49, 0xb8];
        bytes.extend_from_slice(&(self.task.as_ptr() as u64).to_le_bytes());
        bytes.extend_from_slice(&[
            0x49, 0x89, 0x50, 8, 0x4c, 0x89, 0xc0, 0x48, 0x8d, 0x64, 0x24, 0x38, 0xc3,
        ]);
        // Before hooks only; later rebind preserves the installed first word.
        if h(15).load(Ordering::Acquire) & 32 != 0 {
            self.write(true, 0x986068, &bytes[8..]);
        } else {
            self.write(true, 0x986060, &bytes);
        }
    }
    fn producer(&mut self, submit: bool) {
        self.owner[0x88 / 8] = u64::from(submit);
        let call = unsafe {
            std::mem::transmute::<usize, unsafe extern "system" fn(*mut u64)>(
                self.core.base as usize + 0x4a79b0,
            )
        };
        unsafe {
            call(self.owner.as_mut_ptr());
        }
    }
    fn execute(&self, t: *const u64) {
        let call = unsafe {
            std::mem::transmute::<usize, unsafe extern "system" fn(*const u64)>(
                self.host.base as usize + 0x985ec0,
            )
        };
        unsafe {
            call(t);
        }
    }
}
impl Drop for SubmissionPipeline {
    fn drop(&mut self) {
        assert!(unsafe { restore_all() }.is_ok());
    }
}
#[test]
fn submission_owned_os_observed_generic_proxy_reused_t_other_p_is_excluded_before_rows() {
    if !child("submission_owned_os_observed_generic_proxy_reused_t_other_p_is_excluded_before_rows")
    {
        return;
    }
    let mut p = SubmissionPipeline::new();
    p.producer(true);
    let t = p.task.as_ptr();
    p.execute(t);
    let old = submission_observed::fixture_book(0);
    let recorded = h(10).load(Ordering::Acquire);
    let mut other = Box::new([0u64; 8]);
    other[0] = p.core.base as u64 + 0xa06650;
    let counter = Box::new(AtomicU64::new(0));
    let mut body = vec![0x49, 0xbb];
    body.extend_from_slice(&(std::ptr::addr_of!(*counter) as u64).to_le_bytes());
    body.extend_from_slice(&[0x49, 0xff, 3, 0x48, 0x8d, 0x64, 0x24, 0x20, 0x5f, 0xc3]);
    p.write(false, 0x4a8539, &body);
    p.task[1] = other.as_ptr() as u64;
    for _ in 0..100 {
        p.execute(t);
    }
    assert_eq!(
        counter.load(Ordering::Acquire),
        100,
        "every original fixture body still executed"
    );
    assert_eq!(h(10).load(Ordering::Acquire), recorded);
    assert_eq!(submission_observed::fixture_book(0), old);
    assert_eq!(h(14).load(Ordering::Acquire), 0);
    assert!(h(30).load(Ordering::Acquire) >= 100);
    // Restore the legitimate current P; a fresh complete Gov cycle still binds.
    p.task[1] = old[1];
    p.producer(true);
    p.execute(t);
    assert_eq!(submission_observed::fixture_book(1)[15], 0);
    assert_eq!(h(29).load(Ordering::Acquire), 2);
    assert_eq!(h(10).load(Ordering::Acquire), 24);
    assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    let frozen = submission_observed::fixture_bytes();
    let before = counter.load(Ordering::Acquire);
    p.task[1] = other.as_ptr() as u64;
    p.execute(t);
    assert_eq!(counter.load(Ordering::Acquire), before + 1);
    assert_eq!(
        submission_observed::fixture_bytes(),
        frozen,
        "closed gate performs no scope reads or writes"
    );
}
#[test]
fn submission_owned_os_observed_active_tuple_wrong_p_vft_is_error_not_exclusion() {
    if !child("submission_owned_os_observed_active_tuple_wrong_p_vft_is_error_not_exclusion") {
        return;
    }
    let mut p = SubmissionPipeline::new();
    p.producer(true);
    let t = p.task.as_ptr();
    let row = 8; // Host role of the first complete-chain reservation
    let excluded = h(30).load(Ordering::Acquire);
    p.owner[0xd0 / 8] = p.core.base as u64 + 0xa06650;
    p.execute(t);
    assert_eq!(h(30).load(Ordering::Acquire), excluded);
    assert_ne!(
        submission::TRACE.rows[row][20].load(Ordering::Acquire) & 0x400,
        0
    );
    assert_eq!(
        submission::TRACE.rows[row][42].load(Ordering::Acquire),
        p.core.base as u64 + 0xa06650
    );
    assert_eq!(submission::TRACE.rows[row][43].load(Ordering::Acquire), 8);
    assert_ne!(submission_observed::fixture_book(0)[15], 0);
    assert!(h(14).load(Ordering::Acquire) > 0);
}
#[test]
fn submission_owned_os_observed_untracked_governor_and_unreadable_current_p_remain_unknown() {
    if !child(
        "submission_owned_os_observed_untracked_governor_and_unreadable_current_p_remain_unknown",
    ) {
        return;
    }
    let p = SubmissionPipeline::new();
    let governor = Box::new([p.core.base as u64 + 0xa06648, 0, 0, 0, 0, 0, 0, 0]);
    let good = Box::new([p.host.base as u64 + 0x106b3c8, governor.as_ptr() as u64]);
    let bad = Box::new([p.host.base as u64 + 0x106b3c8, 1]);
    let excluded = h(30).load(Ordering::Acquire);
    p.execute(good.as_ptr());
    p.execute(bad.as_ptr());
    assert_eq!(h(29).load(Ordering::Acquire), 0);
    assert_eq!(h(30).load(Ordering::Acquire), excluded);
    assert_eq!(h(10).load(Ordering::Acquire), 0);
    assert_eq!(submission_observed::fixture_extra_next(), 8);
    let base = civ6_bootstrap_rs::protocol::SUBMISSION_ROWS;
    for row in [base, base + 4] {
        assert_eq!(submission::TRACE.rows[row][35].load(Ordering::Acquire), 0);
        assert_ne!(submission::TRACE.rows[row][20].load(Ordering::Acquire), 0);
    }
    assert_eq!(submission::TRACE.rows[base][43].load(Ordering::Acquire), 8);
    assert_eq!(
        submission::TRACE.rows[base + 4][43].load(Ordering::Acquire),
        0
    );
    assert_ne!(
        submission::TRACE.rows[base + 4][44].load(Ordering::Acquire),
        0
    );
    assert_eq!(
        submission::TRACE.rows[base + 4][45].load(Ordering::Acquire),
        0
    );
    assert!(h(14).load(Ordering::Acquire) > 0);
}
#[test]
fn submission_owned_os_observed_no_submit_100_then_real_submit_and_current_t_p_bind() {
    if !child("submission_owned_os_observed_no_submit_100_then_real_submit_and_current_t_p_bind") {
        return;
    }
    let mut p = SubmissionPipeline::new();
    for _ in 0..100 {
        p.producer(false);
    }
    assert_eq!(h(20).load(Ordering::Acquire), 100);
    assert_eq!(h(21).load(Ordering::Acquire), 100);
    assert_eq!(h(10).load(Ordering::Acquire), 0);
    assert_eq!(h(29).load(Ordering::Acquire), 0);
    p.producer(true);
    let task = p.task.as_ptr();
    p.execute(task);
    let b = submission_observed::fixture_book(0);
    assert_eq!(b[0], 101);
    assert_eq!(b[2], task as u64);
    assert_eq!(b[15], 0);
    assert_eq!(b[11], 511 | 8192 | 16384);
    assert_eq!(h(29).load(Ordering::Acquire), 1);
    assert_eq!(h(10).load(Ordering::Acquire), 12);
    assert_eq!(h(11).load(Ordering::Acquire), 0);
    assert_eq!(h(14).load(Ordering::Acquire), 0);
    assert_eq!(h(12).load(Ordering::Acquire), 0);
    assert_eq!(p.owner[0x80 / 8], 101);
}
#[test]
fn submission_owned_os_observed_completed_p_t_reissue_and_late_old_or_unknown_t_stay_unassigned() {
    if !child("submission_owned_os_observed_completed_p_t_reissue_and_late_old_or_unknown_t_stay_unassigned") {return;}
    let mut p = SubmissionPipeline::new();
    p.producer(true);
    let old = p.task.as_ptr();
    p.execute(old);
    let first = submission_observed::fixture_book(0);
    let header = civ6_bootstrap_rs::protocol::SUBMISSION_HEADER;
    let preserved = submission_observed::fixture_bytes()[header..header + 12 * 384].to_vec();
    // Same T address is accepted only after all old normal full-frame returns.
    p.producer(true);
    p.execute(old);
    assert_eq!(submission_observed::fixture_book(1)[15], 0);
    assert_eq!(
        submission_observed::fixture_bytes()[header..header + 12 * 384],
        preserved
    );
    let old_owner = std::mem::replace(&mut p.task, Box::new([p.host.base as u64 + 0x106b3c8, 0]));
    p.bind_task();
    p.producer(true);
    let before =
        civ6_bootstrap_rs::protocol::SUBMISSION_ROWS + submission_observed::fixture_extra_next();
    let unresolved = Box::new([p.host.base as u64 + 0x106b3c8, first[1]]);
    p.execute(old_owner.as_ptr());
    p.execute(unresolved.as_ptr());
    for i in [before, before + 4] {
        assert_eq!(submission::TRACE.rows[i][35].load(Ordering::Acquire), 0);
        assert!(submission::TRACE.rows[i][20].load(Ordering::Acquire) > 0);
        assert_eq!(
            submission::TRACE.rows[i + 2][35].load(Ordering::Acquire),
            0,
            "Core uses current unknown host, never oldest P"
        );
    }
    assert_eq!(
        submission_observed::fixture_book(2)[6],
        0,
        "unknown T receives no new birth"
    );
    assert!(h(14).load(Ordering::Acquire) > 0);
    assert_eq!(h(29).load(Ordering::Acquire), 3);
}
#[test]
fn submission_owned_os_observed_open_birth_reissue_refused_without_overwriting_old_book() {
    if !child(
        "submission_owned_os_observed_open_birth_reissue_refused_without_overwriting_old_book",
    ) {
        return;
    }
    let mut p = SubmissionPipeline::new();
    p.producer(true);
    let first = submission_observed::fixture_book(0);
    p.producer(true);
    assert_eq!(submission_observed::fixture_book(0), first);
    assert_eq!(h(29).load(Ordering::Acquire), 1);
    assert!(h(14).load(Ordering::Acquire) > 0);
    assert_eq!(h(20).load(Ordering::Acquire), 0);
}
#[test]
fn submission_owned_os_observed_sixteen_full_cycles_own_all_192_rows_without_overwrite() {
    if !child("submission_owned_os_observed_sixteen_full_cycles_own_all_192_rows_without_overwrite")
    {
        return;
    }
    use civ6_bootstrap_rs::protocol::{
        SUBMISSION_BYTES, SUBMISSION_HEADER, SUBMISSION_ROW, SUBMISSION_ROWS,
    };
    let mut p = SubmissionPipeline::new();
    let t = p.task.as_ptr();
    for cycle in 0..16 {
        let old = submission_observed::fixture_bytes()
            [SUBMISSION_HEADER..SUBMISSION_HEADER + cycle * 12 * SUBMISSION_ROW]
            .to_vec();
        p.producer(true);
        assert_eq!(
            h(10).load(Ordering::Acquire),
            (cycle as u64 + 1) * 12,
            "whole cycle reserved at dispatch"
        );
        assert_eq!(
            h(12).load(Ordering::Acquire),
            1,
            "unobserved execution holds its sampling reservation"
        );
        p.execute(t);
        assert_eq!(submission_observed::fixture_book(cycle)[15], 0);
        for role in 0..6 {
            let i = cycle * 12 + role * 2;
            assert_eq!(
                submission::TRACE.rows[i][0].load(Ordering::Acquire),
                i as u64 + 1
            );
            assert_eq!(submission::TRACE.rows[i + 1][2].load(Ordering::Acquire), 2);
        }
        assert_eq!(
            &submission_observed::fixture_bytes()[SUBMISSION_HEADER..SUBMISSION_HEADER + old.len()],
            old.as_slice()
        );
        assert_eq!(h(12).load(Ordering::Acquire), 0);
    }
    assert_eq!(h(10).load(Ordering::Acquire), SUBMISSION_ROWS as u64);
    assert_eq!(h(29).load(Ordering::Acquire), 16);
    assert_eq!(h(11).load(Ordering::Acquire), 0);
    assert_eq!(h(14).load(Ordering::Acquire), 0);
    assert_eq!(submission_observed::fixture_bytes().len(), SUBMISSION_BYTES);
    assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    let sealed = submission_observed::fixture_bytes();
    p.execute(t);
    assert_eq!(submission_observed::fixture_bytes(), sealed);
}
#[test]
fn submission_owned_os_observed_closed_reserved_cycle_records_late_host_and_core_returns() {
    if !child(
        "submission_owned_os_observed_closed_reserved_cycle_records_late_host_and_core_returns",
    ) {
        return;
    }
    let mut p = SubmissionPipeline::new();
    p.producer(true);
    let t = p.task.as_ptr();
    assert_eq!(h(10).load(Ordering::Acquire), 12);
    assert_eq!(h(12).load(Ordering::Acquire), 1);
    assert_eq!(
        stop_at(Instant::now()),
        2,
        "absent execution is unresolved, not cancellation or flight zero"
    );
    assert_eq!(submission::TRACE.rows[8][0].load(Ordering::Acquire), 0);
    p.execute(t);
    assert_eq!(submission::TRACE.rows[9][2].load(Ordering::Acquire), 2);
    assert_eq!(submission::TRACE.rows[11][2].load(Ordering::Acquire), 2);
    assert_eq!(h(10).load(Ordering::Acquire), 12);
    assert_eq!(h(12).load(Ordering::Acquire), ROOT_CLOSED);
    assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    let sealed = submission_observed::fixture_bytes();
    p.execute(t);
    assert_eq!(submission_observed::fixture_bytes(), sealed);
}
#[test]
fn submission_owned_os_observed_out_of_order_duplicate_unknown_frames_use_separate_loss_budget() {
    if !child("submission_owned_os_observed_out_of_order_duplicate_unknown_frames_use_separate_loss_budget") {return;}
    use civ6_bootstrap_rs::protocol::{
        SUBMISSION_EXTRA_ROWS, SUBMISSION_HEADER, SUBMISSION_ROW, SUBMISSION_ROWS,
    };
    let mut p = SubmissionPipeline::new();
    p.producer(true);
    let t = p.task.as_ptr();
    let b = submission_observed::fixture_book(0);
    let core = unsafe {
        std::mem::transmute::<usize, unsafe extern "system" fn(u64)>(
            p.core.base as usize + 0x4a8530,
        )
    };
    unsafe {
        core(b[1]);
    } // No accepted Host parent: never consume the owned Core pair.
    assert_eq!(submission::TRACE.rows[10][0].load(Ordering::Acquire), 0);
    assert_eq!(h(10).load(Ordering::Acquire), 12);
    p.execute(t);
    let good = submission_observed::fixture_bytes()
        [SUBMISSION_HEADER..SUBMISSION_HEADER + 12 * SUBMISSION_ROW]
        .to_vec();
    for _ in 0..8 {
        p.execute(t);
    }
    assert_eq!(
        submission_observed::fixture_extra_next(),
        SUBMISSION_EXTRA_ROWS
    );
    assert!(submission_observed::fixture_budget(6) > 0);
    assert!(h(11).load(Ordering::Acquire) > 0);
    assert!(h(14).load(Ordering::Acquire) > 0);
    assert_eq!(h(10).load(Ordering::Acquire), 12);
    assert_eq!(
        &submission_observed::fixture_bytes()[SUBMISSION_HEADER..SUBMISSION_HEADER + good.len()],
        good.as_slice()
    );
    assert_eq!(h(12).load(Ordering::Acquire), 0);
    assert_ne!(
        submission::TRACE.rows[SUBMISSION_ROWS][20].load(Ordering::Acquire),
        0
    );
}
global_asm!(
    ".globl submission_reserved_unwind_driver",
    ".seh_proc submission_reserved_unwind_driver",
    "submission_reserved_unwind_driver:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x80",
    ".seh_stackalloc 0x80",
    ".seh_endprologue",
    "mov rbx,rcx",
    "mov rax,[rcx+8]",
    "mov [rax+0x20],rsp",
    "lea r10,[rip+submission_reserved_unwind_landed]",
    "mov [rax+0x28],r10",
    "call rdx",
    "xor eax,eax",
    "jmp 1f",
    "submission_reserved_unwind_landed:",
    "mov eax,1",
    "1:",
    "lea rsp,[rsp+0x80]",
    "pop rbx",
    "ret",
    ".seh_endproc",
);
unsafe extern "system" {
    fn submission_reserved_unwind_driver(t: *const u64, target: usize) -> u64;
}
#[test]
fn submission_owned_os_observed_reserved_cycle_seh_settles_once_with_unresolved_status() {
    if !child("submission_owned_os_observed_reserved_cycle_seh_settles_once_with_unresolved_status")
    {
        return;
    }
    let mut p = SubmissionPipeline::new();
    p.producer(true);
    let t = p.task.as_ptr();
    let mut table: Box<[IMAGE_RUNTIME_FUNCTION_ENTRY; 1]> = Box::new(unsafe { std::mem::zeroed() });
    // This fixture Host body has the same RBX push/20h allocation as its native
    // entry. Register the private-image unwind table for the actual SEH walk.
    p.write(true, 0xa0f000, &[1, 6, 2, 0, 6, 0x32, 2, 0x30]);
    table[0].BeginAddress = 0x985ec0;
    table[0].EndAddress = 0x985efb;
    table[0].Anonymous.UnwindData = 0xa0f000;
    assert_ne!(
        unsafe { RtlAddFunctionTable(table.as_mut_ptr(), 1, p.host.base as u64) },
        0
    );
    // The same Core release displaced shim runs before the static SEH fixture's
    // remaining large-frame prologue/body; it never fabricates a native RET.
    submission_observed::fixture_continue(
        2,
        submission_observed_seh_core as *const () as usize + 6,
    );
    let landed = unsafe { submission_reserved_unwind_driver(t, p.host.base as usize + 0x985ec0) };
    assert_eq!(landed, 1);
    assert_eq!(h(13).load(Ordering::Acquire), 2);
    assert_eq!(submission::TRACE.rows[9][2].load(Ordering::Acquire), 3);
    assert_eq!(submission::TRACE.rows[11][2].load(Ordering::Acquire), 3);
    assert_eq!(submission::TRACE.rows[9][34].load(Ordering::Acquire), 0);
    assert_eq!(h(12).load(Ordering::Acquire), 0);
    assert_ne!(submission_observed::fixture_book(0)[15], 0);
    assert_eq!(BridgeSubmissionObservationStop(null_mut()), 0);
    assert_ne!(unsafe { RtlDeleteFunctionTable(table.as_mut_ptr()) }, 0);
}
