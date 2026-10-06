//! A real RaiseException in the existing owned Rust fixture. The baseline
//! forwards first chance, observes second chance, then rescues the same child.
use super::super::test_support::Fixture;
use super::*;
use windows_sys::Win32::System::Diagnostics::Debug::RaiseException;

#[inline(never)]
extern "C-unwind" fn naming_target(_: u64, _: u32) -> u8 {
    let name = b"todo880-owned-thread\0";
    // The high DWORD is padding; the Civ6 producer initializes only dwType.
    let args = [
        0x7654321000001000usize,
        name.as_ptr() as usize,
        u32::MAX as usize,
    ];
    unsafe {
        RaiseException(THREAD_NAME_EXCEPTION as u32, 0, 3, args.as_ptr());
    }
    1
}

#[inline(never)]
extern "C-unwind" fn naming_target_legacy(_: u64, _: u32) -> u8 {
    let name = b"Bink IO\0";
    let tid = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
    // Reproduce the observed six-word API call without the real producer's
    // out-of-structure stack read: all six source words are initialized here.
    let args = [
        0x1000usize,
        name.as_ptr() as usize,
        tid as usize,
        0,
        0x1234,
        0x5678,
    ];
    unsafe {
        RaiseException(THREAD_NAME_EXCEPTION as u32, 0, 6, args.as_ptr());
    }
    1
}

#[test]
fn order_capture_thread_name_real_second_chance_and_fixed_continue() {
    for (baseline, parameter_count) in [(true, 3), (false, 3), (true, 6), (false, 6)] {
        let mut child = Fixture::start();
        let pid = child.child.id();
        let base = child.entry - CAN_EVER_RVA;
        let local = unsafe { GetModuleHandleW(std::ptr::null()) } as u64;
        let exe = std::env::current_exe().unwrap();
        let remote = module_named(pid, exe.file_name().unwrap().to_str().unwrap())
            .unwrap()
            .base;
        let function = if parameter_count == 3 {
            naming_target as *const ()
        } else {
            naming_target_legacy as *const ()
        };
        let target = remote + function as u64 - local;
        let path = std::env::temp_dir().join(format!(
            "todo880-threadname-{}-{pid}.jsonl",
            std::process::id()
        ));
        let trace = path.clone();
        let output = Jsonl::new(&path).unwrap();
        let (stop_tx, _) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let observer=child.observe(&stop_tx,move|ack|{
            let mut d=Debugger::attach(pid,process(pid,true).unwrap(),base,0x500000,output,ProbeMode::Governor).unwrap();
            d.close_ack=ack;
            let mut first=0;
            let mut second=0;
            let deadline=Instant::now()+Duration::from_secs(20);
            loop {
                assert!(Instant::now()<deadline,"owned naming event deadline");
                let mut event=DEBUG_EVENT::default();
                if unsafe{WaitForDebugEventEx(&mut event,50)}==0 {continue;}
                d.cleanup_event=Some((event,DBG_EXCEPTION_NOT_HANDLED));
                d.last_dispatch=DispatchOutcome::Unrelated;
                let tid=event.dwThreadId;
                if event.dwDebugEventCode==EXCEPTION_DEBUG_EVENT {
                    let info=unsafe{event.u.Exception};
                    if info.ExceptionRecord.ExceptionCode==EXCEPTION_SINGLE_STEP {
                        let mut c=context(&d.threads[&tid].handle).unwrap();
                        assert_eq!(c.Rip,base+CAN_EVER_RVA);
                        // Only this owned test child is redirected, to compiled
                        // code with the same ABI; executable bytes are untouched.
                        c.Rip=target;c.EFlags|=RF;c.Dr6 &= !0xf;
                        set_context(&d.threads[&tid].handle,&c).unwrap();
                        d.continue_pending_with_status(DBG_CONTINUE).unwrap();continue;
                    }
                    if info.ExceptionRecord.ExceptionCode==THREAD_NAME_EXCEPTION {
                        if info.dwFirstChance!=0 {first+=1;}else{second+=1;}
                        if baseline && first==1 && second==0 {
                            assert_eq!(d.status_after_error(&event),DBG_EXCEPTION_NOT_HANDLED);
                            let row=exception_evidence(&d,&event,&BTreeMap::new());
                            mark(&mut d,row).unwrap();
                            d.continue_pending_with_status(DBG_EXCEPTION_NOT_HANDLED).unwrap();continue;
                        }
                        let mut rejected=0;
                        // Negative mutations of a REAL OS event; no claim these
                        // malformed payloads were raised by the game or fixture.
                        for mutation in 0..10 {
                            let mut invalid=event; let mut invalid_info=info;
                            match mutation {
                                0=>invalid_info.ExceptionRecord.NumberParameters=4,
                                1=>invalid_info.ExceptionRecord.ExceptionFlags=1,
                                2=>invalid_info.ExceptionRecord.ExceptionInformation[0]=0x1001,
                                3=>invalid_info.ExceptionRecord.ExceptionInformation[1]=0,
                                4=>invalid_info.ExceptionRecord.ExceptionInformation[1]=1,
                                5=>invalid_info.ExceptionRecord.ExceptionInformation[2]=1usize<<32,
                                6=>invalid_info.ExceptionRecord.ExceptionInformation[2]=0,
                                7=>invalid_info.ExceptionRecord.ExceptionCode=0x12345678,
                                8=>invalid_info.ExceptionRecord.ExceptionFlags=2,
                                _=>invalid.dwProcessId=pid.wrapping_add(1),
                            }
                            invalid.u.Exception=invalid_info;
                            let status=qualify_exception(&mut d,&invalid,&BTreeMap::new(),&[]).unwrap();
                            d.cleanup_event=Some((event,DBG_EXCEPTION_NOT_HANDLED));
                            assert_eq!(status,DBG_EXCEPTION_NOT_HANDLED);rejected+=1;
                        }
                        let mut explicit=event;
                        let mut explicit_info=info; explicit_info.ExceptionRecord.ExceptionInformation[2]=tid as usize; explicit.u.Exception=explicit_info;
                        assert!(thread_name_notice(&d,&explicit).is_ok());
                        let make_step=||Step{site:plan::FRONT[0],expected:target,armed_eflags:TF,capture:json!({"capture_seq":1}),phase:Phase::Front};
                        let steps=BTreeMap::from([(tid,make_step())]);
                        assert!(qualify_exception(&mut d,&event,&steps,&[]).is_err());
                        assert_eq!(steps.len(),1);
                        assert!(matches!(d.last_dispatch,DispatchOutcome::Ambiguous(_)));
                        d.last_dispatch=DispatchOutcome::Unrelated;
                        let other=d.threads.keys().copied().find(|t|*t!=tid).unwrap();
                        let steps=BTreeMap::from([(other,make_step())]);
                        assert_eq!(qualify_exception(&mut d,&event,&steps,&[]).unwrap(),DBG_CONTINUE);
                        assert_eq!(steps.len(),1);
                        let mut evidence=Value::Null;
                        let mut readonly=File::open(&trace).unwrap();
                        let recorded=classify_then_record(&mut d,&event,&BTreeMap::new(),&[],|d,row|{
                            assert_eq!(d.cleanup_event.as_ref().unwrap().1,DBG_CONTINUE);
                            evidence=row.clone();
                            if baseline {mark(d,row).map_err(std::io::Error::other)}else{readonly.write_all(b"actual denied write")}
                        });
                        assert_eq!(recorded.is_ok(),baseline);
                        assert_eq!(evidence["number_parameters"],parameter_count);
                        assert_eq!(evidence["exception_information"].as_array().unwrap().len(),parameter_count as usize);
                        if parameter_count==6 {
                            assert_eq!(evidence["thread_name_notice"]["trailing_words_uninterpreted"],json!([0,0x1234,0x5678]));
                            assert_eq!(evidence["thread_name_notice"]["name_bytes"],json!(b"Bink IO"));
                        }
                        assert_eq!(evidence["thread_name_notice"]["target_tid"],tid);
                        assert_eq!(evidence["continue_status"],DBG_CONTINUE);
                        cleanup_order(&mut d).unwrap();
                        assert_eq!(d.terminal,TerminalKind::Detached);
                        return json!({"kind":"owned_RaiseException_thread_name","baseline_first_chance_forwarded":baseline,"first_chance":first,"second_chance":second,"exception":evidence,"invalid_event_mutations_rejected":rejected,"pending_ledger_injected_for_negative_check":true,"pending_same_thread_preserved":true,"pending_other_thread_preserved":true,"output_failure_injected":!baseline,"restored":true,"game_live":false});
                    }
                }
                let status=d.handle_event(&event).unwrap();
                d.continue_pending_with_status(status).unwrap();
                if d.ready {d.ready=false;ready_tx.send(()).unwrap();}
            }
        });
        ready_rx.recv_timeout(Duration::from_secs(8)).unwrap();
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        let mut receipt = observer.join().unwrap();
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        assert!(!child.terminate().success());
        receipt["child_reaped"] = json!(true);
        drop(child);
        fs::remove_file(path).unwrap();
        println!("TODO880_THREAD_NAME_RECEIPT={receipt}");
        assert_eq!(receipt["first_chance"], 1);
        assert_eq!(receipt["second_chance"], u64::from(baseline));
    }
}
