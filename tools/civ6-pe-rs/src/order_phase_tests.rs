//! Real owned OS fixture for the production phase barrier and thread ledger.
use super::super::test_support::{AttachReadyBarrier, Fixture};
use super::*;
use std::collections::btree_map::Entry;

core::arch::global_asm!(
    ".text",
    ".globl todo880_phase_spare_a",
    "todo880_phase_spare_a:",
    "nop",
    "ret",
    ".globl todo880_phase_spare_b",
    "todo880_phase_spare_b:",
    "nop",
    "ret",
    ".globl todo880_phase_spare_c",
    "todo880_phase_spare_c:",
    "nop",
    "ret",
    ".globl todo880_phase_spare_d",
    "todo880_phase_spare_d:",
    "nop",
    "ret",
);
extern "C" {
    fn todo880_mov_once();
    fn todo880_phase_spare_a();
    fn todo880_phase_spare_b();
    fn todo880_phase_spare_c();
    fn todo880_phase_spare_d();
}

/// Fixed phase: sample DRs after real CREATE_THREAD arming, at the new thread's
/// first target call, and at a later suspended idle point. No register tampering
/// or delayed exit bookkeeping is injected in this independent case.
#[test]
fn order_capture_fixed_thread_lifecycle_retains_debug_registers() {
    fixed_thread_lifecycle(false);
}

#[test]
fn order_capture_stop_snapshot_detects_owned_os_register_erasure() {
    fixed_thread_lifecycle(true);
}

fn fixed_thread_lifecycle(erase_registers: bool) {
    let mut child = Fixture::start();
    let pid = child.child.id();
    let entry = child.entry;
    let path = std::env::temp_dir().join(format!(
        "todo880-lifecycle-{}-{pid}.jsonl",
        std::process::id()
    ));
    let output = Jsonl::new(&path).unwrap();
    let (stop_tx, _) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let observer = child.observe(&stop_tx, move |ack| {
        let mut d = Debugger::attach(pid, process(pid,true).unwrap(), entry, 1, output, ProbeMode::Lifecycle).unwrap();
        d.mode = ProbeMode::Property;
        d.close_ack = ack;
        let mut ready_barrier = AttachReadyBarrier::default();
        let epoch = CaptureEpoch { phase:Phase::Load, generation:0, sites:[Some(entry),None,None,None] };
        let mut ledger = CaptureThreads::default();
        let steps = BTreeMap::new();
        let mut rows = Vec::new();
        let mut after_ready = false;
        let mut fresh_threads = BTreeSet::new();
        let mut fresh_hits = BTreeSet::new();
        let mut exit_tids = BTreeSet::new();
        let mut hits = 0;
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            assert!(Instant::now() < deadline, "owned lifecycle absolute deadline");
            if finish_rx.try_recv().is_ok() {
                d.freeze().unwrap();
                for tid in d.threads.keys().copied().collect::<Vec<_>>() {
                    let (mut row,c,wait) = arm_evidence(&d,tid,Some(epoch),epoch,&ledger.generations);
                    row["sample_stage"] = json!("suspended_after_calls");
                    row["registers_match"] = json!(wait==WAIT_TIMEOUT && c.as_ref().is_ok_and(|c| installed_sites_match_context(&epoch.sites,c)));
                    rows.push(row);
                }
                let erased_tid = if erase_registers {
                    let tid = *d.held.iter().next().expect("owned live fixture thread");
                    let handle = &d.threads[&tid].handle;
                    let mut c = context(handle).unwrap();
                    c.Dr0=0;c.Dr1=0;c.Dr2=0;c.Dr3=0;c.Dr6=0;c.Dr7=0;
                    set_context(handle,&c).unwrap();
                    let actual=context(handle).unwrap();
                    assert_eq!([actual.Dr0,actual.Dr1,actual.Dr2,actual.Dr3,actual.Dr7],[0;5]);
                    Some(tid)
                } else {None};
                let audit = stop_register_evidence(&d,epoch,&ledger,&steps);
                mark(&mut d,audit.clone()).unwrap();
                assert_eq!(audit["summary"]["mismatched"],u64::from(erase_registers));
                assert_eq!(audit["summary"]["coverage_over_interval"],"unknown");
                assert_eq!(audit["summary"]["zero_hits_proves_unreached"],false);
                if let Some(tid)=erased_tid {
                    let row=audit["thread_checks"].as_array().unwrap().iter().find(|r|r["tid"]==tid).unwrap();
                    assert_eq!(row["disposition"],"debug_register_mismatch");
                    assert_eq!(row["last_written_generation"],0);
                    assert_eq!(row["held"],true);
                    assert_eq!(row["raw_context"]["dr0"],0);
                    assert_eq!(row["raw_context"]["dr7"],0);
                    // The diagnostic itself must not rearm the erased thread.
                    let after=context(&d.threads[&tid].handle).unwrap();
                    assert_eq!([after.Dr0,after.Dr1,after.Dr2,after.Dr3,after.Dr7],[0;5]);
                }
                d.cleanup(None).unwrap();
                assert_eq!(d.terminal,TerminalKind::Detached);
                return json!({"kind":"owned_os_fixed_thread_lifecycle","pid":pid,"phase_changes":0,"register_reset_injected":erase_registers,"erased_tid":erased_tid,"stop_register_audit":audit,"exit_delay_injected":false,"hits":hits,"fresh_thread_ids":fresh_threads,"fresh_hit_ids":fresh_hits,"exit_tids":exit_tids,"samples":rows,"restored":true,"game_live":false});
            }
            let mut event = DEBUG_EVENT::default();
            if unsafe { WaitForDebugEventEx(&mut event,20) } == 0 {continue;}
            assert_eq!(event.dwProcessId,pid);
            d.cleanup_event=Some((event,DBG_EXCEPTION_NOT_HANDLED));
            d.last_dispatch=DispatchOutcome::Unrelated;
            d.last_attach_breakpoint_owned=false;
            let tid=event.dwThreadId;
            let created=matches!(event.dwDebugEventCode,CREATE_PROCESS_DEBUG_EVENT|CREATE_THREAD_DEBUG_EVENT);
            if created && after_ready {fresh_threads.insert(tid);}
            if event.dwDebugEventCode==EXIT_THREAD_DEBUG_EVENT {exit_tids.insert(tid);}
            if capture_thread_event(&mut d,&event,epoch,&mut ledger,&steps).unwrap() {
                if created {
                    let (mut row,c,_) = arm_evidence(&d,tid,Some(epoch),epoch,&ledger.generations);
                    row["sample_stage"] = json!("create_event_after_set_readback");
                    row["after_attach_ready"] = json!(after_ready);
                    row["registers_match"] = json!(installed_sites_match_context(&epoch.sites,&c.unwrap()));
                    rows.push(row);
                }
                d.continue_pending_with_status(DBG_CONTINUE).unwrap();
                if ready_barrier.advance(&mut d,&event) {after_ready=true;ready_tx.send(()).unwrap();}
                continue;
            }
            if event.dwDebugEventCode==EXCEPTION_DEBUG_EVENT {
                let info=unsafe {event.u.Exception};
                classify_then_record(&mut d,&event,&steps,&[entry],|d,row|mark(d,row).map_err(std::io::Error::other)).unwrap();
                if info.ExceptionRecord.ExceptionCode==EXCEPTION_SINGLE_STEP {
                    let (mut row,c,_) = arm_evidence(&d,tid,Some(epoch),epoch,&ledger.generations);
                    let mut c=c.unwrap();
                    assert_eq!(c.Rip,entry);
                    row["sample_stage"] = json!("target_entry_after_thread_initialization");
                    row["registers_match"] = json!(installed_sites_match_context(&epoch.sites,&c));
                    rows.push(row);
                    hits+=1;
                    if fresh_threads.contains(&tid){fresh_hits.insert(tid);}
                    c.EFlags |= RF; c.Dr6 &= !0xf;
                    set_context(&d.threads[&tid].handle,&c).unwrap();
                    d.continue_pending_with_status(DBG_CONTINUE).unwrap();continue;
                }
                if info.ExceptionRecord.ExceptionCode==EXCEPTION_BREAKPOINT && d.last_attach_breakpoint_owned {
                    d.mark_attach_ready().unwrap();
                    d.continue_pending_with_status(DBG_CONTINUE).unwrap();
                    assert!(!ready_barrier.advance(&mut d,&event));continue;
                }
                let status=d.status_after_error(&event);
                d.continue_pending_with_status(status).unwrap();continue;
            }
            let status=d.handle_event(&event).unwrap();
            d.continue_pending_with_status(status).unwrap();
        }
    });
    ready_rx.recv_timeout(Duration::from_secs(8)).unwrap();
    assert_eq!(child.command("T", "PROBE_OUT:"), "1");
    assert_eq!(child.command("M", "PROBE_OUT:"), "M10");
    assert_eq!(child.command("T", "PROBE_OUT:"), "1");
    finish_tx.send(()).unwrap();
    let mut receipt = observer.join().unwrap();
    assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
    assert!(!child.terminate().success());
    receipt["child_reaped"] = json!(true);
    drop(child);
    fs::remove_file(path).unwrap();
    println!("TODO880_THREAD_LIFECYCLE_RECEIPT={receipt}");
    assert_eq!(receipt["hits"], 4);
    assert_eq!(receipt["fresh_hit_ids"].as_array().unwrap().len(), 2);
    for tid in receipt["fresh_hit_ids"].as_array().unwrap() {
        assert!(receipt["exit_tids"].as_array().unwrap().contains(tid));
    }
    for row in receipt["samples"].as_array().unwrap() {
        if row["handle_wait"] == WAIT_TIMEOUT {
            assert_eq!(row["registers_match"], true, "{row}");
        }
    }
}

#[test]
fn order_capture_real_phase_threads_and_pending_step() {
    let mut child = Fixture::start();
    let pid = child.child.id();
    let entry = child.entry;
    let exe = std::env::current_exe().unwrap();
    let local = unsafe { GetModuleHandleW(std::ptr::null()) } as u64;
    let module = module_named(pid, exe.file_name().unwrap().to_str().unwrap()).unwrap();
    let remote = module.base;
    let addresses = [
        todo880_mov_once as *const () as u64,
        todo880_phase_spare_a as *const () as u64,
        todo880_phase_spare_b as *const () as u64,
        todo880_phase_spare_c as *const () as u64,
        todo880_phase_spare_d as *const () as u64,
    ]
    .map(|a| remote + a - local);
    let mov = addresses[0];
    let front = [
        Some(entry),
        Some(mov),
        Some(addresses[1]),
        Some(addresses[2]),
    ];
    let load = [
        Some(entry),
        Some(mov),
        Some(addresses[3]),
        Some(addresses[4]),
    ];
    let path =
        std::env::temp_dir().join(format!("todo880-phase-{}-{pid}.jsonl", std::process::id()));
    let output = Jsonl::new(&path).unwrap();
    let trace_path = path.clone();
    let (stop_tx, _) = mpsc::channel();
    let (control_tx, control_rx) = mpsc::channel::<&'static str>();
    let (notice_tx, notice_rx) = mpsc::channel();
    let observer=child.observe(&stop_tx,move |ack| {
        let mut d=Debugger::attach(pid,process(pid,true).unwrap(),remote,module.size,output,ProbeMode::Lifecycle).unwrap();
        d.mode=ProbeMode::Property;
        d.close_ack=ack;
        let mut ready_barrier=AttachReadyBarrier::default();
        for address in front.into_iter().chain(load).flatten(){executable(&d.game,remote,address,1).unwrap();}
        let mut threads=CaptureThreads::default();
        let mut epoch=CaptureEpoch{phase:Phase::Front,generation:0,sites:front};
        let mut desired=Phase::Front;
        let mut steps=BTreeMap::new();
        let mut original=BTreeMap::new();
        let mut delayed_exit=Vec::<DEBUG_EVENT>::new();
        let mut completed=0;
        let mut load_hits=0;
        let mut deferred=0;
        let mut switches=Vec::new();
        let mut mismatch_proof=Value::Null;
        let deadline=Instant::now()+Duration::from_secs(20);
        loop {
            assert!(Instant::now()<deadline,"owned phase fixture absolute deadline");
            for command in control_rx.try_iter(){match command{
                "front"=>desired=Phase::Front,
                "stop"=>{
                    assert!(steps.is_empty());
                    d.cleanup(None).unwrap();
                    assert_eq!(d.terminal,TerminalKind::Detached);
                    return json!({"kind":"owned_os_phase_fixture","pid":pid,"phase_switches":switches,"completed_mov":completed,"load_worker_hits":load_hits,"pending_barrier_deferred":deferred,"created_events":threads.created,"exit_events":threads.exited,"retirement_bookkeeping_delay_injected":true,"live_mismatch_rejection":mismatch_proof,"restored":true,"game_live":false});
                },
                _=>panic!("unexpected fixture command"),
            }}
            let mut event=DEBUG_EVENT::default();
            if unsafe{WaitForDebugEventEx(&mut event,20)}==0 {
                assert_eq!(std::io::Error::last_os_error().raw_os_error(),Some(121));
                if desired!=epoch.phase {
                    let next=CaptureEpoch{phase:desired,generation:epoch.generation+1,sites:if desired==Phase::Front{front}else{load}};
                    match phase_barrier(&mut d,&steps,epoch,next,&mut threads).unwrap(){
                        PhaseBarrier::Deferred=>{deferred+=1;continue;},
                        PhaseBarrier::Event(pending)=>event=pending,
                        PhaseBarrier::Switched(checks)=>{
                            if next.generation==1 {
                                let before=d.held.iter().map(|tid|(*tid,context(&d.threads[tid].handle).unwrap())).collect::<BTreeMap<_,_>>();
                                let tid=*before.keys().next_back().unwrap();
                                let mut altered=before[&tid]; altered.Dr0+=1;
                                set_context(&d.threads[&tid].handle,&altered).unwrap();
                                let hypothetical=CaptureEpoch{phase:Phase::Front,generation:2,sites:front};
                                assert!(rearm_frozen_threads(&mut d,&steps,next,hypothetical,&mut threads).unwrap_err().contains("prior capture debug-register ownership changed"));
                                for (other,old) in &before {
                                    if *other==tid {continue;}
                                    let actual=context(&d.threads[other].handle).unwrap();
                                    assert_eq!([actual.Dr0,actual.Dr1,actual.Dr2,actual.Dr3,actual.Dr7],[old.Dr0,old.Dr1,old.Dr2,old.Dr3,old.Dr7]);
                                }
                                mismatch_proof=fs::read_to_string(&trace_path).unwrap().lines().filter_map(|line|serde_json::from_str::<Value>(line).ok()).rfind(|row|row["type"]=="arm_rejected").unwrap();
                                assert_eq!(mismatch_proof["tid"],tid);
                                assert_eq!(mismatch_proof["raw_context"]["dr0"],altered.Dr0);
                                assert_eq!(mismatch_proof["expected_generation"],1);
                                assert_eq!(mismatch_proof["held"],true);
                                assert_eq!(mismatch_proof["handle_wait"],WAIT_TIMEOUT);
                                set_context(&d.threads[&tid].handle,&before[&tid]).unwrap();
                            }
                            if next.phase==Phase::Front {
                                assert!(!delayed_exit.is_empty());
                                for exit in &delayed_exit {
                                    let row=checks.iter().find(|r|r["tid"]==exit.dwThreadId).unwrap();
                                    assert_eq!(row["handle_wait"],WAIT_OBJECT_0);
                                    assert_eq!(row["disposition"],"signaled_handle_awaiting_exit_event");
                                }
                            }
                            switches.push(json!({"from":epoch.phase.name(),"to":next.phase.name(),"generation":next.generation,"thread_checks":checks}));
                            epoch=next;
                            // Controlled delay of ledger retirement only. These
                            // are REAL exit events, already continued exactly once;
                            // retaining their handles exercises the terminal window.
                            for exit in delayed_exit.drain(..){capture_thread_event(&mut d,&exit,epoch,&mut threads,&steps).unwrap();}
                            d.resume().unwrap();
                            notice_tx.send(epoch.phase.name()).unwrap();
                            continue;
                        }
                    }
                }else{continue;}
            }
            d.event_ordinal+=1;
            d.last_dispatch=DispatchOutcome::Unrelated;
            d.cleanup_event=Some((event,DBG_EXCEPTION_NOT_HANDLED));
            assert_eq!(event.dwProcessId,pid);
            let tid=event.dwThreadId;
            if event.dwDebugEventCode==EXIT_THREAD_DEBUG_EVENT && epoch.phase==Phase::Load {
                assert!(!steps.contains_key(&tid));
                assert!(d.threads.contains_key(&tid));
                // Negative ledger fault injection on a REAL exit event. It is
                // not a claim that this exiting worker executed a pending MOV.
                steps.insert(tid,Step{site:Site{name:"missing_fixture_step",rva:0,bytes:&[],next:plan::Next::Sequential},expected:mov+7,armed_eflags:TF,capture:json!({"capture_seq":99}),phase:epoch.phase});
                assert!(capture_thread_event(&mut d,&event,epoch,&mut threads,&steps).unwrap_err().contains("before original instruction completion"));
                assert!(steps.contains_key(&tid));
                assert!(d.threads.contains_key(&tid));
                steps.remove(&tid);
                delayed_exit.push(event);
                d.continue_pending_with_status(DBG_CONTINUE).unwrap();
                continue;
            }
            if capture_thread_event(&mut d,&event,epoch,&mut threads,&steps).unwrap(){
                d.continue_pending_with_status(DBG_CONTINUE).unwrap();
                if ready_barrier.advance(&mut d,&event) {notice_tx.send("ready").unwrap();}
                continue;
            }
            if event.dwDebugEventCode==EXCEPTION_DEBUG_EVENT {
                let ex=unsafe{event.u.Exception};
                let sites=epoch.sites.into_iter().flatten().collect::<Vec<_>>();
                classify_then_record(&mut d,&event,&steps,&sites,|d,row|mark(d,row).map_err(std::io::Error::other)).unwrap();
                if ex.ExceptionRecord.ExceptionCode==EXCEPTION_SINGLE_STEP {
                    let mut c=context(&d.threads[&tid].handle).unwrap();
                    match steps.entry(tid) {
                    Entry::Occupied(step) => {
                        let prior=original.remove(&tid).unwrap();
                        c.Rip=prior;
                        c.EFlags=(c.EFlags & !TF)|RF;
                        c.Dr6 &= !((1<<14)|15);
                        set_context(&d.threads[&tid].handle,&c).unwrap();
                        step.remove();
                        completed+=1;
                    }
                    Entry::Vacant(_) if c.Rip==entry => {
                        if epoch.phase==Phase::Load {load_hits+=1;c.EFlags|=RF;}
                        else {original.insert(tid,entry);c.Rip=mov;c.EFlags &= !RF;}
                        c.Dr6 &= !((1<<14)|15);
                        set_context(&d.threads[&tid].handle,&c).unwrap();
                    }
                    Entry::Vacant(step) => {
                        assert_eq!(c.Rip,mov);
                        c.EFlags|=TF|RF;c.Dr6 &= !((1<<14)|15);
                        set_context(&d.threads[&tid].handle,&c).unwrap();
                        let armed=context(&d.threads[&tid].handle).unwrap();
                        step.insert(Step{site:Site{name:"owned_fixture_mov",rva:mov-remote,bytes:&[],next:plan::Next::Sequential},expected:mov+7,armed_eflags:armed.EFlags,capture:json!({"capture_seq":completed+1}),phase:epoch.phase});
                        let next=CaptureEpoch{phase:Phase::Load,generation:epoch.generation+1,sites:load};
                        assert!(matches!(phase_barrier(&mut d,&steps,epoch,next,&mut threads).unwrap(),PhaseBarrier::Deferred));
                        deferred+=1;
                        if epoch.generation==0 {desired=Phase::Load;}
                    }
                    }
                    d.continue_pending_with_status(DBG_CONTINUE).unwrap();
                    if !steps.is_empty() {
                        assert!(d.cleanup_event.is_none());
                        let next=CaptureEpoch{phase:Phase::Load,generation:epoch.generation+1,sites:load};
                        assert!(matches!(phase_barrier(&mut d,&steps,epoch,next,&mut threads).unwrap(),PhaseBarrier::Deferred));
                        deferred+=1;
                    }
                    continue;
                }
                if ex.ExceptionRecord.ExceptionCode==EXCEPTION_BREAKPOINT && d.last_attach_breakpoint_owned {
                    d.mark_attach_ready().unwrap();
                    d.continue_pending_with_status(DBG_CONTINUE).unwrap();
                    assert!(!ready_barrier.advance(&mut d,&event));continue;
                }
                d.continue_pending_with_status(d.status_after_error(&event)).unwrap();continue;
            }
            let status=d.handle_event(&event).unwrap();
            d.continue_pending_with_status(status).unwrap();
        }
    });
    assert_eq!(
        notice_rx.recv_timeout(Duration::from_secs(8)).unwrap(),
        "ready"
    );
    assert_eq!(child.command("T", "PROBE_OUT:"), "1");
    assert_eq!(
        notice_rx.recv_timeout(Duration::from_secs(8)).unwrap(),
        "load"
    );
    assert_eq!(child.command("M", "PROBE_OUT:"), "M10");
    control_tx.send("front").unwrap();
    assert_eq!(
        notice_rx.recv_timeout(Duration::from_secs(8)).unwrap(),
        "front"
    );
    assert_eq!(child.command("T", "PROBE_OUT:"), "1");
    control_tx.send("stop").unwrap();
    let mut receipt = observer.join().unwrap();
    assert_eq!(receipt["completed_mov"], 2);
    assert_eq!(receipt["load_worker_hits"], 2);
    assert_eq!(receipt["phase_switches"].as_array().unwrap().len(), 2);
    assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
    assert!(!child.terminate().success());
    receipt["child_reaped"] = json!(true);
    println!("TODO880_PHASE_RECEIPT={receipt}");
    drop(child);
    fs::remove_file(path).unwrap();
}
