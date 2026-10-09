use super::*;
// The 7193 DLL's guarded_install frame is 0xE44B0 bytes, per its PE unwind.
// Reproduce that caller budget without loading a game or the failed DLL.
core::arch::global_asm!(
    ".globl trace_init_budget_frame", ".seh_proc trace_init_budget_frame", "trace_init_budget_frame:",
    "push rbx", ".seh_pushreg rbx", "mov rbx,rcx", "mov eax,0xe44b0", "call __chkstk", "sub rsp,rax",
    ".seh_stackalloc 0xe44b0", ".seh_endprologue", "mov rcx,[rbx]", "call qword ptr [rbx+8]",
    "add rsp,0xe44b0", "pop rbx", "ret", ".seh_endproc",
    // Exact second prologue from fixed DLL RVA FAC0. Overflow occurs during
    // probing, before any constructor body. Stable across test optimizations.
    ".globl trace_init_legacy_frame", ".seh_proc trace_init_legacy_frame", "trace_init_legacy_frame:",
    "mov eax,0x42068", "call __chkstk", "sub rsp,rax", ".seh_stackalloc 0x42068", ".seh_endprologue",
    "add rsp,0x42068", "ret", ".seh_endproc"
);
unsafe extern "system" {fn trace_init_budget_frame(context:*const usize);fn trace_init_legacy_frame();fn SetErrorMode(flags:u32)->u32;}
unsafe extern "system" fn fixed_initializer(target:*mut JoinedTrace) {
    unsafe {JoinedTrace::initialize_at(target,BridgeMode::ProductionAdmission);}
}
#[test]
fn trace_init_game_stack_child() {
    let Ok(mode)=std::env::var("JAC_TRACE_STACK_CHILD") else {return};
    unsafe {SetErrorMode(2)};
    std::thread::Builder::new().stack_size(1<<20).spawn(move|| {
        let mut out=Box::<JoinedTrace>::new_uninit();
        let target=if mode=="legacy" {trace_init_legacy_frame as *const () as usize}else{fixed_initializer as *const () as usize};
        let context=[out.as_mut_ptr() as usize,target];
        unsafe {trace_init_budget_frame(context.as_ptr());}
        assert_ne!(mode,"legacy","frozen oversized prologues unexpectedly returned");
        let out=unsafe {out.assume_init()};
        assert_eq!(out.calibration.magic,CALIBRATION_TRACE_MAGIC);
        assert_eq!(out.property.scope,BridgeMode::ProductionAdmission as u32);
        assert_eq!(out.property.rows[TRACE_CAPACITY-1].sequence.load(Ordering::Relaxed),0);
    }).unwrap().join().unwrap();
}
fn run_child(mode:&str)->std::process::Output {
    use std::os::windows::process::CommandExt;
    let mut child=std::process::Command::new(std::env::current_exe().unwrap()).creation_flags(0x08000000)
        .args(["--exact","windows::trace_init::tests::trace_init_game_stack_child","--nocapture"])
        .env("JAC_TRACE_STACK_CHILD",mode).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now()>=deadline {child.kill().unwrap();child.wait().unwrap();panic!("owned trace initialization child timeout");}
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}
#[test]
fn trace_init_fits_game_stack_where_legacy_value_constructor_overflows() {
    let legacy=run_child("legacy");
    println!("legacy_child_status={:?} stderr={}",legacy.status.code(),String::from_utf8_lossy(&legacy.stderr));
    assert_eq!(legacy.status.code().map(|n|n as u32),Some(0xc00000fd));
    let fixed=run_child("fixed");
    assert!(fixed.status.success(),"{} {}",String::from_utf8_lossy(&fixed.stdout),String::from_utf8_lossy(&fixed.stderr));
}
#[test]
fn trace_init_headers_and_complete_empty_rows_match_contract() {
    let mut trace=Box::<CompositeTrace>::new_uninit();
    unsafe {ptr::write_bytes(trace.as_mut_ptr().cast::<u8>(),0xa5,std::mem::size_of::<CompositeTrace>());CompositeTrace::initialize_at(trace.as_mut_ptr());}
    let trace=unsafe {trace.assume_init()};
    assert_eq!(trace.joined.calibration.magic,CALIBRATION_TRACE_MAGIC);
    assert_eq!(trace.joined.property.magic,PROPERTY_TRACE_MAGIC);
    assert_eq!(trace.joined.property.scope,BridgeMode::ProductionAdmission as u32);
    assert_eq!(trace.joined.property.filtered.load(Ordering::Relaxed),0);
    assert_eq!(trace.joined.calibration.reserved,0);
    for row in &trace.joined.property.rows {
        assert_eq!(row.sequence.load(Ordering::Relaxed),0);assert_eq!(row.payload.load(Ordering::Relaxed),0);
    }
    for row in &trace.joined.calibration.rows {
        assert_eq!(row.sequence.load(Ordering::Relaxed),0);assert_eq!(row.entry_ticket.load(Ordering::Relaxed),0);
    }
    assert!(trace.effective_rax.iter().all(|v|v.load(Ordering::Relaxed)==0));
    assert!(trace.joined.property.complete_rows().unwrap().is_empty());
}
