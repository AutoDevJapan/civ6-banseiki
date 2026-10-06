//! Execute the real loader-return instruction form in the existing owned OS
//! fixture. No game launch, executable patch, or remote memory setter.
use super::super::test_support::Fixture;
use super::*;

core::arch::global_asm!(
    ".data",
    ".p2align 3",
    ".globl todo880_mov_slot",
    "todo880_mov_slot:",
    ".quad -1",
    ".globl todo880_mov_successors",
    "todo880_mov_successors:",
    ".quad 0",
    ".text",
    ".globl todo880_mov_once",
    "todo880_mov_once:",
    "mov qword ptr [rip + todo880_mov_slot], rax",
    "inc qword ptr [rip + todo880_mov_successors]",
    "ret",
);
extern "C" {
    fn todo880_mov_once();
    static todo880_mov_slot: u64;
    static todo880_mov_successors: u64;
}

fn raw(c: &CONTEXT, event: &DEBUG_EVENT) -> Value {
    let ex = unsafe { event.u.Exception };
    json!({"tid":event.dwThreadId,"code":ex.ExceptionRecord.ExceptionCode,"first_chance":ex.dwFirstChance,"address":ex.ExceptionRecord.ExceptionAddress as u64,"rip":c.Rip,"rax":c.Rax,"rsp":c.Rsp,"eflags":c.EFlags,"dr0":c.Dr0,"dr1":c.Dr1,"dr2":c.Dr2,"dr3":c.Dr3,"dr6":c.Dr6,"dr7":c.Dr7})
}

#[test]
fn order_capture_real_mov_single_step_measurement() {
    let mut child = Fixture::start();
    let pid = child.child.id();
    let local_base = unsafe { GetModuleHandleW(std::ptr::null()) } as u64;
    let exe = std::env::current_exe().unwrap();
    let remote_base = module_named(pid, exe.file_name().unwrap().to_str().unwrap())
        .unwrap()
        .base;
    let mov = remote_base + todo880_mov_once as *const () as u64 - local_base;
    let slot = remote_base + std::ptr::addr_of!(todo880_mov_slot) as u64 - local_base;
    let successors = remote_base + std::ptr::addr_of!(todo880_mov_successors) as u64 - local_base;
    let base = child.entry - CAN_EVER_RVA;
    let path = std::env::temp_dir().join(format!(
        "todo880-step-measurement-{}-{pid}.jsonl",
        std::process::id()
    ));
    let output = Jsonl::new(&path).unwrap();
    let trace_path = path.clone();
    let (stop_tx, _) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let observer = child.observe(&stop_tx, move |ack| {
        let mut d = Debugger::attach(pid, process(pid, true).unwrap(), base, 0x500000, output, ProbeMode::Governor).unwrap();
        d.close_ack = ack;
        let bytes = read_exact::<7>(&d.game, mov).unwrap();
        assert_eq!(&bytes[..3], &[0x48, 0x89, 0x05]);
        let displacement = i32::from_le_bytes(bytes[3..].try_into().unwrap());
        assert_eq!((mov + 7).wrapping_add_signed(i64::from(displacement)), slot);
        assert_eq!(read_pointer(&d.game, slot).unwrap(), u64::MAX);
        assert_eq!(read_pointer(&d.game, successors).unwrap(), 0);
        let mut original = None;
        let mut before = Value::Null;
        let mut instruction_hits = 0;
        let mut armed_eflags = 0;
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            assert!(Instant::now() < deadline, "real MOV fixture event deadline");
            let mut event = DEBUG_EVENT::default();
            if unsafe { WaitForDebugEventEx(&mut event, 50) } == 0 { continue; }
            d.event_ordinal += 1;
            d.cleanup_event = Some((event, DBG_EXCEPTION_NOT_HANDLED));
            d.last_dispatch = DispatchOutcome::Unrelated;
            if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT && unsafe { event.u.Exception.ExceptionRecord.ExceptionCode } == EXCEPTION_SINGLE_STEP {
                let tid = event.dwThreadId;
                let mut c = context(&d.threads[&tid].handle).unwrap();
                if original.is_none() {
                    // Reuse the existing fixture's real call and all its child
                    // lifetime/cleanup contracts. Redirect only this owned test
                    // thread to a compiled instruction, never patch its bytes.
                    assert_eq!(c.Rip, base + CAN_EVER_RVA);
                    original = Some(c);
                    c.Rip = mov;
                    c.Dr0 = mov;
                    c.Dr1 = 0; c.Dr2 = 0; c.Dr3 = 0; c.Dr7 = 1;
                    c.Dr6 &= !((1 << 14) | 15);
                    c.EFlags &= !(TF | RF);
                    d.threads.get_mut(&tid).unwrap().armed_sites = [Some(mov), None, None, None];
                    set_context(&d.threads[&tid].handle, &c).unwrap();
                } else if instruction_hits == 0 {
                    classify_then_record(&mut d, &event, &BTreeMap::new(), &[mov], |_, _| Ok(())).unwrap();
                    assert_eq!(c.Rip, mov);
                    before = raw(&c, &event);
                    instruction_hits += 1;
                    c.EFlags |= TF | RF;
                    c.Dr6 &= !((1 << 14) | 15);
                    set_context(&d.threads[&tid].handle, &c).unwrap();
                    let armed = context(&d.threads[&tid].handle).unwrap();
                    armed_eflags = armed.EFlags;
                    assert_ne!(armed_eflags & TF, 0);
                    assert_eq!(armed.Rip, mov);
                } else {
                    let after = raw(&c, &event);
                    let ex = unsafe { event.u.Exception };
                    let guard = plan::verify_step(mov + 7, ex.ExceptionRecord.ExceptionAddress as u64, c.Rip, c.Dr6, c.EFlags, armed_eflags);
                    let stored = read_pointer(&d.game, slot).unwrap();
                    let following = read_pointer(&d.game, successors).unwrap();
                    let mut evidence = json!({"type":"owned_os_mov_step","kind":"owned_rust_fixture","pid":pid,"instruction_bytes":bytes,"instruction_hits":instruction_hits,"completion_events":1,"before":before,"after":after,"armed_eflags":armed_eflags,"stored_value":stored,"successor_instructions_executed":following,"guard_accepted":guard.is_ok(),"guard_error":guard.as_ref().err(),"game_live":false});
                    println!("TODO880_STEP_MEASUREMENT={evidence}");
                    assert_eq!(c.Rip, mov + 7);
                    assert_eq!(ex.ExceptionRecord.ExceptionAddress as u64, mov + 7);
                    assert_eq!(stored, before["rax"].as_u64().unwrap());
                    assert_ne!(stored, u64::MAX);
                    assert_eq!(following, 0);
                    guard.unwrap();
                    let mut steps = BTreeMap::from([(tid, Step {
                        site: Site { name:"owned_fixture_mov", rva:mov-remote_base, bytes:&[], next:plan::Next::Sequential },
                        expected: mov + 8, armed_eflags,
                        capture:json!({"capture_seq":1}), phase:Phase::Front,
                    })]);
                    let mut failed_row = Value::Null;
                    assert!(classify_then_record(&mut d, &event, &steps, &[mov], |d, row| {
                        failed_row = row.clone();
                        mark(d, row).map_err(std::io::Error::other)
                    }).is_err());
                    assert_eq!(failed_row["raw_context"]["rip"], mov + 7);
                    assert_eq!(failed_row["raw_context"]["dr6"], c.Dr6);
                    assert_eq!(failed_row["thread_owner_pid"], pid);
                    assert_eq!(failed_row["pending_step"]["expected_rip"], mov + 8);
                    assert_eq!(failed_row["qualification_succeeded"], false);
                    assert!(failed_row["qualification_error"].as_str().unwrap().contains("single original instruction"));
                    assert_eq!(d.cleanup_event.as_ref().unwrap().1, DBG_EXCEPTION_NOT_HANDLED);
                    evidence["failed_qualification_raw_context_recorded"] = json!(true);
                    steps.get_mut(&tid).unwrap().expected = mov + 7;
                    let mut second_chance = event;
                    second_chance.u.Exception.dwFirstChance = 0;
                    assert!(classify_then_record(&mut d, &second_chance, &steps, &[mov], |_, row| {
                        assert_eq!(row["qualification_succeeded"], false); Ok(())
                    }).is_err());
                    let mut readonly = File::open(&trace_path).unwrap();
                    assert!(classify_then_record(&mut d, &event, &steps, &[mov], |d, row| {
                        assert_eq!(row["qualification_succeeded"], true);
                        assert_eq!(d.status_after_error(&event), DBG_CONTINUE);
                        readonly.write_all(b"real OS write refusal")
                    }).is_err());
                    assert_eq!(d.cleanup_event.as_ref().unwrap().1, DBG_CONTINUE);
                    assert_eq!(d.status_after_error(&event), DBG_CONTINUE);
                    evidence["record_failure_preserved_owned_continue"] = json!(true);
                    // Restore the interrupted fixture call before the existing
                    // debugger restores every thread's original DR set.
                    let mut resume = original.unwrap();
                    resume.Dr0 = c.Dr0; resume.Dr1 = c.Dr1; resume.Dr2 = c.Dr2; resume.Dr3 = c.Dr3; resume.Dr7 = c.Dr7;
                    resume.Dr6 &= !((1 << 14) | 15);
                    resume.EFlags &= !TF;
                    resume.EFlags |= RF;
                    set_context(&d.threads[&tid].handle, &resume).unwrap();
                    d.last_dispatch = DispatchOutcome::OwnedProven(OwnedTrap::PropertyAfter);
                    d.cleanup(Some((&event, DBG_CONTINUE))).unwrap();
                    assert_eq!(d.terminal, TerminalKind::Detached);
                    return evidence;
                }
                d.last_dispatch = DispatchOutcome::OwnedProven(OwnedTrap::PropertyBefore);
                d.continue_pending_with_status(DBG_CONTINUE).unwrap();
                continue;
            }
            let status = d.handle_event(&event).unwrap();
            d.continue_pending_with_status(status).unwrap();
            if d.ready { d.ready = false; ready_tx.send(()).unwrap(); }
        }
    });
    ready_rx.recv_timeout(Duration::from_secs(8)).unwrap();
    assert_eq!(child.command("T", "PROBE_OUT:"), "1");
    let mut receipt = observer.join().unwrap();
    assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
    assert!(!child.terminate().success());
    receipt["debug_registers_restored"] = json!(true);
    receipt["child_reaped"] = json!(true);
    println!("TODO880_STEP_RECEIPT={receipt}");
    drop(child);
    fs::remove_file(path).unwrap();
}
